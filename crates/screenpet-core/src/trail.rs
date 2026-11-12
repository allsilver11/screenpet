//! 🌈 무지개 구름 자국: 몸 색이 무지개인 동물(🎨 흐물이 색, 손님도 주인이 고른 색)이 달리거나 구르거나 던져져 날아가면
//! 발 뒤로 작은 무지개 구름이 퐁퐁 남음. 구름은 화면에 남아 있어야 해서 펫 창이 아니라 모니터를 덮는 마우스 통과 창
//! (`trail-<n>`, ui/trail.js)이 그림 — 여기서는 언제·어디에·무슨 색으로 하나 남길지만 정함 (빗자루 broom.rs의 퐁처럼 간격을 둠).
//! 걷기는 남기지 않음. 숨기기·자리 비움·전체 화면·하우스 안·친구 화면에 가 있을 땐 앱이 `visible = false`를 줌

use serde::Serialize;

use crate::brain::{Anim, Pet};
use crate::tint::BodyTint;

/// 구름 사이 최소 시간 (초)
pub const PUFF_EVERY: f64 = 0.08;
/// 지난 구름에서 이만큼(몸 크기 배)은 움직여야 다음 구름 — 제자리 달리기는 남기지 않음
pub const MIN_STEP: f64 = 0.12;
/// 빨리 날아갈 때 구름 사이를 메우는 간격 (몸 크기 배)과 한 틱에 최대 개수
pub const GAP: f64 = 0.32;
/// 한 틱에 남기는 구름 최대 수
pub const MAX_PER_TICK: usize = 3;
/// 무지개 색 수 (빨강·주황·노랑·초록·하늘·파랑·보라 — ui/trail.js RAINBOW)
pub const COLORS: u8 = 7;
/// 발 뒤로 (몸 크기 배), 바닥에서 위로 (몸 크기 배)
const BEHIND: f64 = 0.28;
const LIFT: f64 = 0.1;

/// 구름을 남기는 빠른 움직임
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Dash {
    /// 달리기 (줄행랑·술래 놀이·집으로 달려가기 포함)
    Run,
    /// 데굴데굴
    Roll,
    /// 던져져 날아가는 중 — 몸 가운데에서 무지개 포물선
    Fling,
}

impl Dash {
    /// 구름 지름 (몸 크기 배)
    fn puff_size(self) -> f64 {
        match self {
            Dash::Run => 0.3,
            Dash::Roll => 0.36,
            Dash::Fling => 0.34,
        }
    }

    /// 지금 이 동물이 구름을 남길 만큼 빠르게 움직이는가. 하우스 안·친구 화면·들린 동안은 아님
    pub fn of(pet: &Pet) -> Option<Dash> {
        if pet.is_housed() || pet.is_away() || pet.is_held() {
            return None;
        }
        if pet.is_flung() {
            return Some(Dash::Fling);
        }
        // 공중(폴짝·낙하)과 벽 타기는 아니다
        pet.standing?;
        match pet.render().anim {
            Anim::Run | Anim::Panic => Some(Dash::Run),
            Anim::Roll => Some(Dash::Roll),
            _ => None,
        }
    }
}

/// 몸 색이 무지개이고, 그 동물이 몸 색을 칠하는 그림(잉크 선 동물 — pet.json `"tint": true`)일 때만
pub fn rainbow(tint: BodyTint, paints: bool) -> bool {
    paints && tint == BodyTint::Rainbow
}

/// 이번 틱의 동물 모습 (물리 px)
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Runner {
    /// 빨리 움직이는 중 (없으면 구름 없음)
    pub dash: Option<Dash>,
    /// 발 (가운데 아래)
    pub feet: (f64, f64),
    /// +1 오른쪽, -1 왼쪽
    pub facing: f64,
    /// 몸 크기 (물리 px)
    pub size: f64,
    /// 픽셀 스타일이면 칸 구름
    pub pixel: bool,
}

impl Runner {
    /// 동물의 이번 틱 모습
    pub fn of(pet: &Pet, pixel: bool) -> Self {
        Runner { dash: Dash::of(pet), feet: (pet.body.x, pet.body.y), facing: pet.facing, size: pet.cfg.size, pixel }
    }

    /// 구름이 나오는 자리: 발 뒤 (보는 쪽 반대), 던져졌으면 몸 가운데
    fn spot(&self, dash: Dash) -> (f64, f64) {
        let (x, y) = self.feet;
        match dash {
            Dash::Fling => (x, y - self.size * 0.45),
            Dash::Run | Dash::Roll => (x - self.facing.signum() * self.size * BEHIND, y - self.size * LIFT),
        }
    }
}

/// 구름 하나 (물리 px, 화면 좌표)
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct Puff {
    /// 가운데 x (물리 px)
    pub x: f64,
    /// 가운데 y (물리 px)
    pub y: f64,
    /// 지름 (물리 px)
    pub size: f64,
    /// 무지개 몇 번째 색 (0 = 빨강 … 6 = 보라)
    pub color: u8,
    /// 픽셀 스타일 칸 구름
    pub pixel: bool,
    /// 흘러가는 쪽 (-1 왼쪽, +1 오른쪽, 0 제자리)
    pub dir: f64,
}

/// 동물 하나의 구름 내보내기 (내 동물·무지개 손님마다 하나)
#[derive(Debug, Clone, Default)]
pub struct Trail {
    wait: f64,
    color: u8,
    last: Option<(f64, f64)>,
}

impl Trail {
    /// 빈 꼬리
    pub fn new() -> Self {
        Self::default()
    }

    /// 매 틱. 남길 구름을 out에 더함 (visible: 무지개이고 이 동물과 그 모니터가 보이는 중)
    pub fn tick(&mut self, dt: f64, visible: bool, r: &Runner, out: &mut Vec<Puff>) {
        self.wait = (self.wait - dt).max(0.0);
        let Some(dash) = r.dash.filter(|_| visible && r.size > 0.0) else {
            // 멈추면 다음 달리기는 빨강부터
            *self = Trail::default();
            return;
        };
        let spot = r.spot(dash);
        let dir = if dash == Dash::Fling { 0.0 } else { -r.facing.signum() };
        let size = r.size * dash.puff_size();
        let Some(prev) = self.last else {
            self.push(spot, size, r.pixel, dir, out);
            return;
        };
        if self.wait > 0.0 {
            return;
        }
        let (dx, dy) = (spot.0 - prev.0, spot.1 - prev.1);
        let d = dx.hypot(dy);
        if d < r.size * MIN_STEP {
            return;
        }
        let n = ((d / (r.size * GAP)).ceil() as usize).clamp(1, MAX_PER_TICK);
        for k in 1..=n {
            let f = k as f64 / n as f64;
            self.push((prev.0 + dx * f, prev.1 + dy * f), size, r.pixel, dir, out);
        }
    }

    /// 구름 하나를 남기고 다음 색으로 넘어감
    fn push(&mut self, (x, y): (f64, f64), size: f64, pixel: bool, dir: f64, out: &mut Vec<Puff>) {
        out.push(Puff { x, y, size, color: self.color, pixel, dir });
        self.color = (self.color + 1) % COLORS;
        self.last = Some((x, y));
        self.wait = PUFF_EVERY;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::brain::{Standing, State};
    use crate::behavior::Move;
    use crate::physics::Body;
    use crate::surfaces::Owner;
    use crate::test_util::{DT, idle, pet_cfg};

    /// 테스트: 바닥 y 1000의 x에서 facing을 보는 동물 모습
    fn runner(dash: Option<Dash>, x: f64, facing: f64) -> Runner {
        Runner { dash, feet: (x, 1000.0), facing, size: 128.0, pixel: false }
    }

    /// 달리기 속도(약 300 px/s)로 secs초 달림: 남긴 구름
    fn run_for(t: &mut Trail, secs: f64, facing: f64, visible: bool) -> Vec<Puff> {
        let mut out = Vec::new();
        let mut x = 500.0;
        for _ in 0..(secs / DT).round() as usize {
            x += facing * 300.0 * DT;
            t.tick(DT, visible, &runner(Some(Dash::Run), x, facing), &mut out);
        }
        out
    }

    /// 테스트: 바닥에 state로 서 있는 동물
    fn ground_pet(state: State) -> Pet {
        let mut p = Pet::new(pet_cfg(), 500.0, 1000.0, 1);
        p.standing = Some(Standing { owner: Owner::Ground(0), rel_x: 500.0 });
        p.body = Body { x: 500.0, y: 1000.0, vx: 0.0, vy: 0.0 };
        p.state = state;
        p
    }

    #[test]
    fn only_a_rainbow_body_that_is_painted_leaves_clouds() {
        assert!(rainbow(BodyTint::Rainbow, true));
        assert!(!rainbow(BodyTint::Rainbow, false), "pets without an ink layer show no tint");
        for t in BodyTint::ALL.into_iter().filter(|t| *t != BodyTint::Rainbow) {
            assert!(!rainbow(t, true), "{t:?}");
        }
    }

    #[test]
    fn running_rolling_and_flying_through_the_air_dash_but_walking_does_not() {
        let run = ground_pet(State::Move { kind: Move::Run, remaining: 5.0 });
        assert_eq!(Dash::of(&run), Some(Dash::Run));
        let roll = ground_pet(State::Move { kind: Move::Roll, remaining: 5.0 });
        assert_eq!(Dash::of(&roll), Some(Dash::Roll));
        let chase = ground_pet(State::Chase { remaining: 5.0 });
        assert_eq!(Dash::of(&chase), Some(Dash::Run));
        let walk = ground_pet(State::Move { kind: Move::Walk, remaining: 5.0 });
        assert_eq!(Dash::of(&walk), None);
        assert_eq!(Dash::of(&ground_pet(idle())), None);
        let mut thrown = ground_pet(idle());
        thrown.force_throw(900.0, -600.0);
        assert_eq!(Dash::of(&thrown), Some(Dash::Fling));
    }

    #[test]
    fn nothing_while_held_away_or_in_the_house() {
        assert_eq!(Dash::of(&ground_pet(State::Dragged)), None);
        assert_eq!(Dash::of(&ground_pet(State::Away)), None);
        let mut housed = ground_pet(idle());
        housed.enter_house();
        assert!(housed.is_housed());
        housed.state = State::Move { kind: Move::Run, remaining: 5.0 };
        assert_eq!(Dash::of(&housed), None);
        // 공중의 폴짝(던진 게 아님)도 아니다
        let mut hop = ground_pet(State::Air { anim: Anim::Hop });
        hop.standing = None;
        assert_eq!(Dash::of(&hop), None);
    }

    #[test]
    fn a_run_leaves_a_puff_every_so_often() {
        let mut t = Trail::new();
        let out = run_for(&mut t, 1.0, 1.0, true);
        // 0.08초마다 하나 (+ 처음 하나) — 틱에 맞춰 조금 늦어진다
        assert!((9..=13).contains(&out.len()), "{}", out.len());
        // 한 틱에 둘 이상은 달리기에선 나오지 않는다
        let mut t = Trail::new();
        let mut out = Vec::new();
        t.tick(DT, true, &runner(Some(Dash::Run), 500.0, 1.0), &mut out);
        t.tick(DT, true, &runner(Some(Dash::Run), 505.0, 1.0), &mut out);
        assert_eq!(out.len(), 1, "too soon");
    }

    #[test]
    fn hidden_or_stopped_runs_leave_nothing() {
        let mut t = Trail::new();
        assert!(run_for(&mut t, 1.0, 1.0, false).is_empty());
        let mut out = Vec::new();
        for i in 0..60 {
            t.tick(DT, true, &runner(None, 500.0 + i as f64 * 5.0, 1.0), &mut out);
        }
        assert!(out.is_empty());
        // 제자리 달리기 (막혀서) — 처음 하나뿐
        for _ in 0..60 {
            t.tick(DT, true, &runner(Some(Dash::Run), 500.0, 1.0), &mut out);
        }
        assert_eq!(out.len(), 1);
    }

    #[test]
    fn puffs_come_out_behind_the_feet() {
        let mut out = Vec::new();
        Trail::new().tick(DT, true, &runner(Some(Dash::Run), 500.0, 1.0), &mut out);
        Trail::new().tick(DT, true, &runner(Some(Dash::Run), 500.0, -1.0), &mut out);
        let (right, left) = (out[0], out[1]);
        assert!(right.x < 500.0 && left.x > 500.0, "{right:?} {left:?}");
        assert!((500.0 - right.x - (left.x - 500.0)).abs() < 1e-9);
        assert!(right.y < 1000.0 && right.y > 1000.0 - 128.0 * 0.3, "just above the ground");
        // 뒤로 흘러간다
        assert_eq!((right.dir, left.dir), (-1.0, 1.0));
    }

    #[test]
    fn the_colors_go_round_the_rainbow_and_start_red_each_run() {
        let mut t = Trail::new();
        let out = run_for(&mut t, 1.5, -1.0, true);
        let colors: Vec<u8> = out.iter().map(|p| p.color).collect();
        assert_eq!(&colors[..9], &[0, 1, 2, 3, 4, 5, 6, 0, 1]);
        // 멈췄다가 다시 달리면 빨강부터
        let mut out = Vec::new();
        t.tick(DT, true, &runner(None, 0.0, 1.0), &mut out);
        let again = run_for(&mut t, 0.2, 1.0, true);
        assert_eq!(again[0].color, 0);
    }

    #[test]
    fn a_fast_fling_fills_the_gap_but_is_capped() {
        let mut t = Trail::new();
        let mut out = Vec::new();
        let fling = |x: f64| Runner { dash: Some(Dash::Fling), feet: (x, 600.0), facing: 1.0, size: 128.0, pixel: true };
        t.tick(DT, true, &fling(100.0), &mut out);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].y, 600.0 - 128.0 * 0.45, "from the middle of the body");
        assert_eq!(out[0].dir, 0.0);
        assert!(out[0].pixel);
        // 0.1초 동안 300 px — 사이를 메우되 한 틱에 MAX_PER_TICK개까지
        for _ in 0..6 {
            t.tick(DT, true, &fling(100.0), &mut out);
        }
        t.tick(DT, true, &fling(400.0), &mut out);
        assert_eq!(out.len(), 1 + MAX_PER_TICK);
        assert_eq!(out.last().unwrap().x, 400.0);
        let xs: Vec<f64> = out.iter().map(|p| p.x).collect();
        assert!(xs.windows(2).all(|w| w[1] > w[0]), "{xs:?}");
    }

    #[test]
    fn cloud_size_follows_the_pet_size() {
        let mut out = Vec::new();
        Trail::new().tick(DT, true, &Runner { size: 64.0, ..runner(Some(Dash::Run), 0.0, 1.0) }, &mut out);
        Trail::new().tick(DT, true, &Runner { size: 256.0, ..runner(Some(Dash::Run), 0.0, 1.0) }, &mut out);
        assert!((out[1].size / out[0].size - 4.0).abs() < 1e-9);
        assert!(out[0].size > 10.0 && out[1].size < 100.0, "{out:?}");
    }

    #[test]
    fn runner_reads_the_pet() {
        let mut p = ground_pet(State::Move { kind: Move::Run, remaining: 5.0 });
        p.facing = -1.0;
        let r = Runner::of(&p, true);
        assert_eq!(r.dash, Some(Dash::Run));
        assert_eq!((r.feet, r.facing, r.size, r.pixel), ((500.0, 1000.0), -1.0, 128.0, true));
    }
}
