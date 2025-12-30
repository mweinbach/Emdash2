#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod db;
mod browser;
mod container;
mod debug;
mod fs;
mod github;
mod git;
mod host_preview;
mod jira;
mod linear;
mod net;
mod plan_lock;
mod pty;
mod providers;
mod runtime;
mod settings;
mod storage;
mod terminal_snapshots;
mod telemetry;
mod update;
mod worktree;

use tauri::Manager;

use serde_json::{json, Value};
use std::path::Path;
use std::process::{Command, Stdio};

#[tauri::command]
fn app_get_version(app: tauri::AppHandle) -> String {
  app.package_info().version.to_string()
}

#[tauri::command]
fn app_get_platform() -> String {
  if cfg!(target_os = "macos") {
    "darwin".to_string()
  } else if cfg!(target_os = "windows") {
    "win32".to_string()
  } else {
    "linux".to_string()
  }
}

#[tauri::command]
fn app_get_electron_version() -> String {
  format!("tauri-{}", tauri::VERSION)
}

#[tauri::command]
fn app_open_external(app: tauri::AppHandle, url: String) -> Value {
  let _ = app;
  match open::that(url) {
    Ok(_) => json!({ "success": true }),
    Err(err) => json!({ "success": false, "error": err.to_string() }),
  }
}

#[tauri::command]
fn app_open_in(app_handle: tauri::AppHandle, app: String, path: String) -> Value {
  let target = app.trim();
  let target_path = path.trim();
  if target.is_empty() || target_path.is_empty() {
    return json!({ "success": false, "error": "Invalid arguments" });
  }

  if matches!(target, "cursor" | "vscode" | "zed") {
    maybe_prepare_project(&app_handle, target_path);
  }

  match open_in(target, target_path) {
    Ok(_) => json!({ "success": true }),
    Err(err) => json!({ "success": false, "error": err }),
  }
}

#[tauri::command]
fn project_open() -> Value {
  let picked = rfd::FileDialog::new().set_title("Open Project").pick_folder();
  match picked {
    Some(path) => json!({ "success": true, "path": path.to_string_lossy() }),
    None => json!({ "success": false, "error": "No directory selected" }),
  }
}

#[tauri::command]
fn settings_get(app: tauri::AppHandle) -> Result<Value, String> {
  let settings = settings::load_settings(&app);
  Ok(json!({ "success": true, "settings": settings }))
}

#[tauri::command]
fn settings_update(app: tauri::AppHandle, settings: Value) -> Result<Value, String> {
  let updated = settings::update_settings(&app, settings);
  Ok(json!({ "success": true, "settings": updated }))
}

#[tauri::command]
fn telemetry_get_status(app: tauri::AppHandle) -> Result<Value, String> {
  let status = telemetry::get_status(&app);
  Ok(json!({ "success": true, "status": status }))
}

#[tauri::command]
fn telemetry_set_enabled(app: tauri::AppHandle, enabled: bool) -> Result<Value, String> {
  let status = telemetry::set_enabled(&app, enabled);
  Ok(json!({ "success": true, "status": status }))
}

#[tauri::command]
fn telemetry_set_onboarding_seen(app: tauri::AppHandle, flag: bool) -> Result<Value, String> {
  let status = telemetry::set_onboarding_seen(&app, flag);
  Ok(json!({ "success": true, "status": status }))
}

#[tauri::command]
fn telemetry_capture(
  app: tauri::AppHandle,
  event: String,
  properties: Option<Value>,
) -> Result<Value, String> {
  Ok(telemetry::capture(&app, event, properties))
}

fn main() {
  tauri::Builder::default()
    .setup(|app| {
      let state = db::init(&app.handle())
        .map_err(|err| std::io::Error::new(std::io::ErrorKind::Other, err))?;
      app.manage(state);
      app.manage(github::GitHubState::new());
      app.manage(host_preview::HostPreviewState::new());
      app.manage(providers::ProviderState::new(&app.handle()));
      app.manage(pty::PtyState::default());
      app.manage(worktree::WorktreeState::new());
      app.manage(container::ContainerState::new());
      app.manage(browser::BrowserViewState::new());
      app.manage(update::UpdateState::new());
      Ok(())
    })
    .invoke_handler(tauri::generate_handler![
      app_get_version,
      app_get_platform,
      app_get_electron_version,
      app_open_external,
      app_open_in,
      project_open,
      pty::pty_start,
      pty::pty_input,
      pty::pty_resize,
      pty::pty_kill,
      pty::pty_snapshot_get,
      pty::pty_snapshot_save,
      pty::pty_snapshot_clear,
      pty::terminal_get_theme,
      github::github_check_cli_installed,
      github::github_install_cli,
      github::github_auth,
      github::github_cancel_auth,
      github::github_get_status,
      github::github_is_authenticated,
      github::github_get_user,
      github::github_get_repositories,
      github::github_connect,
      github::github_clone_repository,
      github::github_issues_list,
      github::github_issues_search,
      github::github_issue_get,
      github::github_list_pull_requests,
      github::github_logout,
      github::github_get_owners,
      github::github_validate_repo_name,
      github::github_create_new_project,
      github::github_create_pull_request_worktree,
      git::git_get_info,
      git::git_get_status,
      git::git_get_file_diff,
      git::git_stage_file,
      git::git_revert_file,
      git::git_commit_and_push,
      git::git_get_branch_status,
      git::git_get_pr_status,
      git::git_list_remote_branches,
      git::git_generate_pr_content,
      git::git_create_pr,
      providers::providers_get_statuses,
      host_preview::host_preview_setup,
      host_preview::host_preview_start,
      host_preview::host_preview_stop,
      host_preview::host_preview_stop_all,
      worktree::worktree_create,
      worktree::worktree_list,
      worktree::worktree_remove,
      worktree::worktree_status,
      worktree::worktree_merge,
      worktree::worktree_get,
      worktree::worktree_get_all,
      db::db_get_projects,
      db::db_save_project,
      db::db_get_tasks,
      db::db_save_task,
      db::db_delete_project,
      db::db_delete_task,
      db::db_save_conversation,
      db::db_get_conversations,
      db::db_get_or_create_default_conversation,
      db::db_save_message,
      db::db_get_messages,
      db::db_delete_conversation,
      db::project_settings_get,
      db::project_settings_update,
      worktree::project_settings_fetch_base_ref,
      settings_get,
      settings_update,
      telemetry_get_status,
      telemetry_set_enabled,
      telemetry_set_onboarding_seen,
      telemetry_capture,
      update::update_check,
      update::update_download,
      update::update_quit_and_install,
      update::update_open_latest,
      fs::fs_list,
      fs::fs_read,
      fs::fs_write,
      fs::fs_remove,
      fs::fs_save_attachment,
      net::net_probe_ports,
      plan_lock::plan_lock,
      plan_lock::plan_unlock,
      debug::debug_append_log,
      linear::linear_save_token,
      linear::linear_check_connection,
      linear::linear_clear_token,
      linear::linear_initial_fetch,
      linear::linear_search_issues,
      jira::jira_save_credentials,
      jira::jira_clear_credentials,
      jira::jira_check_connection,
      jira::jira_initial_fetch,
      jira::jira_search_issues,
      container::container_load_config,
      container::container_start_run,
      container::container_stop_run,
      container::container_inspect_run,
      container::icons_resolve_service,
      browser::browser_view_show,
      browser::browser_view_hide,
      browser::browser_view_set_bounds,
      browser::browser_view_load_url,
      browser::browser_view_go_back,
      browser::browser_view_go_forward,
      browser::browser_view_reload,
      browser::browser_view_open_devtools,
      browser::browser_view_clear
    ])
    .run(tauri::generate_context!())
    .expect("error while running tauri application");
}

fn command_exists(command: &str) -> bool {
  let resolver = if cfg!(target_os = "windows") {
    "where"
  } else {
    "which"
  };
  Command::new(resolver)
    .arg(command)
    .stdout(Stdio::null())
    .stderr(Stdio::null())
    .status()
    .map(|status| status.success())
    .unwrap_or(false)
}

fn try_command(command: &str, args: &[&str]) -> bool {
  Command::new(command)
    .args(args)
    .stdout(Stdio::null())
    .stderr(Stdio::null())
    .status()
    .map(|status| status.success())
    .unwrap_or(false)
}

fn run_shell_command(command: &str) -> bool {
  let mut cmd = if cfg!(target_os = "windows") {
    let mut cmd = Command::new("cmd");
    cmd.args(["/C", command]);
    cmd
  } else {
    let mut cmd = Command::new("sh");
    cmd.args(["-c", command]);
    cmd
  };

  cmd.stdout(Stdio::null()).stderr(Stdio::null());
  cmd.status().map(|status| status.success()).unwrap_or(false)
}

fn pick_node_install_cmds(target: &Path) -> Vec<String> {
  if target.join("pnpm-lock.yaml").exists() {
    return vec![
      "pnpm install --frozen-lockfile",
      "pnpm install",
      "npm ci",
      "npm install",
    ]
    .into_iter()
    .map(String::from)
    .collect();
  }
  if target.join("yarn.lock").exists() {
    return vec![
      "yarn install --immutable",
      "yarn install --frozen-lockfile",
      "yarn install",
      "npm ci",
      "npm install",
    ]
    .into_iter()
    .map(String::from)
    .collect();
  }
  if target.join("bun.lockb").exists() || target.join("bun.lock").exists() {
    return vec!["bun install", "npm ci", "npm install"]
      .into_iter()
      .map(String::from)
      .collect();
  }
  if target.join("package-lock.json").exists() {
    return vec!["npm ci", "npm install"]
      .into_iter()
      .map(String::from)
      .collect();
  }
  vec!["npm install".to_string()]
}

fn spawn_background_install(target: &Path, cmds: &[String]) {
  if cmds.is_empty() {
    return;
  }
  let chain = cmds.join(" || ");
  let mut cmd = if cfg!(target_os = "windows") {
    let mut cmd = Command::new("cmd");
    cmd.args(["/C", &chain]);
    cmd
  } else {
    let mut cmd = Command::new("sh");
    cmd.args(["-c", &chain]);
    cmd
  };

  cmd
    .current_dir(target)
    .stdin(Stdio::null())
    .stdout(Stdio::null())
    .stderr(Stdio::null());
  let _ = cmd.spawn();
}

fn should_auto_install(app: &tauri::AppHandle) -> bool {
  let settings = settings::load_settings(app);
  settings
    .get("projectPrep")
    .and_then(|v| v.get("autoInstallOnOpenInEditor"))
    .and_then(|v| v.as_bool())
    .unwrap_or(true)
}

fn maybe_prepare_project(app: &tauri::AppHandle, target_path: &str) {
  if !should_auto_install(app) {
    return;
  }
  let target = Path::new(target_path);
  if !target.exists() {
    return;
  }
  if !target.join("package.json").exists() {
    return;
  }
  if target.join("node_modules").exists() {
    return;
  }
  let cmds = pick_node_install_cmds(target);
  spawn_background_install(target, &cmds);
}

fn open_in(app: &str, path: &str) -> Result<(), String> {
  if path.trim().is_empty() {
    return Err("Invalid path".to_string());
  }
  let supported = matches!(
    app,
    "finder" | "cursor" | "vscode" | "terminal" | "ghostty" | "zed" | "iterm2" | "warp"
  );
  if !supported {
    return Err("Unsupported platform or app".to_string());
  }

  if cfg!(target_os = "windows") && (app == "ghostty" || app == "zed") {
    return Err(format!("{} is not supported on Windows", app));
  }
  if !cfg!(target_os = "macos") && app == "iterm2" {
    return Err("iTerm2 is only available on macOS".to_string());
  }

  if app == "warp" {
    let urls = [
      format!("warp://action/new_window?path={}", urlencoding::encode(path)),
      format!(
        "warppreview://action/new_window?path={}",
        urlencoding::encode(path)
      ),
    ];
    for url in urls {
      if open::that(url).is_ok() {
        return Ok(());
      }
    }
    return Err("Warp is not installed or its URI scheme is not registered on this platform.".to_string());
  }

  let opened = if cfg!(target_os = "macos") {
    match app {
      "finder" => try_command("open", &[path]),
      "terminal" => try_command("open", &["-a", "Terminal", path]),
      "iterm2" => {
        try_command("open", &["-b", "com.googlecode.iterm2", path])
          || try_command("open", &["-a", "iTerm", path])
          || try_command("open", &["-a", "iTerm2", path])
      }
      "ghostty" => {
        try_command("open", &["-b", "com.mitchellh.ghostty", path])
          || try_command("open", &["-a", "Ghostty", path])
      }
      "cursor" => {
        if command_exists("cursor") && try_command("cursor", &[path]) {
          true
        } else {
          try_command("open", &["-a", "Cursor", path])
        }
      }
      "vscode" => {
        try_command("open", &["-b", "com.microsoft.VSCode", "--args", path])
          || try_command("open", &["-b", "com.microsoft.VSCodeInsiders", "--args", path])
          || try_command("open", &["-a", "Visual Studio Code", path])
      }
      "zed" => {
        if command_exists("zed") && try_command("zed", &[path]) {
          true
        } else {
          try_command("open", &["-a", "Zed", path])
        }
      }
      _ => false,
    }
  } else if cfg!(target_os = "windows") {
    let quoted = |value: &str| format!("\"{}\"", value.replace('"', "\\\""));
    match app {
      "finder" => try_command("explorer", &[path]),
      "cursor" => {
        try_command("cursor", &[path])
          || run_shell_command(&format!("start \"\" cursor {}", quoted(path)))
      }
      "vscode" => {
        try_command("code", &[path])
          || try_command("code-insiders", &[path])
          || run_shell_command(&format!("start \"\" code {}", quoted(path)))
          || run_shell_command(&format!("start \"\" code-insiders {}", quoted(path)))
      }
      "terminal" => {
        if try_command("wt", &["-d", path]) {
          true
        } else {
          let escaped = path.replace('"', "\\\"");
          run_shell_command(&format!("start cmd /K \"cd /d \\\"{}\\\"\"", escaped))
        }
      }
      _ => false,
    }
  } else {
    match app {
      "finder" => try_command("xdg-open", &[path]),
      "cursor" => try_command("cursor", &[path]),
      "vscode" => try_command("code", &[path]) || try_command("code-insiders", &[path]),
      "terminal" => {
        try_command("x-terminal-emulator", &[&format!("--working-directory={}", path)])
          || try_command("gnome-terminal", &[&format!("--working-directory={}", path)])
          || try_command("konsole", &["--workdir", path])
      }
      "ghostty" => {
        try_command("ghostty", &[&format!("--working-directory={}", path)])
          || try_command("x-terminal-emulator", &[&format!("--working-directory={}", path)])
      }
      "zed" => try_command("zed", &[path]) || try_command("xdg-open", &[path]),
      _ => false,
    }
  };

  if opened {
    return Ok(());
  }

  let pretty = match app {
    "ghostty" => "Ghostty",
    "zed" => "Zed",
    "iterm2" => "iTerm2",
    "warp" => "Warp",
    _ => app,
  };
  let msg = match app {
    "ghostty" => "Ghostty is not installed or not available on this platform.".to_string(),
    "zed" => "Zed is not installed or not available on this platform.".to_string(),
    "iterm2" => "iTerm2 is not installed or not available on this platform.".to_string(),
    _ => format!("Unable to open in {}", pretty),
  };
  Err(msg)
}
