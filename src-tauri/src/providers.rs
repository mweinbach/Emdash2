use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::fs;
use std::io::BufRead;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter, Manager};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderStatus {
  pub installed: bool,
  pub path: Option<String>,
  pub version: Option<String>,
  pub last_checked: i64,
}

#[derive(Default)]
pub struct ProviderState {
  cache: Mutex<HashMap<String, ProviderStatus>>,
  cache_path: PathBuf,
}

impl ProviderState {
  pub fn new(app: &AppHandle) -> Self {
    let dir = app
      .path()
      .app_data_dir()
      .ok()
      .or_else(|| app.path().app_config_dir().ok())
      .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));
    let cache_path = dir.join("provider-status-cache.json");
    let cache = load_cache(&cache_path);
    Self {
      cache: Mutex::new(cache),
      cache_path,
    }
  }

  fn persist(&self) {
    let payload = match serde_json::to_string_pretty(&*self.cache.lock().unwrap()) {
      Ok(data) => data,
      Err(_) => return,
    };
    if let Some(parent) = self.cache_path.parent() {
      let _ = fs::create_dir_all(parent);
    }
    let _ = fs::write(&self.cache_path, payload);
  }

  fn set(&self, id: &str, status: ProviderStatus) {
    let mut guard = self.cache.lock().unwrap();
    guard.insert(id.to_string(), status);
    drop(guard);
    self.persist();
  }

  fn all(&self) -> HashMap<String, ProviderStatus> {
    self.cache.lock().unwrap().clone()
  }
}

fn load_cache(path: &Path) -> HashMap<String, ProviderStatus> {
  if let Ok(raw) = fs::read_to_string(path) {
    if let Ok(parsed) = serde_json::from_str::<HashMap<String, ProviderStatus>>(&raw) {
      return parsed;
    }
  }
  HashMap::new()
}

#[derive(Clone)]
struct ProviderDef {
  id: &'static str,
  commands: &'static [&'static str],
  args: &'static [&'static str],
}

const PROVIDERS: &[ProviderDef] = &[
  ProviderDef {
    id: "codex",
    commands: &["codex"],
    args: &["--version"],
  },
  ProviderDef {
    id: "claude",
    commands: &["claude"],
    args: &["--version"],
  },
  ProviderDef {
    id: "cursor",
    commands: &["cursor-agent", "cursor"],
    args: &["--version"],
  },
  ProviderDef {
    id: "gemini",
    commands: &["gemini"],
    args: &["--version"],
  },
  ProviderDef {
    id: "qwen",
    commands: &["qwen"],
    args: &["--version"],
  },
  ProviderDef {
    id: "droid",
    commands: &["droid"],
    args: &["--version"],
  },
  ProviderDef {
    id: "amp",
    commands: &["amp"],
    args: &["--version"],
  },
  ProviderDef {
    id: "opencode",
    commands: &["opencode"],
    args: &["--version"],
  },
  ProviderDef {
    id: "copilot",
    commands: &["copilot"],
    args: &["--version"],
  },
  ProviderDef {
    id: "charm",
    commands: &["crush"],
    args: &["--version"],
  },
  ProviderDef {
    id: "auggie",
    commands: &["auggie"],
    args: &["--version"],
  },
  ProviderDef {
    id: "kimi",
    commands: &["kimi"],
    args: &["--version"],
  },
  ProviderDef {
    id: "kilocode",
    commands: &["kilocode"],
    args: &["--version"],
  },
  ProviderDef {
    id: "kiro",
    commands: &["kiro-cli", "kiro"],
    args: &["--version"],
  },
  ProviderDef {
    id: "rovo",
    commands: &["rovodev", "acli"],
    args: &["--version"],
  },
  ProviderDef {
    id: "cline",
    commands: &["cline"],
    args: &["help"],
  },
  ProviderDef {
    id: "codebuff",
    commands: &["codebuff"],
    args: &["--version"],
  },
  ProviderDef {
    id: "mistral",
    commands: &["vibe"],
    args: &["-h"],
  },
];

#[derive(Default, Clone)]
struct CommandResult {
  command: String,
  success: bool,
  stdout: String,
  stderr: String,
  status: Option<i32>,
  version: Option<String>,
  resolved_path: Option<String>,
  timed_out: bool,
  not_found: bool,
}

fn resolve_command_path(command: &str) -> Option<String> {
  let resolver = if cfg!(target_os = "windows") {
    "where"
  } else {
    "which"
  };
  Command::new(resolver)
    .arg(command)
    .output()
    .ok()
    .and_then(|out| {
      if out.status.success() {
        let line = String::from_utf8_lossy(&out.stdout)
          .lines()
          .map(|l| l.trim())
          .find(|l| !l.is_empty())
          .map(|l| l.to_string());
        line
      } else {
        None
      }
    })
}

fn extract_version(output: &str) -> Option<String> {
  if output.is_empty() {
    return None;
  }
  let mut buf = String::new();
  let mut started = false;
  for ch in output.chars() {
    if ch.is_ascii_digit() {
      started = true;
      buf.push(ch);
      continue;
    }
    if started && ch == '.' {
      buf.push(ch);
      continue;
    }
    if started {
      if buf.contains('.') {
        return Some(buf);
      }
      buf.clear();
      started = false;
    }
  }
  if started && buf.contains('.') {
    Some(buf)
  } else {
    None
  }
}

fn run_command(command: &str, args: &[&str], timeout_ms: u64) -> CommandResult {
  let mut result = CommandResult::default();
  result.command = command.to_string();
  result.resolved_path = resolve_command_path(command);

  let mut child = match Command::new(command)
    .args(args)
    .stdout(Stdio::piped())
    .stderr(Stdio::piped())
    .spawn()
  {
    Ok(child) => child,
    Err(err) => {
      result.not_found = err.kind() == std::io::ErrorKind::NotFound;
      return result;
    }
  };

  let stdout = child.stdout.take();
  let stderr = child.stderr.take();
  let stdout_buf = Arc::new(Mutex::new(String::new()));
  let stderr_buf = Arc::new(Mutex::new(String::new()));

  if let Some(out) = stdout {
    let buf = stdout_buf.clone();
    std::thread::spawn(move || {
      let mut reader = std::io::BufReader::new(out);
      let mut line = String::new();
      while reader.read_line(&mut line).is_ok() {
        if line.is_empty() {
          break;
        }
        buf.lock().unwrap().push_str(&line);
        line.clear();
      }
    });
  }
  if let Some(err) = stderr {
    let buf = stderr_buf.clone();
    std::thread::spawn(move || {
      let mut reader = std::io::BufReader::new(err);
      let mut line = String::new();
      while reader.read_line(&mut line).is_ok() {
        if line.is_empty() {
          break;
        }
        buf.lock().unwrap().push_str(&line);
        line.clear();
      }
    });
  }

  let start = Instant::now();
  let mut timed_out = false;
  let status = loop {
    if start.elapsed() >= Duration::from_millis(timeout_ms) {
      timed_out = true;
      let _ = child.kill();
      break None;
    }
    match child.try_wait() {
      Ok(Some(status)) => break Some(status),
      Ok(None) => {
        std::thread::sleep(Duration::from_millis(50));
      }
      Err(_) => break None,
    }
  };

  let stdout_final = stdout_buf.lock().unwrap().clone();
  let stderr_final = stderr_buf.lock().unwrap().clone();
  let version = extract_version(&stdout_final).or_else(|| extract_version(&stderr_final));

  result.timed_out = timed_out;
  result.stdout = stdout_final;
  result.stderr = stderr_final;
  result.version = version;
  if let Some(status) = status {
    result.status = status.code();
    result.success = !timed_out && status.success();
  } else {
    result.status = None;
    result.success = false;
  }
  result
}

fn check_provider(def: &ProviderDef, timeout_ms: u64) -> CommandResult {
  let mut last = CommandResult::default();
  for cmd in def.commands {
    let res = run_command(cmd, def.args, timeout_ms);
    last = res.clone();
    if res.success {
      return res;
    }
    if !res.not_found {
      return res;
    }
  }
  last
}

fn compute_status(result: &CommandResult) -> bool {
  if result.timed_out && (result.resolved_path.is_some() || !result.stdout.is_empty()) {
    return true;
  }
  result.success
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderStatusOptions {
  refresh: Option<bool>,
  providers: Option<Vec<String>>,
  provider_id: Option<String>,
}

#[tauri::command]
pub fn providers_get_statuses(
  app: AppHandle,
  state: tauri::State<ProviderState>,
  opts: Option<ProviderStatusOptions>,
) -> Value {
  let refresh = opts.as_ref().and_then(|o| o.refresh).unwrap_or(false);
  if !refresh {
    return json!({ "success": true, "statuses": state.all() });
  }

  let opts_ref = opts.as_ref();
  let requested = if let Some(list) = opts_ref.and_then(|o| o.providers.clone()) {
    if !list.is_empty() {
      list
    } else if let Some(id) = opts_ref.and_then(|o| o.provider_id.clone()) {
      vec![id]
    } else {
      PROVIDERS.iter().map(|p| p.id.to_string()).collect()
    }
  } else if let Some(id) = opts_ref.and_then(|o| o.provider_id.clone()) {
    vec![id]
  } else {
    PROVIDERS.iter().map(|p| p.id.to_string()).collect()
  };

  let now = chrono::Utc::now().timestamp_millis();
  for id in requested {
    if let Some(def) = PROVIDERS.iter().find(|p| p.id == id) {
      let res = check_provider(def, 3000);
      let status = ProviderStatus {
        installed: compute_status(&res),
        path: res.resolved_path,
        version: res.version,
        last_checked: now,
      };
      state.set(def.id, status.clone());
      let payload = json!({ "providerId": def.id, "status": status });
      let _ = app.emit("provider:status-updated", payload);
    }
  }

  json!({ "success": true, "statuses": state.all() })
}
