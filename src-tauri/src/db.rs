use crate::storage;
use rusqlite::{params, Connection, OptionalExtension};
use serde::Deserialize;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use tauri::Manager;

const CURRENT_DB_FILENAME: &str = "emdash.db";
const LEGACY_DB_FILENAMES: &[&str] = &["database.sqlite", "orcbench.db"];
const LEGACY_DIRS: &[&str] = &["Electron", "emdash", "Emdash"];

pub struct DbState {
  conn: Mutex<Option<Connection>>,
  disabled: bool,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct GitInfoInput {
  remote: Option<String>,
  branch: Option<String>,
  base_ref: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct GithubInfoInput {
  repository: String,
  connected: bool,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ProjectInput {
  id: String,
  name: String,
  path: String,
  git_info: GitInfoInput,
  github_info: Option<GithubInfoInput>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct TaskInput {
  id: String,
  project_id: String,
  name: String,
  branch: String,
  path: String,
  status: String,
  agent_id: Option<String>,
  metadata: Option<Value>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ConversationInput {
  id: String,
  task_id: String,
  title: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct MessageInput {
  id: String,
  conversation_id: String,
  content: String,
  sender: String,
  metadata: Option<Value>,
}

#[derive(Deserialize)]
pub struct ProjectSettingsUpdate {
  project_id: String,
  base_ref: String,
}

#[derive(Clone)]
struct MigrationEntry {
  tag: String,
  when: i64,
}

#[derive(Clone)]
struct Migration {
  tag: String,
  when: i64,
  hash: String,
  statements: Vec<String>,
}

fn now_millis() -> i64 {
  use std::time::{SystemTime, UNIX_EPOCH};
  SystemTime::now()
    .duration_since(UNIX_EPOCH)
    .map(|d| d.as_millis() as i64)
    .unwrap_or(0)
}

fn default_branch() -> &'static str {
  "main"
}

fn is_simple_remote_name(value: &str) -> bool {
  if value.is_empty() {
    return false;
  }
  value
    .chars()
    .all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '_' || c == '-')
}

fn remote_alias(remote: Option<&str>) -> String {
  match remote {
    Some(r) => {
      let trimmed = r.trim();
      if trimmed.is_empty() {
        "origin".to_string()
      } else if trimmed.contains("://") {
        "origin".to_string()
      } else if is_simple_remote_name(trimmed) {
        trimmed.to_string()
      } else {
        "origin".to_string()
      }
    }
    None => "origin".to_string(),
  }
}

fn normalize_ref(value: Option<&str>, remote_name: &str) -> Option<String> {
  let trimmed = value?.trim();
  if trimmed.is_empty() || trimmed.contains("://") {
    return None;
  }
  if trimmed.contains('/') {
    let mut parts = trimmed.split('/');
    let head = parts.next().unwrap_or("").trim();
    let rest = parts.collect::<Vec<_>>().join("/").trim().to_string();
    if head.is_empty() && rest.is_empty() {
      return None;
    }
    if !head.is_empty() && !rest.is_empty() {
      return Some(format!("{}/{}", head, rest));
    }
    if head.is_empty() && !rest.is_empty() {
      return Some(format!("{}/{}", remote_name, rest));
    }
    return None;
  }
  Some(format!(
    "{}/{}",
    remote_name,
    trimmed.trim_start_matches('/')
  ))
}

fn compute_base_ref(preferred: Option<&str>, remote: Option<&str>, branch: Option<&str>) -> String {
  let remote_name = remote_alias(remote);
  normalize_ref(preferred, &remote_name)
    .or_else(|| normalize_ref(branch, &remote_name))
    .unwrap_or_else(|| format!("{}/{}", remote_name, default_branch()))
}

fn resolve_database_path(app: &tauri::AppHandle) -> Result<PathBuf, String> {
  if let Ok(custom) = std::env::var("EMDASH_DB_FILE") {
    if !custom.trim().is_empty() {
      return Ok(PathBuf::from(custom));
    }
  }

  let user_data = app
    .path()
    .app_data_dir()
    .ok()
    .or_else(|| app.path().app_config_dir().ok())
    .unwrap_or_else(|| storage::config_dir(app));

  if !user_data.exists() {
    let _ = fs::create_dir_all(&user_data);
  }

  let current_path = user_data.join(CURRENT_DB_FILENAME);
  if current_path.exists() {
    return Ok(current_path);
  }

  if let Some(parent) = user_data.parent() {
    for dir_name in LEGACY_DIRS {
      let candidate_dir = parent.join(dir_name);
      let candidate_current = candidate_dir.join(CURRENT_DB_FILENAME);
      if candidate_current.exists() {
        if let Err(_) = fs::rename(&candidate_current, &current_path) {
          return Ok(candidate_current);
        }
        return Ok(current_path);
      }
    }
  }

  for legacy in LEGACY_DB_FILENAMES {
    let legacy_path = user_data.join(legacy);
    if legacy_path.exists() {
      if let Err(_) = fs::rename(&legacy_path, &current_path) {
        return Ok(legacy_path);
      }
      return Ok(current_path);
    }
  }

  Ok(current_path)
}

fn resolve_migrations_path(app: &tauri::AppHandle) -> Option<PathBuf> {
  let mut candidates: Vec<PathBuf> = Vec::new();

  if let Ok(resource_dir) = app.path().resource_dir() {
    candidates.push(resource_dir.join("drizzle"));
    if let Some(parent) = resource_dir.parent() {
      candidates.push(parent.join("drizzle"));
    }
  }

  if let Ok(cwd) = std::env::current_dir() {
    candidates.push(cwd.join("drizzle"));
    if let Some(parent) = cwd.parent() {
      candidates.push(parent.join("drizzle"));
      if let Some(grand) = parent.parent() {
        candidates.push(grand.join("drizzle"));
      }
    }
  }

  candidates.into_iter().find(|path| path.exists())
}

fn read_journal(migrations_path: &Path) -> Option<Vec<MigrationEntry>> {
  let journal_path = migrations_path.join("meta").join("_journal.json");
  let raw = fs::read_to_string(journal_path).ok()?;
  let parsed: Value = serde_json::from_str(&raw).ok()?;
  let entries = parsed.get("entries")?.as_array()?;

  let mut list: Vec<MigrationEntry> = Vec::new();
  for entry in entries {
    let tag = entry.get("tag")?.as_str()?.to_string();
    let when = entry.get("when").and_then(|v| v.as_i64()).unwrap_or_else(now_millis);
    list.push(MigrationEntry { tag, when });
  }
  Some(list)
}

fn split_statements(contents: &str) -> Vec<String> {
  contents
    .split("--> statement-breakpoint")
    .map(|chunk| chunk.trim().to_string())
    .filter(|chunk| !chunk.is_empty())
    .collect()
}

fn compute_hash(contents: &str) -> String {
  let mut hasher = Sha256::new();
  hasher.update(contents.as_bytes());
  let out = hasher.finalize();
  hex::encode(out)
}

fn load_migrations(migrations_path: &Path) -> Result<Vec<Migration>, String> {
  let journal = read_journal(migrations_path)
    .ok_or_else(|| "Drizzle migrations journal not found".to_string())?;
  let mut list: Vec<Migration> = Vec::new();
  for entry in journal {
    let sql_path = migrations_path.join(format!("{}.sql", entry.tag));
    let contents = fs::read_to_string(&sql_path)
      .map_err(|_| format!("Missing migration SQL: {}", entry.tag))?;
    let hash = compute_hash(&contents);
    let statements = split_statements(&contents);
    list.push(Migration {
      tag: entry.tag,
      when: entry.when,
      hash,
      statements,
    });
  }
  Ok(list)
}

fn table_exists(conn: &Connection, name: &str) -> Result<bool, String> {
  let mut stmt = conn
    .prepare("SELECT name FROM sqlite_master WHERE type='table' AND name=?1 LIMIT 1")
    .map_err(|err| err.to_string())?;
  let mut rows = stmt.query(params![name]).map_err(|err| err.to_string())?;
  Ok(rows.next().map_err(|err| err.to_string())?.is_some())
}

fn table_has_column(conn: &Connection, table: &str, column: &str) -> Result<bool, String> {
  if !table_exists(conn, table)? {
    return Ok(false);
  }
  let mut stmt = conn
    .prepare(&format!("PRAGMA table_info(\"{}\")", table.replace('"', "\"\"")))
    .map_err(|err| err.to_string())?;
  let rows = stmt
    .query_map([], |row| row.get::<_, String>(1))
    .map_err(|err| err.to_string())?;
  for name in rows {
    if let Ok(name) = name {
      if name == column {
        return Ok(true);
      }
    }
  }
  Ok(false)
}

fn ensure_migrations(conn: &Connection, migrations_path: &Path) -> Result<(), String> {
  conn
    .execute_batch("PRAGMA foreign_keys=OFF;")
    .map_err(|err| err.to_string())?;

  let result = (|| {
    let migrations = load_migrations(migrations_path)?;
    conn
      .execute_batch(
        "CREATE TABLE IF NOT EXISTS \"__drizzle_migrations\" (
           id INTEGER PRIMARY KEY AUTOINCREMENT,
           hash text NOT NULL,
           created_at numeric
         );",
      )
      .map_err(|err| err.to_string())?;

    let mut stmt = conn
      .prepare("SELECT hash FROM \"__drizzle_migrations\"")
      .map_err(|err| err.to_string())?;
    let rows = stmt
      .query_map([], |row| row.get::<_, String>(0))
      .map_err(|err| err.to_string())?;
    let mut applied: HashSet<String> = HashSet::new();
    for row in rows {
      if let Ok(hash) = row {
        applied.insert(hash);
      }
    }

    let recovered = table_exists(conn, "tasks")?
      && table_exists(conn, "conversations")?
      && table_exists(conn, "__new_conversations")?
      && table_has_column(conn, "conversations", "workspace_id")?
      && !table_has_column(conn, "conversations", "task_id")?;

    if recovered {
      conn
        .execute_batch(
          "INSERT INTO \"__new_conversations\"(\"id\", \"task_id\", \"title\", \"created_at\", \"updated_at\")
           SELECT \"id\", \"workspace_id\", \"title\", \"created_at\", \"updated_at\" FROM \"conversations\";
           DROP TABLE \"conversations\";
           ALTER TABLE \"__new_conversations\" RENAME TO \"conversations\";
           CREATE INDEX IF NOT EXISTS \"idx_conversations_task_id\" ON \"conversations\" (\"task_id\");",
        )
        .map_err(|err| err.to_string())?;

      let tag = "0002_lyrical_impossible_man";
      if let Some(migration) = migrations.iter().find(|m| m.tag == tag) {
        if !applied.contains(&migration.hash) {
          conn
            .execute(
              "INSERT INTO \"__drizzle_migrations\" (\"hash\", \"created_at\") VALUES (?1, ?2)",
              params![migration.hash, migration.when],
            )
            .map_err(|err| err.to_string())?;
          applied.insert(migration.hash.clone());
        }
      }
    }

    for migration in migrations {
      if applied.contains(&migration.hash) {
        continue;
      }
      if migration.tag == "0002_lyrical_impossible_man"
        && table_exists(conn, "tasks")?
        && !table_exists(conn, "workspaces")?
        && table_exists(conn, "conversations")?
        && table_has_column(conn, "conversations", "task_id")?
      {
        conn
          .execute(
            "INSERT INTO \"__drizzle_migrations\" (\"hash\", \"created_at\") VALUES (?1, ?2)",
            params![migration.hash, migration.when],
          )
          .map_err(|err| err.to_string())?;
        applied.insert(migration.hash);
        continue;
      }

      for statement in &migration.statements {
        let upper = statement.trim().to_uppercase();
        if upper.starts_with("PRAGMA FOREIGN_KEYS=") {
          continue;
        }
        conn
          .execute_batch(statement)
          .map_err(|err| format!("Migration {} failed: {}", migration.tag, err))?;
      }

      conn
        .execute(
          "INSERT INTO \"__drizzle_migrations\" (\"hash\", \"created_at\") VALUES (?1, ?2)",
          params![migration.hash, migration.when],
        )
        .map_err(|err| err.to_string())?;
      applied.insert(migration.hash);
    }

    Ok(())
  })();

  conn
    .execute_batch("PRAGMA foreign_keys=ON;")
    .map_err(|err| err.to_string())?;

  result
}

pub fn init(app: &tauri::AppHandle) -> Result<DbState, String> {
  if std::env::var("EMDASH_DISABLE_NATIVE_DB").ok().as_deref() == Some("1") {
    return Ok(DbState {
      conn: Mutex::new(None),
      disabled: true,
    });
  }

  let db_path = resolve_database_path(app)?;
  if let Some(parent) = db_path.parent() {
    let _ = fs::create_dir_all(parent);
  }
  let conn = Connection::open(&db_path).map_err(|err| err.to_string())?;

  let migrations_path = resolve_migrations_path(app)
    .ok_or_else(|| "Drizzle migrations folder not found".to_string())?;
  ensure_migrations(&conn, &migrations_path)?;

  Ok(DbState {
    conn: Mutex::new(Some(conn)),
    disabled: false,
  })
}

fn metadata_to_string(meta: Option<Value>) -> Option<String> {
  match meta {
    Some(Value::String(s)) => Some(s),
    Some(Value::Null) | None => None,
    Some(other) => serde_json::to_string(&other).ok(),
  }
}

fn parse_metadata(raw: Option<String>) -> Value {
  match raw {
    Some(text) => serde_json::from_str(&text).unwrap_or(Value::Null),
    None => Value::Null,
  }
}

fn lock_conn(state: &DbState) -> Result<std::sync::MutexGuard<'_, Option<Connection>>, String> {
  state.conn.lock().map_err(|_| "DB lock poisoned".to_string())
}

fn query_project_settings(conn: &Connection, project_id: &str) -> Result<Value, String> {
  let row = conn
    .query_row(
      "SELECT id, name, path, git_remote, git_branch, base_ref FROM projects WHERE id = ?1 LIMIT 1",
      params![project_id],
      |row| {
        let git_remote: Option<String> = row.get(3)?;
        let git_branch: Option<String> = row.get(4)?;
        let base_ref: Option<String> = row.get(5)?;
        let base_ref = compute_base_ref(
          base_ref.as_deref(),
          git_remote.as_deref(),
          git_branch.as_deref(),
        );
        Ok(json!({
          "projectId": row.get::<_, String>(0)?,
          "name": row.get::<_, String>(1)?,
          "path": row.get::<_, String>(2)?,
          "gitRemote": git_remote,
          "gitBranch": git_branch,
          "baseRef": base_ref
        }))
      },
    )
    .optional()
    .map_err(|err| err.to_string())?;

  match row {
    Some(settings) => Ok(settings),
    None => Err("Project not found".to_string()),
  }
}

#[tauri::command]
pub fn db_get_projects(state: tauri::State<DbState>) -> Value {
  if state.disabled {
    return json!([]);
  }
  let guard = match lock_conn(&state) {
    Ok(g) => g,
    Err(_) => return json!([]),
  };
  let conn = match guard.as_ref() {
    Some(conn) => conn,
    None => return json!([]),
  };

  let mut stmt = match conn.prepare(
    "SELECT id, name, path, git_remote, git_branch, base_ref, github_repository, github_connected, created_at, updated_at
     FROM projects
     ORDER BY updated_at DESC",
  ) {
    Ok(stmt) => stmt,
    Err(_) => return json!([]),
  };

  let rows = stmt.query_map([], |row| {
    let git_remote: Option<String> = row.get(3)?;
    let git_branch: Option<String> = row.get(4)?;
    let base_ref: Option<String> = row.get(5)?;
    let github_repo: Option<String> = row.get(6)?;
    let github_connected: Option<i64> = row.get(7)?;
    let base_ref = compute_base_ref(
      base_ref.as_deref(),
      git_remote.as_deref(),
      git_branch.as_deref(),
    );

    Ok(json!({
      "id": row.get::<_, String>(0)?,
      "name": row.get::<_, String>(1)?,
      "path": row.get::<_, String>(2)?,
      "gitInfo": {
        "isGitRepo": git_remote.is_some() || git_branch.is_some(),
        "remote": git_remote,
        "branch": git_branch,
        "baseRef": base_ref
      },
      "githubInfo": github_repo.as_ref().map(|repo| json!({
        "repository": repo,
        "connected": github_connected.unwrap_or(0) != 0
      })),
      "createdAt": row.get::<_, String>(8)?,
      "updatedAt": row.get::<_, String>(9)?
    }))
  });

  match rows {
    Ok(iter) => {
      let mut projects: Vec<Value> = Vec::new();
      for item in iter.flatten() {
        projects.push(item);
      }
      Value::Array(projects)
    }
    Err(_) => json!([]),
  }
}

#[tauri::command]
pub fn db_save_project(state: tauri::State<DbState>, project: Value) -> Value {
  if state.disabled {
    return json!({ "success": true });
  }
  let input: ProjectInput = match serde_json::from_value(project) {
    Ok(input) => input,
    Err(_) => return json!({ "success": false, "error": "Invalid project" }),
  };

  let guard = match lock_conn(&state) {
    Ok(g) => g,
    Err(err) => return json!({ "success": false, "error": err }),
  };
  let conn = match guard.as_ref() {
    Some(conn) => conn,
    None => return json!({ "success": false, "error": "DB not initialized" }),
  };

  let base_ref = compute_base_ref(
    input.git_info.base_ref.as_deref(),
    input.git_info.remote.as_deref(),
    input.git_info.branch.as_deref(),
  );
  let github_repo = input.github_info.as_ref().map(|g| g.repository.clone());
  let github_connected = input.github_info.as_ref().map(|g| if g.connected { 1 } else { 0 });

  let result = conn.execute(
    "INSERT INTO projects (id, name, path, git_remote, git_branch, base_ref, github_repository, github_connected, updated_at)
     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, CURRENT_TIMESTAMP)
     ON CONFLICT(path) DO UPDATE SET
       name=excluded.name,
       git_remote=excluded.git_remote,
       git_branch=excluded.git_branch,
       base_ref=excluded.base_ref,
       github_repository=excluded.github_repository,
       github_connected=excluded.github_connected,
       updated_at=CURRENT_TIMESTAMP",
    params![
      input.id,
      input.name,
      input.path,
      input.git_info.remote,
      input.git_info.branch,
      base_ref,
      github_repo,
      github_connected.unwrap_or(0)
    ],
  );

  match result {
    Ok(_) => json!({ "success": true }),
    Err(err) => json!({ "success": false, "error": err.to_string() }),
  }
}

#[tauri::command]
pub fn db_get_tasks(state: tauri::State<DbState>, project_id: Option<String>) -> Value {
  if state.disabled {
    return json!([]);
  }
  let guard = match lock_conn(&state) {
    Ok(g) => g,
    Err(_) => return json!([]),
  };
  let conn = match guard.as_ref() {
    Some(conn) => conn,
    None => return json!([]),
  };

  let sql = "SELECT id, project_id, name, branch, path, status, agent_id, metadata, created_at, updated_at
       FROM tasks
       WHERE (?1 IS NULL OR project_id = ?1)
       ORDER BY updated_at DESC";

  let mut stmt = match conn.prepare(sql) {
    Ok(stmt) => stmt,
    Err(_) => return json!([]),
  };

  let rows = stmt.query_map(params![project_id], |row| {
    let metadata: Option<String> = row.get(7)?;
    Ok(json!({
      "id": row.get::<_, String>(0)?,
      "projectId": row.get::<_, String>(1)?,
      "name": row.get::<_, String>(2)?,
      "branch": row.get::<_, String>(3)?,
      "path": row.get::<_, String>(4)?,
      "status": row.get::<_, String>(5)?,
      "agentId": row.get::<_, Option<String>>(6)?,
      "metadata": parse_metadata(metadata),
      "createdAt": row.get::<_, String>(8)?,
      "updatedAt": row.get::<_, String>(9)?
    }))
  });

  match rows {
    Ok(iter) => {
      let mut tasks: Vec<Value> = Vec::new();
      for item in iter.flatten() {
        tasks.push(item);
      }
      Value::Array(tasks)
    }
    Err(_) => json!([]),
  }
}

#[tauri::command]
pub fn db_save_task(state: tauri::State<DbState>, task: Value) -> Value {
  if state.disabled {
    return json!({ "success": true });
  }
  let input: TaskInput = match serde_json::from_value(task) {
    Ok(input) => input,
    Err(_) => return json!({ "success": false, "error": "Invalid task" }),
  };

  let guard = match lock_conn(&state) {
    Ok(g) => g,
    Err(err) => return json!({ "success": false, "error": err }),
  };
  let conn = match guard.as_ref() {
    Some(conn) => conn,
    None => return json!({ "success": false, "error": "DB not initialized" }),
  };

  let metadata_value = metadata_to_string(input.metadata);

  let result = conn.execute(
    "INSERT INTO tasks (id, project_id, name, branch, path, status, agent_id, metadata, updated_at)
     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, CURRENT_TIMESTAMP)
     ON CONFLICT(id) DO UPDATE SET
       project_id=excluded.project_id,
       name=excluded.name,
       branch=excluded.branch,
       path=excluded.path,
       status=excluded.status,
       agent_id=excluded.agent_id,
       metadata=excluded.metadata,
       updated_at=CURRENT_TIMESTAMP",
    params![
      input.id,
      input.project_id,
      input.name,
      input.branch,
      input.path,
      input.status,
      input.agent_id,
      metadata_value
    ],
  );

  match result {
    Ok(_) => json!({ "success": true }),
    Err(err) => json!({ "success": false, "error": err.to_string() }),
  }
}

#[tauri::command]
pub fn db_delete_project(state: tauri::State<DbState>, project_id: String) -> Value {
  if state.disabled {
    return json!({ "success": true });
  }
  let guard = match lock_conn(&state) {
    Ok(g) => g,
    Err(err) => return json!({ "success": false, "error": err }),
  };
  let conn = match guard.as_ref() {
    Some(conn) => conn,
    None => return json!({ "success": false, "error": "DB not initialized" }),
  };

  match conn.execute("DELETE FROM projects WHERE id = ?1", params![project_id]) {
    Ok(_) => json!({ "success": true }),
    Err(err) => json!({ "success": false, "error": err.to_string() }),
  }
}

#[tauri::command]
pub fn db_delete_task(state: tauri::State<DbState>, task_id: String) -> Value {
  if state.disabled {
    return json!({ "success": true });
  }
  let guard = match lock_conn(&state) {
    Ok(g) => g,
    Err(err) => return json!({ "success": false, "error": err }),
  };
  let conn = match guard.as_ref() {
    Some(conn) => conn,
    None => return json!({ "success": false, "error": "DB not initialized" }),
  };

  match conn.execute("DELETE FROM tasks WHERE id = ?1", params![task_id]) {
    Ok(_) => json!({ "success": true }),
    Err(err) => json!({ "success": false, "error": err.to_string() }),
  }
}

#[tauri::command]
pub fn db_save_conversation(state: tauri::State<DbState>, conversation: Value) -> Value {
  if state.disabled {
    return json!({ "success": true });
  }
  let input: ConversationInput = match serde_json::from_value(conversation) {
    Ok(input) => input,
    Err(_) => return json!({ "success": false, "error": "Invalid conversation" }),
  };

  let guard = match lock_conn(&state) {
    Ok(g) => g,
    Err(err) => return json!({ "success": false, "error": err }),
  };
  let conn = match guard.as_ref() {
    Some(conn) => conn,
    None => return json!({ "success": false, "error": "DB not initialized" }),
  };

  let result = conn.execute(
    "INSERT INTO conversations (id, task_id, title, updated_at)
     VALUES (?1, ?2, ?3, CURRENT_TIMESTAMP)
     ON CONFLICT(id) DO UPDATE SET
       title=excluded.title,
       updated_at=CURRENT_TIMESTAMP",
    params![input.id, input.task_id, input.title],
  );

  match result {
    Ok(_) => json!({ "success": true }),
    Err(err) => json!({ "success": false, "error": err.to_string() }),
  }
}

#[tauri::command]
pub fn db_get_conversations(state: tauri::State<DbState>, task_id: String) -> Value {
  if state.disabled {
    return json!({ "success": true, "conversations": [] });
  }
  let guard = match lock_conn(&state) {
    Ok(g) => g,
    Err(err) => return json!({ "success": false, "error": err }),
  };
  let conn = match guard.as_ref() {
    Some(conn) => conn,
    None => return json!({ "success": true, "conversations": [] }),
  };

  let mut stmt = match conn.prepare(
    "SELECT id, task_id, title, created_at, updated_at
     FROM conversations
     WHERE task_id = ?1
     ORDER BY updated_at DESC",
  ) {
    Ok(stmt) => stmt,
    Err(err) => return json!({ "success": false, "error": err.to_string() }),
  };

  let rows = stmt.query_map(params![task_id], |row| {
    Ok(json!({
      "id": row.get::<_, String>(0)?,
      "taskId": row.get::<_, String>(1)?,
      "title": row.get::<_, String>(2)?,
      "createdAt": row.get::<_, String>(3)?,
      "updatedAt": row.get::<_, String>(4)?
    }))
  });

  match rows {
    Ok(iter) => {
      let mut conversations: Vec<Value> = Vec::new();
      for item in iter.flatten() {
        conversations.push(item);
      }
      json!({ "success": true, "conversations": conversations })
    }
    Err(err) => json!({ "success": false, "error": err.to_string() }),
  }
}

#[tauri::command]
pub fn db_get_or_create_default_conversation(
  state: tauri::State<DbState>,
  task_id: String,
) -> Value {
  if state.disabled {
    let now = chrono::Utc::now().to_rfc3339();
    return json!({
      "success": true,
      "conversation": {
        "id": format!("conv-{}-default", task_id),
        "taskId": task_id,
        "title": "Default Conversation",
        "createdAt": now,
        "updatedAt": now
      }
    });
  }
  let guard = match lock_conn(&state) {
    Ok(g) => g,
    Err(err) => return json!({ "success": false, "error": err }),
  };
  let conn = match guard.as_ref() {
    Some(conn) => conn,
    None => return json!({ "success": false, "error": "DB not initialized" }),
  };

  let existing: Option<Value> = conn
    .query_row(
      "SELECT id, task_id, title, created_at, updated_at
       FROM conversations
       WHERE task_id = ?1
       ORDER BY created_at ASC
       LIMIT 1",
      params![task_id],
      |row| {
        Ok(json!({
          "id": row.get::<_, String>(0)?,
          "taskId": row.get::<_, String>(1)?,
          "title": row.get::<_, String>(2)?,
          "createdAt": row.get::<_, String>(3)?,
          "updatedAt": row.get::<_, String>(4)?
        }))
      },
    )
    .optional()
    .map_err(|err| err.to_string())
    .ok()
    .flatten();

  if let Some(conversation) = existing {
    return json!({ "success": true, "conversation": conversation });
  }

  let conversation_id = format!("conv-{}-{}", task_id, now_millis());
  if let Err(err) = conn.execute(
    "INSERT INTO conversations (id, task_id, title, updated_at)
     VALUES (?1, ?2, ?3, CURRENT_TIMESTAMP)",
    params![conversation_id, task_id, "Default Conversation"],
  ) {
    return json!({ "success": false, "error": err.to_string() });
  }

  let created: Option<Value> = conn
    .query_row(
      "SELECT id, task_id, title, created_at, updated_at
       FROM conversations
       WHERE id = ?1
       LIMIT 1",
      params![conversation_id],
      |row| {
        Ok(json!({
          "id": row.get::<_, String>(0)?,
          "taskId": row.get::<_, String>(1)?,
          "title": row.get::<_, String>(2)?,
          "createdAt": row.get::<_, String>(3)?,
          "updatedAt": row.get::<_, String>(4)?
        }))
      },
    )
    .optional()
    .map_err(|err| err.to_string())
    .ok()
    .flatten();

  if let Some(conversation) = created {
    json!({ "success": true, "conversation": conversation })
  } else {
    let now = chrono::Utc::now().to_rfc3339();
    json!({
      "success": true,
      "conversation": {
        "id": conversation_id,
        "taskId": task_id,
        "title": "Default Conversation",
        "createdAt": now,
        "updatedAt": now
      }
    })
  }
}

#[tauri::command]
pub fn db_save_message(state: tauri::State<DbState>, message: Value) -> Value {
  if state.disabled {
    return json!({ "success": true });
  }
  let input: MessageInput = match serde_json::from_value(message) {
    Ok(input) => input,
    Err(_) => return json!({ "success": false, "error": "Invalid message" }),
  };

  let mut guard = match lock_conn(&state) {
    Ok(g) => g,
    Err(err) => return json!({ "success": false, "error": err }),
  };
  let conn = match guard.as_mut() {
    Some(conn) => conn,
    None => return json!({ "success": false, "error": "DB not initialized" }),
  };

  let meta = metadata_to_string(input.metadata);
  let tx = match conn.transaction() {
    Ok(tx) => tx,
    Err(err) => return json!({ "success": false, "error": err.to_string() }),
  };

  if let Err(err) = tx.execute(
    "INSERT INTO messages (id, conversation_id, content, sender, metadata, timestamp)
     VALUES (?1, ?2, ?3, ?4, ?5, CURRENT_TIMESTAMP)
     ON CONFLICT(id) DO NOTHING",
    params![
      input.id,
      input.conversation_id,
      input.content,
      input.sender,
      meta
    ],
  ) {
    return json!({ "success": false, "error": err.to_string() });
  }

  if let Err(err) = tx.execute(
    "UPDATE conversations SET updated_at = CURRENT_TIMESTAMP WHERE id = ?1",
    params![input.conversation_id],
  ) {
    return json!({ "success": false, "error": err.to_string() });
  }

  if let Err(err) = tx.commit() {
    return json!({ "success": false, "error": err.to_string() });
  }

  json!({ "success": true })
}

#[tauri::command]
pub fn db_get_messages(state: tauri::State<DbState>, conversation_id: String) -> Value {
  if state.disabled {
    return json!({ "success": true, "messages": [] });
  }
  let guard = match lock_conn(&state) {
    Ok(g) => g,
    Err(err) => return json!({ "success": false, "error": err }),
  };
  let conn = match guard.as_ref() {
    Some(conn) => conn,
    None => return json!({ "success": true, "messages": [] }),
  };

  let mut stmt = match conn.prepare(
    "SELECT id, conversation_id, content, sender, timestamp, metadata
     FROM messages
     WHERE conversation_id = ?1
     ORDER BY timestamp ASC",
  ) {
    Ok(stmt) => stmt,
    Err(err) => return json!({ "success": false, "error": err.to_string() }),
  };

  let rows = stmt.query_map(params![conversation_id], |row| {
    let metadata: Option<String> = row.get(5)?;
    Ok(json!({
      "id": row.get::<_, String>(0)?,
      "conversationId": row.get::<_, String>(1)?,
      "content": row.get::<_, String>(2)?,
      "sender": row.get::<_, String>(3)?,
      "timestamp": row.get::<_, String>(4)?,
      "metadata": parse_metadata(metadata)
    }))
  });

  match rows {
    Ok(iter) => {
      let mut messages: Vec<Value> = Vec::new();
      for item in iter.flatten() {
        messages.push(item);
      }
      json!({ "success": true, "messages": messages })
    }
    Err(err) => json!({ "success": false, "error": err.to_string() }),
  }
}

#[tauri::command]
pub fn db_delete_conversation(state: tauri::State<DbState>, conversation_id: String) -> Value {
  if state.disabled {
    return json!({ "success": true });
  }
  let guard = match lock_conn(&state) {
    Ok(g) => g,
    Err(err) => return json!({ "success": false, "error": err }),
  };
  let conn = match guard.as_ref() {
    Some(conn) => conn,
    None => return json!({ "success": false, "error": "DB not initialized" }),
  };

  match conn.execute("DELETE FROM conversations WHERE id = ?1", params![conversation_id]) {
    Ok(_) => json!({ "success": true }),
    Err(err) => json!({ "success": false, "error": err.to_string() }),
  }
}

#[tauri::command]
pub fn project_settings_get(state: tauri::State<DbState>, project_id: String) -> Value {
  if state.disabled {
    return json!({ "success": false, "error": "DB disabled" });
  }
  let guard = match lock_conn(&state) {
    Ok(g) => g,
    Err(err) => return json!({ "success": false, "error": err }),
  };
  let conn = match guard.as_ref() {
    Some(conn) => conn,
    None => return json!({ "success": false, "error": "DB not initialized" }),
  };

  match query_project_settings(conn, &project_id) {
    Ok(settings) => json!({ "success": true, "settings": settings }),
    Err(err) => json!({ "success": false, "error": err }),
  }
}

#[tauri::command]
pub fn project_settings_update(
  state: tauri::State<DbState>,
  args: ProjectSettingsUpdate,
) -> Value {
  if state.disabled {
    return json!({ "success": false, "error": "DB disabled" });
  }
  if args.base_ref.trim().is_empty() {
    return json!({ "success": false, "error": "baseRef is required" });
  }
  let guard = match lock_conn(&state) {
    Ok(g) => g,
    Err(err) => return json!({ "success": false, "error": err }),
  };
  let conn = match guard.as_ref() {
    Some(conn) => conn,
    None => return json!({ "success": false, "error": "DB not initialized" }),
  };

  let row = conn
    .query_row(
      "SELECT git_remote, git_branch FROM projects WHERE id = ?1 LIMIT 1",
      params![args.project_id],
      |row| Ok((row.get::<_, Option<String>>(0)?, row.get::<_, Option<String>>(1)?)),
    )
    .optional();

  let (git_remote, git_branch) = match row {
    Ok(Some(data)) => data,
    Ok(None) => return json!({ "success": false, "error": "Project not found" }),
    Err(err) => return json!({ "success": false, "error": err.to_string() }),
  };

  let normalized = compute_base_ref(
    Some(args.base_ref.as_str()),
    git_remote.as_deref(),
    git_branch.as_deref(),
  );

  if let Err(err) = conn.execute(
    "UPDATE projects SET base_ref = ?1, updated_at = CURRENT_TIMESTAMP WHERE id = ?2",
    params![normalized, args.project_id],
  ) {
    return json!({ "success": false, "error": err.to_string() });
  }

  match query_project_settings(conn, &args.project_id) {
    Ok(settings) => json!({ "success": true, "settings": settings }),
    Err(err) => json!({ "success": false, "error": err }),
  }
}
