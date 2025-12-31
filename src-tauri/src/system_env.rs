use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::process::Command;

pub fn bootstrap() {
  if cfg!(target_os = "windows") {
    return;
  }

  let existing = std::env::var("PATH").unwrap_or_default();
  let shell_path = read_shell_path().unwrap_or_default();

  let mut seen = HashSet::new();
  let mut paths = Vec::new();
  extend_paths(&mut paths, &mut seen, &existing);
  extend_paths(&mut paths, &mut seen, &shell_path);
  add_common_paths(&mut paths, &mut seen);

  if let Ok(joined) = std::env::join_paths(paths) {
    let joined = joined.to_string_lossy().to_string();
    if !joined.is_empty() && joined != existing {
      std::env::set_var("PATH", joined);
    }
  }
}

fn read_shell_path() -> Option<String> {
  let shell = std::env::var("SHELL")
    .ok()
    .filter(|value| Path::new(value).is_absolute())
    .unwrap_or_else(|| "/bin/bash".to_string());
  let marker = "__EMDASH_PATH__";
  let cmd = format!("printf '{marker}%s{marker}' \"$PATH\"");
  let output = Command::new(shell)
    .args(["-lc", &cmd])
    .output()
    .ok()?;
  if !output.status.success() {
    return None;
  }
  let stdout = String::from_utf8_lossy(&output.stdout);
  let start = stdout.find(marker)?;
  let rest = &stdout[start + marker.len()..];
  let end = rest.find(marker)?;
  let path = rest[..end].trim();
  if path.is_empty() {
    None
  } else {
    Some(path.to_string())
  }
}

fn extend_paths(paths: &mut Vec<PathBuf>, seen: &mut HashSet<String>, raw: &str) {
  if raw.trim().is_empty() {
    return;
  }
  for path in std::env::split_paths(raw) {
    let key = path.to_string_lossy().to_string();
    if key.is_empty() {
      continue;
    }
    if seen.insert(key) {
      paths.push(path);
    }
  }
}

fn add_common_paths(paths: &mut Vec<PathBuf>, seen: &mut HashSet<String>) {
  let mut candidates = vec![
    "/opt/homebrew/bin",
    "/opt/homebrew/sbin",
    "/usr/local/bin",
    "/usr/local/sbin",
    "/usr/bin",
    "/bin",
    "/usr/sbin",
    "/sbin",
    "/snap/bin",
  ]
  .into_iter()
  .map(PathBuf::from)
  .collect::<Vec<_>>();

  if let Ok(home) = std::env::var("HOME") {
    let home = PathBuf::from(home);
    candidates.push(home.join(".local/bin"));
    candidates.push(home.join(".cargo/bin"));
    candidates.push(home.join(".bun/bin"));
    candidates.push(home.join(".npm-global/bin"));
    candidates.push(home.join(".pnpm"));
    candidates.push(home.join(".asdf/shims"));
    candidates.push(home.join("Library/pnpm"));
  }

  if let Ok(pnpm_home) = std::env::var("PNPM_HOME") {
    candidates.push(PathBuf::from(pnpm_home));
  }
  if let Ok(npm_prefix) = std::env::var("NPM_CONFIG_PREFIX") {
    candidates.push(PathBuf::from(npm_prefix).join("bin"));
  }

  for candidate in candidates {
    if !candidate.exists() {
      continue;
    }
    let key = candidate.to_string_lossy().to_string();
    if key.is_empty() {
      continue;
    }
    if seen.insert(key) {
      paths.push(candidate);
    }
  }
}
