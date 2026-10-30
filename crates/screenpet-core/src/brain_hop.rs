//! 폴짝 뛰는 동물(개구리, pet.json `locomotion: hop`)과 가끔 깡총거리는 동물(토끼, pet.json `hop_share`)의 Pet 동작 — brain.rs의 자식 모듈 (비공개 필드에 접근).
//! 박자·포물선·목표 계산은 hop.rs.
//!
//! - 걷기·달리기(State::Move)는 디딤판 위 걷기 그대로지만, 떠 있을 때만 앞으로 가고(hop_pace) 그림만 살짝 띄운다
//!   (hop_lift — 발 위치·디딤판은 그대로라 창 끝·모니터 경계·먹이·집 가기 규칙이 다른 동물과 같다).
//! - 벽 타기(Climb·WallClimb·InnerClimb)는 같은 기계를 쓰되 폴짝 오르고 매달리기를 되풀이한다 (climb_pace).
//! - 토끼는 걷기·달리기 토막마다 hop_share 확률로 같은 걸음을 낮은 깡총 박자(Gait::bunny)로 간다 — 벽·큰 점프는 없다.
//! - 가끔 닿는 창 윗면으로 크게 뛴다 (State::Leap: 웅크림 → 포물선 → 그 창 윗면에 내려섬).

use super::*;
use crate::hop::{self, Gait};

impl Pet {
    /// 이동 방식의 폴짝 박자 (폴짝 동물의 걷기·달리기만 — 구르기는 그대로 구른다).
    /// 가끔 깡총거리는 동물(토끼)은 이번 틱의 이동 토막이 깡총일 때만 낮은 깡총 박자
    fn gait(&self, kind: Move) -> Option<Gait> {
        if kind == Move::Roll {
            return None;
        }
        let (speed, run) = (self.cfg.walk_speed * kind.speed_mult(), kind == Move::Run);
        if self.cfg.hops {
            Some(Gait::new(self.cfg.size, speed, run))
        } else if self.bunny_hopping() {
            Some(Gait::bunny(self.cfg.size, speed, run))
        } else {
            None
        }
    }

    /// 깡총 걸음 중 (토끼의 이번 틱 State::Move 토막이 깡총) — 개구리의 큰 점프·벽 폴짝 오르기는 없다
    pub(super) fn bunny_hopping(&self) -> bool {
        !self.cfg.hops && matches!(self.state, State::Move { .. }) && self.bunny.at(self.clock)
    }

    /// update_ground 이동의 이번 틱 속도 배율 — 떠 있을 때만 앞으로 간다 (평균은 그대로). 폴짝 동물이 아니면 1
    pub(super) fn hop_pace(&mut self, kind: Move, dt: f64) -> f64 {
        if !self.cfg.hops && self.cfg.hop_share > 0.0 && kind != Move::Roll {
            // 걷기·달리기 한 토막마다 hop_share 확률로 깡총 (토막 안에서는 그대로)
            let remaining = match self.state {
                State::Move { remaining, .. } => remaining,
                _ => 0.0,
            };
            let (share, rng) = (self.cfg.hop_share, &mut self.rng);
            self.bunny.tick(self.clock, dt, kind as u8, remaining, || rng.f64() < share);
        }
        let Some(g) = self.gait(kind) else { return 1.0 };
        let t = self.hop_beat.tick(self.clock, dt, g.cycle());
        g.pace(t)
    }

    /// 지금 폴짝 높이 (px) — 창 위치(그림)만 띄운다
    pub(super) fn hop_lift(&self) -> f64 {
        let kind = match self.state {
            State::Move { kind, .. } => kind,
            // 술래 놀이도 폴짝 달리기 (개구리만 — 토끼는 그냥 달린다)
            State::Chase { .. } | State::Flee { .. } if self.cfg.hops => Move::Run,
            _ => return 0.0,
        };
        match (self.gait(kind), self.hop_beat.at(self.clock)) {
            (Some(g), Some(t)) => g.lift(t),
            _ => 0.0,
        }
    }

    /// 폴짝 걸음 그림: 떠 있으면 hop, 내려선 직후 land, 웅크림이 길면 앉기(idle)
    pub(super) fn hop_anim(&self, kind: Move) -> Anim {
        let Some(g) = self.gait(kind) else { return kind.into() };
        let t = self.hop_beat.at(self.clock).unwrap_or(0.0);
        if g.airborne(t) {
            Anim::Hop
        } else if t - g.air < hop::LAND_SHOW_SECS {
            Anim::Land
        } else {
            Anim::Idle
        }
    }

    /// 벽 오르기 이번 틱 속도 배율 — 폴짝 오를 때만 오르고 매달려 쉰다 (평균 1). 폴짝 동물이 아니면 1
    pub(super) fn climb_pace(&mut self, dt: f64) -> f64 {
        if !self.cfg.hops {
            return 1.0;
        }
        hop::climb_pace(self.hop_beat.tick(self.clock, dt, hop::climb_cycle()))
    }

    /// move_state: 닿는 창 윗면이 있으면 가끔(BIG_JUMP_CHANCE) 그쪽으로 큰 점프 (창 안·집이 그리운 손님은 안 한다)
    pub(super) fn big_jump_state(&mut self, world: &World) -> Option<State> {
        if !self.cfg.hops || self.inside.is_some() || self.is_heading_out() {
            return None;
        }
        let st = self.standing?;
        let targets = hop::jump_targets(&world.surfaces, st.owner, self.body.x, self.body.y, self.cfg.size, self.cfg.half_w);
        if targets.is_empty() || self.rng.f64() >= hop::BIG_JUMP_CHANCE {
            return None;
        }
        let target = targets[self.rng.usize(..targets.len())];
        let (ax, _) = world.anchor(target.owner)?;
        let dx = target.x - self.body.x;
        if dx != 0.0 {
            self.facing = dx.signum();
        }
        Some(State::Leap { crouch: hop::BIG_JUMP_CROUCH_SECS, remaining: 0.0, owner: target.owner, rel_x: target.x - ax })
    }

    /// 큰 점프: 웅크리는 동안은 디딤판을 따라가고, 다 웅크리면 그때 목표 자리로 포물선을 정해 날아간다
    pub(super) fn update_leap(&mut self, dt: f64, world: &World, crouch: f64, remaining: f64, owner: Owner, rel_x: f64) {
        if crouch > 0.0 {
            if !self.follow_anchor(world) {
                self.start_fall();
                return;
            }
            let crouch = crouch - dt;
            if crouch > 0.0 {
                self.state = State::Leap { crouch, remaining, owner, rel_x };
                return;
            }
            let Some((ax, ay)) = world.anchor(owner) else {
                // 목표 창이 사라짐 — 뛰지 않는다
                self.state = self.idle_state();
                return;
            };
            let top = world.bounds.top + self.cfg.size;
            let arc = hop::leap_arc((self.body.x, self.body.y), (ax + rel_x, ay), self.cfg.size * hop::BIG_JUMP_RISE, top);
            self.pacing.after_beat = None;
            self.standing = None;
            self.body.vx = arc.vx;
            self.body.vy = arc.vy;
            if arc.vx.abs() > 1.0 {
                self.facing = arc.vx.signum();
            }
            self.state = State::Leap { crouch: 0.0, remaining: arc.secs, owner, rel_x };
            return;
        }
        let step = dt.min(remaining);
        self.body.x += self.body.vx * step;
        self.body.y += self.body.vy * step + 0.5 * GRAVITY * step * step;
        self.body.vy += GRAVITY * step;
        let remaining = remaining - dt;
        if remaining > 0.0 {
            self.state = State::Leap { crouch: 0.0, remaining, owner, rel_x };
        } else {
            self.land_leap(world, owner, rel_x);
        }
    }

    /// 도착: 목표 자리가 그대로(보이고 가까이) 있으면 그 창 윗면에 내려서고, 아니면 그 자리에서 떨어진다
    fn land_leap(&mut self, world: &World, owner: Owner, rel_x: f64) {
        let spot = world.anchor(owner).map(|(ax, ay)| (ax + rel_x, ay));
        match spot {
            Some((x, y))
                if world.piece_at(owner, x).is_some()
                    && (x - self.body.x).abs() <= self.cfg.size
                    && (y - self.body.y).abs() <= self.cfg.size =>
            {
                self.body = Body { x, y, vx: 0.0, vy: 0.0 };
                self.standing = Some(Standing { owner, rel_x });
                self.state = State::Land { remaining: LAND_SECS };
            }
            _ => self.start_fall(),
        }
    }
}
