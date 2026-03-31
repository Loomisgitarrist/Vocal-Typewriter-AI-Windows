use crate::input;
use crate::settings;
use crate::settings::OverlayPosition;
use tauri::{AppHandle, Emitter, Manager, PhysicalPosition, PhysicalSize, WebviewWindowBuilder};
use log::debug;
use raw_window_handle::{HasWindowHandle, RawWindowHandle};

const OVERLAY_WIDTH: f64 = 220.0;
const OVERLAY_HEIGHT: f64 = 50.0;

#[cfg(target_os = "macos")]
const OVERLAY_TOP_OFFSET: f64 = 46.0;
#[cfg(any(target_os = "windows", target_os = "linux"))]
const OVERLAY_TOP_OFFSET: f64 = 4.0;

#[cfg(target_os = "macos")]
const OVERLAY_BOTTOM_OFFSET: f64 = 80.0;

#[cfg(any(target_os = "windows", target_os = "linux"))]
const OVERLAY_BOTTOM_OFFSET: f64 = 80.0;

/// Forces a window to be topmost using Win32 API (Windows only)
/// This is more reliable than Tauri's set_always_on_top which can be overridden
#[cfg(target_os = "windows")]
fn force_overlay_topmost(overlay_window: &tauri::webview::WebviewWindow) {
    use windows::Win32::UI::WindowsAndMessaging::{
        SetWindowPos, HWND_TOPMOST, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE, SWP_SHOWWINDOW,
    };

    // Clone because run_on_main_thread takes 'static
    let overlay_clone = overlay_window.clone();

    // Make sure the Win32 call happens on the UI thread
    let _ = overlay_clone.clone().run_on_main_thread(move || {
        if let Ok(hwnd) = overlay_clone.hwnd() {
            unsafe {
                // Force Z-order: make this window topmost without changing size/pos or stealing focus
                let _ = SetWindowPos(
                    hwnd,
                    Some(HWND_TOPMOST),
                    0,
                    0,
                    0,
                    0,
                    SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE | SWP_SHOWWINDOW,
                );
            }
        }
    });
}

fn get_monitor_with_cursor(app_handle: &AppHandle) -> Option<tauri::Monitor> {
    // NOTE: Removed cursor detection via Enigo to prevent permission prompts on shortcut press.
    // Fallback to primary monitor which is safe and doesn't require extra permissions.
    app_handle.primary_monitor().ok().flatten()
}

fn is_mouse_within_monitor(
    mouse_pos: (i32, i32),
    monitor_pos: &PhysicalPosition<i32>,
    monitor_size: &PhysicalSize<u32>,
) -> bool {
    let (mouse_x, mouse_y) = mouse_pos;
    let PhysicalPosition {
        x: monitor_x,
        y: monitor_y,
    } = *monitor_pos;
    let PhysicalSize {
        width: monitor_width,
        height: monitor_height,
    } = *monitor_size;

    mouse_x >= monitor_x
        && mouse_x < (monitor_x + monitor_width as i32)
        && mouse_y >= monitor_y
        && mouse_y < (monitor_y + monitor_height as i32)
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
pub fn create_recording_overlay(app_handle: &AppHandle) {
    let window = if let Some(w) = app_handle.get_webview_window("recording_overlay") {
        debug!("Recording overlay window already exists");
        w
    } else {
        if let Some((x, y)) = calculate_overlay_position(app_handle) {
            let builder = if let Some(window_config) = app_handle.config().app.windows.iter().find(|w| w.label == "recording_overlay") {
                debug!("Creating recording overlay from config");
                WebviewWindowBuilder::from_config(app_handle, window_config).unwrap_or_else(|e| {
                    debug!("Failed to create builder from config: {}", e);
                    WebviewWindowBuilder::new(
                        app_handle,
                        "recording_overlay",
                        tauri::WebviewUrl::App("overlay.html".into()),
                    )
                })
            } else {
                debug!("Creating recording overlay manually");
                WebviewWindowBuilder::new(
                    app_handle,
                    "recording_overlay",
                    tauri::WebviewUrl::App("overlay.html".into()),
                )
            };

            match builder
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
            .focused(false)
            .visible(false)
            .build()
            {
                Ok(window) => {
                    debug!("Recording overlay window created successfully (hidden)");
                    window
                }
                Err(e) => {
                    debug!("Failed to create recording overlay window: {}", e);
                    return;
                }
            }
        } else {
            return;
        }
    };

    // Apply transparency fix on macOS
    #[cfg(target_os = "macos")]
    apply_macos_transparency(&window);

    // Try to set background color to transparent via eval
    let _ = window.eval("document.body.style.backgroundColor = 'transparent';");
    let _ = window.eval("document.documentElement.style.backgroundColor = 'transparent';");
}

#[cfg(target_os = "macos")]
fn apply_macos_transparency(window: &tauri::WebviewWindow) {
    use cocoa::appkit::{NSWindow, NSColor};
    use cocoa::base::{id, nil};
    use objc::{msg_send, sel, sel_impl};

    if let Ok(handle) = window.window_handle() {
        if let RawWindowHandle::AppKit(appkit_handle) = handle.as_raw() {
            let ns_view = appkit_handle.ns_view.as_ptr() as id;
            unsafe {
                let ns_window: id = msg_send![ns_view, window];
                if ns_window != nil {
                    ns_window.setBackgroundColor_(NSColor::clearColor(nil));
                    ns_window.setOpaque_(false);
                    ns_window.setHasShadow_(false);
                }
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
        
        // Ensure background is transparent
        let _ = overlay_window.eval("document.body.style.backgroundColor = 'transparent';");

        let _ = overlay_window.show();

        // On Windows, aggressively re-assert "topmost" in the native Z-order after showing
        #[cfg(target_os = "windows")]
        force_overlay_topmost(&overlay_window);

        // Emit event to trigger fade-in animation with recording state
        let _ = overlay_window.emit("show-overlay", "recording");
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


