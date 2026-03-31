# Product Roadmap: Multi-Engine Architecture & Features

This document outlines the strategic plan to evolve "Vocal Typewriter" into a competitive, professional-grade AI transcription assistant.

## Phase 1: Foundation & Stability (Current)
- [x] **Cross-Platform Support**: Mac & Windows builds (GitHub Actions).
- [x] **Local Privacy**: Zero-data-retention policy (Local Whisper/Parakeet).
- [x] **Code Signing**: Automated signing for trusted installation (Completed).
- [x] **Crash Resolution**: Windows overlay fixed (Monitoring for stability).

## Phase 2: Multi-Engine Architecture (The "Rival" Upgrade)
**Goal:** Match and exceed competitors (Whisper Flow, Superwhisper) by offering user choice between speed and accuracy.

### 1. Engine Abstraction Layer (Completed)
**Problem:** The UI allows downloading Whisper models, but `transcription.rs` is hardcoded to use `ParakeetEngine`. It will crash or fail if a Whisper model is loaded.
**Solution:** Refactor `transcription.rs` to support swappable backends.

- [x] **Task:** Refactor `transcription.rs` to use a `SpeechEngine` trait instead of hardcoded Parakeet.
- [x] **Task:** Implement backend switching logic (If model == Whisper -> Load WhisperEngine).
- [x] **Task:** Ensure `whisper-rs` dependency is added and compiles on Mac/Windows.

- **Trait Definition**: Create a `SpeechEngine` trait.
  ```rust
  pub trait SpeechEngine: Send + Sync {
      fn load(&mut self, model_path: &str) -> Result<()>;
      fn transcribe(&mut self, audio: Vec<f32>) -> Result<String>;
  }
  ```
- **Engine Implementations**:
  - `ParakeetBackend`: Wraps `transcribe-rs` Parakeet implementation.
  - `WhisperBackend`: Wraps `whisper-rs` (C++ bindings for `whisper.cpp`).

### 2. Dynamic Model Loading (Completed)
Update `transcription.rs` to select the correct engine based on the model file type or ID.
- [x] If model ID contains "parakeet" -> Initialize `ParakeetBackend`.
- [x] If model ID contains "whisper" -> Initialize `WhisperBackend`.

### 3. Model Selection Strategy (Completed)
Allow users to select their preferred balance of speed vs. accuracy in Settings (UI already exists, backend needs to match).
- [x] **"Speed Demon"**: Whisper Turbo (Default). Near-instant, good accuracy.
- [x] **"Precision"**: Whisper Large V3. Perfect for long dictation, slower.
- [x] **"Legacy/Specific"**: Parakeet V3. For users who prefer the current behavior.

### 4. Streaming Transcription (Next Step)
Upgrade from "Record -> Stop -> Transcribe" to "Real-time Text Appearance".
- Implement partial result callbacks in the `TranscriptionEngine` trait.
- Stream text events to the frontend (`window.emit("partial-transcription", text)`).

## Phase 3: Advanced Context Awareness
**Goal:** Transform from "Dictation" to "Assistant".

- **Window Awareness**: Detect which app is active (VS Code, Gmail, Slack).
- **Smart Formatting**:
  - *Coding Mode*: If VS Code is active, format output as code snippets / comments.
  - *Email Mode*: If Gmail is active, format as professional prose.
- **Selection Context**: Read the user's selected text to use as "Context" for the AI edit.
  - *Example:* Select a messy paragraph -> Say "Fix this" -> App replaces it with polished text.

## Phase 4: Mobile Companion (See MOBILE_STRATEGY.md)
- Remote microphone functionality.
- Syncing custom vocabulary/prompts.

---
*Note: This roadmap is a living document. Priorities may shift based on user feedback and technical feasibility.*
