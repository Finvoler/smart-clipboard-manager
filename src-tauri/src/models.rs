use serde::{Deserialize, Serialize};

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
pub struct AppSettings {
    pub app_enabled: bool,
    pub capture_enabled: bool,
    pub intercept_win_v: bool,
    pub run_at_startup: bool,
    pub hide_console_window: bool,
    pub ai_protocol: String,
    pub openai_base_url: String,
    pub anthropic_base_url: String,
    pub api_key: String,
    pub search_model: String,
    pub ocr_model: String,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            app_enabled: true,
            capture_enabled: true,
            intercept_win_v: true,
            run_at_startup: false,
            hide_console_window: true,
            ai_protocol: "openai".to_string(),
            openai_base_url: "https://token-plan-cn.xiaomimimo.com/v1".to_string(),
            anthropic_base_url: "https://token-plan-cn.xiaomimimo.com/anthropic".to_string(),
            api_key: String::new(),
            search_model: "mimo2.5pro".to_string(),
            ocr_model: "mimo2.5pro".to_string(),
        }
    }
}

impl AppSettings {
    pub fn normalized(mut self) -> Self {
        if self.ai_protocol != "anthropic" {
            self.ai_protocol = "openai".to_string();
        }
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
        self
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CategoryAssignment {
    pub item_id: String,
    pub folder_name: String,
}
