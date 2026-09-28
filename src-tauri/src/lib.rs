//! ABOUTME: hosts rumble tauri commands and runtime wiring.
//! ABOUTME: coordinates tray icon, API proxy, keychain, sidecar lifecycle, and plugins.
use std::path::PathBuf;
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

const SERVICE_NAME: &str = "elefant-rumble";
const SECRET_HEADER: &str = "X-Rumble-Secret";
const DEFAULT_CLOUD_URL: &str = "https://api.elefant.com";
const HEALTH_POLL_ATTEMPTS: u32 = 10;
const HEALTH_POLL_INTERVAL_MS: u64 = 500;

/// Paths that only the cloud API can handle (billing, auth, search, etc.).
const CLOUD_ONLY_PREFIXES: &[&str] = &[
    "/billing",
    "/auth",
    "/users",
    "/organizations",
    "/notifications",
    "/webhooks",
    "/whoami",
    "/me",
    "/search",
    "/briefcase",
    "/documents",
    "/files",
    "/corpus",
    "/graph",
    "/playbooks",
    "/clause-databases",
    "/legal-requests",
    "/pipelines",
    "/entitlements",
    "/usage",
    "/commencement",
    "/reading-list",
    "/model-performance",
    "/memories",
    "/memory",
    "/orchestrate",
    "/citations",
    "/internal",
];

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
    secret: String,
}

/// Whether a resolved URL points at our own local sidecar (as opposed to
/// the cloud API) — used to decide whether to attach the shared secret.
fn is_sidecar_url(url: &str) -> bool {
    url.starts_with("http://127.0.0.1:")
}

impl AppState {
    /// Route a request path to the correct backend URL.
    /// Premium mode: always cloud. Ollama/BYOK: cloud-only paths go to cloud,
    /// everything else goes to the local sidecar.
    fn resolve_url(&self, path: &str) -> Result<String, String> {
        let mode = self.mode.lock().unwrap_or_else(|e| e.into_inner());
        if *mode == BackendMode::Premium {
            return Ok(format!("{}{}", DEFAULT_CLOUD_URL, path));
        }

        let is_cloud_only = CLOUD_ONLY_PREFIXES
            .iter()
            .any(|prefix| path == *prefix || path.starts_with(&format!("{}/", prefix)));

        if is_cloud_only {
            return Ok(format!("{}{}", DEFAULT_CLOUD_URL, path));
        }

        self.sidecar_url(path)
    }

    /// Build a URL against the local sidecar only, ignoring backend mode —
    /// for operations like document extraction that never go to the cloud.
    fn sidecar_url(&self, path: &str) -> Result<String, String> {
        let port = self.sidecar_port.lock().unwrap_or_else(|e| e.into_inner());
        match *port {
            Some(p) => Ok(format!("http://127.0.0.1:{}{}", p, path)),
            None => Err("sidecar not running".to_string()),
        }
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
    let url = state.resolve_url(&path)?;
    let token = match auth_get_token_inner() {
        Ok(t) => Some(t),
        Err(e) => {
            eprintln!("[keychain] failed to read auth token: {}", e);
            None
        }
    };
    // Only attach the shared secret when talking to our own local sidecar —
    // never send it to the cloud API.
    let secret = if is_sidecar_url(&url) {
        Some(state.secret.as_str())
    } else {
        None
    };
    make_http_request(&state.http, &url, &method, body, params, token.as_deref(), secret).await
}

const EXTRACT_FILENAME_HEADER: &str = "X-Rumble-Filename";

/// Posts a file's bytes to the sidecar's /extract endpoint as multipart form data.
///
/// Maps a 422 (the sidecar's ExtractionError response) to its `detail` message —
/// safe to show the user, since the sidecar only ever puts a human-readable
/// explanation there. Any other failure (connection error, 5xx, unreadable body)
/// collapses to a generic message; we never surface a raw response body from an
/// unexpected failure mode.
async fn post_extract_multipart(
    http: &Client,
    sidecar_url: &str,
    secret: &str,
    filename: &str,
    data: Vec<u8>,
) -> Result<String, String> {
    let part = reqwest::multipart::Part::bytes(data).file_name(filename.to_string());
    let form = reqwest::multipart::Form::new().part("file", part);

    let response = http
        .post(format!("{}/extract", sidecar_url))
        .header(SECRET_HEADER, secret)
        .multipart(form)
        .send()
        .await
        .map_err(|_| "could not reach the sidecar".to_string())?;

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

    if status.as_u16() == 422 {
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
    post_extract_multipart(&state.http, &sidecar_url, &state.secret, &filename, data).await
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

#[tauri::command]
fn store_api_key(provider: String, key: String) -> Result<(), String> {
    validate_provider(&provider)?;
    let entry_name = format!("byok_{}", provider);
    keyring_entry(&entry_name)?
        .set_password(&key)
        .map_err(|e| e.to_string())
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
fn delete_api_key(provider: String) -> Result<(), String> {
    validate_provider(&provider)?;
    let entry_name = format!("byok_{}", provider);
    match keyring_entry(&entry_name)?.delete_credential() {
        Ok(()) => Ok(()),
        Err(keyring::Error::NoEntry) => Ok(()),
        Err(e) => Err(e.to_string()),
    }
}

// --- Backend mode commands ---

#[tauri::command]
fn get_backend_mode(state: State<'_, AppState>) -> BackendMode {
    let mode = state.mode.lock().unwrap_or_else(|e| e.into_inner());
    mode.clone()
}

#[tauri::command]
fn set_backend_mode(state: State<'_, AppState>, mode: String) -> Result<(), String> {
    let parsed = match mode.as_str() {
        "ollama" => BackendMode::Ollama,
        "byok" => BackendMode::Byok,
        "premium" => BackendMode::Premium,
        other => return Err(format!("unknown backend mode: {}", other)),
    };
    let mut current = state.mode.lock().unwrap_or_else(|e| e.into_inner());
    *current = parsed;
    Ok(())
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
        .env("RUMBLE_SIDECAR_SECRET", &state.secret);

    let (mut rx, child) = sidecar_cmd
        .spawn()
        .map_err(|e| format!("failed to spawn sidecar: {}", e))?;

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
                                        let mut sp = st.sidecar_port.lock().unwrap_or_else(|e| e.into_inner());
                                        *sp = Some(p);
                                        port_confirmed = true;
                                        println!("[sidecar] ready on port {}", p);
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
                    let st = state_handle.state::<AppState>();
                    let mut sp = st.sidecar_port.lock().unwrap_or_else(|e| e.into_inner());
                    *sp = None;
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
    let port = state.sidecar_port.lock().unwrap_or_else(|e| e.into_inner()).clone();
    let running = port.is_some();

    let mut result = serde_json::json!({
        "running": running,
        "port": port,
    });

    if let Some(p) = port {
        let url = format!("http://127.0.0.1:{}/health", p);
        match state.http.get(&url).send().await {
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

    Ok(result)
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

    let file = std::fs::File::create(&path).map_err(|e| format!("failed to create file: {}", e))?;

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
        .map_err(|e| format!("failed to write docx: {}", e))?;

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
            http: Client::builder()
                .timeout(Duration::from_secs(30))
                .build()
                .expect("failed to build HTTP client"),
            sidecar_port: Mutex::new(None),
            sidecar_child: Mutex::new(None),
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
            secret: "test-secret".to_string(),
        }
    }

    // --- resolve_url ---

    #[test]
    fn premium_always_routes_to_cloud() {
        let state = make_state(BackendMode::Premium, None);
        assert_eq!(
            state.resolve_url("/chats").unwrap(),
            "https://api.elefant.com/chats"
        );
        assert_eq!(
            state.resolve_url("/billing/subscription").unwrap(),
            "https://api.elefant.com/billing/subscription"
        );
    }

    #[test]
    fn ollama_routes_cloud_only_to_cloud() {
        let state = make_state(BackendMode::Ollama, Some(11435));
        assert_eq!(
            state.resolve_url("/billing").unwrap(),
            "https://api.elefant.com/billing"
        );
        assert_eq!(
            state.resolve_url("/billing/subscription").unwrap(),
            "https://api.elefant.com/billing/subscription"
        );
        assert_eq!(
            state.resolve_url("/search").unwrap(),
            "https://api.elefant.com/search"
        );
        assert_eq!(
            state.resolve_url("/auth/login").unwrap(),
            "https://api.elefant.com/auth/login"
        );
        assert_eq!(
            state.resolve_url("/users").unwrap(),
            "https://api.elefant.com/users"
        );
    }

    #[test]
    fn ollama_routes_sidecar_paths_to_sidecar() {
        let state = make_state(BackendMode::Ollama, Some(11435));
        assert_eq!(
            state.resolve_url("/health").unwrap(),
            "http://127.0.0.1:11435/health"
        );
        assert_eq!(
            state.resolve_url("/chats").unwrap(),
            "http://127.0.0.1:11435/chats"
        );
        assert_eq!(
            state.resolve_url("/chats/abc-123/message").unwrap(),
            "http://127.0.0.1:11435/chats/abc-123/message"
        );
        assert_eq!(
            state.resolve_url("/clarify").unwrap(),
            "http://127.0.0.1:11435/clarify"
        );
        assert_eq!(
            state.resolve_url("/translate").unwrap(),
            "http://127.0.0.1:11435/translate"
        );
        assert_eq!(
            state.resolve_url("/draft").unwrap(),
            "http://127.0.0.1:11435/draft"
        );
    }

    #[test]
    fn byok_routes_same_as_ollama() {
        let state = make_state(BackendMode::Byok, Some(8080));
        assert_eq!(
            state.resolve_url("/billing").unwrap(),
            "https://api.elefant.com/billing"
        );
        assert_eq!(
            state.resolve_url("/chats").unwrap(),
            "http://127.0.0.1:8080/chats"
        );
    }

    #[test]
    fn ollama_unknown_path_routes_to_sidecar() {
        let state = make_state(BackendMode::Ollama, Some(11435));
        assert_eq!(
            state.resolve_url("/some-future-endpoint").unwrap(),
            "http://127.0.0.1:11435/some-future-endpoint"
        );
    }

    #[test]
    fn ollama_no_sidecar_errors_for_sidecar_path() {
        let state = make_state(BackendMode::Ollama, None);
        assert!(state.resolve_url("/chats").is_err());
    }

    #[test]
    fn ollama_no_sidecar_still_routes_cloud_only() {
        let state = make_state(BackendMode::Ollama, None);
        assert_eq!(
            state.resolve_url("/billing").unwrap(),
            "https://api.elefant.com/billing"
        );
    }

    #[test]
    fn cloud_only_prefix_no_false_positive() {
        let state = make_state(BackendMode::Ollama, Some(11435));
        assert_eq!(
            state.resolve_url("/me").unwrap(),
            "https://api.elefant.com/me"
        );
        assert_eq!(
            state.resolve_url("/models").unwrap(),
            "http://127.0.0.1:11435/models"
        );
    }

    #[test]
    fn every_cloud_only_prefix_routes_to_cloud() {
        let state = make_state(BackendMode::Ollama, Some(11435));
        for prefix in CLOUD_ONLY_PREFIXES {
            let result = state.resolve_url(prefix).unwrap();
            assert!(
                result.starts_with(DEFAULT_CLOUD_URL),
                "{} should route to cloud, got: {}",
                prefix,
                result
            );
        }
    }

    #[test]
    fn every_cloud_only_prefix_with_suffix_routes_to_cloud() {
        let state = make_state(BackendMode::Ollama, Some(11435));
        for prefix in CLOUD_ONLY_PREFIXES {
            let path = format!("{}/sub-path", prefix);
            let result = state.resolve_url(&path).unwrap();
            assert!(
                result.starts_with(DEFAULT_CLOUD_URL),
                "{} should route to cloud, got: {}",
                path,
                result
            );
        }
    }

    #[test]
    fn partial_prefix_does_not_falsely_match() {
        let state = make_state(BackendMode::Ollama, Some(11435));
        // "/billing_extra" contains "/billing" as a substring but should NOT match
        let result = state.resolve_url("/billing_extra").unwrap();
        assert!(
            result.starts_with("http://127.0.0.1:11435"),
            "/billing_extra should NOT route to cloud, got: {}",
            result
        );
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
        )
        .await;
        let err = result.unwrap_err();
        assert_eq!(err, "extraction failed");
        assert!(!err.contains("stack trace"), "must not leak raw body: {}", err);
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
