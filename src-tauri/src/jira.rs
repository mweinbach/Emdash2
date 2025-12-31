use crate::storage;
use crate::runtime::run_blocking;
use base64::{engine::general_purpose::STANDARD, Engine as _};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::fs;
use std::path::PathBuf;

const SERVICE_NAME: &str = "emdash-jira";
const ACCOUNT_NAME: &str = "api-token";
const CONFIG_FILE: &str = "jira.json";

#[derive(Debug, Clone, Deserialize, Serialize)]
struct JiraCreds {
  site_url: String,
  email: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct JiraSaveArgs {
  site_url: String,
  email: String,
  token: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct JiraSearchArgs {
  search_term: String,
  limit: Option<u32>,
}

fn config_path(app: &tauri::AppHandle) -> PathBuf {
  storage::config_file(app, CONFIG_FILE)
}

fn read_creds(app: &tauri::AppHandle) -> Option<JiraCreds> {
  let path = config_path(app);
  let raw = fs::read_to_string(path).ok()?;
  let value: Value = serde_json::from_str(&raw).ok()?;
  let site_url = value.get("siteUrl").and_then(|v| v.as_str()).unwrap_or("").trim();
  let email = value.get("email").and_then(|v| v.as_str()).unwrap_or("").trim();
  if site_url.is_empty() || email.is_empty() {
    return None;
  }
  Some(JiraCreds {
    site_url: site_url.to_string(),
    email: email.to_string(),
  })
}

fn write_creds(app: &tauri::AppHandle, creds: &JiraCreds) -> Result<(), String> {
  let path = config_path(app);
  if let Some(parent) = path.parent() {
    fs::create_dir_all(parent).map_err(|err| err.to_string())?;
  }
  let data = json!({ "siteUrl": creds.site_url, "email": creds.email });
  fs::write(path, data.to_string()).map_err(|err| err.to_string())
}

fn clear_creds(app: &tauri::AppHandle) {
  let path = config_path(app);
  let _ = fs::remove_file(path);
}

fn keyring_entry() -> Result<keyring::Entry, String> {
  keyring::Entry::new(SERVICE_NAME, ACCOUNT_NAME).map_err(|err| err.to_string())
}

fn store_token(token: &str) -> Result<(), String> {
  let entry = keyring_entry()?;
  entry.set_password(token).map_err(|err| err.to_string())
}

fn get_token() -> Result<Option<String>, String> {
  let entry = keyring_entry()?;
  match entry.get_password() {
    Ok(token) => Ok(Some(token)),
    Err(keyring::Error::NoEntry) => Ok(None),
    Err(err) => Err(err.to_string()),
  }
}

fn clear_token() -> Result<(), String> {
  let entry = keyring_entry()?;
  match entry.delete_password() {
    Ok(_) => Ok(()),
    Err(keyring::Error::NoEntry) => Ok(()),
    Err(err) => Err(err.to_string()),
  }
}

fn encode_basic(email: &str, token: &str) -> String {
  let raw = format!("{}:{}", email, token);
  STANDARD.encode(raw.as_bytes())
}

fn build_url(base: &str, path: &str) -> String {
  format!("{}{}", base.trim_end_matches('/'), path)
}

fn do_request(
  url: &str,
  email: &str,
  token: &str,
  method: &str,
  payload: Option<&str>,
  extra_headers: Option<Vec<(&str, &str)>>,
) -> Result<String, String> {
  let auth = encode_basic(email, token);
  let mut req = ureq::request(method, url)
    .set("Authorization", &format!("Basic {}", auth))
    .set("Accept", "application/json");

  if let Some(headers) = extra_headers {
    for (k, v) in headers {
      req = req.set(k, v);
    }
  }

  let response = if let Some(body) = payload {
    req.send_string(body)
  } else {
    req.call()
  };

  match response {
    Ok(resp) => resp.into_string().map_err(|err| err.to_string()),
    Err(ureq::Error::Status(code, resp)) => {
      let snippet = resp.into_string().unwrap_or_default();
      let snippet = snippet.chars().take(200).collect::<String>();
      let suffix = if snippet.is_empty() { "" } else { ": " };
      Err(format!("Jira API error {}{}{}", code, suffix, snippet))
    }
    Err(err) => Err(err.to_string()),
  }
}

fn get_myself(site_url: &str, email: &str, token: &str) -> Result<Value, String> {
  let url = build_url(site_url, "/rest/api/3/myself");
  let body = do_request(&url, email, token, "GET", None, None)?;
  let data: Value = serde_json::from_str(&body).map_err(|err| err.to_string())?;
  if data.get("errorMessages").is_some() {
    return Err("Failed to verify Jira token.".to_string());
  }
  Ok(data)
}

fn search_raw(site_url: &str, email: &str, token: &str, jql: &str, limit: u32) -> Result<Vec<Value>, String> {
  let url = build_url(site_url, "/rest/api/3/search");
  let payload = json!({
    "jql": jql,
    "maxResults": limit.clamp(1, 100),
    "fields": ["summary", "updated", "project", "status", "assignee"]
  })
  .to_string();

  let body = do_request(
    &url,
    email,
    token,
    "POST",
    Some(&payload),
    Some(vec![("Content-Type", "application/json")]),
  )?;
  let data: Value = serde_json::from_str(&body).map_err(|err| err.to_string())?;
  Ok(data
    .get("issues")
    .and_then(|v| v.as_array())
    .cloned()
    .unwrap_or_default())
}

fn get_issue_by_key(site_url: &str, email: &str, token: &str, key: &str) -> Result<Option<Value>, String> {
  let url = build_url(site_url, &format!("/rest/api/3/issue/{}?fields=summary,updated,project,status,assignee", key));
  let body = do_request(&url, email, token, "GET", None, None)?;
  let data: Value = serde_json::from_str(&body).map_err(|err| err.to_string())?;
  if data.get("errorMessages").is_some() {
    return Ok(None);
  }
  Ok(Some(data))
}

fn get_recent_issue_keys(
  site_url: &str,
  email: &str,
  token: &str,
  limit: u32,
) -> Result<Vec<String>, String> {
  let url = build_url(site_url, "/rest/api/3/issue/picker?query=&currentJQL=");
  let body = do_request(&url, email, token, "GET", None, None)?;
  let data: Value = serde_json::from_str(&body).map_err(|err| err.to_string())?;
  let mut keys = Vec::new();
  if let Some(sections) = data.get("sections").and_then(|v| v.as_array()) {
    for sec in sections {
      if let Some(issues) = sec.get("issues").and_then(|v| v.as_array()) {
        for issue in issues {
          if let Some(key) = issue.get("key").and_then(|v| v.as_str()) {
            if !keys.contains(&key.to_string()) {
              keys.push(key.to_string());
            }
            if keys.len() >= limit as usize {
              return Ok(keys);
            }
          }
        }
      }
    }
  }
  Ok(keys)
}

fn normalize_issues(site_url: &str, raw: Vec<Value>) -> Vec<Value> {
  let base = site_url.trim_end_matches('/');
  raw
    .into_iter()
    .map(|it| {
      let fields = it.get("fields").cloned().unwrap_or(Value::Null);
      json!({
        "id": it.get("id").and_then(|v| v.as_str()).unwrap_or(it.get("key").and_then(|v| v.as_str()).unwrap_or("")),
        "key": it.get("key").and_then(|v| v.as_str()).unwrap_or(""),
        "summary": fields.get("summary").and_then(|v| v.as_str()).unwrap_or(""),
        "description": Value::Null,
        "url": format!("{}/browse/{}", base, it.get("key").and_then(|v| v.as_str()).unwrap_or("")),
        "status": fields.get("status").map(|status| json!({ "name": status.get("name").and_then(|v| v.as_str()).unwrap_or("") })),
        "project": fields.get("project").map(|project| json!({
          "key": project.get("key").and_then(|v| v.as_str()).unwrap_or(""),
          "name": project.get("name").and_then(|v| v.as_str()).unwrap_or(""),
        })),
        "assignee": fields.get("assignee").map(|assignee| json!({
          "displayName": assignee.get("displayName").and_then(|v| v.as_str()).unwrap_or(""),
          "name": assignee.get("name").and_then(|v| v.as_str()).unwrap_or(""),
        })),
        "updatedAt": fields.get("updated").cloned().unwrap_or(Value::Null),
      })
    })
    .collect()
}

fn looks_like_key(term: &str) -> bool {
  let mut parts = term.split('-');
  let prefix = match parts.next() {
    Some(p) if !p.is_empty() => p,
    _ => return false,
  };
  let suffix = match parts.next() {
    Some(s) if !s.is_empty() => s,
    _ => return false,
  };
  if parts.next().is_some() {
    return false;
  }
  if !prefix.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
    return false;
  }
  suffix.chars().all(|c| c.is_ascii_digit())
}

fn require_auth(app: &tauri::AppHandle) -> Result<(JiraCreds, String), String> {
  let creds = read_creds(app).ok_or_else(|| "Jira credentials not set.".to_string())?;
  let token = get_token()?.ok_or_else(|| "Jira token not found.".to_string())?;
  Ok((creds, token))
}

#[tauri::command]
pub async fn jira_save_credentials(app: tauri::AppHandle, args: JiraSaveArgs) -> Value {
  run_blocking(
    json!({ "success": false, "error": "Task cancelled" }),
    move || {
      let site = args.site_url.trim();
      let email = args.email.trim();
      let token = args.token.trim();
      if site.is_empty() || email.is_empty() || token.is_empty() {
        return json!({ "success": false, "error": "Site URL, email, and API token are required." });
      }

      match get_myself(site, email, token) {
        Ok(me) => {
          if let Err(err) = store_token(token) {
            return json!({ "success": false, "error": err });
          }
          if let Err(err) = write_creds(&app, &JiraCreds { site_url: site.to_string(), email: email.to_string() }) {
            return json!({ "success": false, "error": err });
          }
          json!({ "success": true, "displayName": me.get("displayName").and_then(|v| v.as_str()).unwrap_or("") })
        }
        Err(err) => json!({ "success": false, "error": err }),
      }
    },
  )
  .await
}

#[tauri::command]
pub async fn jira_clear_credentials(app: tauri::AppHandle) -> Value {
  run_blocking(
    json!({ "success": false, "error": "Task cancelled" }),
    move || {
      let _ = clear_token();
      clear_creds(&app);
      json!({ "success": true })
    },
  )
  .await
}

#[tauri::command]
pub async fn jira_check_connection(app: tauri::AppHandle) -> Value {
  run_blocking(
    json!({ "connected": false }),
    move || {
      let creds = match read_creds(&app) {
        Some(c) => c,
        None => return json!({ "connected": false }),
      };
      let token = match get_token() {
        Ok(Some(t)) => t,
        Ok(None) => return json!({ "connected": false }),
        Err(err) => return json!({ "connected": false, "error": err }),
      };

      match get_myself(&creds.site_url, &creds.email, &token) {
        Ok(me) => json!({
          "connected": true,
          "accountId": me.get("accountId").and_then(|v| v.as_str()),
          "displayName": me.get("displayName").and_then(|v| v.as_str()),
          "siteUrl": creds.site_url,
        }),
        Err(err) => json!({ "connected": false, "error": err }),
      }
    },
  )
  .await
}

#[tauri::command]
pub async fn jira_initial_fetch(app: tauri::AppHandle, limit: Option<u32>) -> Value {
  run_blocking(
    json!({ "success": false, "error": "Task cancelled" }),
    move || {
      let (creds, token) = match require_auth(&app) {
        Ok(res) => res,
        Err(err) => return json!({ "success": false, "error": err }),
      };
      let limit = limit.unwrap_or(50).clamp(1, 100);
      let jql_candidates = vec![
        "assignee = currentUser() ORDER BY updated DESC",
        "reporter = currentUser() ORDER BY updated DESC",
        "ORDER BY updated DESC",
      ];

      for jql in jql_candidates {
        if let Ok(issues) = search_raw(&creds.site_url, &creds.email, &token, jql, limit) {
          if !issues.is_empty() {
            return json!({ "success": true, "issues": normalize_issues(&creds.site_url, issues) });
          }
        }
      }

      if let Ok(keys) = get_recent_issue_keys(&creds.site_url, &creds.email, &token, limit) {
        if !keys.is_empty() {
          let mut results = Vec::new();
          for key in keys.into_iter().take(limit as usize) {
            if let Ok(Some(issue)) = get_issue_by_key(&creds.site_url, &creds.email, &token, &key) {
              results.push(issue);
            }
          }
          if !results.is_empty() {
            return json!({ "success": true, "issues": normalize_issues(&creds.site_url, results) });
          }
        }
      }

      json!({ "success": true, "issues": [] })
    },
  )
  .await
}

#[tauri::command]
pub async fn jira_search_issues(app: tauri::AppHandle, args: JiraSearchArgs) -> Value {
  run_blocking(
    json!({ "success": false, "error": "Task cancelled" }),
    move || {
      let term = args.search_term.trim();
      if term.is_empty() {
        return json!({ "success": true, "issues": [] });
      }

      let (creds, token) = match require_auth(&app) {
        Ok(res) => res,
        Err(err) => return json!({ "success": false, "error": err }),
      };
      let limit = args.limit.unwrap_or(20).clamp(1, 100);

      if looks_like_key(term) {
        let key_upper = term.to_uppercase();
        if let Ok(Some(issue)) = get_issue_by_key(&creds.site_url, &creds.email, &token, &key_upper) {
          return json!({ "success": true, "issues": normalize_issues(&creds.site_url, vec![issue]) });
        }
      }

      let sanitized = term.replace('"', "\\\"");
      let extra_key = if looks_like_key(term) {
        format!(" OR issueKey = {}", term.to_uppercase())
      } else {
        String::new()
      };
      let jql = format!("text ~ \"{}\"{}", sanitized, extra_key);
      match search_raw(&creds.site_url, &creds.email, &token, &jql, limit) {
        Ok(issues) => json!({ "success": true, "issues": normalize_issues(&creds.site_url, issues) }),
        Err(err) => json!({ "success": false, "error": err }),
      }
    },
  )
  .await
}
