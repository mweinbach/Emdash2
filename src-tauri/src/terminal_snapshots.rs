use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::fs;
use std::path::{Path, PathBuf};
use tauri::Manager;

const MAX_SNAPSHOT_BYTES: usize = 8 * 1024 * 1024;
const MAX_TOTAL_BYTES: usize = 64 * 1024 * 1024;
pub const TERMINAL_SNAPSHOT_VERSION: u32 = 1;

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TerminalSnapshotPayload {
  pub version: u32,
  pub created_at: String,
  pub cols: u32,
  pub rows: u32,
  pub data: String,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub stats: Option<Value>,
}

#[derive(Clone)]
struct StoredSnapshot {
  payload: TerminalSnapshotPayload,
  bytes: usize,
}

fn base_dir(app: &tauri::AppHandle) -> PathBuf {
  if let Ok(override_dir) = std::env::var("EMDASH_TERMINAL_SNAPSHOT_DIR") {
    let trimmed = override_dir.trim();
    if !trimmed.is_empty() {
      return PathBuf::from(trimmed);
    }
  }
  app
    .path()
    .app_data_dir()
    .ok()
    .or_else(|| app.path().app_config_dir().ok())
    .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")))
    .join("terminal-snapshots")
}

fn sanitize_id(id: &str) -> String {
  id.chars()
    .map(|ch| {
      if ch.is_ascii_alphanumeric() || ch == '.' || ch == '_' || ch == '-' {
        ch
      } else {
        '_'
      }
    })
    .collect()
}

fn snapshot_path(app: &tauri::AppHandle, id: &str) -> PathBuf {
  base_dir(app).join(format!("{}.json", sanitize_id(id)))
}

fn ensure_dir(path: &Path) -> Result<(), String> {
  if let Some(parent) = path.parent() {
    if !parent.exists() {
      fs::create_dir_all(parent).map_err(|err| err.to_string())?;
    }
  }
  Ok(())
}

fn read_snapshot_file(path: &Path) -> Option<StoredSnapshot> {
  let raw = fs::read_to_string(path).ok()?;
  let payload: TerminalSnapshotPayload = serde_json::from_str(&raw).ok()?;
  if payload.version != TERMINAL_SNAPSHOT_VERSION {
    return None;
  }
  Some(StoredSnapshot {
    payload,
    bytes: raw.len(),
  })
}

fn created_at_ts(payload: &TerminalSnapshotPayload) -> i64 {
  DateTime::parse_from_rfc3339(&payload.created_at)
    .map(|dt| dt.with_timezone(&Utc).timestamp())
    .unwrap_or(0)
}

fn list_snapshots(
  app: &tauri::AppHandle,
) -> Result<Vec<(String, PathBuf, StoredSnapshot)>, String> {
  let dir = base_dir(app);
  let entries = match fs::read_dir(&dir) {
    Ok(entries) => entries,
    Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(vec![]),
    Err(err) => return Err(err.to_string()),
  };

  let mut items = Vec::new();
  for entry in entries {
    let entry = match entry {
      Ok(value) => value,
      Err(_) => continue,
    };
    let path = entry.path();
    if path.extension().and_then(|ext| ext.to_str()) != Some("json") {
      continue;
    }
    if let Some(stored) = read_snapshot_file(&path) {
      let id = path
        .file_stem()
        .and_then(|stem| stem.to_str())
        .unwrap_or_default()
        .to_string();
      items.push((id, path, stored));
    }
  }
  Ok(items)
}

pub fn get_snapshot(
  app: &tauri::AppHandle,
  id: &str,
) -> Result<Option<TerminalSnapshotPayload>, String> {
  let path = snapshot_path(app, id);
  Ok(read_snapshot_file(&path).map(|stored| stored.payload))
}

pub fn save_snapshot(
  app: &tauri::AppHandle,
  id: &str,
  payload: TerminalSnapshotPayload,
) -> Result<(), String> {
  if payload.version != TERMINAL_SNAPSHOT_VERSION {
    return Err("Unsupported snapshot version".to_string());
  }

  let json = serde_json::to_string(&payload).map_err(|err| err.to_string())?;
  let bytes = json.len();
  if bytes > MAX_SNAPSHOT_BYTES {
    return Err("Snapshot size exceeds per-task limit".to_string());
  }

  let path = snapshot_path(app, id);
  ensure_dir(&path)?;
  fs::write(&path, json).map_err(|err| err.to_string())?;
  prune_if_needed(app, id)?;
  Ok(())
}

pub fn delete_snapshot(app: &tauri::AppHandle, id: &str) -> Result<(), String> {
  let path = snapshot_path(app, id);
  match fs::remove_file(&path) {
    Ok(_) => Ok(()),
    Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(()),
    Err(err) => Err(err.to_string()),
  }
}

fn prune_if_needed(app: &tauri::AppHandle, recent_id: &str) -> Result<(), String> {
  let mut records = list_snapshots(app)?;
  if records.is_empty() {
    return Ok(());
  }

  let mut total: usize = records.iter().map(|(_, _, stored)| stored.bytes).sum();
  if total <= MAX_TOTAL_BYTES {
    return Ok(());
  }

  records.retain(|(id, _, _)| id != recent_id);
  records.sort_by_key(|(_, _, stored)| created_at_ts(&stored.payload));

  for (_id, path, stored) in &records {
    if total <= MAX_TOTAL_BYTES {
      break;
    }
    if fs::remove_file(path).is_ok() {
      total = total.saturating_sub(stored.bytes);
    }
  }

  if total > MAX_TOTAL_BYTES {
    for (id, path, _stored) in list_snapshots(app)? {
      if id == recent_id {
        continue;
      }
      let _ = fs::remove_file(path);
    }
  }

  Ok(())
}
