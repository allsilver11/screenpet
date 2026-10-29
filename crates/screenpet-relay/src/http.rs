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

/// Address key for per-address limits: IPv4 as is, IPv6 by its /64 (one
/// home or VM usually owns a whole /64).
pub(crate) fn bucket(ip: IpAddr) -> String {
    match ip {
        IpAddr::V4(v4) => v4.to_string(),
        IpAddr::V6(v6) => match v6.to_ipv4_mapped() {
            Some(v4) => v4.to_string(),
            None => {
                let s = v6.segments();
                format!("{:x}:{:x}:{:x}:{:x}::/64", s[0], s[1], s[2], s[3])
            }
        },
    }
}

/// Client address for limits. Forwarded headers are believed only from a
/// trusted proxy (or loopback): `X-Real-IP`, else the *rightmost*
/// `X-Forwarded-For` entry (the one the proxy itself appended). Any other
/// peer is limited by its socket address.
fn client_ip(peer: IpAddr, headers: &HashMap<String, String>, trusted: &[IpAddr]) -> String {
    let peer = peer.to_canonical();
    if peer.is_loopback() || trusted.contains(&peer) {
        let forwarded = headers
            .get("x-real-ip")
            .map(|s| s.trim().to_string())
            .or_else(|| headers.get("x-forwarded-for").and_then(|s| s.rsplit(',').next()).map(|s| s.trim().to_string()));
        if let Some(ip) = forwarded.and_then(|s| s.parse::<IpAddr>().ok()) {
            return bucket(ip.to_canonical());
        }
    }
    bucket(peer)
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
            let ip = client_ip(peer.ip(), &req.headers, &hub.trusted_proxies);
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

#[cfg(test)]
mod tests {
    use super::*;

    fn h(pairs: &[(&str, &str)]) -> HashMap<String, String> {
        pairs.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect()
    }

    #[test]
    fn forwarded_headers_only_from_trusted_proxies() {
        let npm: IpAddr = "192.168.0.52".parse().unwrap();
        let trusted = [npm];
        let fwd = h(&[("x-real-ip", "203.0.113.7"), ("x-forwarded-for", "1.1.1.1, 203.0.113.7")]);
        assert_eq!(client_ip(npm, &fwd, &trusted), "203.0.113.7");
        assert_eq!(client_ip("127.0.0.1".parse().unwrap(), &fwd, &trusted), "203.0.113.7");
        // Another LAN host cannot pick its own address for the limits.
        assert_eq!(client_ip("192.168.0.99".parse().unwrap(), &fwd, &trusted), "192.168.0.99");
        assert_eq!(client_ip("198.51.100.1".parse().unwrap(), &fwd, &trusted), "198.51.100.1");
        // Without X-Real-IP the rightmost X-Forwarded-For entry counts, not
        // the client-supplied leftmost one.
        let xff = h(&[("x-forwarded-for", "6.6.6.6, 203.0.113.9")]);
        assert_eq!(client_ip(npm, &xff, &trusted), "203.0.113.9");
        assert_eq!(client_ip(npm, &h(&[("x-real-ip", "junk")]), &trusted), "192.168.0.52");
    }

    #[test]
    fn ipv6_is_bucketed_by_64() {
        let a = bucket("2001:db8:1:2:aaaa::1".parse().unwrap());
        assert_eq!(a, bucket("2001:db8:1:2:ffff:1:2:3".parse().unwrap()));
        assert_ne!(a, bucket("2001:db8:1:3::1".parse().unwrap()));
        assert_eq!(bucket("::ffff:10.0.0.1".parse().unwrap()), "10.0.0.1");
    }
}
