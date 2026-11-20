//! 🦸 슈퍼인간 낮게 날기 (pet.json `super_flier` + SOAR_ANIMS — PetConfig::soars) — brain.rs의 자식 모듈. 숫자는 soar.rs.
//!
//! - ✈️ 걷기·달리기 토막마다 FLY_SHARE로 날기 (soar_pace — 토막 안에서는 그대로): 디딤판은 그대로 따라가되 창을 바닥보다
//!   몸 크기 0.3~0.8만큼 띄움 (soar_lift — window_pos가 뺌). 빠르기는 느긋한 비행 ↔ 돌진 사이를 천천히 오감 (빠르면 fly_fast).
//!   물건을 들고 있으면 날기 토막 대신 💪 들고 날기 (juggle Wander가 soar_carry_pace·떠오름).
//! - 🔄 날다가 가끔 공중제비 (fly_flip, 살짝 더 솟음).
//! - 디딤판 끝 (soar_edge)·디딤판이 사라짐 (soar_fall): 추락 대신 활공 — 낮은 중력으로 멀리 앞으로 내려가 사뿐 착지 (hover, 콰당 없음).
//! - 날기를 마치면 똑바로 서서 (hover) 천천히 내려앉음.
//! - 🚪 친구 화면으로 나가고 들어올 때 날아서 (soar_transit — Exit/Enter 그림 fly, "to the rescue!"), 들어오면 계속 낢. 위·아래 끝은 hover.
//! - 💪 거대한 것도 늘 번쩍 (brain_juggle.rs hero_lifter — 짧게 힘주고 "easy!").
//!
//! 주사위는 자기 주사위(SoarMood.rng)로 굴림 — 평소 동작과 다른 동물의 주사위 순서는 그대로.

use super::*;
use crate::soar;

/// 낮게 날기의 기억
#[derive(Debug, Clone)]
pub struct SoarMood {
    rng: fastrand::Rng,
    /// 이번 이동 토막은 날기
    pub(super) flying: bool,
    /// 마지막으로 본 이동 틱 (clock, 이동 종류 번호, 남은 시간) — 새 토막인지 볼 때
    last: Option<(f64, u8, f64)>,
    /// 다음 이동 토막은 반드시 낢 (친구 화면에서 날아 들어옴 — 테스트도 씀)
    pub(super) force_fly: bool,
    /// 다음 이동 토막은 반드시 걸음 (테스트)
    pub(super) walk_next: bool,
    /// 지금 바닥 위로 뜬 높이 (물리 px)
    pub(super) lift: f64,
    /// 이번 날기 토막의 높이 (몸 크기 몫)
    target: f64,
    /// 지금 빠르기 (걷기 속도의 배 — 🌱 연령대 세기 빼고)
    speed: f64,
    /// 다가가는 빠르기 목표
    speed_goal: f64,
    /// 이 시각(clock)에 새 빠르기 목표
    speed_until: f64,
    /// 🔄 공중제비를 시작한 시각 (clock) — 끝나면 None
    flip_at: Option<f64>,
    /// 활공 중 (공중 그림 fly·낮은 중력)
    pub(super) gliding: bool,
}

impl SoarMood {
    /// 시드로 난수를 따로 둠. 처음 빠르기는 느긋한 비행
    pub(super) fn new(seed: u64) -> Self {
        SoarMood {
            rng: fastrand::Rng::with_seed(seed ^ 0x736f_6172),
            flying: false,
            last: None,
            force_fly: false,
            walk_next: false,
            lift: 0.0,
            target: soar::LIFT.0,
            speed: soar::CRUISE.0,
            speed_goal: soar::CRUISE.0,
            speed_until: 0.0,
            flip_at: None,
            gliding: false,
        }
    }
}

impl Pet {
    /// 지금 나는 빠르기 (px/s — 🌱 soft_power)
    pub(super) fn soar_speed(&self) -> f64 {
        self.cfg.walk_speed * self.soar.speed * self.soft_power()
    }

    /// 지금 날기 토막을 나는 중 (이번 틱의 이동 토막이 날기, 또는 막 날아 들어와 다음 토막을 날 참)
    pub(super) fn soaring_now(&self) -> bool {
        self.cfg.soars
            && matches!(self.state, State::Move { .. })
            && (self.soar.flying && self.soar.last.is_some_and(|(c, ..)| c == self.clock) || self.soar.force_fly)
    }

    /// 나는 그림: 공중제비 중이면 fly_flip, 빠르면 fly_fast, 아니면 fly
    pub(super) fn soar_anim(&self) -> Anim {
        if self.soar.flip_at.is_some() {
            Anim::FlyFlip
        } else if self.soar.speed >= soar::FAST_ANIM {
            Anim::FlyFast
        } else {
            Anim::Fly
        }
    }

    /// 창을 바닥보다 띄우는 높이 (물리 px): 떠 있는 높이 + 공중제비 때 더 솟은 만큼
    pub(super) fn soar_lift(&self) -> f64 {
        if !self.cfg.soars {
            return 0.0;
        }
        let flip = match self.soar.flip_at {
            Some(at) if matches!(self.state, State::Move { .. }) => {
                crate::play::hop_height((self.clock - at).max(0.0), soar::FLIP_SECS, soar::FLIP_RISE * self.cfg.size)
            }
            _ => 0.0,
        };
        self.soar.lift + flip
    }

    /// 떠 있어 쉬기·돌아서기·착지 그림 대신 hover
    pub(super) fn soar_hovering(&self) -> bool {
        self.cfg.soars && self.standing.is_some() && self.soar.lift > soar::HOVER_MIN * self.cfg.size
    }

    /// 활공 그림: 빠르면 fly_fast, 아니면 fly
    pub(super) fn glide_anim(&self) -> Anim {
        if self.body.vx.abs() >= soar::FAST_ANIM * self.cfg.walk_speed {
            Anim::FlyFast
        } else {
            Anim::Fly
        }
    }

    /// 친구 화면으로 날아서 나가거나 들어오는 중 (옆 끝만, 손에 든 것이 없을 때)
    pub(super) fn soar_transit(&self) -> bool {
        self.cfg.soars
            && matches!(self.state, State::Exit { side, .. } | State::Enter { side, .. } if !visit::is_vertical(side))
            && self.carried_object().is_none()
            && self.hoisting().is_none()
    }

    /// 친구 화면을 날아서 오가는 빠르기 (px/s)
    pub(super) fn soar_transit_speed(&self) -> f64 {
        self.cfg.walk_speed * soar::TRANSIT_SPEED * self.soft_power()
    }

    /// 💪 juggle Wander (들고 돌아다니기)의 빠르기 배: 슈퍼인간은 날아서 빠르게
    pub(super) fn soar_carry_pace(&self) -> f64 {
        if self.cfg.soars {
            soar::CARRY_PACE
        } else {
            1.0
        }
    }

    /// 이번 틱에 떠 있어야 할 높이 (물리 px): 날기 토막·날아서 오가기·들고 날기·날다 돌아서기면 그 높이, 아니면 0
    fn soar_goal(&self) -> f64 {
        let aloft = match self.state {
            State::Move { .. } | State::Turn { .. } => self.soar.flying || self.soar.force_fly,
            State::Exit { side, .. } | State::Enter { side, .. } => !visit::is_vertical(side),
            State::Juggle { phase: juggle_moves::JugglePhase::Wander { .. }, .. } => true,
            _ => false,
        };
        if aloft {
            self.soar.target * self.cfg.size
        } else {
            0.0
        }
    }

    /// 매 틱 맨 앞: 떠 있는 높이를 목표 쪽으로 천천히 옮기고 (날 수 없는 상태면 바로 0), 활공·공중제비 기억을 정리
    pub(super) fn soar_tick(&mut self, dt: f64) {
        if !self.cfg.soars {
            return;
        }
        if !matches!(self.state, State::Air { .. }) {
            self.soar.gliding = false;
        }
        if self.soar.flip_at.is_some_and(|at| self.clock - at >= soar::FLIP_SECS) || !matches!(self.state, State::Move { .. }) {
            self.soar.flip_at = None;
        }
        // 떠 있을 수 있는 상태 (나머지 — 들림·먹기·놀이 등 — 는 바로 바닥에)
        let keeps = self.standing.is_some()
            && matches!(
                self.state,
                State::Move { .. } | State::Idle { .. } | State::Turn { .. } | State::Land { .. } | State::Exit { .. } | State::Enter { .. } | State::Juggle { .. }
            );
        if !keeps {
            self.soar.lift = 0.0;
            return;
        }
        let was = self.soar.lift;
        self.soar.lift = crate::physics::step_toward(was, self.soar_goal(), soar::LIFT_RATE * self.cfg.size * dt);
        // 날기를 마치고 사뿐 내려앉음 — 가끔 "touchdown!"
        if was > 0.0 && self.soar.lift == 0.0 && self.speech.is_none() && self.soar.rng.f64() < soar::SOFT_LINE_CHANCE {
            self.speech = Some(Speech::SoftLand);
        }
    }

    /// update_ground 이동 한 틱: 이번 토막이 날기면 그 빠르기 (None이면 평소 걸음). 새 토막이면 날지 정하고, 나는 동안 빠르기·공중제비
    pub(super) fn soar_pace(&mut self, kind: Move, dt: f64) -> Option<f64> {
        if !self.cfg.soars || kind == Move::Roll || self.cfg.hops || self.carried_object().is_some() {
            self.soar.flying = false;
            return None;
        }
        let remaining = match self.state {
            State::Move { remaining, .. } => remaining,
            _ => 0.0,
        };
        let clock = self.clock;
        let same = self.soar.last.is_some_and(|(c, k, r)| c < clock && clock - c <= dt * 1.5 && k == kind as u8 && remaining <= r);
        if !same {
            let m = &mut self.soar;
            m.flying = !m.walk_next && (m.force_fly || m.rng.f64() < soar::FLY_SHARE);
            m.force_fly = false;
            m.walk_next = false;
            if m.flying {
                m.target = behavior::range(&mut m.rng, soar::LIFT);
                // 다른 말풍선 (🆕 새 버전 응원 등)은 덮지 않음
                if m.lift < soar::HOVER_MIN * self.cfg.size && m.rng.f64() < soar::TAKE_OFF_LINE_CHANCE && self.speech.is_none() {
                    self.speech = Some(Speech::TakeOff);
                }
            }
        }
        self.soar.last = Some((clock, kind as u8, remaining));
        if !self.soar.flying {
            return None;
        }
        // 빠르기: 가끔 새 목표 (돌진 · 느긋), 목표로 천천히
        let m = &mut self.soar;
        if clock >= m.speed_until {
            m.speed_goal = if m.rng.f64() < soar::DASH_CHANCE { behavior::range(&mut m.rng, soar::DASH) } else { behavior::range(&mut m.rng, soar::CRUISE) };
            m.speed_until = clock + behavior::range(&mut m.rng, soar::SPEED_SECS);
        }
        m.speed += (m.speed_goal - m.speed) * (soar::SPEED_EASE * dt).min(1.0);
        // 🔄 가끔 공중제비 (충분히 떠 있을 때)
        if self.soar.flip_at.is_none() && self.soar.lift >= soar::LIFT.0 * 0.5 * self.cfg.size && self.soar.rng.f64() < soar::FLIP_RATE * self.power() * dt {
            self.soar.flip_at = Some(clock);
            if self.soar.rng.f64() < soar::FLIP_LINE_CHANCE && self.speech.is_none() {
                self.speech = Some(Speech::AirFlip);
            }
        }
        Some(self.soar_speed())
    }

    /// 디딤판 끝 (Edge)에 닿음: 날고 있으면 대개 활공해 내려가고 아니면 돌아섬, 걷고 있어도 뚝 떨어지지 않고 활공하거나 돌아섬.
    /// 슈퍼인간이 아니거나 들고 있으면 false (평소대로)
    pub(super) fn soar_edge(&mut self) -> bool {
        if !self.cfg.soars || self.carried_object().is_some() || self.hoisting().is_some() {
            return false;
        }
        let chance = if self.soaring_now() { soar::EDGE_GLIDE_CHANCE } else { soar::WALK_GLIDE_CHANCE };
        if self.is_heading_out() || self.soar.rng.f64() < chance {
            self.start_glide();
        } else {
            self.pacing.after_beat = None;
            self.state = State::Turn { remaining: TURN_PAUSE };
        }
        true
    }

    /// 활공 시작: 떠 있던 높이만큼 몸을 올려 두고 (창이 튀지 않게) 앞으로 살짝 솟으며 디딤판을 떠남
    pub(super) fn start_glide(&mut self) {
        self.pacing.after_beat = None;
        self.standing = None;
        self.body.y -= self.soar.lift;
        self.soar.lift = 0.0;
        self.soar.flip_at = None;
        self.soar.flying = false;
        self.body.vx = self.facing * self.cfg.walk_speed * soar::GLIDE_VX * self.soft_power();
        self.body.vy = soar::GLIDE_VY * self.cfg.size;
        self.soar.gliding = true;
        self.state = State::Air { anim: Anim::Fall };
    }

    /// start_fall 끝에서: 슈퍼인간은 디딤판에 서 있다 떨어질 때만 (던져진 게 아니면) 보는 쪽으로 활공.
    /// 벽·창 옆면 등에서 떨어질 땐 그냥 떨어짐 — 활공이 그 벽 쪽으로 날아가 다시 붙고 또 떨어지기를 매 틱 되풀이하던 버그 (0.7.0)
    pub(super) fn soar_fall(&mut self, was_standing: bool) {
        if self.cfg.soars && was_standing && !self.thrown && !self.is_held() {
            self.start_glide();
        }
    }

    /// 활공 중력 (px/s²) — 활공이 아니면 None
    pub(super) fn glide_gravity(&self) -> Option<f64> {
        (self.cfg.soars && self.soar.gliding && !self.thrown).then(|| GRAVITY * soar::GLIDE_GRAVITY / self.soft_power())
    }

    /// update_air에서 한 걸음 뒤: 활공으로 내려가는 빠르기를 GLIDE_MAX_VY까지로
    pub(super) fn cap_glide(&mut self) {
        if self.glide_gravity().is_some() {
            self.body.vy = self.body.vy.min(soar::GLIDE_MAX_VY * self.cfg.size);
        }
    }

    /// land_or_recover에서 내려섬: 활공을 끝냄. 슈퍼인간이면 사뿐 착지 시간 (콰당 없음)
    pub(super) fn soar_landed(&mut self) -> Option<f64> {
        self.soar.gliding = false;
        self.cfg.soars.then_some(soar::SOFT_LAND_SECS)
    }

    /// 친구 화면으로 나가기 시작함: 날아서 "to the rescue!"
    pub(super) fn soar_exit(&mut self) {
        if self.soar_transit() {
            self.soar.target = self.soar.target.max(soar::LIFT.0);
            self.speech = Some(Speech::Rescue);
        }
    }

    /// 친구 화면에서 들어옴: 떠서 들어오고 들어온 뒤 첫 토막도 낢
    pub(super) fn soar_enter(&mut self) {
        if self.cfg.soars {
            self.soar.force_fly = true;
            self.soar.target = soar::ENTER_LIFT;
            self.soar.lift = soar::ENTER_LIFT * self.cfg.size;
        }
    }
}
