use serde::{Deserialize, Serialize};
use std::{collections::HashMap, fs, path::PathBuf};
use tauri_plugin_autostart::ManagerExt;

fn default_server_url() -> String {
    "http://localhost:20128/v1".into()
}
fn default_refresh_interval() -> u64 {
    300
}
fn default_notifications() -> bool {
    true
}
fn default_language() -> String {
    "en".into()
}
fn default_remember_password() -> bool {
    true
}
fn default_quota_thresholds() -> Vec<f64> {
    vec![20.0, 5.0]
}
fn default_credit_threshold() -> f64 {
    0.5
}
fn default_provider_sort() -> String {
    "dashboard".into()
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct AppSettings {
    #[serde(default = "default_server_url")]
    pub server_url: String,
    #[serde(default = "default_refresh_interval")]
    pub refresh_interval_seconds: u64,
    #[serde(default = "default_notifications")]
    pub enable_notifications: bool,
    #[serde(default)]
    pub open_on_startup: bool,
    #[serde(default = "default_language")]
    pub language: String,
    #[serde(default)]
    pub credit_budgets: HashMap<String, f64>,
    #[serde(default)]
    pub hide_inactive: bool,
    #[serde(default = "default_remember_password")]
    pub remember_password: bool,
    #[serde(default = "default_quota_thresholds")]
    pub quota_thresholds: Vec<f64>,
    #[serde(default = "default_credit_threshold")]
    pub credit_threshold: f64,
    #[serde(default = "default_provider_sort")]
    pub provider_sort: String,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            server_url: default_server_url(),
            refresh_interval_seconds: default_refresh_interval(),
            enable_notifications: true,
            open_on_startup: false,
            language: default_language(),
            credit_budgets: HashMap::new(),
            hide_inactive: false,
            remember_password: true,
            quota_thresholds: default_quota_thresholds(),
            credit_threshold: default_credit_threshold(),
            provider_sort: default_provider_sort(),
        }
    }
}

pub fn get_settings_path() -> PathBuf {
    let base = std::env::var("APPDATA")
        .or_else(|_| std::env::var("HOME"))
        .unwrap_or_else(|_| ".".into());
    let base = PathBuf::from(base);
    let dir = base.join("9RouterUsage");
    let path = dir.join("settings.json");
    if !path.exists() {
        let old_path = base.join("9RouterMonitor").join("settings.json");
        if old_path.exists() {
            let _ = fs::create_dir_all(&dir);
            let _ = fs::copy(old_path, &path);
        }
    }
    let _ = fs::create_dir_all(&dir);
    path
}

pub fn read_settings() -> AppSettings {
    fs::read_to_string(get_settings_path())
        .ok()
        .and_then(|v| serde_json::from_str(&v).ok())
        .unwrap_or_default()
}

pub fn write_settings(settings: &AppSettings) -> Result<(), String> {
    let content = serde_json::to_string_pretty(settings).map_err(|e| e.to_string())?;
    fs::write(get_settings_path(), content).map_err(|e| e.to_string())
}

#[cfg(windows)]
fn credential_account(server_url: &str) -> String {
    reqwest::Url::parse(server_url)
        .ok()
        .and_then(|url| url.host_str().map(str::to_lowercase))
        .unwrap_or_else(|| server_url.to_lowercase())
}

#[cfg(windows)]
fn password_entry(server_url: &str) -> Result<keyring::Entry, String> {
    keyring::Entry::new("9Router Usage", &credential_account(server_url)).map_err(|e| e.to_string())
}

#[cfg(windows)]
fn legacy_password_entry() -> Result<keyring::Entry, String> {
    keyring::Entry::new("9Router Usage", "dashboard-password").map_err(|e| e.to_string())
}

#[cfg(windows)]
fn old_password_entry() -> Result<keyring::Entry, String> {
    keyring::Entry::new("9Router Monitor", "dashboard-password").map_err(|e| e.to_string())
}

#[cfg(windows)]
pub fn save_password(server_url: &str, password: &str) -> Result<(), String> {
    password_entry(server_url)?
        .set_password(password)
        .map_err(|e| e.to_string())
}

#[cfg(not(windows))]
pub fn save_password(_server_url: &str, _password: &str) -> Result<(), String> {
    Ok(())
}

#[cfg(windows)]
pub fn read_password(server_url: &str) -> Option<String> {
    if let Ok(password) = password_entry(server_url).ok()?.get_password() {
        return Some(password);
    }
    let password = legacy_password_entry()
        .ok()
        .and_then(|entry| entry.get_password().ok())
        .or_else(|| old_password_entry().ok()?.get_password().ok())?;
    let _ = password_entry(server_url)
        .and_then(|entry| entry.set_password(&password).map_err(|e| e.to_string()));
    Some(password)
}

#[cfg(not(windows))]
pub fn read_password(_server_url: &str) -> Option<String> {
    None
}

#[cfg(windows)]
pub fn delete_password(server_url: &str) {
    if let Ok(entry) = password_entry(server_url) {
        let _ = entry.delete_credential();
    }
    if let Ok(entry) = legacy_password_entry() {
        let _ = entry.delete_credential();
    }
    if let Ok(entry) = old_password_entry() {
        let _ = entry.delete_credential();
    }
}

#[cfg(not(windows))]
pub fn delete_password(_server_url: &str) {}

#[tauri::command]
pub fn get_settings() -> AppSettings {
    read_settings()
}

#[tauri::command]
pub fn save_settings(app_handle: tauri::AppHandle, settings: AppSettings) -> Result<(), String> {
    let mut settings = settings;
    settings.server_url = settings.server_url.trim().trim_end_matches('/').to_string();
    if settings.refresh_interval_seconds < 60 {
        settings.refresh_interval_seconds = 60;
    }
    settings
        .credit_budgets
        .retain(|_, value| value.is_finite() && *value > 0.0);
    let manager = app_handle.autolaunch();
    if settings.open_on_startup {
        manager.enable().map_err(|e| format!("Gagal mengaktifkan startup: {e}"))?;
    } else {
        manager.disable().map_err(|e| format!("Gagal menonaktifkan startup: {e}"))?;
    }
    write_settings(&settings)
}
