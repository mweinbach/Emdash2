use crate::db::{self, DbState, ProjectSettingsRow};
use crate::runtime::run_blocking;
use crate::settings;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha1::{Digest, Sha1};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Manager, State};

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorktreeInfo {
  pub id: String,
  pub name: String,
  pub branch: String,
  pub path: String,
  pub project_id: String,
  pub status: String,
  pub created_at: String,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub last_activity: Option<String>,
}

#[derive(Default, Clone)]
pub struct WorktreeState {
  inner: Arc<Mutex<HashMap<String, WorktreeInfo>>>,
}

impl WorktreeState {
  pub fn new() -> Self {
    Self {
      inner: Arc::new(Mutex::new(HashMap::new())),
    }
  }
}

#[derive(Debug, Clone)]
struct BaseRefInfo {
  remote: String,
  branch: String,
  full_ref: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorktreeCreateArgs {
  project_path: String,
  task_name: String,
  project_id: String,
  auto_approve: Option<bool>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorktreeListArgs {
  project_path: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorktreeRemoveArgs {
  project_path: String,
  worktree_id: String,
  worktree_path: Option<String>,
  branch: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorktreeStatusArgs {
  worktree_path: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorktreeMergeArgs {
  project_path: String,
  worktree_id: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorktreeGetArgs {
  worktree_id: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorktreeCreateFromBranchArgs {
  pub project_path: String,
  pub task_name: String,
  pub branch_name: String,
  pub project_id: String,
  pub worktree_path: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FetchBaseRefArgs {
  project_id: String,
  project_path: String,
}

fn run_command(cmd: &str, args: &[&str], cwd: Option<&Path>) -> Result<Output, String> {
  let mut command = Command::new(cmd);
  command.args(args);
  if let Some(dir) = cwd {
    command.current_dir(dir);
  }
  command
    .output()
    .map_err(|err| err.to_string())
    .and_then(|output| {
      if output.status.success() {
        Ok(output)
      } else {
        Err(format_output_error(&output))
      }
    })
}

fn run_command_vec(cmd: &str, args: &[String], cwd: Option<&Path>) -> Result<Output, String> {
  let mut command = Command::new(cmd);
  command.args(args);
  if let Some(dir) = cwd {
    command.current_dir(dir);
  }
  command
    .output()
    .map_err(|err| err.to_string())
    .and_then(|output| {
      if output.status.success() {
        Ok(output)
      } else {
        Err(format_output_error(&output))
      }
    })
}

fn format_output_error(output: &Output) -> String {
  let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
  if !stderr.is_empty() {
    return stderr;
  }
  let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
  if !stdout.is_empty() {
    return stdout;
  }
  "Command failed".to_string()
}

fn slugify(name: &str) -> String {
  let mut out = String::new();
  for ch in name.to_lowercase().chars() {
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

fn stable_id_from_path(path: &str) -> String {
  let abs = Path::new(path).canonicalize().unwrap_or_else(|_| PathBuf::from(path));
  let mut hasher = Sha1::new();
  hasher.update(abs.to_string_lossy().as_bytes());
  let digest = hasher.finalize();
  let hex = hex::encode(digest);
  let short = &hex[..12.min(hex.len())];
  format!("wt-{}", short)
}

fn sanitize_branch_name(name: &str) -> String {
  let mut n = name
    .replace(|c: char| c.is_whitespace(), "-")
    .replace(|c: char| !c.is_ascii_alphanumeric() && c != '.' && c != '_' && c != '/' && c != '-', "-");
  while n.contains("//") {
    n = n.replace("//", "/");
  }
  while n.contains("--") {
    n = n.replace("--", "-");
  }
  let trimmed = n.trim_matches(['.', '/', '-']).to_string();
  if trimmed.is_empty() || trimmed == "HEAD" {
    format!("agent/{}", slugify("task"))
  } else {
    trimmed
  }
}

fn render_branch_template(template: &str, slug: &str, timestamp: &str) -> String {
  let replaced = template
    .replace("{slug}", slug)
    .replace("{timestamp}", timestamp);
  sanitize_branch_name(&replaced)
}

fn extract_template_prefix(template: &str) -> Option<String> {
  let idx = template.find('{');
  let head = match idx {
    Some(i) => template[..i].trim(),
    None => template.trim(),
  };
  if head.is_empty() {
    return None;
  }
  let cleaned = head.replace(' ', "");
  if cleaned.is_empty() {
    return None;
  }
  let seg = cleaned
    .split('/')
    .next()
    .unwrap_or("")
    .trim_matches(['.', '/', '-']);
  if seg.is_empty() {
    None
  } else {
    Some(seg.to_string())
  }
}

fn get_default_branch(project_path: &Path) -> String {
  if let Ok(output) = run_command("git", &["remote", "show", "origin"], Some(project_path)) {
    let stdout = String::from_utf8_lossy(&output.stdout);
    for line in stdout.lines() {
      if let Some(idx) = line.find("HEAD branch:") {
        let branch = line[idx + "HEAD branch:".len()..].trim();
        if !branch.is_empty() {
          return branch.to_string();
        }
      }
    }
  }
  "main".to_string()
}

fn parse_base_ref(raw: &str, project_path: Option<&Path>) -> Option<BaseRefInfo> {
  let cleaned = raw
    .trim()
    .trim_start_matches("refs/remotes/")
    .trim_start_matches("remotes/")
    .trim();
  if cleaned.is_empty() || !cleaned.contains('/') {
    return None;
  }
  let mut parts = cleaned.split('/');
  let remote = parts.next().unwrap_or("").trim();
  let branch = parts.collect::<Vec<_>>().join("/");
  if remote.is_empty() || branch.trim().is_empty() {
    return None;
  }

  if let Some(path) = project_path {
    if let Ok(output) = run_command("git", &["remote"], Some(path)) {
      let stdout = String::from_utf8_lossy(&output.stdout);
      let remotes = stdout.lines().map(|l| l.trim()).collect::<Vec<_>>();
      if !remotes.iter().any(|r| *r == remote) {
        return None;
      }
    }
  }

  Some(BaseRefInfo {
    remote: remote.to_string(),
    branch: branch.trim().to_string(),
    full_ref: format!("{}/{}", remote, branch.trim()),
  })
}

fn resolve_project_base_ref(
  project_path: &Path,
  row: &ProjectSettingsRow,
) -> Result<BaseRefInfo, String> {
  let default_remote = row
    .git_remote
    .as_deref()
    .map(|r| r.trim())
    .filter(|r| !r.is_empty())
    .unwrap_or("origin");
  if let Some(base_ref) = row.base_ref.as_deref() {
    if let Some(info) = parse_base_ref(base_ref, Some(project_path)) {
      return Ok(info);
    }

    // Check if base_ref is a local branch name
    if !base_ref.trim().is_empty() {
      if run_command(
        "git",
        &[
          "rev-parse",
          "--verify",
          &format!("refs/heads/{}", base_ref.trim()),
        ],
        Some(project_path),
      )
      .is_ok()
      {
        return Ok(BaseRefInfo {
          remote: default_remote.to_string(),
          branch: base_ref.trim().to_string(),
          full_ref: format!("{}/{}", default_remote, base_ref.trim()),
        });
      }
    }
  }

  let fallback_branch = row
    .git_branch
    .as_deref()
    .map(|b| b.trim())
    .filter(|b| !b.is_empty() && !b.contains(' '))
    .map(|b| b.to_string())
    .unwrap_or_else(|| get_default_branch(project_path));

  Ok(BaseRefInfo {
    remote: default_remote.to_string(),
    branch: fallback_branch.clone(),
    full_ref: format!("{}/{}", default_remote, fallback_branch),
  })
}

fn is_missing_remote_ref_error(message: &str) -> bool {
  let msg = message.to_lowercase();
  msg.contains("couldn't find remote ref")
    || msg.contains("could not find remote ref")
    || msg.contains("remote ref does not exist")
    || msg.contains("fatal: the remote end hung up unexpectedly")
    || msg.contains("no such ref was fetched")
}

fn fetch_base_ref_with_fallback(
  project_path: &Path,
  project_id: &str,
  base_ref: &BaseRefInfo,
  db_state: &DbState,
) -> Result<BaseRefInfo, String> {
  let fetch_res = run_command(
    "git",
    &["fetch", &base_ref.remote, &base_ref.branch],
    Some(project_path),
  );
  if fetch_res.is_ok() {
    return Ok(base_ref.clone());
  }

  let err = fetch_res.err().unwrap_or_else(|| "Failed to fetch base ref".to_string());
  if !is_missing_remote_ref_error(&err) {
    return Err(format!("Failed to fetch {}: {}", base_ref.full_ref, err));
  }

  let fallback_branch = get_default_branch(project_path);
  let fallback = BaseRefInfo {
    remote: "origin".to_string(),
    branch: fallback_branch.clone(),
    full_ref: format!("origin/{}", fallback_branch),
  };

  if fallback.full_ref == base_ref.full_ref {
    return Err(format!("Failed to fetch {}: {}", base_ref.full_ref, err));
  }

  run_command(
    "git",
    &["fetch", &fallback.remote, &fallback.branch],
    Some(project_path),
  )
  .map_err(|err| {
    format!(
      "Failed to fetch base branch. Tried {} and {}. {} Please verify the branch exists on the remote.",
      base_ref.full_ref, fallback.full_ref, err
    )
  })?;

  let _ = db::update_project_base_ref(db_state, project_id, &fallback.full_ref);
  Ok(fallback)
}

fn ensure_codex_log_ignored(worktree_path: &Path) {
  let git_meta = worktree_path.join(".git");
  let mut git_dir = git_meta.clone();
  if git_meta.exists() && git_meta.is_file() {
    if let Ok(content) = fs::read_to_string(&git_meta) {
      for line in content.lines() {
        if let Some(rest) = line.strip_prefix("gitdir:") {
          let dir = rest.trim();
          if !dir.is_empty() {
            git_dir = worktree_path.join(dir);
          }
        }
      }
    }
  }

  let exclude_path = git_dir.join("info").join("exclude");
  if let Some(parent) = exclude_path.parent() {
    let _ = fs::create_dir_all(parent);
  }
  let mut current = String::new();
  if let Ok(text) = fs::read_to_string(&exclude_path) {
    current = text;
  }
  if !current.contains("codex-stream.log") {
    let mut next = current;
    if !next.ends_with('\n') && !next.is_empty() {
      next.push('\n');
    }
    next.push_str("codex-stream.log\n");
    let _ = fs::write(&exclude_path, next);
  }
}

fn ensure_claude_auto_approve(worktree_path: &Path) {
  let claude_dir = worktree_path.join(".claude");
  let settings_path = claude_dir.join("settings.json");
  let _ = fs::create_dir_all(&claude_dir);

  let mut settings_obj = serde_json::Map::new();
  if let Ok(raw) = fs::read_to_string(&settings_path) {
    if let Ok(Value::Object(existing)) = serde_json::from_str::<Value>(&raw) {
      settings_obj = existing;
    }
  }

  settings_obj.insert(
    "defaultMode".to_string(),
    Value::String("bypassPermissions".to_string()),
  );
  let _ = fs::write(
    &settings_path,
    serde_json::to_string_pretty(&Value::Object(settings_obj)).unwrap_or_else(|_| "{}".into())
      + "\n",
  );
}

fn should_push_on_create(app: &AppHandle) -> bool {
  let settings = settings::load_settings(app);
  settings
    .get("repository")
    .and_then(|v| v.get("pushOnCreate"))
    .and_then(|v| v.as_bool())
    .unwrap_or(true)
}

fn branch_template(app: &AppHandle) -> String {
  settings::load_settings(app)
    .get("repository")
    .and_then(|v| v.get("branchTemplate"))
    .and_then(|v| v.as_str())
    .unwrap_or("agent/{slug}-{timestamp}")
    .to_string()
}

pub fn list_worktrees_internal(
  app: &AppHandle,
  state: &WorktreeState,
  project_path: &str,
) -> Result<Vec<WorktreeInfo>, String> {
  let output = run_command("git", &["worktree", "list"], Some(Path::new(project_path)))?;
  let stdout = String::from_utf8_lossy(&output.stdout);
  let mut managed_prefixes = vec!["agent".to_string(), "pr".to_string(), "orch".to_string()];
  if let Some(prefix) = extract_template_prefix(&branch_template(app)) {
    if !managed_prefixes.contains(&prefix) {
      managed_prefixes.push(prefix);
    }
  }

  let tracked = state.inner.lock().unwrap();
  let mut worktrees: Vec<WorktreeInfo> = Vec::new();

  for line in stdout.lines() {
    if !line.contains('[') || !line.contains(']') {
      continue;
    }
    let parts: Vec<&str> = line.split_whitespace().collect();
    if parts.is_empty() {
      continue;
    }
    let worktree_path = parts[0];
    let branch = line
      .split('[')
      .nth(1)
      .and_then(|s| s.split(']').next())
      .unwrap_or("unknown")
      .to_string();

    let managed = managed_prefixes.iter().any(|pf| {
      branch.starts_with(&format!("{}/", pf))
        || branch.starts_with(&format!("{}-", pf))
        || branch.starts_with(&format!("{}", pf))
        || branch.starts_with(&format!("{}.", pf))
        || branch.starts_with(&format!("{}_", pf))
    });

    let existing = tracked.values().find(|wt| wt.path == worktree_path);
    if !managed && existing.is_none() {
      continue;
    }

    if let Some(info) = existing {
      worktrees.push(info.clone());
    } else {
      worktrees.push(WorktreeInfo {
        id: stable_id_from_path(worktree_path),
        name: Path::new(worktree_path)
          .file_name()
          .and_then(|n| n.to_str())
          .unwrap_or(worktree_path)
          .to_string(),
        branch: branch.clone(),
        path: worktree_path.to_string(),
        project_id: Path::new(project_path)
          .file_name()
          .and_then(|n| n.to_str())
          .unwrap_or(project_path)
          .to_string(),
        status: "active".to_string(),
        created_at: Utc::now().to_rfc3339(),
        last_activity: None,
      });
    }
  }

  Ok(worktrees)
}

#[tauri::command]
pub async fn worktree_create(app: AppHandle, args: WorktreeCreateArgs) -> Value {
  run_blocking(
    json!({ "success": false, "error": "Task cancelled" }),
    move || {
      let state: State<WorktreeState> = app.state();
      let db_state: State<DbState> = app.state();
      let project_path = args.project_path.trim();
      let task_name = args.task_name.trim();
      let project_id = args.project_id.trim();

      if project_path.is_empty() || task_name.is_empty() || project_id.is_empty() {
        return json!({ "success": false, "error": "Missing required parameters" });
      }

      let slugged = slugify(task_name);
      let timestamp = Utc::now().timestamp_millis().to_string();
      let template = branch_template(&app);
      let branch_name = render_branch_template(&template, &slugged, &timestamp);

      let worktree_path = Path::new(project_path)
        .join("..")
        .join("worktrees")
        .join(format!("{}-{}", slugged, timestamp));

      if worktree_path.exists() {
        return json!({
          "success": false,
          "error": format!("Worktree directory already exists: {}", worktree_path.display())
        });
      }

      if let Some(parent) = worktree_path.parent() {
        if let Err(err) = fs::create_dir_all(parent) {
          return json!({ "success": false, "error": err.to_string() });
        }
      }

      let project_path_buf = PathBuf::from(project_path);
      let row = match db::project_settings_row(&db_state, project_id) {
        Ok(row) => row,
        Err(err) => return json!({ "success": false, "error": err }),
      };

      let base_ref = match resolve_project_base_ref(&project_path_buf, &row) {
        Ok(info) => info,
        Err(err) => return json!({ "success": false, "error": err }),
      };

      let fetched =
        match fetch_base_ref_with_fallback(&project_path_buf, project_id, &base_ref, &db_state) {
          Ok(info) => info,
          Err(err) => return json!({ "success": false, "error": err }),
        };

      let args_vec = vec![
        "worktree".to_string(),
        "add".to_string(),
        "-b".to_string(),
        branch_name.clone(),
        worktree_path.to_string_lossy().to_string(),
        fetched.full_ref.clone(),
      ];

      if let Err(err) = run_command_vec("git", &args_vec, Some(&project_path_buf)) {
        return json!({ "success": false, "error": err });
      }

      if !worktree_path.exists() {
        return json!({
          "success": false,
          "error": format!("Worktree directory was not created: {}", worktree_path.display())
        });
      }

      ensure_codex_log_ignored(&worktree_path);
      if args.auto_approve.unwrap_or(false) {
        ensure_claude_auto_approve(&worktree_path);
      }

      let worktree_info = WorktreeInfo {
        id: stable_id_from_path(&worktree_path.to_string_lossy()),
        name: task_name.to_string(),
        branch: branch_name.clone(),
        path: worktree_path.to_string_lossy().to_string(),
        project_id: project_id.to_string(),
        status: "active".to_string(),
        created_at: Utc::now().to_rfc3339(),
        last_activity: None,
      };

      state
        .inner
        .lock()
        .unwrap()
        .insert(worktree_info.id.clone(), worktree_info.clone());

      if should_push_on_create(&app) {
        let _ = run_command(
          "git",
          &["push", "--set-upstream", "origin", &branch_name],
          Some(&worktree_path),
        );
      }

      json!({ "success": true, "worktree": worktree_info })
    },
  )
  .await
}

#[tauri::command]
pub async fn worktree_list(app: AppHandle, args: WorktreeListArgs) -> Value {
  run_blocking(
    json!({ "success": false, "error": "Task cancelled" }),
    move || {
      let state: State<WorktreeState> = app.state();
      let project_path = args.project_path.trim();
      if project_path.is_empty() {
        return json!({ "success": false, "error": "projectPath is required" });
      }
      match list_worktrees_internal(&app, &state, project_path) {
        Ok(worktrees) => json!({ "success": true, "worktrees": worktrees }),
        Err(err) => json!({ "success": false, "error": err }),
      }
    },
  )
  .await
}

fn worktree_remove_internal(state: &WorktreeState, args: WorktreeRemoveArgs) -> Value {
  let project_path = args.project_path.trim();
  if project_path.is_empty() {
    return json!({ "success": false, "error": "projectPath is required" });
  }

  let existing = {
    let guard = state.inner.lock().unwrap();
    guard.get(&args.worktree_id).cloned()
  };
  let path_to_remove = existing
    .as_ref()
    .map(|wt| wt.path.clone())
    .or_else(|| args.worktree_path.clone())
    .unwrap_or_default();
  let branch_to_delete = existing
    .as_ref()
    .map(|wt| wt.branch.clone())
    .or_else(|| args.branch.clone());

  if path_to_remove.trim().is_empty() {
    return json!({ "success": false, "error": "Worktree path not provided" });
  }

  let project_path_buf = PathBuf::from(project_path);
  let _ = run_command(
    "git",
    &["worktree", "remove", "--force", &path_to_remove],
    Some(&project_path_buf),
  );
  let _ = run_command("git", &["worktree", "prune", "--verbose"], Some(&project_path_buf));

  let path_buf = PathBuf::from(&path_to_remove);
  if path_buf.exists() {
    if let Err(err) = fs::remove_dir_all(&path_buf) {
      #[cfg(windows)]
      {
        let _ = Command::new("cmd")
          .args([
            "/C",
            "attrib",
            "-R",
            "/S",
            "/D",
            &format!("{}\\*", path_to_remove),
          ])
          .status();
      }
      #[cfg(not(windows))]
      {
        let _ = Command::new("chmod")
          .args(["-R", "u+w", &path_to_remove])
          .status();
      }
      if fs::remove_dir_all(&path_buf).is_err() {
        return json!({ "success": false, "error": err.to_string() });
      }
    }
  }

  if let Some(branch) = branch_to_delete {
    let delete_branch = run_command("git", &["branch", "-D", &branch], Some(&project_path_buf));
    if let Err(err) = delete_branch {
      if err.contains("checked out at") {
        let _ = run_command("git", &["worktree", "prune", "--verbose"], Some(&project_path_buf));
        let _ = run_command("git", &["branch", "-D", &branch], Some(&project_path_buf));
      }
    }

    let mut remote_branch = branch.clone();
    if let Some(stripped) = branch.strip_prefix("origin/") {
      remote_branch = stripped.to_string();
    }
    let _ = run_command(
      "git",
      &["push", "origin", "--delete", &remote_branch],
      Some(&project_path_buf),
    );
  }

  if existing.is_some() {
    let mut guard = state.inner.lock().unwrap();
    guard.remove(&args.worktree_id);
  }

  json!({ "success": true })
}

#[tauri::command]
pub async fn worktree_remove(app: AppHandle, args: WorktreeRemoveArgs) -> Value {
  run_blocking(
    json!({ "success": false, "error": "Task cancelled" }),
    move || {
      let state: State<WorktreeState> = app.state();
      worktree_remove_internal(&state, args)
    },
  )
  .await
}

#[tauri::command]
pub async fn worktree_status(args: WorktreeStatusArgs) -> Value {
  run_blocking(
    json!({ "success": false, "error": "Task cancelled" }),
    move || {
      let worktree_path = args.worktree_path.trim();
      if worktree_path.is_empty() {
        return json!({ "success": false, "error": "worktreePath is required" });
      }

      let output = match run_command(
        "git",
        &["status", "--porcelain", "--untracked-files=all"],
        Some(Path::new(worktree_path)),
      ) {
        Ok(out) => out,
        Err(err) => return json!({ "success": false, "error": err }),
      };

      let mut staged_files: Vec<String> = Vec::new();
      let mut unstaged_files: Vec<String> = Vec::new();
      let mut untracked_files: Vec<String> = Vec::new();

      let stdout = String::from_utf8_lossy(&output.stdout);
      for line in stdout.lines() {
        if line.trim().is_empty() {
          continue;
        }
        if line.starts_with("??") {
          untracked_files.push(line[3..].to_string());
          continue;
        }
        let status = &line[..2];
        let file = line[3..].to_string();
        if status.contains('A') || status.contains('M') || status.contains('D') {
          staged_files.push(file.clone());
        }
        if status.contains('M') || status.contains('D') {
          unstaged_files.push(file.clone());
        }
      }

      let has_changes =
        !staged_files.is_empty() || !unstaged_files.is_empty() || !untracked_files.is_empty();

      json!({
        "success": true,
        "status": {
          "hasChanges": has_changes,
          "stagedFiles": staged_files,
          "unstagedFiles": unstaged_files,
          "untrackedFiles": untracked_files,
        }
      })
    },
  )
  .await
}

#[tauri::command]
pub async fn worktree_merge(app: AppHandle, args: WorktreeMergeArgs) -> Value {
  run_blocking(
    json!({ "success": false, "error": "Task cancelled" }),
    move || {
      let state: State<WorktreeState> = app.state();
      let project_path = args.project_path.trim();
      if project_path.is_empty() {
        return json!({ "success": false, "error": "projectPath is required" });
      }

      let guard = state.inner.lock().unwrap();
      let worktree = match guard.get(&args.worktree_id) {
        Some(wt) => wt.clone(),
        None => return json!({ "success": false, "error": "Worktree not found" }),
      };
      drop(guard);

      let project_path_buf = PathBuf::from(project_path);
      let default_branch = get_default_branch(&project_path_buf);
      if let Err(err) = run_command("git", &["checkout", &default_branch], Some(&project_path_buf)) {
        return json!({ "success": false, "error": err });
      }
      if let Err(err) = run_command("git", &["merge", &worktree.branch], Some(&project_path_buf)) {
        return json!({ "success": false, "error": err });
      }

      let _ = worktree_remove_internal(
        &state,
        WorktreeRemoveArgs {
          project_path: project_path.to_string(),
          worktree_id: worktree.id.clone(),
          worktree_path: Some(worktree.path.clone()),
          branch: Some(worktree.branch.clone()),
        },
      );

      json!({ "success": true })
    },
  )
  .await
}

#[tauri::command]
pub async fn worktree_get(app: AppHandle, args: WorktreeGetArgs) -> Value {
  run_blocking(
    json!({ "success": false, "error": "Task cancelled" }),
    move || {
      let state: State<WorktreeState> = app.state();
      let guard = state.inner.lock().unwrap();
      match guard.get(&args.worktree_id) {
        Some(wt) => json!({ "success": true, "worktree": wt }),
        None => json!({ "success": false, "error": "Worktree not found" }),
      }
    },
  )
  .await
}

#[tauri::command]
pub async fn worktree_get_all(app: AppHandle) -> Value {
  run_blocking(
    json!({ "success": false, "error": "Task cancelled" }),
    move || {
      let state: State<WorktreeState> = app.state();
      let guard = state.inner.lock().unwrap();
      let worktrees: Vec<WorktreeInfo> = guard.values().cloned().collect();
      json!({ "success": true, "worktrees": worktrees })
    },
  )
  .await
}

pub fn create_worktree_from_branch(
  state: &State<WorktreeState>,
  args: WorktreeCreateFromBranchArgs,
) -> Result<WorktreeInfo, String> {
  let project_path = args.project_path.trim();
  let branch_name = args.branch_name.trim();
  let project_id = args.project_id.trim();
  if project_path.is_empty() || branch_name.is_empty() || project_id.is_empty() {
    return Err("Missing required parameters".to_string());
  }

  let normalized_name = if args.task_name.trim().is_empty() {
    branch_name.replace('/', "-")
  } else {
    args.task_name.trim().to_string()
  };
  let slugged = slugify(&normalized_name);
  let default_path = Path::new(project_path)
    .join("..")
    .join("worktrees")
    .join(format!("{}-{}", slugged, Utc::now().timestamp_millis()));
  let worktree_path = args
    .worktree_path
    .map(PathBuf::from)
    .unwrap_or(default_path);

  if worktree_path.exists() {
    return Err(format!("Worktree directory already exists: {}", worktree_path.display()));
  }
  if let Some(parent) = worktree_path.parent() {
    let _ = fs::create_dir_all(parent);
  }

  run_command(
    "git",
    &[
      "worktree",
      "add",
      &worktree_path.to_string_lossy(),
      branch_name,
    ],
    Some(Path::new(project_path)),
  )
  .map_err(|err| format!("Failed to create worktree for branch {}: {}", branch_name, err))?;

  if !worktree_path.exists() {
    return Err(format!("Worktree directory was not created: {}", worktree_path.display()));
  }

  ensure_codex_log_ignored(&worktree_path);

  let worktree_info = WorktreeInfo {
    id: stable_id_from_path(&worktree_path.to_string_lossy()),
    name: normalized_name,
    branch: branch_name.to_string(),
    path: worktree_path.to_string_lossy().to_string(),
    project_id: project_id.to_string(),
    status: "active".to_string(),
    created_at: Utc::now().to_rfc3339(),
    last_activity: None,
  };

  state
    .inner
    .lock()
    .unwrap()
    .insert(worktree_info.id.clone(), worktree_info.clone());

  Ok(worktree_info)
}

#[tauri::command]
pub async fn project_settings_fetch_base_ref(app: AppHandle, args: FetchBaseRefArgs) -> Value {
  run_blocking(
    json!({ "success": false, "error": "Task cancelled" }),
    move || {
      let db_state: State<DbState> = app.state();
      let project_id = args.project_id.trim();
      let project_path = args.project_path.trim();
      if project_id.is_empty() || project_path.is_empty() {
        return json!({ "success": false, "error": "projectId and projectPath are required" });
      }

      let row = match db::project_settings_row(&db_state, project_id) {
        Ok(row) => row,
        Err(err) => return json!({ "success": false, "error": err }),
      };

      let base_ref = match resolve_project_base_ref(Path::new(project_path), &row) {
        Ok(info) => info,
        Err(err) => return json!({ "success": false, "error": err }),
      };

      match fetch_base_ref_with_fallback(Path::new(project_path), project_id, &base_ref, &db_state) {
        Ok(info) => json!({
          "success": true,
          "baseRef": info.full_ref,
          "remote": info.remote,
          "branch": info.branch,
        }),
        Err(err) => json!({ "success": false, "error": err }),
      }
    },
  )
  .await
}
