use std::sync::atomic::Ordering;

use tauri::{AppHandle, State};

use crate::{
    ai,
    db::AppError,
    models::{AppSettings, ClipboardItem, Folder, QuickItem},
    platform, AppState,
};

#[tauri::command]
pub fn hide_window(app: AppHandle) -> Result<(), String> {
    platform::hide_main_window(&app)
}

#[tauri::command]
pub fn execute_paste(
    item_id: String,
    override_text: Option<String>,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let text = if let Some(text) = override_text {
        text
    } else {
        let db = state
            .db
            .lock()
            .map_err(|_| "database lock poisoned".to_string())?;
        let item = db
            .get_item(&item_id)
            .map_err(String::from)?
            .ok_or_else(|| "record not found".to_string())?;
        item.content
            .ok_or_else(|| "image records need OCR before text paste".to_string())?
    };

    state.ignore_next_clipboard.store(true, Ordering::SeqCst);
    platform::paste_text_and_hide(&app, &text, &state.last_foreground_window)
}

#[tauri::command]
pub fn get_history(
    limit: Option<i64>,
    offset: Option<i64>,
    state: State<'_, AppState>,
) -> Result<Vec<ClipboardItem>, String> {
    let mut db = state
        .db
        .lock()
        .map_err(|_| "database lock poisoned".to_string())?;
    db.cleanup_retention().map_err(String::from)?;
    db.get_history(limit.unwrap_or(120), offset.unwrap_or(0))
        .map_err(String::from)
}

#[tauri::command]
pub fn update_item_text(
    id: String,
    text: String,
    state: State<'_, AppState>,
) -> Result<ClipboardItem, String> {
    let db = state
        .db
        .lock()
        .map_err(|_| "database lock poisoned".to_string())?;
    db.update_item_text(&id, &text).map_err(String::from)
}

#[tauri::command]
pub fn delete_item(id: String, state: State<'_, AppState>) -> Result<(), String> {
    let db = state
        .db
        .lock()
        .map_err(|_| "database lock poisoned".to_string())?;
    db.delete_item(&id).map_err(String::from)
}

#[tauri::command]
pub fn toggle_star(
    id: String,
    is_star: bool,
    state: State<'_, AppState>,
) -> Result<ClipboardItem, String> {
    let db = state
        .db
        .lock()
        .map_err(|_| "database lock poisoned".to_string())?;
    db.toggle_star(&id, is_star).map_err(String::from)
}

#[tauri::command]
pub fn get_folders(state: State<'_, AppState>) -> Result<Vec<Folder>, String> {
    let db = state
        .db
        .lock()
        .map_err(|_| "database lock poisoned".to_string())?;
    db.get_folders().map_err(String::from)
}

#[tauri::command]
pub fn create_folder(name: String, state: State<'_, AppState>) -> Result<Folder, String> {
    let db = state
        .db
        .lock()
        .map_err(|_| "database lock poisoned".to_string())?;
    db.create_folder(&name).map_err(String::from)
}

#[tauri::command]
pub fn move_to_folder(
    item_id: String,
    folder_id: Option<String>,
    state: State<'_, AppState>,
) -> Result<ClipboardItem, String> {
    let db = state
        .db
        .lock()
        .map_err(|_| "database lock poisoned".to_string())?;
    db.move_to_folder(&item_id, folder_id.filter(|value| !value.is_empty()))
        .map_err(String::from)
}

#[tauri::command]
pub fn get_quick_pool(state: State<'_, AppState>) -> Result<Vec<QuickItem>, String> {
    let db = state
        .db
        .lock()
        .map_err(|_| "database lock poisoned".to_string())?;
    db.get_quick_pool().map_err(String::from)
}

#[tauri::command]
pub fn get_app_settings(state: State<'_, AppState>) -> Result<AppSettings, String> {
    state
        .settings
        .lock()
        .map_err(|_| "settings lock poisoned".to_string())
        .map(|settings| settings.clone())
}

#[tauri::command]
pub fn save_app_settings(
    settings: AppSettings,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<AppSettings, String> {
    let saved = {
        let db = state
            .db
            .lock()
            .map_err(|_| "database lock poisoned".to_string())?;
        db.save_app_settings(settings).map_err(String::from)?
    };

    platform::set_run_at_startup(&app, saved.run_at_startup)?;
    platform::set_hide_console_window(saved.hide_console_window)?;
    if let Ok(mut cached) = state.settings.lock() {
        *cached = saved.clone();
    }
    Ok(saved)
}

#[tauri::command]
pub async fn test_ai_connection(state: State<'_, AppState>) -> Result<String, String> {
    let settings = state
        .settings
        .lock()
        .map_err(|_| "settings lock poisoned".to_string())?
        .clone();
    ai::test_connection(&settings).await
}

#[tauri::command]
pub fn update_quick_item(
    id: String,
    content: String,
    ttl: i64,
    state: State<'_, AppState>,
) -> Result<QuickItem, String> {
    let db = state
        .db
        .lock()
        .map_err(|_| "database lock poisoned".to_string())?;
    db.update_quick_item(&id, &content, ttl)
        .map_err(String::from)
}

#[tauri::command]
pub fn search_local(
    keyword: String,
    state: State<'_, AppState>,
) -> Result<Vec<ClipboardItem>, String> {
    let db = state
        .db
        .lock()
        .map_err(|_| "database lock poisoned".to_string())?;
    db.search_local(&keyword).map_err(String::from)
}

#[tauri::command]
pub async fn search_ai_semantic(
    query: String,
    state: State<'_, AppState>,
) -> Result<Vec<String>, String> {
    let records = {
        let db = state
            .db
            .lock()
            .map_err(|_| "database lock poisoned".to_string())?;
        db.search_local(&query).map_err(String::from)?
    };
    let settings = state
        .settings
        .lock()
        .map_err(|_| "settings lock poisoned".to_string())?
        .clone();
    ai::semantic_search(&settings, &query, records).await
}

#[tauri::command]
pub async fn trigger_ai_categorize(
    state: State<'_, AppState>,
) -> Result<Vec<ClipboardItem>, String> {
    let records = {
        let db = state
            .db
            .lock()
            .map_err(|_| "database lock poisoned".to_string())?;
        db.recent_uncategorized(80).map_err(String::from)?
    };
    let settings = state
        .settings
        .lock()
        .map_err(|_| "settings lock poisoned".to_string())?
        .clone();
    let assignments = ai::categorize(&settings, records).await?;
    let db = state
        .db
        .lock()
        .map_err(|_| "database lock poisoned".to_string())?;
    let mut updated = Vec::new();
    for assignment in assignments {
        let folder = db
            .get_or_create_folder(&assignment.folder_name)
            .map_err(String::from)?;
        let item = db
            .move_to_folder(&assignment.item_id, Some(folder.id))
            .map_err(String::from)?;
        updated.push(item);
    }
    Ok(updated)
}

#[tauri::command]
pub async fn trigger_ocr(
    image_id: String,
    state: State<'_, AppState>,
) -> Result<ClipboardItem, String> {
    let item = {
        let db = state
            .db
            .lock()
            .map_err(|_| "database lock poisoned".to_string())?;
        db.get_item(&image_id)
            .map_err(String::from)?
            .ok_or_else(|| AppError::NotFound.to_string())?
    };
    let settings = state
        .settings
        .lock()
        .map_err(|_| "settings lock poisoned".to_string())?
        .clone();
    let text = ai::ocr_image(&settings, &item).await?;
    let mut db = state
        .db
        .lock()
        .map_err(|_| "database lock poisoned".to_string())?;
    db.insert_text_item_from_ocr(&text).map_err(String::from)
}
