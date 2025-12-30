use serde::Deserialize;
use serde_json::json;
use std::fs;
use std::io::Write;
use std::path::Path;

#[derive(Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct DebugLogOptions {
  reset: Option<bool>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DebugLogArgs {
  file_path: String,
  content: String,
  options: Option<DebugLogOptions>,
}

#[tauri::command]
pub fn debug_append_log(args: DebugLogArgs) -> serde_json::Value {
  let path = args.file_path.trim();
  if path.is_empty() {
    return json!({ "success": false, "error": "filePath is required" });
  }

  let file_path = Path::new(path);
  if let Some(parent) = file_path.parent() {
    if let Err(err) = fs::create_dir_all(parent) {
      return json!({ "success": false, "error": err.to_string() });
    }
  }

  let reset = args.options.and_then(|o| o.reset).unwrap_or(false);
  let result = if reset {
    fs::File::create(file_path)
      .and_then(|mut file| file.write_all(args.content.as_bytes()))
      .map_err(|err| err.to_string())
  } else {
    fs::OpenOptions::new()
      .create(true)
      .append(true)
      .open(file_path)
      .and_then(|mut file| file.write_all(args.content.as_bytes()))
      .map_err(|err| err.to_string())
  };

  match result {
    Ok(_) => json!({ "success": true }),
    Err(err) => json!({ "success": false, "error": err }),
  }
}
