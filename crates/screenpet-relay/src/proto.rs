//! 릴레이 와이어 프로토콜: 메시지·버전·한도는 `screenpet_proto`에 있음 (앱과 같은 것).
//! 여기서는 그것을 다시 내보내고, 릴레이의 암호·친구 코드 도우미를 더함.
//!
//! Control messages are text frames holding JSON tagged by `t` (see
//! `screenpet_proto::relay` for every message). Friend traffic is binary
//! frames: a 4-byte big-endian channel id followed by one opaque frame (at
//! most 65535 bytes) — Noise messages between the two friends, which the
//! relay can neither read nor forge. While `admin friendcode off`: welcome
//! and code carry code "----", redeem_failed says `disabled`.

pub use screenpet_crypto::{auth_mac, hex32, public_key, random32, to_hex, welcome_proof};
pub use screenpet_proto::relay::{BoardIn, ClientMsg, ServerMsg};
pub use screenpet_proto::{MAX_FRAME, MAX_ID};

/// 2 = takes `weather_want` (v0.6.3). 3 = 📮 `board_put`, `board_list`,
/// `letter_reject` (v0.6.3). Apps send each only to relays that say a high
/// enough `v` in the challenge: older relays close on an unknown message.
pub const PROTOCOL_VERSION: u32 = screenpet_proto::RELAY_PROTOCOL_VERSION;
/// 바이너리 메시지의 채널 id 머리 크기
pub const HEADER: usize = 4;

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
