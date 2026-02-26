use crate::daemon::state::TrayUpdate;
use std::sync::mpsc;
use tray_icon::menu::{Menu, MenuEvent, MenuItem, PredefinedMenuItem};
use tray_icon::{Icon, TrayIconBuilder};
use tracing::{error, info};

/// Icon size (22px is standard for macOS menu bar; 44px for retina).
const ICON_SIZE: u32 = 44;

/// Create a monochrome icon with a letter/symbol.
/// Uses black on transparent so macOS template rendering works
/// (system handles light/dark mode automatically).
fn create_icon(pending: bool) -> Icon {
    let size = ICON_SIZE;
    let mut rgba = vec![0u8; (size * size * 4) as usize];

    // Draw a "C" shape (for Claude) as a thick arc
    let cx = size as f32 / 2.0;
    let cy = size as f32 / 2.0;
    let outer_r = size as f32 / 2.0 - 2.0;
    let inner_r = outer_r - 6.0;

    for y in 0..size {
        for x in 0..size {
            let dx = x as f32 - cx;
            let dy = y as f32 - cy;
            let dist = (dx * dx + dy * dy).sqrt();
            let idx = ((y * size + x) * 4) as usize;

            // Draw the "C" arc (exclude the right side opening)
            if dist >= inner_r && dist <= outer_r {
                let angle = dy.atan2(dx);
                // Opening on the right side: skip angles between -45° and +45°
                if angle.abs() > std::f32::consts::FRAC_PI_4 {
                    rgba[idx] = 0;     // R
                    rgba[idx + 1] = 0; // G
                    rgba[idx + 2] = 0; // B
                    rgba[idx + 3] = 255; // A
                }
            }

            // If pending, draw a small filled dot in the center
            if pending && dist <= 4.0 {
                rgba[idx] = 0;
                rgba[idx + 1] = 0;
                rgba[idx + 2] = 0;
                rgba[idx + 3] = 255;
            }
        }
    }

    Icon::from_rgba(rgba, size, size).expect("Failed to create icon")
}

/// Run the tray icon event loop on the main thread.
/// `update_rx` receives TrayUpdate messages from the async runtime.
/// `shutdown_tx` is called when user clicks "Quit".
pub fn run_tray_loop(
    update_rx: mpsc::Receiver<TrayUpdate>,
    shutdown_tx: tokio::sync::oneshot::Sender<()>,
) {
    let normal_icon = create_icon(false);
    let pending_icon = create_icon(true);

    // Build menu
    let status_item = MenuItem::new("Claude Telegram Bridge — Running", false, None);
    let pending_item = MenuItem::new("0 pending requests", false, None);
    let separator = PredefinedMenuItem::separator();
    let open_config = MenuItem::new("Open config...", true, None);
    let quit_item = MenuItem::new("Quit", true, None);

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
        .with_icon_as_template(true)
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
                    let label = format!("{} pending request{}", n, if n == 1 { "" } else { "s" });
                    let _ = pending_item.set_text(&label);
                    if n > 0 {
                        let _ = tray.set_icon_with_as_template(Some(pending_icon.clone()), true);
                    } else {
                        let _ = tray.set_icon_with_as_template(Some(normal_icon.clone()), true);
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
