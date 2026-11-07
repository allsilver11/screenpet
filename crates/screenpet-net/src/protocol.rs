//! Wire formats: the UDP discovery announcement and the JSON messages carried
//! (encrypted) inside Noise transport messages, each tagged by a `type` field.

use serde::{Deserialize, Serialize};

/// Protocol version carried in announcements and `hello`. 2 = knows the
/// `ladder_*` messages (v0.5.0). 3 = knows `device_friends`, `my_devices` and
/// the `device_*` code-linking messages (linked devices, v0.5.3). Older versions close a link on any message `type`
/// they do not know, so features with new message types check the peer's
/// version first (`Net::peer_version`).
pub const PROTOCOL_VERSION: u32 = 3;
/// Lowest peer version that may be sent `LadderMsg`s.
pub const LADDER_MIN_VERSION: u32 = 2;
/// Lowest peer version that may be sent `device_friends`, `my_devices` and `device_*`.
pub const DEVICES_MIN_VERSION: u32 = 3;
/// Most devices one friend introduces (`my_devices`), in one message and in
/// all: a person links at most two computers, so a friend has one other.
pub const MAX_DEVICES: usize = 1;
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

/// Any optional field: a value that doesn't fit is dropped, never the whole
/// message (a newer or older peer may send something else there).
fn lenient<'de, T: serde::de::DeserializeOwned, D: serde::Deserializer<'de>>(d: D) -> Result<Option<T>, D::Error> {
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

/// Longest note (characters) a visiting pet carries; the receiver sanitizes
/// and clamps again.
pub const MAX_NOTE: usize = 80;

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

/// A thrown object (trash toy) flying over to the receiver's screen. Objects
/// have no owner and never go home: the sender forgets it, the receiver keeps
/// it as one of its objects. Carried in a ping so older versions ignore it
/// (the object is simply lost there; the link stays). `kind`, `fling` and
/// `note` are checked by the receiver's app. `side` = the receiver's edge it
/// flies in through (`vside` wins when present, like `Visit::enter_side`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Toss {
    pub kind: String,
    #[serde(default = "left_side", deserialize_with = "lenient_side")]
    pub side: Side,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "lenient")]
    pub vside: Option<VSide>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "lenient")]
    pub fling: Option<Fling>,
    /// The post-it stuck on the object.
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "lenient")]
    pub note: Option<String>,
    /// The receiver refused it (focusing or in the house) and threw it back to
    /// the sender. Never bounced again; older versions ignore the flag.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub bounced: bool,
    /// How big the thing is drawn (a poop goes with the pet's size). The
    /// receiver limits it; older versions ignore it.
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "lenient")]
    pub scale: Option<f64>,
    /// A coloured poop ("pink", "rainbow"), sent as kind "poop" so older
    /// versions get a plain one. The receiver's app checks the name.
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "lenient")]
    pub look: Option<String>,
}

/// A list whose malformed entries are skipped (never the whole message);
/// anything but an array reads as empty. At most 256 entries are kept.
fn lenient_list<'de, T: serde::de::DeserializeOwned, D: serde::Deserializer<'de>>(d: D) -> Result<Vec<T>, D::Error> {
    let v = serde_json::Value::deserialize(d)?;
    let items = match v {
        serde_json::Value::Array(items) => items,
        _ => Vec::new(),
    };
    Ok(items.into_iter().take(256).filter_map(|v| serde_json::from_value(v).ok()).collect())
}

/// A string that may be missing or something else (read as empty).
fn lenient_bool<'de, D: serde::Deserializer<'de>>(d: D) -> Result<bool, D::Error> {
    let v = serde_json::Value::deserialize(d)?;
    Ok(v.as_bool().unwrap_or(false))
}

fn lenient_string<'de, D: serde::Deserializer<'de>>(d: D) -> Result<String, D::Error> {
    let v = serde_json::Value::deserialize(d)?;
    Ok(v.as_str().map(str::to_string).unwrap_or_default())
}

/// One of my friends, as shared with my other computer (`device_friends`).
/// `key` = the friend's static public key (64 hex digits). `name` and `side`
/// are only first values for the receiver's list (it checks them).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WireFriend {
    pub id: String,
    pub key: String,
    #[serde(default, deserialize_with = "lenient_string")]
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "lenient")]
    pub side: Option<String>,
}

/// 📱 One pet's care as my two computers share it (`device_state`): the
/// receiver checks the shape (`devices::clean_state`), the app the values.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WireCare {
    pub pet: String,
    pub stage: String,
    pub stamina: f64,
    pub fullness: f64,
    #[serde(default, deserialize_with = "lenient_bool")]
    pub locked: bool,
}

/// One of my other computers, as introduced to a friend (`my_devices`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WireDevice {
    pub id: String,
    pub key: String,
}

fn left_side() -> Side {
    Side::Left
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
        #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "lenient")]
        vside: Option<VSide>,
        /// The pet walked over from this friend's screen (owner's friend id);
        /// the receiver enters it from the edge it links to that friend, if any.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        via: Option<String>,
        /// Seconds left before the pet gets homesick (continues across hops).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        homesick_in: Option<f64>,
        /// The pet was thrown across: it flies in keeping this motion.
        #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "lenient")]
        fling: Option<Fling>,
        /// A short message the owner attached to the pet (shown as a 💌 on the
        /// receiver's screen). Re-sent unchanged by the owner when the pet roams
        /// on; older versions ignore it.
        #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "lenient")]
        note: Option<String>,
        /// A small gift (catalog id) the pet will leave on the receiver's
        /// screen. Unknown or foreign ids are dropped by the receiver, which
        /// also limits how often a friend's gift is accepted; never re-sent
        /// when the pet roams on; older versions ignore it.
        #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "lenient")]
        gift: Option<String>,
        /// How the owner shows this pet: "pixel" (absent = round). Purely
        /// cosmetic; the receiver validates it and shows round when it does not
        /// know the value or has no such art. Older versions ignore it.
        #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "lenient")]
        style: Option<String>,
        /// A friend fed this pet and it is temporarily bigger (multiplier). The
        /// receiver limits it; older versions ignore it.
        #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "lenient")]
        boost: Option<f64>,
        /// 📱 The pet belongs to the sender's other computer (this install id),
        /// which may be on standby: the sender carries it (only to
        /// `DEVICES_MIN_VERSION` peers). The receiver takes it only if it knows
        /// both ids as one person's computers, and answers through whichever is
        /// connected.
        #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "lenient")]
        of: Option<String>,
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
        #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "lenient")]
        side: Option<Side>,
        /// It left through the top/bottom edge instead (`side` then holds a
        /// left/right fallback for older owners).
        #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "lenient")]
        vside: Option<VSide>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        homesick_in: Option<f64>,
        /// The guest was thrown out through that edge (home, or on to `next`).
        #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "lenient")]
        fling: Option<Fling>,
        /// A reply the host wrote to the owner when sending the pet home (only
        /// on a homecoming `Leave`, at most once per visit; the owner accepts
        /// it only for its own away pet on that host's link). Older owners
        /// ignore it and the pet just comes home.
        #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "lenient")]
        note: Option<String>,
        /// The host fed the guest: how much bigger it goes on (the owner limits it).
        #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "lenient")]
        boost: Option<f64>,
    },
    ComeHome {
        visit_id: u64,
    },
}

impl Visit {
    /// A plain "went home" `Leave` (same wire shape as protocol v1 without roaming).
    pub fn leave(visit_id: u64) -> Visit {
        Visit::Leave { visit_id, next: None, side: None, vside: None, homesick_in: None, fling: None, note: None, boost: None }
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
    /// extra field as a plain ping and ignore it (also used for `steer` and `toss`).
    Ping {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        ask: Option<String>,
        /// Steering for our pet visiting the receiver (control mode).
        #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "lenient")]
        steer: Option<Steer>,
        /// An object thrown over to the receiver's screen.
        #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "lenient")]
        toss: Option<Toss>,
    },
    /// My friends, sent to my linked computer once it proved our secret on
    /// this link (only to `DEVICES_MIN_VERSION` peers). The receiver takes it
    /// only from a link where the proof checked out, and only adds friends it
    /// does not know. `want` = please send yours back.
    DeviceFriends {
        #[serde(default, deserialize_with = "lenient_list")]
        friends: Vec<WireFriend>,
        #[serde(default, skip_serializing_if = "std::ops::Not::not")]
        want: bool,
    },
    /// My other computer (at most `MAX_DEVICES`), introduced to a friend that
    /// is not one of them (only to `DEVICES_MIN_VERSION` peers). The friend adds
    /// them as friends that leave together with me.
    MyDevices {
        #[serde(default, deserialize_with = "lenient_list")]
        devices: Vec<WireDevice>,
    },
    /// 📱 Linking with a code (all `device_*` messages only to
    /// `DEVICES_MIN_VERSION` peers). The enterer typed the code the receiver
    /// is showing for it: `nonce` = its fresh 32-byte nonce nb (hex). The code
    /// itself never goes on the wire.
    DeviceCode { nonce: String },
    /// The code holder's fresh nonce na, only if it holds an unexpired code
    /// for the sender (`device_no` otherwise).
    DeviceCodeNonce { nonce: String },
    /// The enterer's proof that it knows the code K = BLAKE2s(label || code):
    /// BLAKE2s(label || "enter" || K || nb || na || enterer key || holder key).
    DeviceCodeProof { proof: String },
    /// The holder checked that proof and now proves the code too (it never
    /// proves first, so a wrong guess learns nothing):
    /// BLAKE2s(label || "hold" || K || nb || na || holder key || enterer key).
    /// Both then store S = BLAKE2s(label || K || holder key || enterer key || na || nb).
    DeviceCodeOk { proof: String },
    /// The code did not link: `reason` = "wrong" (`left` tries remain),
    /// "locked" (too many wrong tries: the code is gone), "none" (no code for
    /// you, expired or used), "taken" (I am linked to another computer).
    DeviceNo {
        #[serde(default, deserialize_with = "lenient_string")]
        reason: String,
        #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "lenient")]
        left: Option<u32>,
    },
    /// On every friend link between two linked computers, each sends a fresh
    /// nonce, and answers the other's with `device_proof` =
    /// BLAKE2s(label || S || prover key || verifier key || verifier nonce ||
    /// prover nonce). Friend lists flow only once that proof checked out.
    DeviceNonce { nonce: String },
    DeviceProof { proof: String },
    /// Forget our link (the sender unlinked, or holds no secret for us); also
    /// "cancelled" while confirming a matched code.
    DeviceUnlink,
    /// After a matched code, the sender's user pressed 확인 ("본인의 PC가
    /// 맞습니까?"). Only once both sent it does the link count.
    DeviceConfirm,
    /// 📱 Which of my two linked computers plays (only on a link where the
    /// secret was proven both ways, only to `DEVICES_MIN_VERSION` peers; never
    /// to friends). The one with the highest (`term`, install id) plays; the
    /// other is on standby (no friend links). `you` = the receiver holds that
    /// term, else the sender. A claim is the sender's term + 1. `idle` (seconds
    /// since the sender's user last used it) is only in the first one on each
    /// proven link: the computer used last then claims.
    DeviceActive {
        #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "lenient")]
        term: Option<u64>,
        #[serde(default, skip_serializing_if = "std::ops::Not::not", deserialize_with = "lenient_bool")]
        you: bool,
        #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "lenient")]
        idle: Option<u64>,
        /// This claim settles who plays as the two meet again (the one used
        /// last): the receiver hands nothing over (see `device_state`).
        #[serde(default, skip_serializing_if = "std::ops::Not::not", deserialize_with = "lenient_bool")]
        meet: bool,
    },
    /// 📱 The selected pet and every pet's care, from the computer that plays
    /// to my other computer (same rules as `device_active`). The one on
    /// standby takes it from the computer that plays only. `handover` = the
    /// sender just stopped playing because the receiver claimed: its last
    /// state, taken unless the receiver changed its own meanwhile.
    DeviceState {
        #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "lenient")]
        pet: Option<String>,
        #[serde(default, deserialize_with = "lenient_list")]
        care: Vec<WireCare>,
        #[serde(default, skip_serializing_if = "std::ops::Not::not", deserialize_with = "lenient_bool")]
        handover: bool,
    },
}

/// One player of a locked ladder game, as the host relays it. `token` is a
/// per-game random id (never the install id: players need not be friends of
/// each other).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WirePlayer {
    pub token: String,
    pub name: String,
    pub pet: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub style: Option<String>,
    pub commit: String,
}

/// One result slot at the bottom of a ladder. `mood`: "good" | "bad" | "none".
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WireResult {
    pub text: String,
    #[serde(default)]
    pub mood: String,
}

/// Ladder game messages between the host and each player (friend links only,
/// and only to peers at `LADDER_MIN_VERSION` or newer). Validation of lengths
/// and hex is done by `ladder`'s state machines, not here.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum LadderMsg {
    LadderInvite { game_id: String, host: String, results: Vec<WireResult>, invited: u8 },
    LadderJoin {
        game_id: String,
        token: String,
        commit: String,
        name: String,
        pet: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        style: Option<String>,
    },
    /// `reason`: "no" | "busy" | "playing"
    LadderDecline { game_id: String, reason: String },
    /// `host` = the host's seat in `players` (sorted by token).
    LadderLock { game_id: String, host: u8, players: Vec<WirePlayer>, results: Vec<WireResult> },
    LadderReveal { game_id: String, nonce: String },
    /// Every player's nonce, in `players` order (the host's included).
    LadderReveals { game_id: String, nonces: Vec<String> },
    /// Show the descent now. `mode`: "one_by_one" | "all". Cosmetic only.
    LadderStart { game_id: String, mode: String },
    /// `reason`: "host" | "left" | "timeout" | "invalid"; `who` = a player token.
    LadderCancel {
        game_id: String,
        reason: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        who: Option<String>,
    },
}

/// Any message. Untagged at this level: each inner enum carries its own
/// `type` tag, so an unknown `type` fails to parse as any of them.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Message {
    Control(Control),
    Visit(Visit),
    Ladder(LadderMsg),
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

    /// A `Visit::Visit` message with these fields changed from an ordinary one.
    macro_rules! visit {
        ($($field:ident: $value:expr),* $(,)?) => {{
            #[allow(unused_mut)]
            let mut v = Visit::Visit {
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
                note: None,
                gift: None,
                style: None,
                boost: None,
                of: None,
            };
            #[allow(unused_variables)]
            if let Visit::Visit { $($field,)* .. } = &mut v {
                $(*$field = $value;)*
            }
            Message::Visit(v)
        }};
    }

    #[test]
    fn visit_wire_format_uses_type_tag() {
        let m = visit!(visit_id: 7, scale: 1.5, personality: "playful".into(), owner: "Ann".into());
        let v: serde_json::Value = serde_json::from_slice(&m.to_json()).unwrap();
        assert_eq!(v["type"], "visit");
        assert_eq!(v["enter_side"], "left");
        assert_eq!(Message::parse(&m.to_json()), Some(m));
    }

    #[test]
    fn up_down_edges_ride_next_to_a_side_old_versions_understand() {
        let up = visit!(visit_id: 7, species: "frog".into(), personality: "playful".into(), owner: "Ann".into(), vside: Some(VSide::Down));
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
        let leave = Message::Visit(Visit::Leave { visit_id: 9, next: Some("c".into()), side: Some(Side::Right), vside: Some(VSide::Up), homesick_in: None, fling: None, note: None, boost: None });
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
            Some(Message::Visit(Visit::Leave { visit_id: 3, next: Some("x".into()), side: None, vside: None, homesick_in: None, fling: None, note: None, boost: None }))
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
            Message::Control(Control::Ping { ask: None, steer: None, toss: None }),
            Message::Visit(Visit::leave(3)),
            Message::Visit(Visit::ComeHome { visit_id: 4 }),
        ] {
            assert_eq!(Message::parse(&m.to_json()), Some(m));
        }
        assert_eq!(Message::parse(br#"{"type":"ping"}"#), Some(Message::Control(Control::Ping { ask: None, steer: None, toss: None })));
    }

    #[test]
    fn ladder_messages_round_trip() {
        let player = WirePlayer { token: "t1".into(), name: "나비".into(), pet: "cat".into(), style: Some("pixel".into()), commit: "c1".into() };
        let result = WireResult { text: "당첨".into(), mood: "good".into() };
        for m in [
            LadderMsg::LadderInvite { game_id: "g".into(), host: "Ann".into(), results: vec![result.clone()], invited: 3 },
            LadderMsg::LadderJoin { game_id: "g".into(), token: "t".into(), commit: "c".into(), name: "B".into(), pet: "dog".into(), style: None },
            LadderMsg::LadderDecline { game_id: "g".into(), reason: "busy".into() },
            LadderMsg::LadderLock { game_id: "g".into(), host: 0, players: vec![player], results: vec![result] },
            LadderMsg::LadderReveal { game_id: "g".into(), nonce: "n".into() },
            LadderMsg::LadderReveals { game_id: "g".into(), nonces: vec!["n1".into(), "n2".into()] },
            LadderMsg::LadderStart { game_id: "g".into(), mode: "all".into() },
            LadderMsg::LadderCancel { game_id: "g".into(), reason: "left".into(), who: Some("t".into()) },
        ] {
            let m = Message::Ladder(m);
            assert_eq!(Message::parse(&m.to_json()), Some(m));
        }
        let v: serde_json::Value =
            serde_json::from_slice(&Message::Ladder(LadderMsg::LadderReveal { game_id: "g".into(), nonce: "n".into() }).to_json()).unwrap();
        assert_eq!(v["type"], "ladder_reveal");
        // 모르는 type은 여전히 어느 쪽으로도 읽히지 않는다 (예전 판이 연결을 끊는 이유)
        assert_eq!(Message::parse(br#"{"type":"ladder_dance","game_id":"g"}"#), None);
    }

    #[test]
    fn hello_from_either_version_parses() {
        for v in [1, 2, 99] {
            let raw = format!(r#"{{"type":"hello","id":"a","name":"A","v":{v}}}"#);
            assert!(matches!(Message::parse(raw.as_bytes()), Some(Message::Control(Control::Hello { v: got, .. })) if got == v));
        }
    }

    #[test]
    fn leave_can_name_the_next_friend_and_stays_backward_compatible() {
        let hop = Message::Visit(Visit::Leave { visit_id: 9, next: Some("xx".into()), side: Some(Side::Left), vside: None, homesick_in: Some(42.5), fling: None, note: None, boost: None });
        let v: serde_json::Value = serde_json::from_slice(&hop.to_json()).unwrap();
        assert_eq!((v["next"].as_str(), v["side"].as_str(), v["homesick_in"].as_f64()), (Some("xx"), Some("left"), Some(42.5)));
        assert_eq!(Message::parse(&hop.to_json()), Some(hop));
        // 예전 판의 leave (next 없음) = 집으로
        assert_eq!(
            Message::parse(br#"{"type":"leave","visit_id":3}"#),
            Some(Message::Visit(Visit::Leave { visit_id: 3, next: None, side: None, vside: None, homesick_in: None, fling: None, note: None, boost: None }))
        );
        // 집으로 가는 leave는 예전 판과 같은 모양
        assert_eq!(Message::Visit(Visit::leave(3)).to_json(), br#"{"type":"leave","visit_id":3}"#.to_vec());
    }

    #[test]
    fn visit_can_carry_the_previous_host_and_homesick_time() {
        let old = br#"{"type":"visit","visit_id":1,"species":"cat","stage":"child","scale":1.0,"personality":"friendly","owner":"A","enter_side":"left"}"#;
        let Some(Message::Visit(Visit::Visit { via, homesick_in, .. })) = Message::parse(old) else { panic!("old visit must parse") };
        assert_eq!((via, homesick_in), (None, None));
        let hop = visit!(visit_id: 2, enter_side: Side::Right, via: Some("bb".into()), homesick_in: Some(10.0));
        assert_eq!(Message::parse(&hop.to_json()), Some(hop));
        // 새 필드를 모르는 받는 쪽도 읽을 수 있다 (모르는 필드는 무시)
        assert!(Message::parse(br#"{"type":"leave","visit_id":3,"next":"x","future":1}"#).is_some());
    }

    #[test]
    fn visit_and_leave_can_carry_a_throw_and_old_peers_still_parse() {
        let f = Fling { height: 0.25, vx: -640.0, vy: -120.5 };
        let v = visit!(visit_id: 5, enter_side: Side::Right, fling: Some(f));
        let j: serde_json::Value = serde_json::from_slice(&v.to_json()).unwrap();
        assert_eq!((j["fling"]["height"].as_f64(), j["fling"]["vx"].as_f64(), j["fling"]["vy"].as_f64()), (Some(0.25), Some(-640.0), Some(-120.5)));
        assert_eq!(Message::parse(&v.to_json()), Some(v));
        let back = Message::Visit(Visit::Leave { visit_id: 5, next: None, side: Some(Side::Left), vside: None, homesick_in: None, fling: Some(f), note: None, boost: None });
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
    fn a_visit_can_carry_a_gift_and_old_peers_ignore_it() {
        let v = visit!(gift: Some("cat_yarn_shiny".into()));
        let j: serde_json::Value = serde_json::from_slice(&v.to_json()).unwrap();
        assert_eq!(j["gift"], "cat_yarn_shiny");
        assert_eq!(Message::parse(&v.to_json()), Some(v.clone()));
        // 예전 판은 모르는 필드를 건너뛰고 방문으로 받는다
        #[derive(serde::Deserialize)]
        #[serde(tag = "type", rename_all = "snake_case")]
        enum OldVisit {
            Visit { visit_id: u64, owner: String },
        }
        let OldVisit::Visit { visit_id, owner } = serde_json::from_slice::<OldVisit>(&v.to_json()).unwrap();
        assert_eq!((visit_id, owner.as_str()), (8, "A"));
        // 선물 없는 방문은 예전과 같은 모양, 예전 판의 방문은 선물 없음
        assert!(!String::from_utf8(visit_with_note(None).to_json()).unwrap().contains("gift"));
        let old = br#"{"type":"visit","visit_id":1,"species":"cat","stage":"child","scale":1.0,"personality":"friendly","owner":"A","enter_side":"left"}"#;
        let Some(Message::Visit(Visit::Visit { gift, .. })) = Message::parse(old) else { panic!() };
        assert_eq!(gift, None);
        // 글자가 아닌 선물 하나 때문에 방문 전체를 버리지 않는다
        let bad = br#"{"type":"visit","visit_id":1,"species":"cat","stage":"child","scale":1.0,"personality":"friendly","owner":"A","enter_side":"left","gift":{"x":1}}"#;
        let Some(Message::Visit(Visit::Visit { gift, .. })) = Message::parse(bad) else { panic!("bad gift must not drop the visit") };
        assert_eq!(gift, None);
    }

    fn visit_with_note(text: Option<&str>) -> Message {
        visit!(note: text.map(str::to_string))
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
        let reply = |note: Option<&str>| Message::Visit(Visit::Leave { visit_id: 4, next: None, side: None, vside: None, homesick_in: None, fling: None, note: note.map(str::to_string), boost: None });
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
        let ask = Message::Control(Control::Ping { ask: Some("invite".into()), steer: None, toss: None });
        assert_eq!(ask.to_json(), br#"{"type":"ping","ask":"invite"}"#.to_vec());
        assert_eq!(Message::parse(&ask.to_json()), Some(ask));
        // 평소 ping은 예전 판과 같은 모양
        assert_eq!(Message::Control(Control::Ping { ask: None, steer: None, toss: None }).to_json(), br#"{"type":"ping"}"#.to_vec());
        assert_eq!(Message::parse(br#"{"type":"ping"}"#), Some(Message::Control(Control::Ping { ask: None, steer: None, toss: None })));
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
        let m = Message::Control(Control::Ping { ask: None, steer: Some(st), toss: None });
        let j: serde_json::Value = serde_json::from_slice(&m.to_json()).unwrap();
        assert_eq!((j["type"].as_str(), j["steer"]["visit_id"].as_u64(), j["steer"]["on"].as_bool(), j["steer"]["keys"].as_u64()), (Some("ping"), Some(42), Some(true), Some(34)));
        assert_eq!(Message::parse(&m.to_json()), Some(m.clone()));
        // ask와 함께 실려도 된다
        let both = Message::Control(Control::Ping { ask: Some("invite".into()), steer: Some(st), toss: None });
        assert_eq!(Message::parse(&both.to_json()), Some(both));
        // 조작 없는 ping은 예전 판과 같은 모양
        assert!(!String::from_utf8(Message::Control(Control::Ping { ask: None, steer: None, toss: None }).to_json()).unwrap().contains("steer"));
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
            assert_eq!(Message::parse(bad), Some(Message::Control(Control::Ping { ask: None, steer: None, toss: None })), "{}", String::from_utf8_lossy(bad));
        }
        // on·keys가 없으면 꺼짐·안 눌림
        assert_eq!(
            Message::parse(br#"{"type":"ping","steer":{"visit_id":3}}"#),
            Some(Message::Control(Control::Ping { ask: None, steer: Some(Steer { visit_id: 3, on: false, keys: 0 }), toss: None }))
        );
    }

    #[test]
    fn ping_can_carry_a_toss_that_old_peers_ignore() {
        let t = Toss { kind: "can".into(), side: Side::Right, vside: Some(VSide::Down), fling: Some(Fling { height: 0.5, vx: 300.0, vy: -20.0 }), note: Some("붙였어!".into()), bounced: false, scale: None, look: None };
        let m = Message::Control(Control::Ping { ask: None, steer: None, toss: Some(t) });
        let j: serde_json::Value = serde_json::from_slice(&m.to_json()).unwrap();
        assert_eq!((j["type"].as_str(), j["toss"]["kind"].as_str(), j["toss"]["side"].as_str(), j["toss"]["vside"].as_str(), j["toss"]["note"].as_str()), (Some("ping"), Some("can"), Some("right"), Some("down"), Some("붙였어!")));
        assert_eq!(Message::parse(&m.to_json()), Some(m.clone()));
        // 던지는 것 없는 ping은 예전 판과 같은 모양
        assert_eq!(Message::Control(Control::Ping { ask: None, steer: None, toss: None }).to_json(), br#"{"type":"ping"}"#.to_vec());
        // 예전 판의 Control도 toss가 붙은 ping을 그냥 ping으로 읽는다 (연결을 끊지 않는다)
        #[derive(Debug, PartialEq, Deserialize)]
        #[serde(tag = "type", rename_all = "snake_case")]
        enum OldControl {
            Ping {
                #[serde(default)]
                ask: Option<String>,
            },
        }
        assert_eq!(serde_json::from_slice::<OldControl>(&m.to_json()).unwrap(), OldControl::Ping { ask: None });
        // 깨진 toss 하나 때문에 ping을 버리지 않는다
        for bad in [&br#"{"type":"ping","toss":5}"#[..], br#"{"type":"ping","toss":{"side":"left"}}"#, br#"{"type":"ping","toss":{"kind":7}}"#] {
            assert_eq!(Message::parse(bad), Some(Message::Control(Control::Ping { ask: None, steer: None, toss: None })), "{}", String::from_utf8_lossy(bad));
        }
        // 안쪽 필드가 이상해도 물건은 살린다: 모르는 끝은 왼쪽, 깨진 fling·note는 없음
        let odd = br#"{"type":"ping","toss":{"kind":"can","side":"north","vside":3,"fling":{"height":"x"},"note":[1]}}"#;
        assert_eq!(
            Message::parse(odd),
            Some(Message::Control(Control::Ping { ask: None, steer: None, toss: Some(Toss { kind: "can".into(), side: Side::Left, vside: None, fling: None, note: None, bounced: false, scale: None, look: None }) }))
        );
        // side가 없어도 왼쪽
        // 똥 색 표시: 없으면 None, 있으면 그대로 (옛 버전은 모르는 필드를 무시한다). 글자가 아니면 버린다
        let pink = Message::parse(br#"{"type":"ping","toss":{"kind":"poop","look":"pink"}}"#);
        assert!(matches!(&pink, Some(Message::Control(Control::Ping { toss: Some(Toss { look: Some(l), .. }), .. })) if l == "pink"));
        let odd = Message::parse(br#"{"type":"ping","toss":{"kind":"poop","look":7}}"#);
        assert!(matches!(odd, Some(Message::Control(Control::Ping { toss: Some(Toss { look: None, .. }), .. }))));
        assert!(matches!(Message::parse(br#"{"type":"ping","toss":{"kind":"can"}}"#), Some(Message::Control(Control::Ping { toss: Some(Toss { side: Side::Left, .. }), .. }))));
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

    /// A pet shown in pixel style says so; old peers ignore it; anything odd is dropped, never the whole visit.
    #[test]
    fn visit_style_is_optional_and_backward_compatible() {
        let v = visit!(style: Some("pixel".into()));
        assert_eq!(Message::parse(&v.to_json()), Some(v.clone()));
        // an old peer still reads it
        #[derive(Deserialize)]
        #[serde(tag = "type", rename_all = "snake_case")]
        enum OldVisit {
            Visit { visit_id: u64 },
        }
        let OldVisit::Visit { visit_id } = serde_json::from_slice::<OldVisit>(&v.to_json()).unwrap();
        assert_eq!(visit_id, 8);
        // round sends nothing at all
        assert!(!String::from_utf8(visit_with_note(None).to_json()).unwrap().contains("style"));
        let old = br#"{"type":"visit","visit_id":1,"species":"cat","stage":"baby","scale":1.0,"personality":"aloof","owner":"a","enter_side":"left"}"#;
        let Some(Message::Visit(Visit::Visit { style, .. })) = Message::parse(old) else { panic!("old visit must parse") };
        assert_eq!(style, None);
        let bad = br#"{"type":"visit","visit_id":1,"species":"cat","stage":"baby","scale":1.0,"personality":"aloof","owner":"a","enter_side":"left","style":7}"#;
        let Some(Message::Visit(Visit::Visit { style, .. })) = Message::parse(bad) else { panic!("bad style must not drop the visit") };
        assert_eq!(style, None);
    }

    #[test]
    fn clamp_counts_characters() {
        assert_eq!(clamp_name(&"🐱".repeat(100)).chars().count(), MAX_NAME);
        assert_eq!(clamp_name("Ann"), "Ann");
    }

    #[test]
    fn boost_is_optional_and_a_bad_one_does_not_lose_the_message() {
        let leave = Message::Visit(Visit::leave(7));
        assert!(!String::from_utf8(leave.to_json()).unwrap().contains("boost"));
        let Some(Message::Visit(Visit::Leave { boost, .. })) = Message::parse(br#"{"type":"leave","visit_id":7,"boost":1.5}"#) else { panic!() };
        assert_eq!(boost, Some(1.5));
        let Some(Message::Visit(Visit::Leave { boost, visit_id, .. })) = Message::parse(br#"{"type":"leave","visit_id":7,"boost":"huge"}"#) else { panic!() };
        assert_eq!((boost, visit_id), (None, 7));
    }

    #[test]
    fn device_messages_skip_bad_entries_instead_of_failing() {
        let k = "ab".repeat(32);
        let raw = format!(r#"{{"type":"device_friends","friends":[{{"id":"x","key":"{k}","name":5,"side":7}},3,{{"id":"y"}},{{"id":"z","key":"{k}","name":"Z","side":"up"}}]}}"#);
        let Some(Message::Control(Control::DeviceFriends { friends, want })) = Message::parse(raw.as_bytes()) else { panic!() };
        assert!(!want);
        assert_eq!(friends.len(), 2);
        assert_eq!((friends[0].name.as_str(), friends[0].side.as_deref()), ("", None));
        assert_eq!((friends[1].name.as_str(), friends[1].side.as_deref()), ("Z", Some("up")));
        // 배열이 아니면 빈 목록, 없어도 빈 목록
        let Some(Message::Control(Control::MyDevices { devices })) = Message::parse(br#"{"type":"my_devices","devices":"lots"}"#) else { panic!() };
        assert!(devices.is_empty());
        let Some(Message::Control(Control::DeviceFriends { friends, .. })) = Message::parse(br#"{"type":"device_friends"}"#) else { panic!() };
        assert!(friends.is_empty());
        // 보내는 모양
        let m = Message::Control(Control::MyDevices { devices: vec![WireDevice { id: "d".into(), key: k.clone() }] });
        assert_eq!(String::from_utf8(m.to_json()).unwrap(), format!(r#"{{"type":"my_devices","devices":[{{"id":"d","key":"{k}"}}]}}"#));
        let m = Message::Control(Control::DeviceFriends { friends: vec![], want: false });
        assert_eq!(String::from_utf8(m.to_json()).unwrap(), r#"{"type":"device_friends","friends":[]}"#);
    }

    #[test]
    fn a_visit_may_say_it_is_my_other_computers_pet() {
        let v = visit!(of: Some("b2".into()));
        assert!(String::from_utf8(v.to_json()).unwrap().contains(r#""of":"b2""#));
        assert!(!String::from_utf8(visit!().to_json()).unwrap().contains(r#""of""#), "absent unless carried");
        let Some(Message::Visit(Visit::Visit { of, visit_id, .. })) = Message::parse(br#"{"type":"visit","visit_id":3,"species":"cat","stage":"child","scale":1,"personality":"p","owner":"A","enter_side":"left","of":7}"#) else { panic!() };
        assert_eq!((of, visit_id), (None, 3), "a bad value is dropped, not the visit");
    }

    #[test]
    fn device_link_messages_have_their_wire_shape_and_bad_fields_read_as_nothing() {
        let shape = |m: Control| String::from_utf8(Message::Control(m).to_json()).unwrap();
        assert_eq!(shape(Control::DeviceCode { nonce: "ab".into() }), r#"{"type":"device_code","nonce":"ab"}"#);
        assert_eq!(shape(Control::DeviceCodeNonce { nonce: "ab".into() }), r#"{"type":"device_code_nonce","nonce":"ab"}"#);
        assert_eq!(shape(Control::DeviceCodeProof { proof: "ab".into() }), r#"{"type":"device_code_proof","proof":"ab"}"#);
        assert_eq!(shape(Control::DeviceCodeOk { proof: "ab".into() }), r#"{"type":"device_code_ok","proof":"ab"}"#);
        assert_eq!(shape(Control::DeviceNo { reason: "wrong".into(), left: Some(3) }), r#"{"type":"device_no","reason":"wrong","left":3}"#);
        assert_eq!(shape(Control::DeviceNo { reason: "none".into(), left: None }), r#"{"type":"device_no","reason":"none"}"#);
        assert_eq!(shape(Control::DeviceNonce { nonce: "ab".into() }), r#"{"type":"device_nonce","nonce":"ab"}"#);
        assert_eq!(shape(Control::DeviceProof { proof: "ab".into() }), r#"{"type":"device_proof","proof":"ab"}"#);
        assert_eq!(shape(Control::DeviceUnlink), r#"{"type":"device_unlink"}"#);
        assert_eq!(shape(Control::DeviceConfirm), r#"{"type":"device_confirm"}"#);
        let Some(Message::Control(Control::DeviceNo { reason, left })) = Message::parse(br#"{"type":"device_no","reason":5,"left":"many"}"#) else { panic!() };
        assert_eq!((reason.as_str(), left), ("", None));
        assert_eq!(Message::parse(br#"{"type":"device_mark","on":true}"#), None, "gone before any release");
        // 📱 누가 노는지
        assert_eq!(shape(Control::DeviceActive { term: Some(2), you: false, idle: None, meet: false }), r#"{"type":"device_active","term":2}"#);
        assert_eq!(shape(Control::DeviceActive { term: Some(0), you: true, idle: Some(5), meet: false }), r#"{"type":"device_active","term":0,"you":true,"idle":5}"#);
        let Some(Message::Control(Control::DeviceActive { term, you, idle, .. })) = Message::parse(br#"{"type":"device_active","term":-3,"you":"yes","idle":"x"}"#) else { panic!() };
        assert_eq!((term, you, idle), (None, false, None));
    }

    #[test]
    fn device_state_and_the_meeting_claim_have_their_wire_shape() {
        let shape = |m: Control| String::from_utf8(Message::Control(m).to_json()).unwrap();
        let c = WireCare { pet: "cat".into(), stage: "child".into(), stamina: 3.0, fullness: 0.5, locked: true };
        assert_eq!(
            shape(Control::DeviceState { pet: Some("cat".into()), care: vec![c.clone()], handover: false }),
            r#"{"type":"device_state","pet":"cat","care":[{"pet":"cat","stage":"child","stamina":3.0,"fullness":0.5,"locked":true}]}"#
        );
        assert!(shape(Control::DeviceState { pet: None, care: vec![], handover: true }).ends_with(r#""care":[],"handover":true}"#));
        // 깨진 항목은 그 항목만 버린다
        let Some(Message::Control(Control::DeviceState { pet, care, handover })) =
            Message::parse(br#"{"type":"device_state","pet":7,"care":[{"pet":"cat","stage":"baby","stamina":1,"fullness":1},{"pet":"dog"},5],"handover":"yes"}"#)
        else {
            panic!()
        };
        assert_eq!((pet, handover), (None, false));
        assert_eq!(care, vec![WireCare { pet: "cat".into(), stage: "baby".into(), stamina: 1.0, fullness: 1.0, locked: false }]);
        // 다시 만나서 정한 주장 (넘겨주기 없음)
        assert_eq!(shape(Control::DeviceActive { term: Some(2), you: false, idle: None, meet: true }), r#"{"type":"device_active","term":2,"meet":true}"#);
    }
}
