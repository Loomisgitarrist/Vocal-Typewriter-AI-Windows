use anyhow::Result;
use once_cell::sync::Lazy;
use std::sync::Mutex;
use transcribe_rs::{
    engines::parakeet::{ParakeetEngine, ParakeetInferenceParams, ParakeetModelParams, TimestampGranularity},
    TranscriptionEngine,
};

/// The "Standard Control Panel" (Interface)
/// Any AI engine we add (Parakeet, Whisper, etc.) must sign this contract.
pub trait SpeechEngine: Send + Sync {
    fn load(&mut self, model_path: &str) -> Result<()>;
    fn transcribe(&mut self, audio: Vec<f32>) -> Result<String>;
    fn unload(&mut self) -> Result<()>;
}

/// The Wrapper for the Parakeet Engine
struct ParakeetBackend {
    engine: Option<ParakeetEngine>,
}

impl ParakeetBackend {
    fn new() -> Self {
        Self { engine: None }
    }
}

impl SpeechEngine for ParakeetBackend {
    fn load(&mut self, model_path: &str) -> Result<()> {
        println!("🔄 Loading Parakeet model from: {}", model_path);
        
        let mut engine = ParakeetEngine::new();
        
        // Handle path logic
        let path_buf = std::path::PathBuf::from(model_path);
        let path = path_buf.as_path();
        let model_dir = if path.is_file() {
            path.parent().unwrap_or(path)
        } else {
            path
        };

        println!("📂 Using model directory: {:?}", model_dir);

        engine.load_model_with_params(model_dir, ParakeetModelParams::int8())
            .map_err(|e| anyhow::anyhow!("Failed to load parakeet model: {}", e))?;
            
        println!("✅ Parakeet model loaded successfully!");
        self.engine = Some(engine);
        Ok(())
    }

    fn transcribe(&mut self, audio: Vec<f32>) -> Result<String> {
        let engine = self.engine.as_mut().ok_or_else(|| anyhow::anyhow!("Parakeet model not loaded"))?;
        
        let params = ParakeetInferenceParams {
            timestamp_granularity: TimestampGranularity::Segment,
            ..Default::default()
        };

        let result = engine
            .transcribe_samples(audio, Some(params))
            .map_err(|e| anyhow::anyhow!("Parakeet transcription failed: {}", e))?;
            
        Ok(result.text)
    }

    fn unload(&mut self) -> Result<()> {
        self.engine = None;
        println!("✅ Parakeet model unloaded");
        Ok(())
    }
}

// Global variable now holds a generic "SpeechEngine" instead of just Parakeet
static ACTIVE_ENGINE: Lazy<Mutex<Box<dyn SpeechEngine>>> = Lazy::new(|| {
    // We default to Parakeet for now
    Mutex::new(Box::new(ParakeetBackend::new()))
});

pub fn switch_engine(engine_type: &str) -> Result<()> {
    let mut engine_guard = ACTIVE_ENGINE.lock().map_err(|e| anyhow::anyhow!("Lock failed: {}", e))?;
    match engine_type.to_lowercase().as_str() {
        "parakeet" => {
            println!("🔀 Switching to Parakeet engine");
            *engine_guard = Box::new(ParakeetBackend::new());
        },
        _ => return Err(anyhow::anyhow!("Unknown engine type: {}", engine_type)),
    }
    Ok(())
}


pub fn load_model(model_path: &str) -> Result<()> {
    // In the future, we can check the path or a config to decide WHICH engine to put in the box.
    // For now, we just tell the active engine to load.
    let mut engine_guard = ACTIVE_ENGINE.lock().map_err(|e| anyhow::anyhow!("Lock failed: {}", e))?;
    engine_guard.load(model_path)
}

pub fn unload_model() -> Result<()> {
    let mut engine_guard = ACTIVE_ENGINE.lock().map_err(|e| anyhow::anyhow!("Lock failed: {}", e))?;
    engine_guard.unload()
}

pub async fn transcribe(audio_data: Vec<f32>) -> Result<String, String> {
    println!("🎤 Transcribing {} samples", audio_data.len());
    
    let mut engine_guard = ACTIVE_ENGINE.lock().map_err(|e| e.to_string())?;
    
    match engine_guard.transcribe(audio_data) {
        Ok(text) => {
            println!("✅ Transcription: {}", text);
            Ok(text)
        }
        Err(e) => {
            let err_msg = e.to_string();
            eprintln!("❌ Transcription error: {}", err_msg);
            Err(err_msg)
        }
    }
}
