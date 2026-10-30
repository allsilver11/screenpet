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
/// Most friends one install keeps; pairing is refused (both directions) once full.
pub const MAX_FRIENDS: usize = 50;

/// Which screen edge a visiting pet walks in from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Side {
    Left,
    Right,
}

/// A pet thrown across the screen edge (like a second monitor): feet height
/// as a fraction of the work-area height (0 top … 1 bottom) and velocity in
/// logical px/s. The receiver sanitizes it; older versions ignore it and
/// walk the pet in as usual.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Fling {
    pub height: f64,
    pub vx: f64,
    pub vy: f64,
}

/// An optional `fling`; a malformed one is dropped instead of the whole message.
fn lenient_fling<'de, D: serde::Deserializer<'de>>(d: D) -> Result<Option<Fling>, D::Error> {
    let v = Option::<serde_json::Value>::deserialize(d)?;
    Ok(v.and_then(|v| serde_json::from_value(v).ok()))
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
        /// The pet walked over from this friend's screen (owner's friend id);
        /// the receiver enters it from the edge it links to that friend, if any.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        via: Option<String>,
        /// Seconds left before the pet gets homesick (continues across hops).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        homesick_in: Option<f64>,
        /// The pet was thrown across: it flies in keeping this motion.
        #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "lenient_fling")]
        fling: Option<Fling>,
    },
    /// The guest left the receiver's friend's screen. `next: None` = it went
    /// home (also what older versions send). `next: Some(id)` = it walked out
    /// through the edge linked to friend `id` (through `side`) and wants to
    /// visit them; the owner forwards a `Visit` there only if `id` is its own
    /// connected friend, otherwise it brings the pet home.
    Leave {
        visit_id: u64,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        next: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        side: Option<Side>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        homesick_in: Option<f64>,
        /// The guest was thrown out through that edge (home, or on to `next`).
        #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "lenient_fling")]
        fling: Option<Fling>,
    },
    ComeHome {
        visit_id: u64,
    },
}

impl Visit {
    /// A plain "went home" `Leave` (same wire shape as protocol v1 without roaming).
    pub fn leave(visit_id: u64) -> Visit {
        Visit::Leave { visit_id, next: None, side: None, homesick_in: None, fling: None }
    }
}

/// Connection-management messages handled inside the net crate.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Control {
    /// First message after the Noise handshake, both directions. `friend` =
    /// the sender has the receiver's static key pinned as a friend.
    Hello {
        id: String,
        name: String,
        v: u32,
        #[serde(default)]
        friend: bool,
    },
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
    /// Keep-alive. `ask` = an app request to a friend (e.g. "invite": bring
    /// your pet over, "busy": not now) — older versions parse a ping with an
    /// extra field as a plain ping and ignore it.
    Ping {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        ask: Option<String>,
    },
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
            via: None,
            homesick_in: None,
            fling: None,
        });
        let v: serde_json::Value = serde_json::from_slice(&m.to_json()).unwrap();
        assert_eq!(v["type"], "visit");
        assert_eq!(v["enter_side"], "left");
        assert_eq!(Message::parse(&m.to_json()), Some(m));
    }

    #[test]
    fn control_messages_round_trip() {
        for m in [
            Message::Control(Control::Hello { id: "a".into(), name: "A".into(), v: 1, friend: true }),
            Message::Control(Control::Invite { pet: "cat".into(), commit: "ab".into() }),
            Message::Control(Control::PairRandom { random: "cd".into() }),
            Message::Control(Control::PairReveal { nonce: "ef".into() }),
            Message::Control(Control::InviteReply { accepted: true }),
            Message::Control(Control::Unfriend),
            Message::Control(Control::Ping { ask: None }),
            Message::Visit(Visit::leave(3)),
            Message::Visit(Visit::ComeHome { visit_id: 4 }),
        ] {
            assert_eq!(Message::parse(&m.to_json()), Some(m));
        }
        assert_eq!(Message::parse(br#"{"type":"ping"}"#), Some(Message::Control(Control::Ping { ask: None })));
    }

    #[test]
    fn leave_can_name_the_next_friend_and_stays_backward_compatible() {
        let hop = Message::Visit(Visit::Leave { visit_id: 9, next: Some("xx".into()), side: Some(Side::Left), homesick_in: Some(42.5), fling: None });
        let v: serde_json::Value = serde_json::from_slice(&hop.to_json()).unwrap();
        assert_eq!((v["next"].as_str(), v["side"].as_str(), v["homesick_in"].as_f64()), (Some("xx"), Some("left"), Some(42.5)));
        assert_eq!(Message::parse(&hop.to_json()), Some(hop));
        // 예전 판의 leave (next 없음) = 집으로
        assert_eq!(
            Message::parse(br#"{"type":"leave","visit_id":3}"#),
            Some(Message::Visit(Visit::Leave { visit_id: 3, next: None, side: None, homesick_in: None, fling: None }))
        );
        // 집으로 가는 leave는 예전 판과 같은 모양
        assert_eq!(Message::Visit(Visit::leave(3)).to_json(), br#"{"type":"leave","visit_id":3}"#.to_vec());
    }

    #[test]
    fn visit_can_carry_the_previous_host_and_homesick_time() {
        let old = br#"{"type":"visit","visit_id":1,"species":"cat","stage":"child","scale":1.0,"personality":"friendly","owner":"A","enter_side":"left"}"#;
        let Some(Message::Visit(Visit::Visit { via, homesick_in, .. })) = Message::parse(old) else { panic!("old visit must parse") };
        assert_eq!((via, homesick_in), (None, None));
        let hop = Message::Visit(Visit::Visit {
            visit_id: 2,
            species: "cat".into(),
            stage: "child".into(),
            scale: 1.0,
            personality: "friendly".into(),
            owner: "A".into(),
            enter_side: Side::Right,
            via: Some("bb".into()),
            homesick_in: Some(10.0),
            fling: None,
        });
        assert_eq!(Message::parse(&hop.to_json()), Some(hop));
        // 새 필드를 모르는 받는 쪽도 읽을 수 있다 (모르는 필드는 무시)
        assert!(Message::parse(br#"{"type":"leave","visit_id":3,"next":"x","future":1}"#).is_some());
    }

    #[test]
    fn visit_and_leave_can_carry_a_throw_and_old_peers_still_parse() {
        let f = Fling { height: 0.25, vx: -640.0, vy: -120.5 };
        let v = Message::Visit(Visit::Visit {
            visit_id: 5,
            species: "cat".into(),
            stage: "child".into(),
            scale: 1.0,
            personality: "friendly".into(),
            owner: "A".into(),
            enter_side: Side::Right,
            via: None,
            homesick_in: None,
            fling: Some(f),
        });
        let j: serde_json::Value = serde_json::from_slice(&v.to_json()).unwrap();
        assert_eq!((j["fling"]["height"].as_f64(), j["fling"]["vx"].as_f64(), j["fling"]["vy"].as_f64()), (Some(0.25), Some(-640.0), Some(-120.5)));
        assert_eq!(Message::parse(&v.to_json()), Some(v));
        let back = Message::Visit(Visit::Leave { visit_id: 5, next: None, side: Some(Side::Left), homesick_in: None, fling: Some(f) });
        assert_eq!(Message::parse(&back.to_json()), Some(back));
        // 던지기를 모르는 예전 판: 필드가 없으면 None (평소처럼 걸어 들어온다)
        let old = br#"{"type":"visit","visit_id":1,"species":"cat","stage":"child","scale":1.0,"personality":"friendly","owner":"A","enter_side":"left"}"#;
        let Some(Message::Visit(Visit::Visit { fling, .. })) = Message::parse(old) else { panic!() };
        assert_eq!(fling, None);
        // 던지지 않은 방문·떠남은 예전 판과 같은 모양 (fling 필드 없음)
        assert!(!String::from_utf8(Message::Visit(Visit::leave(3)).to_json()).unwrap().contains("fling"));
        // 깨진 fling 하나 때문에 방문 전체를 버리지 않는다
        let bad = br#"{"type":"leave","visit_id":3,"fling":{"height":"x"}}"#;
        assert_eq!(Message::parse(bad), Some(Message::Visit(Visit::leave(3))));
    }

    #[test]
    fn ping_can_carry_an_ask_that_old_peers_ignore() {
        let ask = Message::Control(Control::Ping { ask: Some("invite".into()) });
        assert_eq!(ask.to_json(), br#"{"type":"ping","ask":"invite"}"#.to_vec());
        assert_eq!(Message::parse(&ask.to_json()), Some(ask));
        // 평소 ping은 예전 판과 같은 모양
        assert_eq!(Message::Control(Control::Ping { ask: None }).to_json(), br#"{"type":"ping"}"#.to_vec());
        assert_eq!(Message::parse(br#"{"type":"ping"}"#), Some(Message::Control(Control::Ping { ask: None })));
        // 예전 판의 Control(Ping은 필드 없는 변형)도 ask가 붙은 ping을 그냥 ping으로 읽는다 (연결을 끊지 않는다)
        #[derive(Debug, PartialEq, Deserialize)]
        #[serde(tag = "type", rename_all = "snake_case")]
        enum OldControl {
            Unfriend,
            Ping,
        }
        assert_eq!(serde_json::from_slice::<OldControl>(br#"{"type":"ping","ask":"invite"}"#).unwrap(), OldControl::Ping);
    }

    #[test]
    fn friend_cap() {
        assert_eq!(MAX_FRIENDS, 50);
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
