use crate::daemon::state::TrayUpdate;
use std::sync::mpsc;
use tray_icon::menu::{Menu, MenuEvent, MenuItem, PredefinedMenuItem};
use tray_icon::{Icon, TrayIconBuilder};
use tracing::{error, info};

const ICON_SIZE: u32 = 32;

/// Create a simple colored icon programmatically.
fn create_icon(r: u8, g: u8, b: u8) -> Icon {
    let mut rgba = Vec::with_capacity((ICON_SIZE * ICON_SIZE * 4) as usize);
    for y in 0..ICON_SIZE {
        for x in 0..ICON_SIZE {
            // Simple circle
            let cx = ICON_SIZE as f32 / 2.0;
            let cy = ICON_SIZE as f32 / 2.0;
            let dist = ((x as f32 - cx).powi(2) + (y as f32 - cy).powi(2)).sqrt();
            let radius = ICON_SIZE as f32 / 2.0 - 1.0;
            if dist <= radius {
                rgba.extend_from_slice(&[r, g, b, 255]);
            } else {
                rgba.extend_from_slice(&[0, 0, 0, 0]);
            }
        }
    }
    Icon::from_rgba(rgba, ICON_SIZE, ICON_SIZE).expect("Failed to create icon")
}

/// Run the tray icon event loop on the main thread.
/// `update_rx` receives TrayUpdate messages from the async runtime.
/// `shutdown_tx` is called when user clicks "Quit".
pub fn run_tray_loop(
    update_rx: mpsc::Receiver<TrayUpdate>,
    shutdown_tx: tokio::sync::oneshot::Sender<()>,
) {
    let normal_icon = create_icon(100, 200, 100); // green
    let pending_icon = create_icon(255, 165, 0); // orange
    let _error_icon = create_icon(255, 80, 80); // red

    // Build menu
    let status_item = MenuItem::new("Claude Telegram Bridge — Running", false, None);
    let pending_item = MenuItem::new("0 個待處理請求", false, None);
    let separator = PredefinedMenuItem::separator();
    let open_config = MenuItem::new("開啟設定檔…", true, None);
    let quit_item = MenuItem::new("結束", true, None);

    let menu = Menu::new();
    let _ = menu.append(&status_item);
    let _ = menu.append(&pending_item);
    let _ = menu.append(&separator);
    let _ = menu.append(&open_config);
    let _ = menu.append(&quit_item);

    let tray = TrayIconBuilder::new()
        .with_menu(Box::new(menu))
        .with_tooltip("Claude Telegram Bridge")
        .with_icon(normal_icon.clone())
        .build()
        .expect("Failed to build tray icon");

    let quit_id = quit_item.id().clone();
    let open_config_id = open_config.id().clone();

    // Event loop
    let event_loop = MenuEvent::receiver();
    let mut shutdown_tx = Some(shutdown_tx);

    loop {
        // Check for tray updates (non-blocking)
        while let Ok(update) = update_rx.try_recv() {
            match update {
                TrayUpdate::PendingCount(n) => {
                    let label = format!("{} 個待處理請求", n);
                    let _ = pending_item.set_text(&label);
                    if n > 0 {
                        let _ = tray.set_icon(Some(pending_icon.clone()));
                    } else {
                        let _ = tray.set_icon(Some(normal_icon.clone()));
                    }
                }
                TrayUpdate::Error(msg) => {
                    let _ =
                        status_item.set_text(&format!("Claude Telegram Bridge — {}", msg));
                }
                TrayUpdate::ClearError => {
                    let _ = status_item.set_text("Claude Telegram Bridge — Running");
                }
            }
        }

        // Check for menu events
        if let Ok(event) = event_loop.try_recv() {
            if *event.id() == quit_id {
                info!("Quit requested from tray menu");
                if let Some(tx) = shutdown_tx.take() {
                    let _ = tx.send(());
                }
                break;
            } else if *event.id() == open_config_id {
                let config_path = crate::config::Config::config_path();
                if let Err(e) = open::that(&config_path) {
                    error!("Failed to open config: {}", e);
                }
            }
        }

        // Sleep briefly to avoid busy-waiting
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
}
