//! 날다가 부딪히기 (순수 계산): 던진 동물·물건이 다른 동물에 닿는 순간과 되튀는 속도.
//! 누가 부딪힐 수 있는지와 쿨다운은 social.rs, 맞은 동물의 반응은 brain_hit.rs.
//!
//! - 닿는 순간만 본다: 지난 틱엔 밖이고 이번 틱에 안이거나 지나쳤을 때. 이미 겹친 채 놓으면 (살며시 내려놓기) 빠져나갈 때까지 아무 일도 없다.
//! - 속도는 논리 px/s로 잰다 (화면 배율과 상관없이 같은 느낌).

/// 이보다 느리면(논리 px/s) 닿아도 아무 일도 없다 — 던진 중력으로 몸 절반쯤(47 px) 떨어지면 나오는 빠르기
pub const HIT_MIN_SPEED: f64 = 400.0;
/// 이보다 빠르면(논리 px/s) 맞은 동물이 데굴 넘어진다 — 아니면 움찔. 던진 속도 상한은 1600
pub const HIT_FAST_SPEED: f64 = 900.0;
/// 같은 짝이 다시 부딪히기까지 (초)
pub const HIT_PAIR_COOLDOWN: f64 = 1.5;
/// 맞은 동물이 다시 맞기까지 (초)
pub const HIT_CALM_SECS: f64 = 1.0;
/// 옆에서 부딪혀 되튈 때: 속도의 이만큼으로 맞은 쪽 반대로, 세로는 남은 만큼에 살짝 띄운다
const SIDE_BACK: f64 = 0.45;
const SIDE_KEEP_VY: f64 = 0.3;
const SIDE_POP: f64 = 0.15;
/// 머리에서 튀어 오를 때 속도의 이만큼으로 위로, 배에 부딪히면 이만큼으로 아래로
const HEAD_UP: f64 = 0.55;
const BELLY_DOWN: f64 = 0.35;
/// 위아래로 튈 때 남는 가로 속도, 머리에 다시 떨어지지 않게 옆으로 미는 속도 (논리 px/s)
const KEEP_VX: f64 = 0.4;
const SLIDE_OFF: f64 = 120.0;

/// 맞은 몸의 어느 쪽으로 들어왔나
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Face {
    /// 머리 위에서 내려왔다
    Top,
    Side,
    /// 아래에서 올라왔다
    Bottom,
}

/// 맞은 동물에게 알리는 것
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Hit {
    pub face: Face,
    /// 부딪힌 것의 속도 (물리 px/s)
    pub vx: f64,
    pub vy: f64,
    /// 맞은 동물이 밀려나는 쪽 (±1)
    pub dir: f64,
    /// HIT_FAST_SPEED보다 빠르다
    pub fast: bool,
    /// 똥에 맞았다
    pub poop: bool,
}

/// 움직이는 몸이 다른 몸에 이번 틱에 처음 닿았으면 들어온 쪽.
/// prev·now는 맞을 몸에 대한 상대 자리 (발 기준), half_x = 두 몸 반 폭의 합, above = 맞을 몸 높이, below = 움직이는 몸 높이.
/// 빠르거나 틱이 길어 한 틱에 뚫고 지나가도 잡는다
pub fn entry(prev: (f64, f64), now: (f64, f64), half_x: f64, above: f64, below: f64) -> Option<Face> {
    let inside = |p: (f64, f64)| p.0.abs() < half_x && -above < p.1 && p.1 < below;
    if inside(prev) {
        return None;
    }
    let (dx, dy) = (now.0 - prev.0, now.1 - prev.1);
    let slab = |from: f64, d: f64, lo: f64, hi: f64| -> Option<(f64, f64)> {
        if d == 0.0 {
            return (lo < from && from < hi).then_some((f64::NEG_INFINITY, f64::INFINITY));
        }
        let (a, b) = ((lo - from) / d, (hi - from) / d);
        Some((a.min(b), a.max(b)))
    };
    let (x_in, x_out) = slab(prev.0, dx, -half_x, half_x)?;
    let (y_in, y_out) = slab(prev.1, dy, -above, below)?;
    let (t_in, t_out) = (x_in.max(y_in), x_out.min(y_out));
    if t_in >= t_out || t_in > 1.0 || t_out < 0.0 {
        return None;
    }
    Some(if x_in >= y_in {
        Face::Side
    } else if dy > 0.0 {
        Face::Top
    } else {
        Face::Bottom
    })
}

/// 부딪힌 뒤의 속도 (물리 px/s): 언제나 전보다 느리고, 빠를수록 세게 튄다. away = 맞은 몸에서 멀어지는 쪽 (±1)
pub fn rebound(face: Face, vx: f64, vy: f64, away: f64, dpi: f64) -> (f64, f64) {
    let speed = vx.hypot(vy);
    match face {
        Face::Side => (away * speed * SIDE_BACK, vy * SIDE_KEEP_VY - speed * SIDE_POP),
        Face::Top => (vx * KEEP_VX + away * SLIDE_OFF * dpi, -speed * HEAD_UP),
        Face::Bottom => (vx * KEEP_VX + away * SLIDE_OFF * dpi, speed * BELLY_DOWN),
    }
}

/// 몸에서 멀어지는 쪽: 맞은 몸에 대한 상대 x로 정하고, 거의 한가운데면 오던 쪽으로 되돌아간다 (위아래로 튈 땐 가던 쪽으로 미끄러진다)
pub fn away_from(rel_x: f64, vx: f64, face: Face) -> f64 {
    if rel_x.abs() > 1.0 {
        return rel_x.signum();
    }
    let along = if vx != 0.0 { vx.signum() } else { 1.0 };
    if face == Face::Side {
        -along
    } else {
        along
    }
}

/// 논리 px/s 빠르기
pub fn logical_speed(vx: f64, vy: f64, dpi: f64) -> f64 {
    vx.hypot(vy) / dpi.max(0.01)
}

#[cfg(test)]
mod tests {
    use super::*;

    const HALF: f64 = 76.0;
    const ABOVE: f64 = 102.0;
    const BELOW: f64 = 102.0;

    #[test]
    fn a_hit_counts_only_on_entering() {
        assert_eq!(entry((-90.0, 0.0), (-70.0, 0.0), HALF, ABOVE, BELOW), Some(Face::Side));
        assert_eq!(entry((90.0, 0.0), (70.0, 0.0), HALF, ABOVE, BELOW), Some(Face::Side));
        assert_eq!(entry((0.0, -110.0), (0.0, -95.0), HALF, ABOVE, BELOW), Some(Face::Top));
        assert_eq!(entry((0.0, 110.0), (0.0, 95.0), HALF, ABOVE, BELOW), Some(Face::Bottom));
        // 이미 겹쳐 있거나, 아직 안 닿았거나, 비껴갔다
        assert_eq!(entry((-70.0, 0.0), (-50.0, 0.0), HALF, ABOVE, BELOW), None);
        assert_eq!(entry((-120.0, 0.0), (-100.0, 0.0), HALF, ABOVE, BELOW), None);
        assert_eq!(entry((-90.0, -110.0), (-80.0, -105.0), HALF, ABOVE, BELOW), None);
        assert_eq!(entry((-100.0, -120.0), (100.0, -120.0), HALF, ABOVE, BELOW), None);
    }

    #[test]
    fn one_long_tick_does_not_tunnel_through() {
        assert_eq!(entry((-200.0, 0.0), (200.0, 0.0), HALF, ABOVE, BELOW), Some(Face::Side));
        assert_eq!(entry((0.0, -300.0), (0.0, 300.0), HALF, ABOVE, BELOW), Some(Face::Top));
    }

    #[test]
    fn a_rebound_is_always_slower_and_grows_with_speed() {
        for dpi in [1.0, 2.0] {
            for face in [Face::Side, Face::Top, Face::Bottom] {
                for (vx, vy) in [(1.0, 0.0), (0.0, 1.0), (0.0, -1.0), (0.7, 0.7), (-0.7, -0.7)] {
                    let mut last = 0.0;
                    for speed in [HIT_MIN_SPEED, 900.0, 1600.0] {
                        let (rx, ry) = rebound(face, vx * speed * dpi, vy * speed * dpi, 1.0, dpi);
                        let after = rx.hypot(ry);
                        assert!(after < speed * dpi, "{face:?} {vx} {vy} {speed}: {after}");
                        assert!(after > last);
                        last = after;
                    }
                }
            }
        }
        assert!(rebound(Face::Side, -1000.0, 0.0, 1.0, 1.0).0 > 0.0, "away from the one it hit");
        assert!(rebound(Face::Top, 0.0, 1000.0, 1.0, 1.0).1 < 0.0, "up off the head");
        assert!(rebound(Face::Bottom, 0.0, -1000.0, 1.0, 1.0).1 > 0.0);
    }

    #[test]
    fn away_is_the_side_it_came_from() {
        assert_eq!(away_from(-30.0, 500.0, Face::Side), -1.0);
        assert_eq!(away_from(30.0, 500.0, Face::Side), 1.0);
        assert_eq!(away_from(0.0, 500.0, Face::Side), -1.0);
        assert_eq!(away_from(0.0, 500.0, Face::Top), 1.0);
    }
}
