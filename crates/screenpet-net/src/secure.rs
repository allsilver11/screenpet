//! `SecureLink`: the post-handshake message pipe. Messages are queued (never
//! blocking the caller) to a writer thread that encrypts them with the Noise
//! transport state and sends them as frames.

use crate::protocol::{Message, MAX_MESSAGE};
use crate::transport::FrameLink;
use crate::Shared;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{sync_channel, Receiver, SyncSender, TrySendError};
use std::sync::{Arc, Mutex};

/// Messages queued per link before it is considered stuck and closed.
const QUEUE: usize = 64;

enum Out {
    Msg(Vec<u8>),
    /// Send nothing more; half-close after everything queued is written.
    Finish,
}

pub(crate) struct SecureLink {
    tx: SyncSender<Out>,
    frames: Arc<dyn FrameLink>,
    closing: AtomicBool,
}

impl SecureLink {
    /// Start the writer thread. `None` if it could not be spawned.
    pub fn start(
        shared: &Shared,
        frames: Arc<dyn FrameLink>,
        transport: Arc<Mutex<snow::TransportState>>,
    ) -> Option<Arc<SecureLink>> {
        let (tx, rx) = sync_channel(QUEUE);
        let f = frames.clone();
        if !shared.spawn(move || writer(rx, f, transport)) {
            return None;
        }
        Some(Arc::new(SecureLink { tx, frames, closing: AtomicBool::new(false) }))
    }

    /// Queue a message without blocking. `false` if the link is closing,
    /// broken, or its queue is full (then the link is closed).
    pub fn send(&self, msg: &Message) -> bool {
        if self.closing() {
            return false;
        }
        let json = msg.to_json();
        if json.len() > MAX_MESSAGE {
            return false;
        }
        match self.tx.try_send(Out::Msg(json)) {
            Ok(()) => true,
            Err(TrySendError::Full(_)) => {
                self.close();
                false
            }
            Err(TrySendError::Disconnected(_)) => false,
        }
    }

    /// Graceful close: everything already queued is still sent, then our
    /// side half-closes; the session keeps reading until the peer closes.
    pub fn finish(&self) {
        if !self.closing.swap(true, Ordering::SeqCst) && self.tx.try_send(Out::Finish).is_err() {
            self.frames.close();
        }
    }

    /// Abort immediately.
    pub fn close(&self) {
        self.closing.store(true, Ordering::SeqCst);
        self.frames.close();
    }

    pub fn closing(&self) -> bool {
        self.closing.load(Ordering::SeqCst)
    }
}

fn writer(rx: Receiver<Out>, frames: Arc<dyn FrameLink>, transport: Arc<Mutex<snow::TransportState>>) {
    let mut buf = vec![0u8; MAX_MESSAGE + 16];
    for out in rx {
        match out {
            Out::Msg(plain) => {
                let n = {
                    let mut t = transport.lock().unwrap_or_else(|e| e.into_inner());
                    t.write_message(&plain, &mut buf)
                };
                match n {
                    Ok(n) if frames.send_frame(&buf[..n]) => {}
                    _ => {
                        frames.close();
                        return;
                    }
                }
            }
            Out::Finish => {
                frames.finish();
                return;
            }
        }
    }
}
