//! 최소 HTTP 앞단임: `GET /healthz` → `200 ok`, WebSocket 업그레이드가 붙은 `GET /ws` → 릴레이 프로토콜(`conn`),
//! `GET|HEAD /downloads/…` → 앱 업데이트(`downloads`), 그 밖 → 404.
//! 진입점 `handle`은 lib의 accept 루프가 연결마다 부름. 믿는 프록시 뒤의 클라이언트 주소도 여기서 정함.

use crate::hub::{Hub, Refused};
use crate::proto::{HEADER, MAX_FRAME};
use std::collections::HashMap;
use std::io::{Read, Write};
use std::net::{IpAddr, TcpStream};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tungstenite::protocol::{Role, WebSocketConfig};
use tungstenite::WebSocket;

/// 받아 주는 가장 큰 요청 머리
const MAX_HEAD: usize = 8 * 1024;
/// 요청 머리를 다 받는 마감
const HEAD_TIMEOUT: Duration = Duration::from_secs(5);

/// 읽은 요청 머리임
struct Request {
    /// HTTP 메서드
    method: String,
    /// 요청 경로
    path: String,
    /// 소문자 이름 → 값
    headers: HashMap<String, String>,
    /// 머리 다음까지 읽힌 바이트 (보통 없음)
    rest: Vec<u8>,
}

/// 요청 머리를 읽음. 너무 크거나 늦거나 깨졌으면 None임
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

/// 짧은 글 본문으로 HTTP 응답을 쓰고 닫음
fn respond(stream: &mut TcpStream, status: &str, body: &str) {
    let _ = write!(
        stream,
        "HTTP/1.1 {status}\r\nContent-Type: text/plain; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    let _ = stream.flush();
}

/// 머리 `name`의 쉼표 목록에 `token`이 (대소문자 무관) 있는지 봄
fn header_has(req: &Request, name: &str, token: &str) -> bool {
    req.headers.get(name).is_some_and(|v| v.split(',').any(|t| t.trim().eq_ignore_ascii_case(token)))
}

/// 주소별 한도용 주소 키임: IPv4는 그대로, IPv6은 /64로 묶음 (집이나 VM 하나가 보통 /64 전체를 가짐)
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

/// 한도용 클라이언트 주소를 정함. 전달 머리는 믿는 프록시(또는 루프백)에서 왔을 때만 믿음:
/// `X-Real-IP`, 없으면 `X-Forwarded-For`의 *가장 오른쪽* 항목 (프록시가 직접 붙인 것). 다른 상대는 소켓 주소로 제한함
fn client_ip(peer: IpAddr, headers: &HashMap<String, String>, trusted: &[IpAddr]) -> String {
    let peer = peer.to_canonical();
    if peer.is_loopback() || trusted.contains(&peer) {
        if let Some(ip) = forwarded_ip(headers) {
            return bucket(ip);
        }
    }
    bucket(peer)
}

/// `X-Real-IP`, 없으면 가장 오른쪽 `X-Forwarded-For` 항목을 읽음 (읽히면)
fn forwarded_ip(headers: &HashMap<String, String>) -> Option<IpAddr> {
    let value = headers.get("x-real-ip").map(String::as_str).or_else(|| headers.get("x-forwarded-for").and_then(|s| s.rsplit(',').next()))?;
    value.trim().parse::<IpAddr>().ok().map(|ip| ip.to_canonical())
}

/// 릴레이 WebSocket 설정임: 메시지·프레임 크기를 채널 머리 + `MAX_FRAME`으로 제한함
pub(crate) fn ws_config() -> WebSocketConfig {
    WebSocketConfig::default().max_message_size(Some(HEADER + MAX_FRAME)).max_frame_size(Some(HEADER + MAX_FRAME))
}

/// 연결 하나를 처리함: 머리를 읽고 경로별로 상태 확인·WebSocket·다운로드·404로 보냄
pub(crate) fn handle(hub: Arc<Hub>, downloads: Arc<crate::downloads::Downloads>, mut stream: TcpStream) {
    let Ok(peer) = stream.peer_addr() else { return };
    let _ = stream.set_write_timeout(Some(Duration::from_secs(5)));
    let Some(req) = read_head(&mut stream) else {
        respond(&mut stream, "400 Bad Request", "bad request\n");
        return;
    };
    if crate::downloads::is_downloads(&req.path) {
        let ip = client_ip(peer.ip(), &req.headers, &hub.trusted_proxies);
        downloads.prepare(&stream);
        return downloads.serve(&req.method, &req.path, &ip, &mut stream, &|| hub.stopping());
    }
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
            // The apps are not browsers and send no Origin: refuse web pages
            // that would spend visitors' browsers on the relay.
            if req.headers.contains_key("origin") {
                crate::hub::bump(&hub.counters.conns_refused_origin);
                respond(&mut stream, "403 Forbidden", "forbidden\n");
                return;
            }
            if hub.trusted_proxies.contains(&peer.ip().to_canonical()) && forwarded_ip(&req.headers).is_none() {
                hub.warn_proxy_without_address();
            }
            let ip = client_ip(peer.ip(), &req.headers, &hub.trusted_proxies);
            let admission = match hub.admit_ip(&ip) {
                Ok(a) => a,
                Err(Refused::Busy) => return respond(&mut stream, "503 Service Unavailable", "busy\n"),
                Err(Refused::Banned) => return respond(&mut stream, "403 Forbidden", "forbidden\n"),
                Err(Refused::RelayOff) => return respond(&mut stream, "503 Service Unavailable", "off\n"),
            };
            let accept = tungstenite::handshake::derive_accept_key(key.as_bytes());
            let ok = write!(
                stream,
                "HTTP/1.1 101 Switching Protocols\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Accept: {accept}\r\n\r\n"
            )
            .and_then(|_| stream.flush());
            if ok.is_ok() {
                let ws = WebSocket::from_partially_read(stream, req.rest, Role::Server, Some(ws_config()));
                crate::conn::serve(hub.clone(), ws, ip.clone(), &admission);
            }
            hub.release_ip(&ip, admission.conn);
        }
        _ => respond(&mut stream, "404 Not Found", "not found\n"),
    }
}

/// 전달 머리 믿기와 IPv6 묶기 시험임
#[cfg(test)]
mod tests {
    use super::*;

    /// 시험용 머리 표를 만듦
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
        // Those last cases are what the proxy warning looks for.
        assert_eq!(forwarded_ip(&h(&[("x-real-ip", "junk")])), None);
        assert_eq!(forwarded_ip(&h(&[])), None);
        assert_eq!(forwarded_ip(&xff), "203.0.113.9".parse().ok());
    }

    #[test]
    fn ipv6_is_bucketed_by_64() {
        let a = bucket("2001:db8:1:2:aaaa::1".parse().unwrap());
        assert_eq!(a, bucket("2001:db8:1:2:ffff:1:2:3".parse().unwrap()));
        assert_ne!(a, bucket("2001:db8:1:3::1".parse().unwrap()));
        assert_eq!(bucket("::ffff:10.0.0.1".parse().unwrap()), "10.0.0.1");
    }
}
