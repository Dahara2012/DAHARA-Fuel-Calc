#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod telemetry;
mod window;

use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use tauri::{
    menu::{CheckMenuItemBuilder, MenuItemBuilder, MenuBuilder, SubmenuBuilder},
    tray::TrayIconBuilder,
    Emitter, Manager, PhysicalPosition, WindowEvent,
};

use telemetry::TelemetryEvent;

#[tauri::command]
fn save_window_position(app: tauri::AppHandle, x: i32, y: i32) {
    window::save_position(&app, PhysicalPosition::new(x, y));
}

#[tauri::command]
async fn start_telemetry(
    channel: tauri::ipc::Channel<TelemetryEvent>,
) -> Result<(), String> {
    tauri::async_runtime::spawn(async move {
        let mut attempt: u32 = 0;
        let mut last_hint: Option<String> = None;
        loop {
            attempt += 1;
            match pitwall::LiveConnection::connect().await {
                Ok(conn) => {
                    eprintln!("[telemetry] STATUS: connected to iRacing (attempt {})", attempt);
                    telemetry::run_telemetry(conn, channel.clone()).await;
                    eprintln!("[telemetry] STATUS: disconnected");
                }
                Err(e) => {
                    eprintln!(
                        "[telemetry] ERROR: connect failed (attempt {}): {}",
                        attempt, format_error_chain(&e)
                    );
                    if let Some(hint) = connect_error_hint(&e) {
                        if last_hint.as_deref() != Some(hint.as_str()) {
                            last_hint = Some(hint.clone());
                            eprintln!("[telemetry] HINT: {hint}");
                        }
                    }
                }
            }
            let _ = channel.send(TelemetryEvent::Status { connected: false });
            eprintln!("[telemetry] STATUS: not connected - retrying in 2s");
            tokio::time::sleep(Duration::from_secs(2)).await;
        }
    });

    Ok(())
}

fn format_error_chain(err: &dyn std::error::Error) -> String {
    let mut parts = vec![err.to_string()];
    let mut cur = err.source();
    while let Some(src) = cur {
        parts.push(src.to_string());
        cur = src.source();
    }
    parts.join(" -> ")
}

fn connect_error_hint(err: &dyn std::error::Error) -> Option<String> {
    #[cfg(windows)]
    {
        let mut cur = Some(err);
        while let Some(e) = cur {
            if let Some(pit_err) = e.downcast_ref::<pitwall::TelemetryError>() {
                if let pitwall::TelemetryError::WindowsApi { source, .. } = pit_err {
                    return match source.code().0 & 0xFFFF {
                        2 => Some(
                            "iRacing is not running - start it and the overlay will connect \
                             automatically."
                                .into(),
                        ),
                        5 => Some(
                            "Access denied opening iRacing shared memory - check antivirus \
                             settings or run the app with the same elevation as iRacing."
                                .into(),
                        ),
                        _ => None,
                    };
                }
            }
            cur = e.source();
        }
    }
    None
}

pub fn run() {
    if std::env::var("DAHARA_LOG").is_ok() || cfg!(debug_assertions) {
        let filter = match std::env::var("DAHARA_LOG") {
            Ok(v) if !v.is_empty() => tracing_subscriber::EnvFilter::new(v),
            _ => tracing_subscriber::EnvFilter::new("info"),
        };
        let _ = tracing_subscriber::fmt().with_env_filter(filter).try_init();
    }

    tauri::Builder::default()
        .setup(|app| {
            let move_item = CheckMenuItemBuilder::with_id("move", "Move Overlay")
                .checked(false)
                .build(app)?;

            let reset_item = MenuItemBuilder::with_id("reset", "Reset Position")
                .build(app)?;

            let quit_item = MenuItemBuilder::with_id("quit", "Quit")
                .build(app)?;

            let nudge_left = MenuItemBuilder::with_id("nudge-left", "← Nudge Left")
                .build(app)?;
            let nudge_right = MenuItemBuilder::with_id("nudge-right", "→ Nudge Right")
                .build(app)?;
            let nudge_up = MenuItemBuilder::with_id("nudge-up", "↑ Nudge Up")
                .build(app)?;
            let nudge_down = MenuItemBuilder::with_id("nudge-down", "↓ Nudge Down")
                .build(app)?;

            let nudge_submenu = SubmenuBuilder::new(app, "Nudge")
                .item(&nudge_left)
                .item(&nudge_right)
                .item(&nudge_up)
                .item(&nudge_down)
                .build()?;

            let menu = MenuBuilder::new(app)
                .item(&move_item)
                .item(&nudge_submenu)
                .separator()
                .item(&reset_item)
                .separator()
                .item(&quit_item)
                .build()?;

            let icon_bytes = include_bytes!("../icons/icon.png");
            let icon = tauri::image::Image::from_bytes(icon_bytes)
                .expect("invalid tray icon");

            let move_clone = move_item.clone();
            let move_active = AtomicBool::new(false);

            let tray = TrayIconBuilder::new()
                .icon(icon)
                .tooltip("DAHARA Fuel Calc")
                .menu(&menu)
                .on_menu_event(move |app, event| {
                    if event.id() == "move" {
                        let is_checked = move_clone.is_checked().unwrap_or(false);
                        eprintln!("[tray] move clicked, is_checked={}", is_checked);
                        if let Some(win) = app.get_webview_window("overlay") {
                            eprintln!("[tray] found overlay window");
                            let now_active =
                                window::toggle_move_mode(&win, app, !is_checked);
                            move_active.store(now_active, Ordering::Relaxed);
                        } else {
                            eprintln!("[tray] WARNING: overlay window NOT FOUND");
                        }
                    } else if event.id() == "nudge-left" {
                        if let Some(win) = app.get_webview_window("overlay") {
                            window::nudge_window(&win, app, -10, 0);
                        }
                    } else if event.id() == "nudge-right" {
                        if let Some(win) = app.get_webview_window("overlay") {
                            window::nudge_window(&win, app, 10, 0);
                        }
                    } else if event.id() == "nudge-up" {
                        if let Some(win) = app.get_webview_window("overlay") {
                            window::nudge_window(&win, app, 0, -10);
                        }
                    } else if event.id() == "nudge-down" {
                        if let Some(win) = app.get_webview_window("overlay") {
                            window::nudge_window(&win, app, 0, 10);
                        }
                    } else if event.id() == "reset" {
                        if let Some(win) = app.get_webview_window("overlay") {
                            let _ = win.set_position(PhysicalPosition::new(20, 20));
                            window::save_position(app, PhysicalPosition::new(20, 20));
                            if move_active.load(Ordering::Relaxed) {
                                let _ = win.set_ignore_cursor_events(true);
                                let _ = win.emit("move-mode", false);
                                let _ = move_clone.set_checked(false);
                                move_active.store(false, Ordering::Relaxed);
                            }
                        }
                    } else if event.id() == "quit" {
                        if move_active.load(Ordering::Relaxed) {
                            if let Some(win) = app.get_webview_window("overlay") {
                                if let Ok(pos) = win.outer_position() {
                                    window::save_position(app, pos);
                                }
                            }
                        }
                        app.exit(0);
                    }
                })
                .build(app)?;

            std::mem::forget(tray);

            let saved_pos = window::load_position(app.handle());
            if let Some(win) = app.get_webview_window("overlay") {
                window::configure_overlay_window(&win, saved_pos);

                let save_on_handle = app.handle().clone();
                let save_on_win = win.clone();
                win.on_window_event(move |event| {
                    if let WindowEvent::CloseRequested { .. } = event {
                        if let Ok(pos) = save_on_win.outer_position() {
                            window::save_position(&save_on_handle, pos);
                        }
                    }
                });
            }

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![save_window_position, start_telemetry])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
