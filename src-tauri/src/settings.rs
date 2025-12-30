use serde_json::{json, Map, Value};
use std::path::PathBuf;
use tauri::Manager;

use crate::storage;

const SETTINGS_FILE: &str = "settings.json";

fn default_projects_dir(app: &tauri::AppHandle) -> String {
  if let Ok(home) = app.path().home_dir() {
    return home.join("emdash-projects").to_string_lossy().to_string();
  }
  "emdash-projects".to_string()
}

fn default_settings(app: &tauri::AppHandle) -> Value {
  json!({
    "repository": {
      "branchTemplate": "agent/{slug}-{timestamp}",
      "pushOnCreate": true
    },
    "projectPrep": {
      "autoInstallOnOpenInEditor": true
    },
    "browserPreview": {
      "enabled": true,
      "engine": "chromium"
    },
    "notifications": {
      "enabled": true,
      "sound": true
    },
    "mcp": {
      "context7": {
        "enabled": false,
        "installHintsDismissed": {}
      }
    },
    "defaultProvider": "claude",
    "tasks": {
      "autoGenerateName": true,
      "autoApproveByDefault": false
    },
    "projects": {
      "defaultDirectory": default_projects_dir(app)
    }
  })
}

fn settings_path(app: &tauri::AppHandle) -> PathBuf {
  storage::config_file(app, SETTINGS_FILE)
}

fn merge_value(base: &mut Value, patch: &Value) {
  match (base, patch) {
    (Value::Object(base_map), Value::Object(patch_map)) => {
      for (key, value) in patch_map {
        match base_map.get_mut(key) {
          Some(existing) => merge_value(existing, value),
          None => {
            base_map.insert(key.clone(), value.clone());
          }
        }
      }
    }
    (base_value, patch_value) => {
      *base_value = patch_value.clone();
    }
  }
}

fn coerce_bool(value: Option<&Value>, fallback: bool) -> bool {
  value.and_then(|v| v.as_bool()).unwrap_or(fallback)
}

fn coerce_string(value: Option<&Value>, fallback: &str) -> String {
  match value.and_then(|v| v.as_str()) {
    Some(s) if !s.trim().is_empty() => s.to_string(),
    _ => fallback.to_string(),
  }
}

fn normalize_settings(value: Value, app: &tauri::AppHandle) -> Value {
  let mut defaults = default_settings(app);
  merge_value(&mut defaults, &value);

  let obj = match defaults.as_object_mut() {
    Some(map) => map,
    None => {
      return default_settings(app);
    }
  };

  if let Some(repo) = obj.get_mut("repository").and_then(Value::as_object_mut) {
    let default_repo = default_settings(app)
      .get("repository")
      .and_then(Value::as_object)
      .cloned()
      .unwrap_or_else(Map::new);
    let template = coerce_string(repo.get("branchTemplate"), "agent/{slug}-{timestamp}");
    let trimmed = template.trim();
    let limited = if trimmed.len() > 200 {
      trimmed[..200].to_string()
    } else {
      trimmed.to_string()
    };
    repo.insert("branchTemplate".to_string(), Value::String(limited));
    let fallback_push = default_repo
      .get("pushOnCreate")
      .and_then(|v| v.as_bool())
      .unwrap_or(true);
    repo.insert(
      "pushOnCreate".to_string(),
      Value::Bool(coerce_bool(repo.get("pushOnCreate"), fallback_push)),
    );
  }

  if let Some(project_prep) = obj.get_mut("projectPrep").and_then(Value::as_object_mut) {
    let fallback = true;
    project_prep.insert(
      "autoInstallOnOpenInEditor".to_string(),
      Value::Bool(coerce_bool(
        project_prep.get("autoInstallOnOpenInEditor"),
        fallback,
      )),
    );
  }

  if let Some(browser_preview) = obj.get_mut("browserPreview").and_then(Value::as_object_mut) {
    browser_preview.insert(
      "enabled".to_string(),
      Value::Bool(coerce_bool(browser_preview.get("enabled"), true)),
    );
    browser_preview.insert("engine".to_string(), Value::String("chromium".to_string()));
  }

  if let Some(notifications) = obj.get_mut("notifications").and_then(Value::as_object_mut) {
    notifications.insert(
      "enabled".to_string(),
      Value::Bool(coerce_bool(notifications.get("enabled"), true)),
    );
    notifications.insert(
      "sound".to_string(),
      Value::Bool(coerce_bool(notifications.get("sound"), true)),
    );
  }

  if let Some(mcp) = obj.get_mut("mcp").and_then(Value::as_object_mut) {
    if let Some(context7) = mcp.get_mut("context7").and_then(Value::as_object_mut) {
      context7.insert(
        "enabled".to_string(),
        Value::Bool(coerce_bool(context7.get("enabled"), false)),
      );
      if !context7.get("installHintsDismissed").map_or(false, |v| v.is_object()) {
        context7.insert("installHintsDismissed".to_string(), json!({}));
      }
    }
  }

  if let Some(tasks) = obj.get_mut("tasks").and_then(Value::as_object_mut) {
    tasks.insert(
      "autoGenerateName".to_string(),
      Value::Bool(coerce_bool(tasks.get("autoGenerateName"), true)),
    );
    tasks.insert(
      "autoApproveByDefault".to_string(),
      Value::Bool(coerce_bool(tasks.get("autoApproveByDefault"), false)),
    );
  }

  if let Some(projects) = obj.get_mut("projects").and_then(Value::as_object_mut) {
    let default_dir = default_projects_dir(app);
    let dir = coerce_string(projects.get("defaultDirectory"), &default_dir);
    projects.insert("defaultDirectory".to_string(), Value::String(dir));
  }

  if let Some(default_provider) = obj.get("defaultProvider") {
    if default_provider.is_null() {
      obj.insert("defaultProvider".to_string(), Value::String("claude".to_string()));
    }
  }

  Value::Object(obj.clone())
}

pub fn load_settings(app: &tauri::AppHandle) -> Value {
  let path = settings_path(app);
  let mut base = default_settings(app);
  if let Some(existing) = storage::read_json(&path) {
    merge_value(&mut base, &existing);
  }
  normalize_settings(base, app)
}

pub fn update_settings(app: &tauri::AppHandle, patch: Value) -> Value {
  let mut current = load_settings(app);
  merge_value(&mut current, &patch);
  let normalized = normalize_settings(current, app);
  let path = settings_path(app);
  let _ = storage::write_json(&path, &normalized);
  normalized
}
