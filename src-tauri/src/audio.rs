use crate::settings;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use std::sync::mpsc;
use std::thread;

use rubato::{FftFixedIn, Resampler};
use tauri::{AppHandle, Emitter};

enum AudioCommand {
    Start,
    Stop(mpsc::Sender<Vec<f32>>),
    Cancel,
    #[allow(dead_code)]
    Terminate,
}

pub struct AudioRecorder {
    command_tx: Option<mpsc::Sender<AudioCommand>>,
    worker_thread: Option<thread::JoinHandle<()>>,
}

// cpal::Stream is not Send on macOS, but we are moving it into a dedicated thread,
// so the AudioRecorder struct itself just holds the channel sender, which is Send.
unsafe impl Send for AudioRecorder {}
unsafe impl Sync for AudioRecorder {}

impl AudioRecorder {
    pub fn new() -> Self {
        Self {
            command_tx: None,
            worker_thread: None,
        }
    }
    
    pub fn init(&mut self, app: AppHandle) -> Result<(), String> {
        if self.command_tx.is_some() {
            return Ok(());
        }

        println!("🎤 AudioRecorder: Initializing worker thread...");
        let (cmd_tx, cmd_rx) = mpsc::channel();
        self.command_tx = Some(cmd_tx);

        let app_clone = app.clone();
        
        self.worker_thread = Some(thread::spawn(move || {
            let host = cpal::default_host();
            
            // Get settings to find configured device
            let settings = settings::load_settings(&app_clone);
            
            // Check for clamshell mode
            let use_clamshell_mic = if let Ok(is_clamshell) = crate::clamshell::is_clamshell() {
                is_clamshell && settings.clamshell_microphone.is_some()
            } else {
                false
            };

            let input_device_id = if use_clamshell_mic {
                let name = settings.clamshell_microphone.as_ref().unwrap();
                println!("💻 Clamshell mode detected! Switching to: {}", name);
                name.clone()
            } else {
                settings.input_device_id
            };
            
            println!("🎤 Audio Thread: Looking for device: '{}'", input_device_id);

            // List all available devices for debugging
            if let Ok(devices) = host.input_devices() {
                println!("📋 Available Input Devices:");
                for (i, d) in devices.enumerate() {
                    if let Ok(name) = d.name() {
                        println!("   {}. '{}'", i + 1, name);
                    }
                }
            }

            let device = if input_device_id == "default" {
                host.default_input_device()
            } else {
                // Try to find the device by ID (name)
                match host.input_devices() {
                    Ok(mut devices) => {
                        devices.find(|d| {
                            if let Ok(name) = d.name() {
                                // Exact match to align with example code and prevent fuzzy match errors
                                name == input_device_id
                            } else {
                                false
                            }
                        })
                    },
                    Err(_) => None,
                }
            };
            
            // Fallback to default if specific device not found
            let device = match device {
                Some(d) => {
                    if let Ok(name) = d.name() {
                        println!("✅ Audio Thread: Using device: {}", name);
                    }
                    d
                },
                None => {
                    println!("⚠️ Audio Thread: Configured device not found, using default");
                    match host.default_input_device() {
                        Some(d) => d,
                        None => {
                            eprintln!("❌ Audio Thread: No input device found");
                            return;
                        }
                    }
                }
            };
            
            let config = match device.default_input_config() {
                Ok(c) => c,
                Err(e) => {
                    eprintln!("❌ Audio Thread: Failed to get config: {}", e);
                    return;
                }
            };
            
            let input_rate = config.sample_rate().0;
            println!("✅ Audio Thread: Initialized at {} Hz", input_rate);
            
            // Channels for sending audio from callback to this thread's loop
            let (audio_tx, audio_rx) = mpsc::channel::<Vec<f32>>();
            
            let mut counter = 0;
            let stream = match device.build_input_stream(
                &config.into(),
                move |data: &[f32], _: &cpal::InputCallbackInfo| {
                    // Always calculate levels for UI
                    let mut sum = 0.0f32;
                    for &v in data {
                        sum += v * v;
                    }
                    let rms = (sum / data.len() as f32).sqrt();
                    let payload = vec![rms * 100.0];
                    let _ = app_clone.emit("mic-level", payload);
                    
                    counter += 1;
                    if counter % 50 == 0 {
                        // Log occasionally to confirm data flow
                         // println!("🎤 Audio Level: {:.4} ({} samples)", rms, data.len());
                    }

                    // Send data to main loop
                    if let Err(_) = audio_tx.send(data.to_vec()) {
                        // Main loop probably exited
                    }
                },
                |err| eprintln!("❌ Audio Thread: Stream error: {}", err),
                None,
            ) {
                Ok(s) => s,
                Err(e) => {
                    eprintln!("❌ Audio Thread: Failed to build stream: {}", e);
                    return;
                }
            };
            
            if let Err(e) = stream.play() {
                eprintln!("❌ Audio Thread: Failed to play stream: {}", e);
                return;
            }
            
            println!("✅ Audio Thread: Hot stream started");

            let mut is_recording = false;
            let mut recorded_samples: Vec<f32> = Vec::new();
            
            loop {
                // Check for commands (non-blocking)
                while let Ok(cmd) = cmd_rx.try_recv() {
                    match cmd {
                        AudioCommand::Start => {
                            is_recording = true;
                            recorded_samples.clear();
                            println!("🎤 Audio Thread: START recording command received");
                        },
                        AudioCommand::Stop(reply_tx) => {
                            is_recording = false;
                            println!("🛑 Audio Thread: STOP recording command received. Samples: {}", recorded_samples.len());
                            
                            // Resample if needed
                            let final_data = if input_rate == 16000 {
                                recorded_samples.clone()
                            } else {
                                resample_audio(&recorded_samples, input_rate, 16000)
                            };
                            
                            let _ = reply_tx.send(final_data);
                        },
                        AudioCommand::Cancel => {
                            is_recording = false;
                            recorded_samples.clear();
                            println!("🚫 Audio Thread: CANCEL recording command received");
                        },
                        AudioCommand::Terminate => {
                            println!("👋 Audio Thread: Terminating");
                            return; // Exit thread
                        }
                    }
                }
                
                // Read audio data
                while let Ok(data) = audio_rx.try_recv() {
                    if is_recording {
                        recorded_samples.extend(data);
                    }
                }
                
                thread::sleep(std::time::Duration::from_millis(5));
            }
        }));
        
        Ok(())
    }
    
    pub fn start(&mut self, app: AppHandle) -> Result<(), String> {
        if self.command_tx.is_none() {
            println!("🎤 Audio recorder not initialized, attempting init...");
            self.init(app.clone())?;
        }

        if let Some(tx) = &self.command_tx {
            tx.send(AudioCommand::Start).map_err(|e| e.to_string())?;
            Ok(())
        } else {
            Err("Audio recorder failed to initialize".to_string())
        }
    }
    
    pub fn stop(&mut self) -> Vec<f32> {
        if let Some(tx) = &self.command_tx {
            let (reply_tx, reply_rx) = mpsc::channel();
            if let Err(e) = tx.send(AudioCommand::Stop(reply_tx)) {
                eprintln!("Failed to send stop command: {}", e);
                return Vec::new();
            }
            
            match reply_rx.recv() {
                Ok(data) => data,
                Err(e) => {
                    eprintln!("Failed to receive audio data: {}", e);
                    Vec::new()
                }
            }
        } else {
            Vec::new()
        }
    }

    pub fn cancel(&mut self) {
        if let Some(tx) = &self.command_tx {
            if let Err(e) = tx.send(AudioCommand::Cancel) {
                eprintln!("Failed to send cancel command: {}", e);
            }
        }
    }

    pub fn reset(&mut self) {
        if let Some(tx) = &self.command_tx {
            let _ = tx.send(AudioCommand::Terminate);
        }
        if let Some(handle) = self.worker_thread.take() {
            // We won't block on join because it might take time, 
            // and we want the UI to be responsive.
            // The thread will exit on its own.
            // But we need to make sure we don't leak it?
            // Actually, joining is safer to ensure resource release.
            // But if the thread is stuck in audio callback...
            // cpal streams usually stop fine.
            let _ = handle.join();
        }
        self.command_tx = None;
        println!("🎤 AudioRecorder: Reset and ready for re-initialization");
    }
}

fn resample_audio(data: &[f32], input_rate: u32, output_rate: u32) -> Vec<f32> {
    if data.is_empty() { return Vec::new(); }
    
    // println!("🔄 Resampling from {} Hz to {} Hz", input_rate, output_rate);
    
    let chunk_size = 1024;
    let mut resampler = match FftFixedIn::<f32>::new(input_rate as usize, output_rate as usize, chunk_size, 1, 1) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("Failed to create resampler: {}", e);
            return data.to_vec(); 
        }
    };
    
    let mut output = Vec::with_capacity((data.len() as f64 * output_rate as f64 / input_rate as f64) as usize + 1024);
    let mut input_buffer = Vec::with_capacity(chunk_size);
    
    // Process in chunks
    for chunk in data.chunks(chunk_size) {
        input_buffer.clear();
        input_buffer.extend_from_slice(chunk);
        
        // Pad if needed
        if input_buffer.len() < chunk_size {
            input_buffer.resize(chunk_size, 0.0);
        }
        
        let waves_in = vec![input_buffer.clone()];
        match resampler.process(&waves_in, None) {
            Ok(waves_out) => {
                output.extend_from_slice(&waves_out[0]);
            },
            Err(e) => eprintln!("Resampling error: {}", e),
        }
    }
    
    output
}
