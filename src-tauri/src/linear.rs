use crate::telemetry;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

const LINEAR_API_URL: &str = "https://api.linear.app/graphql";
const SERVICE_NAME: &str = "emdash-linear";
const ACCOUNT_NAME: &str = "api-token";

#[derive(Debug, Deserialize)]
struct GraphQLError {
  message: Option<String>,
}

#[derive(Debug, Deserialize)]
struct GraphQLResponse<T> {
  data: Option<T>,
  errors: Option<Vec<GraphQLError>>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct LinearViewer {
  name: Option<String>,
  display_name: Option<String>,
  organization: Option<LinearOrg>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
struct LinearOrg {
  name: Option<String>,
}

#[derive(Debug, Deserialize)]
struct LinearIssuesResponse {
  issues: Option<LinearIssuesNodes>,
}

#[derive(Debug, Deserialize)]
struct LinearIssuesNodes {
  nodes: Option<Vec<Value>>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LinearSearchArgs {
  search_term: String,
  limit: Option<u32>,
}

fn keyring_entry() -> Result<keyring::Entry, String> {
  keyring::Entry::new(SERVICE_NAME, ACCOUNT_NAME).map_err(|err| err.to_string())
}

fn get_token() -> Result<Option<String>, String> {
  let entry = keyring_entry()?;
  match entry.get_password() {
    Ok(token) => Ok(Some(token)),
    Err(keyring::Error::NoEntry) => Ok(None),
    Err(err) => Err(err.to_string()),
  }
}

fn store_token(token: &str) -> Result<(), String> {
  let entry = keyring_entry()?;
  entry.set_password(token).map_err(|err| err.to_string())
}

fn clear_token() -> Result<(), String> {
  let entry = keyring_entry()?;
  match entry.delete_password() {
    Ok(_) => Ok(()),
    Err(keyring::Error::NoEntry) => Ok(()),
    Err(err) => Err(err.to_string()),
  }
}

fn graphql<T: for<'de> Deserialize<'de>>(
  token: &str,
  query: &str,
  variables: Option<Value>,
) -> Result<T, String> {
  let body = json!({
    "query": query,
    "variables": variables
  })
  .to_string();

  let response = ureq::post(LINEAR_API_URL)
    .set("Content-Type", "application/json")
    .set("Authorization", token)
    .send_string(&body);

  let response = response.map_err(|err| err.to_string())?;
  let text = response.into_string().map_err(|err| err.to_string())?;
  let parsed: GraphQLResponse<T> = serde_json::from_str(&text).map_err(|err| err.to_string())?;

  if let Some(errors) = parsed.errors {
    if let Some(err) = errors.into_iter().filter_map(|e| e.message).next() {
      return Err(err);
    }
  }

  parsed.data.ok_or_else(|| "No data returned from Linear API".to_string())
}

fn fetch_viewer(token: &str) -> Result<LinearViewer, String> {
  let query = r#"
    query ViewerInfo {
      viewer {
        name
        displayName
        organization { name }
      }
    }
  "#;
  #[derive(Debug, Deserialize)]
  struct ViewerResponse {
    viewer: Option<LinearViewer>,
  }
  let data: ViewerResponse = graphql(token, query, None)?;
  data
    .viewer
    .ok_or_else(|| "Unable to retrieve Linear account information.".to_string())
}

fn normalize_issues(raw: Vec<Value>) -> Vec<Value> {
  raw
    .into_iter()
    .filter(|issue| {
      let state_type = issue
        .get("state")
        .and_then(|v| v.get("type"))
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_lowercase();
      let state_name = issue
        .get("state")
        .and_then(|v| v.get("name"))
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_lowercase();
      if state_type == "completed" || state_type == "canceled" {
        return false;
      }
      if state_name == "done"
        || state_name == "completed"
        || state_name == "canceled"
        || state_name == "cancelled"
      {
        return false;
      }
      true
    })
    .collect()
}

#[tauri::command]
pub fn linear_save_token(app: tauri::AppHandle, token: String) -> Value {
  let trimmed = token.trim();
  if trimmed.is_empty() {
    return json!({ "success": false, "error": "A Linear API token is required." });
  }

  match fetch_viewer(trimmed) {
    Ok(viewer) => {
      if let Err(err) = store_token(trimmed) {
        return json!({ "success": false, "error": err });
      }
      let workspace = viewer
        .organization
        .as_ref()
        .and_then(|org| org.name.clone())
        .or_else(|| viewer.display_name.clone())
        .or_else(|| viewer.name.clone());

      let _ = telemetry::capture(&app, "linear_connected".to_string(), None);

      json!({
        "success": true,
        "workspaceName": workspace,
        "taskName": workspace,
      })
    }
    Err(err) => json!({ "success": false, "error": err }),
  }
}

#[tauri::command]
pub fn linear_clear_token(app: tauri::AppHandle) -> Value {
  match clear_token() {
    Ok(_) => {
      let _ = telemetry::capture(&app, "linear_disconnected".to_string(), None);
      json!({ "success": true })
    }
    Err(err) => json!({ "success": false, "error": err }),
  }
}

#[tauri::command]
pub fn linear_check_connection() -> Value {
  let token = match get_token() {
    Ok(Some(token)) => token,
    Ok(None) => return json!({ "connected": false }),
    Err(err) => return json!({ "connected": false, "error": err }),
  };

  match fetch_viewer(&token) {
    Ok(viewer) => {
      let workspace = viewer
        .organization
        .as_ref()
        .and_then(|org| org.name.clone())
        .or_else(|| viewer.display_name.clone())
        .or_else(|| viewer.name.clone());
      json!({
        "connected": true,
        "workspaceName": workspace,
        "taskName": workspace,
        "viewer": viewer,
      })
    }
    Err(err) => json!({ "connected": false, "error": err }),
  }
}

#[tauri::command]
pub fn linear_initial_fetch(limit: Option<u32>) -> Value {
  let token = match get_token() {
    Ok(Some(token)) => token,
    Ok(None) => return json!({ "success": false, "error": "Linear token not set." }),
    Err(err) => return json!({ "success": false, "error": err }),
  };

  let sanitized_limit = limit.unwrap_or(50).clamp(1, 200) as i64;
  let query = r#"
    query ListIssues($limit: Int!) {
      issues(first: $limit, orderBy: updatedAt) {
        nodes {
          id
          identifier
          title
          description
          url
          state { name type }
          team { name key }
          project { name }
          assignee { displayName name }
          updatedAt
        }
      }
    }
  "#;

  let data: Result<LinearIssuesResponse, String> =
    graphql(&token, query, Some(json!({ "limit": sanitized_limit })));

  match data {
    Ok(resp) => {
      let nodes = resp
        .issues
        .and_then(|issues| issues.nodes)
        .unwrap_or_default();
      let open = normalize_issues(nodes);
      json!({ "success": true, "issues": open })
    }
    Err(err) => json!({ "success": false, "error": err }),
  }
}

#[tauri::command]
pub fn linear_search_issues(args: LinearSearchArgs) -> Value {
  let term = args.search_term.trim();
  if term.is_empty() {
    return json!({ "success": false, "error": "Search term is required." });
  }

  let token = match get_token() {
    Ok(Some(token)) => token,
    Ok(None) => return json!({ "success": false, "error": "Linear token not set." }),
    Err(err) => return json!({ "success": false, "error": err }),
  };

  let sanitized_limit = args.limit.unwrap_or(20).clamp(1, 200) as i64;
  let query = r#"
    query ListAllIssues($limit: Int!) {
      issues(first: $limit, orderBy: updatedAt) {
        nodes {
          id
          identifier
          title
          description
          url
          state { name type }
          team { name key }
          project { name }
          assignee { displayName name }
          updatedAt
        }
      }
    }
  "#;

  let data: Result<LinearIssuesResponse, String> =
    graphql(&token, query, Some(json!({ "limit": 100 })));

  match data {
    Ok(resp) => {
      let nodes = resp
        .issues
        .and_then(|issues| issues.nodes)
        .unwrap_or_default();
      let open = normalize_issues(nodes);
      let term_lower = term.to_lowercase();
      let filtered: Vec<Value> = open
        .into_iter()
        .filter(|issue| {
          let id = issue.get("identifier").and_then(|v| v.as_str()).unwrap_or("");
          let title = issue.get("title").and_then(|v| v.as_str()).unwrap_or("");
          let assignee = issue
            .get("assignee")
            .and_then(|v| v.get("name"))
            .and_then(|v| v.as_str())
            .unwrap_or("");
          let assignee_display = issue
            .get("assignee")
            .and_then(|v| v.get("displayName"))
            .and_then(|v| v.as_str())
            .unwrap_or("");
          id.to_lowercase().contains(&term_lower)
            || title.to_lowercase().contains(&term_lower)
            || assignee.to_lowercase().contains(&term_lower)
            || assignee_display.to_lowercase().contains(&term_lower)
        })
        .take(sanitized_limit as usize)
        .collect();
      json!({ "success": true, "issues": filtered })
    }
    Err(err) => json!({ "success": false, "error": err }),
  }
}
