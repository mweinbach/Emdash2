use serde_json::Value;

pub async fn run_blocking<T, F>(fallback: T, f: F) -> T
where
  T: Send + 'static,
  F: FnOnce() -> T + Send + 'static,
{
  match tauri::async_runtime::spawn_blocking(f).await {
    Ok(value) => value,
    Err(_) => fallback,
  }
}

pub async fn run_blocking_value<F>(fallback: Value, f: F) -> Value
where
  F: FnOnce() -> Value + Send + 'static,
{
  run_blocking(fallback, f).await
}
