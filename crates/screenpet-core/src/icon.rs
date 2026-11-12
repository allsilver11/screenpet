//! 📌 장난감 아이콘 그림 다루기 (순수 계산 — OS마다 읽어 온 칸을 같은 RGBA로 맞춤. 읽기는 platform).
//! 그림은 이 컴퓨터 안에서만 씀: 물건 창의 초기화 스크립트에 base64로 넣어 웹뷰가 canvas에 그림 (CSP img-src 'self' 그대로 — data: URL을 쓰지 않음).

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
