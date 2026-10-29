//! Transport seam: an ordered, reliable pipe of opaque frames to one peer,
//! plus transport-neutral dialing.
//!
//! Everything above this layer is transport-independent: `peers` runs the
//! Noise handshake over frames and then sends encrypted JSON messages through
//! `secure::SecureLink`. A new transport (e.g. a WebSocket relay for friends
//! outside the LAN) implements `FrameLink` + `FrameReader`, adds a
//! `Candidate` kind and a branch in `dial`, and hands accepted links to
//! `peers::run_session`. Noise runs end to end, so a relay only ever sees
//! ciphertext. Peer identity is the pinned Noise static key, so the same
//! friend list works over every transport.

use crate::{relay, tcp, Shared};
use std::net::SocketAddr;
use std::sync::Arc;

/// Sending half; shared between the session thread and the writer thread.
pub(crate) trait FrameLink: Send + Sync {
    /// Send one frame (at most `MAX_FRAME` bytes). `false` = link broken.
    fn send_frame(&self, frame: &[u8]) -> bool;
    /// Graceful close: no more frames from us, but keep receiving.
    fn finish(&self);
    /// Abort the link; the matching `FrameReader` then reports `Closed`.
    fn close(&self);
    /// Who is on the other end, for rate limiting (e.g. the remote IP).
    fn peer_addr(&self) -> String;
}

/// What one receive attempt produced.
pub(crate) enum Recv {
    Frame(Vec<u8>),
    /// Nothing arrived within a short tick; lets the session check timers.
    Idle,
    /// Malformed framing: drop the peer.
    Invalid,
    /// The link is gone.
    Closed,
}

/// Receiving half; owned by the session thread.
pub(crate) trait FrameReader: Send {
    /// Block for at most about one `TICK`.
    fn recv(&mut self) -> Recv;
}

/// Somewhere a peer might be reached. Announcements only produce candidates;
/// a link counts once the Noise handshake proves the peer's identity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum Candidate {
    Lan(SocketAddr),
    /// Through the relay, to this static public key.
    Relay([u8; 32]),
}

/// Dial `peer_id` at `candidate` on a worker thread. The caller has already
/// put `peer_id` into `State::dialing`.
pub(crate) fn dial(shared: &Arc<Shared>, peer_id: String, candidate: Candidate) {
    match candidate {
        Candidate::Lan(addr) => tcp::dial(shared, peer_id, addr),
        Candidate::Relay(key) => relay::dial(shared, peer_id, key),
    }
}
