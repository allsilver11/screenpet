//! Wire formats: the UDP discovery announcement and the JSON messages carried
//! (encrypted) inside Noise transport messages, each tagged by a `type` field.

use serde::{Deserialize, Serialize};

/// Protocol version carried in announcements and `hello`.
pub const PROTOCOL_VERSION: u32 = 1;
/// App tag carried in announcements so unrelated UDP traffic is ignored.
pub const APP_TAG: &str = "screenpet";
/// Largest frame on a link (a Noise message is at most 65535 bytes).
pub const MAX_FRAME: usize = 65535;
/// Largest JSON message (Noise frame minus the 16-byte AEAD tag).
pub const MAX_MESSAGE: usize = MAX_FRAME - 16;
/// Longest accepted install id (bytes).
pub const MAX_ID: usize = 128;
/// Display names and pet labels are clamped to this many characters.
pub const MAX_NAME: usize = 64;

/// Which screen edge a visiting pet walks in from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Side {
    Left,
    Right,
}

/// Messages about visiting pets, delivered to the app via `NetEvent::Received`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Visit {
    Visit {
        visit_id: u64,
        species: String,
        stage: String,
        scale: f64,
        personality: String,
        owner: String,
        enter_side: Side,
    },
    Leave {
        visit_id: u64,
    },
    ComeHome {
        visit_id: u64,
    },
}

/// Connection-management messages handled inside the net crate.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Control {
    /// First message after the Noise handshake, both directions.
    Hello { id: String, name: String, v: u32 },
    /// Pairing request (only on a link the inviter dialed). `commit` =
    /// hex BLAKE2s(nonce_i) for the inviter's secret 32-byte random nonce_i.
    Invite { pet: String, commit: String },
    /// Invitee's 32-byte random r_r (hex), sent after seeing only the commit.
    PairRandom { random: String },
    /// Inviter reveals nonce_i (hex); the invitee checks it against `commit`.
    /// SAS = first 36 bits of BLAKE2s(handshake_hash || nonce_i || r_r).
    PairReveal { nonce: String },
    /// Each side's SAS confirmation (or refusal) for the pairing on this link.
    InviteReply { accepted: bool },
    Unfriend,
    Ping,
}

/// Any message. Untagged at this level: each inner enum carries its own
/// `type` tag, so an unknown `type` fails to parse as either.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Message {
    Control(Control),
    Visit(Visit),
}

impl Message {
    pub fn to_json(&self) -> Vec<u8> {
        serde_json::to_vec(self).expect("message serializes")
    }

    /// `None` for anything malformed or with an unknown `type`.
    pub fn parse(data: &[u8]) -> Option<Message> {
        serde_json::from_slice(data).ok()
    }
}

/// Clamp a peer-supplied display string to `MAX_NAME` characters.
pub fn clamp_name(s: &str) -> String {
    s.chars().take(MAX_NAME).collect()
}

/// UDP discovery announcement (an unauthenticated hint).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Announce {
    pub app: String,
    pub v: u32,
    pub id: String,
    pub name: String,
    pub port: u16,
}

impl Announce {
    pub fn new(id: &str, name: &str, port: u16) -> Announce {
        Announce {
            app: APP_TAG.to_string(),
            v: PROTOCOL_VERSION,
            id: id.to_string(),
            name: name.to_string(),
            port,
        }
    }

    /// Parse a datagram; `None` unless it is a well-formed ScreenPet announcement.
    pub fn parse(data: &[u8]) -> Option<Announce> {
        let mut a: Announce = serde_json::from_slice(data).ok()?;
        if a.app != APP_TAG || a.id.is_empty() || a.id.len() > MAX_ID || a.port == 0 {
            return None;
        }
        a.name = clamp_name(&a.name);
        Some(a)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn visit_wire_format_uses_type_tag() {
        let m = Message::Visit(Visit::Visit {
            visit_id: 7,
            species: "cat".into(),
            stage: "child".into(),
            scale: 1.5,
            personality: "playful".into(),
            owner: "Ann".into(),
            enter_side: Side::Left,
        });
        let v: serde_json::Value = serde_json::from_slice(&m.to_json()).unwrap();
        assert_eq!(v["type"], "visit");
        assert_eq!(v["enter_side"], "left");
        assert_eq!(Message::parse(&m.to_json()), Some(m));
    }

    #[test]
    fn control_messages_round_trip() {
        for m in [
            Message::Control(Control::Hello { id: "a".into(), name: "A".into(), v: 1 }),
            Message::Control(Control::Invite { pet: "cat".into(), commit: "ab".into() }),
            Message::Control(Control::PairRandom { random: "cd".into() }),
            Message::Control(Control::PairReveal { nonce: "ef".into() }),
            Message::Control(Control::InviteReply { accepted: true }),
            Message::Control(Control::Unfriend),
            Message::Control(Control::Ping),
            Message::Visit(Visit::Leave { visit_id: 3 }),
            Message::Visit(Visit::ComeHome { visit_id: 4 }),
        ] {
            assert_eq!(Message::parse(&m.to_json()), Some(m));
        }
        assert_eq!(Message::parse(br#"{"type":"ping"}"#), Some(Message::Control(Control::Ping)));
    }

    #[test]
    fn unknown_or_malformed_messages_are_rejected() {
        assert_eq!(Message::parse(br#"{"type":"bogus"}"#), None);
        assert_eq!(Message::parse(br#"{"no_type":1}"#), None);
        assert_eq!(Message::parse(br#"{"type":"leave"}"#), None);
        assert_eq!(Message::parse(b"not json"), None);
    }

    #[test]
    fn announce_parsing_and_clamping() {
        let a = Announce::new("abc", "Ann", 4000);
        assert_eq!(Announce::parse(&serde_json::to_vec(&a).unwrap()), Some(a));
        assert_eq!(Announce::parse(br#"{"app":"other","v":1,"id":"x","name":"n","port":1}"#), None);
        assert_eq!(Announce::parse(b"garbage"), None);
        let long = Announce::new("abc", &"n".repeat(500), 4000);
        assert_eq!(Announce::parse(&serde_json::to_vec(&long).unwrap()).unwrap().name.chars().count(), MAX_NAME);
        let long_id = Announce::new(&"i".repeat(MAX_ID + 1), "n", 4000);
        assert_eq!(Announce::parse(&serde_json::to_vec(&long_id).unwrap()), None);
    }

    #[test]
    fn clamp_counts_characters() {
        assert_eq!(clamp_name(&"🐱".repeat(100)).chars().count(), MAX_NAME);
        assert_eq!(clamp_name("Ann"), "Ann");
    }
}
