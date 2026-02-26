//! ABOUTME: hosts ivory tauri commands and runtime wiring.
//! ABOUTME: coordinates tray icon, API proxy, keychain, filesystem commands, and plugins.
use std::path::PathBuf;
use std::sync::Mutex;

use reqwest::Client;
use tauri::image::Image;
use tauri::path::BaseDirectory;
use tauri::tray::TrayIconBuilder;
use tauri::{AppHandle, Manager, State};
use trash::delete;
use walkdir::WalkDir;

const SERVICE_NAME: &str = "elefant-ivory";
const DEFAULT_CLOUD_URL: &str = "https://api.elefant.com";
const DEFAULT_SIDECAR_URL: &str = "http://127.0.0.1:11435";

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
}

impl AppState {
    fn base_url(&self) -> String {
        let mode = self.mode.lock().unwrap();
        match *mode {
            BackendMode::Premium => DEFAULT_CLOUD_URL.to_string(),
            _ => DEFAULT_SIDECAR_URL.to_string(),
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
    let base = state.base_url();
    let url = format!("{}{}", base, path);

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
        .manage(AppState {
            mode: Mutex::new(BackendMode::Premium),
            http: Client::new(),
        })
        .setup(|app| {
            let handle = app.handle();
            if let Some(icon) = resolve_tray_icon(&handle) {
                let tray = TrayIconBuilder::new()
                    .icon(icon)
                    .icon_as_template(false)
                    .tooltip("ivory")
                    .build(app)?;
                app.manage(tray);
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
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
