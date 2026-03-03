//! ABOUTME: hosts ivory tauri commands and runtime wiring.
//! ABOUTME: coordinates tray icon, API proxy, keychain, sidecar lifecycle, and plugins.
use std::path::PathBuf;
use std::sync::Mutex;

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

const SERVICE_NAME: &str = "elefant-ivory";
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
}

impl AppState {
    /// Route a request path to the correct backend URL.
    /// Premium mode: always cloud. Ollama/BYOK: cloud-only paths go to cloud,
    /// everything else goes to the local sidecar.
    fn resolve_url(&self, path: &str) -> Result<String, String> {
        let mode = self.mode.lock().unwrap();
        if *mode == BackendMode::Premium {
            return Ok(format!("{}{}", DEFAULT_CLOUD_URL, path));
        }

        let is_cloud_only = CLOUD_ONLY_PREFIXES
            .iter()
            .any(|prefix| path == *prefix || path.starts_with(&format!("{}/", prefix)));

        if is_cloud_only {
            return Ok(format!("{}{}", DEFAULT_CLOUD_URL, path));
        }

        let port = self.sidecar_port.lock().unwrap();
        match *port {
            Some(p) => Ok(format!("http://127.0.0.1:{}{}", p, path)),
            None => Err("sidecar not running".to_string()),
        }
    }
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
    let mut files = Vec::new();
    for entry in WalkDir::new(path).into_iter().filter_map(|e| e.ok()) {
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
        }
    }
    Ok(files)
}

#[tauri::command]
fn move_to_trash(path: String) -> Result<(), String> {
    let pathbuf = PathBuf::from(path);
    delete(&pathbuf).map_err(|e| e.to_string())
}

// --- API proxy ---

#[tauri::command]
async fn api_call(
    state: State<'_, AppState>,
    method: String,
    path: String,
    body: Option<serde_json::Value>,
    params: Option<serde_json::Value>,
) -> Result<serde_json::Value, String> {
    let url = state.resolve_url(&path)?;

    let mut builder = match method.to_uppercase().as_str() {
        "GET" => state.http.get(&url),
        "POST" => state.http.post(&url),
        "PUT" => state.http.put(&url),
        "PATCH" => state.http.patch(&url),
        "DELETE" => state.http.delete(&url),
        other => return Err(format!("unsupported HTTP method: {}", other)),
    };

    // Inject auth token if available
    if let Ok(token) = auth_get_token_inner() {
        builder = builder.bearer_auth(&token);
    }

    // Attach query params
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

    // Attach JSON body
    if let Some(data) = body {
        builder = builder.json(&data);
    }

    let response = builder.send().await.map_err(|e| e.to_string())?;
    let status = response.status();

    if status.is_success() {
        response.json().await.map_err(|e| e.to_string())
    } else {
        let text = response.text().await.unwrap_or_default();
        Err(format!("API error {}: {}", status.as_u16(), text))
    }
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

#[tauri::command]
fn store_api_key(provider: String, key: String) -> Result<(), String> {
    let entry_name = format!("byok_{}", provider);
    keyring_entry(&entry_name)?
        .set_password(&key)
        .map_err(|e| e.to_string())
}

#[tauri::command]
fn get_api_key(provider: String) -> Result<Option<String>, String> {
    let entry_name = format!("byok_{}", provider);
    match keyring_entry(&entry_name)?.get_password() {
        Ok(k) => Ok(Some(k)),
        Err(keyring::Error::NoEntry) => Ok(None),
        Err(e) => Err(e.to_string()),
    }
}

// --- Backend mode commands ---

#[tauri::command]
fn get_backend_mode(state: State<'_, AppState>) -> String {
    let mode = state.mode.lock().unwrap();
    serde_json::to_string(&*mode).unwrap_or_else(|_| "\"premium\"".to_string())
}

#[tauri::command]
fn set_backend_mode(state: State<'_, AppState>, mode: String) -> Result<(), String> {
    let parsed: BackendMode =
        serde_json::from_str(&format!("\"{}\"", mode)).map_err(|e| e.to_string())?;
    let mut current = state.mode.lock().unwrap();
    *current = parsed;
    Ok(())
}

// --- Sidecar lifecycle ---

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

async fn poll_health(http: &Client, port: u16) -> Result<(), String> {
    let url = format!("http://127.0.0.1:{}/health", port);
    for attempt in 1..=HEALTH_POLL_ATTEMPTS {
        tokio::time::sleep(std::time::Duration::from_millis(HEALTH_POLL_INTERVAL_MS)).await;
        match http.get(&url).send().await {
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

    let sidecar_cmd = app
        .shell()
        .sidecar("ivory-sidecar")
        .map_err(|e| format!("failed to create sidecar command: {}", e))?
        .args(["--port", &port.to_string()]);

    let (mut rx, child) = sidecar_cmd
        .spawn()
        .map_err(|e| format!("failed to spawn sidecar: {}", e))?;

    // Store the child process handle for cleanup
    {
        let mut sidecar_child = state.sidecar_child.lock().unwrap();
        *sidecar_child = Some(child);
    }

    // Read stdout in a background task, looking for PORT: confirmation
    let port_clone = port;
    let http_clone = state.http.clone();
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
                                // Poll health before confirming
                                match poll_health(&http_clone, p).await {
                                    Ok(()) => {
                                        let st = state_handle.state::<AppState>();
                                        let mut sp = st.sidecar_port.lock().unwrap();
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
                    let mut sp = st.sidecar_port.lock().unwrap();
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
    let mut child = state.sidecar_child.lock().unwrap();
    if let Some(c) = child.take() {
        println!("[sidecar] shutting down");
        let _ = c.kill();
    }
    let mut port = state.sidecar_port.lock().unwrap();
    *port = None;
}

#[tauri::command]
async fn sidecar_status(state: State<'_, AppState>) -> Result<serde_json::Value, String> {
    let port = state.sidecar_port.lock().unwrap().clone();
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

// --- App entry ---

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_shell::init())
        .manage(AppState {
            mode: Mutex::new(BackendMode::Premium),
            http: Client::new(),
            sidecar_port: Mutex::new(None),
            sidecar_child: Mutex::new(None),
        })
        .setup(|app| {
            let handle = app.handle();

            // Tray icon
            if let Some(icon) = resolve_tray_icon(&handle) {
                let tray = TrayIconBuilder::new()
                    .icon(icon)
                    .icon_as_template(false)
                    .tooltip("ivory")
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
            // Auth / keychain
            auth_store_token,
            auth_get_token,
            auth_clear_token,
            store_api_key,
            get_api_key,
            // Backend mode
            get_backend_mode,
            set_backend_mode,
            // Sidecar
            sidecar_status,
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

// --- Unit tests for resolve_url ---

#[cfg(test)]
mod tests {
    use super::*;

    fn make_state(mode: BackendMode, port: Option<u16>) -> AppState {
        AppState {
            mode: Mutex::new(mode),
            http: Client::new(),
            sidecar_port: Mutex::new(port),
            sidecar_child: Mutex::new(None),
        }
    }

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
        // "/me" should be cloud-only, but "/models" should not
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
}
