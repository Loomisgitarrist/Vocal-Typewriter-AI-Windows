use rdev::{listen, Event, EventType, Key};
use std::collections::HashSet;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, Mutex,
};
use std::thread;
use tauri::{AppHandle, Emitter, Manager};
use crate::AppState;

static LISTENER_RUNNING: AtomicBool = AtomicBool::new(false);

#[derive(Clone, Debug)]
struct KeyCombo {
    modifiers: HashSet<Key>,
    trigger: Key,
}

pub fn start_listener(app_handle: AppHandle) {
    if LISTENER_RUNNING.swap(true, Ordering::SeqCst) {
        return; // Already running
    }

    thread::spawn(move || {
        let pressed: Arc<Mutex<HashSet<Key>>> = Arc::new(Mutex::new(HashSet::new()));
        let pressed_clone = pressed.clone();
        let app = app_handle.clone();
        let app_for_emit = app.clone();

        let callback = move |event: Event| {
            match event.event_type {
                EventType::KeyPress(key) => {
                    {
                        let mut p = pressed_clone.lock().unwrap();
                        p.insert(key);
                    }

                    let combo = {
                        let state: tauri::State<AppState> = app.state();
                        let settings = state.settings.lock().unwrap();
                        parse_combo(&settings.push_to_talk_key)
                    };

                    if let Some(combo) = combo {
                        // Only trigger on press of the main key, while all modifiers are held.
                        if key == combo.trigger {
                            let p = pressed_clone.lock().unwrap();
                            let mods_ok = combo_modifiers_ok(&combo.modifiers, &p);
                            if mods_ok {
                                let state: tauri::State<AppState> = app.state();
                                let is_recording = state.is_recording.lock().unwrap();
                                if !*is_recording {
                                    drop(is_recording);
                                    let _ = app.emit("start-recording", ());
                                }
                            }
                        }
                    }
                }
                EventType::KeyRelease(key) => {
                    {
                        let mut p = pressed_clone.lock().unwrap();
                        p.remove(&key);
                    }

                    let combo = {
                        let state: tauri::State<AppState> = app.state();
                        let settings = state.settings.lock().unwrap();
                        parse_combo(&settings.push_to_talk_key)
                    };

                    if let Some(combo) = combo {
                        // Stop when either the trigger key or any required modifier is released.
                        if key == combo.trigger || combo.modifiers.contains(&key) {
                            let state: tauri::State<AppState> = app.state();
                            let is_recording = state.is_recording.lock().unwrap();
                            if *is_recording {
                                drop(is_recording);
                                let _ = app.emit("stop-recording", ());
                            }
                        }
                    }
                }
                _ => {}
            }
        };

        let _ = app_for_emit.emit("keyboard-listener-started", ());

        if let Err(e) = listen(callback) {
            eprintln!("Error listening to keyboard: {:?}", e);
            let _ = app_for_emit.emit("keyboard-error", format!("{:?}", e));
            LISTENER_RUNNING.store(false, Ordering::SeqCst);
        }
    });
}

fn parse_combo(combo_str: &str) -> Option<KeyCombo> {
    // Accept formats like:
    // - "RightAlt"
    // - "CommandOrControl+Shift+Space"
    // - "Ctrl+Alt+Space"
    // (we keep this permissive because UI is user-editable)

    let parts: Vec<String> = combo_str
        .split('+')
        .map(|p| p.trim().to_string())
        .filter(|p| !p.is_empty())
        .collect();

    if parts.is_empty() {
        return None;
    }

    let mut modifiers: HashSet<Key> = HashSet::new();
    let mut trigger: Option<Key> = None;

    for part in parts {
        let lower = part.to_lowercase();

        // Map common modifier names used by the UI.
        if lower == "commandorcontrol" {
            // We can't know which one the OS will emit from a string.
            // Treat either Cmd or Ctrl as acceptable by storing BOTH and checking either at runtime.
            // Implemented by representing it as two possible modifiers: MetaLeft/MetaRight OR ControlLeft/ControlRight.
            // We'll check this via a special-case below by adding all of them.
            modifiers.insert(Key::MetaLeft);
            modifiers.insert(Key::MetaRight);
            modifiers.insert(Key::ControlLeft);
            modifiers.insert(Key::ControlRight);
            continue;
        }

        if let Some(k) = parse_key(&lower) {
            // Heuristic: known modifiers are modifiers, everything else is trigger.
            if is_modifier(&k) {
                modifiers.insert(k);
            } else {
                trigger = Some(k);
            }
        }
    }

    let trigger = trigger?;

    // Special handling: if we stored both cmd & ctrl for CommandOrControl, treat it as (cmd OR ctrl)
    // by removing them from strict modifier check and instead checking "any-of".
    // We'll implement this by allowing the set to contain the 4 keys, but checking them in a relaxed way.
    // This is handled in combo_modifiers_ok().
    Some(KeyCombo { modifiers, trigger })
}

fn combo_modifiers_ok(mods: &HashSet<Key>, pressed: &HashSet<Key>) -> bool {
    let cmd_ctrl_keys = [Key::MetaLeft, Key::MetaRight, Key::ControlLeft, Key::ControlRight];
    let has_cmd_ctrl_in_combo = cmd_ctrl_keys.iter().any(|k| mods.contains(k));

    // Check all non-cmd/ctrl modifiers strictly.
    for m in mods.iter() {
        if cmd_ctrl_keys.contains(m) {
            continue;
        }
        if !pressed.contains(m) {
            return false;
        }
    }

    if has_cmd_ctrl_in_combo {
        let any_cmd_ctrl_pressed = cmd_ctrl_keys.iter().any(|k| pressed.contains(k));
        if !any_cmd_ctrl_pressed {
            return false;
        }
    }

    true
}

fn is_modifier(key: &Key) -> bool {
    matches!(
        key,
        Key::Alt
            | Key::AltGr
            | Key::ShiftLeft
            | Key::ShiftRight
            | Key::ControlLeft
            | Key::ControlRight
            | Key::MetaLeft
            | Key::MetaRight
            | Key::CapsLock
    )
}

fn parse_key(key_str: &str) -> Option<Key> {
    match key_str {
        "rightalt" | "altgr" => Some(Key::AltGr),
        "leftalt" | "alt" => Some(Key::Alt),
        "rightcontrol" | "rightctrl" => Some(Key::ControlRight),
        "leftcontrol" | "leftctrl" | "control" | "ctrl" => Some(Key::ControlLeft),
        "rightshift" => Some(Key::ShiftRight),
        "leftshift" | "shift" => Some(Key::ShiftLeft),
        "rightmeta" | "rightcommand" | "rightcmd" => Some(Key::MetaRight),
        "leftmeta" | "leftcommand" | "leftcmd" | "meta" | "command" | "cmd" => Some(Key::MetaLeft),
        "space" => Some(Key::Space),
        "capslock" => Some(Key::CapsLock),
        "tab" => Some(Key::Tab),
        "escape" | "esc" => Some(Key::Escape),
        "enter" | "return" => Some(Key::Return),
        "backspace" => Some(Key::Backspace),
        // Letters
        "a" => Some(Key::KeyA),
        "b" => Some(Key::KeyB),
        "c" => Some(Key::KeyC),
        "d" => Some(Key::KeyD),
        "e" => Some(Key::KeyE),
        "f" => Some(Key::KeyF),
        "g" => Some(Key::KeyG),
        "h" => Some(Key::KeyH),
        "i" => Some(Key::KeyI),
        "j" => Some(Key::KeyJ),
        "k" => Some(Key::KeyK),
        "l" => Some(Key::KeyL),
        "m" => Some(Key::KeyM),
        "n" => Some(Key::KeyN),
        "o" => Some(Key::KeyO),
        "p" => Some(Key::KeyP),
        "q" => Some(Key::KeyQ),
        "r" => Some(Key::KeyR),
        "s" => Some(Key::KeyS),
        "t" => Some(Key::KeyT),
        "u" => Some(Key::KeyU),
        "v" => Some(Key::KeyV),
        "w" => Some(Key::KeyW),
        "x" => Some(Key::KeyX),
        "y" => Some(Key::KeyY),
        "z" => Some(Key::KeyZ),
        // Function keys
        "f1" => Some(Key::F1),
        "f2" => Some(Key::F2),
        "f3" => Some(Key::F3),
        "f4" => Some(Key::F4),
        "f5" => Some(Key::F5),
        "f6" => Some(Key::F6),
        "f7" => Some(Key::F7),
        "f8" => Some(Key::F8),
        "f9" => Some(Key::F9),
        "f10" => Some(Key::F10),
        "f11" => Some(Key::F11),
        "f12" => Some(Key::F12),
        _ => None,
    }
}
