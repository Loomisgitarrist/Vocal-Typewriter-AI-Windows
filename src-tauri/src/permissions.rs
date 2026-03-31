use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PermissionStatus {
    pub accessibility: bool,
    pub microphone: bool,
    pub input_monitoring: bool,
    pub screen_capture: bool,
}

#[cfg(target_os = "macos")]
pub fn check_all_permissions() -> PermissionStatus {
    PermissionStatus {
        accessibility: check_accessibility(),
        microphone: check_microphone(),
        input_monitoring: check_input_monitoring(),
        screen_capture: check_screen_capture(),
    }
}

#[cfg(not(target_os = "macos"))]
pub fn check_all_permissions() -> PermissionStatus {
    // On non-macOS, assume permissions granted
    PermissionStatus {
        accessibility: true,
        microphone: true,
        input_monitoring: true,
        screen_capture: true,
    }
}

#[cfg(target_os = "macos")]
fn check_accessibility() -> bool {
    #[link(name = "ApplicationServices", kind = "framework")]
    extern "C" {
        fn AXIsProcessTrusted() -> bool;
    }
    unsafe { AXIsProcessTrusted() }
}

#[cfg(target_os = "macos")]
fn check_microphone() -> bool {
    use cocoa::base::{id, nil};
    use cocoa::foundation::NSString;
    use objc::runtime::Class;
    use objc::{class, msg_send, sel, sel_impl};

    #[link(name = "AVFoundation", kind = "framework")]
    extern "C" {}

    unsafe {
        let cls = Class::get("AVCaptureDevice");
        if cls.is_none() {
            return false;
        }
        let cls = cls.unwrap();
        
        // AVMediaTypeAudio is "soun"
        let media_type = NSString::alloc(nil).init_str("soun");
        let status: i32 = msg_send![cls, authorizationStatusForMediaType:media_type];
        // Release the string
        let _: () = msg_send![media_type, release];
        
        // AVAuthorizationStatusAuthorized = 3
        status == 3
    }
}

#[cfg(target_os = "macos")]
fn check_input_monitoring() -> bool {
    // Input monitoring permission is tied to accessibility on macOS
    // If we have accessibility, we likely have input monitoring
    check_accessibility()
}

#[cfg(target_os = "macos")]
fn check_screen_capture() -> bool {
    #[link(name = "CoreGraphics", kind = "framework")]
    extern "C" {
        fn CGPreflightScreenCaptureAccess() -> bool;
    }
    unsafe { CGPreflightScreenCaptureAccess() }
}

use tauri::AppHandle;
use tauri_plugin_shell::ShellExt;

#[cfg(target_os = "macos")]
pub fn open_accessibility_preferences(app: &AppHandle) {
    // Open Privacy & Security > Accessibility pane
    let _ = app.shell().open("x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility", None);
}

#[cfg(target_os = "macos")]
#[allow(dead_code)]
pub fn open_microphone_preferences(app: &AppHandle) {
    // Open Privacy & Security > Microphone pane
    let _ = app.shell().open("x-apple.systempreferences:com.apple.preference.security?Privacy_Microphone", None);
}

#[cfg(target_os = "macos")]
pub fn open_input_monitoring_preferences(app: &AppHandle) {
    // Open Privacy & Security > Input Monitoring pane (for keyboard shortcuts)
    let _ = app.shell().open("x-apple.systempreferences:com.apple.preference.security?Privacy_ListenEvent", None);
}

#[cfg(target_os = "macos")]
#[allow(dead_code)]
pub fn reset_accessibility_permissions() {
    use std::process::Command;
    println!("🔄 Resetting Accessibility Permissions via tccutil...");
    let _ = Command::new("tccutil")
        .args(["reset", "Accessibility", "com.vocaltypewriter.app"])
        .output();
}

#[cfg(not(target_os = "macos"))]
pub fn open_accessibility_preferences(app: &AppHandle) {
    use std::process::Command;
    // On Windows, open Speech Privacy settings
    #[cfg(target_os = "windows")]
    let _ = Command::new("cmd")
        .args(["/C", "start", "ms-settings:privacy-speech"])
        .spawn();
}

#[cfg(not(target_os = "macos"))]
#[allow(dead_code)]
pub fn open_microphone_preferences(app: &AppHandle) {
    use std::process::Command;
    // On Windows, open Microphone Privacy settings
    #[cfg(target_os = "windows")]
    let _ = Command::new("cmd")
        .args(["/C", "start", "ms-settings:privacy-microphone"])
        .spawn();
}

#[cfg(not(target_os = "macos"))]
pub fn open_input_monitoring_preferences(app: &AppHandle) {
    use std::process::Command;
    // On Windows, open General Privacy settings
    #[cfg(target_os = "windows")]
    let _ = Command::new("cmd")
        .args(["/C", "start", "ms-settings:privacy"])
        .spawn();
}

#[cfg(not(target_os = "macos"))]
pub fn reset_accessibility_permissions() {
    // No-op on Windows/Linux as they don't have the same TCC permission zombie issues as macOS
    println!("Windows/Linux: No accessibility permission reset needed.");
}
