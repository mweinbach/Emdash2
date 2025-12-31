use crate::db::{self, DbState};
use crate::providers;
use crate::runtime::run_blocking;
use tauri::Manager;
use serde::Serialize;
use serde_json::{json, Value};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

const DEFAULT_REMOTE: &str = "origin";
const DEFAULT_BRANCH: &str = "main";

#[derive(Clone, Copy)]
struct ProviderGenerationConfig {
  id: &'static str,
  cli: &'static str,
  version_args: Option<&'static [&'static str]>,
  default_args: Option<&'static [&'static str]>,
  auto_approve_flag: Option<&'static str>,
  initial_prompt_flag: Option<&'static str>,
}

const PROVIDER_GENERATION_CONFIGS: &[ProviderGenerationConfig] = &[
  ProviderGenerationConfig {
    id: "codex",
    cli: "codex",
    version_args: Some(&["--version"]),
    default_args: None,
    auto_approve_flag: Some("--full-auto"),
    initial_prompt_flag: Some(""),
  },
  ProviderGenerationConfig {
    id: "claude",
    cli: "claude",
    version_args: Some(&["--version"]),
    default_args: None,
    auto_approve_flag: Some("--dangerously-skip-permissions"),
    initial_prompt_flag: Some(""),
  },
  ProviderGenerationConfig {
    id: "cursor",
    cli: "cursor-agent",
    version_args: Some(&["--version"]),
    default_args: None,
    auto_approve_flag: Some("-p"),
    initial_prompt_flag: Some(""),
  },
  ProviderGenerationConfig {
    id: "gemini",
    cli: "gemini",
    version_args: Some(&["--version"]),
    default_args: None,
    auto_approve_flag: Some("--yolomode"),
    initial_prompt_flag: Some("-i"),
  },
  ProviderGenerationConfig {
    id: "qwen",
    cli: "qwen",
    version_args: Some(&["--version"]),
    default_args: None,
    auto_approve_flag: Some("--yolo"),
    initial_prompt_flag: Some("-i"),
  },
  ProviderGenerationConfig {
    id: "droid",
    cli: "droid",
    version_args: Some(&["--version"]),
    default_args: None,
    auto_approve_flag: None,
    initial_prompt_flag: Some(""),
  },
  ProviderGenerationConfig {
    id: "amp",
    cli: "amp",
    version_args: Some(&["--version"]),
    default_args: None,
    auto_approve_flag: None,
    initial_prompt_flag: None,
  },
  ProviderGenerationConfig {
    id: "opencode",
    cli: "opencode",
    version_args: Some(&["--version"]),
    default_args: None,
    auto_approve_flag: None,
    initial_prompt_flag: Some("-p"),
  },
  ProviderGenerationConfig {
    id: "copilot",
    cli: "copilot",
    version_args: Some(&["--version"]),
    default_args: None,
    auto_approve_flag: None,
    initial_prompt_flag: None,
  },
  ProviderGenerationConfig {
    id: "charm",
    cli: "crush",
    version_args: Some(&["--version"]),
    default_args: None,
    auto_approve_flag: None,
    initial_prompt_flag: None,
  },
  ProviderGenerationConfig {
    id: "auggie",
    cli: "auggie",
    version_args: Some(&["--version"]),
    default_args: Some(&["--allow-indexing"]),
    auto_approve_flag: None,
    initial_prompt_flag: Some(""),
  },
  ProviderGenerationConfig {
    id: "goose",
    cli: "goose",
    version_args: None,
    default_args: Some(&["run", "-s"]),
    auto_approve_flag: None,
    initial_prompt_flag: Some("-t"),
  },
  ProviderGenerationConfig {
    id: "kimi",
    cli: "kimi",
    version_args: Some(&["--version"]),
    default_args: None,
    auto_approve_flag: None,
    initial_prompt_flag: Some("-c"),
  },
  ProviderGenerationConfig {
    id: "kilocode",
    cli: "kilocode",
    version_args: Some(&["--version"]),
    default_args: None,
    auto_approve_flag: Some("--auto"),
    initial_prompt_flag: Some(""),
  },
  ProviderGenerationConfig {
    id: "kiro",
    cli: "kiro-cli",
    version_args: Some(&["--version"]),
    default_args: Some(&["chat"]),
    auto_approve_flag: None,
    initial_prompt_flag: Some(""),
  },
  ProviderGenerationConfig {
    id: "cline",
    cli: "cline",
    version_args: Some(&["help"]),
    default_args: None,
    auto_approve_flag: None,
    initial_prompt_flag: Some(""),
  },
  ProviderGenerationConfig {
    id: "codebuff",
    cli: "codebuff",
    version_args: Some(&["--version"]),
    default_args: None,
    auto_approve_flag: None,
    initial_prompt_flag: Some(""),
  },
  ProviderGenerationConfig {
    id: "mistral",
    cli: "vibe",
    version_args: Some(&["-h"]),
    default_args: None,
    auto_approve_flag: Some("--auto-approve"),
    initial_prompt_flag: Some("--prompt"),
  },
];

fn provider_generation_config(id: &str) -> Option<&'static ProviderGenerationConfig> {
  PROVIDER_GENERATION_CONFIGS.iter().find(|provider| provider.id == id)
}

fn resolve_git_bin() -> String {
  if let Ok(val) = std::env::var("GIT_PATH") {
    let trimmed = val.trim();
    if !trimmed.is_empty() {
      return trimmed.to_string();
    }
  }
  let candidates = ["/opt/homebrew/bin/git", "/usr/local/bin/git", "/usr/bin/git"];
  for candidate in candidates {
    if Path::new(candidate).exists() {
      return candidate.to_string();
    }
  }
  "git".to_string()
}

fn combine_output(stdout: &str, stderr: &str) -> String {
  let mut parts: Vec<&str> = Vec::new();
  if !stderr.trim().is_empty() {
    parts.push(stderr.trim());
  }
  if !stdout.trim().is_empty() {
    parts.push(stdout.trim());
  }
  if parts.is_empty() {
    "Command failed".to_string()
  } else {
    parts.join("\n")
  }
}

fn run_cmd(bin: &str, args: &[&str], cwd: Option<&Path>) -> Result<String, String> {
  let mut cmd = Command::new(bin);
  cmd.args(args);
  if let Some(dir) = cwd {
    cmd.current_dir(dir);
  }
  let output = cmd.output().map_err(|err| err.to_string())?;
  let stdout = String::from_utf8_lossy(&output.stdout).to_string();
  let stderr = String::from_utf8_lossy(&output.stderr).to_string();
  if output.status.success() {
    Ok(stdout)
  } else {
    Err(combine_output(&stdout, &stderr))
  }
}

fn run_cmd_output(
  bin: &str,
  args: &[&str],
  cwd: Option<&Path>,
) -> Result<(bool, String, String), String> {
  let mut cmd = Command::new(bin);
  cmd.args(args);
  if let Some(dir) = cwd {
    cmd.current_dir(dir);
  }
  let output = cmd.output().map_err(|err| err.to_string())?;
  let stdout = String::from_utf8_lossy(&output.stdout).to_string();
  let stderr = String::from_utf8_lossy(&output.stderr).to_string();
  Ok((output.status.success(), stdout, stderr))
}

fn run_git(cwd: &Path, args: &[&str]) -> Result<String, String> {
  let git = resolve_git_bin();
  run_cmd(&git, args, Some(cwd))
}

fn normalize_remote_name(remote: Option<&str>) -> String {
  let trimmed = remote.unwrap_or("").trim();
  if trimmed.is_empty() {
    return DEFAULT_REMOTE.to_string();
  }
  if trimmed.chars().all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '_' || c == '-')
    && !trimmed.contains("://")
  {
    return trimmed.to_string();
  }
  DEFAULT_REMOTE.to_string()
}

fn compute_base_ref(remote: Option<&str>, branch: Option<&str>) -> String {
  let remote_name = normalize_remote_name(remote);
  let branch_name = branch
    .map(|b| b.trim().to_string())
    .filter(|b| !b.is_empty())
    .unwrap_or_else(|| DEFAULT_BRANCH.to_string());
  if branch_name.contains('/') {
    branch_name
  } else {
    format!("{}/{}", remote_name, branch_name)
  }
}

fn detect_default_branch(cwd: &Path, remote: Option<&str>) -> Option<String> {
  let remote_name = normalize_remote_name(remote);
  let output = run_git(cwd, &["remote", "show", &remote_name]).ok()?;
  let needle = "HEAD branch:";
  for line in output.lines() {
    let trimmed = line.trim();
    if let Some(rest) = trimmed.strip_prefix(needle) {
      let value = rest.trim();
      if !value.is_empty() {
        return Some(value.to_string());
      }
    }
  }
  None
}

fn resolve_real_path(path: &Path) -> PathBuf {
  fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
}

fn normalize_git_path(raw: &str) -> String {
  let trimmed = raw.trim();
  if trimmed.is_empty() {
    return String::new();
  }

  // Handle rename output like "old -> new"
  if let Some(idx) = trimmed.rfind("->") {
    return trimmed[idx + 2..].trim().to_string();
  }

  // Handle rename output like "src/{old => new}.rs"
  if let (Some(start), Some(end)) = (trimmed.find('{'), trimmed.rfind('}')) {
    if start < end {
      let inside = &trimmed[start + 1..end];
      if let Some(idx) = inside.rfind("=>") {
        let prefix = &trimmed[..start];
        let suffix = &trimmed[end + 1..];
        let replacement = inside[idx + 2..].trim();
        return format!("{}{}{}", prefix, replacement, suffix);
      }
    }
  }

  trimmed.to_string()
}

fn parse_numstat_map(output: &str) -> HashMap<String, (i64, i64)> {
  let mut map = HashMap::new();
  for line in output.lines() {
    let line = line.trim();
    if line.is_empty() {
      continue;
    }
    let mut parts = line.split('\t');
    let add_str = parts.next();
    let del_str = parts.next();
    let path_raw = parts.collect::<Vec<_>>().join("\t");
    if path_raw.is_empty() {
      continue;
    }
    if let (Some(add_str), Some(del_str)) = (add_str, del_str) {
      let add = if add_str == "-" {
        0
      } else {
        add_str.parse::<i64>().unwrap_or(0)
      };
      let del = if del_str == "-" {
        0
      } else {
        del_str.parse::<i64>().unwrap_or(0)
      };
      let path = normalize_git_path(&path_raw);
      if path.is_empty() {
        continue;
      }
      map
        .entry(path)
        .and_modify(|entry: &mut (i64, i64)| {
          entry.0 += add;
          entry.1 += del;
        })
        .or_insert((add, del));
    }
  }
  map
}

fn count_file_lines(path: &Path) -> i64 {
  if let Ok(buf) = fs::read(path) {
    return buf.iter().filter(|b| **b == b'\n').count() as i64;
  }
  0
}

fn parse_diff_lines(diff: &str) -> Vec<DiffLine> {
  let mut result = Vec::new();
  for raw in diff.lines() {
    let line = raw.trim_end_matches('\r');
    if line.is_empty() {
      continue;
    }
    if line.starts_with("diff ")
      || line.starts_with("index ")
      || line.starts_with("--- ")
      || line.starts_with("+++ ")
      || line.starts_with("@@")
    {
      continue;
    }
    let mut chars = line.chars();
    let Some(prefix) = chars.next() else {
      continue;
    };
    let content = chars.as_str().to_string();
    match prefix {
      ' ' => result.push(DiffLine {
        left: Some(content.clone()),
        right: Some(content),
        kind: "context".to_string(),
      }),
      '-' => result.push(DiffLine {
        left: Some(content),
        right: None,
        kind: "del".to_string(),
      }),
      '+' => result.push(DiffLine {
        left: None,
        right: Some(content),
        kind: "add".to_string(),
      }),
      _ => result.push(DiffLine {
        left: Some(line.to_string()),
        right: Some(line.to_string()),
        kind: "context".to_string(),
      }),
    }
  }
  result
}

fn parse_shortstat(stat: &str) -> (Option<i64>, Option<i64>, Option<i64>) {
  let mut files = None;
  let mut additions = None;
  let mut deletions = None;
  for chunk in stat.split(',') {
    let trimmed = chunk.trim();
    if trimmed.is_empty() {
      continue;
    }
    let mut iter = trimmed.split_whitespace();
    let num = iter.next().and_then(|value| value.parse::<i64>().ok());
    if let Some(num) = num {
      if trimmed.contains("file") {
        files = Some(num);
      } else if trimmed.contains("insertion") {
        additions = Some(num);
      } else if trimmed.contains("deletion") {
        deletions = Some(num);
      }
    }
  }
  (files, additions, deletions)
}

fn to_base36(mut value: u128) -> String {
  let alphabet = b"0123456789abcdefghijklmnopqrstuvwxyz";
  if value == 0 {
    return "0".to_string();
  }
  let mut buf = Vec::new();
  while value > 0 {
    let idx = (value % 36) as usize;
    buf.push(alphabet[idx]);
    value /= 36;
  }
  buf.reverse();
  String::from_utf8_lossy(&buf).to_string()
}

fn parse_github_repo(url: &str) -> Option<String> {
  let trimmed = url.trim().trim_end_matches(".git");
  if trimmed.is_empty() {
    return None;
  }
  if let Some(idx) = trimmed.to_lowercase().find("github.com") {
    let after = &trimmed[idx + "github.com".len()..];
    let after = after.trim_start_matches(&[':', '/'][..]);
    let mut parts = after.split('/');
    let owner = parts.next()?;
    let repo = parts.next()?;
    if owner.is_empty() || repo.is_empty() {
      return None;
    }
    return Some(format!("{}/{}", owner, repo));
  }
  if let Some(idx) = trimmed.find(':') {
    let after = &trimmed[idx + 1..];
    let mut parts = after.split('/');
    let owner = parts.next()?;
    let repo = parts.next()?;
    if owner.is_empty() || repo.is_empty() {
      return None;
    }
    return Some(format!("{}/{}", owner, repo));
  }
  None
}

fn read_staged_files(cwd: &Path) -> Vec<String> {
  run_git(cwd, &["diff", "--cached", "--name-only"])
    .unwrap_or_default()
    .split('\n')
    .map(|line| line.trim().to_string())
    .filter(|line| !line.is_empty())
    .collect()
}

fn extract_url(text: &str) -> Option<String> {
  for token in text.split_whitespace() {
    if token.starts_with("https://") || token.starts_with("http://") {
      return Some(token.to_string());
    }
  }
  None
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct GitChange {
  path: String,
  status: String,
  additions: i64,
  deletions: i64,
  is_staged: bool,
}

#[derive(Serialize)]
struct DiffLine {
  #[serde(skip_serializing_if = "Option::is_none")]
  left: Option<String>,
  #[serde(skip_serializing_if = "Option::is_none")]
  right: Option<String>,
  #[serde(rename = "type")]
  kind: String,
}

fn git_get_info_sync(project_path: String) -> Value {
  let resolved_path = resolve_real_path(Path::new(&project_path));
  let resolved_str = resolved_path.to_string_lossy().to_string();
  let git_path = resolved_path.join(".git");

  if !git_path.exists() {
    return json!({ "isGitRepo": false, "path": resolved_str });
  }

  let remote = run_git(&resolved_path, &["remote", "get-url", DEFAULT_REMOTE])
    .ok()
    .map(|s| s.trim().to_string())
    .filter(|s| !s.is_empty());

  let branch = run_git(&resolved_path, &["branch", "--show-current"])
    .ok()
    .map(|s| s.trim().to_string())
    .filter(|s| !s.is_empty());

  let default_branch = if branch.is_none() {
    detect_default_branch(&resolved_path, remote.as_deref())
  } else {
    None
  };

  let upstream = run_git(
    &resolved_path,
    &["rev-parse", "--abbrev-ref", "--symbolic-full-name", "@{u}"],
  )
  .ok()
  .map(|s| s.trim().to_string())
  .filter(|s| !s.is_empty());

  let (ahead_count, behind_count) = if upstream.is_some() {
    match run_git(
      &resolved_path,
      &["rev-list", "--left-right", "--count", "HEAD...@{u}"],
    ) {
      Ok(output) => {
        let parts: Vec<&str> = output.trim().split_whitespace().collect();
        let ahead = parts.get(0).and_then(|v| v.parse::<i64>().ok());
        let behind = parts.get(1).and_then(|v| v.parse::<i64>().ok());
        (ahead, behind)
      }
      Err(_) => (None, None),
    }
  } else {
    (None, None)
  };

  let root_path = run_git(&resolved_path, &["rev-parse", "--show-toplevel"])
    .ok()
    .map(|s| s.trim().to_string())
    .filter(|s| !s.is_empty())
    .map(|path| resolve_real_path(Path::new(&path)).to_string_lossy().to_string())
    .unwrap_or_else(|| resolved_str.clone());

  let base_ref = compute_base_ref(
    remote.as_deref(),
    branch.as_deref().or(default_branch.as_deref()),
  );

  json!({
    "isGitRepo": true,
    "remote": remote,
    "branch": branch,
    "baseRef": base_ref,
    "upstream": upstream,
    "aheadCount": ahead_count,
    "behindCount": behind_count,
    "path": resolved_str,
    "rootPath": root_path
  })
}

#[tauri::command]
pub async fn git_get_info(project_path: String) -> Value {
  let fallback_path = project_path.clone();
  run_blocking(
    json!({ "isGitRepo": false, "path": fallback_path, "error": "git_get_info failed" }),
    move || git_get_info_sync(project_path),
  )
  .await
}

fn git_get_status_sync(task_path: String) -> Value {
  let resolved_path = resolve_real_path(Path::new(&task_path));
  if run_git(&resolved_path, &["rev-parse", "--is-inside-work-tree"]).is_err() {
    return json!({ "success": true, "changes": Vec::<GitChange>::new() });
  }

  let status_output = match run_git(
    &resolved_path,
    &["status", "--porcelain", "--untracked-files=all"],
  ) {
    Ok(output) => output,
    Err(err) => return json!({ "success": false, "error": err }),
  };

  if status_output.trim().is_empty() {
    return json!({ "success": true, "changes": Vec::<GitChange>::new() });
  }

  let staged_map = run_git(&resolved_path, &["diff", "--numstat", "--cached", "--"])
    .ok()
    .map(|output| parse_numstat_map(&output))
    .unwrap_or_default();
  let unstaged_map = run_git(&resolved_path, &["diff", "--numstat", "--"])
    .ok()
    .map(|output| parse_numstat_map(&output))
    .unwrap_or_default();

  let mut changes: Vec<GitChange> = Vec::new();
  for raw_line in status_output.lines() {
    let line = raw_line.trim_end_matches('\r');
    if line.len() < 3 {
      continue;
    }
    let status_code = &line[0..2];
    let mut file_path = line[3..].to_string();
    if status_code.contains('R') && file_path.contains("->") {
      if let Some(last) = file_path.split("->").last() {
        file_path = last.trim().to_string();
      }
    }

    if file_path.ends_with("codex-stream.log") {
      continue;
    }

    let status = if status_code.contains('A') || status_code.contains('?') {
      "added"
    } else if status_code.contains('D') {
      "deleted"
    } else if status_code.contains('R') {
      "renamed"
    } else {
      "modified"
    };

    let first = status_code.chars().next().unwrap_or(' ');
    let is_staged = first != ' ' && first != '?';

    let mut additions = 0;
    let mut deletions = 0;

    let normalized_path = normalize_git_path(&file_path);
    if let Some((add, del)) = staged_map.get(&normalized_path) {
      additions += *add;
      deletions += *del;
    } else if let Some((add, del)) = staged_map.get(&file_path) {
      additions += *add;
      deletions += *del;
    }

    if let Some((add, del)) = unstaged_map.get(&normalized_path) {
      additions += *add;
      deletions += *del;
    } else if let Some((add, del)) = unstaged_map.get(&file_path) {
      additions += *add;
      deletions += *del;
    }

    if additions == 0 && deletions == 0 && status_code.contains('?') {
      let abs_path = resolved_path.join(&file_path);
      if abs_path.exists() {
        additions = count_file_lines(&abs_path);
      }
    }

    changes.push(GitChange {
      path: file_path,
      status: status.to_string(),
      additions,
      deletions,
      is_staged,
    });
  }

  json!({ "success": true, "changes": changes })
}

#[tauri::command]
pub async fn git_get_status(task_path: String) -> Value {
  let fallback_path = task_path.clone();
  run_blocking(
    json!({ "success": false, "error": "git_get_status failed", "taskPath": fallback_path }),
    move || git_get_status_sync(task_path),
  )
  .await
}

fn git_get_file_diff_sync(task_path: String, file_path: String) -> Value {
  let resolved_path = resolve_real_path(Path::new(&task_path));
  let diff_output = run_git(
    &resolved_path,
    &["diff", "--no-color", "--unified=2000", "HEAD", "--", &file_path],
  );

  if let Ok(output) = diff_output {
    let lines = parse_diff_lines(&output);
    if !lines.is_empty() {
      return json!({ "success": true, "diff": { "lines": lines } });
    }

    let abs = resolved_path.join(&file_path);
    if abs.exists() {
      if let Ok(content) = fs::read_to_string(&abs) {
        let lines = content
          .split('\n')
          .map(|line| DiffLine {
            left: None,
            right: Some(line.to_string()),
            kind: "add".to_string(),
          })
          .collect::<Vec<DiffLine>>();
        return json!({ "success": true, "diff": { "lines": lines } });
      }
    } else if let Ok(prev) = run_git(&resolved_path, &["show", &format!("HEAD:{}", file_path)]) {
      let lines = prev
        .split('\n')
        .map(|line| DiffLine {
          left: Some(line.to_string()),
          right: None,
          kind: "del".to_string(),
        })
        .collect::<Vec<DiffLine>>();
      return json!({ "success": true, "diff": { "lines": lines } });
    }

    return json!({ "success": true, "diff": { "lines": Vec::<DiffLine>::new() } });
  }

  let abs = resolved_path.join(&file_path);
  if let Ok(content) = fs::read_to_string(&abs) {
    let lines = content
      .split('\n')
      .map(|line| DiffLine {
        left: None,
        right: Some(line.to_string()),
        kind: "add".to_string(),
      })
      .collect::<Vec<DiffLine>>();
    return json!({ "success": true, "diff": { "lines": lines } });
  }

  if let Ok(output) = run_git(
    &resolved_path,
    &["diff", "--no-color", "--unified=2000", "HEAD", "--", &file_path],
  ) {
    let lines = parse_diff_lines(&output);
    if !lines.is_empty() {
      return json!({ "success": true, "diff": { "lines": lines } });
    }
    if let Ok(prev) = run_git(&resolved_path, &["show", &format!("HEAD:{}", file_path)]) {
      let lines = prev
        .split('\n')
        .map(|line| DiffLine {
          left: Some(line.to_string()),
          right: None,
          kind: "del".to_string(),
        })
        .collect::<Vec<DiffLine>>();
      return json!({ "success": true, "diff": { "lines": lines } });
    }
  }

  json!({ "success": true, "diff": { "lines": Vec::<DiffLine>::new() } })
}

#[tauri::command]
pub async fn git_get_file_diff(task_path: String, file_path: String) -> Value {
  let fallback_task_path = task_path.clone();
  run_blocking(
    json!({
      "success": false,
      "error": "git_get_file_diff failed",
      "taskPath": fallback_task_path,
    }),
    move || git_get_file_diff_sync(task_path, file_path),
  )
  .await
}

fn git_stage_file_sync(task_path: String, file_path: String) -> Value {
  let resolved_path = resolve_real_path(Path::new(&task_path));
  match run_git(&resolved_path, &["add", "--", &file_path]) {
    Ok(_) => json!({ "success": true }),
    Err(err) => json!({ "success": false, "error": err }),
  }
}

#[tauri::command]
pub async fn git_stage_file(task_path: String, file_path: String) -> Value {
  let fallback_path = task_path.clone();
  run_blocking(
    json!({ "success": false, "error": "git_stage_file failed", "taskPath": fallback_path }),
    move || git_stage_file_sync(task_path, file_path),
  )
  .await
}

fn git_revert_file_sync(task_path: String, file_path: String) -> Value {
  let resolved_path = resolve_real_path(Path::new(&task_path));
  if let Ok(staged) = run_git(
    &resolved_path,
    &["diff", "--cached", "--name-only", "--", &file_path],
  ) {
    if !staged.trim().is_empty() {
      if let Err(err) = run_git(&resolved_path, &["reset", "HEAD", "--", &file_path]) {
        return json!({ "success": false, "error": err });
      }
      return json!({ "success": true, "action": "unstaged" });
    }
  }

  let exists_in_head = run_git(&resolved_path, &["cat-file", "-e", &format!("HEAD:{}", file_path)])
    .is_ok();

  if !exists_in_head {
    let abs = resolved_path.join(&file_path);
    if abs.exists() {
      if let Ok(meta) = fs::metadata(&abs) {
        if meta.is_file() {
          let _ = fs::remove_file(&abs);
        }
      }
    }
    return json!({ "success": true, "action": "reverted" });
  }

  match run_git(&resolved_path, &["checkout", "HEAD", "--", &file_path]) {
    Ok(_) => json!({ "success": true, "action": "reverted" }),
    Err(err) => json!({ "success": false, "error": err }),
  }
}

#[tauri::command]
pub async fn git_revert_file(task_path: String, file_path: String) -> Value {
  let fallback_path = task_path.clone();
  run_blocking(
    json!({ "success": false, "error": "git_revert_file failed", "taskPath": fallback_path }),
    move || git_revert_file_sync(task_path, file_path),
  )
  .await
}

fn git_commit_and_push_sync(
  task_path: String,
  commit_message: Option<String>,
  create_branch_if_on_default: Option<bool>,
  branch_prefix: Option<String>,
) -> Value {
  let resolved_path = resolve_real_path(Path::new(&task_path));
  let commit_message = commit_message.unwrap_or_else(|| "chore: apply task changes".to_string());
  let create_branch_if_on_default = create_branch_if_on_default.unwrap_or(true);
  let branch_prefix = branch_prefix.unwrap_or_else(|| "orch".to_string());

  if let Err(err) = run_git(&resolved_path, &["rev-parse", "--is-inside-work-tree"]) {
    return json!({ "success": false, "error": err });
  }

  let current_branch = run_git(&resolved_path, &["branch", "--show-current"])
    .unwrap_or_default()
    .trim()
    .to_string();

  let mut default_branch = "main".to_string();
  if let Ok(output) = run_cmd(
    "gh",
    &["repo", "view", "--json", "defaultBranchRef", "-q", ".defaultBranchRef.name"],
    Some(&resolved_path),
  ) {
    let trimmed = output.trim();
    if !trimmed.is_empty() {
      default_branch = trimmed.to_string();
    }
  } else if let Some(db) = detect_default_branch(&resolved_path, Some(DEFAULT_REMOTE)) {
    default_branch = db;
  }

  let mut active_branch = current_branch.clone();
  if create_branch_if_on_default && (current_branch.is_empty() || current_branch == default_branch) {
    let now = SystemTime::now()
      .duration_since(UNIX_EPOCH)
      .unwrap_or_default()
      .as_millis();
    let short = to_base36(now as u128);
    let name = format!("{}/{}", branch_prefix, short);
    if let Err(err) = run_git(&resolved_path, &["checkout", "-b", name.as_str()]) {
      return json!({ "success": false, "error": err });
    }
    active_branch = name;
  }

  if let Ok(status_out) = run_git(
    &resolved_path,
    &["status", "--porcelain", "--untracked-files=all"],
  ) {
    let has_working_changes = !status_out.trim().is_empty();
    let mut staged_files = read_staged_files(&resolved_path);

    if has_working_changes && staged_files.is_empty() {
      let _ = run_git(&resolved_path, &["add", "-A"]);
    }

    let _ = run_git(&resolved_path, &["reset", "-q", ".emdash"]);
    let _ = run_git(&resolved_path, &["reset", "-q", "PLANNING.md"]);
    let _ = run_git(&resolved_path, &["reset", "-q", "planning.md"]);

    staged_files = read_staged_files(&resolved_path);
    if !staged_files.is_empty() {
      if let Err(err) = run_git(&resolved_path, &["commit", "-m", commit_message.as_str()]) {
        if !err.to_lowercase().contains("nothing to commit") {
          return json!({ "success": false, "error": err });
        }
      }
    }
  }

  if let Err(err) = run_git(&resolved_path, &["push"]) {
    let branch = if active_branch.is_empty() {
      run_git(&resolved_path, &["rev-parse", "--abbrev-ref", "HEAD"])
        .unwrap_or_default()
        .trim()
        .to_string()
    } else {
      active_branch.clone()
    };
    if let Err(err2) = run_git(
      &resolved_path,
      &["push", "--set-upstream", "origin", branch.as_str()],
    ) {
      return json!({ "success": false, "error": format!("{}\n{}", err, err2) });
    }
  }

  let output = run_git(&resolved_path, &["status", "-sb"])
    .unwrap_or_default()
    .trim()
    .to_string();

  json!({ "success": true, "branch": active_branch, "output": output })
}

#[tauri::command]
pub async fn git_commit_and_push(
  task_path: String,
  commit_message: Option<String>,
  create_branch_if_on_default: Option<bool>,
  branch_prefix: Option<String>,
) -> Value {
  let fallback_path = task_path.clone();
  run_blocking(
    json!({
      "success": false,
      "error": "git_commit_and_push failed",
      "taskPath": fallback_path,
    }),
    move || git_commit_and_push_sync(task_path, commit_message, create_branch_if_on_default, branch_prefix),
  )
  .await
}

fn git_get_branch_status_sync(task_path: String) -> Value {
  let resolved_path = resolve_real_path(Path::new(&task_path));
  if let Err(err) = run_git(&resolved_path, &["rev-parse", "--is-inside-work-tree"]) {
    return json!({ "success": false, "error": err });
  }

  let branch = run_git(&resolved_path, &["branch", "--show-current"])
    .unwrap_or_default()
    .trim()
    .to_string();

  let mut default_branch = "main".to_string();
  if let Ok(output) = run_cmd(
    "gh",
    &["repo", "view", "--json", "defaultBranchRef", "-q", ".defaultBranchRef.name"],
    Some(&resolved_path),
  ) {
    let trimmed = output.trim();
    if !trimmed.is_empty() {
      default_branch = trimmed.to_string();
    }
  } else if let Ok(output) = run_git(
    &resolved_path,
    &["symbolic-ref", "--short", "refs/remotes/origin/HEAD"],
  ) {
    if let Some(last) = output.trim().split('/').last() {
      if !last.is_empty() {
        default_branch = last.to_string();
      }
    }
  }

  let mut ahead = 0;
  let mut behind = 0;
  if let Ok(output) = run_git(
    &resolved_path,
    &[
      "rev-list",
      "--left-right",
      "--count",
      &format!("origin/{}...HEAD", default_branch),
    ],
  ) {
    let parts: Vec<&str> = output.trim().split_whitespace().collect();
    if parts.len() >= 2 {
      behind = parts[0].parse::<i64>().unwrap_or(0);
      ahead = parts[1].parse::<i64>().unwrap_or(0);
    }
  } else if let Ok(output) = run_git(&resolved_path, &["status", "-sb"]) {
    let line = output.lines().next().unwrap_or("");
    if let Some(idx) = line.find("ahead") {
      let after = &line[idx + 5..];
      if let Some(num) = after.split_whitespace().next() {
        ahead = num.parse::<i64>().unwrap_or(ahead);
      }
    }
    if let Some(idx) = line.find("behind") {
      let after = &line[idx + 6..];
      if let Some(num) = after.split_whitespace().next() {
        behind = num.parse::<i64>().unwrap_or(behind);
      }
    }
  }

  json!({
    "success": true,
    "branch": branch,
    "defaultBranch": default_branch,
    "ahead": ahead,
    "behind": behind
  })
}

#[tauri::command]
pub async fn git_get_branch_status(task_path: String) -> Value {
  let fallback_path = task_path.clone();
  run_blocking(
    json!({
      "success": false,
      "error": "git_get_branch_status failed",
      "taskPath": fallback_path,
    }),
    move || git_get_branch_status_sync(task_path),
  )
  .await
}

fn normalize_status_check_state(raw: &str) -> &'static str {
  let value = raw.trim().to_ascii_lowercase();
  match value.as_str() {
    "success" | "neutral" | "skipped" | "passed" => "passed",
    "failure" | "failed" | "cancelled" | "timed_out" | "action_required" | "error" => "failed",
    "in_progress" | "queued" | "pending" | "waiting" | "requested" => "pending",
    _ => "pending",
  }
}

fn summarize_status_checks(data: &Value) -> Option<Value> {
  let rollup = data.get("statusCheckRollup")?.as_array()?;
  if rollup.is_empty() {
    return None;
  }

  let mut total = 0;
  let mut passed = 0;
  let mut failed = 0;
  let mut pending = 0;

  for item in rollup {
    total += 1;
    let state = item
      .get("conclusion")
      .and_then(|v| v.as_str())
      .or_else(|| item.get("state").and_then(|v| v.as_str()))
      .or_else(|| item.get("status").and_then(|v| v.as_str()))
      .unwrap_or("");
    match normalize_status_check_state(state) {
      "passed" => passed += 1,
      "failed" => failed += 1,
      _ => pending += 1,
    }
  }

  Some(json!({
    "total": total,
    "passed": passed,
    "failed": failed,
    "pending": pending
  }))
}

fn git_get_pr_status_sync(task_path: String) -> Value {
  let resolved_path = resolve_real_path(Path::new(&task_path));
  if let Err(err) = run_git(&resolved_path, &["rev-parse", "--is-inside-work-tree"]) {
    return json!({ "success": false, "error": err });
  }

  let fields = [
    "number",
    "url",
    "state",
    "isDraft",
    "mergeStateStatus",
    "headRefName",
    "baseRefName",
    "title",
    "author",
    "additions",
    "deletions",
    "changedFiles",
    "comments",
    "reviews",
    "statusCheckRollup",
  ];

  let mut args = vec!["pr", "view", "--json"];
  let fields_joined = fields.join(",");
  args.push(fields_joined.as_str());
  args.push("-q");
  args.push(".");

  let output = run_cmd("gh", &args, Some(&resolved_path));
  let raw = match output {
    Ok(out) => out,
    Err(err) => {
      let lowered = err.to_lowercase();
      if lowered.contains("unknown field") && lowered.contains("statuscheckrollup") {
        let fallback_fields = fields
          .iter()
          .copied()
          .filter(|field| *field != "statusCheckRollup")
          .collect::<Vec<&str>>();
        let fallback_joined = fallback_fields.join(",");
        let mut fallback_args = vec!["pr", "view", "--json"];
        fallback_args.push(fallback_joined.as_str());
        fallback_args.push("-q");
        fallback_args.push(".");
        match run_cmd("gh", &fallback_args, Some(&resolved_path)) {
          Ok(out) => out,
          Err(fallback_err) => {
            let fallback_lowered = fallback_err.to_lowercase();
            if fallback_lowered.contains("no pull request")
              || fallback_lowered.contains("not found")
            {
              return json!({ "success": true, "pr": null });
            }
            return json!({ "success": false, "error": fallback_err });
          }
        }
      } else if lowered.contains("no pull request") || lowered.contains("not found") {
        return json!({ "success": true, "pr": null });
      } else {
        return json!({ "success": false, "error": err });
      }
    }
  };

  let mut data: Value = match serde_json::from_str(raw.trim()) {
    Ok(value) => value,
    Err(err) => return json!({ "success": false, "error": err.to_string() }),
  };

  let has_add = data
    .get("additions")
    .and_then(|v| v.as_i64().or_else(|| v.as_str().and_then(|s| s.parse::<i64>().ok())))
    .is_some();
  let has_del = data
    .get("deletions")
    .and_then(|v| v.as_i64().or_else(|| v.as_str().and_then(|s| s.parse::<i64>().ok())))
    .is_some();
  let has_files = data
    .get("changedFiles")
    .and_then(|v| v.as_i64().or_else(|| v.as_str().and_then(|s| s.parse::<i64>().ok())))
    .is_some();

  if (!has_add || !has_del || !has_files) && data.is_object() {
    let base_ref = data
      .get("baseRefName")
      .and_then(|v| v.as_str())
      .unwrap_or("")
      .trim()
      .to_string();
    let target_ref = if base_ref.is_empty() {
      None
    } else {
      Some(format!("origin/{}", base_ref))
    };
    let diff_arg = if let Some(target) = target_ref {
      format!("{}...HEAD", target)
    } else {
      "HEAD~1..HEAD".to_string()
    };

    if let Ok(shortstat) =
      run_git(&resolved_path, &["diff", "--shortstat", diff_arg.as_str()])
    {
      let (files, adds, dels) = parse_shortstat(shortstat.trim());
      if let Some(obj) = data.as_object_mut() {
        if !has_files {
          if let Some(files) = files {
            obj.insert("changedFiles".to_string(), json!(files));
          }
        }
        if !has_add {
          if let Some(adds) = adds {
            obj.insert("additions".to_string(), json!(adds));
          }
        }
        if !has_del {
          if let Some(dels) = dels {
            obj.insert("deletions".to_string(), json!(dels));
          }
        }
      }
    }
  }

  let checks_summary = summarize_status_checks(&data);
  let comments_count = data
    .get("comments")
    .and_then(|v| v.as_array())
    .map(|arr| arr.len() as i64)
    .unwrap_or(0);
  let review_count = data
    .get("reviews")
    .and_then(|v| v.as_array())
    .map(|arr| arr.len() as i64)
    .unwrap_or(0);

  if let Some(obj) = data.as_object_mut() {
    if let Some(summary) = checks_summary {
      obj.insert("checksSummary".to_string(), summary);
    }
    obj.insert("commentsCount".to_string(), json!(comments_count));
    obj.insert("reviewCount".to_string(), json!(review_count));
    obj.remove("comments");
    obj.remove("reviews");
    obj.remove("statusCheckRollup");
  }

  json!({ "success": true, "pr": data })
}

#[tauri::command]
pub async fn git_get_pr_status(task_path: String) -> Value {
  let fallback_path = task_path.clone();
  run_blocking(
    json!({ "success": false, "error": "git_get_pr_status failed", "taskPath": fallback_path }),
    move || git_get_pr_status_sync(task_path),
  )
  .await
}

fn git_get_pr_comments_sync(task_path: String) -> Value {
  let resolved_path = resolve_real_path(Path::new(&task_path));
  if let Err(err) = run_git(&resolved_path, &["rev-parse", "--is-inside-work-tree"]) {
    return json!({ "success": false, "error": err });
  }

  let args = ["pr", "view", "--json", "comments,reviews", "-q", "."];
  let raw = match run_cmd("gh", &args, Some(&resolved_path)) {
    Ok(out) => out,
    Err(err) => {
      let lowered = err.to_lowercase();
      if lowered.contains("no pull request") || lowered.contains("not found") {
        return json!({ "success": true, "comments": [] });
      }
      return json!({ "success": false, "error": err });
    }
  };

  let data: Value = match serde_json::from_str(raw.trim()) {
    Ok(value) => value,
    Err(err) => return json!({ "success": false, "error": err.to_string() }),
  };

  let mut items: Vec<Value> = Vec::new();

  if let Some(comments) = data.get("comments").and_then(|v| v.as_array()) {
    for comment in comments {
      let body = comment
        .get("body")
        .cloned()
        .or_else(|| comment.get("bodyText").cloned())
        .unwrap_or(Value::Null);
      let created_at = comment.get("createdAt").cloned().unwrap_or(Value::Null);
      items.push(json!({
        "type": "comment",
        "id": comment.get("id").cloned().unwrap_or(Value::Null),
        "author": comment.get("author").cloned().unwrap_or(Value::Null),
        "body": body,
        "createdAt": created_at,
        "url": comment.get("url").cloned().unwrap_or(Value::Null)
      }));
    }
  }

  if let Some(reviews) = data.get("reviews").and_then(|v| v.as_array()) {
    for review in reviews {
      let body = review
        .get("body")
        .cloned()
        .or_else(|| review.get("bodyText").cloned())
        .unwrap_or(Value::Null);
      let created_at = review
        .get("submittedAt")
        .cloned()
        .or_else(|| review.get("createdAt").cloned())
        .unwrap_or(Value::Null);
      items.push(json!({
        "type": "review",
        "id": review.get("id").cloned().unwrap_or(Value::Null),
        "author": review.get("author").cloned().unwrap_or(Value::Null),
        "body": body,
        "createdAt": created_at,
        "url": review.get("url").cloned().unwrap_or(Value::Null),
        "state": review.get("state").cloned().unwrap_or(Value::Null)
      }));
    }
  }

  items.sort_by(|a, b| {
    let a_ts = a
      .get("createdAt")
      .and_then(|v| v.as_str())
      .unwrap_or("");
    let b_ts = b
      .get("createdAt")
      .and_then(|v| v.as_str())
      .unwrap_or("");
    b_ts.cmp(a_ts)
  });

  json!({ "success": true, "comments": items })
}

#[tauri::command]
pub async fn git_get_pr_comments(task_path: String) -> Value {
  let fallback_path = task_path.clone();
  run_blocking(
    json!({
      "success": false,
      "error": "git_get_pr_comments failed",
      "taskPath": fallback_path,
    }),
    move || git_get_pr_comments_sync(task_path),
  )
  .await
}

fn git_list_remote_branches_sync(project_path: String, remote: Option<String>) -> Value {
  if project_path.trim().is_empty() {
    return json!({ "success": false, "error": "projectPath is required" });
  }
  let resolved_path = resolve_real_path(Path::new(&project_path));
  if run_git(&resolved_path, &["rev-parse", "--is-inside-work-tree"]).is_err() {
    return json!({ "success": false, "error": "Not a git repository" });
  }

  let remote_name = remote.unwrap_or_else(|| DEFAULT_REMOTE.to_string());
  if run_git(&resolved_path, &["remote", "get-url", remote_name.as_str()]).is_ok() {
    let _ = run_git(&resolved_path, &["fetch", "--prune", remote_name.as_str()]);
  }

  let output = match run_git(
    &resolved_path,
    &[
      "for-each-ref",
      "--format=%(refname:short)",
      &format!("refs/remotes/{}", remote_name),
    ],
  ) {
    Ok(output) => output,
    Err(err) => return json!({ "success": false, "error": err }),
  };

  let branches: Vec<Value> = output
    .split('\n')
    .map(|line| line.trim())
    .filter(|line| !line.is_empty())
    .filter(|line| !line.ends_with("/HEAD"))
    .map(|ref_name| {
      let mut parts = ref_name.split('/');
      let remote_alias = parts.next().unwrap_or(&remote_name);
      let branch = parts.collect::<Vec<&str>>().join("/");
      let branch_name = if branch.is_empty() {
        ref_name.to_string()
      } else {
        branch.clone()
      };
      json!({
        "ref": ref_name,
        "remote": remote_alias,
        "branch": branch_name,
        "label": format!("{}/{}", remote_alias, branch_name)
      })
    })
    .collect();

  json!({ "success": true, "branches": branches })
}

#[tauri::command]
pub async fn git_list_remote_branches(project_path: String, remote: Option<String>) -> Value {
  let fallback_path = project_path.clone();
  run_blocking(
    json!({
      "success": false,
      "error": "git_list_remote_branches failed",
      "projectPath": fallback_path,
    }),
    move || git_list_remote_branches_sync(project_path, remote),
  )
  .await
}

fn parse_output_lines(output: &str) -> Vec<String> {
  output
    .lines()
    .map(|line| line.trim())
    .filter(|line| !line.is_empty())
    .map(|line| line.to_string())
    .collect()
}

fn add_files_from_output(output: &str, seen: &mut HashSet<String>, list: &mut Vec<String>) {
  for line in parse_output_lines(output) {
    if seen.insert(line.clone()) {
      list.push(line);
    }
  }
}

fn append_diff_summary(target: &mut String, addition: &str) {
  let trimmed = addition.trim();
  if trimmed.is_empty() {
    return;
  }
  if target.trim().is_empty() {
    *target = trimmed.to_string();
  } else {
    target.push('\n');
    target.push_str(trimmed);
  }
}

fn shortstat_counts(output: &str) -> (i64, i64, i64) {
  let trimmed = output.trim();
  if trimmed.is_empty() {
    return (0, 0, 0);
  }
  let (files, adds, dels) = parse_shortstat(trimmed);
  (
    files.unwrap_or(0),
    adds.unwrap_or(0),
    dels.unwrap_or(0),
  )
}

fn truncate_string(value: &str, max_chars: usize) -> (String, bool) {
  if max_chars == 0 {
    return (String::new(), !value.is_empty());
  }
  let mut out = String::new();
  let mut truncated = false;
  for (idx, ch) in value.chars().enumerate() {
    if idx >= max_chars {
      truncated = true;
      break;
    }
    out.push(ch);
  }
  (out, truncated)
}

fn build_pr_generation_prompt(diff: &str, commits: &[String]) -> String {
  let commit_context = if commits.is_empty() {
    String::new()
  } else {
    format!(
      "\n\nCommits:\n{}",
      commits
        .iter()
        .map(|commit| format!("- {}", commit))
        .collect::<Vec<String>>()
        .join("\n")
    )
  };

  let diff_context = if diff.trim().is_empty() {
    String::new()
  } else {
    let (snippet, truncated) = truncate_string(diff, 2000);
    format!(
      "\n\nDiff summary:\n{}{}",
      snippet,
      if truncated { "..." } else { "" }
    )
  };

  format!(
    r#"Generate a concise PR title and description based on these changes:

{commit_context}{diff_context}

Please respond in the following JSON format:
{{
  "title": "A concise PR title (max 72 chars, use conventional commit format if applicable)",
  "description": "A well-structured markdown description using proper markdown formatting. Use ## for section headers, - or * for lists, `code` for inline code, and proper line breaks.

Use actual newlines (\n in JSON) for line breaks, not literal \n text. Keep it straightforward and to the point."
}}

Only respond with valid JSON, no other text."#,
    commit_context = commit_context,
    diff_context = diff_context
  )
}

fn parse_provider_response(response: &str) -> Option<(String, String)> {
  let start = response.find('{')?;
  let end = response.rfind('}')?;
  if end <= start {
    return None;
  }
  let slice = &response[start..=end];
  let parsed: Value = serde_json::from_str(slice).ok()?;
  let title = parsed.get("title")?.as_str()?.trim().to_string();
  let mut description = parsed.get("description")?.as_str()?.to_string();

  if description.contains("\\n") {
    description = description.replace("\\n", "\n");
  }
  description = description.replace("\\\\n", "\n");
  description = description.trim().to_string();

  Some((title, description))
}

fn normalize_markdown(text: &str) -> String {
  if text.trim().is_empty() {
    return text.trim().to_string();
  }

  let mut lines: Vec<String> = Vec::new();
  let mut prev_blank = false;

  for line in text.lines() {
    let trimmed_end = line.trim_end();
    if trimmed_end.starts_with("##") {
      if !lines.is_empty() && !prev_blank {
        lines.push(String::new());
      }
    }
    lines.push(trimmed_end.to_string());
    prev_blank = trimmed_end.trim().is_empty();
  }

  let mut normalized = lines.join("\n");
  while normalized.contains("\n\n\n") {
    normalized = normalized.replace("\n\n\n", "\n\n");
  }

  normalized.trim().to_string()
}

#[derive(Default)]
struct ProviderCommandOutput {
  success: bool,
  stdout: String,
}

fn run_provider_command(
  command: &str,
  args: &[String],
  cwd: &Path,
  prompt: Option<&str>,
  timeout_ms: u64,
) -> Option<ProviderCommandOutput> {
  let mut cmd = Command::new(command);
  cmd
    .args(args)
    .current_dir(cwd)
    .stdin(Stdio::piped())
    .stdout(Stdio::piped())
    .stderr(Stdio::piped())
    .env("TERM", "xterm-256color")
    .env("COLORTERM", "truecolor");

  let mut child = cmd.spawn().ok()?;

  if let Some(mut stdin) = child.stdin.take() {
    if let Some(prompt) = prompt {
      if stdin.write_all(prompt.as_bytes()).is_err() {
        let _ = child.kill();
        return None;
      }
      if stdin.write_all(b"\n").is_err() {
        let _ = child.kill();
        return None;
      }
      let _ = stdin.flush();
    }
  }

  let stdout_buf = Arc::new(Mutex::new(String::new()));
  let stderr_buf = Arc::new(Mutex::new(String::new()));

  let stdout_handle = if let Some(stdout) = child.stdout.take() {
    let buf = stdout_buf.clone();
    Some(std::thread::spawn(move || {
      let mut reader = std::io::BufReader::new(stdout);
      let mut contents = String::new();
      let _ = reader.read_to_string(&mut contents);
      *buf.lock().unwrap() = contents;
    }))
  } else {
    None
  };

  let stderr_handle = if let Some(stderr) = child.stderr.take() {
    let buf = stderr_buf.clone();
    Some(std::thread::spawn(move || {
      let mut reader = std::io::BufReader::new(stderr);
      let mut contents = String::new();
      let _ = reader.read_to_string(&mut contents);
      *buf.lock().unwrap() = contents;
    }))
  } else {
    None
  };

  let start = Instant::now();
  let mut timed_out = false;
  let status = loop {
    if start.elapsed() >= Duration::from_millis(timeout_ms) {
      timed_out = true;
      let _ = child.kill();
      let _ = child.wait();
      break None;
    }
    match child.try_wait() {
      Ok(Some(status)) => break Some(status),
      Ok(None) => std::thread::sleep(Duration::from_millis(50)),
      Err(_) => break None,
    }
  };

  if let Some(handle) = stdout_handle {
    let _ = handle.join();
  }
  if let Some(handle) = stderr_handle {
    let _ = handle.join();
  }

  let stdout = stdout_buf.lock().unwrap().clone();
  let _stderr = stderr_buf.lock().unwrap().clone();

  let success = status.as_ref().map(|s| s.success()).unwrap_or(false) && !timed_out;
  Some(ProviderCommandOutput {
    success,
    stdout,
  })
}

fn generate_with_provider(
  provider_id: &str,
  task_path: &Path,
  diff: &str,
  commits: &[String],
) -> Option<(String, String)> {
  let provider = provider_generation_config(provider_id)?;
  let version_args = provider.version_args.unwrap_or(&["--version"]);
  if run_cmd(provider.cli, version_args, Some(task_path)).is_err() {
    return None;
  }

  let prompt = build_pr_generation_prompt(diff, commits);
  let mut args: Vec<String> = Vec::new();

  if let Some(default_args) = provider.default_args {
    args.extend(default_args.iter().map(|arg| arg.to_string()));
  }
  if let Some(flag) = provider.auto_approve_flag {
    if !flag.trim().is_empty() {
      args.push(flag.to_string());
    }
  }

  let mut prompt_via_stdin = true;
  if let Some(flag) = provider.initial_prompt_flag {
    if !flag.is_empty() {
      args.push(flag.to_string());
      args.push(prompt.clone());
      prompt_via_stdin = false;
    }
  }

  let output = run_provider_command(
    provider.cli,
    &args,
    task_path,
    if prompt_via_stdin { Some(prompt.as_str()) } else { None },
    30_000,
  )?;

  if !output.success {
    return None;
  }

  let (title, description) = parse_provider_response(&output.stdout)?;
  Some((title, normalize_markdown(&description)))
}

fn generate_pr_title(commits: &[String], changed_files: &[String]) -> String {
  let prefixes = [
    "feat", "fix", "chore", "docs", "style", "refactor", "test", "perf", "ci", "build", "revert",
  ];

  if let Some(first) = commits.first() {
    let lower = first.to_lowercase();
    let mut prefix: Option<&str> = None;
    for candidate in prefixes.iter() {
      let marker = format!("{}:", candidate);
      if lower.starts_with(&marker) {
        prefix = Some(candidate);
        break;
      }
    }

    if let Some(prefix) = prefix {
      let cleaned = first
        .splitn(2, ':')
        .nth(1)
        .unwrap_or("")
        .trim()
        .to_string();
      let mut title = cleaned;
      if title.len() > 72 {
        title = format!("{}...", &title[..69]);
      }
      return format!("{}: {}", prefix, title);
    }

    let mut title = first.trim().to_string();
    if title.len() > 72 {
      title = format!("{}...", &title[..69]);
    }
    return title;
  }

  if let Some(first_file) = changed_files.first() {
    let file_name = Path::new(first_file)
      .file_name()
      .and_then(|s| s.to_str())
      .unwrap_or(first_file.as_str());
    let base_name = file_name
      .rsplit_once('.')
      .map(|(base, _)| base)
      .unwrap_or(file_name);
    let lower = file_name.to_lowercase();

    if lower.contains("test") || lower.contains("spec") {
      return "test: add tests".to_string();
    }
    if lower.contains("fix") || lower.contains("bug") || lower.contains("error") {
      return "fix: resolve issue".to_string();
    }
    if lower.contains("feat") || lower.contains("feature") || lower.contains("add") {
      return "feat: add feature".to_string();
    }

    if base_name
      .chars()
      .next()
      .map(|c| c.is_uppercase())
      .unwrap_or(false)
    {
      return format!("feat: add {}", base_name);
    }

    let target = if !base_name.is_empty() { base_name } else { file_name };
    return format!("chore: update {}", target);
  }

  "chore: update code".to_string()
}

fn generate_pr_description(
  commits: &[String],
  changed_files: &[String],
  file_count: i64,
  insertions: i64,
  deletions: i64,
) -> String {
  let mut parts: Vec<String> = Vec::new();

  if !commits.is_empty() {
    parts.push("## Changes".to_string());
    for commit in commits {
      parts.push(format!("- {}", commit));
    }
  }

  if !changed_files.is_empty() {
    if changed_files.len() == 1 && file_count == 1 {
      parts.push(String::new());
      parts.push("## Summary".to_string());
      parts.push(format!("- Updated `{}`", changed_files[0]));
      if insertions > 0 || deletions > 0 {
        let mut changes: Vec<String> = Vec::new();
        if insertions > 0 {
          changes.push(format!("+{}", insertions));
        }
        if deletions > 0 {
          changes.push(format!("-{}", deletions));
        }
        if !changes.is_empty() {
          parts.push(format!("- {} lines", changes.join(", ")));
        }
      }
    } else {
      parts.push(String::new());
      parts.push("## Files Changed".to_string());
      for file in changed_files.iter().take(20) {
        parts.push(format!("- `{}`", file));
      }
      if changed_files.len() > 20 {
        parts.push(format!(
          "... and {} more files",
          changed_files.len().saturating_sub(20)
        ));
      }

      if file_count > 0 || insertions > 0 || deletions > 0 {
        parts.push(String::new());
        parts.push("## Summary".to_string());
        if file_count > 0 {
          parts.push(format!(
            "- {} file{} changed",
            file_count,
            if file_count == 1 { "" } else { "s" }
          ));
        }
        if insertions > 0 || deletions > 0 {
          let mut changes: Vec<String> = Vec::new();
          if insertions > 0 {
            changes.push(format!("+{}", insertions));
          }
          if deletions > 0 {
            changes.push(format!("-{}", deletions));
          }
          parts.push(format!("- {} lines", changes.join(", ")));
        }
      }
    }
  } else if file_count > 0 || insertions > 0 || deletions > 0 {
    parts.push(String::new());
    parts.push("## Summary".to_string());
    if file_count > 0 {
      parts.push(format!(
        "- {} file{} changed",
        file_count,
        if file_count == 1 { "" } else { "s" }
      ));
    }
    if insertions > 0 || deletions > 0 {
      let mut changes: Vec<String> = Vec::new();
      if insertions > 0 {
        changes.push(format!("+{}", insertions));
      }
      if deletions > 0 {
        changes.push(format!("-{}", deletions));
      }
      parts.push(format!("- {} lines", changes.join(", ")));
    }
  }

  let description = parts.join("\n").trim().to_string();
  if description.is_empty() {
    "No description available.".to_string()
  } else {
    description
  }
}

fn generate_fallback_content(changed_files: &[String]) -> (String, String) {
  let title = if let Some(first) = changed_files.first() {
    let name = Path::new(first)
      .file_name()
      .and_then(|s| s.to_str())
      .unwrap_or("files");
    format!("chore: update {}", name)
  } else {
    "chore: update code".to_string()
  };

  let description = if !changed_files.is_empty() {
    format!(
      "Updated {} file{}.",
      changed_files.len(),
      if changed_files.len() == 1 { "" } else { "s" }
    )
  } else {
    "No changes detected.".to_string()
  };

  (title, description)
}

fn git_generate_pr_content_sync(state: &DbState, task_path: String, base: Option<String>) -> Value {
  let resolved_path = resolve_real_path(Path::new(&task_path));
  let mut preferred_provider = db::task_agent_id_for_path(state, &task_path);
  if preferred_provider.is_none() {
    let resolved_str = resolved_path.to_string_lossy();
    if resolved_str != task_path {
      preferred_provider = db::task_agent_id_for_path(state, resolved_str.as_ref());
    }
  }
  let preferred_provider = preferred_provider.and_then(|id| {
    let trimmed = id.trim();
    if trimmed.is_empty() {
      None
    } else {
      Some(trimmed.to_string())
    }
  });
  if let Err(err) = run_git(&resolved_path, &["rev-parse", "--is-inside-work-tree"]) {
    return json!({ "success": false, "error": err });
  }

  let _ = run_git(&resolved_path, &["fetch", "origin", "--quiet"]);

  let base_branch = base
    .map(|b| b.trim().to_string())
    .filter(|b| !b.is_empty())
    .unwrap_or_else(|| DEFAULT_BRANCH.to_string());

  let mut base_ref: Option<String> = None;
  let origin_ref = format!("origin/{}", base_branch);
  if run_git(&resolved_path, &["rev-parse", "--verify", origin_ref.as_str()]).is_ok() {
    base_ref = Some(origin_ref);
  } else if run_git(&resolved_path, &["rev-parse", "--verify", base_branch.as_str()]).is_ok() {
    base_ref = Some(base_branch.clone());
  }

  let mut commits: Vec<String> = Vec::new();
  let mut diff_summary = String::new();
  let mut changed_files: Vec<String> = Vec::new();
  let mut seen: HashSet<String> = HashSet::new();
  let mut file_count = 0;
  let mut insertions = 0;
  let mut deletions = 0;

  if let Some(ref base_ref) = base_ref {
    if let Ok(output) = run_git(
      &resolved_path,
      &["log", &format!("{}..HEAD", base_ref), "--pretty=format:%s"],
    ) {
      commits = parse_output_lines(&output);
    }
    if let Ok(output) = run_git(
      &resolved_path,
      &["diff", &format!("{}...HEAD", base_ref), "--stat"],
    ) {
      append_diff_summary(&mut diff_summary, &output);
    }
    if let Ok(output) = run_git(
      &resolved_path,
      &["diff", "--name-only", &format!("{}...HEAD", base_ref)],
    ) {
      add_files_from_output(&output, &mut seen, &mut changed_files);
    }
    if let Ok(output) =
      run_git(&resolved_path, &["diff", "--shortstat", &format!("{}...HEAD", base_ref)])
    {
      let (files, adds, dels) = shortstat_counts(&output);
      file_count += files;
      insertions += adds;
      deletions += dels;
    }
  }

  if let Ok(output) = run_git(&resolved_path, &["diff", "--name-only"]) {
    add_files_from_output(&output, &mut seen, &mut changed_files);
  }
  if let Ok(output) = run_git(&resolved_path, &["diff", "--stat"]) {
    append_diff_summary(&mut diff_summary, &output);
  }
  if let Ok(output) = run_git(&resolved_path, &["diff", "--shortstat"]) {
    let (files, adds, dels) = shortstat_counts(&output);
    file_count += files;
    insertions += adds;
    deletions += dels;
  }

  if commits.is_empty() && changed_files.is_empty() && file_count == 0 && insertions == 0 && deletions == 0 {
    if let Ok(output) = run_git(&resolved_path, &["diff", "--cached", "--name-only"]) {
      add_files_from_output(&output, &mut seen, &mut changed_files);
    }
    if let Ok(output) = run_git(&resolved_path, &["diff", "--cached", "--stat"]) {
      append_diff_summary(&mut diff_summary, &output);
    }
    if let Ok(output) = run_git(&resolved_path, &["diff", "--cached", "--shortstat"]) {
      let (files, adds, dels) = shortstat_counts(&output);
      file_count += files;
      insertions += adds;
      deletions += dels;
    }
  }

  if commits.is_empty() && changed_files.is_empty() && file_count == 0 && insertions == 0 && deletions == 0 {
    let (title, description) = generate_fallback_content(&changed_files);
    return json!({ "success": true, "title": title, "description": description });
  }

  let diff_for_prompt = diff_summary.trim().to_string();
  let has_context = !diff_for_prompt.is_empty() || !commits.is_empty();

  if has_context {
    if let Some(provider_id) = preferred_provider {
      if providers::is_valid_provider_id(&provider_id) {
        if let Some((title, description)) =
          generate_with_provider(&provider_id, &resolved_path, &diff_for_prompt, &commits)
        {
          return json!({ "success": true, "title": title, "description": description });
        }
      }
    }

    if let Some((title, description)) =
      generate_with_provider("claude", &resolved_path, &diff_for_prompt, &commits)
    {
      return json!({ "success": true, "title": title, "description": description });
    }

    if let Some((title, description)) =
      generate_with_provider("codex", &resolved_path, &diff_for_prompt, &commits)
    {
      return json!({ "success": true, "title": title, "description": description });
    }
  }

  let title = generate_pr_title(&commits, &changed_files);
  let description =
    generate_pr_description(&commits, &changed_files, file_count, insertions, deletions);
  json!({ "success": true, "title": title, "description": description })
}

#[tauri::command]
pub async fn git_generate_pr_content(app: tauri::AppHandle, task_path: String, base: Option<String>) -> Value {
  let fallback_path = task_path.clone();
  run_blocking(
    json!({
      "success": false,
      "error": "git_generate_pr_content failed",
      "taskPath": fallback_path,
    }),
    move || {
      let state: tauri::State<DbState> = app.state();
      git_generate_pr_content_sync(&state, task_path, base)
    },
  )
  .await
}

fn git_create_pr_sync(
  task_path: String,
  title: Option<String>,
  body: Option<String>,
  base: Option<String>,
  head: Option<String>,
  draft: Option<bool>,
  web: Option<bool>,
  fill: Option<bool>,
) -> Value {
  let resolved_path = resolve_real_path(Path::new(&task_path));
  if let Err(err) = run_git(&resolved_path, &["rev-parse", "--is-inside-work-tree"]) {
    return json!({ "success": false, "error": err });
  }

  let mut outputs: Vec<String> = Vec::new();

  if let Ok(status_out) = run_git(
    &resolved_path,
    &["status", "--porcelain", "--untracked-files=all"],
  ) {
    if !status_out.trim().is_empty() {
      if let Ok(add_out) = run_git(&resolved_path, &["add", "-A"]) {
        if !add_out.trim().is_empty() {
          outputs.push(add_out.trim().to_string());
        }
      }

      let commit_msg = "stagehand: prepare pull request";
      match run_git(&resolved_path, &["commit", "-m", commit_msg]) {
        Ok(commit_out) => {
          if !commit_out.trim().is_empty() {
            outputs.push(commit_out.trim().to_string());
          }
        }
        Err(err) => {
          if err.to_lowercase().contains("nothing to commit") {
            outputs.push("git commit: nothing to commit".to_string());
          } else {
            return json!({ "success": false, "error": err });
          }
        }
      }
    }
  }

  match run_git(&resolved_path, &["push"]) {
    Ok(_) => outputs.push("git push: success".to_string()),
    Err(_) => {
      let branch = run_git(&resolved_path, &["rev-parse", "--abbrev-ref", "HEAD"])
        .unwrap_or_default()
        .trim()
        .to_string();
      if let Err(err) = run_git(
        &resolved_path,
        &["push", "--set-upstream", "origin", branch.as_str()],
      ) {
        return json!({
          "success": false,
          "error": "Failed to push branch to origin. Please check your Git remotes and authentication.",
          "output": err
        });
      }
      outputs.push(format!("git push --set-upstream origin {}: success", branch));
    }
  }

  let mut repo_name_with_owner = String::new();
  if let Ok(output) = run_cmd(
    "gh",
    &["repo", "view", "--json", "nameWithOwner", "-q", ".nameWithOwner"],
    Some(&resolved_path),
  ) {
    let trimmed = output.trim();
    if !trimmed.is_empty() {
      repo_name_with_owner = trimmed.to_string();
    }
  } else if let Ok(url_out) = run_git(&resolved_path, &["remote", "get-url", "origin"]) {
    if let Some(repo) = parse_github_repo(url_out.trim()) {
      repo_name_with_owner = repo;
    }
  }

  let current_branch = run_git(&resolved_path, &["branch", "--show-current"])
    .unwrap_or_default()
    .trim()
    .to_string();

  let mut default_branch = "main".to_string();
  if let Ok(output) = run_cmd(
    "gh",
    &["repo", "view", "--json", "defaultBranchRef", "-q", ".defaultBranchRef.name"],
    Some(&resolved_path),
  ) {
    let trimmed = output.trim();
    if !trimmed.is_empty() {
      default_branch = trimmed.to_string();
    }
  } else if let Some(db) = detect_default_branch(&resolved_path, Some(DEFAULT_REMOTE)) {
    default_branch = db;
  }

  if let Ok(output) = run_git(
    &resolved_path,
    &[
      "rev-list",
      "--count",
      &format!(
        "origin/{}..HEAD",
        base.clone().unwrap_or_else(|| default_branch.clone())
      ),
    ],
  ) {
    let ahead_count = output.trim().parse::<i64>().unwrap_or(0);
    if ahead_count <= 0 {
      let base_ref = base.clone().unwrap_or_else(|| default_branch.clone());
      return json!({
        "success": false,
        "error": format!(
          "No commits to create a PR. Make a commit on current branch '{}' ahead of base '{}'.",
          current_branch, base_ref
        )
      });
    }
  }

  let mut args: Vec<String> = Vec::new();
  args.push("pr".to_string());
  args.push("create".to_string());
  if !repo_name_with_owner.is_empty() {
    args.push("--repo".to_string());
    args.push(repo_name_with_owner.clone());
  }
  if let Some(title) = title.clone() {
    if !title.trim().is_empty() {
      args.push("--title".to_string());
      args.push(title);
    }
  }

  let mut body_file: Option<PathBuf> = None;
  if let Some(body) = body.clone() {
    if !body.trim().is_empty() {
      let mut file_path = std::env::temp_dir();
      let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
      let name = format!("gh-pr-body-{}-{}.txt", now, std::process::id());
      file_path.push(name);
      if fs::write(&file_path, body.as_bytes()).is_ok() {
        args.push("--body-file".to_string());
        args.push(file_path.to_string_lossy().to_string());
        body_file = Some(file_path);
      } else {
        args.push("--body".to_string());
        args.push(body);
      }
    }
  }

  let base_ref = base.clone().unwrap_or_else(|| default_branch.clone());
  if !base_ref.trim().is_empty() {
    args.push("--base".to_string());
    args.push(base_ref.clone());
  }

  if let Some(head) = head.clone() {
    if !head.trim().is_empty() {
      args.push("--head".to_string());
      args.push(head);
    }
  } else if !current_branch.is_empty() {
    let head_ref = if !repo_name_with_owner.is_empty() {
      let owner = repo_name_with_owner
        .split('/')
        .next()
        .unwrap_or("")
        .to_string();
      if owner.is_empty() {
        current_branch.clone()
      } else {
        format!("{}:{}", owner, current_branch)
      }
    } else {
      current_branch.clone()
    };
    args.push("--head".to_string());
    args.push(head_ref);
  }

  if draft.unwrap_or(false) {
    args.push("--draft".to_string());
  }
  if web.unwrap_or(false) {
    args.push("--web".to_string());
  }
  if fill.unwrap_or(false) {
    args.push("--fill".to_string());
  }

  let (success, stdout, stderr) = match run_cmd_output(
    "gh",
    &args.iter().map(|s| s.as_str()).collect::<Vec<&str>>(),
    Some(&resolved_path),
  ) {
    Ok(result) => result,
    Err(err) => {
      if let Some(path) = body_file.as_ref() {
        let _ = fs::remove_file(path);
      }
      return json!({ "success": false, "error": err });
    }
  };

  if let Some(path) = body_file.as_ref() {
    let _ = fs::remove_file(path);
  }

  let combined = [outputs.join("\n"), stdout.trim().to_string(), stderr.trim().to_string()]
    .into_iter()
    .filter(|s| !s.trim().is_empty())
    .collect::<Vec<String>>()
    .join("\n")
    .trim()
    .to_string();

  if !success {
    let restriction_re = [
      "Auth App access restrictions",
      "authorized OAuth apps",
      "third-parties is limited",
    ];
    let lower = combined.to_lowercase();
    let code = if restriction_re
      .iter()
      .any(|needle| lower.contains(&needle.to_lowercase()))
    {
      Some("ORG_AUTH_APP_RESTRICTED")
    } else {
      None
    };

    if let Some(code) = code {
      return json!({
        "success": false,
        "error": combined,
        "output": combined,
        "code": code
      });
    }

    return json!({ "success": false, "error": combined, "output": combined });
  }

  let url = extract_url(&combined);
  json!({
    "success": true,
    "url": url,
    "output": combined
  })
}

#[tauri::command]
pub async fn git_create_pr(
  task_path: String,
  title: Option<String>,
  body: Option<String>,
  base: Option<String>,
  head: Option<String>,
  draft: Option<bool>,
  web: Option<bool>,
  fill: Option<bool>,
) -> Value {
  let fallback_path = task_path.clone();
  run_blocking(
    json!({ "success": false, "error": "git_create_pr failed", "taskPath": fallback_path }),
    move || git_create_pr_sync(task_path, title, body, base, head, draft, web, fill),
  )
  .await
}

fn git_merge_pr_sync(task_path: String, method: Option<String>, delete_branch: Option<bool>) -> Value {
  let resolved_path = resolve_real_path(Path::new(&task_path));
  if let Err(err) = run_git(&resolved_path, &["rev-parse", "--is-inside-work-tree"]) {
    return json!({ "success": false, "error": err });
  }

  let mut args: Vec<&str> = vec!["pr", "merge", "--yes"];
  if let Some(raw_method) = method {
    let normalized = raw_method.trim().to_ascii_lowercase();
    match normalized.as_str() {
      "merge" => args.push("--merge"),
      "squash" => args.push("--squash"),
      "rebase" => args.push("--rebase"),
      _ => {}
    }
  }
  if delete_branch.unwrap_or(false) {
    args.push("--delete-branch");
  }
  args.push(".");

  let (success, stdout, stderr) = match run_cmd_output("gh", &args, Some(&resolved_path)) {
    Ok(result) => result,
    Err(err) => return json!({ "success": false, "error": err }),
  };

  let combined = [stdout.trim().to_string(), stderr.trim().to_string()]
    .into_iter()
    .filter(|s| !s.trim().is_empty())
    .collect::<Vec<String>>()
    .join("\n")
    .trim()
    .to_string();

  if !success {
    let lowered = combined.to_lowercase();
    if lowered.contains("no pull request") || lowered.contains("not found") {
      return json!({ "success": false, "error": "No pull request found for this branch." });
    }
    return json!({ "success": false, "error": combined, "output": combined });
  }

  let pr_status = git_get_pr_status_sync(task_path);
  let pr_value = pr_status.get("pr").cloned();
  json!({ "success": true, "output": combined, "pr": pr_value })
}

#[tauri::command]
pub async fn git_merge_pr(
  task_path: String,
  method: Option<String>,
  delete_branch: Option<bool>,
) -> Value {
  let fallback_path = task_path.clone();
  run_blocking(
    json!({ "success": false, "error": "git_merge_pr failed", "taskPath": fallback_path }),
    move || git_merge_pr_sync(task_path, method, delete_branch),
  )
  .await
}
