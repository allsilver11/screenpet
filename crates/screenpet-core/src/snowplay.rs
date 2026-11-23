//! ⛄🍂 눈·낙엽 더미에서 놀기 (v0.6.5): 숫자와 순수 계산. 동물의 상태는 brain_snowplay.rs, 더미·눈가루·눈덩이 창은 src-tauri.
//!
//! - ⛄ 발밑에 눈이 쌓였으면 가끔 눈을 뭉쳐 던짐: 같은 화면의 다른 동물(눈싸움 — 맞은 동물이 되던짐), 커서, 친구 화면 너머(내 동물만), 아무 데나
//! - ⛄ 가끔 눈밭에서 데굴 → 눈가루
//! - 🍂🌸🌿 낙엽·꽃잎·푸른 잎이 쌓였으면 가끔 폴짝 뛰어들어 확 흩어짐
//! 눈이 쌓이면 자주 놂 (2026-11-23 — 어떤 캐릭터끼리든 눈싸움): 한 번 놀면 PLAY_EVERY_SECS 동안 쉼 (쉬는 박자가 끝날 때만 주사위를 굴림)

use crate::juggle::{self, Target};
use crate::weather::Weather;

/// 한 번 놀고 다음에 놀 수 있기까지 (초)
pub const PLAY_EVERY_SECS: (f64, f64) = (12.0, 35.0);
/// 놀 수 있을 때 쉬는 박자마다 놀 확률
pub const PLAY_CHANCE: f64 = 0.75;
/// 발밑 더미가 이만큼(평균 높이, 논리 px)은 있어야 놂
pub const MIN_DEPTH: f32 = 1.0;
/// 눈밭에서는 던지기가 이만큼, 나머지는 데굴
pub const THROW_SHARE: f64 = 0.85;
/// 눈을 뭉치는 동안 (기지개 그림) — RELEASE초에 던짐
pub const SCOOP_SECS: f64 = 1.0;
/// 뭉친 눈을 던지는 시각 (초)
pub const SCOOP_RELEASE: f64 = 0.7;
/// 눈을 떠 가는 폭: 발 가운데에서 좌우로 (몸 크기 배)
pub const SCOOP_REACH: f64 = 0.5;
/// 데굴 구르는 시간 (초)
pub const FLOP_SECS: f64 = 1.2;
/// 낙엽에 뛰어드는 폴짝은 평소 폴짝보다 이만큼 높이
pub const DIVE_LIFT: f64 = 1.5;
/// 눈덩이에 맞은 뒤 이 시간 안에 쉬면 되던짐 (초)
pub const REVENGE_SECS: f64 = 10.0;
/// 되던질 확률
pub const REVENGE_CHANCE: f64 = 0.95;
/// 커서를 노릴 확률 (닿는 곳에 있을 때)
pub const CURSOR_CHANCE: f64 = 0.2;
/// 노릴 수 있는 커서: 가로 (몸 크기 배)
pub const CURSOR_RANGE: f64 = 8.0;
/// 노릴 수 있는 커서: 발에서 위로 (몸 크기 배)
pub const CURSOR_RISE: f64 = 6.0;
/// 친구 화면 너머로 던질 확률 (닿는 끝이 있을 때, 다른 동물·커서를 노리지 않으면)
pub const LOB_CHANCE: f64 = 0.35;
/// 아무 데나 던질 때 (논리 px/s): 가로, 위로
pub const TOSS_VX: (f64, f64) = (200.0, 420.0);
/// 아무 데나 던질 때 위로 (논리 px/s)
pub const TOSS_VY: (f64, f64) = (420.0, 620.0);

/// 더미에서 할 놀이
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Play {
    /// ⛄ 눈을 뭉쳐 던짐
    Throw,
    /// ⛄ 눈밭에서 데굴
    Flop,
    /// 🍂🌸🌿 낙엽·꽃잎·푸른 잎에 뛰어듦
    Dive,
}

/// 던질 곳
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Aim {
    /// 같은 화면의 다른 동물
    Pet(Target),
    /// 커서 (물리 px)
    Cursor(f64, f64),
    /// 친구가 연결된 화면 끝 x 너머로
    Lob(f64),
    /// 아무 데나 (쪽 ±1)
    Toss(f64),
}

/// 앱에 알릴 일 (take_snow_play로 한 번 꺼냄)
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SnowPlayEvent {
    /// 발밑 (foot_x, foot_y) 둘레 눈을 떠서 (x, y)에서 (vx, vy)로 던졌음 (물리 px, px/s) — 앱이 그 눈을 비우고 눈덩이를 만들어 날림
    Throw { foot_x: f64, foot_y: f64, x: f64, y: f64, vx: f64, vy: f64 },
    /// (x, 발 y)의 더미에 뛰어들었거나 뒹굴었음 — 앱이 그 둘레 더미를 비우고 알갱이를 위로 확 날림
    Burst { x: f64, y: f64 },
}

/// 발밑 더미로 할 놀이를 고름: 눈이면 던지기·데굴, 낙엽·꽃잎·푸른 잎이면 뛰어들기. 더미가 얕거나 비면 None
pub fn pick_play(rng: &mut fastrand::Rng, kind: Weather, depth: f32) -> Option<Play> {
    if !(depth >= MIN_DEPTH) {
        return None;
    }
    if kind.is_snow() {
        Some(if rng.f64() < THROW_SHARE { Play::Throw } else { Play::Flop })
    } else if matches!(kind, Weather::Leaves | Weather::Petals | Weather::GreenLeaves) {
        Some(Play::Dive)
    } else {
        None
    }
}

/// 커서가 노릴 수 있는 곳에 있음: from = 발 (물리 px), size = 몸 한 변
pub fn cursor_in_reach(from: (f64, f64), size: f64, cursor: (f64, f64)) -> bool {
    let (dx, dy) = (cursor.0 - from.0, from.1 - cursor.1);
    dx.abs() <= CURSOR_RANGE * size && dx.abs() > 1.0 && (-size..=CURSOR_RISE * size).contains(&dy)
}

/// 🎯 fx에서 edge_x 너머로 던져 닿음 (juggle::lob_velocity가 상한에 걸리지 않음)
pub fn lob_reaches(fx: f64, edge_x: f64, dpi: f64) -> bool {
    let g = crate::physics::THROW_GRAVITY_LOGICAL * dpi;
    let air = 2.0 * juggle::LOB_VY * dpi / g;
    (edge_x - fx).abs() * 1.3 / air <= crate::physics::MAX_THROW_SPEED_LOGICAL * dpi * 0.9
}

/// 던질 곳을 고름. from = 손 자리 (물리 px), size = 몸 한 변, targets = 같은 화면의 다른 동물, cursor = 노릴 수 있는 커서,
/// edges = 넘겨 던질 수 있는 친구 쪽 끝 x들 (닿는 것만 넘김), revenge = 눈덩이에 맞아 되던짐 (가장 가까운 동물을 꼭 노림)
pub fn pick_aim(rng: &mut fastrand::Rng, from: (f64, f64), size: f64, targets: &[Target], cursor: Option<(f64, f64)>, edges: &[f64], revenge: bool) -> Aim {
    let pet = juggle::nearest_target(from, size, targets);
    if let Some(t) = pet.filter(|_| revenge) {
        return Aim::Pet(t);
    }
    if let Some((cx, cy)) = cursor.filter(|_| rng.f64() < CURSOR_CHANCE) {
        return Aim::Cursor(cx, cy);
    }
    if let Some(t) = pet {
        return Aim::Pet(t);
    }
    if !edges.is_empty() && rng.f64() < LOB_CHANCE {
        return Aim::Lob(edges[rng.usize(..edges.len())]);
    }
    Aim::Toss(if rng.bool() { 1.0 } else { -1.0 })
}

/// 던지는 속도 (물리 px/s): 동물은 몸 가운데로, 커서는 그 점으로, 끝 너머는 높이 솟게, 아무 데나는 위로
pub fn aim_velocity(rng: &mut fastrand::Rng, aim: Aim, from: (f64, f64), size: f64, dpi: f64) -> (f64, f64) {
    match aim {
        Aim::Pet(t) => juggle::aim_at(from, t, true, 0.0, size, dpi),
        Aim::Cursor(x, y) => juggle::aim_at(from, Target { x, y, size: 0.0 }, true, 0.0, size, dpi),
        Aim::Lob(edge_x) => juggle::lob_velocity(from.0, edge_x, dpi),
        Aim::Toss(dir) => {
            let r = |(lo, hi): (f64, f64), rng: &mut fastrand::Rng| lo + rng.f64() * (hi - lo);
            (dir * r(TOSS_VX, rng) * dpi, -r(TOSS_VY, rng) * dpi)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::physics::THROW_GRAVITY_LOGICAL;

    #[test]
    fn snow_is_for_throwing_or_flopping_and_leaves_for_diving() {
        let mut rng = fastrand::Rng::with_seed(1);
        let mut seen = [0; 2];
        for _ in 0..200 {
            match pick_play(&mut rng, Weather::Snow, 3.0) {
                Some(Play::Throw) => seen[0] += 1,
                Some(Play::Flop) => seen[1] += 1,
                other => panic!("{other:?}"),
            }
        }
        assert!(seen[0] > seen[1] && seen[1] > 20, "{seen:?}");
        assert_eq!(pick_play(&mut rng, Weather::Leaves, 3.0), Some(Play::Dive));
        assert_eq!(pick_play(&mut rng, Weather::LightSnow, 0.5), None, "too thin");
        assert_eq!(pick_play(&mut rng, Weather::Petals, 3.0), Some(Play::Dive), "petals are dived into like leaves");
        assert_eq!(pick_play(&mut rng, Weather::Petals, 0.5), None, "too thin");
        assert_eq!(pick_play(&mut rng, Weather::GreenLeaves, 3.0), Some(Play::Dive), "green leaves too");
        assert_eq!(pick_play(&mut rng, Weather::Rain, 3.0), None);
        assert_eq!(pick_play(&mut rng, Weather::Snow, f32::NAN), None);
    }

    #[test]
    fn a_hit_pet_always_aims_back_at_the_nearest_one() {
        let mut rng = fastrand::Rng::with_seed(2);
        let t = Target { x: 700.0, y: 1000.0, size: 100.0 };
        for _ in 0..50 {
            assert_eq!(pick_aim(&mut rng, (500.0, 940.0), 100.0, &[t], Some((520.0, 600.0)), &[1920.0], true), Aim::Pet(t));
        }
    }

    #[test]
    fn otherwise_the_cursor_a_pet_a_friend_edge_or_anywhere() {
        let mut rng = fastrand::Rng::with_seed(3);
        let t = Target { x: 700.0, y: 1000.0, size: 100.0 };
        let mut cursor = 0;
        for _ in 0..200 {
            match pick_aim(&mut rng, (500.0, 940.0), 100.0, &[t], Some((520.0, 600.0)), &[], false) {
                Aim::Cursor(..) => cursor += 1,
                Aim::Pet(p) => assert_eq!(p, t),
                other => panic!("{other:?}"),
            }
        }
        assert!((20..=70).contains(&cursor), "{cursor}");
        let mut lobs = 0;
        for _ in 0..200 {
            match pick_aim(&mut rng, (500.0, 940.0), 100.0, &[], None, &[1920.0], false) {
                Aim::Lob(x) => {
                    assert_eq!(x, 1920.0);
                    lobs += 1
                }
                Aim::Toss(d) => assert!(d.abs() == 1.0),
                other => panic!("{other:?}"),
            }
        }
        assert!((30..=120).contains(&lobs), "{lobs}");
    }

    #[test]
    fn the_cursor_counts_only_near_and_above() {
        let from = (500.0, 1000.0);
        assert!(cursor_in_reach(from, 100.0, (700.0, 700.0)));
        assert!(!cursor_in_reach(from, 100.0, (1400.0, 700.0)), "too far");
        assert!(!cursor_in_reach(from, 100.0, (700.0, 200.0)), "too high");
        assert!(!cursor_in_reach(from, 100.0, (700.0, 1200.0)), "below the floor");
    }

    #[test]
    fn throws_reach_what_they_aim_at() {
        let mut rng = fastrand::Rng::with_seed(4);
        let from = (500.0, 900.0);
        let g = THROW_GRAVITY_LOGICAL;
        let (vx, vy) = aim_velocity(&mut rng, Aim::Cursor(800.0, 600.0), from, 100.0, 1.0);
        let t = (800.0 - from.0) / vx;
        assert!((from.1 + vy * t + 0.5 * g * t * t - 600.0).abs() < 1.0);
        let (vx, vy) = aim_velocity(&mut rng, Aim::Lob(1920.0), from, 100.0, 1.0);
        assert!(vx > 0.0 && vy < 0.0);
        assert!(lob_reaches(1500.0, 1920.0, 1.0));
        assert!(!lob_reaches(0.0, 1920.0 * 4.0, 1.0), "too far for one throw");
        let (vx, vy) = aim_velocity(&mut rng, Aim::Toss(-1.0), from, 100.0, 2.0);
        assert!(vx <= -TOSS_VX.0 * 2.0 && vy <= -TOSS_VY.0 * 2.0);
    }
}
