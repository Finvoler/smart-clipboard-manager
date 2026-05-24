mod ai;
mod commands;
mod db;
mod models;
mod platform;
mod quick_pool;

use std::sync::{atomic::AtomicBool, Arc, Mutex};

use db::Database;
use models::AppSettings;
use tauri::{
    menu::{Menu, MenuItem},
    tray::TrayIconBuilder,
    Manager,
};

#[derive(Clone)]
pub struct AppState {
    pub db: Arc<Mutex<Database>>,
    pub settings: Arc<Mutex<AppSettings>>,
    pub ignore_next_clipboard: Arc<AtomicBool>,
    pub last_foreground_window: Arc<Mutex<Option<isize>>>,
}

pub fn run() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![
            commands::hide_window,
            commands::execute_paste,
            commands::get_history,
            commands::update_item_text,
            commands::delete_item,
            commands::toggle_star,
            commands::get_folders,
            commands::create_folder,
            commands::move_to_folder,
            commands::get_quick_pool,
            commands::update_quick_item,
            commands::get_app_settings,
            commands::save_app_settings,
            commands::test_ai_connection,
            commands::search_local,
            commands::search_ai_semantic,
            commands::trigger_ai_categorize,
            commands::trigger_ocr
        ])
        .setup(|app| {
            let data_dir = app.path().app_data_dir()?;
            std::fs::create_dir_all(&data_dir)?;
            let db_path = data_dir.join("smart_clipboard.sqlite");
            let mut database = Database::open(db_path, data_dir.join("images"))?;
            database.cleanup_retention()?;
            let settings = database.get_app_settings()?;
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
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("failed to run Smart Clipboard");
}

fn setup_tray(app: &tauri::AppHandle) -> tauri::Result<()> {
    let show = MenuItem::with_id(app, "show", "Show Smart Clipboard", true, None::<&str>)?;
    let pause = MenuItem::with_id(app, "toggle_enabled", "Pause or Resume", true, None::<&str>)?;
    let native = MenuItem::with_id(app, "native_win_v", "Use Native Win+V", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&show, &pause, &native, &quit])?;
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
            } else if id == "toggle_enabled" {
                if let Some(state) = app.try_state::<AppState>() {
                    let _ = toggle_runtime_setting(app, &state, |settings| {
                        settings.app_enabled = !settings.app_enabled
                    });
                }
            } else if id == "native_win_v" {
                if let Some(state) = app.try_state::<AppState>() {
                    let _ = toggle_runtime_setting(app, &state, |settings| {
                        settings.intercept_win_v = false
                    });
                }
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

fn toggle_runtime_setting<F>(
    app: &tauri::AppHandle,
    state: &AppState,
    update: F,
) -> Result<(), String>
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
        *cached = saved;
    }
    Ok(())
}
