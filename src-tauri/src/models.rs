use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ModelType {
    WhisperSmall,
    WhisperMedium,
    WhisperLarge,
    WhisperTurbo,
    ParakeetV2,
    ParakeetV3,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelInfo {
    pub id: String,
    pub name: String,
    pub description: String,
    pub url: String,
    pub size_mb: u64,
    pub model_type: ModelType,
    pub is_active: bool,
    pub installed: bool,
}

impl ModelInfo {
    pub fn all_models() -> Vec<ModelInfo> {
        let mut models = vec![
            // Active model - Parakeet V3
            ModelInfo {
                id: "parakeet-v3".to_string(),
                name: "Parakeet V3".to_string(),
                description: "Fast and accurate".to_string(),
                url: "https://blob.handy.computer/parakeet-v3-int8.tar.gz".to_string(),
                size_mb: 478,
                model_type: ModelType::ParakeetV3,
                is_active: true,
                installed: false, // Updated dynamically
            },
            ModelInfo {
                id: "parakeet-v2".to_string(),
                name: "Parakeet V2".to_string(),
                description: "English only. The best model for English speakers.".to_string(),
                url: "https://blob.handy.computer/parakeet-v2-int8.tar.gz".to_string(),
                size_mb: 473,
                model_type: ModelType::ParakeetV2,
                is_active: false,
                installed: false,
            },
        ];

        #[cfg(target_os = "macos")]
        {
            models.extend([
                ModelInfo {
                    id: "whisper-turbo".to_string(),
                    name: "Whisper Turbo".to_string(),
                    description: "Balanced accuracy and speed.".to_string(),
                    url: "https://blob.handy.computer/ggml-large-v3-turbo-q5_0.bin".to_string(),
                    size_mb: 574,
                    model_type: ModelType::WhisperTurbo,
                    is_active: false,
                    installed: false,
                },
                ModelInfo {
                    id: "whisper-large".to_string(),
                    name: "Whisper Large".to_string(),
                    description: "Good accuracy, but slow.".to_string(),
                    url: "https://blob.handy.computer/ggml-large-v3-q5_0.bin".to_string(),
                    size_mb: 1100,
                    model_type: ModelType::WhisperLarge,
                    is_active: false,
                    installed: false,
                },
                ModelInfo {
                    id: "whisper-small".to_string(),
                    name: "Whisper Small".to_string(),
                    description: "Fast and fairly accurate.".to_string(),
                    url: "https://blob.handy.computer/ggml-small.bin".to_string(),
                    size_mb: 487,
                    model_type: ModelType::WhisperSmall,
                    is_active: false,
                    installed: false,
                },
                ModelInfo {
                    id: "whisper-medium".to_string(),
                    name: "Whisper Medium".to_string(),
                    description: "Good accuracy, medium speed".to_string(),
                    url: "https://blob.handy.computer/whisper-medium-q4_1.bin".to_string(),
                    size_mb: 492,
                    model_type: ModelType::WhisperMedium,
                    is_active: false,
                    installed: false,
                },
            ]);
        }

        models
    }
}
