//! 🎨 몸 색 (흐물이처럼 잉크 선으로 그린 동물만 — pet.json `"tint": true`). 그림은 그대로 두고, 웹뷰가 잉크 자리
//! (`ink_<stage>.png`, tools/draw_heumul.py)를 고른 색으로 덮음. 어두운 바탕화면에서 짙은 잉크 선이 안 보인다는 피드백.
//! 설정에 남고, 놀러 가면 친구 화면에서도 같은 색임 (`Visit.tint` — 모르는 값은 기본).

use serde::{Deserialize, Serialize};

/// 몸 색
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BodyTint {
    /// 그림 그대로 (짙은 잉크)
    #[default]
    Default,
    White,
    Red,
    Orange,
    Yellow,
    Green,
    Sky,
    Blue,
    Purple,
    Pink,
    /// 🌈 머리부터 발끝까지 무지개
    Rainbow,
}

impl BodyTint {
    /// 메뉴 순서 (ui/main.js TINTS와 같은 id)
    pub const ALL: [BodyTint; 11] = [
        BodyTint::Default,
        BodyTint::White,
        BodyTint::Red,
        BodyTint::Orange,
        BodyTint::Yellow,
        BodyTint::Green,
        BodyTint::Sky,
        BodyTint::Blue,
        BodyTint::Purple,
        BodyTint::Pink,
        BodyTint::Rainbow,
    ];

    /// 그림 그대로임
    pub fn is_default(&self) -> bool {
        *self == BodyTint::Default
    }

    /// 설정·메시지·웹뷰에 쓰는 id
    pub fn id(self) -> &'static str {
        match self {
            BodyTint::Default => "default",
            BodyTint::White => "white",
            BodyTint::Red => "red",
            BodyTint::Orange => "orange",
            BodyTint::Yellow => "yellow",
            BodyTint::Green => "green",
            BodyTint::Sky => "sky",
            BodyTint::Blue => "blue",
            BodyTint::Purple => "purple",
            BodyTint::Pink => "pink",
            BodyTint::Rainbow => "rainbow",
        }
    }

    /// 메뉴 글자
    pub fn label(self) -> &'static str {
        match self {
            BodyTint::Default => "기본",
            BodyTint::White => "흰색",
            BodyTint::Red => "빨강",
            BodyTint::Orange => "주황",
            BodyTint::Yellow => "노랑",
            BodyTint::Green => "초록",
            BodyTint::Sky => "하늘",
            BodyTint::Blue => "파랑",
            BodyTint::Purple => "보라",
            BodyTint::Pink => "분홍",
            BodyTint::Rainbow => "🌈 무지개",
        }
    }

    /// id로 색을 찾음
    pub fn from_id(raw: &str) -> Option<BodyTint> {
        BodyTint::ALL.into_iter().find(|t| t.id() == raw)
    }

    /// 친구가 보낸 값 (`Visit.tint`): 아는 id만, 없거나 모르는 값은 기본
    pub fn from_wire(raw: Option<&str>) -> BodyTint {
        raw.and_then(BodyTint::from_id).unwrap_or_default()
    }

    /// 친구에게 보낼 값: 기본은 싣지 않음 (예전 버전과 메시지가 똑같음)
    pub fn to_wire(self) -> Option<String> {
        (!self.is_default()).then(|| self.id().to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_round_trip_and_match_serde() {
        assert_eq!(BodyTint::ALL.len(), 11);
        for t in BodyTint::ALL {
            assert_eq!(BodyTint::from_id(t.id()), Some(t));
            assert_eq!(serde_json::to_string(&t).unwrap(), format!("\"{}\"", t.id()));
            assert_eq!(BodyTint::from_wire(t.to_wire().as_deref()), t);
        }
        assert_eq!(BodyTint::ALL[0], BodyTint::default());
        assert_eq!(BodyTint::ALL[10], BodyTint::Rainbow);
    }

    #[test]
    fn wire_drops_default_and_unknown_values() {
        assert_eq!(BodyTint::Default.to_wire(), None);
        assert_eq!(BodyTint::Rainbow.to_wire().as_deref(), Some("rainbow"));
        for bad in [None, Some(""), Some("Rainbow"), Some("magenta"), Some("default ")] {
            assert_eq!(BodyTint::from_wire(bad), BodyTint::Default, "{bad:?}");
        }
    }

    #[test]
    fn labels_are_korean_and_rainbow_has_its_emoji() {
        assert_eq!(BodyTint::Default.label(), "기본");
        assert_eq!(BodyTint::White.label(), "흰색");
        assert_eq!(BodyTint::Rainbow.label(), "🌈 무지개");
        let labels: std::collections::HashSet<_> = BodyTint::ALL.iter().map(|t| t.label()).collect();
        assert_eq!(labels.len(), BodyTint::ALL.len());
    }
}
