//! Friendship secrets and the hello challenge-response.
//!
//! Each friendship has a 32-byte secret (CSPRNG, hex-encoded) created by the
//! accepting side at pairing time. On every new link each side sends a fresh
//! random nonce in `hello`; the other side proves knowledge of the secret with
//! `HMAC-SHA256(secret, tag || nonce || len(own_id) || own_id || len(peer_id) || peer_id)`
//! where `own_id` is the responder's id. Ordering the ids prevents reflecting
//! a challenge back; the fresh nonce prevents replay.

use hmac::{Hmac, Mac};
use sha2::Sha256;

type HmacSha256 = Hmac<Sha256>;

const TAG: &[u8] = b"screenpet-auth-v1\0";
/// Bytes in a secret or nonce.
pub(crate) const RANDOM_LEN: usize = 32;

/// `RANDOM_LEN` bytes from the OS CSPRNG, hex-encoded.
pub(crate) fn random_hex() -> String {
    let mut b = [0u8; RANDOM_LEN];
    getrandom::fill(&mut b).expect("OS random number generator unavailable");
    to_hex(&b)
}

pub(crate) fn to_hex(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
}

pub(crate) fn from_hex(s: &str) -> Option<Vec<u8>> {
    if s.len() % 2 != 0 || !s.bytes().all(|c| c.is_ascii_hexdigit()) {
        return None;
    }
    (0..s.len()).step_by(2).map(|i| u8::from_str_radix(&s[i..i + 2], 16).ok()).collect()
}

/// A well-formed secret or nonce: exactly `RANDOM_LEN` bytes of hex.
pub(crate) fn valid_random(s: &str) -> bool {
    from_hex(s).is_some_and(|b| b.len() == RANDOM_LEN)
}

fn keyed(secret: &str, nonce: &str, own_id: &str, peer_id: &str) -> Option<HmacSha256> {
    let key = from_hex(secret).filter(|k| k.len() == RANDOM_LEN)?;
    let nonce = from_hex(nonce).filter(|n| n.len() == RANDOM_LEN)?;
    let mut m = HmacSha256::new_from_slice(&key).ok()?;
    m.update(TAG);
    m.update(&nonce);
    for id in [own_id, peer_id] {
        m.update(&(id.len() as u32).to_be_bytes());
        m.update(id.as_bytes());
    }
    Some(m)
}

/// Response proving `own_id` holds `secret`, to the peer's `nonce`.
pub(crate) fn respond(secret: &str, nonce: &str, own_id: &str, peer_id: &str) -> Option<String> {
    keyed(secret, nonce, own_id, peer_id).map(|m| to_hex(&m.finalize().into_bytes()))
}

/// Constant-time check of a peer's response to our `nonce`.
pub(crate) fn verify(secret: &str, nonce: &str, responder_id: &str, our_id: &str, mac_hex: &str) -> bool {
    match (keyed(secret, nonce, responder_id, our_id), from_hex(mac_hex)) {
        (Some(m), Some(mac)) => m.verify_slice(&mac).is_ok(),
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn random_values_are_distinct_hex() {
        let a = random_hex();
        assert!(valid_random(&a));
        assert_ne!(a, random_hex());
    }

    #[test]
    fn respond_verify_round_trip_and_binding() {
        let s = random_hex();
        let n = random_hex();
        let r = respond(&s, &n, "bob", "ann").unwrap();
        assert!(verify(&s, &n, "bob", "ann", &r));
        // Wrong secret, nonce, or id order (reflection) all fail.
        assert!(!verify(&random_hex(), &n, "bob", "ann", &r));
        assert!(!verify(&s, &random_hex(), "bob", "ann", &r));
        assert!(!verify(&s, &n, "ann", "bob", &r));
        // Length prefixes: "ab"+"c" differs from "a"+"bc".
        assert_ne!(respond(&s, &n, "ab", "c"), respond(&s, &n, "a", "bc"));
        assert!(!verify(&s, &n, "bob", "ann", "zz"));
        assert!(respond("short", &n, "a", "b").is_none());
    }
}
