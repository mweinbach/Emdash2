use base64::{engine::general_purpose::STANDARD, Engine as _};
use crate::runtime::run_blocking;
use rand::Rng;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::io::Read;
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::process::Command;
use tauri::{AppHandle, Emitter, Manager};

const CONFIG_RELATIVE_PATH: &str = ".emdash/config.json";
const DEFAULT_VERSION: i64 = 1;
const DEFAULT_START_COMMAND: &str = "npm run dev";
const DEFAULT_BUN_START_COMMAND: &str = "bun run dev";
const DEFAULT_WORKDIR: &str = ".";
const DEFAULT_PREVIEW_SERVICE: &str = "app";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResolvedContainerPortConfig {
  pub service: String,
  pub container: u16,
  pub protocol: String,
  pub preview: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResolvedContainerConfig {
  pub version: i64,
  pub package_manager: String,
  pub start: String,
  pub env_file: Option<String>,
  pub workdir: String,
  pub ports: Vec<ResolvedContainerPortConfig>,
}

#[derive(Debug)]
struct ContainerConfigError {
  message: String,
  path: Option<String>,
}

#[derive(Debug)]
struct ContainerConfigLoadError {
  code: String,
  message: String,
  config_path: Option<String>,
  config_key: Option<String>,
}

#[derive(Debug)]
struct ContainerConfigLoadResult {
  ok: bool,
  config: Option<ResolvedContainerConfig>,
  source_path: Option<String>,
  error: Option<ContainerConfigLoadError>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ContainerLoadArgs {
  task_path: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ContainerStartArgs {
  task_id: String,
  task_path: String,
  run_id: Option<String>,
  mode: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ContainerStopArgs {
  task_id: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ContainerInspectArgs {
  task_id: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResolveIconArgs {
  service: String,
  allow_network: Option<bool>,
  task_path: Option<String>,
}

#[derive(Default)]
pub struct ContainerState {}

impl ContainerState {
  pub fn new() -> Self {
    Self {}
  }
}

fn now_ms() -> i64 {
  chrono::Utc::now().timestamp_millis()
}

fn emit_runner_event(app: &AppHandle, event: Value) {
  let _ = app.emit("run:event", event);
}

fn default_port() -> ResolvedContainerPortConfig {
  ResolvedContainerPortConfig {
    service: DEFAULT_PREVIEW_SERVICE.to_string(),
    container: 3000,
    protocol: "tcp".to_string(),
    preview: true,
  }
}

fn infer_package_manager(task_path: &Path) -> Option<String> {
  let candidates = [
    ("bun.lockb", "bun"),
    ("bun.lock", "bun"),
    ("pnpm-lock.yaml", "pnpm"),
    ("yarn.lock", "yarn"),
    ("package-lock.json", "npm"),
    ("npm-shrinkwrap.json", "npm"),
  ];
  for (file, pm) in candidates {
    if task_path.join(file).exists() {
      return Some(pm.to_string());
    }
  }
  None
}

fn resolve_version(raw: Option<&Value>) -> Result<i64, ContainerConfigError> {
  match raw {
    None | Some(Value::Null) => Ok(DEFAULT_VERSION),
    Some(Value::Number(num)) if num.is_i64() => {
      let v = num.as_i64().unwrap_or(DEFAULT_VERSION);
      if v != DEFAULT_VERSION {
        return Err(ContainerConfigError {
          message: "Only config version 1 is supported in M1".to_string(),
          path: Some("version".to_string()),
        });
      }
      Ok(v)
    }
    _ => Err(ContainerConfigError {
      message: "`version` must be an integer".to_string(),
      path: Some("version".to_string()),
    }),
  }
}

fn resolve_package_manager(
  raw: Option<&Value>,
  inferred: Option<String>,
) -> Result<String, ContainerConfigError> {
  if raw.is_none() || matches!(raw, Some(Value::Null)) {
    return Ok(inferred.unwrap_or_else(|| "npm".to_string()));
  }
  let value = raw.and_then(|v| v.as_str()).unwrap_or("");
  let normalized = value.trim().to_lowercase();
  let allowed = ["npm", "pnpm", "yarn", "bun"];
  if !allowed.contains(&normalized.as_str()) {
    return Err(ContainerConfigError {
      message: "`packageManager` must be one of \"npm\", \"pnpm\", \"yarn\", or \"bun\"".to_string(),
      path: Some("packageManager".to_string()),
    });
  }
  Ok(normalized)
}

fn resolve_start_command(raw: Option<&Value>, package_manager: &str) -> Result<String, ContainerConfigError> {
  if raw.is_none() || matches!(raw, Some(Value::Null)) {
    return Ok(if package_manager == "bun" {
      DEFAULT_BUN_START_COMMAND.to_string()
    } else {
      DEFAULT_START_COMMAND.to_string()
    });
  }
  let value = raw.and_then(|v| v.as_str()).unwrap_or("").trim();
  if value.is_empty() {
    return Err(ContainerConfigError {
      message: "`start` cannot be empty".to_string(),
      path: Some("start".to_string()),
    });
  }
  Ok(value.to_string())
}

fn resolve_env_file(raw: Option<&Value>) -> Result<Option<String>, ContainerConfigError> {
  if raw.is_none() || matches!(raw, Some(Value::Null)) {
    return Ok(None);
  }
  let value = raw.and_then(|v| v.as_str()).unwrap_or("").trim();
  if value.is_empty() {
    return Err(ContainerConfigError {
      message: "`envFile` cannot be empty".to_string(),
      path: Some("envFile".to_string()),
    });
  }
  Ok(Some(value.to_string()))
}

fn resolve_workdir(raw: Option<&Value>) -> Result<String, ContainerConfigError> {
  if raw.is_none() || matches!(raw, Some(Value::Null)) {
    return Ok(DEFAULT_WORKDIR.to_string());
  }
  let value = raw.and_then(|v| v.as_str()).unwrap_or("").trim();
  if value.is_empty() {
    return Err(ContainerConfigError {
      message: "`workdir` cannot be empty".to_string(),
      path: Some("workdir".to_string()),
    });
  }
  Ok(value.to_string())
}

fn resolve_ports(raw: Option<&Value>) -> Result<Vec<ResolvedContainerPortConfig>, ContainerConfigError> {
  if raw.is_none() || matches!(raw, Some(Value::Null)) {
    return Ok(vec![default_port()]);
  }
  let list = raw.and_then(|v| v.as_array()).ok_or_else(|| ContainerConfigError {
    message: "`ports` must be an array".to_string(),
    path: Some("ports".to_string()),
  })?;

  if list.is_empty() {
    return Ok(vec![default_port()]);
  }

  let mut result = Vec::new();
  for (idx, entry) in list.iter().enumerate() {
    let path = format!("ports[{}]", idx);
    let obj = entry.as_object().ok_or_else(|| ContainerConfigError {
      message: "Each port entry must be an object".to_string(),
      path: Some(path.clone()),
    })?;
    let service = obj.get("service").and_then(|v| v.as_str()).unwrap_or("").trim();
    if service.is_empty() {
      return Err(ContainerConfigError {
        message: "`service` must be a non-empty string".to_string(),
        path: Some(format!("{}.service", path)),
      });
    }
    let container = obj.get("container").and_then(|v| v.as_i64()).unwrap_or(-1);
    if container < 1 || container > 65535 {
      return Err(ContainerConfigError {
        message: "`container` must be between 1 and 65535".to_string(),
        path: Some(format!("{}.container", path)),
      });
    }
    if let Some(protocol) = obj.get("protocol") {
      if protocol.as_str().unwrap_or("").to_lowercase() != "tcp" && !protocol.is_null() {
        return Err(ContainerConfigError {
          message: "Only TCP protocol is supported in M1".to_string(),
          path: Some(format!("{}.protocol", path)),
        });
      }
    }
    if let Some(preview) = obj.get("preview") {
      if !preview.is_boolean() {
        return Err(ContainerConfigError {
          message: "`preview` must be a boolean when provided".to_string(),
          path: Some(format!("{}.preview", path)),
        });
      }
    }
    result.push(ResolvedContainerPortConfig {
      service: service.to_string(),
      container: container as u16,
      protocol: "tcp".to_string(),
      preview: obj.get("preview").and_then(|v| v.as_bool()).unwrap_or(false),
    });
  }

  ensure_preview_port(&mut result);
  ensure_unique_services(&result)?;
  Ok(result)
}

fn ensure_preview_port(ports: &mut Vec<ResolvedContainerPortConfig>) {
  if ports.iter().any(|p| p.preview) {
    let mut seen_preview = false;
    for p in ports.iter_mut() {
      if p.preview {
        if seen_preview {
          p.preview = false;
        } else {
          seen_preview = true;
        }
      }
    }
    return;
  }
  if let Some(first) = ports.first_mut() {
    first.preview = true;
  }
}

fn ensure_unique_services(ports: &[ResolvedContainerPortConfig]) -> Result<(), ContainerConfigError> {
  let mut seen = HashSet::new();
  for (idx, port) in ports.iter().enumerate() {
    if !seen.insert(port.service.clone()) {
      return Err(ContainerConfigError {
        message: format!("Duplicate service name \"{}\" found in ports array", port.service),
        path: Some(format!("ports[{}].service", idx)),
      });
    }
  }
  Ok(())
}

fn resolve_container_config(
  input: Value,
  inferred: Option<String>,
) -> Result<ResolvedContainerConfig, ContainerConfigError> {
  let obj = input.as_object().cloned().unwrap_or_default();
  let version = resolve_version(obj.get("version"))?;
  let package_manager = resolve_package_manager(obj.get("packageManager"), inferred)?;
  let start = resolve_start_command(obj.get("start"), &package_manager)?;
  let env_file = resolve_env_file(obj.get("envFile"))?;
  let workdir = resolve_workdir(obj.get("workdir"))?;
  let ports = resolve_ports(obj.get("ports"))?;

  Ok(ResolvedContainerConfig {
    version,
    package_manager,
    start,
    env_file,
    workdir,
    ports,
  })
}

fn read_config_file(path: &Path) -> Result<Option<String>, ContainerConfigLoadError> {
  match fs::read_to_string(path) {
    Ok(content) => Ok(Some(content)),
    Err(err) => {
      if err.kind() == std::io::ErrorKind::NotFound {
        Ok(None)
      } else {
        Err(ContainerConfigLoadError {
          code: "IO_ERROR".to_string(),
          message: format!("Failed to read {}: {}", path.display(), err),
          config_path: Some(path.to_string_lossy().to_string()),
          config_key: None,
        })
      }
    }
  }
}

fn parse_config_json(content: &str, config_path: &Path) -> Result<Value, ContainerConfigLoadError> {
  serde_json::from_str::<Value>(content).map_err(|_err| ContainerConfigLoadError {
    code: "INVALID_JSON".to_string(),
    message: format!("Invalid JSON in {}", config_path.display()),
    config_path: Some(config_path.to_string_lossy().to_string()),
    config_key: None,
  })
}

fn load_task_container_config(task_path: &Path) -> ContainerConfigLoadResult {
  let config_path = task_path.join(CONFIG_RELATIVE_PATH);
  let inferred = infer_package_manager(task_path);

  let content = match read_config_file(&config_path) {
    Ok(content) => content,
    Err(err) => {
      return ContainerConfigLoadResult {
        ok: false,
        config: None,
        source_path: None,
        error: Some(err),
      }
    }
  };

  let mut source_path = None;
  let parsed = if let Some(raw) = content {
    let parsed = match parse_config_json(&raw, &config_path) {
      Ok(value) => value,
      Err(err) => {
        return ContainerConfigLoadResult {
          ok: false,
          config: None,
          source_path: None,
          error: Some(err),
        }
      }
    };
    source_path = Some(config_path.to_string_lossy().to_string());
    parsed
  } else {
    Value::Object(serde_json::Map::new())
  };

  match resolve_container_config(parsed, inferred) {
    Ok(config) => ContainerConfigLoadResult {
      ok: true,
      config: Some(config),
      source_path,
      error: None,
    },
    Err(err) => {
      let config_path_value =
        source_path.clone().or_else(|| Some(config_path.to_string_lossy().to_string()));
      ContainerConfigLoadResult {
        ok: false,
        config: None,
        source_path,
        error: Some(ContainerConfigLoadError {
          code: "VALIDATION_FAILED".to_string(),
          message: err.message,
          config_path: config_path_value,
          config_key: err.path,
        }),
      }
    }
  }
}

struct PortManager {
  min_port: u16,
  max_port: u16,
  max_attempts_per_port: u16,
  host: String,
  reserved: HashSet<u16>,
}

impl PortManager {
  fn new() -> Self {
    Self {
      min_port: 49152,
      max_port: 65535,
      max_attempts_per_port: 128,
      host: "127.0.0.1".to_string(),
      reserved: HashSet::new(),
    }
  }

  fn allocate(&mut self, requests: &[ResolvedContainerPortConfig]) -> Result<Vec<RunnerPortMapping>, String> {
    if requests.is_empty() {
      return Ok(Vec::new());
    }
    let mut allocations = Vec::new();
    for req in requests {
      let host_port = self.find_available_port().map_err(|e| e)?;
      self.reserved.insert(host_port);
      allocations.push(RunnerPortMapping {
        service: req.service.clone(),
        protocol: req.protocol.clone(),
        container: req.container,
        host: host_port,
      });
    }
    Ok(allocations)
  }

  fn find_available_port(&mut self) -> Result<u16, String> {
    let mut attempted = HashSet::new();
    let range = self.max_port - self.min_port + 1;
    for _ in 0..self.max_attempts_per_port {
      let candidate = self.min_port + rand::thread_rng().gen_range(0..range);
      if attempted.contains(&candidate) {
        continue;
      }
      attempted.insert(candidate);
      if self.reserved.contains(&candidate) {
        continue;
      }
      if self.check_port_availability(candidate) {
        return Ok(candidate);
      }
    }
    Err("Unable to allocate a free host port".to_string())
  }

  fn check_port_availability(&self, port: u16) -> bool {
    TcpListener::bind((self.host.as_str(), port)).is_ok()
  }
}

#[derive(Debug, Clone, Serialize)]
struct RunnerPortMapping {
  service: String,
  protocol: String,
  container: u16,
  host: u16,
}

fn detect_package_manager_from_workdir(workdir: &Path) -> String {
  if workdir.join("bun.lockb").exists() || workdir.join("bun.lock").exists() {
    return "bun".to_string();
  }
  if workdir.join("pnpm-lock.yaml").exists() {
    return "pnpm".to_string();
  }
  if workdir.join("yarn.lock").exists() {
    return "yarn".to_string();
  }
  if workdir.join("package-lock.json").exists() || workdir.join("npm-shrinkwrap.json").exists() {
    return "npm".to_string();
  }
  "npm".to_string()
}

fn generate_run_id() -> String {
  format!("r_{}", chrono::Utc::now().to_rfc3339())
}

fn choose_preview_service(requests: &[ResolvedContainerPortConfig]) -> String {
  let names = ["web", "app", "frontend", "ui"];
  for name in names {
    if let Some(p) = requests.iter().find(|r| r.service == name) {
      return p.service.clone();
    }
  }
  if let Some(p) = requests
    .iter()
    .find(|r| [3000, 5173, 8080, 8000].contains(&(r.container as i32)))
  {
    return p.service.clone();
  }
  requests.first().map(|r| r.service.clone()).unwrap_or_else(|| DEFAULT_PREVIEW_SERVICE.to_string())
}

fn choose_preview_service_from_published(ports: &[RunnerPortMapping]) -> Option<String> {
  if ports.is_empty() {
    return None;
  }
  let names = ["web", "app", "frontend", "ui"];
  for name in names {
    if let Some(p) = ports.iter().find(|r| r.service == name) {
      return Some(p.service.clone());
    }
  }
  if let Some(p) = ports.iter().find(|r| [3000, 5173, 8080, 8000].contains(&(r.container as i32))) {
    return Some(p.service.clone());
  }
  ports.first().map(|p| p.service.clone())
}

fn emit_lifecycle(app: &AppHandle, task_id: &str, run_id: &str, mode: &str, status: &str, container_id: Option<String>) {
  let mut payload = json!({
    "ts": now_ms(),
    "taskId": task_id,
    "runId": run_id,
    "mode": mode,
    "type": "lifecycle",
    "status": status,
  });
  if let Some(cid) = container_id {
    if let Some(obj) = payload.as_object_mut() {
      obj.insert("containerId".to_string(), Value::String(cid));
    }
  }
  emit_runner_event(app, payload);
}

fn emit_ports(
  app: &AppHandle,
  task_id: &str,
  run_id: &str,
  mode: &str,
  ports: &[RunnerPortMapping],
  preview_service: &str,
) {
  let mapped: Vec<Value> = ports
    .iter()
    .map(|p| {
      json!({
        "service": p.service,
        "protocol": "tcp",
        "container": p.container,
        "host": p.host,
        "url": format!("http://localhost:{}", p.host),
      })
    })
    .collect();
  emit_runner_event(
    app,
    json!({
      "ts": now_ms(),
      "taskId": task_id,
      "runId": run_id,
      "mode": mode,
      "type": "ports",
      "previewService": preview_service,
      "ports": mapped,
    }),
  );
}

fn emit_error(app: &AppHandle, task_id: &str, run_id: &str, mode: &str, code: &str, message: &str) {
  emit_runner_event(
    app,
    json!({
      "ts": now_ms(),
      "taskId": task_id,
      "runId": run_id,
      "mode": mode,
      "type": "error",
      "code": code,
      "message": message,
    }),
  );
}

fn find_compose_file(task_path: &Path) -> Option<PathBuf> {
  let candidates = [
    "docker-compose.yml",
    "docker-compose.yaml",
    "compose.yml",
    "compose.yaml",
  ];
  for rel in candidates {
    let abs = task_path.join(rel);
    if abs.exists() {
      return Some(abs);
    }
  }
  None
}

fn build_compose_override_yaml(mappings: &[RunnerPortMapping]) -> String {
  let mut by_service: HashMap<String, Vec<&RunnerPortMapping>> = HashMap::new();
  for mapping in mappings {
    by_service
      .entry(mapping.service.clone())
      .or_default()
      .push(mapping);
  }

  let mut lines = Vec::new();
  lines.push("services:".to_string());
  for (svc, ports) in by_service {
    lines.push(format!("  {}:", svc));
    lines.push("    ports:".to_string());
    for p in ports {
      lines.push("      -".to_string());
      lines.push(format!("        target: {}", p.container));
      lines.push(format!("        published: {}", p.host));
      lines.push("        protocol: tcp".to_string());
    }
  }

  lines.join("\n") + "\n"
}

fn parse_compose_ps(out: &str, fallback: &[RunnerPortMapping]) -> Vec<RunnerPortMapping> {
  let trimmed = out.trim();
  if trimmed.is_empty() {
    return fallback.to_vec();
  }
  let mut records: Vec<Value> = Vec::new();
  if trimmed.starts_with('[') {
    if let Ok(parsed) = serde_json::from_str::<Value>(trimmed) {
      if let Some(list) = parsed.as_array() {
        records = list.clone();
      }
    }
  } else {
    for line in trimmed.lines() {
      if let Ok(parsed) = serde_json::from_str::<Value>(line) {
        records.push(parsed);
      }
    }
  }
  let mut result = Vec::new();
  for rec in records {
    let service = rec
      .get("Service")
      .or_else(|| rec.get("service"))
      .or_else(|| rec.get("Name"))
      .or_else(|| rec.get("name"))
      .and_then(|v| v.as_str())
      .unwrap_or("")
      .to_string();
    if service.is_empty() {
      continue;
    }
    let ports = rec
      .get("Publishers")
      .or_else(|| rec.get("Ports"))
      .and_then(|v| v.as_array())
      .cloned()
      .unwrap_or_default();
    for port in ports {
      let target = port
        .get("TargetPort")
        .or_else(|| port.get("target"))
        .or_else(|| port.get("Target"))
        .or_else(|| port.get("ContainerPort"))
        .and_then(|v| v.as_i64());
      let published = port
        .get("PublishedPort")
        .or_else(|| port.get("published"))
        .or_else(|| port.get("HostPort"))
        .and_then(|v| v.as_i64());
      if let (Some(target), Some(published)) = (target, published) {
        result.push(RunnerPortMapping {
          service: service.clone(),
          protocol: "tcp".to_string(),
          container: target as u16,
          host: published as u16,
        });
      }
    }
  }
  if result.is_empty() {
    fallback.to_vec()
  } else {
    result
  }
}

fn load_compose_config_json(compose_file: &Path, task_path: &Path) -> Result<Value, String> {
  let output = Command::new("docker")
    .args([
      "compose",
      "-f",
      compose_file.to_string_lossy().as_ref(),
      "config",
      "--format",
      "json",
    ])
    .current_dir(task_path)
    .output()
    .map_err(|err| err.to_string())?;
  if !output.status.success() {
    return Err(String::from_utf8_lossy(&output.stderr).to_string());
  }
  let stdout = String::from_utf8_lossy(&output.stdout);
  serde_json::from_str(&stdout).map_err(|err| err.to_string())
}

fn sanitize_compose_config(config: &Value, requested: &HashMap<String, Vec<u16>>) -> Value {
  let mut next = config.clone();
  let services = config.get("services").and_then(|v| v.as_object()).cloned().unwrap_or_default();
  let mut next_services = serde_json::Map::new();
  for (name, svc) in services {
    let mut svc_obj = svc.as_object().cloned().unwrap_or_default();
    let mut expose_ports: Vec<u16> = Vec::new();
    if let Some(expose) = svc_obj.get("expose").and_then(|v| v.as_array()) {
      for ex in expose {
        if let Some(num) = ex.as_i64() {
          if num > 0 && num <= 65535 {
            expose_ports.push(num as u16);
          }
        }
      }
    }
    if let Some(req) = requested.get(&name) {
      for port in req {
        if !expose_ports.contains(port) {
          expose_ports.push(*port);
        }
      }
    }
    svc_obj.remove("ports");
    if !expose_ports.is_empty() {
      let expose_vals: Vec<Value> = expose_ports.into_iter().map(|p| json!(p)).collect();
      svc_obj.insert("expose".to_string(), Value::Array(expose_vals));
    }
    next_services.insert(name, Value::Object(svc_obj));
  }
  if let Some(obj) = next.as_object_mut() {
    obj.insert("services".to_string(), Value::Object(next_services));
  }
  next
}

fn discover_compose_ports(compose_file: &Path, task_path: &Path) -> Vec<(String, u16)> {
  let output = Command::new("docker")
    .args([
      "compose",
      "-f",
      compose_file.to_string_lossy().as_ref(),
      "config",
      "--format",
      "json",
    ])
    .current_dir(task_path)
    .output();
  let output = match output {
    Ok(out) if out.status.success() => out,
    _ => return Vec::new(),
  };
  let stdout = String::from_utf8_lossy(&output.stdout);
  let cfg: Value = match serde_json::from_str(&stdout) {
    Ok(v) => v,
    Err(_) => return Vec::new(),
  };
  let services = cfg.get("services").and_then(|v| v.as_object()).cloned().unwrap_or_default();
  let mut result: Vec<(String, u16)> = Vec::new();
  for (svc_name, svc) in services {
    let ports = svc.get("ports").and_then(|v| v.as_array()).cloned().unwrap_or_default();
    for entry in ports {
      if let Some(obj) = entry.as_object() {
        let target = obj
          .get("target")
          .or_else(|| obj.get("TargetPort"))
          .or_else(|| obj.get("ContainerPort"))
          .and_then(|v| v.as_i64());
        let protocol = obj
          .get("protocol")
          .and_then(|v| v.as_str())
          .unwrap_or("tcp")
          .to_lowercase();
        if protocol == "tcp" {
          if let Some(t) = target {
            if t > 0 && t <= 65535 {
              result.push((svc_name.clone(), t as u16));
            }
          }
        }
      } else if let Some(raw) = entry.as_str() {
        let main = raw.split('/').next().unwrap_or("");
        let parts: Vec<&str> = main.split(':').collect();
        let port_str = parts.last().unwrap_or(&"");
        if let Ok(port) = port_str.parse::<u16>() {
          result.push((svc_name.clone(), port));
        }
      }
    }
  }
  let mut seen = HashSet::new();
  result
    .into_iter()
    .filter(|(svc, port)| seen.insert(format!("{}:{}", svc, port)))
    .collect()
}

fn resolve_preview_service(requests: &[ResolvedContainerPortConfig]) -> String {
  if let Some(port) = requests.iter().find(|p| p.preview) {
    return port.service.clone();
  }
  choose_preview_service(requests)
}

fn container_start_mock_run(
  app: &AppHandle,
  task_id: &str,
  run_id: &str,
  mode: &str,
  config: &ResolvedContainerConfig,
) -> Result<(), String> {
  let mut port_manager = PortManager::new();
  let ports = port_manager.allocate(&config.ports)?;
  let preview_service = resolve_preview_service(&config.ports);
  emit_lifecycle(app, task_id, run_id, mode, "building", None);
  emit_lifecycle(app, task_id, run_id, mode, "starting", Some(format!("emdash_ws_{}", task_id)));
  emit_ports(app, task_id, run_id, mode, &ports, &preview_service);
  emit_lifecycle(app, task_id, run_id, mode, "ready", None);
  Ok(())
}

fn container_start_compose_run(
  app: &AppHandle,
  task_id: &str,
  task_path: &Path,
  run_id: &str,
  mode: &str,
  config: &ResolvedContainerConfig,
  compose_file: &Path,
) -> Result<String, String> {
  let output = Command::new("docker")
    .args(["compose", "version"])
    .output()
    .map_err(|err| err.to_string())?;
  if !output.status.success() {
    let message = "Docker Compose is not available. Please install/update Docker Desktop.";
    emit_error(app, task_id, run_id, mode, "UNKNOWN", message);
    return Err(message.to_string());
  }

  let discovered = discover_compose_ports(compose_file, task_path);
  let mut port_requests: Vec<ResolvedContainerPortConfig> = Vec::new();
  if !discovered.is_empty() {
    for (service, container) in discovered {
      port_requests.push(ResolvedContainerPortConfig {
        service,
        container,
        protocol: "tcp".to_string(),
        preview: false,
      });
    }
  } else {
    port_requests = config.ports.clone();
  }

  let mut port_manager = PortManager::new();
  let allocations = port_manager.allocate(&port_requests)?;

  let preview_service = if port_requests.iter().any(|p| p.preview) {
    port_requests.iter().find(|p| p.preview).map(|p| p.service.clone()).unwrap_or_else(|| choose_preview_service(&port_requests))
  } else {
    choose_preview_service(&port_requests)
  };

  let sanitized_path = task_path.join(".emdash").join("compose.sanitized.json");
  let override_path = task_path.join(".emdash").join("compose.override.yml");
  if let Some(parent) = sanitized_path.parent() {
    let _ = fs::create_dir_all(parent);
  }

  let mut requested_map: HashMap<String, Vec<u16>> = HashMap::new();
  for req in &port_requests {
    requested_map.entry(req.service.clone()).or_default().push(req.container);
  }

  if let Ok(cfg_json) = load_compose_config_json(compose_file, task_path) {
    let sanitized = sanitize_compose_config(&cfg_json, &requested_map);
    let _ = fs::write(&sanitized_path, serde_json::to_string_pretty(&sanitized).unwrap_or_default());
  }

  let override_yaml = build_compose_override_yaml(&allocations);
  let _ = fs::write(&override_path, override_yaml);

  let project = format!("emdash_ws_{}", task_id);
  let mut args: Vec<String> = vec![
    "compose".into(),
  ];
  if let Some(env_file) = &config.env_file {
    let env_abs = task_path.join(env_file);
    if env_abs.exists() {
      args.push("--env-file".into());
      args.push(env_abs.to_string_lossy().to_string());
    }
  }
  let compose_path_for_up = if sanitized_path.exists() {
    sanitized_path.clone()
  } else {
    compose_file.to_path_buf()
  };
  args.push("-p".into());
  args.push(project.clone());
  args.push("-f".into());
  args.push(compose_path_for_up.to_string_lossy().to_string());
  args.push("-f".into());
  args.push(override_path.to_string_lossy().to_string());
  args.push("up".into());
  args.push("-d".into());

  emit_lifecycle(app, task_id, run_id, mode, "starting", None);
  let output = Command::new("docker")
    .args(args)
    .current_dir(task_path)
    .output()
    .map_err(|err| err.to_string())?;
  if !output.status.success() {
    let message = String::from_utf8_lossy(&output.stderr).to_string();
    emit_error(app, task_id, run_id, mode, "UNKNOWN", &message);
    return Err(message);
  }

  let ps_output = Command::new("docker")
    .args(["compose", "-p", &project, "ps", "--format", "json"])
    .output()
    .ok();
  let published = ps_output
    .and_then(|out| {
      if out.status.success() {
        Some(parse_compose_ps(&String::from_utf8_lossy(&out.stdout), &allocations))
      } else {
        None
      }
    })
    .unwrap_or_else(|| allocations.clone());

  emit_ports(app, task_id, run_id, mode, &published, &preview_service);
  emit_lifecycle(app, task_id, run_id, mode, "ready", None);
  Ok(project)
}

#[tauri::command]
pub async fn container_load_config(args: ContainerLoadArgs) -> Value {
  run_blocking(
    json!({ "ok": false, "error": { "code": "UNKNOWN", "message": "Task cancelled", "configPath": null, "configKey": null } }),
    move || {
      let task_path = args.task_path.trim();
      if task_path.is_empty() {
        return json!({
          "ok": false,
          "error": {
            "code": "INVALID_ARGUMENT",
            "message": "`taskPath` must be a non-empty string",
            "configPath": null,
            "configKey": null
          }
        });
      }

      let result = load_task_container_config(Path::new(task_path));
      if result.ok {
        json!({
          "ok": true,
          "config": result.config,
          "sourcePath": result.source_path,
        })
      } else {
        let err = result.error.unwrap();
        json!({
          "ok": false,
          "error": {
            "code": err.code,
            "message": err.message,
            "configPath": err.config_path,
            "configKey": err.config_key,
          }
        })
      }
    },
  )
  .await
}

#[tauri::command]
pub async fn container_start_run(app: AppHandle, args: ContainerStartArgs) -> Value {
  run_blocking(
    json!({ "ok": false, "error": { "code": "UNKNOWN", "message": "Task cancelled", "configPath": null, "configKey": null } }),
    move || {
      let task_id = args.task_id.trim();
      let task_path = args.task_path.trim();
      if task_id.is_empty() || task_path.is_empty() {
        return json!({
          "ok": false,
          "error": {
            "code": "INVALID_ARGUMENT",
            "message": "`taskId` and `taskPath` are required",
            "configPath": null,
            "configKey": null,
          }
        });
      }

      let load_result = load_task_container_config(Path::new(task_path));
      if !load_result.ok {
        let err = load_result.error.unwrap();
        return json!({
          "ok": false,
          "error": {
            "code": err.code,
            "message": err.message,
            "configPath": err.config_path,
            "configKey": err.config_key,
          }
        });
      }
      let config = load_result.config.unwrap();
      let run_id = args.run_id.unwrap_or_else(generate_run_id);
      let mode = args.mode.unwrap_or_else(|| "container".to_string());

      if mode != "container" {
        if let Err(err) = container_start_mock_run(&app, task_id, &run_id, &mode, &config) {
          emit_error(&app, task_id, &run_id, &mode, "UNKNOWN", &err);
          return json!({
            "ok": false,
            "error": {
              "code": "UNKNOWN",
              "message": err,
              "configPath": null,
              "configKey": null,
            }
          });
        }
        return json!({ "ok": true, "runId": run_id, "sourcePath": load_result.source_path });
      }

      let abs_task_path = PathBuf::from(task_path);
      let workdir_abs = abs_task_path.join(&config.workdir);
      if !workdir_abs.exists() {
        let message = format!("Configured workdir does not exist: {}", workdir_abs.display());
        emit_error(&app, task_id, &run_id, &mode, "INVALID_CONFIG", &message);
        return json!({
          "ok": false,
          "error": {
            "code": "INVALID_ARGUMENT",
            "message": message,
            "configPath": workdir_abs.to_string_lossy(),
            "configKey": "workdir",
          }
        });
      }

      let pkg_json = workdir_abs.join("package.json");
      if !pkg_json.exists() {
        let message = format!(
          "No package.json found in workdir: {}. Set the correct 'workdir' in .emdash/config.json",
          workdir_abs.display()
        );
        emit_error(&app, task_id, &run_id, &mode, "INVALID_CONFIG", &message);
        return json!({
          "ok": false,
          "error": {
            "code": "INVALID_ARGUMENT",
            "message": message,
            "configPath": workdir_abs.to_string_lossy(),
            "configKey": "workdir",
          }
        });
      }

      let docker_info = Command::new("docker")
        .args(["info", "--format", "{{.ServerVersion}}"]) 
        .output();
      if docker_info.is_err() || !docker_info.as_ref().unwrap().status.success() {
        let message = "Docker is not available or not responding. Please start Docker Desktop.";
        emit_error(&app, task_id, &run_id, &mode, "DOCKER_NOT_AVAILABLE", message);
        return json!({
          "ok": false,
          "error": {
            "code": "UNKNOWN",
            "message": message,
            "configPath": null,
            "configKey": null,
          }
        });
  }

  if let Some(compose_file) = find_compose_file(&abs_task_path) {
    if let Err(err) = container_start_compose_run(&app, task_id, &abs_task_path, &run_id, &mode, &config, &compose_file) {
      return json!({
        "ok": false,
        "error": {
          "code": "UNKNOWN",
          "message": err,
          "configPath": null,
          "configKey": null,
        }
      });
    }
    return json!({ "ok": true, "runId": run_id, "sourcePath": load_result.source_path });
  }

  let mut port_manager = PortManager::new();
  let allocations = match port_manager.allocate(&config.ports) {
    Ok(ports) => ports,
    Err(err) => {
      emit_error(&app, task_id, &run_id, &mode, "PORT_ALLOC_FAILED", &err);
      return json!({
        "ok": false,
        "error": {
          "code": "PORT_ALLOC_FAILED",
          "message": err,
          "configPath": null,
          "configKey": null,
        }
      });
    }
  };

  let preview_service = resolve_preview_service(&config.ports);
  let preview_mapping = allocations.iter().find(|m| m.service == preview_service);

  emit_lifecycle(&app, task_id, &run_id, &mode, "building", None);

  let container_name = format!("emdash_ws_{}", task_id);
  let _ = Command::new("docker")
    .args(["rm", "-f", &container_name])
    .output();

  let detected_pm = detect_package_manager_from_workdir(&workdir_abs);
  let image = if detected_pm == "bun" { "oven/bun:1.3.5" } else { "node:20" };

  let mut args_vec: Vec<String> = vec!["run".into(), "-d".into(), "--name".into(), container_name.clone()];
  for mapping in &allocations {
    args_vec.push("-p".into());
    args_vec.push(format!("{}:{}", mapping.host, mapping.container));
  }
  args_vec.push("-v".into());
  args_vec.push(format!("{}:/workspace", abs_task_path.to_string_lossy()));
  let workdir = Path::new("/workspace").join(config.workdir.replace('\\', "/"));
  args_vec.push("-w".into());
  args_vec.push(workdir.to_string_lossy().to_string());
  args_vec.push("-e".into());
  args_vec.push("HOST=0.0.0.0".into());
  if let Some(preview) = preview_mapping {
    args_vec.push("-e".into());
    args_vec.push(format!("PORT={}", preview.container));
  }
  if let Some(env_file) = &config.env_file {
    let env_abs = abs_task_path.join(env_file);
    if !env_abs.exists() {
      let message = format!("Env file not found: {}", env_abs.display());
      emit_error(&app, task_id, &run_id, &mode, "ENVFILE_NOT_FOUND", &message);
      return json!({
        "ok": false,
        "error": {
          "code": "UNKNOWN",
          "message": message,
          "configPath": env_abs.to_string_lossy(),
          "configKey": "envFile",
        }
      });
    }
    args_vec.push("--env-file".into());
    args_vec.push(env_abs.to_string_lossy().to_string());
  }

  let install_cmd = match detected_pm.as_str() {
    "npm" => "if [ -f package-lock.json ]; then npm ci; else npm install --no-package-lock; fi",
    "bun" => "if [ -f bun.lockb ] || [ -f bun.lock ]; then bun install --frozen-lockfile; else bun install; fi",
    "pnpm" => "corepack enable && if [ -f pnpm-lock.yaml ]; then pnpm install --frozen-lockfile; else pnpm install; fi",
    "yarn" => "corepack enable && if [ -f yarn.lock ]; then yarn install --frozen-lockfile || yarn install; else yarn install; fi",
    _ => "npm install",
  };
  let script = format!("{} && {}", install_cmd, config.start);

  args_vec.push(image.to_string());
  args_vec.push("bash".into());
  args_vec.push("-lc".into());
  args_vec.push(script);

  emit_lifecycle(&app, task_id, &run_id, &mode, "starting", None);

  let output = Command::new("docker")
    .args(args_vec)
    .current_dir(&abs_task_path)
    .output();
  let output = match output {
    Ok(out) => out,
    Err(err) => {
      emit_error(&app, task_id, &run_id, &mode, "UNKNOWN", &err.to_string());
      return json!({
        "ok": false,
        "error": {
          "code": "UNKNOWN",
          "message": err.to_string(),
          "configPath": null,
          "configKey": null,
        }
      });
    }
  };
  if !output.status.success() {
    let message = String::from_utf8_lossy(&output.stderr).to_string();
    emit_error(&app, task_id, &run_id, &mode, "UNKNOWN", &message);
    return json!({
      "ok": false,
      "error": {
        "code": "UNKNOWN",
        "message": message,
        "configPath": null,
        "configKey": null,
      }
    });
  }
  let container_id = String::from_utf8_lossy(&output.stdout).trim().to_string();
  emit_ports(&app, task_id, &run_id, &mode, &allocations, &preview_service);
  emit_lifecycle(&app, task_id, &run_id, &mode, "starting", Some(container_id));
  emit_lifecycle(&app, task_id, &run_id, &mode, "ready", None);

  json!({ "ok": true, "runId": run_id, "sourcePath": load_result.source_path })
    },
  )
  .await
}

#[tauri::command]
pub async fn container_stop_run(app: AppHandle, args: ContainerStopArgs) -> Value {
  run_blocking(
    json!({ "ok": false, "error": "Task cancelled" }),
    move || {
      let task_id = args.task_id.trim();
      if task_id.is_empty() {
        return json!({ "ok": false, "error": "`taskId` must be provided" });
      }

      let run_id = generate_run_id();
      let mode = "container";
      emit_lifecycle(&app, task_id, &run_id, mode, "stopping", None);

      let container_name = format!("emdash_ws_{}", task_id);
      let _ = Command::new("docker")
        .args(["compose", "-p", &container_name, "down", "-v"])
        .output();
      let _ = Command::new("docker").args(["rm", "-f", &container_name]).output();

      emit_lifecycle(&app, task_id, &run_id, mode, "stopped", None);
      json!({ "ok": true })
    },
  )
  .await
}

#[tauri::command]
pub async fn container_inspect_run(args: ContainerInspectArgs) -> Value {
  run_blocking(
    json!({ "ok": false, "error": "Task cancelled" }),
    move || {
      let task_id = args.task_id.trim();
      if task_id.is_empty() {
        return json!({ "ok": false, "error": "`taskId` must be provided" });
      }
      let project = format!("emdash_ws_{}", task_id);
      let output = Command::new("docker")
        .args(["compose", "-p", &project, "ps", "--format", "json"])
        .output();
      let output = match output {
        Ok(out) => out,
        Err(err) => return json!({ "ok": false, "error": err.to_string() }),
      };
      if !output.status.success() {
        return json!({ "ok": false, "error": String::from_utf8_lossy(&output.stderr).to_string() });
      }
      let stdout = String::from_utf8_lossy(&output.stdout);
      let ports = parse_compose_ps(&stdout, &[]);
      let running = stdout.to_lowercase().contains("running");
      let preview_service = choose_preview_service_from_published(&ports);
      json!({
        "ok": true,
        "running": running,
        "ports": ports,
        "previewService": preview_service,
      })
    },
  )
  .await
}

fn to_slug(name: &str) -> String {
  let mut out = String::new();
  for ch in name.trim().to_lowercase().chars() {
    if ch.is_ascii_alphanumeric() {
      out.push(ch);
    } else {
      out.push('-');
    }
  }
  let mut cleaned = String::new();
  let mut prev_dash = false;
  for ch in out.chars() {
    if ch == '-' {
      if !prev_dash {
        cleaned.push(ch);
        prev_dash = true;
      }
    } else {
      cleaned.push(ch);
      prev_dash = false;
    }
  }
  cleaned.trim_matches('-').to_string()
}

fn buffer_to_data_url(bytes: &[u8], content_type: &str) -> String {
  let mime = if content_type.to_lowercase().starts_with("image/") {
    content_type.to_string()
  } else {
    "image/x-icon".to_string()
  };
  format!("data:{};base64,{}", mime, STANDARD.encode(bytes))
}

fn read_file_as_data_url(path: &Path) -> Option<String> {
  let data = fs::read(path).ok()?;
  let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase();
  let mime = match ext.as_str() {
    "svg" => "image/svg+xml",
    "png" => "image/png",
    "jpg" | "jpeg" => "image/jpeg",
    "ico" => "image/x-icon",
    _ => "application/octet-stream",
  };
  Some(buffer_to_data_url(&data, mime))
}

fn get_known_domain(service: &str) -> Option<&'static str> {
  match service.to_lowercase().as_str() {
    "postgres" | "postgresql" => Some("postgresql.org"),
    "redis" => Some("redis.io"),
    "minio" => Some("min.io"),
    "clickhouse" => Some("clickhouse.com"),
    "nginx" => Some("nginx.org"),
    "mysql" => Some("mysql.com"),
    "mariadb" => Some("mariadb.org"),
    "mongo" | "mongodb" => Some("mongodb.com"),
    "rabbitmq" => Some("rabbitmq.com"),
    "kafka" | "zookeeper" => Some("apache.org"),
    _ => None,
  }
}

fn allowlisted(domain: &str) -> bool {
  matches!(
    domain,
    "postgresql.org" | "redis.io" | "min.io" | "clickhouse.com" | "nginx.org" | "mysql.com" | "mariadb.org" | "mongodb.com" | "rabbitmq.com" | "apache.org"
  )
}

fn fetch_https(url: &str, max_bytes: usize) -> Option<(Vec<u8>, String)> {
  let resp = ureq::get(url).call().ok()?;
  if resp.status() >= 300 && resp.status() < 400 {
    if let Some(loc) = resp.header("Location") {
      if loc.starts_with("https://") {
        return fetch_https(loc, max_bytes);
      }
    }
    return None;
  }
  let ct = resp.header("Content-Type").unwrap_or("").to_string();
  if !ct.to_lowercase().starts_with("image/") {
    return None;
  }
  let mut reader = resp.into_reader();
  let mut buf = Vec::new();
  let _ = reader.read_to_end(&mut buf);
  if buf.len() > max_bytes {
    return None;
  }
  Some((buf, ct))
}

#[tauri::command]
pub async fn icons_resolve_service(app: AppHandle, args: ResolveIconArgs) -> Value {
  run_blocking(
    json!({ "ok": false }),
    move || {
      let service = args.service.trim();
      if service.is_empty() {
        return json!({ "ok": false });
      }
      let slug = to_slug(service);

      if let Some(task_path) = args.task_path.as_deref() {
        let base = Path::new(task_path)
          .join(".emdash")
          .join("service-icons");
        let exts = ["svg", "png", "jpg", "jpeg", "ico"];
        for ext in exts {
          let candidate = base.join(format!("{}.{}", slug, ext));
          if candidate.exists() {
            if let Some(data_url) = read_file_as_data_url(&candidate) {
              return json!({ "ok": true, "dataUrl": data_url });
            }
          }
        }
      }

      let cache_dir = app
        .path()
        .app_data_dir()
        .ok()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("icons");
      let _ = fs::create_dir_all(&cache_dir);
      let cache_file = cache_dir.join(format!("{}.ico", slug));
      if cache_file.exists() {
        if let Some(data_url) = read_file_as_data_url(&cache_file) {
          return json!({ "ok": true, "dataUrl": data_url });
        }
      }

      if args.allow_network.unwrap_or(false) {
        if let Some(domain) = get_known_domain(service) {
          if allowlisted(domain) {
            let ddg_url = format!("https://icons.duckduckgo.com/ip3/{}.ico", domain);
            let direct_url = format!("https://{}/favicon.ico", domain);
            let fetched =
              fetch_https(&ddg_url, 200_000).or_else(|| fetch_https(&direct_url, 200_000));
            if let Some((bytes, ct)) = fetched {
              let _ = fs::write(&cache_file, &bytes);
              let data_url = buffer_to_data_url(&bytes, &ct);
              return json!({ "ok": true, "dataUrl": data_url });
            }
          }
        }
      }

      json!({ "ok": false })
    },
  )
  .await
}
