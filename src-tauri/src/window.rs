use tauri::WebviewWindow;

pub fn configure_overlay_window(window: &WebviewWindow) {
    let _ = window.set_decorations(false);
    let _ = window.set_always_on_top(true);
    let _ = window.set_skip_taskbar(true);
    let _ = window.set_resizable(false);
    let _ = window.set_title("DAHARA Fuel Calc");
    let _ = window.set_position(tauri::PhysicalPosition::new(20, 20));
    let _ = window.set_size(tauri::PhysicalSize::new(200, 110));
    // Intentional: v1 has no drag-to-move or clickable controls, so the
    // overlay is always click-through. Without this call, the 200x110
    // window would steal clicks from iRacing underneath it.
    let _ = window.set_ignore_cursor_events(true);
}
