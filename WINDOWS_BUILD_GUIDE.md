# Vocal Typewriter AI - Windows Build Configuration

## Overview
This folder contains the Windows version of Vocal Typewriter AI, configured to build for both x64 and ARM64 architectures.

## What's Included

### Build Targets
- **MSI Installer** (.msi) - Standard Windows Installer
- **NSIS Installer** (.exe) - Executable installer with webview bootstrapper
- **x64** architecture support
- **ARM64** architecture support

### Features (100% parity with macOS version)
All features from the original macOS app work on Windows:

1. **Voice Dictation**
   - Push-to-talk recording
   - Audio transcription using Whisper
   - Automatic text insertion at cursor position

2. **Global Keyboard Shortcuts**
   - Customizable shortcuts for dictation
   - Style selector shortcut
   - Raw mode toggle
   - Highlight/selection shortcuts

3. **Style Selector**
   - Multiple writing styles (Professional, Casual, Technical, etc.)
   - Real-time style switching
   - AI-powered text transformation

4. **Highlight Feature**
   - Screen region selection
   - Screenshot capture
   - Vision AI processing
   - Result display window

5. **Audio Feedback**
   - Start/stop recording sounds
   - Uses rodio library on Windows

6. **System Tray**
   - Tray icon with menu
   - Settings access
   - Quit option

7. **Settings Management**
   - Input device selection
   - Shortcut configuration
   - Style preferences
   - LLM provider settings

8. **Model Management**
   - Automatic model download
   - Local model support
   - Progress tracking

## File Structure

```
Vocal-Typewriter-AI-Windows/
├── src/                      # React frontend source
├── src-tauri/
│   ├── src/                  # Rust backend source
│   │   ├── lib.rs           # Main application logic
│   │   ├── overlay.rs       # Window overlay handling
│   │   ├── typing.rs        # Text input/output
│   │   ├── permissions.rs   # Permission checking
│   │   ├── audio_feedback.rs # Sound playback
│   │   └── ...
│   ├── Cargo.toml           # Rust dependencies
│   ├── tauri.conf.json      # Tauri configuration
│   └── capabilities/        # Permission capabilities
├── build-windows.bat        # Windows batch build script
└── build-windows.sh         # Bash build script (for WSL)
```

## Key Configuration Changes from macOS

### tauri.conf.json
- Changed `targets` from `["dmg"]` to `["msi", "nsis"]`
- Added Windows bundle configuration with webview bootstrapper
- Kept macOS configuration for compatibility

### Cargo.toml
- Uses `#[cfg(target_os = "windows")]` conditionals
- Windows-specific dependencies for Win32 API
- Different audio playback (rodio instead of afplay)
- Different Whisper backend (no CoreML on Windows)

### Source Code Conditionals
The code uses conditional compilation to handle platform differences:

```rust
#[cfg(target_os = "macos")]
// macOS-specific code

#[cfg(target_os = "windows")]
// Windows-specific code

#[cfg(not(target_os = "macos"))]
// Non-macOS code (Windows + Linux)
```

## Platform-Specific Code Locations

1. **Overlay Windows** - `src/overlay.rs`
   - macOS: Uses tauri-nspanel for NSPanel windows
   - Windows: Uses standard WebviewWindowBuilder with Win32 API for always-on-top

2. **Audio Playback** - `src/audio_feedback.rs`
   - macOS: Uses `afplay` system command
   - Windows: Uses rodio library

3. **Permissions** - `src/permissions.rs`
   - macOS: Checks TCC permissions (Accessibility, Microphone, etc.)
   - Windows: Opens Windows Privacy settings

4. **Screenshots** - `src/commands/highlight.rs`
   - macOS: Applies scale factor conversion for Retina displays
   - Windows: Uses raw physical coordinates

5. **Keyboard Shortcuts** - `src/keyboard.rs`
   - Uses rdev library (cross-platform)

6. **Text Input** - `src/typing.rs`
   - macOS: Cmd+V for paste
   - Windows: Ctrl+V for paste

## Building

### Prerequisites
- Windows 10/11 or WSL2
- Rust (install from https://rustup.rs)
- Bun (will be auto-installed if missing)

### Build Commands

**On Windows (Command Prompt or PowerShell):**
```batch
build-windows.bat
```

**On Windows (Git Bash or WSL):**
```bash
./build-windows.sh
```

### Build Output
After successful build, you'll find:
```
release-windows/
├── Vocal Typewriter AI_<version>_x64.msi
├── Vocal Typewriter AI_<version>_x64-setup.exe
├── Vocal Typewriter AI_<version>_arm64.msi
└── Vocal Typewriter AI_<version>_arm64-setup.exe
```

## Code Signing (Optional)
The tauri.conf.json includes configuration for code signing:
- `certificateThumbprint`: Set to your certificate thumbprint
- `timestampUrl`: Uses DigiCert timestamp server
- `digestAlgorithm`: SHA256

## Testing Checklist

Before releasing, verify:
- [ ] App installs on Windows x64
- [ ] App installs on Windows ARM64
- [ ] Voice dictation works
- [ ] Global shortcuts register and work
- [ ] Style selector appears on shortcut
- [ ] Highlight feature captures screenshots
- [ ] Audio feedback plays
- [ ] System tray icon appears
- [ ] Settings save and load
- [ ] Model downloads work
- [ ] Text pastes into other applications
- [ ] App autostarts (if enabled)

## Known Limitations

1. **No NSPanel on Windows**: Uses regular windows with always-on-top
2. **No CoreML**: Uses CPU/GPU for Whisper transcription
3. **No TouchBar support**: Not applicable on Windows
4. **Different permission model**: Windows doesn't have TCC, uses Privacy settings

## Support

For issues or questions:
1. Check the original macOS app documentation
2. Verify Windows-specific paths in the code
3. Test with elevated permissions if needed
4. Check Windows Event Viewer for crash logs
