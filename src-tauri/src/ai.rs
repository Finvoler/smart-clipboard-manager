use crate::models::ClipboardItem;

pub async fn semantic_search(query: &str, records: Vec<ClipboardItem>) -> Result<Vec<String>, String> {
  if std::env::var("SMART_CLIPBOARD_MOCK_AI").ok().as_deref() == Some("1") {
    let keyword = query.to_lowercase();
    return Ok(records
      .into_iter()
      .filter(|item| format!("{} {}", item.preview, item.content.clone().unwrap_or_default()).to_lowercase().contains(&keyword))
      .map(|item| item.id)
      .collect());
  }

  let endpoint = std::env::var("SMART_CLIPBOARD_LLM_ENDPOINT").map_err(|_| "AI provider is not configured; set SMART_CLIPBOARD_LLM_ENDPOINT and trigger manually again".to_string())?;
  let api_key = std::env::var("SMART_CLIPBOARD_LLM_API_KEY").unwrap_or_default();
  let body = serde_json::json!({
    "task": "semantic_search",
    "query": query,
    "records": records.iter().map(|item| serde_json::json!({ "id": item.id, "text": item.content.clone().unwrap_or_else(|| item.preview.clone()) })).collect::<Vec<_>>()
  });
  let client = reqwest::Client::new();
  let response = client.post(endpoint).bearer_auth(api_key).json(&body).send().await.map_err(|error| error.to_string())?;
  let value: serde_json::Value = response.json().await.map_err(|error| error.to_string())?;
  let ids = value.get("ids").and_then(|ids| ids.as_array()).ok_or_else(|| "AI response must contain an ids array".to_string())?;
  Ok(ids.iter().filter_map(|id| id.as_str().map(ToString::to_string)).collect())
}

pub async fn categorize(records: Vec<ClipboardItem>) -> Result<Vec<ClipboardItem>, String> {
  if std::env::var("SMART_CLIPBOARD_MOCK_AI").ok().as_deref() == Some("1") {
    return Ok(records);
  }
  let _endpoint = std::env::var("SMART_CLIPBOARD_LLM_ENDPOINT").map_err(|_| "AI archive is manual-only and needs SMART_CLIPBOARD_LLM_ENDPOINT".to_string())?;
  Err("AI archive transport is configured, but folder move policy is intentionally pending explicit provider schema".to_string())
}

pub async fn ocr_image(item: &ClipboardItem) -> Result<String, String> {
  if std::env::var("SMART_CLIPBOARD_MOCK_AI").ok().as_deref() == Some("1") {
    return Ok(format!("OCR text extracted from {}", item.preview));
  }
  let _endpoint = std::env::var("SMART_CLIPBOARD_LLM_ENDPOINT").map_err(|_| "OCR is manual-only and needs SMART_CLIPBOARD_LLM_ENDPOINT".to_string())?;
  Err("OCR transport is configured, but provider-specific vision payload mapping is not set".to_string())
}
