use tauri::{AppHandle, Manager, Emitter, State, Window, Size, LogicalSize};
use screenshots::Screen;
use std::time::Instant;
use base64::{Engine as _, engine::general_purpose};
use std::io::Cursor;
use image::{DynamicImage, ImageFormat, ImageBuffer, Rgba};
use crate::AppState;
use crate::HighlightStatus;
use tauri_plugin_clipboard_manager::ClipboardExt;

#[tauri::command]
pub fn get_highlight_status(state: State<'_, AppState>) -> HighlightStatus {
    state.highlight_status.lock().unwrap().clone()
}

#[tauri::command]
pub async fn update_highlight_rect(
    app: AppHandle,
    window: Window,
    state: State<'_, AppState>,
    x: i32,
    y: i32,
    width: u32,
    height: u32,
) -> Result<String, String> {
    println!("🚀 update_highlight_rect called: x={}, y={}, w={}, h={}", x, y, width, height);

    // 1. Hide Selection Overlay (Always do this BEFORE capture)
    if let Some(selection_window) = app.get_webview_window("selection_overlay") {
         println!("🙈 Hiding selection_overlay before capture");
         let _ = selection_window.hide();
    }

    // Give the window a moment to disappear from the framebuffer
    tokio::time::sleep(std::time::Duration::from_millis(200)).await;

    // Wrap the core logic in a Result-returning block to catch early errors (like "No screen found")
    let result = async {
        // COMPUTER USE COORDINATE CALCULATION
        // Re-implementing ROBUST logic with Scale Factor and Window Offset.
        // User reported "way too low" previously, but "Double Calculation" before that.
        // The only mathematically correct way for High-DPI is:
        // Physical_Pos = Window_Inner_Pos + (Logical_Pos * Scale_Factor)
        
        let scale_factor = window.scale_factor().map_err(|e| e.to_string())?;
        let window_pos = window.inner_position().map_err(|e| e.to_string())?;

        // 1. Window Position (Physical)
        let win_phy_x = window_pos.x;
        let win_phy_y = window_pos.y;

        // 2. Selection Offset (Logical -> Physical)
        let sel_phy_x = (x as f64 * scale_factor) as i32;
        let sel_phy_y = (y as f64 * scale_factor) as i32;
        let phy_width = (width as f64 * scale_factor) as u32;
        let phy_height = (height as f64 * scale_factor) as u32;

        // 3. Absolute Selection Position (Physical)
        // Add window position to local coordinate.
        let abs_phy_x = win_phy_x + sel_phy_x;
        let abs_phy_y = win_phy_y + sel_phy_y;

        println!("📍 Computer Use Calc (SCALED + ADD): Scale({}), Win({}, {}), SelRaw({}, {}), SelPhy({}, {}), Abs({}, {})", 
                 scale_factor, win_phy_x, win_phy_y, x, y, sel_phy_x, sel_phy_y, abs_phy_x, abs_phy_y);

        // 2. Validate NOT Full Screen
        let screens = Screen::all().map_err(|e| e.to_string())?;
        if !screens.iter().any(|s| {
            let info = s.display_info;
            abs_phy_x >= info.x && abs_phy_x < info.x + info.width as i32 &&
            abs_phy_y >= info.y && abs_phy_y < info.y + info.height as i32
        }) {
             return Err("No screen found for coordinates".to_string());
        }



        // 3. Capture Screenshot
        capture_screen_region(app.clone(), state.clone(), abs_phy_x, abs_phy_y, phy_width, phy_height).await
    }.await;

    match result {
        Ok(base64_image) => {
            println!("✅ Screenshot capture successful");
            
            // 3. Get Pending Audio
            let audio_data = {
                let mut audio_lock = state.pending_audio.lock().unwrap();
                audio_lock.take()
            };

            // 4. Submit
            if let Some(audio) = audio_data {
                println!("🚀 Processing pending audio with captured screenshot...");
                
                // Show Highlight Result Window immediately with "Processing..."
                if let Some(window) = app.get_webview_window("highlight_result") {
                    println!("🪟 Showing highlight_result window...");
                    let _ = window.set_size(Size::Logical(LogicalSize { width: 600.0, height: 500.0 }));
                    let _ = window.center(); // Ensure it's visible on screen
                    let _ = window.show();
                    let _ = window.set_focus();
                    let _ = window.set_always_on_top(true);
                    let _ = window.emit("processing-started", ());
                } else {
                    eprintln!("❌ highlight_result window not found!");
                }

                crate::process_audio_and_submit(app, audio, true);
            } else {
                 println!("⚠️ No pending audio found for screenshot flow");
                 // Notify user of missing audio
                 if let Some(window) = app.get_webview_window("highlight_result") {
                    let _ = window.emit("processing-error", "No audio recorded. Please hold the shortcut while speaking.");
                    let _ = window.center();
                    let _ = window.show();
                    let _ = window.set_focus();
                    let _ = window.set_always_on_top(true);
                }
            }

            Ok(base64_image)
        },
        Err(e) => {
            eprintln!("❌ Screenshot capture failed (caught): {}", e);
            
            // Show error in Highlight Result Window - GUARANTEED
            if let Some(window) = app.get_webview_window("highlight_result") {
                let _ = window.emit("processing-error", &format!("Screenshot failed: {}", e));
                let _ = window.center();
                let _ = window.show();
                let _ = window.set_focus();
                let _ = window.set_always_on_top(true);
            }
            
            Err(e)
        }
    }
}

#[tauri::command]
pub fn clear_screenshot(state: State<'_, AppState>) -> Result<(), String> {
    println!("🗑️ Clearing screenshot and pending audio");
    *state.last_screenshot.lock().unwrap() = None;
    *state.pending_audio.lock().unwrap() = None;
    Ok(())
}

#[tauri::command]
pub fn close_highlight_result(app: AppHandle) -> Result<(), String> {
    if let Some(window) = app.get_webview_window("highlight_result") {
        let _ = window.hide();
    }
    Ok(())
}

#[tauri::command]
pub async fn capture_screen_region(
    app: AppHandle,
    state: State<'_, AppState>,
    x: i32,
    y: i32,
    width: u32,
    height: u32,
) -> Result<String, String> {
    println!("📸 Capturing screen region: x={}, y={}, w={}, h={}", x, y, width, height);
    
    let start = Instant::now();
    let screens = Screen::all().map_err(|e| e.to_string())?;
    
    // Find the screen that contains the region
    let screen = screens.iter().find(|s| {
        let info = s.display_info;
        x >= info.x && x < info.x + info.width as i32 &&
        y >= info.y && y < info.y + info.height as i32
    }).ok_or("No screen found for coordinates")?;

    // Capture the area relative to the screen
    // We receive PHYSICAL coordinates now.
    // Screen info is also in PHYSICAL coordinates.
    let relative_x = x - screen.display_info.x;
    let relative_y = y - screen.display_info.y;
    
    // CRITICAL FIX FOR RETINA / HIGH-DPI DISPLAYS (macOS)
    // The `screenshots` crate documentation says "physical values", but on macOS,
    // the underlying CoreGraphics API (CGWindowListCreateImage) often expects LOGICAL points
    // for the rect, even if the image returned is high-res.
    // If we pass physical pixels to a function expecting points, we get "Double Calculation"
    // (capturing 2x size/offset) or "Way too low" (offset multiplied by scale).
    
    let scale = screen.display_info.scale_factor;
    
    // Check if we are on macOS to apply this fix selectively, or trust scale factor.
    // On macOS, scale is typically 2.0 for Retina.
    // We convert the RELATIVE PHYSICAL coordinates back to LOGICAL for the capture call.
    
    #[cfg(target_os = "macos")]
    let (capture_x, capture_y, capture_width, capture_height) = {
        println!("🍎 macOS detected: Converting Physical -> Logical for capture_area (Scale: {})", scale);
        (
            (relative_x as f32 / scale) as i32,
            (relative_y as f32 / scale) as i32,
            (width as f32 / scale) as u32,
            (height as f32 / scale) as u32,
        )
    };

    #[cfg(not(target_os = "macos"))]
    let (capture_x, capture_y, capture_width, capture_height) = {
        (relative_x, relative_y, width, height)
    };

    println!("🔍 Capture Coords (Final): x={}, y={}, w={}, h={}", capture_x, capture_y, capture_width, capture_height);

    let buffer = screen.capture_area(capture_x, capture_y, capture_width, capture_height).map_err(|e| e.to_string())?;
    
    // Convert from screenshots' image (0.24) to our image (0.25)
    let width = buffer.width();
    let height = buffer.height();
    let raw = buffer.into_raw();
    
    let new_buffer = ImageBuffer::<Rgba<u8>, _>::from_raw(width, height, raw)
        .ok_or("Failed to create image buffer")?;
    let image = DynamicImage::ImageRgba8(new_buffer);
    
    // Convert to base64 to send back to frontend or process
    let mut buffer = Vec::new();
    let mut cursor = Cursor::new(&mut buffer);
    image.write_to(&mut cursor, ImageFormat::Png).map_err(|e| e.to_string())?;
    
    let base64_string = general_purpose::STANDARD.encode(&buffer);
    
    println!("✅ Capture complete in {:?}", start.elapsed());
    
    // Store in AppState
    *state.last_screenshot.lock().unwrap() = Some(base64_string.clone());
    
    // Emit an event with the captured image data
    app.emit("highlight-capture-success", &base64_string).map_err(|e| e.to_string())?;
    
    Ok(base64_string)
}

// Function to handle "Text Mode" - simulate copy and read clipboard
pub fn handle_text_highlight(app: &AppHandle) {
    println!("📝 [DEBUG] Handling Text Highlight Mode - START");
    
    // 1. Simulate Ctrl+C / Cmd+C
    use enigo::{Enigo, Key, Keyboard, Settings, Direction};
    let mut enigo = match Enigo::new(&Settings::default()) {
        Ok(e) => e,
        Err(e) => {
            eprintln!("❌ [DEBUG] Failed to initialize Enigo: {}", e);
            return;
        }
    };
    
    println!("⌨️ [DEBUG] Simulating Copy Shortcut...");
    #[cfg(target_os = "macos")]
    {
        let _ = enigo.key(Key::Meta, Direction::Press);
        let _ = enigo.key(Key::Unicode('c'), Direction::Click);
        let _ = enigo.key(Key::Meta, Direction::Release);
    }
    
    #[cfg(not(target_os = "macos"))]
    {
        let _ = enigo.key(Key::Control, Direction::Press);
        let _ = enigo.key(Key::Unicode('c'), Direction::Click);
        let _ = enigo.key(Key::Control, Direction::Release);
    }
    
    // 2. Wait a bit for clipboard to update
    println!("⏳ [DEBUG] Waiting for clipboard update...");
    std::thread::sleep(std::time::Duration::from_millis(300)); // Increased to 300ms
    
    // 3. Read clipboard using Tauri's clipboard plugin
    match app.clipboard().read_text() {
        Ok(text) => {
            if text.trim().is_empty() {
                println!("⚠️ [DEBUG] Captured text is empty!");
            } else {
                println!("📋 [DEBUG] Captured text: {} chars", text.len());
            }
            // Store in AppState
            if let Some(state) = app.try_state::<AppState>() {
                *state.last_selection.lock().unwrap() = Some(text.clone());
                // Clear any stale screenshot to prevent confusion in AI models
                *state.last_screenshot.lock().unwrap() = None;
                println!("✅ [DEBUG] Text stored in AppState");
            }
            // Emit event
            let _ = app.emit("highlight-text-capture", text);
        },
        Err(e) => eprintln!("❌ [DEBUG] Failed to read clipboard: {}", e),
    }
    println!("📝 [DEBUG] Handling Text Highlight Mode - END");
}