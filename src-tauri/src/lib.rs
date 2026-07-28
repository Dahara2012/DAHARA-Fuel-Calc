#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod sidecar;
mod window;

use std::sync::atomic::{AtomicBool, Ordering};

use tauri::{
    menu::{CheckMenuItemBuilder, MenuItemBuilder, MenuBuilder, SubmenuBuilder},
    tray::TrayIconBuilder,
    Emitter, Manager, PhysicalPosition, WindowEvent,
};

#[tauri::command]
fn save_window_position(app: tauri::AppHandle, x: i32, y: i32) {
    window::save_position(&app, PhysicalPosition::new(x, y));
}

pub fn run() {
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

            let app_handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                if let Err(err) = sidecar::spawn_and_pump(app_handle.clone()).await {
                    eprintln!("[host] sidecar failed to start: {err}");
                }
            });

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![save_window_position])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
