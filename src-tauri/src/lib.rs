//! Vesper: a calm, honest Roblox launcher for Windows.

// `Problem` is a rich, user-facing error (what / why / fix). It is only returned
// on failure paths that are rare and never in hot loops, so its size is fine.
#![allow(clippy::result_large_err)]

pub mod backup;
pub mod cleanup;
pub mod commands;
pub mod config;
pub mod customize;
pub mod diagnostics;
pub mod discord;
pub mod error;
pub mod flags;
pub mod logging;
pub mod mods;
pub mod paths;
pub mod perf;
pub mod profiles;
pub mod redact;
pub mod roblox;
pub mod session;
pub mod state;
pub mod tray;
pub mod updates;

use crate::config::CloseBehavior;
use crate::discord::presence::Presence;
use crate::state::{events, lock, AppState};
use tauri::{Emitter, Manager, WindowEvent};

pub fn run() {
    let args: Vec<String> = std::env::args().collect();
    if let Some(i) = args.iter().position(|a| a == "--import-font") {
        std::process::exit(customize::import_font_cli(args.get(i + 1).map(String::as_str)));
    }
    if args.iter().any(|a| a == "--cleanup") {
        std::process::exit(cleanup::run_cli());
    }

    let paths = match paths::AppPaths::resolve() {
        Ok(p) => p,
        Err(e) => {
            eprintln!("Vesper couldn't find its folders: {e}");
            std::process::exit(1);
        }
    };
    let _ = paths.ensure();
    let initial_level = config::Settings::load(&paths.settings_file()).settings.advanced.log_level;
    logging::init(&paths.logs, logging::parse_level(&initial_level));
    log::info!("Vesper {} starting{}", env!("CARGO_PKG_VERSION"), if paths.portable { " (portable)" } else { "" });

    let state = match AppState::load(paths) {
        Ok(s) => s,
        Err(e) => {
            log::error!("Startup failed: {e}");
            eprintln!("{e}");
            std::process::exit(1);
        }
    };

    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            tray::show_main(app);
        }))
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            Some(vec!["--autostart"]),
        ))
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .manage(state)
        .setup(|app| {
            let handle = app.handle().clone();
            let state = app.state::<AppState>();

            // Discord presence runs on its own thread.
            let status_app = handle.clone();
            let presence = Presence::start(move |s| {
                let _ = status_app.emit(events::DISCORD, &s);
            });
            *lock(&state.presence) = Some(presence);
            state.refresh_presence();

            tray::create(&handle)?;
            state::spawn_roblox_watcher(handle.clone());
            fit_window_to_screen(&handle);

            // Background checks that never block the window.
            let bg = handle.clone();
            tauri::async_runtime::spawn(async move {
                let st = bg.state::<AppState>();
                if st.settings().roblox.check_roblox_updates {
                    let _ = commands::roblox_latest(bg.state::<AppState>(), false).await;
                }
                if st.settings().updates.auto_check {
                    tokio::time::sleep(std::time::Duration::from_secs(8)).await;
                    let _ = commands::updates_check(bg.clone(), bg.state::<AppState>()).await;
                }
            });
            Ok(())
        })
        .on_window_event(|window, event| {
            let app = window.app_handle();
            let state = app.state::<AppState>();
            match event {
                WindowEvent::CloseRequested { api, .. } => {
                    if state.quitting.load(std::sync::atomic::Ordering::Relaxed) {
                        return;
                    }
                    if state.settings().general.close_behavior == CloseBehavior::Tray {
                        api.prevent_close();
                        // The interface explains the tray the first time, then hides itself.
                        let _ = app.emit(events::CLOSE_REQUESTED, ());
                    } else {
                        api.prevent_close();
                        tray::quit(app);
                    }
                }
                WindowEvent::Resized(_)
                    if state.settings().general.minimize_to_tray && window.is_minimized().unwrap_or(false) =>
                {
                    let _ = window.hide();
                }
                _ => {}
            }
        })
        .invoke_handler(tauri::generate_handler![
            commands::bootstrap,
            commands::notices_take,
            commands::settings_update,
            commands::first_run_finish,
            commands::roblox_detect,
            commands::roblox_parse_target,
            commands::roblox_launch,
            commands::roblox_dismiss_launch,
            commands::roblox_latest,
            commands::flags_validate,
            commands::flags_published,
            commands::flags_installed,
            commands::flags_remove_applied,
            commands::profiles_get,
            commands::profiles_set_active,
            commands::profiles_set_default,
            commands::profiles_create,
            commands::profiles_duplicate,
            commands::profiles_update,
            commands::profiles_delete,
            commands::profiles_reset,
            commands::profiles_export,
            commands::profiles_import,
            commands::fps_cap_status,
            commands::fps_cap_restore,
            commands::perf_watch,
            commands::system_info,
            commands::backups_list,
            commands::backups_create,
            commands::backups_restore,
            commands::backups_delete,
            commands::backups_export,
            commands::backups_import,
            commands::recovery_reset_settings,
            commands::open_folder,
            commands::open_url,
            commands::diagnostics_text,
            commands::logs_tail,
            commands::discord_status,
            commands::updates_check,
            commands::updates_install,
            commands::app_hide,
            commands::app_quit,
            commands::app_ready,
            customize::mods_get,
            customize::mods_set_cursor,
            customize::mods_set_wheel,
            customize::mods_set_font,
            customize::mods_set_sound,
            customize::mods_use_launcher_font,
            customize::mods_set_custom,
            customize::mods_clear,
            customize::mods_open_custom_folder,
            customize::appearance_set_background,
            customize::appearance_clear_background,
            customize::appearance_background_data,
        ])
        .build(tauri::generate_context!())
        .expect("error while building Vesper")
        .run(|app, event| {
            if let tauri::RunEvent::Exit = event {
                let state = app.state::<AppState>();
                lock(&state.monitor).handle = None;
                if let Some(p) = lock(&state.presence).take() {
                    p.shutdown();
                }
                log::logger().flush();
            }
        });
}

/// Keeps the default window inside small screens (1280×720 and up).
fn fit_window_to_screen(app: &tauri::AppHandle) {
    let Some(w) = app.get_webview_window("main") else { return };
    let Ok(Some(monitor)) = w.current_monitor() else { return };
    let scale = monitor.scale_factor();
    let area = monitor.work_area().size.to_logical::<f64>(scale);
    let (want_w, want_h) = (1180.0_f64, 760.0_f64);
    if area.width < want_w + 40.0 || area.height < want_h + 40.0 {
        let width = (area.width * 0.94).max(940.0).min(area.width);
        let height = (area.height * 0.92).max(600.0).min(area.height);
        let _ = w.set_size(tauri::LogicalSize::new(width, height));
        let _ = w.center();
    }
}
