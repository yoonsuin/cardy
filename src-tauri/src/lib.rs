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

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct IntegrationSettings {
  jira_base_url: Option<String>,
  jira_email: Option<String>,
  jira_api_token: Option<String>,
  slack_bot_token: Option<String>,
  slack_workspace_name: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct FetchJiraIssuesPayload {
  settings: IntegrationSettings,
  jql: String,
  max_results: Option<u32>,
  fields: Option<Vec<String>>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct FetchJiraIssuePayload {
  settings: IntegrationSettings,
  issue_id_or_key: String,
  fields: Option<Vec<String>>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SlackSettingsPayload {
  settings: IntegrationSettings,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PostSlackMessagePayload {
  settings: IntegrationSettings,
  channel: String,
  message: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct UploadSlackImagesPayload {
  settings: IntegrationSettings,
  channel: String,
  initial_comment: Option<String>,
  images: Vec<SlackImagePayload>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SlackImagePayload {
  filename: String,
  title: Option<String>,
  alt_text: Option<String>,
  data_url: String,
}

#[derive(Debug, Deserialize)]
struct SaveDesktopStatePayload {
  state: Value,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct RuntimeInfo {
  platform: String,
  app_version: String,
  config_dir: String,
  desktop_state_ready: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct SlackChannel {
  id: String,
  name: Option<String>,
  is_private: bool,
  is_im: bool,
  user: Option<String>,
}

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
  let raw = fs::read_to_string(&state_path).map_err(|e| format!("read desktop state failed: {e}"))?;
  serde_json::from_str(&raw).map_err(|e| format!("parse desktop state failed: {e}"))
}

#[tauri::command]
async fn save_desktop_state(app: AppHandle, payload: SaveDesktopStatePayload) -> Result<Value, String> {
  let state_path = desktop_state_path(&app)?;
  if let Some(parent) = state_path.parent() {
    fs::create_dir_all(parent).map_err(|e| format!("create config dir failed: {e}"))?;
  }
  let serialized =
    serde_json::to_string_pretty(&payload.state).map_err(|e| format!("serialize desktop state failed: {e}"))?;
  fs::write(&state_path, serialized).map_err(|e| format!("write desktop state failed: {e}"))?;
  Ok(json!({ "ok": true }))
}

#[tauri::command]
async fn fetch_jira_issues(payload: FetchJiraIssuesPayload) -> Result<Vec<Value>, String> {
  let base_url = payload
    .settings
    .jira_base_url
    .as_deref()
    .ok_or_else(|| "jiraBaseUrl is required".to_string())?
    .trim_end_matches('/')
    .to_string();
  let headers = ensure_jira_headers(&payload.settings)?;
  let client = Client::new();
  let body = json!({
    "jql": payload.jql,
    "maxResults": payload.max_results.unwrap_or(20),
    "fields": payload.fields.unwrap_or_default()
  });

  let response = client
    .post(format!("{base_url}/rest/api/3/search"))
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

#[tauri::command]
async fn fetch_jira_issue(payload: FetchJiraIssuePayload) -> Result<Value, String> {
  let base_url = payload
    .settings
    .jira_base_url
    .as_deref()
    .ok_or_else(|| "jiraBaseUrl is required".to_string())?
    .trim_end_matches('/')
    .to_string();
  let headers = ensure_jira_headers(&payload.settings)?;
  let client = Client::new();
  let fields = payload.fields.unwrap_or_default().join(",");
  let response = client
    .get(format!(
      "{base_url}/rest/api/3/issue/{}",
      payload.issue_id_or_key
    ))
    .headers(headers)
    .query(&[("fields", fields)])
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

#[tauri::command]
async fn list_slack_conversations(payload: SlackSettingsPayload) -> Result<Vec<SlackChannel>, String> {
  let token = ensure_slack_token(&payload.settings)?;
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
      let is_private = channel
        .get("is_private")
        .and_then(Value::as_bool)
        .unwrap_or(false);
      let is_im = channel.get("is_im").and_then(Value::as_bool).unwrap_or(false);
      let name = channel.get("name").and_then(Value::as_str).map(str::to_string);
      let user = channel.get("user").and_then(Value::as_str).map(str::to_string);
      Some(SlackChannel {
        id,
        name,
        is_private,
        is_im,
        user,
      })
    })
    .collect();

  Ok(mapped)
}

#[tauri::command]
async fn post_slack_message(payload: PostSlackMessagePayload) -> Result<Value, String> {
  let token = ensure_slack_token(&payload.settings)?;
  let client = Client::new();
  let response = client
    .post("https://slack.com/api/chat.postMessage")
    .bearer_auth(token)
    .json(&json!({
      "channel": payload.channel,
      "text": payload.message
    }))
    .send()
    .await
    .map_err(|e| format!("slack post request failed: {e}"))?;

  let body: Value = response
    .json()
    .await
    .map_err(|e| format!("slack post decode failed: {e}"))?;

  if !body.get("ok").and_then(Value::as_bool).unwrap_or(false) {
    let err = body
      .get("error")
      .and_then(Value::as_str)
      .unwrap_or("unknown_error");
    return Err(format!("slack post failed: {err}"));
  }

  Ok(body)
}

#[tauri::command]
async fn upload_slack_images(payload: UploadSlackImagesPayload) -> Result<Value, String> {
  let token = ensure_slack_token(&payload.settings)?;
  let client = Client::new();
  let mut uploaded = Vec::new();

  for (index, image) in payload.images.iter().enumerate() {
    let bytes = decode_data_url(&image.data_url)?;
    let mut form = multipart::Form::new()
      .text("channels", payload.channel.clone())
      .part(
        "file",
        multipart::Part::bytes(bytes)
          .file_name(image.filename.clone())
          .mime_str("image/png")
          .map_err(|e| format!("file mime set failed: {e}"))?,
      );

    if let Some(title) = &image.title {
      if !title.trim().is_empty() {
        form = form.text("title", title.clone());
      }
    }

    if let Some(alt_text) = &image.alt_text {
      if !alt_text.trim().is_empty() {
        form = form.text("alt_txt", alt_text.clone());
      }
    }

    if index == 0 {
      if let Some(comment) = &payload.initial_comment {
        if !comment.trim().is_empty() {
          form = form.text("initial_comment", comment.clone());
        }
      }
    }

    let response = client
      .post("https://slack.com/api/files.upload")
      .bearer_auth(token)
      .multipart(form)
      .send()
      .await
      .map_err(|e| format!("slack image upload request failed: {e}"))?;

    let body: Value = response
      .json()
      .await
      .map_err(|e| format!("slack image upload decode failed: {e}"))?;

    if !body.get("ok").and_then(Value::as_bool).unwrap_or(false) {
      let err = body
        .get("error")
        .and_then(Value::as_str)
        .unwrap_or("unknown_error");
      return Err(format!("slack image upload failed: {err}"));
    }

    uploaded.push(body);
  }

  Ok(json!({ "ok": true, "uploads": uploaded, "workspaceName": payload.settings.slack_workspace_name }))
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
