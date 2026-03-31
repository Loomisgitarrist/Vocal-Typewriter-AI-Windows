use tauri::{
    AppHandle, Runtime,
    menu::{Menu, MenuItem, PredefinedMenuItem, Submenu},
};

pub fn create_menu<R: Runtime>(app: &AppHandle<R>) -> Result<Menu<R>, Box<dyn std::error::Error>> {
    // 1. App Menu (Vocal Typewriter)
    let app_menu = Submenu::with_items(
        app,
        "Vocal Typewriter",
        true,
        &[
            &PredefinedMenuItem::about(app, None, None)?,
            &PredefinedMenuItem::separator(app)?,
            &PredefinedMenuItem::services(app, None)?,
            &PredefinedMenuItem::separator(app)?,
            &PredefinedMenuItem::hide(app, None)?,
            &PredefinedMenuItem::hide_others(app, None)?,
            &PredefinedMenuItem::show_all(app, None)?,
            &PredefinedMenuItem::separator(app)?,
            &PredefinedMenuItem::quit(app, None)?,
        ],
    )?;

    // 2. File Menu
    let file_menu = Submenu::with_items(
        app,
        "File",
        true,
        &[
            &PredefinedMenuItem::close_window(app, None)?,
        ],
    )?;

    // 3. Edit Menu
    let edit_menu = Submenu::with_items(
        app,
        "Edit",
        true,
        &[
            &PredefinedMenuItem::undo(app, None)?,
            &PredefinedMenuItem::redo(app, None)?,
            &PredefinedMenuItem::separator(app)?,
            &PredefinedMenuItem::cut(app, None)?,
            &PredefinedMenuItem::copy(app, None)?,
            &PredefinedMenuItem::paste(app, None)?,
            &PredefinedMenuItem::select_all(app, None)?,
        ],
    )?;

    // 4. View Menu
    let view_menu = Submenu::with_items(
        app,
        "View",
        true,
        &[
            &PredefinedMenuItem::fullscreen(app, None)?,
        ],
    )?;

    // 5. Window Menu
    let window_menu = Submenu::with_items(
        app,
        "Window",
        true,
        &[
            &PredefinedMenuItem::minimize(app, None)?,
            &PredefinedMenuItem::separator(app)?,
            &PredefinedMenuItem::close_window(app, None)?,
        ],
    )?;

    // 6. Highlight Menu
    let highlight_menu = Submenu::with_items(
        app,
        "Highlight",
        true,
        &[
            &MenuItem::with_id(app, "highlight_text_mode", "Text Selection Mode", true, None::<&str>)?,
            &MenuItem::with_id(app, "highlight_screenshot_mode", "Screenshot Mode", true, None::<&str>)?,
        ],
    )?;

    // 7. Help Menu
    let help_menu = Submenu::with_items(
        app,
        "Help",
        true,
        &[
            &MenuItem::with_id(app, "contact_email", "Contact Support", true, None::<&str>)?,
        ],
    )?;

    // 8. Models Menu (Added per user request)
    let models_menu = Submenu::with_items(
        app,
        "Models",
        true,
        &[
            &MenuItem::with_id(app, "open_models_folder", "Open Models Folder", true, None::<&str>)?,
            &MenuItem::with_id(app, "change_model_path", "Change Models Folder...", true, None::<&str>)?,
        ],
    )?;

    // Create the menu
    let menu = Menu::with_items(
        app,
        &[
            &app_menu,
            &file_menu,
            &edit_menu,
            &view_menu,
            &window_menu,
            &highlight_menu,
            &models_menu,
            &help_menu,
        ],
    )?;

    Ok(menu)
}
