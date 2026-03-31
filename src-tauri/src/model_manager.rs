use crate::models::{ModelInfo, ModelType};
use anyhow::Result;
use futures_util::StreamExt;
use serde::{Serialize, Deserialize};
use std::fs;
use std::io::Write;
use std::path::PathBuf;
use tauri::{AppHandle, Emitter, Manager};

#[derive(Clone, Serialize, Deserialize)]
pub struct ModelDownloadProgress {
    pub model_id: String,
    pub downloaded: u64,
    pub total: u64,
    pub percentage: f64,
}

pub struct ModelManager {
    models_dir: PathBuf,
}

impl ModelManager {
    pub fn new(app: &AppHandle) -> Result<Self> {
        let mut settings = crate::settings::load_settings(app);
        
        let mut models_dir = if let Some(custom_path) = &settings.custom_models_path {
            PathBuf::from(custom_path)
        } else {
            app.path()
                .app_local_data_dir()
                .map_err(|e| anyhow::anyhow!("Failed to get app local data dir: {}", e))?
                .join("models")
        };

        // Sandbox Verification: Check if we can actually write to this directory
        // If the user has a custom path from a previous non-sandboxed installation,
        // we won't have permission to access it. We must detect this and revert to default.
        let is_writable = if !models_dir.exists() {
            fs::create_dir_all(&models_dir).is_ok()
        } else {
            // Try to create a temporary test file
            let test_file = models_dir.join(".sandbox_check");
            if fs::write(&test_file, "").is_ok() {
                let _ = fs::remove_file(test_file);
                true
            } else {
                false
            }
        };

        if !is_writable {
            println!("⚠️ Models directory {:?} is not writable (likely Sandbox restriction). Reverting to default.", models_dir);
            
            // Revert to default
            models_dir = app.path()
                .app_local_data_dir()
                .map_err(|e| anyhow::anyhow!("Failed to get app local data dir: {}", e))?
                .join("models");
                
            // Update settings to remove the invalid custom path
            if settings.custom_models_path.is_some() {
                settings.custom_models_path = None;
                let _ = crate::settings::save_settings(app, &settings);
                println!("✅ Updated settings to use default models path");
            }
        }

        if !models_dir.exists() {
            fs::create_dir_all(&models_dir)?;
        }

        Ok(Self { models_dir })
    }

    pub fn get_models_dir(&self) -> PathBuf {
        self.models_dir.clone()
    }

    pub fn get_model_path(&self, model_id: &str) -> PathBuf {
        self.models_dir.join(model_id)
    }

    pub fn resolve_model_path(&self, model: &ModelInfo) -> Option<PathBuf> {
        match model.model_type {
            ModelType::WhisperSmall | ModelType::WhisperMedium | 
            ModelType::WhisperLarge | ModelType::WhisperTurbo => {
                // Primary check: Exact expected path (id.bin)
                let expected_filename = format!("{}.bin", model.id);
                let path = self.models_dir.join(&expected_filename);
                
                if path.exists() && path.metadata().map(|m| m.len() > 0).unwrap_or(false) {
                    return Some(path);
                }
                
                // Heuristic check: keywords based on model ID
                let keywords = match model.model_type {
                    ModelType::WhisperTurbo => vec!["turbo", "large-v3-turbo"],
                    ModelType::WhisperLarge => vec!["large-v3", "large"],
                    ModelType::WhisperMedium => vec!["medium"],
                    ModelType::WhisperSmall => vec!["small"],
                    _ => vec![],
                };

                // Secondary check: Look for files matching keywords
                if let Ok(entries) = fs::read_dir(&self.models_dir) {
                    for entry in entries.flatten() {
                        if let Ok(filename) = entry.file_name().into_string() {
                            let filename_lower = filename.to_lowercase();
                            if !filename_lower.ends_with(".bin") { continue; }
                            
                            // Check if it contains the ID
                            if filename_lower.contains(&model.id.to_lowercase()) {
                                return Some(self.models_dir.join(filename));
                            }

                            // Check specific keywords
                            for keyword in &keywords {
                                if filename_lower.contains(keyword) {
                                    // Avoid false positives (e.g. "small" matching "small-talk")
                                    // For now, simple containment is likely enough for ggml files
                                    println!("✅ Found potential match for {}: {}", model.id, filename);
                                    return Some(self.models_dir.join(filename));
                                }
                            }
                        }
                    }
                }
                None
            },
            ModelType::ParakeetV2 | ModelType::ParakeetV3 => {
                 // Check for the extracted file
                let parakeet_dir = self.models_dir.join("parakeet-tdt-0.6b-v3-int8");
                if parakeet_dir.join("encoder-model.int8.onnx").exists() {
                    // For Parakeet V3, we return the directory or the specific file?
                    // The loader likely expects the directory or a specific structure.
                    // But wait, the loader logic in lib.rs handles this.
                    // Let's return the "main" file path that indicates existence.
                    return Some(parakeet_dir.join("encoder-model.int8.onnx"));
                }
                
                // Fallback: Check for the tar.gz file
                let tar_path = self.models_dir.join(format!("{}.tar.gz", model.id));
                if tar_path.exists() {
                    return Some(tar_path);
                }
                None
            }
        }
    }

    pub fn is_model_installed(&self, model: &ModelInfo) -> bool {
        let installed = self.resolve_model_path(model).is_some();
        if installed {
            println!("✅ Model {} is installed", model.id);
        } else {
             println!("❌ Model {} is NOT installed", model.id);
             // Debug info
             if matches!(model.model_type, ModelType::WhisperSmall | ModelType::WhisperMedium | ModelType::WhisperLarge | ModelType::WhisperTurbo) {
                 let _ = fs::read_dir(&self.models_dir).map(|entries| {
                     println!("📂 Contents of {:?}:", self.models_dir);
                     for entry in entries.flatten() {
                         println!("  - {:?}", entry.file_name());
                     }
                 });
             }
        }
        installed
    }


    pub fn reset_models(&self) -> Result<()> {
        if self.models_dir.exists() {
            println!("🗑️ Resetting models in {:?}", self.models_dir);
            for entry in fs::read_dir(&self.models_dir)? {
                let entry = entry?;
                let path = entry.path();
                if path.is_file() {
                    fs::remove_file(path)?;
                } else if path.is_dir() {
                    fs::remove_dir_all(path)?;
                }
            }
        }
        Ok(())
    }

    pub async fn download_model(
        &self,
        model: &ModelInfo,
        app: &AppHandle,
    ) -> Result<()> {
        println!("Starting download of {} from {}", model.name, model.url);
        
        let app_handle = app.clone();
        let model_id = model.id.clone();
        
        // Download the file
        let response = reqwest::get(&model.url).await?;
        let total_size = response.content_length().unwrap_or(model.size_mb * 1_000_000);
        
        let final_file_path = match model.model_type {
            ModelType::WhisperSmall | ModelType::WhisperMedium | 
            ModelType::WhisperLarge | ModelType::WhisperTurbo => {
                // Whisper: save as .bin file
                self.get_model_path(&model.id).with_extension("bin")
            },
            ModelType::ParakeetV2 | ModelType::ParakeetV3 => {
                // Parakeet: save as .tar.gz, will extract later
                self.models_dir.join(format!("{}.tar.gz", model.id))
            }
        };

        // Use a .part extension during download to avoid partial files being detected as installed
        let part_path = match final_file_path.extension() {
            Some(ext) => {
                let mut p = final_file_path.clone();
                p.set_extension(format!("{}.part", ext.to_string_lossy()));
                p
            },
            None => final_file_path.with_extension("part"),
        };
        
        let mut file = fs::File::create(&part_path)?;
        let mut downloaded: u64 = 0;
        let mut stream = response.bytes_stream();
        
        while let Some(chunk) = stream.next().await {
            let chunk = chunk?;
            file.write_all(&chunk)?;
            downloaded += chunk.len() as u64;
            
            let percentage = (downloaded as f64 / total_size as f64) * 100.0;
            
            // Emit progress
            let _ = app_handle.emit("model-download-progress", ModelDownloadProgress {
                model_id: model_id.clone(),
                downloaded,
                total: total_size,
                percentage,
            });
        }
        
        // Ensure file is closed and flushed before we proceed
        file.flush()?;
        drop(file);
        
        println!("Download complete: {}", part_path.display());
        
        // Extract if it's a Parakeet model
        if matches!(model.model_type, ModelType::ParakeetV2 | ModelType::ParakeetV3) {
            println!("Extracting Parakeet model...");
            let _ = app_handle.emit("model-extraction-started", model_id.clone());
            
            let tar_gz = fs::File::open(&part_path)?;
            let tar = flate2::read::GzDecoder::new(tar_gz);
            let mut archive = tar::Archive::new(tar);
            
            // Extract to models directory
            archive.unpack(&self.models_dir)?;
            
            // Clean up tar.gz
            let _ = fs::remove_file(&part_path);
            
            let _ = app_handle.emit("model-extraction-completed", model_id.clone());
            println!("Extraction complete");
        } else {
             // For non-archive models (Whisper), rename .part to final
             fs::rename(&part_path, &final_file_path)?;
        }
        
        let _ = app_handle.emit("model-download-complete", model_id);
        
        Ok(())
    }
}

