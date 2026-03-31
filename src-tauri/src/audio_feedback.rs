#[cfg(target_os = "macos")]
use std::process::Command;
#[cfg(not(target_os = "macos"))]
use std::fs::File;
#[cfg(not(target_os = "macos"))]
use std::io::BufReader;
use std::path::PathBuf;
use tauri::{AppHandle, Manager};
#[cfg(not(target_os = "macos"))]
use rodio::{Decoder, OutputStream, Sink};

pub fn play_start_sound(app: &AppHandle) {
    // Play start sound at 4x volume (400%)
    play_sound(app, "pop_start.wav", 4.0);
}

pub fn play_stop_sound(app: &AppHandle) {
    // Play stop sound at normal volume (100%)
    play_sound(app, "pop_stop.wav", 1.0);
}

fn play_sound(app: &AppHandle, filename: &str, volume: f32) {
    // Try to resolve the path. In dev mode, it might be in 'resources/' subdir.
    // In prod, it might be in the root of resources.
    
    // First try resolving simply
    let mut resource_path = app.path().resolve(
        filename,
        tauri::path::BaseDirectory::Resource
    );

    // If that fails or file doesn't exist, try with 'resources/' prefix (mostly for dev/debug)
    if resource_path.as_ref().map(|p| !p.exists()).unwrap_or(true) {
        let alt_filename = format!("resources/{}", filename);
        if let Ok(alt_path) = app.path().resolve(&alt_filename, tauri::path::BaseDirectory::Resource) {
            if alt_path.exists() {
                resource_path = Ok(alt_path);
            }
        }
    }

    if let Ok(path) = resource_path {
        println!("🔊 Playing sound: {} (vol: {})", path.display(), volume);
        // Play in a separate thread to avoid blocking the UI/main thread
        std::thread::spawn(move || {
            if let Err(e) = play_file(&path, volume) {
                eprintln!("❌ Failed to play sound '{}': {}", path.display(), e);
            } else {
                println!("✅ Sound played successfully: {}", path.display());
            }
        });
    } else {
        eprintln!("❌ Failed to resolve sound path: {}", filename);
    }
}

#[cfg(target_os = "macos")]
fn play_file(path: &PathBuf, volume: f32) -> Result<(), Box<dyn std::error::Error>> {
    // Use afplay on macOS for better stability and less conflict with audio input
    // -v sets volume (1.0 is normal, so 2.0 is double amplitude)
    let status = Command::new("afplay")
        .arg("-v")
        .arg(volume.to_string())
        .arg(path)
        .status()?;

    if !status.success() {
        return Err(format!("afplay exited with status: {}", status).into());
    }
    
    Ok(())
}

#[cfg(not(target_os = "macos"))]
fn play_file(path: &PathBuf, volume: f32) -> Result<(), Box<dyn std::error::Error>> {
    // Get a output stream handle to the default physical sound device
    let (_stream, stream_handle) = OutputStream::try_default()?;
    let sink = Sink::try_new(&stream_handle)?;
    
    let file = File::open(path)?;
    let source = Decoder::new(BufReader::new(file))?;
    
    sink.append(source);
    sink.set_volume(volume);
    
    // The sound plays in a separate thread, so we block this thread until it's done
    sink.sleep_until_end();
    
    Ok(())
}
