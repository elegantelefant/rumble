//! ABOUTME: hosts rumble tauri commands and runtime wiring.
//! ABOUTME: coordinates tray icon, API proxy, keychain, sidecar lifecycle, and plugins.
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::Duration;

use reqwest::Client;
use tauri::image::Image;
use tauri::ipc::Channel;
use tauri::path::BaseDirectory;
use tauri::tray::TrayIconBuilder;
use tauri::{AppHandle, Manager, State};
use tauri_plugin_shell::process::CommandChild;
use tauri_plugin_shell::process::CommandEvent;
use tauri_plugin_shell::ShellExt;
use trash::delete;
use walkdir::WalkDir;
use docx_rs::{Docx, Paragraph, Run};
use percent_encoding::percent_decode_str;
use tauri_plugin_dialog::{DialogExt, MessageDialogButtons, MessageDialogKind};
use uuid::Uuid;

const SERVICE_NAME: &str = "elefant-rumble";
const SECRET_HEADER: &str = "X-Rumble-Secret";
const HEALTH_POLL_ATTEMPTS: u32 = 10;
const HEALTH_POLL_INTERVAL_MS: u64 = 500;

/// The vendored cloud contract (`openapi.json`) serves everything under this prefix.
const CLOUD_API_PREFIX: &str = "/api/v1";
const PREMIUM_REQUIRED: &str = "This feature requires Elefant Premium.";
/// BYOK is OpenAI-only for now (#35): the sidecar only constructs an OpenAI provider.
const BYOK_KEY_PROVIDER: &str = "openai";
const BYOK_KEY_MISSING: &str = "Add an OpenAI API key in Settings before switching to BYOK.";

// --- App state ---

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
enum BackendMode {
    Ollama,
    Byok,
    Premium,
}

struct AppState {
    mode: Mutex<BackendMode>,
    http: Client,
    sidecar_port: Mutex<Option<u16>>,
    sidecar_child: Mutex<Option<CommandChild>>,
    /// Bumped on every spawn; a sidecar's reader task may touch the port only
    /// while its own generation is current.
    sidecar_generation: Mutex<u64>,
    secret: String,
}

/// Whether a URL is our own sidecar — http on 127.0.0.1 at the sidecar's
/// port, by parsed host and port, not by prefix — which decides whether it
/// gets the shared secret.
fn is_sidecar_url(url: &str, sidecar_port: Option<u16>) -> bool {
    reqwest::Url::parse(url).is_ok_and(|u| {
        u.scheme() == "http"
            && u.host_str() == Some("127.0.0.1")
            && u.port().is_some()
            && u.port() == sidecar_port
    })
}

/// Only an absolute path (or none) may be appended to the sidecar's
/// authority: `@evil.example/x` would turn `127.0.0.1:{port}` into userinfo
/// and make evil.example the host; `//host/x` reads as a network path.
fn check_request_path(path: &str) -> Result<(), String> {
    if path.is_empty() || (path.starts_with('/') && !path.starts_with("//")) {
        Ok(())
    } else {
        Err(format!("invalid request path: {:?}", path))
    }
}

/// Whether a path addresses the cloud contract. Judged on the parsed path,
/// percent-decoded, case-folded, with empty segments and `;` parameters
/// dropped, so a query, dot segments, case, `%2F`, `//` or `;x` can't dodge
/// the loud refusal.
fn is_cloud_api_path(path: &str) -> bool {
    let Ok(url) = reqwest::Url::parse(&format!("http://127.0.0.1{}", path)) else {
        return false;
    };
    let decoded = percent_decode_str(url.path()).decode_utf8_lossy().to_ascii_lowercase();
    let mut segments = decoded
        .split('/')
        .filter(|s| !s.is_empty())
        .map(|s| s.split(';').next().unwrap_or_default());
    CLOUD_API_PREFIX
        .split('/')
        .filter(|s| !s.is_empty())
        .all(|prefix| segments.next() == Some(prefix))
}

/// Where an api_call goes, with the sidecar port its URL was built from, so
/// the credentials decision reads that same port rather than re-reading one
/// a respawn may have changed in between.
#[derive(Debug, PartialEq)]
struct Target {
    url: String,
    sidecar_port: Option<u16>,
}

impl AppState {
    /// Route a request path to a backend URL. Ollama/BYOK: the local sidecar,
    /// and a loud refusal for the cloud contract. Premium: refused until its
    /// routing is built against the L0 payload contract
    /// (agent_docs/2026-09-22-premium-payload-contract.md). Never returns a
    /// non-loopback URL.
    fn resolve_url(&self, path: &str) -> Result<Target, String> {
        let mode = self.mode.lock().unwrap_or_else(|e| e.into_inner()).clone();
        match mode {
            BackendMode::Ollama | BackendMode::Byok if !is_cloud_api_path(path) => {
                self.sidecar_target(path)
            }
            _ => Err(PREMIUM_REQUIRED.to_string()),
        }
    }

    fn sidecar_port(&self) -> Option<u16> {
        *self.sidecar_port.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// Starts a new sidecar generation, retiring every earlier reader task.
    fn next_sidecar_generation(&self) -> u64 {
        let mut generation = self.sidecar_generation.lock().unwrap_or_else(|e| e.into_inner());
        *generation += 1;
        *generation
    }

    /// Publishes or clears the sidecar port only while `generation` is
    /// current. After a respawn, the replaced sidecar's reader must neither
    /// clear the new port (its late Terminated) nor publish its own dead one.
    fn set_port_if_current(&self, generation: u64, port: Option<u16>) -> bool {
        let current = self.sidecar_generation.lock().unwrap_or_else(|e| e.into_inner());
        if *current != generation {
            return false;
        }
        *self.sidecar_port.lock().unwrap_or_else(|e| e.into_inner()) = port;
        true
    }

    /// Build a URL against the local sidecar only, ignoring backend mode —
    /// for operations like document extraction that never go to the cloud.
    fn sidecar_url(&self, path: &str) -> Result<String, String> {
        self.sidecar_target(path).map(|target| target.url)
    }

    fn sidecar_target(&self, path: &str) -> Result<Target, String> {
        check_request_path(path)?;
        let port = self.sidecar_port().ok_or("sidecar not running")?;
        Ok(Target {
            url: format!("http://127.0.0.1:{}{}", port, path),
            sidecar_port: Some(port),
        })
    }
}

// --- Path validation ---

fn validate_user_path(path: &str) -> Result<PathBuf, String> {
    let canonical = std::fs::canonicalize(path).map_err(|e| format!("invalid path: {}", e))?;
    let home = dirs::home_dir().ok_or_else(|| "cannot determine home directory".to_string())?;
    let temp = std::env::temp_dir();
    let allowed_roots: Vec<PathBuf> = vec![home, temp];
    let allowed = allowed_roots
        .iter()
        .any(|root| canonical.starts_with(std::fs::canonicalize(root).unwrap_or(root.clone())));
    if !allowed {
        return Err(format!(
            "path {} is outside allowed directories",
            canonical.display()
        ));
    }
    Ok(canonical)
}

// --- Filesystem commands (existing) ---

#[derive(serde::Serialize)]
struct FileInfo {
    pub path: String,
    pub size: u64,
    pub modified: String,
}

#[tauri::command]
fn scan_folder(path: String) -> Result<Vec<FileInfo>, String> {
    let validated = validate_user_path(&path)?;
    let mut files = Vec::new();
    for entry in WalkDir::new(validated).max_depth(10).into_iter().filter_map(|e| e.ok()) {
        let metadata = entry.metadata().map_err(|e| e.to_string())?;
        if metadata.is_file() {
            let size = metadata.len();
            let modified = metadata
                .modified()
                .map_err(|e| e.to_string())
                .and_then(|time| {
                    let datetime: chrono::DateTime<chrono::Local> = time.into();
                    Ok(datetime.format("%Y-%m-%d %H:%M:%S").to_string())
                })
                .unwrap_or_else(|_| "unknown".to_string());
            files.push(FileInfo {
                path: entry.path().display().to_string(),
                size,
                modified,
            });
            if files.len() >= 10_000 {
                break;
            }
        }
    }
    Ok(files)
}

#[tauri::command]
fn move_to_trash(path: String) -> Result<(), String> {
    let validated = validate_user_path(&path)?;
    delete(&validated).map_err(|e| e.to_string())
}

// --- API proxy ---

/// The host's one HTTP client. `no_proxy()` turns off env and system proxies, so
/// loopback traffic (document text, the shared secret, the bearer token) never
/// transits a configured proxy (#73). Cloud calls go direct too.
pub fn http_client() -> Client {
    Client::builder()
        .no_proxy()
        .timeout(Duration::from_secs(30))
        .build()
        .expect("failed to build HTTP client")
}

async fn make_http_request(
    http: &Client,
    url: &str,
    method: &str,
    body: Option<serde_json::Value>,
    params: Option<serde_json::Value>,
    auth_token: Option<&str>,
    secret_header: Option<&str>,
) -> Result<serde_json::Value, String> {
    let mut builder = match method.to_uppercase().as_str() {
        "GET" => http.get(url),
        "POST" => http.post(url),
        "PUT" => http.put(url),
        "PATCH" => http.patch(url),
        "DELETE" => http.delete(url),
        other => return Err(format!("unsupported HTTP method: {}", other)),
    };

    if let Some(token) = auth_token {
        builder = builder.bearer_auth(token);
    }

    if let Some(secret) = secret_header {
        builder = builder.header(SECRET_HEADER, secret);
    }

    if let Some(serde_json::Value::Object(map)) = params {
        let pairs: Vec<(String, String)> = map
            .into_iter()
            .filter_map(|(k, v)| {
                let s = match v {
                    serde_json::Value::String(s) => s,
                    other => other.to_string(),
                };
                Some((k, s))
            })
            .collect();
        builder = builder.query(&pairs);
    }

    if let Some(data) = body {
        builder = builder.json(&data);
    }

    let response = builder.send().await.map_err(|e| e.to_string())?;
    let status = response.status();

    if status.is_success() {
        let text = response.text().await.map_err(|e| e.to_string())?;
        if text.is_empty() {
            return Ok(serde_json::json!({}));
        }
        serde_json::from_str(&text)
            .map_err(|e| format!("failed to parse response as JSON from {}: {}", url, e))
    } else {
        let text = response.text().await.unwrap_or_default();
        Err(format!("API error {}: {}", status.as_u16(), text))
    }
}

#[tauri::command]
async fn api_call(
    state: State<'_, AppState>,
    method: String,
    path: String,
    body: Option<serde_json::Value>,
    params: Option<serde_json::Value>,
) -> Result<serde_json::Value, String> {
    let target = state.resolve_url(&path)?;
    let (token, secret) = request_credentials(&target, &state.secret, || {
        auth_get_token_inner()
            .map_err(|e| eprintln!("[keychain] failed to read auth token: {}", e))
            .ok()
    });
    make_http_request(&state.http, &target.url, &method, body, params, token.as_deref(), secret).await
}

/// The credentials a request carries: the shared secret for our own sidecar,
/// the premium bearer token for anything else — never both, and the keychain
/// is not read at all for a sidecar-bound request (#72).
fn request_credentials<'a>(
    target: &Target,
    secret: &'a str,
    read_token: impl FnOnce() -> Option<String>,
) -> (Option<String>, Option<&'a str>) {
    if is_sidecar_url(&target.url, target.sidecar_port) {
        (None, Some(secret))
    } else {
        (read_token(), None)
    }
}

const EXTRACT_FILENAME_HEADER: &str = "X-Rumble-Filename";
/// Parsing a large document can outlast the client's default 30s timeout while
/// the sidecar is still working, so /extract gets its own, longer budget.
const EXTRACT_TIMEOUT_SECS: u64 = 300;
const EXTRACT_TIMEOUT_MESSAGE: &str =
    "Extraction is taking too long. Try a smaller document, or split it into parts.";
/// Must equal MAX_UPLOAD_MB in sidecar/routes/extract.py and src/api/sidecar.ts;
/// a sidecar test holds the three in step.
const MAX_UPLOAD_MB: usize = 50;
const MAX_UPLOAD_BYTES: usize = MAX_UPLOAD_MB * 1024 * 1024;

/// Posts a file's bytes to the sidecar's /extract endpoint as multipart form data.
///
/// Maps a 422 (the sidecar's ExtractionError response) or 413 (over the upload
/// limit) to its `detail` message — safe to show the user, since the sidecar only
/// ever puts a human-readable explanation there. A timeout says so rather than
/// claiming the sidecar is unreachable. Any other failure (connection error, 5xx,
/// unreadable body) collapses to a generic message; we never surface a raw
/// response body from an unexpected failure mode.
async fn post_extract_multipart(
    http: &Client,
    sidecar_url: &str,
    secret: &str,
    filename: &str,
    data: Vec<u8>,
    timeout: Duration,
) -> Result<String, String> {
    // The sidecar would answer 413; refusing here spares building and sending the form.
    if data.len() > MAX_UPLOAD_BYTES {
        return Err(format!(
            "This file is larger than the {MAX_UPLOAD_MB} MB upload limit."
        ));
    }
    let part = reqwest::multipart::Part::bytes(data).file_name(filename.to_string());
    let form = reqwest::multipart::Form::new().part("file", part);

    let response = http
        .post(format!("{}/extract", sidecar_url))
        .header(SECRET_HEADER, secret)
        .multipart(form)
        .timeout(timeout)
        .send()
        .await
        .map_err(|e| {
            if e.is_timeout() {
                EXTRACT_TIMEOUT_MESSAGE.to_string()
            } else {
                "could not reach the sidecar".to_string()
            }
        })?;

    let status = response.status();

    if status.is_success() {
        let value: serde_json::Value = response
            .json()
            .await
            .map_err(|_| "extraction failed".to_string())?;
        return value
            .get("text")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .ok_or_else(|| "extraction failed".to_string());
    }

    if matches!(status.as_u16(), 413 | 422) {
        let value: serde_json::Value = response
            .json()
            .await
            .map_err(|_| "extraction failed".to_string())?;
        let detail = value
            .get("detail")
            .and_then(|v| v.as_str())
            .unwrap_or("extraction failed");
        return Err(detail.to_string());
    }

    Err("extraction failed".to_string())
}

/// Header values must be Latin-1, so a non-ASCII filename is percent-encoded
/// on the way in (see `extractDocument` in src/api/sidecar.ts); decode it back.
fn decode_filename(raw: &str) -> Result<String, String> {
    percent_decode_str(raw)
        .decode_utf8()
        .map(|s| s.into_owned())
        .map_err(|_| "invalid filename encoding".to_string())
}

#[tauri::command]
async fn extract_document(
    state: State<'_, AppState>,
    request: tauri::ipc::Request<'_>,
) -> Result<String, String> {
    let raw_filename = request
        .headers()
        .get(EXTRACT_FILENAME_HEADER)
        .and_then(|v| v.to_str().ok())
        .ok_or_else(|| "missing filename header".to_string())?;
    let filename = decode_filename(raw_filename)?;

    let data = match request.body() {
        tauri::ipc::InvokeBody::Raw(bytes) => bytes.clone(),
        _ => return Err("expected a raw file body".to_string()),
    };

    let sidecar_url = state.sidecar_url("")?;
    post_extract_multipart(
        &state.http,
        &sidecar_url,
        &state.secret,
        &filename,
        data,
        Duration::from_secs(EXTRACT_TIMEOUT_SECS),
    )
    .await
}

// --- Chat streaming ---

/// How long a chat stream may go without sending anything before it counts
/// as stalled. The first token after a whole document's prompt evaluation on
/// a CPU can be slow; the sidecar sends a `status` event straight away.
const STREAM_IDLE_TIMEOUT_SECS: u64 = 300;
/// Overall safety cap on one stream. It must override the client's 30s total
/// budget, which would cut a long local generation off mid-stream.
const STREAM_MAX_SECS: u64 = 3600;
const STREAM_IDLE_MESSAGE: &str = "The model stopped responding.";

struct StreamTimeouts {
    idle: Duration,
    total: Duration,
}

const STREAM_TIMEOUTS: StreamTimeouts = StreamTimeouts {
    idle: Duration::from_secs(STREAM_IDLE_TIMEOUT_SECS),
    total: Duration::from_secs(STREAM_MAX_SECS),
};

/// One event of a chat stream, as the frontend receives it over the channel.
/// Exactly one `Done` or `Error` ends every stream.
#[derive(Clone, Debug, PartialEq, serde::Serialize)]
#[serde(tag = "type", content = "value", rename_all = "lowercase")]
enum StreamEvent {
    Delta(String),
    Done(String),
    Error(String),
}

/// Accumulates SSE bytes and yields the payloads of complete `data:` lines.
/// Only complete lines are decoded, so a UTF-8 character split across two
/// network chunks is never cut in half.
#[derive(Default)]
struct SseLines {
    buffer: Vec<u8>,
}

impl SseLines {
    fn push(&mut self, bytes: &[u8]) -> Vec<String> {
        self.buffer.extend_from_slice(bytes);
        let mut payloads = Vec::new();
        while let Some(end) = self.buffer.iter().position(|b| *b == b'\n') {
            let line: Vec<u8> = self.buffer.drain(..=end).collect();
            // sse-starlette ends lines with \r\n; trim covers both.
            let line = String::from_utf8_lossy(&line);
            if let Some(payload) = line.trim().strip_prefix("data:") {
                let payload = payload.trim();
                if !payload.is_empty() {
                    payloads.push(payload.to_string());
                }
            }
        }
        payloads
    }
}

/// Posts to the sidecar's SSE stream endpoint and relays each text delta to
/// `on_delta`, in order. Returns the complete text from the sidecar's `done`
/// event, or the message from its `error` event. The shared secret is
/// attached here, host-side; the webview never sees it (#55). Silence longer
/// than `timeouts.idle`, before the headers or between chunks, ends the stream
/// with STREAM_IDLE_MESSAGE.
async fn relay_sse(
    http: &Client,
    url: &str,
    secret: &str,
    body: serde_json::Value,
    timeouts: &StreamTimeouts,
    mut on_delta: impl FnMut(String) -> Result<(), String>,
) -> Result<String, String> {
    let request = http
        .post(url)
        .header(SECRET_HEADER, secret)
        .json(&body)
        .timeout(timeouts.total)
        .send();
    let mut response = tokio::time::timeout(timeouts.idle, request)
        .await
        .map_err(|_| STREAM_IDLE_MESSAGE.to_string())?
        .map_err(|e| e.to_string())?;

    let status = response.status();
    if !status.is_success() {
        let text = response.text().await.unwrap_or_default();
        return Err(format!("Stream request failed ({}): {}", status.as_u16(), text));
    }

    let mut lines = SseLines::default();
    while let Some(chunk) = tokio::time::timeout(timeouts.idle, response.chunk())
        .await
        .map_err(|_| STREAM_IDLE_MESSAGE.to_string())?
        .map_err(|e| e.to_string())?
    {
        for payload in lines.push(&chunk) {
            // Non-JSON data lines are skipped, as the sidecar never sends them.
            let Ok(event) = serde_json::from_str::<serde_json::Value>(&payload) else {
                continue;
            };
            let value = event.get("value").and_then(|v| v.as_str()).unwrap_or_default();
            match event.get("type").and_then(|t| t.as_str()) {
                Some("delta") => on_delta(value.to_string())?,
                Some("done") => return Ok(value.to_string()),
                Some("error") => return Err(value.to_string()),
                _ => {}
            }
        }
    }
    Err("stream ended before completion".to_string())
}

/// Streams a chat reply from the sidecar through `on_event`. Tauri delivers
/// channel messages in order, but not in order with this command's own
/// response (a payload of 8 KB or more travels by a separate fetch), so the
/// frontend settles on the channel's terminal event rather than on this
/// command returning — and every path, failures included, sends exactly one.
#[tauri::command]
async fn stream_message(
    state: State<'_, AppState>,
    chat_id: String,
    text: String,
    model: Option<String>,
    on_event: Channel<StreamEvent>,
) -> Result<(), String> {
    stream_chat(&state, chat_id, text, model, &on_event).await
}

async fn stream_chat(
    state: &AppState,
    chat_id: String,
    text: String,
    model: Option<String>,
    on_event: &Channel<StreamEvent>,
) -> Result<(), String> {
    let outcome = async {
        // Chat is a sidecar feature in local/BYOK; premium chat waits on the L0
        // payload contract like every other cloud call (same rule as resolve_url).
        if *state.mode.lock().unwrap_or_else(|e| e.into_inner()) == BackendMode::Premium {
            return Err(PREMIUM_REQUIRED.to_string());
        }
        let chat_id = Uuid::parse_str(&chat_id).map_err(|_| "invalid chat id".to_string())?;
        let url = state.sidecar_url(&format!("/chats/{}/stream", chat_id))?;
        let body = serde_json::json!({ "text": text, "model": model });
        relay_sse(&state.http, &url, &state.secret, body, &STREAM_TIMEOUTS, |delta| {
            on_event.send(StreamEvent::Delta(delta)).map_err(|e| e.to_string())
        })
        .await
    }
    .await;

    let terminal = match outcome {
        Ok(full_text) => StreamEvent::Done(full_text),
        Err(message) => StreamEvent::Error(message),
    };
    on_event.send(terminal).map_err(|e| e.to_string())
}

// --- Keychain helpers ---

fn keyring_entry(key: &str) -> Result<keyring::Entry, String> {
    keyring::Entry::new(SERVICE_NAME, key).map_err(|e| e.to_string())
}

fn auth_get_token_inner() -> Result<String, String> {
    keyring_entry("auth_token")?
        .get_password()
        .map_err(|e| e.to_string())
}

// --- Keychain commands ---

#[tauri::command]
fn auth_store_token(token: String) -> Result<(), String> {
    keyring_entry("auth_token")?
        .set_password(&token)
        .map_err(|e| e.to_string())
}

#[tauri::command]
fn auth_get_token() -> Result<Option<String>, String> {
    match keyring_entry("auth_token")?.get_password() {
        Ok(t) => Ok(Some(t)),
        Err(keyring::Error::NoEntry) => Ok(None),
        Err(e) => Err(e.to_string()),
    }
}

#[tauri::command]
fn auth_clear_token() -> Result<(), String> {
    match keyring_entry("auth_token")?.delete_credential() {
        Ok(()) => Ok(()),
        Err(keyring::Error::NoEntry) => Ok(()),
        Err(e) => Err(e.to_string()),
    }
}

const ALLOWED_PROVIDERS: &[&str] = &["elefant-local", "openai", "anthropic"];

fn validate_provider(provider: &str) -> Result<(), String> {
    if !ALLOWED_PROVIDERS.contains(&provider) {
        return Err(format!("unknown provider: {}", provider));
    }
    Ok(())
}

/// Sync, like set_backend_mode: a key change in byok mode respawns the sidecar.
#[tauri::command]
fn store_api_key(app: AppHandle, state: State<'_, AppState>, provider: String, key: String) -> Result<(), String> {
    validate_provider(&provider)?;
    let entry_name = format!("byok_{}", provider);
    keyring_entry(&entry_name)?
        .set_password(&key)
        .map_err(|e| e.to_string())?;
    after_key_change(&state, &provider, KeyChange::Stored, || respawn_or_stop(&app))
}

#[tauri::command]
fn get_api_key(provider: String) -> Result<Option<String>, String> {
    validate_provider(&provider)?;
    let entry_name = format!("byok_{}", provider);
    match keyring_entry(&entry_name)?.get_password() {
        Ok(k) => Ok(Some(k)),
        Err(keyring::Error::NoEntry) => Ok(None),
        Err(e) => Err(e.to_string()),
    }
}

#[tauri::command]
fn delete_api_key(app: AppHandle, state: State<'_, AppState>, provider: String) -> Result<(), String> {
    validate_provider(&provider)?;
    let entry_name = format!("byok_{}", provider);
    match keyring_entry(&entry_name)?.delete_credential() {
        Ok(()) => {}
        Err(keyring::Error::NoEntry) => {}
        Err(e) => return Err(e.to_string()),
    }
    after_key_change(&state, &provider, KeyChange::Deleted, || respawn_or_stop(&app))
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum KeyChange {
    Stored,
    Deleted,
}

/// A byok sidecar holds the key it was spawned with, so a change to that key
/// must reach it: a stored key respawns it; a deleted key drops the host to
/// ollama — failing safe, never back to the old key — and respawns. Any
/// other provider, or any other mode, leaves the sidecar alone.
fn after_key_change(
    state: &AppState,
    provider: &str,
    change: KeyChange,
    respawn: impl FnOnce() -> Result<(), String>,
) -> Result<(), String> {
    let mut mode = state.mode.lock().unwrap_or_else(|e| e.into_inner());
    if provider != BYOK_KEY_PROVIDER || *mode != BackendMode::Byok {
        return Ok(());
    }
    if change == KeyChange::Deleted {
        *mode = BackendMode::Ollama;
    }
    drop(mode);
    let done = match change {
        KeyChange::Stored => "Key saved",
        KeyChange::Deleted => "Key removed",
    };
    respawn().map_err(|e| format!("{}, but the local AI service didn't restart: {}", done, e))
}

/// Respawns the sidecar; if that fails, stops the running one rather than
/// leave it on a key that was changed or deleted.
fn respawn_or_stop(app: &AppHandle) -> Result<(), String> {
    spawn_sidecar(app).inspect_err(|_| kill_sidecar(&app.state::<AppState>()))
}

// --- Backend mode commands ---

#[tauri::command]
fn get_backend_mode(state: State<'_, AppState>) -> BackendMode {
    let mode = state.mode.lock().unwrap_or_else(|e| e.into_inner());
    mode.clone()
}

/// Sync, not async: spawn_sidecar uses block_on, which must not run inside
/// the async runtime.
#[tauri::command]
/// Ok(true) when the mode changed and the sidecar is restarting, Ok(false)
/// when it was already in that mode and nothing happened.
fn set_backend_mode(app: AppHandle, state: State<'_, AppState>, mode: String) -> Result<bool, String> {
    apply_mode(&state, parse_mode(&mode)?, || spawn_sidecar(&app))
}

fn parse_mode(mode: &str) -> Result<BackendMode, String> {
    match mode {
        "ollama" => Ok(BackendMode::Ollama),
        "byok" => Ok(BackendMode::Byok),
        "premium" => Ok(BackendMode::Premium),
        other => Err(format!("unknown backend mode: {}", other)),
    }
}

/// Switches the host's mode and respawns the sidecar, which reads its mode
/// only from its spawn environment (decision on PR #66). A failed respawn
/// restores the previous mode: spawn_sidecar fails before it kills the running
/// sidecar, so host and sidecar still agree. Ok(false): already in that mode.
fn apply_mode(
    state: &AppState,
    requested: BackendMode,
    respawn: impl FnOnce() -> Result<(), String>,
) -> Result<bool, String> {
    let previous = {
        let mut current = state.mode.lock().unwrap_or_else(|e| e.into_inner());
        if *current == requested {
            return Ok(false);
        }
        std::mem::replace(&mut *current, requested)
    };
    respawn().map(|()| true).map_err(|e| {
        *state.mode.lock().unwrap_or_else(|e| e.into_inner()) = previous;
        e
    })
}

/// The sidecar's mode env. "premium" never reaches the sidecar; the BYOK key
/// is present only in byok mode. BYOK_API_KEY is always set, to "" outside
/// byok, because the shell Command inherits the host's environment and has no
/// env_remove: a key in the launching shell must not reach an ollama sidecar.
fn sidecar_mode_env(
    mode: &BackendMode,
    byok_key: Option<String>,
) -> Result<Vec<(&'static str, String)>, String> {
    match mode {
        BackendMode::Byok => {
            let key = byok_key.filter(|k| !k.is_empty()).ok_or(BYOK_KEY_MISSING)?;
            Ok(vec![
                ("RUMBLE_BACKEND_MODE", "byok".to_string()),
                ("BYOK_API_KEY", key),
            ])
        }
        BackendMode::Ollama | BackendMode::Premium => Ok(vec![
            ("RUMBLE_BACKEND_MODE", "ollama".to_string()),
            ("BYOK_API_KEY", String::new()),
        ]),
    }
}

// --- Sidecar lifecycle ---

/// TOCTOU: port may be reclaimed between bind and sidecar start.
/// Low risk — the sidecar's `PORT:` protocol confirms the actual port used.
async fn find_available_port() -> Result<u16, String> {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .map_err(|e| format!("failed to bind for port discovery: {}", e))?;
    let port = listener
        .local_addr()
        .map_err(|e| format!("failed to get local addr: {}", e))?
        .port();
    drop(listener);
    Ok(port)
}

async fn poll_health(http: &Client, port: u16, secret: &str) -> Result<(), String> {
    let url = format!("http://127.0.0.1:{}/health", port);
    for attempt in 1..=HEALTH_POLL_ATTEMPTS {
        tokio::time::sleep(std::time::Duration::from_millis(HEALTH_POLL_INTERVAL_MS)).await;
        match http.get(&url).header(SECRET_HEADER, secret).send().await {
            Ok(resp) if resp.status().is_success() => {
                println!("[sidecar] healthy on port {} (attempt {})", port, attempt);
                return Ok(());
            }
            _ => {
                if attempt == HEALTH_POLL_ATTEMPTS {
                    return Err(format!(
                        "sidecar health check failed after {} attempts on port {}",
                        HEALTH_POLL_ATTEMPTS, port
                    ));
                }
            }
        }
    }
    unreachable!()
}

fn spawn_sidecar(app: &AppHandle) -> Result<(), String> {
    let state = app.state::<AppState>();

    let mode = state.mode.lock().unwrap_or_else(|e| e.into_inner()).clone();
    let byok_key = match mode {
        BackendMode::Byok => get_api_key(BYOK_KEY_PROVIDER.to_string())?,
        _ => None,
    };
    let mode_env = sidecar_mode_env(&mode, byok_key)?;

    // Find a free port synchronously via tauri's async runtime
    let port = tauri::async_runtime::block_on(find_available_port())?;

    let data_dir = sidecar_data_dir(app)?;
    std::fs::create_dir_all(&data_dir)
        .map_err(|e| format!("failed to create app data dir: {}", e))?;

    let sidecar_cmd = app
        .shell()
        .sidecar("rumble-sidecar")
        .map_err(|e| format!("failed to create sidecar command: {}", e))?
        .args([
            "--port",
            &port.to_string(),
            "--data-dir",
            &data_dir.to_string_lossy(),
        ])
        // Passed via the environment rather than argv: command-line arguments
        // are visible to any process on the machine via `ps`, which would
        // defeat the point of the secret. `--secret` stays available for
        // manual dev runs.
        .env("RUMBLE_SIDECAR_SECRET", &state.secret)
        // Only the real spawn path sets this. `pnpm dev:sidecar` and pytest never
        // do, so a closed/redirected stdin there can't be mistaken for the host
        // dying.
        .env("RUMBLE_SIDECAR_WATCH_STDIN", "1")
        .envs(mode_env);

    let (mut rx, child) = sidecar_cmd
        .spawn()
        .map_err(|e| format!("failed to spawn sidecar: {}", e))?;

    // Retire the old sidecar's reader before killing it, so it can't publish
    // or clear the port once this one owns it.
    let generation = state.next_sidecar_generation();
    // Kill any existing sidecar before spawning a new one
    kill_sidecar(&state);

    // Store the child process handle for cleanup
    {
        let mut sidecar_child = state.sidecar_child.lock().unwrap_or_else(|e| e.into_inner());
        *sidecar_child = Some(child);
    }

    // Read stdout in a background task, looking for PORT: confirmation
    let port_clone = port;
    let http_clone = state.http.clone();
    let secret_clone = state.secret.clone();
    let state_handle = app.app_handle().clone();
    tauri::async_runtime::spawn(async move {
        let mut port_confirmed = false;
        while let Some(event) = rx.recv().await {
            match event {
                CommandEvent::Stdout(line_bytes) => {
                    let line = String::from_utf8_lossy(&line_bytes);
                    let trimmed = line.trim();
                    if !port_confirmed {
                        if let Some(port_str) = trimmed.strip_prefix("PORT:") {
                            if let Ok(p) = port_str.parse::<u16>() {
                                println!("[sidecar] reported port {}", p);
                                match poll_health(&http_clone, p, &secret_clone).await {
                                    Ok(()) => {
                                        let st = state_handle.state::<AppState>();
                                        if st.set_port_if_current(generation, Some(p)) {
                                            println!("[sidecar] ready on port {}", p);
                                        }
                                        port_confirmed = true;
                                    }
                                    Err(e) => {
                                        eprintln!("[sidecar] health check failed: {}", e);
                                    }
                                }
                            }
                        }
                    }
                }
                CommandEvent::Stderr(line_bytes) => {
                    let line = String::from_utf8_lossy(&line_bytes);
                    eprintln!("[sidecar:err] {}", line.trim());
                }
                CommandEvent::Terminated(payload) => {
                    println!(
                        "[sidecar] terminated with code {:?}",
                        payload.code
                    );
                    state_handle.state::<AppState>().set_port_if_current(generation, None);
                    break;
                }
                _ => {}
            }
        }
    });

    println!("[sidecar] spawned, waiting for port {} to become healthy", port_clone);
    Ok(())
}

fn kill_sidecar(state: &AppState) {
    let mut child = state.sidecar_child.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(c) = child.take() {
        println!("[sidecar] shutting down");
        let _ = c.kill();
    }
    let mut port = state.sidecar_port.lock().unwrap_or_else(|e| e.into_inner());
    *port = None;
}

// --- Local data lifecycle ---

/// The sidecar's SQLite files, placed in this order by a migration so that
/// `rumble.db` arriving last means the copy is complete.
const DB_FILES: &[&str] = &["rumble.db-wal", "rumble.db-shm", "rumble.db"];
const DB_MAIN_FILE: &str = "rumble.db";
/// Without `rumble.db` beside them, these in the new dir can only be a failed migration's.
const DB_SIDE_FILES: &[&str] = &["rumble.db-wal", "rumble.db-shm"];
/// Written by the sidecar once its database is open, removed after it closes (app.py lifespan).
const SIDECAR_PIDFILE: &str = "sidecar.pid";
/// In the names of a migration's copies until they are renamed into place.
const MIGRATION_TEMP_MARKER: &str = ".migrating-";
const SIDECAR_STOP_TIMEOUT: Duration = Duration::from_secs(10);
const SIDECAR_STOP_POLL: Duration = Duration::from_millis(100);
const SIDECAR_STOP_TIMEOUT_MESSAGE: &str =
    "Rumble couldn't stop its local service. Quit and reopen Rumble, then try again.";
const SIDECAR_STARTING_MESSAGE: &str = "Rumble's local service is still starting. Try again in a moment.";
const SIDECAR_PIDFILE_MISSING_MESSAGE: &str = "Another Rumble instance may be using your local data. \
     Close it, restart Rumble, then try again.";
const SIDECAR_RESTART_FAILED_MESSAGE: &str =
    "Your local data was deleted, but Rumble's local service didn't restart. Quit and reopen Rumble.";

#[derive(Debug, PartialEq)]
enum Migration {
    /// Same directory (macOS, Linux), or nothing at the old one.
    NotNeeded,
    /// Both directories hold a database: left alone, never merged.
    BothPresent,
    /// `files` now live in the new dir; `left_behind` couldn't be removed from the old one.
    Copied { files: Vec<&'static str>, left_behind: Vec<String> },
}

/// Copies the sidecar's database from `old` to `new` when `old` has one and `new`
/// doesn't, and only once the copy is complete removes it from `old`: until then
/// `old` is never touched, so any failure leaves it whole for the sidecar to use.
/// Keyed on the database file, not the directory: on Windows WebView2 creates its
/// own folder under the local data dir before setup runs.
fn migrate_data_dir(old: &Path, new: &Path) -> Result<Migration, String> {
    if old == new || !old.join(DB_MAIN_FILE).is_file() {
        return Ok(Migration::NotNeeded);
    }
    if new.join(DB_MAIN_FILE).is_file() {
        return Ok(Migration::BothPresent);
    }
    std::fs::create_dir_all(new).map_err(|e| format!("failed to create {}: {}", new.display(), e))?;
    // A WAL left here by an earlier failed run would be replayed over the database we copy in.
    for name in DB_SIDE_FILES {
        remove_if_present(&new.join(name))?;
    }
    remove_migration_temps(new)?;

    let files: Vec<&'static str> = DB_FILES.iter().copied().filter(|n| old.join(n).is_file()).collect();
    if let Err(e) = copy_into_place(old, new, &files) {
        let _ = remove_migration_temps(new);
        for name in DB_SIDE_FILES {
            let _ = remove_if_present(&new.join(name));
        }
        return Err(e);
    }

    // The database first: once it is gone the old dir can never be migrated again.
    let mut left_behind = Vec::new();
    if let Err(e) = remove_if_present(&old.join(DB_MAIN_FILE)) {
        left_behind.push(e);
    } else {
        for name in DB_SIDE_FILES.iter().chain(std::iter::once(&SIDECAR_PIDFILE)) {
            if let Err(e) = remove_if_present(&old.join(name)) {
                left_behind.push(e);
            }
        }
    }
    Ok(Migration::Copied { files, left_behind })
}

/// Copies every file to a temp name in `new` (a copy works across volumes; the roaming
/// profile can be redirected), then renames each into place in DB_FILES order.
fn copy_into_place(old: &Path, new: &Path, files: &[&str]) -> Result<(), String> {
    let mut temps = Vec::new();
    for name in files {
        let temp = new.join(format!("{}{}{}", name, MIGRATION_TEMP_MARKER, Uuid::new_v4()));
        std::fs::copy(old.join(name), &temp)
            .map_err(|e| format!("failed to copy {}: {}", old.join(name).display(), e))?;
        temps.push((temp, new.join(name)));
    }
    for (temp, dest) in temps {
        std::fs::rename(&temp, &dest).map_err(|e| format!("failed to place {}: {}", dest.display(), e))?;
    }
    Ok(())
}

fn remove_if_present(path: &Path) -> Result<(), String> {
    match std::fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(format!("failed to delete {}: {}", path.display(), e)),
    }
}

fn remove_migration_temps(dir: &Path) -> Result<(), String> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Ok(());
    };
    for entry in entries.flatten() {
        if entry.file_name().to_string_lossy().contains(MIGRATION_TEMP_MARKER) {
            remove_if_present(&entry.path())?;
        }
    }
    Ok(())
}

/// The directory a sidecar spawned now runs against: the old one only while a
/// migration out of it has not succeeded.
fn active_data_dir(old: &Path, new: &Path) -> PathBuf {
    if old != new && old.join(DB_MAIN_FILE).is_file() && !new.join(DB_MAIN_FILE).is_file() {
        old.to_path_buf()
    } else {
        new.to_path_buf()
    }
}

/// The data directory the sidecar uses: the local (non-roaming) app data dir,
/// after migrating a database left in the roaming one by earlier builds (#57).
/// If the migration fails, the old directory is used so the data isn't orphaned.
fn sidecar_data_dir(app: &AppHandle) -> Result<PathBuf, String> {
    let (old, new) = data_dirs(app)?;
    match migrate_data_dir(&old, &new) {
        Ok(Migration::NotNeeded) => {}
        Ok(Migration::Copied { files, left_behind }) => {
            println!("[data] migrated {:?} from {} to {}", files, old.display(), new.display());
            for e in left_behind {
                eprintln!("[data] the old copy stays in {}: {}", old.display(), e);
            }
        }
        Ok(Migration::BothPresent) => eprintln!(
            "[data] databases found in both {} and {}; using the latter, leaving the former untouched",
            old.display(),
            new.display()
        ),
        Err(e) => eprintln!("[data] migration failed, staying in {}: {}", old.display(), e),
    }
    Ok(active_data_dir(&old, &new))
}

/// (roaming app data dir used by earlier builds, local app data dir used now)
fn data_dirs(app: &AppHandle) -> Result<(PathBuf, PathBuf), String> {
    let old = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("failed to resolve app data dir: {}", e))?;
    let new = app
        .path()
        .app_local_data_dir()
        .map_err(|e| format!("failed to resolve app local data dir: {}", e))?;
    Ok((old, new))
}

/// Removes the sidecar's database files, pidfile and any migration temps from `dir`;
/// other files are left alone.
fn delete_local_data_files(dir: &Path) -> Result<(), String> {
    for name in DB_FILES.iter().chain(std::iter::once(&SIDECAR_PIDFILE)) {
        remove_if_present(&dir.join(name))?;
    }
    remove_migration_temps(dir)
}

/// Waits up to `timeout` for the sidecar to remove its pidfile, which it does only
/// after closing the database.
async fn wait_for_sidecar_exit(pidfile: &Path, timeout: Duration) -> bool {
    let deadline = tokio::time::Instant::now() + timeout;
    while pidfile.exists() {
        if tokio::time::Instant::now() >= deadline {
            return false;
        }
        tokio::time::sleep(SIDECAR_STOP_POLL).await;
    }
    true
}

/// Whether delete-all may stop the sidecar: only once it is running (`port_known`: the
/// host records the port after PORT:, which follows the pidfile write) and its pidfile
/// is there to wait on. A missing pidfile beside a running sidecar means another
/// instance on this data dir overwrote it and then removed it on exit, so its absence
/// would no longer mean "database closed".
fn check_ready_to_delete(port_known: bool, pidfile: &Path) -> Result<(), String> {
    if !port_known {
        return Err(SIDECAR_STARTING_MESSAGE.to_string());
    }
    if !pidfile.is_file() {
        return Err(SIDECAR_PIDFILE_MISSING_MESSAGE.to_string());
    }
    Ok(())
}

/// Deletes the local data in `dirs` once the sidecar has let go of `pidfile`, and
/// nothing at all if it doesn't within `timeout`: deleting under a live SQLite
/// connection could let its close unlink the next sidecar's WAL by path.
async fn delete_after_sidecar_exit(pidfile: &Path, dirs: &[&Path], timeout: Duration) -> Result<(), String> {
    if !wait_for_sidecar_exit(pidfile, timeout).await {
        return Err(SIDECAR_STOP_TIMEOUT_MESSAGE.to_string());
    }
    for dir in dirs {
        delete_local_data_files(dir)?;
    }
    Ok(())
}

/// Returns Ok(true) once chats, messages and jobs are deleted, Ok(false) when the
/// user cancelled the confirmation (nothing is touched).
///
/// Assumes one Rumble instance per data dir: a second one (`pnpm tauri dev` beside
/// the packaged app) keeps its own connection open, which the dialog asks the user
/// to close first.
#[tauri::command]
async fn delete_all_local_data(app: AppHandle) -> Result<bool, String> {
    let confirmed = app
        .dialog()
        .message(
            "This permanently deletes every chat, message, and document review, draft and research job \
             stored by Rumble on this device, including the document text inside them.\n\n\
             Close any other Rumble windows first.\n\n\
             Not affected: API keys in the system keychain, .docx files you exported, and Ollama's models.",
        )
        .title("Delete all local data?")
        .kind(MessageDialogKind::Warning)
        .buttons(MessageDialogButtons::OkCancelCustom(
            "Delete everything".to_string(),
            "Cancel".to_string(),
        ))
        .blocking_show();
    if !confirmed {
        return Ok(false);
    }

    let state = app.state::<AppState>();
    let (old, new) = data_dirs(&app)?;
    let active = active_data_dir(&old, &new);
    let port_known = state.sidecar_port.lock().unwrap_or_else(|e| e.into_inner()).is_some();
    check_ready_to_delete(port_known, &active.join(SIDECAR_PIDFILE))?;
    kill_sidecar(&state);
    // The old dir too, so a database left there can't be migrated back in.
    let deleted = delete_after_sidecar_exit(&active.join(SIDECAR_PIDFILE), &[&new, &old], SIDECAR_STOP_TIMEOUT).await;

    // spawn_sidecar blocks on the runtime, which panics on an async worker thread.
    let handle = app.clone();
    let restarted = tauri::async_runtime::spawn_blocking(move || spawn_sidecar(&handle))
        .await
        .map_err(|e| e.to_string())
        .and_then(|r| r);

    match (deleted, restarted) {
        (Err(e), _) => Err(e),
        (Ok(()), Err(e)) => {
            eprintln!("[sidecar] failed to restart after deleting local data: {}", e);
            Err(SIDECAR_RESTART_FAILED_MESSAGE.to_string())
        }
        (Ok(()), Ok(())) => Ok(true),
    }
}

/// Returns the shared secret for the one frontend path that talks to the
/// sidecar directly (SSE streaming, which can't go through `api_call`).
/// Kept separate from `sidecar_status` so the secret doesn't appear in
/// routine status payloads that end up in devtools or log captures.
#[tauri::command]
fn sidecar_secret(state: State<'_, AppState>) -> String {
    state.secret.clone()
}

#[tauri::command]
async fn sidecar_status(state: State<'_, AppState>) -> Result<serde_json::Value, String> {
    Ok(sidecar_status_value(&state.http, state.sidecar_port(), &state.secret).await)
}

/// The status payload: whether a sidecar port is published and, if so, its
/// /health body. /health is asked with the shared secret, without which the
/// sidecar answers 401 (#69).
async fn sidecar_status_value(http: &Client, port: Option<u16>, secret: &str) -> serde_json::Value {
    let mut result = serde_json::json!({
        "running": port.is_some(),
        "port": port,
    });
    if let Some(p) = port {
        let url = format!("http://127.0.0.1:{}/health", p);
        match http.get(&url).header(SECRET_HEADER, secret).send().await {
            Ok(resp) if resp.status().is_success() => {
                if let Ok(body) = resp.json::<serde_json::Value>().await {
                    result["health"] = body;
                }
            }
            _ => {
                result["health"] = serde_json::json!({"status": "unreachable"});
            }
        }
    }
    result
}

// --- Tray icon ---

fn resolve_tray_icon(app: &AppHandle) -> Option<Image<'static>> {
    Image::from_bytes(include_bytes!("../icons/icon.png"))
        .ok()
        .or_else(|| {
            if cfg!(debug_assertions) {
                app.path()
                    .resolve("icons/icon.png", BaseDirectory::Resource)
                    .ok()
                    .and_then(|path| Image::from_path(path).ok())
            } else {
                app.default_window_icon()
                    .map(|icon| icon.clone().to_owned())
            }
        })
}

// --- Document export ---

/// Writes to `dest` atomically. `write` receives a handle to a fresh, uniquely
/// named, dot-prefixed temp file created in `dest`'s own directory; the temp
/// file is renamed onto `dest` only once `write` returns `Ok`. A rename within
/// one filesystem is atomic, so a reader never observes a partial file, and on
/// any failure the temp file is removed and `dest` (existing or not) is left
/// untouched.
fn write_atomically(
    dest: &Path,
    write: impl FnOnce(&std::fs::File) -> Result<(), String>,
) -> Result<(), String> {
    let dir = dest.parent().unwrap_or_else(|| Path::new("."));
    let temp_name = format!(
        ".{}.{}.tmp",
        dest.file_name().and_then(|n| n.to_str()).unwrap_or("export"),
        Uuid::new_v4()
    );
    let temp_path = dir.join(temp_name);

    let file =
        std::fs::File::create(&temp_path).map_err(|e| format!("failed to create temp file: {}", e))?;
    let result = write(&file);
    drop(file);

    if let Err(e) = result {
        let _ = std::fs::remove_file(&temp_path);
        return Err(e);
    }

    std::fs::rename(&temp_path, dest).map_err(|e| {
        let _ = std::fs::remove_file(&temp_path);
        format!("failed to finalize export: {}", e)
    })
}

#[tauri::command]
/// Returns Ok(true) when a file was written, Ok(false) when the user cancelled
/// the save dialog. The caller needs to tell those apart — reporting success for
/// a cancelled save means toasting "exported" for a file that never existed.
async fn export_draft_docx(app: AppHandle, text: String) -> Result<bool, String> {
    let path = app
        .dialog()
        .file()
        .add_filter("Word Document", &["docx"])
        .set_file_name("draft.docx")
        .blocking_save_file();

    let Some(path) = path else {
        // User cancelled the dialog — not an error, but not a write either.
        return Ok(false);
    };

    let path = path
        .into_path()
        .map_err(|e| format!("invalid save path: {}", e))?;

    write_atomically(&path, |file| {
        let mut docx = Docx::new();
        for line in text.lines() {
            let paragraph = if line.trim().is_empty() {
                Paragraph::new()
            } else {
                Paragraph::new().add_run(Run::new().add_text(line))
            };
            docx = docx.add_paragraph(paragraph);
        }

        docx.build()
            .pack(file)
            .map_err(|e| format!("failed to write docx: {}", e))
    })?;

    Ok(true)
}

// --- Settings ---

/// Lives in app_local_data_dir, not app_data_dir: on Windows the latter is the
/// roaming profile, and briefcase and resource names can be client-matter names.
const SETTINGS_FILE_NAME: &str = "settings.json";

type Settings = serde_json::Map<String, serde_json::Value>;

enum StoredSettings {
    Missing,
    Valid(Settings),
    /// Present but not a JSON object: kept aside on the next save, never overwritten.
    Corrupt(String),
}

fn settings_path_in(dir: &Path) -> PathBuf {
    dir.join(SETTINGS_FILE_NAME)
}

/// An unreadable file (permissions, a directory in the way) is an error; bytes
/// that don't parse as a JSON object, invalid UTF-8 included, are `Corrupt`.
fn read_stored_settings(path: &Path) -> Result<StoredSettings, String> {
    let bytes = match std::fs::read(path) {
        Ok(bytes) => bytes,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(StoredSettings::Missing),
        Err(e) => return Err(format!("failed to read {}: {}", path.display(), e)),
    };
    Ok(match serde_json::from_slice(&bytes) {
        Ok(settings) => StoredSettings::Valid(settings),
        Err(e) => StoredSettings::Corrupt(format!("failed to parse {}: {}", path.display(), e)),
    })
}

/// A missing file is "nothing saved yet". A corrupt one is an error, never a
/// silent reset to defaults.
fn read_settings(path: &Path) -> Result<Option<Settings>, String> {
    match read_stored_settings(path)? {
        StoredSettings::Missing => Ok(None),
        StoredSettings::Valid(settings) => Ok(Some(settings)),
        StoredSettings::Corrupt(e) => Err(e),
    }
}

/// Writes `incoming` over the stored file's top-level groups, so groups this
/// caller doesn't send survive. A corrupt file is renamed aside first and its
/// new path returned, so a save never destroys what it couldn't read.
fn save_settings_to(path: &Path, incoming: Settings) -> Result<Option<PathBuf>, String> {
    let context = |e: String| format!("couldn't save settings to {}: {}", path.display(), e);
    let (mut settings, aside) = match read_stored_settings(path).map_err(context)? {
        StoredSettings::Missing => (Settings::new(), None),
        StoredSettings::Valid(stored) => (stored, None),
        StoredSettings::Corrupt(_) => {
            // The uuid keeps two corrupt saves within one second from overwriting each other's aside.
            let aside = path.with_file_name(format!(
                "{}.corrupt-{}-{}",
                SETTINGS_FILE_NAME,
                chrono::Utc::now().format("%Y%m%dT%H%M%SZ"),
                Uuid::new_v4().simple()
            ));
            std::fs::rename(path, &aside)
                .map_err(|e| context(format!("couldn't move the unreadable file aside: {}", e)))?;
            (Settings::new(), Some(aside))
        }
    };
    settings.extend(incoming);
    write_settings(path, &settings)?;
    Ok(aside)
}

/// Atomic rename of an fsynced temp file; the directory itself is not fsynced.
fn write_settings(path: &Path, settings: &Settings) -> Result<(), String> {
    let write = || {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
        }
        write_atomically(path, |file| {
            serde_json::to_writer_pretty(file, settings).map_err(|e| e.to_string())?;
            file.sync_all().map_err(|e| e.to_string())
        })
    };
    write().map_err(|e| format!("couldn't save settings to {}: {}", path.display(), e))
}

fn settings_path(app: &AppHandle) -> Result<PathBuf, String> {
    app.path()
        .app_local_data_dir()
        .map(|dir| settings_path_in(&dir))
        .map_err(|e| format!("failed to resolve app local data dir: {}", e))
}

#[tauri::command]
fn load_settings(app: AppHandle) -> Result<Option<Settings>, String> {
    read_settings(&settings_path(&app)?)
}

/// Returns where a corrupt settings file was moved aside, if one was.
#[tauri::command]
fn save_settings(app: AppHandle, settings: Settings) -> Result<Option<String>, String> {
    save_settings_to(&settings_path(&app)?, settings)
        .map(|aside| aside.map(|p| p.display().to_string()))
}

// --- App entry ---

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_dialog::init())
        .manage(AppState {
            mode: Mutex::new(BackendMode::Ollama),
            http: http_client(),
            sidecar_port: Mutex::new(None),
            sidecar_child: Mutex::new(None),
            sidecar_generation: Mutex::new(0),
            secret: uuid::Uuid::new_v4().to_string(),
        })
        .setup(|app| {
            let handle = app.handle();

            // Tray icon
            if let Some(icon) = resolve_tray_icon(&handle) {
                let tray = TrayIconBuilder::new()
                    .icon(icon)
                    .icon_as_template(false)
                    .tooltip("rumble")
                    .build(app)?;
                app.manage(tray);
            }

            // Spawn sidecar (non-fatal — app works in Premium mode without it)
            if let Err(e) = spawn_sidecar(&handle) {
                eprintln!("[sidecar] failed to spawn: {}", e);
            }

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            // Filesystem
            scan_folder,
            move_to_trash,
            // API proxy
            api_call,
            extract_document,
            stream_message,
            // Auth / keychain
            auth_store_token,
            auth_get_token,
            auth_clear_token,
            store_api_key,
            get_api_key,
            delete_api_key,
            // Backend mode
            get_backend_mode,
            set_backend_mode,
            // Sidecar
            sidecar_status,
            // Document export
            export_draft_docx,
            // Settings
            load_settings,
            save_settings,
            // Local data lifecycle
            delete_all_local_data,
        ])
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|app, event| {
            if let tauri::RunEvent::Exit = event {
                let state = app.state::<AppState>();
                kill_sidecar(&state);
            }
        });
}

// --- Unit tests ---

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn make_state(mode: BackendMode, port: Option<u16>) -> AppState {
        AppState {
            mode: Mutex::new(mode),
            http: Client::new(),
            sidecar_port: Mutex::new(port),
            sidecar_child: Mutex::new(None),
            sidecar_generation: Mutex::new(0),
            secret: "test-secret".to_string(),
        }
    }

    // --- resolve_url: the per-mode routing table ---

    const LOCAL_MODES: [BackendMode; 2] = [BackendMode::Ollama, BackendMode::Byok];

    #[test]
    fn local_modes_route_sidecar_paths_to_the_sidecar() {
        for mode in LOCAL_MODES {
            let state = make_state(mode.clone(), Some(11435));
            for path in ["/health", "/chats", "/chats/abc-123/message", "/draft", "/billing", "/search", "/some-future-endpoint"] {
                assert_eq!(
                    state.resolve_url(path).unwrap().url,
                    format!("http://127.0.0.1:11435{}", path),
                    "{:?} {}",
                    mode,
                    path
                );
            }
        }
    }

    #[test]
    fn local_modes_refuse_the_cloud_contract() {
        for mode in LOCAL_MODES {
            let state = make_state(mode.clone(), Some(11435));
            for path in ["/api/v1", "/api/v1/", "/api/v1/search/keyword", "/api/v1/billing/subscription"] {
                assert_eq!(state.resolve_url(path), Err(PREMIUM_REQUIRED.to_string()), "{:?} {}", mode, path);
            }
        }
    }

    #[test]
    fn local_modes_do_not_treat_lookalike_prefixes_as_cloud() {
        let state = make_state(BackendMode::Ollama, Some(11435));
        for path in ["/api/v10/x", "/api/v1x", "/api"] {
            assert_eq!(state.resolve_url(path).unwrap().url, format!("http://127.0.0.1:11435{}", path));
        }
    }

    #[test]
    fn local_modes_error_when_the_sidecar_is_not_running() {
        for mode in LOCAL_MODES {
            let state = make_state(mode, None);
            assert_eq!(state.resolve_url("/chats"), Err("sidecar not running".to_string()));
        }
    }

    #[test]
    fn premium_refuses_every_path_until_the_payload_contract_lands() {
        let state = make_state(BackendMode::Premium, Some(11435));
        for path in ["/chats", "/health", "/api/v1/search/keyword"] {
            assert_eq!(state.resolve_url(path), Err(PREMIUM_REQUIRED.to_string()), "{}", path);
        }
    }

    #[test]
    fn resolve_url_never_returns_a_non_loopback_url() {
        let paths = ["/chats", "/billing", "/api/v1/me", "/api/v1", "https://api.elefant.com/x", "/me"];
        for mode in [BackendMode::Ollama, BackendMode::Byok, BackendMode::Premium] {
            let state = make_state(mode, Some(11435));
            for path in paths {
                if let Ok(Target { url, .. }) = state.resolve_url(path) {
                    assert!(url.starts_with("http://127.0.0.1:11435"), "{}", url);
                }
            }
        }
    }

    #[test]
    fn a_path_that_would_change_the_host_is_refused() {
        let state = make_state(BackendMode::Ollama, Some(11435));
        for path in ["@evil.example/x", "evil.example/x", "//evil.example/x", "//api/v1/me", ".evil.example/x"] {
            assert!(state.resolve_url(path).is_err(), "{}", path);
            assert!(state.sidecar_url(path).is_err(), "{}", path);
        }
    }

    #[test]
    fn the_cloud_contract_is_judged_on_the_parsed_case_folded_path() {
        let state = make_state(BackendMode::Byok, Some(11435));
        for path in [
            "/api/v1?x=1",
            "/API/v1/me",
            "/Api/V1",
            "/./api/v1/me",
            "/x/../api/v1/me",
            "/api/v1#frag",
            "/api%2Fv1/me",
            "/%61pi/v1/me",
            "/api/v1%2Fme",
            "/api/v1;x",
            "/api//v1/me",
            "/api;x/v1/me",
        ] {
            assert_eq!(state.resolve_url(path), Err(PREMIUM_REQUIRED.to_string()), "{}", path);
        }
    }

    #[test]
    fn sidecar_url_accepts_an_empty_path_for_callers_that_append_their_own() {
        let state = make_state(BackendMode::Ollama, Some(11435));
        assert_eq!(state.sidecar_url("").unwrap(), "http://127.0.0.1:11435");
    }

    // --- is_sidecar_url: parsed host and port, not a prefix ---

    #[test]
    fn only_http_127_0_0_1_at_the_sidecar_port_is_the_sidecar() {
        assert!(is_sidecar_url("http://127.0.0.1:11435/chats", Some(11435)));
        for url in [
            "http://127.0.0.1:11435@evil.example/x",
            "http://127.0.0.1.evil.example:11435/x",
            "http://evil.example/?h=127.0.0.1:11435",
            "http://127.0.0.1:9999/chats",
            "https://127.0.0.1:11435/chats",
            "http://127.0.0.1/chats",
            "not a url",
        ] {
            assert!(!is_sidecar_url(url, Some(11435)), "{}", url);
        }
        assert!(!is_sidecar_url("http://127.0.0.1:11435/chats", None));
    }

    // --- request_credentials (#72) ---

    fn target(url: &str, sidecar_port: Option<u16>) -> Target {
        Target { url: url.to_string(), sidecar_port }
    }

    fn never_read_token() -> Option<String> {
        panic!("the keychain must not be read for a sidecar request")
    }

    #[test]
    fn sidecar_requests_carry_the_secret_and_never_read_the_token() {
        let (token, secret) =
            request_credentials(&target("http://127.0.0.1:11435/chats", Some(11435)), "s3cret", never_read_token);
        assert_eq!((token, secret), (None, Some("s3cret")));
    }

    #[test]
    fn a_respawn_after_resolving_does_not_turn_a_sidecar_request_into_a_cloud_one() {
        let state = make_state(BackendMode::Ollama, Some(11435));
        let resolved = state.resolve_url("/chats").unwrap();
        kill_sidecar(&state);
        *state.sidecar_port.lock().unwrap() = Some(22222);
        let (token, secret) = request_credentials(&resolved, &state.secret, never_read_token);
        assert_eq!((token, secret), (None, Some("test-secret")));
    }

    #[test]
    fn cloud_requests_carry_the_bearer_and_never_the_secret() {
        let (token, secret) =
            request_credentials(&target("https://api.elefant.com/api/v1/me", None), "s3cret", || Some("bearer".to_string()));
        assert_eq!((token, secret), (Some("bearer".to_string()), None));
    }

    #[test]
    fn a_lookalike_sidecar_url_never_gets_the_secret() {
        let (_, secret) =
            request_credentials(&target("http://127.0.0.1:11435@evil.example/x", Some(11435)), "s3cret", || None);
        assert_eq!(secret, None);
    }

    // --- parse_mode ---

    #[test]
    fn parse_mode_maps_each_mode_name() {
        assert_eq!(parse_mode("ollama"), Ok(BackendMode::Ollama));
        assert_eq!(parse_mode("byok"), Ok(BackendMode::Byok));
        assert_eq!(parse_mode("premium"), Ok(BackendMode::Premium));
        assert_eq!(parse_mode("Ollama"), Err("unknown backend mode: Ollama".to_string()));
    }

    // --- sidecar_mode_env: the BYOK key reaches the sidecar only in byok mode ---

    fn env_value(env: &[(&'static str, String)], key: &str) -> Option<String> {
        env.iter().find(|(k, _)| *k == key).map(|(_, v)| v.clone())
    }

    #[test]
    fn byok_env_carries_the_mode_and_the_key() {
        let env = sidecar_mode_env(&BackendMode::Byok, Some("sk-test".to_string())).unwrap();
        assert_eq!(env_value(&env, "RUMBLE_BACKEND_MODE").as_deref(), Some("byok"));
        assert_eq!(env_value(&env, "BYOK_API_KEY").as_deref(), Some("sk-test"));
    }

    #[test]
    fn byok_without_a_key_is_refused() {
        for key in [None, Some(String::new())] {
            assert_eq!(sidecar_mode_env(&BackendMode::Byok, key), Err(BYOK_KEY_MISSING.to_string()));
        }
    }

    #[test]
    fn ollama_and_premium_envs_run_the_sidecar_in_ollama_mode_with_the_key_blanked() {
        for mode in [BackendMode::Ollama, BackendMode::Premium] {
            let env = sidecar_mode_env(&mode, Some("sk-leak".to_string())).unwrap();
            assert_eq!(env_value(&env, "RUMBLE_BACKEND_MODE").as_deref(), Some("ollama"), "{:?}", mode);
            assert_eq!(env_value(&env, "BYOK_API_KEY").as_deref(), Some(""), "{:?}", mode);
            assert!(env.iter().all(|(_, v)| !v.contains("sk-leak")), "{:?}", mode);
        }
    }

    // --- apply_mode: respawn on change ---

    #[test]
    fn same_mode_does_not_respawn() {
        let state = make_state(BackendMode::Ollama, Some(11435));
        let mut respawns = 0;
        let changed = apply_mode(&state, BackendMode::Ollama, || {
            respawns += 1;
            Ok(())
        });
        assert_eq!((changed, respawns), (Ok(false), 0));
    }

    #[test]
    fn a_mode_change_respawns_once_with_the_new_mode_already_set() {
        let state = make_state(BackendMode::Ollama, Some(11435));
        let mut seen = Vec::new();
        let changed = apply_mode(&state, BackendMode::Byok, || {
            seen.push(state.mode.lock().unwrap().clone());
            Ok(())
        });
        assert_eq!(changed, Ok(true));
        assert_eq!(seen, vec![BackendMode::Byok]);
        assert_eq!(*state.mode.lock().unwrap(), BackendMode::Byok);
    }

    #[test]
    fn a_failed_respawn_restores_the_previous_mode() {
        let state = make_state(BackendMode::Ollama, Some(11435));
        let result = apply_mode(&state, BackendMode::Byok, || Err(BYOK_KEY_MISSING.to_string()));
        assert_eq!(result, Err(BYOK_KEY_MISSING.to_string()));
        assert_eq!(*state.mode.lock().unwrap(), BackendMode::Ollama);
    }

    // --- after_key_change: a byok sidecar follows its key ---

    fn key_change(mode: BackendMode, provider: &str, change: KeyChange) -> (u32, BackendMode) {
        let state = make_state(mode, Some(11435));
        let mut respawns = 0;
        after_key_change(&state, provider, change, || {
            respawns += 1;
            Ok(())
        })
        .unwrap();
        let mode = state.mode.lock().unwrap().clone();
        (respawns, mode)
    }

    #[test]
    fn storing_the_openai_key_in_byok_respawns_the_sidecar() {
        assert_eq!(key_change(BackendMode::Byok, "openai", KeyChange::Stored), (1, BackendMode::Byok));
    }

    #[test]
    fn deleting_the_openai_key_in_byok_drops_to_ollama_and_respawns() {
        assert_eq!(key_change(BackendMode::Byok, "openai", KeyChange::Deleted), (1, BackendMode::Ollama));
    }

    #[test]
    fn key_changes_outside_byok_or_for_other_providers_leave_the_sidecar_alone() {
        for change in [KeyChange::Stored, KeyChange::Deleted] {
            assert_eq!(key_change(BackendMode::Ollama, "openai", change), (0, BackendMode::Ollama));
            assert_eq!(key_change(BackendMode::Byok, "anthropic", change), (0, BackendMode::Byok));
        }
    }

    #[test]
    fn a_failed_respawn_after_a_key_change_says_the_key_change_happened() {
        for (change, done) in [(KeyChange::Stored, "Key saved"), (KeyChange::Deleted, "Key removed")] {
            let state = make_state(BackendMode::Byok, Some(11435));
            let result = after_key_change(&state, "openai", change, || Err("spawn failed".to_string()));
            assert_eq!(
                result,
                Err(format!("{}, but the local AI service didn't restart: spawn failed", done))
            );
        }
    }

    // --- set_port_if_current: the generation guard ---

    #[test]
    fn the_current_generation_publishes_and_clears_the_port() {
        let state = make_state(BackendMode::Ollama, None);
        let generation = state.next_sidecar_generation();
        assert!(state.set_port_if_current(generation, Some(9999)));
        assert_eq!(state.sidecar_port(), Some(9999));
        assert!(state.set_port_if_current(generation, None));
        assert_eq!(state.sidecar_port(), None);
    }

    #[test]
    fn a_replaced_sidecar_cannot_publish_or_clear_the_port() {
        let state = make_state(BackendMode::Ollama, None);
        let old = state.next_sidecar_generation();
        let new = state.next_sidecar_generation();
        assert!(state.set_port_if_current(new, Some(11435)));
        assert!(!state.set_port_if_current(old, Some(9999)));
        assert!(!state.set_port_if_current(old, None));
        assert_eq!(state.sidecar_port(), Some(11435));
    }

    // --- the health checks send the shared secret (#69) ---

    async fn health_server() -> MockServer {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/health"))
            .and(header(SECRET_HEADER, "test-secret"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"status": "ok", "mode": "ollama"})))
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/health"))
            .respond_with(ResponseTemplate::new(401))
            .mount(&server)
            .await;
        server
    }

    #[tokio::test]
    async fn sidecar_status_reports_health_asked_with_the_secret() {
        let server = health_server().await;
        let port = server.address().port();
        let status = sidecar_status_value(&Client::new(), Some(port), "test-secret").await;
        assert_eq!(
            status,
            serde_json::json!({"running": true, "port": port, "health": {"status": "ok", "mode": "ollama"}})
        );
    }

    #[tokio::test]
    async fn sidecar_status_without_a_port_reports_not_running() {
        let status = sidecar_status_value(&Client::new(), None, "test-secret").await;
        assert_eq!(status, serde_json::json!({"running": false, "port": null}));
    }

    #[tokio::test]
    async fn the_spawn_health_poll_sends_the_secret() {
        let server = health_server().await;
        let result = poll_health(&Client::new(), server.address().port(), "test-secret").await;
        assert_eq!(result, Ok(()));
    }

    // --- BackendMode serde ---

    #[test]
    fn backend_mode_serializes_to_lowercase() {
        assert_eq!(
            serde_json::to_string(&BackendMode::Ollama).unwrap(),
            "\"ollama\""
        );
        assert_eq!(
            serde_json::to_string(&BackendMode::Byok).unwrap(),
            "\"byok\""
        );
        assert_eq!(
            serde_json::to_string(&BackendMode::Premium).unwrap(),
            "\"premium\""
        );
    }

    #[test]
    fn backend_mode_deserializes_from_lowercase() {
        let ollama: BackendMode = serde_json::from_str("\"ollama\"").unwrap();
        assert_eq!(ollama, BackendMode::Ollama);
        let byok: BackendMode = serde_json::from_str("\"byok\"").unwrap();
        assert_eq!(byok, BackendMode::Byok);
        let premium: BackendMode = serde_json::from_str("\"premium\"").unwrap();
        assert_eq!(premium, BackendMode::Premium);
    }

    #[test]
    fn backend_mode_invalid_string_errors() {
        let result: Result<BackendMode, _> = serde_json::from_str("\"invalid\"");
        assert!(result.is_err());
    }

    #[test]
    fn file_info_serializes_to_json() {
        let info = FileInfo {
            path: "/tmp/test.txt".to_string(),
            size: 1024,
            modified: "2025-01-01 00:00:00".to_string(),
        };
        let json = serde_json::to_value(&info).unwrap();
        assert_eq!(json["path"], "/tmp/test.txt");
        assert_eq!(json["size"], 1024);
        assert_eq!(json["modified"], "2025-01-01 00:00:00");
    }

    // --- scan_folder ---

    fn home_tempdir() -> tempfile::TempDir {
        let home = dirs::home_dir().expect("need $HOME for tests");
        tempfile::tempdir_in(home).unwrap()
    }

    #[test]
    fn scan_folder_empty_directory() {
        let dir = home_tempdir();
        let result = scan_folder(dir.path().to_string_lossy().to_string()).unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn scan_folder_with_files() {
        let dir = home_tempdir();
        let file_path = dir.path().join("test.txt");
        let mut f = std::fs::File::create(&file_path).unwrap();
        f.write_all(b"hello world").unwrap();

        let result = scan_folder(dir.path().to_string_lossy().to_string()).unwrap();
        assert_eq!(result.len(), 1);
        assert!(result[0].path.contains("test.txt"));
        assert_eq!(result[0].size, 11); // "hello world" = 11 bytes
        assert!(!result[0].modified.is_empty());
    }

    #[test]
    fn scan_folder_nested_directories() {
        let dir = home_tempdir();
        let sub = dir.path().join("subdir");
        std::fs::create_dir_all(&sub).unwrap();
        std::fs::File::create(dir.path().join("root.txt")).unwrap();
        std::fs::File::create(sub.join("nested.txt")).unwrap();

        let result = scan_folder(dir.path().to_string_lossy().to_string()).unwrap();
        assert_eq!(result.len(), 2);
        let paths: Vec<&str> = result.iter().map(|f| f.path.as_str()).collect();
        assert!(paths.iter().any(|p| p.contains("root.txt")));
        assert!(paths.iter().any(|p| p.contains("nested.txt")));
    }

    #[test]
    fn scan_folder_nonexistent_returns_error() {
        let home = dirs::home_dir().expect("need $HOME");
        let result = scan_folder(
            home.join("nonexistent-rumble-test-dir-xyz")
                .to_string_lossy()
                .to_string(),
        );
        assert!(result.is_err());
    }

    // --- move_to_trash ---

    #[test]
    fn move_to_trash_nonexistent_file_errors() {
        let result = move_to_trash("/tmp/nonexistent-rumble-test-file-xyz".to_string());
        assert!(result.is_err());
    }

    // --- write_atomically ---

    fn entries(dir: &std::path::Path) -> Vec<std::path::PathBuf> {
        std::fs::read_dir(dir)
            .unwrap()
            .map(|e| e.unwrap().path())
            .collect()
    }

    #[test]
    fn write_atomically_failed_write_leaves_nothing_behind() {
        let dir = tempfile::tempdir().unwrap();
        let dest = dir.path().join("draft.docx");

        let result = write_atomically(&dest, |_file| Err("write failed".to_string()));

        assert!(result.is_err());
        assert!(!dest.exists());
        assert!(entries(dir.path()).is_empty());
    }

    #[test]
    fn write_atomically_success_writes_content_and_cleans_up_temp() {
        let dir = tempfile::tempdir().unwrap();
        let dest = dir.path().join("draft.docx");

        let result = write_atomically(&dest, |mut file| {
            use std::io::Write;
            file.write_all(b"hello docx").map_err(|e| e.to_string())
        });

        assert!(result.is_ok());
        assert_eq!(std::fs::read(&dest).unwrap(), b"hello docx");
        assert_eq!(entries(dir.path()), vec![dest]);
    }

    #[test]
    fn write_atomically_failed_write_leaves_existing_destination_untouched() {
        let dir = tempfile::tempdir().unwrap();
        let dest = dir.path().join("draft.docx");
        std::fs::write(&dest, b"original content").unwrap();

        let result = write_atomically(&dest, |_file| Err("write failed".to_string()));

        assert!(result.is_err());
        assert_eq!(std::fs::read(&dest).unwrap(), b"original content");
    }

    // --- settings ---

    fn sample_settings() -> Settings {
        serde_json::json!({
            "appearance": {"theme": "dark", "showChatSidebarByDefault": false},
            "workspace": {"briefcases": ["Matter A"]},
        })
        .as_object()
        .unwrap()
        .clone()
    }

    fn settings_of(value: serde_json::Value) -> Settings {
        value.as_object().unwrap().clone()
    }

    fn corrupt_asides(dir: &std::path::Path) -> Vec<std::path::PathBuf> {
        entries(dir)
            .into_iter()
            .filter(|p| p.to_string_lossy().contains(".corrupt-"))
            .collect()
    }

    #[test]
    fn settings_path_is_settings_json_in_the_given_dir() {
        assert_eq!(
            settings_path_in(Path::new("/data/com.ielegante.rumble")),
            Path::new("/data/com.ielegante.rumble/settings.json")
        );
    }

    #[test]
    fn settings_survive_a_save_and_a_fresh_read() {
        let dir = tempfile::tempdir().unwrap();
        let path = settings_path_in(dir.path());

        save_settings_to(&path, sample_settings()).unwrap();

        assert_eq!(read_settings(&path).unwrap(), Some(sample_settings()));
    }

    #[test]
    fn settings_missing_file_reads_as_nothing_saved() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(read_settings(&settings_path_in(dir.path())).unwrap(), None);
    }

    #[test]
    fn settings_corrupt_file_is_an_error_not_a_reset() {
        let dir = tempfile::tempdir().unwrap();
        let path = settings_path_in(dir.path());
        std::fs::write(&path, b"{not json").unwrap();

        let err = read_settings(&path).unwrap_err();

        assert!(err.contains("failed to parse"), "got: {}", err);
    }

    #[test]
    fn settings_non_object_json_is_corrupt() {
        let dir = tempfile::tempdir().unwrap();
        let path = settings_path_in(dir.path());
        std::fs::write(&path, b"[1, 2]").unwrap();

        assert!(read_settings(&path).is_err());
    }

    #[test]
    fn settings_save_keeps_top_level_groups_it_was_not_sent() {
        let dir = tempfile::tempdir().unwrap();
        let path = settings_path_in(dir.path());
        std::fs::write(&path, br#"{"backendMode": "byok", "appearance": {"theme": "light"}}"#).unwrap();

        save_settings_to(&path, settings_of(serde_json::json!({"appearance": {"theme": "dark"}}))).unwrap();

        assert_eq!(
            read_settings(&path).unwrap(),
            Some(settings_of(serde_json::json!({
                "backendMode": "byok",
                "appearance": {"theme": "dark"},
            })))
        );
    }

    #[test]
    fn settings_save_moves_a_corrupt_file_aside_instead_of_overwriting_it() {
        let dir = tempfile::tempdir().unwrap();
        let path = settings_path_in(dir.path());
        std::fs::write(&path, b"{not json").unwrap();

        let aside = save_settings_to(&path, sample_settings()).unwrap().unwrap();

        assert_eq!(std::fs::read(&aside).unwrap(), b"{not json");
        assert_eq!(corrupt_asides(dir.path()), vec![aside]);
        assert_eq!(read_settings(&path).unwrap(), Some(sample_settings()));
    }

    #[test]
    fn settings_invalid_utf8_file_is_moved_aside_like_any_corrupt_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = settings_path_in(dir.path());
        std::fs::write(&path, b"{\"a\": \"\xff\xfe\"}").unwrap();

        assert!(read_settings(&path).unwrap_err().contains("failed to parse"));
        let aside = save_settings_to(&path, sample_settings()).unwrap().unwrap();

        assert_eq!(std::fs::read(&aside).unwrap(), b"{\"a\": \"\xff\xfe\"}");
        assert_eq!(read_settings(&path).unwrap(), Some(sample_settings()));
    }

    #[test]
    fn settings_two_corrupt_saves_keep_two_asides() {
        let dir = tempfile::tempdir().unwrap();
        let path = settings_path_in(dir.path());

        std::fs::write(&path, b"first").unwrap();
        save_settings_to(&path, sample_settings()).unwrap();
        std::fs::write(&path, b"second").unwrap();
        save_settings_to(&path, sample_settings()).unwrap();

        let mut kept: Vec<Vec<u8>> =
            corrupt_asides(dir.path()).iter().map(|p| std::fs::read(p).unwrap()).collect();
        kept.sort();
        assert_eq!(kept, vec![b"first".to_vec(), b"second".to_vec()]);
    }

    #[test]
    fn settings_save_over_a_valid_file_moves_nothing_aside() {
        let dir = tempfile::tempdir().unwrap();
        let path = settings_path_in(dir.path());
        save_settings_to(&path, sample_settings()).unwrap();

        assert_eq!(save_settings_to(&path, sample_settings()).unwrap(), None);
        assert!(corrupt_asides(dir.path()).is_empty());
    }

    #[test]
    fn settings_save_refuses_to_write_over_an_unreadable_path() {
        let dir = tempfile::tempdir().unwrap();
        let path = settings_path_in(dir.path());
        std::fs::create_dir(&path).unwrap();

        let err = save_settings_to(&path, sample_settings()).unwrap_err();

        assert!(err.starts_with(&format!("couldn't save settings to {}: ", path.display())), "got: {}", err);
        assert!(path.is_dir());
    }

    #[test]
    fn settings_write_leaves_only_the_settings_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = settings_path_in(dir.path());

        save_settings_to(&path, sample_settings()).unwrap();

        assert_eq!(entries(dir.path()), vec![path]);
    }

    #[cfg(unix)]
    #[test]
    fn settings_write_replaces_the_file_rather_than_rewriting_it_in_place() {
        use std::io::Read;
        let dir = tempfile::tempdir().unwrap();
        let path = settings_path_in(dir.path());
        std::fs::write(&path, b"{}").unwrap();
        let mut held = std::fs::File::open(&path).unwrap();

        save_settings_to(&path, sample_settings()).unwrap();

        let mut seen = String::new();
        held.read_to_string(&mut seen).unwrap();
        assert_eq!(seen, "{}");
    }

    #[test]
    fn settings_write_creates_missing_parent_dir() {
        let dir = tempfile::tempdir().unwrap();
        let path = settings_path_in(&dir.path().join("not-yet-created"));

        save_settings_to(&path, sample_settings()).unwrap();

        assert_eq!(read_settings(&path).unwrap(), Some(sample_settings()));
    }

    #[test]
    fn settings_write_error_names_settings_and_the_path() {
        let dir = tempfile::tempdir().unwrap();
        let blocker = dir.path().join("a-file");
        std::fs::write(&blocker, b"").unwrap();
        let path = settings_path_in(&blocker);

        let err = write_settings(&path, &sample_settings()).unwrap_err();

        assert!(err.starts_with("couldn't save settings to "), "got: {}", err);
        assert!(err.contains(&path.display().to_string()), "got: {}", err);
    }

    // --- make_http_request (wiremock) ---

    use wiremock::matchers::{bearer_token, body_json, body_string_contains, header, method, path, query_param};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[tokio::test]
    async fn http_get_returns_json() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/data"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"ok": true})))
            .mount(&server)
            .await;

        let http = Client::new();
        let url = format!("{}/data", server.uri());
        let result = make_http_request(&http, &url, "GET", None, None, None, None).await;
        assert_eq!(result.unwrap(), serde_json::json!({"ok": true}));
    }

    #[tokio::test]
    async fn http_post_sends_json_body() {
        let server = MockServer::start().await;
        let payload = serde_json::json!({"name": "test"});
        Mock::given(method("POST"))
            .and(path("/items"))
            .and(body_json(&payload))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"id": "1"})))
            .mount(&server)
            .await;

        let http = Client::new();
        let url = format!("{}/items", server.uri());
        let result = make_http_request(&http, &url, "POST", Some(payload), None, None, None).await;
        assert_eq!(result.unwrap(), serde_json::json!({"id": "1"}));
    }

    #[tokio::test]
    async fn http_put_forwards_method() {
        let server = MockServer::start().await;
        Mock::given(method("PUT"))
            .and(path("/items/1"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"updated": true})))
            .mount(&server)
            .await;

        let http = Client::new();
        let url = format!("{}/items/1", server.uri());
        let result = make_http_request(&http, &url, "PUT", None, None, None, None).await;
        assert_eq!(result.unwrap(), serde_json::json!({"updated": true}));
    }

    #[tokio::test]
    async fn http_patch_forwards_method() {
        let server = MockServer::start().await;
        Mock::given(method("PATCH"))
            .and(path("/items/1"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"patched": true})))
            .mount(&server)
            .await;

        let http = Client::new();
        let url = format!("{}/items/1", server.uri());
        let result = make_http_request(&http, &url, "PATCH", None, None, None, None).await;
        assert_eq!(result.unwrap(), serde_json::json!({"patched": true}));
    }

    #[tokio::test]
    async fn http_delete_forwards_method() {
        let server = MockServer::start().await;
        Mock::given(method("DELETE"))
            .and(path("/items/1"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"deleted": true})))
            .mount(&server)
            .await;

        let http = Client::new();
        let url = format!("{}/items/1", server.uri());
        let result = make_http_request(&http, &url, "DELETE", None, None, None, None).await;
        assert_eq!(result.unwrap(), serde_json::json!({"deleted": true}));
    }

    #[tokio::test]
    async fn http_unsupported_method_returns_error() {
        let http = Client::new();
        let result = make_http_request(&http, "http://localhost", "TRACE", None, None, None, None).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("unsupported HTTP method"));
    }

    #[tokio::test]
    async fn http_query_params_appended() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/search"))
            .and(query_param("foo", "bar"))
            .and(query_param("n", "42"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"found": true})))
            .mount(&server)
            .await;

        let http = Client::new();
        let url = format!("{}/search", server.uri());
        let params = serde_json::json!({"foo": "bar", "n": 42});
        let result = make_http_request(&http, &url, "GET", None, Some(params), None, None).await;
        assert_eq!(result.unwrap(), serde_json::json!({"found": true}));
    }

    #[tokio::test]
    async fn http_auth_token_injected_as_bearer() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/secure"))
            .and(bearer_token("test-token"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"auth": true})))
            .mount(&server)
            .await;

        let http = Client::new();
        let url = format!("{}/secure", server.uri());
        let result =
            make_http_request(&http, &url, "GET", None, None, Some("test-token"), None).await;
        assert_eq!(result.unwrap(), serde_json::json!({"auth": true}));
    }

    #[tokio::test]
    async fn http_no_auth_omits_header() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/open"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"open": true})))
            .expect(1)
            .mount(&server)
            .await;

        let http = Client::new();
        let url = format!("{}/open", server.uri());
        let result = make_http_request(&http, &url, "GET", None, None, None, None).await;
        assert_eq!(result.unwrap(), serde_json::json!({"open": true}));
        // Verify the mock was hit (no auth header required)
    }

    #[tokio::test]
    async fn http_4xx_returns_error_with_status() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/missing"))
            .respond_with(ResponseTemplate::new(404).set_body_string("not found"))
            .mount(&server)
            .await;

        let http = Client::new();
        let url = format!("{}/missing", server.uri());
        let result = make_http_request(&http, &url, "GET", None, None, None, None).await;
        let err = result.unwrap_err();
        assert!(err.contains("API error 404"), "got: {}", err);
        assert!(err.contains("not found"), "got: {}", err);
    }

    #[tokio::test]
    async fn http_5xx_returns_error_with_body() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/crash"))
            .respond_with(ResponseTemplate::new(500).set_body_string("internal failure"))
            .mount(&server)
            .await;

        let http = Client::new();
        let url = format!("{}/crash", server.uri());
        let result = make_http_request(&http, &url, "GET", None, None, None, None).await;
        let err = result.unwrap_err();
        assert!(err.contains("API error 500"), "got: {}", err);
        assert!(err.contains("internal failure"), "got: {}", err);
    }

    #[tokio::test]
    async fn http_connection_refused_returns_error() {
        let http = Client::new();
        // Use a port that's almost certainly not listening
        let result =
            make_http_request(&http, "http://127.0.0.1:1", "GET", None, None, None, None).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn http_empty_body_returns_empty_json_object() {
        let server = MockServer::start().await;
        Mock::given(method("DELETE"))
            .and(path("/items/1"))
            .respond_with(ResponseTemplate::new(200).set_body_string(""))
            .mount(&server)
            .await;

        let http = Client::new();
        let url = format!("{}/items/1", server.uri());
        let result = make_http_request(&http, &url, "DELETE", None, None, None, None).await;
        assert_eq!(result.unwrap(), serde_json::json!({}));
    }

    // --- decode_filename ---

    #[test]
    fn decode_filename_passes_through_ascii() {
        assert_eq!(decode_filename("contract.pdf").unwrap(), "contract.pdf");
    }

    #[test]
    fn decode_filename_decodes_percent_encoded_unicode() {
        // encodeURIComponent("合同.pdf") on the TS side
        assert_eq!(
            decode_filename("%E5%90%88%E5%90%8C.pdf").unwrap(),
            "合同.pdf"
        );
    }

    // --- post_extract_multipart (wiremock) ---

    const TEST_EXTRACT_TIMEOUT: Duration = Duration::from_secs(5);

    #[tokio::test]
    async fn extract_sends_file_part_and_secret_header() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/extract"))
            .and(header(SECRET_HEADER, "test-secret"))
            .and(body_string_contains("filename=\"contract.pdf\""))
            .and(body_string_contains("Clause one."))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(serde_json::json!({"text": "Clause one."})),
            )
            .mount(&server)
            .await;

        let http = Client::new();
        let result = post_extract_multipart(
            &http,
            &server.uri(),
            "test-secret",
            "contract.pdf",
            b"Clause one.".to_vec(),
            TEST_EXTRACT_TIMEOUT,
        )
        .await;
        assert_eq!(result.unwrap(), "Clause one.");
    }

    #[tokio::test]
    async fn extract_422_surfaces_detail_message() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/extract"))
            .respond_with(ResponseTemplate::new(422).set_body_json(
                serde_json::json!({"detail": "Unsupported file type: contract.doc"}),
            ))
            .mount(&server)
            .await;

        let http = Client::new();
        let result = post_extract_multipart(
            &http,
            &server.uri(),
            "test-secret",
            "contract.doc",
            b"anything".to_vec(),
            TEST_EXTRACT_TIMEOUT,
        )
        .await;
        assert_eq!(
            result.unwrap_err(),
            "Unsupported file type: contract.doc"
        );
    }

    #[tokio::test]
    async fn extract_500_surfaces_generic_message_not_raw_body() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/extract"))
            .respond_with(
                ResponseTemplate::new(500)
                    .set_body_string("<html>Internal Server Error - stack trace here</html>"),
            )
            .mount(&server)
            .await;

        let http = Client::new();
        let result = post_extract_multipart(
            &http,
            &server.uri(),
            "test-secret",
            "contract.pdf",
            b"bytes".to_vec(),
            TEST_EXTRACT_TIMEOUT,
        )
        .await;
        let err = result.unwrap_err();
        assert_eq!(err, "extraction failed");
        assert!(!err.contains("stack trace"), "must not leak raw body: {}", err);
    }

    #[tokio::test]
    async fn extract_413_surfaces_upload_limit_message() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/extract"))
            .respond_with(ResponseTemplate::new(413).set_body_json(
                serde_json::json!({"detail": "This file is larger than the 50 MB upload limit."}),
            ))
            .mount(&server)
            .await;

        let http = Client::new();
        let result = post_extract_multipart(
            &http,
            &server.uri(),
            "test-secret",
            "bundle.pdf",
            b"bytes".to_vec(),
            TEST_EXTRACT_TIMEOUT,
        )
        .await;
        assert_eq!(
            result.unwrap_err(),
            "This file is larger than the 50 MB upload limit."
        );
    }

    #[tokio::test]
    async fn extract_over_upload_limit_is_refused_before_sending() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/extract"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(serde_json::json!({"text": "sent"})),
            )
            .expect(0)
            .mount(&server)
            .await;

        let http = Client::new();
        let result = post_extract_multipart(
            &http,
            &server.uri(),
            "test-secret",
            "bundle.pdf",
            vec![0; MAX_UPLOAD_BYTES + 1],
            TEST_EXTRACT_TIMEOUT,
        )
        .await;
        assert_eq!(
            result.unwrap_err(),
            "This file is larger than the 50 MB upload limit."
        );
    }

    #[tokio::test]
    async fn extract_at_upload_limit_is_sent() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/extract"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(serde_json::json!({"text": "sent"})),
            )
            .expect(1)
            .mount(&server)
            .await;

        let http = Client::new();
        let result = post_extract_multipart(
            &http,
            &server.uri(),
            "test-secret",
            "bundle.pdf",
            vec![0; MAX_UPLOAD_BYTES],
            TEST_EXTRACT_TIMEOUT,
        )
        .await;
        assert_eq!(result.unwrap(), "sent");
    }

    #[tokio::test]
    async fn extract_timeout_says_extraction_is_slow_not_unreachable() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/extract"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_json(serde_json::json!({"text": "late"}))
                    .set_delay(Duration::from_millis(500)),
            )
            .mount(&server)
            .await;

        let http = Client::new();
        let result = post_extract_multipart(
            &http,
            &server.uri(),
            "test-secret",
            "bundle.pdf",
            b"bytes".to_vec(),
            Duration::from_millis(50),
        )
        .await;
        assert_eq!(result.unwrap_err(), EXTRACT_TIMEOUT_MESSAGE);
    }

    // --- migrate_data_dir ---

    fn write_files(dir: &Path, names: &[&str], tag: &str) {
        std::fs::create_dir_all(dir).unwrap();
        for name in names {
            std::fs::write(dir.join(name), format!("{} {}", tag, name)).unwrap();
        }
    }

    fn read(path: PathBuf) -> String {
        std::fs::read_to_string(path).unwrap()
    }

    fn names_in(dir: &Path) -> Vec<String> {
        let mut names: Vec<String> =
            entries(dir).iter().map(|p| p.file_name().unwrap().to_string_lossy().into_owned()).collect();
        names.sort();
        names
    }

    fn roaming_and_local() -> (tempfile::TempDir, PathBuf, PathBuf) {
        let root = tempfile::tempdir().unwrap();
        let old = root.path().join("Roaming").join("com.ielegante.rumble");
        let new = root.path().join("Local").join("com.ielegante.rumble");
        (root, old, new)
    }

    /// A non-empty directory where rumble.db should land: the WAL and SHM get placed, the database doesn't.
    fn block_db_placement(new: &Path) {
        std::fs::create_dir_all(new.join("rumble.db").join("occupied")).unwrap();
    }

    #[test]
    fn migration_copies_db_wal_and_shm_then_clears_the_old_dir() {
        let (_root, old, new) = roaming_and_local();
        write_files(&old, DB_FILES, "old");
        write_files(&old, &[SIDECAR_PIDFILE], "old");

        assert_eq!(
            migrate_data_dir(&old, &new).unwrap(),
            Migration::Copied { files: DB_FILES.to_vec(), left_behind: vec![] }
        );
        for name in DB_FILES {
            assert_eq!(read(new.join(name)), format!("old {}", name));
        }
        assert!(names_in(&old).is_empty(), "left in old: {:?}", names_in(&old));
    }

    #[test]
    fn migration_run_twice_is_a_no_op_the_second_time() {
        let (_root, old, new) = roaming_and_local();
        write_files(&old, DB_FILES, "old");
        migrate_data_dir(&old, &new).unwrap();

        assert_eq!(migrate_data_dir(&old, &new).unwrap(), Migration::NotNeeded);
        assert_eq!(read(new.join("rumble.db")), "old rumble.db");
    }

    #[test]
    fn failed_migration_leaves_the_old_dir_untouched_and_no_debris_in_the_new() {
        let (_root, old, new) = roaming_and_local();
        write_files(&old, DB_FILES, "old");
        block_db_placement(&new);

        assert!(migrate_data_dir(&old, &new).is_err());
        for name in DB_FILES {
            assert_eq!(read(old.join(name)), format!("old {}", name));
        }
        assert_eq!(names_in(&new), vec!["rumble.db"]); // only the obstacle
        assert_eq!(active_data_dir(&old, &new), old);
    }

    #[test]
    fn a_stale_wal_in_the_new_dir_is_never_replayed_over_the_migrated_db() {
        let (_root, old, new) = roaming_and_local();
        write_files(&old, &["rumble.db"], "old"); // cleanly closed: no WAL
        write_files(&new, &["rumble.db-wal", "rumble.db-shm"], "stale");

        migrate_data_dir(&old, &new).unwrap();

        assert_eq!(read(new.join("rumble.db")), "old rumble.db");
        assert!(!new.join("rumble.db-wal").exists());
        assert!(!new.join("rumble.db-shm").exists());
    }

    #[test]
    fn a_retry_after_a_crash_that_placed_a_wal_migrates_the_old_wal_intact() {
        let (_root, old, new) = roaming_and_local();
        write_files(&old, DB_FILES, "old");
        // An earlier run crashed after placing its WAL; this run fails too; then one succeeds.
        write_files(&new, &["rumble.db-wal"], "stale");
        block_db_placement(&new);
        assert!(migrate_data_dir(&old, &new).is_err());
        assert_eq!(read(old.join("rumble.db-wal")), "old rumble.db-wal");

        std::fs::remove_dir_all(new.join("rumble.db")).unwrap();
        migrate_data_dir(&old, &new).unwrap();
        for name in DB_FILES {
            assert_eq!(read(new.join(name)), format!("old {}", name));
        }
    }

    #[test]
    fn a_crash_after_placing_the_db_but_before_clearing_old_uses_the_new_copy() {
        let (_root, old, new) = roaming_and_local();
        write_files(&old, DB_FILES, "old");
        write_files(&new, DB_FILES, "old"); // the copy completed; the old files were not yet removed

        assert_eq!(migrate_data_dir(&old, &new).unwrap(), Migration::BothPresent);
        assert_eq!(active_data_dir(&old, &new), new);
        assert_eq!(names_in(&old).len(), 3, "the old copy is left alone");
    }

    #[test]
    fn migration_removes_temp_copies_left_by_an_earlier_run() {
        let (_root, old, new) = roaming_and_local();
        write_files(&old, &["rumble.db"], "old");
        write_files(&new, &["rumble.db.migrating-0000"], "stale");

        migrate_data_dir(&old, &new).unwrap();

        assert_eq!(names_in(&new), vec!["rumble.db"]);
    }

    #[test]
    fn migration_is_a_no_op_when_both_dirs_are_the_same() {
        let (_root, old, _new) = roaming_and_local();
        write_files(&old, DB_FILES, "old");

        assert_eq!(migrate_data_dir(&old, &old).unwrap(), Migration::NotNeeded);
        assert_eq!(names_in(&old).len(), 3);
    }

    #[test]
    fn migration_ignores_a_local_dir_that_exists_without_a_db() {
        let (_root, old, new) = roaming_and_local();
        write_files(&old, &["rumble.db"], "old");
        std::fs::create_dir_all(new.join("EBWebView")).unwrap();

        assert_eq!(
            migrate_data_dir(&old, &new).unwrap(),
            Migration::Copied { files: vec!["rumble.db"], left_behind: vec![] }
        );
        assert_eq!(names_in(&new), vec!["EBWebView", "rumble.db"]);
    }

    #[test]
    fn migration_is_a_no_op_when_the_old_dir_has_no_db() {
        let (_root, old, new) = roaming_and_local();

        assert_eq!(migrate_data_dir(&old, &new).unwrap(), Migration::NotNeeded);
        assert!(!new.exists());
        assert_eq!(active_data_dir(&old, &new), new);
    }

    // --- delete_local_data_files / delete_after_sidecar_exit ---

    #[test]
    fn delete_local_data_files_removes_db_files_pidfile_and_temps_only() {
        let dir = tempfile::tempdir().unwrap();
        write_files(dir.path(), DB_FILES, "x");
        write_files(dir.path(), &[SIDECAR_PIDFILE, "rumble.db.migrating-1", "unrelated.txt"], "x");

        delete_local_data_files(dir.path()).unwrap();

        assert_eq!(names_in(dir.path()), vec!["unrelated.txt"]);
    }

    #[test]
    fn delete_local_data_files_succeeds_when_nothing_is_there() {
        let dir = tempfile::tempdir().unwrap();
        assert!(delete_local_data_files(&dir.path().join("never-created")).is_ok());
    }

    #[test]
    fn delete_is_refused_while_the_sidecar_is_starting() {
        let dir = tempfile::tempdir().unwrap();
        let pidfile = dir.path().join(SIDECAR_PIDFILE);
        std::fs::write(&pidfile, "123").unwrap();
        assert_eq!(check_ready_to_delete(false, &pidfile).unwrap_err(), SIDECAR_STARTING_MESSAGE);
    }

    #[test]
    fn delete_is_refused_when_the_running_sidecar_has_no_pidfile() {
        let dir = tempfile::tempdir().unwrap();
        let pidfile = dir.path().join(SIDECAR_PIDFILE);
        assert_eq!(check_ready_to_delete(true, &pidfile).unwrap_err(), SIDECAR_PIDFILE_MISSING_MESSAGE);
    }

    #[test]
    fn delete_may_proceed_once_the_sidecar_runs_with_its_pidfile() {
        let dir = tempfile::tempdir().unwrap();
        let pidfile = dir.path().join(SIDECAR_PIDFILE);
        std::fs::write(&pidfile, "123").unwrap();
        assert!(check_ready_to_delete(true, &pidfile).is_ok());
    }

    #[tokio::test]
    async fn delete_waits_for_the_sidecar_to_release_the_db_then_deletes() {
        let dir = tempfile::tempdir().unwrap();
        write_files(dir.path(), DB_FILES, "x");
        let pidfile = dir.path().join(SIDECAR_PIDFILE);
        std::fs::write(&pidfile, "123").unwrap();
        let exiting_sidecar = {
            let pidfile = pidfile.clone();
            tokio::spawn(async move {
                tokio::time::sleep(SIDECAR_STOP_POLL * 2).await;
                std::fs::remove_file(pidfile).unwrap();
            })
        };

        delete_after_sidecar_exit(&pidfile, &[dir.path()], SIDECAR_STOP_TIMEOUT).await.unwrap();

        assert!(names_in(dir.path()).is_empty());
        exiting_sidecar.await.unwrap();
    }

    #[tokio::test]
    async fn delete_deletes_nothing_when_the_sidecar_never_lets_go() {
        let dir = tempfile::tempdir().unwrap();
        write_files(dir.path(), DB_FILES, "x");
        let pidfile = dir.path().join(SIDECAR_PIDFILE);
        std::fs::write(&pidfile, "123").unwrap();

        let result = delete_after_sidecar_exit(&pidfile, &[dir.path()], SIDECAR_STOP_POLL * 3).await;

        assert_eq!(result.unwrap_err(), SIDECAR_STOP_TIMEOUT_MESSAGE);
        assert_eq!(names_in(dir.path()).len(), 4);
    }

    // --- SseLines ---

    #[test]
    fn sse_lines_yields_data_payloads_from_crlf_lines() {
        let mut lines = SseLines::default();
        let payloads = lines.push(b"event: message\r\ndata: {\"a\":1}\r\n\r\n");
        assert_eq!(payloads, vec!["{\"a\":1}".to_string()]);
    }

    #[test]
    fn sse_lines_holds_a_partial_line_until_it_completes() {
        let mut lines = SseLines::default();
        assert!(lines.push(b"data: {\"type\":\"del").is_empty());
        assert_eq!(lines.push(b"ta\"}\n"), vec!["{\"type\":\"delta\"}".to_string()]);
    }

    #[test]
    fn sse_lines_keeps_a_utf8_character_split_across_chunks() {
        let mut lines = SseLines::default();
        let line = "data: 合同\n".as_bytes();
        // Split inside the three-byte encoding of 合.
        assert!(lines.push(&line[..7]).is_empty());
        assert_eq!(lines.push(&line[7..]), vec!["合同".to_string()]);
    }

    // --- relay_sse (wiremock) ---

    fn sse_body(events: &[serde_json::Value]) -> String {
        events
            .iter()
            .map(|e| format!("event: message\r\ndata: {}\r\n\r\n", e))
            .collect()
    }

    async fn relay_against(body: String, status: u16) -> (Result<String, String>, Vec<String>, MockServer) {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/chats/c/stream"))
            .respond_with(
                ResponseTemplate::new(status)
                    .insert_header("content-type", "text/event-stream")
                    .set_body_string(body),
            )
            .mount(&server)
            .await;
        let mut deltas = Vec::new();
        let result = relay_sse(
            &Client::new(),
            &format!("{}/chats/c/stream", server.uri()),
            "test-secret",
            serde_json::json!({"text": "hi"}),
            &STREAM_TIMEOUTS,
            |d| {
                deltas.push(d);
                Ok(())
            },
        )
        .await;
        (result, deltas, server)
    }

    #[tokio::test]
    async fn relay_forwards_deltas_in_order_and_returns_done_text() {
        let body = sse_body(&[
            serde_json::json!({"type": "status", "value": "generating"}),
            serde_json::json!({"type": "delta", "value": "Hel"}),
            serde_json::json!({"type": "delta", "value": "lo"}),
            serde_json::json!({"type": "done", "value": "Hello"}),
        ]);
        let (result, deltas, _server) = relay_against(body, 200).await;
        assert_eq!(deltas, vec!["Hel".to_string(), "lo".to_string()]);
        assert_eq!(result.unwrap(), "Hello");
    }

    #[tokio::test]
    async fn relay_sends_secret_header_and_no_bearer_token() {
        let body = sse_body(&[serde_json::json!({"type": "done", "value": ""})]);
        let (_result, _deltas, server) = relay_against(body, 200).await;
        let requests = server.received_requests().await.unwrap();
        assert_eq!(requests.len(), 1);
        assert_eq!(requests[0].headers.get(SECRET_HEADER).unwrap(), "test-secret");
        assert!(requests[0].headers.get("authorization").is_none());
    }

    #[tokio::test]
    async fn relay_error_event_becomes_err_with_its_message() {
        let body = sse_body(&[
            serde_json::json!({"type": "delta", "value": "par"}),
            serde_json::json!({"type": "error", "value": "ollama unreachable"}),
        ]);
        let (result, _deltas, _server) = relay_against(body, 200).await;
        assert_eq!(result.unwrap_err(), "ollama unreachable");
    }

    #[tokio::test]
    async fn relay_non_success_status_becomes_err_with_status() {
        let (result, deltas, _server) =
            relay_against("{\"detail\":\"chat not found\"}".to_string(), 404).await;
        let err = result.unwrap_err();
        assert!(err.contains("(404)"), "got: {}", err);
        assert!(err.contains("chat not found"), "got: {}", err);
        assert!(deltas.is_empty());
    }

    #[tokio::test]
    async fn relay_stream_ending_without_terminal_event_is_err() {
        let body = sse_body(&[serde_json::json!({"type": "delta", "value": "cut"})]);
        let (result, _deltas, _server) = relay_against(body, 200).await;
        assert_eq!(result.unwrap_err(), "stream ended before completion");
    }

    /// A channel that records every message it is sent, as the JSON the webview would receive.
    fn recording_channel() -> (Channel<StreamEvent>, std::sync::Arc<Mutex<Vec<serde_json::Value>>>) {
        let sent = std::sync::Arc::new(Mutex::new(Vec::new()));
        let sink = sent.clone();
        let channel = Channel::new(move |body| {
            if let tauri::ipc::InvokeResponseBody::Json(json) = body {
                sink.lock().unwrap().push(serde_json::from_str(&json).unwrap());
            }
            Ok(())
        });
        (channel, sent)
    }

    const CHAT_ID: &str = "78072e58-05d6-496b-820d-9e09d9e7d0cc";

    #[tokio::test]
    async fn stream_chat_sends_deltas_then_exactly_one_done() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path(format!("/chats/{}/stream", CHAT_ID)))
            .respond_with(ResponseTemplate::new(200).set_body_string(sse_body(&[
                serde_json::json!({"type": "delta", "value": "Hel"}),
                serde_json::json!({"type": "delta", "value": "lo"}),
                serde_json::json!({"type": "done", "value": "Hello"}),
            ])))
            .mount(&server)
            .await;
        let state = make_state(BackendMode::Ollama, Some(server.address().port()));
        let (channel, sent) = recording_channel();

        stream_chat(&state, CHAT_ID.into(), "hi".into(), None, &channel).await.unwrap();

        assert_eq!(
            *sent.lock().unwrap(),
            vec![
                serde_json::json!({"type": "delta", "value": "Hel"}),
                serde_json::json!({"type": "delta", "value": "lo"}),
                serde_json::json!({"type": "done", "value": "Hello"}),
            ]
        );
    }

    #[tokio::test]
    async fn stream_chat_rejects_a_non_uuid_chat_id_with_one_error_event() {
        let state = make_state(BackendMode::Ollama, Some(1));
        let (channel, sent) = recording_channel();

        stream_chat(&state, "../health".into(), "hi".into(), None, &channel).await.unwrap();

        assert_eq!(
            *sent.lock().unwrap(),
            vec![serde_json::json!({"type": "error", "value": "invalid chat id"})]
        );
    }

    #[tokio::test]
    async fn stream_chat_without_a_sidecar_sends_one_error_event() {
        let state = make_state(BackendMode::Ollama, None);
        let (channel, sent) = recording_channel();

        stream_chat(&state, CHAT_ID.into(), "hi".into(), None, &channel).await.unwrap();

        assert_eq!(
            *sent.lock().unwrap(),
            vec![serde_json::json!({"type": "error", "value": "sidecar not running"})]
        );
    }

    /// A one-shot SSE server that writes each (delay, event) chunk after its
    /// delay, so a test can control the gaps between chunks.
    fn slow_sse_server(chunks: Vec<(u64, serde_json::Value)>) -> String {
        use std::io::{Read, Write};
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}/chats/c/stream", listener.local_addr().unwrap());
        std::thread::spawn(move || {
            let (mut socket, _) = listener.accept().unwrap();
            let mut request = [0u8; 8192];
            let _ = socket.read(&mut request);
            let _ = socket.write_all(
                b"HTTP/1.1 200 OK\r\ncontent-type: text/event-stream\r\ntransfer-encoding: chunked\r\n\r\n",
            );
            for (delay_ms, event) in chunks {
                std::thread::sleep(Duration::from_millis(delay_ms));
                let data = format!("data: {}\r\n\r\n", event);
                let _ = socket.write_all(format!("{:x}\r\n{}\r\n", data.len(), data).as_bytes());
            }
            let _ = socket.write_all(b"0\r\n\r\n");
        });
        url
    }

    const TEST_TIMEOUTS: StreamTimeouts = StreamTimeouts {
        idle: Duration::from_millis(300),
        total: Duration::from_secs(10),
    };

    #[tokio::test]
    async fn relay_stream_that_keeps_sending_outlasts_the_idle_timeout() {
        // Six gaps of 100ms: 600ms in all, twice the idle timeout, never idle.
        let mut chunks: Vec<(u64, serde_json::Value)> = (0..5)
            .map(|i| (100, serde_json::json!({"type": "delta", "value": i.to_string()})))
            .collect();
        chunks.push((100, serde_json::json!({"type": "done", "value": "01234"})));
        let url = slow_sse_server(chunks);

        let result =
            relay_sse(&Client::new(), &url, "s", serde_json::json!({}), &TEST_TIMEOUTS, |_| Ok(())).await;

        assert_eq!(result.unwrap(), "01234");
    }

    #[tokio::test]
    async fn relay_stream_that_goes_silent_ends_with_the_idle_message() {
        let url = slow_sse_server(vec![
            (0, serde_json::json!({"type": "status", "value": "generating"})),
            (1000, serde_json::json!({"type": "done", "value": "too late"})),
        ]);

        let result =
            relay_sse(&Client::new(), &url, "s", serde_json::json!({}), &TEST_TIMEOUTS, |_| Ok(())).await;

        assert_eq!(result.unwrap_err(), STREAM_IDLE_MESSAGE);
    }

    #[tokio::test]
    async fn stream_chat_posts_the_text_and_model_to_the_sidecar() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path(format!("/chats/{}/stream", CHAT_ID)))
            .and(body_json(serde_json::json!({"text": "hi", "model": "llama3.2"})))
            .respond_with(ResponseTemplate::new(200).set_body_string(sse_body(&[
                serde_json::json!({"type": "done", "value": "ok"}),
            ])))
            .mount(&server)
            .await;
        let state = make_state(BackendMode::Ollama, Some(server.address().port()));
        let (channel, sent) = recording_channel();

        stream_chat(&state, CHAT_ID.into(), "hi".into(), Some("llama3.2".into()), &channel)
            .await
            .unwrap();

        assert_eq!(
            *sent.lock().unwrap(),
            vec![serde_json::json!({"type": "done", "value": "ok"})]
        );
    }

    #[test]
    fn stream_event_serializes_as_type_and_value() {
        assert_eq!(
            serde_json::to_value(StreamEvent::Delta("x".into())).unwrap(),
            serde_json::json!({"type": "delta", "value": "x"})
        );
        assert_eq!(
            serde_json::to_value(StreamEvent::Error("e".into())).unwrap(),
            serde_json::json!({"type": "error", "value": "e"})
        );
    }

    // --- validate_user_path ---

    #[test]
    fn validate_user_path_accepts_home_subdir() {
        let home = dirs::home_dir().expect("need $HOME");
        let dir = tempfile::tempdir_in(&home).unwrap();
        let result = validate_user_path(&dir.path().to_string_lossy());
        assert!(result.is_ok());
    }

    #[test]
    fn validate_user_path_rejects_etc_passwd() {
        let result = validate_user_path("/etc/passwd");
        assert!(result.is_err());
        assert!(
            result.unwrap_err().contains("outside allowed directories"),
            "should reject /etc/passwd"
        );
    }

    #[test]
    fn validate_user_path_rejects_traversal() {
        let home = dirs::home_dir().expect("need $HOME");
        let traversal = format!("{}/../../../etc/passwd", home.display());
        let result = validate_user_path(&traversal);
        assert!(result.is_err());
    }

    #[test]
    fn validate_user_path_rejects_nonexistent() {
        let result = validate_user_path("/nonexistent/path/xyz");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("invalid path"));
    }
}
