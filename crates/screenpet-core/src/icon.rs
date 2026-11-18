//! 📌 장난감 아이콘 그림 다루기 (순수 계산 — OS마다 읽어 온 칸을 같은 RGBA로 맞춤. 읽기는 platform).
//! 물건 창의 초기화 스크립트에 base64로 넣어 웹뷰가 canvas에 그림 (CSP img-src 'self' 그대로 — data: URL을 쓰지 않음).
//! 🔓 (v0.6.5, 사용자 결정) 장난감을 친구에게 던지거나 들고 가면 그림 칸도 같이 감: WIRE_ICON_PX로 줄인 RGBA를 base64로 (to_wire).
//! 받는 쪽은 크기·글자 수·글자·빈 그림을 엄격히 확인하고 (accept_wire) 조금이라도 이상하면 버림. 그림 칸뿐이고 앱 이름·창 제목은 없음

/// 정사각형 RGBA 그림 (곧은 알파, 위→아래 줄)
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IconImage {
    /// 한 변 (px)
    pub size: u32,
    /// RGBA 바이트 (size × size × 4)
    pub rgba: Vec<u8>,
}

impl IconImage {
    /// 거의 다 투명하면 (빈 아이콘) 장난감으로 쓰지 않음
    pub fn is_blank(&self) -> bool {
        let seen = self.rgba.chunks_exact(4).filter(|p| p[3] >= 32).count();
        seen * 20 < (self.size * self.size) as usize
    }
}

/// Windows: 같은 아이콘을 검은 바탕과 흰 바탕에 그린 BGRA 칸 (DrawIconEx) → 곧은 알파 RGBA.
/// 바탕이 비치는 만큼이 투명함임 (마스크 아이콘·알파 아이콘 모두): a = 255 − (흰 − 검), 색 = 검 ÷ a
pub fn from_black_white(size: u32, black: &[u8], white: &[u8]) -> Option<IconImage> {
    let n = (size as usize) * (size as usize) * 4;
    if size == 0 || black.len() < n || white.len() < n {
        return None;
    }
    let mut rgba = vec![0u8; n];
    for i in (0..n).step_by(4) {
        let (b, w) = (&black[i..i + 3], &white[i..i + 3]);
        let shown: i32 = (0..3).map(|c| i32::from(w[c]) - i32::from(b[c])).sum::<i32>() / 3;
        let a = (255 - shown).clamp(0, 255);
        if a == 0 {
            continue;
        }
        // BGRA → RGBA, 검은 바탕에 그린 색은 알파가 곱해져 있다
        for (dst, src) in [(0, 2), (1, 1), (2, 0)] {
            rgba[i + dst] = ((i32::from(b[src]) * 255 + a / 2) / a).clamp(0, 255) as u8;
        }
        rgba[i + 3] = a as u8;
    }
    let icon = IconImage { size, rgba };
    (!icon.is_blank()).then_some(icon)
}

/// macOS: 알파가 곱해진 RGBA (CGBitmapContext premultiplied last) → 곧은 알파
pub fn from_premultiplied(size: u32, mut rgba: Vec<u8>) -> Option<IconImage> {
    let n = (size as usize) * (size as usize) * 4;
    if size == 0 || rgba.len() < n {
        return None;
    }
    rgba.truncate(n);
    for p in rgba.chunks_exact_mut(4) {
        let a = u32::from(p[3]);
        if a == 0 {
            p[..3].fill(0);
            continue;
        }
        for c in &mut p[..3] {
            *c = ((u32::from(*c) * 255 + a / 2) / a).min(255) as u8;
        }
    }
    let icon = IconImage { size, rgba };
    (!icon.is_blank()).then_some(icon)
}

/// 친구에게 보내는 아이콘 한 변의 상한 (px): 48×48×4 = 9 KB → base64 12 KB라 메시지 한도(64 KB) 안에 넉넉함
pub const WIRE_ICON_PX: u32 = 48;
/// 받는 아이콘 한 변의 하한 (px) — 이보다 작으면 아이콘이 아님
pub const MIN_WIRE_ICON_PX: u32 = 8;

/// 한 변 size 아이콘의 base64 글자 수
pub fn wire_len(size: u32) -> usize {
    (size as usize * size as usize * 4).div_ceil(3) * 4
}

/// 한 변 px로 줄임 (칸 평균, 알파를 곱해 섞어 투명한 칸의 색이 번지지 않음). 이미 작거나 같으면 그대로
pub fn downscale(icon: &IconImage, px: u32) -> IconImage {
    if icon.size <= px || px == 0 {
        return icon.clone();
    }
    let (src, n) = (icon.size as usize, px as usize);
    let mut rgba = vec![0u8; n * n * 4];
    for y in 0..n {
        let (y0, y1) = (y * src / n, ((y + 1) * src / n).max(y * src / n + 1));
        for x in 0..n {
            let (x0, x1) = (x * src / n, ((x + 1) * src / n).max(x * src / n + 1));
            let mut sum = [0u64; 4];
            for sy in y0..y1 {
                for sx in x0..x1 {
                    let p = &icon.rgba[(sy * src + sx) * 4..(sy * src + sx) * 4 + 4];
                    let a = u64::from(p[3]);
                    for c in 0..3 {
                        sum[c] += u64::from(p[c]) * a;
                    }
                    sum[3] += a;
                }
            }
            let cells = ((y1 - y0) * (x1 - x0)) as u64;
            let o = (y * n + x) * 4;
            if sum[3] > 0 {
                for c in 0..3 {
                    rgba[o + c] = ((sum[c] + sum[3] / 2) / sum[3]).min(255) as u8;
                }
            }
            rgba[o + 3] = ((sum[3] + cells / 2) / cells).min(255) as u8;
        }
    }
    IconImage { size: px, rgba }
}

/// 친구에게 보낼 꼴: (한 변, base64 RGBA) — WIRE_ICON_PX보다 크면 줄임
pub fn to_wire(icon: &IconImage) -> (u32, String) {
    let small = downscale(icon, WIRE_ICON_PX);
    (small.size, base64(&small.rgba))
}

/// 친구가 보낸 아이콘을 엄격히 확인함: 한 변이 MIN_WIRE_ICON_PX..=WIRE_ICON_PX, 글자 수가 그 크기에 꼭 맞고, 표준 base64이고, 빈 그림이 아님. 아니면 None
pub fn accept_wire(size: u32, text: &str) -> Option<IconImage> {
    if !(MIN_WIRE_ICON_PX..=WIRE_ICON_PX).contains(&size) || text.len() != wire_len(size) {
        return None;
    }
    let rgba = from_base64(text)?;
    if rgba.len() != size as usize * size as usize * 4 {
        return None;
    }
    let icon = IconImage { size, rgba };
    (!icon.is_blank()).then_some(icon)
}

/// 표준 base64를 엄격히 풂: 4글자씩, = 는 맨 끝 두 개까지만, 그 밖의 글자(빈칸·줄바꿈 포함)가 있으면 None
pub fn from_base64(text: &str) -> Option<Vec<u8>> {
    let b = text.as_bytes();
    if b.len() % 4 != 0 {
        return None;
    }
    let value = |c: u8| -> Option<u32> {
        Some(match c {
            b'A'..=b'Z' => c - b'A',
            b'a'..=b'z' => c - b'a' + 26,
            b'0'..=b'9' => c - b'0' + 52,
            b'+' => 62,
            b'/' => 63,
            _ => return None,
        } as u32)
    };
    let mut out = Vec::with_capacity(b.len() / 4 * 3);
    for (k, chunk) in b.chunks(4).enumerate() {
        let last = k == b.len() / 4 - 1;
        let pad = chunk.iter().rev().take_while(|&&c| c == b'=').count();
        if pad > 2 || (pad > 0 && !last) {
            return None;
        }
        let mut v = 0u32;
        for &c in &chunk[..4 - pad] {
            v = (v << 6) | value(c)?;
        }
        v <<= 6 * pad as u32;
        let bytes = [(v >> 16) as u8, (v >> 8) as u8, v as u8];
        out.extend_from_slice(&bytes[..3 - pad]);
    }
    Some(out)
}

/// 표준 base64 (초기화 스크립트의 JS 문자열에 그대로 넣음 — 글자는 A–Z a–z 0–9 + / = 뿐)
pub fn base64(bytes: &[u8]) -> String {
    const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let v = (u32::from(chunk[0]) << 16) | (u32::from(*chunk.get(1).unwrap_or(&0)) << 8) | u32::from(*chunk.get(2).unwrap_or(&0));
        for k in 0..4 {
            if k <= chunk.len() {
                out.push(T[((v >> (18 - 6 * k)) & 63) as usize] as char);
            } else {
                out.push('=');
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 2×2: 불투명 빨강, 투명, 반투명 파랑, 불투명 흰색 — 검은 바탕·흰 바탕에 그린 모습 (BGRA)
    fn drawn() -> (Vec<u8>, Vec<u8>) {
        let black = vec![0, 0, 255, 255, 0, 0, 0, 255, 128, 0, 0, 255, 255, 255, 255, 255];
        let white = vec![0, 0, 255, 255, 255, 255, 255, 255, 255, 127, 127, 255, 255, 255, 255, 255];
        (black, white)
    }

    #[test]
    fn black_and_white_backgrounds_give_straight_alpha() {
        let (b, w) = drawn();
        let icon = from_black_white(2, &b, &w).unwrap();
        assert_eq!(&icon.rgba[0..4], &[255, 0, 0, 255], "불투명 빨강");
        assert_eq!(icon.rgba[7], 0, "투명");
        let blue = &icon.rgba[8..12];
        assert!((126..=130).contains(&blue[3]) && blue[2] >= 250 && blue[0] == 0, "{blue:?}");
        assert_eq!(&icon.rgba[12..16], &[255, 255, 255, 255], "흰 칸은 흰 바탕에서도 흰색 — 불투명");
    }

    #[test]
    fn blank_or_short_pictures_are_not_toys() {
        assert_eq!(from_black_white(2, &[0; 16], &[255; 16]), None, "다 투명");
        assert_eq!(from_black_white(4, &[0; 16], &[0; 16]), None, "칸이 모자란다");
        assert_eq!(from_premultiplied(2, vec![0; 16]), None);
        assert_eq!(from_black_white(0, &[], &[]), None);
    }

    #[test]
    fn premultiplied_colors_are_divided_back() {
        let icon = from_premultiplied(1, vec![64, 32, 0, 128]).unwrap();
        assert_eq!(icon.rgba, vec![128, 64, 0, 128]);
    }

    /// 테스트: 한 변 size, 가운데 네모만 불투명한 빨강 아이콘
    fn square(size: u32) -> IconImage {
        let mut rgba = vec![0u8; (size * size * 4) as usize];
        for y in size / 4..size * 3 / 4 {
            for x in size / 4..size * 3 / 4 {
                let i = ((y * size + x) * 4) as usize;
                rgba[i..i + 4].copy_from_slice(&[220, 40, 40, 255]);
            }
        }
        IconImage { size, rgba }
    }

    #[test]
    fn a_big_icon_is_shrunk_for_the_wire_and_keeps_its_look() {
        let (size, text) = to_wire(&square(96));
        assert_eq!(size, WIRE_ICON_PX);
        assert_eq!(text.len(), wire_len(WIRE_ICON_PX));
        let back = accept_wire(size, &text).expect("valid");
        assert_eq!(back.size, WIRE_ICON_PX);
        let mid = ((WIRE_ICON_PX / 2 * WIRE_ICON_PX + WIRE_ICON_PX / 2) * 4) as usize;
        assert_eq!(&back.rgba[mid..mid + 4], &[220, 40, 40, 255]);
        assert_eq!(back.rgba[3], 0, "corners stay clear");
        // 작은 아이콘은 그대로
        let small = square(32);
        let (size, text) = to_wire(&small);
        assert_eq!(accept_wire(size, &text), Some(small));
    }

    #[test]
    fn odd_icons_from_a_friend_are_refused() {
        let (size, text) = to_wire(&square(48));
        assert!(accept_wire(size, &text).is_some());
        assert_eq!(accept_wire(WIRE_ICON_PX + 1, &base64(&vec![255; ((WIRE_ICON_PX + 1) * (WIRE_ICON_PX + 1) * 4) as usize])), None, "too big");
        assert_eq!(accept_wire(0, ""), None);
        assert_eq!(accept_wire(4, &base64(&[255; 64])), None, "too small to be an icon");
        assert_eq!(accept_wire(size, &text[..text.len() - 4]), None, "short");
        assert_eq!(accept_wire(size, &format!("{text}AAAA")), None, "long");
        let mut bad = text.clone();
        bad.replace_range(10..11, "*");
        assert_eq!(accept_wire(size, &bad), None, "not base64");
        let mut bad = text.clone();
        bad.replace_range(8..9, "=");
        assert_eq!(accept_wire(size, &bad), None, "padding in the middle");
        assert_eq!(accept_wire(16, &base64(&[0; 16 * 16 * 4])), None, "blank");
    }

    #[test]
    fn base64_decodes_what_it_encodes_and_nothing_else() {
        for bytes in [&b""[..], b"f", b"fo", b"foo", b"foobar", &[255, 254, 253, 0, 1]] {
            assert_eq!(from_base64(&base64(bytes)).as_deref(), Some(bytes));
        }
        for bad in ["Zg=", "Z===", "Zm9v=", "Zm 9v", "Zm9v\n", "é", "Zg==Zg=="] {
            assert_eq!(from_base64(bad), None, "{bad:?}");
        }
    }

    #[test]
    fn base64_matches_the_standard_alphabet() {
        assert_eq!(base64(b""), "");
        assert_eq!(base64(b"f"), "Zg==");
        assert_eq!(base64(b"fo"), "Zm8=");
        assert_eq!(base64(b"foo"), "Zm9v");
        assert_eq!(base64(b"foobar"), "Zm9vYmFy");
        assert_eq!(base64(&[255, 254, 253]), "//79");
        assert!(base64(&[7; 100]).chars().all(|c| c.is_ascii_alphanumeric() || "+/=".contains(c)));
    }
}
