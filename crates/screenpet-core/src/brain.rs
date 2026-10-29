use std::collections::VecDeque;

use serde::Serialize;

use crate::care::{FeedResult, Food};
use crate::behavior::{self, ClickOutcome, ClickReaction, Family, Move, Personality, Rest, Special, Trick};
use crate::physics::{clamp_speed, step_air, Body, GRAVITY};
use crate::surfaces::{EndKind, Owner};
use crate::world::{Side, World};

const TURN_PAUSE: f64 = 0.3;
const LAND_SECS: f64 = 0.25;
const HOP_VX: f64 = 180.0;
const HOP_VY: f64 = -650.0;
const HAPPY_VY: f64 = -500.0;
/// 옆 모니터 바닥과 높이가 이만큼(px) 이내면 그대로 걸어 넘어간다
const SEAM_LEVEL_TOLERANCE: f64 = 2.0;
/// 옆 모니터 바닥이 이만큼(크기 128 기준 px) 높을 때까지는 뛰어오른다
const SEAM_MAX_HOP_UP: f64 = 150.0;
/// 뛰어오를 때 높이 차 위로 더 솟는 여유 (크기 128 기준 px)
const SEAM_HOP_MARGIN: f64 = 24.0;
const HEART_SECS: f64 = 1.0;
const CLICK_SLOP: f64 = 5.0;
const VELOCITY_WINDOW: f64 = 0.1;
const TICKLE_WINDOW: f64 = 0.6;
const TICKLE_DIST: f64 = 250.0;
const HOVER_PAUSE_SECS: f64 = 0.25;
/// 커서가 올라와 있는 동안 쳐다보기를 이어 가는 시간
const HOVER_LOOK_SECS: f64 = 1.5;
const TICKLE_HOLD: f64 = 1.5;
const TICKLE_WAKE_SECS: f64 = 5.0;
const CHATTER_SECS: (f64, f64) = (20.0, 40.0);
const FIRST_CHATTER_SECS: f64 = 25.0;
const CLICK_SPEECH_CHANCE: f64 = 0.5;
const HUNGRY_REST_CHANCE: f64 = 0.5;
const HUNGRY_SECS: (f64, f64) = (3.0, 5.0);
const HUNGRY_TALK_SECS: (f64, f64) = (15.0, 25.0);
const EAT_SECS: f64 = 2.5;
const AFTER_FAVORITE_HEART_SECS: f64 = 2.0;
const LEVEL_UP_SECS: f64 = 2.0;
const REFUSE_TILT_SECS: f64 = 2.0;
/// 자는 동물을 이 시간 안에 3번 누르면 깬다
const WAKE_CLICKS: u32 = 3;
const WAKE_CLICK_WINDOW: f64 = 2.0;
const SLEEPY_ANNOYED_SECS: f64 = 1.5;
const WAKE_STRETCH_SECS: f64 = 2.5;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Anim {
    Walk,
    Run,
    Roll,
    Idle,
    Look,
    Sleep,
    Hop,
    Fall,
    Dragged,
    Land,
    SitFront,
    Wave,
    Tilt,
    Belly,
    Stretch,
    Heart,
    Tickle,
    Annoyed,
    Arch,
    Wag,
    Hungry,
    Eat,
    LevelUp,
    /// 새침한 고양이의 흥! (앞모습)
    Huff,
    /// 벽 타기 — 기어오르기 (뒷모습)
    Climb,
    /// 벽 타기 — 미끄러져 내려오기 (뒷모습)
    Slide,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Speech {
    Chatter,
    Tickle,
    Grumpy,
    Hiss,
    Hungry,
    Full,
    LevelUp,
    /// 클릭 무시 ("...")
    Ignore,
}

impl Anim {
    /// 뒷모습 동작 — 좌우 반전하지 않는다.
    pub fn is_back(self) -> bool {
        matches!(self, Anim::Climb | Anim::Slide)
    }

    /// 앞모습 동작 — 좌우 반전하지 않는다.
    pub fn is_front(self) -> bool {
        matches!(self, Anim::SitFront | Anim::Wave | Anim::Tilt | Anim::Belly | Anim::Stretch | Anim::Heart | Anim::Tickle | Anim::Wag | Anim::Hungry | Anim::Eat | Anim::LevelUp | Anim::Huff)
    }
}

impl From<Move> for Anim {
    fn from(m: Move) -> Self {
        match m {
            Move::Walk => Anim::Walk,
            Move::Run => Anim::Run,
            Move::Roll => Anim::Roll,
        }
    }
}

impl From<Special> for Anim {
    fn from(s: Special) -> Self {
        match s {
            Special::Arch => Anim::Arch,
            Special::Wag => Anim::Wag,
        }
    }
}

impl From<Trick> for Anim {
    fn from(t: Trick) -> Self {
        match t {
            Trick::SitFront => Anim::SitFront,
            Trick::Wave => Anim::Wave,
            Trick::Tilt => Anim::Tilt,
            Trick::Belly => Anim::Belly,
            Trick::Stretch => Anim::Stretch,
            Trick::Heart => Anim::Heart,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum State {
    Move { kind: Move, remaining: f64 },
    Idle { remaining: f64, look: bool },
    Sleep { remaining: f64 },
    Turn { remaining: f64 },
    Land { remaining: f64 },
    Trick { kind: Trick, remaining: f64 },
    /// 간질이는 중 (앞모습 꼬물꼬물)
    Tickled { remaining: f64 },
    /// 자다가 간질여짐 — tickled_for가 TICKLE_WAKE_SECS를 넘으면 깸
    Annoyed { remaining: f64, tickled_for: f64 },
    /// Hop / Fall(낙하·던지기) / Happy 점프
    Air { anim: Anim },
    /// 마우스를 눌렀지만 아직 드래그 거리 미만
    Pressed { start_cx: f64, start_cy: f64, grounded: bool, was_sleeping: bool },
    Dragged,
    /// 종별 특별 동작
    Special { kind: Special, remaining: f64 },
    /// 배고파서 시무룩
    Hungry { remaining: f64 },
    /// 냠냠 — 끝나면 favorite면 하트, level_up이면 성장 연출
    Eating { remaining: f64, favorite: bool, level_up: bool },
    /// 성장 축하
    LevelUp { remaining: f64 },
    /// 새침하게 흥! (클릭 반응)
    Huff { remaining: f64 },
    /// 화면을 기어오르는 중 (디딤판 없음). 목표에 닿으면 hang초 매달렸다가 Slide
    Climb { target_y: f64, hang: f64 },
    /// 화면을 따라 일정한 속도로 미끄러져 내려오는 중 — 지나치는 창 윗면이나 바닥에 내려선다
    Slide,
    /// 던져져 창 옆면에 달라붙어 기어오르는 중. rel_y = 발 높이 − 창 윗면
    WallClimb { window: u64, side: Side, rel_y: f64 },
}

impl State {
    /// 활동 묶음 (쉬기·자기·공중 등은 None)
    fn family(self) -> Option<Family> {
        match self {
            State::Move { kind, .. } => Some(kind.family()),
            State::Trick { .. } | State::Huff { .. } | State::Tickled { .. } | State::Hungry { .. } | State::Eating { .. } | State::LevelUp { .. } => {
                Some(Family::Front)
            }
            State::Special { .. } => Some(Family::Special),
            State::Climb { .. } => Some(Family::Climb),
            _ => None,
        }
    }
}

/// 쉬는 박자(Idle)가 끝나면 할 일
#[derive(Debug, Clone, Copy, PartialEq)]
enum AfterBeat {
    /// 이동이 끝난 뒤의 쉬기 결정 (애교/특별/잠/이동)
    Rest,
    /// 미리 정해 둔 다음 활동
    Go(State),
}

/// 활동 전환 속도 조절 — 사이 박자 뒤 할 일과 이동 관성
#[derive(Debug, Clone, Copy, PartialEq, Default)]
struct Pacing {
    /// 지금 Idle이 활동 사이 박자라면 끝난 뒤 할 일
    after_beat: Option<AfterBeat>,
    /// 마지막으로 실제로 시작한 이동 방식 (이동 관성)
    last_move: Option<Move>,
}

/// 서 있는 디딤판과 그 기준점(anchor)으로부터의 x 오프셋.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Standing {
    pub owner: Owner,
    pub rel_x: f64,
}

#[derive(Debug, Clone, Copy)]
pub struct PetConfig {
    /// 스프라이트 한 변 (물리 px)
    pub size: f64,
    /// 몸 폭의 절반 — 디딤판 끝 판정에 사용
    pub half_w: f64,
    /// px/s
    pub walk_speed: f64,
    /// 이 동물의 특별 동작 (없으면 None)
    pub special: Option<Special>,
    /// 성격 (pet.json `personality`)
    pub personality: Personality,
    /// 벽 타기 (pet.json에 climb·slide가 있는 고양이·곰)
    pub can_climb: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct RenderState {
    pub anim: Anim,
    pub flip: bool,
    pub heart: bool,
}

pub struct Pet {
    pub cfg: PetConfig,
    pub body: Body,
    pub state: State,
    pub standing: Option<Standing>,
    /// +1 오른쪽, -1 왼쪽
    pub facing: f64,
    heart_timer: f64,
    grab_dx: f64,
    grab_dy: f64,
    /// (clock, cursor x, cursor y)
    history: VecDeque<(f64, f64, f64)>,
    clock: f64,
    hover_hist: VecDeque<(f64, f64, f64)>,
    tickling: bool,
    /// 커서가 연속으로 스프라이트 위에 있기 시작한 시각 (clock 기준)
    hover_since: Option<f64>,
    chatter_timer: f64,
    speech: Option<Speech>,
    hungry: bool,
    food_request: bool,
    /// 활동 사이 박자와 이동 관성
    pacing: Pacing,
    /// 누르기 직전 상태 — 새침하게 무시하면 되돌린다
    before_press: Option<(State, Option<AfterBeat>)>,
    /// 자는 동안 누른 횟수와 첫 클릭 시각
    sleep_clicks: (u32, f64),
    rng: fastrand::Rng,
}

impl Pet {
    pub fn new(cfg: PetConfig, x: f64, y: f64, seed: u64) -> Self {
        Self {
            cfg,
            body: Body { x, y, vx: 0.0, vy: 0.0 },
            state: State::Air { anim: Anim::Fall },
            standing: None,
            facing: 1.0,
            heart_timer: 0.0,
            grab_dx: 0.0,
            grab_dy: 0.0,
            history: VecDeque::new(),
            clock: 0.0,
            hover_hist: VecDeque::new(),
            tickling: false,
            hover_since: None,
            chatter_timer: FIRST_CHATTER_SECS,
            speech: None,
            hungry: false,
            food_request: false,
            pacing: Pacing::default(),
            before_press: None,
            sleep_clicks: (0, 0.0),
            rng: fastrand::Rng::with_seed(seed),
        }
    }

    pub fn update(&mut self, dt: f64, world: &World) {
        self.clock += dt;
        self.heart_timer = (self.heart_timer - dt).max(0.0);
        match self.state {
            State::Dragged => {}
            State::Pressed { grounded, .. } => {
                if grounded {
                    self.follow_anchor(world);
                }
            }
            State::Air { .. } => self.update_air(dt, world),
            State::Climb { target_y, hang } => self.update_climb(dt, target_y, hang),
            State::Slide => self.update_slide(dt, world),
            State::WallClimb { window, side, rel_y } => self.update_wall_climb(dt, world, window, side, rel_y),
            _ => {
                self.pause_for_hover();
                self.react_to_tickle();
                self.update_chatter(dt);
                self.update_ground(dt, world);
            }
        }
        self.on_started();
    }

    pub fn render(&self) -> RenderState {
        let anim = match self.state {
            State::Move { kind, .. } => kind.into(),
            State::Trick { kind, .. } => kind.into(),
            State::Tickled { .. } => Anim::Tickle,
            State::Annoyed { .. } => Anim::Annoyed,
            State::Special { kind, .. } => kind.into(),
            State::Hungry { .. } => Anim::Hungry,
            State::Eating { .. } => Anim::Eat,
            State::LevelUp { .. } => Anim::LevelUp,
            State::Huff { .. } => Anim::Huff,
            State::Idle { look: true, .. } => Anim::Look,
            State::Idle { .. } | State::Turn { .. } => Anim::Idle,
            State::Sleep { .. } => Anim::Sleep,
            State::Land { .. } => Anim::Land,
            State::Air { anim } => anim,
            State::Climb { .. } | State::WallClimb { .. } => Anim::Climb,
            State::Slide => Anim::Slide,
            State::Pressed { grounded: true, .. } => Anim::Idle,
            State::Pressed { .. } | State::Dragged => Anim::Dragged,
        };
        RenderState { anim, flip: self.facing < 0.0 && !anim.is_front() && !anim.is_back(), heart: self.heart_timer > 0.0 }
    }

    pub fn window_pos(&self) -> (f64, f64) {
        (self.body.x - self.cfg.size / 2.0, self.body.y - self.cfg.size)
    }

    /// 먹는 중 (먹다가 눌린 동안도 포함 — 손을 떼면 계속 먹는다)
    pub fn is_eating(&self) -> bool {
        match self.state {
            State::Eating { .. } => true,
            State::Pressed { .. } => matches!(self.before_press, Some((State::Eating { .. }, _))),
            _ => false,
        }
    }

    pub fn is_held(&self) -> bool {
        matches!(self.state, State::Pressed { .. } | State::Dragged)
    }

    /// 공중이거나 벽에 붙어 있음 (디딤판 위가 아님)
    fn off_surface(&self) -> bool {
        matches!(self.state, State::Air { .. } | State::Climb { .. } | State::Slide | State::WallClimb { .. })
    }

    /// 매 틱 update 전에 호출 (버튼 안 누른 상태). inside = 커서가 스프라이트 사각형 안
    pub fn hover(&mut self, cx: f64, cy: f64, inside: bool) {
        if self.is_held() || self.off_surface() {
            self.hover_hist.clear();
            self.tickling = false;
            self.hover_since = None;
            return;
        }
        if inside {
            self.hover_hist.push_back((self.clock, cx, cy));
            self.hover_since.get_or_insert(self.clock);
        } else {
            self.hover_since = None;
        }
        while self.hover_hist.front().is_some_and(|&(t, _, _)| self.clock - t > TICKLE_WINDOW) {
            self.hover_hist.pop_front();
        }
        // 커서가 잠깐 벗어났다 돌아온 틈(샘플 간 간격 > 0.1초)은 경로에 넣지 않는다
        let path: f64 = self
            .hover_hist
            .iter()
            .zip(self.hover_hist.iter().skip(1))
            .filter(|(a, b)| b.0 - a.0 <= 0.1)
            .map(|(a, b)| (b.1 - a.1).hypot(b.2 - a.2))
            .sum();
        self.tickling = inside && path >= TICKLE_DIST * self.cfg.size / 128.0;
    }

    /// 걷거나 달리는 중에 커서가 0.25초 넘게 올라와 있으면 멈춰서 쳐다본다.
    /// 커서가 머무는 동안엔 계속 쳐다본다 (다시 걷다 멈추며 방향이 바뀌지 않게).
    fn pause_for_hover(&mut self) {
        if !self.hover_since.is_some_and(|t| self.clock - t >= HOVER_PAUSE_SECS) {
            return;
        }
        match self.state {
            State::Move { .. } => self.state = State::Idle { remaining: HOVER_LOOK_SECS, look: true },
            State::Idle { remaining, look: true } => {
                self.state = State::Idle { remaining: remaining.max(HOVER_LOOK_SECS), look: true }
            }
            _ => {}
        }
    }

    /// 이번 틱에 띄울 말풍선 (한 번 꺼내면 None)
    pub fn take_speech(&mut self) -> Option<Speech> {
        self.speech.take()
    }

    /// 동물 교체 시 특별 동작을 바꾼다. 진행 중이던 Special은 (새 동물에 없을 수 있으니) 끝낸다.
    pub fn set_special(&mut self, special: Option<Special>) {
        self.cfg.special = special;
        if matches!(self.state, State::Special { .. }) {
            self.state = self.idle_state();
        }
    }

    pub fn pointer_down(&mut self, cx: f64, cy: f64) {
        if self.is_held() {
            return;
        }
        let grounded = self.standing.is_some() && !self.off_surface();
        let was_sleeping = matches!(self.state, State::Sleep { .. } | State::Annoyed { .. });
        self.grab_dx = self.body.x - cx;
        self.grab_dy = self.body.y - cy;
        self.hover_hist.clear();
        self.tickling = false;
        self.hover_since = None;
        self.history.clear();
        self.history.push_back((self.clock, cx, cy));
        self.before_press = Some((self.state, self.pacing.after_beat.take()));
        if !grounded {
            self.standing = None;
            self.body.vx = 0.0;
            self.body.vy = 0.0;
        }
        self.state = State::Pressed { start_cx: cx, start_cy: cy, grounded, was_sleeping };
    }

    pub fn pointer_move(&mut self, cx: f64, cy: f64) {
        match self.state {
            State::Pressed { start_cx, start_cy, .. } => {
                self.record(cx, cy);
                if (cx - start_cx).hypot(cy - start_cy) >= CLICK_SLOP {
                    self.standing = None;
                    self.state = State::Dragged;
                    self.drag_to(cx, cy);
                }
            }
            State::Dragged => {
                self.record(cx, cy);
                self.drag_to(cx, cy);
            }
            _ => {}
        }
    }

    /// 동물 교체 시 벽 타기 가능 여부를 바꾼다. 못 타는 동물로 바뀌었는데 벽에 붙어 있으면 떨어진다.
    pub fn set_can_climb(&mut self, can_climb: bool) {
        self.cfg.can_climb = can_climb;
        if !can_climb && matches!(self.state, State::Climb { .. } | State::Slide | State::WallClimb { .. }) {
            self.start_fall();
        }
    }

    pub fn set_hungry(&mut self, hungry: bool) {
        self.hungry = hungry;
        if !hungry && matches!(self.state, State::Hungry { .. }) {
            self.state = self.idle_state();
        }
    }

    /// 짧은 클릭으로 음식 고르기를 원하면 true를 한 번 돌려준다
    pub fn take_food_request(&mut self) -> bool {
        std::mem::take(&mut self.food_request)
    }

    /// 먹이 결과 반영. 공중·드래그 중이면 무시하고 false
    pub fn feed_result(&mut self, result: FeedResult, food: Food) -> bool {
        let grounded = self.standing.is_some() && !self.off_surface() && !self.is_held();
        if !grounded {
            return false;
        }
        self.pacing.after_beat = None;
        self.state = match result {
            FeedResult::Refused => {
                self.speech = Some(Speech::Full);
                State::Trick { kind: Trick::Tilt, remaining: REFUSE_TILT_SECS }
            }
            FeedResult::Ate { leveled_up } => {
                // 먹는 도중 또 먹어도 앞서 정해진 성장 축하·하트는 남긴다
                let (was_favorite, was_level_up) = match self.state {
                    State::Eating { favorite, level_up, .. } => (favorite, level_up),
                    _ => (false, false),
                };
                State::Eating { remaining: EAT_SECS, favorite: was_favorite || food == Food::Favorite, level_up: was_level_up || leveled_up }
            }
        };
        true
    }

    pub fn pointer_up(&mut self, cx: f64, cy: f64) {
        match self.state {
            State::Pressed { grounded: true, was_sleeping: true, .. } => self.click_sleeper(),
            State::Pressed { grounded: true, .. } => {
                // 먹는 중·성장 축하 중엔 클릭해도 하던 걸 계속한다
                if let Some((state @ (State::Eating { .. } | State::LevelUp { .. }), after_beat)) = self.before_press {
                    self.state = state;
                    self.pacing.after_beat = after_beat;
                    return;
                }
                if self.hungry {
                    self.food_request = true;
                    self.state = State::Hungry { remaining: behavior::range(&mut self.rng, HUNGRY_SECS) };
                    return;
                }
                let reaction = match behavior::pick_click_outcome(&mut self.rng, self.cfg.personality, self.hungry) {
                    ClickOutcome::React(r) => r,
                    ClickOutcome::Ignore { speak } => {
                        if let Some((state, after_beat)) = self.before_press {
                            self.state = state;
                            self.pacing.after_beat = after_beat;
                        }
                        if speak {
                            self.speech = Some(Speech::Ignore);
                        }
                        return;
                    }
                    ClickOutcome::Huff => {
                        self.speech = Some(Speech::Grumpy);
                        self.state = State::Huff { remaining: behavior::HUFF_SECS };
                        return;
                    }
                };
                match reaction {
                    ClickReaction::Hop => {
                        self.heart_timer = HEART_SECS;
                        self.standing = None;
                        self.body.vx = 0.0;
                        self.body.vy = HAPPY_VY;
                        self.state = State::Air { anim: Anim::Hop };
                    }
                    ClickReaction::Trick(kind) => {
                        self.state = State::Trick { kind, remaining: behavior::CLICK_TRICK_SECS };
                    }
                }
                if self.rng.f64() < CLICK_SPEECH_CHANCE {
                    self.speech = Some(Speech::Chatter);
                }
            }
            State::Pressed { .. } => self.start_fall(),
            State::Dragged => {
                self.record(cx, cy);
                let (vx, vy) = self.release_velocity();
                let (vx, vy) = clamp_speed(vx, vy);
                if vx.abs() > 50.0 {
                    self.facing = vx.signum();
                }
                self.standing = None;
                self.body.vx = vx;
                self.body.vy = vy;
                self.state = State::Air { anim: Anim::Fall };
            }
            _ => {}
        }
    }

    /// 자는 동물 클릭: 한 번은 뒤척이고(다시 잠), 2초 안에 세 번이면 기지개 켜며 깬다
    fn click_sleeper(&mut self) {
        let (count, first) = self.sleep_clicks;
        self.sleep_clicks = if count == 0 || self.clock - first > WAKE_CLICK_WINDOW { (1, self.clock) } else { (count + 1, first) };
        self.state = if self.sleep_clicks.0 >= WAKE_CLICKS {
            self.sleep_clicks = (0, 0.0);
            State::Trick { kind: Trick::Stretch, remaining: WAKE_STRETCH_SECS }
        } else {
            State::Annoyed { remaining: SLEEPY_ANNOYED_SECS, tickled_for: 0.0 }
        };
    }

    fn drag_to(&mut self, cx: f64, cy: f64) {
        self.body.x = cx + self.grab_dx;
        self.body.y = cy + self.grab_dy;
    }

    fn record(&mut self, cx: f64, cy: f64) {
        self.history.push_back((self.clock, cx, cy));
        while self.history.len() > 2 && self.clock - self.history[0].0 > VELOCITY_WINDOW {
            self.history.pop_front();
        }
    }

    fn release_velocity(&self) -> (f64, f64) {
        let (Some(&(t0, x0, y0)), Some(&(t1, x1, y1))) = (self.history.front(), self.history.back()) else {
            return (0.0, 0.0);
        };
        let dt = t1 - t0;
        if dt < 1e-6 {
            return (0.0, 0.0);
        }
        ((x1 - x0) / dt, (y1 - y0) / dt)
    }

    /// 서 있는 디딤판 기준점을 따라 위치 갱신. 기준점이 사라졌으면 false.
    fn follow_anchor(&mut self, world: &World) -> bool {
        let Some(st) = self.standing else { return false };
        let Some((ax, ay)) = world.anchor(st.owner) else { return false };
        self.body.x = ax + st.rel_x;
        self.body.y = ay;
        true
    }

    fn react_to_tickle(&mut self) {
        if !self.tickling {
            return;
        }
        if !matches!(self.state, State::Special { .. } | State::Eating { .. } | State::LevelUp { .. }) {
            self.pacing.after_beat = None;
        }
        self.state = match self.state {
            State::Sleep { .. } => State::Annoyed { remaining: TICKLE_HOLD, tickled_for: 0.0 },
            State::Annoyed { tickled_for, .. } => State::Annoyed { remaining: TICKLE_HOLD, tickled_for },
            State::Tickled { .. } => State::Tickled { remaining: TICKLE_HOLD },
            State::Special { .. } | State::Eating { .. } | State::LevelUp { .. } => return,
            _ => {
                if let Some(kind) = self.cfg.special {
                    if self.rng.f64() < kind.tickle_chance() {
                        self.start_special(kind);
                        return;
                    }
                }
                self.speech = Some(Speech::Tickle);
                State::Tickled { remaining: TICKLE_HOLD }
            }
        };
    }

    pub(crate) fn start_special(&mut self, kind: Special) {
        self.speech = Some(match kind {
            Special::Arch => Speech::Hiss,
            Special::Wag => Speech::Chatter,
        });
        self.state = State::Special { kind, remaining: behavior::range(&mut self.rng, kind.secs()) };
    }

    fn update_chatter(&mut self, dt: f64) {
        if matches!(
            self.state,
            State::Sleep { .. } | State::Annoyed { .. } | State::Tickled { .. } | State::Special { .. } | State::Eating { .. } | State::LevelUp { .. } | State::Huff { .. }
        ) {
            return;
        }
        self.chatter_timer -= dt;
        if self.chatter_timer <= 0.0 {
            if self.speech.is_some() {
                // 말풍선 자리가 비면 다음 틱에 다시 시도
                self.chatter_timer = 0.0;
                return;
            }
            self.speech = Some(if self.hungry { Speech::Hungry } else { Speech::Chatter });
            self.chatter_timer = behavior::range(&mut self.rng, if self.hungry { HUNGRY_TALK_SECS } else { CHATTER_SECS });
        }
    }

    fn update_ground(&mut self, dt: f64, world: &World) {
        if !self.follow_anchor(world) {
            self.start_fall();
            return;
        }
        let st = self.standing.unwrap();
        let Some(piece) = world.piece_at(st.owner, self.body.x).copied() else {
            self.start_fall();
            return;
        };
        if piece.width() < 2.0 * self.cfg.half_w && !matches!(self.state, State::Land { .. }) {
            self.start_hop(self.facing);
            return;
        }

        match self.state {
            State::Move { kind, remaining } => {
                let speed = self.cfg.walk_speed * kind.speed_mult();
                let new_x = self.body.x + self.facing * speed * dt;
                let (edge_x, end) = if self.facing > 0.0 { (piece.x_end, piece.end_kind) } else { (piece.x_start, piece.start_kind) };
                // x가 진행 방향으로 이 조각 끝을 넘었는지
                let facing = self.facing;
                let beyond = |x: f64| (x - edge_x) * facing > 0.0;
                let hit = beyond(new_x + self.facing * self.cfg.half_w);
                if let (true, EndKind::Seam { to }) = (hit, end) {
                    if !self.at_seam(world, to, kind, speed, new_x, beyond(new_x)) {
                        return;
                    }
                } else if hit {
                    match (end, kind) {
                        (EndKind::Edge, Move::Walk) if self.rng.f64() < behavior::WALK_EDGE_HOP_CHANCE => {
                            self.start_hop(self.facing)
                        }
                        (EndKind::Edge, Move::Run) => {
                            self.start_hop(self.facing);
                            self.body.vx *= behavior::RUN_LEAP_VX_MULT;
                        }
                        (EndKind::Edge, Move::Roll) => {
                            self.standing = None;
                            self.body.vx = self.facing * speed;
                            self.body.vy = 0.0;
                            self.state = State::Air { anim: Anim::Roll };
                        }
                        _ => self.state = State::Turn { remaining: TURN_PAUSE },
                    }
                    return;
                } else {
                    self.standing = Some(Standing { owner: st.owner, rel_x: st.rel_x + (new_x - self.body.x) });
                    self.body.x = new_x;
                }
                let remaining = remaining - dt;
                self.state = if remaining > 0.0 {
                    State::Move { kind, remaining }
                } else if self.rng.f64() < behavior::MOVE_END_TURN_CHANCE {
                    State::Turn { remaining: TURN_PAUSE }
                } else {
                    self.rest_beat()
                };
            }
            State::Idle { remaining, look } => {
                let remaining = remaining - dt;
                self.state = if remaining > 0.0 {
                    State::Idle { remaining, look }
                } else {
                    match self.pacing.after_beat.take() {
                        Some(AfterBeat::Go(next)) => next,
                        Some(AfterBeat::Rest) => self.rest_state(),
                        None => self.sleep_or_move(world),
                    }
                };
            }
            State::Trick { kind, remaining } => {
                let remaining = remaining - dt;
                self.state = if remaining > 0.0 { State::Trick { kind, remaining } } else { self.move_state(true, world) };
            }
            State::Huff { remaining } => {
                let remaining = remaining - dt;
                self.state = if remaining > 0.0 { State::Huff { remaining } } else { self.move_state(true, world) };
            }
            State::Tickled { remaining } => {
                let remaining = remaining - dt;
                self.state = if remaining > 0.0 { State::Tickled { remaining } } else { self.move_state(true, world) };
            }
            State::Annoyed { remaining, tickled_for } => {
                let tickled_for = if self.tickling { tickled_for + dt } else { tickled_for };
                if tickled_for >= TICKLE_WAKE_SECS {
                    self.speech = Some(Speech::Grumpy);
                    self.state = self.idle_state();
                } else {
                    let remaining = remaining - dt;
                    self.state = if remaining > 0.0 {
                        State::Annoyed { remaining, tickled_for }
                    } else {
                        State::Sleep { remaining: behavior::range(&mut self.rng, behavior::SLEEP_SECS) }
                    };
                }
            }
            State::Special { kind, remaining } => {
                let remaining = remaining - dt;
                self.state = if remaining > 0.0 { State::Special { kind, remaining } } else { self.move_state(true, world) };
            }
            State::Hungry { remaining } => {
                let remaining = remaining - dt;
                self.state = if remaining > 0.0 { State::Hungry { remaining } } else { self.move_state(true, world) };
            }
            State::Eating { remaining, favorite, level_up } => {
                let remaining = remaining - dt;
                self.state = if remaining > 0.0 {
                    State::Eating { remaining, favorite, level_up }
                } else if level_up {
                    self.speech = Some(Speech::LevelUp);
                    State::LevelUp { remaining: LEVEL_UP_SECS }
                } else if favorite {
                    self.next_after_beat(State::Trick { kind: Trick::Heart, remaining: AFTER_FAVORITE_HEART_SECS })
                } else {
                    self.move_state(true, world)
                };
            }
            State::LevelUp { remaining } => {
                let remaining = remaining - dt;
                self.state = if remaining > 0.0 { State::LevelUp { remaining } } else { self.move_state(true, world) };
            }
            State::Sleep { remaining } => {
                let remaining = remaining - dt;
                self.state = if remaining > 0.0 { State::Sleep { remaining } } else { self.idle_state() };
            }
            State::Turn { remaining } => {
                let remaining = remaining - dt;
                self.state = if remaining > 0.0 {
                    State::Turn { remaining }
                } else {
                    self.facing = -self.facing;
                    self.move_state(false, world)
                };
            }
            State::Land { remaining } => {
                let remaining = remaining - dt;
                self.state = if remaining > 0.0 { State::Land { remaining } } else { self.rest_beat() };
            }
            State::Air { .. } | State::Pressed { .. } | State::Dragged | State::Climb { .. } | State::Slide | State::WallClimb { .. } => {}
        }
    }

    fn update_air(&mut self, dt: f64, world: &World) {
        let (prev_x, prev_y, prev_vx) = (self.body.x, self.body.y, self.body.vx);
        step_air(&mut self.body, dt, &world.bounds, self.cfg.half_w, self.cfg.size);
        if self.body.vx.abs() > 1.0 {
            self.facing = self.body.vx.signum();
        }
        if self.cfg.can_climb && prev_vx.abs() > behavior::WALL_STICK_MIN_VX {
            if let Some(hit) = world.side_hit(prev_x, self.body.x, self.body.y, self.cfg.half_w) {
                let top = world.windows.iter().find(|w| w.id == hit.window).expect("hit window exists").rect.top;
                self.body = Body { x: self.wall_x(hit.x, hit.side), y: self.body.y, vx: 0.0, vy: 0.0 };
                self.state = State::WallClimb { window: hit.window, side: hit.side, rel_y: self.body.y - top };
                return;
            }
        }
        self.land_or_recover(prev_y, world);
    }

    /// 낙하·미끄러짐 공통: 이번 틱에 지나친 디딤판에 내려서거나, 화면 밖으로 사라졌으면 위에서 다시 떨어뜨린다
    fn land_or_recover(&mut self, prev_y: f64, world: &World) {
        let landing = world.landing(self.body.x, prev_y, self.body.y).copied().or_else(|| {
            // 이미 작업 영역 바닥보다 아래로 내려간 경우(작업표시줄 위에 놓음)엔 Ground를 바닥으로 취급
            if self.body.vy >= 0.0 { world.ground_below(self.body.x, self.body.y).copied() } else { None }
        });
        if let Some(s) = landing {
            let (ax, _) = world.anchor(s.owner).expect("surface owner has anchor");
            self.body = Body { x: self.body.x, y: s.y, vx: 0.0, vy: 0.0 };
            self.standing = Some(Standing { owner: s.owner, rel_x: self.body.x - ax });
            self.state = State::Land { remaining: LAND_SECS };
        } else if self.body.y > world.bounds.bottom + self.cfg.size * 4.0 {
            // 화면 밖으로 사라짐 → 가까운 위치 상단에서 다시 떨어뜨림
            let b = world.bounds;
            self.body = Body {
                x: self.body.x.clamp(b.left + self.cfg.half_w, b.right - self.cfg.half_w),
                y: b.top + self.cfg.size,
                vx: 0.0,
                vy: 0.0,
            };
        }
    }

    /// 크기 128 기준 속도를 지금 크기에 맞춘다
    fn scaled(&self, speed: f64) -> f64 {
        speed * self.cfg.size / 128.0
    }

    /// 창 옆면 바깥에 붙어 있을 때 몸 중심 x
    fn wall_x(&self, edge_x: f64, side: Side) -> f64 {
        match side {
            Side::Left => edge_x - self.cfg.half_w,
            Side::Right => edge_x + self.cfg.half_w,
        }
    }

    /// 화면 타기 목표 발 높이 — 발밑 모니터 작업 영역 기준. 이미 그보다 높이 서 있으면 None
    fn climb_target(&mut self, world: &World) -> Option<f64> {
        let wa = world.work_area_at(self.body.x, self.body.y);
        let target_y = behavior::climb_target(&mut self.rng, wa.bottom, wa.top + self.cfg.size);
        (self.body.y > target_y).then_some(target_y)
    }

    /// 지금 서 있는 곳에서 화면 타기를 시작한다 (테스트용). 목표가 발보다 낮으면 false
    #[cfg(test)]
    pub(crate) fn start_climb(&mut self, world: &World) -> bool {
        let Some(target_y) = self.climb_target(world) else { return false };
        self.pacing.after_beat = None;
        self.state = State::Climb { target_y, hang: behavior::CLIMB_HANG_SECS };
        self.on_started();
        true
    }

    /// 새 상태가 시작될 때 공통 처리: 실제로 시작한 이동만 관성으로 기억하고, 화면 타기는 디딤판을 떠난다
    fn on_started(&mut self) {
        match self.state {
            State::Move { kind, .. } => self.pacing.last_move = Some(kind),
            State::Climb { .. } if self.standing.is_some() => {
                self.standing = None;
                self.body.vx = 0.0;
                self.body.vy = 0.0;
            }
            _ => {}
        }
    }

    fn update_climb(&mut self, dt: f64, target_y: f64, hang: f64) {
        if self.body.y > target_y {
            self.body.y = (self.body.y - self.scaled(behavior::CLIMB_SPEED) * dt).max(target_y);
            return;
        }
        let hang = hang - dt;
        self.state = if hang > 0.0 { State::Climb { target_y, hang } } else { State::Slide };
    }

    fn update_slide(&mut self, dt: f64, world: &World) {
        let prev_y = self.body.y;
        self.body.vx = 0.0;
        self.body.vy = self.scaled(behavior::SLIDE_SPEED);
        self.body.y += self.body.vy * dt;
        self.land_or_recover(prev_y, world);
    }

    fn update_wall_climb(&mut self, dt: f64, world: &World, window: u64, side: Side, rel_y: f64) {
        let Some(rect) = world.windows.iter().find(|w| w.id == window).map(|w| w.rect) else {
            self.start_fall();
            return;
        };
        let edge_x = match side {
            Side::Left => rect.left,
            Side::Right => rect.right,
        };
        let rel_y = rel_y - self.scaled(behavior::WALL_CLIMB_SPEED) * dt;
        if rel_y <= 0.0 {
            // 꼭대기: 옆면 바로 안쪽 윗면에 올라선다
            let inset = self.cfg.half_w + behavior::WALL_TOP_INSET;
            let rel_x = match side {
                Side::Left => inset,
                Side::Right => rect.width() - inset,
            };
            self.standing = Some(Standing { owner: Owner::Window(window), rel_x });
            self.body = Body { x: rect.left + rel_x, y: rect.top, vx: 0.0, vy: 0.0 };
            self.state = State::Land { remaining: LAND_SECS };
            return;
        }
        let y = rect.top + rel_y;
        if world.side_covered(window, edge_x, y) != Some(false) {
            self.start_fall();
            return;
        }
        self.body = Body { x: self.wall_x(edge_x, side), y, vx: 0.0, vy: 0.0 };
        self.state = State::WallClimb { window, side, rel_y };
    }

    /// 몸 앞쪽이 옆 모니터 경계(Seam)를 넘는 중. 이웃 바닥 높이에 따라 그대로 걸어 넘어가거나(true — 서 있을 곳과
    /// x를 이미 옮겼다), 뛰어내리거나 뛰어오르거나 굴러떨어지거나 돌아선다(false).
    /// center_past = 발 중심이 이 바닥 끝을 넘었는지
    fn at_seam(&mut self, world: &World, to: usize, kind: Move, speed: f64, new_x: f64, center_past: bool) -> bool {
        let (Some(st), Some(wa)) = (self.standing, world.work_areas.get(to).copied()) else {
            self.state = State::Turn { remaining: TURN_PAUSE };
            return false;
        };
        // + = 이웃 바닥이 더 높다
        let rise = self.body.y - wa.bottom;
        if rise.abs() <= SEAM_LEVEL_TOLERANCE || rise < 0.0 {
            if !center_past {
                // 발 중심이 넘어갈 때까지 지금 바닥에서 계속 걷는다
                self.standing = Some(Standing { owner: st.owner, rel_x: st.rel_x + (new_x - self.body.x) });
                self.body.x = new_x;
                return true;
            }
            if rise.abs() <= SEAM_LEVEL_TOLERANCE {
                // 같은 높이: 이웃 바닥으로 갈아타고 그대로 걷는다
                let x = new_x.clamp(wa.left, wa.right);
                self.standing = Some(Standing { owner: Owner::Ground(to), rel_x: x - wa.left });
                self.body.x = x;
                self.body.y = wa.bottom;
                return true;
            }
            // 이웃이 더 낮다: 경계 너머에서 뛰어내리거나 굴러떨어진다 (지금 바닥에 다시 내려서지 않도록)
            self.body.x = new_x;
            if kind == Move::Roll {
                self.standing = None;
                self.body.vx = self.facing * speed;
                self.body.vy = 0.0;
                self.state = State::Air { anim: Anim::Roll };
            } else {
                self.start_hop(self.facing);
            }
            return false;
        }
        if kind == Move::Roll || rise > self.scaled(SEAM_MAX_HOP_UP) {
            self.state = State::Turn { remaining: TURN_PAUSE };
            return false;
        }
        // 이웃이 조금 높다: 높이 차 + 여유만큼 솟구치고, 내려오며 이웃 바닥에 몸 전체가 들어가도록 vx를 맞춘다
        let margin = self.scaled(SEAM_HOP_MARGIN);
        let vy = -(2.0 * GRAVITY * (rise + margin)).sqrt();
        let t = -vy / GRAVITY + (2.0 * margin / GRAVITY).sqrt();
        let target_x = if self.facing > 0.0 { wa.left + self.cfg.half_w } else { wa.right - self.cfg.half_w };
        self.start_hop(self.facing);
        self.body.vx = self.facing * HOP_VX.max((target_x - self.body.x).abs() / t);
        self.body.vy = vy;
        false
    }

    pub(crate) fn start_hop(&mut self, dir: f64) {
        self.pacing.after_beat = None;
        self.standing = None;
        self.body.vx = dir * HOP_VX;
        self.body.vy = HOP_VY;
        self.state = State::Air { anim: Anim::Hop };
    }

    fn start_fall(&mut self) {
        self.pacing.after_beat = None;
        self.standing = None;
        self.body.vx = 0.0;
        self.body.vy = 0.0;
        self.state = State::Air { anim: Anim::Fall };
    }

    fn idle_state(&mut self) -> State {
        State::Idle { remaining: behavior::range(&mut self.rng, behavior::IDLE_SECS), look: self.rng.bool() }
    }

    /// 활동 사이의 짧은 옆모습 앉기
    fn beat(&mut self, then: AfterBeat) -> State {
        self.pacing.after_beat = Some(then);
        State::Idle { remaining: behavior::range(&mut self.rng, behavior::BEAT_SECS), look: false }
    }

    /// 지금 활동과 묶음이 다르면 먼저 잠깐 앉았다가 next로 간다 (Turn은 직전 이동 방식 기준)
    fn next_after_beat(&mut self, next: State) -> State {
        let current = match self.state {
            State::Turn { .. } => self.pacing.last_move.map(Move::family),
            s => s.family(),
        };
        match (current, next.family()) {
            (Some(a), Some(b)) if a != b => self.beat(AfterBeat::Go(next)),
            _ => next,
        }
    }

    /// 이동 시작. may_flip이면 절반 확률로 방향을 바꾼다 (Turn 직후엔 이미 바꿨으므로 false).
    fn move_state(&mut self, may_flip: bool, world: &World) -> State {
        if self.cfg.can_climb && self.standing.is_some() && self.rng.f64() < behavior::CLIMB_CHANCE {
            if let Some(target_y) = self.climb_target(world) {
                return self.next_after_beat(State::Climb { target_y, hang: behavior::CLIMB_HANG_SECS });
            }
        }
        if may_flip && self.rng.bool() {
            self.facing = -self.facing;
        }
        let after_front = self.state.family() == Some(Family::Front);
        let kind = behavior::pick_next_move(&mut self.rng, self.pacing.last_move, after_front);
        let next = State::Move { kind, remaining: behavior::range(&mut self.rng, kind.secs()) };
        self.next_after_beat(next)
    }

    /// 이동·착지 뒤엔 항상 잠깐 앉고, 그 뒤에 무엇을 할지 정한다
    fn rest_beat(&mut self) -> State {
        self.beat(AfterBeat::Rest)
    }

    /// 쉬는 박자가 끝난 뒤: 시무룩 / 특별 동작 / 앞모습 애교 / 잠 / 이동
    fn rest_state(&mut self) -> State {
        if self.hungry && self.rng.f64() < HUNGRY_REST_CHANCE {
            return State::Hungry { remaining: behavior::range(&mut self.rng, HUNGRY_SECS) };
        }
        if let Some(kind) = self.cfg.special {
            if self.rng.f64() < kind.rest_chance() {
                self.start_special(kind);
                return self.state;
            }
        }
        match behavior::pick_rest(&mut self.rng, self.cfg.personality.trick_chance(self.hungry)) {
            // 박자에 이어 조금 더 앉아 있다가 잠/이동을 정한다
            Rest::Idle => self.idle_state(),
            Rest::Trick(kind) => State::Trick { kind, remaining: behavior::range(&mut self.rng, behavior::TRICK_SECS) },
        }
    }

    fn sleep_or_move(&mut self, world: &World) -> State {
        if self.rng.f64() < behavior::SLEEP_CHANCE {
            State::Sleep { remaining: behavior::range(&mut self.rng, behavior::SLEEP_SECS) }
        } else {
            self.move_state(true, world)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::behavior::{Move, Trick};
    use crate::geom::{Rect, WindowInfo};

    const DT: f64 = 1.0 / 60.0;

    pub(super) fn cfg() -> PetConfig {
        PetConfig { size: 128.0, half_w: 38.0, walk_speed: 60.0, special: None, personality: Personality::Friendly, can_climb: false }
    }

    pub(super) fn win(id: u64, l: f64, t: f64, r: f64, b: f64) -> WindowInfo {
        WindowInfo { id, rect: Rect::new(l, t, r, b) }
    }

    /// windows는 앞→뒤 순서
    pub(super) fn world(windows: Vec<WindowInfo>) -> World {
        World::new(windows, vec![Rect::new(0.0, 0.0, 1920.0, 1040.0)], 128.0)
    }

    pub(super) fn w1() -> WindowInfo {
        win(1, 500.0, 400.0, 1100.0, 900.0)
    }

    pub(super) fn place(p: &mut Pet, w: &World, owner: Owner, x: f64, state: State) {
        let (ax, ay) = w.anchor(owner).unwrap();
        p.standing = Some(Standing { owner, rel_x: x - ax });
        p.body = Body { x, y: ay, vx: 0.0, vy: 0.0 };
        p.state = state;
    }

    pub(super) fn run(p: &mut Pet, w: &World, ticks: usize) {
        for _ in 0..ticks {
            p.update(DT, w);
        }
    }

    /// 착지·쉬는 박자가 끝나 다음 활동이 정해질 때까지 돌린다
    pub(super) fn run_past_beat(p: &mut Pet, w: &World) {
        for _ in 0..200 {
            p.update(DT, w);
            if !matches!(p.state, State::Land { .. } | State::Idle { .. }) {
                return;
            }
        }
    }

    #[test]
    fn falls_and_lands_on_window_top() {
        let w = world(vec![w1()]);
        let mut p = Pet::new(cfg(), 800.0, 200.0, 1);
        run(&mut p, &w, 120);
        assert_eq!(p.standing.unwrap().owner, Owner::Window(1));
        assert_eq!(p.body.y, 400.0);
    }

    #[test]
    fn falls_to_ground_when_nothing_below() {
        let w = world(vec![w1()]);
        let mut p = Pet::new(cfg(), 200.0, 200.0, 1);
        run(&mut p, &w, 120);
        assert_eq!(p.standing.unwrap().owner, Owner::Ground(0));
        assert_eq!(p.body.y, 1040.0);
    }

    #[test]
    fn released_below_ground_line_lands_on_ground() {
        let w = world(vec![]);
        let mut p = Pet::new(cfg(), 800.0, 1090.0, 1);
        p.body.vy = 0.0;
        p.state = State::Air { anim: Anim::Fall };
        p.update(DT, &w);
        assert_eq!(p.standing.unwrap().owner, Owner::Ground(0));
        assert_eq!(p.body.y, 1040.0);
        assert!(matches!(p.state, State::Land { .. }));
    }

    #[test]
    fn landing_shows_land_then_idle() {
        let w = world(vec![w1()]);
        let mut p = Pet::new(cfg(), 800.0, 390.0, 1);
        run(&mut p, &w, 10);
        assert!(matches!(p.state, State::Land { .. }));
        run(&mut p, &w, 30);
        assert!(matches!(p.state, State::Idle { .. }));
    }

    #[test]
    fn rides_window_when_it_moves() {
        let w = world(vec![w1()]);
        let mut p = Pet::new(cfg(), 0.0, 0.0, 1);
        place(&mut p, &w, Owner::Window(1), 800.0, State::Idle { remaining: 100.0, look: false });
        let moved = world(vec![win(1, 600.0, 450.0, 1200.0, 950.0)]);
        p.update(DT, &moved);
        assert_eq!((p.body.x, p.body.y), (900.0, 450.0));
    }

    #[test]
    fn falls_when_window_disappears() {
        let w = world(vec![w1()]);
        let mut p = Pet::new(cfg(), 0.0, 0.0, 1);
        place(&mut p, &w, Owner::Window(1), 800.0, State::Idle { remaining: 100.0, look: false });
        p.update(DT, &world(vec![]));
        assert_eq!(p.state, State::Air { anim: Anim::Fall });
        assert!(p.standing.is_none());
    }

    #[test]
    fn falls_when_covered_by_front_window() {
        let w = world(vec![w1()]);
        let mut p = Pet::new(cfg(), 0.0, 0.0, 1);
        place(&mut p, &w, Owner::Window(1), 800.0, State::Idle { remaining: 100.0, look: false });
        let covered = world(vec![win(2, 700.0, 300.0, 900.0, 600.0), w1()]);
        p.update(DT, &covered);
        assert_eq!(p.state, State::Air { anim: Anim::Fall });
    }

    #[test]
    fn blocked_end_turns_around() {
        let w = world(vec![win(2, 900.0, 300.0, 1300.0, 600.0), w1()]);
        let mut p = Pet::new(cfg(), 0.0, 0.0, 1);
        place(&mut p, &w, Owner::Window(1), 840.0, State::Move { kind: Move::Walk, remaining: 100.0 });
        p.facing = 1.0;
        let mut turned = false;
        for _ in 0..60 {
            p.update(DT, &w);
            assert!(!matches!(p.state, State::Air { .. }));
            if matches!(p.state, State::Turn { .. }) {
                turned = true;
                break;
            }
        }
        assert!(turned);
        run(&mut p, &w, 20);
        assert_eq!(p.facing, -1.0);
        assert!(matches!(p.state, State::Move { .. }));
    }

    #[test]
    fn wall_end_always_turns() {
        let w = world(vec![]);
        for seed in 0..50 {
            let mut p = Pet::new(cfg(), 0.0, 0.0, seed);
            place(&mut p, &w, Owner::Ground(0), 1920.0 - 38.0 - 0.5, State::Move { kind: Move::Walk, remaining: 100.0 });
            p.facing = 1.0;
            p.update(DT, &w);
            assert!(matches!(p.state, State::Turn { .. }), "seed {seed}");
        }
    }

    #[test]
    fn walking_to_a_window_edge_hops_about_80_percent() {
        let w = world(vec![w1()]);
        let (mut hops, mut turns) = (0, 0);
        for seed in 0..400 {
            let mut p = Pet::new(cfg(), 0.0, 0.0, seed);
            place(&mut p, &w, Owner::Window(1), 1100.0 - 38.0 - 0.5, State::Move { kind: Move::Walk, remaining: 100.0 });
            p.facing = 1.0;
            p.update(DT, &w);
            match p.state {
                State::Air { anim: Anim::Hop } => hops += 1,
                State::Turn { .. } => turns += 1,
                s => panic!("unexpected {s:?}"),
            }
        }
        assert!((280..=360).contains(&hops), "hops={hops}");
        assert_eq!(hops + turns, 400);
    }

    #[test]
    fn hop_off_edge_leaves_the_window() {
        let w = world(vec![w1()]);
        let mut p = Pet::new(cfg(), 0.0, 0.0, 1);
        place(&mut p, &w, Owner::Window(1), 1061.0, State::Move { kind: Move::Walk, remaining: 100.0 });
        p.facing = 1.0;
        p.start_hop(1.0);
        run(&mut p, &w, 120);
        assert_eq!(p.standing.unwrap().owner, Owner::Ground(0));
    }

    #[test]
    fn narrow_piece_makes_pet_hop_down() {
        let w = world(vec![win(2, 550.0, 300.0, 1300.0, 600.0), w1()]);
        let mut p = Pet::new(cfg(), 0.0, 0.0, 1);
        place(&mut p, &w, Owner::Window(1), 525.0, State::Idle { remaining: 100.0, look: false });
        p.update(DT, &w);
        assert_eq!(p.state, State::Air { anim: Anim::Hop });
    }

    #[test]
    fn expired_move_becomes_a_calm_beat_or_turn() {
        let w = world(vec![]);
        let (mut beats, mut turns) = (0, 0);
        for seed in 0..300 {
            for kind in [Move::Walk, Move::Run, Move::Roll] {
                let mut p = Pet::new(cfg(), 0.0, 0.0, seed);
                place(&mut p, &w, Owner::Ground(0), 900.0, State::Move { kind, remaining: 0.001 });
                p.update(DT, &w);
                match p.state {
                    State::Idle { remaining, look: false } => {
                        assert!((0.6..=1.5).contains(&remaining), "{remaining}");
                        beats += 1;
                    }
                    State::Turn { .. } => turns += 1,
                    s => panic!("seed {seed} {kind:?}: {s:?}"),
                }
            }
        }
        assert!(beats > 500 && turns > 150, "beats {beats} turns {turns}");
    }

    /// 쉬기 결정이 그냥 앉기면 박자 뒤에 IDLE_SECS 동안 더 앉아 있다가 잠/이동을 정한다
    #[test]
    fn plain_rest_sits_for_idle_secs_after_the_beat() {
        let w = world(vec![]);
        let mut sits = 0;
        for seed in 0..300 {
            let mut p = Pet::new(cfg(), 0.0, 0.0, seed);
            place(&mut p, &w, Owner::Ground(0), 900.0, State::Idle { remaining: 0.001, look: false });
            p.pacing.after_beat = Some(AfterBeat::Rest);
            p.update(DT, &w);
            match p.state {
                State::Idle { remaining, .. } => {
                    assert!((1.5..=3.0).contains(&remaining), "seed {seed}: {remaining}");
                    assert_eq!(p.pacing.after_beat, None);
                    sits += 1;
                    for _ in 0..(60 * 3 + 1) {
                        p.update(DT, &w);
                        if !matches!(p.state, State::Idle { .. }) {
                            break;
                        }
                    }
                    assert!(matches!(p.state, State::Sleep { .. } | State::Move { .. }), "seed {seed}: {:?}", p.state);
                }
                State::Trick { .. } => {}
                s => panic!("seed {seed}: {s:?}"),
            }
        }
        // 애교 55% 아니면 앉기
        assert!((100..=170).contains(&sits), "{sits}");
    }

    #[test]
    fn momentum_remembers_only_moves_that_really_started() {
        let w = world(vec![]);
        let mut checked = 0;
        for seed in 0..200 {
            let mut p = Pet::new(cfg(), 0.0, 0.0, seed);
            place(&mut p, &w, Owner::Ground(0), 900.0, State::Trick { kind: Trick::Wave, remaining: 0.001 });
            p.pacing.last_move = Some(Move::Run);
            p.update(DT, &w);
            let Some(AfterBeat::Go(State::Move { kind, .. })) = p.pacing.after_beat else { panic!("seed {seed}: {:?}", p.pacing.after_beat) };
            assert_eq!(p.pacing.last_move, Some(Move::Run), "pending move not yet started");
            if kind == Move::Run {
                continue;
            }
            for _ in 0..120 {
                p.update(DT, &w);
                if matches!(p.state, State::Move { .. }) {
                    break;
                }
            }
            assert_eq!(p.pacing.last_move, Some(kind), "seed {seed}");
            checked += 1;
        }
        assert!(checked > 50, "{checked}");
    }

    #[test]
    fn trick_ends_with_a_beat_then_moves() {
        let w = world(vec![]);
        for seed in 0..50 {
            let mut p = Pet::new(cfg(), 0.0, 0.0, seed);
            place(&mut p, &w, Owner::Ground(0), 900.0, State::Trick { kind: Trick::Wave, remaining: 0.001 });
            p.update(DT, &w);
            assert!(matches!(p.state, State::Idle { look: false, .. }), "seed {seed}: {:?}", p.state);
            for _ in 0..(60 * 2) {
                p.update(DT, &w);
                if !matches!(p.state, State::Idle { .. }) {
                    break;
                }
            }
            assert!(matches!(p.state, State::Move { .. }), "seed {seed}: {:?}", p.state);
        }
    }

    #[test]
    fn walk_and_roll_switch_through_a_beat_after_a_turn() {
        let w = world(vec![]);
        for seed in 0..200 {
            let mut p = Pet::new(cfg(), 0.0, 0.0, seed);
            place(&mut p, &w, Owner::Ground(0), 900.0, State::Turn { remaining: 0.001 });
            p.pacing.last_move = Some(Move::Walk);
            p.update(DT, &w);
            assert!(!matches!(p.state, State::Move { kind: Move::Roll, .. }), "seed {seed}");
        }
    }

    #[test]
    fn expired_idle_becomes_sleep_or_walk() {
        let w = world(vec![]);
        let mut p = Pet::new(cfg(), 0.0, 0.0, 3);
        place(&mut p, &w, Owner::Ground(0), 900.0, State::Idle { remaining: 0.001, look: true });
        p.update(DT, &w);
        assert!(matches!(p.state, State::Sleep { .. } | State::Move { .. }));
    }

    #[test]
    fn render_maps_state_and_facing() {
        let mut p = Pet::new(cfg(), 0.0, 0.0, 1);
        p.state = State::Idle { remaining: 1.0, look: true };
        p.facing = -1.0;
        assert_eq!(p.render(), RenderState { anim: Anim::Look, flip: true, heart: false });
        p.state = State::Turn { remaining: 0.1 };
        assert_eq!(p.render().anim, Anim::Idle);
    }

    #[test]
    fn window_pos_is_top_left_of_sprite() {
        let mut p = Pet::new(cfg(), 0.0, 0.0, 1);
        p.body = Body { x: 800.0, y: 400.0, vx: 0.0, vy: 0.0 };
        assert_eq!(p.window_pos(), (736.0, 272.0));
    }

    #[test]
    fn running_moves_two_and_a_half_times_faster() {
        let w = world(vec![]);
        let mut p = Pet::new(cfg(), 0.0, 0.0, 1);
        place(&mut p, &w, Owner::Ground(0), 900.0, State::Move { kind: Move::Run, remaining: 100.0 });
        p.facing = 1.0;
        p.update(DT, &w);
        assert!((p.body.x - (900.0 + 60.0 * 2.5 * DT)).abs() < 1e-9);
    }

    #[test]
    fn running_off_a_window_edge_always_leaps_far() {
        let w = world(vec![w1()]);
        for seed in 0..50 {
            let mut p = Pet::new(cfg(), 0.0, 0.0, seed);
            place(&mut p, &w, Owner::Window(1), 1100.0 - 38.0 - 0.5, State::Move { kind: Move::Run, remaining: 100.0 });
            p.facing = 1.0;
            p.update(DT, &w);
            assert_eq!(p.state, State::Air { anim: Anim::Hop }, "seed {seed}");
            assert!(p.body.vx > 300.0);
        }
    }

    #[test]
    fn rolling_off_a_window_edge_keeps_rolling_speed() {
        let w = world(vec![w1()]);
        let mut p = Pet::new(cfg(), 0.0, 0.0, 1);
        place(&mut p, &w, Owner::Window(1), 1100.0 - 38.0 - 0.5, State::Move { kind: Move::Roll, remaining: 100.0 });
        p.facing = 1.0;
        p.update(DT, &w);
        assert_eq!(p.state, State::Air { anim: Anim::Roll });
        assert_eq!((p.body.vx, p.body.vy), (120.0, 0.0));
        assert!(p.standing.is_none());
    }

    #[test]
    fn blocked_end_turns_for_every_move_kind() {
        let w = world(vec![win(2, 900.0, 300.0, 1300.0, 600.0), w1()]);
        for kind in [Move::Walk, Move::Run, Move::Roll] {
            let mut p = Pet::new(cfg(), 0.0, 0.0, 1);
            place(&mut p, &w, Owner::Window(1), 900.0 - 38.0 - 0.5, State::Move { kind, remaining: 100.0 });
            p.facing = 1.0;
            p.update(DT, &w);
            assert!(matches!(p.state, State::Turn { .. }), "{kind:?}");
        }
    }

    #[test]
    fn tricks_face_front_and_end_by_moving() {
        let w = world(vec![]);
        let mut p = Pet::new(cfg(), 0.0, 0.0, 1);
        place(&mut p, &w, Owner::Ground(0), 900.0, State::Trick { kind: Trick::Wave, remaining: 0.5 });
        p.facing = -1.0;
        assert_eq!(p.render(), RenderState { anim: Anim::Wave, flip: false, heart: false });
        run(&mut p, &w, 40 + 90);
        assert!(matches!(p.state, State::Move { .. }));
    }

    #[test]
    fn covered_during_trick_falls() {
        let w = world(vec![w1()]);
        let mut p = Pet::new(cfg(), 0.0, 0.0, 1);
        place(&mut p, &w, Owner::Window(1), 800.0, State::Trick { kind: Trick::Belly, remaining: 3.0 });
        p.update(DT, &world(vec![win(2, 700.0, 300.0, 900.0, 600.0), w1()]));
        assert_eq!(p.state, State::Air { anim: Anim::Fall });
    }

    #[test]
    fn side_animations_still_flip() {
        let mut p = Pet::new(cfg(), 0.0, 0.0, 1);
        p.facing = -1.0;
        p.state = State::Move { kind: Move::Roll, remaining: 1.0 };
        assert_eq!(p.render(), RenderState { anim: Anim::Roll, flip: true, heart: false });
    }

    #[test]
    fn front_animations_are_the_tricks_and_tickle() {
        let all = [Anim::Walk, Anim::Run, Anim::Roll, Anim::Idle, Anim::Look, Anim::Sleep, Anim::Hop, Anim::Fall, Anim::Dragged,
            Anim::Land, Anim::SitFront, Anim::Wave, Anim::Tilt, Anim::Belly, Anim::Stretch, Anim::Heart, Anim::Tickle, Anim::Annoyed, Anim::Arch, Anim::Wag, Anim::Hungry, Anim::Eat, Anim::LevelUp, Anim::Huff, Anim::Climb, Anim::Slide];
        let front: Vec<Anim> = all.into_iter().filter(|a| a.is_front()).collect();
        assert_eq!(front, vec![Anim::SitFront, Anim::Wave, Anim::Tilt, Anim::Belly, Anim::Stretch, Anim::Heart, Anim::Tickle, Anim::Wag, Anim::Hungry, Anim::Eat, Anim::LevelUp, Anim::Huff]);
        assert_eq!(serde_json::to_string(&Anim::SitFront).unwrap(), "\"sit_front\"");
    }

    #[test]
    fn every_anim_name_is_a_required_manifest_animation() {
        let all = [Anim::Walk, Anim::Run, Anim::Roll, Anim::Idle, Anim::Look, Anim::Sleep, Anim::Hop, Anim::Fall, Anim::Dragged,
            Anim::Land, Anim::SitFront, Anim::Wave, Anim::Tilt, Anim::Belly, Anim::Stretch, Anim::Heart, Anim::Tickle, Anim::Annoyed, Anim::Hungry, Anim::Eat, Anim::LevelUp, Anim::Arch, Anim::Wag, Anim::Huff, Anim::Climb, Anim::Slide];
        let names: Vec<String> = all.iter().map(|a| serde_json::to_string(a).unwrap().trim_matches('"').to_string()).collect();
        let mut expected: Vec<&str> = crate::manifest::REQUIRED_ANIMS.to_vec();
        expected.extend(crate::manifest::OPTIONAL_ANIMS);
        assert_eq!(names, expected);
    }

    /// 10분 시뮬레이션 — "계속 자기만 한다" 회귀 방지 (spec §6)
    #[test]
    fn ten_minutes_are_mostly_moving_and_rarely_sleeping() {
        let w = world(vec![]);
        let (mut moving, mut sleeping, mut front, mut total) = (0usize, 0usize, 0usize, 0usize);
        for seed in 0..20 {
            let mut p = Pet::new(sim_cfg(), 960.0, 128.0, seed);
            for _ in 0..(60 * 600) {
                p.update(DT, &w);
                let a = p.render().anim;
                match a {
                    Anim::Walk | Anim::Run | Anim::Roll => moving += 1,
                    Anim::Sleep => sleeping += 1,
                    _ => {}
                }
                if a.is_front() {
                    front += 1;
                }
                total += 1;
            }
        }
        let (m, s, f) = (moving as f64 / total as f64, sleeping as f64 / total as f64, front as f64 / total as f64);
        // 2B §6 재측정 (쉬는 앉기 IDLE_SECS 반영): moving 0.584, front 0.139, sleeping 0.068
        assert!((0.53..=0.63).contains(&m), "moving {m:.3} (front {f:.3}, sleeping {s:.3})");
        assert!(f >= 0.109, "front-facing {f:.3} (moving {m:.3}, sleeping {s:.3})");
        assert!(s <= 0.12, "sleeping {s:.3} (moving {m:.3}, front {f:.3})");
    }

    pub(super) fn sim_cfg() -> PetConfig {
        PetConfig { size: 128.0, half_w: 38.4, walk_speed: 90.0, special: None, personality: Personality::Friendly, can_climb: false }
    }

    /// 10분 동안 바뀐 동작 순서 (같은 동작 연속은 하나로)
    fn anim_sequence(cfg: PetConfig, seed: u64) -> Vec<Anim> {
        let w = world(vec![win(1, 300.0, 600.0, 1100.0, 900.0)]);
        let mut p = Pet::new(cfg, 960.0, 128.0, seed);
        let mut seq: Vec<Anim> = Vec::new();
        for _ in 0..(60 * 600) {
            p.update(DT, &w);
            let a = p.render().anim;
            if seq.last() != Some(&a) {
                seq.push(a);
            }
        }
        seq
    }

    fn family(a: Anim) -> Option<behavior::Family> {
        match a {
            Anim::Walk | Anim::Run => Some(behavior::Family::Move),
            Anim::Roll => Some(behavior::Family::Roll),
            Anim::Arch | Anim::Wag => Some(behavior::Family::Special),
            a if a.is_front() => Some(behavior::Family::Front),
            _ => None,
        }
    }

    #[test]
    fn activities_only_change_family_after_sitting_a_moment() {
        for special in [None, Some(Special::Arch), Some(Special::Wag)] {
            for personality in [Personality::Friendly, Personality::Aloof] {
                for seed in 0..6 {
                    let seq = anim_sequence(PetConfig { special, personality, ..sim_cfg() }, seed);
                    // 공중·착지 동작은 건너뛰고, Idle/Look/Sleep을 만나면 초기화
                    let mut last: Option<behavior::Family> = None;
                    for (i, &a) in seq.iter().enumerate() {
                        match a {
                            Anim::Idle | Anim::Look | Anim::Sleep | Anim::Annoyed => last = None,
                            Anim::Hop | Anim::Fall | Anim::Land | Anim::Dragged => {}
                            _ => {
                                let f = family(a);
                                assert!(last.is_none() || last == f, "{special:?} {personality:?} seed {seed}: {:?}", &seq[i.saturating_sub(4)..=i]);
                                last = f;
                            }
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn consecutive_moves_usually_keep_their_kind() {
        let (mut same, mut pairs) = (0usize, 0usize);
        for seed in 0..20 {
            let moves: Vec<Anim> = anim_sequence(sim_cfg(), seed).into_iter().filter(|a| matches!(a, Anim::Walk | Anim::Run | Anim::Roll)).collect();
            for pair in moves.windows(2) {
                pairs += 1;
                same += (pair[0] == pair[1]) as usize;
            }
        }
        let share = same as f64 / pairs as f64;
        assert!(share >= 0.45, "repeat {share:.3} of {pairs}");
    }
}

#[cfg(test)]
mod input_tests {
    use super::tests::{cfg, place, run, w1, world};
    use super::*;
    use crate::behavior::Trick;

    const DT: f64 = 1.0 / 60.0;

    fn idle() -> State {
        State::Idle { remaining: 100.0, look: false }
    }

    #[test]
    fn click_on_grounded_pet_reacts_with_hop_or_trick() {
        let w = world(vec![w1()]);
        let mut seen = std::collections::HashSet::new();
        for seed in 0..80 {
            let mut p = Pet::new(cfg(), 0.0, 0.0, seed);
            place(&mut p, &w, Owner::Window(1), 800.0, idle());
            p.pointer_down(800.0, 350.0);
            p.pointer_up(800.0, 350.0);
            match p.state {
                State::Air { anim: Anim::Hop } => {
                    assert!(p.render().heart);
                    run(&mut p, &w, 120);
                    assert_eq!(p.standing.unwrap().owner, Owner::Window(1));
                    seen.insert("hop");
                }
                State::Trick { kind, remaining } => {
                    assert_eq!(remaining, 2.0);
                    assert!(p.standing.is_some());
                    seen.insert(match kind {
                        Trick::Heart => "heart",
                        Trick::Wave => "wave",
                        Trick::Tilt => "tilt",
                        k => panic!("unexpected {k:?}"),
                    });
                }
                s => panic!("unexpected {s:?}"),
            }
        }
        assert_eq!(seen.len(), 4);
    }

    #[test]
    fn tiny_move_still_counts_as_click() {
        let w = world(vec![w1()]);
        let mut p = Pet::new(cfg(), 0.0, 0.0, 1);
        place(&mut p, &w, Owner::Window(1), 800.0, idle());
        p.pointer_down(800.0, 350.0);
        p.pointer_move(803.0, 351.0);
        p.pointer_up(803.0, 351.0);
        assert!(matches!(p.state, State::Air { anim: Anim::Hop } | State::Trick { .. }));
    }

    #[test]
    fn one_click_annoys_a_sleeper_three_clicks_wake_it() {
        let w = world(vec![w1()]);
        let click = |p: &mut Pet| {
            p.pointer_down(800.0, 350.0);
            p.pointer_up(800.0, 350.0);
        };
        let mut p = Pet::new(cfg(), 0.0, 0.0, 1);
        place(&mut p, &w, Owner::Window(1), 800.0, State::Sleep { remaining: 30.0 });
        click(&mut p);
        assert_eq!(p.state, State::Annoyed { remaining: 1.5, tickled_for: 0.0 });
        assert_eq!(p.render().anim, Anim::Annoyed);
        assert!(!p.render().heart);
        run(&mut p, &w, 96);
        assert!(matches!(p.state, State::Sleep { .. }), "{:?}", p.state);

        // 세 번 빠르게 → 기지개 켜며 깸
        let mut p = Pet::new(cfg(), 0.0, 0.0, 1);
        place(&mut p, &w, Owner::Window(1), 800.0, State::Sleep { remaining: 30.0 });
        click(&mut p);
        run(&mut p, &w, 30);
        click(&mut p);
        run(&mut p, &w, 30);
        click(&mut p);
        assert_eq!(p.state, State::Trick { kind: Trick::Stretch, remaining: 2.5 });

        // 2초가 넘게 띄엄띄엄 누르면 다시 처음부터 센다
        let mut p = Pet::new(cfg(), 0.0, 0.0, 1);
        place(&mut p, &w, Owner::Window(1), 800.0, State::Sleep { remaining: 30.0 });
        for _ in 0..3 {
            click(&mut p);
            assert!(matches!(p.state, State::Annoyed { .. }), "{:?}", p.state);
            run(&mut p, &w, 75);
        }
    }

    fn aloof() -> PetConfig {
        PetConfig { personality: Personality::Aloof, ..cfg() }
    }

    #[test]
    fn aloof_cat_mostly_huffs_or_ignores_clicks() {
        let w = world(vec![w1()]);
        let n = 2000;
        let (mut aegyo, mut huff, mut ignore, mut ignore_said) = (0, 0, 0, 0);
        for seed in 0..n {
            let mut p = Pet::new(aloof(), 0.0, 0.0, seed);
            let before = State::Move { kind: Move::Walk, remaining: 7.0 };
            place(&mut p, &w, Owner::Window(1), 800.0, before);
            p.pointer_down(800.0, 350.0);
            p.pointer_up(800.0, 350.0);
            match p.state {
                State::Huff { remaining } => {
                    assert_eq!(remaining, 1.5);
                    assert_eq!(p.render().anim, Anim::Huff);
                    assert_eq!(p.take_speech(), Some(Speech::Grumpy));
                    huff += 1;
                }
                s if s == before => {
                    ignore += 1;
                    match p.take_speech() {
                        Some(Speech::Ignore) => ignore_said += 1,
                        None => {}
                        sp => panic!("ignore said {sp:?}"),
                    }
                }
                State::Air { anim: Anim::Hop } | State::Trick { .. } => aegyo += 1,
                s => panic!("seed {seed}: {s:?}"),
            }
        }
        let share = |k: i32| k as f64 / n as f64;
        assert!((0.05..=0.15).contains(&share(aegyo)), "aegyo {}", share(aegyo));
        assert!((0.38..=0.52).contains(&share(huff)), "huff {}", share(huff));
        assert!((0.38..=0.52).contains(&share(ignore)), "ignore {}", share(ignore));
        let said = ignore_said as f64 / ignore as f64;
        assert!((0.40..=0.60).contains(&said), "ignore said {said}");
    }

    #[test]
    fn ignored_click_keeps_a_calm_beat_going() {
        let w = world(vec![w1()]);
        for seed in 0..200 {
            let mut p = Pet::new(aloof(), 0.0, 0.0, seed);
            place(&mut p, &w, Owner::Window(1), 800.0, State::Trick { kind: Trick::Wave, remaining: 0.001 });
            p.update(DT, &w);
            let beat = p.state;
            let pending = p.pacing.after_beat;
            assert!(matches!(pending, Some(AfterBeat::Go(State::Move { .. }))), "seed {seed}: {pending:?}");
            p.pointer_down(800.0, 350.0);
            p.pointer_up(800.0, 350.0);
            if p.state == beat {
                assert_eq!(p.pacing.after_beat, pending, "seed {seed}: the pending move is restored exactly");
                run(&mut p, &w, 100);
                assert!(matches!(p.state, State::Move { .. }), "seed {seed}: {:?}", p.state);
                return;
            }
        }
        panic!("never ignored");
    }

    #[test]
    fn huff_ends_with_a_beat_then_moves() {
        let w = world(vec![w1()]);
        let mut p = Pet::new(aloof(), 0.0, 0.0, 1);
        place(&mut p, &w, Owner::Window(1), 800.0, State::Huff { remaining: 0.001 });
        p.facing = -1.0;
        assert!(!p.render().flip, "huff faces front");
        p.update(DT, &w);
        assert!(matches!(p.state, State::Idle { look: false, .. }), "{:?}", p.state);
        run(&mut p, &w, 91);
        assert!(matches!(p.state, State::Move { .. }), "{:?}", p.state);
    }

    #[test]
    fn hungry_aloof_cat_still_asks_for_food() {
        let w = world(vec![w1()]);
        for seed in 0..50 {
            let mut p = Pet::new(aloof(), 0.0, 0.0, seed);
            p.set_hungry(true);
            place(&mut p, &w, Owner::Window(1), 800.0, idle());
            p.pointer_down(800.0, 350.0);
            p.pointer_up(800.0, 350.0);
            assert!(p.take_food_request(), "seed {seed}");
        }
    }

    /// 착지 뒤 쉬기 결정(박자가 끝난 뒤)이 애교였던 비율
    fn rest_trick_share(cfg: PetConfig, hungry: bool) -> f64 {
        let w = world(vec![w1()]);
        let n = 3000;
        let hits = (0..n)
            .filter(|&seed| {
                let mut p = Pet::new(cfg, 0.0, 0.0, seed);
                p.set_hungry(hungry);
                place(&mut p, &w, Owner::Window(1), 800.0, State::Land { remaining: 0.001 });
                for _ in 0..200 {
                    p.update(DT, &w);
                    if !matches!(p.state, State::Land { .. } | State::Idle { .. }) {
                        break;
                    }
                }
                matches!(p.state, State::Trick { .. })
            })
            .count();
        hits as f64 / n as f64
    }

    #[test]
    fn aloof_cat_rarely_does_tricks_when_resting() {
        let aloof_share = rest_trick_share(aloof(), false);
        assert!((0.07..=0.13).contains(&aloof_share), "aloof {aloof_share}");
        let friendly = rest_trick_share(cfg(), false);
        assert!((0.51..=0.59).contains(&friendly), "friendly {friendly}");
        // 배고프면 절반은 시무룩, 나머지 중 60%가 애교
        let hungry = rest_trick_share(aloof(), true);
        assert!((0.26..=0.34).contains(&hungry), "hungry {hungry}");
    }

    #[test]
    fn pressed_grounded_pet_keeps_riding_window() {
        let w = world(vec![w1()]);
        let mut p = Pet::new(cfg(), 0.0, 0.0, 1);
        place(&mut p, &w, Owner::Window(1), 800.0, idle());
        p.pointer_down(800.0, 350.0);
        p.update(DT, &world(vec![super::tests::win(1, 600.0, 400.0, 1200.0, 900.0)]));
        assert_eq!(p.body.x, 900.0);
    }

    #[test]
    fn drag_moves_pet_with_cursor_keeping_grab_offset() {
        let w = world(vec![w1()]);
        let mut p = Pet::new(cfg(), 0.0, 0.0, 1);
        place(&mut p, &w, Owner::Window(1), 800.0, idle());
        p.pointer_down(810.0, 350.0);
        p.pointer_move(910.0, 250.0);
        assert_eq!(p.state, State::Dragged);
        assert!(p.standing.is_none());
        assert_eq!((p.body.x, p.body.y), (900.0, 300.0));
        assert_eq!(p.render().anim, Anim::Dragged);
    }

    #[test]
    fn release_throws_with_recent_cursor_velocity() {
        let w = world(vec![w1()]);
        let mut p = Pet::new(cfg(), 0.0, 0.0, 1);
        place(&mut p, &w, Owner::Window(1), 800.0, idle());
        p.pointer_down(800.0, 300.0);
        for i in 1..=6 {
            p.update(DT, &w);
            p.pointer_move(800.0 + 20.0 * i as f64, 300.0);
        }
        p.pointer_up(920.0, 300.0);
        assert_eq!(p.state, State::Air { anim: Anim::Fall });
        assert!((p.body.vx - 1200.0).abs() < 1.0, "vx={}", p.body.vx);
        assert!(p.body.vy.abs() < 1.0);
        assert_eq!(p.facing, 1.0);
    }

    #[test]
    fn click_while_airborne_drops_with_zero_velocity() {
        let w = world(vec![w1()]);
        let mut p = Pet::new(cfg(), 800.0, 200.0, 1);
        p.body.vx = 500.0;
        p.body.vy = 300.0;
        p.pointer_down(800.0, 150.0);
        p.update(DT, &w);
        assert_eq!(p.body.y, 200.0, "held pet must not move");
        p.pointer_up(800.0, 150.0);
        assert_eq!(p.state, State::Air { anim: Anim::Fall });
        assert_eq!((p.body.vx, p.body.vy), (0.0, 0.0));
        assert!(!p.render().heart);
    }

    #[test]
    fn second_pointer_down_while_held_is_ignored() {
        let w = world(vec![w1()]);
        let mut p = Pet::new(cfg(), 0.0, 0.0, 1);
        place(&mut p, &w, Owner::Window(1), 800.0, idle());
        p.pointer_down(800.0, 350.0);
        p.pointer_move(900.0, 350.0);
        p.pointer_down(950.0, 350.0);
        assert_eq!(p.state, State::Dragged);
    }

    #[test]
    fn release_after_pause_throws_with_zero_velocity() {
        let w = world(vec![w1()]);
        let mut p = Pet::new(cfg(), 0.0, 0.0, 1);
        place(&mut p, &w, Owner::Window(1), 800.0, idle());
        p.pointer_down(800.0, 300.0);
        for i in 1..=6 {
            p.update(DT, &w);
            p.pointer_move(800.0 + 50.0 * i as f64, 300.0);
        }
        for _ in 0..12 {
            p.update(DT, &w);
            p.pointer_move(1100.0, 300.0);
        }
        p.pointer_up(1100.0, 300.0);
        assert!(p.body.vx.abs() < 1.0, "vx={}", p.body.vx);
        assert!(p.body.vy.abs() < 1.0, "vy={}", p.body.vy);
    }
}

#[cfg(test)]
mod tickle_tests {
    use super::tests::{cfg, place, w1, world};
    use super::*;

    const DT: f64 = 1.0 / 60.0;

    /// 커서를 40px씩 좌우로 흔들며 n틱 간질인다
    fn tickle(p: &mut Pet, w: &World, ticks: usize) {
        for i in 0..ticks {
            p.hover(800.0 + if i % 2 == 0 { 0.0 } else { 40.0 }, 350.0, true);
            p.update(DT, w);
        }
    }

    fn idle() -> State {
        State::Idle { remaining: 100.0, look: false }
    }

    fn running() -> State {
        State::Move { kind: Move::Run, remaining: 100.0 }
    }

    #[test]
    fn shaking_over_a_running_pet_tickles_it() {
        let w = world(vec![w1()]);
        // 빠른 달리기(450px/s): 패딩 포함 사각형 안에 0.5초도 못 머문다
        let fast = PetConfig { walk_speed: 180.0, ..cfg() };
        let mut p = Pet::new(fast, 0.0, 0.0, 1);
        place(&mut p, &w, Owner::Window(1), 530.0, running());
        p.facing = 1.0;
        let mut tickled_at = None;
        for i in 0..60 {
            // 고정된 지점 근처에서 7.8px/틱으로 40px 폭을 왕복
            let ph = (i as f64 * 7.8) % 80.0;
            let (cx, cy) = (640.0 + if ph < 40.0 { ph } else { 80.0 - ph }, 350.0);
            let (sx, sy) = p.window_pos();
            let (side, pad) = (p.cfg.size, p.cfg.size * 0.2);
            let inside = cx >= sx - pad && cx < sx + side + pad && cy >= sy - pad && cy < sy + side + pad;
            p.hover(cx, cy, inside);
            p.update(DT, &w);
            if matches!(p.state, State::Tickled { .. }) {
                tickled_at = Some(i);
                break;
            }
        }
        assert!(tickled_at.is_some(), "never tickled; state {:?}", p.state);
    }

    #[test]
    fn hovering_a_moving_pet_makes_it_stop_and_look() {
        let w = world(vec![w1()]);
        let mut p = Pet::new(cfg(), 0.0, 0.0, 1);
        place(&mut p, &w, Owner::Window(1), 560.0, State::Move { kind: Move::Walk, remaining: 100.0 });
        for _ in 0..18 {
            p.hover(600.0, 350.0, true);
            p.update(DT, &w);
        }
        assert!(matches!(p.state, State::Idle { look: true, .. }), "{:?}", p.state);
    }

    #[test]
    fn staying_hovered_keeps_looking_without_turning() {
        for seed in 0..20 {
            let w = world(vec![w1()]);
            let mut p = Pet::new(cfg(), 0.0, 0.0, seed);
            place(&mut p, &w, Owner::Window(1), 560.0, State::Move { kind: Move::Walk, remaining: 100.0 });
            for _ in 0..18 {
                p.hover(600.0, 350.0, true);
                p.update(DT, &w);
            }
            assert!(matches!(p.state, State::Idle { look: true, .. }), "{:?}", p.state);
            let (facing, last_move) = (p.facing, p.pacing.last_move);
            for i in 0..(60 * 6) {
                p.hover(600.0, 350.0, true);
                p.update(DT, &w);
                assert!(matches!(p.state, State::Idle { look: true, .. }), "seed {seed} tick {i}: {:?}", p.state);
                assert_eq!((p.facing, p.pacing.last_move), (facing, last_move), "seed {seed} tick {i}");
            }
        }
    }

    #[test]
    fn brief_hover_does_not_stop_a_moving_pet() {
        let w = world(vec![w1()]);
        let mut p = Pet::new(cfg(), 0.0, 0.0, 1);
        place(&mut p, &w, Owner::Window(1), 560.0, State::Move { kind: Move::Walk, remaining: 100.0 });
        for _ in 0..8 {
            p.hover(600.0, 350.0, true);
            p.update(DT, &w);
        }
        assert!(matches!(p.state, State::Move { .. }), "{:?}", p.state);
    }

    #[test]
    fn tickling_decays_within_the_window_after_leaving() {
        let w = world(vec![w1()]);
        let mut p = Pet::new(cfg(), 0.0, 0.0, 1);
        place(&mut p, &w, Owner::Window(1), 800.0, idle());
        tickle(&mut p, &w, 30);
        assert!(p.tickling);
        for _ in 0..40 {
            p.hover(0.0, 0.0, false);
            p.update(DT, &w);
        }
        assert!(!p.tickling);
    }

    #[test]
    fn leaving_and_reentering_does_not_count_the_gap() {
        let w = world(vec![w1()]);
        let mut p = Pet::new(cfg(), 0.0, 0.0, 1);
        place(&mut p, &w, Owner::Window(1), 800.0, idle());
        p.hover(700.0, 350.0, true);
        p.update(DT, &w);
        for _ in 0..12 {
            p.hover(0.0, 0.0, false);
            p.update(DT, &w);
        }
        p.hover(900.0, 350.0, true);
        p.update(DT, &w);
        assert!(!p.tickling, "200px jump across a gap must not count");
    }

    #[test]
    fn fast_hover_tickles_and_speaks_once() {
        let w = world(vec![w1()]);
        let mut p = Pet::new(cfg(), 0.0, 0.0, 1);
        place(&mut p, &w, Owner::Window(1), 800.0, idle());
        tickle(&mut p, &w, 30);
        assert!(matches!(p.state, State::Tickled { .. }));
        assert_eq!(p.render().anim, Anim::Tickle);
        assert_eq!(p.take_speech(), Some(Speech::Tickle));
        tickle(&mut p, &w, 10);
        assert_eq!(p.take_speech(), None, "speech only when tickling starts");
    }

    #[test]
    fn slow_hover_does_not_tickle() {
        let w = world(vec![w1()]);
        let mut p = Pet::new(cfg(), 0.0, 0.0, 1);
        place(&mut p, &w, Owner::Window(1), 800.0, idle());
        for i in 0..60 {
            p.hover(800.0 + i as f64 * 2.0, 350.0, true);
            p.update(DT, &w);
        }
        assert!(matches!(p.state, State::Idle { .. }));
    }

    #[test]
    fn tickle_stops_1_5s_after_cursor_leaves() {
        let w = world(vec![w1()]);
        let mut p = Pet::new(cfg(), 0.0, 0.0, 1);
        place(&mut p, &w, Owner::Window(1), 800.0, idle());
        tickle(&mut p, &w, 30);
        for _ in 0..80 {
            p.hover(0.0, 0.0, false);
            p.update(DT, &w);
        }
        assert!(matches!(p.state, State::Tickled { .. }), "still giggling before 1.5s");
        for _ in 0..20 {
            p.hover(0.0, 0.0, false);
            p.update(DT, &w);
        }
        assert!(matches!(p.state, State::Idle { look: false, .. }), "calm beat before moving: {:?}", p.state);
        for _ in 0..90 {
            p.hover(0.0, 0.0, false);
            p.update(DT, &w);
        }
        assert!(matches!(p.state, State::Move { .. }));
    }

    #[test]
    fn tickling_a_sleeper_annoys_then_it_sleeps_again() {
        let w = world(vec![w1()]);
        let mut p = Pet::new(cfg(), 0.0, 0.0, 1);
        place(&mut p, &w, Owner::Window(1), 800.0, State::Sleep { remaining: 30.0 });
        tickle(&mut p, &w, 120);
        assert!(matches!(p.state, State::Annoyed { .. }));
        assert_eq!(p.render().anim, Anim::Annoyed);
        assert_eq!(p.take_speech(), None);
        for _ in 0..100 {
            p.hover(0.0, 0.0, false);
            p.update(DT, &w);
        }
        assert!(matches!(p.state, State::Sleep { .. }));
    }

    #[test]
    fn five_seconds_of_tickling_wakes_a_sleeper() {
        let w = world(vec![w1()]);
        let mut p = Pet::new(cfg(), 0.0, 0.0, 1);
        place(&mut p, &w, Owner::Window(1), 800.0, State::Sleep { remaining: 30.0 });
        tickle(&mut p, &w, 280);
        assert!(matches!(p.state, State::Annoyed { .. }), "not yet at ~4.5s");
        // 깨어난 틱에서 멈춘다 (계속 간질이면 깬 뒤 바로 간질 반응으로 넘어가는 게 정상)
        let mut woke = false;
        for i in 0..40 {
            p.hover(800.0 + if i % 2 == 0 { 40.0 } else { 0.0 }, 350.0, true);
            p.update(DT, &w);
            if matches!(p.state, State::Idle { .. }) {
                woke = true;
                break;
            }
        }
        assert!(woke);
        assert_eq!(p.take_speech(), Some(Speech::Grumpy));
    }

    #[test]
    fn airborne_pet_ignores_tickling() {
        let w = world(vec![w1()]);
        let mut p = Pet::new(cfg(), 800.0, 100.0, 1);
        tickle(&mut p, &w, 10);
        assert!(matches!(p.state, State::Air { .. }));
    }

    #[test]
    fn set_special_ends_a_running_special() {
        let w = world(vec![]);
        let mut p = Pet::new(cfg(), 0.0, 0.0, 1);
        place(&mut p, &w, Owner::Ground(0), 900.0, State::Idle { remaining: 1000.0, look: false });
        p.start_special(Special::Wag);
        assert!(matches!(p.state, State::Special { .. }));
        p.set_special(None);
        assert_eq!(p.cfg.special, None);
        assert!(matches!(p.state, State::Idle { .. }));
    }

    #[test]
    fn chatters_every_20_to_40_seconds_but_not_while_sleeping() {
        let w = world(vec![]);
        let mut p = Pet::new(cfg(), 0.0, 0.0, 1);
        place(&mut p, &w, Owner::Ground(0), 900.0, State::Idle { remaining: 1000.0, look: false });
        let mut said = 0;
        for _ in 0..(60 * 90) {
            p.update(DT, &w);
            if p.take_speech() == Some(Speech::Chatter) {
                said += 1;
            }
        }
        assert!((2..=5).contains(&said), "said {said} times in 90s");
        place(&mut p, &w, Owner::Ground(0), 900.0, State::Sleep { remaining: 1000.0 });
        for _ in 0..(60 * 90) {
            p.update(DT, &w);
            assert_eq!(p.take_speech(), None);
        }
    }

    #[test]
    fn about_half_of_clicks_speak() {
        let w = world(vec![w1()]);
        let mut spoke = 0;
        for seed in 0..200 {
            let mut p = Pet::new(cfg(), 0.0, 0.0, seed);
            place(&mut p, &w, Owner::Window(1), 800.0, idle());
            p.pointer_down(800.0, 350.0);
            p.pointer_up(800.0, 350.0);
            if p.take_speech() == Some(Speech::Chatter) {
                spoke += 1;
            }
        }
        assert!((70..=130).contains(&spoke), "spoke {spoke}/200");
    }

    #[test]
    fn chatter_does_not_overwrite_a_pending_speech() {
        let w = world(vec![w1()]);
        let mut p = Pet::new(cfg(), 0.0, 0.0, 1);
        place(&mut p, &w, Owner::Window(1), 800.0, idle());
        p.chatter_timer = 0.001;
        p.speech = Some(Speech::Tickle);
        p.update(DT, &w);
        assert_eq!(p.take_speech(), Some(Speech::Tickle));
        p.update(DT, &w);
        assert_eq!(p.take_speech(), Some(Speech::Chatter));
    }

    #[test]
    fn chatter_waits_while_tickled() {
        let w = world(vec![w1()]);
        let mut p = Pet::new(cfg(), 0.0, 0.0, 1);
        place(&mut p, &w, Owner::Window(1), 800.0, State::Tickled { remaining: 1.0 });
        p.chatter_timer = 0.001;
        p.update(DT, &w);
        assert_eq!(p.take_speech(), None);
    }

    #[test]
    fn shaking_the_cursor_during_a_hop_does_not_tickle_on_landing() {
        let w = world(vec![w1()]);
        let mut p = Pet::new(cfg(), 800.0, 300.0, 1);
        p.state = State::Air { anim: Anim::Fall };
        let mut airborne_ticks = 0;
        for i in 0..40 {
            p.hover(800.0 + if i % 2 == 0 { 0.0 } else { 40.0 }, 350.0, true);
            let was_air = matches!(p.state, State::Air { .. });
            p.update(DT, &w);
            if was_air {
                airborne_ticks += 1;
            }
            if matches!(p.state, State::Land { .. }) {
                assert!(airborne_ticks >= 10, "fall too short to build up a tickle: {airborne_ticks}");
                p.hover(800.0, 350.0, true);
                p.update(DT, &w);
                assert!(!matches!(p.state, State::Tickled { .. }), "tickled right after landing");
                return;
            }
            assert!(!matches!(p.state, State::Tickled { .. }));
        }
        panic!("never landed");
    }

    #[test]
    fn click_right_after_tickling_keeps_its_reaction() {
        let w = world(vec![w1()]);
        for seed in 0..20 {
            let mut p = Pet::new(cfg(), 0.0, 0.0, seed);
            place(&mut p, &w, Owner::Window(1), 800.0, idle());
            tickle(&mut p, &w, 30);
            assert!(p.tickling);
            p.pointer_down(800.0, 350.0);
            p.pointer_up(800.0, 350.0);
            p.update(DT, &w);
            assert!(
                matches!(p.state, State::Air { anim: Anim::Hop } | State::Trick { .. }),
                "seed {seed}: {:?}",
                p.state
            );
        }
    }

    #[test]
    fn tickle_distance_scales_with_display_size() {
        let w = world(vec![w1()]);
        let shake = |size: f64| {
            let mut p = Pet::new(PetConfig { size, ..cfg() }, 0.0, 0.0, 1);
            place(&mut p, &w, Owner::Window(1), 800.0, idle());
            tickle(&mut p, &w, 10);
            matches!(p.state, State::Tickled { .. })
        };
        assert!(shake(128.0));
        assert!(!shake(256.0));
    }

    #[test]
    fn no_chatter_during_a_special() {
        let w = world(vec![w1()]);
        let mut p = Pet::new(cfg(), 0.0, 0.0, 1);
        place(&mut p, &w, Owner::Window(1), 800.0, idle());
        p.start_special(Special::Wag);
        p.take_speech();
        p.chatter_timer = 0.001;
        p.update(DT, &w);
        assert!(matches!(p.state, State::Special { .. }));
        assert_eq!(p.take_speech(), None);
    }

    #[test]
    fn roll_spin_follows_the_bounce() {
        let w = world(vec![]);
        let mut p = Pet::new(cfg(), 39.0, 500.0, 1);
        p.state = State::Air { anim: Anim::Roll };
        p.body.vx = -300.0;
        p.facing = -1.0;
        p.update(DT, &w);
        assert!(p.body.vx > 0.0);
        assert_eq!(p.facing, 1.0);
    }
}

#[cfg(test)]
mod special_tests {
    use super::tests::{cfg, place, w1, world};
    use super::*;
    use crate::behavior::Special;

    const DT: f64 = 1.0 / 60.0;

    fn with(special: Option<Special>) -> PetConfig {
        PetConfig { special, ..cfg() }
    }

    /// 착지 직후 쉬기 진입을 n번 시켜 특별 동작이 나온 비율
    fn rest_share(special: Option<Special>) -> f64 {
        let w = world(vec![w1()]);
        let n = 3000;
        let hits = (0..n)
            .filter(|&seed| {
                let mut p = Pet::new(with(special), 0.0, 0.0, seed);
                place(&mut p, &w, Owner::Window(1), 800.0, State::Land { remaining: 0.001 });
                super::tests::run_past_beat(&mut p, &w);
                matches!(p.state, State::Special { .. })
            })
            .count();
        hits as f64 / n as f64
    }

    /// 깨어 있는 동물을 간질이기 시작했을 때 특별 동작이 나온 비율
    fn tickle_share(special: Option<Special>) -> f64 {
        let w = world(vec![w1()]);
        let n = 1000;
        let hits = (0..n)
            .filter(|&seed| {
                let mut p = Pet::new(with(special), 0.0, 0.0, seed);
                place(&mut p, &w, Owner::Window(1), 800.0, State::Idle { remaining: 100.0, look: false });
                for i in 0..10 {
                    p.hover(800.0 + if i % 2 == 0 { 0.0 } else { 40.0 }, 350.0, true);
                    p.update(DT, &w);
                }
                matches!(p.state, State::Special { .. })
            })
            .count();
        hits as f64 / n as f64
    }

    #[test]
    fn rest_specials_follow_species_chances() {
        let arch = rest_share(Some(Special::Arch));
        let wag = rest_share(Some(Special::Wag));
        assert!((0.05..=0.11).contains(&arch), "arch {arch}");
        assert!((0.09..=0.15).contains(&wag), "wag {wag}");
        assert_eq!(rest_share(None), 0.0);
    }

    #[test]
    fn tickling_sometimes_triggers_the_special() {
        let arch = tickle_share(Some(Special::Arch));
        let wag = tickle_share(Some(Special::Wag));
        assert!((0.19..=0.31).contains(&arch), "arch {arch}");
        assert!((0.24..=0.36).contains(&wag), "wag {wag}");
        assert_eq!(tickle_share(None), 0.0);
    }

    #[test]
    fn special_speech_facing_and_ending() {
        let w = world(vec![w1()]);
        for (kind, speech, anim, front) in [(Special::Arch, Speech::Hiss, Anim::Arch, false), (Special::Wag, Speech::Chatter, Anim::Wag, true)] {
            let mut p = Pet::new(with(Some(kind)), 0.0, 0.0, 1);
            place(&mut p, &w, Owner::Window(1), 800.0, State::Idle { remaining: 100.0, look: false });
            p.start_special(kind);
            assert_eq!(p.take_speech(), Some(speech));
            p.facing = -1.0;
            assert_eq!(p.render().anim, anim);
            assert_eq!(p.render().flip, !front);
            for _ in 0..(60 * 5) {
                p.update(DT, &w);
            }
            assert!(!matches!(p.state, State::Special { .. }), "{kind:?} should end within its duration");
        }
    }

    #[test]
    fn tickling_does_not_interrupt_a_special() {
        let w = world(vec![w1()]);
        let mut p = Pet::new(with(Some(Special::Arch)), 0.0, 0.0, 1);
        place(&mut p, &w, Owner::Window(1), 800.0, State::Idle { remaining: 100.0, look: false });
        p.start_special(Special::Arch);
        let State::Special { remaining: start, .. } = p.state else { panic!() };
        for i in 0..30 {
            p.hover(800.0 + if i % 2 == 0 { 0.0 } else { 40.0 }, 350.0, true);
            p.update(DT, &w);
        }
        let State::Special { kind: Special::Arch, remaining } = p.state else { panic!("{:?}", p.state) };
        assert!((start - remaining - 30.0 * DT).abs() < 1e-6);
    }
}

#[cfg(test)]
mod care_tests {
    use super::tests::{cfg, place, run, w1, world};
    use super::*;
    use crate::care::{FeedResult, Food};

    const DT: f64 = 1.0 / 60.0;

    fn grounded(seed: u64) -> (Pet, World) {
        let w = world(vec![w1()]);
        let mut p = Pet::new(cfg(), 0.0, 0.0, seed);
        place(&mut p, &w, Owner::Window(1), 800.0, State::Idle { remaining: 100.0, look: false });
        (p, w)
    }

    #[test]
    fn hungry_pets_often_sulk_when_resting() {
        let w = world(vec![w1()]);
        let n = 2000;
        let sulks = (0..n)
            .filter(|&seed| {
                let mut p = Pet::new(cfg(), 0.0, 0.0, seed);
                p.set_hungry(true);
                place(&mut p, &w, Owner::Window(1), 800.0, State::Land { remaining: 0.001 });
                super::tests::run_past_beat(&mut p, &w);
                matches!(p.state, State::Hungry { .. })
            })
            .count() as f64
            / n as f64;
        assert!((0.45..=0.55).contains(&sulks), "{sulks}");
        assert_eq!(Pet::new(cfg(), 0.0, 0.0, 1).render().flip, false);
    }

    #[test]
    fn hungry_speech_replaces_chatter() {
        let (mut p, w) = grounded(1);
        p.set_hungry(true);
        let mut said = Vec::new();
        for _ in 0..(60 * 60) {
            p.update(DT, &w);
            if let Some(s) = p.take_speech() {
                said.push(s);
            }
        }
        assert!(said.len() >= 2);
        assert!(said.iter().all(|s| *s == Speech::Hungry), "{said:?}");
    }

    #[test]
    fn clicking_a_hungry_pet_asks_for_food_instead_of_a_trick() {
        let (mut p, _w) = grounded(1);
        p.set_hungry(true);
        p.pointer_down(800.0, 350.0);
        p.pointer_up(800.0, 350.0);
        assert!(p.take_food_request());
        assert!(!p.take_food_request(), "only once");
        assert!(matches!(p.state, State::Hungry { .. }));
        let (mut p, _w) = grounded(1);
        p.pointer_down(800.0, 350.0);
        p.pointer_up(800.0, 350.0);
        assert!(!p.take_food_request(), "not hungry → normal click reaction");
    }

    #[test]
    fn eating_then_moving_or_hearting() {
        for (food, then_heart) in [(Food::Meal, false), (Food::Favorite, true)] {
            let (mut p, w) = grounded(1);
            assert!(p.feed_result(FeedResult::Ate { leveled_up: false }, food));
            assert_eq!(p.render().anim, Anim::Eat);
            for _ in 0..(60 * 2 + 40) {
                p.update(DT, &w);
            }
            if !then_heart {
                assert!(matches!(p.state, State::Idle { look: false, .. }), "calm beat after eating: {:?}", p.state);
                for _ in 0..90 {
                    p.update(DT, &w);
                }
            }
            if then_heart {
                assert!(matches!(p.state, State::Trick { kind: Trick::Heart, .. }), "{:?}", p.state);
            } else {
                assert!(matches!(p.state, State::Move { .. }), "{:?}", p.state);
            }
        }
    }

    #[test]
    fn eating_then_level_up_celebrates() {
        let (mut p, w) = grounded(1);
        p.feed_result(FeedResult::Ate { leveled_up: true }, Food::Meal);
        for _ in 0..(60 * 2 + 40) {
            p.update(DT, &w);
        }
        assert!(matches!(p.state, State::LevelUp { .. }), "{:?}", p.state);
        assert_eq!(p.render().anim, Anim::LevelUp);
        assert_eq!(p.take_speech(), Some(Speech::LevelUp));
        for _ in 0..(60 * 3) {
            p.update(DT, &w);
        }
        assert!(!matches!(p.state, State::LevelUp { .. }));
    }

    #[test]
    fn refusing_says_full_and_tilts() {
        let (mut p, _w) = grounded(1);
        assert!(p.feed_result(FeedResult::Refused, Food::Meal));
        assert_eq!(p.take_speech(), Some(Speech::Full));
        assert!(matches!(p.state, State::Trick { kind: Trick::Tilt, .. }));
    }

    #[test]
    fn airborne_pets_ignore_food() {
        let w = world(vec![w1()]);
        let mut p = Pet::new(cfg(), 800.0, 100.0, 1);
        p.update(DT, &w);
        assert!(!p.feed_result(FeedResult::Ate { leveled_up: true }, Food::Meal));
        assert!(matches!(p.state, State::Air { .. }));
    }

    #[test]
    fn new_front_animations() {
        for a in [Anim::Hungry, Anim::Eat, Anim::LevelUp] {
            assert!(a.is_front(), "{a:?}");
        }
        assert_eq!(serde_json::to_string(&Anim::LevelUp).unwrap(), "\"level_up\"");
        assert_eq!(serde_json::to_string(&Speech::LevelUp).unwrap(), "\"level_up\"");
    }

    fn aloof_cfg() -> PetConfig {
        PetConfig { personality: Personality::Aloof, ..cfg() }
    }

    #[test]
    fn clicking_mid_meal_keeps_eating_and_its_celebration() {
        for seed in 0..50 {
            for c in [cfg(), aloof_cfg()] {
                let w = world(vec![w1()]);
                let mut p = Pet::new(c, 0.0, 0.0, seed);
                place(&mut p, &w, Owner::Window(1), 800.0, State::Idle { remaining: 100.0, look: false });
                assert!(p.feed_result(FeedResult::Ate { leveled_up: true }, Food::Favorite));
                p.update(DT, &w);
                assert!(p.is_eating());
                p.pointer_down(800.0, 350.0);
                assert!(p.is_eating(), "pressed mid-meal still counts as eating");
                p.update(DT, &w);
                p.pointer_up(800.0, 350.0);
                assert!(matches!(p.state, State::Eating { favorite: true, level_up: true, .. }), "seed {seed}: {:?}", p.state);
                assert!(p.is_eating());
            }
        }
    }

    #[test]
    fn clicking_during_level_up_keeps_celebrating() {
        for seed in 0..50 {
            for c in [cfg(), aloof_cfg()] {
                let w = world(vec![w1()]);
                let mut p = Pet::new(c, 0.0, 0.0, seed);
                place(&mut p, &w, Owner::Window(1), 800.0, State::LevelUp { remaining: 2.0 });
                p.pointer_down(800.0, 350.0);
                assert!(!p.is_eating());
                p.pointer_up(800.0, 350.0);
                assert!(matches!(p.state, State::LevelUp { .. }), "seed {seed}: {:?}", p.state);
                assert_eq!(p.render().anim, Anim::LevelUp);
            }
        }
    }

    #[test]
    fn pressing_a_resting_pet_is_not_eating() {
        let (mut p, _w) = grounded(1);
        assert!(!p.is_eating());
        p.pointer_down(800.0, 350.0);
        assert!(!p.is_eating());
    }

    #[test]
    fn feeding_again_mid_meal_keeps_level_up_and_favorite() {
        let (mut p, w) = grounded(1);
        assert!(p.feed_result(FeedResult::Ate { leveled_up: true }, Food::Favorite));
        run(&mut p, &w, 30);
        assert!(p.feed_result(FeedResult::Ate { leveled_up: false }, Food::Meal));
        assert!(matches!(p.state, State::Eating { favorite: true, level_up: true, .. }), "{:?}", p.state);
        run(&mut p, &w, 60 * 2 + 40);
        assert!(matches!(p.state, State::LevelUp { .. }), "{:?}", p.state);
    }
}

#[cfg(test)]
mod climb_tests {
    use super::tests::{cfg, place, run, sim_cfg, win, world};
    use super::*;
    use crate::world::Side;

    const DT: f64 = 1.0 / 60.0;

    fn climber() -> PetConfig {
        PetConfig { can_climb: true, ..cfg() }
    }

    /// 상태가 바뀔 때까지(최대 ticks) 돌린다
    fn run_while(p: &mut Pet, w: &World, ticks: usize, keep: impl Fn(&State) -> bool) -> usize {
        for i in 0..ticks {
            if !keep(&p.state) {
                return i;
            }
            p.update(DT, w);
        }
        ticks
    }

    #[test]
    fn losing_climbing_drops_a_climbing_pet() {
        let w = world(vec![]);
        for state in [State::Climb { target_y: 400.0, hang: 0.4 }, State::Slide, State::WallClimb { window: 1, side: Side::Left, rel_y: 50.0 }] {
            let mut p = Pet::new(climber(), 0.0, 0.0, 1);
            place(&mut p, &w, Owner::Ground(0), 900.0, State::Idle { remaining: 10.0, look: false });
            p.standing = None;
            p.state = state;
            p.set_can_climb(true);
            assert_eq!(p.state, state, "still a climber");
            p.set_can_climb(false);
            assert!(!p.cfg.can_climb);
            assert_eq!(p.state, State::Air { anim: Anim::Fall }, "{state:?}");
        }
        let mut p = Pet::new(climber(), 0.0, 0.0, 1);
        place(&mut p, &w, Owner::Ground(0), 900.0, State::Idle { remaining: 10.0, look: false });
        p.set_can_climb(false);
        assert!(matches!(p.state, State::Idle { .. }), "grounded pets keep what they do");
    }

    #[test]
    fn cannot_eat_while_climbing() {
        use crate::care::{FeedResult, Food};
        let w = world(vec![]);
        let mut p = Pet::new(climber(), 0.0, 0.0, 1);
        place(&mut p, &w, Owner::Ground(0), 900.0, State::Idle { remaining: 10.0, look: false });
        // 화면 타기를 막 고른 첫 틱 — 아직 디딤판 기록이 남아 있다
        p.state = State::Climb { target_y: 400.0, hang: 0.4 };
        assert!(p.standing.is_some());
        assert!(!p.feed_result(FeedResult::Ate { leveled_up: false }, Food::Meal));
        for state in [State::Slide, State::WallClimb { window: 1, side: Side::Left, rel_y: 50.0 }] {
            p.state = state;
            assert!(!p.feed_result(FeedResult::Refused, Food::Snack), "{state:?}");
        }
    }

    #[test]
    fn forced_climb_reaches_target_hangs_then_slides_to_the_ground() {
        let w = world(vec![]);
        let mut p = Pet::new(climber(), 0.0, 0.0, 1);
        place(&mut p, &w, Owner::Ground(0), 900.0, State::Idle { remaining: 10.0, look: false });
        assert!(p.start_climb(&w));
        let State::Climb { target_y, .. } = p.state else { panic!("{:?}", p.state) };
        assert!(p.standing.is_none());
        assert_eq!(p.render().anim, Anim::Climb);
        p.update(DT, &w);
        assert!((p.body.y - (1040.0 - 70.0 * DT)).abs() < 1e-9, "{}", p.body.y);
        assert_eq!(p.body.x, 900.0);
        // 목표에 닿고 잠깐 매달린 뒤 미끄러진다
        let ticks = run_while(&mut p, &w, 60 * 20, |s| matches!(s, State::Climb { .. }));
        let expected = ((1040.0 - target_y) / 70.0 + 0.4) * 60.0;
        assert!((ticks as f64 - expected).abs() < 3.0, "{ticks} vs {expected}");
        assert!(matches!(p.state, State::Slide), "{:?}", p.state);
        assert_eq!(p.render().anim, Anim::Slide);
        let y = p.body.y;
        p.update(DT, &w);
        assert!((p.body.y - (y + 250.0 * DT)).abs() < 1e-9);
        run_while(&mut p, &w, 60 * 10, |s| matches!(s, State::Slide));
        assert!(matches!(p.state, State::Land { .. }), "{:?}", p.state);
        assert_eq!(p.standing.unwrap().owner, Owner::Ground(0));
        assert_eq!((p.body.x, p.body.y), (900.0, 1040.0));
    }

    #[test]
    fn climb_hangs_about_0_4_s_at_the_target() {
        let w = world(vec![]);
        let mut p = Pet::new(climber(), 0.0, 0.0, 1);
        place(&mut p, &w, Owner::Ground(0), 900.0, State::Idle { remaining: 10.0, look: false });
        assert!(p.start_climb(&w));
        let State::Climb { target_y, .. } = p.state else { panic!() };
        let mut at_top = 0;
        for _ in 0..(60 * 20) {
            p.update(DT, &w);
            match p.state {
                State::Climb { .. } if p.body.y <= target_y => {
                    assert_eq!(p.body.y, target_y);
                    at_top += 1;
                }
                State::Climb { .. } => assert_eq!(at_top, 0),
                _ => break,
            }
        }
        assert!((23..=25).contains(&at_top), "{at_top}");
        assert!(matches!(p.state, State::Slide));
    }

    #[test]
    fn slide_lands_on_a_window_top_it_crosses() {
        let w = world(vec![win(1, 700.0, 600.0, 1100.0, 900.0)]);
        let mut p = Pet::new(climber(), 0.0, 0.0, 1);
        place(&mut p, &w, Owner::Ground(0), 900.0, State::Idle { remaining: 10.0, look: false });
        assert!(p.start_climb(&w));
        // 창 윗면(600)보다 높이 오르게 한다 — 오르는 동안엔 창 윗면에 걸리지 않는다
        p.state = State::Climb { target_y: 400.0, hang: behavior::CLIMB_HANG_SECS };
        run_while(&mut p, &w, 60 * 30, |s| matches!(s, State::Climb { .. } | State::Slide));
        assert!(matches!(p.state, State::Land { .. }), "{:?}", p.state);
        assert_eq!(p.standing.unwrap().owner, Owner::Window(1));
        assert_eq!((p.body.x, p.body.y), (900.0, 600.0));
    }

    #[test]
    fn climb_target_stays_between_a_quarter_and_the_top() {
        let w = world(vec![]);
        let range = 1040.0 - 128.0;
        let (mut lo, mut hi) = (f64::MAX, f64::MIN);
        for seed in 0..500 {
            let mut p = Pet::new(climber(), 0.0, 0.0, seed);
            place(&mut p, &w, Owner::Ground(0), 900.0, State::Idle { remaining: 10.0, look: false });
            assert!(p.start_climb(&w));
            let State::Climb { target_y, .. } = p.state else { panic!() };
            assert!((1040.0 - range..=1040.0 - 0.25 * range).contains(&target_y), "seed {seed}: {target_y}");
            lo = lo.min(target_y);
            hi = hi.max(target_y);
        }
        assert!(lo < 150.0 && hi > 790.0, "{lo} {hi}");
    }

    #[test]
    fn standing_above_the_target_skips_climbing() {
        let w = world(vec![win(1, 700.0, 200.0, 1100.0, 900.0)]);
        let mut skipped = 0;
        for seed in 0..200 {
            let mut p = Pet::new(climber(), 0.0, 0.0, seed);
            place(&mut p, &w, Owner::Window(1), 900.0, State::Idle { remaining: 10.0, look: false });
            if p.start_climb(&w) {
                let State::Climb { target_y, .. } = p.state else { panic!() };
                assert!(target_y < 200.0);
            } else {
                assert_eq!(p.standing.unwrap().owner, Owner::Window(1));
                skipped += 1;
            }
        }
        assert!(skipped > 150, "{skipped}");
    }

    /// 쉬기 끝에 새 이동을 시작할 때 화면 타기를 고른 비율
    fn climb_share(cfg: PetConfig) -> f64 {
        let w = world(vec![]);
        let (mut climbs, mut moves) = (0, 0);
        for seed in 0..5000 {
            let mut p = Pet::new(cfg, 0.0, 0.0, seed);
            place(&mut p, &w, Owner::Ground(0), 900.0, State::Idle { remaining: 0.001, look: false });
            p.update(DT, &w);
            match p.state {
                State::Climb { .. } => {
                    assert!(p.standing.is_none());
                    climbs += 1;
                }
                State::Move { .. } => moves += 1,
                State::Sleep { .. } => {}
                s => panic!("seed {seed}: {s:?}"),
            }
        }
        climbs as f64 / (climbs + moves) as f64
    }

    #[test]
    fn climbers_climb_about_6_percent_of_move_starts() {
        let share = climb_share(climber());
        assert!((0.04..=0.08).contains(&share), "{share}");
        assert_eq!(climb_share(cfg()), 0.0);
    }

    #[test]
    fn climb_after_a_front_activity_comes_after_a_beat() {
        let w = world(vec![]);
        for seed in 0..2000 {
            let mut p = Pet::new(climber(), 0.0, 0.0, seed);
            place(&mut p, &w, Owner::Ground(0), 900.0, State::Trick { kind: Trick::Wave, remaining: 0.001 });
            p.update(DT, &w);
            assert!(!matches!(p.state, State::Climb { .. }), "seed {seed}");
            if matches!(p.pacing.after_beat, Some(AfterBeat::Go(State::Climb { .. }))) {
                assert!(p.standing.is_some());
                run_while(&mut p, &w, 120, |s| matches!(s, State::Idle { .. }));
                assert!(matches!(p.state, State::Climb { .. }));
                assert!(p.standing.is_none());
                p.update(DT, &w);
                assert!(p.body.y < 1040.0);
                return;
            }
        }
        panic!("never climbed after a trick");
    }

    #[test]
    fn back_view_animations_never_flip() {
        let mut p = Pet::new(climber(), 0.0, 0.0, 1);
        p.facing = -1.0;
        for (state, anim) in [
            (State::Climb { target_y: 100.0, hang: 0.4 }, Anim::Climb),
            (State::Slide, Anim::Slide),
            (State::WallClimb { window: 1, side: Side::Left, rel_y: 100.0 }, Anim::Climb),
        ] {
            p.state = state;
            assert_eq!(p.render(), RenderState { anim, flip: false, heart: false });
        }
        assert!(Anim::Climb.is_back() && Anim::Slide.is_back() && !Anim::Climb.is_front() && !Anim::Walk.is_back());
        assert_eq!(serde_json::to_string(&Anim::Slide).unwrap(), "\"slide\"");
    }

    #[test]
    fn climbing_pet_can_be_grabbed_and_dragged() {
        let w = world(vec![]);
        for state in [State::Climb { target_y: 300.0, hang: 0.4 }, State::Slide] {
            let mut p = Pet::new(climber(), 0.0, 0.0, 1);
            place(&mut p, &w, Owner::Ground(0), 900.0, State::Idle { remaining: 10.0, look: false });
            assert!(p.start_climb(&w));
            run(&mut p, &w, 30);
            p.state = state;
            p.pointer_down(900.0, p.body.y - 50.0);
            assert!(matches!(p.state, State::Pressed { grounded: false, .. }), "{:?}", p.state);
            p.pointer_move(1000.0, 500.0);
            assert_eq!(p.state, State::Dragged);
            p.pointer_up(1000.0, 500.0);
            assert!(matches!(p.state, State::Air { .. }));
        }
    }

    #[test]
    fn climbing_pet_ignores_tickles_hover_and_chatter() {
        let w = world(vec![]);
        let mut p = Pet::new(climber(), 0.0, 0.0, 1);
        place(&mut p, &w, Owner::Ground(0), 900.0, State::Idle { remaining: 10.0, look: false });
        assert!(p.start_climb(&w));
        p.chatter_timer = 0.0;
        for i in 0..90 {
            let (cx, cy) = (880.0 + if i % 2 == 0 { 0.0 } else { 40.0 }, p.body.y - 60.0);
            p.hover(cx, cy, true);
            p.update(DT, &w);
            assert!(matches!(p.state, State::Climb { .. }), "{:?}", p.state);
            assert_eq!(p.take_speech(), None);
        }
    }

    // --- 창 옆면 타기 ---

    fn side_world() -> World {
        world(vec![win(1, 800.0, 300.0, 1300.0, 900.0)])
    }

    fn thrown(cfg: PetConfig, x: f64, y: f64, vx: f64) -> Pet {
        let mut p = Pet::new(cfg, x, y, 1);
        p.body.vx = vx;
        p.state = State::Air { anim: Anim::Fall };
        p
    }

    #[test]
    fn thrown_climber_sticks_to_the_left_side_climbs_and_stands_on_top() {
        let w = side_world();
        let mut p = thrown(climber(), 700.0, 600.0, 600.0);
        run_while(&mut p, &w, 60, |s| matches!(s, State::Air { .. }));
        let State::WallClimb { window: 1, side: Side::Left, rel_y } = p.state else { panic!("{:?}", p.state) };
        assert_eq!(p.body.x, 800.0 - 38.0);
        assert_eq!((p.body.vx, p.body.vy), (0.0, 0.0));
        assert!(p.standing.is_none());
        assert_eq!(p.render().anim, Anim::Climb);
        p.update(DT, &w);
        assert!((p.body.y - (300.0 + rel_y - 90.0 * DT)).abs() < 1e-9);
        run_while(&mut p, &w, 60 * 10, |s| matches!(s, State::WallClimb { .. }));
        assert!(matches!(p.state, State::Land { .. }), "{:?}", p.state);
        let st = p.standing.unwrap();
        assert_eq!(st.owner, Owner::Window(1));
        assert_eq!((p.body.x, p.body.y), (800.0 + 38.0 + 2.0, 300.0));
    }

    #[test]
    fn thrown_climber_sticks_to_the_right_side_too() {
        let w = side_world();
        let mut p = thrown(climber(), 1400.0, 600.0, -600.0);
        run_while(&mut p, &w, 60, |s| matches!(s, State::Air { .. }));
        assert!(matches!(p.state, State::WallClimb { window: 1, side: Side::Right, .. }), "{:?}", p.state);
        assert_eq!(p.body.x, 1300.0 + 38.0);
        run_while(&mut p, &w, 60 * 10, |s| matches!(s, State::WallClimb { .. }));
        assert_eq!((p.body.x, p.body.y), (1300.0 - 38.0 - 2.0, 300.0));
    }

    #[test]
    fn wall_climber_follows_a_moving_window() {
        let w = side_world();
        let mut p = thrown(climber(), 700.0, 600.0, 600.0);
        run_while(&mut p, &w, 60, |s| matches!(s, State::Air { .. }));
        let State::WallClimb { rel_y, .. } = p.state else { panic!() };
        let moved = world(vec![win(1, 900.0, 350.0, 1400.0, 950.0)]);
        p.update(DT, &moved);
        assert_eq!(p.body.x, 900.0 - 38.0);
        assert!((p.body.y - (350.0 + rel_y - 90.0 * DT)).abs() < 1e-9);
    }

    #[test]
    fn wall_climber_falls_when_the_window_goes_or_the_side_gets_covered() {
        let w = side_world();
        let mut p = thrown(climber(), 700.0, 600.0, 600.0);
        run_while(&mut p, &w, 60, |s| matches!(s, State::Air { .. }));
        assert!(matches!(p.state, State::WallClimb { .. }));
        p.update(DT, &world(vec![]));
        assert_eq!(p.state, State::Air { anim: Anim::Fall });

        let mut p = thrown(climber(), 700.0, 600.0, 600.0);
        run_while(&mut p, &w, 60, |s| matches!(s, State::Air { .. }));
        assert!(matches!(p.state, State::WallClimb { .. }));
        p.update(DT, &world(vec![win(2, 700.0, 400.0, 900.0, 800.0), win(1, 800.0, 300.0, 1300.0, 900.0)]));
        assert_eq!(p.state, State::Air { anim: Anim::Fall });
    }

    /// 던진 펫이 창 옆면에 한 번이라도 달라붙었는지
    fn ever_sticks(cfg: PetConfig, w: &World, vx: f64) -> bool {
        let mut p = thrown(cfg, 700.0, 600.0, vx);
        for _ in 0..120 {
            p.update(DT, w);
            if matches!(p.state, State::WallClimb { .. }) {
                return true;
            }
        }
        false
    }

    #[test]
    fn covered_side_slow_throws_and_non_climbers_do_not_stick() {
        let covered = world(vec![win(2, 700.0, 400.0, 900.0, 800.0), win(1, 800.0, 300.0, 1300.0, 900.0)]);
        assert!(!ever_sticks(climber(), &covered, 600.0));
        assert!(!ever_sticks(climber(), &side_world(), 45.0));
        assert!(!ever_sticks(cfg(), &side_world(), 600.0));
        assert!(ever_sticks(climber(), &side_world(), 600.0));
    }

    /// 벽 타는 동물 10분 — 패닉 없이, 화면 안에서, 벽 타기 동작이 나온다
    #[test]
    fn climber_ten_minutes_smoke() {
        let w = world(vec![win(1, 300.0, 600.0, 1100.0, 900.0), win(2, 1300.0, 350.0, 1800.0, 700.0)]);
        let (mut climb, mut slide) = (0usize, 0usize);
        for seed in 0..8 {
            let mut p = Pet::new(PetConfig { can_climb: true, ..sim_cfg() }, 960.0, 128.0, seed);
            for _ in 0..(60 * 600) {
                p.update(DT, &w);
                match p.render().anim {
                    Anim::Climb => climb += 1,
                    Anim::Slide => slide += 1,
                    _ => {}
                }
                let b = w.bounds;
                assert!(p.body.x >= b.left && p.body.x <= b.right, "seed {seed}: x {}", p.body.x);
                assert!(p.body.y >= b.top + 128.0 - 1e-9 && p.body.y <= b.bottom, "seed {seed}: y {} {:?}", p.body.y, p.state);
            }
        }
        assert!(climb > 0 && slide > 0, "climb {climb} slide {slide}");
    }
}

#[cfg(test)]
mod seam_tests {
    use super::tests::{cfg, place, run, win};
    use super::*;
    use crate::behavior::Move;
    use crate::geom::Rect;

    const DT: f64 = 1.0 / 60.0;
    const KINDS: [Move; 3] = [Move::Walk, Move::Run, Move::Roll];

    /// 왼쪽 모니터 바닥 1040, 오른쪽 모니터 바닥 right_bottom (옆으로 맞닿음)
    fn duo(right_bottom: f64) -> World {
        World::new(vec![], vec![Rect::new(0.0, 0.0, 1920.0, 1040.0), Rect::new(1920.0, right_bottom - 1040.0, 3840.0, right_bottom)], 128.0)
    }

    /// 왼쪽 바닥 오른쪽 끝(몸 앞쪽이 막 닿기 직전)에서 오른쪽으로 kind 이동 시작
    fn at_seam(w: &World, kind: Move) -> Pet {
        let mut p = Pet::new(cfg(), 0.0, 0.0, 7);
        place(&mut p, w, Owner::Ground(0), 1920.0 - 38.0 - 0.5, State::Move { kind, remaining: 100.0 });
        p.facing = 1.0;
        p
    }

    /// 새 바닥에 내려설 때까지 돌리며 거친 상태를 모은다
    fn run_until_on(p: &mut Pet, w: &World, owner: Owner, ticks: usize) -> Vec<State> {
        let mut seen = Vec::new();
        for _ in 0..ticks {
            p.update(DT, w);
            seen.push(p.state);
            if p.standing.map(|s| s.owner) == Some(owner) {
                break;
            }
        }
        seen
    }

    #[test]
    fn walks_across_to_a_same_height_monitor_without_turning() {
        let w = duo(1040.0);
        for kind in KINDS {
            let mut p = at_seam(&w, kind);
            let seen = run_until_on(&mut p, &w, Owner::Ground(1), 600);
            assert_eq!(p.standing.map(|s| s.owner), Some(Owner::Ground(1)), "{kind:?}");
            assert!(seen.iter().all(|s| matches!(s, State::Move { .. })), "{kind:?}: {seen:?}");
            assert_eq!((p.body.y, p.facing), (1040.0, 1.0), "{kind:?}");
            // 이어서 매끄럽게 계속 걷는다
            let x = p.body.x;
            assert!(x >= 1920.0 && x < 1925.0, "{kind:?} x={x}");
            p.update(DT, &w);
            assert!(p.body.x > x && p.body.y == 1040.0, "{kind:?}");
        }
    }

    #[test]
    fn walks_across_leftward_too() {
        let w = duo(1041.5);
        let mut p = Pet::new(cfg(), 0.0, 0.0, 7);
        place(&mut p, &w, Owner::Ground(1), 1920.0 + 38.0 + 0.5, State::Move { kind: Move::Walk, remaining: 100.0 });
        p.facing = -1.0;
        let seen = run_until_on(&mut p, &w, Owner::Ground(0), 600);
        assert_eq!(p.standing.map(|s| s.owner), Some(Owner::Ground(0)));
        assert!(seen.iter().all(|s| matches!(s, State::Move { .. })), "{seen:?}");
        assert!(p.body.x <= 1920.0 && p.body.y == 1040.0);
    }

    #[test]
    fn hops_or_rolls_down_to_a_lower_monitor() {
        let w = duo(1240.0);
        for kind in KINDS {
            let mut p = at_seam(&w, kind);
            let seen = run_until_on(&mut p, &w, Owner::Ground(1), 600);
            let air = if kind == Move::Roll { Anim::Roll } else { Anim::Hop };
            assert!(seen.contains(&State::Air { anim: air }), "{kind:?}: {seen:?}");
            assert!(!seen.iter().any(|s| matches!(s, State::Turn { .. })), "{kind:?}");
            assert_eq!(p.standing.map(|s| s.owner), Some(Owner::Ground(1)), "{kind:?}");
            assert_eq!(p.body.y, 1240.0);
        }
    }

    #[test]
    fn hops_up_to_a_monitor_100px_higher() {
        let w = duo(940.0);
        for kind in [Move::Walk, Move::Run] {
            let mut p = at_seam(&w, kind);
            p.update(DT, &w);
            assert_eq!(p.state, State::Air { anim: Anim::Hop }, "{kind:?}");
            let seen = run_until_on(&mut p, &w, Owner::Ground(1), 600);
            assert_eq!(p.standing.map(|s| s.owner), Some(Owner::Ground(1)), "{kind:?}: {seen:?}");
            assert_eq!(p.body.y, 940.0);
        }
    }

    #[test]
    fn hop_up_height_limit_scales_with_size() {
        // 140 px 위: 크기 128이면 뛰어오르고, 크기 64(한계 75 px)면 돌아선다
        let w = duo(900.0);
        let mut p = at_seam(&w, Move::Walk);
        p.update(DT, &w);
        assert_eq!(p.state, State::Air { anim: Anim::Hop });
        run_until_on(&mut p, &w, Owner::Ground(1), 600);
        assert_eq!(p.standing.map(|s| s.owner), Some(Owner::Ground(1)));

        let small = PetConfig { size: 64.0, half_w: 19.0, ..cfg() };
        let mut p = Pet::new(small, 0.0, 0.0, 7);
        place(&mut p, &w, Owner::Ground(0), 1920.0 - 19.0 - 0.5, State::Move { kind: Move::Walk, remaining: 100.0 });
        p.facing = 1.0;
        p.update(DT, &w);
        assert!(matches!(p.state, State::Turn { .. }), "{:?}", p.state);
    }

    #[test]
    fn turns_at_a_monitor_too_high_to_reach_and_when_rolling_up() {
        let w = duo(640.0);
        for kind in KINDS {
            let mut p = at_seam(&w, kind);
            p.update(DT, &w);
            assert!(matches!(p.state, State::Turn { .. }), "{kind:?}: {:?}", p.state);
        }
        let w = duo(940.0);
        let mut p = at_seam(&w, Move::Roll);
        p.update(DT, &w);
        assert!(matches!(p.state, State::Turn { .. }), "{:?}", p.state);
    }

    #[test]
    fn outermost_edges_are_still_walls() {
        let w = duo(1040.0);
        for kind in KINDS {
            let mut p = Pet::new(cfg(), 0.0, 0.0, 7);
            place(&mut p, &w, Owner::Ground(1), 3840.0 - 38.0 - 0.5, State::Move { kind, remaining: 100.0 });
            p.facing = 1.0;
            p.update(DT, &w);
            assert!(matches!(p.state, State::Turn { .. }), "right {kind:?}");
            place(&mut p, &w, Owner::Ground(0), 38.0 + 0.5, State::Move { kind, remaining: 100.0 });
            p.facing = -1.0;
            p.update(DT, &w);
            assert!(matches!(p.state, State::Turn { .. }), "left {kind:?}");
        }
    }

    #[test]
    fn stacked_monitors_keep_walls() {
        let w = World::new(vec![], vec![Rect::new(0.0, 0.0, 1920.0, 1040.0), Rect::new(0.0, -1080.0, 1920.0, -40.0)], 128.0);
        for (owner, y) in [(Owner::Ground(0), 1040.0), (Owner::Ground(1), -40.0)] {
            let mut p = Pet::new(cfg(), 0.0, 0.0, 7);
            place(&mut p, &w, owner, 1920.0 - 38.0 - 0.5, State::Move { kind: Move::Walk, remaining: 100.0 });
            p.facing = 1.0;
            p.update(DT, &w);
            assert!(matches!(p.state, State::Turn { .. }), "{owner:?}");
            assert_eq!(p.body.y, y);
        }
    }

    #[test]
    fn window_spanning_the_seam_is_one_piece() {
        let w = World::new(
            vec![win(1, 1700.0, 600.0, 2200.0, 900.0)],
            vec![Rect::new(0.0, 0.0, 1920.0, 1040.0), Rect::new(1920.0, 0.0, 3840.0, 1040.0)],
            128.0,
        );
        let mut p = Pet::new(cfg(), 0.0, 0.0, 7);
        place(&mut p, &w, Owner::Window(1), 1900.0, State::Move { kind: Move::Walk, remaining: 100.0 });
        p.facing = 1.0;
        run(&mut p, &w, 120);
        assert_eq!(p.standing.map(|s| s.owner), Some(Owner::Window(1)));
        assert!(p.body.x > 1920.0);
    }
}
