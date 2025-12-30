use serde::Deserialize;
use serde_json::{json, Value};
use std::sync::{Arc, Mutex};
use tauri::webview::{PageLoadEvent, WebviewBuilder};
use tauri::{AppHandle, Emitter, Manager, Webview, WebviewUrl, Window};

const BROWSER_VIEW_LABEL: &str = "browser-preview";

#[derive(Clone, Default)]
pub struct BrowserViewState {
  visible: Arc<Mutex<bool>>,
}

impl BrowserViewState {
  pub fn new() -> Self {
    Self {
      visible: Arc::new(Mutex::new(false)),
    }
  }
}

#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct BrowserBounds {
  pub x: f64,
  pub y: f64,
  pub width: f64,
  pub height: f64,
}

#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct BrowserLoadArgs {
  pub url: String,
  pub force_reload: Option<bool>,
}

fn emit_event(app: &AppHandle, payload: Value) {
  let _ = app.emit("browser:view:event", payload);
}

fn rect_from_bounds(bounds: &BrowserBounds) -> tauri::Rect {
  let position = tauri::LogicalPosition::new(bounds.x, bounds.y);
  let size = tauri::LogicalSize::new(bounds.width, bounds.height);
  tauri::Rect {
    position: position.into(),
    size: size.into(),
  }
}

fn ensure_webview(
  window: &Window,
  app: &AppHandle,
  bounds: &BrowserBounds,
  url: Option<String>,
) -> Result<Webview, String> {
  if let Some(webview) = app.get_webview(BROWSER_VIEW_LABEL) {
    return Ok(webview);
  }

  let initial_url = url
    .and_then(|u| tauri::Url::parse(&u).ok())
    .unwrap_or_else(|| tauri::Url::parse("about:blank").unwrap());

  let app_handle = app.clone();
  let app_handle_nav = app.clone();

  let builder = WebviewBuilder::new(BROWSER_VIEW_LABEL, WebviewUrl::External(initial_url))
    .on_navigation(move |url| {
      emit_event(
        &app_handle_nav,
        json!({ "type": "did-start-navigation", "url": url.as_str() }),
      );
      true
    })
    .on_page_load(move |_webview, payload| {
      if payload.event() == PageLoadEvent::Finished {
        emit_event(&app_handle, json!({ "type": "did-finish-load" }));
      }
    });

  window
    .add_child(builder, rect_from_bounds(bounds).position, rect_from_bounds(bounds).size)
    .map_err(|err| err.to_string())
}

fn get_webview(app: &AppHandle) -> Option<Webview> {
  app.get_webview(BROWSER_VIEW_LABEL)
}

#[tauri::command]
pub fn browser_view_show(
  window: Window,
  app: AppHandle,
  state: tauri::State<BrowserViewState>,
  bounds: BrowserBounds,
  url: Option<String>,
) -> Value {
  if bounds.width <= 0.0 || bounds.height <= 0.0 {
    return json!({ "ok": true });
  }

  let webview = match ensure_webview(&window, &app, &bounds, url.clone()) {
    Ok(w) => w,
    Err(err) => return json!({ "ok": false, "error": err }),
  };

  let rect = rect_from_bounds(&bounds);
  let _ = webview.set_bounds(rect);
  let _ = webview.set_focus();

  if let Some(url) = url {
    if let Ok(parsed) = tauri::Url::parse(&url) {
      let current = webview.url().ok().map(|u| u.to_string()).unwrap_or_default();
      if current.trim_end_matches('/') != url.trim_end_matches('/') {
        let _ = webview.navigate(parsed);
      }
    }
  }

  if let Ok(mut visible) = state.visible.lock() {
    *visible = true;
  }

  json!({ "ok": true })
}

#[tauri::command]
pub fn browser_view_hide(app: AppHandle, state: tauri::State<BrowserViewState>) -> Value {
  if let Some(webview) = get_webview(&app) {
    let hidden = BrowserBounds {
      x: -10000.0,
      y: -10000.0,
      width: 1.0,
      height: 1.0,
    };
    let _ = webview.set_bounds(rect_from_bounds(&hidden));
  }
  if let Ok(mut visible) = state.visible.lock() {
    *visible = false;
  }
  json!({ "ok": true })
}

#[tauri::command]
pub fn browser_view_set_bounds(app: AppHandle, bounds: BrowserBounds) -> Value {
  if let Some(webview) = get_webview(&app) {
    let _ = webview.set_bounds(rect_from_bounds(&bounds));
  }
  json!({ "ok": true })
}

#[tauri::command]
pub fn browser_view_load_url(app: AppHandle, args: BrowserLoadArgs) -> Value {
  let url = args.url.trim();
  if url.is_empty() {
    return json!({ "ok": true });
  }
  if let Some(webview) = get_webview(&app) {
    if let Ok(parsed) = tauri::Url::parse(url) {
      let current = webview.url().ok().map(|u| u.to_string()).unwrap_or_default();
      if args.force_reload.unwrap_or(false) || current.trim_end_matches('/') != url.trim_end_matches('/') {
        let _ = webview.navigate(parsed);
      }
    }
  }
  json!({ "ok": true })
}

#[tauri::command]
pub fn browser_view_go_back(app: AppHandle) -> Value {
  if let Some(webview) = get_webview(&app) {
    let _ = webview.eval("history.back()");
  }
  json!({ "ok": true })
}

#[tauri::command]
pub fn browser_view_go_forward(app: AppHandle) -> Value {
  if let Some(webview) = get_webview(&app) {
    let _ = webview.eval("history.forward()");
  }
  json!({ "ok": true })
}

#[tauri::command]
pub fn browser_view_reload(app: AppHandle) -> Value {
  if let Some(webview) = get_webview(&app) {
    let _ = webview.reload();
  }
  json!({ "ok": true })
}

#[tauri::command]
pub fn browser_view_open_devtools(_app: AppHandle) -> Value {
  #[cfg(debug_assertions)]
  if let Some(webview) = get_webview(&_app) {
    webview.open_devtools();
  }
  json!({ "ok": true })
}

#[tauri::command]
pub fn browser_view_clear(app: AppHandle) -> Value {
  if let Some(webview) = get_webview(&app) {
    if let Ok(blank) = tauri::Url::parse("about:blank") {
      let _ = webview.navigate(blank);
    }
  }
  json!({ "ok": true })
}
