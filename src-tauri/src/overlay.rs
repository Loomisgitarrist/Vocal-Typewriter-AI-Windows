use crate::settings;
use crate::settings::OverlayPosition;
use tauri::{AppHandle, Emitter, Manager};

#[cfg(not(target_os = "macos"))]
use tauri::WebviewWindowBuilder;

#[cfg(target_os = "macos")]
use tauri::WebviewUrl;

#[cfg(target_os = "macos")]
use tauri_nspanel::{tauri_panel, CollectionBehavior, PanelBuilder, PanelLevel};

#[cfg(target_os = "macos")]
tauri_panel! {
    panel!(RecordingOverlayPanel {
        config: {
            can_become_key_window: false,
            is_floating_panel: true
        }
    })
}

const OVERLAY_WIDTH: f64 = 172.0;
const OVERLAY_HEIGHT: f64 = 36.0;

const HUD_WIDTH: f64 = 1000.0;
const HUD_HEIGHT: f64 = 400.0;

#[cfg(target_os = "macos")]
const OVERLAY_TOP_OFFSET: f64 = 46.0;
#[cfg(any(target_os = "windows", target_os = "linux"))]
const OVERLAY_TOP_OFFSET: f64 = 4.0;

#[cfg(target_os = "macos")]
const OVERLAY_BOTTOM_OFFSET: f64 = 15.0;

#[cfg(any(target_os = "windows", target_os = "linux"))]
const OVERLAY_BOTTOM_OFFSET: f64 = 40.0;

/// Forces a window to be topmost using Win32 API (Windows only)
/// This is more reliable than Tauri's set_always_on_top which can be overridden
#[cfg(target_os = "windows")]
fn force_overlay_topmost(overlay_window: &tauri::webview::WebviewWindow) {
    // On Windows, sometimes aggressive Z-ordering can cause crashes or instability.
    // For now, let's just rely on Tauri's always_on_top which is safer.
    let _ = overlay_window.set_always_on_top(true);
}

fn get_monitor_with_cursor(app_handle: &AppHandle) -> Option<tauri::Monitor> {
    // NOTE: Removed cursor detection via Enigo to prevent permission prompts on shortcut press.
    // Fallback to primary monitor which is safe and doesn't require extra permissions.
    app_handle.primary_monitor().ok().flatten()
}



fn calculate_overlay_position(app_handle: &AppHandle) -> Option<(f64, f64)> {
    if let Some(monitor) = get_monitor_with_cursor(app_handle) {
        let work_area = monitor.work_area();
        let scale = monitor.scale_factor();
        let work_area_width = work_area.size.width as f64 / scale;
        let work_area_height = work_area.size.height as f64 / scale;
        let work_area_x = work_area.position.x as f64 / scale;
        let work_area_y = work_area.position.y as f64 / scale;

        let settings = settings::load_settings(app_handle);

        let x = work_area_x + (work_area_width - OVERLAY_WIDTH) / 2.0;
        let y = match settings.overlay_position {
            OverlayPosition::Top => work_area_y + OVERLAY_TOP_OFFSET,
            OverlayPosition::Bottom | OverlayPosition::None => {
                // don't subtract the overlay height it puts it too far up
                work_area_y + work_area_height - OVERLAY_BOTTOM_OFFSET
            }
        };

        return Some((x, y));
    }
    None
}

/// Creates the recording overlay window and keeps it hidden by default
#[cfg(not(target_os = "macos"))]
pub fn create_recording_overlay(app_handle: &AppHandle) {
    println!("Starting creation of recording overlay for non-macOS");
    if let Some((x, y)) = calculate_overlay_position(app_handle) {
        println!("Calculated overlay position: x={}, y={}", x, y);
        match WebviewWindowBuilder::new(
            app_handle,
            "recording_overlay",
            tauri::WebviewUrl::App("overlay.html".into()),
        )
        .title("Recording")
        .position(x, y)
        .resizable(false)
        .inner_size(OVERLAY_WIDTH, OVERLAY_HEIGHT)
        .shadow(false)
        .maximizable(false)
        .minimizable(false)
        .closable(false)
        .accept_first_mouse(true)
        .decorations(false)
        .always_on_top(true)
        .skip_taskbar(true)
        .transparent(true)
        .focused(false)
        .visible(false)
        .build()
        {
            Ok(window) => {
                println!("Recording overlay window created successfully (hidden)");
                #[cfg(target_os = "windows")]
                {
                    println!("Forcing overlay topmost on Windows");
                    force_overlay_topmost(&window);
                }
            }
            Err(e) => {
                eprintln!("Failed to create recording overlay window: {}", e);
            }
        }
    } else {
        eprintln!("Failed to calculate overlay position - overlay creation skipped");
    }
}

/// Creates the recording overlay panel and keeps it hidden by default (macOS)
#[cfg(target_os = "macos")]
pub fn create_recording_overlay(app_handle: &AppHandle) {
    if let Some((x, y)) = calculate_overlay_position(app_handle) {
        // PanelBuilder creates a Tauri window then converts it to NSPanel.
        // The window remains registered, so get_webview_window() still works.
        match PanelBuilder::<_, RecordingOverlayPanel>::new(app_handle, "recording_overlay")
            .url(WebviewUrl::App("overlay.html".into()))
            .title("Recording")
            .position(tauri::Position::Logical(tauri::LogicalPosition { x, y }))
            .level(PanelLevel::Status)
            .size(tauri::Size::Logical(tauri::LogicalSize {
                width: OVERLAY_WIDTH,
                height: OVERLAY_HEIGHT,
            }))
            .has_shadow(false)
            .transparent(true)
            .no_activate(true)
            .corner_radius(0.0)
            .with_window(|w| w.decorations(false).transparent(true))
            .collection_behavior(
                CollectionBehavior::new()
                    .can_join_all_spaces()
                    .full_screen_auxiliary(),
            )
            .build()
        {
            Ok(panel) => {
                let _ = panel.hide();
            }
            Err(e) => {
                log::error!("Failed to create recording overlay panel: {}", e);
            }
        }
    }
}

/// Shows the recording overlay window with fade-in animation
pub fn show_recording_overlay(app_handle: &AppHandle) {
    // Check if overlay should be shown based on position setting
    let settings = settings::load_settings(app_handle);
    if settings.overlay_position == OverlayPosition::None {
        return;
    }

    if let Some(overlay_window) = app_handle.get_webview_window("recording_overlay") {
        // Update position before showing to prevent flicker from position changes
        if let Some((x, y)) = calculate_overlay_position(app_handle) {
            let _ = overlay_window
                .set_position(tauri::Position::Logical(tauri::LogicalPosition { x, y }));
        }

        let _ = overlay_window.show();

        // On Windows, aggressively re-assert "topmost" in the native Z-order after showing
        #[cfg(target_os = "windows")]
        force_overlay_topmost(&overlay_window);

        // Emit event to trigger fade-in animation with recording state
        let _ = overlay_window.emit("show-overlay", "recording");
    }
}

/// Shows the highlight overlay window
pub fn show_highlight_overlay(app_handle: &AppHandle) {
    // Check if overlay should be shown based on position setting
    let settings = settings::load_settings(app_handle);
    if settings.overlay_position == OverlayPosition::None {
        return;
    }

    if let Some(overlay_window) = app_handle.get_webview_window("recording_overlay") {
        // Update position before showing to prevent flicker from position changes
        if let Some((x, y)) = calculate_overlay_position(app_handle) {
            let _ = overlay_window
                .set_position(tauri::Position::Logical(tauri::LogicalPosition { x, y }));
        }
        
        // Ensure background is transparent
        let _ = overlay_window.eval("document.body.style.backgroundColor = 'transparent';");

        let _ = overlay_window.show();

        // On Windows, aggressively re-assert "topmost" in the native Z-order after showing
        #[cfg(target_os = "windows")]
        force_overlay_topmost(&overlay_window);

        // Emit event to trigger fade-in animation with highlighting state
        let _ = overlay_window.emit("show-overlay", "highlighting");
    }
}

/// Shows the transcribing overlay window
pub fn show_transcribing_overlay(app_handle: &AppHandle) {
    // Check if overlay should be shown based on position setting
    let settings = settings::load_settings(app_handle);
    if settings.overlay_position == OverlayPosition::None {
        return;
    }

    update_overlay_position(app_handle);

    if let Some(overlay_window) = app_handle.get_webview_window("recording_overlay") {
        let _ = overlay_window.show();

        // On Windows, aggressively re-assert "topmost" in the native Z-order after showing
        #[cfg(target_os = "windows")]
        force_overlay_topmost(&overlay_window);

        // Emit event to switch to transcribing state
        let _ = overlay_window.emit("show-overlay", "transcribing");
    }
}

/// Updates the overlay window position based on current settings
pub fn update_overlay_position(app_handle: &AppHandle) {
    if let Some(overlay_window) = app_handle.get_webview_window("recording_overlay") {
        if let Some((x, y)) = calculate_overlay_position(app_handle) {
            let _ = overlay_window
                .set_position(tauri::Position::Logical(tauri::LogicalPosition { x, y }));
        }
    }
}

/// Hides the recording overlay window with fade-out animation
pub fn hide_recording_overlay(app_handle: &AppHandle) {
    // Always hide the overlay regardless of settings - if setting was changed while recording,
    // we still want to hide it properly
    if let Some(overlay_window) = app_handle.get_webview_window("recording_overlay") {
        // Emit event to trigger fade-out animation
        let _ = overlay_window.emit("hide-overlay", ());
        // Hide the window after a short delay to allow animation to complete
        let window_clone = overlay_window.clone();
        std::thread::spawn(move || {
            std::thread::sleep(std::time::Duration::from_millis(300));
            let _ = window_clone.hide();
        });
    }
}

fn calculate_hud_position(app_handle: &AppHandle) -> Option<(f64, f64)> {
    if let Some(monitor) = get_monitor_with_cursor(app_handle) {
        let work_area = monitor.work_area();
        let scale = monitor.scale_factor();
        let work_area_width = work_area.size.width as f64 / scale;
        let work_area_height = work_area.size.height as f64 / scale;
        let work_area_x = work_area.position.x as f64 / scale;
        let work_area_y = work_area.position.y as f64 / scale;

        let x = work_area_x + (work_area_width - HUD_WIDTH) / 2.0;
        let y = work_area_y + (work_area_height - HUD_HEIGHT) / 2.0;

        return Some((x, y));
    }
    None
}

#[cfg(not(target_os = "macos"))]
pub fn create_hud_window(app_handle: &AppHandle) {
    if let Some((x, y)) = calculate_hud_position(app_handle) {
        match WebviewWindowBuilder::new(
            app_handle,
            "hud_overlay",
            tauri::WebviewUrl::App("overlay.html?type=hud".into()),
        )
        .title("HUD")
        .position(x, y)
        .resizable(false)
        .inner_size(HUD_WIDTH, HUD_HEIGHT)
        .shadow(false)
        .maximizable(false)
        .minimizable(false)
        .closable(false)
        .accept_first_mouse(true)
        .decorations(false)
        .always_on_top(true)
        .skip_taskbar(true)
        .transparent(true)
        .focused(false)
        .visible(false)
        .build()
        {
            Ok(window) => {
                println!("HUD window created");
                 #[cfg(target_os = "windows")]
                {
                    force_overlay_topmost(&window);
                }
            },
            Err(e) => eprintln!("Failed to create HUD window: {}", e),
        }
    }
}

#[cfg(target_os = "macos")]
pub fn create_hud_window(app_handle: &AppHandle) {
    if let Some((x, y)) = calculate_hud_position(app_handle) {
        match PanelBuilder::<_, RecordingOverlayPanel>::new(app_handle, "hud_overlay")
            .url(WebviewUrl::App("overlay.html?type=hud".into()))
            .title("HUD")
            .position(tauri::Position::Logical(tauri::LogicalPosition { x, y }))
            .level(PanelLevel::Status)
            .size(tauri::Size::Logical(tauri::LogicalSize {
                width: HUD_WIDTH,
                height: HUD_HEIGHT,
            }))
            .has_shadow(false)
            .transparent(true)
            .no_activate(true)
            .corner_radius(0.0)
            .with_window(|w| w.decorations(false).transparent(true))
            .collection_behavior(
                CollectionBehavior::new()
                    .can_join_all_spaces()
                    .full_screen_auxiliary(),
            )
            .build()
        {
            Ok(panel) => { let _ = panel.hide(); },
            Err(e) => log::error!("Failed to create HUD panel: {}", e),
        }
    }
}

pub fn show_hud_message(app_handle: &AppHandle, message: &str) {
    if let Some(window) = app_handle.get_webview_window("hud_overlay") {
        if let Some((x, y)) = calculate_hud_position(app_handle) {
             let _ = window.set_position(tauri::Position::Logical(tauri::LogicalPosition { x, y }));
        }
        
        // Show window first to ensure JS runtime is active
        let _ = window.show();
        
        #[cfg(target_os = "windows")]
        force_overlay_topmost(&window);
        
        // Use direct JS execution for immediate update (more reliable than events for waking windows)
        let js = format!("if (window.setHudMessage) window.setHudMessage('{}')", message.replace("'", "\\'"));
        let _ = window.eval(&js);
        
        // Also emit event as backup
        let _ = window.emit("hud-message", message);
        
        // Hide after 2 seconds
        let window_clone = window.clone();
        std::thread::spawn(move || {
            std::thread::sleep(std::time::Duration::from_secs(2));
            let _ = window_clone.hide();
        });
    }
}


