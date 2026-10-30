//! 다른 동물과 인사·술래 놀이 (v0.3.4)의 Pet 동작 — brain.rs의 자식 모듈 (비공개 필드에 접근).
//! 누구와 언제 인사하고 누가 술래인지, 언제 끝나는지는 social.rs(코디네이터)가 정하고, 여기서는
//! 그 결정대로 서서 인사하거나(State::Greet) 쫓거나(State::Chase) 도망간다(State::Flee).
//!
//! - 걷는 동물(개구리 포함)은 서 있는 디딤판 조각 안에서만 달린다 — 끝에서 뛰어내리거나 화면 밖으로 나가지 않는다.
//! - 나는 동물(꿀벌)은 나는 범위 안에서 친구를 쫓아가거나 좌우로 오가며 도망간다.
//! - 사용자 손길(누르기·간질이기·먹이·하우스)과 손님의 집 생각이 놀이보다 먼저다.

use std::sync::atomic::{AtomicU64, Ordering};

use super::*;

/// 놀이 상태의 안전 시한 여유 — 코디네이터가 끝내 주지 못해도(사라짐 등) 그만큼 뒤엔 스스로 끝낸다
const SAFETY_SECS: f64 = 1.0;
/// 인사할 때 먼저 친구 쪽을 쳐다보는 시간 (인사 전체의 최대 1/3)
const GREET_LOOK_SECS: f64 = 0.6;
/// 술래인 나는 동물이 노리는 높이 — 친구 발보다 몸 이만큼 위
const FLY_CHASE_LIFT: f64 = 0.3;
/// 도망가는 나는 동물이 친구 발보다 떠 있는 높이와 살랑임 (몸 배)
const FLY_FLEE_LIFT: f64 = 0.9;
const FLY_FLEE_SWAY: f64 = 0.35;
/// 나는 동물이 놀 때 속도 (걷기 속도 배) — 달리는 친구를 따라잡을 만큼
const FLY_PLAY_MULT: f64 = 3.0;
/// 도망자는 신나서 조금 느긋하게 달린다 — 대개 잡히며 끝난다
const FLEE_PACE: f64 = 0.85;

static NEXT_KEY: AtomicU64 = AtomicU64::new(1);

/// 새 동물의 구별 번호
pub(super) fn next_key() -> u64 {
    NEXT_KEY.fetch_add(1, Ordering::Relaxed)
}

/// 놀이 중 맡은 일
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SocialRole {
    /// 인사 (잡힌 뒤 하트, 자는 친구 쳐다보기 포함)
    Greet,
    Chase,
    Flee,
}

/// 인사 방식
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GreetKind {
    /// 손 흔들기·갸웃 + "hi!" + 작은 하트
    Hello,
    /// 새침한 고양이: 흥! (하트 없음)
    Hmph,
    /// 자는 친구를 살짝 쳐다본다 ("...", 깨우지 않는다)
    Peek,
    /// 잡혔다/잡았다 — 하트. speak = "hehe!"
    Caught { speak: bool },
}

impl Pet {
    /// 다른 동물과 구별하는 번호
    pub fn social_key(&self) -> u64 {
        self.social_key
    }

    /// 지금 인사·놀이 중이면 맡은 일
    pub fn social_role(&self) -> Option<SocialRole> {
        match self.state {
            State::Greet { .. } => Some(SocialRole::Greet),
            State::Chase { .. } => Some(SocialRole::Chase),
            State::Flee { .. } => Some(SocialRole::Flee),
            _ => None,
        }
    }

    /// 놀이를 끊는 일이 생겼다: 사용자 손길·먹이·집 가기·손님의 집 생각·창 안
    fn social_interrupted(&self) -> bool {
        self.tickling
            || self.housed
            || self.home_trip.is_some()
            || self.food.is_some()
            || self.meal_pending.is_some()
            || self.hand.is_some()
            || self.farewell_pending
            || self.inside.is_some()
            || self.is_heading_out()
    }

    /// 새 인사·놀이를 시작할 수 있음: 깨어 있고 디딤판 위(나는 동물은 떠서도) 걷거나 쉬는 중, 바쁜 일 없음
    pub fn social_free(&self) -> bool {
        matches!(self.state, State::Move { .. } | State::Idle { .. } | State::Turn { .. })
            && (self.standing.is_some() || self.cfg.flies)
            && self.hover_since.is_none()
            && !self.thrown
            && !self.social_interrupted()
    }

    /// 디딤판 위에서 자는 중 (친구가 살짝 쳐다보기만 한다)
    pub fn social_asleep(&self) -> bool {
        matches!(self.state, State::Sleep { .. }) && self.standing.is_some() && !self.housed
    }

    /// 친구 자리를 알려 준다 (발 위치)
    pub fn set_play_target(&mut self, x: f64, y: f64) {
        self.play_target = Some((x, y));
    }

    fn face_x(&mut self, x: f64) {
        let dx = x - self.body.x;
        if dx.abs() > 1.0 {
            self.facing = dx.signum();
        }
    }

    /// 놀이 시작 공통: 하던 길·박자를 잊는다
    fn begin_social(&mut self) {
        self.pacing.after_beat = None;
        self.flight = None;
        self.hover_at = None;
        self.clear_hover();
        self.body.vx = 0.0;
        self.body.vy = 0.0;
    }

    /// 친구(x 쪽)를 보고 secs초 인사한다
    pub fn start_greet(&mut self, toward_x: f64, kind: GreetKind, secs: f64) {
        self.begin_social();
        self.face_x(toward_x);
        let look = GREET_LOOK_SECS.min(secs / 3.0);
        let (look, anim) = match kind {
            GreetKind::Hello => {
                self.heart_timer = HEART_SECS;
                self.say(Speech::Greet);
                (look, if self.rng.bool() { Anim::Wave } else { Anim::Tilt })
            }
            GreetKind::Hmph => {
                self.say(Speech::Grumpy);
                (look, Anim::Huff)
            }
            GreetKind::Peek => {
                self.say(Speech::Ignore);
                (secs, Anim::Look)
            }
            GreetKind::Caught { speak } => {
                self.heart_timer = HEART_SECS.max(secs);
                if speak {
                    self.say(Speech::Tickle);
                }
                (0.0, Anim::Heart)
            }
        };
        self.state = State::Greet { look, anim, remaining: secs + SAFETY_SECS };
    }

    /// 술래가 되어 친구(at)를 쫓는다
    pub fn start_chase(&mut self, at: (f64, f64), secs: f64) {
        self.begin_social();
        self.play_target = Some(at);
        self.face_x(at.0);
        self.say(Speech::Play);
        self.state = State::Chase { remaining: secs + SAFETY_SECS };
    }

    /// 술래(from)를 피해 반대쪽으로 도망간다
    pub fn start_flee(&mut self, from: (f64, f64), secs: f64) {
        self.begin_social();
        self.play_target = Some(from);
        let dx = self.body.x - from.0;
        let dir = if dx.abs() > 1.0 { dx.signum() } else { -self.facing };
        self.facing = dir;
        self.say(Speech::Flee);
        self.state = State::Flee { remaining: secs + SAFETY_SECS, dir };
    }

    /// 놀이 끝 — 잠깐 앉았다가 평소대로
    pub fn end_social(&mut self) {
        self.play_target = None;
        if self.social_role().is_none() {
            return;
        }
        self.pacing.after_beat = None;
        self.hover_at = None;
        self.state = self.beat(AfterBeat::Rest);
    }

    /// 말풍선 자리가 비어 있을 때만 말한다
    fn say(&mut self, speech: Speech) {
        if self.speech.is_none() {
            self.speech = Some(speech);
        }
    }

    /// update에서: 인사·놀이 중이면 이번 틱을 처리하고 true. 끊을 일이 생겼으면 놀이를 끝내고 false (평소 처리로)
    pub(super) fn update_social(&mut self, dt: f64, world: &World) -> bool {
        let remaining = match self.state {
            State::Greet { remaining, .. } | State::Chase { remaining } | State::Flee { remaining, .. } => remaining - dt,
            _ => return false,
        };
        if self.social_interrupted() || remaining <= 0.0 {
            self.end_social();
            return false;
        }
        match self.state {
            State::Greet { look, anim, .. } => {
                if let Some((x, _)) = self.play_target {
                    if look > 0.0 {
                        self.face_x(x);
                    }
                }
                self.state = State::Greet { look: (look - dt).max(0.0), anim, remaining };
                self.hold_still(world);
            }
            State::Chase { .. } => {
                self.state = State::Chase { remaining };
                self.play_run(dt, world);
            }
            State::Flee { dir, .. } => {
                self.state = State::Flee { remaining, dir };
                self.play_run(dt, world);
            }
            _ => {}
        }
        true
    }

    /// 인사하는 동안 제자리: 디딤판을 따라가거나 (사라지면 떨어진다) 떠서 살랑
    fn hold_still(&mut self, world: &World) {
        if self.standing.is_some() {
            if !self.follow_anchor(world) {
                if self.cfg.flies {
                    self.take_off();
                } else {
                    self.start_fall();
                }
            }
        } else if self.cfg.flies {
            self.hover_in_place(world);
        } else {
            self.start_fall();
        }
    }

    fn play_run(&mut self, dt: f64, world: &World) {
        if self.cfg.flies {
            self.play_fly(dt, world);
        } else {
            self.play_ground(dt, world);
        }
    }

    /// 걷는 동물: 서 있는 디딤판 조각 안에서만 달린다 (끝에서 술래는 멈추고 도망자는 돌아선다)
    fn play_ground(&mut self, dt: f64, world: &World) {
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
        if lo > hi {
            // 몸보다 좁은 조각 — 제자리
            return;
        }
        let pace = if matches!(self.state, State::Flee { .. }) { FLEE_PACE } else { 1.0 };
        let speed = self.cfg.walk_speed * Move::Run.speed_mult() * pace * self.hop_pace(Move::Run, dt);
        let x = self.body.x;
        let new_x = match self.state {
            State::Chase { .. } => {
                let tx = self.play_target.map_or(x, |t| t.0);
                self.face_x(tx);
                let dx = tx - x;
                x + dx.signum() * (speed * dt).min(dx.abs())
            }
            State::Flee { remaining, dir } => {
                let want = x + dir * speed * dt;
                // 끝에 닿으면 돌아선다 (뛰어내리지 않는다)
                if (dir > 0.0 && want >= hi) || (dir < 0.0 && want <= lo) {
                    self.state = State::Flee { remaining, dir: -dir };
                }
                want
            }
            _ => x,
        };
        let new_x = new_x.clamp(lo, hi);
        if let State::Flee { dir, .. } = self.state {
            self.facing = dir;
        }
        self.standing = Some(Standing { owner: st.owner, rel_x: st.rel_x + (new_x - x) });
        self.body.x = new_x;
    }

    /// 나는 동물: 술래는 친구에게 곧장 날아가고, 도망자는 친구 위에서 좌우로 오가며 살랑인다
    fn play_fly(&mut self, dt: f64, world: &World) {
        if self.standing.is_some() {
            self.take_off();
        }
        self.flight = None;
        self.hover_at = None;
        let b = self.fly_limits(world);
        let pace = if matches!(self.state, State::Flee { .. }) { FLEE_PACE } else { 1.0 };
        let step = self.cfg.walk_speed * FLY_PLAY_MULT * pace * dt;
        let (px, py) = self.play_target.unwrap_or((self.body.x, self.body.y));
        let (x, y) = match self.state {
            // 술래: 친구 머리 높이로 곧장
            State::Chase { .. } => {
                self.face_x(px);
                let (tx, ty) = crate::flight::clamp(&b, px, py - FLY_CHASE_LIFT * self.cfg.size);
                let (dx, dy) = (tx - self.body.x, ty - self.body.y);
                let dist = dx.hypot(dy);
                if dist > step { (self.body.x + dx / dist * step, self.body.y + dy / dist * step) } else { (tx, ty) }
            }
            // 도망자: 좌우로는 늘 달아나고 (끝에서 돌아선다), 높이는 친구 위에서 살랑이며 따라간다
            State::Flee { remaining, dir } => {
                let want = self.body.x + dir * step;
                let dir = if (dir > 0.0 && want >= b.right) || (dir < 0.0 && want <= b.left) { -dir } else { dir };
                self.state = State::Flee { remaining, dir };
                self.facing = dir;
                let sway = FLY_FLEE_SWAY * self.cfg.size * (self.clock * 3.0).sin();
                let ty = py - FLY_FLEE_LIFT * self.cfg.size + sway;
                (self.body.x + dir * step, self.body.y + (ty - self.body.y).clamp(-step, step))
            }
            _ => (self.body.x, self.body.y),
        };
        let (x, y) = crate::flight::clamp(&b, x, y);
        self.body = Body { x, y, vx: 0.0, vy: 0.0 };
    }
}
