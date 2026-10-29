//! Noise handshake and the emoji short authentication string (SAS).
//!
//! Every link runs `Noise_XX_25519_ChaChaPoly_BLAKE2s` (dialer = initiator).
//! XX transmits both static keys encrypted, so after the handshake each side
//! knows the peer's static public key: a friend is recognised by comparing it
//! with the pinned key; a stranger can only pair, and pairing is confirmed by
//! both users comparing a 6-emoji SAS. A MITM runs two different handshakes,
//! and the SAS also mixes a committed inviter nonce and an invitee random
//! (commit -> random -> reveal), so the MITM cannot grind keys to make the
//! two SAS match: each attempt succeeds with probability 2^-36.
//! After the handshake every message is a Noise transport message
//! (ChaCha20-Poly1305 with implicit nonces: no replay, reordering or
//! injection on the link).

use crate::protocol::MAX_FRAME;
use crate::transport::{FrameLink, FrameReader, Recv};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;

pub(crate) const PARAMS: &str = "Noise_XX_25519_ChaChaPoly_BLAKE2s";
pub(crate) const PROLOGUE: &[u8] = b"screenpet-net/1";

/// 64 SAS symbols (6 bits each): animals, food, objects.
pub const SAS_EMOJI: [&str; 64] = [
    "🐶", "🐱", "🐭", "🐹", "🐰", "🦊", "🐻", "🐼", "🐨", "🐯", "🦁", "🐮", "🐷", "🐸", "🐵", "🐔", //
    "🐧", "🐦", "🐤", "🦆", "🦉", "🐴", "🦄", "🐝", "🐛", "🦋", "🐌", "🐞", "🐢", "🐍", "🐙", "🦀", //
    "🐠", "🐬", "🐳", "🦒", "🍎", "🍐", "🍊", "🍋", "🍌", "🍉", "🍇", "🍓", "🍒", "🍑", "🍍", "🥕", //
    "🌽", "🍄", "🥐", "🧀", "🍕", "🍩", "🍪", "🎂", "⭐", "🌙", "🎈", "🎁", "⚽", "🔑", "🎵", "🚗", //
];

/// Number of SAS symbols (6 bits each → 36 bits).
pub(crate) const SAS_LEN: usize = 6;

fn blake2s(parts: &[&[u8]]) -> [u8; 32] {
    use blake2::{Blake2s256, Digest};
    let mut h = Blake2s256::new();
    for p in parts {
        h.update(p);
    }
    h.finalize().into()
}

/// Commitment to the inviter's pairing nonce: BLAKE2s(nonce_i).
pub(crate) fn commit(nonce_i: &[u8; 32]) -> [u8; 32] {
    blake2s(&[nonce_i])
}

/// SAS: the first 36 bits of BLAKE2s(handshake_hash || nonce_i || r_r) as six
/// symbols, space-separated.
pub(crate) fn sas(handshake_hash: &[u8; 32], nonce_i: &[u8; 32], r_r: &[u8; 32]) -> String {
    let d = blake2s(&[handshake_hash, nonce_i, r_r]);
    let bits = d[..5].iter().fold(0u64, |acc, b| (acc << 8) | u64::from(*b)); // 40 bits
    (0..SAS_LEN)
        .map(|i| SAS_EMOJI[((bits >> (34 - 6 * i)) & 63) as usize])
        .collect::<Vec<_>>()
        .join(" ")
}

/// 32 bytes from the OS CSPRNG.
pub(crate) fn random32() -> [u8; 32] {
    let mut k = [0u8; 32];
    getrandom::fill(&mut k).expect("OS random number generator unavailable");
    k
}

/// New X25519 private key from the OS CSPRNG.
pub(crate) fn generate_static_key() -> [u8; 32] {
    random32()
}

pub(crate) fn to_hex(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
}

/// Exactly 32 bytes of hex, else `None`.
pub(crate) fn hex32(s: &str) -> Option<[u8; 32]> {
    if s.len() != 64 || !s.bytes().all(|c| c.is_ascii_hexdigit()) {
        return None;
    }
    let mut out = [0u8; 32];
    for (i, o) in out.iter_mut().enumerate() {
        *o = u8::from_str_radix(&s[2 * i..2 * i + 2], 16).ok()?;
    }
    Some(out)
}

/// X25519 public key for a private key.
pub(crate) fn public_key(private: &[u8; 32]) -> [u8; 32] {
    curve25519_dalek::MontgomeryPoint::mul_base_clamped(*private).to_bytes()
}

pub(crate) struct Established {
    pub transport: snow::TransportState,
    pub remote_static: [u8; 32],
    pub hash: [u8; 32],
}

/// Run the XX handshake over a fresh link. `None` on any failure, timeout or
/// shutdown (the caller drops the link).
pub(crate) fn handshake(
    static_key: &[u8; 32],
    initiator: bool,
    link: &dyn FrameLink,
    reader: &mut dyn FrameReader,
    deadline: Instant,
    stop: &AtomicBool,
) -> Option<Established> {
    let builder = snow::Builder::new(PARAMS.parse().ok()?)
        .local_private_key(static_key)
        .ok()?
        .prologue(PROLOGUE)
        .ok()?;
    let mut hs = if initiator { builder.build_initiator() } else { builder.build_responder() }.ok()?;
    let mut buf = vec![0u8; MAX_FRAME];
    while !hs.is_handshake_finished() {
        if hs.is_my_turn() {
            let n = hs.write_message(&[], &mut buf).ok()?;
            if !link.send_frame(&buf[..n]) {
                return None;
            }
        } else {
            let frame = loop {
                if stop.load(Ordering::SeqCst) || Instant::now() > deadline {
                    return None;
                }
                match reader.recv() {
                    Recv::Frame(f) => break f,
                    Recv::Idle => {}
                    Recv::Invalid | Recv::Closed => return None,
                }
            };
            // Handshake messages carry no payload; anything extra is rejected.
            if hs.read_message(&frame, &mut buf).ok()? != 0 {
                return None;
            }
        }
    }
    let remote_static: [u8; 32] = hs.get_remote_static()?.try_into().ok()?;
    let hash: [u8; 32] = hs.get_handshake_hash().try_into().ok()?;
    let transport = hs.into_transport_mode().ok()?;
    Some(Established { transport, remote_static, hash })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn sas_alphabet_has_64_distinct_symbols() {
        let set: HashSet<_> = SAS_EMOJI.iter().collect();
        assert_eq!(set.len(), 64);
    }

    #[test]
    fn sas_has_six_symbols_from_the_alphabet() {
        let s = sas(&random32(), &random32(), &random32());
        let parts: Vec<_> = s.split(' ').collect();
        assert_eq!(parts.len(), 6);
        assert!(parts.iter().all(|p| SAS_EMOJI.contains(p)));
    }

    #[test]
    fn sas_uses_all_six_positions() {
        // Over many inputs every position takes many different symbols.
        let (h, n) = (random32(), random32());
        let samples: Vec<Vec<String>> = (0..400)
            .map(|_| sas(&h, &n, &random32()).split(' ').map(String::from).collect())
            .collect();
        for pos in 0..6 {
            let distinct: HashSet<_> = samples.iter().map(|s| s[pos].clone()).collect();
            assert!(distinct.len() > 32, "position {pos} only had {} symbols", distinct.len());
        }
    }

    #[test]
    fn sas_mixes_hash_and_both_randoms() {
        let (h, n, r) = (random32(), random32(), random32());
        let base = sas(&h, &n, &r);
        assert_eq!(base, sas(&h, &n, &r));
        assert_ne!(base, sas(&random32(), &n, &r));
        assert_ne!(base, sas(&h, &random32(), &r));
        assert_ne!(base, sas(&h, &n, &random32()));
        // Order matters (nonce_i and r_r are not interchangeable).
        assert_ne!(base, sas(&h, &r, &n));
    }

    #[test]
    fn commitment_binds_the_nonce() {
        let n = random32();
        assert_eq!(commit(&n), commit(&n));
        assert_ne!(commit(&n), commit(&random32()));
        assert_eq!(hex32(&to_hex(&n)), Some(n));
        assert_eq!(hex32("zz"), None);
    }

    #[test]
    fn keys_are_random_and_public_key_is_deterministic() {
        let a = generate_static_key();
        assert_ne!(a, generate_static_key());
        assert_eq!(public_key(&a), public_key(&a));
        assert_ne!(public_key(&a), a);
    }
}
