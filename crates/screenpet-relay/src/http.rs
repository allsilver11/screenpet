//! Minimal HTTP front: `GET /healthz` → `200 ok`, `GET /ws` with a WebSocket
//! upgrade → the relay protocol, anything else → 404.

use crate::hub::Hub;
use crate::proto::{HEADER, MAX_FRAME};
use std::collections::HashMap;
use std::io::{Read, Write};
use std::net::{IpAddr, TcpStream};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tungstenite::protocol::{Role, WebSocketConfig};
use tungstenite::WebSocket;

/// Largest request head accepted.
const MAX_HEAD: usize = 8 * 1024;
const HEAD_TIMEOUT: Duration = Duration::from_secs(5);

struct Request {
    method: String,
    path: String,
    headers: HashMap<String, String>,
    /// Bytes read past the head (normally none).
    rest: Vec<u8>,
}

fn read_head(stream: &mut TcpStream) -> Option<Request> {
    stream.set_read_timeout(Some(Duration::from_millis(500))).ok()?;
    let deadline = Instant::now() + HEAD_TIMEOUT;
    let mut buf = Vec::new();
    let mut chunk = [0u8; 1024];
    let end = loop {
        if let Some(i) = buf.windows(4).position(|w| w == b"\r\n\r\n") {
            break i;
        }
        if buf.len() > MAX_HEAD || Instant::now() > deadline {
            return None;
        }
        match stream.read(&mut chunk) {
            Ok(0) => return None,
            Ok(n) => buf.extend_from_slice(&chunk[..n]),
            Err(e) if matches!(e.kind(), std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut | std::io::ErrorKind::Interrupted) => {}
            Err(_) => return None,
        }
    };
    let head = std::str::from_utf8(&buf[..end]).ok()?;
    let mut lines = head.split("\r\n");
    let mut first = lines.next()?.split(' ');
    let method = first.next()?.to_string();
    let path = first.next()?.split('?').next()?.to_string();
    let mut headers = HashMap::new();
    for line in lines {
        let (k, v) = line.split_once(':')?;
        headers.insert(k.trim().to_ascii_lowercase(), v.trim().to_string());
    }
    Some(Request { method, path, headers, rest: buf[end + 4..].to_vec() })
}

fn respond(stream: &mut TcpStream, status: &str, body: &str) {
    let _ = write!(
        stream,
        "HTTP/1.1 {status}\r\nContent-Type: text/plain; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    let _ = stream.flush();
}

fn header_has(req: &Request, name: &str, token: &str) -> bool {
    req.headers.get(name).is_some_and(|v| v.split(',').any(|t| t.trim().eq_ignore_ascii_case(token)))
}

/// Client address for limits. Behind a reverse proxy on a private/loopback
/// address (Nginx Proxy Manager), trust its `X-Real-IP` / `X-Forwarded-For`.
fn client_ip(peer: IpAddr, req: &Request) -> String {
    let proxied = match peer {
        IpAddr::V4(v4) => v4.is_loopback() || v4.is_private(),
        IpAddr::V6(v6) => v6.is_loopback(),
    };
    if proxied {
        let forwarded = req
            .headers
            .get("x-real-ip")
            .map(|s| s.trim().to_string())
            .or_else(|| req.headers.get("x-forwarded-for").and_then(|s| s.split(',').next()).map(|s| s.trim().to_string()));
        if let Some(ip) = forwarded.and_then(|s| s.parse::<IpAddr>().ok()) {
            return ip.to_string();
        }
    }
    peer.to_string()
}

pub(crate) fn ws_config() -> WebSocketConfig {
    WebSocketConfig::default().max_message_size(Some(HEADER + MAX_FRAME)).max_frame_size(Some(HEADER + MAX_FRAME))
}

pub(crate) fn handle(hub: Arc<Hub>, mut stream: TcpStream) {
    let Ok(peer) = stream.peer_addr() else { return };
    let _ = stream.set_write_timeout(Some(Duration::from_secs(5)));
    let Some(req) = read_head(&mut stream) else {
        respond(&mut stream, "400 Bad Request", "bad request\n");
        return;
    };
    if req.method != "GET" {
        respond(&mut stream, "405 Method Not Allowed", "method not allowed\n");
        return;
    }
    match req.path.as_str() {
        "/healthz" => respond(&mut stream, "200 OK", "ok\n"),
        "/ws" => {
            let key = req.headers.get("sec-websocket-key").cloned();
            let upgrade = header_has(&req, "upgrade", "websocket") && header_has(&req, "connection", "upgrade");
            let version = req.headers.get("sec-websocket-version").map(String::as_str) == Some("13");
            let Some(key) = key.filter(|_| upgrade && version) else {
                respond(&mut stream, "426 Upgrade Required", "websocket only\n");
                return;
            };
            let ip = client_ip(peer.ip(), &req);
            if !hub.admit_ip(&ip) {
                respond(&mut stream, "503 Service Unavailable", "busy\n");
                return;
            }
            let accept = tungstenite::handshake::derive_accept_key(key.as_bytes());
            let ok = write!(
                stream,
                "HTTP/1.1 101 Switching Protocols\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Accept: {accept}\r\n\r\n"
            )
            .and_then(|_| stream.flush());
            if ok.is_ok() {
                let ws = WebSocket::from_partially_read(stream, req.rest, Role::Server, Some(ws_config()));
                crate::conn::serve(hub.clone(), ws, ip.clone());
            }
            hub.release_ip(&ip);
        }
        _ => respond(&mut stream, "404 Not Found", "not found\n"),
    }
}
