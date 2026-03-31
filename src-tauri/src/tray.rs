use tauri::{
    AppHandle, Manager, Emitter,
    tray::{TrayIcon, TrayIconBuilder, MouseButton, MouseButtonState, TrayIconEvent},
    menu::{Menu, MenuItem, PredefinedMenuItem},
};

pub fn create_tray(app: &AppHandle) -> Result<TrayIcon, Box<dyn std::error::Error>> {
    // Embed the tray icon at compile-time so the app never crashes due to missing runtime files.
    // Path is relative to the Cargo.toml directory (src-tauri/).
    // Use a dedicated tray icon (usually monochrome/template for macOS)
    let icon = tauri::include_image!("icons/tray-icon.png");

    // Create menu items
    let settings_item = MenuItem::with_id(app, "settings", "Settings", true, None::<&str>)?;
    let separator = PredefinedMenuItem::separator(app)?;
    let quit_item = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;

    let menu = Menu::with_items(app, &[&settings_item, &separator, &quit_item])?;

    let tray = TrayIconBuilder::new()
        .icon(icon)
        //.icon_as_template(true)
        .menu(&menu)
        .tooltip("Vocal Typewriter - Press shortcut to dictate")
        .on_menu_event(|app, event| match event.id.as_ref() {
            "settings" => {
                if let Some(window) = app.get_webview_window("main") {
                    // Switch to Regular policy to show Dock icon and Menu Bar
                    #[cfg(target_os = "macos")]
                    app.set_activation_policy(tauri::ActivationPolicy::Regular);

                    let _ = window.show();
                    let _ = window.set_focus();
                    // Force window to front
                    let _ = window.set_always_on_top(true);
                    let _ = window.set_always_on_top(false);
                    let _ = window.emit("open-settings", ());
                }
            }
            "quit" => {
                app.exit(0);
            }
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                let app = tray.app_handle();
        if let Some(window) = app.get_webview_window("main") {
            // Switch to Regular policy to show Dock icon and Menu Bar
            #[cfg(target_os = "macos")]
            app.set_activation_policy(tauri::ActivationPolicy::Regular);

            let _ = window.show();
            let _ = window.set_focus();
            // Force window to front
            let _ = window.set_always_on_top(true);
            let _ = window.set_always_on_top(false);
            let _ = window.emit("open-settings", ());
        }
            }
        })
        .build(app)?;

    Ok(tray)
}
