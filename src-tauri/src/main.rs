#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod db;
mod fs;
mod github;
mod git;
mod host_preview;
mod providers;
mod settings;
mod storage;
mod telemetry;

use tauri::Manager;

use serde_json::{json, Value};

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
fn app_open_external(app: tauri::AppHandle, url: String) -> Value {
  let _ = app;
  match open::that(url) {
    Ok(_) => json!({ "success": true }),
    Err(err) => json!({ "success": false, "error": err.to_string() }),
  }
}

#[tauri::command]
fn app_open_in(app: String, path: String) -> Value {
  match open_in(app.as_str(), path.as_str()) {
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
fn telemetry_capture(_event: String, _properties: Option<Value>) -> Result<Value, String> {
  Ok(json!({ "success": true }))
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
      Ok(())
    })
    .invoke_handler(tauri::generate_handler![
      app_get_version,
      app_get_platform,
      app_open_external,
      app_open_in,
      project_open,
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
      settings_get,
      settings_update,
      telemetry_get_status,
      telemetry_set_enabled,
      telemetry_set_onboarding_seen,
      telemetry_capture,
      fs::fs_list,
      fs::fs_read,
      fs::fs_write,
      fs::fs_remove,
      fs::fs_save_attachment
    ])
    .run(tauri::generate_context!())
    .expect("error while running tauri application");
}

fn open_in(app: &str, path: &str) -> Result<(), String> {
  if !cfg!(target_os = "macos") {
    return Err("Open in is only implemented for macOS".to_string());
  }
  if path.trim().is_empty() {
    return Err("Invalid path".to_string());
  }

  let open_cmd = |args: &[&str]| -> Result<(), String> {
    let status = std::process::Command::new("open")
      .args(args)
      .status()
      .map_err(|err| err.to_string())?;
    if status.success() {
      Ok(())
    } else {
      Err("Failed to open target app".to_string())
    }
  };

  match app {
    "finder" => open_cmd(&[path]),
    "terminal" => open_cmd(&["-a", "Terminal", path]),
    "iterm2" => open_cmd(&["-b", "com.googlecode.iterm2", path])
      .or_else(|_| open_cmd(&["-a", "iTerm", path]))
      .or_else(|_| open_cmd(&["-a", "iTerm2", path])),
    "ghostty" => open_cmd(&["-b", "com.mitchellh.ghostty", path])
      .or_else(|_| open_cmd(&["-a", "Ghostty", path])),
    "cursor" => open_cmd(&["-a", "Cursor", path]),
    "vscode" => open_cmd(&["-b", "com.microsoft.VSCode", "--args", path])
      .or_else(|_| open_cmd(&["-b", "com.microsoft.VSCodeInsiders", "--args", path]))
      .or_else(|_| open_cmd(&["-a", "Visual Studio Code", path])),
    "zed" => open_cmd(&["-a", "Zed", path]),
    "warp" => {
      let url = format!("warp://action/new_window?path={}", urlencoding::encode(path));
      open_cmd(&[&url]).or_else(|_| {
        let preview = format!("warppreview://action/new_window?path={}", urlencoding::encode(path));
        open_cmd(&[&preview])
      })
    }
    _ => Err("Unsupported target app".to_string()),
  }
}
