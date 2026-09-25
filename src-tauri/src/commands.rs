//! 前端到后端的 IPC 命令层。
//!
//! 这里尽量只做参数编排、状态锁和错误转换，真正的数据读写在 db.rs，
//! 真正的系统交互在 platform/*，真正的 AI 请求在 ai.rs。

use std::sync::atomic::Ordering;

use base64::{engine::general_purpose, Engine as _};
use tauri::{AppHandle, Emitter, State};

use crate::{
    ai, ocr,
    db::AppError,
    models::{AppSettings, ClipboardItem, Folder, QuickItem, QuickSuggestion},
    platform, schedule_data_dir_change, sync_tray_menu, validate_data_dir_change, AppState,
    DataDirectoryChangeResult,
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
    if let Some(text) = override_text {
        paste_text_and_track(&app, &state, &text)?;
        return Ok(());
    }

    let item = {
        let db = state
            .db
            .lock()
            .map_err(|_| "database lock poisoned".to_string())?;
        db.get_item(&item_id)
            .map_err(String::from)?
            .ok_or_else(|| "record not found".to_string())?
    };

    if item.kind == "image" {
        let image_path = item
            .image_path
            .ok_or_else(|| "image file is missing".to_string())?;
        state.ignore_next_clipboard.store(true, Ordering::SeqCst);
        platform::paste_image_and_hide(&app, &image_path, &state.last_foreground_window)?;
        return Ok(());
    }

    let text = item
        .content
        .ok_or_else(|| "text record has no content".to_string())?;
    paste_text_and_track(&app, &state, &text)
}

fn paste_text_and_track(app: &AppHandle, state: &AppState, text: &str) -> Result<(), String> {
    state.ignore_next_clipboard.store(true, Ordering::SeqCst);
    platform::paste_text_and_hide(app, text, &state.last_foreground_window)?;

    let quick_suggestions = {
        let db = state
            .db
            .lock()
            .map_err(|_| "database lock poisoned".to_string())?;
        db.observe_text_for_quick_pool(&text)
            .map_err(String::from)?
    };
    for quick_suggestion in quick_suggestions {
        let _ = app.emit("on_quick_suggestion_detected", quick_suggestion);
    }
    Ok(())
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
    db.get_history(limit.unwrap_or(0), offset.unwrap_or(0))
        .map_err(String::from)
}

#[tauri::command]
pub fn get_history_light(
    limit: Option<i64>,
    offset: Option<i64>,
    state: State<'_, AppState>,
) -> Result<Vec<ClipboardItem>, String> {
    let mut db = state
        .db
        .lock()
        .map_err(|_| "database lock poisoned".to_string())?;
    db.cleanup_retention().map_err(String::from)?;
    db.get_history_light(limit.unwrap_or(0), offset.unwrap_or(0))
        .map_err(String::from)
}

#[tauri::command]
pub fn get_item(
    id: String,
    state: State<'_, AppState>,
) -> Result<ClipboardItem, String> {
    let db = state
        .db
        .lock()
        .map_err(|_| "database lock poisoned".to_string())?;
    db.get_item(&id)
        .map_err(String::from)?
        .ok_or_else(|| "record not found".to_string())
}

#[tauri::command]
pub fn get_items_by_ids(
    ids: Vec<String>,
    state: State<'_, AppState>,
) -> Result<Vec<ClipboardItem>, String> {
    let db = state
        .db
        .lock()
        .map_err(|_| "database lock poisoned".to_string())?;
    db.get_items_by_ids(&ids).map_err(String::from)
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
pub fn get_image_data_url(id: String, state: State<'_, AppState>) -> Result<String, String> {
    let item = {
        let db = state
            .db
            .lock()
            .map_err(|_| "database lock poisoned".to_string())?;
        db.get_item(&id)
            .map_err(String::from)?
            .ok_or_else(|| AppError::NotFound.to_string())?
    };
    let path = item
        .image_path
        .ok_or_else(|| "image record has no local image path".to_string())?;
    let bytes = std::fs::read(path).map_err(|error| error.to_string())?;
    Ok(format!(
        "data:image/png;base64,{}",
        general_purpose::STANDARD.encode(bytes)
    ))
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
pub fn delete_folder(id: String, state: State<'_, AppState>) -> Result<(), String> {
    let db = state
        .db
        .lock()
        .map_err(|_| "database lock poisoned".to_string())?;
    db.delete_folder(&id).map_err(String::from)
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
pub fn get_quick_suggestions(state: State<'_, AppState>) -> Result<Vec<QuickSuggestion>, String> {
    let db = state
        .db
        .lock()
        .map_err(|_| "database lock poisoned".to_string())?;
    db.get_quick_suggestions().map_err(String::from)
}

#[tauri::command]
pub fn get_app_settings(state: State<'_, AppState>) -> Result<AppSettings, String> {
    let mut settings = state
        .settings
        .lock()
        .map_err(|_| "settings lock poisoned".to_string())
        .map(|settings| settings.clone())?;
    settings.resolved_data_directory = crate::current_data_dir_string(&state)?;
    Ok(settings)
}

#[tauri::command]
pub fn save_app_settings(
    mut settings: AppSettings,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<AppSettings, String> {
    settings.app_enabled = true;
    settings.resolved_data_directory.clear();
    let saved = {
        let db = state
            .db
            .lock()
            .map_err(|_| "database lock poisoned".to_string())?;
        db.save_app_settings(settings).map_err(String::from)?
    };

    platform::set_run_at_startup(&app, saved.run_at_startup)?;
    platform::set_hide_console_window(saved.hide_console_window)?;
    sync_tray_menu(&app, saved.intercept_win_v)?;
    if let Ok(mut cached) = state.settings.lock() {
        let mut cached_saved = saved.clone();
        cached_saved.resolved_data_directory = crate::current_data_dir_string(&state)?;
        *cached = cached_saved;
    }
    let mut response = saved;
    response.resolved_data_directory = crate::current_data_dir_string(&state)?;
    Ok(response)
}

#[tauri::command]
pub fn change_data_directory(
    mut settings: AppSettings,
    state: State<'_, AppState>,
    app: AppHandle,
) -> Result<DataDirectoryChangeResult, String> {
    settings.app_enabled = true;
    settings.data_directory = settings.data_directory.trim().to_string();
    settings.resolved_data_directory.clear();
    validate_data_dir_change(
        &app,
        if settings.data_directory.is_empty() {
            None
        } else {
            Some(settings.data_directory.as_str())
        },
    )?;

    let saved = {
        let db = state
            .db
            .lock()
            .map_err(|_| "database lock poisoned".to_string())?;
        db.save_app_settings(settings).map_err(String::from)?
    };

    schedule_data_dir_change(
        &app,
        if saved.data_directory.trim().is_empty() {
            None
        } else {
            Some(saved.data_directory.as_str())
        },
    )?;

    if let Ok(mut cached) = state.settings.lock() {
        *cached = saved.clone();
    }

    let mut response_settings = saved;
    response_settings.resolved_data_directory = crate::current_data_dir_string(&state)?;
    Ok(DataDirectoryChangeResult {
        settings: response_settings,
        message: "Data directory updated. Smart Clipboard is restarting to migrate existing data."
            .to_string(),
        restart_required: true,
    })
}

#[tauri::command]
pub fn restart_application(app: AppHandle) -> Result<(), String> {
    crate::restart_app(&app, "manual-restart")
}

#[tauri::command]
pub async fn test_ai_connection(settings: AppSettings) -> Result<String, String> {
    ai::test_connection(&settings.normalized()).await
}

#[tauri::command]
pub async fn list_ai_models(settings: AppSettings) -> Result<Vec<String>, String> {
    match ai::list_models(&settings.normalized()).await {
        Ok(models) => Ok(models),
        Err(error) if error.contains("API key is empty") => Ok(ai::known_models()),
        Err(error) => Err(error),
    }
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
pub fn accept_quick_suggestion(
    id: String,
    ttl: Option<i64>,
    state: State<'_, AppState>,
) -> Result<QuickItem, String> {
    let db = state
        .db
        .lock()
        .map_err(|_| "database lock poisoned".to_string())?;
    db.accept_quick_suggestion(&id, ttl.unwrap_or(24 * 60 * 60))
        .map_err(String::from)
}

#[tauri::command]
pub fn dismiss_quick_suggestion(id: String, state: State<'_, AppState>) -> Result<(), String> {
    let db = state
        .db
        .lock()
        .map_err(|_| "database lock poisoned".to_string())?;
    db.dismiss_quick_suggestion(&id).map_err(String::from)
}

#[tauri::command]
pub fn delete_quick_item(id: String, state: State<'_, AppState>) -> Result<(), String> {
    let db = state
        .db
        .lock()
        .map_err(|_| "database lock poisoned".to_string())?;
    db.delete_quick_item(&id).map_err(String::from)
}

#[tauri::command]
pub fn star_quick_item(id: String, state: State<'_, AppState>) -> Result<ClipboardItem, String> {
    let db = state
        .db
        .lock()
        .map_err(|_| "database lock poisoned".to_string())?;
    db.star_quick_item(&id).map_err(String::from)
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
pub fn search_local_light(
    keyword: String,
    state: State<'_, AppState>,
) -> Result<Vec<ClipboardItem>, String> {
    let db = state
        .db
        .lock()
        .map_err(|_| "database lock poisoned".to_string())?;
    db.search_local_light(&keyword).map_err(String::from)
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
        db.get_history(0, 0).map_err(String::from)?
    };
    let settings = state
        .settings
        .lock()
        .map_err(|_| "settings lock poisoned".to_string())?
        .clone();
    let mut matched = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for batch in ai_record_batches(split_long_search_records(records), 50) {
        let batch_ids: std::collections::HashSet<_> = batch.iter().map(|item| item.id.clone()).collect();
        for id in ai::semantic_search(&settings, &query, batch).await? {
            if batch_ids.contains(&id) && seen.insert(id.clone()) {
                matched.push(id);
            }
        }
    }
    Ok(matched)
}

#[tauri::command]
pub async fn trigger_ai_categorize(
    state: State<'_, AppState>,
) -> Result<Vec<ClipboardItem>, String> {
    let (records, folders) = {
        let db = state
            .db
            .lock()
            .map_err(|_| "database lock poisoned".to_string())?;
        (
            db.recent_uncategorized(300).map_err(String::from)?,
            db.get_folders().map_err(String::from)?,
        )
    };
    let settings = state
        .settings
        .lock()
        .map_err(|_| "settings lock poisoned".to_string())?
        .clone();
    let mut known_folders = folders;
    let mut assignments = Vec::new();
    for batch in ai_record_batches(records, 25) {
        let batch_ids: std::collections::HashSet<_> = batch.iter().map(|item| item.id.clone()).collect();
        for assignment in ai::categorize(&settings, batch, known_folders.clone()).await? {
            if !batch_ids.contains(&assignment.item_id) {
                continue;
            }
            if !known_folders.iter().any(|folder| folder.name == assignment.folder_name) {
                known_folders.push(crate::models::Folder {
                    id: assignment.folder_name.clone(),
                    name: assignment.folder_name.clone(),
                    created_at: 0,
                });
            }
            assignments.push(assignment);
        }
    }
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

const AI_BATCH_MAX_CHARS: usize = 50_000;

fn ai_record_batches(records: Vec<ClipboardItem>, max_records: usize) -> Vec<Vec<ClipboardItem>> {
    let mut batches = Vec::new();
    let mut batch = Vec::new();
    let mut batch_chars = 0;
    for item in records {
        let chars = item.content.as_deref().or(item.ocr_text.as_deref())
            .unwrap_or(&item.preview).chars().count();
        if !batch.is_empty() && (batch.len() >= max_records || batch_chars + chars > AI_BATCH_MAX_CHARS) {
            batches.push(std::mem::take(&mut batch));
            batch_chars = 0;
        }
        batch_chars += chars;
        batch.push(item);
    }
    if !batch.is_empty() {
        batches.push(batch);
    }
    batches
}

fn split_long_search_records(records: Vec<ClipboardItem>) -> Vec<ClipboardItem> {
    const CHUNK_CHARS: usize = AI_BATCH_MAX_CHARS - 1_000;
    const OVERLAP_CHARS: usize = 200;
    let mut parts = Vec::new();
    for item in records {
        let text = item.content.as_deref().or(item.ocr_text.as_deref())
            .unwrap_or(&item.preview);
        let chars: Vec<char> = text.chars().collect();
        if chars.len() <= AI_BATCH_MAX_CHARS {
            parts.push(item);
            continue;
        }
        let mut start = 0;
        while start < chars.len() {
            let end = (start + CHUNK_CHARS).min(chars.len());
            let mut part = item.clone();
            part.content = Some(chars[start..end].iter().collect());
            parts.push(part);
            if end == chars.len() { break; }
            start = end - OVERLAP_CHARS;
        }
    }
    parts
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
    let image_path = item
        .image_path
        .as_deref()
        .ok_or_else(|| "image record has no local image path".to_string())?
        .to_owned();
    let text = tauri::async_runtime::spawn_blocking(move || {
        ocr::recognize_image(std::path::Path::new(&image_path))
    })
    .await
    .map_err(|error| format!("OCR task failed: {error}"))??;
    let db = state
        .db
        .lock()
        .map_err(|_| "database lock poisoned".to_string())?;
    db.update_image_ocr_text(&image_id, &text)
        .map_err(String::from)
}

#[cfg(test)]
mod ai_batch_tests {
    use super::{ai_record_batches, split_long_search_records, ClipboardItem, AI_BATCH_MAX_CHARS};

    fn item(id: usize, content: String) -> ClipboardItem {
        ClipboardItem {
            id: id.to_string(), kind: "text".to_string(), content: Some(content),
            image_path: None, preview: String::new(), is_star: false, folder_id: None,
            created_at: 0, updated_at: 0, expires_at: None, mime_type: None,
            width: None, height: None, image_hash: None, ocr_text: None,
        }
    }

    #[test]
    fn all_records_are_kept_across_search_and_archive_batches() {
        let records = (0..300).map(|id| item(id, "some text".to_string())).collect::<Vec<_>>();
        let search = ai_record_batches(records.clone(), 50);
        let archive = ai_record_batches(records, 25);
        assert_eq!(search.iter().map(Vec::len).sum::<usize>(), 300);
        assert_eq!(archive.iter().map(Vec::len).sum::<usize>(), 300);
        assert!(search.iter().all(|batch| batch.len() <= 50));
        assert!(archive.iter().all(|batch| batch.len() <= 25));
    }

    #[test]
    fn long_search_records_keep_their_full_text_across_chunks() {
        let text = format!("{}needle{}", "中".repeat(60_000), "文".repeat(60_000));
        let parts = split_long_search_records(vec![item(1, text.clone())]);
        assert!(parts.len() > 1);
        assert!(parts.iter().all(|part| part.content.as_ref().unwrap().chars().count() <= AI_BATCH_MAX_CHARS));
        assert!(parts.iter().any(|part| part.content.as_ref().unwrap().contains("needle")));
        let mut rebuilt = String::new();
        for (index, part) in parts.iter().enumerate() {
            rebuilt.extend(part.content.as_ref().unwrap().chars().skip(if index == 0 { 0 } else { 200 }));
        }
        assert_eq!(rebuilt, text);
    }
}
