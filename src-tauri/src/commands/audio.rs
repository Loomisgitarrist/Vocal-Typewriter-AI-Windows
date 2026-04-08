use crate::audio_toolkit::{list_input_devices, list_output_devices};
use crate::managers::audio::{AudioRecordingManager, MicrophoneMode};
use crate::settings;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct AudioDevice {
    pub id: String,
    pub label: String,
    pub is_default: bool,
}

#[tauri::command]
pub fn get_available_microphones() -> Result<Vec<AudioDevice>, String> {
    let devices =
        list_input_devices().map_err(|e| format!("Failed to list audio devices: {}", e))?;

    // Filter out duplicate names since we use name as ID
    let mut seen_names = std::collections::HashSet::new();
    let mut result = Vec::new();

    for d in devices {
        if !seen_names.contains(&d.name) {
            seen_names.insert(d.name.clone());
            result.push(AudioDevice {
                id: d.name.clone(),
                label: d.name,
                is_default: false,
            });
        }
    }

    Ok(result)
}

#[tauri::command]
pub fn set_selected_microphone(app: AppHandle, device_name: String) -> Result<(), String> {
    // Update the audio manager to use the new device
    let rm = app.state::<std::sync::Arc<AudioRecordingManager>>();
    rm.update_selected_device()
        .map_err(|e| format!("Failed to update selected device: {}", e))?;

    Ok(())
}

#[tauri::command]
pub fn get_selected_microphone(app: AppHandle) -> Result<String, String> {
    let settings = settings::load_settings(&app);
    Ok(settings.input_device_id)
}

#[tauri::command]
pub fn get_available_output_devices() -> Result<Vec<AudioDevice>, String> {
    let devices =
        list_output_devices().map_err(|e| format!("Failed to list output devices: {}", e))?;

    // Filter out duplicate names since we use name as ID
    let mut seen_names = std::collections::HashSet::new();
    let mut result = Vec::new();

    for d in devices {
        if !seen_names.contains(&d.name) {
            seen_names.insert(d.name.clone());
            result.push(AudioDevice {
                id: d.name.clone(),
                label: d.name,
                is_default: false,
            });
        }
    }

    Ok(result)
}

#[tauri::command]
pub fn set_clamshell_microphone(app: AppHandle, device_name: String) -> Result<(), String> {
    let mut settings = settings::load_settings(&app);
    settings.clamshell_microphone = if device_name == "default" {
        None
    } else {
        Some(device_name)
    };
    settings::save_settings(&app, &settings)?;

    // Update the audio manager to use the new device
    let rm = app.state::<std::sync::Arc<AudioRecordingManager>>();
    rm.update_selected_device()
        .map_err(|e| format!("Failed to update selected device: {}", e))?;

    Ok(())
}

#[tauri::command]
pub fn get_clamshell_microphone(app: AppHandle) -> Result<String, String> {
    let settings = settings::load_settings(&app);
    Ok(settings.clamshell_microphone.unwrap_or_else(|| "default".to_string()))
}

#[tauri::command]
pub fn update_microphone_mode(app: AppHandle, always_on: bool) -> Result<(), String> {
    // Update settings
    let mut settings = settings::load_settings(&app);
    settings.always_on_microphone = always_on;
    settings::save_settings(&app, &settings)?;

    // Update the audio manager mode
    let rm = app.state::<std::sync::Arc<AudioRecordingManager>>();
    let new_mode = if always_on {
        MicrophoneMode::AlwaysOn
    } else {
        MicrophoneMode::OnDemand
    };

    rm.update_mode(new_mode)
        .map_err(|e| format!("Failed to update microphone mode: {}", e))?;

    Ok(())
}

#[tauri::command]
pub fn get_microphone_mode(app: AppHandle) -> Result<bool, String> {
    let settings = settings::load_settings(&app);
    Ok(settings.always_on_microphone)
}

#[tauri::command]
pub fn is_recording(app: AppHandle) -> Result<bool, String> {
    let rm = app.state::<std::sync::Arc<AudioRecordingManager>>();
    Ok(rm.is_recording())
}