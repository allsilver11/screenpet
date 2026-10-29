//! LAN transport: TCP with line-delimited JSON (max line `MAX_LINE`).

use crate::discovery::mark_candidate;
use crate::peers::{dial_failed, run_session};
use crate::protocol::{Message, MAX_LINE};
use crate::transport::{Link, LinkReader, Recv};
use crate::{Shared, TICK};
use std::collections::VecDeque;
use std::io::{ErrorKind, Read, Write};
use std::net::{Shutdown, SocketAddr, TcpListener, TcpStream};
use std::sync::{Arc, Mutex};
use std::time::Duration;

pub(crate) struct TcpLink(Mutex<TcpStream>);

impl Link for TcpLink {
    fn send(&self, msg: &Message) -> bool {
        let mut s = self.0.lock().unwrap_or_else(|e| e.into_inner());
        if s.write_all(msg.to_line().as_bytes()).and_then(|_| s.flush()).is_err() {
            let _ = s.shutdown(Shutdown::Both);
            return false;
        }
        true
    }
    fn close(&self) {
        let s = self.0.lock().unwrap_or_else(|e| e.into_inner());
        let _ = s.shutdown(Shutdown::Both);
    }
}

pub(crate) struct TcpReader {
    stream: TcpStream,
    buf: Vec<u8>,
    ready: VecDeque<Recv>,
}

impl TcpReader {
    /// Split complete lines out of `buf` into `ready`. `false` = oversized.
    fn split_lines(&mut self) -> bool {
        while let Some(pos) = self.buf.iter().position(|&b| b == b'\n') {
            let mut line: Vec<u8> = self.buf.drain(..=pos).collect();
            line.pop();
            if line.last() == Some(&b'\r') {
                line.pop();
            }
            if line.len() > MAX_LINE {
                return false;
            }
            if line.iter().all(|b| b.is_ascii_whitespace()) {
                continue;
            }
            self.ready.push_back(match Message::parse(&line) {
                Some(m) => Recv::Msg(m),
                None => Recv::Invalid,
            });
        }
        self.buf.len() <= MAX_LINE
    }
}

impl LinkReader for TcpReader {
    fn recv(&mut self) -> Recv {
        if let Some(r) = self.ready.pop_front() {
            return r;
        }
        let mut chunk = [0u8; 8192];
        match self.stream.read(&mut chunk) {
            Ok(0) => Recv::Closed,
            Ok(n) => {
                self.buf.extend_from_slice(&chunk[..n]);
                if !self.split_lines() {
                    return Recv::Invalid;
                }
                self.ready.pop_front().unwrap_or(Recv::Idle)
            }
            Err(e) if matches!(e.kind(), ErrorKind::WouldBlock | ErrorKind::TimedOut | ErrorKind::Interrupted) => {
                Recv::Idle
            }
            Err(_) => Recv::Closed,
        }
    }
}

fn wrap(stream: TcpStream) -> std::io::Result<(Arc<dyn Link>, Box<dyn LinkReader>)> {
    stream.set_nonblocking(false)?;
    stream.set_nodelay(true)?;
    stream.set_read_timeout(Some(TICK))?;
    stream.set_write_timeout(Some(Duration::from_secs(2)))?;
    let writer = stream.try_clone()?;
    Ok((
        Arc::new(TcpLink(Mutex::new(writer))),
        Box::new(TcpReader { stream, buf: Vec::new(), ready: VecDeque::new() }),
    ))
}

/// Accept incoming LAN connections until shutdown.
pub(crate) fn accept_loop(shared: Arc<Shared>, listener: TcpListener) {
    while !shared.stopping() {
        match listener.accept() {
            Ok((stream, _)) => {
                if let Ok((link, reader)) = wrap(stream) {
                    let s = shared.clone();
                    shared.spawn(move || {
                        run_session(s, link, reader, None);
                    });
                }
            }
            Err(_) => std::thread::sleep(TICK),
        }
    }
}

/// Connect to a candidate address for `peer_id` on a worker thread. The
/// caller has already put `peer_id` into `State::dialing`. The candidate is
/// marked bad unless the session verified the peer's identity.
pub(crate) fn dial(shared: &Arc<Shared>, peer_id: String, addr: SocketAddr) {
    let s = shared.clone();
    shared.spawn(move || {
        let timeout = s.timing.conn_timeout.min(Duration::from_secs(2));
        let verified = match TcpStream::connect_timeout(&addr, timeout).and_then(wrap) {
            Ok((link, reader)) => run_session(s.clone(), link, reader, Some(peer_id.clone())),
            Err(_) => {
                dial_failed(&s, &peer_id);
                false
            }
        };
        mark_candidate(&s, &peer_id, addr, verified);
    });
}
