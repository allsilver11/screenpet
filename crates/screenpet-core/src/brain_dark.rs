//! 🐍 볼드모트 (pet.json `dark_lord` + cast 그림 — PetConfig::dark_lord) + 💫 기절 (누구나) — brain.rs의 자식 모듈. 숫자는 wizard.rs.
//!
//! - 🐍 나기니: 한가할 때 가끔 발밑 바닥을 보고 "Serpensortia!" (State::Cast — brain_spell.rs) → 맞는 때 큰 뱀이 스르륵 나타나
//!   부른 자리 둘레 (NAGINI_RANGE)를 오가며 기어 다니다 NAGINI_SECS 뒤 스르륵 사라짐. 뱀은 덮개 창이 그림 (magic_fx → MagicFx::Snake)
//! - 😈 "hehehe": 한가할 때 가끔 (말만 — 하던 일은 그대로). ⚡ Stupefy를 맞힌 뒤에도 반쯤 (brain_spell.rs)
//! - 💫 기절 (State::Faint — 누구나): Stupefy에 맞으면 (be_stunned — 들 수 있는 때만) 쿵 (land 그림) → 누워 (sleep 그림) 머리 위에 💫
//!   (render.thud) FAINT_SECS 뒤 일어남. 디딤판이 사라지면 떨어짐
//!
//! 주사위는 자기 주사위(DarkMood.rng)로 굴림 — 평소 동작과 다른 동물의 주사위 순서는 그대로.

use super::spell_moves::SpellTarget;
use super::*;
use crate::magic::MagicFx;
use crate::wizard::{self as wz, Spell};

/// 🐍 기어 다니는 나기니 (물리 px)
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Snake {
    /// 머리 자리 x
    pub x: f64,
    /// 바닥 y (부른 때의 발밑)
    pub y: f64,
    /// 오가는 가운데 x (부른 자리)
    pub home: f64,
    /// 머리 쪽 (±1)
    pub dir: f64,
    /// 나온 뒤 흐른 시간 · 전체 (초)
    pub t: f64,
    pub len: f64,
}

/// 볼드모트의 기억
#[derive(Debug, Clone)]
pub struct DarkMood {
    rng: fastrand::Rng,
    /// 🐍 이 시각(clock)부터 나기니를 부를 수 있음
    pub(super) nagini_ready_at: f64,
    /// 🐍 지금 기어 다니는 나기니
    pub(super) snake: Option<Snake>,
}

impl DarkMood {
    /// 시드로 난수를 따로 둠. 첫 나기니는 NAGINI_FIRST_SECS 뒤부터
    pub(super) fn new(seed: u64) -> Self {
        DarkMood { rng: fastrand::Rng::with_seed(seed ^ 0x6461_726b), nagini_ready_at: wz::NAGINI_FIRST_SECS, snake: None }
    }
}

impl Pet {
    /// 🐍 지금 기어 다니는 나기니 (다른 펫·테스트가 봄)
    pub fn nagini(&self) -> Option<Snake> {
        self.dark.snake
    }

    /// 💫 기절해 쓰러져 있음
    pub fn is_fainted(&self) -> bool {
        matches!(self.state, State::Faint { .. })
    }

    /// 매 틱 (상태와 상관없이): 나기니가 기어 다니고, 때가 되면 (또는 볼드모트가 떠나면) 사라짐
    pub(super) fn dark_tick(&mut self, dt: f64, world: &World) {
        let Some(mut s) = self.dark.snake else { return };
        s.t += dt;
        if s.t >= s.len || matches!(self.state, State::Away) || self.housed {
            self.dark.snake = None;
            self.dark.nagini_ready_at = self.clock + behavior::range(&mut self.dark.rng, wz::NAGINI_CALM_SECS);
            return;
        }
        let size = self.cfg.size;
        let wa = world.work_area_at(s.home, s.y - 1.0);
        let reach = wz::NAGINI_RANGE * size;
        let half = 0.5 * wz::NAGINI_LEN * size;
        let (lo, hi) = ((s.home - reach).max(wa.left + half), (s.home + reach).min(wa.right - half));
        s.x += s.dir * wz::NAGINI_SPEED * size * dt;
        if (s.dir > 0.0 && s.x >= hi) || (s.dir < 0.0 && s.x <= lo) {
            s.x = s.x.clamp(lo.min(hi), hi.max(lo));
            s.dir = -s.dir;
        }
        self.dark.snake = Some(s);
    }

    /// 매 틱 (주문 뒤): 😈 가끔 "hehehe" (말만), 🐍 가끔 나기니를 부름 (State::Cast). 주문을 시작했으면 true
    pub(super) fn update_dark(&mut self, dt: f64, _world: &World) -> bool {
        if !self.cfg.dark_lord || !self.play_free() || self.romp_interrupted() || self.is_user_away() {
            return false;
        }
        if self.speech.is_none() && self.dark.rng.f64() < wz::CACKLE_RATE * dt {
            self.speech = Some(Speech::Hehehe);
        }
        if self.dark.snake.is_some() || self.clock < self.dark.nagini_ready_at || self.dark.rng.f64() >= wz::NAGINI_RATE * self.power() * dt {
            return false;
        }
        self.call_nagini();
        true
    }

    /// 🐍 발 앞 바닥을 보고 "Serpensortia!" (맞는 때 나기니가 나옴 — summon_nagini)
    pub(super) fn call_nagini(&mut self) {
        let x = self.body.x + self.facing * 0.9 * self.cfg.size;
        self.begin_cast_at(Spell::Serpensortia, SpellTarget::Floor { x }, (x, self.body.y));
    }

    /// 🐍 나기니가 x (발밑 바닥)에 스르륵 나타남 — 볼드모트 쪽을 보지 않고 바깥으로 기어 감
    pub(super) fn summon_nagini(&mut self, x: f64) {
        let len = behavior::range(&mut self.dark.rng, wz::NAGINI_SECS);
        let dir = if (x - self.body.x).abs() > 1.0 { (x - self.body.x).signum() } else { self.facing };
        self.dark.snake = Some(Snake { x, y: self.body.y, home: self.body.x, dir, t: 0.0, len });
    }

    /// 🐍 덮개 창에 그릴 나기니
    pub(super) fn dark_fx(&self, out: &mut Vec<MagicFx>) {
        if let Some(s) = self.dark.snake {
            out.push(MagicFx::Snake { x: s.x, y: s.y, size: wz::NAGINI_LEN * self.cfg.size, dir: s.dir, t: s.t, alpha: wz::nagini_alpha(s.t, s.len) });
        }
    }

    /// 💫 ⚡ Stupefy에 맞음: 들 수 있는 때 (서서 쉬거나 걷거나 자는 중 — 나는 동물은 아님)면 secs초 동안 기절. 쓰러졌으면 true
    pub fn be_stunned(&mut self, secs: f64) -> bool {
        if !self.lift_ok() || self.cfg.flies {
            return false;
        }
        self.pacing.after_beat = None;
        self.clear_hover();
        self.speech = Some(Speech::Dazed);
        self.state = State::Faint { t: 0.0, len: secs };
        true
    }

    /// 💫 기절 한 틱 (누구나): 디딤판을 따라가고, 다 되면 일어나 쉼. 처리했으면 true
    pub(super) fn update_faint(&mut self, dt: f64, world: &World) -> bool {
        let State::Faint { t, len } = self.state else { return false };
        if !self.follow_anchor(world) {
            self.start_fall();
            return true;
        }
        let t = t + dt;
        self.state = if t < len { State::Faint { t, len } } else { self.rest_beat() };
        true
    }
}
