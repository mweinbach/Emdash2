use semver::Version;
use serde_json::{json, Value};
use std::fs::File;
use std::io::{Read, Write};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tauri::{AppHandle, Emitter, Manager};

#[cfg(target_os = "linux")]
use std::os::unix::fs::PermissionsExt;

const RELEASES_API: &str = "https://api.github.com/repos/generalaction/emdash/releases/latest";
const RELEASES_PAGE: &str = "https://github.com/generalaction/emdash/releases/latest";

#[derive(Clone, Default)]
pub struct UpdateState {
  latest: Arc<Mutex<Option<ReleaseInfo>>>,
  downloaded_path: Arc<Mutex<Option<PathBuf>>>,
}

#[derive(Clone)]
struct ReleaseInfo {
  version: String,
  notes: Option<String>,
  published_at: Option<String>,
  download_url: Option<String>,
}

impl UpdateState {
  pub fn new() -> Self {
    Self::default()
  }
}

fn emit_update(app: &AppHandle, event_type: &str, payload: Option<Value>) {
  let mut data = json!({ "type": event_type });
  if let Some(payload) = payload {
    if let Some(obj) = data.as_object_mut() {
      obj.insert("payload".to_string(), payload);
    }
  }
  let _ = app.emit("update:event", data.clone());
  let channel = match event_type {
    "checking" => "update:checking",
    "available" => "update:available",
    "not-available" => "update:not-available",
    "download-progress" => "update:download-progress",
    "downloaded" => "update:downloaded",
    "error" => "update:error",
    _ => "update:event",
  };
  let _ = app.emit(channel, data.get("payload").cloned().unwrap_or(Value::Null));
}

fn is_dev_mode() -> bool {
  tauri::is_dev()
}

fn parse_version(raw: &str) -> Option<Version> {
  let trimmed = raw.trim().trim_start_matches('v');
  Version::parse(trimmed).ok()
}

fn current_version(app: &AppHandle) -> String {
  app.package_info().version.to_string()
}

fn choose_asset_name() -> String {
  let arch = if cfg!(target_arch = "aarch64") { "arm64" } else { "x64" };
  if cfg!(target_os = "macos") {
    format!("emdash-{}.dmg", arch)
  } else if cfg!(target_os = "windows") {
    "emdash-x64.exe".to_string()
  } else {
    "emdash-x86_64.AppImage".to_string()
  }
}

fn fallback_download_url() -> String {
  if !(cfg!(target_os = "macos") || cfg!(target_os = "windows") || cfg!(target_os = "linux")) {
    return RELEASES_PAGE.to_string();
  }
  let name = choose_asset_name();
  format!("https://github.com/generalaction/emdash/releases/latest/download/{}", name)
}

fn fetch_latest_release() -> Result<ReleaseInfo, String> {
  let response = ureq::get(RELEASES_API)
    .set("User-Agent", "emdash-tauri")
    .call()
    .map_err(|err| err.to_string())?;
  let body = response
    .into_string()
    .map_err(|err| err.to_string())?;
  let data: Value = serde_json::from_str(&body).map_err(|err| err.to_string())?;

  let version = data
    .get("tag_name")
    .and_then(|v| v.as_str())
    .unwrap_or("")
    .trim()
    .to_string();
  if version.is_empty() {
    return Err("No release tag found".to_string());
  }
  let notes = data.get("body").and_then(|v| v.as_str()).map(|s| s.to_string());
  let published_at = data
    .get("published_at")
    .and_then(|v| v.as_str())
    .map(|s| s.to_string());

  let asset_name = choose_asset_name();
  let mut download_url = None;
  if let Some(assets) = data.get("assets").and_then(|v| v.as_array()) {
    for asset in assets {
      let name = asset.get("name").and_then(|v| v.as_str()).unwrap_or("");
      if name == asset_name {
        download_url = asset
          .get("browser_download_url")
          .and_then(|v| v.as_str())
          .map(|s| s.to_string());
        break;
      }
    }
  }

  Ok(ReleaseInfo {
    version,
    notes,
    published_at,
    download_url,
  })
}

fn version_is_newer(latest: &str, current: &str) -> bool {
  match (parse_version(latest), parse_version(current)) {
    (Some(a), Some(b)) => a > b,
    _ => latest.trim() != current.trim(),
  }
}

#[tauri::command]
pub fn update_check(app: AppHandle, state: tauri::State<UpdateState>) -> Value {
  if is_dev_mode() && std::env::var("EMDASH_DEV_UPDATES") != Ok("true".to_string()) {
    return json!({ "success": false, "error": "Updates are disabled in development.", "devDisabled": true });
  }

  emit_update(&app, "checking", None);

  let latest = match fetch_latest_release() {
    Ok(info) => info,
    Err(err) => {
      emit_update(&app, "error", Some(json!({ "message": err.clone() })));
      return json!({ "success": false, "error": err });
    }
  };

  let current = current_version(&app);
  let available = version_is_newer(&latest.version, &current);
  let payload = json!({
    "version": latest.version,
    "notes": latest.notes,
    "publishedAt": latest.published_at,
  });

  if available {
    emit_update(&app, "available", Some(payload.clone()));
  } else {
    emit_update(&app, "not-available", Some(payload.clone()));
  }

  let mut guard = state.latest.lock().unwrap();
  *guard = Some(latest);

  json!({ "success": true, "result": payload })
}

#[tauri::command]
pub fn update_download(app: AppHandle, state: tauri::State<UpdateState>) -> Value {
  if is_dev_mode() && std::env::var("EMDASH_DEV_UPDATES") != Ok("true".to_string()) {
    return json!({ "success": false, "error": "Cannot download updates in development.", "devDisabled": true });
  }

  let latest = state.latest.lock().unwrap().clone();
  let release = match latest {
    Some(info) => info,
    None => {
      return json!({ "success": false, "error": "No update available" });
    }
  };

  let url = release.download_url.unwrap_or_else(fallback_download_url);
  let resp = match ureq::get(&url).set("User-Agent", "emdash-tauri").call() {
    Ok(resp) => resp,
    Err(err) => {
      emit_update(&app, "error", Some(json!({ "message": err.to_string() })));
      return json!({ "success": false, "error": err.to_string() });
    }
  };

  let total = resp.header("Content-Length").and_then(|v| v.parse::<u64>().ok());
  let mut reader = resp.into_reader();

  let cache_dir = app
    .path()
    .app_cache_dir()
    .ok()
    .unwrap_or_else(std::env::temp_dir);
  let _ = std::fs::create_dir_all(&cache_dir);
  let filename = choose_asset_name();
  let dest = cache_dir.join(filename);
  let mut file = match File::create(&dest) {
    Ok(f) => f,
    Err(err) => return json!({ "success": false, "error": err.to_string() }),
  };

  let mut buf = [0u8; 8192];
  let mut transferred: u64 = 0;
  loop {
    let read = match reader.read(&mut buf) {
      Ok(0) => break,
      Ok(n) => n,
      Err(err) => {
        emit_update(&app, "error", Some(json!({ "message": err.to_string() })));
        return json!({ "success": false, "error": err.to_string() });
      }
    };
    if file.write_all(&buf[..read]).is_err() {
      return json!({ "success": false, "error": "Failed to write update file" });
    }
    transferred += read as u64;
    let percent = total.map(|t| (transferred as f64 / t as f64 * 100.0).min(100.0));
    emit_update(
      &app,
      "download-progress",
      Some(json!({
        "percent": percent.unwrap_or(0.0),
        "transferred": transferred,
        "total": total.unwrap_or(0),
      })),
    );
  }

  #[cfg(target_os = "linux")]
  {
    let _ = std::fs::set_permissions(&dest, std::fs::Permissions::from_mode(0o755));
  }

  *state.downloaded_path.lock().unwrap() = Some(dest.clone());
  emit_update(&app, "downloaded", Some(json!({ "path": dest.to_string_lossy() })));

  json!({ "success": true })
}

#[tauri::command]
pub fn update_quit_and_install(app: AppHandle, state: tauri::State<UpdateState>) -> Value {
  let path = state.downloaded_path.lock().unwrap().clone();
  let Some(path) = path else {
    return json!({ "success": false, "error": "No update downloaded" });
  };

  let _ = open::that(&path);
  let handle = app.clone();
  std::thread::spawn(move || {
    std::thread::sleep(Duration::from_millis(300));
    handle.exit(0);
  });

  json!({ "success": true })
}

#[tauri::command]
pub fn update_open_latest(app: AppHandle) -> Value {
  let _ = open::that(fallback_download_url());
  let handle = app.clone();
  std::thread::spawn(move || {
    std::thread::sleep(Duration::from_millis(300));
    handle.exit(0);
  });
  json!({ "success": true })
}
