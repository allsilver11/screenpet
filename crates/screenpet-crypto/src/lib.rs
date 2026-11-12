//! 앱 네트워크 코드와 릴레이가 똑같이 계산해야 하는 암호 도우미 모음임.
//! BLAKE2s 해시, X25519 공개키, hex 변환, 릴레이 핸드셰이크 MAC(`auth`·`welcome`)을 다룸.
//! screenpet-net(앱)과 screenpet-relay(서버)가 같이 씀.

/// 릴레이 `auth` MAC 앞에 붙이는 라벨임 (바꾸면 옛 앱이 못 붙음)
pub const AUTH_LABEL: &[u8] = b"screenpet-relay/1 auth";
/// 릴레이 `welcome` 증명 앞에 붙이는 라벨임
pub const WELCOME_LABEL: &[u8] = b"screenpet-relay/1 welcome";

/// 조각들을 이어 붙인 BLAKE2s-256 해시를 돌려줌
pub fn blake2s(parts: &[&[u8]]) -> [u8; 32] {
    use blake2::{Blake2s256, Digest};
    let mut h = Blake2s256::new();
    for p in parts {
        h.update(p);
    }
    h.finalize().into()
}

/// 개인키에 맞는 X25519 공개키를 돌려줌
pub fn public_key(private: &[u8; 32]) -> [u8; 32] {
    curve25519_dalek::MontgomeryPoint::mul_base_clamped(*private).to_bytes()
}

/// `auth`에 담는 클라이언트 증명임: `client_pub`의 개인키를 가졌음을 보임
pub fn auth_mac(shared: &[u8; 32], nonce: &[u8; 32], relay_pub: &[u8; 32], client_pub: &[u8; 32], id: &str) -> [u8; 32] {
    blake2s(&[AUTH_LABEL, shared, nonce, relay_pub, client_pub, id.as_bytes()])
}

/// `welcome`에 담는 증명임: 릴레이가 알린 키의 개인키를 가졌음을 보임
pub fn welcome_proof(shared: &[u8; 32], cnonce: &[u8; 32], nonce: &[u8; 32], client_pub: &[u8; 32]) -> [u8; 32] {
    blake2s(&[WELCOME_LABEL, shared, cnonce, nonce, client_pub])
}

/// OS 암호 난수로 32바이트를 만듦
pub fn random32() -> [u8; 32] {
    let mut k = [0u8; 32];
    getrandom::fill(&mut k).expect("OS random number generator unavailable");
    k
}

/// 바이트를 소문자 hex 문자열로 바꿈
pub fn to_hex(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
}

/// 정확히 N바이트짜리 hex(대소문자 무관)만 받음. 아니면 None임
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

/// 정확히 32바이트짜리 hex만 받음. 아니면 None임
pub fn hex32(s: &str) -> Option<[u8; 32]> {
    hex_array::<32>(s)
}

/// hex 변환과 핸드셰이크 MAC 고정 시험임
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

    /// 와이어 위 바이트임: 라벨이나 순서를 바꾸면 옛 클라이언트가 릴레이에 못 붙음
    #[test]
    fn handshake_macs_stay_what_old_peers_compute() {
        let (s, n, r, c) = ([1u8; 32], [2u8; 32], [3u8; 32], [4u8; 32]);
        assert_eq!(auth_mac(&s, &n, &r, &c, "id"), blake2s(&[b"screenpet-relay/1 auth", &s, &n, &r, &c, b"id"]));
        assert_eq!(welcome_proof(&s, &c, &n, &r), blake2s(&[b"screenpet-relay/1 welcome", &s, &c, &n, &r]));
    }
}
