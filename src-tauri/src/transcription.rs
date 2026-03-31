use anyhow::Result;
use once_cell::sync::Lazy;
use std::sync::Mutex;
use transcribe_rs::{
    engines::parakeet::{ParakeetEngine, ParakeetInferenceParams, ParakeetModelParams, TimestampGranularity},
    TranscriptionEngine,
};

#[cfg(target_os = "macos")]
use whisper_rs::{WhisperContext, FullParams, SamplingStrategy, WhisperContextParameters};

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

/// The Wrapper for the Whisper Engine
#[cfg(target_os = "macos")]
struct WhisperBackend {
    context: Option<WhisperContext>,
}

#[cfg(target_os = "macos")]
impl WhisperBackend {
    fn new() -> Self {
        Self { context: None }
    }
}

#[cfg(target_os = "macos")]
impl SpeechEngine for WhisperBackend {
    fn load(&mut self, model_path: &str) -> Result<()> {
        println!("🔄 Loading Whisper model from: {}", model_path);
        
        // Whisper expects a file path to the model binary (e.g., ggml-base.en.bin)
        let ctx = WhisperContext::new_with_params(model_path, WhisperContextParameters::default())
            .map_err(|e| anyhow::anyhow!("Failed to load Whisper model: {}", e))?;
            
        println!("✅ Whisper model loaded successfully!");
        self.context = Some(ctx);
        Ok(())
    }

    fn transcribe(&mut self, audio: Vec<f32>) -> Result<String> {
        let ctx = self.context.as_mut().ok_or_else(|| anyhow::anyhow!("Whisper model not loaded"))?;
        let mut state = ctx.create_state().map_err(|e| anyhow::anyhow!("Failed to create Whisper state: {}", e))?;
        
        // Configure Whisper parameters
        let mut params = FullParams::new(SamplingStrategy::Greedy { best_of: 1 });
        params.set_n_threads(4); // Use 4 threads by default
        params.set_translate(false);
        params.set_language(Some("en")); // Default to English, can be made configurable later
        params.set_print_special(false);
        params.set_print_progress(false);
        params.set_print_realtime(false);
        params.set_print_timestamps(false);

        // Run transcription
        state.full(params, &audio[..])
            .map_err(|e| anyhow::anyhow!("Whisper transcription failed: {}", e))?;

        // Retrieve segments
        let num_segments = state.full_n_segments().map_err(|e| anyhow::anyhow!("Failed to get segment count: {}", e))?;
        let mut text = String::new();
        
        for i in 0..num_segments {
            let segment = state.full_get_segment_text(i).map_err(|e| anyhow::anyhow!("Failed to get segment text: {}", e))?;
            text.push_str(&segment);
            text.push(' ');
        }

        Ok(text.trim().to_string())
    }

    fn unload(&mut self) -> Result<()> {
        self.context = None;
        println!("✅ Whisper model unloaded");
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
        #[cfg(target_os = "macos")]
        "whisper" => {
            println!("🔀 Switching to Whisper engine");
            *engine_guard = Box::new(WhisperBackend::new());
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
