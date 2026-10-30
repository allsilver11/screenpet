use std::collections::VecDeque;

use serde::Serialize;

use crate::care::{FeedResult, Food};
use crate::flight::Flight;
use crate::food::{self, Prop, PropEvent};
use crate::geom::Rect;
use crate::sting::{ClickMeter, PathMeter, StingPhase};
use crate::inside::{self, Mode, Stay};
use crate::behavior::{self, ClickOutcome, ClickReaction, Family, Move, Personality, Rest, Special, Trick};
use crate::physics::{step_air, throw_velocity, Body, GRAVITY, THROW_GRAVITY_LOGICAL, THROW_LIFT};
use crate::surfaces::{EndKind, Owner, Surface};
use crate::visit::{self, ExitEdges, Guest};
use crate::world::{Side, World};

const TURN_PAUSE: f64 = 0.3;
const LAND_SECS: f64 = 0.25;
/// 크기 128 기준 (scaled로 크기에 맞춘다)
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
const VELOCITY_WINDOW: f64 = 0.12;
/// 처음·마지막 구간 속도가 나머지 구간 중앙값의 이 배수를 넘으면 튄 샘플로 보고 뺀다
const OUTLIER_RATIO: f64 = 3.0;
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
/// 집으로 가는 길에 진척 없는 시간이 이만큼(초) 쌓이면 문 앞으로 한 번에 뛰어든다
/// (들려 있는 동안, 하우스 바닥에서 문으로 달리는 동안은 세지 않는다)
const HOME_STALL_LIMIT_SECS: f64 = 12.0;
/// 문까지 x 거리가 지금까지의 최고 기록보다 이만큼(px) 줄어야 진척으로 본다
const HOME_PROGRESS_PX: f64 = 20.0;
/// 마지막 진척 뒤 이 시간(초)이 지나야 진척 없는 시간을 세기 시작한다
const HOME_PROGRESS_WINDOW_SECS: f64 = 4.0;
/// 안전 점프가 출발·도착 중 높은 쪽보다 더 솟는 높이 (크기 128 기준 px)
const HOME_LEAP_RISE: f64 = 200.0;

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
    /// 꿀벌 쏘기 — "!" 하고 화면 쪽으로 돌진 (앞모습)
    Sting,
    /// 개구리 큰 점프 — 깊게 웅크렸다가 쭉 뻗어 날아오른다 (옆모습, brain_hop.rs)
    #[serde(rename = "bigjump")]
    BigJump,
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
    /// 꿀벌이 화면을 쏨 ("콕!")
    Sting,
    /// 배고플 때 손에 든 음식을 봤다 ("냠!?")
    Eager,
    /// 다른 동물에게 인사 ("hi!", brain_social.rs)
    Greet,
    /// 술래가 놀자고 쫓아간다 ("play!")
    Play,
    /// 쫓기며 도망 ("catch me!")
    Flee,
    /// 손님이 다음 친구 화면으로 보내져 떠난다 ("bye~!", brain_send.rs)
    Bye,
}

impl Anim {
    /// 뒷모습 동작 — 좌우 반전하지 않는다.
    pub fn is_back(self) -> bool {
        matches!(self, Anim::Climb | Anim::Slide)
    }

    /// 앞모습 동작 — 좌우 반전하지 않는다.
    pub fn is_front(self) -> bool {
        matches!(self, Anim::SitFront | Anim::Wave | Anim::Tilt | Anim::Belly | Anim::Stretch | Anim::Heart | Anim::Tickle | Anim::Wag | Anim::Hungry | Anim::Eat | Anim::LevelUp | Anim::Huff | Anim::Sting)
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
    /// 하우스로 달려가는 중 (디딤판 위). 문 위치는 Pet의 home_trip
    GoHome,
    /// 집으로 가는 길이 너무 오래 걸려 문 앞으로 한 번에 뛰어드는 중 — 충돌 없이 포물선을 따라간다
    HomeLeap { remaining: f64 },
    /// 하우스 안 (물리·세계 상호작용 없음). 하우스 안에서 먹을 땐 Eating/LevelUp/Trick을 거쳐 돌아온다
    Housed,
    /// 화면 끝(side) 밖으로 걸어 나가는 중 — 최소 remaining초, 몸이 다 빠져나갈 때까지. 끝나면 take_exit가 Some(side)
    Exit { remaining: f64, side: Side },
    /// 화면 끝(side) 바깥 바닥에서 안쪽으로 걸어 들어오는 중 — 최소 remaining초, 몸이 다 들어올 때까지
    Enter { remaining: f64, side: Side },
    /// 손님이 "집에 와"로 손 흔드는 중 (제자리) — 끝나면 걸어 나가지 않고 사라진다
    Farewell { remaining: f64 },
    /// 화면을 떠남 (친구 화면에 가 있음). 아무것도 하지 않는다
    Away,
    /// 창 안쪽 벽을 기어올라 윗면으로 나가는 중 (spec 2B §9). rel_x = 몸 중심 − 창 왼쪽, rel_y = 발 높이 − 창 윗면
    InnerClimb { window: u64, rel_x: f64, rel_y: f64 },
    /// 꿀벌이 괴롭힘에 화나 화면을 쏘는 중 (spec 2B §10) — 제자리에서 "!" → 돌진 → 콕! → 흥!
    Sting { phase: StingPhase, remaining: f64 },
    /// 먹이를 받아 그릇(나는 동물은 꽃)으로 가는 중 — remaining초 안에 못 닿으면 그 자리에서 먹는다
    ToFood { remaining: f64 },
    /// 손에 든 음식(직접 주기)을 보고 기다리는 중 — 같은 바닥 가까이면 걸어가고, 나는 동물은 날아간다
    WantFood { walking: bool },
    /// 개구리 큰 점프 (brain_hop.rs): crouch초 웅크린 뒤 owner 위 rel_x로 포물선을 따라 remaining초 날아간다
    Leap { crouch: f64, remaining: f64, owner: Owner, rel_x: f64 },
    /// 다른 동물과 마주쳐 인사 (잡힌 뒤 하트도) — look초 친구 쪽을 쳐다본 뒤 anim. 끝은 social.rs가 정하고, remaining은 안전 시한 (brain_social.rs)
    Greet { look: f64, anim: Anim, remaining: f64 },
    /// 술래 놀이의 술래 — 친구(play_target)를 쫓아 달린다 (brain_social.rs)
    Chase { remaining: f64 },
    /// 술래 놀이에서 도망 — dir 쪽으로 달리다 디딤판 끝에서 돌아선다 (brain_social.rs)
    Flee { remaining: f64, dir: f64 },
    /// 손님을 클릭하면 "잡아 봐!" — 커서를 피해 달아난다. commit: 끝에서 돌아선 뒤 그쪽으로 버티는 시간, dodge > 0: 폴짝 피하는 중 (brain_guestplay.rs)
    CatchMe { remaining: f64, dir: f64, commit: f64, dodge: f64 },
}

impl State {
    /// 활동 묶음 (쉬기·자기·공중 등은 None)
    fn family(self) -> Option<Family> {
        match self {
            State::Move { kind, .. } => Some(kind.family()),
            State::Exit { .. } | State::Enter { .. } => Some(Family::Move),
            State::Trick { .. }
            | State::Huff { .. }
            | State::Tickled { .. }
            | State::Hungry { .. }
            | State::Eating { .. }
            | State::LevelUp { .. }
            | State::Sting { .. } => {
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

/// 하우스로 가는 길: 문이 있는 바닥과 문 x, 지금까지 걸린 시간
#[derive(Debug, Clone, Copy, PartialEq)]
struct HomeTrip {
    owner: Owner,
    door_x: f64,
    /// 문까지 x 거리의 최고 기록
    best: f64,
    /// 마지막 진척 뒤 흐른 시간
    since_progress: f64,
    /// 진척 없는 시간 (누적)
    stalled: f64,
}

impl HomeTrip {
    fn new(owner: Owner, door_x: f64, x: f64) -> Self {
        Self { owner, door_x, best: (door_x - x).abs(), since_progress: 0.0, stalled: 0.0 }
    }
}

/// 받은 먹이 — 그릇으로 가서 먹는다 (phase 3)
#[derive(Debug, Clone, Copy, PartialEq)]
struct FoodTrip {
    food: Food,
    /// 다 먹고 하트 (좋아하는 음식·간식·물)
    heart: bool,
    level_up: bool,
    /// 그릇 자리 (디딤판, 기준점에서의 x) — 아직 안 놓였으면 None
    target: Option<(Owner, f64)>,
    /// 하우스 안에서 받음 — 다음 update에서 문 앞으로 나온다
    from_house: bool,
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
    /// 날아다닌다 (pet.json `locomotion: fly` — 꿀벌, spec 2B §10)
    pub flies: bool,
    /// 괴롭히면 화면을 쏜다 (pet.json에 sting이 있는 꿀벌)
    pub stings: bool,
    /// 폴짝폴짝 뛰어다닌다 (pet.json `locomotion: hop` — 개구리, brain_hop.rs)
    pub hops: bool,
    /// 화면 배율 (논리 px 하나가 물리 px 몇 개인가, 기본 1.0) — 던진 비행은 논리 px로 잰다
    pub dpi: f64,
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
    /// 하우스로 가는 중이면 Some (들려 있거나 공중이어도 유지된다)
    home_trip: Option<HomeTrip>,
    /// 하우스 안에 있음
    housed: bool,
    /// 친구가 연결된 화면 끝 (phase 3)
    exit_edges: ExitEdges,
    /// 손님으로 와 있으면 Some
    guest: Option<Guest>,
    /// 트레이 "놀러 보내기"로 친구가 연결된 끝으로 가는 중 (brain_send.rs)
    send: Option<send_moves::SendTrip>,
    /// 던져서 친구 화면으로 넘어간 움직임 (take_fling으로 한 번 꺼낸다, brain_fling.rs)
    fling: Option<visit::Fling>,
    /// 들려 있거나 공중일 때 "집에 와"를 받음 — 놓여서 내려서면 손 흔든다
    farewell_pending: bool,
    /// 화면을 떠난 쪽 (take_exit로 한 번 꺼낸다)
    exited: Option<Side>,
    /// 창 안에서 노는 중이면 Some (spec 2B §9)
    inside: Option<Stay>,
    /// 방금 드래그해서 놓음 — 다음 update에서 창 안에 놓였는지 본다
    drop_check: bool,
    /// 나는 동물이 날아가는 길 (spec 2B §10)
    flight: Option<Flight>,
    /// 나는 동물이 제자리에 떠 있는 기준점 (위아래로 살랑인다)
    hover_at: Option<(f64, f64)>,
    /// 마지막 커서 위치 (떠서 쳐다볼 때 그쪽을 본다)
    cursor: (f64, f64),
    /// 떠 있을 때 먹이를 받음 — 내려앉으면 먹는다 (favorite, level_up)
    meal_pending: Option<(bool, bool)>,
    /// 나는 동물이 먹으러 내려앉을 자리와 그 자리의 소품 (디딤판, 기준점에서의 x, 소품)
    meal_spot: Option<(Owner, f64, Prop)>,
    /// 그릇으로 가는 중인 먹이 (걷는 동물, 그리고 꽃 자리를 아직 안 정한 나는 동물)
    food: Option<FoodTrip>,
    /// 앱에 보낼 소품 변화 (take_prop으로 한 번 꺼낸다)
    prop_event: Option<PropEvent>,
    /// 소품이 놓여 있음
    prop_shown: bool,
    /// 하우스 문 (디딤판, 문 x) — 하우스 안에서 먹이를 받으면 문 앞으로 나와 먹는다
    house_door: Option<(Owner, f64)>,
    /// 하우스에서 나와 먹는 중 — 다 먹으면 이 문으로 돌아간다
    return_home: Option<(Owner, f64)>,
    /// 나중에 먹을 먹이 (음식, 성장함): 공중·들린 채·벽 타는 중에 받은 간식·물, 성장 축하·하트 중에 받은 먹이
    queued_food: Option<(Food, bool)>,
    /// 손에 든 음식의 자리 (직접 주기 중이면 Some — 앱이 매 틱 offer_hand로 알려 준다)
    hand: Option<(f64, f64)>,
    /// 이번에 내민 음식을 이미 알아챘음 (배고프면 한 번 "냠!?")
    hand_noticed: bool,
    /// 디딤판에 놓인 소품 (소품, 디딤판, 기준점에서의 x, 마지막으로 보낸 자리) — 창이 움직이면 따라간다
    prop_anchor: Option<(Prop, Owner, f64, (f64, f64))>,
    /// 쏜 화면 지점 (take_sting으로 한 번 꺼낸다)
    sting_at: Option<(f64, f64)>,
    /// 이 시각(clock)부터 다시 쏠 수 있다
    sting_ready_at: f64,
    /// 들고 흔든 경로 (쏘기 판정)
    shake: PathMeter,
    /// 최근 클릭 (쏘기 판정)
    pester: ClickMeter,
    /// 폴짝 동물의 걸음·벽 오르기 박자 (brain_hop.rs)
    hop_beat: crate::hop::Beat,
    /// 손에서 던져져 날아가는 중 (State::Air) — 논리 px 기준의 부드러운 중력을 쓴다. 공중을 벗어나면 꺼진다
    thrown: bool,
    /// 다른 동물과 놀 때 구별하는 번호 (프로세스 안에서 유일, social.rs)
    social_key: u64,
    /// 인사·술래 놀이 중인 친구의 자리 (발 위치) — social.rs가 매 틱 알려 준다
    play_target: Option<(f64, f64)>,
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
            home_trip: None,
            housed: false,
            exit_edges: ExitEdges::default(),
            guest: None,
            send: None,
            fling: None,
            farewell_pending: false,
            exited: None,
            inside: None,
            drop_check: false,
            flight: None,
            hover_at: None,
            cursor: (x, y),
            meal_pending: None,
            meal_spot: None,
            food: None,
            prop_event: None,
            prop_shown: false,
            house_door: None,
            return_home: None,
            queued_food: None,
            hand: None,
            hand_noticed: false,
            prop_anchor: None,
            sting_at: None,
            sting_ready_at: 0.0,
            shake: PathMeter::default(),
            pester: ClickMeter::default(),
            hop_beat: Default::default(),
            thrown: false,
            social_key: social_moves::next_key(),
            play_target: None,
            rng: fastrand::Rng::with_seed(seed),
        }
    }

    pub fn update(&mut self, dt: f64, world: &World) {
        self.clock += dt;
        if !matches!(self.state, State::Air { anim: Anim::Fall }) {
            self.thrown = false;
        }
        self.heart_timer = (self.heart_timer - dt).max(0.0);
        self.drop_flying_state();
        self.after_meal();
        self.track_prop(world);
        self.serve_queued_food();
        self.notice_hand();
        self.tick_send(dt, world);
        if self.housed && self.food.is_some_and(|f| f.from_house) {
            self.come_out_for_food(world);
        }
        if self.housed {
            self.update_housed(dt);
            return;
        }
        match self.state {
            State::Away => return,
            State::Exit { remaining, side } => {
                if self.cfg.flies {
                    self.update_fly_exit(dt, world, remaining, side);
                } else {
                    self.update_exit(dt, world, remaining, side);
                }
                return;
            }
            State::Farewell { remaining } => {
                self.update_farewell(dt, remaining);
                return;
            }
            _ => {}
        }
        if self.farewell_pending && !self.is_held() && !matches!(self.state, State::Air { .. }) {
            self.farewell_pending = false;
            self.go_home_now();
            return;
        }
        self.tick_guest(dt);
        if let State::Sting { phase, remaining } = self.state {
            self.update_sting(dt, world, phase, remaining);
            self.on_started();
            return;
        }
        if self.update_social(dt, world) || self.update_catch_me(dt, world) {
            self.on_started();
            return;
        }
        if self.cfg.flies {
            self.update_flyer(dt, world);
            self.on_started();
            return;
        }
        if self.track_home_progress(dt) {
            self.start_home_leap(world);
        }
        self.update_inside(dt, world);
        match self.state {
            State::Housed => {}
            State::HomeLeap { remaining } => self.update_home_leap(dt, remaining, world),
            State::Leap { crouch, remaining, owner, rel_x } => self.update_leap(dt, world, crouch, remaining, owner, rel_x),
            State::Enter { remaining, side } => self.update_enter(dt, world, remaining, side),
            State::Climb { .. } | State::Slide | State::WallClimb { .. } | State::InnerClimb { .. } if self.home_trip.is_some() => {
                self.start_fall()
            }
            _ if self.home_trip.is_some() && !self.is_held() && !self.off_surface() && !self.celebrating() => {
                self.update_chatter(dt);
                self.update_go_home(dt, world);
            }
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
            State::InnerClimb { window, rel_x, rel_y } => self.update_inner_climb(dt, world, window, rel_x, rel_y),
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
            // 나는 동물의 공중제비(Roll)는 원을 도는 동안만 — 그 앞뒤는 그냥 난다
            State::Move { kind: Move::Roll, .. } if self.cfg.flies && !self.flight.is_some_and(|f| f.looping.is_some()) => Anim::Walk,
            State::Move { kind, .. } if self.cfg.hops => self.hop_anim(kind),
            State::Move { kind, .. } => kind.into(),
            State::Leap { .. } => Anim::BigJump,
            State::Greet { look, .. } if look > 0.0 => Anim::Look,
            State::Greet { anim, .. } => anim,
            State::Chase { .. } | State::Flee { .. } if self.cfg.hops => self.hop_anim(Move::Run),
            State::Chase { .. } | State::Flee { .. } => Anim::Run,
            State::CatchMe { dodge, .. } if dodge > 0.0 => Anim::Hop,
            State::CatchMe { .. } if self.cfg.hops => self.hop_anim(Move::Run),
            State::CatchMe { .. } => Anim::Run,
            State::Sting { phase: StingPhase::Recoil, .. } => Anim::Annoyed,
            State::Sting { .. } => Anim::Sting,
            State::Exit { .. } | State::Enter { .. } | State::ToFood { .. } | State::WantFood { walking: true } => Anim::Walk,
            State::WantFood { walking: false } => Anim::Look,
            State::Farewell { .. } => Trick::Wave.into(),
            State::Away => Anim::Idle,
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
            State::Climb { .. } | State::WallClimb { .. } | State::InnerClimb { .. } => Anim::Climb,
            State::Slide => Anim::Slide,
            State::GoHome => Anim::Run,
            State::HomeLeap { .. } => Anim::Hop,
            State::Housed => Anim::SitFront,
            State::Pressed { grounded: true, .. } => Anim::Idle,
            State::Pressed { .. } | State::Dragged => Anim::Dragged,
        };
        RenderState { anim, flip: self.facing < 0.0 && !anim.is_front() && !anim.is_back(), heart: self.heart_timer > 0.0 }
    }

    pub fn window_pos(&self) -> (f64, f64) {
        (self.body.x - self.cfg.size / 2.0, self.body.y - self.cfg.size - self.hop_lift())
    }

    /// 먹는 중 (먹다가 눌린 동안도 포함 — 손을 떼면 계속 먹는다)
    pub fn is_eating(&self) -> bool {
        // 나는 동물이 먹이를 받고 내려앉으러 가는 중
        if self.meal_pending.is_some() || self.food.is_some() {
            return true;
        }
        match self.state {
            State::Eating { .. } => true,
            State::Pressed { .. } => matches!(self.before_press, Some((State::Eating { .. }, _))),
            _ => false,
        }
    }

    /// 하우스 안에 있음 (가는 중은 아님). 하우스에 있는 동안엔 놀러 가지 않는다 (phase 3)
    pub fn is_housed(&self) -> bool {
        self.housed
    }

    /// 하우스로 가는 중 (아직 도착 전)
    pub fn is_going_home(&self) -> bool {
        self.home_trip.is_some()
    }

    /// 성장해서 먹는 중이거나 성장 축하 중 — 하우스로 가거나 나올 때도 끝까지 한다
    fn celebrating(&self) -> bool {
        matches!(self.state, State::Eating { level_up: true, .. } | State::LevelUp { .. })
    }

    /// 집으로 가는 길의 진척을 잰다. 안전 점프를 할 때가 되면 true
    fn track_home_progress(&mut self, dt: f64) -> bool {
        let held = self.is_held();
        let celebrating = self.celebrating();
        let x = self.body.x;
        let on_home_run = self.state == State::GoHome;
        let state = self.state;
        let standing = self.standing;
        let Some(trip) = self.home_trip.as_mut() else { return false };
        if held || celebrating || matches!(state, State::HomeLeap { .. }) {
            return false;
        }
        // 하우스 바닥에서 문으로 달리는 중엔 언젠가 반드시 도착하므로 세지 않는다
        if on_home_run && standing.map(|s| s.owner) == Some(trip.owner) {
            trip.since_progress = 0.0;
            return false;
        }
        let d = (trip.door_x - x).abs();
        if d <= trip.best - HOME_PROGRESS_PX {
            trip.best = d;
            trip.since_progress = 0.0;
            return false;
        }
        trip.since_progress += dt;
        if trip.since_progress > HOME_PROGRESS_WINDOW_SECS {
            trip.stalled += dt;
        }
        trip.stalled >= HOME_STALL_LIMIT_SECS
    }

    /// 바로 하우스 안에 둔다 (앱 시작 시 하우스에 있던 경우, 그리고 도착했을 때)
    pub fn enter_house(&mut self) {
        self.housed = true;
        self.home_trip = None;
        self.inside = None;
        self.standing = None;
        self.pacing.after_beat = None;
        self.before_press = None;
        self.body.vx = 0.0;
        self.body.vy = 0.0;
        self.clear_hover();
        self.flight = None;
        self.hover_at = None;
        self.meal_spot = None;
        // 먹이를 받고 내려앉기 전에 집에 들어갔으면 집 안에서 먹는다
        // 그릇으로 가다가 집에 불려 왔어도 집 안에서 먹는다
        let trip = self.food.take().map(|t| (t.heart, t.level_up));
        self.state = match self.meal_pending.take().or(trip) {
            Some((favorite, level_up)) => State::Eating { remaining: EAT_SECS, favorite, level_up },
            None => State::Housed,
        };
    }

    /// 하우스로 달려간다. door_x = owner(하우스가 있는 작업 영역 바닥) 위의 문 x.
    /// 들려 있으면 놓인 뒤에, 공중이면 내려선 뒤에 달려가고, 벽·화면을 타는 중이면 바로 떨어진다.
    pub fn go_home(&mut self, owner: Owner, door_x: f64) {
        // 손님은 하우스가 없고, 화면을 떠나는 중·떠난 뒤엔 가지 않는다
        if self.housed || self.guest.is_some() || self.leaving() {
            return;
        }
        self.home_trip = Some(match self.home_trip {
            // 이미 가는 중이면 지금까지의 기록을 이어 간다
            Some(t) => HomeTrip { owner, door_x, ..t },
            None => HomeTrip::new(owner, door_x, self.body.x),
        });
        // 그릇으로 가던 길은 접고 집 안에서 먹는다 (가다가 멈추면 새 자리에 그릇을 다시 놓는다). 그릇·꽃은 거둔다
        if let Some(trip) = self.food.as_mut() {
            trip.target = None;
            trip.from_house = false;
        }
        self.return_home = None;
        if !matches!(self.state, State::Eating { .. }) {
            self.hide_prop();
        }
        self.pacing.after_beat = None;
        self.clear_hover();
        if self.celebrating() {
            // 성장 축하를 마친 뒤에 간다
            return;
        }
        match self.state {
            // 들어오는 중이면 다 들어온 뒤에 간다
            State::Pressed { .. } | State::Dragged | State::Air { .. } | State::HomeLeap { .. } | State::Enter { .. } => {}
            State::Climb { .. } | State::Slide | State::WallClimb { .. } | State::InnerClimb { .. } => self.start_fall(),
            // 창 안이면 update_inside가 먼저 뛰어나가게 한다. 나는 동물은 어디서든 날아간다
            _ if self.standing.is_some() || self.cfg.flies => self.state = State::GoHome,
            _ => self.start_fall(),
        }
    }

    /// 하우스에서 나온다: owner 바닥의 문 x에 서서 왼쪽으로 걸어 나온다.
    /// 성장해서 먹는 중·축하 중이었다면 문 앞에서 그걸 마저 한 뒤 평소처럼 움직인다.
    /// 아직 가는 중이었다면 가던 길만 멈춘다 (그 자리에 그대로).
    pub fn leave_house(&mut self, world: &World, door_x: f64, owner: Owner) {
        // 나와서 먹고 있었다면 다 먹은 뒤에도 밖에 있는다
        self.return_home = None;
        if !self.housed {
            if self.home_trip.take().is_some() && matches!(self.state, State::GoHome | State::HomeLeap { .. }) {
                if matches!(self.state, State::HomeLeap { .. }) {
                    self.state = State::Air { anim: Anim::Fall };
                } else {
                    self.state = self.idle_state();
                }
            }
            return;
        }
        self.housed = false;
        self.home_trip = None;
        self.facing = -1.0;
        let Some((ax, ay)) = world.anchor(owner) else {
            self.body.x = door_x;
            self.start_fall();
            return;
        };
        self.body = Body { x: door_x, y: ay, vx: 0.0, vy: 0.0 };
        self.standing = Some(Standing { owner, rel_x: door_x - ax });
        if self.celebrating() {
            return;
        }
        self.state = State::Move { kind: Move::Walk, remaining: behavior::range(&mut self.rng, Move::Walk.secs()) };
        self.on_started();
    }

    fn clear_hover(&mut self) {
        self.hover_hist.clear();
        self.tickling = false;
        self.hover_since = None;
    }

    /// 하우스 안: 말풍선 타이머와 하우스 안에서 먹기·성장 축하·갸웃·하트만 흐른다
    fn update_housed(&mut self, dt: f64) {
        self.update_chatter(dt);
        self.state = match self.state {
            State::Eating { remaining, favorite, level_up } => {
                let remaining = remaining - dt;
                if remaining > 0.0 {
                    State::Eating { remaining, favorite, level_up }
                } else if level_up {
                    self.speech = Some(Speech::LevelUp);
                    State::LevelUp { remaining: LEVEL_UP_SECS }
                } else if favorite {
                    State::Trick { kind: Trick::Heart, remaining: AFTER_FAVORITE_HEART_SECS }
                } else {
                    State::Housed
                }
            }
            State::LevelUp { remaining } if remaining > dt => State::LevelUp { remaining: remaining - dt },
            State::Trick { kind, remaining } if remaining > dt => State::Trick { kind, remaining: remaining - dt },
            _ => State::Housed,
        };
    }

    /// 문에 도착: 문 앞 바닥에 두고 하우스 안으로
    fn arrive_home(&mut self, world: &World) {
        let Some(trip) = self.home_trip else { return };
        self.body.x = trip.door_x;
        if let Some((_, ay)) = world.anchor(trip.owner) {
            self.body.y = ay;
        }
        self.house_door = Some((trip.owner, trip.door_x));
        self.enter_house();
    }

    /// 집으로 달린다. 하우스 바닥에선 문까지, 다른 디딤판에선 집 쪽 끝까지 가서 반드시 떠난다
    /// (창 모서리·가려짐·화면 끝은 뛰어내리고, 모니터 경계는 넘어간다 — 돌아서지 않는다).
    fn update_go_home(&mut self, dt: f64, world: &World) {
        let Some(trip) = self.home_trip else { return };
        if !self.follow_anchor(world) {
            self.start_fall();
            return;
        }
        let st = self.standing.unwrap();
        let Some(piece) = world.piece_at(st.owner, self.body.x).copied() else {
            self.start_fall();
            return;
        };
        let home_here = st.owner == trip.owner && piece.contains_x(trip.door_x);
        let dx = trip.door_x - self.body.x;
        if home_here && dx == 0.0 {
            self.arrive_home(world);
            return;
        }
        // 다른 디딤판에서 문 바로 위·아래에 있으면 방향을 바꾸지 않고 하던 쪽으로 계속 간다 (제자리 맴돌기 방지)
        let dir = if home_here || dx.abs() >= self.cfg.half_w { dx.signum() } else { self.facing };
        self.facing = dir;
        if !home_here && piece.width() < 2.0 * self.cfg.half_w {
            self.start_hop(dir);
            return;
        }
        let speed = self.cfg.walk_speed * Move::Run.speed_mult();
        let new_x = self.body.x + dir * speed * dt;
        let step_to = |p: &mut Pet, x: f64| {
            p.standing = Some(Standing { owner: st.owner, rel_x: st.rel_x + (x - p.body.x) });
            p.body.x = x;
            p.state = State::GoHome;
        };
        if home_here {
            if (trip.door_x - new_x) * dir <= 0.0 {
                self.arrive_home(world);
            } else {
                step_to(self, new_x);
            }
            return;
        }
        let (edge_x, end) = if dir > 0.0 { (piece.x_end, piece.end_kind) } else { (piece.x_start, piece.start_kind) };
        let beyond = |x: f64| (x - edge_x) * dir > 0.0;
        if !beyond(new_x + dir * self.cfg.half_w) {
            step_to(self, new_x);
            return;
        }
        match end {
            EndKind::Seam { to } => {
                if self.at_seam(world, to, Move::Run, speed, new_x, beyond(new_x)) {
                    self.state = State::GoHome;
                } else if matches!(self.state, State::Turn { .. }) {
                    // 너무 높은 이웃 모니터: 돌아서지 않고 그쪽으로 뛰어 본다 (안 되면 안전 점프가 해결)
                    self.start_hop(dir);
                }
            }
            _ => {
                self.start_hop(dir);
                self.body.vx *= behavior::RUN_LEAP_VX_MULT;
            }
        }
    }

    /// 지금 위치에서 문 앞 바닥까지 한 번에 닿는 포물선 (화면 위로 머리가 나가지 않게)
    fn start_home_leap(&mut self, world: &World) {
        let Some(trip) = self.home_trip else { return };
        let Some((_, gy)) = world.anchor(trip.owner) else {
            // 하우스가 있던 모니터가 사라짐 — 그냥 하우스 안으로
            self.enter_house();
            return;
        };
        let (x0, y0) = (self.body.x, self.body.y);
        let low = y0.min(gy);
        let apex = (low - self.scaled(HOME_LEAP_RISE)).max(world.bounds.top + self.cfg.size).min(low);
        let vy = -(2.0 * GRAVITY * (y0 - apex)).sqrt();
        let t = -vy / GRAVITY + (2.0 * (gy - apex) / GRAVITY).sqrt();
        if t < 1e-3 {
            self.arrive_home(world);
            return;
        }
        let vx = (trip.door_x - x0) / t;
        self.pacing.after_beat = None;
        self.inside = None;
        self.standing = None;
        self.body.vx = vx;
        self.body.vy = vy;
        if vx.abs() > 1.0 {
            self.facing = vx.signum();
        }
        self.state = State::HomeLeap { remaining: t };
    }

    fn update_home_leap(&mut self, dt: f64, remaining: f64, world: &World) {
        let step = dt.min(remaining);
        self.body.x += self.body.vx * step;
        self.body.y += self.body.vy * step + 0.5 * GRAVITY * step * step;
        self.body.vy += GRAVITY * step;
        let remaining = remaining - dt;
        if remaining > 0.0 {
            self.state = State::HomeLeap { remaining };
        } else {
            self.arrive_home(world);
        }
    }

    /// 친구가 연결된 화면 끝을 정한다 — 바닥을 걷다 그 끝에 닿으면 가끔 걸어 나간다
    pub fn set_exit_edges(&mut self, left: bool, right: bool) {
        self.exit_edges = ExitEdges { left, right };
    }

    /// 손님 모드: home = 집으로 가는 화면 끝, homesick_after초 뒤 집이 그리워진다 (visit::pick_homesick_after).
    /// 손님은 배고파하지 않고 하우스로 가지 않는다
    pub fn set_guest(&mut self, home: Side, homesick_after: f64) {
        self.guest = Some(Guest { home, homesick_in: homesick_after });
        self.hungry = false;
        self.food_request = false;
        if matches!(self.state, State::Hungry { .. }) {
            self.state = self.idle_state();
        }
    }

    /// 손님이면 집 쪽 끝과 남은 그리움 시간 (다른 친구 화면으로 건널 때 이어 간다)
    pub fn guest(&self) -> Option<Guest> {
        self.guest
    }

    /// 한쪽 화면 끝으로 곧장 가는 중: 집을 그리워하는 손님, 또는 "놀러 보내기"로 보낸 내 동물 (brain_send.rs의 heading)
    pub fn is_homesick(&self) -> bool {
        self.heading().is_some()
    }

    /// 화면 끝(side) 바깥 바닥에서 걸어 들어오는 새 동물 (친구 동물이 놀러 옴)
    pub fn new_entering(cfg: PetConfig, world: &World, side: Side, seed: u64) -> Self {
        let mut p = Self::new(cfg, 0.0, 0.0, seed);
        p.enter(world, side);
        p
    }

    /// 화면 끝(side) 바깥 바닥에 두고 안쪽으로 걸어 들어오게 한다 (놀러 갔다 돌아올 때도)
    pub fn enter(&mut self, world: &World, side: Side) {
        let inward = -visit::dir(side);
        self.exited = None;
        self.home_trip = None;
        self.inside = None;
        self.pacing.after_beat = None;
        self.before_press = None;
        self.clear_hover();
        self.facing = inward;
        if self.cfg.flies {
            self.enter_flying(world, side);
            return;
        }
        let Some((owner, edge_x, y)) = visit::entry_ground(world, side) else {
            self.body = Body { x: edge_x_fallback(world, side, self.cfg.half_w), y: world.bounds.top + self.cfg.size, vx: 0.0, vy: 0.0 };
            self.start_fall();
            return;
        };
        let x = edge_x - inward * self.cfg.half_w;
        let (ax, _) = world.anchor(owner).expect("surface owner has anchor");
        self.body = Body { x, y, vx: 0.0, vy: 0.0 };
        self.standing = Some(Standing { owner, rel_x: x - ax });
        self.state = State::Enter { remaining: visit::ENTER_SECS, side };
    }

    /// 손님에게 "집에 와" (들려 있거나 공중이면 내려선 뒤): 제자리에서 손 흔든 뒤 걸어 나가지 않고 사라진다 (take_exit = Some(집 쪽))
    pub fn go_home_now(&mut self) {
        if self.guest.is_none() || self.leaving() {
            return;
        }
        // 끌려가는 도중엔 바꾸지 않는다 — 놓여서 내려선 뒤 update가 다시 부른다
        if self.is_held() || matches!(self.state, State::Air { .. }) {
            self.farewell_pending = true;
            return;
        }
        self.pacing.after_beat = None;
        self.before_press = None;
        self.clear_hover();
        self.body.vx = 0.0;
        self.body.vy = 0.0;
        self.state = State::Farewell { remaining: visit::FAREWELL_SECS };
    }

    /// 화면을 떠났으면 그 쪽을 한 번 돌려준다
    pub fn take_exit(&mut self) -> Option<Side> {
        self.exited.take()
    }

    /// 화면을 떠나 있음 (친구 화면에 가 있거나 손님이 집으로 돌아감)
    pub fn is_away(&self) -> bool {
        self.state == State::Away
    }

    /// 떠나는 중이거나 떠났음 — 잡기·먹이·집 가기를 받지 않는다
    fn leaving(&self) -> bool {
        matches!(self.state, State::Exit { .. } | State::Farewell { .. } | State::Away)
    }

    /// 떠나거나 들어오는 중
    fn in_transit(&self) -> bool {
        self.leaving() || matches!(self.state, State::Enter { .. })
    }

    /// 걷기 속도 (px/s)
    fn walk_speed(&self) -> f64 {
        self.cfg.walk_speed * Move::Walk.speed_mult()
    }

    /// 손님 그리움 타이머. 그리워지는 순간 집 반대쪽으로 걷고 있었다면 돌아선다
    fn tick_guest(&mut self, dt: f64) {
        let Some(g) = self.guest.as_mut() else { return };
        let was = g.is_homesick();
        g.homesick_in -= dt;
        let (now, home) = (g.is_homesick(), g.home);
        // 그리워진 지 3분이 지나도 못 갔으면 (붙잡혀 있으면 놓인 뒤) 제자리에서 손 흔들고 집으로
        if g.is_overdue() {
            self.go_home_now();
            return;
        }
        if !was && now && matches!(self.state, State::Move { .. }) && self.facing != visit::dir(home) {
            self.pacing.after_beat = None;
            self.state = State::Turn { remaining: TURN_PAUSE };
        }
    }

    /// 걷다가 화면 가장 바깥 끝(piece의 진행 방향 끝)에 닿음: 걸어 나갈 거면 그 쪽
    fn wants_exit(&mut self, world: &World, piece: &Surface) -> Option<Side> {
        let side = visit::screen_edge(world, piece, self.facing)?;
        let chance = self.exit_odds(side);
        (chance >= 1.0 || chance > 0.0 && self.rng.f64() < chance).then_some(side)
    }

    fn start_exit(&mut self, side: Side) {
        self.pacing.after_beat = None;
        self.clear_hover();
        self.state = State::Exit { remaining: visit::EXIT_SECS, side };
    }

    /// 바닥 높이를 따라 dx만큼 걷는다 (경계 밖이어도). 바닥이 사라졌으면 x만 옮긴다
    fn walk_by(&mut self, dx: f64, world: &World) {
        match self.standing {
            Some(st) => {
                self.standing = Some(Standing { rel_x: st.rel_x + dx, ..st });
                if !self.follow_anchor(world) {
                    self.standing = None;
                    self.body.x += dx;
                }
            }
            None => self.body.x += dx,
        }
    }

    fn update_exit(&mut self, dt: f64, world: &World, remaining: f64, side: Side) {
        self.facing = visit::dir(side);
        self.walk_by(self.facing * self.walk_speed() * dt, world);
        let remaining = remaining - dt;
        if remaining <= 0.0 && visit::fully_outside(&world.bounds, side, self.body.x, self.cfg.half_w) {
            self.vanish(side);
        } else {
            self.state = State::Exit { remaining, side };
        }
    }

    fn update_enter(&mut self, dt: f64, world: &World, remaining: f64, side: Side) {
        self.facing = -visit::dir(side);
        if self.standing.is_none() {
            self.start_fall();
            return;
        }
        self.walk_by(self.facing * self.walk_speed() * dt, world);
        if self.standing.is_none() {
            self.start_fall();
            return;
        }
        let remaining = remaining - dt;
        self.state = if remaining <= 0.0 && visit::fully_inside(&world.bounds, side, self.body.x, self.cfg.half_w) {
            State::Move { kind: Move::Walk, remaining: behavior::range(&mut self.rng, Move::Walk.secs()) }
        } else {
            State::Enter { remaining, side }
        };
    }

    fn update_farewell(&mut self, dt: f64, remaining: f64) {
        let remaining = remaining - dt;
        match self.guest {
            Some(g) if remaining <= 0.0 => self.vanish(g.home),
            _ => self.state = State::Farewell { remaining },
        }
    }

    /// 화면에서 사라진다 (side 쪽으로)
    fn vanish(&mut self, side: Side) {
        self.exited = Some(side);
        self.standing = None;
        self.inside = None;
        self.body.vx = 0.0;
        self.body.vy = 0.0;
        self.pacing.after_beat = None;
        self.clear_hover();
        self.state = State::Away;
    }

    pub fn is_held(&self) -> bool {
        matches!(self.state, State::Pressed { .. } | State::Dragged)
    }

    /// 공중이거나 벽에 붙어 있음 (디딤판 위가 아님)
    fn off_surface(&self) -> bool {
        matches!(
            self.state,
            State::Air { .. } | State::Climb { .. } | State::Slide | State::WallClimb { .. } | State::InnerClimb { .. } | State::HomeLeap { .. } | State::Leap { .. }
        )
    }

    /// 매 틱 update 전에 호출 (버튼 안 누른 상태). inside = 커서가 스프라이트 사각형 안
    pub fn hover(&mut self, cx: f64, cy: f64, inside: bool) {
        self.cursor = (cx, cy);
        // 음식을 들고 있는 동안엔 간질이지 않는다
        if self.hand.is_some() || self.is_held() || self.off_surface() || self.housed || self.home_trip.is_some() || self.in_transit() {
            self.clear_hover();
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
            _ => return,
        }
        // 나는 동물은 제자리에 떠서 커서 쪽을 본다
        let dx = self.cursor.0 - self.body.x;
        if self.cfg.flies && dx.abs() > 1.0 {
            self.facing = dx.signum();
        }
    }

    /// 쏜 화면 지점 (한 번 꺼내면 None) — 앱이 그 자리에 침 자국 창을 띄운다
    pub fn take_sting(&mut self) -> Option<(f64, f64)> {
        self.sting_at.take()
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
        // 걸어 들어오는 중엔 (몸이 아직 화면 밖일 수 있다) 잡지 않는다
        if self.is_held() || self.housed || self.in_transit() {
            return;
        }
        // 쏘기는 콕! 할 때까지 손으로 막을 수 없다
        if matches!(self.state, State::Sting { phase: StingPhase::Alert | StingPhase::Dash, .. }) {
            return;
        }
        // 떠 있는 나는 동물은 디딤판 위처럼 제자리에서 눌린다
        let grounded = (self.standing.is_some() || self.cfg.flies) && !self.off_surface();
        let was_sleeping = matches!(self.state, State::Sleep { .. } | State::Annoyed { .. });
        self.grab_dx = self.body.x - cx;
        self.grab_dy = self.body.y - cy;
        self.hover_hist.clear();
        self.tickling = false;
        self.hover_since = None;
        self.history.clear();
        self.history.push_back((self.clock, cx, cy));
        self.thrown = false;
        self.shake.clear();
        // 나는 동물: 눌리면 가던 길은 잊는다 (놓인 자리에서 다시 정한다)
        self.flight = None;
        // 놀던 중이면 놀이는 끝난다 — 무시해도 놀이로 돌아가지 않고 잠깐 앉는다
        let before = if self.social_role().is_some() { State::Idle { remaining: TURN_PAUSE, look: false } } else { self.state };
        self.play_target = None;
        self.before_press = Some((before, self.pacing.after_beat.take()));
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
                    // 집어 들면 창 밖 — 놓은 자리에서 다시 정한다
                    self.inside = None;
                    self.state = State::Dragged;
                    self.drag_to(cx, cy);
                    self.shake.push(self.clock, start_cx, start_cy);
                    self.shake_check(cx, cy);
                }
            }
            State::Dragged => {
                self.record(cx, cy);
                self.drag_to(cx, cy);
                self.shake_check(cx, cy);
            }
            _ => {}
        }
    }

    /// 동물 교체 시 벽 타기 가능 여부를 바꾼다. 못 타는 동물로 바뀌었는데 벽에 붙어 있으면 떨어진다.
    pub fn set_can_climb(&mut self, can_climb: bool) {
        self.cfg.can_climb = can_climb;
        if !can_climb && matches!(self.state, State::Climb { .. } | State::Slide | State::WallClimb { .. } | State::InnerClimb { .. }) {
            self.start_fall();
        }
    }

    pub fn set_hungry(&mut self, hungry: bool) {
        // 손님은 배고파하지 않는다 (배고픔은 주인 쪽에만)
        if self.guest.is_some() {
            return;
        }
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
    /// 하우스 안에서도 먹는다 (하우스 창이 문간에서 먹는 모습을 보여 준다). 하우스로 가는 중엔 false
    pub fn feed_result(&mut self, result: FeedResult, food: Food) -> bool {
        let ate = match result {
            FeedResult::Ate { leveled_up } => Some(leveled_up),
            FeedResult::Refused => None,
        };
        if !self.feedable() {
            // 간식·물은 기억해 뒀다가 내려서면 먹는다 (떠나는 중·손님은 아니다)
            if let (true, Some(l)) = (food.is_treat() && !self.leaving() && self.guest.is_none(), ate) {
                self.queue_food(food, l);
                return true;
            }
            return false;
        }
        // 성장 축하·하트를 끊지 않는다 — 끝난 뒤에 먹는다
        if let (true, Some(l)) = (self.celebrating_now(), ate) {
            self.queue_food(food, l);
            return true;
        }
        if self.cfg.flies && !self.housed && self.standing.is_none() {
            return self.feed_flying(result, food);
        }
        self.pacing.after_beat = None;
        match result {
            FeedResult::Refused => {
                self.speech = Some(Speech::Full);
                self.state = State::Trick { kind: Trick::Tilt, remaining: REFUSE_TILT_SECS };
            }
            FeedResult::Ate { leveled_up } => self.serve(food, leveled_up),
        }
        true
    }

    /// 지금 먹이를 받을 수 있음: 하우스 안, 또는 디딤판 위(나는 동물은 떠서)에서 놓여 있고 하우스로 가는 중이 아님
    fn feedable(&self) -> bool {
        if self.housed {
            return true;
        }
        let settled = (self.standing.is_some() || self.cfg.flies) && !self.off_surface() && !self.is_held() && !self.in_transit();
        settled && self.home_trip.is_none() && !matches!(self.state, State::Sting { .. })
    }

    /// 성장 축하 중이거나 하트 중
    fn celebrating_now(&self) -> bool {
        matches!(self.state, State::LevelUp { .. } | State::Trick { kind: Trick::Heart, .. })
    }

    /// 나중에 먹을 먹이를 기억한다 (여럿이면 자라는 음식이 우선, 성장은 남긴다)
    fn queue_food(&mut self, food: Food, level_up: bool) {
        self.queued_food = Some(match self.queued_food {
            Some((old, l)) if old.grows() && !food.grows() => (old, l || level_up),
            Some((_, l)) => (food, l || level_up),
            None => (food, level_up),
        });
    }

    /// 기억해 둔 먹이를 받을 수 있게 되면(내려서거나 축하가 끝나면) 준다
    fn serve_queued_food(&mut self) {
        if self.queued_food.is_none() || !self.feedable() || self.celebrating_now() || matches!(self.state, State::Land { .. }) && self.cfg.flies {
            return;
        }
        if let Some((food, level_up)) = self.queued_food.take() {
            self.pacing.after_beat = None;
            self.serve(food, level_up);
        }
    }

    /// 디딤판에 놓인 소품이 창과 함께 움직였으면 새 자리를 다시 보낸다
    fn track_prop(&mut self, world: &World) {
        if !self.prop_shown {
            self.prop_anchor = None;
            return;
        }
        let Some((prop, owner, rel_x, last)) = self.prop_anchor else { return };
        let Some((ax, ay)) = world.anchor(owner) else { return };
        let now = (ax + rel_x, ay);
        if (now.0 - last.0).hypot(now.1 - last.1) > 1.0 {
            self.prop_event = Some(PropEvent::Show { prop, x: now.0, y: now.1 });
            self.prop_anchor = Some((prop, owner, rel_x, now));
        }
    }

    /// 디딤판(owner) 위 기준점에서 rel_x 자리에 소품을 놓는다 (창이 움직이면 따라간다)
    pub(super) fn show_prop_on(&mut self, prop: Prop, owner: Owner, rel_x: f64, world: &World) {
        let Some((ax, ay)) = world.anchor(owner) else { return };
        self.show_prop(prop, ax + rel_x, ay);
        self.prop_anchor = Some((prop, owner, rel_x, (ax + rel_x, ay)));
    }

    /// 나는 동물이 아니게 됐다 (동물 바꾸기): 날기 상태를 버리고, 내려앉아 먹으려던 먹이는 그릇으로 가서 먹는다
    fn drop_flying_state(&mut self) {
        if self.cfg.flies {
            return;
        }
        self.flight = None;
        self.hover_at = None;
        if let Some((heart, level_up)) = self.meal_pending.take() {
            let food = match self.meal_spot.take() {
                Some((_, _, Prop::WaterBowl)) => Food::Water,
                _ => Food::Meal,
            };
            self.food = Some(FoodTrip { food, heart, level_up, target: None, from_house: false });
        }
    }

    /// 먹이를 받았다. 먹는 중·가는 중이면 그대로 이어 가고 (앞서 정해진 성장 축하·하트는 남긴다),
    /// 간식은 그 자리에서 쿠키를 오물오물, 밥·좋아하는 음식·물은 그릇(나는 동물은 꽃)으로 가서 먹는다.
    /// 하우스 안이면 문 앞으로 나와 먹는다 (문을 모르거나 간식이면 하우스 안에서)
    pub(super) fn serve(&mut self, food: Food, level_up: bool) {
        let heart = food == Food::Favorite || food.is_treat();
        if let State::Eating { favorite, level_up: was, .. } = self.state {
            self.state = State::Eating { remaining: EAT_SECS, favorite: favorite || heart, level_up: was || level_up };
            return;
        }
        if let Some(trip) = self.food.as_mut() {
            trip.heart |= heart;
            trip.level_up |= level_up;
            return;
        }
        if let Some((favorite, was)) = self.meal_pending {
            self.meal_pending = Some((favorite || heart, was || level_up));
            return;
        }
        if food == Food::Snack || self.housed && self.house_door.is_none() {
            if food == Food::Snack && !self.housed {
                let (x, y) = (self.body.x, self.body.y - self.cfg.size * food::COOKIE_RISE);
                self.show_prop(Prop::Cookie, x, y);
            }
            self.state = State::Eating { remaining: EAT_SECS, favorite: heart, level_up };
            return;
        }
        self.food = Some(FoodTrip { food, heart, level_up, target: None, from_house: self.housed });
        if !self.housed {
            self.flight = None;
            self.state = State::ToFood { remaining: food::FOOD_TIMEOUT_SECS };
        }
    }

    /// 입 (손에 든 음식이 여기 닿으면 받아먹는다)
    pub fn mouth(&self) -> (f64, f64) {
        if self.cfg.flies {
            crate::sting::face_point(self.body.x, self.body.y, self.cfg.size)
        } else {
            (self.body.x, self.body.y - food::MOUTH_HEIGHT * self.cfg.size)
        }
    }

    /// 직접 주기: 손에 든 음식이 (x, y)에 있다 (매 틱 부른다). 동물이 음식을 보고 기다리다가 입에 닿았으면 true —
    /// 그때 앱이 돌보기에 먹이고 eat_from_hand를 부른다
    pub fn offer_hand(&mut self, x: f64, y: f64) -> bool {
        self.hand = Some((x, y));
        matches!(self.state, State::WantFood { .. }) && food::in_reach(self.mouth(), (x, y), self.cfg.size)
    }

    /// 손에 든 음식을 거뒀다 (먹지 않음)
    pub fn end_hand(&mut self) {
        self.hand = None;
        self.hand_noticed = false;
        if matches!(self.state, State::WantFood { .. }) {
            self.state = self.idle_state();
        }
    }

    /// 손에서 받아먹기 (offer_hand가 true일 때): 그릇 없이 그 자리에서 먹고 하트. 배부르면 갸웃 (음식은 손에 남는다)
    pub fn eat_from_hand(&mut self, result: FeedResult, _food: Food) -> bool {
        if self.hand.is_none() || !matches!(self.state, State::WantFood { .. }) {
            return false;
        }
        self.pacing.after_beat = None;
        match result {
            FeedResult::Refused => {
                self.speech = Some(Speech::Full);
                self.state = State::Trick { kind: Trick::Tilt, remaining: REFUSE_TILT_SECS };
            }
            FeedResult::Ate { leveled_up } => {
                self.hand = None;
                self.hand_noticed = false;
                self.state = State::Eating { remaining: EAT_SECS, favorite: true, level_up: leveled_up };
            }
        }
        true
    }

    /// 손에 든 음식을 알아챈다: 한가할 때(쉬기·이동·잠 등) 하던 걸 멈추고 음식을 본다
    fn notice_hand(&mut self) {
        if self.hand.is_none() {
            if matches!(self.state, State::WantFood { .. }) {
                self.state = self.idle_state();
            }
            return;
        }
        let busy = self.housed
            || self.guest.is_some()
            || self.home_trip.is_some()
            || self.food.is_some()
            || self.meal_pending.is_some()
            || self.is_held()
            || self.off_surface()
            || self.in_transit();
        let free = matches!(
            self.state,
            State::Idle { .. }
                | State::Move { .. }
                | State::Turn { .. }
                | State::Hungry { .. }
                | State::Sleep { .. }
                | State::Land { .. }
                | State::Tickled { .. }
                | State::Annoyed { .. }
                | State::Huff { .. }
                | State::Special { .. }
        );
        if busy || !free {
            return;
        }
        if !std::mem::replace(&mut self.hand_noticed, true) && self.hungry {
            self.speech = Some(Speech::Eager);
        }
        self.pacing.after_beat = None;
        self.flight = None;
        self.state = State::WantFood { walking: false };
    }

    /// 손에 든 음식 쪽을 본다. 같은 바닥 가까이(위로 몸 2.5개, 옆으로 4개 안)면 그 아래까지 걸어간다 (바닥 안에서만)
    fn update_want_food(&mut self, dt: f64, st: Standing, piece: &Surface) {
        let Some((fx, fy)) = self.hand else {
            self.state = self.idle_state();
            return;
        };
        let size = self.cfg.size;
        let dx = fx - self.body.x;
        if dx.abs() > 1.0 {
            self.facing = dx.signum();
        }
        let rise = self.body.y - fy;
        let near = rise >= -0.5 * size && rise <= 2.5 * size && dx.abs() <= 4.0 * size;
        let (lo, hi) = (piece.x_start + self.cfg.half_w, piece.x_end - self.cfg.half_w);
        let goal = if lo > hi { self.body.x } else { fx.clamp(lo, hi) };
        let gap = goal - self.body.x;
        let walking = near && gap.abs() > 2.0;
        if walking {
            let step = (self.walk_speed() * dt).min(gap.abs());
            let new_x = self.body.x + gap.signum() * step;
            self.standing = Some(Standing { owner: st.owner, rel_x: st.rel_x + (new_x - self.body.x) });
            self.body.x = new_x;
        }
        self.state = State::WantFood { walking };
    }

    /// 소품 변화 (한 번 꺼내면 None) — 앱이 그릇·꽃·쿠키 창을 띄우거나 거둔다
    pub fn take_prop(&mut self) -> Option<PropEvent> {
        self.prop_event.take()
    }

    /// 하우스 문 자리 (앱이 하우스를 놓을 때·끌어 옮길 때·먹이를 줄 때 알려 준다).
    /// 집으로 가는 중이거나 나와서 먹는 중이면 새 문으로 간다 — 하던 일(축하·쏘기·날기)은 그대로
    pub fn set_house_door(&mut self, owner: Owner, door_x: f64) {
        self.house_door = Some((owner, door_x));
        if let Some(back) = self.return_home.as_mut() {
            *back = (owner, door_x);
        }
        if let Some(trip) = self.home_trip.as_mut() {
            if trip.owner != owner {
                // 다른 바닥으로 옮겼다 — 거리 기록은 새로 잰다 (안전 점프 판정)
                *trip = HomeTrip::new(owner, door_x, self.body.x);
            } else if trip.door_x != door_x {
                // 문이 멀어졌을 수 있다 — 진척 기준을 지금 거리로
                trip.door_x = door_x;
                trip.best = (door_x - self.body.x).abs();
                trip.since_progress = 0.0;
            }
        }
    }

    /// 하우스에서 나와 먹는 중 (다 먹으면 들어간다)
    pub fn returns_home(&self) -> bool {
        self.return_home.is_some()
    }

    fn show_prop(&mut self, prop: Prop, x: f64, y: f64) {
        self.prop_anchor = None;
        self.prop_shown = true;
        self.prop_event = Some(PropEvent::Show { prop, x, y });
    }

    fn hide_prop(&mut self) {
        if std::mem::take(&mut self.prop_shown) {
            self.prop_event = Some(PropEvent::Hide);
        }
    }

    /// 매 틱: 다 먹었으면(또는 먹기가 끊겼으면) 그릇을 거두고, 하우스에서 나와 먹었으면 하트까지 본 뒤 집으로
    fn after_meal(&mut self) {
        if self.is_eating() {
            return;
        }
        self.hide_prop();
        if self.return_home.is_some() && !matches!(self.state, State::Trick { kind: Trick::Heart, .. }) {
            if let Some((owner, door_x)) = self.return_home.take() {
                self.go_home(owner, door_x);
            }
        }
    }

    /// 하우스 안에서 먹이를 받았다: 문 앞 바닥으로 나와 그릇으로 간다 (다 먹으면 돌아간다)
    fn come_out_for_food(&mut self, world: &World) {
        let (Some((owner, door_x)), Some(mut trip)) = (self.house_door, self.food) else { return };
        self.leave_house(world, door_x, owner);
        trip.from_house = false;
        self.food = Some(trip);
        self.return_home = Some((owner, door_x));
        if self.standing.is_some() {
            self.state = State::ToFood { remaining: food::FOOD_TIMEOUT_SECS };
        }
    }

    /// 그릇으로 걷는다 (걷는 동물). 그릇은 처음에 바라보는 쪽 앞에 놓이고, 다른 디딤판으로 옮겨졌으면 거기 다시 놓인다
    fn update_to_food(&mut self, dt: f64, world: &World, st: Standing, piece: &Surface, remaining: f64) {
        let Some(mut trip) = self.food else {
            self.state = self.idle_state();
            return;
        };
        let Some((ax, _)) = world.anchor(st.owner) else { return };
        let (lo, hi) = (piece.x_start + self.cfg.half_w, piece.x_end - self.cfg.half_w);
        let placed = trip.target.filter(|&(owner, rel)| owner == st.owner && (lo > hi || (lo..=hi).contains(&(ax + rel))));
        let target = match placed {
            Some((_, rel)) => ax + rel,
            None => {
                let x = food::bowl_x(self.body.x, self.facing, self.scaled(food::BOWL_GAP), lo, hi);
                trip.target = Some((st.owner, x - ax));
                self.show_prop_on(Prop::for_food(trip.food, false), st.owner, x - ax, world);
                x
            }
        };
        self.food = Some(trip);
        let dx = target - self.body.x;
        let step = self.walk_speed() * dt;
        let remaining = remaining - dt;
        if dx.abs() > step && remaining <= 0.0 {
            // 너무 오래 걸렸다 — 그릇을 지금 자리로 옮겨 여기서 먹는다
            self.show_prop_on(Prop::for_food(trip.food, false), st.owner, self.body.x - ax, world);
            self.start_eating();
            return;
        }
        let new_x = if dx.abs() <= step { target } else { self.body.x + dx.signum() * step };
        if dx.abs() > 1e-9 {
            self.facing = dx.signum();
        }
        self.standing = Some(Standing { owner: st.owner, rel_x: st.rel_x + (new_x - self.body.x) });
        self.body.x = new_x;
        if new_x == target {
            self.start_eating();
        } else {
            self.state = State::ToFood { remaining };
        }
    }

    /// 그릇 앞에 닿았다 (또는 기다리다 지쳤다): 받은 먹이를 먹기 시작한다
    fn start_eating(&mut self) {
        let Some(trip) = self.food.take() else { return };
        self.pacing.after_beat = None;
        self.state = State::Eating { remaining: EAT_SECS, favorite: trip.heart, level_up: trip.level_up };
    }

    pub fn pointer_up(&mut self, cx: f64, cy: f64) {
        // 하우스로 가는 중엔 클릭에 반응하지 않고 계속 간다 (드래그해서 던지면 내려선 뒤 다시 간다)
        if self.home_trip.is_some() && matches!(self.state, State::Pressed { grounded: true, .. }) {
            self.state = match self.before_press {
                // 성장 축하는 마저 한다
                Some((state @ (State::Eating { level_up: true, .. } | State::LevelUp { .. }), _)) => state,
                _ => State::GoHome,
            };
            return;
        }
        // 꿀벌: 깨어 있을 때 2초 안에 세 번 누르면 쏜다
        if let State::Pressed { was_sleeping: false, .. } = self.state {
            // 배고파서 음식을 고르려고 누르는 건 괴롭힘이 아니다
            if self.cfg.stings && !self.hungry && self.pester.push(self.clock) && self.can_sting_now() {
                self.start_sting();
                return;
            }
        }
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
                // 친구 동물: 반반 애교 / 잡아 봐! (brain_guestplay.rs)
                if self.guest.is_some() {
                    self.guest_click();
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
                    // 나는 동물은 뛰어오르는 대신 떠서 하트 (공중에 뜨면 다음 클릭을 못 받는다)
                    ClickReaction::Hop if self.cfg.flies => {
                        self.heart_timer = HEART_SECS;
                        self.state = State::Trick { kind: Trick::Heart, remaining: behavior::CLICK_TRICK_SECS };
                    }
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
                if let Some(trip) = self.home_trip.as_mut() {
                    // 옮겨 놓은 자리부터 진척을 다시 잰다
                    *trip = HomeTrip { stalled: trip.stalled, ..HomeTrip::new(trip.owner, trip.door_x, cx + self.grab_dx) };
                }
                self.record(cx, cy);
                let (vx, vy) = self.release_velocity();
                let lift = if self.cfg.flies { 0.0 } else { THROW_LIFT };
                let (vx, vy) = throw_velocity(vx, vy, self.cfg.dpi, lift);
                if vx.abs() > 50.0 {
                    self.facing = vx.signum();
                }
                self.standing = None;
                self.body.vx = vx;
                self.body.vy = vy;
                self.state = State::Air { anim: Anim::Fall };
                self.thrown = true;
                self.drop_check = true;
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

    /// 최근 커서 샘플의 처음→끝 평균 속도 (물리 px/s). 같은 틱의 샘플은 마지막 것만 쓰고,
    /// 처음이나 마지막 한 구간만 유난히 빠르면(튄 이벤트) 그 샘플 하나를 뺀다
    fn release_velocity(&self) -> (f64, f64) {
        let mut pts: Vec<(f64, f64, f64)> = Vec::with_capacity(self.history.len());
        for &s in &self.history {
            match pts.last_mut() {
                Some(last) if s.0 - last.0 < 1e-9 => *last = s,
                _ => pts.push(s),
            }
        }
        let speed = |a: (f64, f64, f64), b: (f64, f64, f64)| (b.1 - a.1).hypot(b.2 - a.2) / (b.0 - a.0);
        let (mut lo, mut hi) = (0, pts.len().saturating_sub(1));
        if pts.len() >= 4 {
            let segs: Vec<f64> = pts.windows(2).map(|w| speed(w[0], w[1])).collect();
            let n = segs.len();
            let median_of = |skip: usize| {
                let mut rest: Vec<f64> = segs.iter().enumerate().filter(|&(i, _)| i != skip).map(|(_, &v)| v).collect();
                rest.sort_by(f64::total_cmp);
                rest[rest.len() / 2]
            };
            let (first, last) = (segs[0], segs[n - 1]);
            let first_out = first > OUTLIER_RATIO * median_of(0);
            let last_out = last > OUTLIER_RATIO * median_of(n - 1);
            if last_out && (!first_out || last >= first) {
                hi -= 1;
            } else if first_out {
                lo += 1;
            }
        }
        let (Some(&(t0, x0, y0)), Some(&(t1, x1, y1))) = (pts.get(lo), pts.get(hi)) else {
            return (0.0, 0.0);
        };
        let dt = t1 - t0;
        if dt < 1e-6 {
            return (0.0, 0.0);
        }
        ((x1 - x0) / dt, (y1 - y0) / dt)
    }

    /// 공중 중력: 던진 비행이면 논리 px 기준의 부드러운 중력, 아니면 평소 중력
    fn air_gravity(&self) -> f64 {
        if self.thrown {
            THROW_GRAVITY_LOGICAL * self.cfg.dpi
        } else {
            GRAVITY
        }
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
        if !matches!(self.state, State::Special { .. } | State::Eating { .. } | State::LevelUp { .. } | State::ToFood { .. }) {
            self.pacing.after_beat = None;
        }
        self.state = match self.state {
            State::Sleep { .. } => State::Annoyed { remaining: TICKLE_HOLD, tickled_for: 0.0 },
            State::Annoyed { tickled_for, .. } => State::Annoyed { remaining: TICKLE_HOLD, tickled_for },
            // 꿀벌은 간질이면 낄낄대지 않고 화나서 쏜다 (쏜 지 8초가 안 됐으면 투덜대기만)
            State::Tickled { .. } if self.cfg.stings && self.can_sting_now() => {
                self.start_sting();
                return;
            }
            State::Tickled { .. } => State::Tickled { remaining: TICKLE_HOLD },
            State::Special { .. } | State::Eating { .. } | State::LevelUp { .. } | State::Sting { .. } | State::ToFood { .. } => return,
            _ if self.cfg.stings => {
                if self.can_sting_now() {
                    self.start_sting();
                    return;
                }
                self.speech = Some(Speech::Grumpy);
                State::Tickled { remaining: TICKLE_HOLD }
            }
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
        // 그릇이 기다리고 있다 — 내려서거나 쉬던 참이면 그릇으로 간다 (나는 동물은 꽃으로 날아간다)
        if self.food.is_some()
            && matches!(self.state, State::Idle { .. } | State::Move { .. } | State::Turn { .. } | State::Land { .. } | State::Hungry { .. } | State::Sleep { .. })
        {
            self.pacing.after_beat = None;
            self.state = State::ToFood { remaining: food::FOOD_TIMEOUT_SECS };
            if self.cfg.flies {
                return;
            }
        }

        match self.state {
            State::ToFood { remaining } => self.update_to_food(dt, world, st, &piece, remaining),
            State::WantFood { .. } => self.update_want_food(dt, st, &piece),
            State::Move { kind, remaining } => {
                let speed = self.cfg.walk_speed * kind.speed_mult() * self.hop_pace(kind, dt);
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
                } else if let (true, EndKind::Edge, Move::Roll) = (hit, end, kind) {
                    // 발 중심이 창 모서리를 넘을 때까지 굴러가다가 굴러떨어진다
                    // (vy = 0으로 떠나므로 중심이 아직 창 위면 다음 틱에 바로 다시 내려선다)
                    self.standing = Some(Standing { owner: st.owner, rel_x: st.rel_x + (new_x - self.body.x) });
                    self.body.x = new_x;
                    if beyond(new_x) {
                        self.standing = None;
                        self.body.vx = self.facing * speed;
                        self.body.vy = 0.0;
                        self.state = State::Air { anim: Anim::Roll };
                        return;
                    }
                } else if hit {
                    if end == EndKind::Wall && self.climb_out_at(world, st.owner, edge_x, dt) {
                        return;
                    }
                    match (end, kind) {
                        // 집이 그리운 손님은 돌아서지 않고 뛰어내린다
                        (EndKind::Edge, Move::Walk) if self.is_homesick() || self.rng.f64() < behavior::WALK_EDGE_HOP_CHANCE => {
                            self.start_hop(self.facing)
                        }
                        (EndKind::Edge, Move::Run) => {
                            self.start_hop(self.facing);
                            self.body.vx *= behavior::RUN_LEAP_VX_MULT;
                        }
                        // 화면 가장 바깥 끝: 친구 화면으로 걸어 나가거나 뒤돈다
                        (EndKind::Wall, Move::Walk | Move::Run) => match self.wants_exit(world, &piece) {
                            Some(side) => self.start_exit(side),
                            None => self.state = State::Turn { remaining: TURN_PAUSE },
                        },
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
                } else if !self.is_homesick() && self.rng.f64() < behavior::MOVE_END_TURN_CHANCE {
                    State::Turn { remaining: TURN_PAUSE }
                } else {
                    self.rest_beat()
                };
            }
            _ => self.tick_activity(dt, world),
        }
    }

    /// 이동이 아닌 활동(쉬기·애교·잠·먹기 등)의 시간을 흘리고 끝나면 다음 활동을 정한다 —
    /// 디딤판 위의 동물과 떠 있는 나는 동물이 같이 쓴다
    fn tick_activity(&mut self, dt: f64, world: &World) {
        match self.state {
            State::Idle { remaining, look } => {
                let remaining = remaining - dt;
                self.state = if remaining > 0.0 {
                    State::Idle { remaining, look }
                } else {
                    match self.pacing.after_beat.take() {
                        Some(AfterBeat::Go(next)) => next,
                        Some(AfterBeat::Rest) => self.rest_state(world),
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
            // 가던 길이 취소된 뒤 남은 GoHome / 하우스 밖의 Housed는 쉬기로
            State::GoHome | State::Housed => self.state = self.idle_state(),
            State::Air { .. }
            | State::Pressed { .. }
            | State::Dragged
            | State::Climb { .. }
            | State::Slide
            | State::WallClimb { .. }
            | State::HomeLeap { .. }
            | State::Exit { .. }
            | State::Enter { .. }
            | State::Farewell { .. }
            | State::Away
            | State::InnerClimb { .. }
            | State::Sting { .. }
            | State::ToFood { .. }
            | State::WantFood { .. }
            | State::Leap { .. }
            | State::Greet { .. }
            | State::Chase { .. }
            | State::Flee { .. }
            | State::CatchMe { .. }
            | State::Move { .. } => {}
        }
    }

    fn update_air(&mut self, dt: f64, world: &World) {
        if let Some(stay) = self.inside {
            self.update_air_inside(dt, world, stay);
            return;
        }
        // 던져서 친구가 연결된 화면 끝에 닿으면 튕기지 않고 넘어간다 (brain_fling.rs)
        let (half_w, b) = (self.cfg.half_w, world.bounds);
        if self.fling_out(world, self.body.x + self.body.vx * dt, b.left + half_w, b.right - half_w) {
            return;
        }
        let (prev_x, prev_y, prev_vx) = (self.body.x, self.body.y, self.body.vx);
        let g = self.air_gravity();
        step_air(&mut self.body, dt, &world.bounds, self.cfg.half_w, self.cfg.size, g);
        if self.body.vx.abs() > 1.0 {
            self.facing = self.body.vx.signum();
        }
        if self.cfg.can_climb && self.home_trip.is_none() && prev_vx.abs() > behavior::WALL_STICK_MIN_VX * self.cfg.dpi {
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
            self.body.y = (self.body.y - self.scaled(behavior::CLIMB_SPEED) * self.climb_pace(dt) * dt).max(target_y);
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
        let rel_y = rel_y - self.scaled(behavior::WALL_CLIMB_SPEED) * self.climb_pace(dt) * dt;
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
        // 머리가 화면 위로 나갈 만큼 높이 올라가면 (꼭대기가 화면 위에 붙은 창) 떨어진다
        if world.side_covered(window, edge_x, y) != Some(false) || y - self.cfg.size < world.bounds.top {
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
        self.body.vx = self.facing * self.scaled(HOP_VX).max((target_x - self.body.x).abs() / t);
        self.body.vy = vy;
        false
    }

    pub(crate) fn start_hop(&mut self, dir: f64) {
        self.pacing.after_beat = None;
        self.standing = None;
        // 크기에 비례: 다시 같은 높이로 내려올 때(0.52초) 발 중심이 몸 반 폭(0.3×크기)보다 멀리 가 있도록
        self.body.vx = dir * self.scaled(HOP_VX);
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
    /// 집이 그리운 손님은 기어오르지 않고, 마음대로 방향을 고를 땐 늘 집 쪽으로 간다 (Turn 직후엔 돌아선 쪽 그대로).
    fn move_state(&mut self, may_flip: bool, world: &World) -> State {
        if self.cfg.flies {
            return self.fly_move_state();
        }
        let homesick = self.is_homesick();
        if !homesick && self.cfg.can_climb && self.standing.is_some() && self.inside.is_none() && self.rng.f64() < behavior::CLIMB_CHANCE {
            if let Some(target_y) = self.climb_target(world) {
                return self.next_after_beat(State::Climb { target_y, hang: behavior::CLIMB_HANG_SECS });
            }
        }
        if let Some(leap) = self.big_jump_state(world) {
            return leap;
        }
        if let Some(side) = self.heading() {
            if may_flip {
                self.facing = visit::dir(side);
            }
        } else if may_flip && self.rng.bool() {
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
    fn rest_state(&mut self, world: &World) -> State {
        if self.cfg.flies && self.standing.is_none() {
            return self.fly_rest_state(world);
        }
        if let Some((window, rect)) = self.hop_in_window(world) {
            if self.rng.f64() < inside::HOP_IN_CHANCE {
                self.start_hop_in(window, &rect);
                return self.state;
            }
        }
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
        // 나는 동물은 내려앉아 있을 때만 (더 자주) 잔다
        let chance = match (self.cfg.flies, self.standing.is_some()) {
            (false, _) => behavior::SLEEP_CHANCE,
            (true, true) => crate::flight::PERCH_SLEEP_CHANCE,
            (true, false) => 0.0,
        };
        if self.rng.f64() < chance {
            State::Sleep { remaining: behavior::range(&mut self.rng, behavior::SLEEP_SECS) }
        } else {
            self.move_state(true, world)
        }
    }
}

/// 창 안에서 놀기 (spec 2B §9)
impl Pet {
    /// 창 안에서 노는 중이면 그 창 id (기어 나가는 중·뛰어나가는 중도 포함)
    pub fn inside_window(&self) -> Option<u64> {
        self.inside.map(|s| s.window)
    }

    /// 매 틱 (주 동작 전): 놓인 자리 판정, 창이 사라지면 밖으로, 선 자리가 가려졌거나 집에 가야 하면 뛰어나가기,
    /// 머문 시간이 다 되면 나가기 (벽 타는 동물은 절반은 안쪽 벽으로 걸어가 기어오른다)
    fn update_inside(&mut self, dt: f64, world: &World) {
        if std::mem::take(&mut self.drop_check) && matches!(self.state, State::Air { .. }) && self.home_trip.is_none() {
            let target = inside::drop_target(world, self.body.x, self.body.y, self.cfg.half_w, self.cfg.size);
            self.inside = target.map(|id| Stay::new(id, Mode::In, &mut self.rng));
        }
        let Some(mut stay) = self.inside else { return };
        stay.leave_in -= dt;
        self.inside = Some(stay);
        let Some(rect) = world.window(stay.window).map(|w| w.rect) else {
            // 창이 닫히거나 최소화됨 → 밖에서 평소처럼 떨어진다
            self.inside = None;
            if !self.is_held() && !matches!(self.state, State::Air { .. }) {
                self.start_fall();
            }
            return;
        };
        if self.is_held() || self.off_surface() || !self.follow_anchor(world) {
            return;
        }
        let room = world.piece_at(Owner::Inside(stay.window), self.body.x).is_some_and(|p| p.width() >= 2.0 * self.cfg.half_w);
        // 집에 가는 중이거나 집이 그리운 손님이면 바로 나간다
        if !room || (self.home_trip.is_some() || self.is_homesick()) && !self.celebrating() {
            self.hop_out(world, &rect, dt);
            return;
        }
        if stay.leave_in > 0.0 || !matches!(self.state, State::Move { .. } | State::Idle { .. } | State::Turn { .. }) {
            return;
        }
        if stay.climb_out {
            if stay.leave_in < -inside::CLIMB_OUT_GIVE_UP_SECS {
                self.hop_out(world, &rect, dt);
            }
            return;
        }
        if self.cfg.can_climb && self.rng.f64() < inside::CLIMB_OUT_CHANCE {
            // 가까운 안쪽 벽으로 걸어간다 — 벽에 닿으면 climb_out_at이 기어오르게 한다.
            // 포기 시간은 지금부터 잰다 (오래 자고 일어났어도)
            self.inside = Some(Stay { climb_out: true, leave_in: 0.0, ..stay });
            self.pacing.after_beat = None;
            self.facing = if self.body.x - rect.left < rect.right - self.body.x { -1.0 } else { 1.0 };
            self.state = State::Move { kind: Move::Walk, remaining: rect.width() / self.walk_speed() + 1.0 };
        } else {
            self.hop_out(world, &rect, dt);
        }
    }

    /// 창 윗면 위로 크게 뛰어 나간다 (천장 없음, 틱 dt의 적분 손실까지 감안). 두 번 뛰고도 못 나갔으면 벽·천장을 무시하는
    /// 도약(LeapOut)으로 반드시 나간다. 그 자리 윗면이 없으면(가려졌거나 화면 꼭대기에 붙은 창) 밖으로 떨어진다
    fn hop_out(&mut self, world: &World, rect: &Rect, dt: f64) {
        let Some(stay) = self.inside else { return };
        self.pacing.after_beat = None;
        if world.piece_at(Owner::Window(stay.window), self.body.x).is_some() {
            // 머리가 화면 위로 나가지 않을 만큼만 솟는다
            let clear = self.scaled(inside::LEAVE_CLEAR).min(rect.top - (world.bounds.top + self.cfg.size)).max(0.0);
            let leap = stay.hops >= 2;
            self.standing = None;
            self.body.vx = 0.0;
            self.body.vy = inside::leave_vy(self.body.y, rect.top, clear, if leap { 0.0 } else { dt });
            let mode = if leap { Mode::LeapOut } else { Mode::HopOut };
            self.inside = Some(Stay { mode, hops: stay.hops + 1, climb_out: false, ..stay });
            self.state = State::Air { anim: Anim::Hop };
        } else {
            self.inside = None;
            self.start_fall();
        }
    }

    /// 창 안 공중: 창 사각형이 경계(옆 = 안쪽 벽, 위 = 천장 — 뛰어들고 뛰어나갈 땐 없음), 안쪽 바닥에만 내려선다
    fn update_air_inside(&mut self, dt: f64, world: &World, stay: Stay) {
        let Some(rect) = world.window(stay.window).map(|w| w.rect) else {
            self.inside = None;
            self.update_air(dt, world);
            return;
        };
        if stay.mode == Mode::LeapOut {
            // 정확한 포물선 (꼭대기 = 윗면 위 clear). 꼭대기를 지나면 윗면 높이에서 바깥이 된다 → 다음 틱에 윗면에 선다
            self.body.y += self.body.vy * dt + 0.5 * GRAVITY * dt * dt;
            self.body.vy += GRAVITY * dt;
            if self.body.y <= rect.top || self.body.vy >= 0.0 {
                self.body.y = self.body.y.min(rect.top);
                self.inside = None;
            }
            return;
        }
        let mut stay = stay;
        if stay.mode == Mode::In && self.body.y - self.cfg.size < rect.top - 0.5 {
            // 머리가 윗면 위로 나와 있음 (기어 나가다 붙잡힘 등) — 천장에 끌어내리지 않고 천장 없이 떨어지거나 나간다
            stay.mode = Mode::HopOut;
            self.inside = Some(stay);
        }
        let bounds = inside::air_bounds(&rect, &world.bounds, stay.mode == Mode::In);
        let prev_y = self.body.y;
        let g = self.air_gravity();
        step_air(&mut self.body, dt, &bounds, self.cfg.half_w, self.cfg.size, g);
        if self.body.vx.abs() > 1.0 {
            self.facing = self.body.vx.signum();
        }
        match stay.mode {
            // 발이 윗면 위로 올라감 → 이제 바깥 (내려오며 윗면에 선다)
            Mode::HopOut if self.body.y <= rect.top => {
                self.inside = None;
                return;
            }
            Mode::HopIn if self.body.y - self.cfg.size >= rect.top => self.inside = Some(Stay { mode: Mode::In, ..stay }),
            _ => {}
        }
        if self.body.vy < 0.0 || self.body.y < inside::floor_y(&rect) {
            return;
        }
        let piece = world.piece_at(Owner::Inside(stay.window), self.body.x).copied().filter(|p| p.width() >= 2.0 * self.cfg.half_w);
        match piece {
            Some(s) => {
                self.body = Body { x: self.body.x, y: s.y, vx: 0.0, vy: 0.0 };
                self.standing = Some(Standing { owner: s.owner, rel_x: self.body.x - rect.left });
                self.inside = Some(Stay { mode: Mode::In, ..stay });
                self.state = State::Land { remaining: LAND_SECS };
            }
            // 내려설 자리가 앞쪽 창에 가려짐 → 밖으로 계속 떨어진다 (이번 틱에 지나친 바깥 디딤판이 있으면 선다)
            None => {
                self.inside = None;
                self.land_or_recover(prev_y, world);
            }
        }
    }

    /// 기어 나가기로 한 동물이 안쪽 벽(edge_x)에 닿음: 벽을 기어오른다 (그 자리 윗면이 없으면 뛰어나간다). 처리했으면 true
    fn climb_out_at(&mut self, world: &World, owner: Owner, edge_x: f64, dt: f64) -> bool {
        let (Owner::Inside(id), Some(stay)) = (owner, self.inside) else { return false };
        let Some(rect) = world.window(id).map(|w| w.rect) else { return false };
        if !stay.climb_out {
            return false;
        }
        let x = edge_x - self.facing * self.cfg.half_w;
        // 그 사이 벽을 못 타는 동물로 바뀌었거나 그 자리 윗면이 없으면 뛰어나간다
        if !self.cfg.can_climb || world.piece_at(Owner::Window(id), x).is_none() {
            self.hop_out(world, &rect, dt);
            return true;
        }
        self.inside = Some(Stay { climb_out: false, ..stay });
        self.pacing.after_beat = None;
        self.standing = None;
        self.body = Body { x, y: self.body.y, vx: 0.0, vy: 0.0 };
        self.state = State::InnerClimb { window: id, rel_x: x - rect.left, rel_y: self.body.y - rect.top };
        true
    }

    /// 안쪽 벽 타기: 창을 따라 움직이며 기어올라, 발이 윗면에 닿으면 그 자리 윗면에 올라선다 (바깥).
    /// 창이 사라지거나 붙은 자리가 앞쪽 창에 가려지면 밖으로 떨어진다
    fn update_inner_climb(&mut self, dt: f64, world: &World, window: u64, rel_x: f64, rel_y: f64) {
        let Some(rect) = world.window(window).map(|w| w.rect) else {
            self.inside = None;
            self.start_fall();
            return;
        };
        let x = rect.left + rel_x;
        let rel_y = rel_y - self.scaled(behavior::WALL_CLIMB_SPEED) * self.climb_pace(dt) * dt;
        if rel_y <= 0.0 {
            self.inside = None;
            self.body = Body { x, y: rect.top, vx: 0.0, vy: 0.0 };
            if world.piece_at(Owner::Window(window), x).is_some() {
                self.standing = Some(Standing { owner: Owner::Window(window), rel_x });
                self.state = State::Land { remaining: LAND_SECS };
            } else {
                self.start_fall();
            }
            return;
        }
        let y = rect.top + rel_y;
        if world.side_covered(window, x, y) != Some(false) {
            self.inside = None;
            self.body.x = x;
            self.start_fall();
            return;
        }
        self.body = Body { x, y, vx: 0.0, vy: 0.0 };
        self.state = State::InnerClimb { window, rel_x, rel_y };
    }

    /// 쉬기를 정할 때 뛰어들 수 있는 창: 들어갈 만한 창의 윗면에 서 있고, 집에 가는 중·집이 그리운 손님이 아니어야 한다
    fn hop_in_window(&self, world: &World) -> Option<(u64, Rect)> {
        let Some(Standing { owner: Owner::Window(id), .. }) = self.standing else { return None };
        if self.cfg.flies || self.inside.is_some() || self.home_trip.is_some() || self.is_homesick() {
            return None;
        }
        let rect = world.window(id)?.rect;
        // 윗면 모서리 바로 위(반 폭 + INSET 안)에선 뛰어들지 않는다 — 안쪽 벽에 끌려 들어가는 것처럼 보인다
        let margin = self.cfg.half_w + inside::INSET;
        let clear_of_edges = self.body.x - rect.left >= margin && rect.right - self.body.x >= margin;
        (clear_of_edges && inside::enterable(&rect, self.cfg.half_w, self.cfg.size) && inside::has_floor(world, id)).then_some((id, rect))
    }

    /// 창 윗면에서 안쪽 가운데 쪽으로 살짝 뛰어올라 윗면 선을 지나 떨어진다
    fn start_hop_in(&mut self, window: u64, rect: &Rect) {
        let dir = if (rect.left + rect.right) / 2.0 >= self.body.x { 1.0 } else { -1.0 };
        self.start_hop(dir);
        self.body.vx *= 0.5;
        self.inside = Some(Stay::new(window, Mode::HopIn, &mut self.rng));
    }
}

#[cfg(test)]
#[path = "brain_inside_tests.rs"]
mod inside_tests;

/// 날아다니는 동물 (spec 2B §10)
#[path = "brain_fly.rs"]
mod fly;

#[cfg(test)]
#[path = "brain_fly_tests.rs"]
mod fly_tests;

#[cfg(test)]
#[path = "brain_food_tests.rs"]
mod food_tests;

#[cfg(test)]
#[path = "brain_hand_tests.rs"]
mod hand_tests;

/// 폴짝 뛰는 동물 (개구리)
#[path = "brain_hop.rs"]
mod hop_moves;

/// 트레이 "놀러 보내기" — 친구가 연결된 끝으로 곧장 가서 나간다
#[path = "brain_send.rs"]
pub(crate) mod send_moves;

#[cfg(test)]
#[path = "brain_send_tests.rs"]
mod send_tests;

/// 던져서 친구 화면으로 넘어가기 (듀얼 모니터처럼)
#[path = "brain_fling.rs"]
mod fling_moves;

#[cfg(test)]
#[path = "brain_fling_tests.rs"]
mod fling_tests;

#[cfg(test)]
#[path = "brain_hop_tests.rs"]
mod hop_tests;

#[cfg(test)]
#[path = "brain_throw_tests.rs"]
mod throw_tests;

/// 다른 동물과 인사·술래 놀이 (짝짓기·쿨다운은 social.rs)
#[path = "brain_social.rs"]
pub mod social_moves;

/// 친구 동물과 놀아 주기 — 클릭하면 애교 또는 "잡아 봐!"
#[path = "brain_guestplay.rs"]
pub mod guest_play;

#[cfg(test)]
#[path = "brain_guestplay_tests.rs"]
mod guest_play_tests;

impl AsRef<Pet> for Pet {
    fn as_ref(&self) -> &Pet {
        self
    }
}

impl AsMut<Pet> for Pet {
    fn as_mut(&mut self) -> &mut Pet {
        self
    }
}

/// 들어올 바닥이 없을 때 (작업 영역이 없는 경우 등): 그 끝 안쪽에서 떨어뜨린다
fn edge_x_fallback(world: &World, side: Side, half_w: f64) -> f64 {
    match side {
        Side::Left => world.bounds.left + half_w,
        Side::Right => world.bounds.right - half_w,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::behavior::{Move, Trick};
    use crate::geom::{Rect, WindowInfo};

    const DT: f64 = 1.0 / 60.0;

    pub(super) fn cfg() -> PetConfig {
        PetConfig {
            size: 128.0,
            half_w: 38.0,
            walk_speed: 60.0,
            special: None,
            personality: Personality::Friendly,
            can_climb: false,
            flies: false,
            stings: false,
            hops: false,
            dpi: 1.0,
        }
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
    fn edge_hops_clear_the_window_at_every_size() {
        // 큰 창: 384 px 펫도 올라설 수 있게 (윗면 600, 머리 공간 충분)
        let w = World::new(vec![win(1, 500.0, 600.0, 1500.0, 900.0)], vec![Rect::new(0.0, 0.0, 1920.0, 1040.0)], 384.0);
        for size in [128.0, 256.0, 384.0] {
            let half_w = size * 0.3;
            let c = PetConfig { size, half_w, walk_speed: 60.0 * size / 128.0, ..cfg() };
            // 걷다가 모서리에서 뛰기 (몸 앞쪽이 막 닿음)
            let mut p = Pet::new(c, 0.0, 0.0, 1);
            place(&mut p, &w, Owner::Window(1), 1500.0 - half_w - 0.5, State::Move { kind: Move::Walk, remaining: 100.0 });
            p.facing = 1.0;
            p.start_hop(1.0);
            run(&mut p, &w, 240);
            assert_eq!(p.standing.map(|s| s.owner), Some(Owner::Ground(0)), "size {size}");
            assert!(p.body.x > 1500.0, "size {size}");
        }
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
        // 몸 앞쪽이 모서리에 닿아도 발 중심이 넘을 때까지는 계속 구른다
        place(&mut p, &w, Owner::Window(1), 1100.0 - 38.0 - 0.5, State::Move { kind: Move::Roll, remaining: 100.0 });
        p.facing = 1.0;
        p.update(DT, &w);
        assert!(matches!(p.state, State::Move { kind: Move::Roll, .. }), "{:?}", p.state);
        assert_eq!(p.standing.map(|s| s.owner), Some(Owner::Window(1)));
        place(&mut p, &w, Owner::Window(1), 1100.0 - 0.5, State::Move { kind: Move::Roll, remaining: 100.0 });
        p.update(DT, &w);
        assert_eq!(p.state, State::Air { anim: Anim::Roll });
        assert_eq!((p.body.vx, p.body.vy), (120.0, 0.0));
        assert!(p.standing.is_none());
        assert!(p.body.x > 1100.0);
    }

    #[test]
    fn rolling_off_a_window_edge_really_leaves_it() {
        let w = world(vec![w1()]);
        let mut p = Pet::new(cfg(), 0.0, 0.0, 1);
        place(&mut p, &w, Owner::Window(1), 1100.0 - 38.0 - 0.5, State::Move { kind: Move::Roll, remaining: 100.0 });
        p.facing = 1.0;
        let mut rolled_off = false;
        for _ in 0..240 {
            p.update(DT, &w);
            rolled_off |= p.state == State::Air { anim: Anim::Roll };
            if rolled_off && p.standing.is_some() {
                break;
            }
        }
        assert!(rolled_off);
        assert_eq!(p.standing.map(|s| s.owner), Some(Owner::Ground(0)));
        assert_eq!(p.body.y, 1040.0);
        assert!(p.body.x > 1100.0);
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
            Anim::Land, Anim::SitFront, Anim::Wave, Anim::Tilt, Anim::Belly, Anim::Stretch, Anim::Heart, Anim::Tickle, Anim::Annoyed, Anim::Hungry, Anim::Eat, Anim::LevelUp, Anim::Arch, Anim::Wag, Anim::Huff, Anim::Climb, Anim::Slide, Anim::Sting, Anim::BigJump];
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
        PetConfig {
            size: 128.0,
            half_w: 38.4,
            walk_speed: 90.0,
            special: None,
            personality: Personality::Friendly,
            can_climb: false,
            flies: false,
            stings: false,
            hops: false,
            dpi: 1.0,
        }
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
                    // 내려선 뒤 쉬는 박자 안 (박자가 끝나면 5%는 창 안으로 뛰어든다, spec 2B §9)
                    run(&mut p, &w, 60);
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
        let vx = 1200.0 * crate::physics::THROW_FEEL;
        assert!((p.body.vx - vx).abs() < 1.0, "vx={}", p.body.vx);
        assert!((p.body.vy + THROW_LIFT * vx).abs() < 1.0, "vy={}", p.body.vy);
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

    /// 밥은 그릇으로 걸어가서 먹는다 — 먹기 시작할 때까지 돌린다
    fn walk_to_bowl(p: &mut Pet, w: &World) {
        for _ in 0..(60 * 5) {
            if matches!(p.state, State::Eating { .. }) {
                return;
            }
            p.update(DT, w);
        }
        panic!("never reached the bowl: {:?}", p.state);
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
            walk_to_bowl(&mut p, &w);
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
        walk_to_bowl(&mut p, &w);
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
                walk_to_bowl(&mut p, &w);
                p.update(DT, &w);
                assert!(p.is_eating());
                let x = p.body.x;
                p.pointer_down(x, 350.0);
                assert!(p.is_eating(), "pressed mid-meal still counts as eating");
                p.update(DT, &w);
                p.pointer_up(x, 350.0);
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
        walk_to_bowl(&mut p, &w);
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

#[cfg(test)]
mod house_tests {
    use super::tests::{cfg, place, run, w1, world};
    use super::*;
    use crate::geom::Rect;

    const DT: f64 = 1.0 / 60.0;
    /// 집 문 x (기본 화면 바닥 Ground(0) 위)
    const DOOR: f64 = 1500.0;
    const HOME: Owner = Owner::Ground(0);

    fn idle() -> State {
        State::Idle { remaining: 5.0, look: false }
    }

    /// 집에 들어갈 때까지 돌리며 거친 상태를 모은다
    fn run_until_housed(p: &mut Pet, w: &World, ticks: usize) -> Vec<State> {
        let mut seen = Vec::new();
        for _ in 0..ticks {
            p.update(DT, w);
            seen.push(p.state);
            if p.is_housed() {
                break;
            }
        }
        seen
    }

    fn assert_at_door(p: &Pet, y: f64) {
        assert!(p.is_housed(), "state {:?}", p.state);
        assert_eq!(p.state, State::Housed);
        assert_eq!((p.body.x, p.body.y), (DOOR, y));
    }

    fn housed_pet(w: &World) -> Pet {
        let mut p = Pet::new(cfg(), 0.0, 0.0, 3);
        place(&mut p, w, HOME, DOOR, idle());
        p.enter_house();
        p
    }

    #[test]
    fn housed_pet_does_not_move_for_a_minute() {
        let w = world(vec![w1()]);
        let mut p = housed_pet(&w);
        let body = p.body;
        for _ in 0..3600 {
            p.hover(p.body.x, p.body.y - 10.0, true);
            p.update(DT, &w);
            assert_eq!(p.state, State::Housed);
            assert_eq!(p.body, body);
        }
        assert!(p.is_housed());
        assert_eq!(p.render().anim, Anim::SitFront);
    }

    #[test]
    fn housed_pet_still_says_it_is_hungry() {
        let w = world(vec![]);
        let mut p = housed_pet(&w);
        p.set_hungry(true);
        let mut said = Vec::new();
        for _ in 0..3600 {
            p.update(DT, &w);
            said.extend(p.take_speech());
        }
        assert!(said.contains(&Speech::Hungry), "{said:?}");
        assert!(p.is_housed());
    }

    #[test]
    fn housed_pet_ignores_pointer_down() {
        let w = world(vec![]);
        let mut p = housed_pet(&w);
        p.pointer_down(DOOR, 1000.0);
        assert!(!p.is_held());
        assert_eq!(p.state, State::Housed);
    }

    #[test]
    fn feeding_a_housed_pet_eats_in_the_house_then_sits_again() {
        let w = world(vec![]);
        let mut p = housed_pet(&w);
        assert!(p.feed_result(FeedResult::Ate { leveled_up: true }, Food::Meal));
        assert!(p.is_housed() && p.is_eating());
        assert_eq!(p.render().anim, Anim::Eat);
        let mut said = Vec::new();
        let mut anims = Vec::new();
        for _ in 0..600 {
            p.update(DT, &w);
            said.extend(p.take_speech());
            anims.push(p.render().anim);
        }
        assert!(anims.contains(&Anim::LevelUp), "{anims:?}");
        assert!(said.contains(&Speech::LevelUp));
        assert_eq!(p.state, State::Housed);
        assert!(p.is_housed() && !p.is_eating());
        assert_eq!(p.body.x, DOOR);
    }

    #[test]
    fn full_housed_pet_refuses_with_a_tilt_and_stays_home() {
        let w = world(vec![]);
        let mut p = housed_pet(&w);
        assert!(p.feed_result(FeedResult::Refused, Food::Snack));
        assert_eq!(p.take_speech(), Some(Speech::Full));
        assert_eq!(p.render().anim, Anim::Tilt);
        run(&mut p, &w, 300);
        assert_eq!(p.state, State::Housed);
    }

    #[test]
    fn leaving_the_house_walks_left_from_the_door_on_the_ground() {
        let w = world(vec![]);
        let mut p = housed_pet(&w);
        p.leave_house(&w, DOOR, HOME);
        assert!(!p.is_housed());
        assert_eq!((p.body.x, p.body.y), (DOOR, 1040.0));
        assert_eq!(p.standing.map(|s| s.owner), Some(HOME));
        assert!(matches!(p.state, State::Move { kind: Move::Walk, .. }), "{:?}", p.state);
        assert_eq!(p.facing, -1.0);
        assert!(p.render().flip);
        p.update(DT, &w);
        assert!(p.body.x < DOOR && p.body.y == 1040.0);
    }

    #[test]
    fn runs_home_along_the_same_ground() {
        let w = world(vec![]);
        for (start, facing) in [(400.0, 1.0), (1800.0, -1.0)] {
            let mut p = Pet::new(cfg(), 0.0, 0.0, 5);
            place(&mut p, &w, HOME, start, idle());
            p.go_home(HOME, DOOR);
            assert_eq!(p.render().anim, Anim::Run);
            let seen = run_until_housed(&mut p, &w, 900);
            assert_at_door(&p, 1040.0);
            assert!(seen[..seen.len() - 1].iter().all(|s| *s == State::GoHome), "{seen:?}");
            assert_eq!(p.facing, facing);
            // 달리기 속도로 간다 (60 px/s × 2.5 = 150 px/s)
            let secs = seen.len() as f64 * DT;
            let expect = (DOOR - start).abs() / (60.0 * Move::Run.speed_mult());
            assert!((secs - expect).abs() < 0.1, "{secs} vs {expect}");
        }
    }

    #[test]
    fn hops_off_a_window_top_facing_away_and_arrives() {
        let w = world(vec![w1()]);
        let mut p = Pet::new(cfg(), 0.0, 0.0, 5);
        place(&mut p, &w, Owner::Window(1), 700.0, State::Move { kind: Move::Walk, remaining: 10.0 });
        p.facing = -1.0;
        p.go_home(HOME, DOOR);
        let seen = run_until_housed(&mut p, &w, 720);
        assert_at_door(&p, 1040.0);
        assert!(seen.contains(&State::Air { anim: Anim::Hop }), "{seen:?}");
        assert!(!seen.iter().any(|s| matches!(s, State::Turn { .. } | State::HomeLeap { .. })), "{seen:?}");
    }

    #[test]
    fn crosses_a_monitor_seam_to_get_home() {
        let w = World::new(vec![], vec![Rect::new(0.0, 0.0, 1920.0, 1040.0), Rect::new(1920.0, 0.0, 3840.0, 1040.0)], 128.0);
        let mut p = Pet::new(cfg(), 0.0, 0.0, 5);
        place(&mut p, &w, Owner::Ground(1), 2600.0, idle());
        p.go_home(HOME, DOOR);
        let seen = run_until_housed(&mut p, &w, 720);
        assert_at_door(&p, 1040.0);
        assert!(seen[..seen.len() - 1].iter().all(|s| *s == State::GoHome), "{seen:?}");
    }

    #[test]
    fn stuck_pet_leaps_home_after_twelve_seconds_without_progress() {
        // 아래 모니터에 있는 펫: 집(위 모니터 바닥)으로 이어지는 길이 없다
        let w = World::new(vec![], vec![Rect::new(0.0, 0.0, 1920.0, 1040.0), Rect::new(0.0, 1080.0, 1920.0, 2120.0)], 128.0);
        let mut p = Pet::new(cfg(), 0.0, 0.0, 5);
        place(&mut p, &w, Owner::Ground(1), DOOR, idle());
        p.go_home(HOME, DOOR);
        let seen = run_until_housed(&mut p, &w, (20.0 / DT) as usize);
        assert_at_door(&p, 1040.0);
        let leap = seen.iter().position(|s| matches!(s, State::HomeLeap { .. })).expect("leaped") as f64 * DT;
        assert!((12.0..=16.5).contains(&leap), "leap at {leap}s");
        assert!(!seen.iter().any(|s| matches!(s, State::Turn { .. })), "turned");
        assert_eq!(p.standing.map(|s| s.owner), None);
    }

    #[test]
    fn too_high_monitor_is_never_turned_away_from() {
        // 오른쪽 모니터가 200 px 높다 (뛰어오를 수 없다)
        let w = World::new(vec![], vec![Rect::new(0.0, 0.0, 1920.0, 1040.0), Rect::new(1920.0, 200.0, 3840.0, 1240.0)], 128.0);
        let mut p = Pet::new(cfg(), 0.0, 0.0, 5);
        place(&mut p, &w, Owner::Ground(1), 2200.0, idle());
        p.go_home(HOME, DOOR);
        let seen = run_until_housed(&mut p, &w, (22.0 / DT) as usize);
        assert_at_door(&p, 1040.0);
        assert!(!seen.iter().any(|s| matches!(s, State::Turn { .. })), "turned");
    }

    #[test]
    fn leap_stays_below_the_screen_top() {
        let w = World::new(vec![], vec![Rect::new(0.0, 0.0, 1920.0, 1040.0), Rect::new(0.0, 1080.0, 1920.0, 2120.0)], 128.0);
        let mut p = Pet::new(cfg(), 0.0, 0.0, 5);
        place(&mut p, &w, Owner::Ground(1), 300.0, idle());
        p.go_home(HOME, DOOR);
        for _ in 0..(40.0 / DT) as usize {
            p.update(DT, &w);
            assert!(p.body.y - p.cfg.size >= w.bounds.top - 1e-6, "{:?}", p.body);
            if p.is_housed() {
                break;
            }
        }
        assert!(p.is_housed());
    }

    #[test]
    fn airborne_pet_lands_first_then_runs_home() {
        let w = world(vec![]);
        let mut p = Pet::new(cfg(), 300.0, 600.0, 5);
        p.go_home(HOME, DOOR);
        assert!(matches!(p.state, State::Air { .. }));
        let seen = run_until_housed(&mut p, &w, 720);
        assert_at_door(&p, 1040.0);
        assert!(seen.contains(&State::GoHome));
    }

    #[test]
    fn climbing_pet_drops_off_the_wall_at_once() {
        let w = world(vec![]);
        let mut c = cfg();
        c.can_climb = true;
        let mut p = Pet::new(c, 0.0, 0.0, 5);
        place(&mut p, &w, HOME, 300.0, idle());
        p.standing = None;
        p.body.y = 700.0;
        p.state = State::Climb { target_y: 400.0, hang: 0.4 };
        p.go_home(HOME, DOOR);
        assert_eq!(p.state, State::Air { anim: Anim::Fall });
        run_until_housed(&mut p, &w, 720);
        assert_at_door(&p, 1040.0);
    }

    #[test]
    fn clicks_and_tickles_do_not_interrupt_the_trip() {
        let w = world(vec![]);
        let mut p = Pet::new(cfg(), 0.0, 0.0, 5);
        place(&mut p, &w, HOME, 400.0, idle());
        p.set_hungry(true);
        p.go_home(HOME, DOOR);
        run(&mut p, &w, 30);
        p.pointer_down(p.body.x, p.body.y - 40.0);
        p.update(DT, &w);
        p.pointer_up(p.body.x, p.body.y - 40.0);
        assert_eq!(p.state, State::GoHome);
        assert!(!p.take_food_request());
        // 커서로 문질러도 간지럼을 타지 않는다
        for i in 0..60 {
            let x = p.body.x + if i % 2 == 0 { -30.0 } else { 30.0 };
            p.hover(x, p.body.y - 40.0, true);
            p.update(DT, &w);
            assert_eq!(p.state, State::GoHome);
        }
        run_until_housed(&mut p, &w, 900);
        assert_at_door(&p, 1040.0);
    }

    #[test]
    fn dragging_mid_trip_then_releasing_resumes_the_trip() {
        let w = world(vec![]);
        let mut p = Pet::new(cfg(), 0.0, 0.0, 5);
        place(&mut p, &w, HOME, 400.0, idle());
        p.go_home(HOME, DOOR);
        run(&mut p, &w, 30);
        let (x, y) = (p.body.x, p.body.y - 40.0);
        p.pointer_down(x, y);
        p.pointer_move(x - 100.0, y - 300.0);
        assert_eq!(p.state, State::Dragged);
        // 오래 들고 있어도 그동안은 안전 점프를 하지 않는다
        for _ in 0..(13.0 / DT) as usize {
            p.update(DT, &w);
            assert_eq!(p.state, State::Dragged);
        }
        p.pointer_up(x - 100.0, y - 300.0);
        let seen = run_until_housed(&mut p, &w, 720);
        assert_at_door(&p, 1040.0);
        assert!(seen.contains(&State::GoHome));
        assert!(!seen.iter().any(|s| matches!(s, State::HomeLeap { .. })));
    }

    #[test]
    fn feeding_on_the_way_home_is_refused() {
        let w = world(vec![]);
        let mut p = Pet::new(cfg(), 0.0, 0.0, 5);
        place(&mut p, &w, HOME, 400.0, idle());
        p.go_home(HOME, DOOR);
        assert!(!p.feed_result(FeedResult::Ate { leveled_up: false }, Food::Meal));
        assert_eq!(p.state, State::GoHome);
    }

    #[test]
    fn leaving_before_arriving_just_cancels_the_trip() {
        let w = world(vec![]);
        let mut p = Pet::new(cfg(), 0.0, 0.0, 5);
        place(&mut p, &w, HOME, 400.0, idle());
        p.go_home(HOME, DOOR);
        run(&mut p, &w, 30);
        let x = p.body.x;
        p.leave_house(&w, DOOR, HOME);
        assert!(!p.is_housed());
        assert_eq!(p.body.x, x);
        assert_ne!(p.state, State::GoHome);
        run(&mut p, &w, 600);
        assert!(!p.is_housed());
    }

    #[test]
    fn going_home_twice_or_when_housed_is_harmless() {
        let w = world(vec![]);
        let mut p = housed_pet(&w);
        p.go_home(HOME, DOOR);
        assert_eq!(p.state, State::Housed);
        let mut p = Pet::new(cfg(), 0.0, 0.0, 5);
        place(&mut p, &w, HOME, 400.0, idle());
        p.go_home(HOME, DOOR);
        run(&mut p, &w, 60);
        p.go_home(HOME, DOOR);
        run_until_housed(&mut p, &w, 900);
        assert_at_door(&p, 1040.0);
    }

    /// 아기 곰처럼 느린 동물 (달리기 100 px/s)
    fn slow() -> PetConfig {
        PetConfig { size: 64.0, half_w: 19.0, walk_speed: 40.0, ..cfg() }
    }

    #[test]
    fn slow_pet_runs_all_the_way_across_the_screen() {
        let w = world(vec![]);
        let mut p = Pet::new(slow(), 0.0, 0.0, 5);
        place(&mut p, &w, HOME, 40.0, idle());
        p.go_home(HOME, 1880.0);
        let seen = run_until_housed(&mut p, &w, (25.0 / DT) as usize);
        assert!(p.is_housed() && p.body.x == 1880.0);
        assert!(seen.len() as f64 * DT > 17.0);
        assert!(!seen.iter().any(|s| matches!(s, State::HomeLeap { .. })));
    }

    #[test]
    fn slow_pet_runs_all_the_way_from_another_monitor() {
        let w = World::new(vec![], vec![Rect::new(0.0, 0.0, 1920.0, 1040.0), Rect::new(1920.0, 0.0, 3840.0, 1040.0)], 64.0);
        let mut p = Pet::new(slow(), 0.0, 0.0, 5);
        place(&mut p, &w, Owner::Ground(1), 3800.0, idle());
        p.go_home(HOME, DOOR);
        let seen = run_until_housed(&mut p, &w, (30.0 / DT) as usize);
        assert_at_door(&p, 1040.0);
        assert!(!seen.iter().any(|s| matches!(s, State::HomeLeap { .. })));
    }

    #[test]
    fn going_home_mid_meal_still_celebrates_the_level_up() {
        let w = world(vec![]);
        let mut p = Pet::new(cfg(), 0.0, 0.0, 5);
        place(&mut p, &w, HOME, 400.0, State::Eating { remaining: 1.0, favorite: false, level_up: true });
        p.go_home(HOME, DOOR);
        assert!(p.is_eating());
        // 축하 중 클릭해도 축하는 그대로
        run(&mut p, &w, 70);
        assert!(matches!(p.state, State::LevelUp { .. }), "{:?}", p.state);
        assert_eq!(p.take_speech(), Some(Speech::LevelUp));
        p.pointer_down(p.body.x, p.body.y - 40.0);
        p.pointer_up(p.body.x, p.body.y - 40.0);
        assert!(matches!(p.state, State::LevelUp { .. }), "{:?}", p.state);
        let seen = run_until_housed(&mut p, &w, 900);
        assert_at_door(&p, 1040.0);
        assert!(seen.contains(&State::GoHome));
    }

    #[test]
    fn leaving_the_house_mid_meal_celebrates_before_walking_out() {
        let w = world(vec![]);
        let mut p = housed_pet(&w);
        assert!(p.feed_result(FeedResult::Ate { leveled_up: true }, Food::Meal));
        p.leave_house(&w, DOOR, HOME);
        assert!(!p.is_housed() && p.is_eating());
        assert_eq!((p.body.x, p.body.y), (DOOR, 1040.0));
        assert_eq!(p.standing.map(|s| s.owner), Some(HOME));
        let mut said = Vec::new();
        let mut anims = Vec::new();
        for _ in 0..600 {
            p.update(DT, &w);
            said.extend(p.take_speech());
            anims.push(p.render().anim);
        }
        assert!(anims.contains(&Anim::LevelUp));
        assert!(said.contains(&Speech::LevelUp));
        assert!(!p.is_housed());
    }
}

#[cfg(test)]
mod visit_tests {
    use super::tests::{cfg, place, run, world};
    use super::*;
    use crate::behavior::Move;
    use crate::geom::Rect;

    const DT: f64 = 1.0 / 60.0;
    const RIGHT_EDGE: f64 = 1920.0 - 38.0 - 0.5;
    const LEFT_EDGE: f64 = 38.0 + 0.5;

    /// 화면 오른쪽(dir=1) 또는 왼쪽(dir=-1) 끝 바로 앞에서 kind 이동 중인 펫
    fn at_edge(w: &World, seed: u64, dir: f64, kind: Move) -> Pet {
        let mut p = Pet::new(cfg(), 0.0, 0.0, seed);
        place(&mut p, w, Owner::Ground(0), if dir > 0.0 { RIGHT_EDGE } else { LEFT_EDGE }, State::Move { kind, remaining: 100.0 });
        p.facing = dir;
        p
    }

    /// 끝에 닿은 한 틱 뒤 (Exit 수, Turn 수)
    fn outcomes(n: u64, dir: f64, kind: Move, setup: impl Fn(&mut Pet)) -> (usize, usize) {
        let w = world(vec![]);
        let (mut exits, mut turns) = (0, 0);
        for seed in 0..n {
            let mut p = at_edge(&w, seed, dir, kind);
            setup(&mut p);
            p.update(DT, &w);
            match p.state {
                State::Exit { side, .. } => {
                    assert_eq!(side, visit::side_of(dir));
                    exits += 1
                }
                State::Turn { .. } => turns += 1,
                s => panic!("seed {seed}: {s:?}"),
            }
        }
        (exits, turns)
    }

    /// 나가기를 시작한 펫 (시드를 돌며 찾는다)
    fn exiting(w: &World) -> Pet {
        for seed in 0..100 {
            let mut p = at_edge(w, seed, 1.0, Move::Walk);
            p.set_exit_edges(false, true);
            p.update(DT, w);
            if matches!(p.state, State::Exit { .. }) {
                return p;
            }
        }
        panic!("no exit in 100 seeds");
    }

    #[test]
    fn connected_edge_exits_about_30_percent() {
        for kind in [Move::Walk, Move::Run] {
            let (exits, turns) = outcomes(400, 1.0, kind, |p| p.set_exit_edges(false, true));
            assert!((90..=150).contains(&exits), "{kind:?} exits={exits}");
            assert_eq!(exits + turns, 400);
            let (exits, _) = outcomes(400, -1.0, kind, |p| p.set_exit_edges(true, false));
            assert!((90..=150).contains(&exits), "{kind:?} left exits={exits}");
        }
    }

    #[test]
    fn unconnected_edge_and_rolling_always_turn() {
        assert_eq!(outcomes(100, 1.0, Move::Walk, |p| p.set_exit_edges(true, false)), (0, 100));
        assert_eq!(outcomes(100, -1.0, Move::Run, |p| p.set_exit_edges(false, true)), (0, 100));
        assert_eq!(outcomes(100, 1.0, Move::Walk, |_| {}), (0, 100));
        assert_eq!(outcomes(100, 1.0, Move::Roll, |p| p.set_exit_edges(true, true)), (0, 100));
    }

    #[test]
    fn monitor_seam_is_not_an_exit() {
        let w = World::new(vec![], vec![Rect::new(0.0, 0.0, 1920.0, 1040.0), Rect::new(1920.0, 0.0, 3840.0, 1040.0)], 128.0);
        for seed in 0..50 {
            let mut p = at_edge(&w, seed, 1.0, Move::Walk);
            p.set_exit_edges(true, true);
            run(&mut p, &w, 90);
            assert!(matches!(p.state, State::Move { .. }), "seed {seed}: {:?}", p.state);
            assert_eq!(p.standing.map(|s| s.owner), Some(Owner::Ground(1)));
        }
    }

    #[test]
    fn exit_walks_off_the_screen_then_reports_once() {
        let w = world(vec![]);
        let mut p = exiting(&w);
        assert_eq!(p.render().anim, Anim::Walk);
        assert_eq!(p.take_exit(), None);
        let mut last_x = p.body.x;
        let mut ticks = 0;
        while p.take_exit().is_none() {
            p.update(DT, &w);
            ticks += 1;
            assert!(ticks < 600, "never left: {:?} x={}", p.state, p.body.x);
            if matches!(p.state, State::Exit { .. }) {
                assert!(p.body.x > last_x, "keeps walking out");
                assert_eq!(p.body.y, 1040.0);
                assert_eq!(p.render(), RenderState { anim: Anim::Walk, flip: false, heart: false });
            }
            last_x = p.body.x;
        }
        // 최소 1초, 몸이 다 빠져나갈 때까지
        assert!(ticks >= 59, "{ticks}");
        assert!(p.body.x - 38.0 >= 1920.0, "{}", p.body.x);
        assert!(p.is_away());
        assert_eq!(p.take_exit(), None);
        let body = p.body;
        run(&mut p, &w, 600);
        assert_eq!(p.take_exit(), None);
        assert_eq!(p.body, body);
        assert!(p.is_away());
    }

    #[test]
    fn exit_reports_the_side() {
        let w = world(vec![]);
        let mut p = exiting(&w);
        run(&mut p, &w, 600);
        assert_eq!(p.take_exit(), Some(Side::Right));
    }

    #[test]
    fn exiting_pet_cannot_be_grabbed_or_fed_or_sent_home() {
        let w = world(vec![]);
        let mut p = exiting(&w);
        p.pointer_down(p.body.x, p.body.y - 40.0);
        assert!(!p.is_held());
        assert!(!p.feed_result(FeedResult::Ate { leveled_up: false }, Food::Meal));
        p.go_home(Owner::Ground(0), 100.0);
        assert!(!p.is_going_home());
        assert!(matches!(p.state, State::Exit { .. }));
        run(&mut p, &w, 600);
        assert_eq!(p.take_exit(), Some(Side::Right));
    }

    #[test]
    fn entering_walks_in_from_outside_then_behaves_normally() {
        let w = world(vec![]);
        let mut p = Pet::new_entering(cfg(), &w, Side::Left, 1);
        assert!(matches!(p.state, State::Enter { .. }));
        assert_eq!((p.body.x, p.body.y, p.facing), (-38.0, 1040.0, 1.0));
        assert_eq!(p.standing.map(|s| s.owner), Some(Owner::Ground(0)));
        assert_eq!(p.render().anim, Anim::Walk);
        let mut ticks = 0;
        while matches!(p.state, State::Enter { .. }) {
            let x = p.body.x;
            p.update(DT, &w);
            assert!(p.body.x > x);
            ticks += 1;
            assert!(ticks < 600);
        }
        assert!(ticks >= 59, "{ticks}");
        assert!(p.body.x >= 38.0, "{}", p.body.x);
        assert!(matches!(p.state, State::Move { kind: Move::Walk, .. }), "{:?}", p.state);
        assert_eq!(p.facing, 1.0);
        // 그 뒤로는 평소처럼 (화면 안 바닥에서)
        let mut anims = Vec::new();
        for _ in 0..(60 * 60) {
            p.update(DT, &w);
            anims.push(p.render().anim);
            assert!(p.body.x >= 0.0 && p.body.x <= 1920.0);
        }
        assert!(anims.contains(&Anim::Idle));
        assert_eq!(p.take_exit(), None);
    }

    #[test]
    fn entering_from_the_right_faces_left() {
        let w = world(vec![]);
        let mut p = Pet::new_entering(cfg(), &w, Side::Right, 2);
        assert_eq!((p.body.x, p.facing), (1920.0 + 38.0, -1.0));
        run(&mut p, &w, 300);
        assert!(p.body.x <= 1920.0 - 38.0);
    }

    #[test]
    fn away_pet_can_come_back_in() {
        let w = world(vec![]);
        let mut p = exiting(&w);
        run(&mut p, &w, 600);
        assert_eq!(p.take_exit(), Some(Side::Right));
        p.enter(&w, Side::Right);
        assert!(!p.is_away());
        assert!(matches!(p.state, State::Enter { .. }));
        run(&mut p, &w, 300);
        assert!(p.body.x <= 1920.0 - 38.0);
    }

    fn guest(seed: u64, dir: f64, kind: Move, home: Side, homesick_after: f64) -> Pet {
        let w = world(vec![]);
        let mut p = at_edge(&w, seed, dir, kind);
        p.set_guest(home, homesick_after);
        p
    }

    #[test]
    fn guest_leaves_toward_home_half_the_time() {
        let (exits, turns) = outcomes(400, 1.0, Move::Walk, |p| p.set_guest(Side::Right, 1000.0));
        assert!((165..=235).contains(&exits), "exits={exits}");
        assert_eq!(exits + turns, 400);
        // 반대 끝에 다른 친구가 없으면 뒤돈다
        assert_eq!(outcomes(100, -1.0, Move::Walk, |p| p.set_guest(Side::Right, 1000.0)), (0, 100));
    }

    #[test]
    fn guest_roams_on_through_an_edge_linked_to_another_friend() {
        let (exits, turns) = outcomes(400, -1.0, Move::Walk, |p| {
            p.set_exit_edges(true, true);
            p.set_guest(Side::Right, 1000.0)
        });
        assert!((250..=310).contains(&exits), "exits={exits}");
        assert_eq!(exits + turns, 400);
        // 그리우면 더 놀러 가지 않는다
        assert_eq!(
            outcomes(100, -1.0, Move::Walk, |p| {
                p.set_exit_edges(true, true);
                p.set_guest(Side::Right, 0.0)
            }),
            (0, 100)
        );
    }

    #[test]
    fn guest_reports_its_home_and_homesick_time() {
        let w = world(vec![]);
        let mut p = guest(1, 1.0, Move::Walk, Side::Left, 100.0);
        assert_eq!(p.guest().map(|g| g.home), Some(Side::Left));
        place(&mut p, &w, Owner::Ground(0), 900.0, State::Idle { remaining: 100.0, look: false });
        run(&mut p, &w, 60);
        let left = p.guest().unwrap().homesick_in;
        assert!((left - 99.0).abs() < 0.01, "{left}");
        assert_eq!(at_edge(&w, 1, 1.0, Move::Walk).guest(), None);
    }

    #[test]
    fn homesick_guest_gives_up_and_waves_home_three_minutes_after_onset() {
        let w = world(vec![]);
        let mut p = guest(1, 1.0, Move::Walk, Side::Left, 0.0);
        place(&mut p, &w, Owner::Ground(0), 900.0, State::Idle { remaining: 100.0, look: false });
        // 붙잡혀 집에 못 가는 손님
        let (x, y) = (p.body.x, p.body.y - 40.0);
        p.pointer_down(x, y);
        p.pointer_move(x + 50.0, y - 100.0);
        run(&mut p, &w, 60 * 179);
        assert_eq!(p.state, State::Dragged);
        run(&mut p, &w, 60 * 2);
        assert_eq!(p.state, State::Dragged);
        p.pointer_up(x + 50.0, y - 100.0);
        let mut waved = false;
        for _ in 0..600 {
            p.update(DT, &w);
            waved |= matches!(p.state, State::Farewell { .. });
            if p.is_away() {
                break;
            }
        }
        assert!(waved);
        assert_eq!(p.take_exit(), Some(Side::Left));
    }

    #[test]
    fn overdue_homesick_guest_waves_home_where_it_stands() {
        let w = world(vec![]);
        let mut p = guest(1, 1.0, Move::Walk, Side::Right, -179.95);
        place(&mut p, &w, Owner::Ground(0), 900.0, State::Idle { remaining: 100.0, look: false });
        run(&mut p, &w, 10);
        assert!(matches!(p.state, State::Farewell { .. }), "{:?}", p.state);
        run(&mut p, &w, 70);
        assert_eq!(p.take_exit(), Some(Side::Right));
    }

    #[test]
    fn entering_guest_cannot_be_grabbed() {
        let w = world(vec![]);
        let mut p = Pet::new_entering(cfg(), &w, Side::Left, 1);
        p.set_guest(Side::Left, 1000.0);
        run(&mut p, &w, 20);
        p.pointer_down(p.body.x, p.body.y - 40.0);
        assert!(!p.is_held());
        assert!(matches!(p.state, State::Enter { .. }), "{:?}", p.state);
    }

    #[test]
    fn come_home_now_while_entering_waves_then_goes_back() {
        let w = world(vec![]);
        let mut p = Pet::new_entering(cfg(), &w, Side::Right, 1);
        p.set_guest(Side::Right, 1000.0);
        run(&mut p, &w, 20);
        p.go_home_now();
        assert!(matches!(p.state, State::Farewell { .. }), "{:?}", p.state);
        let body = p.body;
        run(&mut p, &w, 70);
        assert_eq!(p.take_exit(), Some(Side::Right));
        assert!(p.is_away());
        assert_eq!(p.body, body);
    }

    #[test]
    fn away_guest_says_nothing() {
        let w = world(vec![]);
        let mut p = guest(1, 1.0, Move::Walk, Side::Left, 1000.0);
        place(&mut p, &w, Owner::Ground(0), 900.0, State::Idle { remaining: 100.0, look: false });
        p.go_home_now();
        run(&mut p, &w, 70);
        assert!(p.is_away());
        let _ = p.take_speech();
        let (x, y) = (p.body.x, p.body.y - 40.0);
        for _ in 0..600 {
            p.pointer_down(x, y);
            p.pointer_up(x, y);
            p.hover(x, y, true);
            p.update(DT, &w);
            assert_eq!(p.take_speech(), None);
        }
    }

    #[test]
    fn homesick_guest_always_leaves_toward_home() {
        let (exits, _) = outcomes(100, 1.0, Move::Walk, |p| p.set_guest(Side::Right, 0.0));
        assert_eq!(exits, 100);
        let (exits, _) = outcomes(100, -1.0, Move::Walk, |p| p.set_guest(Side::Right, 0.0));
        assert_eq!(exits, 0);
    }

    #[test]
    fn guest_becomes_homesick_after_the_given_time() {
        let w = world(vec![]);
        let mut p = guest(1, 1.0, Move::Walk, Side::Right, 2.0);
        place(&mut p, &w, Owner::Ground(0), 900.0, State::Idle { remaining: 100.0, look: false });
        assert!(!p.is_homesick());
        run(&mut p, &w, 110);
        assert!(!p.is_homesick());
        run(&mut p, &w, 20);
        assert!(p.is_homesick());
    }

    #[test]
    fn homesick_guest_heads_home_and_leaves() {
        let w = world(vec![]);
        for seed in 0..10 {
            let mut p = guest(seed, -1.0, Move::Walk, Side::Right, 0.01);
            place(&mut p, &w, Owner::Ground(0), 900.0, State::Move { kind: Move::Walk, remaining: 100.0 });
            // 집 반대쪽으로 걷던 중이면 곧 돌아선다
            run(&mut p, &w, 60);
            assert_eq!(p.facing, 1.0, "seed {seed}");
            let mut left = None;
            for _ in 0..(60 * 240) {
                p.update(DT, &w);
                if let State::Move { .. } = p.state {
                    assert_eq!(p.facing, 1.0, "seed {seed}: only walks toward home");
                }
                left = left.or(p.take_exit());
                if left.is_some() {
                    break;
                }
            }
            assert_eq!(left, Some(Side::Right), "seed {seed}");
        }
    }

    #[test]
    fn come_home_now_waves_then_vanishes_without_walking() {
        let w = world(vec![]);
        let mut p = guest(1, 1.0, Move::Walk, Side::Left, 1000.0);
        place(&mut p, &w, Owner::Ground(0), 900.0, State::Move { kind: Move::Run, remaining: 100.0 });
        p.go_home_now();
        assert_eq!(p.render().anim, Anim::Wave);
        let body = p.body;
        p.pointer_down(p.body.x, p.body.y - 40.0);
        assert!(!p.is_held());
        run(&mut p, &w, 50);
        assert_eq!(p.body, body);
        assert_eq!(p.render().anim, Anim::Wave);
        assert_eq!(p.take_exit(), None);
        run(&mut p, &w, 15);
        assert_eq!(p.take_exit(), Some(Side::Left));
        assert!(p.is_away());
        assert_eq!(p.body, body);
        run(&mut p, &w, 120);
        assert_eq!(p.take_exit(), None);
    }

    #[test]
    fn come_home_now_waits_until_a_held_guest_is_let_go() {
        let w = world(vec![]);
        let mut p = guest(1, 1.0, Move::Walk, Side::Left, 1000.0);
        place(&mut p, &w, Owner::Ground(0), 900.0, State::Idle { remaining: 100.0, look: false });
        let (x, y) = (p.body.x, p.body.y - 40.0);
        p.pointer_down(x, y);
        assert!(p.is_held());
        p.go_home_now();
        assert!(p.is_held());
        p.pointer_move(x + 100.0, y - 200.0);
        run(&mut p, &w, 30);
        assert_eq!(p.state, State::Dragged);
        assert_eq!(p.take_exit(), None);
        p.pointer_up(x + 100.0, y - 200.0);
        // 떨어져 내려선 뒤 손 흔들고 집으로
        let mut waved = false;
        for _ in 0..600 {
            p.update(DT, &w);
            if matches!(p.state, State::Farewell { .. }) {
                assert!(p.standing.is_some());
                waved = true;
            }
            if p.is_away() {
                break;
            }
        }
        assert!(waved);
        assert_eq!(p.take_exit(), Some(Side::Left));
    }

    #[test]
    fn come_home_now_is_only_for_guests() {
        let w = world(vec![]);
        let mut p = at_edge(&w, 1, 1.0, Move::Walk);
        place(&mut p, &w, Owner::Ground(0), 900.0, State::Idle { remaining: 100.0, look: false });
        p.go_home_now();
        assert_eq!(p.state, State::Idle { remaining: 100.0, look: false });
        run(&mut p, &w, 120);
        assert_eq!(p.take_exit(), None);
    }

    #[test]
    fn guests_never_get_hungry_or_go_into_the_house() {
        let w = world(vec![]);
        let mut p = guest(1, 1.0, Move::Walk, Side::Right, 1000.0);
        place(&mut p, &w, Owner::Ground(0), 900.0, State::Idle { remaining: 100.0, look: false });
        p.set_hungry(true);
        let mut said = Vec::new();
        for _ in 0..(60 * 120) {
            p.update(DT, &w);
            said.extend(p.take_speech());
            assert!(!matches!(p.state, State::Hungry { .. }));
        }
        assert!(!said.contains(&Speech::Hungry), "{said:?}");
        // 클릭하면 밥 달라 하지 않고 평소처럼 반응
        place(&mut p, &w, Owner::Ground(0), 900.0, State::Idle { remaining: 100.0, look: false });
        p.pointer_down(900.0, 1000.0);
        p.pointer_up(900.0, 1000.0);
        assert!(!p.take_food_request());
        assert!(!matches!(p.state, State::Hungry { .. }));
        p.go_home(Owner::Ground(0), 100.0);
        assert!(!p.is_going_home());
        run(&mut p, &w, 600);
        assert!(!p.is_housed());
    }

    #[test]
    fn hungry_pet_stops_being_hungry_when_it_becomes_a_guest() {
        let w = world(vec![]);
        let mut p = Pet::new(cfg(), 0.0, 0.0, 1);
        place(&mut p, &w, Owner::Ground(0), 900.0, State::Hungry { remaining: 3.0 });
        p.set_hungry(true);
        p.set_guest(Side::Left, 1000.0);
        assert!(!matches!(p.state, State::Hungry { .. }));
        p.pointer_down(900.0, 1000.0);
        p.pointer_up(900.0, 1000.0);
        assert!(!p.take_food_request());
    }

    #[test]
    fn housed_or_homeward_pets_do_not_exit() {
        let w = world(vec![]);
        // 하우스로 가는 길에 화면 끝 쪽으로 달려도 나가지 않는다 (문이 오른쪽 끝 가까이)
        for seed in 0..30 {
            let mut p = at_edge(&w, seed, 1.0, Move::Walk);
            p.set_exit_edges(true, true);
            p.go_home(Owner::Ground(0), 1900.0);
            run(&mut p, &w, 120);
            assert!(p.is_housed(), "seed {seed}: {:?}", p.state);
            assert_eq!(p.take_exit(), None);
            run(&mut p, &w, 600);
            assert_eq!(p.take_exit(), None);
        }
    }
}
