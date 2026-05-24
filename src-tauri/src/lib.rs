mod ai;
mod commands;
mod db;
mod models;
mod platform;
mod quick_pool;

use std::sync::{atomic::AtomicBool, Arc, Mutex};

use db::Database;
use tauri::Manager;

pub struct AppState {
  pub db: Arc<Mutex<Database>>,
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

      let state = AppState {
        db: Arc::new(Mutex::new(database)),
        ignore_next_clipboard: Arc::new(AtomicBool::new(false)),
        last_foreground_window: Arc::new(Mutex::new(None)),
      };

      platform::start_system_integrations(app.handle().clone(), &state);
      app.manage(state);
      Ok(())
    })
    .run(tauri::generate_context!())
    .expect("failed to run Smart Clipboard");
}
