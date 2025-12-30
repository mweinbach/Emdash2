use serde_json::{json, Value};
use std::path::PathBuf;

use crate::storage;

const TELEMETRY_FILE: &str = "telemetry.json";

fn telemetry_path(app: &tauri::AppHandle) -> PathBuf {
  storage::config_file(app, TELEMETRY_FILE)
}

fn ensure_instance_id(state: &mut Value) {
  let needs_id = match state.get("instanceId") {
    Some(Value::String(s)) => s.trim().is_empty(),
    _ => true,
  };
  if needs_id {
    let id = uuid::Uuid::new_v4().to_string();
    if let Some(obj) = state.as_object_mut() {
      obj.insert("instanceId".to_string(), Value::String(id));
    }
  }
}

fn ensure_bool(state: &mut Value, key: &str, fallback: bool) -> bool {
  let current = match state.get(key) {
    Some(Value::Bool(b)) => Some(*b),
    _ => None,
  };
  let value = current.unwrap_or(fallback);
  if let Some(obj) = state.as_object_mut() {
    obj.insert(key.to_string(), Value::Bool(value));
  }
  value
}

fn load_state(app: &tauri::AppHandle) -> Value {
  let path = telemetry_path(app);
  let mut state = storage::read_json(&path).unwrap_or_else(|| json!({}));
  if !state.is_object() {
    state = json!({});
  }
  ensure_instance_id(&mut state);
  ensure_bool(&mut state, "enabled", true);
  ensure_bool(&mut state, "onboardingSeen", false);
  state
}

fn save_state(app: &tauri::AppHandle, state: &Value) {
  let path = telemetry_path(app);
  let _ = storage::write_json(&path, state);
}

fn status_from_state(state: &Value) -> Value {
  let enabled = state.get("enabled").and_then(Value::as_bool).unwrap_or(true);
  let onboarding_seen = state
    .get("onboardingSeen")
    .and_then(Value::as_bool)
    .unwrap_or(false);
  json!({
    "enabled": enabled,
    "envDisabled": false,
    "userOptOut": !enabled,
    "hasKeyAndHost": false,
    "onboardingSeen": onboarding_seen
  })
}

pub fn get_status(app: &tauri::AppHandle) -> Value {
  let state = load_state(app);
  status_from_state(&state)
}

pub fn set_enabled(app: &tauri::AppHandle, enabled: bool) -> Value {
  let mut state = load_state(app);
  if let Some(obj) = state.as_object_mut() {
    obj.insert("enabled".to_string(), Value::Bool(enabled));
  }
  save_state(app, &state);
  status_from_state(&state)
}

pub fn set_onboarding_seen(app: &tauri::AppHandle, flag: bool) -> Value {
  let mut state = load_state(app);
  if let Some(obj) = state.as_object_mut() {
    obj.insert("onboardingSeen".to_string(), Value::Bool(flag));
  }
  save_state(app, &state);
  status_from_state(&state)
}
