//! 🧳 포트키 (spec docs/superpowers/specs/2026-11-23-portkey.md)의 숫자와 짝 물건 열쇠 — 동작은 brain_portkey.rs, 앱이 물건 자리를 PortkeySpot으로 알려 줌.
//!
//! - 포트키는 늘 같은 물건 둘 사이를 잇는 문: 같은 종류 + 같은 그림 (PortkeyKey — 📌 장난감은 아이콘 그림 해시 icon_look, 나머지는 0)
//! - 친구에게 물어볼 땐 Visit.portkey {kind, look}로 (to_wire·from_wire — 아이콘은 그림 해시만, 앱 이름·창 제목은 보내지 않음)
//! - 숫자: 처음 PORTKEY_FIRST_SECS, 한가할 때 초당 PORTKEY_RATE, 다음은 PORTKEY_CALM_SECS 뒤 · 같은 디딤판 PORTKEY_REACH 안의 물건만 ·
//!   손 대기 TOUCH_SECS → 빙글 작아지며 사라짐 VANISH_SECS → 짝 물건 옆에 빙글 커지며 나타남 APPEAR_SECS ·
//!   같은 포트키 JOIN_REACH 안의 한가한 마법사는 같이 감 · 짝이 있어도 FRIEND_FIRST로 친구 쪽 먼저

use crate::icon::{self, IconImage};
use crate::juggle::ObjectSpot;
use crate::object::{ObjectKind, TOY_ID};

/// 처음 포트키를 쓸 수 있게 되는 때 (clock 초)
pub const PORTKEY_FIRST_SECS: f64 = 90.0;
/// 한가할 때 포트키를 찾는 빈도 (초당 — 🌱 power를 곱함)
pub const PORTKEY_RATE: f64 = 0.01;
/// 다음 포트키까지 (초 — 몇 분)
pub const PORTKEY_CALM_SECS: (f64, f64) = (180.0, 360.0);
/// 달려가는 물건: 같은 디딤판 이만큼 (몸 크기 배) 안
pub const PORTKEY_REACH: f64 = 12.0;
/// 물건까지 이만큼 (초) 달려도 못 닿으면 그만 (움직이는 물건·막힌 길)
pub const RUN_MAX_SECS: f64 = 8.0;
/// 물건에 손을 대는 시간 · 빙글 돌며 작아져 사라지는 시간 · 짝 물건 옆에 빙글 커지며 나타나는 시간 (초)
pub const TOUCH_SECS: f64 = 0.35;
pub const VANISH_SECS: f64 = 1.0;
pub const APPEAR_SECS: f64 = 0.8;
/// 사라지고 나타나는 동안 도는 바퀴 수
pub const SPIN_TURNS: f64 = 3.0;
/// 같은 포트키에 이만큼 (몸 크기 배, 가로) 가까운 한가한 마법사는 같이 잡고 같이 감
pub const JOIN_REACH: f64 = 4.0;
/// 내 화면에 짝이 있어도 (새 버전 친구가 연결돼 있으면) 친구 쪽을 먼저 물어보는 몫
pub const FRIEND_FIRST: f64 = 0.5;
/// 친구가 이만큼 (초) 안에 돌려보내면 거절로 봄 — 내 화면의 짝 물건으로, 없으면 제자리 (앱이 셈)
pub const ANSWER_SECS: f64 = 4.0;

/// 🧳 짝 물건을 알아보는 열쇠: 물건 종류 + 그림 (📌 장난감은 아이콘 그림 해시, 나머지는 0)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PortkeyKey {
    pub kind: ObjectKind,
    pub look: u64,
}

impl PortkeyKey {
    /// 장난감이 아닌 물건의 열쇠 (그림은 종류가 정함 — 색 똥·거대함도 종류가 다름)
    pub fn of(kind: ObjectKind) -> PortkeyKey {
        PortkeyKey { kind, look: 0 }
    }

    /// 📌 장난감의 열쇠 (아이콘 그림 해시)
    pub fn toy(icon: &IconImage) -> PortkeyKey {
        PortkeyKey { kind: ObjectKind::AppIcon, look: icon_look(icon) }
    }

    /// 친구에게 보내는 이름과 그림 (Visit.portkey): 물건은 Toss와 같은 이름·색, 장난감은 "app_icon" + 그림 해시 16자리
    pub fn to_wire(self) -> (String, Option<String>) {
        if self.kind.is_toy() {
            (TOY_ID.to_string(), Some(format!("{:016x}", self.look)))
        } else {
            (self.kind.wire_id().to_string(), self.kind.look().map(str::to_string))
        }
    }

    /// 친구가 보낸 이름과 그림 → 열쇠. 아는 이름·색이 정확히 맞을 때만 (보낸 쪽 to_wire와 같은 모양), 아니면 None
    pub fn from_wire(kind: &str, look: Option<&str>) -> Option<PortkeyKey> {
        if kind == TOY_ID {
            let hex = look.filter(|h| h.len() == 16 && h.bytes().all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase()))?;
            return u64::from_str_radix(hex, 16).ok().map(|look| PortkeyKey { kind: ObjectKind::AppIcon, look });
        }
        let k = ObjectKind::wire_kind(kind, look)?;
        (k.wire_id() == kind && k.look() == look).then_some(PortkeyKey::of(k))
    }
}

/// 📌 아이콘 그림 해시: 친구에게 보내는 크기 (icon::WIRE_ICON_PX)로 줄인 RGBA의 FNV-1a — 던져 받은 장난감과 같은 값
pub fn icon_look(icon: &IconImage) -> u64 {
    let small = icon::downscale(icon, icon::WIRE_ICON_PX);
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in small.size.to_le_bytes().iter().chain(small.rgba.iter()) {
        h ^= u64::from(*b);
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    h
}

/// 🧳 포트키가 될 수 있는 놓인 물건 하나 (앱이 매 틱 — 바닥·창 위에 가만히 놓인 것만)
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PortkeySpot {
    /// 자리 (물건 id·바닥 가운데·디딤판·반 폭·거대함)
    pub spot: ObjectSpot,
    /// 짝 열쇠
    pub key: PortkeyKey,
}

/// 같은 열쇠의 다른 물건 (짝)이 있음
pub fn has_twin(spots: &[PortkeySpot], key: PortkeyKey, id: u64) -> bool {
    spots.iter().any(|s| s.key == key && s.spot.id != id)
}

/// 친구가 물어본 열쇠와 같은 물건 하나 (받는 쪽 — 여럿이면 id가 가장 작은 것: 같이 온 마법사가 같은 자리에)
pub fn pick_arrival(spots: &[PortkeySpot], key: PortkeyKey) -> Option<PortkeySpot> {
    spots.iter().filter(|s| s.key == key).min_by_key(|s| s.spot.id).copied()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::surfaces::Owner;

    /// 테스트 물건 자리
    fn at(id: u64, kind: ObjectKind) -> PortkeySpot {
        PortkeySpot { spot: ObjectSpot { id, x: 100.0 * id as f64, y: 1040.0, owner: Owner::Ground(0), half_w: 20.0, giant: kind.is_giant(), local: kind.is_toy() }, key: PortkeyKey::of(kind) }
    }

    #[test]
    fn keys_go_over_the_wire_like_tosses_and_bad_values_are_refused() {
        for kind in ObjectKind::ALL {
            let key = PortkeyKey::of(kind);
            let (k, l) = key.to_wire();
            assert_eq!(PortkeyKey::from_wire(&k, l.as_deref()), Some(key), "{kind:?}");
        }
        assert_eq!(PortkeyKey::of(ObjectKind::Pink).to_wire(), ("poop".to_string(), Some("pink".to_string())));
        assert_eq!(PortkeyKey::of(ObjectKind::GiantStone).to_wire(), ("stone".to_string(), Some("giant".to_string())));
        let toy = PortkeyKey { kind: ObjectKind::AppIcon, look: 0xdead_beef_0000_0001 };
        assert_eq!(toy.to_wire(), ("app_icon".to_string(), Some("deadbeef00000001".to_string())));
        assert_eq!(PortkeyKey::from_wire("app_icon", Some("deadbeef00000001")), Some(toy));
        for (k, l) in [("app_icon", None), ("app_icon", Some("DEADBEEF00000001")), ("app_icon", Some("xyz")), ("nope", None), ("poop", Some("plaid")), ("can", Some("giant")), ("sock", Some("pink")), ("", None)] {
            assert_eq!(PortkeyKey::from_wire(k, l), None, "{k} {l:?}");
        }
        // 옛 이름 (양말 = 티셔츠)도 보낸 모양 그대로면 받음
        assert_eq!(PortkeyKey::from_wire("sock", None), Some(PortkeyKey::of(ObjectKind::Shirt)));
    }

    #[test]
    fn icon_looks_match_only_the_same_picture_and_survive_the_wire() {
        let red = IconImage { size: 64, rgba: [255, 0, 0, 255].repeat(64 * 64) };
        let blue = IconImage { size: 64, rgba: [0, 0, 255, 255].repeat(64 * 64) };
        assert_eq!(icon_look(&red), icon_look(&red.clone()));
        assert_ne!(icon_look(&red), icon_look(&blue));
        let (size, text) = icon::to_wire(&red);
        let got = icon::accept_wire(size, &text).unwrap();
        assert_eq!(icon_look(&got), icon_look(&red), "던져 받은 장난감과 같은 해시");
    }

    #[test]
    fn twins_and_arrivals_need_the_same_key() {
        let spots = [at(1, ObjectKind::Paper), at(2, ObjectKind::Paper), at(3, ObjectKind::Stone), at(4, ObjectKind::Poop), at(5, ObjectKind::Pink)];
        assert!(has_twin(&spots, PortkeyKey::of(ObjectKind::Paper), 1));
        assert!(!has_twin(&spots, PortkeyKey::of(ObjectKind::Stone), 3));
        assert!(!has_twin(&spots, PortkeyKey::of(ObjectKind::Poop), 4), "갈색 똥과 핑크 똥은 다른 그림");
        assert_eq!(pick_arrival(&spots, PortkeyKey::of(ObjectKind::Paper)).map(|s| s.spot.id), Some(1));
        assert_eq!(pick_arrival(&spots, PortkeyKey::of(ObjectKind::Can)), None);
    }
}
