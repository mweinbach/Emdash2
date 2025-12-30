use serde_json::{json, Value};
use std::fs;
use std::path::{Path, PathBuf};
use tauri::Emitter;

const DEFAULT_IGNORES: &[&str] = &[
  ".git",
  "node_modules",
  "dist",
  "build",
  "out",
  ".next",
  ".nuxt",
  ".cache",
  "coverage",
  ".DS_Store",
];

const ALLOWED_IMAGE_EXTENSIONS: &[&str] = &[
  ".png", ".jpg", ".jpeg", ".gif", ".webp", ".bmp", ".svg",
];

const DEFAULT_ATTACHMENTS_SUBDIR: &str = "attachments";

fn normalize_rel_path(path: &str) -> Result<PathBuf, String> {
  if path.trim().is_empty() {
    return Err("Invalid relPath".to_string());
  }
  let rel = Path::new(path);
  if rel.is_absolute() {
    return Err("Path escapes root".to_string());
  }
  for component in rel.components() {
    if matches!(component, std::path::Component::ParentDir) {
      return Err("Path escapes root".to_string());
    }
  }
  Ok(rel.to_path_buf())
}

fn resolve_root(root: &str) -> Result<PathBuf, String> {
  if root.trim().is_empty() {
    return Err("Invalid root path".to_string());
  }
  let root_path = PathBuf::from(root);
  if !root_path.exists() {
    return Err("Invalid root path".to_string());
  }
  Ok(root_path)
}

fn list_files(root: &Path, include_dirs: bool, max_entries: usize) -> Vec<Value> {
  let mut items: Vec<Value> = Vec::new();
  let mut stack: Vec<PathBuf> = vec![PathBuf::from(".")];

  while let Some(rel) = stack.pop() {
    let abs = if rel.as_os_str() == "." {
      root.to_path_buf()
    } else {
      root.join(&rel)
    };

    let metadata = match fs::metadata(&abs) {
      Ok(meta) => meta,
      Err(_) => continue,
    };

    if metadata.is_dir() {
      if rel.as_os_str() != "." {
        if let Some(name) = abs.file_name().and_then(|s| s.to_str()) {
          if DEFAULT_IGNORES.contains(&name) {
            continue;
          }
        }
        if include_dirs {
          let rel_str = rel.to_string_lossy().replace('\\', "/");
          items.push(json!({ "path": rel_str, "type": "dir" }));
          if items.len() >= max_entries {
            break;
          }
        }
      }

      let entries = match fs::read_dir(&abs) {
        Ok(entries) => entries,
        Err(_) => continue,
      };

      let mut collected: Vec<PathBuf> = Vec::new();
      for entry in entries.flatten() {
        let name = entry.file_name();
        let name_str = name.to_string_lossy();
        if DEFAULT_IGNORES.contains(&name_str.as_ref()) {
          continue;
        }
        let next_rel = if rel.as_os_str() == "." {
          PathBuf::from(name_str.as_ref())
        } else {
          rel.join(name_str.as_ref())
        };
        collected.push(next_rel);
      }
      for next in collected.into_iter().rev() {
        stack.push(next);
      }
    } else if metadata.is_file() {
      let rel_str = rel.to_string_lossy().replace('\\', "/");
      items.push(json!({ "path": rel_str, "type": "file" }));
      if items.len() >= max_entries {
        break;
      }
    }
  }

  items
}

fn emit_plan_event(app: &tauri::AppHandle, payload: Value) {
  let _ = app.emit("plan:event", payload);
}

#[tauri::command]
pub fn fs_list(root: String, include_dirs: Option<bool>, max_entries: Option<usize>) -> Value {
  let include_dirs = include_dirs.unwrap_or(true);
  let max_entries = max_entries.unwrap_or(5000).clamp(100, 20000);
  let root_path = match resolve_root(&root) {
    Ok(path) => path,
    Err(err) => return json!({ "success": false, "error": err }),
  };
  let items = list_files(&root_path, include_dirs, max_entries);
  json!({ "success": true, "items": items })
}

#[tauri::command]
pub fn fs_read(root: String, rel_path: String, max_bytes: Option<usize>) -> Value {
  let root_path = match resolve_root(&root) {
    Ok(path) => path,
    Err(err) => return json!({ "success": false, "error": err }),
  };
  let rel = match normalize_rel_path(&rel_path) {
    Ok(rel) => rel,
    Err(err) => return json!({ "success": false, "error": err }),
  };
  let abs = root_path.join(&rel);
  let metadata = match fs::metadata(&abs) {
    Ok(meta) => meta,
    Err(_) => return json!({ "success": false, "error": "Not found" }),
  };
  if metadata.is_dir() {
    return json!({ "success": false, "error": "Is a directory" });
  }
  let max_bytes = max_bytes.unwrap_or(200 * 1024).clamp(1024, 5 * 1024 * 1024);
  let size = metadata.len() as usize;
  let bytes_to_read = std::cmp::min(size, max_bytes);
  let content = match fs::File::open(&abs) {
    Ok(mut file) => {
      use std::io::Read;
      let mut buf = vec![0_u8; bytes_to_read];
      let mut offset = 0;
      while offset < bytes_to_read {
        match file.read(&mut buf[offset..]) {
          Ok(0) => break,
          Ok(read) => offset += read,
          Err(_) => return json!({ "success": false, "error": "Failed to read file" }),
        }
      }
      buf.truncate(offset);
      String::from_utf8_lossy(&buf).to_string()
    }
    Err(_) => return json!({ "success": false, "error": "Failed to read file" }),
  };
  let truncated = size > bytes_to_read;
  json!({
    "success": true,
    "path": rel_path,
    "size": size,
    "truncated": truncated,
    "content": content
  })
}

#[tauri::command]
pub fn fs_write(
  app: tauri::AppHandle,
  root: String,
  rel_path: String,
  content: String,
  mkdirs: Option<bool>,
) -> Value {
  let root_path = match resolve_root(&root) {
    Ok(path) => path,
    Err(err) => return json!({ "success": false, "error": err }),
  };
  let rel = match normalize_rel_path(&rel_path) {
    Ok(rel) => rel,
    Err(err) => return json!({ "success": false, "error": err }),
  };
  let abs = root_path.join(&rel);
  if mkdirs.unwrap_or(true) {
    if let Some(parent) = abs.parent() {
      if let Err(err) = fs::create_dir_all(parent) {
        return json!({ "success": false, "error": err.to_string() });
      }
    }
  }

  match fs::write(&abs, content.as_bytes()) {
    Ok(_) => json!({ "success": true }),
    Err(err) => {
      if err.kind() == std::io::ErrorKind::PermissionDenied {
        emit_plan_event(
          &app,
          json!({
            "type": "write_blocked",
            "root": root,
            "relPath": rel_path,
            "code": "EACCES",
            "message": err.to_string()
          }),
        );
      }
      json!({ "success": false, "error": "Failed to write file" })
    }
  }
}

#[tauri::command]
pub fn fs_remove(app: tauri::AppHandle, root: String, rel_path: String) -> Value {
  let root_path = match resolve_root(&root) {
    Ok(path) => path,
    Err(err) => return json!({ "success": false, "error": err }),
  };
  let rel = match normalize_rel_path(&rel_path) {
    Ok(rel) => rel,
    Err(err) => return json!({ "success": false, "error": err }),
  };
  let abs = root_path.join(&rel);
  if !abs.exists() {
    return json!({ "success": true });
  }
  let metadata = match fs::metadata(&abs) {
    Ok(meta) => meta,
    Err(_) => return json!({ "success": false, "error": "Not found" }),
  };
  if metadata.is_dir() {
    return json!({ "success": false, "error": "Is a directory" });
  }

  let remove_result = fs::remove_file(&abs).or_else(|err| {
    if err.kind() == std::io::ErrorKind::PermissionDenied {
      #[cfg(unix)]
      {
        use std::os::unix::fs::PermissionsExt;
        if let Some(parent) = abs.parent() {
          if let Ok(meta) = fs::metadata(parent) {
            let mut perm = meta.permissions();
            perm.set_mode(perm.mode() | 0o222);
            let _ = fs::set_permissions(parent, perm);
          }
        }
        if let Ok(meta) = fs::metadata(&abs) {
          let mut perm = meta.permissions();
          perm.set_mode(perm.mode() | 0o222);
          let _ = fs::set_permissions(&abs, perm);
        }
      }
    }
    fs::remove_file(&abs)
  });

  match remove_result {
    Ok(_) => json!({ "success": true }),
    Err(err) => {
      if err.kind() == std::io::ErrorKind::PermissionDenied {
        emit_plan_event(
          &app,
          json!({
            "type": "remove_blocked",
            "root": root,
            "relPath": rel_path,
            "code": "EACCES",
            "message": err.to_string()
          }),
        );
      }
      json!({ "success": false, "error": "Failed to remove file" })
    }
  }
}

#[tauri::command]
pub fn fs_save_attachment(task_path: String, src_path: String, subdir: Option<String>) -> Value {
  if task_path.trim().is_empty() {
    return json!({ "success": false, "error": "Invalid taskPath" });
  }
  if src_path.trim().is_empty() {
    return json!({ "success": false, "error": "Invalid srcPath" });
  }

  let task_root = PathBuf::from(&task_path);
  if !task_root.exists() {
    return json!({ "success": false, "error": "Invalid taskPath" });
  }
  let src = PathBuf::from(&src_path);
  if !src.exists() {
    return json!({ "success": false, "error": "Invalid srcPath" });
  }

  let ext = src
    .extension()
    .and_then(|s| s.to_str())
    .map(|s| format!(".{}", s.to_lowercase()))
    .unwrap_or_else(|| "".to_string());
  if !ALLOWED_IMAGE_EXTENSIONS.contains(&ext.as_str()) {
    return json!({ "success": false, "error": "Unsupported attachment type" });
  }

  let subdir = subdir.unwrap_or_else(|| DEFAULT_ATTACHMENTS_SUBDIR.to_string());
  let base_dir = task_root.join(".emdash").join(subdir);
  if let Err(err) = fs::create_dir_all(&base_dir) {
    return json!({ "success": false, "error": err.to_string() });
  }

  let base_name = match src.file_name().and_then(|s| s.to_str()) {
    Some(name) => name.to_string(),
    None => return json!({ "success": false, "error": "Invalid srcPath" }),
  };
  let mut dest_name = base_name.clone();
  let mut dest_abs = base_dir.join(&dest_name);
  let mut counter = 1;
  while dest_abs.exists() {
    let stem = Path::new(&base_name)
      .file_stem()
      .and_then(|s| s.to_str())
      .unwrap_or("attachment");
    dest_name = format!("{}-{}{}", stem, counter, ext);
    dest_abs = base_dir.join(&dest_name);
    counter += 1;
  }

  if let Err(err) = fs::copy(&src, &dest_abs) {
    return json!({ "success": false, "error": err.to_string() });
  }

  let rel = dest_abs
    .strip_prefix(&task_root)
    .ok()
    .and_then(|p| p.to_str())
    .unwrap_or(dest_abs.to_string_lossy().as_ref())
    .to_string();

  json!({
    "success": true,
    "absPath": dest_abs.to_string_lossy(),
    "relPath": rel.replace('\\', "/"),
    "fileName": dest_name
  })
}
