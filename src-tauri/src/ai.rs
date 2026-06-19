//! AI 适配层。
//!
//! 负责和 OpenAI 兼容 / Anthropic 兼容接口对话，提供模型列表、语义搜索、
//! 分类归档和 OCR 四类能力。这里刻意保持“按需触发”，不做后台轮询。

use std::fs;

use base64::{engine::general_purpose, Engine as _};
use serde_json::Value;

use crate::models::{AppSettings, CategoryAssignment, ClipboardItem, Folder};

const KNOWN_MIMO_MODELS: [&str; 9] = [
    "mimo-v2.5-pro",
    "mimo-v2.5",
    "mimo-v2.5-tts",
    "mimo-v2.5-tts-voicedesign",
    "mimo-v2.5-tts-voiceclone",
    "mimo-v2-pro",
    "mimo-v2-omni",
    "mimo-v2-tts",
    "mimo-v2-flash",
];

pub async fn test_connection(settings: &AppSettings) -> Result<String, String> {
    let settings = settings.clone().normalized();
    ensure_ai_ready(&settings, false)?;
    let content = complete_text(
        &settings,
        &settings.search_model,
        "Reply with exactly: ok",
        "Connection test. Reply with exactly: ok",
    )
    .await?;

    if content.to_lowercase().contains("ok") {
        Ok("AI connection ok".to_string())
    } else {
        Ok(format!(
            "AI answered: {}",
            content.chars().take(120).collect::<String>()
        ))
    }
}

pub async fn list_models(settings: &AppSettings) -> Result<Vec<String>, String> {
    let settings = settings.clone().normalized();
    ensure_api_key(&settings)?;
    let response = reqwest::Client::new()
        .get(&openai_models_url(&settings))
        .bearer_auth(&settings.api_key)
        .header("api-key", &settings.api_key)
        .send()
        .await
        .map_err(|error| error.to_string())?;
    let value = response_json(response).await?;
    let mut models = value
        .get("data")
        .and_then(|data| data.as_array())
        .into_iter()
        .flatten()
        .filter_map(|item| {
            item.get("id")
                .and_then(|id| id.as_str())
                .map(ToString::to_string)
        })
        .collect::<Vec<_>>();
    if models.is_empty() {
        models.extend(KNOWN_MIMO_MODELS.iter().map(|model| model.to_string()));
    }
    models.sort();
    models.dedup();
    Ok(models)
}

pub fn known_models() -> Vec<String> {
    KNOWN_MIMO_MODELS
        .iter()
        .map(|model| model.to_string())
        .collect()
}

pub async fn semantic_search(
    settings: &AppSettings,
    query: &str,
    records: Vec<ClipboardItem>,
) -> Result<Vec<String>, String> {
    if std::env::var("SMART_CLIPBOARD_MOCK_AI").ok().as_deref() == Some("1") {
        let keyword = query.to_lowercase();
        let wants_web = ["网页", "网站", "链接", "网址", "url", "http", "www"]
            .iter()
            .any(|needle| keyword.contains(needle));
        return Ok(records
            .into_iter()
            .filter(|item| {
                let haystack = format!(
                    "{} {}",
                    item.preview,
                    item.content.clone().unwrap_or_default()
                )
                .to_lowercase();
                haystack.contains(&keyword)
                    || (wants_web && (haystack.contains("http") || haystack.contains("www.")))
            })
            .map(|item| item.id)
            .collect());
    }

    let settings = settings.clone().normalized();
    ensure_ai_ready(&settings, false)?;
    let now = crate::db::now_ts();
    let payload = records
        .iter()
        .map(|item| {
            let age_seconds = now.saturating_sub(item.created_at);
            serde_json::json!({
              "id": item.id,
              "kind": item.kind,
                            "createdAtUnix": item.created_at,
                            "updatedAtUnix": item.updated_at,
                            "ageSeconds": age_seconds,
                            "ageMinutes": age_seconds / 60,
                            "ageHours": age_seconds / 3600,
                            "ageText": human_age(age_seconds),
                            "text": item.content.clone().or_else(|| item.ocr_text.clone()).unwrap_or_else(|| item.preview.clone())
            })
        })
        .collect::<Vec<_>>();

    eprintln!("[AI SEARCH] query={}, records={}", query, records.len());
    let user_msg = format!("currentUnixSeconds: {now}\nUser query: {query}\nClipboard records JSON: {}", Value::Array(payload));
    let system_prompt = "You are a permissive local clipboard search assistant. Return compact JSON only with the exact shape {\"ids\":[\"item-id\"]}. Your job is to find clipboard records that are useful for the user's short, fuzzy query. Use broad semantic intent, synonyms, abbreviations, likely entities, and contextual inference. The user may ask relative-time questions such as 刚刚/几分钟前/几小时前/昨天/前几天; never guess from your training date. Use currentUnixSeconds plus each record's createdAtUnix, ageSeconds, ageMinutes, ageHours, and ageText to judge time accurately. If the query asks for webpages, websites, links, URLs, or says words like 网页/网站/链接/网址/url/http/www, include records containing http, https, www, domains, or URL-like text even if the query word is not literally present. If the query is vague, such as 什么药/药名/那个药, infer likely medicine or drug names from the records and include plausible matches. Include exact keyword matches, fuzzy semantic matches, inferred relevant records, and time-relevant records. Do not invent ids. Do not explain. Prefer recall over strictness, but exclude clearly unrelated records.";

    let content = complete_text(&settings, &settings.search_model, system_prompt, &user_msg).await?;
    eprintln!("[AI SEARCH] raw response ({} chars): {:?}", content.len(), content.chars().take(300).collect::<String>());

    let parsed = match parse_json_content(&content) {
        Ok(v) => v,
        Err(err) => {
            eprintln!("[AI SEARCH] first parse failed: {}, retrying with stricter prompt", err);
            let retry_user = format!("Your previous response was not valid JSON. Return ONLY a JSON object, no explanation, no markdown fences.\n\n{}", user_msg);
            let retry_system = "Return ONLY valid JSON. No explanation, no markdown fences, no extra text. Shape: {{\"ids\":[\"item-id\"]}}";
            let retry_content = complete_text(&settings, &settings.search_model, retry_system, &retry_user).await?;
            eprintln!("[AI SEARCH] retry response ({} chars): {:?}", retry_content.len(), retry_content.chars().take(300).collect::<String>());
            parse_json_content(&retry_content)?
        }
    };

    let ids = parsed
        .get("ids")
        .and_then(|ids| ids.as_array())
        .ok_or_else(|| "AI response must contain an ids array".to_string())?;
    Ok(ids
        .iter()
        .filter_map(|id| id.as_str().map(ToString::to_string))
        .collect())
}

fn human_age(seconds: i64) -> String {
    if seconds < 60 {
        return "刚刚".to_string();
    }
    if seconds < 60 * 60 {
        return format!("{} 分钟前", seconds / 60);
    }
    if seconds < 24 * 60 * 60 {
        return format!("{} 小时前", seconds / 3600);
    }
    format!("{} 天前", seconds / (24 * 60 * 60))
}

pub async fn categorize(
    settings: &AppSettings,
    records: Vec<ClipboardItem>,
    existing_folders: Vec<Folder>,
) -> Result<Vec<CategoryAssignment>, String> {
    if std::env::var("SMART_CLIPBOARD_MOCK_AI").ok().as_deref() == Some("1") {
        let folder_name = existing_folders
            .first()
            .map(|folder| folder.name.clone())
            .unwrap_or_else(|| "AI Archive".to_string());
        return Ok(records
            .into_iter()
            .map(|item| CategoryAssignment {
                item_id: item.id,
                folder_name: folder_name.clone(),
            })
            .collect());
    }

    let settings = settings.clone().normalized();
    ensure_ai_ready(&settings, false)?;
    let payload = records
        .iter()
        .map(|item| {
            serde_json::json!({
              "id": item.id,
                            "text": item.content.clone().or_else(|| item.ocr_text.clone()).unwrap_or_else(|| item.preview.clone())
            })
        })
        .collect::<Vec<_>>();
    let folder_payload = existing_folders
        .iter()
        .map(|folder| {
            serde_json::json!({
              "id": folder.id,
              "name": folder.name
            })
        })
        .collect::<Vec<_>>();

    eprintln!("[AI CATEGORIZE] records={}, folders={}", records.len(), existing_folders.len());
    let user_msg = format!("Requested folder language: {}\nExisting folders JSON: {}\nUncategorized clipboard records JSON: {}", settings.language, Value::Array(folder_payload), Value::Array(payload));
    let system_prompt = "You organize clipboard history. Return compact JSON only, with the shape {\"assignments\":[{\"id\":\"item-id\",\"folder\":\"short folder name\"}]}. Prefer existing folders whenever a reasonable match exists; use the exact existing folder name in that case. Create new folders only for broad reusable categories that fit multiple records. Minimize the number of new folders, avoid one-record niche folders, and group related ambiguous records into a small general folder instead of creating many tiny folders. Skip unclear records only when no existing or broad new folder is appropriate. For newly created folder names, use the requested UI language: Chinese for zh and English for en. Existing folder names must remain unchanged.";

    let content = complete_text(&settings, &settings.search_model, system_prompt, &user_msg).await?;
    eprintln!("[AI CATEGORIZE] raw response ({} chars): {:?}", content.len(), content.chars().take(300).collect::<String>());

    let parsed = match parse_json_content(&content) {
        Ok(v) => v,
        Err(err) => {
            eprintln!("[AI CATEGORIZE] first parse failed: {}, retrying", err);
            let retry_user = format!("Your previous response was not valid JSON. Return ONLY a JSON object, no explanation, no markdown fences.\n\n{}", user_msg);
            let retry_system = "Return ONLY valid JSON. No explanation, no markdown fences. Shape: {{\"assignments\":[{{\"id\":\"item-id\",\"folder\":\"folder name\"}}]}}";
            let retry_content = complete_text(&settings, &settings.search_model, retry_system, &retry_user).await?;
            eprintln!("[AI CATEGORIZE] retry response ({} chars): {:?}", retry_content.len(), retry_content.chars().take(300).collect::<String>());
            parse_json_content(&retry_content)?
        }
    };

    let assignments = parsed
        .get("assignments")
        .and_then(|items| items.as_array())
        .ok_or_else(|| "AI response must contain an assignments array".to_string())?;
    Ok(assignments
        .iter()
        .filter_map(|assignment| {
            Some(CategoryAssignment {
                item_id: assignment.get("id")?.as_str()?.to_string(),
                folder_name: assignment.get("folder")?.as_str()?.to_string(),
            })
        })
        .collect())
}

pub async fn ocr_image(settings: &AppSettings, item: &ClipboardItem) -> Result<String, String> {
    if std::env::var("SMART_CLIPBOARD_MOCK_AI").ok().as_deref() == Some("1") {
        return Ok(format!("OCR text extracted from {}", item.preview));
    }

    let settings = settings.clone().normalized();
    ensure_ai_ready(&settings, true)?;
    let image_path = item
        .image_path
        .as_ref()
        .ok_or_else(|| "image record has no local image path".to_string())?;
    let image_bytes = fs::read(image_path).map_err(|error| error.to_string())?;
    let image_base64 = general_purpose::STANDARD.encode(image_bytes);
    complete_image(
    &settings,
    &settings.ocr_model,
    "Extract all readable text from this image. Return only the extracted text, preserving line breaks when useful.",
    &image_base64,
  ).await
}

fn ensure_ai_ready(settings: &AppSettings, ocr: bool) -> Result<(), String> {
    ensure_api_key(settings)?;
    if settings.search_model.is_empty() || (ocr && settings.ocr_model.is_empty()) {
        return Err("AI model is empty. Set search/archive and OCR model names first.".to_string());
    }
    Ok(())
}

fn ensure_api_key(settings: &AppSettings) -> Result<(), String> {
    if settings.api_key.is_empty() {
        return Err("AI API key is empty. Paste it in API Settings and save it first.".to_string());
    }
    Ok(())
}

async fn complete_text(
    settings: &AppSettings,
    model: &str,
    system: &str,
    user: &str,
) -> Result<String, String> {
    match settings.ai_protocol.as_str() {
        "anthropic" => complete_anthropic_text(settings, model, system, user).await,
        _ => complete_openai_text(settings, model, system, user).await,
    }
}

async fn complete_image(
    settings: &AppSettings,
    model: &str,
    prompt: &str,
    image_base64: &str,
) -> Result<String, String> {
    match settings.ai_protocol.as_str() {
        "anthropic" => complete_anthropic_image(settings, model, prompt, image_base64).await,
        _ => complete_openai_image(settings, model, prompt, image_base64).await,
    }
}

async fn complete_openai_text(
    settings: &AppSettings,
    model: &str,
    system: &str,
    user: &str,
) -> Result<String, String> {
    let body = serde_json::json!({
      "model": model,
            "max_completion_tokens": 8000,
            "thinking": { "type": "disabled" },
      "messages": [
        { "role": "system", "content": system },
        { "role": "user", "content": user }
      ]
    });
    let value = post_json_bearer(&openai_chat_url(settings), &settings.api_key, body).await?;
    extract_openai_content(&value)
}

async fn complete_openai_image(
    settings: &AppSettings,
    model: &str,
    prompt: &str,
    image_base64: &str,
) -> Result<String, String> {
    let body = serde_json::json!({
      "model": model,
            "max_completion_tokens": 2000,
            "thinking": { "type": "disabled" },
      "messages": [
        {
          "role": "user",
          "content": [
            { "type": "text", "text": prompt },
            { "type": "image_url", "image_url": { "url": format!("data:image/png;base64,{image_base64}") } }
          ]
        }
      ]
    });
    let value = post_json_bearer(&openai_chat_url(settings), &settings.api_key, body).await?;
    extract_openai_content(&value)
}

async fn complete_anthropic_text(
    settings: &AppSettings,
    model: &str,
    system: &str,
    user: &str,
) -> Result<String, String> {
    let body = serde_json::json!({
      "model": model,
      "max_tokens": 1200,
      "system": system,
      "messages": [{ "role": "user", "content": user }]
    });
    let value =
        post_json_anthropic(&anthropic_messages_url(settings), &settings.api_key, body).await?;
    extract_anthropic_content(&value)
}

async fn complete_anthropic_image(
    settings: &AppSettings,
    model: &str,
    prompt: &str,
    image_base64: &str,
) -> Result<String, String> {
    let body = serde_json::json!({
      "model": model,
      "max_tokens": 2000,
      "messages": [{
        "role": "user",
        "content": [
          { "type": "text", "text": prompt },
          { "type": "image", "source": { "type": "base64", "media_type": "image/png", "data": image_base64 } }
        ]
      }]
    });
    let value =
        post_json_anthropic(&anthropic_messages_url(settings), &settings.api_key, body).await?;
    extract_anthropic_content(&value)
}

async fn post_json_bearer(url: &str, api_key: &str, body: Value) -> Result<Value, String> {
    let response = reqwest::Client::new()
        .post(url)
        .bearer_auth(api_key)
        .header("api-key", api_key)
        .json(&body)
        .send()
        .await
        .map_err(|error| error.to_string())?;
    response_json(response).await
}

async fn post_json_anthropic(url: &str, api_key: &str, body: Value) -> Result<Value, String> {
    let response = reqwest::Client::new()
        .post(url)
        .header("api-key", api_key)
        .header("x-api-key", api_key)
        .header("anthropic-version", "2023-06-01")
        .json(&body)
        .send()
        .await
        .map_err(|error| error.to_string())?;
    response_json(response).await
}

async fn response_json(response: reqwest::Response) -> Result<Value, String> {
    let status = response.status();
    let text = response.text().await.map_err(|error| error.to_string())?;
    if !status.is_success() {
        return Err(format!(
            "AI request failed with {status}: {}",
            text.chars().take(500).collect::<String>()
        ));
    }
    serde_json::from_str(&text).map_err(|error| format!("AI response is not JSON: {error}"))
}

fn openai_chat_url(settings: &AppSettings) -> String {
    let base = settings.openai_base_url.trim_end_matches('/');
    if base.ends_with("/chat/completions") {
        base.to_string()
    } else {
        format!("{base}/chat/completions")
    }
}

fn openai_models_url(settings: &AppSettings) -> String {
    let base = settings.openai_base_url.trim_end_matches('/');
    if base.ends_with("/models") {
        base.to_string()
    } else if base.ends_with("/chat/completions") {
        format!("{}/models", base.trim_end_matches("/chat/completions"))
    } else {
        format!("{base}/models")
    }
}

fn anthropic_messages_url(settings: &AppSettings) -> String {
    let base = settings.anthropic_base_url.trim_end_matches('/');
    if base.ends_with("/messages") {
        base.to_string()
    } else if base.ends_with("/v1") {
        format!("{base}/messages")
    } else {
        format!("{base}/v1/messages")
    }
}

fn extract_openai_content(value: &Value) -> Result<String, String> {
    eprintln!("[AI DEBUG] OpenAI response keys: {:?}", value.as_object().map(|m| m.keys().collect::<Vec<_>>()));
    let choice = value
        .get("choices")
        .and_then(|choices| choices.as_array())
        .and_then(|choices| choices.first());

    let message = choice.and_then(|c| c.get("message"));
    let content = message.and_then(|m| m.get("content")).and_then(value_to_text);
    let finish_reason = choice.and_then(|c| c.get("finish_reason")).and_then(|f| f.as_str());

    match &content {
        Some(text) if !text.is_empty() => {
            eprintln!("[AI DEBUG] extracted content ({} chars)", text.len());
            return Ok(text.clone());
        }
        _ => {}
    }

    // Check if thinking model consumed all tokens
    let reasoning = message.and_then(|m| m.get("reasoning_content")).and_then(value_to_text);
    if let (Some(reasoning), Some("length")) = (&reasoning, finish_reason) {
        eprintln!("[AI DEBUG] thinking model hit token limit. reasoning ({} chars): {:?}", reasoning.len(), reasoning.chars().take(200).collect::<String>());
        return Err("AI model used all tokens for reasoning. Try using a non-thinking model (e.g. mimo-v2-flash) or increase max_completion_tokens.".to_string());
    }

    eprintln!("[AI DEBUG] no content. finish_reason={:?}, message={:?}", finish_reason, message.map(|m| serde_json::to_string(m).unwrap_or_default().chars().take(300).collect::<String>()));
    Err("OpenAI-compatible response did not contain choices[0].message.content".to_string())
}

fn extract_anthropic_content(value: &Value) -> Result<String, String> {
    let Some(items) = value.get("content").and_then(|content| content.as_array()) else {
        return Err("Anthropic-compatible response did not contain content array".to_string());
    };
    let text = items
        .iter()
        .filter_map(|item| item.get("text").and_then(|value| value.as_str()))
        .collect::<Vec<_>>()
        .join("\n");
    if text.trim().is_empty() {
        Err("Anthropic-compatible response did not contain text content".to_string())
    } else {
        Ok(text)
    }
}

fn value_to_text(value: &Value) -> Option<String> {
    if let Some(text) = value.as_str() {
        return Some(text.to_string());
    }
    value.as_array().map(|items| {
        items
            .iter()
            .filter_map(|item| item.get("text").and_then(|text| text.as_str()))
            .collect::<Vec<_>>()
            .join("\n")
    })
}

fn parse_json_content(content: &str) -> Result<Value, String> {
    eprintln!("[AI DEBUG] raw content ({} chars): {:?}", content.len(), content.chars().take(500).collect::<String>());

    // Strategy 1: Strip markdown fences and try direct parse
    let stripped = strip_markdown_fences(content);
    if let Ok(value) = try_parse_json(stripped) {
        eprintln!("[AI DEBUG] parse OK via strip+direct");
        return Ok(value);
    }

    // Strategy 2: Extract JSON by brace matching (handles text before/after JSON)
    if let Some(value) = extract_json_by_braces(stripped) {
        eprintln!("[AI DEBUG] parse OK via brace matching");
        return Ok(value);
    }

    // Strategy 3: Try extracting from the raw content (in case fence stripping broke something)
    if stripped != content.trim() {
        if let Some(value) = extract_json_by_braces(content.trim()) {
            eprintln!("[AI DEBUG] parse OK via brace matching on raw");
            return Ok(value);
        }
    }

    // Strategy 4: Last resort - find any valid JSON sub-string by trying progressively smaller slices
    if let Some(value) = extract_json_by_rfind(stripped) {
        eprintln!("[AI DEBUG] parse OK via rfind fallback");
        return Ok(value);
    }

    Err(format!(
        "AI response JSON parse failed. Response: {}",
        content.chars().take(400).collect::<String>()
    ))
}

fn strip_markdown_fences(s: &str) -> &str {
    let s = s.trim();
    // Handle ```json\n...\n``` and ```\n...\n``` patterns
    let s = s.strip_prefix("```json").or_else(|| s.strip_prefix("```"))
        .unwrap_or(s)
        .trim();
    s.strip_suffix("```").map(|s| s.trim()).unwrap_or(s)
}

fn try_parse_json(s: &str) -> Result<Value, String> {
    serde_json::from_str(s).map_err(|e| e.to_string())
}

fn extract_json_by_braces(s: &str) -> Option<Value> {
    // Find the first { or [
    let (start, open_char) = s.char_indices().find(|(_, c)| *c == '{' || *c == '[')?;
    let close_char = if open_char == '{' { '}' } else { ']' };

    // Count nesting depth, handling strings properly
    let mut depth = 0i32;
    let mut end_pos = None;
    let mut in_string = false;
    let mut escape_next = false;

    for (i, c) in s[start..].char_indices() {
        if escape_next {
            escape_next = false;
            continue;
        }
        if c == '\\' && in_string {
            escape_next = true;
            continue;
        }
        if c == '"' {
            in_string = !in_string;
            continue;
        }
        if in_string {
            continue;
        }
        if c == open_char {
            depth += 1;
        } else if c == close_char {
            depth -= 1;
            if depth == 0 {
                end_pos = Some(start + i);
                break;
            }
        }
    }

    let end = end_pos?;
    let extracted = &s[start..=end];

    // Try parsing the extracted JSON
    if let Ok(value) = try_parse_json(extracted) {
        return Some(value);
    }

    // Try fixing common issues (trailing commas)
    let fixed = fix_common_json_issues(extracted);
    try_parse_json(&fixed).ok()
}

fn extract_json_by_rfind(s: &str) -> Option<Value> {
    // Find the last } or ] and try to match backwards
    let end = s.rfind('}').or_else(|| s.rfind(']'))?;
    let open_char = if s[end..].starts_with('}') { '{' } else { '[' };
    let close_char = if open_char == '{' { '}' } else { ']' };

    // Scan backwards from end to find matching open
    let mut depth = 0i32;
    let mut start_pos = None;
    let chars: Vec<char> = s[..=end].chars().collect();

    for i in (0..chars.len()).rev() {
        let c = chars[i];
        if c == close_char {
            depth += 1;
        } else if c == open_char {
            depth -= 1;
            if depth == 0 {
                start_pos = Some(i);
                break;
            }
        }
    }

    let start = start_pos?;
    let extracted: String = chars[start..=end].iter().collect();

    if let Ok(value) = try_parse_json(&extracted) {
        return Some(value);
    }

    let fixed = fix_common_json_issues(&extracted);
    try_parse_json(&fixed).ok()
}

fn fix_common_json_issues(input: &str) -> String {
    let mut result = input.to_string();
    while let Some(pos) = result.find(",}") {
        result.replace_range(pos..pos + 1, "");
    }
    while let Some(pos) = result.find(",]") {
        result.replace_range(pos..pos + 1, "");
    }
    result
}
