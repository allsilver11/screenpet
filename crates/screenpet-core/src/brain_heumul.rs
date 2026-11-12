//! 흐물이 (pet.json `timid` · `energetic` · `rigging`)의 Pet 동작 — brain.rs의 자식 모듈 (비공개 필드에 접근).
//!
//! - 겁쟁이(timid): 다른 동물과 마주치면 social.rs가 반쯤 start_panic을 부름 — 친구 반대쪽으로 돌아서서 "eek!",
//!   두 팔 번쩍(panic 그림) 들고 달리기보다 빠르게 2~3초 줄행랑. 서 있는 디딤판 끝에 닿으면 뛰어내리지 않고 멈춰 숨을 고름
//! - 기운 넘침(energetic): 걷기로 정해진 이동의 ENERGETIC_RUN_CHANCE를 달리기로 바꾸고, 창 위로 크게 뛰는 확률은 ENERGETIC_JUMP_MULT배.
//! - 매달리기(rigging): 평소 이동을 고를 때 가까운 창 옆면이 있으면 가끔 (RIG_CHANCE) 뛰어올라 옆면을 한 손으로 잡음
//!   (State::Rig). 매달려 다른 팔로 손 흔들기(hang) ↔ 밧줄 타듯 옆면 오르내리기(rope)를 몇 번 되풀이하고, 창 반대쪽으로 뛰어내림
//!   (떨어져 내리는 건 보통 낙하 — 높으면 콰당). 창이 사라지거나 앞 창에 가려지면 떨어짐
//!
//! 주사위는 이 동물들만 굴림 (다른 동물의 주사위 순서는 그대로).

use super::*;
use crate::physics::GRAVITY;

/// 줄행랑 시간
pub const PANIC_SECS: (f64, f64) = (2.0, 3.0);
/// 줄행랑 속도 (달리기 속도 배)
pub const PANIC_PACE: f64 = 1.2;
/// 걷기로 정해진 이동을 달리기로 바꾸는 몫 (기운 넘치는 동물)
pub const ENERGETIC_RUN_CHANCE: f64 = 0.6;
/// 창 위로 크게 뛰는 확률 배율 (기운 넘치는 동물)
pub const ENERGETIC_JUMP_MULT: f64 = 2.0;
/// 이동을 고를 때 닿을 만한 창 옆면이 있으면 매달리러 갈 확률
pub const RIG_CHANCE: f64 = 0.15;
/// 매달리러 뛸 수 있는 옆면까지 가로 거리 (몸 크기 배)
pub const RIG_REACH: f64 = 3.0;
/// 잡는 높이: 발이 지금보다 몸 이만큼 위
pub const RIG_GRAB_RISE: f64 = 1.0;
/// 옆면 윗끝에서 발까지 최소 (몸 크기 배) — 머리가 창 위로 삐져나가지 않게
pub const RIG_TOP_MARGIN: f64 = 0.9;
/// 뛰어오를 때 더 솟는 높이 (몸 크기 배)
const RIG_JUMP_RISE: f64 = 0.3;
/// 매달려 손 흔드는 시간 · 오르내리는 시간
pub const RIG_HANG_SECS: (f64, f64) = (1.2, 2.4);
/// 줄 타듯 오르내리는 한 번의 시간 (초)
pub const RIG_ROPE_SECS: (f64, f64) = (0.8, 1.6);
/// 매달려 있는 동안 오르내리는 횟수
const RIG_TURNS: (usize, usize) = (1, 3);
/// 놓고 뛰어내리는 속도 (크기 128 기준 px/s): 창 반대쪽으로, 살짝 위로
const RIG_LEAP_VX: f64 = 90.0;
const RIG_LEAP_VY: f64 = 160.0;

/// 매달리기 단계
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum RigPhase {
    /// 옆면으로 뛰어오르는 중 (포물선 — 몸 속도)
    Jump,
    /// 한 손으로 매달려 다른 팔로 손 흔들기
    Hang,
    /// 손을 번갈아 옆면을 오르내림 (dir -1 위, +1 아래)
    Rope { dir: f64 },
}

/// 매달릴 곳: 창, 옆면, 그 옆면 x, 발 높이
#[derive(Debug, Clone, Copy)]
struct RigSpot {
    window: u64,
    side: Side,
    edge_x: f64,
    y: f64,
    top: f64,
}

impl Pet {
    // ---------------------------------------------------------------- 겁쟁이 줄행랑
    /// 친구(from_x)를 보고 깜짝 놀라 반대쪽으로 줄행랑 (겁쟁이만, 놀 수 있는 상태일 때만). 시작했으면 true
    pub fn start_panic(&mut self, from_x: f64) -> bool {
        if !self.cfg.timid || !self.social_free() || self.cfg.flies {
            return false;
        }
        self.pacing.after_beat = None;
        self.clear_hover();
        self.play_target = None;
        let dx = self.body.x - from_x;
        let dir = if dx.abs() > 1.0 { dx.signum() } else { -self.facing };
        self.facing = dir;
        self.speech = Some(Speech::Eek);
        self.state = State::Panic { remaining: behavior::range(&mut self.rng, PANIC_SECS), dir };
        true
    }

    /// 줄행랑: 디딤판을 따라 dir 쪽으로 빠르게. 조각 끝에 닿으면 멈춰 숨 고르기, 시간이 다 되면 진정
    pub(super) fn update_panic(&mut self, dt: f64, world: &World, remaining: f64, dir: f64) {
        if self.social_interrupted() {
            self.state = self.rest_beat();
            return;
        }
        if !self.follow_anchor(world) {
            self.start_fall();
            return;
        }
        let st = self.standing.expect("follow_anchor keeps standing");
        let x = self.body.x;
        let Some(piece) = world.piece_at(st.owner, x).copied() else {
            self.start_fall();
            return;
        };
        let (lo, hi) = (piece.x_start + self.cfg.half_w, piece.x_end - self.cfg.half_w);
        let (lo, hi) = (lo.min(x), hi.max(x));
        let want = x + dir * self.cfg.walk_speed * Move::Run.speed_mult() * PANIC_PACE * dt;
        let new_x = want.clamp(lo, hi);
        self.standing = Some(Standing { owner: st.owner, rel_x: st.rel_x + (new_x - x) });
        self.body.x = new_x;
        let remaining = remaining - dt;
        self.state = if remaining <= 0.0 || new_x != want { self.rest_beat() } else { State::Panic { remaining, dir } };
    }

    // ---------------------------------------------------------------- 창 옆면 매달리기
    /// 닿을 만한 가장 가까운 창 옆면 (매달리는 동물만, 디딤판 위에서)
    fn rig_spot(&self, world: &World) -> Option<RigSpot> {
        if !self.cfg.rigging || self.cfg.flies || self.standing.is_none() || self.inside.is_some() || self.is_heading_out() || self.control.is_some() {
            return None;
        }
        let size = self.cfg.size;
        let (x, y) = (self.body.x, self.body.y);
        let mut best: Option<(f64, RigSpot)> = None;
        for w in &world.windows {
            let r = w.rect;
            for (side, edge_x) in [(Side::Left, r.left), (Side::Right, r.right)] {
                let wx = self.wall_x(edge_x, side);
                let dist = match side {
                    Side::Left => wx - x,
                    _ => x - wx,
                };
                if !(0.0..=RIG_REACH * size).contains(&dist) {
                    continue;
                }
                let gy = (y - RIG_GRAB_RISE * size).max(r.top + RIG_TOP_MARGIN * size).min(r.bottom);
                if gy > y - 0.3 * size || gy - size < world.bounds.top || gy < r.top + RIG_TOP_MARGIN * size {
                    continue;
                }
                let open = |yy: f64| world.side_covered(w.id, edge_x, yy) == Some(false);
                if !(open(gy) && open(gy - 0.5 * size)) {
                    continue;
                }
                if best.as_ref().map_or(true, |(d, _)| dist < *d) {
                    best = Some((dist, RigSpot { window: w.id, side, edge_x, y: gy, top: r.top }));
                }
            }
        }
        best.map(|(_, s)| s)
    }

    /// 가까운 창 옆면으로 뛰어올라 매달림 (주사위 없이 — 고르는 건 move_state). 시작했으면 true
    pub(super) fn try_rig(&mut self, world: &World) -> bool {
        let Some(spot) = self.rig_spot(world) else { return false };
        let to = (self.wall_x(spot.edge_x, spot.side), spot.y);
        let arc = crate::hop::leap_arc((self.body.x, self.body.y), to, RIG_JUMP_RISE * self.cfg.size, world.bounds.top + self.cfg.size);
        let turns = self.rng.usize(RIG_TURNS.0..=RIG_TURNS.1) as u8;
        self.pacing.after_beat = None;
        self.clear_hover();
        self.poop.stepping_off();
        self.standing = None;
        self.facing = if spot.side == Side::Left { 1.0 } else { -1.0 };
        self.body.vx = arc.vx;
        self.body.vy = arc.vy;
        self.state = State::Rig { window: spot.window, side: spot.side, rel_y: spot.y - spot.top, phase: RigPhase::Jump, remaining: arc.secs, turns };
        true
    }

    /// move_state에서: 매달리는 동물이 닿을 만한 옆면 가까이 있으면 RIG_CHANCE로 매달리러 감
    pub(super) fn maybe_rig(&mut self, world: &World) -> bool {
        self.cfg.rigging && self.rig_spot(world).is_some() && self.rng.f64() < RIG_CHANCE && self.try_rig(world)
    }

    /// 매달린 채 한 틱
    #[allow(clippy::too_many_arguments)]
    pub(super) fn update_rig(&mut self, dt: f64, world: &World, window: u64, side: Side, rel_y: f64, phase: RigPhase, remaining: f64, turns: u8) {
        let Some(rect) = world.windows.iter().find(|w| w.id == window).map(|w| w.rect) else {
            self.start_fall();
            return;
        };
        let edge_x = if side == Side::Left { rect.left } else { rect.right };
        self.facing = if side == Side::Left { 1.0 } else { -1.0 };
        let remaining = remaining - dt;
        if phase == RigPhase::Jump {
            self.body.vy += GRAVITY * dt;
            self.body.x += self.body.vx * dt;
            self.body.y += self.body.vy * dt;
            if remaining > 0.0 {
                self.state = State::Rig { window, side, rel_y, phase, remaining, turns };
                return;
            }
            let secs = behavior::range(&mut self.rng, RIG_HANG_SECS);
            self.state = State::Rig { window, side, rel_y, phase: RigPhase::Hang, remaining: secs, turns };
        } else {
            let (lo, hi) = (RIG_TOP_MARGIN * self.cfg.size, rect.height());
            let (mut rel_y, mut phase, mut remaining, mut turns) = (rel_y, phase, remaining, turns);
            if let RigPhase::Rope { dir } = phase {
                let want = rel_y + dir * self.scaled(behavior::WALL_CLIMB_SPEED) * dt;
                rel_y = want.clamp(lo, hi.max(lo));
                if remaining <= 0.0 || rel_y != want {
                    phase = RigPhase::Hang;
                    remaining = behavior::range(&mut self.rng, RIG_HANG_SECS);
                }
            } else if remaining <= 0.0 {
                if turns == 0 {
                    self.let_go_of_rig();
                    return;
                }
                turns -= 1;
                let dir = if rel_y - lo < 0.3 * self.cfg.size {
                    1.0
                } else if hi - rel_y < 0.3 * self.cfg.size {
                    -1.0
                } else if self.rng.bool() {
                    1.0
                } else {
                    -1.0
                };
                phase = RigPhase::Rope { dir };
                remaining = behavior::range(&mut self.rng, RIG_ROPE_SECS);
            }
            self.state = State::Rig { window, side, rel_y, phase, remaining, turns };
        }
        let State::Rig { rel_y, .. } = self.state else { return };
        let y = rect.top + rel_y;
        if world.side_covered(window, edge_x, y) != Some(false) || y - self.cfg.size < world.bounds.top {
            self.start_fall();
            return;
        }
        self.body = Body { x: self.wall_x(edge_x, side), y, vx: 0.0, vy: 0.0 };
    }

    /// 손을 놓고 창 반대쪽으로 폴짝 뛰어내림 (그 뒤는 보통 낙하·착지)
    fn let_go_of_rig(&mut self) {
        let away = -self.facing;
        self.start_fall();
        self.facing = away;
        self.body.vx = away * self.scaled(RIG_LEAP_VX);
        self.body.vy = -self.scaled(RIG_LEAP_VY);
    }

    /// 매달리기 그림: 뛰어오르기는 폴짝, 매달리면 hang, 오르내리면 rope
    pub(super) fn rig_anim(phase: RigPhase) -> Anim {
        match phase {
            RigPhase::Jump => Anim::Hop,
            RigPhase::Hang => Anim::Hang,
            RigPhase::Rope { .. } => Anim::Rope,
        }
    }
}
