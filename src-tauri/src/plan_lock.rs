use serde::{Deserialize, Serialize};
use serde_json::json;
use crate::runtime::run_blocking;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Serialize, Deserialize, Clone)]
struct Entry {
  p: String,
  m: u32,
}

fn is_symlink(path: &Path) -> bool {
  fs::symlink_metadata(path)
    .map(|meta| meta.file_type().is_symlink())
    .unwrap_or(false)
}

fn collect_paths(root: &Path) -> Vec<PathBuf> {
  let mut result = Vec::new();
  let mut stack = vec![PathBuf::from(".")];

  while let Some(rel) = stack.pop() {
    let abs = root.join(&rel);
    if is_symlink(&abs) {
      continue;
    }
    let meta = match fs::metadata(&abs) {
      Ok(m) => m,
      Err(_) => continue,
    };
    if meta.is_dir() {
      if rel == PathBuf::from(".emdash") || rel.starts_with(".emdash") {
        continue;
      }
      result.push(rel.clone());
      let entries = match fs::read_dir(&abs) {
        Ok(e) => e,
        Err(_) => continue,
      };
      for entry in entries.flatten() {
        let next_rel = if rel == PathBuf::from(".") {
          PathBuf::from(entry.file_name())
        } else {
          rel.join(entry.file_name())
        };
        stack.push(next_rel);
      }
    } else if meta.is_file() {
      result.push(rel.clone());
    }
  }

  result
}

#[cfg(unix)]
fn chmod_no_write(mode: u32, is_dir: bool) -> u32 {
  let no_write = mode & !0o222;
  if is_dir {
    (no_write | 0o111) & 0o7777
  } else {
    no_write & 0o7777
  }
}

#[cfg(unix)]
fn apply_lock(root: &Path) -> Result<usize, String> {
  use std::os::unix::fs::PermissionsExt;

  let entries = collect_paths(root);
  let mut state: Vec<Entry> = Vec::new();
  let mut changed = 0usize;

  for rel in entries {
    let abs = root.join(&rel);
    let meta = match fs::metadata(&abs) {
      Ok(m) => m,
      Err(_) => continue,
    };
    let is_dir = meta.is_dir();
    let prev_mode = meta.permissions().mode() & 0o7777;
    let next_mode = chmod_no_write(prev_mode, is_dir);
    if next_mode != prev_mode {
      if fs::set_permissions(&abs, fs::Permissions::from_mode(next_mode)).is_ok() {
        state.push(Entry {
          p: rel.to_string_lossy().to_string(),
          m: prev_mode,
        });
        changed += 1;
      }
    }
  }

  let state_path = root.join(".emdash").join(".planlock.json");
  if let Some(parent) = state_path.parent() {
    let _ = fs::create_dir_all(parent);
  }
  let _ = fs::write(state_path, serde_json::to_string(&state).unwrap_or_else(|_| "[]".into()));

  Ok(changed)
}

#[cfg(windows)]
fn apply_lock(root: &Path) -> Result<usize, String> {
  let entries = collect_paths(root);
  let mut state: Vec<Entry> = Vec::new();
  let mut changed = 0usize;

  for rel in entries {
    let abs = root.join(&rel);
    let meta = match fs::metadata(&abs) {
      Ok(m) => m,
      Err(_) => continue,
    };
    if meta.is_dir() {
      continue;
    }
    let mut perms = meta.permissions();
    let prev_readonly = perms.readonly();
    if !prev_readonly {
      perms.set_readonly(true);
      if fs::set_permissions(&abs, perms).is_ok() {
        state.push(Entry {
          p: rel.to_string_lossy().to_string(),
          m: if prev_readonly { 0o444 } else { 0o666 },
        });
        changed += 1;
      }
    }
  }

  let state_path = root.join(".emdash").join(".planlock.json");
  if let Some(parent) = state_path.parent() {
    let _ = fs::create_dir_all(parent);
  }
  let _ = fs::write(state_path, serde_json::to_string(&state).unwrap_or_else(|_| "[]".into()));

  Ok(changed)
}

#[cfg(unix)]
fn release_lock(root: &Path) -> Result<usize, String> {
  use std::os::unix::fs::PermissionsExt;
  let state_path = root.join(".emdash").join(".planlock.json");
  if !state_path.exists() {
    return Ok(0);
  }
  let raw = fs::read_to_string(&state_path).unwrap_or_default();
  let entries: Vec<Entry> = serde_json::from_str(&raw).unwrap_or_default();
  let mut restored = 0usize;
  for ent in entries {
    let abs = root.join(&ent.p);
    if fs::set_permissions(&abs, fs::Permissions::from_mode(ent.m)).is_ok() {
      restored += 1;
    }
  }
  let _ = fs::remove_file(state_path);
  Ok(restored)
}

#[cfg(windows)]
fn release_lock(root: &Path) -> Result<usize, String> {
  let state_path = root.join(".emdash").join(".planlock.json");
  if !state_path.exists() {
    return Ok(0);
  }
  let raw = fs::read_to_string(&state_path).unwrap_or_default();
  let entries: Vec<Entry> = serde_json::from_str(&raw).unwrap_or_default();
  let mut restored = 0usize;
  for ent in entries {
    let abs = root.join(&ent.p);
    let meta = match fs::metadata(&abs) {
      Ok(m) => m,
      Err(_) => continue,
    };
    if meta.is_dir() {
      continue;
    }
    let mut perms = meta.permissions();
    let readonly = (ent.m & 0o222) == 0;
    perms.set_readonly(readonly);
    if fs::set_permissions(&abs, perms).is_ok() {
      restored += 1;
    }
  }
  let _ = fs::remove_file(state_path);
  Ok(restored)
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlanLockArgs {
  task_path: String,
}

#[tauri::command]
pub async fn plan_lock(args: PlanLockArgs) -> serde_json::Value {
  run_blocking(
    json!({ "success": false, "changed": 0, "error": "Task cancelled" }),
    move || {
      let root = Path::new(args.task_path.trim());
      if args.task_path.trim().is_empty() {
        return json!({ "success": false, "changed": 0, "error": "taskPath is required" });
      }
      match apply_lock(root) {
        Ok(changed) => json!({ "success": true, "changed": changed }),
        Err(err) => json!({ "success": false, "changed": 0, "error": err }),
      }
    },
  )
  .await
}

#[tauri::command]
pub async fn plan_unlock(args: PlanLockArgs) -> serde_json::Value {
  run_blocking(
    json!({ "success": false, "restored": 0, "error": "Task cancelled" }),
    move || {
      if args.task_path.trim().is_empty() {
        return json!({ "success": false, "restored": 0, "error": "taskPath is required" });
      }
      let root = Path::new(args.task_path.trim());
      match release_lock(root) {
        Ok(restored) => json!({ "success": true, "restored": restored }),
        Err(err) => json!({ "success": false, "restored": 0, "error": err }),
      }
    },
  )
  .await
}
