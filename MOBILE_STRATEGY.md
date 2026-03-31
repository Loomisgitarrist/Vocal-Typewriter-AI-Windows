# Mobile Development Strategy (iOS/Android)
## Overview
Moving "Vocal Typewriter" to mobile is not a simple port; it requires a new architectural approach due to OS sandboxing and hardware constraints. This document outlines the roadmap for building a mobile version that matches the desktop experience.

## 1. The Core Challenges
### A. The "Typing" Problem (Sandboxing)
*   **Desktop:** We can inject text into any window (Word, Chrome, etc.).
*   **Mobile:** Apps are sandboxed. You CANNOT write text into other apps directly.
*   **Solution:** You must build a **Custom Keyboard Extension**.
    *   User installs app -> Enables "Vocal Typewriter Keyboard" in System Settings.
    *   User taps Globe icon -> Switches to your keyboard.
    *   Your keyboard has a big "Mic" button -> Records -> Transcribes -> Inserts text.

### B. The "Brain" Problem (Hardware)
*   **Desktop:** Uses ~1.5GB RAM and high-wattage CPU/GPU.
*   **Mobile:** Strict memory limits (<200MB preferred for extensions) and battery constraints.
*   **Solution:** Use **CoreML (iOS)** and **TFLite (Android)** with quantized models.

---

## 2. Technical Stack Recommendations

### Option A: Native (Recommended for Performance)
*   **iOS:** Swift + SwiftUI + [WhisperKit](https://github.com/argentlabs/WhisperKit) (CoreML optimized).
*   **Android:** Kotlin + Jetpack Compose + [whisper.tflite](https://github.com/ggerganov/whisper.cpp).
*   **Pros:** Best battery life, fastest inference (Neural Engine), smallest app size.
*   **Cons:** Two separate codebases to maintain.

### Option B: Cross-Platform (Tauri v2 Mobile)
*   **Stack:** Rust (Backend) + React (Frontend).
*   **Pros:** Share 70% of code with Desktop app.
*   **Cons:** Harder to build Keyboard Extensions (requires bridging Native code to Rust). High memory overhead.

---

## 3. Implementation Roadmap

### Phase 1: The Engine (Whisper on Mobile)
Instead of `transcribe-rs`, we need a mobile-optimized engine.
*   **iOS:** Use **CoreML**. This converts the Whisper model to `.mlpackage` format to run on the Apple Neural Engine (ANE). This is how apps like "Whisper Flow" achieve real-time local transcription without draining battery.
*   **Model:** Use `base.en` or `small.en` quantized to `int8`.
    *   *Size:* ~75MB - 200MB.
    *   *RAM Usage:* ~150MB (Safe for mobile).

### Phase 2: The Keyboard Extension
This is the most critical feature.
1.  Create a **Keyboard Extension Target** in Xcode/Android Studio.
2.  **Shared Container:** The main app downloads the model; the Keyboard Extension needs permission to read that file (App Groups).
3.  **UI:** A simple keyboard layout (QWERTY) or just a big "Record" button overlay.

### Phase 3: Syncing Settings
*   Use iCloud KeyChain or a simple backend (Supabase/Firebase) to sync User Prompts and API Keys between Desktop and Mobile.

## 4. Why "Whisper Flow" Works
*   **CoreML:** They run on the Neural Engine (NPU), not the CPU.
*   **Native Swift:** They don't have the overhead of a web view (Electron/Tauri), saving precious RAM.
*   **Quantization:** They use compressed models that trade 1-2% accuracy for 500% speed gains.

## 5. Next Steps (When Ready)
1.  **Do not touch the current Desktop codebase.**
2.  Create a new repo: `vocal-typewriter-mobile`.
3.  Start with an iOS prototype using **Swift** and **WhisperKit** to test if the "Small" model runs smoothly on your phone.
