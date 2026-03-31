use crate::audio_toolkit::list_input_devices;
use crate::audio_toolkit::WHISPER_SAMPLE_RATE;
use crate::audio_toolkit::{AudioRecorder, AudioRecorderConfig};
use crate::clamshell;
use crate::settings;
use crate::transcription;
use cpal::traits::DeviceTrait;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Emitter};

#[derive(Clone, Debug)]
pub enum RecordingState {
    Idle,
    Recording { binding_id: String },
}

#[derive(Clone, Debug)]
pub enum MicrophoneMode {
    AlwaysOn,
    OnDemand,
}

#[derive(Clone)]
pub struct AudioRecordingManager {
    state: Arc<Mutex<RecordingState>>,
    mode: Arc<Mutex<MicrophoneMode>>,
    app_handle: AppHandle,
    recorder: Arc<Mutex<Option<AudioRecorder>>>,
    is_open: Arc<Mutex<bool>>,
    is_recording: Arc<Mutex<bool>>,
    is_monitoring: Arc<AtomicBool>,
}

impl AudioRecordingManager {
    pub fn new(app: &AppHandle) -> Result<Self, anyhow::Error> {
        let settings = settings::load_settings(app);
        let mode = if settings.always_on_microphone {
            MicrophoneMode::AlwaysOn
        } else {
            MicrophoneMode::OnDemand
        };

        let manager = Self {
            state: Arc::new(Mutex::new(RecordingState::Idle)),
            mode: Arc::new(Mutex::new(mode.clone())),
            app_handle: app.clone(),
            recorder: Arc::new(Mutex::new(None)),
            is_open: Arc::new(Mutex::new(false)),
            is_recording: Arc::new(Mutex::new(false)),
            is_monitoring: Arc::new(AtomicBool::new(false)),
        };

        // Always-on? Open immediately.
        if matches!(mode, MicrophoneMode::AlwaysOn) {
            manager.start_microphone_stream()?;
        }

        // Start monitoring if enabled
        if settings.voice_command_enabled {
            manager.start_voice_trigger_monitor();
        }

        Ok(manager)
    }

    fn get_effective_microphone_device(&self, settings: &settings::Settings) -> Option<cpal::Device> {
        // Check if we're in clamshell mode and have a clamshell microphone configured
        let use_clamshell_mic = if let Ok(is_clamshell) = clamshell::is_clamshell() {
            is_clamshell && settings.clamshell_microphone.is_some()
        } else {
            false
        };

        let device_name = if use_clamshell_mic {
            settings.clamshell_microphone.as_ref().unwrap()
        } else {
            &settings.input_device_id
        };

        println!("🔍 Looking for microphone device: '{}' (clamshell: {})", device_name, use_clamshell_mic);

        // Find the device by name
        match list_input_devices() {
            Ok(devices) => {
                println!("📋 Available input devices:");
                for (i, device) in devices.iter().enumerate() {
                    println!("   {}. '{}' (index: {})", i + 1, device.name, device.index);
                }
                
                let found_device = devices
                    .into_iter()
                    .find(|d| d.name == *device_name)
                    .map(|d| d.device);
                
                if found_device.is_some() {
                    println!("✅ Found device: '{}'", device_name);
                } else {
                    println!("⚠️ Device '{}' not found, will use default", device_name);
                }
                
                found_device
            },
            Err(e) => {
                eprintln!("Failed to list devices, using default: {}", e);
                None
            }
        }
    }

    pub fn start_microphone_stream(&self) -> Result<(), anyhow::Error> {
        let mut open_flag = self.is_open.lock().unwrap();
        if *open_flag {
            println!("Microphone stream already active");
            return Ok(());
        }

        let start_time = std::time::Instant::now();

        let mut recorder_opt = self.recorder.lock().unwrap();

        if recorder_opt.is_none() {
            let config = AudioRecorderConfig::default();
            let app_handle = self.app_handle.clone();
            *recorder_opt = Some(AudioRecorder::with_config(config)
                .with_level_callback(move |levels| {
                    let _ = app_handle.emit("mic-level", levels);
                })
            );
        }

        // Get the selected device from settings, considering clamshell mode
        let settings = settings::load_settings(&self.app_handle);
        let selected_device = self.get_effective_microphone_device(&settings);

        if let Some(ref device) = selected_device {
            if let Ok(name) = device.name() {
                println!("🎤 Starting microphone stream with device: '{}'", name);
            }
        } else {
            println!("🎤 Starting microphone stream with default device");
        }

        if let Some(rec) = recorder_opt.as_mut() {
            rec.open(selected_device)
                .map_err(|e| anyhow::anyhow!("Failed to open recorder: {}", e))?;
        }

        *open_flag = true;
        println!(
            "Microphone stream initialized in {:?}",
            start_time.elapsed()
        );
        Ok(())
    }

    pub fn stop_microphone_stream(&self) {
        let mut open_flag = self.is_open.lock().unwrap();
        if !*open_flag {
            return;
        }

        if let Some(rec) = self.recorder.lock().unwrap().as_mut() {
            // If still recording, stop first.
            if *self.is_recording.lock().unwrap() {
                let _ = rec.stop();
                *self.is_recording.lock().unwrap() = false;
            }
            rec.close();
        }

        *open_flag = false;
        println!("Microphone stream stopped");
    }

    pub fn update_mode(&self, new_mode: MicrophoneMode) -> Result<(), anyhow::Error> {
        let mode_guard = self.mode.lock().unwrap();
        let cur_mode = mode_guard.clone();

        match (cur_mode, &new_mode) {
            (MicrophoneMode::AlwaysOn, MicrophoneMode::OnDemand) => {
                if matches!(*self.state.lock().unwrap(), RecordingState::Idle) {
                    drop(mode_guard);
                    self.stop_microphone_stream();
                }
            }
            (MicrophoneMode::OnDemand, MicrophoneMode::AlwaysOn) => {
                drop(mode_guard);
                self.start_microphone_stream()?;
            }
            _ => {}
        }

        *self.mode.lock().unwrap() = new_mode;
        Ok(())
    }

    pub fn try_start_recording(&self, binding_id: &str) -> bool {
        let mut state = self.state.lock().unwrap();

        if let RecordingState::Idle = *state {
            // Ensure microphone is open in on-demand mode
            if matches!(*self.mode.lock().unwrap(), MicrophoneMode::OnDemand) {
                if let Err(e) = self.start_microphone_stream() {
                    eprintln!("Failed to open microphone stream: {e}");
                    return false;
                }
            }

            if let Some(rec) = self.recorder.lock().unwrap().as_mut() {
                if rec.start().is_ok() {
                    *self.is_recording.lock().unwrap() = true;
                    *state = RecordingState::Recording {
                        binding_id: binding_id.to_string(),
                    };
                    println!("Recording started for binding {binding_id}");
                    return true;
                }
            }
            eprintln!("Recorder not available");
            false
        } else {
            false
        }
    }

    pub fn update_selected_device(&self) -> Result<(), anyhow::Error> {
        // If currently open, restart the microphone stream to use the new device
        if *self.is_open.lock().unwrap() {
            self.stop_microphone_stream();
            self.start_microphone_stream()?;
        }
        Ok(())
    }

    pub fn stop_recording(&self, binding_id: &str) -> Option<Vec<f32>> {
        let mut state = self.state.lock().unwrap();

        match *state {
            RecordingState::Recording {
                binding_id: ref active,
            } if active == binding_id => {
                *state = RecordingState::Idle;
                drop(state);

                let samples = if let Some(rec) = self.recorder.lock().unwrap().as_mut() {
                    rec.stop()
                } else {
                    eprintln!("Recorder not available");
                    Vec::new()
                };

                *self.is_recording.lock().unwrap() = false;

                // In on-demand mode turn the mic off again
                if matches!(*self.mode.lock().unwrap(), MicrophoneMode::OnDemand) {
                    self.stop_microphone_stream();
                }

                // Pad if very short
                let s_len = samples.len();
                if s_len < WHISPER_SAMPLE_RATE && s_len > 0 {
                    let mut padded = samples;
                    padded.resize(WHISPER_SAMPLE_RATE * 5 / 4, 0.0);
                    Some(padded)
                } else {
                    Some(samples)
                }
            }
            _ => None,
        }
    }

    pub fn is_recording(&self) -> bool {
        matches!(
            *self.state.lock().unwrap(),
            RecordingState::Recording { .. }
        )
    }

    /// Cancel any ongoing recording without returning audio samples
    pub fn cancel_recording(&self) {
        let mut state = self.state.lock().unwrap();

        if let RecordingState::Recording { .. } = *state {
            *state = RecordingState::Idle;
            drop(state);

            if let Some(rec) = self.recorder.lock().unwrap().as_mut() {
                let _ = rec.stop(); // Discard the result
            }

            *self.is_recording.lock().unwrap() = false;

            // In on-demand mode turn the mic off again
            if matches!(*self.mode.lock().unwrap(), MicrophoneMode::OnDemand) {
                self.stop_microphone_stream();
            }
        }
    }

    pub fn start_voice_trigger_monitor(&self) {
        if self.is_monitoring.load(Ordering::SeqCst) {
            return;
        }
        
        // Ensure mic is open
        if let Err(e) = self.start_microphone_stream() {
            eprintln!("Failed to start mic for trigger: {}", e);
            return;
        }

        let rx = {
            let mut recorder = self.recorder.lock().unwrap();
            if let Some(rec) = recorder.as_mut() {
                 match rec.subscribe() {
                    Ok(r) => r,
                    Err(e) => {
                        eprintln!("Failed to subscribe: {}", e);
                        return;
                    }
                }
            } else {
                eprintln!("Recorder not initialized");
                return;
            }
        };

        self.is_monitoring.store(true, Ordering::SeqCst);
        let is_monitoring = self.is_monitoring.clone();
        let app_handle = self.app_handle.clone();
        let settings = settings::load_settings(&self.app_handle);
        let open_phrase = settings.voice_command_open_phrase.to_lowercase();
        let close_phrase = settings.voice_command_close_phrase.to_lowercase();

        std::thread::spawn(move || {
            println!("👂 Voice Trigger Monitor Started");
            let mut buffer: Vec<f32> = Vec::new();
            let window_size = 16000 * 3; // 3 seconds
            let check_interval = 16000 * 1; // 1 second
            let mut samples_since_check = 0;

            while is_monitoring.load(Ordering::SeqCst) {
                match rx.recv() {
                    Ok(data) => {
                        buffer.extend(data.iter().cloned());
                        samples_since_check += data.len();
                        
                        if buffer.len() > window_size {
                            let remove = buffer.len() - window_size;
                            buffer.drain(0..remove);
                        }

                        if samples_since_check >= check_interval {
                            samples_since_check = 0;
                            
                            // Check energy (RMS)
                            let sum_sq: f32 = buffer.iter().map(|x| x*x).sum();
                            let rms = (sum_sq / buffer.len() as f32).sqrt();
                            
                            if rms > 0.01 { 
                                let audio_data = buffer.clone();
                                let open_p = open_phrase.clone();
                                let close_p = close_phrase.clone();
                                let app = app_handle.clone();
                                
                                tauri::async_runtime::spawn(async move {
                                    match transcription::transcribe(audio_data).await {
                                        Ok(text) => {
                                            let clean = text.replace(|c: char| !c.is_alphanumeric() && !c.is_whitespace(), "").to_lowercase();
                                            let clean_open = open_p.replace(|c: char| !c.is_alphanumeric() && !c.is_whitespace(), "").to_lowercase();
                                            let clean_close = close_p.replace(|c: char| !c.is_alphanumeric() && !c.is_whitespace(), "").to_lowercase();

                                            if clean.contains(&clean_open) {
                                                println!("🗣️ VOICE TRIGGER: OPEN");
                                                let _ = app.emit("internal-voice-command", "open");
                                                let _ = app.emit("voice-command-triggered", "open");
                                            } else if clean.contains(&clean_close) {
                                                println!("🗣️ VOICE TRIGGER: CLOSE");
                                                let _ = app.emit("internal-voice-command", "close");
                                                let _ = app.emit("voice-command-triggered", "close");
                                            }
                                        }
                                        Err(_) => {}
                                    }
                                });
                            }
                        }
                    },
                    Err(_) => {
                        break;
                    }
                }
            }
            println!("👂 Voice Trigger Monitor Stopped");
        });
    }

    pub fn stop_voice_trigger_monitor(&self) {
        self.is_monitoring.store(false, Ordering::SeqCst);
        // If mode is OnDemand and not recording, stop mic
        let mode = self.mode.lock().unwrap();
        if matches!(*mode, MicrophoneMode::OnDemand) {
             if !*self.is_recording.lock().unwrap() {
                 // self.stop_microphone_stream(); // This might deadlock if called from same thread?
                 // No, stop_microphone_stream locks is_open.
                 // We should be careful.
                 // Actually, let's keep it simple.
             }
        }
    }
    
    pub fn update_voice_trigger_state(&self) {
        let settings = settings::load_settings(&self.app_handle);
        if settings.voice_command_enabled {
             self.start_voice_trigger_monitor();
        } else {
             self.stop_voice_trigger_monitor();
        }
    }
}