use std::{fs, path::PathBuf};

use image::ImageBuffer;
use rusqlite::{params, Connection, OptionalExtension, Row};
use uuid::Uuid;

use crate::{models::{ClipboardItem, Folder, QuickItem}, quick_pool::extract_candidates};

const DAY_SECONDS: i64 = 24 * 60 * 60;
const RETENTION_SECONDS: i64 = 30 * DAY_SECONDS;
const QUICK_POOL_SECONDS: i64 = DAY_SECONDS;
const QUICK_THRESHOLD: i64 = 5;

pub struct Database {
  conn: Connection,
  image_dir: PathBuf,
}

impl Database {
  pub fn open(path: PathBuf, image_dir: PathBuf) -> Result<Self, AppError> {
    if let Some(parent) = path.parent() {
      fs::create_dir_all(parent)?;
    }
    fs::create_dir_all(&image_dir)?;
    let conn = Connection::open(path)?;
    let database = Self { conn, image_dir };
    database.init()?;
    Ok(database)
  }

  fn init(&self) -> Result<(), AppError> {
    self.conn.execute_batch(
      "
      PRAGMA journal_mode = WAL;
      PRAGMA foreign_keys = ON;

      CREATE TABLE IF NOT EXISTS folders (
        id TEXT PRIMARY KEY,
        name TEXT NOT NULL UNIQUE,
        created_at INTEGER NOT NULL
      );

      CREATE TABLE IF NOT EXISTS items (
        id TEXT PRIMARY KEY,
        kind TEXT NOT NULL CHECK(kind IN ('text', 'image')),
        content TEXT,
        image_path TEXT,
        preview TEXT NOT NULL,
        is_star INTEGER NOT NULL DEFAULT 0,
        folder_id TEXT REFERENCES folders(id) ON DELETE SET NULL,
        created_at INTEGER NOT NULL,
        updated_at INTEGER NOT NULL,
        expires_at INTEGER,
        mime_type TEXT,
        width INTEGER,
        height INTEGER
      );

      CREATE INDEX IF NOT EXISTS idx_items_created_at ON items(created_at DESC);
      CREATE INDEX IF NOT EXISTS idx_items_folder ON items(folder_id);
      CREATE INDEX IF NOT EXISTS idx_items_star ON items(is_star);

      CREATE TABLE IF NOT EXISTS quick_phrase_hits (
        phrase TEXT PRIMARY KEY,
        first_seen INTEGER NOT NULL,
        last_seen INTEGER NOT NULL,
        hit_count INTEGER NOT NULL
      );

      CREATE TABLE IF NOT EXISTS quick_items (
        id TEXT PRIMARY KEY,
        content TEXT NOT NULL UNIQUE,
        hit_count INTEGER NOT NULL,
        created_at INTEGER NOT NULL,
        updated_at INTEGER NOT NULL,
        expires_at INTEGER,
        is_pinned INTEGER NOT NULL DEFAULT 0
      );
      "
    )?;
    Ok(())
  }

  pub fn insert_text_item(&mut self, text: &str) -> Result<(ClipboardItem, Vec<QuickItem>), AppError> {
    let now = now_ts();
    let id = Uuid::new_v4().to_string();
    let preview = make_preview(text);
    self.conn.execute(
      "INSERT INTO items (id, kind, content, preview, created_at, updated_at, expires_at, mime_type) VALUES (?1, 'text', ?2, ?3, ?4, ?4, ?5, 'text/plain;charset=utf-8')",
      params![id, text, preview, now, now + RETENTION_SECONDS],
    )?;
    let item = self.get_item(&id)?.ok_or(AppError::NotFound)?;
    let quick_items = self.observe_text_for_quick_pool(text)?;
    Ok((item, quick_items))
  }

  pub fn insert_text_item_from_ocr(&mut self, text: &str) -> Result<ClipboardItem, AppError> {
    let (item, _) = self.insert_text_item(text)?;
    Ok(item)
  }

  pub fn insert_image_item(&mut self, width: usize, height: usize, bytes: &[u8]) -> Result<ClipboardItem, AppError> {
    let now = now_ts();
    let id = Uuid::new_v4().to_string();
    let image_path = self.image_dir.join(format!("{id}.png"));
    let buffer = ImageBuffer::<image::Rgba<u8>, _>::from_raw(width as u32, height as u32, bytes.to_vec())
      .ok_or_else(|| AppError::Other("invalid RGBA clipboard image buffer".to_string()))?;
    buffer.save(&image_path)?;

    self.conn.execute(
      "INSERT INTO items (id, kind, image_path, preview, created_at, updated_at, expires_at, mime_type, width, height) VALUES (?1, 'image', ?2, ?3, ?4, ?4, ?5, 'image/png', ?6, ?7)",
      params![id, image_path.to_string_lossy(), format!("Image {} x {}", width, height), now, now + RETENTION_SECONDS, width as i64, height as i64],
    )?;
    self.get_item(&id)?.ok_or(AppError::NotFound)
  }

  pub fn get_history(&self, limit: i64, offset: i64) -> Result<Vec<ClipboardItem>, AppError> {
    let mut statement = self.conn.prepare(
      "SELECT id, kind, content, image_path, preview, is_star, folder_id, created_at, updated_at, expires_at, mime_type, width, height
       FROM items ORDER BY created_at DESC LIMIT ?1 OFFSET ?2",
    )?;
    let rows = statement.query_map(params![limit, offset], row_to_item)?;
    collect_rows(rows)
  }

  pub fn get_item(&self, id: &str) -> Result<Option<ClipboardItem>, AppError> {
    self.conn.query_row(
      "SELECT id, kind, content, image_path, preview, is_star, folder_id, created_at, updated_at, expires_at, mime_type, width, height FROM items WHERE id = ?1",
      params![id],
      row_to_item,
    ).optional().map_err(AppError::from)
  }

  pub fn update_item_text(&self, id: &str, text: &str) -> Result<ClipboardItem, AppError> {
    let now = now_ts();
    let changed = self.conn.execute(
      "UPDATE items SET kind = 'text', content = ?2, image_path = NULL, preview = ?3, updated_at = ?4, mime_type = 'text/plain;charset=utf-8', width = NULL, height = NULL WHERE id = ?1",
      params![id, text, make_preview(text), now],
    )?;
    if changed == 0 {
      return Err(AppError::NotFound);
    }
    self.get_item(id)?.ok_or(AppError::NotFound)
  }

  pub fn delete_item(&self, id: &str) -> Result<(), AppError> {
    if let Some(item) = self.get_item(id)? {
      if let Some(path) = item.image_path {
        let _ = fs::remove_file(path);
      }
    }
    self.conn.execute("DELETE FROM items WHERE id = ?1", params![id])?;
    Ok(())
  }

  pub fn toggle_star(&self, id: &str, is_star: bool) -> Result<ClipboardItem, AppError> {
    let item = self.get_item(id)?.ok_or(AppError::NotFound)?;
    let expires_at = if is_star || item.folder_id.is_some() { None } else { Some(now_ts() + RETENTION_SECONDS) };
    self.conn.execute(
      "UPDATE items SET is_star = ?2, expires_at = ?3, updated_at = ?4 WHERE id = ?1",
      params![id, bool_to_int(is_star), expires_at, now_ts()],
    )?;
    self.get_item(id)?.ok_or(AppError::NotFound)
  }

  pub fn get_folders(&self) -> Result<Vec<Folder>, AppError> {
    let mut statement = self.conn.prepare("SELECT id, name, created_at FROM folders ORDER BY name ASC")?;
    let rows = statement.query_map([], |row| Ok(Folder { id: row.get(0)?, name: row.get(1)?, created_at: row.get(2)? }))?;
    collect_rows(rows)
  }

  pub fn create_folder(&self, name: &str) -> Result<Folder, AppError> {
    let id = Uuid::new_v4().to_string();
    let now = now_ts();
    self.conn.execute("INSERT INTO folders (id, name, created_at) VALUES (?1, ?2, ?3)", params![id, name, now])?;
    Ok(Folder { id, name: name.to_string(), created_at: now })
  }

  pub fn move_to_folder(&self, item_id: &str, folder_id: Option<String>) -> Result<ClipboardItem, AppError> {
    let item = self.get_item(item_id)?.ok_or(AppError::NotFound)?;
    let expires_at = if folder_id.is_some() || item.is_star { None } else { Some(now_ts() + RETENTION_SECONDS) };
    self.conn.execute(
      "UPDATE items SET folder_id = ?2, expires_at = ?3, updated_at = ?4 WHERE id = ?1",
      params![item_id, folder_id, expires_at, now_ts()],
    )?;
    self.get_item(item_id)?.ok_or(AppError::NotFound)
  }

  pub fn get_quick_pool(&self) -> Result<Vec<QuickItem>, AppError> {
    self.cleanup_quick_pool()?;
    let mut statement = self.conn.prepare(
      "SELECT id, content, hit_count, created_at, updated_at, expires_at, is_pinned FROM quick_items ORDER BY is_pinned DESC, updated_at DESC",
    )?;
    let rows = statement.query_map([], row_to_quick_item)?;
    collect_rows(rows)
  }

  pub fn update_quick_item(&self, id: &str, content: &str, ttl: i64) -> Result<QuickItem, AppError> {
    let now = now_ts();
    let (expires_at, is_pinned) = if ttl <= 0 { (None, true) } else { (Some(now + ttl), false) };
    let changed = self.conn.execute(
      "UPDATE quick_items SET content = ?2, expires_at = ?3, is_pinned = ?4, updated_at = ?5 WHERE id = ?1",
      params![id, content, expires_at, bool_to_int(is_pinned), now],
    )?;
    if changed == 0 {
      return Err(AppError::NotFound);
    }
    self.conn.query_row(
      "SELECT id, content, hit_count, created_at, updated_at, expires_at, is_pinned FROM quick_items WHERE id = ?1",
      params![id],
      row_to_quick_item,
    ).map_err(AppError::from)
  }

  pub fn search_local(&self, keyword: &str) -> Result<Vec<ClipboardItem>, AppError> {
    let like = format!("%{}%", keyword.trim());
    let mut statement = self.conn.prepare(
      "SELECT id, kind, content, image_path, preview, is_star, folder_id, created_at, updated_at, expires_at, mime_type, width, height
       FROM items WHERE preview LIKE ?1 OR content LIKE ?1 ORDER BY created_at DESC LIMIT 300",
    )?;
    let rows = statement.query_map(params![like], row_to_item)?;
    collect_rows(rows)
  }

  pub fn recent_uncategorized(&self, limit: i64) -> Result<Vec<ClipboardItem>, AppError> {
    let mut statement = self.conn.prepare(
      "SELECT id, kind, content, image_path, preview, is_star, folder_id, created_at, updated_at, expires_at, mime_type, width, height
       FROM items WHERE folder_id IS NULL ORDER BY created_at DESC LIMIT ?1",
    )?;
    let rows = statement.query_map(params![limit], row_to_item)?;
    collect_rows(rows)
  }

  pub fn cleanup_retention(&mut self) -> Result<(), AppError> {
    let now = now_ts();
    let expired = self.expired_image_paths(now)?;
    self.conn.execute(
      "DELETE FROM items WHERE is_star = 0 AND folder_id IS NULL AND (created_at < ?1 OR (expires_at IS NOT NULL AND expires_at <= ?2))",
      params![now - RETENTION_SECONDS, now],
    )?;
    for path in expired {
      let _ = fs::remove_file(path);
    }
    self.cleanup_quick_pool()?;
    self.conn.execute("DELETE FROM quick_phrase_hits WHERE last_seen < ?1", params![now - DAY_SECONDS])?;
    Ok(())
  }

  fn cleanup_quick_pool(&self) -> Result<(), AppError> {
    self.conn.execute("DELETE FROM quick_items WHERE is_pinned = 0 AND expires_at IS NOT NULL AND expires_at <= ?1", params![now_ts()])?;
    Ok(())
  }

  fn observe_text_for_quick_pool(&self, text: &str) -> Result<Vec<QuickItem>, AppError> {
    let now = now_ts();
    let mut extracted = Vec::new();
    for phrase in extract_candidates(text) {
      let existing = self.conn.query_row(
        "SELECT first_seen, hit_count FROM quick_phrase_hits WHERE phrase = ?1",
        params![phrase],
        |row| Ok((row.get::<_, i64>(0)?, row.get::<_, i64>(1)?)),
      ).optional()?;

      let hit_count = match existing {
        Some((first_seen, count)) if now - first_seen <= DAY_SECONDS => count + 1,
        Some(_) => 1,
        None => 1,
      };
      let first_seen = match existing {
        Some((first_seen, _)) if now - first_seen <= DAY_SECONDS => first_seen,
        _ => now,
      };

      self.conn.execute(
        "INSERT INTO quick_phrase_hits (phrase, first_seen, last_seen, hit_count) VALUES (?1, ?2, ?3, ?4)
         ON CONFLICT(phrase) DO UPDATE SET first_seen = excluded.first_seen, last_seen = excluded.last_seen, hit_count = excluded.hit_count",
        params![phrase, first_seen, now, hit_count],
      )?;

      if hit_count >= QUICK_THRESHOLD && !self.quick_item_exists(&phrase)? {
        let item = self.insert_quick_item(&phrase, hit_count)?;
        extracted.push(item);
      }
    }
    Ok(extracted)
  }

  fn quick_item_exists(&self, content: &str) -> Result<bool, AppError> {
    let exists: Option<i64> = self.conn.query_row("SELECT 1 FROM quick_items WHERE content = ?1", params![content], |row| row.get(0)).optional()?;
    Ok(exists.is_some())
  }

  fn insert_quick_item(&self, content: &str, hit_count: i64) -> Result<QuickItem, AppError> {
    let id = Uuid::new_v4().to_string();
    let now = now_ts();
    self.conn.execute(
      "INSERT INTO quick_items (id, content, hit_count, created_at, updated_at, expires_at, is_pinned) VALUES (?1, ?2, ?3, ?4, ?4, ?5, 0)",
      params![id, content, hit_count, now, now + QUICK_POOL_SECONDS],
    )?;
    Ok(QuickItem { id, content: content.to_string(), hit_count, created_at: now, updated_at: now, expires_at: Some(now + QUICK_POOL_SECONDS), is_pinned: false })
  }

  fn expired_image_paths(&self, now: i64) -> Result<Vec<String>, AppError> {
    let mut statement = self.conn.prepare(
      "SELECT image_path FROM items WHERE kind = 'image' AND image_path IS NOT NULL AND is_star = 0 AND folder_id IS NULL AND (created_at < ?1 OR (expires_at IS NOT NULL AND expires_at <= ?2))",
    )?;
    let rows = statement.query_map(params![now - RETENTION_SECONDS, now], |row| row.get::<_, String>(0))?;
    collect_rows(rows)
  }
}

fn row_to_item(row: &Row<'_>) -> rusqlite::Result<ClipboardItem> {
  Ok(ClipboardItem {
    id: row.get(0)?,
    kind: row.get(1)?,
    content: row.get(2)?,
    image_path: row.get(3)?,
    preview: row.get(4)?,
    is_star: int_to_bool(row.get::<_, i64>(5)?),
    folder_id: row.get(6)?,
    created_at: row.get(7)?,
    updated_at: row.get(8)?,
    expires_at: row.get(9)?,
    mime_type: row.get(10)?,
    width: row.get(11)?,
    height: row.get(12)?,
  })
}

fn row_to_quick_item(row: &Row<'_>) -> rusqlite::Result<QuickItem> {
  Ok(QuickItem {
    id: row.get(0)?,
    content: row.get(1)?,
    hit_count: row.get(2)?,
    created_at: row.get(3)?,
    updated_at: row.get(4)?,
    expires_at: row.get(5)?,
    is_pinned: int_to_bool(row.get::<_, i64>(6)?),
  })
}

fn collect_rows<T>(rows: rusqlite::MappedRows<'_, impl FnMut(&Row<'_>) -> rusqlite::Result<T>>) -> Result<Vec<T>, AppError> {
  let mut values = Vec::new();
  for row in rows {
    values.push(row?);
  }
  Ok(values)
}

fn make_preview(text: &str) -> String {
  let normalized = text.split_whitespace().collect::<Vec<_>>().join(" ");
  let mut preview: String = normalized.chars().take(180).collect();
  if normalized.chars().count() > 180 {
    preview.push_str("...");
  }
  if preview.is_empty() { "Empty text".to_string() } else { preview }
}

fn bool_to_int(value: bool) -> i64 { if value { 1 } else { 0 } }
fn int_to_bool(value: i64) -> bool { value != 0 }

pub fn now_ts() -> i64 {
  std::time::SystemTime::now()
    .duration_since(std::time::UNIX_EPOCH)
    .map(|duration| duration.as_secs() as i64)
    .unwrap_or_default()
}

#[derive(Debug, thiserror::Error)]
pub enum AppError {
  #[error("database error: {0}")]
  Sql(#[from] rusqlite::Error),
  #[error("io error: {0}")]
  Io(#[from] std::io::Error),
  #[error("image error: {0}")]
  Image(#[from] image::ImageError),
  #[error("record not found")]
  NotFound,
  #[error("{0}")]
  Other(String),
}

impl From<AppError> for String {
  fn from(value: AppError) -> Self {
    value.to_string()
  }
}

#[cfg(test)]
mod tests {
  use super::Database;

  #[test]
  fn quick_pool_extracts_after_five_repeated_copies() {
    let temp = std::env::temp_dir().join(format!("smart-clipboard-test-{}", uuid::Uuid::new_v4()));
    let db_path = temp.join("test.sqlite");
    let image_dir = temp.join("images");
    let mut db = Database::open(db_path, image_dir).unwrap();

    let phrase = "reusable phrase longer than ten";
    let mut extracted = Vec::new();
    for index in 0..5 {
      let (_, quick_items) = db.insert_text_item(&format!("prefix {index} {phrase} suffix {index}")).unwrap();
      extracted.extend(quick_items);
    }

    assert!(extracted.iter().any(|item| item.content.contains(phrase)));
    let _ = std::fs::remove_dir_all(temp);
  }
}
