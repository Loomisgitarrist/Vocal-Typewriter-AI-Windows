use tauri::{AppHandle, Emitter};

#[allow(dead_code)]
pub fn emit_levels(app_handle: &AppHandle, levels: &[f32]) {
    let _ = app_handle.emit("mic-level", levels);
}