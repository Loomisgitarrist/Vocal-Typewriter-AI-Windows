#![allow(unexpected_cfgs)]

mod transcription;
mod keyboard;
mod tray;
mod settings;
mod typing;
mod permissions;
mod model_download;
mod models;
mod model_manager;
mod menu;

mod overlay;
mod audio_feedback;
mod clamshell;
mod audio_toolkit;
mod managers;
mod commands;
mod openrouter;

use std::sync::{Arc, Mutex};
use tauri::{Manager, WindowEvent, Emitter, Listener};
use tauri_plugin_autostart::MacosLauncher;
use cpal::traits::{DeviceTrait, HostTrait};
use serde::Serialize;
use managers::audio::AudioRecordingManager;
use raw_window_handle::{HasWindowHandle, RawWindowHandle};

#[derive(Serialize)]
pub struct AudioDevice {
    pub id: String,
    pub label: String,
}

#[derive(Clone, serde::Serialize, serde::Deserialize, Debug, PartialEq)]
pub enum HighlightStatus {
    Idle,
    Processing,
    Success(String),
    Error(String),
}

#[tauri::command]
fn get_audio_devices() -> Result<Vec<AudioDevice>, String> {
    let host = cpal::default_host();
    let devices = host.input_devices().map_err(|e| e.to_string())?;
    
    let mut result = Vec::new();
    
    for device in devices {
        if let Ok(name) = device.name() {
            result.push(AudioDevice {
                id: name.clone(),
                label: name,
            });
        }
    }
    
    Ok(result)
}

pub struct AppState {
    is_recording: Arc<Mutex<bool>>,
    settings: Arc<Mutex<settings::Settings>>,
    model_loaded: Arc<Mutex<bool>>,
    current_shortcut: Arc<Mutex<Option<String>>>,
    current_style_shortcut: Arc<Mutex<Option<String>>>,
    current_raw_mode_shortcut: Arc<Mutex<Option<String>>>,
    current_highlight_shortcut: Arc<Mutex<Option<String>>>,
    audio_manager: Arc<AudioRecordingManager>,
    last_selection: Arc<Mutex<Option<String>>>,
    last_screenshot: Arc<Mutex<Option<String>>>,
    pub pending_audio: Arc<Mutex<Option<Vec<f32>>>>,
    pub highlight_status: Arc<Mutex<HighlightStatus>>,
}

fn normalize_shortcut_str(shortcut: &str) -> String {
    shortcut
        .split('+')
        .map(|part| part.trim().to_lowercase().replace("option", "alt"))
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join("+")
}

pub fn process_audio_and_submit(
    app: tauri::AppHandle,
    audio_data: Vec<f32>,
    is_highlight_flow: bool,
) {
    use tauri_plugin_clipboard_manager::ClipboardExt;

    if audio_data.is_empty() {
        overlay::hide_recording_overlay(&app);
        
        if is_highlight_flow {
             let ah_error = app.clone();
             let _ = app.run_on_main_thread(move || {
                 if let Some(state) = ah_error.try_state::<AppState>() {
                     *state.highlight_status.lock().unwrap() = HighlightStatus::Error("No audio detected".to_string());
                 }
                 if let Some(window) = ah_error.get_webview_window("highlight_result") {
                     let _ = window.emit("processing-error", "No audio detected");
                     let _ = window.show();
                     let _ = window.set_focus();
                 }
             });
        }
        return;
    }

    // Show transcribing overlay
    overlay::show_transcribing_overlay(&app);

    // If Highlight Flow, show the result window IMMEDIATELY with a loading state
    if is_highlight_flow {
        let ah_loading = app.clone();
        let _ = app.run_on_main_thread(move || {
            if let Some(state) = ah_loading.try_state::<AppState>() {
                *state.highlight_status.lock().unwrap() = HighlightStatus::Processing;
            }
            if let Some(window) = ah_loading.get_webview_window("highlight_result") {
                // Ensure window is visible and focused
                let _ = window.show();
                let _ = window.set_focus();
                let _ = window.set_always_on_top(true);
                let _ = window.set_ignore_cursor_events(false);
                
                // Emit processing event to show spinner/loading text
                println!("⏳ Emitting processing-started event...");
                if let Err(e) = window.emit("processing-started", ()) {
                     eprintln!("❌ Failed to emit processing-started: {}", e);
                }
            } else {
                 eprintln!("❌ Could not find highlight_result window for loading state");
            }
        });
    }

    let state: tauri::State<AppState> = app.state();
    let settings = match state.settings.lock() {
        Ok(guard) => guard.clone(),
        Err(e) => {
            eprintln!("❌ Failed to lock settings: {}", e);
            return;
        }
    };
    let last_selection = state.last_selection.clone();
    let last_screenshot = state.last_screenshot.clone();
    let ah_clone = app.clone();
    
    // For stats
    let audio_len = audio_data.len();
    
    println!("🎤 process_audio_and_submit started. is_highlight_flow: {}", is_highlight_flow);

    tauri::async_runtime::spawn(async move {
        // RACE CONDITION FIX: Wait for text selection if in highlight flow (Text Mode)
        if is_highlight_flow && settings.highlight_mode == "text" {
             println!("⏳ Waiting for text selection to be captured...");
             let mut attempts = 0;
             while attempts < 15 { // Wait up to 3 seconds
                 let has_selection = match last_selection.lock() {
                     Ok(guard) => guard.is_some(),
                     Err(_) => false,
                 };
                 if has_selection {
                     println!("✅ Text selection captured!");
                     break;
                 }
                 tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;
                 attempts += 1;
             }
             if attempts >= 15 {
                 println!("⚠️ Timed out waiting for text selection - proceeding without context");
             }
        }

        println!("🎤 Starting transcription...");
        match transcription::transcribe(audio_data).await {
            Ok(text) => {
                if text.trim().is_empty() {
                    println!("⚠️ No transcription result");
                    overlay::hide_recording_overlay(&ah_clone);
                    
                    if is_highlight_flow {
                         let ah_error = ah_clone.clone();
                         let _ = ah_clone.run_on_main_thread(move || {
                             if let Some(state) = ah_error.try_state::<AppState>() {
                                 if let Ok(mut guard) = state.highlight_status.lock() {
                                     *guard = HighlightStatus::Error("No speech detected".to_string());
                                 }
                             }
                             if let Some(window) = ah_error.get_webview_window("highlight_result") {
                                 let _ = window.emit("processing-error", "No speech detected");
                                 let _ = window.show();
                                 let _ = window.set_focus();
                             }
                         });
                    }
                    return;
                }
                println!("✅ Transcription: {}", text);

                // Emit stats event
                let duration_sec = audio_len as f32 / audio_toolkit::constants::WHISPER_SAMPLE_RATE as f32;
                let word_count = text.split_whitespace().count();
                
                #[derive(Serialize, Clone)]
                struct StatsPayload {
                    word_count: usize,
                    duration_sec: f32,
                }
                
                let _ = ah_clone.emit("transcription-stats", StatsPayload { word_count, duration_sec });

                // Check for voice command (ONLY if not in highlight flow, usually)
                // Actually, voice commands like "open style" might be useful even in highlight flow?
                // But for now let's keep it same behavior.
                
                let clean_text = text.replace(|c: char| !c.is_alphanumeric() && !c.is_whitespace(), "").to_lowercase();
                let clean_text = clean_text.trim();
                
                let open_phrase = settings.voice_command_open_phrase.replace(|c: char| !c.is_alphanumeric() && !c.is_whitespace(), "").to_lowercase();
                let open_phrase = open_phrase.trim();
                
                let close_phrase = settings.voice_command_close_phrase.replace(|c: char| !c.is_alphanumeric() && !c.is_whitespace(), "").to_lowercase();
                let close_phrase = close_phrase.trim();
                
                if settings.voice_command_enabled && !is_highlight_flow {
                    if !open_phrase.is_empty() && clean_text.contains(open_phrase) {
                        println!("🗣️ Voice Command OPEN Detected");
                        let app_handle = ah_clone.clone();
                        let _ = ah_clone.emit("voice-command-triggered", "open");
                        let _ = ah_clone.run_on_main_thread(move || {
                            perform_open_style_window(&app_handle);
                        });
                        overlay::hide_recording_overlay(&ah_clone);
                        return;
                    }
                    if !close_phrase.is_empty() && clean_text.contains(close_phrase) {
                        println!("🗣️ Voice Command CLOSE Detected");
                        let app_handle = ah_clone.clone();
                        let _ = ah_clone.emit("voice-command-triggered", "close");
                        let _ = ah_clone.run_on_main_thread(move || {
                            perform_close_style_window(&app_handle);
                        });
                        overlay::hide_recording_overlay(&ah_clone);
                        return;
                    }
                }

                // Get context
                let context = last_selection.lock().unwrap().take();
                if let Some(ref ctx) = context {
                    println!("📋 Using context ({} chars)", ctx.len());
                }
                
                // Get screenshot context
                let screenshot_context = last_screenshot.lock().unwrap().take();
                let mut active_model = settings.ollama_model.clone();

                if let Some(ref img) = screenshot_context {
                    println!("📸 Using screenshot context ({} chars)", img.len());
                    // Force vision model if screenshot is present
                    println!("📸 Switching to vision model: {}", settings.ollama_vision_model);
                    active_model = settings.ollama_vision_model.clone();
                } else {
                    println!("📸 No screenshot context found");
                }

                // Check if we should show result in window (Highlight mode OR explicit highlight flow)
                let show_result_window = is_highlight_flow;
                println!("🪟 show_result_window: {}", show_result_window);

                // Determine effective mode - Force AI if context is present
                let mut effective_mode = settings.correction_mode.clone();
                if show_result_window || context.is_some() || screenshot_context.is_some() {
                     if effective_mode == "none" {
                         println!("🖍️ Context detected in Raw Mode - forcing AI mode");
                         effective_mode = if settings.last_llm_mode != "none" && !settings.last_llm_mode.is_empty() {
                             settings.last_llm_mode.clone()
                         } else {
                             "local".to_string() // Default fallback
                         };
                     }
                }

                let context_len = context.as_ref().map(|s| s.len()).unwrap_or(0);
                println!("🚀 [DEBUG] Dispatching to provider: mode={}, context_len={}, screenshot={}", effective_mode, context_len, screenshot_context.is_some());
                
                // Clone screenshot context for UI display later
                let display_screenshot = screenshot_context.clone();
                let processed_text = match effective_mode.as_str() {
                    "local" => {
                        println!("🤖 Processing with Local AI...");
                        match typing::process_with_ai(&text, "edit", &settings.system_prompt, &active_model, context, settings.user_name.clone(), settings.target_language.clone(), screenshot_context).await {
                            Ok(processed) => {
                                println!("✅ Local AI success");
                                processed
                            },
                            Err(e) => {
                                eprintln!("AI processing failed: {}, using basic", e);
                                typing::format_basic(&text)
                            }
                        }
                    },
                    "openai" => {
                        println!("🤖 Processing with OpenAI...");
                        if let Some(key) = &settings.openai_api_key {
                            if key.is_empty() { 
                                println!("⚠️ OpenAI API key is empty");
                                typing::format_basic(&text) 
                            }
                            else {
                                match typing::process_with_openai(&text, &settings.system_prompt, &settings.openai_model, key, context, settings.user_name.clone(), settings.target_language.clone(), screenshot_context).await {
                                    Ok(processed) => {
                                        println!("✅ OpenAI success");
                                        processed
                                    },
                                    Err(e) => { 
                                        eprintln!("OpenAI failed: {}", e); 
                                        if show_result_window {
                                             // If we are in highlight window mode, we should tell the user about the error
                                             format!("Error: {}", e)
                                        } else {
                                             typing::format_basic(&text)
                                        }
                                    }
                                }
                            }
                        } else { 
                            println!("⚠️ OpenAI API key is missing");
                            typing::format_basic(&text) 
                        }
                    },
                    "openrouter" => {
                        println!("🤖 Processing with OpenRouter...");
                        if let Some(key) = &settings.openrouter_api_key {
                             if key.is_empty() { 
                                println!("⚠️ OpenRouter API key is empty");
                                typing::format_basic(&text) 
                             }
                             else {
                                match typing::process_with_openrouter(&text, &settings.system_prompt, &settings.openrouter_model, key, context, settings.user_name.clone(), settings.target_language.clone(), screenshot_context).await {
                                    Ok(processed) => {
                                        println!("✅ OpenRouter success");
                                        processed
                                    },
                                    Err(e) => { 
                                        eprintln!("OpenRouter failed: {}", e); 
                                        if show_result_window {
                                             format!("Error: {}", e)
                                        } else {
                                             typing::format_basic(&text)
                                        }
                                    }
                                }
                             }
                        } else { 
                            println!("⚠️ OpenRouter API key is missing");
                            typing::format_basic(&text) 
                        }
                    },
                    "none" => text,
                    _ => typing::format_basic(&text)
                };

                // Handle result (Paste or Show Window)
                let result_text = processed_text.clone();
                let ah_handle = ah_clone.clone();
                let auto_copy = settings.auto_copy;
                
                let _ = ah_clone.run_on_main_thread(move || {
                    // Hide overlay FIRST
                    overlay::hide_recording_overlay(&ah_handle);
                    
                    if show_result_window {
                        println!("🖍️ Highlight Context detected - Showing Result Window");
                        if let Some(window) = ah_handle.get_webview_window("highlight_result") {
                            let _ = window.show();
                            let _ = window.set_focus();
                            let _ = window.set_always_on_top(true);
                            let _ = window.set_ignore_cursor_events(false);
                            
                            // Emit result with optional screenshot
                            #[derive(Serialize, Clone)]
                            struct ResultPayload {
                                text: String,
                                image: Option<String>,
                            }

                            let payload = ResultPayload {
                                text: result_text.clone(),
                                image: display_screenshot.clone(), // Use the preserved screenshot context
                            };
                            
                            println!("📤 Emitting show-highlight-result. Text Length: {}, Image: {}", result_text.len(), display_screenshot.is_some());
                            
                            // Update state BEFORE emitting
                            if let Some(state) = ah_handle.try_state::<AppState>() {
                                *state.highlight_status.lock().unwrap() = HighlightStatus::Success(result_text.clone());
                            }

                            if let Err(e) = window.emit("show-highlight-result", &payload) {
                                eprintln!("Failed to emit highlight result: {}", e);
                            } else {
                                println!("✅ Emit successful");
                            }
                            if auto_copy {
                                let _ = ah_handle.clipboard().write_text(&result_text);
                            }
                        } else {
                            eprintln!("❌ Could not find highlight_result window");
                        }
                    } else {
                        // Standard Paste Mode
                        std::thread::sleep(std::time::Duration::from_millis(400));
                        if let Err(e) = typing::paste_text(&result_text, &ah_handle) {
                            eprintln!("Failed to paste text: {}", e);
                        } else {
                            println!("✅ Pasted: {}", result_text);
                            if auto_copy {
                                let _ = ah_handle.clipboard().write_text(&result_text);
                            }
                        }
                    }
                });
            }
            Err(e) => {
                eprintln!("Transcription failed: {}", e);
                overlay::hide_recording_overlay(&ah_clone);
                
                if is_highlight_flow {
                     let ah_error = ah_clone.clone();
                     let err_msg = e.to_string();
                     let _ = ah_clone.run_on_main_thread(move || {
                         if let Some(state) = ah_error.try_state::<AppState>() {
                             *state.highlight_status.lock().unwrap() = HighlightStatus::Error(format!("Transcription failed: {}", err_msg));
                         }
                         if let Some(window) = ah_error.get_webview_window("highlight_result") {
                             let _ = window.emit("processing-error", &format!("Transcription failed: {}", err_msg));
                             let _ = window.show();
                             let _ = window.set_focus();
                         }
                     });
                }
            },
        }
    });
}

#[tauri::command]
fn get_settings(state: tauri::State<'_, AppState>) -> Result<settings::Settings, String> {
    state.settings.lock().map_err(|e| e.to_string()).map(|s| s.clone())
}

#[tauri::command]
fn save_settings(
    state: tauri::State<'_, AppState>, 
    app: tauri::AppHandle,
    new_settings: settings::Settings
) -> Result<(), String> {
    println!("💾 save_settings called. user_name: {:?}", new_settings.user_name);
    use tauri_plugin_global_shortcut::{GlobalShortcutExt, ShortcutState};
    use tauri::Emitter;
    
    // Check if shortcut changed - SAFE LOCKING
    let old_shortcut = state.current_shortcut.lock().map_err(|e| e.to_string())?.clone();
    let old_style_shortcut = state.current_style_shortcut.lock().map_err(|e| e.to_string())?.clone();
    let old_raw_mode_shortcut = state.current_raw_mode_shortcut.lock().map_err(|e| e.to_string())?.clone();
    let old_highlight_shortcut = state.current_highlight_shortcut.lock().map_err(|e| e.to_string())?.clone();
    
    let new_shortcut_norm = normalize_shortcut_str(&new_settings.push_to_talk_key);
    let new_style_shortcut_norm = new_settings
        .style_selector_key
        .as_deref()
        .map(normalize_shortcut_str)
        .filter(|s| !s.is_empty());
    let new_raw_mode_shortcut_norm = new_settings
        .raw_mode_toggle_key
        .as_deref()
        .map(normalize_shortcut_str)
        .filter(|s| !s.is_empty());
    let new_highlight_shortcut_norm = new_settings
        .highlight_shortcut
        .as_deref()
        .map(normalize_shortcut_str)
        .filter(|s| !s.is_empty());

    // Check if input device changed - SAFE LOCKING
    let (old_input_device_id, old_clamshell_mic, old_correction_mode) = {
        let settings = state.settings.lock().map_err(|e| e.to_string())?;
        (
            settings.input_device_id.clone(),
            settings.clamshell_microphone.clone(),
            settings.correction_mode.clone()
        )
    };

    let input_device_changed = old_input_device_id != new_settings.input_device_id 
        || old_clamshell_mic != new_settings.clamshell_microphone;
    
    // Update settings in memory - SAFE LOCKING
    {
        let mut settings = state.settings.lock().map_err(|e| e.to_string())?;
        *settings = new_settings.clone();
    }
    
    // Save settings to disk MOVED TO END


    // Emit settings updated event for frontend sync
    app.emit("settings-updated", new_settings.clone())
        .map_err(|e| e.to_string())?;

    // Check for correction mode change (for HUD)
    if old_correction_mode != new_settings.correction_mode {
        let mode_text = if new_settings.correction_mode == "none" { "Raw" } else { "LLM" };
        let message = format!("Toggled to {} Mode", mode_text);
        overlay::show_hud_message(&app, &message);
    }

    // If input device changed, update audio manager
    if input_device_changed {
        println!("🎤 Input device changed from '{}' to '{}'. Updating audio manager...", 
                 old_input_device_id, new_settings.input_device_id);
        let audio_manager = state.audio_manager.clone();
        if let Err(e) = audio_manager.update_selected_device() {
            eprintln!("Failed to update audio device: {}", e);
        }
    }

    // Update voice trigger state
    state.audio_manager.update_voice_trigger_state();
    
    // If shortcut changed, re-register it
    if old_shortcut.as_deref() != Some(&new_shortcut_norm) {
        println!("🔄 Shortcut changed from {:?} to {}", old_shortcut, new_shortcut_norm);
        
        // Unregister old shortcut
        if let Some(old_str) = &old_shortcut {
            if let Ok(old_sc) = old_str.parse::<tauri_plugin_global_shortcut::Shortcut>() {
                let _ = app.global_shortcut().unregister(old_sc);
                println!("🗑️ Unregistered old shortcut: {}", old_str);
            }
        }
        
        // Register new shortcut
        if let Ok(new_sc) = new_shortcut_norm.parse::<tauri_plugin_global_shortcut::Shortcut>() {
            let _ = app.global_shortcut().unregister(new_sc);
            let shortcut_result = app.global_shortcut().on_shortcut(new_sc, move |ah, _, event| {
                if event.state == ShortcutState::Pressed {
                    let state: tauri::State<AppState> = ah.state();
                    
                    let model_loaded = match state.model_loaded.lock() {
                        Ok(guard) => *guard,
                        Err(_) => {
                             eprintln!("❌ Failed to lock model_loaded state");
                             return;
                        }
                    };
                    
                    if !model_loaded {
                        let _ = ah.emit("model-not-loaded", ());
                        println!("❌ Model not loaded, cannot start recording");
                        return;
                    }

                    // Start recording
                    let audio_manager = state.audio_manager.clone();
                    
                    // Play start sound immediately
                    audio_feedback::play_start_sound(ah);

                    if !audio_manager.try_start_recording("main") {
                        eprintln!("Failed to start recording");
                        return;
                    }
                    
                    if let Ok(mut is_recording) = state.is_recording.lock() {
                        *is_recording = true;
                    }

                    println!("🎤 Shortcut PRESSED");
                    let _ = ah.emit("start-recording", ());

                    // Show overlay
                    overlay::show_recording_overlay(ah);
                    
                    // Chat Mode: Capture selection - DISABLED/REMOVED
                    if let Ok(mut last_sel) = state.last_selection.lock() {
                        *last_sel = None;
                    }
                    if let Ok(mut last_scr) = state.last_screenshot.lock() {
                        *last_scr = None;
                    };
                } else if event.state == ShortcutState::Released {
                    let state: tauri::State<AppState> = ah.state();
                    let audio_data = {
                        let audio_manager = state.audio_manager.clone();
                        audio_manager.stop_recording("main").unwrap_or_default()
                    };
                    
                    if let Ok(mut is_recording) = state.is_recording.lock() {
                        *is_recording = false;
                    }

                    println!("🛑 Shortcut RELEASED - Processing {} samples", audio_data.len());
                    let _ = ah.emit("stop-recording", ());

                    // Play stop sound
                    audio_feedback::play_stop_sound(ah);

                    // Process audio if we have data
                    if !audio_data.is_empty() {
                         // Process with AI and submit (is_highlight_flow = false)
                         process_audio_and_submit(ah.clone(), audio_data, false);
                    } else {
                        // No audio data, just hide
                        overlay::hide_recording_overlay(ah);
                    }
                }
            
            });
            if let Err(e) = shortcut_result {
                eprintln!("❌ Failed to register new shortcut: {}", e);
                return Err(format!("Failed to register shortcut '{}': {}", new_shortcut_norm, e));
            }
            println!("✅ New shortcut '{}' registered!", new_shortcut_norm);
            if let Ok(mut guard) = state.current_shortcut.lock() {
                *guard = Some(new_shortcut_norm);
            }
        } else {
            return Err(format!("Invalid shortcut format: '{}'. Use format like 'alt+space' or 'ctrl+shift+r'", new_shortcut_norm));
        }
    }

    // --- Update Style Selector Shortcut ---
    if old_style_shortcut != new_style_shortcut_norm {
        println!("🔄 Style Shortcut changed from {:?} to {:?}", old_style_shortcut, new_style_shortcut_norm);
        
        // Unregister old style shortcut
        if let Some(old_str) = &old_style_shortcut {
            if let Ok(old_sc) = old_str.parse::<tauri_plugin_global_shortcut::Shortcut>() {
                let _ = app.global_shortcut().unregister(old_sc);
                println!("🗑️ Unregistered old style shortcut: {}", old_str);
            }
        }
        
        // Register new style shortcut (if set)
        if let Some(new_str) = &new_style_shortcut_norm {
            if !new_str.trim().is_empty() {
                 if let Ok(new_sc) = new_str.parse::<tauri_plugin_global_shortcut::Shortcut>() {
                    let _ = app.global_shortcut().unregister(new_sc);
                    let _ = app.global_shortcut().on_shortcut(new_sc, move |ah, _, event| {
                        if event.state == ShortcutState::Pressed {
                            // Play start sound
                            audio_feedback::play_start_sound(ah);

                            if let Some(window) = ah.get_webview_window("style_selector") {
                                if window.is_visible().unwrap_or(false) {
                                    let _ = window.hide();
                                } else {
                                    #[cfg(target_os = "macos")]
                                    {
                                        use objc::{msg_send, sel, sel_impl};
                                        
                                        let mut showed = false;
                                        if let Ok(handle) = window.window_handle() {
                                            if let RawWindowHandle::AppKit(appkit_handle) = handle.as_raw() {
                                                // Suppress deprecated warnings for cocoa types
                                                #[allow(deprecated)]
                                                let ns_view = appkit_handle.ns_view.as_ptr() as *mut objc::runtime::Object;
                                                unsafe {
                                                    let ns_window: *mut objc::runtime::Object = msg_send![ns_view, window];
                                                    if !ns_window.is_null() {
                                                        let _: () = msg_send![ns_window, orderFront:std::ptr::null_mut::<objc::runtime::Object>()];
                                                        showed = true;
                                                    }
                                                }
                                            }
                                        }
                                        if !showed {
                                            let _ = window.show();
                                        }
                                    }
                                    #[cfg(target_os = "windows")]
                                    {
                                        use windows::Win32::UI::WindowsAndMessaging::{ShowWindow, SW_SHOWNOACTIVATE};
                                        use windows::Win32::Foundation::HWND;
                                        
                                        let mut showed = false;
                                        if let Ok(handle) = window.window_handle() {
                                            if let RawWindowHandle::Win32(win32_handle) = handle.as_raw() {
                                                // Convert NonZeroIsize to isize, then to HWND
                                                let hwnd = HWND(win32_handle.hwnd.get());
                                                unsafe {
                                                    let _ = ShowWindow(hwnd, SW_SHOWNOACTIVATE);
                                                    showed = true;
                                                }
                                            }
                                        }
                                        if !showed {
                                            let _ = window.show();
                                        }
                                    }
                                    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
                                    {
                                        let _ = window.show();
                                    }
                                }
                            }
                        }
                    });
                    println!("✅ Style Selector shortcut registered successfully!");
                    if let Ok(mut guard) = state.current_style_shortcut.lock() {
                        *guard = Some(new_str.clone());
                    }
                 } else {
                    eprintln!("❌ Failed to parse Style Selector shortcut: {}", new_str);
                     if let Ok(mut guard) = state.current_style_shortcut.lock() {
                         *guard = None;
                     }
                 }
            } else {
                if let Ok(mut guard) = state.current_style_shortcut.lock() {
                    *guard = None;
                }
            }
        } else {
            if let Ok(mut guard) = state.current_style_shortcut.lock() {
                *guard = None;
            }
        }
    }
    // --- Update Raw Mode Toggle Shortcut ---
    if old_raw_mode_shortcut != new_raw_mode_shortcut_norm {
        println!("🔄 Raw Mode Shortcut changed from {:?} to {:?}", old_raw_mode_shortcut, new_raw_mode_shortcut_norm);
        
        // Unregister old shortcut
        if let Some(old_str) = &old_raw_mode_shortcut {
            if let Ok(old_sc) = old_str.parse::<tauri_plugin_global_shortcut::Shortcut>() {
                let _ = app.global_shortcut().unregister(old_sc);
                println!("🗑️ Unregistered old raw mode shortcut: {}", old_str);
            }
        }
        
        // Register new shortcut
        if let Some(new_str) = new_raw_mode_shortcut_norm.clone() {
            if !new_str.is_empty() {
                 if let Ok(new_sc) = new_str.parse::<tauri_plugin_global_shortcut::Shortcut>() {
                    let _ = app.global_shortcut().unregister(new_sc);
                    let _ = app.global_shortcut().on_shortcut(new_sc, move |ah, _, event| {
                         if event.state == ShortcutState::Pressed {
                            // Play start sound
                            audio_feedback::play_start_sound(ah);

                            let state: tauri::State<AppState> = ah.state();
                            println!("🎤 Raw Mode Toggle Shortcut PRESSED");
                            
                            // Toggle logic
                            let mut settings = match state.settings.lock() {
                                Ok(guard) => guard,
                                Err(_) => {
                                    eprintln!("❌ Failed to lock settings for raw mode toggle");
                                    return;
                                }
                            };
                            
                            let current_mode = settings.correction_mode.clone();
                            let last_mode = settings.last_llm_mode.clone();
                            
                            let new_mode = if current_mode == "none" {
                                // Switch to last used LLM mode
                                if last_mode == "none" {
                                    "local".to_string() // Fallback
                                } else {
                                    last_mode
                                }
                            } else {
                                // Switch to Raw Mode
                                // Update last_llm_mode before switching
                                settings.last_llm_mode = current_mode;
                                "none".to_string()
                            };
                            
                            settings.correction_mode = new_mode.clone();
                            
                            // Save settings
                            let _ = settings::save_settings(&ah, &settings);
                            
                            // Emit update to frontend
                            let _ = ah.emit("settings-updated", settings.clone());
                            
                            // Show notification/log
                            println!("🔄 Toggled Correction Mode to: {}", new_mode);
                            
                            // Show HUD
                            let mode_text = if new_mode == "none" { "Raw" } else { "LLM" };
                            let message = format!("Toggled to {} Mode", mode_text);
                            overlay::show_hud_message(&ah, &message);
                         }
                    });
                    println!("✅ Raw Mode Toggle shortcut registered successfully!");
                    if let Ok(mut guard) = state.current_raw_mode_shortcut.lock() {
                        *guard = Some(new_str.clone());
                    }
                 } else {
                    eprintln!("❌ Failed to parse Raw Mode shortcut: {}", new_str);
                     if let Ok(mut guard) = state.current_raw_mode_shortcut.lock() {
                         *guard = None;
                     }
                 }
            } else {
                if let Ok(mut guard) = state.current_raw_mode_shortcut.lock() {
                    *guard = None;
                }
            }
        } else {
            if let Ok(mut guard) = state.current_raw_mode_shortcut.lock() {
                *guard = None;
            }
        }
    }

    // --- Update Highlight Shortcut ---
    if old_highlight_shortcut != new_highlight_shortcut_norm {
        println!("🔄 Highlight Shortcut changed from {:?} to {:?}", old_highlight_shortcut, new_highlight_shortcut_norm);
        
        // Unregister old shortcut
        if let Some(old_str) = &old_highlight_shortcut {
            if let Ok(old_sc) = old_str.parse::<tauri_plugin_global_shortcut::Shortcut>() {
                let _ = app.global_shortcut().unregister(old_sc);
                println!("🗑️ Unregistered old highlight shortcut: {}", old_str);
            }
        }
        
        // Register new shortcut
        if let Some(new_str) = new_highlight_shortcut_norm.clone() {
            if !new_str.is_empty() {
                 if let Ok(new_sc) = new_str.parse::<tauri_plugin_global_shortcut::Shortcut>() {
                    let _ = app.global_shortcut().unregister(new_sc);
                    let _ = app.global_shortcut().on_shortcut(new_sc, move |ah, _, event| {
                         if event.state == ShortcutState::Pressed {
                            let state: tauri::State<AppState> = ah.state();
                            
                            let model_loaded = match state.model_loaded.lock() {
                                Ok(guard) => *guard,
                                Err(_) => {
                                     eprintln!("❌ Failed to lock model_loaded for highlight");
                                     return;
                                }
                            };
                            
                            println!("🔍 [DEBUG] Checking model_loaded: {}", model_loaded);
                            if !model_loaded {
                                let _ = ah.emit("model-not-loaded", ());
                                println!("❌ [DEBUG] Model NOT loaded. Highlight recording aborted.");
                                return;
                            }

                            let mode = match state.settings.lock() {
                                Ok(settings) => settings.highlight_mode.clone(),
                                Err(_) => {
                                    eprintln!("❌ Failed to lock settings for highlight mode");
                                    return;
                                }
                            };
                            
                            let audio_manager = state.audio_manager.clone();

                            if mode == "text" {
                                // Text Mode: Start Recording immediately
                                audio_feedback::play_start_sound(ah);

                                if !audio_manager.try_start_recording("main") {
                                    eprintln!("Failed to start recording for highlight");
                                    return;
                                }
                                if let Ok(mut is_recording) = state.is_recording.lock() {
                                    *is_recording = true;
                                }

                                println!("🖍️ Highlight Shortcut PRESSED - Recording Started (Text Mode)");
                                let _ = ah.emit("start-recording", ());

                                if let Ok(mut last_sel) = state.last_selection.lock() {
                                    *last_sel = None;
                                }
                                if let Ok(mut last_scr) = state.last_screenshot.lock() {
                                    *last_scr = None;
                                }
                                
                                let ah_clone = ah.clone();
                                tauri::async_runtime::spawn_blocking(move || {
                                    commands::highlight::handle_text_highlight(&ah_clone);
                                });
                                
                                // Show recording overlay (pill)
                                overlay::show_highlight_overlay(ah);
                            } else {
                                // Screenshot Mode: Start Recording (Hold to Talk)
                                audio_feedback::play_start_sound(ah);

                                if !audio_manager.try_start_recording("main") {
                                    eprintln!("Failed to start recording for highlight screenshot");
                                    return;
                                }
                                if let Ok(mut is_recording) = state.is_recording.lock() {
                                    *is_recording = true;
                                }
                                
                                println!("📸 Highlight Shortcut PRESSED - Recording Started (Screenshot Mode)");
                                let _ = ah.emit("start-recording", ());

                                // Clear previous screenshot & pending audio
                                if let Ok(mut last_scr) = state.last_screenshot.lock() {
                                    *last_scr = None;
                                }
                                if let Ok(mut pending) = state.pending_audio.lock() {
                                    *pending = None;
                                }
                                
                                // Show recording overlay (pill)
                                overlay::show_highlight_overlay(ah);
                            }
                         } else if event.state == ShortcutState::Released {
                            let state: tauri::State<AppState> = ah.state();
                            
                            let is_recording = state.is_recording.lock().map(|guard| *guard).unwrap_or(false);
                            
                            if is_recording {
                                let audio_manager = state.audio_manager.clone();
                                let audio_data = audio_manager.stop_recording("main").unwrap_or_default();
                                
                                if let Ok(mut is_rec) = state.is_recording.lock() {
                                    *is_rec = false;
                                }
                                
                                println!("🛑 Highlight Shortcut RELEASED - Processing {} samples", audio_data.len());
                                let _ = ah.emit("stop-recording", ());
                                audio_feedback::play_stop_sound(ah);

                                let mode = state.settings.lock().map(|s| s.highlight_mode.clone()).unwrap_or("text".to_string());

                                if mode == "text" {
                                            if !audio_data.is_empty() {
                                                // Show Highlight Result Window immediately with "Processing..."
                                                if let Some(window) = ah.get_webview_window("highlight_result") {
                                                    let _ = window.show();
                                                    let _ = window.set_focus();
                                                    let _ = window.set_always_on_top(true);
                                                    let _ = window.set_ignore_cursor_events(false);
                                                    let _ = window.emit("processing-started", ());
                                                }

                                                process_audio_and_submit(ah.clone(), audio_data, true);
                                            }
                                            
                                            // Hide recording pill
                                            overlay::hide_recording_overlay(ah);
                                } else {
                                    // Screenshot Mode: Buffer Audio & Show Selection
                                    println!("📸 Highlight Screenshot RELEASED - Buffering Audio & Showing Selection");
                                    if !audio_data.is_empty() {
                                        if let Ok(mut pending) = state.pending_audio.lock() {
                                            *pending = Some(audio_data);
                                        }
                                    }
                                    
                                    // Hide recording pill
                                    overlay::hide_recording_overlay(ah);
                                    
                                    // Show Selection Overlay NOW (after release)
                                    if let Some(window) = ah.get_webview_window("selection_overlay") {
                                         // Reset selection state in frontend
                                         let _ = window.emit("reset-selection", ());
                                         
                                         // Ensure fullscreen and visible
                                         let _ = window.set_fullscreen(true);
                                         let _ = window.show();
                                         let _ = window.set_focus();
                                         let _ = window.set_always_on_top(true);
                                         
                                         #[cfg(target_os = "macos")]
                                         {
                                             let _ = window.set_focus();
                                         }
                                    } else {
                                        eprintln!("❌ Selection window not found!");
                                    }
                                }
                            }
                         }
                    });
                    println!("✅ Highlight shortcut registered successfully!");
                    if let Ok(mut guard) = state.current_highlight_shortcut.lock() {
                        *guard = Some(new_str.clone());
                    }
                 } else {
                    eprintln!("❌ Failed to parse Highlight shortcut: {}", new_str);
                     if let Ok(mut guard) = state.current_highlight_shortcut.lock() {
                         *guard = None;
                     }
                 }
            } else {
                if let Ok(mut guard) = state.current_highlight_shortcut.lock() {
                    *guard = None;
                }
            }
        } else {
            if let Ok(mut guard) = state.current_highlight_shortcut.lock() {
                *guard = None;
            }
        }
    }

    // Save settings to disk - Moved to end to ensure valid state before saving
    settings::save_settings(&app, &new_settings)?;

    Ok(())
}

fn perform_open_style_window(app: &tauri::AppHandle) {
    if let Some(window) = app.get_webview_window("style_selector") {
        if !window.is_visible().unwrap_or(false) {
             #[cfg(target_os = "macos")]
            {
                use objc::{msg_send, sel, sel_impl};
                
                let mut showed = false;
                if let Ok(handle) = window.window_handle() {
                    if let RawWindowHandle::AppKit(appkit_handle) = handle.as_raw() {
                        // Suppress deprecated warnings for cocoa types
                        #[allow(deprecated)]
                        let ns_view = appkit_handle.ns_view.as_ptr() as *mut objc::runtime::Object;
                        unsafe {
                            let ns_window: *mut objc::runtime::Object = msg_send![ns_view, window];
                            if !ns_window.is_null() {
                                let _: () = msg_send![ns_window, orderFront:std::ptr::null_mut::<objc::runtime::Object>()];
                                showed = true;
                            }
                        }
                    }
                }
                if !showed {
                    let _ = window.show();
                }
            }
            #[cfg(target_os = "windows")]
            {
                use windows::Win32::UI::WindowsAndMessaging::{ShowWindow, SW_SHOWNOACTIVATE};
                use windows::Win32::Foundation::HWND;
                
                let mut showed = false;
                if let Ok(handle) = window.window_handle() {
                    if let RawWindowHandle::Win32(win32_handle) = handle.as_raw() {
                        let hwnd = HWND(win32_handle.hwnd.get());
                        unsafe {
                            let _ = ShowWindow(hwnd, SW_SHOWNOACTIVATE);
                            showed = true;
                        }
                    }
                }
                if !showed {
                    let _ = window.show();
                }
            }
            #[cfg(not(any(target_os = "macos", target_os = "windows")))]
            {
                let _ = window.show();
            }
        }
    }
}

fn perform_close_style_window(app: &tauri::AppHandle) {
    if let Some(window) = app.get_webview_window("style_selector") {
        if window.is_visible().unwrap_or(false) {
            let _ = window.hide();
        }
    }
}

fn perform_toggle_style_window(app: &tauri::AppHandle) {
    if let Some(window) = app.get_webview_window("style_selector") {
        if window.is_visible().unwrap_or(false) {
            perform_close_style_window(app);
        } else {
            perform_open_style_window(app);
        }
    }
}

#[tauri::command]
fn toggle_style_window(app: tauri::AppHandle) {
    perform_toggle_style_window(&app);
}

#[tauri::command]
async fn transcribe_audio(audio_data: Vec<f32>) -> Result<String, String> {
    transcription::transcribe(audio_data).await
}

#[tauri::command]
fn type_text(text: String) -> Result<(), String> {
    typing::type_string(&text)
}

#[tauri::command]
fn paste_text(text: String, app: tauri::AppHandle) -> Result<(), String> {
    typing::paste_text(&text, &app)
}

#[tauri::command]
fn format_basic(text: String) -> String {
    typing::format_basic(&text)
}

#[tauri::command]
async fn process_with_ai(
    app: tauri::AppHandle,
    text: String, 
    action: String, 
    system_prompt: String,
    model: String
) -> Result<String, String> {
    let settings = settings::load_settings(&app);
    typing::process_with_ai(&text, &action, &system_prompt, &model, None, settings.user_name, settings.target_language, None).await
}

#[tauri::command]
fn check_permissions() -> permissions::PermissionStatus {
    permissions::check_all_permissions()
}

#[tauri::command]
fn open_accessibility_settings(app: tauri::AppHandle) {
    permissions::open_accessibility_preferences(&app);
}

#[tauri::command]
fn open_microphone_settings(app: tauri::AppHandle) {
    permissions::open_microphone_preferences(&app);
}

#[tauri::command]
fn open_input_monitoring_settings(app: tauri::AppHandle) {
    permissions::open_input_monitoring_preferences(&app);
}

#[tauri::command]
fn check_model_status(app: tauri::AppHandle) -> model_download::ModelStatus {
    model_download::check_model_status(&app)
}

#[tauri::command]
async fn download_model(app: tauri::AppHandle, window: tauri::WebviewWindow) -> Result<(), String> {
    model_download::download_model(&app, &window).await
}

#[tauri::command]
fn get_model_size() -> String {
    "~478 MB".to_string()
}

#[tauri::command]
async fn get_available_models(app: tauri::AppHandle) -> Result<Vec<models::ModelInfo>, String> {
    let mut models = models::ModelInfo::all_models();
    
    let app_handle = app.clone();
    let manager_result = tauri::async_runtime::spawn_blocking(move || {
        model_manager::ModelManager::new(&app_handle)
    }).await.map_err(|e| e.to_string())?;

    let manager = match manager_result {
        Ok(m) => Some(m),
        Err(e) => {
            println!("Error creating ModelManager: {}", e);
            None
        }
    };
    
    // Get active model ID from settings
    let state: tauri::State<AppState> = app.state();
    let active_id = state.settings.lock().unwrap().active_model_id.clone();
    
    // We need to run file checks in a blocking thread too if they are slow
    if let Some(manager) = manager {
        let manager = std::sync::Arc::new(manager);
        let mut models_clone = models.clone();
        
        let models_result = tauri::async_runtime::spawn_blocking(move || {
             for model in &mut models_clone {
                let installed = manager.is_model_installed(model);
                // println!("🔍 Checking model {}: installed={}", model.id, installed);
                model.installed = installed;
             }
             models_clone
        }).await.map_err(|e| e.to_string())?;
        
        models = models_result;
    } else {
        for model in &mut models {
            model.installed = false;
        }
    }
    
    for model in &mut models {
         // Sync is_active with settings
        model.is_active = model.id == active_id;
    }
    
    Ok(models)
}

#[tauri::command]
async fn download_ollama_model(app: tauri::AppHandle, model: String) -> Result<(), String> {
    typing::download_ollama_model(&app, &model).await
}


#[tauri::command]
async fn get_installed_ollama_models() -> Result<Vec<typing::OllamaModel>, String> {
    typing::get_installed_ollama_models().await
}

#[tauri::command]
fn get_active_model(state: tauri::State<'_, AppState>) -> String {
    // Return the currently active model ID from settings
    state.settings.lock().unwrap().active_model_id.clone()
}

#[tauri::command]
fn set_transcription_engine(engine: String) -> Result<(), String> {
    #[cfg(not(target_os = "macos"))]
    {
        if engine.to_lowercase() != "parakeet" {
            return Err("Only Parakeet is supported in this build.".to_string());
        }
    }
    transcription::switch_engine(&engine).map_err(|e| e.to_string())
}

#[tauri::command]
async fn download_model_by_id(model_id: String, app: tauri::AppHandle) -> Result<(), String> {
    // Get model info
    let models = models::ModelInfo::all_models();
    let model = models.iter().find(|m| m.id == model_id)
        .ok_or("Model not found")?.clone();
    
    println!("Downloading model: {} from {}", model.name, model.url);
    
    // Create model manager and download
    let manager = model_manager::ModelManager::new(&app)
        .map_err(|e| e.to_string())?;
    
    manager.download_model(&model, &app).await
        .map_err(|e| e.to_string())?;
    
    println!("Model {} downloaded successfully!", model.name);

    Ok(())
}

#[tauri::command]
fn load_model_by_id(model_id: String, app: tauri::AppHandle) -> Result<(), String> {
    // Get model info
    let models = models::ModelInfo::all_models();
    let model = models.iter().find(|m| m.id == model_id)
        .ok_or("Model not found")?.clone();
    
    // Check if installed
    let manager = model_manager::ModelManager::new(&app)
        .map_err(|e| e.to_string())?;
        
    let final_path_buf = manager.resolve_model_path(&model)
        .ok_or("Model not installed (path resolution failed)".to_string())?;
    
    // Switch engine based on model type
    let engine_type = match model.model_type {
        models::ModelType::ParakeetV3 => "parakeet",
        _ => return Err("Only Parakeet V3 is supported in this build.".to_string()),
    };
    
    println!("Switching to engine: {}", engine_type);
    transcription::switch_engine(engine_type).map_err(|e| e.to_string())?;
    
    println!("Loading model from: {:?}", final_path_buf);
    transcription::load_model(final_path_buf.to_str().unwrap()).map_err(|e| e.to_string())?;
    
    // Update settings
    let state: tauri::State<AppState> = app.state();
    {
        let mut settings = state.settings.lock().unwrap();
        settings.active_model_id = model.id.clone();
        settings.transcription_engine = engine_type.to_string();
        settings::save_settings(&app, &settings).map_err(|e| e.to_string())?;
    }
    
    Ok(())
}

#[tauri::command]
fn reset_models(app: tauri::AppHandle) -> Result<(), String> {
    // Unload the model first to release file locks
    if let Err(e) = transcription::unload_model() {
        eprintln!("⚠️ Failed to unload model before reset: {}", e);
        // Continue anyway, as we might be resetting because of a bad load
    }
    
    let manager = model_manager::ModelManager::new(&app)
        .map_err(|e| e.to_string())?;
    manager.reset_models().map_err(|e| e.to_string())
}

#[tauri::command]
fn restart_app(app: tauri::AppHandle) {
    app.restart();
}

#[tauri::command]
async fn get_model_paths(app: tauri::AppHandle) -> Result<serde_json::Value, String> {
    let app_handle = app.clone();
    
    // Run in blocking thread to avoid freezing main thread on file operations
    let paths = tauri::async_runtime::spawn_blocking(move || {
        let settings = settings::load_settings(&app_handle);
        let core_path = if let Some(custom_path) = settings.custom_models_path {
            custom_path
        } else {
            app_handle.path()
                .app_local_data_dir()
                .map(|p| p.join("models").to_string_lossy().to_string())
                .unwrap_or_else(|_| "Error getting app local data dir".to_string())
        };
        
        // Return JSON object matching frontend interface
        serde_json::json!({
            "core": core_path,
            "text": "Managed by Ollama (check Ollama app)",
            "vision": "Managed by Ollama (check Ollama app)"
        })
    }).await.map_err(|e| e.to_string())?;

    Ok(paths)
}

#[tauri::command]
fn configure_ollama_path(path: String) -> Result<String, String> {
    let _ = path; // Suppress unused warning
    Ok("Note: Ollama models are managed by the Ollama app itself. Please configure the models path in Ollama's settings if needed.".to_string())
}

#[tauri::command]
fn cancel_recording(state: tauri::State<'_, AppState>, app: tauri::AppHandle) {
    let audio_manager = state.audio_manager.clone();
    audio_manager.cancel_recording();
    *state.is_recording.lock().unwrap() = false;
    
    // Hide overlay
    overlay::hide_recording_overlay(&app);
    
    // Emit stop-recording event
    let _ = app.emit("stop-recording", ());
    println!("🚫 Recording CANCELLED by user");
}

#[cfg(target_os = "macos")]
fn init_nspanel<R: tauri::Runtime>(builder: tauri::Builder<R>) -> tauri::Builder<R> {
    builder.plugin(tauri_nspanel::init())
}

#[cfg(not(target_os = "macos"))]
fn init_nspanel<R: tauri::Runtime>(builder: tauri::Builder<R>) -> tauri::Builder<R> {
    builder
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let mut builder = tauri::Builder::default()
        .plugin(tauri_plugin_store::Builder::new().build())
        .plugin(tauri_plugin_os::init())
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_autostart::init(MacosLauncher::LaunchAgent, Some(vec![])))
        .plugin(tauri_plugin_single_instance::init(|app, _, _| {
            overlay::show_hud_message(app, "Already running - focusing window");
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.show();
                let _ = window.set_focus();
            }
        }));

    builder = init_nspanel(builder);
    
    builder
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .plugin(tauri_plugin_macos_permissions::init())
        .setup(|app| {
            // Create App Menu
            let menu = menu::create_menu(app.handle())?;
            app.set_menu(menu)?;

            // Handle Menu Events
            app.on_menu_event(|app, event| {
                match event.id.as_ref() {
                    "contact_email" => {
                         let _ = std::process::Command::new("open")
                             .arg("mailto:coderjazzguitarist-1@yahoo.com")
                             .spawn();
                    }
                    "highlight_text_mode" => {
                        let state: tauri::State<AppState> = app.state();
                        let mut settings = state.settings.lock().unwrap();
                        settings.highlight_mode = "text".to_string();
                        settings::save_settings(app, &settings).unwrap_or_default();
                        
                        // Emit update
                        let _ = app.emit("settings-updated", settings.clone());
                        
                        // Show HUD
                        overlay::show_hud_message(app, "Switched to Text Selection Mode");
                    }
                    "highlight_screenshot_mode" => {
                        let state: tauri::State<AppState> = app.state();
                        let mut settings = state.settings.lock().unwrap();
                        settings.highlight_mode = "screenshot".to_string();
                        settings::save_settings(app, &settings).unwrap_or_default();
                        
                        // Emit update
                        let _ = app.emit("settings-updated", settings.clone());
                        
                        // Show HUD
                        overlay::show_hud_message(app, "Switched to Screenshot Mode");
                    }
                    "open_models_folder" => {
                        let state: tauri::State<AppState> = app.state();
                        let settings = state.settings.lock().unwrap();
                        let path = if let Some(custom) = &settings.custom_models_path {
                            custom.clone()
                        } else {
                             app.path().download_dir().map(|p| p.join("Downloaded Files").to_string_lossy().to_string()).unwrap_or_default()
                        };
                        let _ = std::process::Command::new("open")
                             .arg(path)
                             .spawn();
                    }
                    "change_model_path" => {
                        // Bring window to front
                        if let Some(window) = app.get_webview_window("main") {
                            let _ = window.show();
                            let _ = window.set_focus();
                        }
                        let _ = app.emit("menu-change-model-path", ());
                    }
                    _ => {}
                }
            });

            // Force main window to be visible and focused on launch
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.show();
                let _ = window.set_focus();
            }
            // Register panic hook
            std::panic::set_hook(Box::new(|info| {
                eprintln!("🔥 PANIC: {:?}", info);
                // Write to a file for persistence
                if let Ok(mut file) = std::fs::File::create("panic.log") {
                    use std::io::Write;
                    let _ = writeln!(file, "Panic: {:?}", info);
                }
            }));

            // Create recording overlay
            overlay::create_recording_overlay(app.handle());
            
            // Create HUD window
            overlay::create_hud_window(app.handle());

            // Load settings from disk (persisted from previous session)
            let loaded_settings = settings::load_settings(app.handle());
            println!("📝 Loaded settings: shortcut={}, mic={}", 
                     loaded_settings.push_to_talk_key, 
                     loaded_settings.input_device_id);
            
            // Initialize the audio recording manager
            let audio_manager = Arc::new(AudioRecordingManager::new(app.handle())?);
            app.manage(audio_manager.clone());
            
            // Initialize microphone stream for testing
            println!("🎤 Initializing microphone stream for testing...");
            if let Err(e) = audio_manager.start_microphone_stream() {
                eprintln!("⚠️ Failed to initialize microphone stream: {}", e);
            } else {
                println!("✅ Microphone stream initialized successfully");
            }
            
            let state = AppState {
                is_recording: Arc::new(Mutex::new(false)),
                settings: Arc::new(Mutex::new(loaded_settings)),
                model_loaded: Arc::new(Mutex::new(false)),
                current_shortcut: Arc::new(Mutex::new(None)),
                current_style_shortcut: Arc::new(Mutex::new(None)),
                current_raw_mode_shortcut: Arc::new(Mutex::new(None)),
                current_highlight_shortcut: Arc::new(Mutex::new(None)),
                audio_manager,
                last_selection: Arc::new(Mutex::new(None)),
                last_screenshot: Arc::new(Mutex::new(None)),
                pending_audio: Arc::new(Mutex::new(None)),
                highlight_status: Arc::new(Mutex::new(HighlightStatus::Idle)),
            };
            app.manage(state);
            
            // Audio manager is now initialized automatically by AudioRecordingManager::new
            
             // Setup tray icon
             let tray = tray::create_tray(app.handle())?;
             app.manage(tray);

             // Keyboard listener removed in favor of tauri-plugin-global-shortcut
             keyboard::start_listener(app.handle().clone());

             // Listen for internal voice command triggers (hands-free)
             let handle = app.handle().clone();
             app.listen("internal-voice-command", move |event: tauri::Event| {
                 if let Ok(payload) = serde_json::from_str::<String>(event.payload()) {
                     println!("⚡ Internal Voice Command: {}", payload);
                     if payload == "open" {
                        let h = handle.clone();
                        let _ = handle.run_on_main_thread(move || {
                            perform_open_style_window(&h);
                        });
                     } else if payload == "close" {
                        let h = handle.clone();
                        let _ = handle.run_on_main_thread(move || {
                            perform_close_style_window(&h);
                        });
                     }
                 }
             });

             // Register global shortcut using tauri-plugin-global-shortcut
            use tauri_plugin_global_shortcut::{GlobalShortcutExt, ShortcutState};
            let _ = app.global_shortcut().unregister_all();
            
            // Get shortcut from settings
            let shortcut_str = {
                let state: tauri::State<AppState> = app.state();
                let settings = state.settings.lock().unwrap();
                // Normalize shortcut format for tauri-plugin-global-shortcut
                // "Option+Space" -> "option+space", "CommandOrControl+Shift+A" -> "commandorcontrol+shift+a"
                settings.push_to_talk_key.to_lowercase().replace("option", "alt")
            };
            
            println!("🎹 Registering global shortcut: {}", shortcut_str);
            
            if let Ok(shortcut) = shortcut_str.parse::<tauri_plugin_global_shortcut::Shortcut>() {
                println!("✅ Shortcut parsed successfully");
                
                let shortcut_result = app.global_shortcut().on_shortcut(shortcut, move |ah, _, event| {
                if event.state == ShortcutState::Pressed {
                    println!("👇 Shortcut PRESSED event received");
                    let state: tauri::State<AppState> = ah.state();
                    let model_loaded = state.model_loaded.lock().unwrap();
                    if !*model_loaded {
                        println!("⚠️ Model NOT loaded status: {}", *model_loaded);
                        // Temporarily allow recording even if model check fails, to debug mic
                        // drop(model_loaded);
                        // let _ = ah.emit("model-not-loaded", ());
                        // println!("❌ Model not loaded, cannot start recording");
                        // return;
                    }
                    drop(model_loaded);

                    // Start recording
                    let audio_manager = state.audio_manager.clone();
                    if !audio_manager.try_start_recording("main") {
                        eprintln!("❌ Failed to start recording");
                        return;
                    }
                    *state.is_recording.lock().unwrap() = true;

                    println!("🎤 Shortcut PRESSED - Recording started");
                    let _ = ah.emit("start-recording", ());
                    
                    // Play start sound
                    audio_feedback::play_start_sound(ah);
                    
                    // Show overlay
                    overlay::show_recording_overlay(ah);
                    
                    // Chat Mode: REMOVED
                    *state.last_selection.lock().unwrap() = None;
                } else if event.state == ShortcutState::Released {
                    println!("☝️ Shortcut RELEASED event received");
                    let state: tauri::State<AppState> = ah.state();
                    let audio_data = {
                        let audio_manager = state.audio_manager.clone();
                        audio_manager.stop_recording("main").unwrap_or_default()
                    };
                    *state.is_recording.lock().unwrap() = false;

                    println!("🛑 Shortcut RELEASED - Processing {} samples", audio_data.len());
                    let _ = ah.emit("stop-recording", ());

                    // Play stop sound
                    audio_feedback::play_stop_sound(ah);

                    // Process audio if we have data
                    if !audio_data.is_empty() {
                         // Show transcribing overlay
                         overlay::show_transcribing_overlay(ah);

                        let settings = state.settings.lock().unwrap().clone();
                        let ah_clone = ah.clone();
                        let last_selection = state.last_selection.clone();
                        let last_screenshot = state.last_screenshot.clone();
                        tauri::async_runtime::spawn(async move {
                            let audio_len = audio_data.len();
                            match transcription::transcribe(audio_data).await {
                                Ok(text) => {
                                    if text.trim().is_empty() {
                                        println!("⚠️ No transcription result");
                                        overlay::hide_recording_overlay(&ah_clone);
                                        return;
                                    }
                                    println!("✅ Transcription: {}", text);

                                    // Emit stats event
                                    let duration_sec = audio_len as f32 / audio_toolkit::constants::WHISPER_SAMPLE_RATE as f32;
                                    let word_count = text.split_whitespace().count();
                                    
                                    #[derive(Serialize, Clone)]
                                    struct StatsPayload {
                                        word_count: usize,
                                        duration_sec: f32,
                                    }
                                    
                                    let _ = ah_clone.emit("transcription-stats", StatsPayload { word_count, duration_sec });

                                    // Get context from Chat Mode if available
                                    let context = last_selection.lock().unwrap().take();
                                    if let Some(ref ctx) = context {
                                        println!("📋 Using context ({} chars)", ctx.len());
                                    }
                                    
                                    // Get screenshot context if available
                                    let screenshot_context = last_screenshot.lock().unwrap().take();
                                    if let Some(ref img) = screenshot_context {
                                        println!("📸 Using screenshot context ({} chars)", img.len());
                                    }
                                    
                                    // Check if we should show result in window (Highlight mode)
                                    let show_result_window = context.is_some() || screenshot_context.is_some();

                                    let processed_text = match settings.correction_mode.as_str() {
                                        "local" => {
                                            // Try AI processing
                                            match typing::process_with_ai(&text, "edit", &settings.system_prompt, &settings.ollama_model, context, settings.user_name.clone(), settings.target_language.clone(), screenshot_context).await {
                                                Ok(processed) => processed,
                                                Err(e) => {
                                                    eprintln!("AI processing failed: {}, using basic", e);
                                                    typing::format_basic(&text)
                                                }
                                            }
                                        },
                                        "openai" => {
                                            if let Some(key) = &settings.openai_api_key {
                                                if key.is_empty() {
                                                    eprintln!("OpenAI API key empty, using basic");
                                                    typing::format_basic(&text)
                                                } else {
                                                    match typing::process_with_openai(&text, &settings.system_prompt, &settings.openai_model, key, context, settings.user_name.clone(), settings.target_language.clone(), screenshot_context).await {
                                                        Ok(processed) => processed,
                                                        Err(e) => {
                                                            eprintln!("OpenAI processing failed: {}, using basic", e);
                                                            typing::format_basic(&text)
                                                        }
                                                    }
                                                }
                                            } else {
                                                eprintln!("OpenAI API key missing, using basic");
                                                typing::format_basic(&text)
                                            }
                                        },
                                        "openrouter" => {
                                            if let Some(key) = &settings.openrouter_api_key {
                                                if key.is_empty() {
                                                    eprintln!("OpenRouter API key empty, using basic");
                                                    typing::format_basic(&text)
                                                } else {
                                                    match typing::process_with_openrouter(&text, &settings.system_prompt, &settings.openrouter_model, key, context, settings.user_name.clone(), settings.target_language.clone(), screenshot_context).await {
                                                        Ok(processed) => processed,
                                                        Err(e) => {
                                                            eprintln!("OpenRouter processing failed: {}, using basic", e);
                                                            typing::format_basic(&text)
                                                        }
                                                    }
                                                }
                                            } else {
                                                eprintln!("OpenRouter API key missing, using basic");
                                                typing::format_basic(&text)
                                            }
                                        },
                                        "none" => {
                                            // Raw transcription without any formatting
                                            text
                                        },
                                        _ => {
                                            // "basic" or unknown -> just basic formatting
                                            typing::format_basic(&text)
                                        }
                                    };

                                    // Handle result (Paste or Show Window)
                                    let result_text = processed_text.clone();
                                    let ah_handle = ah_clone.clone();
                                    let auto_copy = settings.auto_copy;
                                    
                                    let _ = ah_clone.run_on_main_thread(move || {
                                        // Hide overlay FIRST
                                        overlay::hide_recording_overlay(&ah_handle);
                                        
                                        if show_result_window {
                                            println!("🖍️ Highlight Context detected - Showing Result Window");
                                            if let Some(window) = ah_handle.get_webview_window("highlight_result") {
                                                // Show window
                                                let _ = window.show();
                                                let _ = window.set_focus();
                                                let _ = window.set_always_on_top(true);
                                                
                                                // Emit event with result
                                                if let Err(e) = window.emit("show-highlight-result", &result_text) {
                                                    eprintln!("Failed to emit highlight result: {}", e);
                                                }
                                                
                                                // Also copy to clipboard if auto_copy is on
                                                if auto_copy {
                                                    use tauri_plugin_clipboard_manager::ClipboardExt;
                                                    let _ = ah_handle.clipboard().write_text(&result_text);
                                                }
                                            } else {
                                                eprintln!("❌ Could not find highlight_result window");
                                            }
                                        } else {
                                            // Standard Paste Mode
                                            // Wait a bit for focus to restore
                                            std::thread::sleep(std::time::Duration::from_millis(400));
    
                                            if let Err(e) = typing::paste_text(&result_text, &ah_handle) {
                                                eprintln!("Failed to paste text: {}", e);
                                            } else {
                                                println!("✅ Pasted: {}", result_text);
                                                if auto_copy {
                                                    use tauri_plugin_clipboard_manager::ClipboardExt;
                                                    let _ = ah_handle.clipboard().write_text(&result_text);
                                                }
                                            }
                                        }
                                    });
                                }
                                Err(e) => {
                                    eprintln!("Transcription failed: {}", e);
                                    overlay::hide_recording_overlay(&ah_clone);
                                },
                            }
                        });
                    } else {
                        // No audio data, just hide
                        overlay::hide_recording_overlay(ah);
                    }
                }
                });
                if let Err(e) = shortcut_result {
                    eprintln!("❌ Failed to register shortcut: {}", e);
                } else {
                    println!("✅ Global shortcut registered successfully!");
                    // Store current shortcut in state
                    let state: tauri::State<AppState> = app.state();
                    *state.current_shortcut.lock().unwrap() = Some(shortcut_str.clone());
                }
            } else {
                eprintln!("❌ Failed to parse shortcut string: {}", shortcut_str);
            }
            
            // --- Register Style Selector Shortcut ---
            let style_shortcut_str = {
                let state: tauri::State<AppState> = app.state();
                let settings = state.settings.lock().unwrap();
                settings
                    .style_selector_key
                    .as_deref()
                    .map(normalize_shortcut_str)
                    .filter(|s| !s.is_empty())
            };

            if let Some(style_shortcut_str) = style_shortcut_str {
                println!("🎹 Registering Style Selector shortcut: {}", style_shortcut_str);
                if let Ok(style_shortcut) = style_shortcut_str.parse::<tauri_plugin_global_shortcut::Shortcut>() {
                    let _ = app.global_shortcut().on_shortcut(style_shortcut, move |ah, _, event| {
                        if event.state == ShortcutState::Pressed {
                            if let Some(window) = ah.get_webview_window("style_selector") {
                                if window.is_visible().unwrap_or(false) {
                                    let _ = window.hide();
                                } else {
                                    let _ = window.show();
                                    let _ = window.set_focus();
                                }
                            }
                        }
                    });
                    println!("✅ Style Selector shortcut registered successfully!");
                    let state: tauri::State<AppState> = app.state();
                    *state.current_style_shortcut.lock().unwrap() = Some(style_shortcut_str.clone());
                } else {
                    eprintln!("❌ Failed to parse Style Selector shortcut");
                }
            }

            // --- Register Raw Mode Toggle Shortcut ---
            let raw_mode_shortcut_str = {
                let state: tauri::State<AppState> = app.state();
                let settings = state.settings.lock().unwrap();
                settings
                    .raw_mode_toggle_key
                    .as_deref()
                    .map(normalize_shortcut_str)
                    .filter(|s| !s.is_empty())
            };

            if let Some(raw_mode_shortcut_str) = raw_mode_shortcut_str {
                if !raw_mode_shortcut_str.is_empty() {
                    println!("🎹 Registering Raw Mode Toggle shortcut: {}", raw_mode_shortcut_str);
                    if let Ok(raw_mode_shortcut) = raw_mode_shortcut_str.parse::<tauri_plugin_global_shortcut::Shortcut>() {
                        let ah_clone = app.handle().clone();
                        let _ = app.global_shortcut().on_shortcut(raw_mode_shortcut, move |ah, _, event| {
                            if event.state == ShortcutState::Pressed {
                                let state: tauri::State<AppState> = ah.state();
                                println!("🎤 Raw Mode Toggle Shortcut PRESSED");
                                
                                // Toggle logic
                                let mut settings = state.settings.lock().unwrap();
                                let current_mode = settings.correction_mode.clone();
                                let last_mode = settings.last_llm_mode.clone();
                                
                                let new_mode = if current_mode == "none" {
                                    // Switch to last used LLM mode
                                    if last_mode == "none" {
                                        "local".to_string() // Fallback
                                    } else {
                                        last_mode
                                    }
                                } else {
                                    // Switch to Raw Mode
                                    // Update last_llm_mode before switching
                                    settings.last_llm_mode = current_mode;
                                    "none".to_string()
                                };
                                
                                settings.correction_mode = new_mode.clone();
                                
                                // Save settings
                                let _ = settings::save_settings(&ah_clone, &settings);
                                
                                // Emit update to frontend
                                let _ = ah_clone.emit("settings-updated", settings.clone());
                                
                                // Show notification/log
                                println!("🔄 Toggled Correction Mode to: {}", new_mode);
                                
                                // Show HUD
                                let mode_text = if new_mode == "none" { "Raw" } else { "LLM" };
                                let message = format!("Toggled to {} Mode", mode_text);
                                overlay::show_hud_message(&ah_clone, &message);
                            }
                        });
                        println!("✅ Raw Mode Toggle shortcut registered successfully!");
                        let state: tauri::State<AppState> = app.state();
                        *state.current_raw_mode_shortcut.lock().unwrap() = Some(raw_mode_shortcut_str.clone());
                    } else {
                        eprintln!("❌ Failed to parse Raw Mode Toggle shortcut");
                    }
                }
            }

            // --- Register Highlight Shortcut ---
            let highlight_shortcut_str = {
                let state: tauri::State<AppState> = app.state();
                let settings = state.settings.lock().unwrap();
                settings
                    .highlight_shortcut
                    .as_deref()
                    .map(normalize_shortcut_str)
                    .filter(|s| !s.is_empty())
            };

            if let Some(highlight_shortcut_str) = highlight_shortcut_str {
                if !highlight_shortcut_str.is_empty() {
                    println!("🖍️ Registering Highlight shortcut: {}", highlight_shortcut_str);
                    if let Ok(highlight_shortcut) = highlight_shortcut_str.parse::<tauri_plugin_global_shortcut::Shortcut>() {
                        let _ = app.global_shortcut().on_shortcut(highlight_shortcut, move |ah, _, event| {
                            if event.state == ShortcutState::Pressed {
                                let state: tauri::State<AppState> = ah.state();
                                println!("🖍️ Highlight Shortcut PRESSED");
                                
                                // 1. Play Start Sound
                                audio_feedback::play_start_sound(ah);

                                // 2. Show Highlight Overlay (Pill)
                                overlay::show_highlight_overlay(ah);

                                // 3. Start Recording
                                let mut is_recording = state.is_recording.lock().unwrap();
                                if !*is_recording {
                                    if state.audio_manager.try_start_recording("main") {
                                        *is_recording = true;
                                        println!("🎤 Recording started (Highlight Mode)");
                                    } else {
                                        eprintln!("Failed to start recording");
                                    }
                                }
                                drop(is_recording);
                                
                                // 4. Trigger Highlight Action
                                // Check mode (text or screenshot)
                                let mode = {
                                    let settings = state.settings.lock().unwrap();
                                    settings.highlight_mode.clone()
                                };

                                if mode == "text" {
                                    // Text Highlight Mode
                                    commands::highlight::handle_text_highlight(ah);
                                    
                                    // Show recording overlay (pill)
                                    overlay::show_highlight_overlay(ah);
                                } else {
                                    // Screenshot Mode - Start Recording ONLY
                                    // Selection overlay will show on RELEASE
                                    
                                    // Clear previous screenshot & pending audio
                                    *state.last_screenshot.lock().unwrap() = None;
                                    *state.pending_audio.lock().unwrap() = None;
                                    
                                    // Show recording overlay (pill)
                                    overlay::show_highlight_overlay(ah);
                                }
                            } else if event.state == ShortcutState::Released {
                                let state: tauri::State<AppState> = ah.state();
                                println!("🖍️ Highlight Shortcut RELEASED");

                                // 1. Play Stop Sound
                                audio_feedback::play_stop_sound(ah);

                                // 2. Stop Recording & Process
                                let mut is_recording = state.is_recording.lock().unwrap();
                                if *is_recording {
                                    let audio_data = state.audio_manager.stop_recording("main").unwrap_or_default();
                                    *is_recording = false;
                                    drop(is_recording); // Drop lock before processing

                                    if !audio_data.is_empty() {
                                        // Check mode
                                        let mode = {
                                            let settings = state.settings.lock().unwrap();
                                            settings.highlight_mode.clone()
                                        };

                                        if mode == "screenshot" {
                                            println!("⏳ Audio buffered, waiting for screenshot selection...");
                                            *state.pending_audio.lock().unwrap() = Some(audio_data);
                                            
                                            // Hide Highlight Overlay (Pill)
                                            overlay::hide_recording_overlay(ah);
                                            
                                            // Show Selection Overlay NOW
                                            if let Some(window) = ah.get_webview_window("selection_overlay") {
                                                // Reset selection state in frontend
                                                let _ = window.emit("reset-selection", ());
                                                
                                                // Ensure visible
                                                let _ = window.set_fullscreen(false); // Explicitly disable fullscreen
                                                let _ = window.unmaximize(); // Ensure not maximized
                                                let _ = window.show();
                                                let _ = window.set_focus();
                                                let _ = window.set_always_on_top(true);
                                                
                                                #[cfg(target_os = "macos")]
                                                {
                                                    let _ = window.set_focus();
                                                }
                                            } else {
                                                eprintln!("❌ Selection window not found!");
                                            }
                                        } else {
                                            println!("Processing {} samples (Highlight Mode)", audio_data.len());
                                            
                                            // Hide Highlight Overlay (Pill)
                                            overlay::hide_recording_overlay(ah);
                                            
                                            // Show Highlight Result Window immediately with "Processing..."
                                            if let Some(window) = ah.get_webview_window("highlight_result") {
                                                let _ = window.show();
                                                let _ = window.set_focus();
                                                let _ = window.set_always_on_top(true);
                                                let _ = window.set_ignore_cursor_events(false);
                                                let _ = window.emit("processing-started", ());
                                            }
                                            
                                            // Process with AI and submit (is_highlight_flow = true)
                                            process_audio_and_submit(ah.clone(), audio_data, true);
                                        }
                                    } else {
                                        println!("⚠️ No audio data captured");
                                        // Hide Highlight Overlay (Pill) even if no audio
                                        overlay::hide_recording_overlay(ah);
                                    }
                                } else {
                                    // Ensure pill is hidden if not recording
                                    overlay::hide_recording_overlay(ah);
                                }
                            }

                        });
                        println!("✅ Highlight shortcut registered successfully!");
                        let state: tauri::State<AppState> = app.state();
                        *state.current_highlight_shortcut.lock().unwrap() = Some(highlight_shortcut_str.clone());
                    } else {
                        eprintln!("❌ Failed to parse Highlight shortcut");
                    }
                }
            }

            // Hide dock icon on Mac (menu bar app)
            #[cfg(target_os = "macos")]
            app.set_activation_policy(tauri::ActivationPolicy::Accessory);
            
            // Check permissions on startup
            let perms = permissions::check_all_permissions();
            
            // --- Startup Model Loading (Async to prevent blocking UI) ---
            println!("🚀 Checking model status on startup...");
            
            let app_handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                // Get active model settings
                let (active_model_id, engine) = {
                    let state: tauri::State<AppState> = app_handle.state();
                    let settings = state.settings.lock().unwrap();
                    (settings.active_model_id.clone(), settings.transcription_engine.clone())
                };
                
                println!("Target Model: {}, Engine: {}", active_model_id, engine);
    
                let mut loaded = false;
                
                if let Ok(manager) = model_manager::ModelManager::new(&app_handle) {
                     let all_models = models::ModelInfo::all_models();
                     if let Some(model) = all_models.iter().find(|m| m.id == active_model_id) {
                         if manager.is_model_installed(model) {
                             println!("✅ Active model is installed. Attempting to load...");
                             
                             // Switch engine
                             if let Err(e) = transcription::switch_engine(&engine) {
                                 eprintln!("Failed to switch engine: {}", e);
                             } else {
                                 // Get path using unified resolution
                                 if let Some(final_path_buf) = manager.resolve_model_path(model) {
                                     if let Some(p) = final_path_buf.to_str() {
                                         if let Err(e) = transcription::load_model(p) {
                                             eprintln!("Failed to load model: {}", e);
                                             let _ = app_handle.emit("model-error", format!("Failed to load model: {}", e));
                                         } else {
                                             println!("✅ Model loaded successfully from: {:?}", final_path_buf);
                                             let state: tauri::State<AppState> = app_handle.state();
                                             *state.model_loaded.lock().unwrap() = true;
                                             let _ = app_handle.emit("model-ready", ());
                                             loaded = true;
                                         }
                                     }
                                 } else {
                                     eprintln!("❌ Model path resolution failed for installed model {}", model.id);
                                 }
                             }
                         }
                     }
                }
                
                if !loaded {
                    println!("⚠️ Active model not loaded.");
                    // Disabled auto-download to prevent "stuck" state on startup
                    // if active_model_id == "parakeet-v3" {
                    //    model_download::auto_download_model(app_handle.clone());
                    // } else {
                         println!("Active model '{}' not installed. Waiting for user action.", active_model_id);
                    // }
                }
            });

            if !perms.accessibility || !perms.microphone {
                // Show settings window if permissions missing
                if let Some(window) = app.get_webview_window("main") {
                    let _ = window.show();
                    let _ = window.set_focus();
                }
            }
            
            Ok(())
        })
        .on_window_event(|window, event| {
            // Hide window instead of closing to keep tray icon alive
            if let WindowEvent::CloseRequested { api, .. } = event {
                window.hide().unwrap();
                api.prevent_close();
            }
        })
        .on_menu_event(|app, event| {
            match event.id.as_ref() {
                "settings" => {
                    if let Some(window) = app.get_webview_window("main") {
                        let _ = window.show();
                        let _ = window.set_focus();
                        let _ = window.emit("open-settings", "general");
                    }
                }
                "change_model_path" => {
                    if let Some(window) = app.get_webview_window("main") {
                        let _ = window.show();
                        let _ = window.set_focus();
                        // Emit event to frontend to switch to storage settings
                        let _ = window.emit("open-settings", "storage");
                    }
                }
                "open_models_folder" => {
                    let state: tauri::State<AppState> = app.state();
                    let path = {
                        let settings = state.settings.lock().unwrap();
                        if let Some(custom_path) = settings.custom_models_path.clone() {
                            std::path::PathBuf::from(custom_path)
                        } else {
                            app.path()
                                .download_dir()
                                .unwrap_or_else(|_| std::path::PathBuf::from("."))
                                .join("Downloaded Files")
                        }
                    };
                    let _ = std::process::Command::new("open")
                        .arg(path)
                        .spawn();
                }
                "highlight_text_mode" => {
                    let state: tauri::State<AppState> = app.state();
                    let mut settings = state.settings.lock().unwrap();
                    settings.highlight_mode = "text".to_string();
                    let _ = settings::save_settings(app, &settings);
                    
                    // Emit update to frontend
                    let _ = app.emit("settings-updated", settings.clone());
                    
                    println!("✅ Switched to Text Selection Mode");
                    overlay::show_hud_message(app, "Switched to Text Selection Mode");
                }
                "highlight_screenshot_mode" => {
                    let state: tauri::State<AppState> = app.state();
                    let mut settings = state.settings.lock().unwrap();
                    settings.highlight_mode = "screenshot".to_string();
                    let _ = settings::save_settings(app, &settings);
                    
                    // Emit update to frontend
                    let _ = app.emit("settings-updated", settings.clone());
                    
                    println!("✅ Switched to Screenshot Mode");
                    overlay::show_hud_message(app, "Switched to Screenshot Mode");
                }
                "contact_email" => {
                     let _ = std::process::Command::new("open")
                         .arg("mailto:coderjazzguitarist-1@yahoo.com")
                         .spawn();
                }
                 _ => {}
            }
        })
        .invoke_handler(tauri::generate_handler![
            get_settings,
            save_settings,
            toggle_style_window,
            get_audio_devices,
            transcribe_audio,
            type_text,
            paste_text,
            format_basic,
            process_with_ai,
            check_permissions,
            open_accessibility_settings,
            open_microphone_settings,
            open_input_monitoring_settings,
            check_model_status,
            download_model,
            get_model_size,
            get_available_models,
            download_ollama_model,
            get_installed_ollama_models,
            typing::start_ollama,
            get_active_model,
            set_transcription_engine,
            download_model_by_id,
            load_model_by_id,
            reset_models,
            restart_app,
            cancel_recording,
            configure_ollama_path,
            get_model_paths,
            clamshell::is_laptop,
            commands::audio::get_available_microphones,
            commands::audio::set_selected_microphone,
            commands::audio::get_selected_microphone,
            commands::audio::get_available_output_devices,
            commands::audio::set_clamshell_microphone,
            commands::audio::get_clamshell_microphone,
            commands::audio::update_microphone_mode,
            commands::audio::get_microphone_mode,
            commands::audio::is_recording,
            commands::highlight::capture_screen_region,
            commands::highlight::update_highlight_rect,
            commands::highlight::clear_screenshot,
            commands::highlight::close_highlight_result,
            commands::highlight::get_highlight_status,
            openrouter::fetch_openrouter_models,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
