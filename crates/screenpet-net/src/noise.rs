//! Noise 핸드셰이크와 이모지 짧은 인증 문자열(SAS)임.
//!
//! 모든 링크는 `Noise_XX_25519_ChaChaPoly_BLAKE2s`를 돌림 (다이얼하는 쪽 = initiator).
//! XX는 양쪽 정적 키를 암호화해 보내므로 끝나면 서로의 정적 공개키를 앎: 친구는 고정된 키와 비교해 알아보고,
//! 낯선 이는 짝짓기만 할 수 있으며 두 사용자가 이모지 6개 SAS를 비교해 확정함.
//! 중간자는 핸드셰이크 둘을 따로 돌리게 되고, SAS에 초대자 nonce 커밋과 초대받은 쪽 난수를 섞어
//! (commit -> random -> reveal) 키를 갈아 SAS를 맞출 수 없음: 시도마다 성공 확률 2^-36임.
//! 핸드셰이크 뒤 모든 메시지는 Noise 전송 메시지임 (ChaCha20-Poly1305, 암묵적 nonce: 재전송·순서 바꾸기·끼워 넣기 불가).

pub use screenpet_crypto::{hex32, to_hex};
pub(crate) use screenpet_crypto::{blake2s, hex_array, public_key, random32};

/// 아무 길이의 hex를 바이트로 바꿈. 모양이 틀리면 None임
pub(crate) fn hex_vec(s: &str) -> Option<Vec<u8>> {
    if s.len() % 2 != 0 {
        return None;
    }
    (0..s.len()).step_by(2).map(|i| u8::from_str_radix(s.get(i..i + 2)?, 16).ok()).collect()
}

use crate::protocol::MAX_FRAME;
use crate::transport::{FrameLink, FrameReader, Recv};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;

/// 모든 링크가 쓰는 Noise 패턴 이름임
pub(crate) const PARAMS: &str = "Noise_XX_25519_ChaChaPoly_BLAKE2s";
/// 핸드셰이크 프롤로그임 (바꾸면 옛 앱과 안 통함)
pub(crate) const PROLOGUE: &[u8] = b"screenpet-net/1";

/// SAS 기호 64개 (기호당 6비트): 동물, 음식, 물건
pub const SAS_EMOJI: [&str; 64] = [
    "🐶", "🐱", "🐭", "🐹", "🐰", "🦊", "🐻", "🐼", "🐨", "🐯", "🦁", "🐮", "🐷", "🐸", "🐵", "🐔", //
    "🐧", "🐦", "🐤", "🦆", "🦉", "🐴", "🦄", "🐝", "🐛", "🦋", "🐌", "🐞", "🐢", "🐍", "🐙", "🦀", //
    "🐠", "🐬", "🐳", "🦒", "🍎", "🍐", "🍊", "🍋", "🍌", "🍉", "🍇", "🍓", "🍒", "🍑", "🍍", "🥕", //
    "🌽", "🍄", "🥐", "🧀", "🍕", "🍩", "🍪", "🎂", "⭐", "🌙", "🎈", "🎁", "⚽", "🔑", "🎵", "🚗", //
];

/// SAS 기호 수 (6비트씩 → 36비트)
pub(crate) const SAS_LEN: usize = 6;

/// 초대자 짝짓기 nonce의 커밋임: BLAKE2s(nonce_i)
pub(crate) fn commit(nonce_i: &[u8; 32]) -> [u8; 32] {
    blake2s(&[nonce_i])
}

/// SAS를 만듦: BLAKE2s(handshake_hash || nonce_i || r_r)의 앞 36비트를 기호 6개로, 공백으로 이음
pub(crate) fn sas(handshake_hash: &[u8; 32], nonce_i: &[u8; 32], r_r: &[u8; 32]) -> String {
    let d = blake2s(&[handshake_hash, nonce_i, r_r]);
    let bits = d[..5].iter().fold(0u64, |acc, b| (acc << 8) | u64::from(*b)); // 40 bits
    (0..SAS_LEN)
        .map(|i| SAS_EMOJI[((bits >> (34 - 6 * i)) & 63) as usize])
        .collect::<Vec<_>>()
        .join(" ")
}

/// OS 암호 난수로 새 X25519 개인키를 만듦
pub(crate) fn generate_static_key() -> [u8; 32] {
    random32()
}

/// 핸드셰이크가 끝난 결과임
pub(crate) struct Established {
    /// 이후 메시지를 암호화·복호화하는 Noise 전송 상태
    pub transport: snow::TransportState,
    /// 상대의 정적 공개키 (친구 신원)
    pub remote_static: [u8; 32],
    /// 핸드셰이크 해시 (SAS 계산용)
    pub hash: [u8; 32],
}

/// 새 링크 위에서 XX 핸드셰이크를 돌림.
/// 실패·시간 초과·종료 중이면 None이고 호출자가 링크를 버림
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

/// SAS 기호표와 커밋·키 생성 시험임
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
