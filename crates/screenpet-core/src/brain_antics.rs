//! 흐물이 더 놀기 (v0.6.3, pet.json에 ANTICS_ANIMS가 다 있는 흐물이 — PetConfig::antics) — brain.rs의 자식 모듈.
//! 하는 중인 상태를 이어 가고, 새로 시작할 것을 고름. 하나하나는 brain_swing·hoist·hide·dance·rally·doodle·typing.rs.
//!
//! 주사위는 놀이마다 제 주사위로 굴림 — 평소 동작(Pet::rng)과 다른 동물의 주사위 순서는 그대로.

use super::*;

impl Pet {
    /// update에서 (자리 비움 뒤, 커서 장난 전). 이번 틱을 여기서 처리했으면 true
    pub(super) fn update_antics(&mut self, dt: f64, world: &World) -> bool {
        // 들려 있는 동물은 흐물이가 아니어도 여기서 (자리는 hoist_moves::sync_lifts가 옮긴다)
        if let State::Lifted { by } = self.state {
            self.update_lifted(dt, world, by);
            return true;
        }
        if !self.cfg.antics {
            return false;
        }
        self.track_swing_cursor(dt);
        self.track_typing(dt);
        match self.state {
            State::Swing { phase, remaining } => self.update_swing(dt, world, phase, remaining),
            State::Hoist { phase, t } => self.update_hoist(dt, world, phase, t),
            State::Hide { phase, remaining } => self.update_hide(dt, world, phase, remaining),
            State::Dance { remaining } => self.update_dance(dt, world, remaining),
            State::Doodle { remaining } => self.update_doodle(dt, world, remaining),
            State::TypeCheer { remaining } => self.update_type_cheer(dt, world, remaining),
            _ => {
                return self.maybe_dance_now()
                    || self.maybe_swing(world)
                    || self.maybe_type_cheer()
                    || self.maybe_hoist(world)
                    || self.maybe_hide(dt, world)
                    || self.maybe_doodle(dt, world);
            }
        }
        true
    }

    /// 새 놀이를 시작할 수 있음: 디딤판 위에서 스스로 걷거나 쉬는 중, 볼일·잡힘·조용한 때·떠나는 길·든 것 없음
    pub(super) fn antics_free(&self) -> bool {
        matches!(self.state, State::Move { .. } | State::Idle { .. } | State::Turn { .. })
            && self.standing.is_some()
            && !self.cfg.flies
            && self.inside.is_none()
            && !self.housed
            && !self.has_errand()
            && !self.presence_quiet()
            && !self.is_heading_out()
            && !self.thrown
            && !self.tickling
            && !self.rider_on
            && self.juggle.carry.is_none()
    }

    /// 하던 놀이를 그만둬야 함 (사용자 볼일·자리 비움·집중·하우스·조작)
    pub(super) fn antics_interrupted(&self) -> bool {
        self.housed || self.has_errand() || self.presence_quiet() || self.inside.is_some()
    }
}
