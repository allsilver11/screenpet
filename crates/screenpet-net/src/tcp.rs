//! LAN 전송임: TCP 위에 `u16` 빅엔디언 길이 + 내용으로 프레임을 나름.
//! 받기는 `accept_loop`, 걸기는 `dial`임. 위로는 `transport`의 `FrameLink`/`FrameReader`를 구현하고 `peers` 세션에 넘김.

use crate::discovery::mark_candidate;
use crate::peers::{dial_failed, run_session, DialOutcome};
use crate::protocol::MAX_FRAME;
use crate::transport::{Candidate, FrameLink, FrameReader, Recv};
use crate::{Shared, TICK};
use std::io::{ErrorKind, Read, Write};
use std::net::{Shutdown, SocketAddr, TcpListener, TcpStream};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// TCP 링크의 보내는 쪽 반쪽임
pub(crate) struct TcpLink {
    /// 프레임 하나를 다 쓰는 동안 잡음. 쓰기 시간 초과만큼 막힐 수 있음
    writer: Mutex<TcpStream>,
    /// 그 잠금 없이 소켓을 내림: 닫기는 writer를 기다리지 않음
    control: TcpStream,
    /// 상대 IP (속도 제한용)
    ip: String,
}

impl FrameLink for TcpLink {
    /// 길이 접두어를 붙여 프레임 하나를 씀. 너무 크거나 쓰기 실패면 `false`임
    fn send_frame(&self, frame: &[u8]) -> bool {
        if frame.len() > MAX_FRAME {
            return false;
        }
        let mut out = Vec::with_capacity(frame.len() + 2);
        out.extend_from_slice(&(frame.len() as u16).to_be_bytes());
        out.extend_from_slice(frame);
        let mut s = self.writer.lock().unwrap_or_else(|e| e.into_inner());
        if s.write_all(&out).and_then(|_| s.flush()).is_err() {
            let _ = s.shutdown(Shutdown::Both);
            return false;
        }
        true
    }
    /// 쓰기 쪽만 내림
    fn finish(&self) {
        let _ = self.control.shutdown(Shutdown::Write);
    }
    /// 소켓 양쪽을 내림
    fn close(&self) {
        let _ = self.control.shutdown(Shutdown::Both);
    }
    /// 상대 IP를 돌려줌
    fn peer_addr(&self) -> String {
        self.ip.clone()
    }
}

/// TCP 링크의 받는 쪽 반쪽임 (읽은 바이트를 버퍼에 모음)
pub(crate) struct TcpReader {
    stream: TcpStream,
    buf: Vec<u8>,
}

impl TcpReader {
    /// 버퍼에서 완성된 프레임 하나를 꺼냄. 아직 덜 왔으면 None임
    fn take_frame(&mut self) -> Option<Recv> {
        if self.buf.len() < 2 {
            return None;
        }
        let len = usize::from(u16::from_be_bytes([self.buf[0], self.buf[1]]));
        if len == 0 {
            return Some(Recv::Invalid);
        }
        if self.buf.len() < 2 + len {
            return None;
        }
        let frame = self.buf[2..2 + len].to_vec();
        self.buf.drain(..2 + len);
        Some(Recv::Frame(frame))
    }
}

impl FrameReader for TcpReader {
    /// 버퍼에 프레임이 있으면 바로 주고, 없으면 틱 하나만큼 읽어 봄
    fn recv(&mut self) -> Recv {
        if let Some(r) = self.take_frame() {
            return r;
        }
        let mut chunk = [0u8; 16 * 1024];
        match self.stream.read(&mut chunk) {
            Ok(0) => Recv::Closed,
            Ok(n) => {
                self.buf.extend_from_slice(&chunk[..n]);
                self.take_frame().unwrap_or(Recv::Idle)
            }
            Err(e) if matches!(e.kind(), ErrorKind::WouldBlock | ErrorKind::TimedOut | ErrorKind::Interrupted) => {
                Recv::Idle
            }
            Err(_) => Recv::Closed,
        }
    }
}

/// TCP 스트림을 시간 제한·nodelay를 맞춘 링크 두 반쪽으로 감쌈
fn wrap(stream: TcpStream) -> std::io::Result<(Arc<dyn FrameLink>, Box<dyn FrameReader>)> {
    stream.set_nonblocking(false)?;
    stream.set_nodelay(true)?;
    stream.set_read_timeout(Some(TICK))?;
    stream.set_write_timeout(Some(Duration::from_secs(2)))?;
    let writer = stream.try_clone()?;
    let ip = stream.peer_addr()?.ip().to_string();
    let control = stream.try_clone()?;
    Ok((Arc::new(TcpLink { writer: Mutex::new(writer), control, ip }), Box::new(TcpReader { stream, buf: Vec::new() })))
}

/// 종료까지 LAN 연결을 받음. 주소별 예산이나 핸드셰이크 상한을 넘은 연결은 여기서 버림(스레드·키 교환 없음).
/// 나머지는 크기 제한된 미검증 풀에 들어감(필요하면 가장 오래된 미검증 링크를 밀어냄). 이 루프는 패닉하지 않음
pub(crate) fn accept_loop(shared: Arc<Shared>, listener: TcpListener) {
    while !shared.stopping() {
        match listener.accept() {
            Ok((stream, from)) => {
                if !shared.allow_inbound(&from.ip().to_string(), Instant::now()) {
                    continue;
                }
                if let Ok((link, reader)) = wrap(stream) {
                    let Some(guard) = shared.admit_inbound(link.clone()) else { continue };
                    let s = shared.clone();
                    // On spawn failure the closure (link + guard) is dropped.
                    shared.spawn(move || {
                        run_session(s, link, reader, None, guard);
                    });
                }
            }
            Err(_) => std::thread::sleep(TICK),
        }
    }
}

/// 워커 스레드에서 `addr`로 `peer_id`에게 연결함.
/// 핸드셰이크가 다이얼한 신원을 증명해야만 후보를 검증됨으로 표시함
pub(crate) fn dial(shared: &Arc<Shared>, peer_id: String, addr: SocketAddr) {
    let guard = shared.outbound_guard();
    let s = shared.clone();
    let id = peer_id.clone();
    let spawned = shared.spawn(move || {
        let timeout = s.timing.conn_timeout.min(Duration::from_secs(2));
        let outcome = match TcpStream::connect_timeout(&addr, timeout).and_then(wrap) {
            Ok((link, reader)) => run_session(s.clone(), link, reader, Some(id.clone()), guard),
            Err(_) => {
                dial_failed(&s, &id);
                DialOutcome::Failed
            }
        };
        mark_candidate(&s, &id, Candidate::Lan(addr), outcome);
    });
    if !spawned {
        dial_failed(shared, &peer_id);
    }
}

/// 막힌 writer가 있어도 닫기가 바로 끝나는지 보는 시험임
#[cfg(test)]
mod tests {
    use super::*;

    /// 상대가 읽지 않아 writer가 `send_frame` 안에서 막힌 링크를 만듦
    fn stuck_link() -> (Arc<dyn FrameLink>, TcpStream) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let stream = TcpStream::connect(listener.local_addr().unwrap()).unwrap();
        let (silent_peer, _) = listener.accept().unwrap();
        let (link, _reader) = wrap(stream).unwrap();
        let writer = link.clone();
        std::thread::spawn(move || while writer.send_frame(&[0u8; MAX_FRAME]) {});
        std::thread::sleep(Duration::from_millis(400)); // the socket buffer fills up; the writer blocks
        (link, silent_peer)
    }

    #[test]
    fn close_never_waits_for_a_stuck_writer() {
        let (link, _peer) = stuck_link();
        let t = Instant::now();
        link.close();
        assert!(t.elapsed() < Duration::from_millis(200), "close took {:?}", t.elapsed());
    }

    #[test]
    fn finish_never_waits_for_a_stuck_writer() {
        let (link, _peer) = stuck_link();
        let t = Instant::now();
        link.finish();
        assert!(t.elapsed() < Duration::from_millis(200), "finish took {:?}", t.elapsed());
    }
}
