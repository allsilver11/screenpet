//! 🧹 빗자루 비행 (해리포터·헤르미온느 — 볼드모트도 같은 코드)의 숫자와 길·🟡 골든 스니치. 동작은 brain_broom.rs.
//! (메뉴로 커서를 따라다니는 사용자 빗자루는 broom.rs — 이름만 비슷함)
//!
//! - 한가할 때 BROOM_FIRST_SECS 뒤부터 초당 BROOM_RATE (🌱 power)로 빗자루에 올라 (MOUNT_SECS) 화면을 날아다님:
//!   허공 점을 LEGS번 이어 날고 (낮은 호 LOW_ARC — 몸 크기 배, 높은 호 HIGH_ARC — 화면 높이 몫) 마지막엔 아무 디딤판에 내려앉음.
//!   다음 비행은 BROOM_CALM_SECS 뒤. 빠르기 BROOM_SPEED (걷기 배 — 🌱 soft_power)
//! - 친구가 연결된 옆 끝이 있으면 마지막 호를 그 끝 너머로 날아 나가기도 함 (visit::exit_chance만큼 — 집이 그리운 손님은 늘)
//! - 친구 화면을 걸어서 나가고 들어올 때도 빗자루 (TRANSIT_SPEED, 창을 TRANSIT_LIFT만큼 띄움)
//! - 비행이 끊기면 (잡기 말고 — 볼일·자리 비움 등) 빗자루째 천천히 내려옴 (DESCENT_GRAVITY), 내려앉기는 늘 사뿐 (LAND_SECS)
//! - 🟡 골든 스니치 (seeker — 해리): SNITCH_FIRST_SECS 뒤부터 초당 SNITCH_RATE로 화면 위쪽에 작은 금빛 공이 나타남 → 잠깐 올려다보고 (SPOT_SECS)
//!   빗자루로 쫓음 (CHASE_SPEED). 스니치는 SNITCH_SPEED로 이리저리 (JINK_SECS마다 새 목표, 가까우면 DODGE_REACH로 피함),
//!   TIRE_SECS 뒤엔 지쳐 느려짐 (TIRED_PACE) — CATCH_REACH 안에 들면 "Got it!" (CAUGHT_SECS 떠서 쥐고) 내려앉음. GIVE_UP_SECS면 사라짐

use crate::geom::Rect;

/// 처음 빗자루를 탈 수 있게 되는 때 (clock 초)
pub const BROOM_FIRST_SECS: f64 = 40.0;
/// 한가할 때 빗자루에 오르는 빈도 (초당 — 🌱 power를 곱함)
pub const BROOM_RATE: f64 = 0.01;
/// 다음 비행까지 (초)
pub const BROOM_CALM_SECS: (f64, f64) = (60.0, 150.0);
/// 빗자루에 오르는 시간 (초)
pub const MOUNT_SECS: f64 = 0.35;
/// 비행 빠르기 (걷기 속도의 배 — 🌱 soft_power)
pub const BROOM_SPEED: f64 = 3.6;
/// 허공 점 수 (그 뒤 내려앉는 호 하나)
pub const LEGS: (u8, u8) = (1, 3);
/// 낮은 호의 몫 (나머지는 높은 호)
pub const LOW_ARC_CHANCE: f64 = 0.5;
/// 낮은 호: 두 끝 중 높은 쪽보다 이만큼 위로 휨 (몸 크기 배)
pub const LOW_ARC: (f64, f64) = (0.5, 1.6);
/// 높은 호: 화면 위에서 이만큼 아래까지 솟음 (작업 영역 높이 몫)
pub const HIGH_ARC: (f64, f64) = (0.08, 0.35);
/// 허공 점이 화면 위·옆에서 떨어지는 거리 (몸 크기 배 — 위는 창이 화면 밖으로 나가지 않게 1보다 큼)
pub const AIR_MARGIN: f64 = 1.3;
/// 날아오를 때 "up we go!" 하는 몫
pub const UP_LINE_CHANCE: f64 = 0.5;
/// 내려앉을 때 "smooth landing" 하는 몫
pub const LAND_LINE_CHANCE: f64 = 0.35;
/// 사뿐 내려앉는 시간 (초)
pub const LAND_SECS: f64 = 0.4;
/// 끊긴 비행이 빗자루째 내려오는 중력 (보통 중력 몫)과 가장 큰 내려오는 빠르기 (몸 크기/초)
pub const DESCENT_GRAVITY: f64 = 0.25;
pub const DESCENT_MAX_VY: f64 = 2.5;
/// 친구 화면을 빗자루로 오가는 빠르기 (걷기 배)와 창을 띄우는 높이 (몸 크기 배)
pub const TRANSIT_SPEED: f64 = 2.6;
pub const TRANSIT_LIFT: f64 = 0.45;
/// 끝 너머로 날아 나가는 호의 끝: 화면 끝에서 이만큼 밖 (몸 크기 배)
pub const OUT_PAST: f64 = 1.0;
/// 한 호의 가장 짧은 시간 (초)
pub const MIN_LEG_SECS: f64 = 0.8;
/// 내려앉을 디딤판의 가장 짧은 폭 (몸 크기 배)
pub const LAND_MIN_WIDTH: f64 = 1.5;

/// 🟡 처음 스니치가 나올 수 있는 때 (clock 초)
pub const SNITCH_FIRST_SECS: f64 = 100.0;
/// 🟡 한가할 때 스니치가 나오는 빈도 (초당 — 🌱 power)
pub const SNITCH_RATE: f64 = 0.004;
/// 🟡 다음 스니치까지 (초)
pub const SNITCH_CALM_SECS: (f64, f64) = (180.0, 420.0);
/// 🟡 스니치를 보고 올려다보는 시간 (초)
pub const SPOT_SECS: f64 = 0.8;
/// 🟡 스니치 빠르기 (몸 크기/초)
pub const SNITCH_SPEED: f64 = 4.2;
/// 🟡 스니치가 목표를 바꾸는 간격 (초)
pub const JINK_SECS: (f64, f64) = (0.35, 1.0);
/// 🟡 쫓는 펫이 이만큼 (몸 크기 배) 가까우면 반대쪽으로 휙 피함 (지치기 전)
pub const DODGE_REACH: f64 = 1.6;
/// 🟡 이만큼 쫓기면 지쳐 느려짐 (초)과 그때 빠르기 배
pub const TIRE_SECS: f64 = 7.0;
pub const TIRED_PACE: f64 = 0.35;
/// 🟡 쫓는 빠르기 (몸 크기/초 — 🌱 soft_power)와 방향을 트는 몫 (초당)
pub const CHASE_SPEED: f64 = 4.8;
pub const CHASE_TURN: f64 = 3.0;
/// 🟡 잡는 거리: 손 (몸 가운데 앞) 에서 스니치까지 (몸 크기 배)
pub const CATCH_REACH: f64 = 0.45;
/// 🟡 잡고 떠서 쥐고 있는 시간 (초)
pub const CAUGHT_SECS: f64 = 1.2;
/// 🟡 이만큼 못 잡으면 스니치가 사라지고 내려앉음 (초)
pub const GIVE_UP_SECS: f64 = 22.0;
/// 🟡 스니치 공 지름 (펫 크기 몫 — 앱이 그림)
pub const SNITCH_SIZE: f64 = 0.16;

/// 2차 베지어 곡선의 u (0..1) 자리
pub fn bezier(p0: (f64, f64), c: (f64, f64), p1: (f64, f64), u: f64) -> (f64, f64) {
    let v = 1.0 - u;
    (v * v * p0.0 + 2.0 * v * u * c.0 + u * u * p1.0, v * v * p0.1 + 2.0 * v * u * c.1 + u * u * p1.1)
}

/// 2차 베지어 곡선의 길이 (16토막으로 잼)
pub fn bezier_len(p0: (f64, f64), c: (f64, f64), p1: (f64, f64)) -> f64 {
    let mut len = 0.0;
    let mut prev = p0;
    for i in 1..=16 {
        let q = bezier(p0, c, p1, i as f64 / 16.0);
        len += (q.0 - prev.0).hypot(q.1 - prev.1);
        prev = q;
    }
    len
}

/// 천천히 출발해 천천히 닿는 진행 (0..1 → 0..1)
pub fn ease(u: f64) -> f64 {
    let u = u.clamp(0.0, 1.0);
    u * u * (3.0 - 2.0 * u)
}

/// 호의 휜 점: 낮은 호는 두 끝 중 높은 쪽보다 lift (물리 px) 위, 높은 호는 화면 위에서 high (물리 px) 아래.
/// 창이 화면 위로 나가지 않게 area 위 + top_gap 아래로 자름 (호는 세 점이 이루는 세모 안에 있으므로 끝점도 그 아래면 곡선 전체가 그 아래)
pub fn arc_ctrl(from: (f64, f64), to: (f64, f64), area: Rect, top_gap: f64, low: Option<f64>, high: f64) -> (f64, f64) {
    let mx = (from.0 + to.0) / 2.0;
    let y = match low {
        Some(lift) => from.1.min(to.1) - lift,
        None => area.top + high,
    };
    (mx, y.max(area.top + top_gap))
}

/// 🟡 골든 스니치 하나 — 작업 영역 안을 이리저리 날다 쫓기면 피하고, 오래 쫓기면 지침 (자리는 물리 px, 공 가운데)
#[derive(Debug, Clone)]
pub struct Snitch {
    /// 공 가운데 (물리 px)
    pub pos: (f64, f64),
    vel: (f64, f64),
    goal: (f64, f64),
    goal_in: f64,
    /// 나온 뒤 흐른 시간 (초)
    pub age: f64,
    rng: fastrand::Rng,
}

impl Snitch {
    /// pos에 나타남 (seed로 난수)
    pub fn new(pos: (f64, f64), seed: u64) -> Snitch {
        Snitch { pos, vel: (0.0, 0.0), goal: pos, goal_in: 0.0, age: 0.0, rng: fastrand::Rng::with_seed(seed ^ 0x736e_6974) }
    }

    /// 지쳤음 (TIRE_SECS 넘게 쫓김)
    pub fn tired(&self) -> bool {
        self.age >= TIRE_SECS
    }

    /// 한 틱: 목표 쪽으로 날고 (빠르기 = SNITCH_SPEED × size), 가끔 새 목표 — 쫓는 쪽(chaser)이 가까우면 반대쪽 목표로 휙 (지치기 전).
    /// area 안 (margin만큼 안쪽)에 머묾
    pub fn tick(&mut self, dt: f64, area: Rect, size: f64, chaser: (f64, f64)) {
        self.age += dt;
        let m = size * 0.6;
        let (lo_x, hi_x, lo_y, hi_y) = (area.left + m, (area.right - m).max(area.left + m), area.top + m, (area.bottom - 1.5 * size).max(area.top + m));
        let near = (chaser.0 - self.pos.0).hypot(chaser.1 - self.pos.1) < DODGE_REACH * size;
        self.goal_in -= dt;
        if self.goal_in <= 0.0 || (near && !self.tired()) {
            self.goal_in = crate::behavior::range(&mut self.rng, JINK_SECS);
            let (mut gx, mut gy) = (lo_x + self.rng.f64() * (hi_x - lo_x), lo_y + self.rng.f64() * (hi_y - lo_y));
            if near && !self.tired() {
                // 쫓는 쪽 반대편으로
                let (dx, dy) = (self.pos.0 - chaser.0, self.pos.1 - chaser.1);
                let d = dx.hypot(dy).max(1.0);
                gx = self.pos.0 + dx / d * 3.0 * size;
                gy = self.pos.1 + dy / d * 3.0 * size;
            }
            self.goal = (gx.clamp(lo_x, hi_x), gy.clamp(lo_y, hi_y));
        }
        let speed = SNITCH_SPEED * size * if self.tired() { TIRED_PACE } else { 1.0 };
        let (dx, dy) = (self.goal.0 - self.pos.0, self.goal.1 - self.pos.1);
        let d = dx.hypot(dy);
        let want = if d > 1.0 { (dx / d * speed, dy / d * speed) } else { (0.0, 0.0) };
        // 휙휙 꺾임 (빠르게 방향을 바꿈)
        let k = (8.0 * dt).min(1.0);
        self.vel = (self.vel.0 + (want.0 - self.vel.0) * k, self.vel.1 + (want.1 - self.vel.1) * k);
        self.pos = ((self.pos.0 + self.vel.0 * dt).clamp(lo_x, hi_x), (self.pos.1 + self.vel.1 * dt).clamp(lo_y, hi_y));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn area() -> Rect {
        Rect { left: 0.0, top: 0.0, right: 1920.0, bottom: 1040.0 }
    }

    #[test]
    fn a_bezier_runs_from_start_to_end_through_the_bend() {
        let (a, c, b) = ((0.0, 100.0), (50.0, 0.0), (100.0, 100.0));
        assert_eq!(bezier(a, c, b, 0.0), a);
        assert_eq!(bezier(a, c, b, 1.0), b);
        assert!((bezier(a, c, b, 0.5).1 - 50.0).abs() < 1e-9);
        let len = bezier_len(a, c, b);
        assert!(len > 141.0 && len < 160.0, "{len}");
        assert_eq!((ease(0.0), ease(1.0)), (0.0, 1.0));
        assert!(ease(0.1) < 0.1 && ease(0.9) > 0.9);
    }

    #[test]
    fn arcs_bend_up_but_never_above_the_screen_top() {
        let (from, to) = ((100.0, 900.0), (900.0, 700.0));
        let low = arc_ctrl(from, to, area(), 160.0, Some(100.0), 0.0);
        assert_eq!(low, (500.0, 600.0));
        let high = arc_ctrl(from, to, area(), 160.0, None, 200.0);
        assert_eq!(high, (500.0, 200.0));
        let clipped = arc_ctrl(from, to, area(), 160.0, None, 20.0);
        assert_eq!(clipped.1, 160.0);
    }

    #[test]
    fn the_snitch_darts_around_inside_the_screen_and_tires() {
        let mut s = Snitch::new((900.0, 300.0), 7);
        let mut moved = 0.0;
        let mut prev = s.pos;
        for _ in 0..600 {
            s.tick(1.0 / 60.0, area(), 128.0, (100.0, 900.0));
            moved += (s.pos.0 - prev.0).hypot(s.pos.1 - prev.1);
            prev = s.pos;
            assert!(s.pos.0 >= 0.0 && s.pos.0 <= 1920.0 && s.pos.1 >= 0.0 && s.pos.1 <= 1040.0);
        }
        assert!(moved > 128.0 * 10.0, "darts around {moved}");
        assert!(s.tired());
        // 지친 스니치는 느림
        let before = s.pos;
        let mut slow = 0.0;
        for _ in 0..60 {
            s.tick(1.0 / 60.0, area(), 128.0, (100.0, 900.0));
        }
        slow += (s.pos.0 - before.0).hypot(s.pos.1 - before.1);
        assert!(slow <= SNITCH_SPEED * TIRED_PACE * 128.0 * 1.05, "{slow}");
    }

    #[test]
    fn the_snitch_dodges_a_close_chaser_before_it_tires() {
        let mut s = Snitch::new((900.0, 400.0), 3);
        s.tick(1.0 / 60.0, area(), 128.0, (850.0, 400.0));
        // 쫓는 쪽(왼쪽)에서 멀어지는 목표
        assert!(s.goal.0 > 900.0, "{:?}", s.goal);
    }
}
