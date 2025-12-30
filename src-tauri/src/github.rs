use serde::Deserialize;
use serde_json::{json, Value};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::{
  atomic::{AtomicBool, Ordering},
  Arc, Mutex,
};
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter, Manager};

use crate::settings;

const CLIENT_ID: &str = "Ov23ligC35uHWopzCeWf";
const SCOPES: &str = "repo read:user read:org";

#[derive(Default)]
pub struct GitHubState {
  cancel_flag: Arc<Mutex<Option<Arc<AtomicBool>>>>,
}

impl GitHubState {
  pub fn new() -> Self {
    Self {
      cancel_flag: Arc::new(Mutex::new(None)),
    }
  }

  fn set_cancel_flag(&self, flag: Arc<AtomicBool>) {
    if let Ok(mut guard) = self.cancel_flag.lock() {
      *guard = Some(flag);
    }
  }

  fn cancel_current(&self) {
    if let Ok(guard) = self.cancel_flag.lock() {
      if let Some(flag) = guard.as_ref() {
        flag.store(true, Ordering::SeqCst);
      }
    }
  }

  fn cancel_store(&self) -> Arc<Mutex<Option<Arc<AtomicBool>>>> {
    self.cancel_flag.clone()
  }
}

#[derive(Debug, Deserialize)]
struct DeviceCodeResponse {
  device_code: Option<String>,
  user_code: Option<String>,
  verification_uri: Option<String>,
  expires_in: Option<u64>,
  interval: Option<u64>,
  error: Option<String>,
  error_description: Option<String>,
}

#[derive(Debug, Deserialize)]
struct TokenResponse {
  access_token: Option<String>,
  error: Option<String>,
  error_description: Option<String>,
}

fn emit(app: &AppHandle, event: &str, payload: Value) {
  let _ = app.emit(event, payload);
}

fn run_command(command: &str, args: &[&str], cwd: Option<&Path>) -> Result<String, String> {
  let mut cmd = Command::new(command);
  cmd.args(args);
  if let Some(dir) = cwd {
    cmd.current_dir(dir);
  }
  let output = cmd.output().map_err(|err| err.to_string())?;
  if output.status.success() {
    Ok(String::from_utf8_lossy(&output.stdout).to_string())
  } else {
    Err(String::from_utf8_lossy(&output.stderr).to_string())
  }
}

fn gh_installed() -> bool {
  Command::new("gh")
    .arg("--version")
    .stdout(Stdio::null())
    .stderr(Stdio::null())
    .status()
    .map(|status| status.success())
    .unwrap_or(false)
}

fn gh_auth_status() -> bool {
  Command::new("gh")
    .args(["auth", "status"])
    .stdout(Stdio::null())
    .stderr(Stdio::null())
    .status()
    .map(|status| status.success())
    .unwrap_or(false)
}

fn gh_api_user() -> Result<Value, String> {
  let stdout = run_command("gh", &["api", "user"], None)?;
  serde_json::from_str(&stdout).map_err(|err| err.to_string())
}

fn gh_auth_login(token: &str) -> Result<(), String> {
  let mut cmd = Command::new("gh");
  cmd.args(["auth", "login", "--with-token"]);
  cmd.stdin(Stdio::piped());
  let mut child = cmd.spawn().map_err(|err| err.to_string())?;
  if let Some(mut stdin) = child.stdin.take() {
    use std::io::Write;
    stdin
      .write_all(token.as_bytes())
      .map_err(|err| err.to_string())?;
  }
  let status = child.wait().map_err(|err| err.to_string())?;
  if status.success() {
    Ok(())
  } else {
    Err("Failed to authenticate gh CLI".to_string())
  }
}

fn has_github_remote(project_path: &Path) -> bool {
  run_command("git", &["remote", "-v"], Some(project_path))
    .map(|stdout| stdout.contains("github.com"))
    .unwrap_or(false)
}

fn validate_repo_name(name: &str) -> Result<(), String> {
  let trimmed = name.trim();
  if trimmed.is_empty() {
    return Err("Repository name is required".to_string());
  }
  if trimmed.len() > 100 {
    return Err("Repository name must be 100 characters or less".to_string());
  }
  if !trimmed
    .chars()
    .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == '.')
  {
    return Err(
      "Repository name can only contain letters, numbers, hyphens, underscores, and dots"
        .to_string(),
    );
  }
  if trimmed.starts_with(['-', '.', '_']) || trimmed.ends_with(['-', '.', '_']) {
    return Err(
      "Repository name cannot start or end with a hyphen, dot, or underscore".to_string(),
    );
  }
  if trimmed.chars().all(|c| c == '.') {
    return Err("Repository name cannot be all dots".to_string());
  }
  let reserved = [
    "con", "prn", "aux", "nul", "com1", "com2", "com3", "com4", "com5", "com6", "com7", "com8",
    "com9", "lpt1", "lpt2", "lpt3", "lpt4", "lpt5", "lpt6", "lpt7", "lpt8", "lpt9",
  ];
  if reserved.contains(&trimmed.to_lowercase().as_str()) {
    return Err("Repository name is reserved".to_string());
  }
  Ok(())
}

fn request_device_code() -> Result<DeviceCodeResponse, String> {
  let body = format!(
    "client_id={}&scope={}",
    CLIENT_ID,
    urlencoding::encode(SCOPES)
  );
  let response = ureq::post("https://github.com/login/device/code")
    .set("Accept", "application/json")
    .set("Content-Type", "application/x-www-form-urlencoded")
    .send_string(&body)
    .map_err(|err| err.to_string())?;
  response
    .into_json::<DeviceCodeResponse>()
    .map_err(|err| err.to_string())
}

fn poll_device_token(device_code: &str) -> Result<TokenResponse, String> {
  let body = format!(
    "client_id={}&device_code={}&grant_type=urn:ietf:params:oauth:grant-type:device_code",
    CLIENT_ID,
    urlencoding::encode(device_code)
  );
  let response = ureq::post("https://github.com/login/oauth/access_token")
    .set("Accept", "application/json")
    .set("Content-Type", "application/x-www-form-urlencoded")
    .send_string(&body)
    .map_err(|err| err.to_string())?;
  response
    .into_json::<TokenResponse>()
    .map_err(|err| err.to_string())
}

fn expand_tilde(path: &str, app: &AppHandle) -> PathBuf {
  if let Some(stripped) = path.strip_prefix("~/") {
    if let Ok(home) = app.path().home_dir() {
      return home.join(stripped);
    }
  }
  PathBuf::from(path)
}

#[tauri::command]
pub fn github_check_cli_installed() -> bool {
  gh_installed()
}

#[tauri::command]
pub fn github_install_cli() -> Value {
  if cfg!(target_os = "macos") {
    if !run_command("which", &["brew"], None).is_ok() {
      return json!({
        "success": false,
        "error": "Homebrew not found. Please install from https://brew.sh/ first."
      });
    }

    return match run_command("brew", &["install", "gh"], None) {
      Ok(_) => json!({ "success": true }),
      Err(err) => json!({ "success": false, "error": err }),
    };
  }

  if cfg!(target_os = "linux") {
    return match run_command(
      "sh",
      &["-c", "sudo apt update && sudo apt install -y gh"],
      None,
    ) {
      Ok(_) => json!({ "success": true }),
      Err(_) => json!({
        "success": false,
        "error": "Could not install gh CLI. Please install manually: https://cli.github.com/"
      }),
    };
  }

  if cfg!(target_os = "windows") {
    return match run_command("winget", &["install", "GitHub.cli"], None) {
      Ok(_) => json!({ "success": true }),
      Err(_) => json!({
        "success": false,
        "error": "Could not install gh CLI. Please install manually: https://cli.github.com/"
      }),
    };
  }

  json!({
    "success": false,
    "error": format!("Unsupported platform: {}", std::env::consts::OS)
  })
}

#[tauri::command]
pub fn github_auth(app: AppHandle, state: tauri::State<GitHubState>) -> Value {
  // Cancel any existing auth flow
  state.cancel_current();

  let device = match request_device_code() {
    Ok(resp) => resp,
    Err(err) => return json!({ "success": false, "error": err }),
  };

  let device_code = match device.device_code.clone() {
    Some(code) => code,
    None => {
      return json!({
        "success": false,
        "error": device.error_description.or(device.error).unwrap_or_else(|| "Failed to request device code".to_string())
      })
    }
  };
  let user_code = device.user_code.clone().unwrap_or_default();
  let verification_uri = device.verification_uri.clone().unwrap_or_default();
  let expires_in = device.expires_in.unwrap_or(900);
  let interval = device.interval.unwrap_or(5);

  let cancel_flag = Arc::new(AtomicBool::new(false));
  state.set_cancel_flag(cancel_flag.clone());
  let cancel_store = state.cancel_store();
  let app_handle = app.clone();
  let device_code_for_poll = device_code.clone();

  std::thread::spawn(move || {
    std::thread::sleep(Duration::from_millis(100));
    emit(
      &app_handle,
      "github:auth:device-code",
      json!({
        "userCode": user_code,
        "verificationUri": verification_uri,
        "expiresIn": expires_in,
        "interval": interval
      }),
    );

    let start = Instant::now();
    let mut current_interval = interval;

    loop {
      if cancel_flag.load(Ordering::SeqCst) {
        emit(&app_handle, "github:auth:cancelled", json!({}));
        break;
      }

      if start.elapsed() >= Duration::from_secs(expires_in) {
        emit(
          &app_handle,
          "github:auth:error",
          json!({
            "error": "expired_token",
            "message": "Authorization code expired. Please try again."
          }),
        );
        break;
      }

      std::thread::sleep(Duration::from_secs(current_interval));

      let token = match poll_device_token(&device_code_for_poll) {
        Ok(resp) => resp,
        Err(err) => {
          emit(
            &app_handle,
            "github:auth:error",
            json!({
              "error": "network_error",
              "message": err
            }),
          );
          break;
        }
      };

      if let Some(access_token) = token.access_token.clone() {
        let _ = gh_auth_login(&access_token);
        let user = gh_api_user().ok();
        emit(
          &app_handle,
          "github:auth:success",
          json!({
            "token": access_token,
            "user": user
          }),
        );
        emit(
          &app_handle,
          "github:auth:user-updated",
          json!({
            "user": user
          }),
        );
        break;
      }

      if let Some(error) = token.error.clone() {
        match error.as_str() {
          "authorization_pending" => {
            emit(&app_handle, "github:auth:polling", json!({ "status": "waiting" }));
          }
          "slow_down" => {
            current_interval += 5;
            emit(
              &app_handle,
              "github:auth:slow-down",
              json!({ "newInterval": current_interval }),
            );
          }
          "access_denied" => {
            emit(
              &app_handle,
              "github:auth:error",
              json!({
                "error": "access_denied",
                "message": "Authorization was cancelled."
              }),
            );
            break;
          }
          "expired_token" => {
            emit(
              &app_handle,
              "github:auth:error",
              json!({
                "error": "expired_token",
                "message": "Authorization code expired. Please try again."
              }),
            );
            break;
          }
          _ => {
            emit(
              &app_handle,
              "github:auth:error",
              json!({
                "error": error,
                "message": token
                  .error_description
                  .unwrap_or_else(|| "Authentication failed".to_string())
              }),
            );
            break;
          }
        }
      }
    }

    if let Ok(mut guard) = cancel_store.lock() {
      if let Some(current) = guard.as_ref() {
        if Arc::ptr_eq(current, &cancel_flag) {
          *guard = None;
        }
      }
    }
  });

  json!({
    "success": true,
    "device_code": device_code,
    "user_code": device.user_code,
    "verification_uri": device.verification_uri,
    "expires_in": expires_in,
    "interval": interval
  })
}

#[tauri::command]
pub fn github_cancel_auth(app: AppHandle, state: tauri::State<GitHubState>) -> Value {
  state.cancel_current();
  emit(&app, "github:auth:cancelled", json!({}));
  json!({ "success": true })
}

#[tauri::command]
pub fn github_get_status() -> Value {
  if !gh_installed() {
    return json!({ "installed": false, "authenticated": false });
  }

  match gh_api_user() {
    Ok(user) => json!({ "installed": true, "authenticated": true, "user": user }),
    Err(_) => json!({ "installed": true, "authenticated": false, "user": Value::Null }),
  }
}

#[tauri::command]
pub fn github_is_authenticated() -> bool {
  gh_auth_status()
}

#[tauri::command]
pub fn github_get_user() -> Value {
  match gh_api_user() {
    Ok(user) => user,
    Err(_) => Value::Null,
  }
}

#[tauri::command]
pub fn github_get_repositories() -> Value {
  let stdout = match run_command(
    "gh",
    &[
      "repo",
      "list",
      "--limit",
      "100",
      "--json",
      "name,nameWithOwner,description,url,defaultBranchRef,isPrivate,updatedAt,primaryLanguage,stargazerCount,forkCount",
    ],
    None,
  ) {
    Ok(out) => out,
    Err(_) => return json!([]),
  };

  let parsed: Value = serde_json::from_str(&stdout).unwrap_or_else(|_| json!([]));
  let list = parsed.as_array().cloned().unwrap_or_default();
  let mapped: Vec<Value> = list
    .into_iter()
    .enumerate()
    .map(|(idx, repo)| {
      let name_with_owner = repo
        .get("nameWithOwner")
        .and_then(|v| v.as_str())
        .unwrap_or("");
      json!({
        "id": idx as u64,
        "name": repo.get("name").and_then(|v| v.as_str()).unwrap_or(""),
        "full_name": name_with_owner,
        "description": repo.get("description").and_then(|v| v.as_str()).unwrap_or(""),
        "html_url": repo.get("url").and_then(|v| v.as_str()).unwrap_or(""),
        "clone_url": format!("https://github.com/{}.git", name_with_owner),
        "ssh_url": format!("git@github.com:{}.git", name_with_owner),
        "default_branch": repo
          .get("defaultBranchRef")
          .and_then(|v| v.get("name"))
          .and_then(|v| v.as_str())
          .unwrap_or("main"),
        "private": repo.get("isPrivate").and_then(|v| v.as_bool()).unwrap_or(false),
        "updated_at": repo.get("updatedAt").and_then(|v| v.as_str()),
        "language": repo
          .get("primaryLanguage")
          .and_then(|v| v.get("name"))
          .and_then(|v| v.as_str()),
        "stargazers_count": repo.get("stargazerCount").and_then(|v| v.as_i64()).unwrap_or(0),
        "forks_count": repo.get("forkCount").and_then(|v| v.as_i64()).unwrap_or(0)
      })
    })
    .collect();

  Value::Array(mapped)
}

#[tauri::command]
pub fn github_connect(project_path: String) -> Value {
  if !gh_auth_status() {
    return json!({ "success": false, "error": "GitHub CLI not authenticated" });
  }

  let stdout = match run_command(
    "gh",
    &["repo", "view", "--json", "name,nameWithOwner,defaultBranchRef"],
    Some(Path::new(&project_path)),
  ) {
    Ok(out) => out,
    Err(_) => {
      return json!({
        "success": false,
        "error": "Repository not found on GitHub or not connected to GitHub CLI"
      })
    }
  };

  let parsed: Value = serde_json::from_str(&stdout).unwrap_or_else(|_| json!({}));
  let repo = parsed
    .get("nameWithOwner")
    .and_then(|v| v.as_str())
    .unwrap_or("");
  let branch = parsed
    .get("defaultBranchRef")
    .and_then(|v| v.get("name"))
    .and_then(|v| v.as_str())
    .unwrap_or("main");

  json!({ "success": true, "repository": repo, "branch": branch })
}

#[tauri::command]
pub fn github_clone_repository(repo_url: String, local_path: String) -> Value {
  let local = PathBuf::from(local_path);
  if let Some(parent) = local.parent() {
    let _ = fs::create_dir_all(parent);
  }

  match run_command("git", &["clone", &repo_url, local.to_str().unwrap_or("")], None) {
    Ok(_) => json!({ "success": true }),
    Err(err) => json!({ "success": false, "error": err }),
  }
}

#[tauri::command]
pub fn github_issues_list(project_path: String, limit: Option<u64>) -> Value {
  let safe_limit = limit.unwrap_or(50).clamp(1, 200);
  let path = Path::new(&project_path);
  if !has_github_remote(path) {
    return json!({ "success": true, "issues": [] });
  }

  let stdout = match run_command(
    "gh",
    &[
      "issue",
      "list",
      "--state",
      "open",
      "--limit",
      &safe_limit.to_string(),
      "--json",
      "number,title,url,state,updatedAt,assignees,labels",
    ],
    Some(path),
  ) {
    Ok(out) => out,
    Err(err) => return json!({ "success": false, "error": err }),
  };

  let issues: Value = serde_json::from_str(&stdout).unwrap_or_else(|_| json!([]));
  json!({ "success": true, "issues": issues })
}

#[tauri::command]
pub fn github_issues_search(project_path: String, search_term: String, limit: Option<u64>) -> Value {
  let term = search_term.trim().to_string();
  if term.is_empty() {
    return json!({ "success": true, "issues": [] });
  }
  let safe_limit = limit.unwrap_or(20).clamp(1, 200);
  let path = Path::new(&project_path);
  if !has_github_remote(path) {
    return json!({ "success": true, "issues": [] });
  }

  let stdout = match run_command(
    "gh",
    &[
      "issue",
      "list",
      "--state",
      "open",
      "--search",
      &term,
      "--limit",
      &safe_limit.to_string(),
      "--json",
      "number,title,url,state,updatedAt,assignees,labels",
    ],
    Some(path),
  ) {
    Ok(out) => out,
    Err(err) => return json!({ "success": false, "error": err }),
  };

  let issues: Value = serde_json::from_str(&stdout).unwrap_or_else(|_| json!([]));
  json!({ "success": true, "issues": issues })
}

#[tauri::command]
pub fn github_issue_get(project_path: String, number: u64) -> Value {
  if number == 0 {
    return json!({ "success": false, "error": "Issue number is required" });
  }
  let path = Path::new(&project_path);
  let stdout = match run_command(
    "gh",
    &[
      "issue",
      "view",
      &number.to_string(),
      "--json",
      "number,title,body,url,state,updatedAt,assignees,labels",
    ],
    Some(path),
  ) {
    Ok(out) => out,
    Err(err) => return json!({ "success": false, "error": err }),
  };
  let issue: Value = serde_json::from_str(&stdout).unwrap_or(Value::Null);
  json!({ "success": !issue.is_null(), "issue": issue })
}

#[tauri::command]
pub fn github_list_pull_requests(project_path: String) -> Value {
  let path = Path::new(&project_path);
  let stdout = match run_command(
    "gh",
    &[
      "pr",
      "list",
      "--state",
      "open",
      "--json",
      "number,title,headRefName,baseRefName,url,isDraft,updatedAt,headRefOid,author,headRepositoryOwner,headRepository",
    ],
    Some(path),
  ) {
    Ok(out) => out,
    Err(err) => return json!({ "success": false, "error": err }),
  };
  let prs: Value = serde_json::from_str(&stdout).unwrap_or_else(|_| json!([]));
  json!({ "success": true, "prs": prs })
}

#[tauri::command]
pub fn github_logout() -> Value {
  let _ = run_command("gh", &["auth", "logout", "--hostname", "github.com", "--yes"], None);
  json!({ "success": true })
}

#[tauri::command]
pub fn github_get_owners() -> Value {
  let user = match gh_api_user() {
    Ok(user) => user,
    Err(err) => return json!({ "success": false, "error": err }),
  };
  let mut owners = vec![json!({
    "login": user.get("login").and_then(|v| v.as_str()).unwrap_or(""),
    "type": "User"
  })];

  if let Ok(stdout) = run_command("gh", &["api", "user/orgs"], None) {
    if let Ok(orgs) = serde_json::from_str::<Value>(&stdout) {
      if let Some(list) = orgs.as_array() {
        for org in list {
          if let Some(login) = org.get("login").and_then(|v| v.as_str()) {
            owners.push(json!({ "login": login, "type": "Organization" }));
          }
        }
      }
    }
  }

  json!({ "success": true, "owners": owners })
}

#[tauri::command]
pub fn github_validate_repo_name(name: String, owner: String) -> Value {
  if let Err(err) = validate_repo_name(&name) {
    return json!({ "success": true, "valid": false, "exists": false, "error": err });
  }

  let repo_id = format!("{}/{}", owner.trim(), name.trim());
  let exists = run_command("gh", &["repo", "view", &repo_id], None).is_ok();
  if exists {
    return json!({
      "success": true,
      "valid": true,
      "exists": true,
      "error": format!("Repository {repo_id} already exists")
    });
  }

  json!({ "success": true, "valid": true, "exists": false })
}

#[tauri::command]
pub fn github_create_new_project(
  app: AppHandle,
  name: String,
  description: Option<String>,
  owner: String,
  is_private: bool,
) -> Value {
  if let Err(err) = validate_repo_name(&name) {
    return json!({ "success": false, "error": err });
  }

  let repo_id = format!("{}/{}", owner.trim(), name.trim());
  if run_command("gh", &["repo", "view", &repo_id], None).is_ok() {
    return json!({
      "success": false,
      "error": format!("Repository {repo_id} already exists")
    });
  }

  let settings = settings::load_settings(&app);
  let project_dir = settings
    .get("projects")
    .and_then(|v| v.get("defaultDirectory"))
    .and_then(|v| v.as_str())
    .unwrap_or("~/emdash-projects");
  let project_root = expand_tilde(project_dir, &app);
  if let Err(err) = fs::create_dir_all(&project_root) {
    return json!({ "success": false, "error": err.to_string() });
  }

  let visibility = if is_private { "--private" } else { "--public" };
  let mut args = vec![
    "repo".to_string(),
    "create".to_string(),
    repo_id.clone(),
    visibility.to_string(),
    "--confirm".to_string(),
    "--clone".to_string(),
    "--add-readme".to_string(),
  ];
  if let Some(desc) = description.as_ref().and_then(|d| {
    let trimmed = d.trim();
    if trimmed.is_empty() {
      None
    } else {
      Some(trimmed.to_string())
    }
  }) {
    args.push("--description".to_string());
    args.push(desc);
  }

  let arg_refs: Vec<&str> = args.iter().map(String::as_str).collect();
  if let Err(err) = run_command("gh", &arg_refs, Some(&project_root)) {
    return json!({ "success": false, "error": err });
  }

  let local_path = project_root.join(&name);
  let stdout = run_command(
    "gh",
    &[
      "repo",
      "view",
      &repo_id,
      "--json",
      "name,nameWithOwner,url,defaultBranchRef",
    ],
    None,
  )
  .unwrap_or_default();
  let info: Value = serde_json::from_str(&stdout).unwrap_or_else(|_| json!({}));

  json!({
    "success": true,
    "projectPath": local_path.to_string_lossy(),
    "repoUrl": info.get("url").and_then(|v| v.as_str()).unwrap_or("").to_string(),
    "fullName": info.get("nameWithOwner").and_then(|v| v.as_str()).unwrap_or("").to_string(),
    "defaultBranch": info
      .get("defaultBranchRef")
      .and_then(|v| v.get("name"))
      .and_then(|v| v.as_str())
      .unwrap_or("main")
  })
}
