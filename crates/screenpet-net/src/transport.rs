//! Transport seam: a bidirectional, message-framed pipe to one peer.
//!
//! The session driver (`peers::run_session`) only ever sees `Message`s through
//! these traits, so a new transport (e.g. a WebSocket relay for friends outside
//! the LAN) only has to implement `Link` + `LinkReader` and hand them to
//! `run_session`; the protocol, the friend/invite logic and the public `Net`
//! API stay untouched. Peer identity is the install id carried in `hello`, so
//! the same friend list works over any transport.

use crate::protocol::Message;

/// Sending half; shared between the session thread and `Net` methods.
pub(crate) trait Link: Send + Sync {
    /// Send one message. `false` if the link is broken (it is then closed).
    fn send(&self, msg: &Message) -> bool;
    /// Close the link; the matching `LinkReader` then reports `Closed`.
    fn close(&self);
}

/// What one receive attempt produced.
pub(crate) enum Recv {
    /// A well-formed message.
    Msg(Message),
    /// Nothing arrived within a short tick; lets the session check timers.
    Idle,
    /// Unknown `type`, malformed JSON or oversized frame: drop the peer.
    Invalid,
    /// The link is gone.
    Closed,
}

/// Receiving half; owned by the session thread.
pub(crate) trait LinkReader: Send {
    /// Block for at most about one `TICK`.
    fn recv(&mut self) -> Recv;
}
