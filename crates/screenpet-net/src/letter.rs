//! 💌 개발자에게 보내는 건의 편지임 (스펙 2026-11-10-feedback-mailbox).
//!
//! 앱이 [`Letter`]를 개발자 공개키에 한 메시지짜리 `Noise_N_25519_ChaChaPoly_BLAKE2s`로 봉인함:
//! 개발자 비밀키를 가진 사람만 열 수 있음. 릴레이는 봉인된 바이트만 저장하고 글은 못 봄.
//! 🖼️ 사진(WebP/JPEG 한 장, ≤ `MAX_IMAGE`)은 같은 Noise 세션의 전송 메시지들로 따로 봉인함: 편지 봉투는 작게 남아 목록에 실리고,
//! 사진은 그 편지에만 묶임 (바꾸거나 자르거나 다른 편지 사진을 붙이면 못 엶). 사진 없는 봉투는 옛 모양과 같음.
//! 주요 진입점: `seal`, `seal_letter`, `open`, `open_photo`, `clean`. 앱의 `feedback`/`mailbox`가 부름.

use serde::{Deserialize, Serialize};

/// 개발자 공개키(hex)임. 비밀키는 개발자의 `config.json`(`dev_key`)에만 있음.
/// 새 쌍은 `cargo run -p screenpet-net --example dev_key`로 만듦
pub const DEV_KEY: &str = "0116e5d331acff1d42f3dbee5be0edf72187fa861f44e7bbcf9182012cfa046a";
/// 편지 본문 최대 글자 수 (다듬은 뒤)
pub const MAX_LETTER_CHARS: usize = 1000;
/// 편지함에 보이는 보낸이 이름 최대 길이
pub const MAX_NAME: usize = 64;
/// 펫·앱 id 필드 최대 길이
const MAX_ID: usize = 32;
/// 가장 큰 봉인 편지 (릴레이가 더 큰 것은 거절함)
pub use screenpet_proto::MAX_SEALED;
/// 🖼️ 사진 한 장 최대 바이트와 봉인 사진 최대 바이트
pub use screenpet_proto::{MAX_IMAGE, MAX_IMG_SEALED};
/// 사진을 나눠 봉인하는 조각 크기 (Noise 메시지 한 개는 64 KiB까지)
const PHOTO_CHUNK: usize = 60_000;
/// Noise 전송 메시지의 인증 태그 바이트
const TAG: usize = 16;

/// 편지 봉인에 쓰는 Noise 패턴임
const PATTERN: &str = "Noise_N_25519_ChaChaPoly_BLAKE2s";
/// 편지 핸드셰이크 프롤로그임
const PROLOGUE: &[u8] = b"screenpet-letter/1";

/// 릴레이가 나열하는 개발자 편지함의 편지 하나임 (아직 봉인 상태임.
/// 앱이 자기 `dev_key`로 열고, net 계층은 내용을 못 봄)
#[derive(Debug, Clone, PartialEq)]
pub struct MailItem {
    /// 편지 id
    pub id: u64,
    /// 릴레이가 받은 유닉스 초
    pub at: u64,
    /// 보낸 설치의 키(hex), 차단용
    pub from: String,
    /// 읽음 표시
    pub read: bool,
    /// 개발자가 "반영함"으로 체크했음
    pub done: bool,
    /// 🙅 개발자가 반려했음 (done과 함께는 아님)
    pub rejected: bool,
    /// 🔒 공개 건의함에서 숨겼음 (비공개)
    pub private: bool,
    /// 봉인된 바이트
    pub sealed: Vec<u8>,
    /// 🖼️ 릴레이에 둔 봉인 사진 바이트 수 (0 = 없음, [`crate::Net::mailbox_photo`]로 받음)
    pub img: u32,
}

/// 개발자가 읽는 내용임. 보낸이에 대해 이 밖에는 보내지 않음 (주소·이메일·친구 없음)
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Letter {
    /// 편지 본문
    pub text: String,
    /// 보낸이 표시 이름 (없으면 "")
    pub name: String,
    /// 편지를 나르는 펫
    pub species: String,
    /// 펫 성장 단계
    pub stage: String,
    /// 펫 스타일 (옛 편지 호환용, v0.7.0부터 늘 "round" — 받는 쪽은 쓰지 않음)
    pub style: String,
    /// 앱 버전
    pub version: String,
    /// OS (`windows` / `macos`)
    pub os: String,
    /// 🖼️ 같이 봉인한 사진 바이트 수 (0 = 사진 없음, 와이어에 안 나옴 — 옛 편지와 같은 모양)
    #[serde(skip_serializing_if = "is_zero")]
    pub img: u32,
}

/// 봉인한 편지임: 사진 없는 봉투와, 사진이 있으면 (사진을 말하는 봉투, 봉인 사진)
#[derive(Debug, Clone, PartialEq)]
pub struct Sealed {
    /// 사진 없는 봉투 (옛 릴레이에 보내는 것)
    pub plain: Vec<u8>,
    /// 사진을 말하는 봉투와 봉인 사진 (v5 릴레이에 보내는 것)
    pub photo: Option<(Vec<u8>, Vec<u8>)>,
}

/// 0이면 JSON에 안 씀
fn is_zero(n: &u32) -> bool {
    *n == 0
}

/// 앱이 만드는 사진 형식인지 머리 바이트로 봄 (WebP, JPEG만). MIME을 돌려줌
pub fn photo_kind(b: &[u8]) -> Option<&'static str> {
    if b.len() >= 12 && &b[..4] == b"RIFF" && &b[8..12] == b"WEBP" {
        Some("image/webp")
    } else if b.len() >= 3 && b[..3] == [0xFF, 0xD8, 0xFF] {
        Some("image/jpeg")
    } else {
        None
    }
}

/// 버릴 보이지 않는·제어 문자인지 봄 (줄바꿈은 남김)
fn hidden(c: char) -> bool {
    (c.is_control() && c != '\n')
        || matches!(c, '\u{200B}'..='\u{200F}' | '\u{202A}'..='\u{202E}' | '\u{2060}'..='\u{2064}' | '\u{2066}'..='\u{2069}' | '\u{FEFF}' | '\u{061C}' | '\u{180E}' | '\u{00AD}' | '\u{3164}' | '\u{115F}' | '\u{1160}' | '\u{FFA0}' | '\u{2800}' | '\u{E0000}'..='\u{E007F}' | '\u{2028}' | '\u{2029}')
}

/// 본문을 다듬음: CRLF → LF, 보이지 않는·제어 문자 제거(줄바꿈은 남김),
/// 빈 줄은 연속 하나까지, 앞뒤 공백 제거, 최대 `MAX_LETTER_CHARS`글자
fn clean_body(raw: &str) -> String {
    let raw = raw.replace("\r\n", "\n").replace('\r', "\n");
    let mut out = String::new();
    let mut newlines = 0;
    for c in raw.chars().filter(|&c| !hidden(c)) {
        if c == '\n' {
            newlines += 1;
            if newlines > 2 {
                continue;
            }
        } else {
            newlines = 0;
        }
        out.push(c);
    }
    let t: String = out.trim().chars().take(MAX_LETTER_CHARS).collect();
    t.trim_end().to_string()
}

/// 한 줄 필드를 다듬음: 숨은 문자·줄바꿈 제거, 앞뒤 공백 제거, 최대 `max`글자
fn clean_line(raw: &str, max: usize) -> String {
    let t: String = raw.chars().filter(|&c| !hidden(c) && c != '\n').collect();
    t.trim().chars().take(max).collect::<String>().trim_end().to_string()
}

/// id 필드에 영숫자와 `_-.`만 남기고 `MAX_ID`글자로 자름
fn clean_id(raw: &str) -> String {
    raw.chars().filter(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | '.')).take(MAX_ID).collect::<String>().to_ascii_lowercase()
}

/// 모든 필드를 다듬음. 본문이 남지 않으면 None임
pub fn clean(l: Letter) -> Option<Letter> {
    let text = clean_body(&l.text);
    if text.is_empty() {
        return None;
    }
    Some(Letter {
        text,
        name: clean_line(&l.name, MAX_NAME),
        species: clean_id(&l.species),
        stage: clean_id(&l.stage),
        style: clean_id(&l.style),
        version: clean_id(&l.version),
        os: clean_id(&l.os),
        img: l.img.min(MAX_IMAGE as u32),
    })
}

/// `letter`를 공개키 `to`에 봉인함. 할 말이 없거나 너무 크면 None임
pub fn seal(letter: &Letter, to: &[u8; 32]) -> Option<Vec<u8>> {
    let letter = Letter { img: 0, ..clean(letter.clone())? };
    seal_session(&letter, to).map(|(sealed, _)| sealed)
}

/// 다듬은 편지를 봉인하고 그 세션을 (사진을 이어 봉인하게) 돌려줌. 봉투가 너무 크면 None임
fn seal_session(letter: &Letter, to: &[u8; 32]) -> Option<(Vec<u8>, snow::HandshakeState)> {
    let payload = serde_json::to_vec(letter).ok()?;
    let mut hs = snow::Builder::new(PATTERN.parse().ok()?).prologue(PROLOGUE).ok()?.remote_public_key(to).ok()?.build_initiator().ok()?;
    let mut buf = vec![0u8; payload.len() + 128];
    let n = hs.write_message(&payload, &mut buf).ok()?;
    buf.truncate(n);
    (buf.len() <= MAX_SEALED).then_some((buf, hs))
}

/// 🖼️ 편지와 (있으면) 사진 한 장을 봉인함: 사진 없는 봉투는 늘, 사진이 있으면 사진을 말하는 봉투와 봉인 사진도.
/// 할 말이 없거나, 사진이 WebP/JPEG가 아니거나 `MAX_IMAGE`보다 크면 None임
pub fn seal_letter(letter: &Letter, photo: Option<&[u8]>, to: &[u8; 32]) -> Option<Sealed> {
    let plain = seal(letter, to)?;
    let Some(photo) = photo else { return Some(Sealed { plain, photo: None }) };
    if photo.is_empty() || photo.len() > MAX_IMAGE || photo_kind(photo).is_none() {
        return None;
    }
    let with = Letter { img: photo.len() as u32, ..clean(letter.clone())? };
    let (sealed, hs) = seal_session(&with, to)?;
    let mut t = hs.into_transport_mode().ok()?;
    let mut out = Vec::with_capacity(photo.len() + 64);
    for chunk in photo.chunks(PHOTO_CHUNK) {
        let mut c = vec![0u8; chunk.len() + TAG];
        let n = t.write_message(chunk, &mut c).ok()?;
        out.extend_from_slice(&(n as u16).to_be_bytes());
        out.extend_from_slice(&c[..n]);
    }
    (out.len() <= MAX_IMG_SEALED).then_some(Sealed { plain, photo: Some((sealed, out)) })
}

/// 🖼️ 봉투 `sealed`에 묶인 봉인 사진을 엶. 봉투가 사진을 말하지 않거나, 길이가 다르거나, 바뀌었거나,
/// WebP/JPEG가 아니면 None임
pub fn open_photo(sealed: &[u8], photo: &[u8], secret: &[u8; 32]) -> Option<Vec<u8>> {
    if sealed.is_empty() || sealed.len() > MAX_SEALED || photo.is_empty() || photo.len() > MAX_IMG_SEALED {
        return None;
    }
    let mut hs = snow::Builder::new(PATTERN.parse().ok()?).prologue(PROLOGUE).ok()?.local_private_key(secret).ok()?.build_responder().ok()?;
    let mut buf = vec![0u8; sealed.len()];
    let n = hs.read_message(sealed, &mut buf).ok()?;
    let want = clean(serde_json::from_slice::<Letter>(&buf[..n]).ok()?)?.img as usize;
    if want == 0 {
        return None;
    }
    let mut t = hs.into_transport_mode().ok()?;
    let mut out = Vec::with_capacity(want);
    let mut rest = photo;
    while !rest.is_empty() {
        let len = u16::from_be_bytes([rest[0], *rest.get(1)?]) as usize;
        rest = &rest[2..];
        if len <= TAG || len > rest.len() {
            return None;
        }
        let mut p = vec![0u8; len];
        let m = t.read_message(&rest[..len], &mut p).ok()?;
        out.extend_from_slice(&p[..m]);
        if out.len() > want {
            return None;
        }
        rest = &rest[len..];
    }
    (out.len() == want && photo_kind(&out).is_some()).then_some(out)
}

/// 개발자 비밀키로 봉인 편지를 엶. 우리 것이 아니거나 바뀌었거나 편지로 안 읽히면 None임.
/// 결과는 다시 다듬음
pub fn open(sealed: &[u8], secret: &[u8; 32]) -> Option<Letter> {
    if sealed.is_empty() || sealed.len() > MAX_SEALED {
        return None;
    }
    let mut hs = snow::Builder::new(PATTERN.parse().ok()?).prologue(PROLOGUE).ok()?.local_private_key(secret).ok()?.build_responder().ok()?;
    let mut buf = vec![0u8; sealed.len()];
    let n = hs.read_message(sealed, &mut buf).ok()?;
    clean(serde_json::from_slice(&buf[..n]).ok()?)
}

/// 배포된 개발자 공개키를 돌려줌
pub fn dev_key() -> Option<[u8; 32]> {
    crate::noise::hex32(DEV_KEY)
}

/// `config.json`에 적힌 개발자 비밀키(hex, 앞뒤 공백 무시)를 읽음
pub fn secret_from_hex(s: &str) -> Option<[u8; 32]> {
    crate::noise::hex32(s.trim())
}

/// 새 (비밀키, 공개키) 쌍을 hex로 만듦
pub fn new_dev_keypair() -> (String, String) {
    let secret = crate::noise::random32();
    (crate::noise::to_hex(&secret), crate::noise::to_hex(&crate::noise::public_key(&secret)))
}

/// 봉인·열기·다듬기·배포 키 시험임
#[cfg(test)]
mod tests {
    use super::*;

    fn keypair() -> ([u8; 32], [u8; 32]) {
        let secret = crate::noise::random32();
        (secret, crate::noise::public_key(&secret))
    }

    #[test]
    fn a_sealed_letter_opens_only_with_the_developer_secret() {
        let (secret, public) = keypair();
        let l = Letter { text: "펭귄 귀여워요\n고마워요".into(), name: "단니".into(), species: "penguin".into(), ..Default::default() };
        let sealed = seal(&l, &public).unwrap();
        let opened = open(&sealed, &secret).unwrap();
        assert_eq!((opened.text.as_str(), opened.name.as_str(), opened.species.as_str()), ("펭귄 귀여워요\n고마워요", "단니", "penguin"));
        let (other, _) = keypair();
        assert!(open(&sealed, &other).is_none());
        assert!(open(b"junk", &secret).is_none());
        let mut tampered = sealed.clone();
        *tampered.last_mut().unwrap() ^= 1;
        assert!(open(&tampered, &secret).is_none());
    }

    #[test]
    fn letters_are_cleaned_and_limited() {
        let long = Letter { text: "가".repeat(1500), ..Default::default() };
        assert_eq!(clean(long).unwrap().text.chars().count(), MAX_LETTER_CHARS);
        assert!(clean(Letter { text: " \n \u{200B} ".into(), ..Default::default() }).is_none());
        let messy = clean(Letter { text: "a\r\nb\u{0007}\n\n\n\n\nc".into(), species: "<b>cat</b>".into(), ..Default::default() }).unwrap();
        assert_eq!(messy.text, "a\nb\n\nc", "CRLF, control chars and runs of blank lines");
        assert_eq!(messy.species, "bcatb", "ids keep only [a-z0-9_.-]");
        let (secret, public) = keypair();
        let sealed = seal(&Letter { text: "가".repeat(1000), name: "x".repeat(200), ..Default::default() }, &public).unwrap();
        assert!(sealed.len() <= MAX_SEALED, "{}", sealed.len());
        assert_eq!(open(&sealed, &secret).unwrap().name.chars().count(), MAX_NAME);
        assert!(seal(&Letter::default(), &public).is_none(), "nothing to say");
    }

    /// 시험용 WebP 머리 + 채움 (`len` 바이트)
    fn webp(len: usize) -> Vec<u8> {
        let mut v = b"RIFF\0\0\0\0WEBPVP8 ".to_vec();
        v.extend((0..len - v.len()).map(|i| (i * 7 % 251) as u8));
        v
    }

    #[test]
    fn a_photo_letter_seals_the_photo_apart_and_bound_to_its_letter() {
        let (secret, public) = keypair();
        let l = Letter { text: "디멘터 성 사진이에요".into(), ..Default::default() };
        let photo = webp(MAX_IMAGE);
        let s = seal_letter(&l, Some(&photo), &public).unwrap();
        // 사진 없는 봉투도 같이 (옛 릴레이에는 이것만) — 옛 편지와 같은 모양
        let plain = open(&s.plain, &secret).unwrap();
        assert_eq!((plain.text.as_str(), plain.img), ("디멘터 성 사진이에요", 0));
        let (with, sealed_photo) = s.photo.clone().unwrap();
        assert!(with.len() <= MAX_SEALED && sealed_photo.len() <= MAX_IMG_SEALED, "{} {}", with.len(), sealed_photo.len());
        assert_eq!(open(&with, &secret).unwrap().img as usize, MAX_IMAGE, "the letter says a photo comes with it");
        assert_eq!(open_photo(&with, &sealed_photo, &secret).unwrap(), photo);
        assert!(open_photo(&with, &sealed_photo, &keypair().0).is_none(), "someone else's key");
        // 바꾸거나 자르거나 다른 편지의 사진을 붙이면 못 엶
        let mut bad = sealed_photo.clone();
        bad[100] ^= 1;
        assert!(open_photo(&with, &bad, &secret).is_none(), "tampered");
        assert!(open_photo(&with, &sealed_photo[..sealed_photo.len() - 1], &secret).is_none(), "cut");
        let other = seal_letter(&l, Some(&webp(5000)), &public).unwrap().photo.unwrap();
        assert!(open_photo(&with, &other.1, &secret).is_none() && open_photo(&other.0, &sealed_photo, &secret).is_none(), "swapped");
        assert!(open_photo(&s.plain, &sealed_photo, &secret).is_none(), "a letter without a photo");
        assert!(open_photo(&with, &[], &secret).is_none());
        // 사진이 없으면 봉투 하나
        assert!(seal_letter(&l, None, &public).unwrap().photo.is_none());
    }

    #[test]
    fn only_small_webp_or_jpeg_photos_go_in() {
        let (_, public) = keypair();
        let l = Letter { text: "사진".into(), ..Default::default() };
        assert!(seal_letter(&l, Some(&webp(MAX_IMAGE + 1)), &public).is_none(), "too big");
        let mut jpeg = vec![0xFF, 0xD8, 0xFF, 0xE0];
        jpeg.resize(2000, 9);
        assert!(seal_letter(&l, Some(&jpeg), &public).unwrap().photo.is_some());
        let mut png = b"\x89PNG\r\n\x1a\n".to_vec();
        png.resize(2000, 9);
        assert!(seal_letter(&l, Some(&png), &public).is_none(), "only what the app makes (WebP, JPEG)");
        assert!(seal_letter(&l, Some(b"<svg onload=x>"), &public).is_none());
        assert_eq!((photo_kind(&webp(100)), photo_kind(&jpeg), photo_kind(&png), photo_kind(b"RIFF")), (Some("image/webp"), Some("image/jpeg"), None, None));
        assert!(seal_letter(&Letter::default(), Some(&jpeg), &public).is_none(), "a photo alone is not a letter");
    }

    #[test]
    fn the_shipped_developer_key_is_a_valid_public_key() {
        assert!(dev_key().is_some());
        let (s, p) = new_dev_keypair();
        assert_eq!(crate::noise::to_hex(&crate::noise::public_key(&crate::noise::hex32(&s).unwrap())), p);
        assert_eq!(secret_from_hex(&format!("  {s}\n")), crate::noise::hex32(&s));
        assert_eq!(secret_from_hex("nope"), None);
    }
}
