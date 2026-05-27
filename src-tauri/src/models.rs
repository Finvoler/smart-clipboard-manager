//! 前后端共享的数据模型。
//!
//! 这些结构体既是 SQLite 读写后的业务对象，也是 Tauri IPC 的序列化边界。

use serde::{Deserialize, Serialize};

pub const DEFAULT_OPENAI_BASE_URL: &str = "https://token-plan-cn.xiaomimimo.com/v1";
pub const DEFAULT_ANTHROPIC_BASE_URL: &str = "https://token-plan-cn.xiaomimimo.com/anthropic";
pub const LEGACY_OPENAI_BASE_URL: &str = "https://api.xiaomimimo.com/v1";
pub const LEGACY_ANTHROPIC_BASE_URL: &str = "https://api.xiaomimimo.com/anthropic";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClipboardItem {
    pub id: String,
    pub kind: String,
    pub content: Option<String>,
    pub image_path: Option<String>,
    pub preview: String,
    pub is_star: bool,
    pub folder_id: Option<String>,
    pub created_at: i64,
    pub updated_at: i64,
    pub expires_at: Option<i64>,
    pub mime_type: Option<String>,
    pub width: Option<i64>,
    pub height: Option<i64>,
    pub image_hash: Option<String>,
    pub ocr_text: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Folder {
    pub id: String,
    pub name: String,
    pub created_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QuickItem {
    pub id: String,
    pub content: String,
    pub hit_count: i64,
    pub created_at: i64,
    pub updated_at: i64,
    pub expires_at: Option<i64>,
    pub is_pinned: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QuickSuggestion {
    pub id: String,
    pub content: String,
    pub hit_count: i64,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppSettings {
    pub app_enabled: bool,
    pub capture_enabled: bool,
    pub intercept_win_v: bool,
    pub run_at_startup: bool,
    pub hide_console_window: bool,
    pub data_directory: String,
    pub resolved_data_directory: String,
    pub ai_protocol: String,
    pub openai_base_url: String,
    pub anthropic_base_url: String,
    pub api_key: String,
    pub search_model: String,
    pub ocr_model: String,
    pub language: String,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            app_enabled: true,
            capture_enabled: true,
            intercept_win_v: true,
            run_at_startup: false,
            hide_console_window: true,
            data_directory: String::new(),
            resolved_data_directory: String::new(),
            ai_protocol: "openai".to_string(),
            openai_base_url: DEFAULT_OPENAI_BASE_URL.to_string(),
            anthropic_base_url: DEFAULT_ANTHROPIC_BASE_URL.to_string(),
            api_key: String::new(),
            search_model: "mimo-v2.5-pro".to_string(),
            ocr_model: "mimo-v2.5".to_string(),
            language: "zh".to_string(),
        }
    }
}

impl AppSettings {
    pub fn normalized(mut self) -> Self {
        self.app_enabled = true;
        if self.ai_protocol != "anthropic" {
            self.ai_protocol = "openai".to_string();
        }
        self.data_directory = self.data_directory.trim().to_string();
        self.resolved_data_directory = self.resolved_data_directory.trim().to_string();
        self.openai_base_url = self
            .openai_base_url
            .trim()
            .trim_end_matches('/')
            .to_string();
        self.anthropic_base_url = self
            .anthropic_base_url
            .trim()
            .trim_end_matches('/')
            .to_string();
        self.api_key = self.api_key.trim().to_string();
        self.search_model = self.search_model.trim().to_string();
        self.ocr_model = self.ocr_model.trim().to_string();
        self.language = match self.language.trim().to_ascii_lowercase().as_str() {
            "en" | "english" => "en".to_string(),
            _ => "zh".to_string(),
        };

        if self.openai_base_url == LEGACY_OPENAI_BASE_URL {
            self.openai_base_url = DEFAULT_OPENAI_BASE_URL.to_string();
        }
        if self.anthropic_base_url == LEGACY_ANTHROPIC_BASE_URL {
            self.anthropic_base_url = DEFAULT_ANTHROPIC_BASE_URL.to_string();
        }
        self.search_model = normalize_model_name(&self.search_model);
        self.ocr_model = normalize_model_name(&self.ocr_model);

        let defaults = Self::default();
        if self.openai_base_url.is_empty() {
            self.openai_base_url = defaults.openai_base_url;
        }
        if self.anthropic_base_url.is_empty() {
            self.anthropic_base_url = defaults.anthropic_base_url;
        }
        if self.search_model.is_empty() {
            self.search_model = defaults.search_model.clone();
        }
        if self.ocr_model.is_empty() {
            self.ocr_model = self.search_model.clone();
        }
        if self.resolved_data_directory.is_empty() {
            self.resolved_data_directory = self.data_directory.clone();
        }
        self
    }
}

fn normalize_model_name(model: &str) -> String {
    match model.trim().to_ascii_lowercase().as_str() {
        "mimo2.5pro" | "mimo-v2.5pro" | "mimo-v25-pro" | "mimo-v2-5-pro" => {
            "mimo-v2.5-pro".to_string()
        }
        "mimo2.5" | "mimo-v25" | "mimo-v2-5" => "mimo-v2.5".to_string(),
        value => value.to_string(),
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CategoryAssignment {
    pub item_id: String,
    pub folder_name: String,
}
