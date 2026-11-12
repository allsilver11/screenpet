//! 🖍️ 바닥에 낙서 (흐물이, v0.6.3) — brain.rs의 자식 모듈.
//!
//! 바닥에서 쉬거나 걷던 흐물이가 가끔 (DOODLE_RATE, 쿨다운 COOLDOWN_SECS) 멈춰 쪼그려 앉아 DOODLE_SECS(2초) 동안 분필로 바닥에 그림 (draw).
//! 그리기 시작하는 순간 take_doodle이 한 번 Doodle을 돌려줌 — 그림 자체(하트·웃는 얼굴·별·이름)는 앱의 덧창이
//! 그리는 동안 천천히 나타나게 그리고, 잠시 뒤 흐려 지움

use serde::Serialize;

use super::*;

/// 그리는 시간 (초) — 덧창은 이 동안 그림이 나타나게 함
pub const DOODLE_SECS: f64 = 2.0;
/// 바닥에서 쉬거나 걷는 동안 낙서를 시작할 빈도 (초당 확률)
pub const DOODLE_RATE: f64 = 1.0 / 90.0;
/// 한 번 그린 뒤 다시 그리기까지
pub const COOLDOWN_SECS: (f64, f64) = (180.0, 360.0);
/// 앱을 켜고 처음 그리기까지
const FIRST_READY_SECS: f64 = 60.0;
/// 그림 가운데: 몸 가운데에서 앞으로 몸 이만큼
pub const DOODLE_AHEAD: f64 = 0.45;

/// 무엇을 그리나
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DoodleKind {
    Heart,
    Smile,
    Star,
    /// 이름 (글자는 앱이 그 동물의 이름으로)
    Name,
}

/// 바닥 낙서 하나 (take_doodle로 한 번 꺼냄)
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct Doodle {
    /// 그림 가운데 x, 바닥 y (물리 px — 그림은 이 선 바로 위에 누움)
    pub x: f64,
    /// 바닥 y (물리 px)
    pub y: f64,
    /// 낙서 종류
    pub kind: DoodleKind,
    /// +1 오른쪽을 보고 그렸음, -1 왼쪽
    pub facing: f64,
    /// 그린 동물의 크기 배율 (몸 한 변 / 128 — 화면 배율까지, 물리 px 기준)
    pub scale: f64,
}

/// 바닥 낙서의 기억: 난수, 다음 낙서 시각, 꺼내 가지 않은 낙서
#[derive(Debug, Clone)]
pub struct DoodleMood {
    rng: fastrand::Rng,
    /// 이 시각(clock)부터 낙서할 수 있음
    pub(super) ready_at: f64,
    out: Option<Doodle>,
}

impl DoodleMood {
    /// 시드로 난수를 따로 둠 (다른 장난의 난수 순서를 흔들지 않음)
    pub(super) fn new(seed: u64) -> Self {
        DoodleMood { rng: fastrand::Rng::with_seed(seed ^ 0x646f_6f64), ready_at: FIRST_READY_SECS, out: None }
    }
}

impl Pet {
    /// 🖍️ 새 낙서 (그리기 시작할 때 한 번)
    pub fn take_doodle(&mut self) -> Option<Doodle> {
        self.doodle.out.take()
    }

    /// 한가할 때 DOODLE_RATE 확률로 낙서를 시작함. 시작했으면 true
    pub(super) fn maybe_doodle(&mut self, dt: f64, world: &World) -> bool {
        if self.clock < self.doodle.ready_at || !self.antics_free() || self.doodle.rng.f64() >= DOODLE_RATE * dt {
            return false;
        }
        self.start_doodle(world)
    }

    /// 앞 바닥에 그림 (그릴 자리가 같은 바닥 조각 위가 아니면 돌아서서). 시작했으면 true
    pub(super) fn start_doodle(&mut self, world: &World) -> bool {
        let Some(st) = self.standing else { return false };
        let Some(piece) = world.piece_at(st.owner, self.body.x).copied() else { return false };
        let ahead = DOODLE_AHEAD * self.cfg.size;
        let fits = |x: f64| (piece.x_start..=piece.x_end).contains(&x);
        if !fits(self.body.x + self.facing * ahead) {
            self.facing = -self.facing;
            if !fits(self.body.x + self.facing * ahead) {
                return false;
            }
        }
        let kind = [DoodleKind::Heart, DoodleKind::Smile, DoodleKind::Star, DoodleKind::Name][self.doodle.rng.usize(..4)];
        self.doodle.ready_at = self.clock + behavior::range(&mut self.doodle.rng, COOLDOWN_SECS);
        self.doodle.out = Some(Doodle { x: self.body.x + self.facing * ahead, y: self.body.y, kind, facing: self.facing, scale: self.cfg.size / PET_LOGICAL_SIZE });
        self.pacing.after_beat = None;
        self.clear_hover();
        self.state = State::Doodle { remaining: DOODLE_SECS };
        true
    }

    /// 낙서하는 한 틱: 방해받거나 디딤판이 사라지면 그만둠
    pub(super) fn update_doodle(&mut self, dt: f64, world: &World, remaining: f64) {
        if self.antics_interrupted() {
            self.state = self.idle_state();
            return;
        }
        if !self.follow_anchor(world) {
            self.start_fall();
            return;
        }
        let remaining = remaining - dt;
        self.state = if remaining > 0.0 { State::Doodle { remaining } } else { self.rest_beat() };
    }
}
