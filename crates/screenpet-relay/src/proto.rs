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
//! client -> relay   ping                      -> pong
//! relay  -> client  error {reason}            (then the connection closes)
//! ```

use serde::{Deserialize, Serialize};

pub const PROTOCOL_VERSION: u32 = 1;
/// Domain separation for the authentication MAC.
pub const AUTH_LABEL: &[u8] = b"screenpet-relay/1 auth";
pub const WELCOME_LABEL: &[u8] = b"screenpet-relay/1 welcome";
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
}

impl ServerMsg {
    pub fn to_json(&self) -> String {
        serde_json::to_string(self).expect("server message serializes")
    }
}

pub fn blake2s(parts: &[&[u8]]) -> [u8; 32] {
    use blake2::{Blake2s256, Digest};
    let mut h = Blake2s256::new();
    for p in parts {
        h.update(p);
    }
    h.finalize().into()
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

pub fn public_key(private: &[u8; 32]) -> [u8; 32] {
    curve25519_dalek::MontgomeryPoint::mul_base_clamped(*private).to_bytes()
}

pub fn auth_mac(shared: &[u8; 32], nonce: &[u8; 32], relay_pub: &[u8; 32], client_pub: &[u8; 32], id: &str) -> [u8; 32] {
    blake2s(&[AUTH_LABEL, shared, nonce, relay_pub, client_pub, id.as_bytes()])
}

/// Proof in `welcome` that the relay holds the private key it announced.
pub fn welcome_proof(shared: &[u8; 32], cnonce: &[u8; 32], nonce: &[u8; 32], client_pub: &[u8; 32]) -> [u8; 32] {
    blake2s(&[WELCOME_LABEL, shared, cnonce, nonce, client_pub])
}

/// Equality without an early exit.
pub fn ct_eq(a: &[u8; 32], b: &[u8; 32]) -> bool {
    a.iter().zip(b).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

pub fn random32() -> [u8; 32] {
    let mut k = [0u8; 32];
    getrandom::fill(&mut k).expect("OS random number generator unavailable");
    k
}

pub fn to_hex(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
}

/// Exactly 32 bytes of lowercase-or-uppercase hex, else `None`.
pub fn hex32(s: &str) -> Option<[u8; 32]> {
    if s.len() != 64 || !s.bytes().all(|c| c.is_ascii_hexdigit()) {
        return None;
    }
    let mut out = [0u8; 32];
    for (i, o) in out.iter_mut().enumerate() {
        *o = u8::from_str_radix(&s[2 * i..2 * i + 2], 16).ok()?;
    }
    Some(out)
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
}
