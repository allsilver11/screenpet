//! Helpers the app's network code and the relay must agree on: hashing, keys, hex and the relay handshake's MACs.

pub const AUTH_LABEL: &[u8] = b"screenpet-relay/1 auth";
pub const WELCOME_LABEL: &[u8] = b"screenpet-relay/1 welcome";

pub fn blake2s(parts: &[&[u8]]) -> [u8; 32] {
    use blake2::{Blake2s256, Digest};
    let mut h = Blake2s256::new();
    for p in parts {
        h.update(p);
    }
    h.finalize().into()
}

/// X25519 public key for a private key.
pub fn public_key(private: &[u8; 32]) -> [u8; 32] {
    curve25519_dalek::MontgomeryPoint::mul_base_clamped(*private).to_bytes()
}

/// Client's proof in `auth`: it holds the private key of `client_pub`.
pub fn auth_mac(shared: &[u8; 32], nonce: &[u8; 32], relay_pub: &[u8; 32], client_pub: &[u8; 32], id: &str) -> [u8; 32] {
    blake2s(&[AUTH_LABEL, shared, nonce, relay_pub, client_pub, id.as_bytes()])
}

/// Proof in `welcome` that the relay holds the private key it announced.
pub fn welcome_proof(shared: &[u8; 32], cnonce: &[u8; 32], nonce: &[u8; 32], client_pub: &[u8; 32]) -> [u8; 32] {
    blake2s(&[WELCOME_LABEL, shared, cnonce, nonce, client_pub])
}

/// 32 bytes from the OS CSPRNG.
pub fn random32() -> [u8; 32] {
    let mut k = [0u8; 32];
    getrandom::fill(&mut k).expect("OS random number generator unavailable");
    k
}

pub fn to_hex(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
}

/// Exactly N bytes of lowercase-or-uppercase hex, else `None`.
pub fn hex_array<const N: usize>(s: &str) -> Option<[u8; N]> {
    if s.len() != 2 * N || !s.bytes().all(|c| c.is_ascii_hexdigit()) {
        return None;
    }
    let mut out = [0u8; N];
    for (i, o) in out.iter_mut().enumerate() {
        *o = u8::from_str_radix(&s[2 * i..2 * i + 2], 16).ok()?;
    }
    Some(out)
}

/// Exactly 32 bytes of hex, else `None`.
pub fn hex32(s: &str) -> Option<[u8; 32]> {
    hex_array::<32>(s)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hex_round_trips_and_rejects_wrong_lengths_and_characters() {
        let k = random32();
        assert_eq!(hex32(&to_hex(&k)), Some(k));
        assert_eq!(hex32(&to_hex(&k).to_uppercase()), Some(k));
        assert_eq!(hex32(&to_hex(&k[..31])), None);
        assert_eq!(hex_array::<1>("zz"), None);
    }

    /// The bytes on the wire: changing a label or the order locks old clients out of the relay
    #[test]
    fn handshake_macs_stay_what_old_peers_compute() {
        let (s, n, r, c) = ([1u8; 32], [2u8; 32], [3u8; 32], [4u8; 32]);
        assert_eq!(auth_mac(&s, &n, &r, &c, "id"), blake2s(&[b"screenpet-relay/1 auth", &s, &n, &r, &c, b"id"]));
        assert_eq!(welcome_proof(&s, &c, &n, &r), blake2s(&[b"screenpet-relay/1 welcome", &s, &c, &n, &r]));
    }
}
