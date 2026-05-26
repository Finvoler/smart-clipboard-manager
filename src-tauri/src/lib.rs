mod ai;
mod commands;
mod db;
mod models;
mod platform;
mod quick_pool;

use std::{
    process::Command,
    sync::{atomic::AtomicBool, Arc, Mutex},
};

use db::Database;
use models::AppSettings;
use tauri::{
    menu::{Menu, MenuItem},
    tray::TrayIconBuilder,
    Manager,
};

const STARTUP_ARG: &str = "--startup";

#[derive(Clone)]
pub struct AppState {
    pub db: Arc<Mutex<Database>>,
    pub settings: Arc<Mutex<AppSettings>>,
    pub ignore_next_clipboard: Arc<AtomicBool>,
    pub last_foreground_window: Arc<Mutex<Option<isize>>>,
}

pub fn run() {
    install_auto_restart_hook();

    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![
            commands::hide_window,
            commands::execute_paste,
            commands::get_history,
            commands::update_item_text,
            commands::delete_item,
            commands::get_image_data_url,
            commands::toggle_star,
            commands::get_folders,
            commands::create_folder,
            commands::delete_folder,
            commands::move_to_folder,
            commands::get_quick_pool,
            commands::get_quick_suggestions,
            commands::update_quick_item,
            commands::accept_quick_suggestion,
            commands::dismiss_quick_suggestion,
            commands::delete_quick_item,
            commands::star_quick_item,
            commands::get_app_settings,
            commands::save_app_settings,
            commands::test_ai_connection,
            commands::list_ai_models,
            commands::search_local,
            commands::search_ai_semantic,
            commands::trigger_ai_categorize,
            commands::trigger_ocr
        ])
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                if window.label() == "main" {
                    api.prevent_close();
                    let _ = window.hide();
                }
            }
        })
        .setup(|app| {
            let data_dir = app.path().app_data_dir()?;
            std::fs::create_dir_all(&data_dir)?;
            let db_path = data_dir.join("smart_clipboard.sqlite");
            let mut database = Database::open(db_path, data_dir.join("images"))?;
            database.cleanup_retention()?;
            let mut settings = database.get_app_settings()?;
            if !settings.app_enabled {
                settings.app_enabled = true;
                settings = database.save_app_settings(settings)?;
            }
            let run_at_startup = settings.run_at_startup;
            let hide_console_window = settings.hide_console_window;

            let state = AppState {
                db: Arc::new(Mutex::new(database)),
                settings: Arc::new(Mutex::new(settings)),
                ignore_next_clipboard: Arc::new(AtomicBool::new(false)),
                last_foreground_window: Arc::new(Mutex::new(None)),
            };

            let integrations_state = state.clone();
            app.manage(state);
            platform::start_system_integrations(app.handle().clone(), &integrations_state);
            platform::set_run_at_startup(app.handle(), run_at_startup)?;
            platform::set_hide_console_window(hide_console_window)?;
            setup_tray(app.handle())?;
            sync_initial_window_visibility(app.handle());
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("failed to run Smart Clipboard");
}

fn sync_initial_window_visibility(app: &tauri::AppHandle) {
    let Some(window) = app.get_webview_window("main") else {
        return;
    };
    if is_startup_launch() {
        let _ = window.hide();
    } else {
        let _ = window.show();
        let _ = window.set_focus();
    }
}

fn is_startup_launch() -> bool {
    std::env::args_os().any(|arg| arg == std::ffi::OsStr::new(STARTUP_ARG))
}

fn setup_tray(app: &tauri::AppHandle) -> tauri::Result<()> {
    let menu = build_tray_menu(app, current_intercept_win_v(app))?;
    let mut tray = TrayIconBuilder::with_id("main")
        .tooltip("Smart Clipboard")
        .menu(&menu)
        .show_menu_on_left_click(true)
        .on_menu_event(|app, event| {
            let id = event.id().as_ref();
            if id == "show" {
                if let Some(state) = app.try_state::<AppState>() {
                    let _ = platform::show_main_window(app, &state.last_foreground_window);
                }
            } else if id == "toggle_win_v" {
                if let Some(state) = app.try_state::<AppState>() {
                    if let Ok(saved) = toggle_runtime_setting(app, &state, |settings| {
                        settings.intercept_win_v = !settings.intercept_win_v;
                    }) {
                        let _ = sync_tray_menu(app, saved.intercept_win_v);
                    }
                }
            } else if id == "restart" {
                let _ = restart_app(app, "manual");
            } else if id == "quit" {
                app.exit(0);
            }
        });

    if let Some(icon) = app.default_window_icon() {
        tray = tray.icon(icon.clone());
    }
    tray.build(app)?;
    Ok(())
}

fn build_tray_menu(
    app: &tauri::AppHandle,
    intercept_win_v: bool,
) -> tauri::Result<Menu<tauri::Wry>> {
    let show = MenuItem::with_id(app, "show", "Show Smart Clipboard", true, None::<&str>)?;
    let native = MenuItem::with_id(
        app,
        "toggle_win_v",
        win_v_tray_label(intercept_win_v),
        true,
        None::<&str>,
    )?;
    let restart = MenuItem::with_id(
        app,
        "restart",
        "Restart Smart Clipboard",
        true,
        None::<&str>,
    )?;
    let quit = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
    Menu::with_items(app, &[&show, &native, &restart, &quit])
}

fn current_intercept_win_v(app: &tauri::AppHandle) -> bool {
    app.try_state::<AppState>()
        .and_then(|state| {
            state
                .settings
                .lock()
                .ok()
                .map(|settings| settings.intercept_win_v)
        })
        .unwrap_or(true)
}

pub(crate) fn sync_tray_menu(app: &tauri::AppHandle, intercept_win_v: bool) -> Result<(), String> {
    if let Some(tray) = app.tray_by_id("main") {
        let menu = build_tray_menu(app, intercept_win_v).map_err(|error| error.to_string())?;
        tray.set_menu(Some(menu))
            .map_err(|error| error.to_string())?;
    }
    Ok(())
}

fn win_v_tray_label(intercept_win_v: bool) -> &'static str {
    if intercept_win_v {
        "Use Native Win+V"
    } else {
        "Use Smart Clipboard Win+V"
    }
}

fn restart_app(app: &tauri::AppHandle, reason: &str) -> Result<(), String> {
    spawn_replacement_process(reason)?;
    app.exit(0);
    Ok(())
}

fn spawn_replacement_process(reason: &str) -> Result<(), String> {
    let exe = std::env::current_exe().map_err(|error| error.to_string())?;
    let mut cmd = Command::new(exe);
    cmd.env("SMART_CLIPBOARD_RESTART_REASON", reason)
        .env("SMART_CLIPBOARD_RESTARTED", "1");
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x08000000 /* CREATE_NO_WINDOW */);
    }
    cmd.spawn()
        .map(|_| ())
        .map_err(|error| error.to_string())
}

fn install_auto_restart_hook() {
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |panic_info| {
        previous(panic_info);
        if std::env::var_os("SMART_CLIPBOARD_RESTARTED").is_none() {
            let _ = spawn_replacement_process("panic");
        }
    }));
}

fn toggle_runtime_setting<F>(
    app: &tauri::AppHandle,
    state: &AppState,
    update: F,
) -> Result<AppSettings, String>
where
    F: FnOnce(&mut AppSettings),
{
    let saved = {
        let mut settings = state
            .settings
            .lock()
            .map_err(|_| "settings lock poisoned".to_string())?
            .clone();
        update(&mut settings);
        let db = state
            .db
            .lock()
            .map_err(|_| "database lock poisoned".to_string())?;
        db.save_app_settings(settings).map_err(String::from)?
    };
    platform::set_run_at_startup(app, saved.run_at_startup)?;
    platform::set_hide_console_window(saved.hide_console_window)?;
    if let Ok(mut cached) = state.settings.lock() {
        *cached = saved.clone();
    }
    Ok(saved)
}
