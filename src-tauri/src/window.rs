use std::fs;
use std::path::PathBuf;
use tauri::{AppHandle, Emitter, Manager, PhysicalPosition, WebviewWindow};

pub fn configure_overlay_window(
    window: &WebviewWindow,
    saved_pos: Option<PhysicalPosition<i32>>,
) {
    let _ = window.set_decorations(false);
    let _ = window.set_always_on_top(true);
    let _ = window.set_skip_taskbar(true);
    let _ = window.set_resizable(false);
    let _ = window.set_title("DAHARA Fuel Calc");
    let pos = saved_pos.unwrap_or(PhysicalPosition::new(20, 20));
    let _ = window.set_position(pos);
    let _ = window.set_size(tauri::PhysicalSize::new(200, 110));
    let _ = window.set_ignore_cursor_events(true);
}

/// Toggles move mode. `was_active` should be the state *before* the
/// toggle.  Returns `true` if now in move mode.
pub fn toggle_move_mode(win: &WebviewWindow, app: &AppHandle, was_active: bool) -> bool {
    if was_active {
        if let Ok(pos) = win.outer_position() {
            save_position(app, pos);
        }
        let _ = win.set_ignore_cursor_events(true);
        let _ = win.emit("move-mode", false);
        eprintln!("[window] exited move mode");
        false
    } else {
        let _ = win.set_ignore_cursor_events(false);
        let _ = win.set_focus();
        let _ = win.emit("move-mode", true);
        eprintln!("[window] entered move mode");
        true
    }
}

fn config_path(app: &AppHandle) -> PathBuf {
    app.path()
        .app_config_dir()
        .expect("failed to resolve config dir")
        .join("overlay_position.json")
}

pub fn save_position(app: &AppHandle, pos: PhysicalPosition<i32>) {
    let path = config_path(app);
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    let content = serde_json::json!({ "x": pos.x, "y": pos.y });
    if let Ok(json) = serde_json::to_string_pretty(&content) {
        let _ = fs::write(path, json);
    }
}

pub fn nudge_window(win: &WebviewWindow, app: &AppHandle, dx: i32, dy: i32) {
    if let Ok(pos) = win.outer_position() {
        let new_pos = PhysicalPosition::new(pos.x + dx, pos.y + dy);
        let _ = win.set_position(new_pos);
        save_position(app, new_pos);
    }
}

pub fn load_position(app: &AppHandle) -> Option<PhysicalPosition<i32>> {
    let path = config_path(app);
    let content = fs::read_to_string(path).ok()?;
    let data: serde_json::Value = serde_json::from_str(&content).ok()?;
    let x = data.get("x")?.as_i64()? as i32;
    let y = data.get("y")?.as_i64()? as i32;
    Some(PhysicalPosition::new(x, y))
}
