//! 다른 동물들도 더 놀기 (v0.6.3): 숫자와 앱에 알릴 물건 일. 동물의 상태는 brain_knock.rs · brain_stash.rs · brain_quake.rs ·
//! brain_highfive.rs, 여러 동물을 맞추는 일은 social_play.rs · social.rs (짝!).
//!
//! - 🐱 고양이 툭툭 (pet.json `knocker` + paw — 고양이): 창 윗면에 놓인 작은 물건을 보면 가끔 다가가 쳐다보고 툭… 툭… 하다가
//!   가까운 끝으로 밀어 떨어뜨리고, 끝에서 내려다봄
//! - 🦖 공룡 쿵! (공룡의 쿵쾅 Special::Stomp): 쉬는 때가 지났으면 큰 쿵! — 같은 바닥의 물건이 톡 튀어 오르고 친구들이 깜짝 폴짝.
//! - 🐿️ 다람쥐 볼 빵빵 (pet.json `stasher` + cheeks_walk·cheeks_idle·dig — 다람쥐): 바닥의 작은 물건을 볼에 넣고(뿅) 한 번에 셋까지,
//!   그다음 조금 걸어가 땅에 묻음
//! - 🙌 하이파이브 (모든 동물): 마주 오다 만나면 가끔 멈춰 마주 보고 폴짝 "High five!" + 반짝.
//!
//! 주사위는 동물마다 자기 놀이 주사위(PlayMood.rng)로 — 평소 동작과 다른 동물의 주사위 순서는 그대로.

use crate::surfaces::Owner;

// ---- 🐱 고양이 툭툭 ----

/// 처음 알아챈 물건마다 툭툭하러 갈 확률
pub const KNOCK_CHANCE: f64 = 0.4;
/// 한 번 떨어뜨린 뒤 다시 하기까지 (초)
pub const KNOCK_CALM_SECS: (f64, f64) = (120.0, 240.0);
/// 알아채는 거리 (몸 크기 배, 같은 창 윗면)
pub const KNOCK_REACH: f64 = 4.0;
/// 같은 물건은 이만큼 다시 주사위를 굴리지 않음 (초)
pub const SAME_OBJECT_SECS: f64 = 45.0;
/// 다가가다 이만큼 못 닿으면 그만둠 (초)
pub const APPROACH_SECS: f64 = 6.0;
/// 물건 가장자리에서 몸 가운데까지 (몸 크기 배) — 앞발(paw 그림)이 여기서 물건에 닿음
pub const PAW_GAP: f64 = 0.3;
/// 물건을 쳐다보는 시간
pub const LOOK_SECS: f64 = 0.8;
/// 툭 한 번 (paw 그림 4칸 8fps 한 번)
pub const TAP_SECS: f64 = 0.5;
/// 툭 하고 미는 순간 (paw 셋째 칸)
pub const TAP_AT: f64 = 0.25;
/// 떨어뜨리기 전 툭툭 수
pub const TAPS: (u32, u32) = (2, 3);
/// 그냥 툭 — 물건이 조금 밀림 (몸 크기 배)
pub const TAP_REACH: f64 = 0.06;
/// 마지막 밀기: 끝까지의 거리 + 이만큼 (몸 크기 배) 더 밀려 끝을 넘음
pub const PUSH_PAST: f64 = 0.2;
/// 밀고 난 뒤 끝 쪽으로 걸어가 내려다보는 시간 (걸어가기 최대, 내려다보기)
pub const WATCH_WALK_SECS: f64 = 3.0;
/// 끝 쪽을 내려다보는 시간 (초)
pub const WATCH_SECS: f64 = 1.6;

// ---- 깜짝 폴짝 ----

/// 제자리 깜짝 폴짝 시간 (초)
pub const BOUNCE_SECS: f64 = 0.45;

// ---- 🦖 공룡 쿵! ----

/// 큰 쿵! 사이 쉬는 때 (초) — 그 사이의 쿵쾅은 예전처럼 흔들기만
pub const QUAKE_CALM_SECS: (f64, f64) = (180.0, 360.0);
/// 큰 쿵이 닿는 거리 (두 동물 크기 평균 배 — 물건은 공룡 크기 배, 같은 바닥)
pub const QUAKE_BODIES: f64 = 6.0;
/// 물건이 톡 튀어 오르는 속도 (논리 px/s, 위로)
pub const QUAKE_POP_VY: f64 = 260.0;
/// 깜짝 폴짝 높이 (몸 크기 배)
pub const QUAKE_HOP_RISE: f64 = 0.2;

// ---- 🐿️ 다람쥐 볼 빵빵 ----

/// 처음 알아챈 물건마다 볼에 넣으러 갈 확률
pub const STASH_CHANCE: f64 = 0.5;
/// 한 번 묻고 난 뒤 다시 하기까지 (초)
pub const STASH_CALM_SECS: (f64, f64) = (150.0, 300.0);
/// 알아채는 거리 (몸 크기 배, 같은 디딤판)
pub const STASH_REACH: f64 = 5.0;
/// 한 번에 볼에 넣는 물건 수 상한
pub const MAX_CHEEKS: u8 = 3;
/// 볼에 넣는 시간 (그 가운데에 물건이 뿅 사라짐)
pub const STUFF_SECS: f64 = 0.9;
/// 볼에 넣는 시간 중 물건이 사라지는 시각 (초)
pub const GULP_AT: f64 = 0.35;
/// 묻으러 걸어가는 시간, 땅 파서 묻는 시간 (dig 4칸 8fps 네 번)
pub const BURY_WALK_SECS: (f64, f64) = (1.5, 3.0);
/// 땅 파서 묻는 시간 (초)
pub const DIG_SECS: f64 = 2.0;

// ---- 🙌 하이파이브 ----

/// 마주 오다 만난 짝이 하이파이브할 확률
pub const HIGH_FIVE_CHANCE: f64 = 0.35;
/// 같은 짝이 다시 하이파이브하기까지 (초)
pub const HIGH_FIVE_COOLDOWN: (f64, f64) = (60.0, 150.0);
/// 서로 다가가는 시간 상한 (그 뒤엔 그 자리에서 뜀)
pub const FIVE_CLOSE_SECS: f64 = 1.5;
/// 마주 보고 잠깐 (초), 폴짝 (초), 내려서서 잠깐 (초)
pub const FIVE_LOOK_SECS: f64 = 0.3;
/// 하이파이브 폴짝 시간 (초)
pub const FIVE_JUMP_SECS: f64 = 0.6;
/// 내려서서 잠깐 (초)
pub const FIVE_LAND_SECS: f64 = 0.35;
/// 폴짝 높이 (몸 크기 배)
pub const FIVE_RISE: f64 = 0.28;
/// 짝! 반짝이 (폴짝 가운데 이 길이만큼)
pub const CLAP_SECS: f64 = 0.25;
/// 맞댄 손: 두 몸 반 폭 합의 이만큼 떨어져 섬
pub const FIVE_GAP: f64 = 0.95;

/// 앱이 물건에 할 일 (Pet::take_object_play로 한 번 꺼냄)
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ObjectPlay {
    /// 🐱 바닥의 물건을 dir 쪽으로 툭 밂: 바닥을 따라 reach(물리 px)만큼 미끄러지고, 끝을 넘으면 떨어짐 (Object::nudge)
    Nudge { id: u64, dir: f64, reach: f64 },
    /// 🐿️ 볼에 넣었음: 앱이 이 물건을 뿅 치움
    Gulp { id: u64 },
    /// 🦖 큰 쿵!: 같은 바닥(owner, 높이 y) 가로 reach(물리 px) 안의 물건이 톡 튀어 오름 (Object::pop)
    Quake { x: f64, y: f64, owner: Owner, reach: f64 },
    /// 🕷️ 거미인간이 줄로 끌어옴: 바닥에 있는 이 물건을 (vx, vy)로 날림 (물리 px/s, Object::launch — brain_web.rs)
    /// ✨ Expelliarmus도 바닥의 물건을 이 길로 휙 날림 (brain_spell.rs)
    Yank { id: u64, vx: f64, vy: f64 },
    /// ✨ Wingardium Leviosa: 바닥에 있는 이 물건이 rise (물리 px)만큼 둥둥 떠올라 secs초 뒤 내려옴 (Object::levitate — brain_spell.rs)
    Levitate { id: u64, rise: f64, secs: f64 },
    /// ✨ Reparo·👜 구슬 가방: 바닥에 있는 이 물건을 뿅 치움 — 거대한 것도 (📌 장난감은 안 치움, brain_spell.rs·brain_study.rs)
    Vanish { id: u64 },
    /// 🧔 🪨 해그리드의 락 케이크: (x, y)에 이 물건을 새로 놓음 (물리 px — 앱이 물건 수 제한을 지키며 만듦, brain_hagrid.rs)
    Spawn { kind: crate::object::ObjectKind, x: f64, y: f64 },
}

/// 위로 rise만큼 솟았다 내려오는 폴짝의 t초째 높이 (secs 동안, 포물선)
pub fn hop_height(t: f64, secs: f64, rise: f64) -> f64 {
    if !(0.0..=secs).contains(&t) || secs <= 0.0 {
        return 0.0;
    }
    let u = t / secs;
    4.0 * rise * u * (1.0 - u)
}

/// 🐱 물건(obj_x)을 밀어 떨어뜨릴 쪽: 디딤판 조각 [lo, hi]의 가까운 끝 (+1 오른쪽, -1 왼쪽)
pub fn push_dir(obj_x: f64, lo: f64, hi: f64) -> f64 {
    if hi - obj_x <= obj_x - lo {
        1.0
    } else {
        -1.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hop_rises_to_the_top_halfway_and_lands() {
        assert_eq!(hop_height(0.0, 1.0, 10.0), 0.0);
        assert!((hop_height(0.5, 1.0, 10.0) - 10.0).abs() < 1e-9);
        assert_eq!(hop_height(1.0, 1.0, 10.0), 0.0);
        assert_eq!(hop_height(1.5, 1.0, 10.0), 0.0);
    }

    #[test]
    fn it_pushes_toward_the_nearer_end() {
        assert_eq!(push_dir(900.0, 500.0, 1100.0), 1.0);
        assert_eq!(push_dir(600.0, 500.0, 1100.0), -1.0);
    }
}
