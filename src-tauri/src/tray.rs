//! The system tray icon and its menu.

use crate::roblox::deeplink::LaunchTarget;
use crate::state::{events, lock, AppState};
use std::sync::atomic::Ordering;
use tauri::image::Image;
use tauri::menu::{CheckMenuItem, Menu, MenuItem, PredefinedMenuItem, Submenu};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Emitter, Manager, Wry};

pub const TRAY_ID: &str = "vesper";

fn build_menu(app: &AppHandle) -> tauri::Result<Menu<Wry>> {
    let state = app.state::<AppState>();
    let running = state.roblox_running.load(Ordering::Relaxed);
    let store = lock(&state.profiles).clone();

    let show = MenuItem::with_id(app, "show", "Show Vesper", true, None::<&str>)?;
    let launch_label = if running { "Roblox is running" } else { "Launch Roblox" };
    let launch = MenuItem::with_id(app, "launch", launch_label, !running, None::<&str>)?;
    let profiles = Submenu::with_id(app, "profiles", format!("Profile: {}", store.active().name), true)?;
    for p in &store.profiles {
        let item = CheckMenuItem::with_id(app, format!("profile:{}", p.id), &p.name, true, p.id == store.active, None::<&str>)?;
        profiles.append(&item)?;
    }
    let settings = MenuItem::with_id(app, "settings", "Settings…", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "Quit Vesper", true, None::<&str>)?;
    Menu::with_items(
        app,
        &[
            &show,
            &launch,
            &PredefinedMenuItem::separator(app)?,
            &profiles,
            &settings,
            &PredefinedMenuItem::separator(app)?,
            &quit,
        ],
    )
}

pub fn create(app: &AppHandle) -> tauri::Result<()> {
    let icon = Image::from_bytes(include_bytes!("../icons/tray.png"))?;
    TrayIconBuilder::with_id(TRAY_ID)
        .icon(icon)
        .tooltip("Vesper")
        .menu(&build_menu(app)?)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| on_menu(app, event.id().as_ref()))
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click { button: MouseButton::Left, button_state: MouseButtonState::Up, .. } = event {
                show_main(tray.app_handle());
            }
        })
        .build(app)?;
    Ok(())
}

/// Rebuilds the menu after profile or Roblox state changes.
pub fn refresh(app: &AppHandle) {
    if let Some(tray) = app.tray_by_id(TRAY_ID) {
        match build_menu(app) {
            Ok(menu) => {
                let _ = tray.set_menu(Some(menu));
            }
            Err(e) => log::warn!("Couldn't rebuild the tray menu: {e}"),
        }
    }
}

pub fn show_main(app: &AppHandle) {
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.unminimize();
        let _ = w.show();
        let _ = w.set_focus();
    }
}

pub fn quit(app: &AppHandle) {
    let state = app.state::<AppState>();
    state.quitting.store(true, Ordering::Relaxed);
    lock(&state.monitor).handle = None;
    if let Some(p) = lock(&state.presence).take() {
        p.shutdown();
    }
    log::info!("Vesper is quitting");
    log::logger().flush();
    app.exit(0);
}

fn on_menu(app: &AppHandle, id: &str) {
    match id {
        "show" => show_main(app),
        "launch" => {
            if let Err(problem) = crate::session::begin(app.clone(), LaunchTarget::App) {
                show_main(app);
                let state = app.state::<AppState>();
                state.set_launch(app, crate::state::LaunchStatus::Failed { problem });
            }
        }
        "settings" => {
            show_main(app);
            let _ = app.emit(events::NAVIGATE, "settings");
        }
        "quit" => quit(app),
        other => {
            if let Some(profile) = other.strip_prefix("profile:") {
                let state = app.state::<AppState>();
                let ok = lock(&state.profiles).set_active(profile).is_ok();
                if ok {
                    let _ = state.save_profiles();
                    let store = lock(&state.profiles).clone();
                    let _ = app.emit(events::PROFILES, &store);
                    state.refresh_presence();
                }
                refresh(app);
            }
        }
    }
}
