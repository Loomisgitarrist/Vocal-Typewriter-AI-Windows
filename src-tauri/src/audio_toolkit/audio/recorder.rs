use crate::audio_toolkit::constants::WHISPER_SAMPLE_RATE;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use std::sync::mpsc;
use std::thread;

#[derive(Clone)]
pub struct AudioRecorderConfig {
    pub sample_rate: u32,
    #[allow(dead_code)]
    pub channels: u16,
}

impl Default for AudioRecorderConfig {
    fn default() -> Self {
        Self {
            sample_rate: WHISPER_SAMPLE_RATE as u32,
            channels: 1,
        }
    }
}

enum AudioCommand {
    Start,
    Stop(mpsc::Sender<Vec<f32>>),
    Subscribe(mpsc::Sender<Vec<f32>>),
    #[allow(dead_code)]
    Cancel,
    Terminate,
}

pub struct AudioRecorder {
    command_tx: Option<mpsc::Sender<AudioCommand>>,
    worker_thread: Option<thread::JoinHandle<()>>,
    config: AudioRecorderConfig,
    level_callback: Option<std::sync::Arc<dyn Fn(Vec<f32>) + Send + Sync>>,
}

// cpal::Stream is not Send on macOS, but we are moving it into a dedicated thread,
// so the AudioRecorder struct itself just holds the channel sender, which is Send.
unsafe impl Send for AudioRecorder {}
unsafe impl Sync for AudioRecorder {}

#[allow(dead_code)]
impl AudioRecorder {
    pub fn new() -> Self {
        Self::with_config(AudioRecorderConfig::default())
    }

    pub fn with_config(config: AudioRecorderConfig) -> Self {
        Self {
            command_tx: None,
            worker_thread: None,
            config,
            level_callback: None,
        }
    }

    pub fn with_vad(self, _vad: Box<dyn crate::audio_toolkit::vad::Vad>) -> Self {
        // VAD implementation would go here
        // For now, we'll just return self as placeholder
        self
    }

    pub fn with_level_callback<F>(mut self, callback: F) -> Self
    where
        F: Fn(Vec<f32>) + Send + Sync + 'static,
    {
        self.level_callback = Some(std::sync::Arc::new(callback));
        self
    }

    pub fn subscribe(&self) -> Result<mpsc::Receiver<Vec<f32>>, String> {
        let (tx, rx) = mpsc::channel();
        if let Some(cmd_tx) = &self.command_tx {
            cmd_tx.send(AudioCommand::Subscribe(tx))
                .map_err(|e| format!("Failed to subscribe: {}", e))?;
            Ok(rx)
        } else {
            Err("Recorder not started".to_string())
        }
    }

    pub fn open(&mut self, device: Option<cpal::Device>) -> Result<(), String> {
        if self.command_tx.is_some() {
            return Ok(());
        }

        println!("🎤 AudioRecorder: Opening with device: {:?}", device.as_ref().and_then(|d| d.name().ok()));
        
        let (cmd_tx, cmd_rx) = mpsc::channel();
        self.command_tx = Some(cmd_tx);

        let config = self.config.clone();
        let level_callback = self.level_callback.clone();
        
        self.worker_thread = Some(thread::spawn(move || {
            let host = cpal::default_host();
            
            let device = match device {
                Some(d) => d,
                None => match host.default_input_device() {
                    Some(d) => d,
                    None => {
                        eprintln!("❌ AudioRecorder: No input device found");
                        return;
                    }
                }
            };

            let mut input_config = match device.default_input_config() {
                Ok(c) => c.into(),
                Err(e) => {
                    eprintln!("❌ AudioRecorder: Failed to get default input config: {}", e);
                    // Fallback to our hardcoded config if default fails
                    cpal::StreamConfig {
                        channels: 1,
                        sample_rate: cpal::SampleRate(16000),
                        buffer_size: cpal::BufferSize::Fixed(4096),
                    }
                }
            };
            
            // Ensure we use 1 channel
            input_config.channels = 1;
            input_config.buffer_size = cpal::BufferSize::Fixed(4096); // Force larger buffer

            let sample_rate = input_config.sample_rate.0;
            
            println!("✅ AudioRecorder: Using device '{}' at {} Hz", 
                device.name().unwrap_or_else(|_| "Unknown".to_string()),
                sample_rate);

            // Channels for sending audio from callback to this thread's loop
            let (audio_tx, audio_rx) = mpsc::channel::<Vec<f32>>();
            
            // Shared callback for the stream thread
            let callback = level_callback.clone();

            let stream = match device.build_input_stream(
                &input_config,
                move |data: &[f32], _: &cpal::InputCallbackInfo| {
                    // Calculate levels
                    if let Some(cb) = callback.as_ref() {
                         let mut sum = 0.0f32;
                         for &v in data {
                             sum += v * v;
                         }
                         let rms = (sum / data.len() as f32).sqrt();
                         cb(vec![rms * 100.0]);
                    }

                    // Send data to main loop
                    if let Err(_) = audio_tx.send(data.to_vec()) {
                        // Main loop probably exited
                    }
                },
                |err| eprintln!("❌ AudioRecorder: Stream error: {}", err),
                None,
            ) {
                Ok(s) => s,
                Err(e) => {
                    eprintln!("❌ AudioRecorder: Failed to build stream: {}", e);
                    return;
                }
            };
            
            if let Err(e) = stream.play() {
                eprintln!("❌ AudioRecorder: Failed to play stream: {}", e);
                return;
            }
            
            println!("✅ AudioRecorder: Stream started");

            let mut is_recording = false;
            let mut recorded_samples: Vec<f32> = Vec::new();
            let mut subscribers: Vec<mpsc::Sender<Vec<f32>>> = Vec::new();
            
            loop {
                // Check for commands (non-blocking)
                while let Ok(cmd) = cmd_rx.try_recv() {
                    match cmd {
                        AudioCommand::Start => {
                            is_recording = true;
                            recorded_samples.clear();
                            println!("🎤 AudioRecorder: START recording");
                        },
                        AudioCommand::Stop(reply_tx) => {
                            is_recording = false;
                            println!("🛑 AudioRecorder: STOP recording. Samples: {}", recorded_samples.len());
                            
                            // Resample if needed
                            let final_data = if sample_rate == config.sample_rate {
                                recorded_samples.clone()
                            } else {
                                resample_audio(&recorded_samples, sample_rate, config.sample_rate)
                            };
                            
                            let _ = reply_tx.send(final_data);
                        },
                        AudioCommand::Subscribe(tx) => {
                            subscribers.push(tx);
                            println!("🎧 AudioRecorder: New subscriber added");
                        },
                        AudioCommand::Cancel => {
                            is_recording = false;
                            recorded_samples.clear();
                            println!("🚫 AudioRecorder: CANCEL recording");
                        },
                        AudioCommand::Terminate => {
                            println!("👋 AudioRecorder: Terminating");
                            return;
                        }
                    }
                }
                
                // Read audio data
                while let Ok(data) = audio_rx.try_recv() {
                    // Forward to subscribers
                    subscribers.retain(|sub| {
                        sub.send(data.clone()).is_ok()
                    });

                    if is_recording {
                        recorded_samples.extend(data);
                    }
                }
                
                thread::sleep(std::time::Duration::from_millis(5));
            }
        }));
        
        Ok(())
    }
    
    pub fn start(&mut self) -> Result<(), String> {
        if let Some(tx) = &self.command_tx {
            tx.send(AudioCommand::Start).map_err(|e| e.to_string())?;
            Ok(())
        } else {
            Err("Audio recorder not initialized".to_string())
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

    pub fn close(&mut self) {
        if let Some(tx) = &self.command_tx {
            let _ = tx.send(AudioCommand::Terminate);
        }
        if let Some(handle) = self.worker_thread.take() {
            let _ = handle.join();
        }
        self.command_tx = None;
        println!("🎤 AudioRecorder: Closed");
    }
}

fn resample_audio(data: &[f32], input_rate: u32, output_rate: u32) -> Vec<f32> {
    if data.is_empty() { return Vec::new(); }
    
    use rubato::{FftFixedIn, Resampler};
    
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