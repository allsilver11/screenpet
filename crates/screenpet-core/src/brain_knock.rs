//! 🐱 고양이 툭툭 (v0.6.3, pet.json `knocker` + paw 그림 — 고양이·페르시안) — brain.rs의 자식 모듈. 숫자는 play.rs.
//!
//! 창 윗면에 선 고양이가 같은 윗면에 놓인 작은 물건(쓰레기·똥 — 거대한 것·📌 장난감은 아님)을 처음 보면 KNOCK_CHANCE로
//! 그 물건 뒤(가까운 끝의 반대쪽)로 걸어가 쳐다보고, "Tap… tap…" 앞발로 몇 번 툭 (물건이 조금씩 밀림), 마지막 툭에 끝 너머로
//! 밀어 버림 ("Oops~"). 그러고는 끝까지 걸어가 고개를 내밀고 떨어진 곳을 내려다봄
//! 물건은 앱이 밂 (ObjectPlay::Nudge → Object::nudge — 바닥을 따라 미끄러지다 끝을 넘으면 떨어지고, 떨어지며 동물에 맞을 수 있음).
//! 내 고양이·놀러 온 고양이 모두. 자리 비움·집중·하우스·볼일 중엔 하지 않음 (숨겨 두면 앱이 물건을 보여 주지 않음).

use super::*;
use crate::juggle::ObjectSpot;
use crate::play::{self, ObjectPlay};

/// 툭툭 단계 (State::Knock — t는 이 단계에 들어선 뒤 흐른 시간). dir = 밀어 떨어뜨릴 쪽
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum KnockPhase {
    /// 물건 뒤 x로 걸어감
    Approach { id: u64, x: f64, dir: f64 },
    /// 물건을 빤히 쳐다봄
    Look { id: u64, dir: f64 },
    /// 앞발로 툭 (left = 떨어뜨리기 전 남은 툭 수, 이번 것 포함)
    Tap { id: u64, dir: f64, left: u32, done: bool },
    /// 마지막 툭 — 끝 너머로
    Push { id: u64, dir: f64, done: bool },
    /// 끝까지 걸어가 내려다봄 (looking = 끝에 닿았음)
    Watch { dir: f64, looking: bool },
}

impl Pet {
    /// 툭툭 중이면 그 물건 (테스트·앱 확인용)
    pub fn knocking(&self) -> Option<u64> {
        match self.state {
            State::Knock { phase: KnockPhase::Approach { id, .. } | KnockPhase::Look { id, .. } | KnockPhase::Tap { id, .. } | KnockPhase::Push { id, .. }, .. } => Some(id),
            _ => None,
        }
    }

    /// 툭툭 칠 물건을 살핌: 하던 것이 사라졌으면 그만, 처음 본 물건이면 주사위를 굴림
    pub(super) fn notice_knock(&mut self, world: &World) {
        // 하던 툭툭: 물건이 아직 그 윗면에 있나 (누가 치웠거나 던졌으면 그만)
        if let Some(id) = self.knocking() {
            let owner = self.standing.map(|s| s.owner);
            let spot = self.play.spots.iter().find(|s| s.id == id && Some(s.owner) == owner).copied();
            match (spot, self.state) {
                (None, State::Knock { phase: KnockPhase::Push { done: true, .. }, .. }) => {}
                (None, _) => self.end_knock(false),
                (Some(s), State::Knock { phase: KnockPhase::Approach { id, dir, .. }, t }) => match self.knock_stand(&s, dir, world) {
                    Some(x) => self.state = State::Knock { phase: KnockPhase::Approach { id, x, dir }, t },
                    None => self.end_knock(false),
                },
                _ => {}
            }
            return;
        }
        if self.clock < self.play.knock_ready_at || !self.play_free() {
            return;
        }
        let Some(st) = self.standing.filter(|s| matches!(s.owner, Owner::Window(_))) else { return };
        let (x, y, reach) = (self.body.x, self.body.y, play::KNOCK_REACH * self.cfg.size);
        let near = |s: &&ObjectSpot| !s.giant && !s.local && s.owner == st.owner && (s.y - y).abs() <= crate::juggle::SAME_LEVEL_PX && (s.x - x).abs() <= reach;
        let Some(spot) = self.play.spots.iter().filter(near).min_by(|a, b| (a.x - x).abs().total_cmp(&(b.x - x).abs())).copied() else { return };
        if !self.first_look(spot.id) || self.play.rng.f64() >= play::KNOCK_CHANCE {
            return;
        }
        self.start_knock(&spot, world);
    }

    /// 물건(spot)을 툭툭하러 감 — 가까운 끝 쪽으로 밀 자리에 설 수 없으면 반대쪽으로, 그것도 안 되면 false
    pub(super) fn start_knock(&mut self, spot: &ObjectSpot, world: &World) -> bool {
        let Some(piece) = world.piece_at(spot.owner, spot.x).copied() else { return false };
        let near = play::push_dir(spot.x, piece.x_start, piece.x_end);
        for dir in [near, -near] {
            if let Some(x) = self.knock_stand(spot, dir, world) {
                self.pacing.after_beat = None;
                self.clear_hover();
                self.poop.stepping_off();
                self.state = State::Knock { phase: KnockPhase::Approach { id: spot.id, x, dir }, t: 0.0 };
                return true;
            }
        }
        false
    }

    /// dir 쪽으로 밀려면 설 자리 (물건 뒤 — 앞발이 닿는 곳). 같은 조각 위에 설 수 없으면 None
    fn knock_stand(&self, spot: &ObjectSpot, dir: f64, world: &World) -> Option<f64> {
        let st = self.standing?;
        let piece = world.piece_at(st.owner, self.body.x)?;
        if !(piece.x_start..=piece.x_end).contains(&spot.x) {
            return None;
        }
        let x = spot.x - dir * (spot.half_w + play::PAW_GAP * self.cfg.size);
        (piece.x_start + self.cfg.half_w <= x && x <= piece.x_end - self.cfg.half_w).then_some(x)
    }

    /// 그만둠 (did = 떨어뜨렸음 — 쉬는 때를 길게)
    fn end_knock(&mut self, did: bool) {
        let calm = if did { behavior::range(&mut self.play.rng, play::KNOCK_CALM_SECS) } else { 10.0 };
        self.play.knock_ready_at = self.clock + calm;
        self.state = self.rest_beat();
    }

    /// 툭툭 한 틱: 다가가 쳐다보고, 톡톡 치다가 밀어 떨어뜨림
    pub(super) fn update_knock(&mut self, dt: f64, world: &World, phase: KnockPhase, t: f64) {
        if self.romp_interrupted() {
            self.end_knock(false);
            return;
        }
        if !self.follow_anchor(world) {
            self.play.knock_ready_at = self.clock + 10.0;
            self.start_fall();
            return;
        }
        let t = t + dt;
        let next = |phase| State::Knock { phase, t };
        match phase {
            KnockPhase::Approach { id, x, dir } => {
                if t > play::APPROACH_SECS {
                    self.end_knock(false);
                    return;
                }
                let dx = x - self.body.x;
                let step = self.walk_speed() * dt;
                if dx.abs() > 1.0 {
                    self.facing = dx.signum();
                    if self.juggle_walk(world, dx.signum(), step.min(dx.abs())).is_none() {
                        return;
                    }
                    self.state = next(phase);
                    return;
                }
                self.facing = dir;
                self.state = State::Knock { phase: KnockPhase::Look { id, dir }, t: 0.0 };
            }
            KnockPhase::Look { id, dir } => {
                self.state = if t < play::LOOK_SECS {
                    next(phase)
                } else {
                    let taps = self.play.rng.u32(play::TAPS.0..=play::TAPS.1);
                    self.speech = Some(Speech::Tap);
                    State::Knock { phase: KnockPhase::Tap { id, dir, left: taps, done: false }, t: 0.0 }
                };
            }
            KnockPhase::Tap { id, dir, left, done } => {
                let done = done || {
                    let fire = t >= play::TAP_AT;
                    if fire {
                        self.play.event = Some(ObjectPlay::Nudge { id, dir, reach: play::TAP_REACH * self.cfg.size });
                    }
                    fire
                };
                self.state = if t < play::TAP_SECS {
                    State::Knock { phase: KnockPhase::Tap { id, dir, left, done }, t }
                } else if left > 1 {
                    State::Knock { phase: KnockPhase::Tap { id, dir, left: left - 1, done: false }, t: 0.0 }
                } else {
                    State::Knock { phase: KnockPhase::Push { id, dir, done: false }, t: 0.0 }
                };
            }
            KnockPhase::Push { id, dir, done } => {
                let done = done || {
                    let fire = t >= play::TAP_AT;
                    if fire {
                        let reach = self.push_reach(id, dir, world);
                        self.play.event = Some(ObjectPlay::Nudge { id, dir, reach });
                        self.speech = Some(Speech::Oops);
                    }
                    fire
                };
                self.state = if t < play::TAP_SECS {
                    State::Knock { phase: KnockPhase::Push { id, dir, done }, t }
                } else {
                    State::Knock { phase: KnockPhase::Watch { dir, looking: false }, t: 0.0 }
                };
            }
            KnockPhase::Watch { dir, looking } => {
                if !looking {
                    // 끝까지 걸어가 고개를 내민다 (몸 가운데는 윗면 안 — 끝에서 몸 조금 안쪽)
                    let piece = self.standing.and_then(|st| world.piece_at(st.owner, self.body.x)).copied();
                    let edge = piece.map_or(self.body.x, |p| if dir > 0.0 { p.x_end } else { p.x_start });
                    let goal = edge - dir * 0.15 * self.cfg.size;
                    let dx = goal - self.body.x;
                    if dx * dir > 1.0 && t < play::WATCH_WALK_SECS {
                        let step = (self.walk_speed() * dt).min(dx.abs());
                        if let Some(st) = self.standing {
                            self.body.x += dir * step;
                            self.standing = Some(Standing { owner: st.owner, rel_x: st.rel_x + dir * step });
                        }
                        self.state = next(phase);
                    } else {
                        self.state = State::Knock { phase: KnockPhase::Watch { dir, looking: true }, t: 0.0 };
                    }
                    self.facing = dir;
                    return;
                }
                if t < play::WATCH_SECS {
                    self.state = next(phase);
                } else {
                    self.end_knock(true);
                }
            }
        }
    }

    /// 마지막 밀기: 물건이 끝을 넘어 PUSH_PAST만큼 더 가게 (물리 px)
    fn push_reach(&self, id: u64, dir: f64, world: &World) -> f64 {
        let spot = self.play.spots.iter().find(|s| s.id == id);
        let past = play::PUSH_PAST * self.cfg.size;
        match spot.and_then(|s| world.piece_at(s.owner, s.x).map(|p| (s.x, p))) {
            Some((x, p)) => (if dir > 0.0 { p.x_end - x } else { x - p.x_start }).max(0.0) + past,
            None => past,
        }
    }

    /// 툭툭 단계별 그림
    pub(super) fn knock_anim(&self, phase: KnockPhase) -> Anim {
        match phase {
            KnockPhase::Approach { .. } | KnockPhase::Watch { looking: false, .. } => Anim::Walk,
            KnockPhase::Look { .. } | KnockPhase::Watch { looking: true, .. } => Anim::Look,
            KnockPhase::Tap { .. } | KnockPhase::Push { .. } => Anim::Paw,
        }
    }
}
