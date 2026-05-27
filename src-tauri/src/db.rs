//! SQLite 数据层。
//!
//! 这个文件是项目的持久化核心：历史记录、文件夹、临时池、待确认候选、
//! 应用设置、图片元数据和 OCR 文本都在这里落库。

use std::{fs, path::PathBuf};

use image::ImageBuffer;
use rusqlite::{params, Connection, OptionalExtension, Row};
use uuid::Uuid;

use crate::{
    models::{
        AppSettings, ClipboardItem, Folder, QuickItem, QuickSuggestion,
        DEFAULT_ANTHROPIC_BASE_URL, DEFAULT_OPENAI_BASE_URL, LEGACY_ANTHROPIC_BASE_URL,
        LEGACY_OPENAI_BASE_URL,
    },
    quick_pool::extract_candidates,
};

const DAY_SECONDS: i64 = 24 * 60 * 60;
const RETENTION_SECONDS: i64 = 30 * DAY_SECONDS;
const QUICK_SUGGESTION_SECONDS: i64 = 5 * 60 * 60;
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
        // 表结构集中初始化，避免把 schema 分散到命令层或业务逻辑里。
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
                height INTEGER,
                image_hash TEXT,
                ocr_text TEXT
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

      CREATE TABLE IF NOT EXISTS quick_suggestions (
        id TEXT PRIMARY KEY,
        content TEXT NOT NULL UNIQUE,
        hit_count INTEGER NOT NULL,
        created_at INTEGER NOT NULL,
        updated_at INTEGER NOT NULL
      );

      CREATE TABLE IF NOT EXISTS app_settings (
        id INTEGER PRIMARY KEY CHECK(id = 1),
        app_enabled INTEGER NOT NULL,
        capture_enabled INTEGER NOT NULL,
        intercept_win_v INTEGER NOT NULL,
        run_at_startup INTEGER NOT NULL,
        hide_console_window INTEGER NOT NULL,
                data_directory TEXT NOT NULL DEFAULT '',
        ai_protocol TEXT NOT NULL,
        openai_base_url TEXT NOT NULL,
        anthropic_base_url TEXT NOT NULL,
        api_key TEXT NOT NULL,
        search_model TEXT NOT NULL,
        ocr_model TEXT NOT NULL,
        language TEXT NOT NULL DEFAULT 'zh',
        updated_at INTEGER NOT NULL
      );
      ",
        )?;
        self.ensure_column("items", "image_hash", "TEXT")?;
        self.ensure_column("items", "ocr_text", "TEXT")?;
        self.ensure_column("app_settings", "language", "TEXT NOT NULL DEFAULT 'zh'")?;
        self.ensure_column("app_settings", "data_directory", "TEXT NOT NULL DEFAULT ''")?;
        self.conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_items_image_hash ON items(image_hash)",
            [],
        )?;
        self.ensure_settings_row()?;
        self.migrate_legacy_xiaomimimo_base_urls()?;
        Ok(())
    }

    fn migrate_legacy_xiaomimimo_base_urls(&self) -> Result<(), AppError> {
        self.conn.execute(
            "UPDATE app_settings
             SET openai_base_url = CASE openai_base_url
                    WHEN ?1 THEN ?2
                    ELSE openai_base_url
                 END,
                 anthropic_base_url = CASE anthropic_base_url
                    WHEN ?3 THEN ?4
                    ELSE anthropic_base_url
                 END,
                 updated_at = CASE
                    WHEN openai_base_url = ?1 OR anthropic_base_url = ?3 THEN ?5
                    ELSE updated_at
                 END
             WHERE id = 1",
            params![
                LEGACY_OPENAI_BASE_URL,
                DEFAULT_OPENAI_BASE_URL,
                LEGACY_ANTHROPIC_BASE_URL,
                DEFAULT_ANTHROPIC_BASE_URL,
                now_ts(),
            ],
        )?;
        Ok(())
    }

    fn ensure_column(&self, table: &str, column: &str, definition: &str) -> Result<(), AppError> {
        let mut statement = self.conn.prepare(&format!("PRAGMA table_info({table})"))?;
        let rows = statement.query_map([], |row| row.get::<_, String>(1))?;
        for row in rows {
            if row? == column {
                return Ok(());
            }
        }
        self.conn.execute(
            &format!("ALTER TABLE {table} ADD COLUMN {column} {definition}"),
            [],
        )?;
        Ok(())
    }

    fn ensure_settings_row(&self) -> Result<(), AppError> {
        let defaults = AppSettings::default();
        self.conn.execute(
        "INSERT OR IGNORE INTO app_settings (id, app_enabled, capture_enabled, intercept_win_v, run_at_startup, hide_console_window, data_directory, ai_protocol, openai_base_url, anthropic_base_url, api_key, search_model, ocr_model, language, updated_at)
         VALUES (1, ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)",
      params![
        bool_to_int(defaults.app_enabled),
        bool_to_int(defaults.capture_enabled),
        bool_to_int(defaults.intercept_win_v),
        bool_to_int(defaults.run_at_startup),
        bool_to_int(defaults.hide_console_window),
                defaults.data_directory,
        defaults.ai_protocol,
        defaults.openai_base_url,
        defaults.anthropic_base_url,
        defaults.api_key,
        defaults.search_model,
        defaults.ocr_model,
                defaults.language,
        now_ts(),
      ],
    )?;
        Ok(())
    }

    pub fn get_app_settings(&self) -> Result<AppSettings, AppError> {
        self.conn.query_row(
        "SELECT app_enabled, capture_enabled, intercept_win_v, run_at_startup, hide_console_window, data_directory, ai_protocol, openai_base_url, anthropic_base_url, api_key, search_model, ocr_model, language FROM app_settings WHERE id = 1",
      [],
      |row| Ok(AppSettings {
        app_enabled: int_to_bool(row.get::<_, i64>(0)?),
        capture_enabled: int_to_bool(row.get::<_, i64>(1)?),
        intercept_win_v: int_to_bool(row.get::<_, i64>(2)?),
        run_at_startup: int_to_bool(row.get::<_, i64>(3)?),
        hide_console_window: int_to_bool(row.get::<_, i64>(4)?),
                data_directory: row.get(5)?,
                resolved_data_directory: String::new(),
                ai_protocol: row.get(6)?,
                openai_base_url: row.get(7)?,
                anthropic_base_url: row.get(8)?,
                api_key: row.get(9)?,
                search_model: row.get(10)?,
                ocr_model: row.get(11)?,
                                language: row.get(12)?,
      }),
    ).map(|settings| settings.normalized()).map_err(AppError::from)
    }

    pub fn save_app_settings(&self, settings: AppSettings) -> Result<AppSettings, AppError> {
        let settings = settings.normalized();
        self.conn.execute(
        "UPDATE app_settings SET app_enabled = ?1, capture_enabled = ?2, intercept_win_v = ?3, run_at_startup = ?4, hide_console_window = ?5, data_directory = ?6, ai_protocol = ?7, openai_base_url = ?8, anthropic_base_url = ?9, api_key = ?10, search_model = ?11, ocr_model = ?12, language = ?13, updated_at = ?14 WHERE id = 1",
      params![
        bool_to_int(settings.app_enabled),
        bool_to_int(settings.capture_enabled),
        bool_to_int(settings.intercept_win_v),
        bool_to_int(settings.run_at_startup),
        bool_to_int(settings.hide_console_window),
                settings.data_directory,
        settings.ai_protocol,
        settings.openai_base_url,
        settings.anthropic_base_url,
        settings.api_key,
        settings.search_model,
        settings.ocr_model,
                settings.language,
        now_ts(),
      ],
    )?;
        self.get_app_settings()
    }

    pub fn insert_text_item(
        &mut self,
        text: &str,
    ) -> Result<(ClipboardItem, Vec<QuickSuggestion>), AppError> {
        let now = now_ts();
        let quick_items = self.observe_text_for_quick_pool(text)?;
        if let Some(existing) = self.find_recent_duplicate_text_item(text)? {
            let expires_at = if existing.is_star {
                None
            } else {
                Some(now + RETENTION_SECONDS)
            };
            self.conn.execute(
                "UPDATE items SET created_at = ?2, updated_at = ?2, expires_at = ?3 WHERE id = ?1",
                params![existing.id, now, expires_at],
            )?;
            let item = self.get_item(&existing.id)?.ok_or(AppError::NotFound)?;
            return Ok((item, quick_items));
        }

        let id = Uuid::new_v4().to_string();
        let preview = make_preview(text);
        self.conn.execute(
      "INSERT INTO items (id, kind, content, preview, created_at, updated_at, expires_at, mime_type) VALUES (?1, 'text', ?2, ?3, ?4, ?4, ?5, 'text/plain;charset=utf-8')",
      params![id, text, preview, now, now + RETENTION_SECONDS],
    )?;
        let item = self.get_item(&id)?.ok_or(AppError::NotFound)?;
        Ok((item, quick_items))
    }

    pub fn insert_text_item_from_ocr(&mut self, text: &str) -> Result<ClipboardItem, AppError> {
        let (item, _) = self.insert_text_item(text)?;
        Ok(item)
    }

    pub fn insert_image_item(
        &mut self,
        width: usize,
        height: usize,
        bytes: &[u8],
    ) -> Result<ClipboardItem, AppError> {
        let now = now_ts();
        let image_hash = make_image_hash(width, height, bytes);
        if let Some(existing) = self.find_recent_duplicate_image_item(&image_hash)? {
            let expires_at = if existing.is_star {
                None
            } else {
                Some(now + RETENTION_SECONDS)
            };
            self.conn.execute(
                "UPDATE items SET created_at = ?2, updated_at = ?2, expires_at = ?3 WHERE id = ?1",
                params![existing.id, now, expires_at],
            )?;
            return self.get_item(&existing.id)?.ok_or(AppError::NotFound);
        }

        let id = Uuid::new_v4().to_string();
        let image_path = self.image_dir.join(format!("{id}.png"));
        let buffer = ImageBuffer::<image::Rgba<u8>, _>::from_raw(
            width as u32,
            height as u32,
            bytes.to_vec(),
        )
        .ok_or_else(|| AppError::Other("invalid RGBA clipboard image buffer".to_string()))?;
        buffer.save(&image_path)?;

        self.conn.execute(
            "INSERT INTO items (id, kind, image_path, preview, created_at, updated_at, expires_at, mime_type, width, height, image_hash) VALUES (?1, 'image', ?2, ?3, ?4, ?4, ?5, 'image/png', ?6, ?7, ?8)",
            params![id, image_path.to_string_lossy(), format!("Image {} x {}", width, height), now, now + RETENTION_SECONDS, width as i64, height as i64, image_hash],
    )?;
        self.get_item(&id)?.ok_or(AppError::NotFound)
    }

    pub fn get_history(&self, limit: i64, offset: i64) -> Result<Vec<ClipboardItem>, AppError> {
        let mut statement = self.conn.prepare(
    "SELECT id, kind, content, image_path, preview, is_star, folder_id, created_at, updated_at, expires_at, mime_type, width, height, image_hash, ocr_text
       FROM items ORDER BY created_at DESC LIMIT ?1 OFFSET ?2",
    )?;
        let rows = statement.query_map(params![limit, offset], row_to_item)?;
        collect_rows(rows)
    }

    fn find_recent_duplicate_text_item(
        &self,
        text: &str,
    ) -> Result<Option<ClipboardItem>, AppError> {
        let normalized = normalize_text_for_match(text);
        if normalized.is_empty() {
            return Ok(None);
        }

        let mut statement = self.conn.prepare(
            "SELECT id, kind, content, image_path, preview, is_star, folder_id, created_at, updated_at, expires_at, mime_type, width, height, image_hash, ocr_text
             FROM items WHERE kind = 'text' ORDER BY created_at DESC LIMIT 20",
        )?;
        let rows = statement.query_map([], row_to_item)?;
        for row in rows {
            let item = row?;
            if item
                .content
                .as_ref()
                .map(|content| normalize_text_for_match(content) == normalized)
                .unwrap_or(false)
            {
                return Ok(Some(item));
            }
        }
        Ok(None)
    }

    fn find_recent_duplicate_image_item(
        &self,
        image_hash: &str,
    ) -> Result<Option<ClipboardItem>, AppError> {
        self.conn
            .query_row(
                "SELECT id, kind, content, image_path, preview, is_star, folder_id, created_at, updated_at, expires_at, mime_type, width, height, image_hash, ocr_text
                 FROM items WHERE kind = 'image' AND image_hash = ?1 ORDER BY created_at DESC LIMIT 1",
                params![image_hash],
                row_to_item,
            )
            .optional()
            .map_err(AppError::from)
    }

    pub fn get_item(&self, id: &str) -> Result<Option<ClipboardItem>, AppError> {
        self.conn.query_row(
    "SELECT id, kind, content, image_path, preview, is_star, folder_id, created_at, updated_at, expires_at, mime_type, width, height, image_hash, ocr_text FROM items WHERE id = ?1",
      params![id],
      row_to_item,
    ).optional().map_err(AppError::from)
    }

    pub fn update_item_text(&self, id: &str, text: &str) -> Result<ClipboardItem, AppError> {
        let now = now_ts();
        let changed = self.conn.execute(
            "UPDATE items SET kind = 'text', content = ?2, image_path = NULL, preview = ?3, updated_at = ?4, mime_type = 'text/plain;charset=utf-8', width = NULL, height = NULL, image_hash = NULL, ocr_text = NULL WHERE id = ?1",
      params![id, text, make_preview(text), now],
    )?;
        if changed == 0 {
            return Err(AppError::NotFound);
        }
        self.get_item(id)?.ok_or(AppError::NotFound)
    }

    pub fn update_image_ocr_text(&self, id: &str, text: &str) -> Result<ClipboardItem, AppError> {
        let item = self.get_item(id)?.ok_or(AppError::NotFound)?;
        if item.kind != "image" {
            return Err(AppError::Other("record is not an image".to_string()));
        }
        let changed = self.conn.execute(
            "UPDATE items SET ocr_text = ?2, updated_at = ?3 WHERE id = ?1",
            params![id, text, now_ts()],
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
        self.conn
            .execute("DELETE FROM items WHERE id = ?1", params![id])?;
        Ok(())
    }

    pub fn toggle_star(&self, id: &str, is_star: bool) -> Result<ClipboardItem, AppError> {
        let _item = self.get_item(id)?.ok_or(AppError::NotFound)?;
        let expires_at = if is_star {
            None
        } else {
            Some(now_ts() + RETENTION_SECONDS)
        };
        self.conn.execute(
            "UPDATE items SET is_star = ?2, expires_at = ?3, updated_at = ?4 WHERE id = ?1",
            params![id, bool_to_int(is_star), expires_at, now_ts()],
        )?;
        self.get_item(id)?.ok_or(AppError::NotFound)
    }

    pub fn get_folders(&self) -> Result<Vec<Folder>, AppError> {
        let mut statement = self
            .conn
            .prepare("SELECT id, name, created_at FROM folders ORDER BY name ASC")?;
        let rows = statement.query_map([], |row| {
            Ok(Folder {
                id: row.get(0)?,
                name: row.get(1)?,
                created_at: row.get(2)?,
            })
        })?;
        collect_rows(rows)
    }

    pub fn create_folder(&self, name: &str) -> Result<Folder, AppError> {
        let id = Uuid::new_v4().to_string();
        let now = now_ts();
        self.conn.execute(
            "INSERT INTO folders (id, name, created_at) VALUES (?1, ?2, ?3)",
            params![id, name, now],
        )?;
        Ok(Folder {
            id,
            name: name.to_string(),
            created_at: now,
        })
    }

    pub fn delete_folder(&self, id: &str) -> Result<(), AppError> {
        let now = now_ts();
        self.conn.execute(
            "UPDATE items SET folder_id = NULL, expires_at = CASE WHEN is_star = 1 THEN NULL WHEN expires_at IS NULL THEN ?2 ELSE expires_at END, updated_at = ?3 WHERE folder_id = ?1",
            params![id, now + RETENTION_SECONDS, now],
        )?;
        let changed = self
            .conn
            .execute("DELETE FROM folders WHERE id = ?1", params![id])?;
        if changed == 0 {
            return Err(AppError::NotFound);
        }
        Ok(())
    }

    pub fn get_or_create_folder(&self, name: &str) -> Result<Folder, AppError> {
        let trimmed = name.trim();
        if trimmed.is_empty() {
            return Err(AppError::Other("folder name cannot be empty".to_string()));
        }

        if let Some(folder) = self
            .conn
            .query_row(
                "SELECT id, name, created_at FROM folders WHERE name = ?1",
                params![trimmed],
                |row| {
                    Ok(Folder {
                        id: row.get(0)?,
                        name: row.get(1)?,
                        created_at: row.get(2)?,
                    })
                },
            )
            .optional()?
        {
            return Ok(folder);
        }

        self.create_folder(trimmed)
    }

    pub fn move_to_folder(
        &self,
        item_id: &str,
        folder_id: Option<String>,
    ) -> Result<ClipboardItem, AppError> {
        let item = self.get_item(item_id)?.ok_or(AppError::NotFound)?;
        let expires_at = if item.is_star {
            None
        } else {
            item.expires_at.or(Some(now_ts() + RETENTION_SECONDS))
        };
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

    pub fn get_quick_suggestions(&self) -> Result<Vec<QuickSuggestion>, AppError> {
        self.cleanup_quick_pool()?;
        let mut statement = self.conn.prepare(
            "SELECT id, content, hit_count, created_at, updated_at FROM quick_suggestions ORDER BY updated_at DESC",
        )?;
        let rows = statement.query_map([], row_to_quick_suggestion)?;
        collect_rows(rows)
    }

    pub fn update_quick_item(
        &self,
        id: &str,
        content: &str,
        ttl: i64,
    ) -> Result<QuickItem, AppError> {
        let now = now_ts();
        let (expires_at, is_pinned) = if ttl <= 0 {
            (None, true)
        } else {
            (Some(now + ttl), false)
        };
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

    pub fn accept_quick_suggestion(&self, id: &str, ttl: i64) -> Result<QuickItem, AppError> {
        let suggestion = self.conn.query_row(
            "SELECT id, content, hit_count, created_at, updated_at FROM quick_suggestions WHERE id = ?1",
            params![id],
            row_to_quick_suggestion,
        ).optional()?.ok_or(AppError::NotFound)?;

        let quick_item = if let Some(existing) =
            self.find_quick_item_by_content(&suggestion.content)?
        {
            let now = now_ts();
            let expires_at = if ttl <= 0 { None } else { Some(now + ttl) };
            let is_pinned = ttl <= 0;
            self.conn.execute(
                "UPDATE quick_items SET hit_count = MAX(hit_count, ?2), updated_at = ?3, expires_at = ?4, is_pinned = ?5 WHERE id = ?1",
                params![existing.id, suggestion.hit_count, now, expires_at, bool_to_int(is_pinned)],
            )?;
            self.conn.query_row(
                "SELECT id, content, hit_count, created_at, updated_at, expires_at, is_pinned FROM quick_items WHERE id = ?1",
                params![existing.id],
                row_to_quick_item,
            )?
        } else {
            self.insert_quick_item(&suggestion.content, suggestion.hit_count, ttl)?
        };

        self.conn
            .execute("DELETE FROM quick_suggestions WHERE id = ?1", params![id])?;
        Ok(quick_item)
    }

    pub fn dismiss_quick_suggestion(&self, id: &str) -> Result<(), AppError> {
        let content = self
            .conn
            .query_row(
                "SELECT content FROM quick_suggestions WHERE id = ?1",
                params![id],
                |row| row.get::<_, String>(0),
            )
            .optional()?;
        self.conn
            .execute("DELETE FROM quick_suggestions WHERE id = ?1", params![id])?;
        if let Some(content) = content {
            self.conn.execute(
                "DELETE FROM quick_phrase_hits WHERE phrase = ?1",
                params![content],
            )?;
        }
        Ok(())
    }

    pub fn delete_quick_item(&self, id: &str) -> Result<(), AppError> {
        let changed = self
            .conn
            .execute("DELETE FROM quick_items WHERE id = ?1", params![id])?;
        if changed == 0 {
            return Err(AppError::NotFound);
        }
        Ok(())
    }

    pub fn star_quick_item(&self, id: &str) -> Result<ClipboardItem, AppError> {
        let quick_item = self
            .conn
            .query_row(
                "SELECT id, content, hit_count, created_at, updated_at, expires_at, is_pinned FROM quick_items WHERE id = ?1",
                params![id],
                row_to_quick_item,
            )
            .optional()?
            .ok_or(AppError::NotFound)?;

        let now = now_ts();
        let starred = if let Some(existing) = self.find_text_item_by_content(&quick_item.content)? {
            self.conn.execute(
                "UPDATE items SET is_star = 1, expires_at = NULL, updated_at = ?2 WHERE id = ?1",
                params![existing.id, now],
            )?;
            self.get_item(&existing.id)?.ok_or(AppError::NotFound)?
        } else {
            let item_id = Uuid::new_v4().to_string();
            self.conn.execute(
                "INSERT INTO items (id, kind, content, preview, is_star, created_at, updated_at, expires_at, mime_type) VALUES (?1, 'text', ?2, ?3, 1, ?4, ?4, NULL, 'text/plain;charset=utf-8')",
                params![item_id, quick_item.content, make_preview(&quick_item.content), now],
            )?;
            self.get_item(&item_id)?.ok_or(AppError::NotFound)?
        };

        self.conn
            .execute("DELETE FROM quick_items WHERE id = ?1", params![id])?;
        self.conn.execute(
            "DELETE FROM quick_suggestions WHERE content = ?1",
            params![quick_item.content],
        )?;
        self.conn.execute(
            "DELETE FROM quick_phrase_hits WHERE phrase = ?1",
            params![quick_item.content],
        )?;
        Ok(starred)
    }

    pub fn search_local(&self, keyword: &str) -> Result<Vec<ClipboardItem>, AppError> {
        let like = format!("%{}%", keyword.trim());
        let mut statement = self.conn.prepare(
    "SELECT id, kind, content, image_path, preview, is_star, folder_id, created_at, updated_at, expires_at, mime_type, width, height, image_hash, ocr_text
             FROM items WHERE preview LIKE ?1 OR content LIKE ?1 OR ocr_text LIKE ?1 ORDER BY created_at DESC LIMIT 300",
    )?;
        let rows = statement.query_map(params![like], row_to_item)?;
        collect_rows(rows)
    }

    pub fn recent_uncategorized(&self, limit: i64) -> Result<Vec<ClipboardItem>, AppError> {
        let mut statement = self.conn.prepare(
    "SELECT id, kind, content, image_path, preview, is_star, folder_id, created_at, updated_at, expires_at, mime_type, width, height, image_hash, ocr_text
       FROM items WHERE folder_id IS NULL ORDER BY created_at DESC LIMIT ?1",
    )?;
        let rows = statement.query_map(params![limit], row_to_item)?;
        collect_rows(rows)
    }

    pub fn cleanup_retention(&mut self) -> Result<(), AppError> {
        let now = now_ts();
        let expired = self.expired_image_paths(now)?;
        self.conn.execute(
    "DELETE FROM items WHERE is_star = 0 AND (created_at < ?1 OR (expires_at IS NOT NULL AND expires_at <= ?2))",
      params![now - RETENTION_SECONDS, now],
    )?;
        for path in expired {
            let _ = fs::remove_file(path);
        }
        self.cleanup_quick_pool()?;
        self.conn.execute(
            "DELETE FROM quick_phrase_hits WHERE last_seen < ?1",
            params![now - DAY_SECONDS],
        )?;
        Ok(())
    }

    fn cleanup_quick_pool(&self) -> Result<(), AppError> {
        self.conn.execute("DELETE FROM quick_items WHERE is_pinned = 0 AND expires_at IS NOT NULL AND expires_at <= ?1", params![now_ts()])?;
        self.conn.execute(
            "DELETE FROM quick_suggestions WHERE updated_at <= ?1",
            params![now_ts() - QUICK_SUGGESTION_SECONDS],
        )?;
        Ok(())
    }

    pub fn observe_text_for_quick_pool(
        &self,
        text: &str,
    ) -> Result<Vec<QuickSuggestion>, AppError> {
        let now = now_ts();
        let mut extracted = Vec::new();
        for phrase in extract_candidates(text) {
            let existing = self
                .conn
                .query_row(
                    "SELECT first_seen, hit_count FROM quick_phrase_hits WHERE phrase = ?1",
                    params![phrase],
                    |row| Ok((row.get::<_, i64>(0)?, row.get::<_, i64>(1)?)),
                )
                .optional()?;

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

            if hit_count >= QUICK_THRESHOLD {
                if let Some(existing_item) = self.find_quick_item_by_content(&phrase)? {
                    self.conn.execute(
                        "UPDATE quick_items SET hit_count = MAX(hit_count, ?2), updated_at = ?3 WHERE id = ?1",
                        params![existing_item.id, hit_count, now],
                    )?;
                } else if let Some(existing_suggestion) =
                    self.find_quick_suggestion_by_content(&phrase)?
                {
                    self.conn.execute(
                        "UPDATE quick_suggestions SET hit_count = MAX(hit_count, ?2), updated_at = ?3 WHERE id = ?1",
                        params![existing_suggestion.id, hit_count, now],
                    )?;
                } else {
                    let suggestion = self.insert_quick_suggestion(&phrase, hit_count)?;
                    extracted.push(suggestion);
                }
            }
        }
        Ok(extracted)
    }

    fn find_quick_item_by_content(&self, content: &str) -> Result<Option<QuickItem>, AppError> {
        self.conn.query_row(
            "SELECT id, content, hit_count, created_at, updated_at, expires_at, is_pinned FROM quick_items WHERE content = ?1",
            params![content],
            row_to_quick_item,
        ).optional().map_err(AppError::from)
    }

    fn find_quick_suggestion_by_content(
        &self,
        content: &str,
    ) -> Result<Option<QuickSuggestion>, AppError> {
        self.conn.query_row(
            "SELECT id, content, hit_count, created_at, updated_at FROM quick_suggestions WHERE content = ?1",
            params![content],
            row_to_quick_suggestion,
        ).optional().map_err(AppError::from)
    }

    fn find_text_item_by_content(&self, content: &str) -> Result<Option<ClipboardItem>, AppError> {
        if let Some(item) = self
            .conn
            .query_row(
                "SELECT id, kind, content, image_path, preview, is_star, folder_id, created_at, updated_at, expires_at, mime_type, width, height, image_hash, ocr_text
                 FROM items WHERE kind = 'text' AND content = ?1 ORDER BY created_at DESC LIMIT 1",
                params![content],
                row_to_item,
            )
            .optional()?
        {
            return Ok(Some(item));
        }

        let normalized = normalize_text_for_match(content);
        if normalized.is_empty() {
            return Ok(None);
        }

        let mut statement = self.conn.prepare(
            "SELECT id, kind, content, image_path, preview, is_star, folder_id, created_at, updated_at, expires_at, mime_type, width, height, image_hash, ocr_text
             FROM items WHERE kind = 'text' ORDER BY created_at DESC LIMIT 500",
        )?;
        let rows = statement.query_map([], row_to_item)?;
        for row in rows {
            let item = row?;
            if item
                .content
                .as_ref()
                .map(|value| normalize_text_for_match(value) == normalized)
                .unwrap_or(false)
            {
                return Ok(Some(item));
            }
        }
        Ok(None)
    }

    fn insert_quick_item(
        &self,
        content: &str,
        hit_count: i64,
        ttl: i64,
    ) -> Result<QuickItem, AppError> {
        let id = Uuid::new_v4().to_string();
        let now = now_ts();
        let (expires_at, is_pinned) = if ttl <= 0 {
            (None, true)
        } else {
            (Some(now + ttl), false)
        };
        self.conn.execute(
            "INSERT INTO quick_items (id, content, hit_count, created_at, updated_at, expires_at, is_pinned) VALUES (?1, ?2, ?3, ?4, ?4, ?5, ?6)",
            params![id, content, hit_count, now, expires_at, bool_to_int(is_pinned)],
    )?;
        Ok(QuickItem {
            id,
            content: content.to_string(),
            hit_count,
            created_at: now,
            updated_at: now,
            expires_at,
            is_pinned,
        })
    }

    fn insert_quick_suggestion(
        &self,
        content: &str,
        hit_count: i64,
    ) -> Result<QuickSuggestion, AppError> {
        let id = Uuid::new_v4().to_string();
        let now = now_ts();
        self.conn.execute(
            "INSERT INTO quick_suggestions (id, content, hit_count, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?4)",
            params![id, content, hit_count, now],
        )?;
        Ok(QuickSuggestion {
            id,
            content: content.to_string(),
            hit_count,
            created_at: now,
            updated_at: now,
        })
    }

    fn expired_image_paths(&self, now: i64) -> Result<Vec<String>, AppError> {
        let mut statement = self.conn.prepare(
    "SELECT image_path FROM items WHERE kind = 'image' AND image_path IS NOT NULL AND is_star = 0 AND (created_at < ?1 OR (expires_at IS NOT NULL AND expires_at <= ?2))",
    )?;
        let rows = statement.query_map(params![now - RETENTION_SECONDS, now], |row| {
            row.get::<_, String>(0)
        })?;
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
        image_hash: row.get(13)?,
        ocr_text: row.get(14)?,
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

fn row_to_quick_suggestion(row: &Row<'_>) -> rusqlite::Result<QuickSuggestion> {
    Ok(QuickSuggestion {
        id: row.get(0)?,
        content: row.get(1)?,
        hit_count: row.get(2)?,
        created_at: row.get(3)?,
        updated_at: row.get(4)?,
    })
}

fn collect_rows<T>(
    rows: rusqlite::MappedRows<'_, impl FnMut(&Row<'_>) -> rusqlite::Result<T>>,
) -> Result<Vec<T>, AppError> {
    let mut values = Vec::new();
    for row in rows {
        values.push(row?);
    }
    Ok(values)
}

fn make_preview(text: &str) -> String {
    let normalized = normalize_text_for_match(text);
    let mut preview: String = normalized.chars().take(180).collect();
    if normalized.chars().count() > 180 {
        preview.push_str("...");
    }
    if preview.is_empty() {
        "Empty text".to_string()
    } else {
        preview
    }
}

fn normalize_text_for_match(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn make_image_hash(width: usize, height: usize, bytes: &[u8]) -> String {
    let mut hash = 0xcbf29ce484222325u64;
    let width_bytes = (width as u64).to_le_bytes();
    let height_bytes = (height as u64).to_le_bytes();
    for byte in width_bytes
        .iter()
        .chain(height_bytes.iter())
        .chain(bytes.iter())
    {
        hash ^= *byte as u64;
        hash = hash.wrapping_mul(0x100000001b3);
    }
    format!("{hash:016x}")
}

fn bool_to_int(value: bool) -> i64 {
    if value {
        1
    } else {
        0
    }
}
fn int_to_bool(value: i64) -> bool {
    value != 0
}

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
    use super::{now_ts, Database};
    use rusqlite::params;

    #[test]
    fn quick_pool_extracts_after_five_exact_repeated_copies() {
        let temp =
            std::env::temp_dir().join(format!("smart-clipboard-test-{}", uuid::Uuid::new_v4()));
        let db_path = temp.join("test.sqlite");
        let image_dir = temp.join("images");
        let mut db = Database::open(db_path, image_dir).unwrap();

        let phrase = "reusable phrase longer than ten";
        let mut extracted = Vec::new();
        for _ in 0..5 {
            let (_, quick_items) = db.insert_text_item(phrase).unwrap();
            extracted.extend(quick_items);
        }

        assert!(extracted.iter().any(|item| item.content == phrase));
        assert_eq!(db.get_history(10, 0).unwrap().len(), 1);
        let _ = std::fs::remove_dir_all(temp);
    }

    #[test]
    fn repeated_text_reuses_recent_history_item() {
        let temp =
            std::env::temp_dir().join(format!("smart-clipboard-test-{}", uuid::Uuid::new_v4()));
        let db_path = temp.join("test.sqlite");
        let image_dir = temp.join("images");
        let mut db = Database::open(db_path, image_dir).unwrap();

        let (first, _) = db.insert_text_item("duplicate clipboard text").unwrap();
        let (second, _) = db.insert_text_item(" duplicate   clipboard text ").unwrap();

        assert_eq!(first.id, second.id);
        assert_eq!(db.get_history(10, 0).unwrap().len(), 1);
        let _ = std::fs::remove_dir_all(temp);
    }

    #[test]
    fn repeated_image_reuses_recent_history_item() {
        let temp =
            std::env::temp_dir().join(format!("smart-clipboard-test-{}", uuid::Uuid::new_v4()));
        let db_path = temp.join("test.sqlite");
        let image_dir = temp.join("images");
        let mut db = Database::open(db_path, image_dir).unwrap();

        let bytes = [255, 0, 0, 255];
        let first = db.insert_image_item(1, 1, &bytes).unwrap();
        let second = db.insert_image_item(1, 1, &bytes).unwrap();

        assert_eq!(first.id, second.id);
        assert_eq!(db.get_history(10, 0).unwrap().len(), 1);
        assert!(second.image_hash.is_some());
        let _ = std::fs::remove_dir_all(temp);
    }

    #[test]
    fn quick_item_star_moves_to_starred_history() {
        let temp =
            std::env::temp_dir().join(format!("smart-clipboard-test-{}", uuid::Uuid::new_v4()));
        let db_path = temp.join("test.sqlite");
        let image_dir = temp.join("images");
        let mut db = Database::open(db_path, image_dir).unwrap();

        let content = "favorite quick text";
        for _ in 0..5 {
            let _ = db.insert_text_item(content).unwrap();
        }
        let suggestion = db.get_quick_suggestions().unwrap().pop().unwrap();
        let quick_item = db
            .accept_quick_suggestion(&suggestion.id, 24 * 60 * 60)
            .unwrap();

        let starred = db.star_quick_item(&quick_item.id).unwrap();

        assert!(starred.is_star);
        assert_eq!(starred.content.as_deref(), Some(content));
        assert!(starred.expires_at.is_none());
        assert!(db.get_quick_pool().unwrap().is_empty());
        assert!(db.get_quick_suggestions().unwrap().is_empty());
        let _ = std::fs::remove_dir_all(temp);
    }

    #[test]
    fn deleting_folder_moves_records_back_to_history() {
        let temp =
            std::env::temp_dir().join(format!("smart-clipboard-test-{}", uuid::Uuid::new_v4()));
        let db_path = temp.join("test.sqlite");
        let image_dir = temp.join("images");
        let mut db = Database::open(db_path, image_dir).unwrap();

        let folder = db.create_folder("Projects").unwrap();
        let (item, _) = db.insert_text_item("foldered clipboard text").unwrap();
        let moved = db
            .move_to_folder(&item.id, Some(folder.id.clone()))
            .unwrap();
        assert_eq!(moved.folder_id.as_deref(), Some(folder.id.as_str()));
        assert!(moved.expires_at.is_some());

        db.delete_folder(&folder.id).unwrap();
        let restored = db.get_item(&item.id).unwrap().unwrap();

        assert!(restored.folder_id.is_none());
        assert!(restored.expires_at.is_some());
        assert!(db.get_folders().unwrap().is_empty());
        let _ = std::fs::remove_dir_all(temp);
    }

    #[test]
    fn foldered_non_starred_records_still_expire() {
        let temp =
            std::env::temp_dir().join(format!("smart-clipboard-test-{}", uuid::Uuid::new_v4()));
        let db_path = temp.join("test.sqlite");
        let image_dir = temp.join("images");
        let mut db = Database::open(db_path, image_dir).unwrap();

        let folder = db.create_folder("Archive").unwrap();
        let (item, _) = db.insert_text_item("old foldered clipboard text").unwrap();
        db.move_to_folder(&item.id, Some(folder.id)).unwrap();
        db.conn
            .execute(
                "UPDATE items SET expires_at = ?2 WHERE id = ?1",
                params![item.id, now_ts() - 1],
            )
            .unwrap();

        db.cleanup_retention().unwrap();

        assert!(db.get_item(&item.id).unwrap().is_none());
        let _ = std::fs::remove_dir_all(temp);
    }

    #[test]
    fn pending_quick_suggestions_expire_after_five_hours() {
        let temp =
            std::env::temp_dir().join(format!("smart-clipboard-test-{}", uuid::Uuid::new_v4()));
        let db_path = temp.join("test.sqlite");
        let image_dir = temp.join("images");
        let mut db = Database::open(db_path, image_dir).unwrap();

        for _ in 0..5 {
            let _ = db.insert_text_item("temporary candidate text").unwrap();
        }
        assert_eq!(db.get_quick_suggestions().unwrap().len(), 1);

        db.conn
            .execute(
                "UPDATE quick_suggestions SET updated_at = ?1",
                params![now_ts() - (5 * 60 * 60) - 1],
            )
            .unwrap();

        assert!(db.get_quick_suggestions().unwrap().is_empty());
        let _ = std::fs::remove_dir_all(temp);
    }

    #[test]
    fn quick_pool_ignores_reused_inner_phrase_with_different_full_text() {
        let temp =
            std::env::temp_dir().join(format!("smart-clipboard-test-{}", uuid::Uuid::new_v4()));
        let db_path = temp.join("test.sqlite");
        let image_dir = temp.join("images");
        let mut db = Database::open(db_path, image_dir).unwrap();

        let phrase = "reusable phrase longer than ten";
        let mut extracted = Vec::new();
        for index in 0..5 {
            let (_, quick_items) = db
                .insert_text_item(&format!("prefix {index} {phrase} suffix {index}"))
                .unwrap();
            extracted.extend(quick_items);
        }

        assert!(extracted.is_empty());
        let _ = std::fs::remove_dir_all(temp);
    }
}
