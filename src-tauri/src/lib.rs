//! ABOUTME: hosts rumble tauri commands and runtime wiring.
//! ABOUTME: coordinates tray icon, API proxy, keychain, sidecar lifecycle, and plugins.
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::Duration;

use reqwest::Client;
use tauri::image::Image;
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
use tauri_plugin_dialog::DialogExt;
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

    let data_dir = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("failed to resolve app data dir: {}", e))?;
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
            sidecar_secret,
            // Document export
            export_draft_docx,
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
