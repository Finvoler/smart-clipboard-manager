use std::sync::Mutex;

use tauri::{AppHandle, Emitter};

use crate::{db::Database, models::ClipboardItem};

pub(super) fn record_clipboard(app: &AppHandle, db: &Mutex<Database>) {
    let Ok(mut clipboard) = arboard::Clipboard::new() else {
        return;
    };

    if let Ok(text) = clipboard.get_text() {
        if text.trim().is_empty() {
            return;
        }
        if let Ok(mut db) = db.lock() {
            if let Ok((item, quick_suggestions)) = db.insert_text_item(&text) {
                emit_new_item(app, item);
                for quick_suggestion in quick_suggestions {
                    let _ = app.emit("on_quick_suggestion_detected", quick_suggestion);
                }
            }
        }
        return;
    }

    let image = clipboard.get_image();
    #[cfg(target_os = "macos")]
    let image = image.or_else(|error| super::fallback::read_png().ok_or(error));
    if let Ok(image) = image {
        if let Ok(mut db) = db.lock() {
            if let Ok(item) = db.insert_image_item(image.width, image.height, image.bytes.as_ref())
            {
                emit_new_item(app, item);
            }
        }
    }
}

fn emit_new_item(app: &AppHandle, mut item: ClipboardItem) {
    // Match get_history_light: large text is fetched only for editing/expanding.
    item.content = None;
    let _ = app.emit("on_new_item", item);
}

