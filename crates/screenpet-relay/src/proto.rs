//! Relay wire protocol (WebSocket at `/ws`).
//!
//! Control messages are text frames holding JSON tagged by `t`. Friend
//! traffic is binary frames: a 4-byte big-endian channel id followed by one
//! opaque frame (at most 65535 bytes) — Noise messages between the two
//! friends, which the relay can neither read nor forge.
//!
//! ```text
//! relay  -> client  challenge {relay: hex32 relay public key, nonce: hex32, v}
//! client -> relay   auth {id, key: hex32 static public key, mac: hex32, friends: [hex32]}
//!                   mac = BLAKE2s(AUTH_LABEL || X25519(client, relay) || nonce
//!                                 || relay_pub || client_pub || id)
//!                   (+ cnonce: hex32 client nonce, optional)
//! relay  -> client  welcome {code, code_ttl, online: [hex32], proof}
//!                   proof = BLAKE2s(WELCOME_LABEL || X25519 || cnonce || nonce
//!                                   || client_pub): the relay holds its key
//! client -> relay   new_code                  -> code {code, code_ttl}
//! client -> relay   friends {keys}            -> online {keys}
//! relay  -> client  presence {key, online}    (mutual friends only)
//! relay  -> client  code {code, code_ttl}     (24 h rotation)
//! client -> relay   redeem {code}             -> redeemed {code, key, id} | redeem_failed {code, reason}
//! client -> relay   open {req, to}            -> opened {req, ch} | open_failed {req, reason}
//! relay  -> client  incoming {ch, from, id}
//! both              close {ch}
//! client -> relay   letter {box: hex}          -> letter_ok | letter_refused {reason}
//!                   (a feedback letter sealed to the developer key; reasons:
//!                   daily, ip, new, global, full, size, blocked)
//! client -> relay   letters                    -> letters {items}       (mailbox readers only)
//! client -> relay   letter_read {id} | letter_delete {id} | letter_block {from} | letter_done {id, done}
//!                                              (readers only) -> letters_changed to every reader
//!                   (items carry `done` and, from v3, `rejected`; `letter_done false` clears both)
//! relay  -> client  letter_new {item}          (to readers online when a letter arrives)
//! client -> relay   letter_reject {id, rejected} (v3, readers only) 🙅 반려 — clears 반영; -> letters_changed
//! client -> relay   letter_private {id, private} (v3, readers only) 🔒 비공개: off the public
//!                   board (items, total, pages) until turned off; -> letters_changed
//!                   (letters carry `private`)
//! client -> relay   board_put {items: [{id, name, line}]}
//!                   (v3, readers only, ≤ 20 a message) 📮 public digest of a
//!                   letter: masked name (`김**`) and its first line; checked
//!                   again here, kept with the letter. `letters` answers
//!                   carry `board: [id]`, the letters already on the board.
//! client -> relay   board_list {page}          (v3, anyone, ≥ 0.5 s apart; faster ones are ignored)
//!                   -> board_page {items: [{id, name, line, at, status}], page, pages, total}
//!                   5 a page, newest first, page 1-based and clamped,
//!                   status none | done | rejected
//! client -> relay   weather_want {place}      (v2 relays only; -> weather if known)
//!                   place = a built-in city id ("yongin", "seoul", …,
//!                   `weather::PLACES`); unknown ids are ignored
//! relay  -> client  weather {place, code, at}
//!                   ☀️ current WMO weather code of that place, fetched by
//!                   the relay at `at` (unix s); sent after the welcome
//!                   (Yongin until the app says what it wants) and on every
//!                   new reading. Older apps ignore unknown messages.
//! client -> relay   ping                      -> pong
//! relay  -> client  error {reason}            (then the connection closes)
//!                   apps only log the reason. While `admin friendcode off`:
//!                   welcome and code carry code "----", redeem_failed says
//!                   `disabled`.
//! ```

pub use screenpet_crypto::{auth_mac, hex32, public_key, random32, to_hex, welcome_proof};
use serde::{Deserialize, Serialize};

/// 2 = takes `weather_want` (v0.6.3). 3 = 📮 `board_put`, `board_list`,
/// `letter_reject` (v0.6.3). Apps send each only to relays that say a high
/// enough `v` in the challenge: older relays close on an unknown message.
pub const PROTOCOL_VERSION: u32 = 3;
/// Largest opaque frame carried on a channel (same as the LAN transport).
pub const MAX_FRAME: usize = 65535;
/// Channel id header in binary messages.
pub const HEADER: usize = 4;
/// Longest install id accepted.
pub const MAX_ID: usize = 128;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "t", rename_all = "snake_case")]
pub enum ClientMsg {
    Auth {
        id: String,
        key: String,
        mac: String,
        #[serde(default)]
        friends: Vec<String>,
        #[serde(default)]
        cnonce: Option<String>,
    },
    Friends {
        keys: Vec<String>,
    },
    Redeem {
        code: String,
    },
    Open {
        req: u32,
        to: String,
    },
    Close {
        ch: u32,
    },
    NewCode,
    Ping,
    Letter {
        r#box: String,
    },
    Letters,
    LetterRead {
        id: u64,
    },
    LetterDelete {
        id: u64,
    },
    LetterDone {
        id: u64,
        done: bool,
    },
    LetterBlock {
        from: String,
    },
    WeatherWant {
        place: String,
    },
    LetterReject {
        id: u64,
        rejected: bool,
    },
    LetterPrivate {
        id: u64,
        private: bool,
    },
    BoardPut {
        items: Vec<BoardIn>,
    },
    BoardList {
        #[serde(default)]
        page: u32,
    },
}

/// 📮 A digest the developer's app publishes (checked again by the mailbox).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BoardIn {
    pub id: u64,
    pub name: String,
    pub line: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "t", rename_all = "snake_case")]
pub enum ServerMsg {
    Challenge { relay: String, nonce: String, v: u32 },
    Welcome {
        code: String,
        code_ttl: u64,
        online: Vec<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        proof: Option<String>,
    },
    Online { keys: Vec<String> },
    Presence { key: String, online: bool },
    Code { code: String, code_ttl: u64 },
    Redeemed { code: String, key: String, id: String },
    RedeemFailed { code: String, reason: String },
    Opened { req: u32, ch: u32 },
    OpenFailed { req: u32, reason: String },
    Incoming { ch: u32, from: String, id: String },
    Close { ch: u32 },
    Pong,
    Error { reason: String },
    LetterOk,
    LetterRefused { reason: String },
    Letters {
        items: Vec<crate::mailbox::StoredLetter>,
        /// 📮 Ids of the letters already on the board.
        board: Vec<u64>,
    },
    LetterNew { item: crate::mailbox::StoredLetter },
    LettersChanged,
    Weather { place: String, code: u16, at: u64 },
    BoardPage { items: Vec<crate::board::Item>, page: u32, pages: u32, total: u32 },
}

impl ServerMsg {
    pub fn to_json(&self) -> String {
        serde_json::to_string(self).expect("server message serializes")
    }
}

/// X25519(private, public). `None` for a low-order public key (all-zero result).
pub fn dh(private: &[u8; 32], public: &[u8; 32]) -> Option<[u8; 32]> {
    let shared = curve25519_dalek::MontgomeryPoint(*public).mul_clamped(*private).to_bytes();
    if shared.iter().all(|b| *b == 0) {
        None
    } else {
        Some(shared)
    }
}

/// Equality without an early exit.
pub fn ct_eq(a: &[u8; 32], b: &[u8; 32]) -> bool {
    a.iter().zip(b).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

/// Canonical (lowercase) hex of a 32-byte key, or `None` if malformed.
pub fn norm_key(s: &str) -> Option<String> {
    hex32(s).map(|k| to_hex(&k))
}

/// Crockford base32 alphabet (no I, L, O, U).
const CROCKFORD: &[u8; 32] = b"0123456789ABCDEFGHJKMNPQRSTVWXYZ";

/// A fresh friend code: 8 Crockford base32 characters (40 random bits)
/// formatted `ABCD-1234`.
pub fn new_code() -> String {
    let r = random32();
    let bits = r[..5].iter().fold(0u64, |acc, b| (acc << 8) | u64::from(*b));
    let chars: String = (0..8).map(|i| CROCKFORD[((bits >> (35 - 5 * i)) & 31) as usize] as char).collect();
    format!("{}-{}", &chars[..4], &chars[4..])
}

/// Normalize user input to `ABCD-1234`: case-insensitive, separators
/// ignored, Crockford look-alikes mapped (O→0, I/L→1). `None` if not a code.
pub fn normalize_code(input: &str) -> Option<String> {
    let mut chars = String::new();
    for c in input.chars() {
        if c == '-' || c == ' ' {
            continue;
        }
        let c = match c.to_ascii_uppercase() {
            'O' => '0',
            'I' | 'L' => '1',
            c => c,
        };
        if !c.is_ascii() || !CROCKFORD.contains(&(c as u8)) {
            return None;
        }
        chars.push(c);
        if chars.len() > 8 {
            return None;
        }
    }
    (chars.len() == 8).then(|| format!("{}-{}", &chars[..4], &chars[4..]))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn codes_are_eight_crockford_characters() {
        for _ in 0..200 {
            let c = new_code();
            assert_eq!(c.len(), 9);
            assert_eq!(&c[4..5], "-");
            assert_eq!(normalize_code(&c).as_deref(), Some(c.as_str()));
        }
        assert_ne!(new_code(), new_code());
    }

    #[test]
    fn code_normalization() {
        assert_eq!(normalize_code("abcd-efgh").as_deref(), Some("ABCD-EFGH"));
        assert_eq!(normalize_code(" ab cd 1o2l ").as_deref(), Some("ABCD-1021"));
        assert_eq!(normalize_code("ABCD-EFG"), None);
        assert_eq!(normalize_code("ABCD-EFGHJ"), None);
        assert_eq!(normalize_code("ABCD-EFGU"), None, "U is not Crockford");
        assert_eq!(normalize_code("ÄBCD-EFGH"), None);
    }

    #[test]
    fn mac_binds_every_input() {
        let (c, r) = (random32(), random32());
        let (cp, rp) = (public_key(&c), public_key(&r));
        let s1 = dh(&c, &rp).unwrap();
        let s2 = dh(&r, &cp).unwrap();
        assert_eq!(s1, s2);
        let n = random32();
        let m = auth_mac(&s1, &n, &rp, &cp, "id");
        assert!(ct_eq(&m, &auth_mac(&s2, &n, &rp, &cp, "id")));
        assert!(!ct_eq(&m, &auth_mac(&s2, &random32(), &rp, &cp, "id")));
        assert!(!ct_eq(&m, &auth_mac(&s2, &n, &rp, &cp, "other")));
        assert_eq!(dh(&c, &[0u8; 32]), None, "low-order point rejected");
    }

    #[test]
    fn messages_round_trip() {
        let m: ClientMsg = serde_json::from_str(r#"{"t":"open","req":3,"to":"ab"}"#).unwrap();
        assert_eq!(m, ClientMsg::Open { req: 3, to: "ab".into() });
        assert!(serde_json::from_str::<ClientMsg>(r#"{"t":"bogus"}"#).is_err());
        let s = ServerMsg::Presence { key: "k".into(), online: true }.to_json();
        assert_eq!(s, r#"{"t":"presence","key":"k","online":true}"#);
    }

    #[test]
    fn board_messages_round_trip() {
        assert_eq!(PROTOCOL_VERSION, 3);
        let m: ClientMsg = serde_json::from_str(r#"{"t":"board_list","page":2}"#).unwrap();
        assert_eq!(m, ClientMsg::BoardList { page: 2 });
        let m: ClientMsg = serde_json::from_str(r#"{"t":"board_put","items":[{"id":3,"name":"김**","line":"펭귄"}]}"#).unwrap();
        assert_eq!(m, ClientMsg::BoardPut { items: vec![BoardIn { id: 3, name: "김**".into(), line: "펭귄".into() }] });
        let m: ClientMsg = serde_json::from_str(r#"{"t":"letter_reject","id":4,"rejected":true}"#).unwrap();
        assert_eq!(m, ClientMsg::LetterReject { id: 4, rejected: true });
        let m: ClientMsg = serde_json::from_str(r#"{"t":"letter_private","id":4,"private":true}"#).unwrap();
        assert_eq!(m, ClientMsg::LetterPrivate { id: 4, private: true });
        let item = crate::board::Item { id: 3, name: "김**".into(), line: "펭귄".into(), at: 9, status: crate::board::Status::Rejected };
        let s = ServerMsg::BoardPage { items: vec![item], page: 1, pages: 1, total: 1 }.to_json();
        assert_eq!(s, r#"{"t":"board_page","items":[{"id":3,"name":"김**","line":"펭귄","at":9,"status":"rejected"}],"page":1,"pages":1,"total":1}"#);
        let s = ServerMsg::Letters { items: vec![], board: vec![3] }.to_json();
        assert_eq!(s, r#"{"t":"letters","items":[],"board":[3]}"#);
    }
}
