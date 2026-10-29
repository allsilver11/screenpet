//! 다음 행동 고르기 — 이동 방식, 쉬기/애교, 클릭 반응. 확률·시간 수치는 모두 여기에 둔다.

pub const IDLE_SECS: (f64, f64) = (1.5, 3.0);
pub const SLEEP_SECS: (f64, f64) = (6.0, 12.0);
pub const SLEEP_CHANCE: f64 = 0.08;
pub const TRICK_SECS: (f64, f64) = (2.0, 4.0);
pub const TRICK_CHANCE: f64 = 0.3;
pub const MOVE_END_TURN_CHANCE: f64 = 0.3;
pub const WALK_EDGE_HOP_CHANCE: f64 = 0.8;
pub const RUN_LEAP_VX_MULT: f64 = 1.8;
pub const CLICK_TRICK_SECS: f64 = 2.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Move {
    Walk,
    Run,
    Roll,
}

impl Move {
    pub fn speed_mult(self) -> f64 {
        match self {
            Move::Walk => 1.0,
            Move::Run => 2.5,
            Move::Roll => 2.0,
        }
    }

    pub fn secs(self) -> (f64, f64) {
        match self {
            Move::Walk => (4.0, 10.0),
            Move::Run => (2.0, 5.0),
            Move::Roll => (2.0, 4.0),
        }
    }
}

/// 앞모습 애교 동작
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Trick {
    SitFront,
    Wave,
    Tilt,
    Belly,
    Stretch,
    Heart,
}

pub const TRICKS: [Trick; 6] = [Trick::SitFront, Trick::Wave, Trick::Tilt, Trick::Belly, Trick::Stretch, Trick::Heart];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Rest {
    Idle,
    Trick(Trick),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClickReaction {
    /// v1의 하트 + 제자리 점프
    Hop,
    Trick(Trick),
}

pub fn pick_move(rng: &mut fastrand::Rng) -> Move {
    let r = rng.f64();
    if r < 0.55 {
        Move::Walk
    } else if r < 0.85 {
        Move::Run
    } else {
        Move::Roll
    }
}

pub fn pick_rest(rng: &mut fastrand::Rng) -> Rest {
    if rng.f64() < TRICK_CHANCE {
        Rest::Trick(pick_trick(rng))
    } else {
        Rest::Idle
    }
}

pub fn pick_trick(rng: &mut fastrand::Rng) -> Trick {
    TRICKS[rng.usize(..TRICKS.len())]
}

pub fn pick_click(rng: &mut fastrand::Rng) -> ClickReaction {
    match rng.usize(..4) {
        0 => ClickReaction::Hop,
        1 => ClickReaction::Trick(Trick::Heart),
        2 => ClickReaction::Trick(Trick::Wave),
        _ => ClickReaction::Trick(Trick::Tilt),
    }
}

pub fn range(rng: &mut fastrand::Rng, (lo, hi): (f64, f64)) -> f64 {
    lo + rng.f64() * (hi - lo)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn share<T: PartialEq>(n: usize, mut f: impl FnMut(&mut fastrand::Rng) -> T, want: T) -> f64 {
        let mut rng = fastrand::Rng::with_seed(7);
        (0..n).filter(|_| f(&mut rng) == want).count() as f64 / n as f64
    }

    #[test]
    fn move_mix_is_55_30_15() {
        assert!((0.52..=0.58).contains(&share(20_000, pick_move, Move::Walk)));
        assert!((0.27..=0.33).contains(&share(20_000, pick_move, Move::Run)));
        assert!((0.13..=0.17).contains(&share(20_000, pick_move, Move::Roll)));
    }

    #[test]
    fn rest_is_trick_30_percent() {
        let mut rng = fastrand::Rng::with_seed(3);
        let n = 20_000;
        let tricks = (0..n).filter(|_| matches!(pick_rest(&mut rng), Rest::Trick(_))).count() as f64 / n as f64;
        assert!((0.27..=0.33).contains(&tricks), "{tricks}");
    }

    #[test]
    fn every_trick_and_click_reaction_appears() {
        let mut rng = fastrand::Rng::with_seed(5);
        for t in TRICKS {
            assert!((0..600).any(|_| pick_trick(&mut rng) == t), "{t:?}");
        }
        let mut seen = [false; 4];
        for _ in 0..400 {
            let i = match pick_click(&mut rng) {
                ClickReaction::Hop => 0,
                ClickReaction::Trick(Trick::Heart) => 1,
                ClickReaction::Trick(Trick::Wave) => 2,
                ClickReaction::Trick(Trick::Tilt) => 3,
                ClickReaction::Trick(t) => panic!("click gave {t:?}"),
            };
            seen[i] = true;
        }
        assert_eq!(seen, [true; 4]);
    }

    #[test]
    fn move_speeds_and_durations() {
        assert_eq!((Move::Walk.speed_mult(), Move::Run.speed_mult(), Move::Roll.speed_mult()), (1.0, 2.5, 2.0));
        assert_eq!((Move::Walk.secs(), Move::Run.secs(), Move::Roll.secs()), ((4.0, 10.0), (2.0, 5.0), (2.0, 4.0)));
    }

    #[test]
    fn range_stays_inside() {
        let mut rng = fastrand::Rng::with_seed(1);
        assert!((0..1000).all(|_| (1.5..=3.0).contains(&range(&mut rng, (1.5, 3.0)))));
    }
}
