use serde::Deserialize;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::fs;
use std::io::{BufRead, BufReader};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::{
  atomic::{AtomicBool, Ordering},
  Arc, Mutex,
};
use std::thread;
use std::time::Duration;
use tauri::{AppHandle, Emitter};

#[derive(Default)]
pub struct HostPreviewState {
  procs: Arc<Mutex<HashMap<String, Child>>>,
}

impl HostPreviewState {
  pub fn new() -> Self {
    Self {
      procs: Arc::new(Mutex::new(HashMap::new())),
    }
  }
}

#[derive(Deserialize)]
struct PackageJson {
  scripts: Option<HashMap<String, String>>,
  dependencies: Option<HashMap<String, String>>,
  #[serde(rename = "devDependencies")]
  dev_dependencies: Option<HashMap<String, String>>,
}

fn emit_event(app: &AppHandle, payload: Value) {
  let _ = app.emit("preview:host:event", payload);
}

fn detect_package_manager(dir: &Path) -> &'static str {
  if dir.join("bun.lockb").exists() || dir.join("bun.lock").exists() {
    return "bun";
  }
  if dir.join("pnpm-lock.yaml").exists() {
    return "pnpm";
  }
  if dir.join("yarn.lock").exists() {
    return "yarn";
  }
  "npm"
}

fn normalize_url(line: &str) -> Option<String> {
  let needles = [
    "http://localhost",
    "http://127.0.0.1",
    "http://0.0.0.0",
    "http://[::1]",
    "https://localhost",
    "https://127.0.0.1",
    "https://0.0.0.0",
    "https://[::1]",
  ];
  for needle in needles {
    if let Some(idx) = line.find(needle) {
      let rest = &line[idx..];
      let end = rest
        .find(|c: char| c.is_whitespace() || c == '"' || c == '\'')
        .unwrap_or(rest.len());
      let candidate = &rest[..end];
      let normalized = candidate.replace("0.0.0.0", "localhost");
      return Some(normalized);
    }
  }
  None
}

fn pick_available_port(preferred: &[u16]) -> u16 {
  for port in preferred {
    if TcpListener::bind(("127.0.0.1", *port)).is_ok() {
      return *port;
    }
  }
  TcpListener::bind(("127.0.0.1", 0))
    .ok()
    .and_then(|listener| listener.local_addr().ok().map(|addr| addr.port()))
    .unwrap_or(5173)
}

fn probe_port(host: &str, port: u16) -> bool {
  TcpStream::connect_timeout(&format!("{host}:{port}").parse().unwrap(), Duration::from_millis(200))
    .map(|stream| {
      let _ = stream.shutdown(std::net::Shutdown::Both);
    })
    .is_ok()
}

fn read_package_json(path: &Path) -> Option<PackageJson> {
  let raw = fs::read_to_string(path).ok()?;
  serde_json::from_str(&raw).ok()
}

fn select_script(pkg: Option<&PackageJson>) -> String {
  if let Some(pkg) = pkg {
    if let Some(scripts) = pkg.scripts.as_ref() {
      for key in ["dev", "start", "serve", "preview"] {
        if scripts.contains_key(key) {
          return key.to_string();
        }
      }
    }
  }
  "dev".to_string()
}

fn install_args(pm: &str, cwd: &Path) -> Vec<String> {
  let has_pkg_lock = cwd.join("package-lock.json").exists();
  let has_bun_lock = cwd.join("bun.lockb").exists() || cwd.join("bun.lock").exists();
  match pm {
    "npm" => {
      if has_pkg_lock {
        vec!["ci".to_string()]
      } else {
        vec!["install".to_string()]
      }
    }
    "bun" => {
      if has_bun_lock {
        vec!["install".to_string(), "--frozen-lockfile".to_string()]
      } else {
        vec!["install".to_string()]
      }
    }
    _ => vec!["install".to_string()],
  }
}

fn spawn_line_reader<R: std::io::Read + Send + 'static>(
  reader: R,
  on_line: Arc<dyn Fn(String) + Send + Sync>,
) {
  thread::spawn(move || {
    let buf = BufReader::new(reader);
    for line in buf.lines().flatten() {
      on_line(line);
    }
  });
}

fn run_command_streaming(
  app: &AppHandle,
  task_id: &str,
  command: &str,
  args: &[String],
  cwd: &Path,
) -> Result<(), String> {
  let mut child = Command::new(command)
    .args(args)
    .current_dir(cwd)
    .env("BROWSER", "none")
    .stdout(Stdio::piped())
    .stderr(Stdio::piped())
    .spawn()
    .map_err(|err| err.to_string())?;

  emit_event(
    app,
    json!({ "type": "setup", "taskId": task_id, "status": "starting" }),
  );

  let handler = {
    let app = app.clone();
    let task_id = task_id.to_string();
    Arc::new(move |line: String| {
      emit_event(
        &app,
        json!({
          "type": "setup",
          "taskId": task_id,
          "status": "line",
          "line": line
        }),
      );
    })
  };

  if let Some(stdout) = child.stdout.take() {
    spawn_line_reader(stdout, handler.clone());
  }
  if let Some(stderr) = child.stderr.take() {
    spawn_line_reader(stderr, handler);
  }

  let status = child.wait().map_err(|err| err.to_string())?;
  if status.success() {
    emit_event(
      app,
      json!({ "type": "setup", "taskId": task_id, "status": "done" }),
    );
    Ok(())
  } else {
    emit_event(
      app,
      json!({
        "type": "setup",
        "taskId": task_id,
        "status": "error",
        "line": format!("install exited with {status}")
      }),
    );
    Err("install failed".to_string())
  }
}

#[tauri::command]
pub fn host_preview_setup(app: AppHandle, task_id: String, task_path: String) -> Value {
  let cwd = PathBuf::from(&task_path);
  if !cwd.exists() {
    return json!({ "ok": false, "error": "task path not found" });
  }
  let pm = detect_package_manager(&cwd);
  let args = install_args(pm, &cwd);
  match run_command_streaming(&app, &task_id, pm, &args, &cwd) {
    Ok(()) => json!({ "ok": true }),
    Err(err) => json!({ "ok": false, "error": err }),
  }
}

#[tauri::command]
pub fn host_preview_start(
  app: AppHandle,
  state: tauri::State<HostPreviewState>,
  task_id: String,
  task_path: String,
  script: Option<String>,
) -> Value {
  let cwd = PathBuf::from(&task_path);
  if !cwd.exists() {
    return json!({ "ok": false, "error": "task path not found" });
  }

  // Stop existing process for this task.
  {
    let mut map = state.procs.lock().unwrap();
    if let Some(mut child) = map.remove(&task_id) {
      let _ = child.kill();
    }
  }

  let pkg_path = cwd.join("package.json");
  let pkg = read_package_json(&pkg_path);
  let script_name = script
    .as_ref()
    .and_then(|s| {
      let trimmed = s.trim();
      if trimmed.is_empty() {
        None
      } else {
        Some(trimmed.to_string())
      }
    })
    .unwrap_or_else(|| select_script(pkg.as_ref()));

  let pm = detect_package_manager(&cwd);
  let mut args: Vec<String> = if pm == "npm" || pm == "bun" {
    vec!["run".to_string(), script_name.clone()]
  } else {
    vec![script_name.clone()]
  };

  let mut envs: Vec<(String, String)> = Vec::new();
  let preferred = [5173u16, 5174, 3001, 3002, 8080, 4200, 5500, 7000];
  let port = pick_available_port(&preferred);
  envs.push(("PORT".to_string(), port.to_string()));
  envs.push(("VITE_PORT".to_string(), port.to_string()));
  envs.push(("BROWSER".to_string(), "none".to_string()));

  // Add framework port hints.
  if let Some(pkg) = pkg.as_ref() {
    let script_cmd = pkg
      .scripts
      .as_ref()
      .and_then(|s| s.get(&script_name))
      .map(|s| s.to_lowercase())
      .unwrap_or_default();
    let deps = pkg
      .dependencies
      .as_ref()
      .cloned()
      .unwrap_or_default()
      .into_iter()
      .chain(
        pkg.dev_dependencies
          .as_ref()
          .cloned()
          .unwrap_or_default()
          .into_iter(),
      )
      .collect::<HashMap<_, _>>();
    let looks_like_next = script_cmd.contains("next") || deps.contains_key("next");
    let looks_like_vite = script_cmd.contains("vite") || deps.contains_key("vite");
    let looks_like_webpack = script_cmd.contains("webpack-dev-server")
      || deps.contains_key("webpack-dev-server");
    let looks_like_angular = script_cmd.contains("angular")
      || script_cmd.split_whitespace().any(|s| s == "ng")
      || deps.contains_key("@angular/cli");
    let mut extra: Vec<String> = Vec::new();
    if looks_like_next {
      extra.push("-p".to_string());
      extra.push(port.to_string());
    } else if looks_like_vite || looks_like_webpack || looks_like_angular {
      extra.push("--port".to_string());
      extra.push(port.to_string());
    }
    if !extra.is_empty() {
      if pm == "npm" || pm == "bun" {
        args.push("--".to_string());
      }
      args.extend(extra);
    }
  }

  let mut cmd = Command::new(pm);
  cmd.args(&args)
    .current_dir(&cwd)
    .stdout(Stdio::piped())
    .stderr(Stdio::piped());
  for (key, value) in envs {
    cmd.env(key, value);
  }

  let mut child = match cmd.spawn() {
    Ok(child) => child,
    Err(err) => return json!({ "ok": false, "error": err.to_string() }),
  };

  let url_emitted = Arc::new(AtomicBool::new(false));
  let task_id_clone = task_id.clone();
  let app_clone = app.clone();
  let url_emitted_clone = url_emitted.clone();

  let on_line = Arc::new(move |line: String| {
    emit_event(
      &app_clone,
      json!({
        "type": "setup",
        "taskId": task_id_clone,
        "status": "line",
        "line": line
      }),
    );
    if !url_emitted_clone.load(Ordering::SeqCst) {
      if let Some(url) = normalize_url(&line) {
        if !url_emitted_clone.swap(true, Ordering::SeqCst) {
          emit_event(
            &app_clone,
            json!({ "type": "url", "taskId": task_id_clone, "url": url }),
          );
        }
      }
    }
  });

  if let Some(stdout) = child.stdout.take() {
    spawn_line_reader(stdout, on_line.clone());
  }
  if let Some(stderr) = child.stderr.take() {
    spawn_line_reader(stderr, on_line);
  }

  {
    let mut map = state.procs.lock().unwrap();
    map.insert(task_id.clone(), child);
  }

  // Probe for server readiness and emit URL if needed.
  let app_probe = app.clone();
  let task_probe = task_id.clone();
  let url_emitted_probe = url_emitted.clone();
  thread::spawn(move || {
    for _ in 0..40 {
      if url_emitted_probe.load(Ordering::SeqCst) {
        return;
      }
      if probe_port("127.0.0.1", port) {
        if !url_emitted_probe.swap(true, Ordering::SeqCst) {
          emit_event(
            &app_probe,
            json!({
              "type": "url",
              "taskId": task_probe,
              "url": format!("http://localhost:{port}")
            }),
          );
        }
        return;
      }
      thread::sleep(Duration::from_millis(800));
    }
  });

  // Monitor exit.
  let procs = state.procs.clone();
  let app_exit = app.clone();
  let task_exit = task_id.clone();
  thread::spawn(move || loop {
    let status = {
      let mut map = procs.lock().unwrap();
      if let Some(child) = map.get_mut(&task_exit) {
        child.try_wait().ok().flatten()
      } else {
        return;
      }
    };
    if status.is_some() {
      let mut map = procs.lock().unwrap();
      map.remove(&task_exit);
      emit_event(&app_exit, json!({ "type": "exit", "taskId": task_exit }));
      return;
    }
    thread::sleep(Duration::from_millis(500));
  });

  json!({ "ok": true })
}

#[tauri::command]
pub fn host_preview_stop(
  _app: AppHandle,
  state: tauri::State<HostPreviewState>,
  task_id: String,
) -> Value {
  let mut map = state.procs.lock().unwrap();
  if let Some(mut child) = map.remove(&task_id) {
    let _ = child.kill();
  }
  json!({ "ok": true })
}

#[tauri::command]
pub fn host_preview_stop_all(
  _app: AppHandle,
  state: tauri::State<HostPreviewState>,
  except_id: Option<String>,
) -> Value {
  let except = except_id.unwrap_or_default();
  let mut map = state.procs.lock().unwrap();
  let mut stopped: Vec<String> = Vec::new();
  let keys: Vec<String> = map.keys().cloned().collect();
  for key in keys {
    if !except.is_empty() && key == except {
      continue;
    }
    if let Some(mut child) = map.remove(&key) {
      let _ = child.kill();
      stopped.push(key);
    }
  }
  json!({ "ok": true, "stopped": stopped })
}
