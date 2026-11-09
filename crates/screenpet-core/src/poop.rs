//! 💩 똥 (순수 계산): 왼쪽 버튼을 새로 누른 순간(눌렀다는 사실만 — 자리도 대상도 모른다)마다 화면 위 동물이 낮은 확률로 싼다.
//! 똥은 물건(object.rs)이다: 끌고 던지고 치우고 친구 화면으로 넘길 수 있다. 메뉴로 꺼내지는 못하고, 물건 수와 따로 센다.
//! 동물의 동작은 brain_poop.rs.
//!
//! 확률 계산: 부지런히 쓸 때 1분에 10번쯤 누른다고 보면 (쉬는 시간 뺀) 평균 대기는 1 / (0.04 × 10) = 2.5분,
//! 여기에 싼 뒤 쉬는 10~20분(평균 15분)을 더해 한 마리당 약 17.5분에 한 번이다. 1분에 3번이면 약 23분, 30번이면 약 16분.

use crate::object::ObjectKind;
use crate::surfaces::Owner;

/// 누를 때마다 쌀 확률
pub const CLICK_CHANCE: f64 = 0.04;
/// 한 번 싸고 다시 쌀 수 있을 때까지 (초)
pub const COOLDOWN_SECS: (f64, f64) = (600.0, 1200.0);
/// 저절로 사라질 때까지 (초)
pub const LIFE_SECS: f64 = 1800.0;
/// 걷다가 똥을 알아채는 앞쪽 거리 (몸 크기 배)
pub const LOOK_AHEAD: f64 = 0.9;
/// 이만큼(몸 크기 배) 앞까지 오면 밟았다
pub const STEP_REACH: f64 = 0.12;
/// 못 보고 밟아 미끄러질 확률 (나머지는 "ew!" 하고 피한다)
pub const SLIP_CHANCE: f64 = 0.2;
/// 피할 때 뛰어넘을 확률 (뛸 자리가 있을 때, 나머지는 돌아선다)
pub const HOP_OVER_CHANCE: f64 = 0.5;
/// 같은 똥에 다시 반응하기까지 (초)
pub const SAME_POOP_SECS: f64 = 90.0;
/// 어떤 똥이든 다시 반응하기까지 (초) — 똥이 모여 있어도 말풍선이 줄줄이 나오지 않는다
pub const CALM_SECS: f64 = 6.0;
/// 같은 디딤판으로 보는 높이 차 (px)
pub const SAME_LEVEL_PX: f64 = 4.0;
/// 뛰어넘을 때 똥 너머로 더 가는 거리 (몸 크기 배, 몸 반 폭에 더한다)
pub const HOP_CLEAR: f64 = 0.3;

/// 바닥에 놓인 똥 하나 (앱이 매 틱 Pet::notice_poops로 넘긴다)
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PoopSpot {
    pub id: u64,
    pub x: f64,
    pub y: f64,
    pub owner: Owner,
}

/// 앱 쪽 조건: 메뉴에서 켜 뒀고, 모두 숨겨 두지 않았고, 집중 중이 아니다 (손님도 집중을 방해하지 않는다).
/// 자리 비움·전체 화면은 동물이 있는 모니터마다 weather::overlay_paused로 본다
pub fn may_poop(off: bool, hidden: bool, focusing: bool) -> bool {
    !off && !hidden && !focusing
}

/// 똥 그림 크기 범위 (96 논리 px 칸에 곱한다), 친구가 크기를 안 보냈을 때
const SIZE_RANGE: (f64, f64) = (0.2, 0.9);
const SIZE_UNKNOWN: f64 = 0.5;
/// 몸 크기 배율에 곱한다: 몸의 4분의 1쯤 되는 똥
const SIZE_PER_BODY: f64 = 0.5;

/// 몸 크기 배율(단계 × 동물별, 일시 확대까지) → 똥 크기: 어린 동물일수록 작다
pub fn size_for(body_scale: f64) -> f64 {
    if body_scale.is_finite() {
        (body_scale * SIZE_PER_BODY).clamp(SIZE_RANGE.0, SIZE_RANGE.1)
    } else {
        SIZE_UNKNOWN
    }
}

/// 친구가 보낸 똥 크기를 다듬는다
pub fn sanitize_size(sent: Option<f64>) -> f64 {
    match sent {
        Some(s) if s.is_finite() => s.clamp(SIZE_RANGE.0, SIZE_RANGE.1),
        _ => SIZE_UNKNOWN,
    }
}

/// 동물 종류 → 똥 그림: 토끼는 알갱이, 나머지는 똥 (크기는 몸을 따른다)
pub fn kind_for(species: &str) -> ObjectKind {
    match species {
        "rabbit" => ObjectKind::Pellets,
        _ => ObjectKind::Poop,
    }
}

/// 똥 색 비율: 무지개 10%, 핑크 30%, 나머지 60%는 그 동물의 똥 (토끼는 알갱이)
pub const RAINBOW_CHANCE: f64 = 0.10;
pub const PINK_CHANCE: f64 = 0.30;

/// 싼 똥의 종류: roll = 0..1 난수
pub fn pick_kind(species: &str, roll: f64) -> ObjectKind {
    if roll < RAINBOW_CHANCE {
        ObjectKind::Rainbow
    } else if roll < RAINBOW_CHANCE + PINK_CHANCE {
        ObjectKind::Pink
    } else {
        kind_for(species)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn poop_size_follows_the_body_and_a_friend_cannot_send_a_giant() {
        // 고양이 아기 → 어른, 산타 어른
        let baby = size_for(0.45 * 0.91);
        let adult = size_for(1.15 * 0.91);
        assert!((baby - 0.2047).abs() < 1e-3 && (adult - 0.523).abs() < 1e-3, "{baby} {adult}");
        assert!(baby < size_for(0.78 * 0.91) && size_for(0.78 * 0.91) < adult);
        assert!((size_for(1.15 * 1.65) - 0.9).abs() < 1e-9);
        assert_eq!(size_for(f64::NAN), SIZE_UNKNOWN);
        for (sent, want) in [(None, SIZE_UNKNOWN), (Some(f64::INFINITY), SIZE_UNKNOWN), (Some(50.0), 0.9), (Some(0.001), 0.2), (Some(0.4), 0.4)] {
            assert_eq!(sanitize_size(sent), want, "{sent:?}");
        }
    }

    #[test]
    fn only_the_app_switches_decide_here() {
        assert!(may_poop(false, false, false));
        assert!(!may_poop(true, false, false), "turned off");
        assert!(!may_poop(false, true, false), "windows hidden");
        assert!(!may_poop(false, false, true), "focusing");
    }

    #[test]
    fn poops_are_sixty_brown_thirty_pink_ten_rainbow() {
        let mut n = std::collections::HashMap::new();
        for i in 0..1000 {
            *n.entry(pick_kind("cat", (i as f64 + 0.5) / 1000.0)).or_insert(0) += 1;
        }
        assert_eq!((n[&ObjectKind::Poop], n[&ObjectKind::Pink], n[&ObjectKind::Rainbow]), (600, 300, 100));
        // 토끼는 갈색 자리에 알갱이
        assert_eq!(pick_kind("rabbit", 0.99), ObjectKind::Pellets);
        assert_eq!(pick_kind("rabbit", 0.0), ObjectKind::Rainbow);
        assert_eq!(pick_kind("rabbit", 0.2), ObjectKind::Pink);
    }

    #[test]
    fn rabbits_get_pellets_little_pets_a_small_swirl_and_everyone_else_the_big_one() {
        assert_eq!(kind_for("rabbit"), ObjectKind::Pellets);
        // 곰·꿀벌·강아지는 v0.5.5에서 빠졌지만 예전 버전 친구가 보낸 똥도 보통 똥이 된다 (모르는 동물과 같다)
        for s in ["frog", "duck", "bird", "ghost", "cat", "giraffe", "dino_green", "santa", "bear", "bee", "dog", "unknown"] {
            assert_eq!(kind_for(s), ObjectKind::Poop, "{s}");
        }
        for k in [ObjectKind::Poop, ObjectKind::Pink, ObjectKind::Rainbow, ObjectKind::Pellets] {
            assert!(k.is_poop());
        }
    }
}
