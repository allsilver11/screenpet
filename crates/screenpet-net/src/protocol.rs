//! Wire formats: the UDP discovery announcement and the line-delimited JSON
//! messages exchanged over a TCP connection (each tagged by a `type` field).

use serde::{Deserialize, Serialize};

/// Protocol version carried in announcements and `hello`.
pub const PROTOCOL_VERSION: u32 = 1;
/// App tag carried in announcements so unrelated UDP traffic is ignored.
pub const APP_TAG: &str = "screenpet";
/// Longest accepted line (bytes, excluding the newline). Longer → disconnect.
pub const MAX_LINE: usize = 64 * 1024;

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
    /// First message on every link; `nonce` is this side's fresh challenge.
    Hello { id: String, name: String, v: u32, nonce: String },
    /// Answer to the peer's hello nonce (friends only, see `auth`).
    Auth { mac: String },
    Invite { pet: String },
    /// `secret` (hex, 32 bytes) is present iff accepted: the new friendship key.
    InviteReply {
        accepted: bool,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        secret: Option<String>,
    },
    Unfriend,
    Ping,
}

/// Any message on the wire. Untagged at this level: each inner enum carries
/// its own `type` tag, so an unknown `type` fails to parse as either.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Message {
    Control(Control),
    Visit(Visit),
}

impl Message {
    /// Serialize to one JSON line, including the trailing `\n`.
    pub fn to_line(&self) -> String {
        let mut s = serde_json::to_string(self).expect("message serializes");
        s.push('\n');
        s
    }

    /// Parse one line (without the newline). `None` for anything malformed
    /// or with an unknown `type`.
    pub fn parse(line: &[u8]) -> Option<Message> {
        serde_json::from_slice(line).ok()
    }
}

/// UDP discovery announcement.
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
        let a: Announce = serde_json::from_slice(data).ok()?;
        (a.app == APP_TAG && !a.id.is_empty()).then_some(a)
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
        let line = m.to_line();
        assert!(line.ends_with('\n'));
        let v: serde_json::Value = serde_json::from_str(line.trim_end()).unwrap();
        assert_eq!(v["type"], "visit");
        assert_eq!(v["enter_side"], "left");
        assert_eq!(Message::parse(line.trim_end().as_bytes()), Some(m));
    }

    #[test]
    fn control_messages_round_trip() {
        for m in [
            Message::Control(Control::Hello { id: "a".into(), name: "A".into(), v: 1, nonce: "00".into() }),
            Message::Control(Control::Auth { mac: "ab".into() }),
            Message::Control(Control::Invite { pet: "cat".into() }),
            Message::Control(Control::InviteReply { accepted: true, secret: Some("cd".into()) }),
            Message::Control(Control::Unfriend),
            Message::Control(Control::Ping),
            Message::Visit(Visit::Leave { visit_id: 3 }),
            Message::Visit(Visit::ComeHome { visit_id: 4 }),
        ] {
            let line = m.to_line();
            assert_eq!(Message::parse(line.trim_end().as_bytes()), Some(m));
        }
        assert_eq!(
            Message::parse(br#"{"type":"invite_reply","accepted":false}"#),
            Some(Message::Control(Control::InviteReply { accepted: false, secret: None }))
        );
        assert_eq!(Message::parse(br#"{"type":"ping"}"#), Some(Message::Control(Control::Ping)));
    }

    #[test]
    fn unknown_or_malformed_messages_are_rejected() {
        assert_eq!(Message::parse(br#"{"type":"bogus"}"#), None);
        assert_eq!(Message::parse(br#"{"no_type":1}"#), None);
        assert_eq!(Message::parse(br#"{"type":"leave"}"#), None);
        // hello without a nonce is malformed
        assert_eq!(Message::parse(br#"{"type":"hello","id":"a","name":"A","v":1}"#), None);
        assert_eq!(Message::parse(b"not json"), None);
    }

    #[test]
    fn announce_parsing() {
        let a = Announce::new("abc", "Ann", 4000);
        let bytes = serde_json::to_vec(&a).unwrap();
        assert_eq!(Announce::parse(&bytes), Some(a));
        assert_eq!(
            Announce::parse(br#"{"app":"other","v":1,"id":"x","name":"n","port":1}"#),
            None
        );
        assert_eq!(Announce::parse(b"garbage"), None);
    }
}
