//! 💩 똥 싸기와 똥 앞에서의 반응 — brain.rs의 자식 모듈 (비공개 필드에 접근). 숫자와 까닭은 poop.rs.
//!
//! - 싸기 (`try_poop`): 앱이 왼쪽 버튼을 새로 누른 틱에만 부른다. 누른 자리는 받지 않는다. 쉬거나 걷는 중에만, 싸고 나면 10~20분 쉰다.
//! - 반응 (`notice_poops`): 앱이 update 전에 바닥의 똥 자리를 넘긴다. 걷는 동물이 같은 디딤판의 똥에 다가가면 한 번만 정한다:
//!   대개 "ew!" 하고 돌아서거나 뛰어넘고, 가끔 못 보고 밟아 꽈당 (State::Slip). 같은 똥엔 SAME_POOP_SECS 동안 다시 반응하지 않는다.
//! - 조작 모드·들림·먹이·집 가기·보내기·잠·집중·집이 그리운 손님은 걷는 State::Move가 아니거나 여기서 걸러진다.

use super::*;
use crate::poop::{self, PoopSpot};

/// 꽈당: 미끄러지는 동안 Fall, 그다음 Land, 끝에 Annoyed로 투덜 (남은 시간 기준)
const SLIP_SECS: f64 = 1.7;
const SLIP_LAND_FROM: f64 = 1.0;
const SLIP_GRUMPY_FROM: f64 = 0.75;
/// 미끄러지는 처음 속도 (크기 128 기준 px/s, 점점 느려진다)
const SLIP_SPEED: f64 = 170.0;
/// "ew!" 하고 잠깐 멈췄다가 돌아서는 시간
const EW_PAUSE_SECS: f64 = 0.5;
/// 똥이 떨어지기 시작하는 자리: 발에서 위로, 몸 가운데에서 뒤로 (몸 크기 배)
const DROP_RISE: f64 = 0.3;
const DROP_BEHIND: f64 = 0.22;

/// 똥에 관한 기억
#[derive(Debug, Clone, Default)]
pub struct PoopMood {
    /// 이 시각(clock)부터 다시 쌀 수 있다
    pub(super) ready_at: f64,
    /// 이 시각까지는 어떤 똥에도 반응하지 않는다
    pub(super) calm_until: f64,
    /// 이미 반응한 똥과 다시 반응할 수 있는 시각
    pub(super) seen: Vec<(u64, f64)>,
    /// 못 보고 밟으러 가는 똥
    pub(super) stepping: Option<u64>,
}

impl PoopMood {
    /// 다른 일을 시작했다: 밟으러 가던 것은 잊는다
    pub(super) fn stepping_off(&mut self) {
        self.stepping = None;
    }
}

impl Pet {
    /// 지금 쌀 수 있는 모습: 디딤판 위(나는 동물은 떠서도) 쉬거나 걷는 중이고, 볼일·집중·자리 비움이 없다
    fn can_poop(&self) -> bool {
        matches!(self.state, State::Move { .. } | State::Idle { .. } | State::Turn { .. } | State::Trick { .. })
            && (self.standing.is_some() || self.cfg.flies)
            && self.inside.is_none()
            && !self.housed
            && !self.presence_quiet()
            && !self.is_eating()
            && !self.is_heading_out()
            && self.home_trip.is_none()
            && self.send.is_none()
            && !self.farewell_pending
    }

    /// 왼쪽 버튼을 새로 눌렀다 (누른 자리는 모른다). 쌌으면 똥이 떨어지기 시작할 자리 — 나는 동물은 공중에서 떨어뜨린다
    pub fn try_poop(&mut self) -> Option<(f64, f64)> {
        if self.clock < self.poop.ready_at || !self.can_poop() || self.rng.f64() >= poop::CLICK_CHANCE {
            return None;
        }
        self.poop.ready_at = self.clock + behavior::range(&mut self.rng, poop::COOLDOWN_SECS);
        Some((self.body.x - self.facing * DROP_BEHIND * self.cfg.size, self.body.y - DROP_RISE * self.cfg.size))
    }

    /// 똥을 알아챌 수 있는 때: 스스로 걷거나 달리는 동물만 — 볼일이 있으면 가던 길을 간다
    fn walks_past_poop(&self) -> bool {
        matches!(self.state, State::Move { kind: Move::Walk | Move::Run, .. })
            && !self.cfg.flies
            && self.standing.is_some()
            && self.inside.is_none()
            && !self.has_errand()
            && !self.presence_quiet()
            && !self.is_heading_out()
            && !self.tickling
    }

    /// 매 틱 update 전에: 바닥에 놓인 똥 자리 (없으면 할 일이 없다)
    pub fn notice_poops(&mut self, spots: &[PoopSpot], world: &World) {
        if !self.walks_past_poop() {
            self.poop.stepping = None;
            return;
        }
        if let Some(id) = self.poop.stepping {
            self.step_toward(spots, id);
            return;
        }
        if self.clock < self.poop.calm_until {
            return;
        }
        let (Some(st), reach, clock) = (self.standing, poop::LOOK_AHEAD * self.cfg.size, self.clock) else { return };
        let (x, y, facing) = (self.body.x, self.body.y, self.facing);
        let seen = &self.poop.seen;
        let ahead_of = |s: &&PoopSpot| {
            let ahead = (s.x - x) * facing;
            s.owner == st.owner && (s.y - y).abs() <= poop::SAME_LEVEL_PX && ahead > 0.0 && ahead <= reach && !seen.iter().any(|&(id, until)| id == s.id && clock < until)
        };
        let Some(spot) = spots.iter().filter(ahead_of).min_by(|a, b| ((a.x - x) * facing).total_cmp(&((b.x - x) * facing))).copied() else { return };
        self.poop.seen.retain(|&(_, until)| clock < until);
        self.poop.seen.push((spot.id, clock + poop::SAME_POOP_SECS));
        self.poop.calm_until = clock + poop::CALM_SECS;
        if self.rng.f64() < poop::SLIP_CHANCE {
            self.keep_walking_onto(&spot);
        } else {
            self.avoid_poop(&spot, world);
        }
    }

    /// 못 봤다: 그대로 밟을 때까지 걷는다 (이번 걷기가 그 전에 끝나지 않게 늘린다)
    fn keep_walking_onto(&mut self, spot: &PoopSpot) {
        self.poop.stepping = Some(spot.id);
        if let State::Move { kind, remaining } = self.state {
            let speed = self.cfg.walk_speed * kind.speed_mult();
            let need = (spot.x - self.body.x).abs() / speed.max(1.0) + 0.5;
            self.state = State::Move { kind, remaining: remaining.max(need) };
        }
    }

    fn step_toward(&mut self, spots: &[PoopSpot], id: u64) {
        let on_surface = |s: &&PoopSpot| s.id == id && self.standing.is_some_and(|st| st.owner == s.owner);
        let Some(spot) = spots.iter().find(on_surface) else {
            // 누가 치웠거나 던졌다
            self.poop.stepping = None;
            return;
        };
        if (spot.x - self.body.x) * self.facing <= poop::STEP_REACH * self.cfg.size {
            self.poop.stepping = None;
            self.pacing.after_beat = None;
            self.clear_hover();
            self.speech = Some(Speech::Oof);
            self.state = State::Slip { remaining: SLIP_SECS, dir: self.facing };
        }
    }

    /// "ew!": 넘어갈 자리가 있으면 가끔 뛰어넘고, 아니면 돌아선다
    fn avoid_poop(&mut self, spot: &PoopSpot, world: &World) {
        self.speech = Some(Speech::Ew);
        self.pacing.after_beat = None;
        let land_x = spot.x + self.facing * (self.cfg.half_w + poop::HOP_CLEAR * self.cfg.size);
        let room = self.standing.and_then(|st| world.piece_at(st.owner, self.body.x)).is_some_and(|p| p.x_start + self.cfg.half_w <= land_x && land_x <= p.x_end - self.cfg.half_w);
        if room && self.rng.f64() < poop::HOP_OVER_CHANCE {
            self.start_hop(self.facing);
            // 다시 같은 높이로 내려올 때까지 떠 있는 동안 land_x까지 간다
            let air = 2.0 * -HOP_VY / GRAVITY;
            self.body.vx = self.facing * self.scaled(HOP_VX).max((land_x - self.body.x).abs() / air);
        } else {
            self.state = State::Turn { remaining: EW_PAUSE_SECS };
        }
    }

    /// 꽈당: 처음엔 미끄러지고 (디딤판 끝에서 멈춘다), 그 뒤 제자리에서 일어나 투덜댄다
    pub(super) fn update_slip(&mut self, dt: f64, world: &World, remaining: f64, dir: f64) {
        let fade = ((remaining - SLIP_LAND_FROM) / (SLIP_SECS - SLIP_LAND_FROM)).clamp(0.0, 1.0);
        if !self.skid(dt, world, dir, self.scaled(SLIP_SPEED) * fade) {
            return;
        }
        let remaining = remaining - dt;
        self.state = if remaining > 0.0 { State::Slip { remaining, dir } } else { self.rest_beat() };
    }

    pub(super) fn slip_anim(&self, remaining: f64) -> Anim {
        if remaining > SLIP_LAND_FROM {
            Anim::Fall
        } else if remaining > SLIP_GRUMPY_FROM {
            Anim::Land
        } else {
            Anim::Annoyed
        }
    }
}
