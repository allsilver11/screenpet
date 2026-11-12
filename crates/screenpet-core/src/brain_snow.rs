//! 눈사람 만들기·벚꽃 올려다보기 (v0.3.7) — brain.rs의 자식 모듈 (비공개 필드에 접근).
//!
//! 앱이 바닥에 눈이 쌓였다고 알려 주면 (set_snow_ready — 내 동물에게만, 눈사람이 이미 서 있으면 false), 내 동물이 바닥에서 쉬다가
//! 가끔 눈덩이를 굴림 (굴리기 → 쌓기 → 뿌듯). 눈덩이 그림은 앱이 snowball_at 자리에 놓고,
//! 다 쌓으면 take_snowman이 눈사람 자리를 한 번 돌려줌. 들어 올리면 눈덩이는 사라지고 눈사람은 없음

use super::*;

/// 쉬는 박자가 끝날 때 눈사람을 만들기 시작할 확률
const SNOWMAN_CHANCE: f64 = 0.05;
/// 눈덩이 굴리는 시간 (ui/prop.html의 눈덩이 자라는 시간과 같음)
pub const ROLL_SECS: f64 = 3.0;
/// 굴리는 속도 (크기 128 기준 px/s)
const ROLL_SPEED: f64 = 60.0;
/// 눈덩이 위에 쌓아 올리는 시간
const STACK_SECS: f64 = 1.2;
/// 다 만들고 뿌듯해하는 시간
const ADMIRE_SECS: f64 = 1.6;
/// 눈덩이 가운데가 몸 중심에서 몸 크기의 이 비율만큼 앞
const BALL_REACH: f64 = 0.45;
/// 벚꽃잎이 날리는 동안 올려다보는 빈도 배수
pub const PETAL_LOOK_UP_MULT: f64 = 3.0;

/// 눈사람 만들기 단계: 굴리기 → 쌓기 → 감상
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SnowPhase {
    Roll,
    Stack,
    Admire,
}

/// 눈·꽃잎 날씨의 기억
#[derive(Debug, Clone, Copy, Default)]
pub(super) struct SnowMood {
    ready: bool,
    petals: bool,
    /// 방금 다 만든 눈사람 자리 (take_snowman으로 한 번 꺼냄)
    built: Option<(f64, f64)>,
}

impl Pet {
    /// 바닥에 눈이 쌓였고 눈사람을 만들어도 됨. false면 만들던 것도 그만둠
    pub fn set_snow_ready(&mut self, ready: bool) {
        self.snow.ready = ready;
        if !ready {
            self.snow.built = None;
        }
    }

    /// 벚꽃잎이 날림 — 더 자주 올려다봄
    pub fn set_petals(&mut self, on: bool) {
        self.snow.petals = on;
    }

    /// 방금 다 만든 눈사람의 자리 (아래 가운데, 물리 px). 한 번만
    pub fn take_snowman(&mut self) -> Option<(f64, f64)> {
        self.snow.built.take()
    }

    /// 굴리는 중인 눈덩이의 자리 (아래 가운데, 물리 px)
    pub fn snowball_at(&self) -> Option<(f64, f64)> {
        match self.state {
            State::Snowman { phase: SnowPhase::Roll | SnowPhase::Stack, dir, .. } => Some((self.body.x + dir * BALL_REACH * self.cfg.size, self.body.y)),
            _ => None,
        }
    }

    /// 동물을 바꿨음: 만들던 눈사람은 그만둠 (눈덩이 창은 snowball_at이 None이 되면 앱이 치움). 만드는 중이 아니면 그대로
    pub fn end_snowman(&mut self) {
        if matches!(self.state, State::Snowman { .. }) {
            self.state = self.idle_state();
        }
    }

    /// 올려다보기 빈도 배수: 꽃잎·눈구름이면 PETAL_LOOK_UP_MULT배
    pub(super) fn look_up_mult(&self) -> f64 {
        if self.snow.petals || self.snow_cloud_up() {
            PETAL_LOOK_UP_MULT
        } else {
            1.0
        }
    }

    /// 지금 눈사람을 만들 수 있나: 눈이 쌓였고, 내 동물이 넓은 바닥에 서 있고, 다른 볼일이 없음
    pub(super) fn can_build_snowman(&self, world: &World) -> bool {
        if !self.snow.ready
            || self.cfg.flies
            || self.guest.is_some()
            || self.control.is_some()
            || self.home_trip.is_some()
            || self.food.is_some()
            || self.send.is_some()
            || self.inside.is_some()
            || self.hand.is_some()
            || self.housed
        {
            return false;
        }
        let Some(st) = self.standing.filter(|s| matches!(s.owner, Owner::Ground(_))) else { return false };
        world.piece_at(st.owner, self.body.x).is_some_and(|p| p.width() >= 4.0 * (self.cfg.half_w + BALL_REACH * self.cfg.size))
    }

    /// 쉬는 박자가 끝날 때: 가끔 눈사람을 만들기 시작함
    pub(super) fn snowman_rest(&mut self, world: &World) -> Option<State> {
        if !self.can_build_snowman(world) || self.rng.f64() >= SNOWMAN_CHANCE {
            return None;
        }
        let dir = if self.rng.bool() { 1.0 } else { -1.0 };
        self.start_snowman(dir);
        Some(self.state)
    }

    /// dir 쪽으로 눈덩이를 굴리기 시작함
    pub(super) fn start_snowman(&mut self, dir: f64) {
        self.pacing.after_beat = None;
        self.facing = dir;
        self.state = State::Snowman { phase: SnowPhase::Roll, remaining: ROLL_SECS, dir };
    }

    /// 눈사람 단계별 그림 (기린은 구르지 않고 밀며 걸음)
    pub(super) fn snowman_anim(&self, phase: SnowPhase) -> Anim {
        match phase {
            // 기린은 구르지 않는다 — 눈덩이를 밀며 걷는다
            SnowPhase::Roll if self.cfg.special.is_some_and(Special::replaces_roll) => Anim::Walk,
            SnowPhase::Roll => Anim::Roll,
            SnowPhase::Stack => Trick::Stretch.into(),
            SnowPhase::Admire => Trick::Heart.into(),
        }
    }

    /// 눈사람 한 틱: 굴리고 쌓고 감상함. 눈이 녹으면 그만둠
    pub(super) fn update_snowman(&mut self, dt: f64, world: &World, phase: SnowPhase, remaining: f64, dir: f64) {
        if !self.follow_anchor(world) {
            self.start_fall();
            return;
        }
        if !self.snow.ready && phase != SnowPhase::Admire {
            self.state = self.rest_beat();
            return;
        }
        let remaining = remaining - dt;
        let mut dir = dir;
        if phase == SnowPhase::Roll {
            dir = self.push_snowball(dt, world, dir);
        }
        if remaining > 0.0 {
            self.state = State::Snowman { phase, remaining, dir };
            return;
        }
        self.state = match phase {
            SnowPhase::Roll => State::Snowman { phase: SnowPhase::Stack, remaining: STACK_SECS, dir },
            SnowPhase::Stack => {
                self.snow.built = Some((self.body.x + dir * BALL_REACH * self.cfg.size, self.body.y));
                self.snow.ready = false;
                self.speech = Some(Speech::Snowman);
                self.heart_timer = ADMIRE_SECS;
                State::Snowman { phase: SnowPhase::Admire, remaining: ADMIRE_SECS, dir }
            }
            SnowPhase::Admire => self.rest_beat(),
        };
    }

    /// 바닥을 따라 dir 쪽으로 밂 — 눈덩이가 끝에 닿으면 돌아섬. 지금 미는 쪽을 돌려줌
    fn push_snowball(&mut self, dt: f64, world: &World, dir: f64) -> f64 {
        let Some(st) = self.standing else { return dir };
        let x = self.body.x;
        let Some(piece) = world.piece_at(st.owner, x).copied() else { return dir };
        let room = self.cfg.half_w + BALL_REACH * self.cfg.size;
        let (lo, hi) = (piece.x_start + room, piece.x_end - room);
        let new_x = x + dir * self.scaled(ROLL_SPEED) * dt;
        if (dir < 0.0 && new_x < lo) || (dir > 0.0 && new_x > hi) {
            self.facing = -dir;
            return -dir;
        }
        self.standing = Some(Standing { owner: st.owner, rel_x: st.rel_x + (new_x - x) });
        self.body.x = new_x;
        dir
    }
}
