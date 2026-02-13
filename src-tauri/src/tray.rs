//! System tray implementation for Paporg desktop.

use std::sync::Arc;

use log::info;
use tauri::{
    image::Image,
    menu::{Menu, MenuItem, PredefinedMenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    AppHandle, Manager,
};
use tokio::sync::RwLock;

use crate::state::TauriAppState;

/// Sets up the system tray icon and menu.
pub fn setup_tray(app: &AppHandle) -> Result<(), Box<dyn std::error::Error>> {
    // Create menu items
    let show_item = MenuItem::with_id(app, "show", "Show Window", true, None::<&str>)?;
    let process_item = MenuItem::with_id(app, "process", "Process Now", true, None::<&str>)?;
    let separator = PredefinedMenuItem::separator(app)?;
    let quit_item = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;

    // Build the menu
    let menu = Menu::with_items(app, &[&show_item, &process_item, &separator, &quit_item])?;

    // Load tray icon
    let icon = load_tray_icon()?;

    // Build tray
    let _tray = TrayIconBuilder::new()
        .icon(icon)
        .menu(&menu)
        .tooltip("Paporg - Document Processing")
        .on_menu_event(move |app, event| {
            handle_menu_event(app, event.id.as_ref());
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                // Show window on left click
                if let Some(window) = tray.app_handle().get_webview_window("main") {
                    if let Err(e) = window.show() {
                        log::warn!("Failed to show main window: {}", e);
                    }
                    if let Err(e) = window.set_focus() {
                        log::warn!("Failed to focus main window: {}", e);
                    }
                }
            }
        })
        .build(app)?;

    info!("System tray initialized");
    Ok(())
}

/// Loads the tray icon from the icons directory.
///
/// TODO: Replace placeholder icon with actual icon from icons/icon.png before release.
/// The placeholder is a simple 16x16 blue square for development purposes.
fn load_tray_icon() -> Result<Image<'static>, Box<dyn std::error::Error>> {
    // TODO: Load real icon from icons/icon.png and remove placeholder code
    // For now, create a simple 16x16 placeholder icon

    // Create a simple colored square as placeholder
    let size = 16usize;
    let mut rgba = Vec::with_capacity(size * size * 4);
    for _ in 0..(size * size) {
        // Blue color: #4a90d9
        rgba.push(74); // R
        rgba.push(144); // G
        rgba.push(217); // B
        rgba.push(255); // A
    }

    Ok(Image::new_owned(rgba, size as u32, size as u32))
}

/// Handles menu item clicks.
fn handle_menu_event(app: &AppHandle, item_id: &str) {
    match item_id {
        "show" => {
            if let Some(window) = app.get_webview_window("main") {
                if let Err(e) = window.show() {
                    log::warn!("Failed to show main window: {}", e);
                }
                if let Err(e) = window.set_focus() {
                    log::warn!("Failed to focus main window: {}", e);
                }
            }
        }
        "process" => {
            info!("Process Now triggered from tray menu");
            let app_clone = app.clone();
            tauri::async_runtime::spawn(async move {
                let state: &Arc<RwLock<TauriAppState>> =
                    app_clone.state::<Arc<RwLock<TauriAppState>>>().inner();
                let state = state.read().await;
                state.trigger_processing();
            });
        }
        "quit" => {
            info!("Quit requested from tray menu");
            // Graceful shutdown
            let app_clone = app.clone();
            tauri::async_runtime::spawn(async move {
                let state: &Arc<RwLock<TauriAppState>> =
                    app_clone.state::<Arc<RwLock<TauriAppState>>>().inner();
                let mut state = state.write().await;
                state.stop_workers();
                drop(state);
                app_clone.exit(0);
            });
        }
        _ => {}
    }
}
