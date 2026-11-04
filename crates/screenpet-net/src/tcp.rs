//! LAN transport: TCP carrying frames as `u16` big-endian length + payload.

use crate::discovery::mark_candidate;
use crate::peers::{dial_failed, run_session, DialOutcome};
use crate::protocol::MAX_FRAME;
use crate::transport::{Candidate, FrameLink, FrameReader, Recv};
use crate::{Shared, TICK};
use std::io::{ErrorKind, Read, Write};
use std::net::{Shutdown, SocketAddr, TcpListener, TcpStream};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

pub(crate) struct TcpLink {
    /// Held for a whole frame, which can block for the write timeout.
    writer: Mutex<TcpStream>,
    /// Shuts the socket down without that lock: closing never waits for a writer.
    control: TcpStream,
    ip: String,
}

impl FrameLink for TcpLink {
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
    fn finish(&self) {
        let _ = self.control.shutdown(Shutdown::Write);
    }
    fn close(&self) {
        let _ = self.control.shutdown(Shutdown::Both);
    }
    fn peer_addr(&self) -> String {
        self.ip.clone()
    }
}

pub(crate) struct TcpReader {
    stream: TcpStream,
    buf: Vec<u8>,
}

impl TcpReader {
    /// Pop one complete frame from the buffer, if any.
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

/// Accept incoming LAN connections until shutdown. A connection over its
/// address's budget, or over the handshake cap, is dropped here (no thread, no
/// key exchange). Each other joins the bounded unverified pool (evicting the
/// oldest unverified link if needed); nothing here can panic the loop.
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

/// Connect to `addr` for `peer_id` on a worker thread; the candidate is
/// marked verified only if the handshake proved the dialed identity.
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

#[cfg(test)]
mod tests {
    use super::*;

    /// A link whose peer never reads, with a writer stuck inside `send_frame`.
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
