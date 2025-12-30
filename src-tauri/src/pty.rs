use crate::terminal_snapshots::{self, TerminalSnapshotPayload};
use portable_pty::{native_pty_system, ChildKiller, CommandBuilder, MasterPty, PtySize};
use serde::Deserialize;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Emitter, State, Window};

#[derive(Clone)]
struct PtyHandle {
  writer: Arc<Mutex<Box<dyn Write + Send>>>,
  master: Arc<Mutex<Box<dyn MasterPty + Send>>>,
  killer: Arc<Mutex<Box<dyn ChildKiller + Send + Sync>>>,
}

#[derive(Default, Clone)]
pub struct PtyState {
  inner: Arc<Mutex<HashMap<String, PtyHandle>>>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
#[allow(dead_code)]
pub struct PtyStartArgs {
  id: String,
  cwd: Option<String>,
  shell: Option<String>,
  command: Option<String>,
  env: Option<HashMap<String, String>>,
  cols: Option<u16>,
  rows: Option<u16>,
  auto_approve: Option<bool>,
  initial_prompt: Option<String>,
  skip_resume: Option<bool>,
}

fn default_shell() -> String {
  if cfg!(target_os = "windows") {
    std::env::var("COMSPEC").unwrap_or_else(|_| "C:\\Windows\\System32\\cmd.exe".to_string())
  } else {
    std::env::var("SHELL").unwrap_or_else(|_| "/bin/bash".to_string())
  }
}

fn resolve_cwd(cwd: &Option<String>) -> PathBuf {
  if let Some(value) = cwd {
    if !value.trim().is_empty() {
      return PathBuf::from(value);
    }
  }
  std::env::current_dir()
    .ok()
    .or_else(|| std::env::var("HOME").ok().map(PathBuf::from))
    .unwrap_or_else(|| PathBuf::from("."))
}

fn build_env(
  default_shell: &str,
  overrides: Option<HashMap<String, String>>,
) -> HashMap<String, String> {
  let mut env = HashMap::new();
  env.insert("TERM".to_string(), "xterm-256color".to_string());
  env.insert("COLORTERM".to_string(), "truecolor".to_string());
  env.insert("TERM_PROGRAM".to_string(), "emdash".to_string());

  if let Ok(home) = std::env::var("HOME") {
    env.insert("HOME".to_string(), home);
  }
  if let Ok(user) = std::env::var("USER") {
    env.insert("USER".to_string(), user);
  }
  env.insert("SHELL".to_string(), default_shell.to_string());

  if let Ok(lang) = std::env::var("LANG") {
    env.insert("LANG".to_string(), lang);
  }
  if let Ok(display) = std::env::var("DISPLAY") {
    env.insert("DISPLAY".to_string(), display);
  }
  if let Ok(sock) = std::env::var("SSH_AUTH_SOCK") {
    env.insert("SSH_AUTH_SOCK".to_string(), sock);
  }

  if let Some(extra) = overrides {
    env.extend(extra);
  }
  env
}

fn shell_basename(shell: &str) -> String {
  Path::new(shell)
    .file_name()
    .and_then(|name| name.to_str())
    .unwrap_or(shell)
    .to_lowercase()
}

fn build_shell_args(shell: &str, command: Option<&str>) -> Vec<String> {
  let base = shell_basename(shell);

  if cfg!(target_os = "windows") {
    if let Some(cmd) = command {
      if base.contains("powershell") || base.contains("pwsh") {
        return vec!["-NoExit".to_string(), "-Command".to_string(), cmd.to_string()];
      }
      return vec!["/K".to_string(), cmd.to_string()];
    }
    return Vec::new();
  }

  if let Some(cmd) = command {
    match base.as_str() {
      "zsh" | "bash" => vec!["-lic".to_string(), cmd.to_string()],
      "fish" => vec!["-ic".to_string(), cmd.to_string()],
      "sh" => vec!["-lc".to_string(), cmd.to_string()],
      _ => vec!["-c".to_string(), cmd.to_string()],
    }
  } else {
    match base.as_str() {
      "zsh" | "bash" | "fish" | "sh" => vec!["-il".to_string()],
      _ => vec!["-i".to_string()],
    }
  }
}

fn build_command_chain(command: Option<&str>, shell_path: &str) -> Option<String> {
  let cmd = command?;
  if cfg!(target_os = "windows") {
    return Some(cmd.to_string());
  }
  let escaped_shell = shell_path.replace('\'', "'\\''");
  Some(format!("{cmd}; exec '{escaped_shell}' -il"))
}

#[tauri::command]
pub fn pty_start(
  window: Window,
  app: AppHandle,
  state: State<PtyState>,
  args: PtyStartArgs,
) -> Result<Value, String> {
  if std::env::var("EMDASH_DISABLE_PTY").map(|v| v == "1").unwrap_or(false) {
    return Ok(json!({ "ok": false, "error": "PTY disabled via EMDASH_DISABLE_PTY=1" }));
  }

  let id = args.id.clone();

  {
    let guard = state.inner.lock().unwrap();
    if guard.contains_key(&id) {
      let _ = app.emit_to(window.label(), "pty:started", json!({ "id": id }));
      return Ok(json!({ "ok": true }));
    }
  }

  let cols = args.cols.unwrap_or(80);
  let rows = args.rows.unwrap_or(24);
  let cwd = resolve_cwd(&args.cwd);
  let shell_path = args
    .shell
    .clone()
    .filter(|value| !value.trim().is_empty())
    .unwrap_or_else(default_shell);
  let default_shell = default_shell();
  let command_chain = if args.command.as_deref().is_some() {
    build_command_chain(args.command.as_deref(), &default_shell)
  } else {
    None
  };
  let launch_shell = if command_chain.is_some() {
    default_shell.clone()
  } else {
    shell_path.clone()
  };
  let shell_args = build_shell_args(&launch_shell, command_chain.as_deref());
  let env = build_env(&default_shell, args.env);

  let pty_system = native_pty_system();
  let pair = pty_system
    .openpty(PtySize {
      rows,
      cols,
      pixel_width: 0,
      pixel_height: 0,
    })
    .map_err(|err| err.to_string())?;

  let mut cmd = CommandBuilder::new(launch_shell.clone());
  cmd.cwd(cwd);
  if !shell_args.is_empty() {
    cmd.args(shell_args);
  }
  for (key, value) in env {
    cmd.env(key, value);
  }

  let mut child = pair
    .slave
    .spawn_command(cmd)
    .map_err(|err| err.to_string())?;
  drop(pair.slave);

  let reader = pair
    .master
    .try_clone_reader()
    .map_err(|err| err.to_string())?;
  let writer = pair
    .master
    .take_writer()
    .map_err(|err| err.to_string())?;

  let handle = PtyHandle {
    writer: Arc::new(Mutex::new(writer)),
    master: Arc::new(Mutex::new(pair.master)),
    killer: Arc::new(Mutex::new(child.clone_killer())),
  };

  state.inner.lock().unwrap().insert(id.clone(), handle);

  let label = window.label().to_string();
  let data_event = format!("pty:data:{}", &id);
  let app_handle = app.clone();
  std::thread::spawn(move || {
    let mut reader = reader;
    let mut buf = [0u8; 8192];
    loop {
      match reader.read(&mut buf) {
        Ok(0) => break,
        Ok(n) => {
          let chunk = String::from_utf8_lossy(&buf[..n]).to_string();
          let _ = app_handle.emit_to(&label, &data_event, chunk);
        }
        Err(_) => break,
      }
    }
  });

  let exit_event = format!("pty:exit:{}", &id);
  let exit_label = window.label().to_string();
  let exit_state = state.inner.clone();
  let exit_app = app.clone();
  let exit_id = id.clone();
  std::thread::spawn(move || {
    let status = child.wait().ok();
    {
      let mut guard = exit_state.lock().unwrap();
      guard.remove(&exit_id);
    }
    let exit_code = status.as_ref().map(|s| s.exit_code() as i64);
    let signal = status
      .as_ref()
      .and_then(|s| s.signal())
      .map(|s| s.to_string());
    let _ = exit_app.emit_to(
      &exit_label,
      &exit_event,
      json!({ "exitCode": exit_code, "signal": signal }),
    );
  });

  let _ = app.emit_to(window.label(), "pty:started", json!({ "id": id }));
  Ok(json!({ "ok": true }))
}

#[tauri::command]
pub fn pty_input(state: State<PtyState>, id: String, data: String) -> Result<(), String> {
  let handle = state.inner.lock().unwrap().get(&id).cloned();
  if let Some(handle) = handle {
    let mut writer = handle.writer.lock().unwrap();
    writer.write_all(data.as_bytes()).map_err(|err| err.to_string())?;
    let _ = writer.flush();
  }
  Ok(())
}

#[tauri::command]
pub fn pty_resize(state: State<PtyState>, id: String, cols: u16, rows: u16) -> Result<(), String> {
  let handle = state.inner.lock().unwrap().get(&id).cloned();
  if let Some(handle) = handle {
    let master = handle.master.lock().unwrap();
    master
      .resize(PtySize {
        rows,
        cols,
        pixel_width: 0,
        pixel_height: 0,
      })
      .map_err(|err| err.to_string())?;
  }
  Ok(())
}

#[tauri::command]
pub fn pty_kill(state: State<PtyState>, id: String) -> Result<(), String> {
  let handle = state.inner.lock().unwrap().get(&id).cloned();
  if let Some(handle) = handle {
    let mut killer = handle.killer.lock().unwrap();
    let _ = killer.kill();
  }
  Ok(())
}

#[tauri::command]
pub fn pty_snapshot_get(app: AppHandle, id: String) -> Result<Value, String> {
  match terminal_snapshots::get_snapshot(&app, &id) {
    Ok(snapshot) => Ok(json!({ "ok": true, "snapshot": snapshot })),
    Err(err) => Ok(json!({ "ok": false, "error": err })),
  }
}

#[tauri::command]
pub fn pty_snapshot_save(
  app: AppHandle,
  id: String,
  payload: TerminalSnapshotPayload,
) -> Result<Value, String> {
  match terminal_snapshots::save_snapshot(&app, &id, payload) {
    Ok(_) => Ok(json!({ "ok": true })),
    Err(err) => Ok(json!({ "ok": false, "error": err })),
  }
}

#[tauri::command]
pub fn pty_snapshot_clear(app: AppHandle, id: String) -> Result<Value, String> {
  match terminal_snapshots::delete_snapshot(&app, &id) {
    Ok(_) => Ok(json!({ "ok": true })),
    Err(err) => Ok(json!({ "ok": false, "error": err })),
  }
}

#[tauri::command]
pub fn terminal_get_theme() -> Result<Value, String> {
  if !(cfg!(target_os = "macos") || cfg!(target_os = "linux")) {
    return Ok(json!({ "ok": false, "error": "No terminal configuration found" }));
  }

  let home = std::env::var("HOME").unwrap_or_default();
  if home.trim().is_empty() {
    return Ok(json!({ "ok": false, "error": "No terminal configuration found" }));
  }

  let config_path = Path::new(&home).join(".config").join("ghostty").join("config");
  if !config_path.exists() {
    return Ok(json!({ "ok": false, "error": "No terminal configuration found" }));
  }

  let content = std::fs::read_to_string(config_path).map_err(|err| err.to_string())?;
  let mut theme = serde_json::Map::new();

  for line in content.lines() {
    let trimmed = line.trim();
    if trimmed.is_empty() || trimmed.starts_with('#') || !trimmed.contains('=') {
      continue;
    }
    let mut parts = trimmed.splitn(2, '=');
    let key = parts.next().unwrap_or("").trim();
    let value = parts
      .next()
      .unwrap_or("")
      .trim()
      .trim_matches('"')
      .trim_matches('\'')
      .to_string();

    match key {
      "background" => {
        theme.insert("background".to_string(), Value::String(value));
      }
      "foreground" => {
        theme.insert("foreground".to_string(), Value::String(value));
      }
      "cursor" => {
        theme.insert("cursor".to_string(), Value::String(value));
      }
      "color0" => {
        theme.insert("black".to_string(), Value::String(value));
      }
      "color1" => {
        theme.insert("red".to_string(), Value::String(value));
      }
      "color2" => {
        theme.insert("green".to_string(), Value::String(value));
      }
      "color3" => {
        theme.insert("yellow".to_string(), Value::String(value));
      }
      "color4" => {
        theme.insert("blue".to_string(), Value::String(value));
      }
      "color5" => {
        theme.insert("magenta".to_string(), Value::String(value));
      }
      "color6" => {
        theme.insert("cyan".to_string(), Value::String(value));
      }
      "color7" => {
        theme.insert("white".to_string(), Value::String(value));
      }
      "color8" => {
        theme.insert("brightBlack".to_string(), Value::String(value));
      }
      "color9" => {
        theme.insert("brightRed".to_string(), Value::String(value));
      }
      "color10" => {
        theme.insert("brightGreen".to_string(), Value::String(value));
      }
      "color11" => {
        theme.insert("brightYellow".to_string(), Value::String(value));
      }
      "color12" => {
        theme.insert("brightBlue".to_string(), Value::String(value));
      }
      "color13" => {
        theme.insert("brightMagenta".to_string(), Value::String(value));
      }
      "color14" => {
        theme.insert("brightCyan".to_string(), Value::String(value));
      }
      "color15" => {
        theme.insert("brightWhite".to_string(), Value::String(value));
      }
      "font" => {
        theme.insert("fontFamily".to_string(), Value::String(value));
      }
      "font-size" => {
        if let Ok(size) = value.parse::<i64>() {
          theme.insert("fontSize".to_string(), Value::Number(size.into()));
        }
      }
      _ => {}
    }
  }

  Ok(json!({
    "ok": true,
    "config": {
      "terminal": "Ghostty",
      "theme": Value::Object(theme)
    }
  }))
}
