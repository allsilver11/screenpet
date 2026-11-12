//! 🐿️ 다람쥐 볼 빵빵 (v0.6.3, pet.json `stasher` + cheeks_walk·cheeks_idle·dig 그림 — 다람쥐) — brain.rs의 자식 모듈. 숫자는 play.rs.
//!
//! 내 다람쥐(손님은 아니다)가 같은 디딤판의 작은 물건(쓰레기·똥 — 거대한 것·📌 장난감은 아니다)을 처음 보면 STASH_CHANCE로
//! 달려가 "Nom nom~" 볼에 쏙 넣는다 (앱이 그 물건을 뿅 치운다 — ObjectPlay::Gulp). 가까이 또 있으면 한 번에 MAX_CHEEKS개까지 더 넣고,
//! 볼이 빵빵한 채(cheeks_walk·cheeks_idle) 조금 걸어가 땅을 파고(dig) 묻는다 — 그러면 볼이 원래대로.
//! 볼에 든 채 잡히거나 다른 일이 끼면, 다시 한가해졌을 때 마저 묻으러 간다.
//! 하우스는 동물이 하우스 안에 있을 때만 화면에 있으므로 (밖에서 노는 동안엔 집이 없다) 늘 땅에 묻는다.

use super::*;
use crate::juggle::ObjectSpot;
use crate::play::{self, ObjectPlay};

/// 볼 빵빵 단계 (State::Stash — t는 이 단계에 들어선 뒤 흐른 시간)
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum StashPhase {
    /// 물건 옆 x로 달려간다
    Approach { id: u64, x: f64, obj_x: f64 },
    /// 냠 — 볼에 쏙 (gulped = 이미 넣었다)
    Stuff { id: u64, gulped: bool },
    /// 볼이 빵빵한 채 묻을 곳으로 걸어간다
    Carry { secs: f64 },
    /// 땅을 파고 묻는다
    Dig,
}

impl Pet {
    /// 🐿️ 볼에 든 물건 수
    pub fn cheeks(&self) -> u8 {
        self.play.cheeks
    }

    pub(super) fn notice_stash(&mut self, world: &World) {
        if let State::Stash { phase: StashPhase::Approach { id, .. }, t } = self.state {
            let owner = self.standing.map(|s| s.owner);
            let spot = self.play.spots.iter().find(|s| s.id == id && Some(s.owner) == owner).copied();
            match spot.and_then(|s| self.approach_x(&s, world).map(|x| (x, s))) {
                Some((x, s)) => self.state = State::Stash { phase: StashPhase::Approach { id, x, obj_x: s.x }, t },
                // 누가 치웠다: 볼에 든 게 있으면 묻으러, 없으면 그만
                None => self.after_gulp(world),
            }
            return;
        }
        if self.guest.is_some() || self.clock < self.play.stash_ready_at || self.play.cheeks > 0 || !self.play_free() {
            return;
        }
        let Some(spot) = self.stash_pick(world, true) else { return };
        if !self.first_look(spot.id) || self.play.rng.f64() >= play::STASH_CHANCE {
            return;
        }
        self.start_stash(&spot, world);
    }

    /// 볼에 넣을 만한 가장 가까운 물건 (fresh면 아직 주사위를 굴리지 않은 것만)
    fn stash_pick(&self, world: &World, fresh: bool) -> Option<ObjectSpot> {
        let st = self.standing?;
        let (x, y, reach, clock) = (self.body.x, self.body.y, play::STASH_REACH * self.cfg.size, self.clock);
        let seen = &self.play.seen;
        self.play
            .spots
            .iter()
            .filter(|s| !s.giant && !s.local && s.owner == st.owner && (s.y - y).abs() <= crate::juggle::SAME_LEVEL_PX && (s.x - x).abs() <= reach)
            .filter(|s| !fresh || !seen.iter().any(|&(id, until)| id == s.id && clock < until))
            .filter(|s| self.approach_x(s, world).is_some())
            .min_by(|a, b| (a.x - x).abs().total_cmp(&(b.x - x).abs()))
            .copied()
    }

    /// 물건(spot)을 볼에 넣으러 달려간다
    pub(super) fn start_stash(&mut self, spot: &ObjectSpot, world: &World) -> bool {
        let Some(x) = self.approach_x(spot, world) else { return false };
        self.pacing.after_beat = None;
        self.clear_hover();
        self.poop.stepping_off();
        self.state = State::Stash { phase: StashPhase::Approach { id: spot.id, x, obj_x: spot.x }, t: 0.0 };
        true
    }

    /// 하나 넣은 뒤 (또는 가던 물건이 사라졌다): 더 넣을 수 있고 가까이 또 있으면 그것을, 볼에 든 게 있으면 묻으러, 아니면 그만
    fn after_gulp(&mut self, world: &World) {
        if self.play.cheeks < play::MAX_CHEEKS {
            if let Some(spot) = self.stash_pick(world, false) {
                if self.start_stash(&spot, world) {
                    return;
                }
            }
        }
        if self.play.cheeks > 0 {
            self.start_bury();
        } else {
            self.play.stash_ready_at = self.clock + 10.0;
            self.state = self.rest_beat();
        }
    }

    fn start_bury(&mut self) {
        self.pacing.after_beat = None;
        if self.play.rng.bool() {
            self.facing = -self.facing;
        }
        let secs = behavior::range(&mut self.play.rng, play::BURY_WALK_SECS);
        self.state = State::Stash { phase: StashPhase::Carry { secs }, t: 0.0 };
    }

    /// 볼에 든 채 다른 일이 끼었다가 한가해졌다: 마저 묻으러 간다
    pub(super) fn resume_stash(&mut self) -> bool {
        if self.play.cheeks == 0 || !self.cfg.stashes || !self.play_free() {
            return false;
        }
        self.start_bury();
        true
    }

    pub(super) fn update_stash(&mut self, dt: f64, world: &World, phase: StashPhase, t: f64) {
        if self.romp_interrupted() {
            // 볼에 든 것은 그대로 — 나중에 마저 묻는다
            self.state = self.idle_state();
            return;
        }
        if !self.follow_anchor(world) {
            self.start_fall();
            return;
        }
        let t = t + dt;
        let next = |phase| State::Stash { phase, t };
        match phase {
            StashPhase::Approach { id, x, obj_x } => {
                if t > play::APPROACH_SECS {
                    self.after_gulp(world);
                    return;
                }
                let dx = x - self.body.x;
                if dx.abs() > 1.0 {
                    let step = (self.cfg.walk_speed * Move::Run.speed_mult() * dt).min(dx.abs());
                    self.facing = dx.signum();
                    if self.juggle_walk(world, dx.signum(), step).is_none() {
                        return;
                    }
                    self.state = next(phase);
                    return;
                }
                if (obj_x - self.body.x).abs() > 1.0 {
                    self.facing = (obj_x - self.body.x).signum();
                }
                self.state = State::Stash { phase: StashPhase::Stuff { id, gulped: false }, t: 0.0 };
            }
            StashPhase::Stuff { id, gulped } => {
                let gulped = gulped || {
                    let now = t >= play::GULP_AT;
                    if now {
                        self.play.event = Some(ObjectPlay::Gulp { id });
                        self.play.cheeks += 1;
                        if self.speech.is_none() {
                            self.speech = Some(Speech::Nom);
                        }
                    }
                    now
                };
                if t < play::STUFF_SECS {
                    self.state = State::Stash { phase: StashPhase::Stuff { id, gulped }, t };
                } else {
                    self.after_gulp(world);
                }
            }
            StashPhase::Carry { secs } => {
                let Some(blocked) = self.juggle_walk(world, self.facing, self.walk_speed() * dt) else { return };
                if blocked {
                    self.facing = -self.facing;
                }
                self.state = if t < secs { next(phase) } else { State::Stash { phase: StashPhase::Dig, t: 0.0 } };
            }
            StashPhase::Dig => {
                if t < play::DIG_SECS {
                    self.state = next(phase);
                    return;
                }
                // 묻었다: 볼이 원래대로
                self.play.cheeks = 0;
                self.play.stash_ready_at = self.clock + behavior::range(&mut self.play.rng, play::STASH_CALM_SECS);
                self.state = self.rest_beat();
            }
        }
    }

    pub(super) fn stash_anim(&self, phase: StashPhase) -> Anim {
        match phase {
            StashPhase::Approach { .. } => Anim::Run,
            StashPhase::Stuff { .. } => Anim::Eat,
            StashPhase::Carry { .. } => Anim::Walk,
            StashPhase::Dig => Anim::Dig,
        }
    }
}
