use serde_json::Value;
use std::fs;
use std::path::{Path, PathBuf};
use tauri::Manager;

pub fn config_dir(app: &tauri::AppHandle) -> PathBuf {
  app
    .path()
    .app_config_dir()
    .ok()
    .or_else(|| app.path().app_data_dir().ok())
    .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")))
}

pub fn config_file(app: &tauri::AppHandle, name: &str) -> PathBuf {
  config_dir(app).join(name)
}

pub fn read_json(path: &Path) -> Option<Value> {
  let raw = fs::read_to_string(path).ok()?;
  serde_json::from_str(&raw).ok()
}

pub fn write_json(path: &Path, value: &Value) -> Result<(), String> {
  if let Some(parent) = path.parent() {
    if !parent.exists() {
      fs::create_dir_all(parent).map_err(|err| err.to_string())?;
    }
  }
  let data = serde_json::to_string_pretty(value).map_err(|err| err.to_string())?;
  fs::write(path, data).map_err(|err| err.to_string())
}
