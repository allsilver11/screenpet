//! 곁에 있기와 하루 리듬의 동작 (v0.3.7) — brain.rs의 자식 모듈 (비공개 필드에 접근). 언제 할지는 presence.rs가 정함
//!
//! - 자리 비움: 내 동물도 이 화면의 손님도 하던 일을 그만두고 돌아올 때까지 잠만 잠 (update_user_away). 말도 놀이도 쿵쾅도 없고, 나는 동물은
//!   앉은 자리나 떠 있던 자리에서 졺. 떨어지기·창 따라가기 같은 물리만 그대로고, 내려서면 바로 다시 잠듦
//!   이기는 것: 사용자 볼일(먹이·하우스·조작 등) — ⏱️ 집중 타이머는 조용히 돌기만 하고 (돌아오면 그때 국면으로), 친구의 "펫 데리고 오기"(손 흔들고 뿅 건너감, 돌아오면 다시 잠)와
//!   손님의 그리움 상한·"집에 와", 그리고 주인이 키로 조종 중인 손님 (주인은 자리에 있으니 키를 따르고, 키가 끊기면 다시 잠듦).
//!   돌아오면 바로 깨어 커서가 있는 모니터로 달려가 (바닥이 다르면 폴짝, 나는 동물은 날아서) 통통 뛰며 "you're back!!" + 하트.
//! - 졸린 시간: 하품이 잦고, 더 자주 더 오래 자고, 가끔 "go to bed~". 수면 모자는 웹뷰가 그림
//! - 집중: 지금 모니터의 오른쪽 아래 구석에 앉아 조용히 있음 (돌아다니기·수다·간질이기·커서 장난·친구 놀이 없음).
//!   사용자 손길(잡기·먹이·조작·집 가기·보내기)이 먼저고, 끝나면 구석으로 돌아옴

use super::*;

/// 자리 비움: 잠이 끝나지 않게 남은 시간을 매 틱 이만큼으로 채움 (돌아오면 set_user_away가 바로 깨움)
const NAP_HOLD_SECS: f64 = 60.0;
/// 돌아온 사용자를 맞는 통통 횟수
const WELCOME_BOUNCES: u8 = 2;
/// 통통마다 하트 하나 (뛰는 시간 0.4초보다 짧아야 다음 통통에 새 하트가 뜸)
const WELCOME_HEART_SECS: f64 = 0.3;
/// 뛰지 못하는 때 (나는 동물·창 안·하우스)의 하트
const WELCOME_HOVER_SECS: f64 = 2.0;
/// 맞으러 / 구석으로 달리는 최대 시간 — 넓은 화면에선 그만큼 빨리 달림
const WELCOME_RUN_SECS: f64 = 2.5;
const FOCUS_RUN_SECS: f64 = 5.0;
/// 커서에서 이 거리(몸 배) 옆에 멈춤
const WELCOME_REACH: f64 = 0.8;
/// 나는 동물이 커서 아래 이만큼(몸 배)에 뜸
const WELCOME_FLY_DROP: f64 = 0.9;
/// 이미 이만큼(몸 배) 가까우면 그 자리에서 맞음
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
/// 이만큼(px) 안이면 구석에 도착한 것임
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
    /// 같은 바닥을 따라 달림
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

/// 마중·배웅 길: 왜, 어떻게, 어디서 어디로, 걸린 시간
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct Trip {
    why: TripWhy,
    way: TripWay,
    from: (f64, f64),
    end: TripEnd,
    elapsed: f64,
    secs: f64,
}

/// 자리 비움·졸림·집중·길의 기억
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

    /// 사용자가 자리를 비웠음 / 돌아왔음 — 돌아오면 바로 깸 (내 동물은 welcome_back도 부름, 손님은 깨기만)
    pub fn set_user_away(&mut self, away: bool) {
        let back = self.presence.user_away && !away;
        self.presence.user_away = away;
        if back {
            self.wake_for_user();
        }
    }

    /// 돌아온 사용자 때문에 깸. 자고 있으면 일어남
    fn wake_for_user(&mut self) {
        if matches!(self.state, State::Sleep { .. }) {
            self.pacing.after_beat = None;
            self.state = self.idle_state();
        }
        // 돌아온 사용자의 첫 입력이 동물을 누른 것이었다: 놓으면 깨어 있던 것처럼
        if let State::Pressed { start_cx, start_cy, grounded, was_sleeping: true } = self.state {
            self.state = State::Pressed { start_cx, start_cy, grounded, was_sleeping: false };
            let awake = self.idle_state();
            self.before_press = Some((awake, None));
        }
    }

    /// 사용자가 자리를 비웠음 — 잠만 잠
    pub fn is_user_away(&self) -> bool {
        self.presence.user_away
    }

    /// ⏱️ 집중 중 — 구석에 앉아 조용히
    pub fn set_focus(&mut self, on: bool) {
        self.presence.focus = on;
    }

    /// 집중 시작 인사를 함
    pub fn say_focus(&mut self) {
        self.speech = Some(Speech::Focus);
    }

    /// 자리 비움·집중 중, 🔄 새 버전 응원 중 — 커서 장난·친구 놀이를 시작하지 않음
    pub(super) fn presence_quiet(&self) -> bool {
        self.presence.user_away || self.presence.focus || self.cheering()
    }

    /// ⏱️ 집중 중인가 (그동안 선물을 떨어뜨리지 않음, gift.rs)
    pub fn is_focusing(&self) -> bool {
        self.presence.focus
    }

    /// 집중 중(쉬는 시간 제외)이거나 하우스 안이면 새 손님(걸어오든 던져지든)과 던진 물건을 받지 않음 (이미 와 있는 손님은 그대로)
    pub fn refuses_arrivals(&self) -> bool {
        self.presence.focus || self.housed
    }

    /// 집중 중임
    pub(super) fn focusing(&self) -> bool {
        self.presence.focus
    }

    /// 사용자·손님 볼일이 먼저임
    fn presence_blocked(&self) -> bool {
        self.guest.is_some() || self.has_errand()
    }

    /// 사용자가 시킨 일, 주인의 조종, 떠나는 길
    pub(super) fn has_errand(&self) -> bool {
        self.control.is_some()
            || self.home_trip.is_some()
            || self.return_home.is_some()
            || self.food.is_some()
            || self.meal_pending.is_some()
            || self.queued_food.is_some()
            || self.hand.is_some()
            || self.send.is_some()
            || self.farewell_pending
    }

    /// 지금 잠만 자야 함 (손님도)
    pub(super) fn sleeps_for_user(&self) -> bool {
        self.presence.user_away && !self.has_errand()
    }

    /// update에서 (커서 장난 전): 자리 비움이면 잠만 잠. 이번 틱을 다 처리했으면 true.
    /// 공중·들림·떠나고 들어오는 길·먹기는 평소 길로 끝나고 (내려서면 다음 틱에 여기로 옴), 나머지는 곧바로 그만둠
    pub(super) fn update_user_away(&mut self, dt: f64, world: &World) -> bool {
        if !self.sleeps_for_user() {
            return false;
        }
        match self.state {
            State::Air { .. }
            | State::Pressed { .. }
            | State::Dragged
            | State::Enter { .. }
            | State::Exit { .. }
            | State::Farewell { .. }
            | State::Away
            | State::Eating { .. }
            | State::LevelUp { .. }
            | State::Lifted { .. } => return false,
            // 벽·커서·창 모서리에 매달려 있었거나 뛰던 중: 떨어져 내린 뒤에 잔다 (나는 동물은 떠서)
            State::Climb { .. }
            | State::Slide
            | State::WallClimb { .. }
                | State::Rig { .. }
            | State::InnerClimb { .. }
            | State::Leap { .. }
            | State::Meet { .. }
            | State::HomeLeap { .. }
            | State::CursorLeap { .. }
            | State::CursorHang { .. }
            | State::EdgeCling { .. }
            | State::Swing { .. }
            | State::PresenceTrip => {
                self.settle_for_sleep();
                if !self.cfg.flies {
                    self.start_fall();
                    return true;
                }
                self.standing = None;
            }
            State::Sleep { .. } => {}
            _ => self.settle_for_sleep(),
        }
        self.state = State::Sleep { remaining: NAP_HOLD_SECS };
        // 디딤판을 따라가고, 사라졌으면 떨어지거나 떠오른다 — 평소의 잠과 같은 길
        match (self.cfg.flies, self.standing.is_some()) {
            (false, _) => self.update_ground(dt, world),
            (true, true) => self.update_perched(dt, world),
            (true, false) => self.update_flying(dt, world),
        }
        true
    }

    /// 잠들기 전에 하던 길과 놀이를 잊음
    fn settle_for_sleep(&mut self) {
        self.pacing.after_beat = None;
        self.flight = None;
        self.hover_at = None;
        self.gaze_up = false;
        self.play_target = None;
        self.body.vx = 0.0;
        self.body.vy = 0.0;
        self.presence.bounces = 0;
        self.presence.trip = None;
        self.forget_glare();
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
        if self.presence.user_away {
            return false;
        }
        if self.presence.focus {
            return self.focus_tick(world);
        }
        self.nag_bedtime(dt);
        false
    }

    // ---- 돌아옴 ----

    /// 돌아왔음: 깨어 커서 쪽으로 가서 통통. 맞이할 수 없는 때(사용자 볼일·집중·친구 화면)면 false.
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

    /// 반가워 한 번 통통 뜀 (하트)
    fn bounce_once(&mut self) {
        self.heart_timer = WELCOME_HEART_SECS;
        self.standing = None;
        self.body.vx = 0.0;
        self.body.vy = HAPPY_VY;
        self.state = State::Air { anim: Anim::Hop };
    }

    // ---- 달려가기 ----

    /// 정한 곳으로 가는 길을 시작함. 거리에 따라 달리기·뛰기·날기를 고름
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

    /// 길 한 틱: 목적지로 가다가 닿으면 끝냄
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

    /// 가던 길을 그만둠 (사용자 볼일이 생겼거나 도착할 자리가 사라졌음)
    fn end_trip(&mut self) {
        self.presence.trip = None;
        if self.standing.is_some() || self.cfg.flies {
            self.state = self.idle_state();
        } else {
            self.start_fall();
        }
    }

    /// 길 그림 (달리기·뛰기·날기)
    pub(super) fn trip_anim(&self) -> Anim {
        match self.presence.trip.map(|t| t.way) {
            Some(TripWay::Run) => Anim::Run,
            Some(TripWay::Leap) => Anim::Hop,
            Some(TripWay::Glide) => Anim::Walk,
            None => Anim::Idle,
        }
    }

    // ---- 집중 ----

    /// 구석으로 가거나 구석에 앉아 있음. 지금 하는 일을 먼저 끝내야 하면 false
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
                | State::Rig { .. }
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

    /// 화면 쪽을 보고 앉아 있음 — 이 자세가 끝나지 않게 매 틱 다시 채움 (집중이 끝나면 곧 평소대로)
    fn sit_in_corner(&mut self) {
        self.pacing.after_beat = None;
        self.gaze_up = false;
        self.facing = -1.0;
        self.state = State::Idle { remaining: 1.0, look: false };
    }

    /// 집중이 끝났음: 기지개 + "break time!" + 하트 (하우스 안이나 다른 일 중이면 말과 하트만)
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

    /// 졸린 시간엔 훨씬 자주 잠듦 (떠 있는 나는 동물도 그 자리에서 꾸벅)
    pub(super) fn sleep_chance(&self, base: f64) -> f64 {
        if self.presence.sleepy {
            base.max(SLEEPY_SLEEP_CHANCE)
        } else {
            base
        }
    }

    /// 이번 잠의 길이 (졸린 시간엔 훨씬 긺)
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

/// 목적지 디딤판의 기준점 (공중이면 None)
fn trip_anchor(end: TripEnd, world: &World) -> Option<(f64, f64)> {
    match end {
        TripEnd::On { owner, .. } => world.anchor(owner),
        TripEnd::Float(..) => None,
    }
}

/// 목적지 자리 (디딤판이 사라졌으면 None)
fn trip_target(end: TripEnd, world: &World) -> Option<(f64, f64)> {
    match end {
        TripEnd::On { owner, rel_x } => world.anchor(owner).map(|(ax, ay)| (ax + rel_x, ay)),
        TripEnd::Float(x, y) => Some((x, y)),
    }
}
