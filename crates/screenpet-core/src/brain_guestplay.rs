//! 친구 동물(손님)과 놀아 주기 (v0.3.4) — brain.rs의 자식 모듈 (비공개 필드에 접근).
//!
//! 손님 동물을 클릭하면(누르고 끌지 않고 뗌) 반반 확률로
//! - 애교: 앞모습 재주(하트·손 흔들기·갸웃·배 보이기·기지개) + "hehe!"
//! - 잡아 봐!: 4~7초 동안 커서를 피해 디딤판 위를 달아난다 (State::CatchMe). 끝에선 돌아서고
//!   (일부러 떨어지지 않는다), 커서가 따라오면 방향을 다시 정하고, 몸 반 안으로 다가오면 폴짝 피한다.
//!   끝나면 하트, 아니면 커서 쪽을 힐끗 돌아본다.
//!
//! 새침한 고양이는 애교가 적다 (35/65, 도망갈 때 "흥!"도). 내 동물의 클릭 반응은 그대로다.
//! 집 생각·작별·하우스·먹이·창 안·붙잡기가 놀이보다 먼저다.

use super::*;

/// 손님을 클릭했을 때 애교 부릴 확률 (나머지는 잡아 봐!)
pub const GUEST_AEGYO_CHANCE: f64 = 0.5;
/// 새침한 동물은 애교가 적다
pub const ALOOF_AEGYO_CHANCE: f64 = 0.35;
/// 잡아 봐! 시간 (초)
pub const CATCH_ME_SECS: (f64, f64) = (4.0, 7.0);
/// 커서가 이 거리(몸 크기 배) 안이면 반대쪽으로 달아난다 — 멀면 하던 방향으로 달리다 끝에서 돌아선다
pub const WATCH_BODIES: f64 = 4.0;
/// 끝에 몰려 돌아서면 이만큼은 그 방향으로 커서 옆을 빠져나간다 (끝에서 떨지 않게)
pub const CORNER_COMMIT_SECS: f64 = 0.9;
/// 끝에 닿았다고 보는 여유 (px)
const EDGE_EPS: f64 = 2.0;
/// 커서가 몸 가운데에서 이 거리(몸 크기 배) 안이면 폴짝 피한다
pub const DODGE_BODIES: f64 = 0.5;
/// 폴짝 피하는 시간과 그동안의 속도 배
pub const DODGE_SECS: f64 = 0.35;
const DODGE_BOOST: f64 = 1.8;
/// 폴짝 뒤 다시 피할 수 있을 때까지
const DODGE_COOLDOWN_SECS: f64 = 0.6;
/// 나는 동물이 달아나는 속도 (걷기 속도 배)
const FLY_RUN_MULT: f64 = 2.5;
/// 나는 동물은 커서와 이만큼(몸 크기 배) 높이를 벌린다
const FLY_KEEP_AWAY: f64 = 1.2;
const FLY_SWAY: f64 = 0.25;
/// 끝나고 커서 쪽을 힐끗 돌아보는 시간
const LOOK_BACK_SECS: f64 = 1.2;

/// 손님 클릭 반응에 쓰는 애교 (앞모습 재주)
const AEGYO_TRICKS: [Trick; 5] = [Trick::Heart, Trick::Wave, Trick::Tilt, Trick::Belly, Trick::Stretch];

/// 손님 클릭 반응
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GuestClick {
    Aegyo(Trick),
    RunAway,
}

/// 손님 클릭: 반반 애교 / 잡아 봐! (새침하면 애교 35%)
pub fn pick_guest_click(rng: &mut fastrand::Rng, personality: Personality) -> GuestClick {
    let aegyo = if personality == Personality::Aloof { ALOOF_AEGYO_CHANCE } else { GUEST_AEGYO_CHANCE };
    if rng.f64() < aegyo {
        GuestClick::Aegyo(AEGYO_TRICKS[rng.usize(..AEGYO_TRICKS.len())])
    } else {
        GuestClick::RunAway
    }
}

/// 달아날 방향 정하기 (순수 함수). x: 몸, cx: 커서, [lo, hi]: 몸이 갈 수 있는 범위, watch: 겁내는 거리.
/// 돌아선 직후(commit > 0)가 아니고 커서가 watch 안이면 커서 반대쪽으로. 그 방향 끝에 닿아 있으면
/// 돌아서서 commit초 동안 그쪽으로 빠져나간다. 반환: (방향 ±1, 남은 commit)
pub fn steer_away(x: f64, cx: f64, dir: f64, commit: f64, lo: f64, hi: f64, watch: f64) -> (f64, f64) {
    let (mut dir, mut commit) = (dir, commit);
    let dx = x - cx;
    if commit <= 0.0 && dx.abs() < watch && dx.abs() > 1.0 {
        dir = dx.signum();
    }
    if (dir > 0.0 && x >= hi - EDGE_EPS) || (dir < 0.0 && x <= lo + EDGE_EPS) {
        dir = -dir;
        commit = CORNER_COMMIT_SECS;
    }
    (dir, commit)
}

/// 폴짝 피하는 동안은 더 빠르다
fn dodge_boost(dodge: f64) -> f64 {
    if dodge > 0.0 {
        DODGE_BOOST
    } else {
        1.0
    }
}

impl Pet {
    /// 손님으로 와 있는 동물을 클릭했다 (pointer_up에서, 디딤판 위·깨어 있음·배고프지 않음)
    pub(super) fn guest_click(&mut self) {
        let click = pick_guest_click(&mut self.rng, self.cfg.personality);
        // 집 생각이 나면 도망 놀이는 하지 않는다 (애교만 잠깐)
        let kind = match click {
            GuestClick::RunAway if !self.catch_me_interrupted() => {
                self.start_catch_me();
                return;
            }
            GuestClick::RunAway => AEGYO_TRICKS[self.rng.usize(..AEGYO_TRICKS.len())],
            GuestClick::Aegyo(kind) => kind,
        };
        if kind == Trick::Heart {
            self.heart_timer = HEART_SECS;
        }
        self.speech = Some(Speech::Tickle);
        self.state = State::Trick { kind, remaining: behavior::CLICK_TRICK_SECS };
    }

    /// 잡아 봐! 시작 — 커서 반대쪽으로 폴짝 뛰어 달아난다
    pub(super) fn start_catch_me(&mut self) {
        self.pacing.after_beat = None;
        self.flight = None;
        self.hover_at = None;
        self.clear_hover();
        self.body.vx = 0.0;
        self.body.vy = 0.0;
        let dx = self.body.x - self.cursor.0;
        let dir = if dx.abs() > 1.0 { dx.signum() } else { -self.facing };
        self.facing = dir;
        let aloof = self.cfg.personality == Personality::Aloof;
        self.speech = Some(if aloof && self.rng.bool() { Speech::Grumpy } else { Speech::Flee });
        let remaining = behavior::range(&mut self.rng, CATCH_ME_SECS);
        self.state = State::CatchMe { remaining, dir, commit: 0.0, dodge: DODGE_SECS };
    }

    /// 지금 잡아 봐! 놀이 중
    pub fn is_catch_me(&self) -> bool {
        matches!(self.state, State::CatchMe { .. })
    }

    /// 도망 놀이를 끊는 일: 하우스·먹이·직접 주기·작별·창 안·집 생각 (붙잡기는 pointer_down이 끊는다)
    fn catch_me_interrupted(&self) -> bool {
        self.housed
            || self.home_trip.is_some()
            || self.food.is_some()
            || self.meal_pending.is_some()
            || self.hand.is_some()
            || self.farewell_pending
            || self.inside.is_some()
            || self.is_homesick()
    }

    /// update에서: 잡아 봐! 중이면 이번 틱을 처리하고 true. 끊을 일이 생겼으면 끝내고 false (평소 처리로)
    pub(super) fn update_catch_me(&mut self, dt: f64, world: &World) -> bool {
        let State::CatchMe { remaining, dir, commit, dodge } = self.state else { return false };
        if self.catch_me_interrupted() {
            self.state = self.beat(AfterBeat::Rest);
            return false;
        }
        let remaining = remaining - dt;
        if remaining <= 0.0 {
            self.finish_catch_me();
            return true;
        }
        let (cx, cy) = self.cursor;
        let size = self.cfg.size;
        let near = (cx - self.body.x).hypot(cy - (self.body.y - 0.5 * size)) < DODGE_BODIES * size;
        let mut dodge = dodge - dt;
        let mut commit = (commit - dt).max(0.0);
        if near && dodge <= -DODGE_COOLDOWN_SECS {
            // 손이 닿을락 말락 — 폴짝 피하며 커서 반대쪽으로 다시 정한다
            dodge = DODGE_SECS;
            commit = 0.0;
        }
        self.state = State::CatchMe { remaining, dir, commit, dodge };
        if self.cfg.flies {
            self.catch_me_fly(dt, world);
        } else {
            self.catch_me_ground(dt, world);
        }
        true
    }

    /// 끝: 반은 하트, 반은 커서 쪽을 힐끗 돌아본다
    fn finish_catch_me(&mut self) {
        let dx = self.cursor.0 - self.body.x;
        if dx.abs() > 1.0 {
            self.facing = dx.signum();
        }
        self.pacing.after_beat = None;
        self.state = if self.rng.bool() {
            self.heart_timer = HEART_SECS;
            State::Trick { kind: Trick::Heart, remaining: behavior::CLICK_TRICK_SECS }
        } else {
            State::Idle { remaining: LOOK_BACK_SECS, look: true }
        };
    }

    /// 걷는 동물: 서 있는 디딤판 조각 안에서만 달아난다 (끝에서 돌아선다 — 뛰어내리지 않는다)
    fn catch_me_ground(&mut self, dt: f64, world: &World) {
        if !self.follow_anchor(world) {
            self.start_fall();
            return;
        }
        let st = self.standing.expect("follow_anchor keeps standing");
        let Some(piece) = world.piece_at(st.owner, self.body.x).copied() else {
            self.start_fall();
            return;
        };
        let (lo, hi) = (piece.x_start + self.cfg.half_w, piece.x_end - self.cfg.half_w);
        let State::CatchMe { remaining, dir, commit, dodge } = self.state else { return };
        if lo > hi {
            // 몸보다 좁은 조각 — 제자리
            return;
        }
        let x = self.body.x;
        let (dir, commit) = steer_away(x, self.cursor.0, dir, commit, lo, hi, WATCH_BODIES * self.cfg.size);
        let boost = dodge_boost(dodge);
        let speed = self.cfg.walk_speed * Move::Run.speed_mult() * boost * self.hop_pace(Move::Run, dt);
        let new_x = (x + dir * speed * dt).clamp(lo, hi);
        self.facing = dir;
        self.standing = Some(Standing { owner: st.owner, rel_x: st.rel_x + (new_x - x) });
        self.body.x = new_x;
        self.state = State::CatchMe { remaining, dir, commit, dodge };
    }

    /// 나는 동물: 나는 범위 안에서 좌우로 달아나고, 높이는 커서와 벌린다 (살랑이며)
    fn catch_me_fly(&mut self, dt: f64, world: &World) {
        if self.standing.is_some() {
            self.take_off();
        }
        self.flight = None;
        self.hover_at = None;
        let b = self.fly_limits(world);
        let State::CatchMe { remaining, dir, commit, dodge } = self.state else { return };
        let (cx, cy) = self.cursor;
        let size = self.cfg.size;
        let (dir, commit) = steer_away(self.body.x, cx, dir, commit, b.left, b.right, WATCH_BODIES * size);
        let step = self.cfg.walk_speed * FLY_RUN_MULT * dodge_boost(dodge) * dt;
        // 몸 가운데가 커서보다 위면 더 위로, 아래면 더 아래로
        let center = self.body.y - 0.5 * size;
        let away = if center <= cy { -FLY_KEEP_AWAY } else { FLY_KEEP_AWAY };
        let sway = FLY_SWAY * size * (self.clock * 3.0).sin();
        let ty = cy + away * size + 0.5 * size + sway;
        let x = self.body.x + dir * step;
        let y = self.body.y + (ty - self.body.y).clamp(-step, step);
        let (x, y) = crate::flight::clamp(&b, x, y);
        self.facing = dir;
        self.body = Body { x, y, vx: 0.0, vy: 0.0 };
        self.state = State::CatchMe { remaining, dir, commit, dodge };
    }
}
