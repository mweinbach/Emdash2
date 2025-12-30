use serde::Deserialize;
use serde_json::json;
use std::net::{TcpStream, ToSocketAddrs};
use std::time::Duration;

fn probe_port(host: &str, port: u16, timeout_ms: u64) -> bool {
  let addr = format!("{}:{}", host, port);
  let addrs = match addr.to_socket_addrs() {
    Ok(list) => list.collect::<Vec<_>>(),
    Err(_) => return false,
  };
  let timeout = Duration::from_millis(timeout_ms.max(1));
  for socket in addrs {
    if let Ok(stream) = TcpStream::connect_timeout(&socket, timeout) {
      let _ = stream.shutdown(std::net::Shutdown::Both);
      return true;
    }
  }
  false
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NetProbeArgs {
  host: String,
  ports: Vec<i64>,
  timeout_ms: Option<u64>,
}

#[tauri::command]
pub fn net_probe_ports(args: NetProbeArgs) -> serde_json::Value {
  let h = args.host.trim();
  let host = if h.is_empty() { "localhost" } else { h };
  let timeout = args.timeout_ms.unwrap_or(800).max(1);

  let mut reachable: Vec<u16> = Vec::new();
  for port in args.ports {
    if port <= 0 || port > 65535 {
      continue;
    }
    let port_u16 = port as u16;
    if probe_port(host, port_u16, timeout) {
      reachable.push(port_u16);
    }
  }

  json!({ "reachable": reachable })
}
