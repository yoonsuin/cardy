use std::{fs, path::PathBuf};

use base64::{engine::general_purpose, Engine as _};
use reqwest::{
  header::{HeaderMap, HeaderValue, AUTHORIZATION, CONTENT_TYPE},
  multipart,
  Client,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tauri::{AppHandle, Manager};

const DESKTOP_STATE_FILE: &str = "signal-deck-state.json";

// ── 공용 설정 구조체 ───────────────────────────────────────────
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct IntegrationSettings {
  jira_base_url: Option<String>,
  jira_email: Option<String>,
  jira_api_token: Option<String>,
  slack_bot_token: Option<String>,
  slack_workspace_name: Option<String>,
}

// ── Slack 이미지 페이로드 (JS camelCase 그대로 수신) ──────────
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SlackImagePayload {
  filename: String,
  title: Option<String>,
  alt_text: Option<String>,
  data_url: String,
}

// ── 런타임 정보 응답 ──────────────────────────────────────────
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct RuntimeInfo {
  platform: String,
  app_version: String,
  config_dir: String,
  desktop_state_ready: bool,
}

// ── Slack 채널 응답 ───────────────────────────────────────────
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct SlackChannel {
  id: String,
  name: Option<String>,
  is_private: bool,
  is_im: bool,
  user: Option<String>,
}

// ── 내부 유틸 ─────────────────────────────────────────────────
fn app_config_dir(app: &AppHandle) -> Result<PathBuf, String> {
  app
    .path()
    .app_config_dir()
    .map_err(|e| format!("config dir resolve failed: {e}"))
}

fn desktop_state_path(app: &AppHandle) -> Result<PathBuf, String> {
  let dir = app_config_dir(app)?;
  Ok(dir.join(DESKTOP_STATE_FILE))
}

fn ensure_jira_headers(settings: &IntegrationSettings) -> Result<HeaderMap, String> {
  let base = settings
    .jira_base_url
    .as_deref()
    .filter(|v| !v.trim().is_empty())
    .ok_or_else(|| "jiraBaseUrl is required".to_string())?;
  let email = settings
    .jira_email
    .as_deref()
    .filter(|v| !v.trim().is_empty())
    .ok_or_else(|| "jiraEmail is required".to_string())?;
  let token = settings
    .jira_api_token
    .as_deref()
    .filter(|v| !v.trim().is_empty())
    .ok_or_else(|| "jiraApiToken is required".to_string())?;

  if !base.starts_with("http://") && !base.starts_with("https://") {
    return Err("jiraBaseUrl must start with http:// or https://".to_string());
  }

  let mut headers = HeaderMap::new();
  let auth = format!("{email}:{token}");
  let encoded = general_purpose::STANDARD.encode(auth.as_bytes());
  let auth_header = HeaderValue::from_str(&format!("Basic {encoded}"))
    .map_err(|e| format!("invalid jira authorization header: {e}"))?;
  headers.insert(AUTHORIZATION, auth_header);
  headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
  Ok(headers)
}

fn ensure_slack_token(settings: &IntegrationSettings) -> Result<&str, String> {
  settings
    .slack_bot_token
    .as_deref()
    .filter(|v| !v.trim().is_empty())
    .ok_or_else(|| "slackBotToken is required".to_string())
}

fn decode_data_url(data_url: &str) -> Result<Vec<u8>, String> {
  let marker = ";base64,";
  let index = data_url
    .find(marker)
    .ok_or_else(|| "invalid dataUrl format".to_string())?;
  let encoded = &data_url[index + marker.len()..];
  general_purpose::STANDARD
    .decode(encoded)
    .map_err(|e| format!("dataUrl decode failed: {e}"))
}

// ── Tauri 커맨드 ──────────────────────────────────────────────

#[tauri::command]
async fn get_runtime_info(app: AppHandle) -> Result<RuntimeInfo, String> {
  let config_dir = app_config_dir(&app)?;
  fs::create_dir_all(&config_dir).map_err(|e| format!("config dir create failed: {e}"))?;
  let app_version = app.package_info().version.to_string();
  Ok(RuntimeInfo {
    platform: std::env::consts::OS.to_string(),
    app_version,
    config_dir: config_dir.to_string_lossy().to_string(),
    desktop_state_ready: true,
  })
}

#[tauri::command]
async fn load_desktop_state(app: AppHandle) -> Result<Value, String> {
  let state_path = desktop_state_path(&app)?;
  if !state_path.exists() {
    return Ok(json!({}));
  }
  let raw = fs::read_to_string(&state_path)
    .map_err(|e| format!("read desktop state failed: {e}"))?;
  serde_json::from_str(&raw).map_err(|e| format!("parse desktop state failed: {e}"))
}

/// JS: tauriInvoke("save_desktop_state", { state: {...} })
#[tauri::command]
async fn save_desktop_state(app: AppHandle, state: Value) -> Result<Value, String> {
  let state_path = desktop_state_path(&app)?;
  if let Some(parent) = state_path.parent() {
    fs::create_dir_all(parent).map_err(|e| format!("create config dir failed: {e}"))?;
  }
  let serialized =
    serde_json::to_string_pretty(&state).map_err(|e| format!("serialize desktop state failed: {e}"))?;
  fs::write(&state_path, serialized).map_err(|e| format!("write desktop state failed: {e}"))?;
  Ok(json!({ "ok": true }))
}

/// JS: tauriInvoke("fetch_jira_issues", { settings, jql, max_results, fields })
#[tauri::command]
async fn fetch_jira_issues(
  settings: IntegrationSettings,
  jql: String,
  max_results: Option<u32>,
  fields: Option<Vec<String>>,
) -> Result<Vec<Value>, String> {
  let base_url = settings
    .jira_base_url
    .as_deref()
    .ok_or_else(|| "jiraBaseUrl is required".to_string())?
    .trim_end_matches('/')
    .to_string();
  let headers = ensure_jira_headers(&settings)?;
  let client = Client::new();
  let body = json!({
    "jql": jql,
    "maxResults": max_results.unwrap_or(20),
    "fields": fields.unwrap_or_default()
  });

  let response = client
    .post(format!("{base_url}/rest/api/3/search/jql"))
    .headers(headers)
    .json(&body)
    .send()
    .await
    .map_err(|e| format!("jira search request failed: {e}"))?;

  if !response.status().is_success() {
    let status = response.status();
    let text = response.text().await.unwrap_or_default();
    return Err(format!("jira search failed ({status}): {text}"));
  }

  let json: Value = response
    .json()
    .await
    .map_err(|e| format!("jira search decode failed: {e}"))?;
  let issues = json
    .get("issues")
    .and_then(|v| v.as_array())
    .cloned()
    .unwrap_or_default();
  Ok(issues)
}

/// JS: tauriInvoke("fetch_jira_issue", { settings, issue_id_or_key, fields })
#[tauri::command]
async fn fetch_jira_issue(
  settings: IntegrationSettings,
  issue_id_or_key: String,
  fields: Option<Vec<String>>,
) -> Result<Value, String> {
  let base_url = settings
    .jira_base_url
    .as_deref()
    .ok_or_else(|| "jiraBaseUrl is required".to_string())?
    .trim_end_matches('/')
    .to_string();
  let headers = ensure_jira_headers(&settings)?;
  let client = Client::new();
  let fields_str = fields.unwrap_or_default().join(",");
  let response = client
    .get(format!("{base_url}/rest/api/3/issue/{issue_id_or_key}"))
    .headers(headers)
    .query(&[("fields", fields_str)])
    .send()
    .await
    .map_err(|e| format!("jira issue request failed: {e}"))?;

  if !response.status().is_success() {
    let status = response.status();
    let text = response.text().await.unwrap_or_default();
    return Err(format!("jira issue failed ({status}): {text}"));
  }

  response
    .json::<Value>()
    .await
    .map_err(|e| format!("jira issue decode failed: {e}"))
}

/// JS: tauriInvoke("list_slack_conversations", { settings })
#[tauri::command]
async fn list_slack_conversations(settings: IntegrationSettings) -> Result<Vec<SlackChannel>, String> {
  let token = ensure_slack_token(&settings)?;
  let client = Client::new();

  let response = client
    .get("https://slack.com/api/conversations.list")
    .bearer_auth(token)
    .query(&[
      ("types", "public_channel,private_channel,im,mpim"),
      ("limit", "1000"),
      ("exclude_archived", "true"),
    ])
    .send()
    .await
    .map_err(|e| format!("slack conversations request failed: {e}"))?;

  let body: Value = response
    .json()
    .await
    .map_err(|e| format!("slack conversations decode failed: {e}"))?;

  let ok = body.get("ok").and_then(Value::as_bool).unwrap_or(false);
  if !ok {
    let err = body
      .get("error")
      .and_then(Value::as_str)
      .unwrap_or("unknown_error");
    return Err(format!("slack conversations failed: {err}"));
  }

  let channels = body
    .get("channels")
    .and_then(Value::as_array)
    .cloned()
    .unwrap_or_default();

  let mapped = channels
    .into_iter()
    .filter_map(|channel| {
      let id = channel.get("id")?.as_str()?.to_string();
      let is_private = channel.get("is_private").and_then(Value::as_bool).unwrap_or(false);
      let is_im = channel.get("is_im").and_then(Value::as_bool).unwrap_or(false);
      let name = channel.get("name").and_then(Value::as_str).map(str::to_string);
      let user = channel.get("user").and_then(Value::as_str).map(str::to_string);
      Some(SlackChannel { id, name, is_private, is_im, user })
    })
    .collect();

  Ok(mapped)
}

/// JS: tauriInvoke("post_slack_message", { settings, channel, message })
#[tauri::command]
async fn post_slack_message(
  settings: IntegrationSettings,
  channel: String,
  message: String,
) -> Result<Value, String> {
  let token = ensure_slack_token(&settings)?;
  let client = Client::new();
  let response = client
    .post("https://slack.com/api/chat.postMessage")
    .bearer_auth(token)
    .json(&json!({ "channel": channel, "text": message }))
    .send()
    .await
    .map_err(|e| format!("slack post request failed: {e}"))?;

  let body: Value = response
    .json()
    .await
    .map_err(|e| format!("slack post decode failed: {e}"))?;

  if !body.get("ok").and_then(Value::as_bool).unwrap_or(false) {
    let err = body.get("error").and_then(Value::as_str).unwrap_or("unknown_error");
    return Err(format!("slack post failed: {err}"));
  }
  Ok(body)
}

/// JS: tauriInvoke("upload_slack_images", { settings, channel, initial_comment, images })
/// Slack Files v2 API: getUploadURLExternal → upload → completeUploadExternal
#[tauri::command]
async fn upload_slack_images(
  settings: IntegrationSettings,
  channel: String,
  initial_comment: Option<String>,
  images: Vec<SlackImagePayload>,
) -> Result<Value, String> {
  let token = ensure_slack_token(&settings)?;
  let client = Client::new();
  let mut file_ids: Vec<(String, Option<String>)> = Vec::new();

  for image in images.iter() {
    let bytes = decode_data_url(&image.data_url)?;

    // ── Step 1: 업로드 URL 발급 (form-encoded, JSON 아님) ──
    let length_str = bytes.len().to_string();
    let get_url_body: Value = client
      .post("https://slack.com/api/files.getUploadURLExternal")
      .bearer_auth(token)
      .form(&[
        ("filename", image.filename.as_str()),
        ("length",   length_str.as_str()),
      ])
      .send()
      .await
      .map_err(|e| format!("slack getUploadURL request failed: {e}"))?
      .json()
      .await
      .map_err(|e| format!("slack getUploadURL decode failed: {e}"))?;

    if !get_url_body.get("ok").and_then(Value::as_bool).unwrap_or(false) {
      let err = get_url_body.get("error").and_then(Value::as_str).unwrap_or("unknown_error");
      let hint = get_url_body.get("needed").and_then(Value::as_str).unwrap_or("");
      return Err(format!("slack getUploadURL failed: {err}{}", if hint.is_empty() { String::new() } else { format!(" (needed scope: {hint})") }));
    }

    let upload_url = get_url_body
      .get("upload_url")
      .and_then(Value::as_str)
      .ok_or_else(|| "slack getUploadURL: missing upload_url".to_string())?
      .to_string();
    let file_id = get_url_body
      .get("file_id")
      .and_then(Value::as_str)
      .ok_or_else(|| "slack getUploadURL: missing file_id".to_string())?
      .to_string();

    // ── Step 2: 파일 바이트 업로드 (PUT, Slack 문서 기준) ──
    let upload_resp = client
      .put(&upload_url)
      .header("Content-Type", "application/octet-stream")
      .body(bytes)
      .send()
      .await
      .map_err(|e| format!("slack file upload request failed: {e}"))?;
    if !upload_resp.status().is_success() {
      let status = upload_resp.status();
      return Err(format!("slack file upload failed: HTTP {status}"));
    }

    file_ids.push((file_id, image.title.clone()));
  }

  // ── Step 3: 업로드 완료 (channel_id 포함 → 채널 공유 시도) ──
  let channel_id = channel.trim().to_string();
  let files_payload: Vec<Value> = file_ids
    .iter()
    .map(|(id, title)| {
      let mut obj = json!({ "id": id });
      if let Some(t) = title {
        if !t.trim().is_empty() {
          obj["title"] = json!(t);
        }
      }
      obj
    })
    .collect();

  let comment_text = initial_comment
    .as_deref()
    .map(|s| s.trim())
    .filter(|s| !s.is_empty())
    .unwrap_or("")
    .to_string();

  let mut complete_payload = json!({
    "files": files_payload,
    "channel_id": channel_id
  });
  if !comment_text.is_empty() {
    complete_payload["initial_comment"] = json!(&comment_text);
  }

  let complete_body: Value = client
    .post("https://slack.com/api/files.completeUploadExternal")
    .bearer_auth(token)
    .json(&complete_payload)
    .send()
    .await
    .map_err(|e| format!("slack completeUpload request failed: {e}"))?
    .json()
    .await
    .map_err(|e| format!("slack completeUpload decode failed: {e}"))?;

  if !complete_body.get("ok").and_then(Value::as_bool).unwrap_or(false) {
    let err = complete_body.get("error").and_then(Value::as_str).unwrap_or("unknown_error");
    let needed = complete_body.get("needed").and_then(Value::as_str).unwrap_or("");
    return Err(format!("slack completeUpload failed: {err}{}", if needed.is_empty() { String::new() } else { format!(" (needed scope: {needed})") }));
  }

  // ── Step 4: 채널 공유 확인 → 누락 시 chat.postMessage fallback ──
  // completeUploadExternal 이 ok:true 여도 channel sharing 이 조용히 실패할 수 있음.
  // 응답의 shares 필드를 확인해 실제로 공유됐는지 검증한다.
  let shared_to_channel = complete_body
    .get("files")
    .and_then(Value::as_array)
    .and_then(|arr| arr.first())
    .and_then(|f| f.get("shares"))
    .map(|shares| !shares.as_object().map(|m| m.is_empty()).unwrap_or(true))
    .unwrap_or(false);

  let mut share_method = if shared_to_channel { "completeUpload" } else { "fallback" }.to_string();

  if !shared_to_channel {
    // fallback: chat.postMessage 로 채널에 텍스트 메시지 전송
    // 이미지는 파일로 이미 워크스페이스에 존재하므로, 링크 포함 메시지를 보냄
    let file_links: Vec<String> = complete_body
      .get("files")
      .and_then(Value::as_array)
      .cloned()
      .unwrap_or_default()
      .iter()
      .filter_map(|f| {
        f.get("permalink").and_then(Value::as_str).map(|s| s.to_string())
      })
      .collect();

    let fallback_text = if file_links.is_empty() {
      if comment_text.is_empty() {
        "카드뉴스를 발행했습니다.".to_string()
      } else {
        comment_text.clone()
      }
    } else {
      let links = file_links.join("\n");
      if comment_text.is_empty() {
        format!("카드뉴스를 발행했습니다.\n{links}")
      } else {
        format!("{comment_text}\n{links}")
      }
    };

    let post_resp: Value = client
      .post("https://slack.com/api/chat.postMessage")
      .bearer_auth(token)
      .json(&json!({ "channel": channel_id, "text": fallback_text }))
      .send()
      .await
      .map_err(|e| format!("slack fallback postMessage request failed: {e}"))?
      .json()
      .await
      .map_err(|e| format!("slack fallback postMessage decode failed: {e}"))?;

    if post_resp.get("ok").and_then(Value::as_bool).unwrap_or(false) {
      share_method = "chat.postMessage(fallback)".to_string();
    } else {
      let err = post_resp.get("error").and_then(Value::as_str).unwrap_or("unknown_error");
      return Err(format!("slack 채널 공유 실패 (completeUpload 및 fallback 모두 실패): {err}"));
    }
  }

  Ok(json!({
    "ok": true,
    "shareMethod": share_method,
    "fileCount": file_ids.len(),
    "channel": channel_id,
    "uploads": complete_body,
    "workspaceName": settings.slack_workspace_name
  }))
}

pub fn run() {
  tauri::Builder::default()
    .plugin(tauri_plugin_opener::init())
    .invoke_handler(tauri::generate_handler![
      get_runtime_info,
      load_desktop_state,
      save_desktop_state,
      fetch_jira_issues,
      fetch_jira_issue,
      list_slack_conversations,
      post_slack_message,
      upload_slack_images
    ])
    .run(tauri::generate_context!())
    .expect("error while running tauri application");
}
