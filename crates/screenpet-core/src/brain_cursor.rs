//! 커서 장난 · 창 흔들림 · 창 닫힘 (v0.3.7) — brain.rs의 자식 모듈 (비공개 필드에 접근).
//!
//! 커서 장난: 커서가 근처에 가만히(또는 천천히) 있으면 동물이 장난기가 돈다 — 종마다 다르게 다가가서(CursorStyle),
//! 닿는 높이면 폴짝 뛰어(CursorLeap) 커서 끝에 매달린다(CursorHang). 최대 5초, 마우스를 세게 흔들면 일찍 떨어진다.
//! 동물별 쿨다운 2~4분. 앱이 매 틱 cursor_input으로 버튼·바쁨·설정을 알려 주어야 시작한다
//! (누군가 끌고 있을 때·조작 모드·먹이·하우스·떠나는 중엔 하지 않는다).
//!
//! 매달린 동안 클릭은 언제나 사용자 것이다: 몸은 커서 끝 바로 아래에서 머리로 커서를 문다(hang_spot — 창이 커서를 덮는다).
//! 그래서 뛰는 동안부터 떨어져 커서에서 벗어나거나 내려설 때까지 cursor_through가 참이고, 앱이 창을 옮기기 전에
//! 먼저 마우스가 통과하게 한다 (slots.rs place). 버튼을 눌러도 매달린 채로 둔다.
//!
//! 창 흔들림: 서 있는 창이 0.8초 안에 세 번 넘게 왔다 갔다 하면 "whoa!" — 60% 떨어지고, 40% 모서리에 1.5초 매달렸다가 올라오거나 떨어진다.
//! 창 닫힘: 서 있는 창이 사라지면 "eek!" 떨어지고, 내려선 뒤 "hmph!" 째려본다. 다른 창에 가려진 건 닫힘이 아니다.

use std::collections::VecDeque;

use super::*;

/// 매달려 있는 시간
pub const HANG_SECS: (f64, f64) = (3.5, 5.0);
/// 장난 뒤 다시 장난치기까지
pub const COOLDOWN_SECS: (f64, f64) = (120.0, 240.0);
/// 앱을 켜고 처음 장난치기까지
const FIRST_READY_SECS: f64 = 30.0;
/// 커서가 이만큼 가만히(느리게) 있어야 눈에 들어온다
const STILL_SECS: f64 = 1.2;
/// 이보다 느리면 "가만히" (논리 px/s)
const SLOW_SPEED: f64 = 240.0;
/// 눈에 들어온 뒤 장난을 시작할 빈도 (초당 확률)
const PLAY_RATE: f64 = 0.35;
/// 새침한 동물은 이만큼만
const ALOOF_RATE_MULT: f64 = 0.6;
/// 근처: 옆으로 몸 이만큼 안, 머리 위로 몸 이만큼 안
const NEAR_BODIES: f64 = 5.0;
const NEAR_RISE: f64 = 4.0;
/// 다가가는 동안 커서가 근처의 이 배수 밖으로 가면 그만둔다
const FAR_MULT: f64 = 1.6;
/// 장난 한 번의 제한 시간 (닿지 못하면 올려다보고 그만둔다)
const PLAY_DEADLINE_SECS: f64 = 9.0;
const GIVE_UP_LOOK_SECS: f64 = 1.2;
/// 뛰는 동안 커서가 몸 이만큼 넘게 달아나면 헛손질
const MISS_BODIES: f64 = 2.0;
/// 뛰어오르는 호의 높이 (몸 배)
const LEAP_ARC: f64 = 0.35;
/// 펫 창의 최소 너비 (논리 px — slots::PET_LOGICAL_SIZE, 창 너비 = 128 × max(1, 몸 배율))
const WINDOW_MIN_WIDTH: f64 = 128.0;
/// 놓은 뒤 이만큼(초)까지만 마우스를 통과시킨다 — 나는 동물은 커서 아래에 떠서 멈추기도 하니까
const LET_GO_THROUGH_SECS: f64 = 1.0;
/// 창 위쪽 말풍선 자리 (논리 px — slots::BUBBLE_SPACE)
const WINDOW_HEADROOM: f64 = 36.0;
/// 떨어질 때 커서 속도를 이어 받는 몫
const LET_GO_VELOCITY: f64 = 0.25;
const LET_GO_MAX_SPEED: f64 = 500.0;
/// 마우스 흔들기: 이 폭(논리 px) 넘게 SHAKE_WINDOW초 안에 SHAKE_REVERSALS번 뒤집기
const SHAKE_SWING: f64 = 60.0;
const SHAKE_WINDOW: f64 = 1.0;
const SHAKE_REVERSALS: usize = 3;
/// 창 흔들림: 이 폭(논리 px) 넘게 QUAKE_WINDOW초 안에 QUAKE_REVERSALS번 뒤집기
const QUAKE_SWING: f64 = 8.0;
const QUAKE_WINDOW: f64 = 0.8;
const QUAKE_REVERSALS: usize = 3;
/// 흔들려 떨어질 몫 (나머지는 모서리에 매달린다)
const QUAKE_FALL_CHANCE: f64 = 0.6;
/// 모서리에 매달려 있는 시간
pub const QUAKE_CLING_SECS: f64 = 1.5;
/// 모서리에 매달렸을 때 발이 창 윗면보다 이만큼(몸 배) 아래 (앞발만 걸친다)
const CLING_DROP: f64 = 0.72;
const CLIMB_BACK_CHANCE: f64 = 0.5;
/// 떨어지며 옆으로 밀리는 속도 (크기 128 기준 px/s)
const QUAKE_TUMBLE_VX: f64 = 70.0;
/// 윗면 바로 아래에서 떨어지기 시작한다 — 그래야 그 창 앞을 지나 아래로 내려간다 (px)
const BELOW_TOP: f64 = 2.0;
/// 한 번 흔들린 뒤 다시 흔들리기까지
const QUAKE_CALM_SECS: f64 = 5.0;
/// 째려보는 시간
pub const GLARE_SECS: f64 = 1.5;
/// 수다쟁이(오리)가 가는 길에 또 말하는 간격
const CHATTY_SECS: f64 = 1.3;

/// 왔다 갔다 재기: 축마다 swing 넘게 움직인 뒤 반대로 swing 넘게 돌아오면 뒤집힘 한 번
#[derive(Debug, Clone, Default)]
struct Axis {
    /// 지금 방향으로 가장 멀리 간 자리
    extreme: Option<f64>,
    dir: f64,
    flips: VecDeque<f64>,
}

impl Axis {
    fn push(&mut self, t: f64, v: f64, swing: f64, window: f64) {
        let Some(extreme) = self.extreme else {
            self.extreme = Some(v);
            return;
        };
        let d = v - extreme;
        if self.dir == 0.0 {
            if d.abs() >= swing {
                self.dir = d.signum();
                self.extreme = Some(v);
            }
        } else if d * self.dir > 0.0 {
            self.extreme = Some(v);
        } else if -d * self.dir >= swing {
            self.dir = -self.dir;
            self.extreme = Some(v);
            self.flips.push_back(t);
        }
        while self.flips.front().is_some_and(|&t0| t - t0 > window) {
            self.flips.pop_front();
        }
    }
}

/// 최근 window초 안에 방향이 몇 번 뒤집혔나 (가로·세로 중 많은 쪽)
#[derive(Debug, Clone)]
pub struct ShakeMeter {
    x: Axis,
    y: Axis,
    swing: f64,
    window: f64,
}

impl ShakeMeter {
    pub fn new(swing: f64, window: f64) -> Self {
        Self { x: Axis::default(), y: Axis::default(), swing, window }
    }

    pub fn push(&mut self, t: f64, x: f64, y: f64) {
        self.x.push(t, x, self.swing, self.window);
        self.y.push(t, y, self.swing, self.window);
    }

    pub fn reversals(&self) -> usize {
        self.x.flips.len().max(self.y.flips.len())
    }

    pub fn clear(&mut self) {
        self.x = Axis::default();
        self.y = Axis::default();
    }
}

/// 장난의 국면
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlayPhase {
    /// 시작 자세 (공룡 크앙, 유령 부~)
    Prep,
    /// 커서 쪽으로 간다 (걷는 동물은 디딤판 위로만, 나는 동물은 곧장)
    Approach,
    /// 닿을 자리에서 한 번 더 (고양이 엉덩이 씰룩, 페르시안 기지개, 기린 목 늘이기)
    Reach,
}

/// 종마다 다른 놀이 — 그림은 이미 있는 것만 쓴다
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CursorStyle {
    /// 시작 자세 (그림, 초)
    prep: Option<(Anim, f64)>,
    /// 다가갈 때 그림
    gait: Anim,
    /// 다가가는 속도 (걷기 속도 배)
    speed: f64,
    /// 통통 튀는 높이 (몸 배 — 그림만 띄운다)
    lift: f64,
    /// 한 번 튀는 시간
    beat: f64,
    /// 닿을 자리에서 한 번 더 (그림, 초)
    arrive: Option<(Anim, f64)>,
    /// 그동안 몸을 씰룩인다
    wiggle: bool,
    /// 뛰어오를 때 그림과 시간
    leap: Anim,
    leap_secs: f64,
    /// 옆으로 몸 이만큼 안이면 뛸 수 있다
    range: f64,
    /// 머리 위로 몸 이만큼 안이면 닿는다
    reach: f64,
    /// 나는 동물의 흔들림 (폭: 몸 배, 초당 횟수)
    sway: (f64, f64),
    /// 가는 길에 또 말한다
    chatty: bool,
    /// 째려볼 때 그림
    glare: Anim,
}

const BASE_STYLE: CursorStyle = CursorStyle {
    prep: None,
    gait: Anim::Walk,
    speed: 1.0,
    lift: 0.0,
    beat: 0.4,
    arrive: None,
    wiggle: false,
    leap: Anim::Hop,
    leap_secs: 0.4,
    range: 1.0,
    reach: 2.0,
    sway: (0.2, 1.5),
    chatty: false,
    glare: Anim::Annoyed,
};

impl CursorStyle {
    /// 번들 동물의 놀이 (모르는 동물은 None — 몸놀림에 맞는 기본 놀이)
    pub fn of(species: &str) -> Option<Self> {
        let b = BASE_STYLE;
        let dino = CursorStyle { speed: 1.05, lift: 0.05, beat: 0.36, leap_secs: 0.5, ..b };
        Some(match species {
            // 살금살금 다가가 엉덩이를 씰룩이다가 먼 곳에서 와락
            "cat" => CursorStyle { speed: 0.6, arrive: Some((Anim::Look, 0.9)), wiggle: true, range: 3.5, leap_secs: 0.5, glare: Anim::Huff, ..b },
            // 페르시안: 느릿느릿 어슬렁 가서 기지개 한 번 — 멀리서 와락은 귀찮다
            "persian" => CursorStyle { speed: 0.4, arrive: Some((Anim::Stretch, 1.0)), range: 1.5, leap_secs: 0.6, glare: Anim::Huff, ..b },
            "rabbit" => CursorStyle { gait: Anim::Hop, speed: 2.2, lift: 0.18, beat: 0.4, range: 1.5, ..b },
            // 다람쥐는 호기심 많고 날쌔다 — 짧게 깡충깡충 쪼르르 와서 고개를 갸웃
            "squirrel" => CursorStyle { gait: Anim::Hop, speed: 2.6, lift: 0.12, beat: 0.28, arrive: Some((Anim::Tilt, 0.6)), range: 2.0, chatty: true, ..b },
            // 멀리서도 크게 한 번에
            "frog" => CursorStyle { gait: Anim::Hop, speed: 1.8, lift: 0.3, beat: 0.55, leap: Anim::BigJump, range: 2.5, reach: 2.5, ..b },
            // 묵직하게 통, 통
            "pumpkin" => CursorStyle { gait: Anim::Hop, speed: 1.2, lift: 0.12, beat: 0.5, leap_secs: 0.5, ..b },
            "rudolph" => CursorStyle { gait: Anim::Hop, speed: 1.8, lift: 0.24, beat: 0.45, range: 1.5, ..b },
            // 뒤뚱뒤뚱, 꽥꽥
            "duck" => CursorStyle { speed: 1.1, lift: 0.04, beat: 0.3, chatty: true, ..b },
            // 천천히 가서 목을 쭉 — 더 높이 닿는다
            "giraffe" => CursorStyle { speed: 0.9, arrive: Some((Anim::NeckStretch, behavior::NECK_STRETCH_SECS)), reach: 3.0, leap_secs: 0.5, ..b },
            // 썰매 타고 스르륵
            "santa" => CursorStyle { prep: Some((Anim::Wave, 0.5)), gait: Anim::Roll, speed: 2.0, ..b },
            // 눈덩이가 통통 — 가서 나뭇가지 팔을 흔든다
            "snowman" => CursorStyle { gait: Anim::Hop, speed: 1.5, lift: 0.14, beat: 0.42, arrive: Some((Anim::Wave, 0.6)), ..b },
            // 공룡 셋: 크앙 / 쿵쾅 / 화르륵 한 뒤 쿵쿵 걸어간다
            "dino_yellow" => CursorStyle { prep: Some((Anim::Roar, 0.8)), ..dino },
            "dino_green" => CursorStyle { prep: Some((Anim::Stomp, 0.9)), ..dino },
            "dino_purple" => CursorStyle { prep: Some((Anim::Fire, 1.0)), ..dino },
            // 유령은 부~ 한 뒤 느릿느릿 둥실
            "ghost" => CursorStyle { prep: Some((Anim::Tickle, 0.6)), speed: 3.0, sway: (0.15, 0.8), ..b },
            // 새는 쌩 — 가장 빠르게 날아와 짹짹 (파닥이는 폭은 작고 빠르게)
            "bird" => CursorStyle { speed: 4.5, sway: (0.2, 4.0), chatty: true, ..b },
            // 펭귄은 배로 쭉 미끄러져 와서 지느러미를 파닥파닥
            "penguin" => CursorStyle { gait: Anim::Roll, speed: 1.6, arrive: Some((Anim::Wave, 0.6)), chatty: true, ..b },
            // 레서판다는 총총 달려와서는 커서에 깜짝 놀라 두 팔을 번쩍
            "redpanda" => CursorStyle { gait: Anim::Run, speed: 2.2, arrive: Some((Anim::Surprise, 0.9)), chatty: true, ..b },
            _ => return None,
        })
    }

    /// 몸놀림에 맞는 기본 놀이 (번들에 없는 동물)
    fn fallback(cfg: &PetConfig) -> Self {
        if cfg.flies {
            CursorStyle { speed: 3.0, ..BASE_STYLE }
        } else if cfg.hops {
            CursorStyle { gait: Anim::Hop, speed: 1.5, lift: 0.2, ..BASE_STYLE }
        } else {
            BASE_STYLE
        }
    }

    /// 이 놀이가 쓰는 그림 (테스트: 그 동물 pet.json에 있어야 한다)
    pub fn anims(&self) -> Vec<Anim> {
        let mut v = vec![self.gait, self.leap, self.glare];
        v.extend(self.prep.map(|p| p.0));
        v.extend(self.arrive.map(|a| a.0));
        v
    }
}

/// 커서 장난·창 흔들림의 기억
#[derive(Debug, Clone)]
pub struct CursorMood {
    /// 이 동물의 놀이 (None = 몸놀림에 맞는 기본)
    style: Option<CursorStyle>,
    /// 앱이 켜 두었음 (메뉴 "🐾 커서 장난")
    enabled: bool,
    /// 지금 시작하면 안 됨 (음식을 들고 있음·누가 잡혀 있음·편지·숨김). 앱이 알려 주기 전엔 참
    busy: bool,
    left_down: bool,
    last_cursor: Option<(f64, f64)>,
    /// 커서 속도 (물리 px/s)
    velocity: (f64, f64),
    still_for: f64,
    /// 이 시각(clock)부터 장난칠 수 있다
    pub(super) ready_at: f64,
    /// 이 시각(clock)까지 닿지 못하면 그만둔다
    deadline: f64,
    /// 뛰기 시작했을 때 커서 자리
    leap_aim: (f64, f64),
    /// 놓은 뒤 아직 마우스를 통과시키는 남은 시간
    through_after: f64,
    chatty_in: f64,
    shake: ShakeMeter,
    quake: ShakeMeter,
    quake_window: Option<u64>,
    calm_until: f64,
    /// 내려서면 째려본다
    glare_pending: bool,
}

impl CursorMood {
    pub(super) fn new() -> Self {
        Self {
            style: None,
            enabled: false,
            busy: true,
            left_down: false,
            last_cursor: None,
            velocity: (0.0, 0.0),
            still_for: 0.0,
            ready_at: FIRST_READY_SECS,
            deadline: 0.0,
            leap_aim: (0.0, 0.0),
            through_after: 0.0,
            chatty_in: 0.0,
            shake: ShakeMeter::new(SHAKE_SWING, SHAKE_WINDOW),
            quake: ShakeMeter::new(QUAKE_SWING, QUAKE_WINDOW),
            quake_window: None,
            calm_until: 0.0,
            glare_pending: false,
        }
    }
}

impl Pet {
    /// 이 동물의 놀이를 정한다 (동물 교체·손님이 올 때)
    pub fn set_cursor_species(&mut self, species: &str) {
        self.cursor_mood.style = CursorStyle::of(species);
    }

    /// 매 틱 update 전에 (앱): 왼쪽 버튼, 지금 시작하면 안 되는지, 메뉴 설정
    pub fn cursor_input(&mut self, left_down: bool, busy: bool, enabled: bool) {
        let m = &mut self.cursor_mood;
        m.left_down = left_down;
        m.busy = busy;
        m.enabled = enabled;
    }

    /// 창이 마우스를 통과시켜야 한다 — 뛰어오르는 중과 매달린 동안 (클릭은 사용자 것)
    pub fn cursor_through(&self) -> bool {
        // 나는 동물은 다가오는 동안 이미 커서 쪽으로 곧장 날아 창이 커서를 덮는다
        let gliding = self.cfg.flies && matches!(self.state, State::CursorPlay { phase: PlayPhase::Approach, .. });
        gliding || self.cursor_mood.through_after > 0.0 || matches!(self.state, State::CursorLeap { .. } | State::CursorHang { .. })
    }

    /// 1 = 커서에 매달림, 0 = 아님 — 웹뷰가 커서 끝(머리 꼭대기)을 축으로 흔든다
    pub(super) fn hang_side(&self) -> i8 {
        i8::from(matches!(self.state, State::CursorHang { .. }))
    }

    /// 창이 커서 끝을 덮고 있다
    pub(super) fn window_covers_cursor(&self) -> bool {
        let (cx, cy) = self.cursor;
        let dpi = self.cfg.dpi;
        let half = self.cfg.size.max(WINDOW_MIN_WIDTH * dpi) / 2.0;
        (self.body.x - half..self.body.x + half).contains(&cx) && (self.body.y - self.cfg.size - WINDOW_HEADROOM * dpi..self.body.y).contains(&cy)
    }

    /// 놓은 뒤: 떨어져 커서에서 벗어나거나 다른 상태(내려섬·잡힘)가 되면 다시 클릭을 받는다
    fn settle_let_go_through(&mut self, dt: f64) {
        if self.cursor_mood.through_after <= 0.0 {
            return;
        }
        let falling = matches!(self.state, State::Air { .. }) && self.window_covers_cursor();
        self.cursor_mood.through_after = if falling { (self.cursor_mood.through_after - dt).max(0.0) } else { 0.0 };
    }

    fn cursor_style(&self) -> CursorStyle {
        self.cursor_mood.style.unwrap_or_else(|| CursorStyle::fallback(&self.cfg))
    }

    /// 이 상태들의 그림
    pub(super) fn cursor_anim(&self) -> Anim {
        let style = self.cursor_style();
        match self.state {
            State::CursorPlay { phase: PlayPhase::Prep, .. } => style.prep.map_or(Anim::Idle, |p| p.0),
            State::CursorPlay { phase: PlayPhase::Reach, .. } => style.arrive.map_or(Anim::Idle, |a| a.0),
            State::CursorPlay { phase: PlayPhase::Approach, .. } => style.gait,
            State::CursorLeap { .. } => style.leap,
            State::Glare { .. } => style.glare,
            _ => Anim::Dragged,
        }
    }

    /// 다가가며 통통 튀는 높이 (그림만)
    pub(super) fn cursor_lift(&self) -> f64 {
        let style = self.cursor_style();
        if !matches!(self.state, State::CursorPlay { phase: PlayPhase::Approach, .. }) || style.lift <= 0.0 || self.cfg.flies {
            return 0.0;
        }
        style.lift * self.cfg.size * (std::f64::consts::PI * self.clock / style.beat).sin().abs()
    }

    /// 한 틱 (update에서, 평소 동작 전에). 이번 틱을 여기서 처리했으면 true
    pub(super) fn update_cursor(&mut self, dt: f64, world: &World) -> bool {
        self.track_cursor(dt);
        self.settle_let_go_through(dt);
        self.sense_window_closed(world);
        self.sense_quake(world);
        if self.control.is_some() {
            self.end_mood_for_control();
        }
        if self.cursor_mood.glare_pending && matches!(self.state, State::Land { .. }) {
            self.start_glare();
        }
        match self.state {
            State::CursorPlay { phase, remaining } => self.update_play(dt, world, phase, remaining),
            State::CursorLeap { from, elapsed } => self.update_leap_to_cursor(dt, world, from, elapsed),
            State::CursorHang { remaining } => self.update_hang(dt, world, remaining),
            State::EdgeCling { window, rel_x, remaining } => self.update_cling(dt, world, window, rel_x, remaining),
            State::Glare { remaining } => self.update_glare(dt, world, remaining),
            _ => {
                self.maybe_start_play(dt);
                return matches!(self.state, State::CursorPlay { .. });
            }
        }
        true
    }

    fn track_cursor(&mut self, dt: f64) {
        let cursor = self.cursor;
        let m = &mut self.cursor_mood;
        if let (Some(last), true) = (m.last_cursor, dt > 0.0) {
            m.velocity = ((cursor.0 - last.0) / dt, (cursor.1 - last.1) / dt);
        }
        m.last_cursor = Some(cursor);
        let speed = m.velocity.0.hypot(m.velocity.1) / self.cfg.dpi.max(0.01);
        m.still_for = if speed <= SLOW_SPEED { m.still_for + dt } else { 0.0 };
    }

    /// 머리 위로 잰 커서 높이 (발 기준, 위가 +)
    fn cursor_rise(&self) -> f64 {
        self.body.y - self.cursor.1
    }

    fn cursor_near(&self, mult: f64) -> bool {
        let size = self.cfg.size;
        let dx = (self.cursor.0 - self.body.x).abs();
        if self.cfg.flies {
            return dx.hypot(self.cursor_rise()) <= NEAR_BODIES * mult * size;
        }
        let rise = self.cursor_rise();
        dx <= NEAR_BODIES * mult * size && rise >= 0.0 && rise <= (HEAD_TOP + NEAR_RISE) * mult * size
    }

    /// 닿는 자리: 옆으로 range 안, 발 위 ~ 머리 위 reach 안
    fn cursor_in_reach(&self, style: &CursorStyle) -> bool {
        let size = self.cfg.size;
        let rise = self.cursor_rise();
        (self.cursor.0 - self.body.x).abs() <= style.range * size && rise >= 0.0 && rise <= (HEAD_TOP + style.reach) * size
    }

    /// 장난을 그만두어야 하는 일 (먹이·집·조작 등이 먼저다)
    fn play_interrupted(&self) -> bool {
        self.housed
            || self.home_trip.is_some()
            || self.return_home.is_some()
            || self.food.is_some()
            || self.meal_pending.is_some()
            || self.hand.is_some()
            || self.farewell_pending
            || self.inside.is_some()
            || self.is_heading_out()
            || self.send.is_some()
            || self.control.is_some()
            || self.presence_quiet()
    }

    fn maybe_start_play(&mut self, dt: f64) {
        let m = &self.cursor_mood;
        if !m.enabled || m.busy || m.left_down || self.clock < m.ready_at || m.still_for < STILL_SECS {
            return;
        }
        let free = matches!(self.state, State::Move { .. } | State::Idle { .. } | State::Turn { .. })
            && (self.standing.is_some() || self.cfg.flies)
            && self.hover_since.is_none()
            && !self.thrown
            && !self.tickling
            && !self.play_interrupted();
        if !free || !self.cursor_near(1.0) {
            return;
        }
        let rate = if self.cfg.personality == Personality::Aloof { PLAY_RATE * ALOOF_RATE_MULT } else { PLAY_RATE };
        if self.rng.f64() >= rate * dt {
            return;
        }
        self.start_play();
    }

    fn start_play(&mut self) {
        let style = self.cursor_style();
        let cooldown = behavior::range(&mut self.rng, COOLDOWN_SECS);
        let m = &mut self.cursor_mood;
        m.ready_at = self.clock + cooldown;
        m.deadline = self.clock + PLAY_DEADLINE_SECS;
        m.chatty_in = CHATTY_SECS;
        self.pacing.after_beat = None;
        self.flight = None;
        self.hover_at = None;
        if self.cfg.flies {
            self.standing = None;
        }
        self.body.vx = 0.0;
        self.body.vy = 0.0;
        self.speech = Some(Speech::CursorPlay);
        self.face_cursor();
        self.state = match style.prep {
            Some((_, secs)) => State::CursorPlay { phase: PlayPhase::Prep, remaining: secs },
            None => State::CursorPlay { phase: PlayPhase::Approach, remaining: 0.0 },
        };
    }

    fn face_cursor(&mut self) {
        let dx = self.cursor.0 - self.body.x;
        if dx.abs() > 2.0 {
            self.facing = dx.signum();
        }
    }

    /// 그만둔다: 제자리에서 쉬기로 (나는 동물은 뜬 채로)
    fn end_play(&mut self) {
        self.state = self.idle_state();
    }

    /// 닿지 못했다 — 커서를 올려다보고 그만둔다
    fn give_up_play(&mut self) {
        self.face_cursor();
        self.gaze_up = true;
        self.state = State::Idle { remaining: GIVE_UP_LOOK_SECS, look: true };
    }

    fn update_play(&mut self, dt: f64, world: &World, phase: PlayPhase, remaining: f64) {
        let m = &self.cursor_mood;
        if m.busy || m.left_down || !m.enabled || self.play_interrupted() {
            self.end_play();
            return;
        }
        let style = self.cursor_style();
        // 디딤판이 사라졌으면 평소처럼 떨어진다
        if !self.cfg.flies && !self.follow_anchor(world) {
            self.start_fall();
            return;
        }
        if !self.cursor_near(FAR_MULT) {
            self.end_play();
            return;
        }
        match phase {
            PlayPhase::Prep | PlayPhase::Reach if remaining > dt => {
                self.state = State::CursorPlay { phase, remaining: remaining - dt };
                if phase == PlayPhase::Reach && style.wiggle {
                    self.wobble_timer = self.wobble_timer.max(2.0 * dt);
                }
                self.face_cursor();
            }
            PlayPhase::Prep => self.state = State::CursorPlay { phase: PlayPhase::Approach, remaining: 0.0 },
            PlayPhase::Reach if self.cursor_in_reach(&style) => self.start_cursor_leap(),
            PlayPhase::Reach => self.state = State::CursorPlay { phase: PlayPhase::Approach, remaining: 0.0 },
            PlayPhase::Approach => self.update_approach(dt, world, &style),
        }
    }

    fn update_approach(&mut self, dt: f64, world: &World, style: &CursorStyle) {
        if self.clock >= self.cursor_mood.deadline {
            self.give_up_play();
            return;
        }
        if style.chatty {
            self.cursor_mood.chatty_in -= dt;
            if self.cursor_mood.chatty_in <= 0.0 {
                self.cursor_mood.chatty_in = CHATTY_SECS;
                self.speech = Some(Speech::CursorPlay);
            }
        }
        self.face_cursor();
        let speed = self.cfg.walk_speed * style.speed;
        if self.cfg.flies {
            self.fly_to_cursor(dt, world, style, speed);
            return;
        }
        if self.cursor_in_reach(style) {
            match style.arrive {
                Some((_, secs)) => self.state = State::CursorPlay { phase: PlayPhase::Reach, remaining: secs },
                None => self.start_cursor_leap(),
            }
            return;
        }
        let st = self.standing.expect("follow_anchor keeps standing");
        let Some(piece) = world.piece_at(st.owner, self.body.x).copied() else {
            self.start_fall();
            return;
        };
        // 디딤판 끝에서 멈춘다 (커서를 쫓다 일부러 떨어지지 않는다) — 이미 끝을 넘어 서 있으면 더 나가지 않는다
        let x = self.body.x;
        let (lo, hi) = ((piece.x_start + self.cfg.half_w).min(x), (piece.x_end - self.cfg.half_w).max(x));
        let new_x = crate::physics::step_toward(x, self.cursor.0, speed * dt).clamp(lo, hi);
        self.standing = Some(Standing { owner: st.owner, rel_x: st.rel_x + (new_x - x) });
        self.body.x = new_x;
    }

    /// 나는 동물: 매달릴 자리로 곧장 (살랑살랑)
    fn fly_to_cursor(&mut self, dt: f64, world: &World, style: &CursorStyle, speed: f64) {
        let (ax, ay) = self.hang_spot(world);
        let (dx, dy) = (ax - self.body.x, ay - self.body.y);
        let dist = dx.hypot(dy);
        let step = speed * dt;
        if dist <= step.max(0.1 * self.cfg.size) {
            self.start_hang(world);
            return;
        }
        let (amp, hz) = style.sway;
        let tau = std::f64::consts::TAU;
        let sway = amp * self.cfg.size * tau * hz * (tau * hz * self.clock).cos() * dt;
        self.body.x += dx / dist * step;
        self.body.y += dy / dist * step + sway;
    }

    fn start_cursor_leap(&mut self) {
        self.cursor_mood.leap_aim = self.cursor;
        self.standing = None;
        self.body.vx = 0.0;
        self.body.vy = 0.0;
        self.state = State::CursorLeap { from: (self.body.x, self.body.y), elapsed: 0.0 };
    }

    fn update_leap_to_cursor(&mut self, dt: f64, world: &World, from: (f64, f64), elapsed: f64) {
        let elapsed = elapsed + dt;
        let t = (elapsed / self.cursor_style().leap_secs).min(1.0);
        let (ax, ay) = self.hang_spot(world);
        // 뜨면서 빠르게, 닿을 때 천천히 + 위로 볼록
        let ease = 1.0 - (1.0 - t) * (1.0 - t);
        let arc = LEAP_ARC * self.cfg.size * (std::f64::consts::PI * t).sin();
        self.body.x = from.0 + (ax - from.0) * ease;
        self.body.y = from.1 + (ay - from.1) * ease - arc;
        self.state = State::CursorLeap { from, elapsed };
        if t < 1.0 {
            return;
        }
        let aim = self.cursor_mood.leap_aim;
        if (self.cursor.0 - aim.0).hypot(self.cursor.1 - aim.1) > MISS_BODIES * self.cfg.size {
            // 헛손질!
            self.speech = Some(Speech::Ignore);
            self.state = State::Air { anim: Anim::Fall };
        } else {
            self.start_hang(world);
        }
    }

    /// 매달릴 자리 (몸 가운데 x, 발 y): 머리 꼭대기(hang_grip)가 커서 끝에 닿게 커서 바로 아래 —
    /// 화면 끝에서는 잘리지 않게 안쪽으로 민다
    fn hang_spot(&self, world: &World) -> (f64, f64) {
        let (cx, cy) = self.cursor;
        let half = self.cfg.size.max(WINDOW_MIN_WIDTH * self.cfg.dpi) / 2.0;
        let (grip_x, grip_y) = self.cfg.hang_grip;
        let b = world.bounds;
        let x = cx - self.cfg.size * grip_x;
        let x = if b.right - b.left >= half * 2.0 { x.clamp(b.left + half, b.right - half) } else { x };
        (x, (cy + self.cfg.size * (1.0 - grip_y)).min(b.bottom))
    }

    fn start_hang(&mut self, world: &World) {
        let (x, y) = self.hang_spot(world);
        self.body = Body { x, y, vx: 0.0, vy: 0.0 };
        self.standing = None;
        self.cursor_mood.shake.clear();
        self.speech = Some(Speech::Gotcha);
        self.state = State::CursorHang { remaining: behavior::range(&mut self.rng, HANG_SECS) };
    }

    fn update_hang(&mut self, dt: f64, world: &World, remaining: f64) {
        if !self.cursor_mood.enabled || self.play_interrupted() {
            self.let_go(None);
            return;
        }
        let (x, y) = self.hang_spot(world);
        self.body.x = x;
        self.body.y = y;
        let (cx, cy) = self.cursor;
        let dpi = self.cfg.dpi.max(0.01);
        let clock = self.clock;
        let m = &mut self.cursor_mood;
        m.shake.push(clock, cx / dpi, cy / dpi);
        if m.shake.reversals() >= SHAKE_REVERSALS {
            self.wobble_timer = behavior_wobble();
            self.let_go(Some(Speech::Whoa));
            return;
        }
        let remaining = remaining - dt;
        if remaining > 0.0 {
            self.state = State::CursorHang { remaining };
        } else {
            self.let_go(Some(Speech::Whee));
        }
    }

    /// 놓고 떨어진다 (평소 물리 — 나는 동물은 떠서 멈춘다). 커서가 움직이던 쪽으로 조금 밀린다
    fn let_go(&mut self, speech: Option<Speech>) {
        if speech.is_some() {
            self.speech = speech;
        }
        let max = self.scaled(LET_GO_MAX_SPEED);
        let (vx, vy) = self.cursor_mood.velocity;
        self.body.vx = (vx * LET_GO_VELOCITY).clamp(-max, max);
        self.body.vy = (vy * LET_GO_VELOCITY).clamp(-max, max).max(0.0);
        self.standing = None;
        self.thrown = false;
        self.state = State::Air { anim: Anim::Fall };
        self.cursor_mood.through_after = LET_GO_THROUGH_SECS;
    }

    // ── 창 흔들림 ──

    fn sense_quake(&mut self, world: &World) {
        let window = match self.standing.map(|s| s.owner) {
            Some(Owner::Window(id)) => Some(id),
            _ => None,
        };
        if window != self.cursor_mood.quake_window {
            self.cursor_mood.quake_window = window;
            self.cursor_mood.quake.clear();
        }
        let Some((ax, ay)) = window.and_then(|id| world.anchor(Owner::Window(id))) else { return };
        let dpi = self.cfg.dpi.max(0.01);
        let clock = self.clock;
        self.cursor_mood.quake.push(clock, ax / dpi, ay / dpi);
        if self.cursor_mood.quake.reversals() < QUAKE_REVERSALS || clock < self.cursor_mood.calm_until || !self.quake_feels() {
            return;
        }
        self.cursor_mood.quake.clear();
        self.cursor_mood.calm_until = clock + QUAKE_CALM_SECS;
        self.feel_quake(window.expect("quake needs a window"));
    }

    /// 조작 모드 중엔 째려보거나 매달리지 않는다 — 키를 빼앗지 않게 (창이 사라져 떨어지는 것만 그대로). 하던 중이었어도 바로 끝낸다
    fn end_mood_for_control(&mut self) {
        self.cursor_mood.glare_pending = false;
        match self.state {
            State::Glare { .. } => self.state = self.idle_state(),
            State::EdgeCling { .. } => self.start_fall(),
            _ => {}
        }
    }

    /// 흔들림을 느낄 수 있는 상태: 창 위에서 쉬거나 놀고 있음 — 들려 있거나 먹거나 집에 가거나 조작 중이면 그대로
    fn quake_feels(&self) -> bool {
        !self.is_held()
            && self.control.is_none()
            && self.food.is_none()
            && self.meal_pending.is_none()
            && self.home_trip.is_none()
            && matches!(
                self.state,
                State::Move { .. }
                    | State::Idle { .. }
                    | State::Turn { .. }
                    | State::Land { .. }
                    | State::Trick { .. }
                    | State::Tickled { .. }
                    | State::Hungry { .. }
                    | State::Huff { .. }
                    | State::Special { .. }
                    | State::Sleep { .. }
                    | State::Annoyed { .. }
                    | State::Greet { .. }
                    | State::Chase { .. }
                    | State::Flee { .. }
                    | State::CatchMe { .. }
                    | State::Tumble { .. }
                    | State::CursorPlay { .. }
                    | State::Glare { .. }
            )
    }

    fn feel_quake(&mut self, window: u64) {
        self.speech = Some(Speech::Whoa);
        self.wobble_timer = behavior_wobble();
        self.pacing.after_beat = None;
        self.clear_hover();
        if self.cfg.flies {
            self.take_off();
            if !matches!(self.state, State::Move { .. } | State::Idle { .. } | State::Turn { .. }) {
                self.state = self.idle_state();
            }
            return;
        }
        if self.rng.f64() < QUAKE_FALL_CHANCE {
            // 데굴데굴 떨어진다 — 윗면 바로 아래에서 시작해 그 창 앞을 지나 내려간다
            let dir = if self.rng.bool() { 1.0 } else { -1.0 };
            self.standing = None;
            self.body.y += BELOW_TOP;
            self.body.vx = dir * self.scaled(QUAKE_TUMBLE_VX);
            self.body.vy = 0.0;
            self.state = State::Air { anim: Anim::Roll };
        } else {
            let rel_x = self.standing.map_or(0.0, |s| s.rel_x);
            self.standing = None;
            self.body.vx = 0.0;
            self.body.vy = 0.0;
            self.state = State::EdgeCling { window, rel_x, remaining: QUAKE_CLING_SECS };
        }
    }

    /// 창 모서리에 매달려 창을 따라간다. 끝나면 올라오거나 떨어진다
    fn update_cling(&mut self, dt: f64, world: &World, window: u64, rel_x: f64, remaining: f64) {
        let Some(rect) = world.window(window).map(|w| w.rect) else {
            self.start_fall();
            return;
        };
        self.body.x = rect.left + rel_x;
        self.body.y = rect.top + CLING_DROP * self.cfg.size;
        let remaining = remaining - dt;
        if remaining > 0.0 {
            self.state = State::EdgeCling { window, rel_x, remaining };
            return;
        }
        let owner = Owner::Window(window);
        if self.rng.f64() < CLIMB_BACK_CHANCE && world.piece_at(owner, self.body.x).is_some() {
            self.body.y = rect.top;
            self.standing = Some(Standing { owner, rel_x });
            self.state = State::Land { remaining: LAND_SECS };
        } else {
            self.start_fall();
        }
    }

    // ── 창 닫힘 ──

    /// 서 있던 창이 사라졌다 (가려진 건 아님): "eek!" — 떨어지는 건 평소 그대로 (디딤판을 잃은 상태가 알아서)
    fn sense_window_closed(&mut self, world: &World) {
        if self.is_held() || self.home_trip.is_some() {
            self.cursor_mood.glare_pending = false;
            return;
        }
        let Some(Standing { owner: Owner::Window(id), .. }) = self.standing else { return };
        if world.window(id).is_some() {
            return;
        }
        self.speech = Some(Speech::Eek);
        if self.cfg.flies {
            // 자던 동물은 놀라 떠오르기만, 깨어 있던 동물은 그 자리에서 째려본다
            let awake = matches!(self.state, State::Move { .. } | State::Idle { .. } | State::Turn { .. });
            self.take_off();
            if awake && self.control.is_none() {
                self.start_glare();
                self.speech = Some(Speech::Eek);
            }
            return;
        }
        // 다음 틱에 다시 알아채지 않게 지금 떨어뜨린다 — 먹는 중 등 평소 처리가 있는 상태도 디딤판을 잃으면 같은 길이다
        self.cursor_mood.glare_pending = self.food.is_none() && self.meal_pending.is_none();
        self.start_fall();
    }

    /// 내려서면 째려보려던 것을 잊는다 (자리 비움으로 잠들었다)
    pub(super) fn forget_glare(&mut self) {
        self.cursor_mood.glare_pending = false;
    }

    fn start_glare(&mut self) {
        self.cursor_mood.glare_pending = false;
        self.speech = Some(Speech::Hmph);
        self.pacing.after_beat = None;
        self.state = State::Glare { remaining: GLARE_SECS };
    }

    fn update_glare(&mut self, dt: f64, world: &World, remaining: f64) {
        if self.standing.is_some() && !self.follow_anchor(world) {
            self.start_fall();
            return;
        }
        let remaining = remaining - dt;
        self.state = if remaining > 0.0 { State::Glare { remaining } } else { self.idle_state() };
    }
}

/// 놀라 덜덜 떠는 시간 (쿵쾅에 흔들릴 때와 같다)
fn behavior_wobble() -> f64 {
    super::stomp_moves::WOBBLE_SECS
}
