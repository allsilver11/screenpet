//! 펫끼리 놀기 더 (2026-11-20) 의 Pet 동작 — brain.rs의 자식 모듈. 🥊 투닥투닥 · 😈 몰래 찌르기 · 🫸 밀치기 · 😴 같이 낮잠 ·
//! 🏁 달리기 시합 · 💬 수다의 짝·차례는 social_romp.rs(코디네이터)가 정하고, 여기서는 틱마다 받은 몸짓(Pose)대로 움직임.
//!
//! - State::Romp 동안 코디네이터가 매 틱 몸짓을 줌: 그림, 갈 x와 빠르기(걷기 속도 배 — 무한이면 그 자리로 바로), 볼 쪽,
//!   그림이 뜨는 높이, 덜덜·반짝·콰당 효과. 디딤판 조각 안에서만 움직임 (끝에서 뛰어내리지 않음)
//! - 몸짓 소식이 ROMP_STALE_SECS 넘게 없으면 (코디네이터가 사라짐) 스스로 그만두고 쉼
//! - 사용자 손길·먹이·집 가기·조작·손님의 집 생각 등이 생기면 바로 그만둠 (코디네이터가 짝도 풀어 줌)
//! - 😴 같이 낮잠의 잠은 평소 State::Sleep (클릭·간질이기 반응 그대로), 😈 찔린 뒤 쫓기는 평소 술래 놀이 (brain_social.rs)

use super::*;
use crate::social::RompKind;

/// 몸짓 소식이 이만큼 (초) 없으면 스스로 그만둠
pub const ROMP_STALE_SECS: f64 = 0.5;
/// 같이 낮잠의 잠 시간 상한 (먼저 자던 친구가 깨면 코디네이터가 더 일찍 깨움)
pub const NAP_MAX_SECS: f64 = 120.0;

/// 코디네이터가 틱마다 주는 몸짓
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Pose {
    /// 그림
    pub anim: Anim,
    /// 이 x로 감 (None이면 제자리)
    pub to: Option<f64>,
    /// 빠르기 — 걷기 속도(cfg.walk_speed) 배. 무한이면 그 자리로 바로 (데굴데굴 덩어리)
    pub pace: f64,
    /// 볼 쪽 (±1, 0이면 가는 쪽을 봄)
    pub face: f64,
    /// 그림이 뜬 높이 (몸 크기 배 — 발은 디딤판 그대로)
    pub lift: f64,
    /// 덜덜 (render.wobble)
    pub wobble: bool,
    /// 반짝이 (render.sparkle)
    pub sparkle: bool,
    /// 콰당 💫 (render.thud)
    pub thud: bool,
}

impl Pose {
    /// 제자리에서 그림 하나
    pub fn hold(anim: Anim) -> Self {
        Pose { anim, to: None, pace: 0.0, face: 0.0, lift: 0.0, wobble: false, sparkle: false, thud: false }
    }

    /// x로 pace(걷기 속도 배)만큼 빠르게 가며 그림 하나
    pub fn go(anim: Anim, x: f64, pace: f64) -> Self {
        Pose { to: Some(x), pace, ..Pose::hold(anim) }
    }

    /// 볼 쪽을 정함
    pub fn facing(self, face: f64) -> Self {
        Pose { face, ..self }
    }
}

impl Default for Pose {
    /// 그냥 서 있기
    fn default() -> Self {
        Pose::hold(Anim::Idle)
    }
}

impl Pet {
    /// 새 펫끼리 놀이를 시작할 수 있음: 걷는 동물이 디딤판 위에서 스스로 걷거나 쉬는 중, 볼일·조작·잡힘·조용한 때 없음
    pub fn romp_free(&self) -> bool {
        self.social_free() && self.play_free()
    }

    /// 걷지 않고 쉬는 중 (달리기 시합·수다·장난 상대)
    pub fn romp_resting(&self) -> bool {
        matches!(self.state, State::Idle { .. } | State::Turn { .. })
    }

    /// 서 있는 디딤판 조각 (왼쪽, 오른쪽) — 지난 update 때
    pub fn ground_span(&self) -> Option<(f64, f64)> {
        self.standing.and(self.ground_span)
    }

    /// 펫끼리 놀이 종류마다 이 펫이 얼마나 좋아하나 (확률에 곱함). 지금은 다 1 — 연령대(2부)가 여기서 바꿈
    pub fn social_bias(&self, _kind: RompKind) -> f64 {
        1.0
    }

    /// 펫끼리 놀이 중 (몸짓을 받는 중)
    pub fn in_romp(&self) -> bool {
        matches!(self.state, State::Romp { .. })
    }

    /// 펫끼리 놀이를 시작함: 하던 길·박자를 잊고 pose부터
    pub fn start_romp(&mut self, pose: Pose) {
        self.pacing.after_beat = None;
        self.flight = None;
        self.hover_at = None;
        self.clear_hover();
        self.body.vx = 0.0;
        self.romp = pose;
        self.state = State::Romp { remaining: ROMP_STALE_SECS };
    }

    /// 이번 틱의 몸짓 (놀이 중일 때만)
    pub fn set_romp_pose(&mut self, pose: Pose) {
        if self.in_romp() {
            self.romp = pose;
            self.state = State::Romp { remaining: ROMP_STALE_SECS };
        }
    }

    /// 놀이 끝 — 잠깐 앉았다가 평소대로
    pub fn end_romp(&mut self) {
        if self.in_romp() {
            self.romp = Pose::default();
            self.state = self.rest_beat();
        }
    }

    /// 놀이 대사 (먼저 있던 말을 덮음 — 주고받는 말이라 늦으면 어색함)
    pub fn say_now(&mut self, speech: Speech) {
        self.speech = Some(speech);
    }

    /// 하트를 한 번 띄움
    pub fn romp_heart(&mut self) {
        self.heart_timer = HEART_SECS;
    }

    /// 😴 친구 옆에서 같이 잠
    pub fn start_nap(&mut self) {
        self.romp = Pose::default();
        self.pacing.after_beat = None;
        self.state = State::Sleep { remaining: NAP_MAX_SECS };
    }

    /// 😴 같이 자던 친구가 깼음: 아직 자고 있으면 기지개 켜며 깸
    pub fn wake_from_nap(&mut self) {
        if matches!(self.state, State::Sleep { .. }) {
            self.state = State::Trick { kind: Trick::Stretch, remaining: WAKE_STRETCH_SECS };
        }
    }

    /// 잠자는 중 (같이 낮잠 — 하우스 안은 아님)
    pub fn is_sleeping(&self) -> bool {
        matches!(self.state, State::Sleep { .. }) && !self.housed
    }

    /// 놀이 그림 효과: (덜덜, 반짝, 콰당)
    pub(super) fn romp_marks(&self) -> (bool, bool, bool) {
        if !self.in_romp() {
            return (false, false, false);
        }
        (self.romp.wobble, self.romp.sparkle, self.romp.thud)
    }

    /// 놀이 그림이 뜬 높이 (물리 px)
    pub(super) fn romp_lift(&self) -> f64 {
        if self.in_romp() {
            self.romp.lift * self.cfg.size
        } else {
            0.0
        }
    }

    /// 펫끼리 놀이 한 틱: 받은 몸짓대로 움직임. 끊을 일이 생겼거나 소식이 끊기면 쉬는 박자로
    pub(super) fn update_romp_pose(&mut self, dt: f64, world: &World, remaining: f64) {
        let remaining = remaining - dt;
        if self.romp_interrupted() || self.social_interrupted() || remaining <= 0.0 {
            self.end_romp();
            return;
        }
        if !self.follow_anchor(world) {
            self.romp = Pose::default();
            self.start_fall();
            return;
        }
        let pose = self.romp;
        if let Some(x) = pose.to {
            let dx = x - self.body.x;
            if dx.abs() > 0.5 {
                let step = if pose.pace.is_finite() { (pose.pace * self.cfg.walk_speed * dt).min(dx.abs()) } else { dx.abs() };
                if self.juggle_walk(world, dx.signum(), step).is_none() {
                    self.romp = Pose::default();
                    return;
                }
                if pose.face == 0.0 {
                    self.facing = dx.signum();
                }
            }
        }
        if pose.face != 0.0 {
            self.facing = pose.face.signum();
        }
        self.state = State::Romp { remaining };
    }
}
