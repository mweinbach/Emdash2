use serde::Deserialize;
use serde_json::{json, Map, Value};
use std::path::PathBuf;
use std::sync::OnceLock;
use std::time::{SystemTime, UNIX_EPOCH};
use tauri::Manager;

use crate::storage;

const TELEMETRY_FILE: &str = "telemetry.json";
const LIB_NAME: &str = "emdash";

const RENDERER_ALLOWED_EVENTS: &[&str] = &[
  "feature_used",
  "error",
  "project_add_clicked",
  "project_open_clicked",
  "project_added_success",
  "project_deleted",
  "project_view_opened",
  "task_created",
  "task_deleted",
  "task_provider_switched",
  "task_custom_named",
  "task_advanced_options_opened",
  "terminal_entered",
  "terminal_command_executed",
  "terminal_new_terminal_created",
  "terminal_deleted",
  "changes_viewed",
  "plan_mode_enabled",
  "plan_mode_disabled",
  "pr_created",
  "pr_creation_failed",
  "pr_viewed",
  "linear_connected",
  "linear_disconnected",
  "linear_issues_searched",
  "linear_issue_selected",
  "jira_connected",
  "jira_disconnected",
  "jira_issues_searched",
  "jira_issue_selected",
  "container_connect_clicked",
  "container_connect_success",
  "container_connect_failed",
  "toolbar_feedback_clicked",
  "toolbar_left_sidebar_clicked",
  "toolbar_right_sidebar_clicked",
  "toolbar_settings_clicked",
  "toolbar_open_in_menu_clicked",
  "toolbar_open_in_selected",
  "toolbar_kanban_toggled",
  "browser_preview_closed",
  "browser_preview_url_navigated",
  "settings_tab_viewed",
  "theme_changed",
  "telemetry_toggled",
  "notification_settings_changed",
  "default_provider_changed",
];

const ALLOWED_PROP_KEYS: &[&str] = &[
  "provider",
  "source",
  "tab",
  "theme",
  "trigger",
  "has_initial_prompt",
  "custom_name",
  "state",
  "success",
  "error_type",
  "gh_cli_installed",
  "feature",
  "type",
  "enabled",
  "sound",
  "app",
  "duration_ms",
  "session_duration_ms",
  "outcome",
  "applied_migrations",
  "applied_migrations_bucket",
  "recovered",
  "task_count",
  "task_count_bucket",
  "project_count",
  "project_count_bucket",
];

static SESSION_START_MS: OnceLock<i64> = OnceLock::new();

#[derive(Default, Deserialize)]
struct AppConfig {
  #[serde(rename = "posthogHost")]
  posthog_host: Option<String>,
  #[serde(rename = "posthogKey")]
  posthog_key: Option<String>,
}

struct TelemetryConfig {
  env_enabled: bool,
  api_key: Option<String>,
  host: Option<String>,
  install_source: Option<String>,
  is_dev: bool,
  app_version: String,
}

fn telemetry_path(app: &tauri::AppHandle) -> PathBuf {
  storage::config_file(app, TELEMETRY_FILE)
}

fn now_ms() -> i64 {
  SystemTime::now()
    .duration_since(UNIX_EPOCH)
    .unwrap_or_default()
    .as_millis() as i64
}

fn session_start_ms() -> i64 {
  *SESSION_START_MS.get_or_init(now_ms)
}

fn is_allowed_event(event: &str) -> bool {
  RENDERER_ALLOWED_EVENTS.iter().any(|ev| *ev == event)
}

fn sanitize_properties(props: Option<Value>) -> Map<String, Value> {
  let mut out = Map::new();
  let Some(Value::Object(map)) = props else {
    return out;
  };

  for (key, value) in map {
    if !ALLOWED_PROP_KEYS.iter().any(|allowed| *allowed == key) {
      continue;
    }
    match value {
      Value::String(s) => {
        let trimmed = s.trim();
        if !trimmed.is_empty() {
          let clipped = if trimmed.len() > 100 {
            trimmed[..100].to_string()
          } else {
            trimmed.to_string()
          };
          out.insert(key, Value::String(clipped));
        }
      }
      Value::Number(n) => {
        if let Some(val) = n.as_i64() {
          let clamped = val.max(0).min(1_000_000);
          out.insert(key, Value::Number(clamped.into()));
        } else if let Some(val) = n.as_f64() {
          let clamped = val.max(0.0).min(1_000_000.0);
          if let Some(num) = serde_json::Number::from_f64(clamped) {
            out.insert(key, Value::Number(num));
          }
        }
      }
      Value::Bool(b) => {
        out.insert(key, Value::Bool(b));
      }
      _ => {}
    }
  }

  out
}

fn normalize_host(raw: Option<String>) -> Option<String> {
  let Some(host) = raw else { return None };
  let mut s = host.trim().to_string();
  if s.is_empty() {
    return None;
  }
  if !s.starts_with("http://") && !s.starts_with("https://") {
    s = format!("https://{}", s);
  }
  let trimmed = s.trim_end_matches('/').to_string();
  if trimmed.is_empty() {
    None
  } else {
    Some(trimmed)
  }
}

fn load_app_config(app: &tauri::AppHandle) -> Option<AppConfig> {
  let mut candidates: Vec<PathBuf> = Vec::new();
  if let Ok(dir) = app.path().resource_dir() {
    candidates.push(dir.join("appConfig.json"));
  }
  if let Ok(cwd) = std::env::current_dir() {
    candidates.push(cwd.join("src-tauri/appConfig.json"));
    candidates.push(cwd.join("../src-tauri/appConfig.json"));
  }

  for path in candidates {
    if let Ok(raw) = std::fs::read_to_string(&path) {
      if let Ok(parsed) = serde_json::from_str::<AppConfig>(&raw) {
        return Some(parsed);
      }
    }
  }
  None
}

fn load_config(app: &tauri::AppHandle) -> TelemetryConfig {
  let enabled_env = std::env::var("TELEMETRY_ENABLED").unwrap_or_else(|_| "true".to_string());
  let enabled_env = match enabled_env.trim().to_lowercase().as_str() {
    "false" | "0" | "no" => false,
    _ => true,
  };

  let app_config = load_app_config(app).unwrap_or_default();
  let api_key = std::env::var("POSTHOG_PROJECT_API_KEY").ok().filter(|v| !v.trim().is_empty())
    .or(app_config.posthog_key);
  let host = normalize_host(
    std::env::var("POSTHOG_HOST").ok().filter(|v| !v.trim().is_empty())
      .or(app_config.posthog_host),
  );
  let install_source = std::env::var("INSTALL_SOURCE").ok().filter(|v| !v.trim().is_empty());

  TelemetryConfig {
    env_enabled: enabled_env,
    api_key,
    host,
    install_source,
    is_dev: cfg!(debug_assertions),
    app_version: app.package_info().version.to_string(),
  }
}

fn platform_string() -> &'static str {
  if cfg!(target_os = "macos") {
    "darwin"
  } else if cfg!(target_os = "windows") {
    "win32"
  } else {
    "linux"
  }
}

fn load_state(app: &tauri::AppHandle) -> Value {
  let path = telemetry_path(app);
  let mut state = storage::read_json(&path).unwrap_or_else(|| json!({}));
  let mut changed = false;
  if !state.is_object() {
    state = json!({});
    changed = true;
  }
  let obj = state.as_object_mut().expect("telemetry state must be object");

  let instance_id = match obj.get("instanceId").and_then(Value::as_str) {
    Some(value) if !value.trim().is_empty() => value.to_string(),
    _ => {
      let id = uuid::Uuid::new_v4().to_string();
      obj.insert("instanceId".to_string(), Value::String(id.clone()));
      changed = true;
      id
    }
  };

  if obj.get("onboardingSeen").and_then(Value::as_bool).is_none() {
    obj.insert("onboardingSeen".to_string(), Value::Bool(false));
    changed = true;
  }

  if obj.get("enabled").is_some() && obj.get("enabled").and_then(Value::as_bool).is_none() {
    obj.remove("enabled");
    changed = true;
  }

  if changed {
    let _ = storage::write_json(&path, &state);
  }

  let _ = instance_id; // keep alive for clarity
  state
}

fn save_state(app: &tauri::AppHandle, state: &Value) {
  let path = telemetry_path(app);
  let _ = storage::write_json(&path, state);
}

fn get_instance_id(state: &Value) -> String {
  state
    .get("instanceId")
    .and_then(Value::as_str)
    .unwrap_or("")
    .to_string()
}

fn get_onboarding_seen(state: &Value) -> bool {
  state
    .get("onboardingSeen")
    .and_then(Value::as_bool)
    .unwrap_or(false)
}

fn get_enabled_override(state: &Value) -> Option<bool> {
  state.get("enabled").and_then(Value::as_bool)
}

fn status_from_state(state: &Value, config: &TelemetryConfig) -> Value {
  let instance_id = get_instance_id(state);
  let user_opt_out = get_enabled_override(state).map(|enabled| !enabled).unwrap_or(false);
  let has_key_and_host = config.api_key.is_some() && config.host.is_some();
  let enabled =
    config.env_enabled && !user_opt_out && has_key_and_host && !instance_id.trim().is_empty();
  json!({
    "enabled": enabled,
    "envDisabled": !config.env_enabled,
    "userOptOut": user_opt_out,
    "hasKeyAndHost": has_key_and_host,
    "onboardingSeen": get_onboarding_seen(state)
  })
}

fn ensure_state_object(state: &mut Value) -> &mut Map<String, Value> {
  if !state.is_object() {
    *state = json!({});
  }
  state.as_object_mut().expect("telemetry state must be object")
}

fn merge_state(app: &tauri::AppHandle, update: impl FnOnce(&mut Map<String, Value>)) -> Value {
  let mut state = load_state(app);
  let obj = ensure_state_object(&mut state);
  update(obj);
  obj.insert(
    "updatedAt".to_string(),
    Value::String(chrono::Utc::now().to_rfc3339()),
  );
  if obj.get("createdAt").is_none() {
    obj.insert(
      "createdAt".to_string(),
      Value::String(chrono::Utc::now().to_rfc3339()),
    );
  }
  save_state(app, &state);
  state
}

fn build_base_props(config: &TelemetryConfig) -> Value {
  json!({
    "app_version": config.app_version,
    "electron_version": tauri::VERSION,
    "platform": platform_string(),
    "arch": std::env::consts::ARCH,
    "is_dev": config.is_dev,
    "install_source": config.install_source.clone().unwrap_or_else(|| if config.is_dev { "dev".to_string() } else { "dmg".to_string() }),
    "$lib": LIB_NAME
  })
}

pub fn get_status(app: &tauri::AppHandle) -> Value {
  let config = load_config(app);
  let state = load_state(app);
  status_from_state(&state, &config)
}

pub fn set_enabled(app: &tauri::AppHandle, enabled: bool) -> Value {
  let state = merge_state(app, |obj| {
    obj.insert("enabled".to_string(), Value::Bool(enabled));
  });
  let config = load_config(app);
  status_from_state(&state, &config)
}

pub fn set_onboarding_seen(app: &tauri::AppHandle, flag: bool) -> Value {
  let state = merge_state(app, |obj| {
    obj.insert("onboardingSeen".to_string(), Value::Bool(flag));
  });
  let config = load_config(app);
  status_from_state(&state, &config)
}

pub fn capture(app: &tauri::AppHandle, event: String, properties: Option<Value>) -> Value {
  let config = load_config(app);
  let state = load_state(app);
  let status = status_from_state(&state, &config);

  let enabled = status
    .get("enabled")
    .and_then(Value::as_bool)
    .unwrap_or(false);
  if !enabled {
    return json!({ "success": false, "disabled": true });
  }

  if !is_allowed_event(event.as_str()) {
    return json!({ "success": false, "error": "event_not_allowed" });
  }

  let Some(api_key) = config.api_key.clone() else {
    return json!({ "success": false, "disabled": true });
  };
  let Some(host) = config.host.clone() else {
    return json!({ "success": false, "disabled": true });
  };

  let instance_id = get_instance_id(&state);
  if instance_id.trim().is_empty() {
    return json!({ "success": false, "disabled": true });
  }

  let mut props = Map::new();
  props.insert("distinct_id".to_string(), Value::String(instance_id));
  if let Value::Object(base_props) = build_base_props(&config) {
    for (key, value) in base_props {
      props.insert(key, value);
    }
  }

  let extra = sanitize_properties(properties);
  for (key, value) in extra {
    props.insert(key, value);
  }

  let url = format!("{}/capture/", host.trim_end_matches('/'));
  let payload = json!({
    "api_key": api_key,
    "event": event,
    "properties": Value::Object(props)
  });

  let _ = std::thread::spawn(move || {
    let _ = ureq::post(&url)
      .set("Content-Type", "application/json")
      .send_json(payload);
  });

  let _ = session_start_ms();
  json!({ "success": true })
}
