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

/// The top or bottom screen edge (friends above / below, v0.3.6). Sent as an
/// optional `vside` next to a `Side` that older versions understand (they
/// see a plain left/right entry); when present it wins.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VSide {
    Up,
    Down,
}

/// A required `enter_side`; an unknown value (a newer edge) reads as `left`
/// instead of dropping the whole message.
fn lenient_side<'de, D: serde::Deserializer<'de>>(d: D) -> Result<Side, D::Error> {
    let v = serde_json::Value::deserialize(d)?;
    Ok(serde_json::from_value(v).unwrap_or(Side::Left))
}

/// An optional `side`; an unknown value is dropped instead of the whole message.
fn lenient_opt_side<'de, D: serde::Deserializer<'de>>(d: D) -> Result<Option<Side>, D::Error> {
    let v = Option::<serde_json::Value>::deserialize(d)?;
    Ok(v.and_then(|v| serde_json::from_value(v).ok()))
}

/// An optional `vside`; an unknown value is dropped (the plain `side` stands).
fn lenient_vside<'de, D: serde::Deserializer<'de>>(d: D) -> Result<Option<VSide>, D::Error> {
    let v = Option::<serde_json::Value>::deserialize(d)?;
    Ok(v.and_then(|v| serde_json::from_value(v).ok()))
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

/// Longest note (characters) a visiting pet carries; the receiver sanitizes
/// and clamps again.
pub const MAX_NOTE: usize = 80;

/// An optional `note`; anything but a string is dropped instead of the whole message.
fn lenient_note<'de, D: serde::Deserializer<'de>>(d: D) -> Result<Option<String>, D::Error> {
    let v = Option::<serde_json::Value>::deserialize(d)?;
    Ok(match v {
        Some(serde_json::Value::String(s)) => Some(s),
        _ => None,
    })
}

/// The owner of a pet visiting the receiver steers it (control mode): `on` =
/// control mode is on, `keys` = held keys as bits (left 1, right 2, up 4,
/// down 8, jump 16, run 32; unknown bits ignored). Carried in a ping so older
/// versions ignore it. Sent on change plus a 5 Hz keepalive while keys are
/// held; the receiver applies it only to that friend's own guest with
/// `visit_id` and releases the keys when updates stop.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Steer {
    pub visit_id: u64,
    #[serde(default)]
    pub on: bool,
    #[serde(default)]
    pub keys: u8,
}

/// An optional `steer`; a malformed one is dropped instead of the whole ping.
fn lenient_steer<'de, D: serde::Deserializer<'de>>(d: D) -> Result<Option<Steer>, D::Error> {
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
        #[serde(deserialize_with = "lenient_side")]
        enter_side: Side,
        /// It enters through the receiver's top/bottom edge instead (the
        /// sender is below/above); `enter_side` then holds a left/right
        /// fallback for older versions.
        #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "lenient_vside")]
        vside: Option<VSide>,
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
        /// A short message the owner attached to the pet (shown as a 💌 on the
        /// receiver's screen). Re-sent unchanged by the owner when the pet roams
        /// on; older versions ignore it.
        #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "lenient_note")]
        note: Option<String>,
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
        #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "lenient_opt_side")]
        side: Option<Side>,
        /// It left through the top/bottom edge instead (`side` then holds a
        /// left/right fallback for older owners).
        #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "lenient_vside")]
        vside: Option<VSide>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        homesick_in: Option<f64>,
        /// The guest was thrown out through that edge (home, or on to `next`).
        #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "lenient_fling")]
        fling: Option<Fling>,
        /// A reply the host wrote to the owner when sending the pet home (only
        /// on a homecoming `Leave`, at most once per visit; the owner accepts
        /// it only for its own away pet on that host's link). Older owners
        /// ignore it and the pet just comes home.
        #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "lenient_note")]
        note: Option<String>,
    },
    ComeHome {
        visit_id: u64,
    },
}

impl Visit {
    /// A plain "went home" `Leave` (same wire shape as protocol v1 without roaming).
    pub fn leave(visit_id: u64) -> Visit {
        Visit::Leave { visit_id, next: None, side: None, vside: None, homesick_in: None, fling: None, note: None }
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
    /// extra field as a plain ping and ignore it (also used for `steer`).
    Ping {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        ask: Option<String>,
        /// Steering for our pet visiting the receiver (control mode).
        #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "lenient_steer")]
        steer: Option<Steer>,
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
            vside: None,
            via: None,
            homesick_in: None,
            fling: None,
            note: None,
        });
        let v: serde_json::Value = serde_json::from_slice(&m.to_json()).unwrap();
        assert_eq!(v["type"], "visit");
        assert_eq!(v["enter_side"], "left");
        assert_eq!(Message::parse(&m.to_json()), Some(m));
    }

    #[test]
    fn up_down_edges_ride_next_to_a_side_old_versions_understand() {
        let up = Message::Visit(Visit::Visit {
            visit_id: 7,
            species: "frog".into(),
            stage: "child".into(),
            scale: 1.0,
            personality: "playful".into(),
            owner: "Ann".into(),
            enter_side: Side::Left,
            vside: Some(VSide::Down),
            via: None,
            homesick_in: None,
            fling: None,
            note: None,
        });
        let v: serde_json::Value = serde_json::from_slice(&up.to_json()).unwrap();
        assert_eq!((v["enter_side"].as_str(), v["vside"].as_str()), (Some("left"), Some("down")));
        assert_eq!(Message::parse(&up.to_json()), Some(up.clone()));
        // 예전 판은 vside를 모르고 왼쪽에서 들어오는 보통 방문으로 읽는다
        #[derive(Deserialize)]
        #[serde(tag = "type", rename_all = "snake_case")]
        enum OldVisit {
            Visit { visit_id: u64, enter_side: Side },
            Leave { visit_id: u64, side: Option<Side> },
        }
        let OldVisit::Visit { visit_id, enter_side } = serde_json::from_slice::<OldVisit>(&up.to_json()).unwrap() else { panic!() };
        assert_eq!((visit_id, enter_side), (7, Side::Left));
        let leave = Message::Visit(Visit::Leave { visit_id: 9, next: Some("c".into()), side: Some(Side::Right), vside: Some(VSide::Up), homesick_in: None, fling: None, note: None });
        let j: serde_json::Value = serde_json::from_slice(&leave.to_json()).unwrap();
        assert_eq!((j["side"].as_str(), j["vside"].as_str()), (Some("right"), Some("up")));
        assert_eq!(Message::parse(&leave.to_json()), Some(leave.clone()));
        let OldVisit::Leave { visit_id, side } = serde_json::from_slice::<OldVisit>(&leave.to_json()).unwrap() else { panic!() };
        assert_eq!((visit_id, side), (9, Some(Side::Right)));
        // vside가 없는 예전 메시지는 그대로 (위/아래 아님)
        let old = br#"{"type":"visit","visit_id":1,"species":"cat","stage":"child","scale":1.0,"personality":"friendly","owner":"A","enter_side":"right"}"#;
        let Some(Message::Visit(Visit::Visit { enter_side, vside, .. })) = Message::parse(old) else { panic!("old visit must parse") };
        assert_eq!((enter_side, vside), (Side::Right, None));
    }

    #[test]
    fn unknown_edge_values_never_drop_the_message() {
        // 모르는 vside는 버리고 side를 쓴다, 모르는 enter_side는 왼쪽, 모르는 leave side는 없음
        let odd = br#"{"type":"visit","visit_id":1,"species":"cat","stage":"child","scale":1.0,"personality":"friendly","owner":"A","enter_side":"diagonal","vside":"sideways"}"#;
        let Some(Message::Visit(Visit::Visit { enter_side, vside, .. })) = Message::parse(odd) else { panic!("odd visit must parse") };
        assert_eq!((enter_side, vside), (Side::Left, None));
        let up_side = br#"{"type":"visit","visit_id":1,"species":"cat","stage":"child","scale":1.0,"personality":"friendly","owner":"A","enter_side":"up","vside":7}"#;
        assert!(matches!(Message::parse(up_side), Some(Message::Visit(Visit::Visit { enter_side: Side::Left, vside: None, .. }))));
        let leave = br#"{"type":"leave","visit_id":3,"next":"x","side":"up","vside":"north"}"#;
        assert_eq!(
            Message::parse(leave),
            Some(Message::Visit(Visit::Leave { visit_id: 3, next: Some("x".into()), side: None, vside: None, homesick_in: None, fling: None, note: None }))
        );
        let leave = br#"{"type":"leave","visit_id":3,"side":"left","vside":"down"}"#;
        assert!(matches!(Message::parse(leave), Some(Message::Visit(Visit::Leave { side: Some(Side::Left), vside: Some(VSide::Down), .. }))));
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
            Message::Control(Control::Ping { ask: None, steer: None }),
            Message::Visit(Visit::leave(3)),
            Message::Visit(Visit::ComeHome { visit_id: 4 }),
        ] {
            assert_eq!(Message::parse(&m.to_json()), Some(m));
        }
        assert_eq!(Message::parse(br#"{"type":"ping"}"#), Some(Message::Control(Control::Ping { ask: None, steer: None })));
    }

    #[test]
    fn leave_can_name_the_next_friend_and_stays_backward_compatible() {
        let hop = Message::Visit(Visit::Leave { visit_id: 9, next: Some("xx".into()), side: Some(Side::Left), vside: None, homesick_in: Some(42.5), fling: None, note: None });
        let v: serde_json::Value = serde_json::from_slice(&hop.to_json()).unwrap();
        assert_eq!((v["next"].as_str(), v["side"].as_str(), v["homesick_in"].as_f64()), (Some("xx"), Some("left"), Some(42.5)));
        assert_eq!(Message::parse(&hop.to_json()), Some(hop));
        // 예전 판의 leave (next 없음) = 집으로
        assert_eq!(
            Message::parse(br#"{"type":"leave","visit_id":3}"#),
            Some(Message::Visit(Visit::Leave { visit_id: 3, next: None, side: None, vside: None, homesick_in: None, fling: None, note: None }))
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
            vside: None,
            via: Some("bb".into()),
            homesick_in: Some(10.0),
            fling: None,
            note: None,
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
            vside: None,
            via: None,
            homesick_in: None,
            fling: Some(f),
            note: None,
        });
        let j: serde_json::Value = serde_json::from_slice(&v.to_json()).unwrap();
        assert_eq!((j["fling"]["height"].as_f64(), j["fling"]["vx"].as_f64(), j["fling"]["vy"].as_f64()), (Some(0.25), Some(-640.0), Some(-120.5)));
        assert_eq!(Message::parse(&v.to_json()), Some(v));
        let back = Message::Visit(Visit::Leave { visit_id: 5, next: None, side: Some(Side::Left), vside: None, homesick_in: None, fling: Some(f), note: None });
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

    fn visit_with_note(note: Option<&str>) -> Message {
        Message::Visit(Visit::Visit {
            visit_id: 8,
            species: "cat".into(),
            stage: "child".into(),
            scale: 1.0,
            personality: "friendly".into(),
            owner: "A".into(),
            enter_side: Side::Left,
            vside: None,
            via: None,
            homesick_in: None,
            fling: None,
            note: note.map(str::to_string),
        })
    }

    #[test]
    fn visit_can_carry_a_note_and_old_peers_still_parse() {
        let m = visit_with_note(Some("안녕 🐱 놀러 왔어!"));
        let j: serde_json::Value = serde_json::from_slice(&m.to_json()).unwrap();
        assert_eq!(j["note"].as_str(), Some("안녕 🐱 놀러 왔어!"));
        assert_eq!(Message::parse(&m.to_json()), Some(m));
        // 편지 없는 방문은 예전 판과 같은 모양 (note 필드 없음)
        assert!(!String::from_utf8(visit_with_note(None).to_json()).unwrap().contains("note"));
        // 편지를 모르는 예전 판이 보낸 방문: note 없음
        let old = br#"{"type":"visit","visit_id":8,"species":"cat","stage":"child","scale":1.0,"personality":"friendly","owner":"A","enter_side":"left"}"#;
        assert_eq!(Message::parse(old), Some(visit_with_note(None)));
        // 예전 판(note를 모르는 Visit)도 편지가 실린 방문을 그냥 방문으로 읽는다
        #[derive(Debug, Deserialize)]
        #[serde(tag = "type", rename_all = "snake_case")]
        enum OldVisit {
            Visit { visit_id: u64, owner: String },
        }
        let OldVisit::Visit { visit_id, owner } = serde_json::from_slice::<OldVisit>(&visit_with_note(Some("hi")).to_json()).unwrap();
        assert_eq!((visit_id, owner.as_str()), (8, "A"));
        // 글자가 아닌 note 하나 때문에 방문 전체를 버리지 않는다
        let bad = br#"{"type":"visit","visit_id":8,"species":"cat","stage":"child","scale":1.0,"personality":"friendly","owner":"A","enter_side":"left","note":{"x":1}}"#;
        assert_eq!(Message::parse(bad), Some(visit_with_note(None)));
        let num = br#"{"type":"visit","visit_id":8,"species":"cat","stage":"child","scale":1.0,"personality":"friendly","owner":"A","enter_side":"left","note":5}"#;
        assert_eq!(Message::parse(num), Some(visit_with_note(None)));
    }

    #[test]
    fn leave_can_carry_a_reply_and_old_owners_still_parse() {
        let reply = |note: Option<&str>| Message::Visit(Visit::Leave { visit_id: 4, next: None, side: None, vside: None, homesick_in: None, fling: None, note: note.map(str::to_string) });
        let m = reply(Some("잘 놀다 가 🐶"));
        let j: serde_json::Value = serde_json::from_slice(&m.to_json()).unwrap();
        assert_eq!(j["note"].as_str(), Some("잘 놀다 가 🐶"));
        assert_eq!(Message::parse(&m.to_json()), Some(m.clone()));
        // 답장 없는 귀환은 예전 판과 같은 모양
        assert_eq!(reply(None).to_json(), br#"{"type":"leave","visit_id":4}"#.to_vec());
        // 답장을 모르는 예전 주인도 그냥 귀환으로 읽는다
        #[derive(Deserialize)]
        #[serde(tag = "type", rename_all = "snake_case")]
        enum OldVisit {
            Leave { visit_id: u64, next: Option<String> },
        }
        let OldVisit::Leave { visit_id, next } = serde_json::from_slice::<OldVisit>(&m.to_json()).unwrap();
        assert_eq!((visit_id, next), (4, None));
        // 글자가 아닌 note 하나 때문에 귀환을 버리지 않는다
        assert_eq!(Message::parse(br#"{"type":"leave","visit_id":4,"note":[1,2]}"#), Some(reply(None)));
        assert_eq!(Message::parse(br#"{"type":"leave","visit_id":4,"note":null}"#), Some(reply(None)));
    }

    #[test]
    fn ping_can_carry_an_ask_that_old_peers_ignore() {
        let ask = Message::Control(Control::Ping { ask: Some("invite".into()), steer: None });
        assert_eq!(ask.to_json(), br#"{"type":"ping","ask":"invite"}"#.to_vec());
        assert_eq!(Message::parse(&ask.to_json()), Some(ask));
        // 평소 ping은 예전 판과 같은 모양
        assert_eq!(Message::Control(Control::Ping { ask: None, steer: None }).to_json(), br#"{"type":"ping"}"#.to_vec());
        assert_eq!(Message::parse(br#"{"type":"ping"}"#), Some(Message::Control(Control::Ping { ask: None, steer: None })));
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
    fn ping_can_carry_a_steer_that_old_peers_ignore() {
        let st = Steer { visit_id: 42, on: true, keys: 0b10_0010 };
        let m = Message::Control(Control::Ping { ask: None, steer: Some(st) });
        let j: serde_json::Value = serde_json::from_slice(&m.to_json()).unwrap();
        assert_eq!((j["type"].as_str(), j["steer"]["visit_id"].as_u64(), j["steer"]["on"].as_bool(), j["steer"]["keys"].as_u64()), (Some("ping"), Some(42), Some(true), Some(34)));
        assert_eq!(Message::parse(&m.to_json()), Some(m.clone()));
        // ask와 함께 실려도 된다
        let both = Message::Control(Control::Ping { ask: Some("invite".into()), steer: Some(st) });
        assert_eq!(Message::parse(&both.to_json()), Some(both));
        // 조작 없는 ping은 예전 판과 같은 모양
        assert!(!String::from_utf8(Message::Control(Control::Ping { ask: None, steer: None }).to_json()).unwrap().contains("steer"));
        // 예전 판의 Control도 steer가 붙은 ping을 그냥 ping으로 읽는다 (연결을 끊지 않는다)
        #[derive(Debug, PartialEq, Deserialize)]
        #[serde(tag = "type", rename_all = "snake_case")]
        enum OldControl {
            Ping {
                #[serde(default)]
                ask: Option<String>,
            },
        }
        assert_eq!(serde_json::from_slice::<OldControl>(&m.to_json()).unwrap(), OldControl::Ping { ask: None });
        // 깨진 steer 하나 때문에 ping을 버리지 않는다 (steer만 버린다)
        for bad in [
            &br#"{"type":"ping","steer":{"visit_id":"x"}}"#[..],
            br#"{"type":"ping","steer":{"visit_id":1,"keys":300}}"#,
            br#"{"type":"ping","steer":5}"#,
        ] {
            assert_eq!(Message::parse(bad), Some(Message::Control(Control::Ping { ask: None, steer: None })), "{}", String::from_utf8_lossy(bad));
        }
        // on·keys가 없으면 꺼짐·안 눌림
        assert_eq!(
            Message::parse(br#"{"type":"ping","steer":{"visit_id":3}}"#),
            Some(Message::Control(Control::Ping { ask: None, steer: Some(Steer { visit_id: 3, on: false, keys: 0 }) }))
        );
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
