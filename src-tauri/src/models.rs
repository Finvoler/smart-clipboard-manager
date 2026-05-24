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
