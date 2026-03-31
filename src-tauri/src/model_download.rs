use serde::{Deserialize, Serialize};
use std::fs;
use std::io::Write;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use tauri::{AppHandle, Emitter, WebviewWindow};
use tauri::Manager;

use crate::transcription;

const MODEL_URL: &str = "https://blob.handy.computer/parakeet-v3-int8.tar.gz";
const MODEL_DIR_NAME: &str = "parakeet-tdt-0.6b-v3-int8";
const MODEL_FILE: &str = "encoder-model.int8.onnx";

static DOWNLOAD_IN_PROGRESS: AtomicBool = AtomicBool::new(false);

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelStatus {
    pub installed: bool,
    pub path: String,
    pub size: String,
}

#[derive(Clone, Serialize)]
pub struct DownloadProgress {
    pub model_id: String,
    pub downloaded: u64,
    pub total: u64,
    pub percentage: f64,
    pub status: String,
}

pub fn get_model_dir(app: &AppHandle) -> PathBuf {
    // Use ModelManager to get the correct, sandbox-verified models directory
    match crate::model_manager::ModelManager::new(app) {
        Ok(manager) => manager.get_models_dir().join(MODEL_DIR_NAME),
        Err(_) => {
            // Fallback to old logic if manager fails (unlikely)
            let data_dir = app.path().app_data_dir().unwrap_or_else(|_| PathBuf::from("."));
            data_dir.join("models").join(MODEL_DIR_NAME)
        }
    }
}

pub fn get_model_path(app: &AppHandle) -> PathBuf {
    get_model_dir(app).join(MODEL_FILE)
}

pub fn check_model_status(app: &AppHandle) -> ModelStatus {
    let model_dir = get_model_dir(app);
    let model_path = model_dir.join(MODEL_FILE);
    let installed = model_path.exists();

    ModelStatus {
        installed,
        path: model_dir.to_string_lossy().to_string(),
        size: if installed {
            match fs::metadata(&model_path) {
                Ok(m) => format!("{:.1} MB", m.len() as f64 / 1_000_000.0),
                Err(_) => "Unknown".to_string(),
            }
        } else {
            "~478 MB".to_string()
        },
    }
}

/// Auto-download model on startup (runs in background thread)
pub fn auto_download_model(app: AppHandle) {
    let model_path = get_model_path(&app);

    // Skip if already installed: load it and notify the UI.
    if model_path.exists() {
        if let Some(p) = model_path.to_str() {
            if let Err(e) = transcription::load_model(p) {
                let _ = emit_to_frontend(&app, "model-error", format!("Failed to load model: {e}"));
                return;
            }
        }
        let _ = emit_to_frontend(&app, "model-ready", ());
        return;
    }

    // Skip if already downloading
    if DOWNLOAD_IN_PROGRESS.swap(true, Ordering::SeqCst) {
        return;
    }

    let _ = emit_to_frontend(&app, "model-download-started", ());

    // Spawn background download
    std::thread::spawn(move || {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            if let Err(e) = download_model_background(&app).await {
                eprintln!("Model download failed: {}", e);
                let _ = emit_to_frontend(&app, "model-error", e);
            } else {
                let _ = emit_to_frontend(&app, "model-ready", ());
            }
            DOWNLOAD_IN_PROGRESS.store(false, Ordering::SeqCst);
        });
    });
}

fn emit_to_frontend<S: serde::Serialize + Clone>(app: &AppHandle, event: &str, payload: S) -> Result<(), tauri::Error> {
    // Prefer emitting to the main window so the frontend always receives it.
    if let Some(win) = app.get_webview_window("main") {
        win.emit(event, payload)
    } else {
        app.emit(event, payload)
    }
}

/// Background download without window reference
async fn download_model_background(app: &AppHandle) -> Result<(), String> {
    let model_dir = get_model_dir(app);
    let part_path = model_dir.parent().unwrap().join("parakeet-v3-int8.tar.gz.part");

    // Create directories
    fs::create_dir_all(model_dir.parent().unwrap()).map_err(|e| e.to_string())?;

    // Emit starting status
    let _ = emit_to_frontend(app, "model-download-progress", DownloadProgress {
        model_id: "parakeet-v3".to_string(),
        downloaded: 0,
        total: 0,
        percentage: 0.0,
        status: "Starting model download...".to_string(),
    });

    // Download the file
    let response = reqwest::get(MODEL_URL).await.map_err(|e| e.to_string())?;
    let total_size = response.content_length().unwrap_or(500_000_000);

    let mut file = fs::File::create(&part_path).map_err(|e| e.to_string())?;
    let mut downloaded: u64 = 0;
    let mut stream = response.bytes_stream();

    use futures_util::StreamExt;
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|e| e.to_string())?;
        file.write_all(&chunk).map_err(|e| e.to_string())?;
        downloaded += chunk.len() as u64;

        let percentage = (downloaded as f64 / total_size as f64) * 100.0;
        let _ = emit_to_frontend(app, "model-download-progress", DownloadProgress {
            model_id: "parakeet-v3".to_string(),
            downloaded,
            total: total_size,
            percentage,
            status: format!("Downloading model... {:.1}%", percentage),
        });
    }

    // Extract the tar.gz
    let _ = emit_to_frontend(app, "model-download-progress", DownloadProgress {
        model_id: "parakeet-v3".to_string(),
        downloaded: total_size,
        total: total_size,
        percentage: 100.0,
        status: "Extracting model files...".to_string(),
    });

    // Use tar and gzip to extract
    let tar_gz = fs::File::open(&part_path).map_err(|e| e.to_string())?;
    let tar = flate2::read::GzDecoder::new(tar_gz);
    let mut archive = tar::Archive::new(tar);
    archive.unpack(model_dir.parent().unwrap()).map_err(|e| e.to_string())?;

    // Clean up tar.gz file
    let _ = fs::remove_file(&part_path);

    // IMPORTANT: after download, also load the model into memory.
    let model_path = get_model_path(app);
    if let Some(p) = model_path.to_str() {
        transcription::load_model(p).map_err(|e| e.to_string())?;
    } else {
        return Err("Invalid model path".to_string());
    }

    let _ = emit_to_frontend(app, "model-download-progress", DownloadProgress {
        model_id: "parakeet-v3".to_string(),
        downloaded: total_size,
        total: total_size,
        percentage: 100.0,
        status: "Model installed successfully!".to_string(),
    });

    Ok(())
}

pub async fn download_model(app: &AppHandle, window: &WebviewWindow) -> Result<(), String> {
    let model_dir = get_model_dir(app);
    let part_path = model_dir.parent().unwrap().join("parakeet-v3-int8.tar.gz.part");

    // Create directories
    fs::create_dir_all(model_dir.parent().unwrap()).map_err(|e| e.to_string())?;

    // Emit starting status
    let _ = window.emit("model-download-progress", DownloadProgress {
        model_id: "parakeet-v3".to_string(),
        downloaded: 0,
        total: 0,
        percentage: 0.0,
        status: "Starting download...".to_string(),
    });

    // Download the file
    let response = reqwest::get(MODEL_URL).await.map_err(|e| e.to_string())?;
    let total_size = response.content_length().unwrap_or(500_000_000);

    let mut file = fs::File::create(&part_path).map_err(|e| e.to_string())?;
    let mut downloaded: u64 = 0;
    let mut stream = response.bytes_stream();

    use futures_util::StreamExt;
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|e| e.to_string())?;
        file.write_all(&chunk).map_err(|e| e.to_string())?;
        downloaded += chunk.len() as u64;

        let percentage = (downloaded as f64 / total_size as f64) * 100.0;
        let _ = window.emit("model-download-progress", DownloadProgress {
            model_id: "parakeet-v3".to_string(),
            downloaded,
            total: total_size,
            percentage,
            status: format!("Downloading model... {:.1}%", percentage),
        });
    }

    // Extract the tar.gz
    let _ = window.emit("model-download-progress", DownloadProgress {
        model_id: "parakeet-v3".to_string(),
        downloaded: total_size,
        total: total_size,
        percentage: 100.0,
        status: "Extracting model files...".to_string(),
    });

    // Use tar and gzip to extract
    let tar_gz = fs::File::open(&part_path).map_err(|e| e.to_string())?;
    let tar = flate2::read::GzDecoder::new(tar_gz);
    let mut archive = tar::Archive::new(tar);
    archive.unpack(model_dir.parent().unwrap()).map_err(|e| e.to_string())?;

    // Clean up tar.gz file
    let _ = fs::remove_file(&part_path);

    // Load model into memory immediately after install.
    let model_path = get_model_path(app);
    if let Some(p) = model_path.to_str() {
        transcription::load_model(p).map_err(|e| e.to_string())?;
    } else {
        return Err("Invalid model path".to_string());
    }

    let _ = window.emit("model-download-progress", DownloadProgress {
        model_id: "parakeet-v3".to_string(),
        downloaded: total_size,
        total: total_size,
        percentage: 100.0,
        status: "Model installed successfully!".to_string(),
    });

    Ok(())
}
