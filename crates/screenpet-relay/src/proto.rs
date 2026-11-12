//! 릴레이 와이어 프로토콜임: 메시지·버전·한도는 `screenpet_proto`에 있음 (앱과 같은 것).
//! 여기서는 그것을 다시 내보내고 릴레이의 암호·친구 코드 도우미를 더함.
//!
//! 제어 메시지는 `t`로 꼬리표를 단 JSON 텍스트 프레임임 (모든 메시지는 `screenpet_proto::relay` 참고).
//! 친구 사이 트래픽은 바이너리 프레임임: 4바이트 빅엔디언 채널 id + 불투명 프레임 하나(최대 65535바이트).
//! 두 친구 사이의 Noise 메시지라 릴레이는 읽지도 위조하지도 못함.
//! `admin friendcode off`인 동안: welcome과 code는 코드 "----"를 싣고, redeem_failed는 `disabled`라고 함.

pub use screenpet_crypto::{auth_mac, hex32, public_key, random32, to_hex, welcome_proof};
pub use screenpet_proto::relay::{BoardIn, ClientMsg, ServerMsg};
pub use screenpet_proto::{MAX_FRAME, MAX_ID};

/// 2 = `weather_want`를 받음 (v0.6.3). 3 = 📮 `board_put`, `board_list`, `letter_reject` (v0.6.3).
/// 앱은 챌린지에서 `v`가 충분히 높은 릴레이에만 각각을 보냄: 옛 릴레이는 모르는 메시지에 연결을 닫음
pub const PROTOCOL_VERSION: u32 = screenpet_proto::RELAY_PROTOCOL_VERSION;
/// 바이너리 메시지의 채널 id 머리 크기
pub const HEADER: usize = 4;

/// X25519(private, public)를 계산함. 낮은 차수 공개키(결과가 모두 0)면 None임
pub fn dh(private: &[u8; 32], public: &[u8; 32]) -> Option<[u8; 32]> {
    let shared = curve25519_dalek::MontgomeryPoint(*public).mul_clamped(*private).to_bytes();
    if shared.iter().all(|b| *b == 0) {
        None
    } else {
        Some(shared)
    }
}

/// 일찍 빠져나가지 않고 같은지 봄
pub fn ct_eq(a: &[u8; 32], b: &[u8; 32]) -> bool {
    a.iter().zip(b).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

/// 32바이트 키의 표준(소문자) hex를 돌려줌. 모양이 틀리면 None임
pub fn norm_key(s: &str) -> Option<String> {
    hex32(s).map(|k| to_hex(&k))
}

/// Crockford base32 기호 (I, L, O, U 없음)
const CROCKFORD: &[u8; 32] = b"0123456789ABCDEFGHJKMNPQRSTVWXYZ";

/// 새 친구 코드를 만듦: Crockford base32 8자(난수 40비트)를 `ABCD-1234` 모양으로
pub fn new_code() -> String {
    let r = random32();
    let bits = r[..5].iter().fold(0u64, |acc, b| (acc << 8) | u64::from(*b));
    let chars: String = (0..8).map(|i| CROCKFORD[((bits >> (35 - 5 * i)) & 31) as usize] as char).collect();
    format!("{}-{}", &chars[..4], &chars[4..])
}

/// 사용자 입력을 `ABCD-1234`로 맞춤: 대소문자 무관, 구분자 무시, 헷갈리는 글자 바꿈(O→0, I/L→1). 코드가 아니면 None임
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

/// 코드·MAC·메시지 왕복 시험임
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
