//! 곁에 있기와 하루 리듬의 동작 (v0.3.7) — brain.rs의 자식 모듈 (비공개 필드에 접근). 언제 할지는 presence.rs가 정한다.
//!
//! - 자리 비움: 쉬거나 걷던 내 동물이 잠들고, 돌아올 때까지 잔다. 돌아오면 커서가 있는 모니터로 달려가 (바닥이 다르면 폴짝, 나는 동물은 날아서)
//!   통통 뛰며 "you're back!!" + 하트.
//! - 졸린 시간: 하품이 잦고, 더 자주 더 오래 자고, 가끔 "go to bed~". 수면 모자는 웹뷰가 그린다.
//! - 집중: 지금 모니터의 오른쪽 아래 구석에 앉아 조용히 있는다 (돌아다니기·수다·간질이기·커서 장난·친구 놀이 없음).
//!   사용자 손길(잡기·먹이·조작·집 가기·보내기)이 먼저고, 끝나면 구석으로 돌아온다.

use super::*;

/// 자리 비움: 잠이 끝나지 않게 남은 시간을 이만큼으로 유지한다 (돌아왔는데 맞으러 못 가도 그 안에 깬다)
const NAP_HOLD_SECS: f64 = 60.0;
/// 돌아온 사용자를 맞는 통통 횟수
const WELCOME_BOUNCES: u8 = 2;
/// 통통마다 하트 하나 (뛰는 시간 0.4초보다 짧아야 다음 통통에 새 하트가 뜬다)
const WELCOME_HEART_SECS: f64 = 0.3;
/// 뛰지 못하는 때 (나는 동물·창 안·하우스)의 하트
const WELCOME_HOVER_SECS: f64 = 2.0;
/// 맞으러 / 구석으로 달리는 최대 시간 — 넓은 화면에선 그만큼 빨리 달린다
const WELCOME_RUN_SECS: f64 = 2.5;
const FOCUS_RUN_SECS: f64 = 5.0;
/// 커서에서 이 거리(몸 배) 옆에 멈춘다
const WELCOME_REACH: f64 = 0.8;
/// 나는 동물이 커서 아래 이만큼(몸 배)에 뜬다
const WELCOME_FLY_DROP: f64 = 0.9;
/// 이미 이만큼(몸 배) 가까우면 그 자리에서 맞는다
const WELCOME_NEAR: f64 = 1.5;
/// 폴짝 가기: 솟는 높이(몸 배), 속도(크기 128 기준 px/s), 시간 범위
const LEAP_RISE: f64 = 1.2;
const LEAP_SPEED: f64 = 1100.0;
const LEAP_SECS: (f64, f64) = (0.6, 1.4);
/// 나는 동물이 곧장 가는 속도 (걷기 속도 배), 시간 범위
const GLIDE_MULT: f64 = 3.0;
const GLIDE_SECS: (f64, f64) = (0.5, 3.0);
/// 구석에서 몸 중심까지 (몸 배)
const CORNER_INSET: f64 = 0.75;
/// 이만큼(px) 안이면 구석에 도착한 것이다
const ARRIVED_PX: f64 = 4.0;
/// 아침 기지개, 집중이 끝난 기지개
const MORNING_STRETCH_SECS: f64 = 3.0;
const BREAK_STRETCH_SECS: f64 = 2.5;
const BREAK_HEART_SECS: f64 = 2.0;
/// 졸린 시간: 잠들 확률, 잠 길이 배수, 쉬다가 하품할 확률, 하품 시간, "go to bed~" 간격
const SLEEPY_SLEEP_CHANCE: f64 = 0.6;
const SLEEPY_SLEEP_MULT: f64 = 3.0;
const YAWN_CHANCE: f64 = 0.3;
const YAWN_SECS: f64 = 2.5;
const BED_NAG_SECS: (f64, f64) = (240.0, 480.0);
const FIRST_BED_NAG_SECS: (f64, f64) = (60.0, 180.0);

#[derive(Debug, Clone, Copy, PartialEq)]
enum TripWhy {
    Welcome,
    Focus,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum TripWay {
    /// 같은 바닥을 따라 달린다
    Run,
    /// 충돌 없이 포물선
    Leap,
    /// 나는 동물이 곧장
    Glide,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum TripEnd {
    On { owner: Owner, rel_x: f64 },
    Float(f64, f64),
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct Trip {
    why: TripWhy,
    way: TripWay,
    from: (f64, f64),
    end: TripEnd,
    elapsed: f64,
    secs: f64,
}

#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub(super) struct PresenceMood {
    user_away: bool,
    sleepy: bool,
    focus: bool,
    /// 남은 통통
    bounces: u8,
    bed_nag_in: f64,
    trip: Option<Trip>,
}

/// (x, y)가 있는 모니터의 바닥
fn ground_at(world: &World, x: f64, y: f64) -> Option<(usize, Rect)> {
    if world.work_areas.is_empty() {
        return None;
    }
    let wa = world.work_area_at(x, y);
    world.work_areas.iter().position(|r| *r == wa).map(|i| (i, wa))
}

impl Pet {
    /// 지금 시각이 졸린 시간인지
    pub fn set_sleepy(&mut self, sleepy: bool) {
        if sleepy && !self.presence.sleepy {
            self.presence.bed_nag_in = behavior::range(&mut self.rng, FIRST_BED_NAG_SECS);
        }
        self.presence.sleepy = sleepy;
    }

    /// 사용자가 자리를 비웠다 / 돌아왔다 (돌아왔을 땐 welcome_back도 부른다)
    pub fn set_user_away(&mut self, away: bool) {
        self.presence.user_away = away;
    }

    /// ⏱️ 집중 중 — 구석에 앉아 조용히
    pub fn set_focus(&mut self, on: bool) {
        self.presence.focus = on;
    }

    pub fn say_focus(&mut self) {
        self.speech = Some(Speech::Focus);
    }

    /// 자리 비움·집중 중 — 커서 장난·친구 놀이를 시작하지 않는다
    pub(super) fn presence_quiet(&self) -> bool {
        self.presence.user_away || self.presence.focus
    }

    /// ⏱️ 집중 중인가 (그동안 선물을 떨어뜨리지 않는다, gift.rs)
    pub fn is_focusing(&self) -> bool {
        self.presence.focus
    }

    pub(super) fn focusing(&self) -> bool {
        self.presence.focus
    }

    /// 사용자·손님 볼일이 먼저다
    fn presence_blocked(&self) -> bool {
        self.guest.is_some()
            || self.control.is_some()
            || self.home_trip.is_some()
            || self.return_home.is_some()
            || self.food.is_some()
            || self.meal_pending.is_some()
            || self.queued_food.is_some()
            || self.hand.is_some()
            || self.send.is_some()
            || self.farewell_pending
    }

    /// update에서 (커서 장난 다음): 이번 틱을 다 처리했으면 true
    pub(super) fn update_presence(&mut self, dt: f64, world: &World) -> bool {
        let tripping = matches!(self.state, State::PresenceTrip);
        if !tripping {
            self.presence.trip = None;
        }
        if self.is_held() {
            self.presence.bounces = 0;
            return false;
        }
        if self.presence.bounces > 0 && matches!(self.state, State::Land { .. }) && self.standing.is_some() {
            self.presence.bounces -= 1;
            self.bounce_once();
            return false;
        }
        if self.presence_blocked() {
            if tripping {
                self.end_trip();
            }
            return false;
        }
        if tripping {
            self.update_trip(dt, world);
            return true;
        }
        if self.presence.focus {
            return self.focus_tick(world);
        }
        if self.presence.user_away {
            self.keep_napping();
            return false;
        }
        self.nag_bedtime(dt);
        false
    }

    // ---- 자리 비움 ----

    fn keep_napping(&mut self) {
        match self.state {
            State::Sleep { remaining } => self.state = State::Sleep { remaining: remaining.max(NAP_HOLD_SECS) },
            State::Idle { .. } | State::Move { .. } | State::Turn { .. } => {
                self.pacing.after_beat = None;
                self.flight = None;
                self.state = State::Sleep { remaining: NAP_HOLD_SECS };
            }
            // 하던 동작·공중은 끝나면 쉬기로 온다
            _ => {}
        }
    }

    /// 돌아왔다: 깨어 커서 쪽으로 가서 통통. 맞이할 수 없는 때(사용자 볼일·집중·친구 화면)면 false.
    /// 하우스 안이나 공중이면 그 자리에서 말과 하트만
    pub fn welcome_back(&mut self, world: &World) -> bool {
        if self.housed {
            self.speech = Some(Speech::WelcomeBack);
            self.heart_timer = WELCOME_HOVER_SECS;
            return true;
        }
        if self.presence_blocked() || self.presence.focus || self.is_held() || matches!(self.state, State::Away) {
            return false;
        }
        let free = matches!(
            self.state,
            State::Sleep { .. }
                | State::Annoyed { .. }
                | State::Idle { .. }
                | State::Move { .. }
                | State::Turn { .. }
                | State::Land { .. }
                | State::Trick { .. }
                | State::Huff { .. }
                | State::Special { .. }
                | State::Hungry { .. }
        );
        if !free {
            self.speech = Some(Speech::WelcomeBack);
            self.heart_timer = WELCOME_HOVER_SECS;
            return true;
        }
        let (cx, cy) = self.cursor;
        let Some((i, wa)) = ground_at(world, cx, cy) else { return false };
        let side = if self.body.x < cx { -1.0 } else { 1.0 };
        let reach = WELCOME_REACH * self.cfg.size;
        let near = WELCOME_NEAR * self.cfg.size;
        if self.cfg.flies {
            self.inside = None;
            let (x, y) = crate::flight::clamp(&self.fly_limits(world), cx + side * reach, cy + WELCOME_FLY_DROP * self.cfg.size);
            if (x - self.body.x).hypot(y - self.body.y) < near {
                self.happy_bounce();
            } else {
                self.start_trip(TripWhy::Welcome, TripEnd::Float(x, y), world, WELCOME_RUN_SECS);
            }
            return true;
        }
        let lo = wa.left + self.cfg.half_w;
        let x = (cx + side * reach).clamp(lo, (wa.right - self.cfg.half_w).max(lo));
        if (x - self.body.x).hypot(wa.bottom - self.body.y) < near {
            self.happy_bounce();
        } else {
            self.start_trip(TripWhy::Welcome, TripEnd::On { owner: Owner::Ground(i), rel_x: x - wa.left }, world, WELCOME_RUN_SECS);
        }
        true
    }

    /// "you're back!!" + 통통 (나는 동물·창 안이면 제자리에서 하트)
    fn happy_bounce(&mut self) {
        self.speech = Some(Speech::WelcomeBack);
        self.pacing.after_beat = None;
        if self.cfg.flies || self.inside.is_some() {
            self.heart_timer = WELCOME_HOVER_SECS;
            self.state = State::Trick { kind: Trick::Heart, remaining: WELCOME_HOVER_SECS };
            return;
        }
        self.presence.bounces = WELCOME_BOUNCES - 1;
        self.bounce_once();
    }

    fn bounce_once(&mut self) {
        self.heart_timer = WELCOME_HEART_SECS;
        self.standing = None;
        self.body.vx = 0.0;
        self.body.vy = HAPPY_VY;
        self.state = State::Air { anim: Anim::Hop };
    }

    // ---- 달려가기 ----

    fn start_trip(&mut self, why: TripWhy, end: TripEnd, world: &World, max_run_secs: f64) {
        let Some(to) = trip_target(end, world) else { return };
        let dx = to.0 - self.body.x;
        let dist = dx.hypot(to.1 - self.body.y);
        if dx.abs() > 1.0 {
            self.facing = dx.signum();
        }
        let same_floor = match end {
            TripEnd::On { owner, .. } => self.standing.is_some_and(|s| s.owner == owner),
            TripEnd::Float(..) => false,
        };
        let (way, secs) = if self.cfg.flies {
            (TripWay::Glide, (dist / (self.cfg.walk_speed * GLIDE_MULT).max(1.0)).clamp(GLIDE_SECS.0, GLIDE_SECS.1))
        } else if same_floor && !self.cfg.hops {
            (TripWay::Run, (dist / (self.cfg.walk_speed * Move::Run.speed_mult()).max(1.0)).min(max_run_secs))
        } else {
            (TripWay::Leap, (dist / self.scaled(LEAP_SPEED).max(1.0)).clamp(LEAP_SECS.0, LEAP_SECS.1))
        };
        self.presence.bounces = 0;
        self.pacing.after_beat = None;
        self.flight = None;
        self.hover_at = None;
        self.inside = None;
        self.thrown = false;
        self.clear_hover();
        self.body.vx = 0.0;
        self.body.vy = 0.0;
        if way != TripWay::Run {
            self.standing = None;
        }
        self.presence.trip = Some(Trip { why, way, from: (self.body.x, self.body.y), end, elapsed: 0.0, secs });
        self.state = State::PresenceTrip;
    }

    fn update_trip(&mut self, dt: f64, world: &World) {
        let Some(mut trip) = self.presence.trip else {
            self.end_trip();
            return;
        };
        let Some(to) = trip_target(trip.end, world) else {
            self.end_trip();
            return;
        };
        trip.elapsed += dt;
        let t = if trip.secs > 0.0 { (trip.elapsed / trip.secs).min(1.0) } else { 1.0 };
        let lerp = |a: f64, b: f64, t: f64| a + (b - a) * t;
        let (x, y) = match trip.way {
            TripWay::Run => (lerp(trip.from.0, to.0, t), to.1),
            TripWay::Leap => {
                let y = lerp(trip.from.1, to.1, t) - 4.0 * LEAP_RISE * self.cfg.size * t * (1.0 - t);
                (lerp(trip.from.0, to.0, t), y.max(world.bounds.top + self.cfg.size))
            }
            TripWay::Glide => {
                let s = t * t * (3.0 - 2.0 * t);
                (lerp(trip.from.0, to.0, s), lerp(trip.from.1, to.1, s))
            }
        };
        self.body = Body { x, y, vx: 0.0, vy: 0.0 };
        if let (TripWay::Run, TripEnd::On { owner, .. }, Some((ax, _))) = (trip.way, trip.end, trip_anchor(trip.end, world)) {
            self.standing = Some(Standing { owner, rel_x: x - ax });
        }
        if t < 1.0 {
            self.presence.trip = Some(trip);
            return;
        }
        self.presence.trip = None;
        match trip.end {
            TripEnd::On { owner, rel_x } => self.standing = Some(Standing { owner, rel_x }),
            TripEnd::Float(hx, hy) => {
                self.standing = None;
                self.hover_at = Some((hx, hy));
            }
        }
        match trip.why {
            TripWhy::Welcome => self.happy_bounce(),
            TripWhy::Focus => self.sit_in_corner(),
        }
    }

    /// 가던 길을 그만둔다 (사용자 볼일이 생겼거나 도착할 자리가 사라졌다)
    fn end_trip(&mut self) {
        self.presence.trip = None;
        if self.standing.is_some() || self.cfg.flies {
            self.state = self.idle_state();
        } else {
            self.start_fall();
        }
    }

    pub(super) fn trip_anim(&self) -> Anim {
        match self.presence.trip.map(|t| t.way) {
            Some(TripWay::Run) => Anim::Run,
            Some(TripWay::Leap) => Anim::Hop,
            Some(TripWay::Glide) => Anim::Walk,
            None => Anim::Idle,
        }
    }

    // ---- 집중 ----

    /// 구석으로 가거나 구석에 앉아 있는다. 지금 하는 일을 먼저 끝내야 하면 false
    fn focus_tick(&mut self, world: &World) -> bool {
        let movable = matches!(
            self.state,
            State::Idle { .. }
                | State::Move { .. }
                | State::Turn { .. }
                | State::Sleep { .. }
                | State::Hungry { .. }
                | State::Special { .. }
                | State::Snowman { .. }
                | State::Climb { .. }
                | State::Slide
                | State::WallClimb { .. }
                | State::InnerClimb { .. }
        );
        if !movable {
            return false;
        }
        let Some((i, wa)) = ground_at(world, self.body.x, self.body.y) else { return false };
        let x = (wa.right - CORNER_INSET * self.cfg.size).max(wa.left + self.cfg.half_w);
        let owner = Owner::Ground(i);
        let there = self.standing.is_some_and(|s| s.owner == owner) && (self.body.x - x).abs() <= ARRIVED_PX;
        if there {
            if self.follow_anchor(world) {
                self.sit_in_corner();
            } else {
                self.start_fall();
            }
        } else {
            self.start_trip(TripWhy::Focus, TripEnd::On { owner, rel_x: x - wa.left }, world, FOCUS_RUN_SECS);
        }
        true
    }

    /// 화면 쪽을 보고 앉아 있다 — 이 자세가 끝나지 않게 매 틱 다시 채운다 (집중이 끝나면 곧 평소대로)
    fn sit_in_corner(&mut self) {
        self.pacing.after_beat = None;
        self.gaze_up = false;
        self.facing = -1.0;
        self.state = State::Idle { remaining: 1.0, look: false };
    }

    /// 집중이 끝났다: 기지개 + "break time!" + 하트 (하우스 안이나 다른 일 중이면 말과 하트만)
    pub fn focus_break(&mut self) {
        if matches!(self.state, State::Away) {
            return;
        }
        self.speech = Some(Speech::BreakTime);
        self.heart_timer = BREAK_HEART_SECS;
        if !self.housed && !self.presence_blocked() && matches!(self.state, State::Idle { .. } | State::Move { .. } | State::Turn { .. } | State::Sleep { .. } | State::Hungry { .. }) {
            self.pacing.after_beat = None;
            self.state = State::Trick { kind: Trick::Stretch, remaining: BREAK_STRETCH_SECS };
        }
    }

    // ---- 하루 일과 ----

    /// 큰 기지개 + "good morning!" — 지금 할 수 없으면 false (다음에 다시)
    pub fn good_morning(&mut self) -> bool {
        if self.housed {
            self.speech = Some(Speech::GoodMorning);
            return true;
        }
        if self.presence_blocked() || self.presence.focus || !matches!(self.state, State::Idle { .. } | State::Move { .. } | State::Turn { .. } | State::Sleep { .. }) {
            return false;
        }
        self.pacing.after_beat = None;
        self.flight = None;
        self.speech = Some(Speech::GoodMorning);
        self.state = State::Trick { kind: Trick::Stretch, remaining: MORNING_STRETCH_SECS };
        true
    }

    /// 시무룩 + "lunch time?" — 지금 할 수 없으면 false (다음에 다시). 배고프게 만드는 건 돌보기 쪽 (presence.rs)
    pub fn lunch_time(&mut self) -> bool {
        if self.housed {
            self.speech = Some(Speech::LunchTime);
            return true;
        }
        if self.presence_blocked() || self.presence.focus || !matches!(self.state, State::Idle { .. } | State::Move { .. } | State::Turn { .. }) {
            return false;
        }
        self.pacing.after_beat = None;
        self.flight = None;
        self.speech = Some(Speech::LunchTime);
        self.state = State::Hungry { remaining: behavior::range(&mut self.rng, HUNGRY_SECS) };
        true
    }

    /// 쉬는 박자가 끝날 때: 졸리면 가끔 하품
    pub(super) fn sleepy_rest(&mut self) -> Option<State> {
        if !self.presence.sleepy || self.rng.f64() >= YAWN_CHANCE {
            return None;
        }
        self.speech.get_or_insert(Speech::Yawn);
        Some(State::Trick { kind: Trick::Stretch, remaining: YAWN_SECS })
    }

    /// 졸린 시간엔 훨씬 자주 잠든다 (떠 있는 나는 동물도 그 자리에서 꾸벅)
    pub(super) fn sleep_chance(&self, base: f64) -> f64 {
        if self.presence.sleepy {
            base.max(SLEEPY_SLEEP_CHANCE)
        } else {
            base
        }
    }

    /// 이번 잠의 길이 (졸린 시간엔 훨씬 길다)
    pub(super) fn sleep_secs(&mut self) -> f64 {
        let secs = behavior::range(&mut self.rng, behavior::SLEEP_SECS);
        if self.presence.sleepy {
            secs * SLEEPY_SLEEP_MULT
        } else {
            secs
        }
    }

    /// 졸린 시간에 깨어 있으면 가끔 "go to bed~"
    fn nag_bedtime(&mut self, dt: f64) {
        if !self.presence.sleepy || matches!(self.state, State::Sleep { .. } | State::Annoyed { .. }) {
            return;
        }
        self.presence.bed_nag_in -= dt;
        if self.presence.bed_nag_in <= 0.0 && self.speech.is_none() {
            self.speech = Some(Speech::GoToBed);
            self.presence.bed_nag_in = behavior::range(&mut self.rng, BED_NAG_SECS);
        }
    }
}

fn trip_anchor(end: TripEnd, world: &World) -> Option<(f64, f64)> {
    match end {
        TripEnd::On { owner, .. } => world.anchor(owner),
        TripEnd::Float(..) => None,
    }
}

fn trip_target(end: TripEnd, world: &World) -> Option<(f64, f64)> {
    match end {
        TripEnd::On { owner, rel_x } => world.anchor(owner).map(|(ax, ay)| (ax + rel_x, ay)),
        TripEnd::Float(x, y) => Some((x, y)),
    }
}
