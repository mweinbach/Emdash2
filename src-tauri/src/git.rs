use serde::Serialize;
use serde_json::{json, Value};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

const DEFAULT_REMOTE: &str = "origin";
const DEFAULT_BRANCH: &str = "main";

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

fn parse_numstat(output: &str) -> (i64, i64) {
  let mut additions = 0;
  let mut deletions = 0;
  for line in output.lines() {
    let line = line.trim();
    if line.is_empty() {
      continue;
    }
    let mut parts = line.split('\t');
    let add_str = parts.next();
    let del_str = parts.next();
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
      additions += add;
      deletions += del;
    }
  }
  (additions, deletions)
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

#[tauri::command]
pub fn git_get_info(project_path: String) -> Value {
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
pub fn git_get_status(task_path: String) -> Value {
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

    if let Ok(output) = run_git(
      &resolved_path,
      &["diff", "--numstat", "--cached", "--", &file_path],
    ) {
      let (add, del) = parse_numstat(&output);
      additions += add;
      deletions += del;
    }

    if let Ok(output) = run_git(&resolved_path, &["diff", "--numstat", "--", &file_path]) {
      let (add, del) = parse_numstat(&output);
      additions += add;
      deletions += del;
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
pub fn git_get_file_diff(task_path: String, file_path: String) -> Value {
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
pub fn git_stage_file(task_path: String, file_path: String) -> Value {
  let resolved_path = resolve_real_path(Path::new(&task_path));
  match run_git(&resolved_path, &["add", "--", &file_path]) {
    Ok(_) => json!({ "success": true }),
    Err(err) => json!({ "success": false, "error": err }),
  }
}

#[tauri::command]
pub fn git_revert_file(task_path: String, file_path: String) -> Value {
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
pub fn git_commit_and_push(
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
pub fn git_get_branch_status(task_path: String) -> Value {
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
pub fn git_get_pr_status(task_path: String) -> Value {
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
      if lowered.contains("no pull request") || lowered.contains("not found") {
        return json!({ "success": true, "pr": null });
      }
      return json!({ "success": false, "error": err });
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

  json!({ "success": true, "pr": data })
}

#[tauri::command]
pub fn git_list_remote_branches(project_path: String, remote: Option<String>) -> Value {
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
pub fn git_generate_pr_content(_task_path: String, _base: Option<String>) -> Value {
  json!({
    "success": false,
    "error": "PR content generation is not implemented in the Tauri backend yet"
  })
}

#[tauri::command]
pub fn git_create_pr(
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
