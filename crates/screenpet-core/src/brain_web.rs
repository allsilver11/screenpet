//! 🕷️ 거미인간 줄 놀이 — 거미집·그네·영웅 착지·끌어오기·🌀 휘둘러 던지기·🙃 거꾸로 매달리기 (pet.json `web_slinger` + WEB_ANIMS — PetConfig::slings_webs) — brain.rs의 자식 모듈. 숫자·계산은 web.rs.
//!
//! - 🕸️ 거미집: 모서리를 보고 줄을 쏨 (shoot) → 줄 끝이 닿는 순간 take_web이 WebSpin을 한 번 돌려줌 (앱이 그 모서리에 거미집을 그림).
//!   크기는 제각각 (sized_web)이고, 첫 줄 뒤에도 거미집 여기저기로 줄을 몇 번 더 휙휙 쏴서 짬 (Weave — 앱이 그동안 살·고리를 차례로 그림).
//!   손님으로 친구 화면에 가 있으면 자주, 집에서는 가끔.
//!   🕸️⭕ 가끔 (WebMood.orb_odds = web::ORB_CHANCE) 구석 대신 손이 닿는 아무 자리 (화면이나 창 위 — web::orb_fit)에 둥근 거미집 (orb_web):
//!   한 화면에 web::MAX_ORBS개까지 (앱이 알려 준 거미집으로 셈), 사방으로 살 → 고리 차례로 짬
//! - 🌉 그네: 머리 위 창 아랫변·화면 윗변에 줄을 붙이고 (shoot) → 줄을 당겨 몸이 떠오름 (Zip) → 진자 (web_swing) → 반대편에서 놓고 날아감.
//!   🌉🌉 줄 잇기: 놓고 날아가는 중 (State::Air) 꼭대기 언저리에서 앞 위로 새 줄을 쏴 ("thwip!" / 가끔 "woohoo!") Zip 없이
//!   지금 자리·속도 그대로 진자를 이어 감 (try_chain_swing) — 한 번에 web::chain_range번, 닻이 없으면 그냥 떨어져 영웅 착지.
//! - 🦸 영웅 착지: 몸 크기 HERO_FALL_SIZES배 넘게 떨어져 내려서면 콰당 대신 hero_land ("nailed it!") — land_or_recover가 부름.
//! - 🪝 끌어오기: 바닥의 물건(ObjectPlay::Yank — 앱이 날림)이나 다른 동물(sync_lifts가 be_yanked)에 줄을 붙여 휙 (web_pull).
//!   반은 앞에 떨어지게 끌어오고 반은 머리 너머 뒤로 던짐. 💪 거대한 것도 — 무거워서 조금만 끌려와 그 쪽 앞에 (giant_landing)
//! - 🌀 휘둘러 던지기: 내 동물이고 친구가 연결된 끝이 있으면 (friend_sides) 물건은 자주 줄로 낚아 머리 위로 빙빙 (web_whirl) →
//!   그 끝 너머로 휙. 도는 동안은 흐물이가 든 물건처럼 carried_object·carry_point로 앱이 옮기고, 놓을 때 JuggleEvent::Release
//!   (앱이 날림 — 끝을 넘으면 친구 모니터로). 낚아채이면 (lose_carry) "hmph!", 끼어들리면 줄을 놓아 물건이 떨어짐.
//!   💪 거대한 것은 더 천천히·작게·덜 돌다 (web::whirl_secs(true)) 가까이만 던짐 (throw_target — 먼 끝·동물이면 그 앞 바닥에)
//! - 💪🕷️ 거대한 것 끌고 가기 (v0.7.1): 낚은 거대한 것의 친구 끝이 던져 닿지 않게 멀고 (GIANT_THROW_REACH 너머) 그 친구가 거대한 것을 받으면
//!   (giant_sides — 앱의 set_giant_edges) 휘두르지 않고 엉덩이(실젖)에서 나온 줄에 매달아 몸 뒤 바닥으로 질질 끌며 (Drag, drag_point) 그 끝으로 걸어 나감.
//!   juggle.carry에 들고 있는 것으로 쳐서 앱이 Visit.carry에 실음 (거대한 것까지 다 나가야 사라짐 — brain.rs transit_half).
//!   친구 화면엔 끌고 들어와 (hold_giant) 거대한 것까지 다 들어와 서면 줄을 풀어 그 자리에 내려놓음 ("tada!" — settle_drag). 끝이 막히면 그 자리에 놓음
//! - 🐛 고치 말기: 닿는 거리에 쉬는 다른 동물(친구 동물 포함)이 있으면 가끔 (COCOON_RATE) 그 몸 둘레로 줄을 COCOON_SHOTS번 쏴서 (Wrap)
//!   칭칭 — 첫 줄이 닿으면 sync_cocoons가 그 동물을 감고 (brain_snare.rs), 다 감으면 "hehe".
//! - 🕸️🧗 거미집 놀이: 몸 WEB_PLAY_REACH배 안에 보이는 거미집 (누구 것이든 — 앱이 set_webs로 알려 줌)이 있으면 가끔 (WEB_PLAY_RATE)
//!   그 위로 줄을 쏴 붙고 (Shoot → Crawl) 살·고리 자리 (web::perch_point)를 몇 군데 기어 다님 (climb 그림) → 매달려 (Dangle, web_swing 그림)
//!   가끔 통통 출렁 → 뚝 떨어짐 (높으면 영웅 착지). 거미집이 옮겨 가면 따라가고, 사라지면 떨어짐. 거미집은 안 찢음
//! - 🌉🚪 친구 화면으로: 친구가 연결된 옆 끝으로 걸어 나가려 할 때 (start_web_exit) 그 끝 바로 안쪽 위에 줄을 붙여 (web::exit_anchor)
//!   뒤로 당겨 올랐다가 흔들려 끝 너머로 휙 — 날아가다 끝을 넘으면 (web_exit_out) 던져 넘어간 것처럼 사라짐 (take_exit 한 번 + take_fling).
//!   닻이 없으면 평소처럼 걸어 나감. 넘어온 쪽 화면에서는 (web_arrival — enter_flung) 줄을 한 번 쏴 그네 타고 들어와 영웅 착지.
//!   위·아래 끝은 평소대로 (brain_vert.rs).
//! - 🙃 거꾸로 매달리기: 한가할 때 몇 분에 한 번 (web::HANG_RATE × 연령대 세기) 바로 머리 위 (창 아랫변·창 면·화면 윗변 — web::hang_anchors)에
//!   줄을 쏴 붙이고 (Shoot) 당겨 올라가 (HangRise, web_swing 그림) 뒤집혀 줄을 타고 스르륵 (HangSlide) — 머리가 바닥에서 몸 HANG_HEAD_CLEAR배
//!   높이에서 살랑살랑 매달림 (Hang, hang_down 그림 — 웅크린 채 두 손·두 발로 줄을 잡음). 가끔 가까운 펫이나 커서 쪽을 보며 "hey there".
//!   끝나면 몸을 뒤집어 사뿐 내려앉거나 (낮아서 영웅 착지 없음) 줄을 타고 다시 올라가 (HangClimb) 놓고 영웅 착지.
//!   줄은 닻 → 맨 위 손발 (grip.hang_down). 창에 붙은 줄은 창을 따라가고 창이 닫히면 놓음, 끼어들면 (잡기·자리 비움·조작 모드 등) 놓고 떨어짐
//! - 🎮 조작 모드에선 ↑↑로 줄을 쏴 그네를 직접 탐 (brain_feat.rs — 그동안 update_web은 손을 뗌, step_zip·swing_hand_offset을 빌려 씀).
//! - 지금 뻗은 줄은 web_line (앱이 덮개 창에 그림).
//!
//! 주사위는 자기 주사위(WebMood.rng)로 굴림 — 평소 동작과 다른 동물의 주사위 순서는 그대로.

use super::hoist_moves::PetSpot;
use super::*;
use crate::juggle::{JuggleEvent, ObjectSpot};
use crate::play::ObjectPlay;
use crate::web::{self, Anchor, HangLine, WebLine, WebSpin, WebSpot};

/// 줄을 쏜 목적
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum WebAim {
    /// 🕸️ 모서리에 거미집 (🕸️⭕ corner가 Center면 그 자리에 둥근 거미집)
    Corner(WebSpin),
    /// 🌉 그네 닻
    Anchor(Anchor),
    /// 🪝 바닥의 물건 (at: 줄이 붙는 점, to: 떨어질 자리, heavy: 💪 거대한 것)
    Object { id: u64, at: (f64, f64), to: (f64, f64), heavy: bool },
    /// 🪝 다른 동물 (key = social_key)
    Pet { key: u64, at: (f64, f64), to: (f64, f64) },
    /// 🌀 바닥의 물건을 낚아 휘둘러 side 끝 너머로 (at: 줄이 붙는 점 = 물건 가운데, half: 물건 반 폭, heavy: 💪 거대한 것)
    Lasso { id: u64, at: (f64, f64), half: f64, side: Side, heavy: bool },
    /// 🕸️🧗 거미집(web)의 k번째 자리 (at: 쏠 때 그 자리)에 붙으러
    Perch { web: u64, k: u32, at: (f64, f64) },
    /// 🙃 바로 머리 위 닻 (거꾸로 매달려 내려오려고)
    Hang(HangLine),
}

impl WebAim {
    /// 줄 끝이 닿을 점 (물리 px)
    pub fn tip(&self) -> (f64, f64) {
        match *self {
            WebAim::Corner(s) => (s.x, s.y),
            WebAim::Anchor(a) => (a.x, a.y),
            WebAim::Hang(l) => (l.x, l.y),
            WebAim::Object { at, .. } | WebAim::Pet { at, .. } | WebAim::Lasso { at, .. } | WebAim::Perch { at, .. } => at,
        }
    }
}

/// 줄 놀이 단계 (State::Web — t는 이 단계에 들어선 뒤 흐른 시간)
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum WebPhase {
    /// 줄 쏘기 (hit = 이미 닿았음)
    Shoot { aim: WebAim, hit: bool },
    /// 줄을 당겨 그네 시작 자리로 떠오름 (from = 시작한 발 자리)
    Zip { anchor: Anchor, from: (f64, f64) },
    /// 진자 (θ: 수직에서 진행 쪽 +, ω: 각속도)
    Swing { anchor: Anchor, theta: f64, omega: f64 },
    /// 줄을 끌어당김 (tip = 줄이 붙었던 점 — 손으로 감겨 들어옴)
    Pull { tip: (f64, f64) },
    /// 🕸️ 거미집 짜기: shot번째 줄을 쏘는 중 (web::weave_target으로 노림 — 첫 줄은 Shoot)
    Weave { spot: WebSpin, shot: u32 },
    /// 🌀 줄 끝의 물건을 머리 위로 휘두름 (from: 낚은 순간 물건 가운데, half: 물건 반 폭, dir: 던질 쪽 ±1, heavy: 💪 거대한 것 — 천천히·작게)
    Whirl { id: u64, from: (f64, f64), half: f64, dir: f64, heavy: bool },
    /// 💪 줄에 매달은 거대한 것을 몸 뒤로 끌며 친구 끝(side)으로 걸어감 (from: 낚은 순간 거대한 것 바닥 가운데 — 처음엔 거기서 끌려옴)
    Drag { id: u64, from: (f64, f64), side: Side },
    /// 🐛 다른 동물(key)을 고치로 감는 shot번째 줄 (web::cocoon_target으로 노림 — feet: 그 동물 발, size: 그 몸 크기)
    Wrap { key: u64, feet: (f64, f64), size: f64, shot: u32 },
    /// 🕸️🧗 거미집(web) 위를 from (발 자리)에서 k번째 자리로 기어 감 — left = 그 뒤 더 길 자리 수
    Crawl { web: u64, from: (f64, f64), k: u32, left: u32 },
    /// 🕸️🧗 거미집(web)에 매달려 있음 — rel = 모서리에서 발까지, bounce = 통통 출렁, len = 매달릴 시간
    Dangle { web: u64, rel: (f64, f64), bounce: bool, len: f64 },
    /// 🙃 줄을 당겨 닻 바로 아래로 올라감 (from = 시작한 발 자리)
    HangRise { line: HangLine, from: (f64, f64) },
    /// 🙃 뒤집혀 줄을 타고 내려옴 (발 y: from_y → to_y)
    HangSlide { line: HangLine, from_y: f64, to_y: f64 },
    /// 🙃 거꾸로 매달려 살랑 — len = 매달릴 시간, greet = 인사하며 볼 쪽 (±1, 인사하면 None), climb = 끝에 다시 올라감
    Hang { line: HangLine, len: f64, greet: Option<f64>, climb: bool },
    /// 🙃 줄을 타고 다시 올라감 (발 y: from_y → 손발이 닻에 닿는 자리) — 다 오르면 놓음
    HangClimb { line: HangLine, from_y: f64 },
}

/// 줄 놀이의 기억
#[derive(Debug, Clone)]
pub struct WebMood {
    rng: fastrand::Rng,
    /// 이 시각(clock)부터 거미집·그네·끌어오기를 할 수 있음
    pub(super) web_ready_at: f64,
    pub(super) swing_ready_at: f64,
    pub(super) pull_ready_at: f64,
    pub(super) cocoon_ready_at: f64,
    /// 🕸️🧗 이 시각(clock)부터 거미집 놀이를 할 수 있음
    pub(super) play_ready_at: f64,
    /// 🙃 이 시각(clock)부터 거꾸로 매달리기를 할 수 있음
    pub(super) hang_ready_at: f64,
    /// 🙃 매달리러 떠난 디딤판 높이 (발 y — 머리를 이보다 HANG_HEAD_CLEAR 위에 둠)
    hang_floor: f64,
    /// 🌉🚪 친구 화면으로 나가려고 탄 그네의 끝 (놓기 전)
    exit_side: Option<Side>,
    /// 🌉🚪 그네를 놓고 날아가는 중: 이 끝을 이 시각(clock)까지 넘으면 친구 화면으로
    exit_out: Option<(Side, f64)>,
    /// 새 거미집 (take_web으로 한 번)
    out: Option<WebSpin>,
    /// 🕸️⭕ 거미집을 칠 때 둥근 거미집을 칠 몫 (web::ORB_CHANCE — 테스트가 0으로 끄기도 함)
    pub(super) orb_odds: f64,
    /// 끌어온 동물 (social_key, vx, vy) — sync_lifts가 한 번 꺼냄
    pub(super) yank_pet: Option<(u64, f64, f64)>,
    /// 🌀 휘두른 물건을 던질 동물 (social_key) — 없으면 친구 끝 너머로
    whirl_at: Option<u64>,
    /// 🐛 첫 고치 줄이 닿은 동물 (social_key) — sync_cocoons가 한 번 꺼냄
    pub(super) cocoon_pet: Option<u64>,
    /// 같은 화면의 다른 동물 (notice_pets가 채움)
    pub(super) pets: Vec<PetSpot>,
    /// 영웅 착지 자세가 끝나는 시각
    pub(super) hero_until: f64,
    /// 🌉🌉 지금 그네 뒤에 더 이어 탈 그네 수
    pub(super) chain_left: u32,
    /// 🌉🌉 이 시각(clock)부터 다음 줄을 쏨
    chain_at: f64,
    /// 🌉🌉 이 시각까지 닻을 못 찾으면 잇기를 그만둠
    chain_until: f64,
    /// 🌉🌉 이어 가는 쪽 (±1)
    chain_dir: f64,
}

impl WebMood {
    /// 시드로 난수를 따로 둠. 모든 놀이는 FIRST_READY_SECS 뒤부터 (손님은 들어오자마자 거의 곧)
    pub(super) fn new(seed: u64) -> Self {
        WebMood {
            rng: fastrand::Rng::with_seed(seed ^ 0x7765_6273),
            web_ready_at: web::FIRST_READY_SECS,
            swing_ready_at: web::FIRST_READY_SECS * 4.0,
            pull_ready_at: web::FIRST_READY_SECS * 2.0,
            cocoon_ready_at: web::FIRST_READY_SECS * 3.0,
            play_ready_at: web::FIRST_READY_SECS * 2.0,
            hang_ready_at: web::FIRST_READY_SECS * 5.0,
            hang_floor: 0.0,
            exit_side: None,
            exit_out: None,
            out: None,
            orb_odds: web::ORB_CHANCE,
            yank_pet: None,
            whirl_at: None,
            cocoon_pet: None,
            pets: Vec::new(),
            hero_until: f64::NEG_INFINITY,
            chain_left: 0,
            chain_at: f64::INFINITY,
            chain_until: f64::NEG_INFINITY,
            chain_dir: 1.0,
        }
    }
}

impl Pet {
    /// 🕸️ 새 거미집 (줄 끝이 모서리에 닿을 때 한 번)
    pub fn take_web(&mut self) -> Option<WebSpin> {
        self.web.out.take()
    }

    /// 지금 뻗어 있는 줄 (손 → 줄 끝, 물리 px — 🙃 매달리기는 맨 위 손발 → 닻). 줄 놀이 중이 아니면 None
    pub fn web_line(&self) -> Option<WebLine> {
        // 💪 끄는 거대한 것: 엉덩이(실젖)에서 거대한 것 가운데로
        if let Some(line) = self.drag_line() {
            return Some(line);
        }
        let State::Web { phase, t } = self.state else { return None };
        let hand = self.web_hand();
        let lerp = |a: (f64, f64), b: (f64, f64), u: f64| (a.0 + (b.0 - a.0) * u, a.1 + (b.1 - a.1) * u);
        let to = match phase {
            WebPhase::Shoot { aim, hit } => lerp(hand, aim.tip(), if hit { 1.0 } else { (t / web::SHOOT_SECS).min(1.0) }),
            WebPhase::Zip { anchor, .. } | WebPhase::Swing { anchor, .. } => (anchor.x, anchor.y),
            WebPhase::HangRise { line, .. } | WebPhase::HangSlide { line, .. } | WebPhase::Hang { line, .. } | WebPhase::HangClimb { line, .. } => (line.x, line.y),
            WebPhase::Pull { tip } => lerp(tip, hand, (t / web::PULL_SECS).min(1.0)),
            WebPhase::Weave { spot, shot } => lerp(hand, web::weave_target(&spot, shot), (t / (0.5 * web::WEAVE_SHOT_SECS)).min(1.0)),
            WebPhase::Whirl { from, dir, heavy, .. } => self.whirl_spot(from, dir, heavy, t),
            WebPhase::Drag { .. } => return None,
            WebPhase::Wrap { feet, size, shot, .. } => lerp(hand, web::cocoon_target(feet, size, shot), (t / (0.5 * web::WEAVE_SHOT_SECS)).min(1.0)),
            // 🕸️🧗 거미집 위에서는 줄이 따로 없음 (거미집이 곧 줄)
            WebPhase::Crawl { .. } | WebPhase::Dangle { .. } => return None,
        };
        Some(WebLine { from: hand, to })
    }

    /// 🪝 거미인간 줄에 끌려 (vx, vy)로 날아옴 (물리 px/s) — 들 수 있는 동물만 (lift_ok). 날아왔으면 true
    pub fn be_yanked(&mut self, vx: f64, vy: f64) -> bool {
        if !self.lift_ok() {
            return false;
        }
        self.pacing.after_beat = None;
        self.clear_hover();
        self.poop.stepping_off();
        self.standing = None;
        self.web.chain_left = 0;
        self.body.vx = vx;
        self.body.vy = vy;
        if vx.abs() > 1.0 {
            self.facing = vx.signum();
        }
        self.thrown = false;
        self.gentle_drop = true;
        self.speech = Some(Speech::Whoa);
        self.state = State::Air { anim: Anim::Fall };
        true
    }

    /// 🦸 영웅 착지 자세 중 (Land 상태이고 아직 HERO_SECS 안)
    pub(super) fn hero_landing(&self) -> bool {
        // 그림이 있는 거미인간만 (영웅 착지 중에 다른 종이 되면 평소 착지)
        self.cfg.slings_webs && matches!(self.state, State::Land { .. }) && self.clock < self.web.hero_until
    }

    /// land_or_recover에서: 이만큼(fell, 물리 px) 떨어져 내려섰으면 영웅 착지를 함. 했으면 true (콰당은 없음)
    pub(super) fn try_hero_landing(&mut self, fell: f64) -> bool {
        if !self.cfg.slings_webs || self.is_user_away() || fell < web::HERO_FALL_SIZES * self.cfg.size {
            return false;
        }
        self.web.hero_until = self.clock + web::HERO_SECS;
        self.speech = Some(Speech::HeroLand);
        true
    }

    /// 손 자리 (물리 px): 지금 그림의 grip (shoot·web_swing·web_pull·web_whirl·🙃 hang_down), 왼쪽을 보면 좌우 반전
    fn web_hand(&self) -> (f64, f64) {
        let g = &self.cfg.grip;
        let (gx, gy) = match self.state {
            State::Web { phase: WebPhase::Shoot { .. } | WebPhase::Weave { .. } | WebPhase::Wrap { .. }, t } => g.shoot.at(t),
            State::Web { phase: WebPhase::Zip { .. } | WebPhase::Swing { .. } | WebPhase::HangRise { .. }, t } => g.web_swing.at(t),
            State::Web { phase: WebPhase::HangSlide { .. } | WebPhase::Hang { .. } | WebPhase::HangClimb { .. }, t } => g.hang_down.at(t),
            State::Web { phase: WebPhase::Pull { .. }, t } => g.web_pull.at(t),
            State::Web { phase: WebPhase::Whirl { .. }, t } => g.web_whirl.at(t),
            _ => g.shoot.at(1.0),
        };
        let gx = if self.facing < 0.0 { -gx } else { gx };
        let (_, top) = self.window_pos();
        (self.body.x + gx * self.cfg.size, top + gy * self.cfg.size)
    }

    /// 그네 그림의 손에서 발까지 (가로 — 진행 쪽 +, 세로 — 아래 +, 물리 px)
    pub(super) fn swing_hand_offset(&self) -> (f64, f64) {
        let (gx, gy) = self.cfg.grip.web_swing.at(0.0);
        (gx * self.cfg.size, (1.0 - gy) * self.cfg.size)
    }

    /// update에서 (다른 동물 놀이 뒤). 이번 틱을 여기서 처리했으면 true
    pub(super) fn update_web(&mut self, dt: f64, world: &World) -> bool {
        if !self.cfg.slings_webs {
            return false;
        }
        // 🎮 ↑↑로 쏜 줄은 조작 모드가 몲 (brain_feat.rs)
        if self.feat_on() == Some(super::feat_moves::Feat::Swing) {
            return false;
        }
        self.settle_drag();
        if let State::Web { phase, t } = self.state {
            self.step_web(dt, world, phase, t);
            return true;
        }
        if self.web.chain_left > 0 {
            if matches!(self.state, State::Air { anim: Anim::Fall }) {
                if self.try_chain_swing(world) {
                    return true;
                }
            } else {
                self.web.chain_left = 0;
            }
        }
        if !self.play_free() || self.romp_interrupted() {
            return false;
        }
        self.maybe_pull(dt, world) || self.maybe_cocoon(dt, world) || self.maybe_play_web(dt) || self.maybe_spin_web(dt, world) || self.maybe_web_swing(dt, world)
            || self.maybe_hang_down(dt, world)
    }

    /// 🕸️🧗 놀러 갈 만한 거미집: 보이는 것 중 몸 가운데에서 거미집 가장자리까지 WEB_PLAY_REACH배 안
    fn playable_webs(&self) -> Vec<WebSpot> {
        let size = self.cfg.size;
        let (cx, cy) = (self.body.x, self.body.y - 0.5 * size);
        self.known_webs()
            .iter()
            .filter(|w| !w.hidden && w.size > 0.0 && ((w.x - cx).hypot(w.y - cy) - w.size).max(0.0) <= web::WEB_PLAY_REACH * size)
            .copied()
            .collect()
    }

    /// 🕸️🧗 가까이 보이는 거미집이 있으면 주사위: 거미집 놀이
    fn maybe_play_web(&mut self, dt: f64) -> bool {
        if self.clock < self.web.play_ready_at || self.known_webs().is_empty() {
            return false;
        }
        if self.playable_webs().is_empty() || self.web.rng.f64() >= web::WEB_PLAY_RATE * dt {
            return false;
        }
        self.start_web_play_now()
    }

    /// 테스트: 줄 놀이 주사위를 모두 끔 (start_*로만)
    #[cfg(test)]
    pub(crate) fn hush_web_play(&mut self) {
        self.web.web_ready_at = f64::INFINITY;
        self.web.swing_ready_at = f64::INFINITY;
        self.web.pull_ready_at = f64::INFINITY;
        self.web.cocoon_ready_at = f64::INFINITY;
        self.web.play_ready_at = f64::INFINITY;
        self.web.hang_ready_at = f64::INFINITY;
    }

    /// 🕸️🧗 가까운 거미집 하나에 줄을 쏴 놀러 감 (주사위 없이 — 테스트용). 거미집이 없으면 false
    #[cfg(test)]
    pub(super) fn start_web_play(&mut self, _world: &World) -> bool {
        self.start_web_play_now()
    }

    /// 🕸️🧗 놀 거미집과 처음 붙을 자리를 골라 줄을 쏨. 거미집이 없으면 false
    fn start_web_play_now(&mut self) -> bool {
        let webs = self.playable_webs();
        if webs.is_empty() {
            return false;
        }
        let w = webs[self.web.rng.usize(..webs.len())];
        let k = self.web.rng.u32(0..7);
        let at = web::perch_point(&w, k);
        self.web.play_ready_at = self.clock + behavior::range(&mut self.web.rng, web::WEB_PLAY_CALM);
        self.facing = if at.0 >= self.body.x { 1.0 } else { -1.0 };
        self.begin_shoot(WebAim::Perch { web: w.id, k, at });
        true
    }

    /// 🕸️🧗 거미집 위 자리 at (물리 px — 몸 가운데)에 몸을 둘 때의 발 자리 — 화면 안으로
    fn perch_feet(&self, world: &World, at: (f64, f64)) -> (f64, f64) {
        let (size, hw, b) = (self.cfg.size, self.cfg.half_w, world.bounds);
        let x = at.0.clamp(b.left + hw, (b.right - hw).max(b.left + hw));
        let y = (at.1 + 0.5 * size).clamp(b.top + size, b.bottom.max(b.top + size));
        (x, y)
    }

    /// 🕸️🧗 기어 가는 한 틱: 그 자리에 닿으면 다음 자리로, 다 기었으면 매달림. 거미집이 사라졌으면 떨어짐
    fn step_crawl(&mut self, world: &World, id: u64, from: (f64, f64), k: u32, left: u32, t: f64) {
        let Some(w) = self.known_webs().iter().find(|w| w.id == id).copied() else {
            self.start_fall();
            return;
        };
        let to = self.perch_feet(world, web::perch_point(&w, k));
        let u = (t / web::WEB_CRAWL_SECS).min(1.0);
        let ease = u * u * (3.0 - 2.0 * u);
        self.body.x = from.0 + (to.0 - from.0) * ease;
        self.body.y = from.1 + (to.1 - from.1) * ease;
        self.body.vx = 0.0;
        self.body.vy = 0.0;
        if (to.0 - from.0).abs() > 1.0 {
            self.facing = (to.0 - from.0).signum();
        }
        if u < 1.0 {
            self.state = State::Web { phase: WebPhase::Crawl { web: id, from, k, left }, t };
        } else if left > 0 {
            let next = (k + 1 + self.web.rng.u32(0..3)) % 7;
            self.state = State::Web { phase: WebPhase::Crawl { web: id, from: (self.body.x, self.body.y), k: next, left: left - 1 }, t: 0.0 };
        } else {
            let bounce = self.web.rng.f64() < web::WEB_BOUNCE_CHANCE;
            let len = behavior::range(&mut self.web.rng, web::WEB_DANGLE_SECS);
            if bounce {
                self.speech = Some(Speech::WebSwing);
            }
            self.state = State::Web { phase: WebPhase::Dangle { web: id, rel: (self.body.x - w.x, self.body.y - w.y), bounce, len }, t: 0.0 };
        }
    }

    /// 🕸️🧗 매달린 한 틱: 거미집을 따라가며 (통통이면 출렁출렁), 시간이 다 되면 뚝 떨어짐 (높으면 영웅 착지). 거미집이 사라지면 떨어짐
    fn step_dangle(&mut self, id: u64, rel: (f64, f64), bounce: bool, len: f64, t: f64) {
        let Some(w) = self.known_webs().iter().find(|w| w.id == id).copied() else {
            self.start_fall();
            return;
        };
        let bob = if bounce { web::snag_spring((0.0, 1.0), web::SNAG_STRETCH_PET * self.cfg.size, t % web::SNAG_SPRING_SECS).1 } else { 0.0 };
        self.body.x = w.x + rel.0;
        self.body.y = w.y + rel.1 + bob;
        self.body.vx = 0.0;
        self.body.vy = 0.0;
        if t < len {
            self.state = State::Web { phase: WebPhase::Dangle { web: id, rel, bounce, len }, t };
            return;
        }
        self.thrown = false;
        self.gentle_drop = false;
        self.state = State::Air { anim: Anim::Fall };
    }

    /// 🌉🚪 친구가 연결된 옆 끝(side)으로 나가려 함: 걸어 나가는 대신 그 끝 바로 안쪽 위에 줄을 붙여 그네로 넘어감 (거미인간만, 손이 비었을 때).
    /// 닻이 없으면 false (평소처럼 걸어 나감)
    pub(super) fn start_web_exit(&mut self, side: Side, world: &World) -> bool {
        if !self.cfg.slings_webs || visit::is_vertical(side) || self.is_controlled() || self.is_user_away() || self.standing.is_none() {
            return false;
        }
        if self.carried_object().is_some() || self.hoisting().is_some() {
            return false;
        }
        let dir = visit::dir(side);
        let (gx, drop) = self.swing_hand_offset();
        let hand = (self.body.x + dir * gx, self.body.y - drop);
        let floor = world.work_area_at(self.body.x, self.body.y - 1.0).bottom;
        let Some(anchor) = web::exit_anchor(world, hand, dir, self.cfg.size, drop, floor) else { return false };
        self.web.chain_left = 0;
        self.facing = dir;
        self.begin_shoot(WebAim::Anchor(anchor));
        self.web.exit_side = Some(side);
        true
    }

    /// 🌉🚪 update_air에서 (던지기 넘기기와 같은 자리): 나가려고 탄 그네를 놓고 날아가다 다음 x(next_x)가 그 끝 [left, right]를 넘으면
    /// 친구 화면으로 넘어감 (take_exit 한 번 + take_fling). 넘어갔으면 true
    pub(super) fn web_exit_out(&mut self, world: &World, next_x: f64, left: f64, right: f64) -> bool {
        let Some((side, until)) = self.web.exit_out else { return false };
        if self.clock > until || self.thrown || self.inside.is_some() {
            self.web.exit_out = None;
            return false;
        }
        let crossing = match side {
            Side::Left => next_x < left && self.body.vx < 0.0,
            Side::Right => next_x > right && self.body.vx > 0.0,
            Side::Up | Side::Down => false,
        };
        if !crossing {
            return false;
        }
        self.web.exit_out = None;
        if self.exit_odds(side) <= 0.0 {
            return false;
        }
        self.fling_over(world, side);
        true
    }

    /// 🌉🚪 친구 화면에서 날아 들어옴 (enter_flung이 부름 — 거미인간만): 던져진 게 아니라 곧 안쪽(inward) 위로 줄을 한 번 쏴 그네 타고 들어옴
    pub(super) fn web_arrival(&mut self, inward: f64) {
        if !self.cfg.slings_webs {
            return;
        }
        self.thrown = false;
        self.thrown_down = false;
        self.web.exit_side = None;
        self.web.exit_out = None;
        self.web.chain_left = 1;
        self.web.chain_at = self.clock + behavior::range(&mut self.web.rng, web::CHAIN_DELAY);
        self.web.chain_until = self.web.chain_at + web::ARRIVAL_CHAIN_WINDOW;
        self.web.chain_dir = inward;
    }


    /// 🙃 한가할 때 주사위 (몇 분에 한 번 — 연령대 세기를 곱함): 거꾸로 매달리기
    fn maybe_hang_down(&mut self, dt: f64, world: &World) -> bool {
        if self.clock < self.web.hang_ready_at || self.web.rng.f64() >= web::HANG_RATE * self.power() * dt {
            return false;
        }
        self.start_hang_down(world)
    }

    /// 🙃 hang_down 그림의 맨 위 손발 (줄이 붙는 점): 몸 가운데에서 가로 (보는 쪽 +), 그림 윗변에서 세로 (몸 크기 배)
    fn hang_grip(&self) -> (f64, f64) {
        self.cfg.grip.hang_down.at(0.0)
    }

    /// 🙃 손발이 이 높이(grip_y)에 오게 매달렸을 때의 발 y (그림 아래 끝 — 거꾸로라 머리)
    fn hang_feet_y(&self, grip_y: f64) -> f64 {
        grip_y + (1.0 - self.hang_grip().1) * self.cfg.size
    }

    /// 🙃 바로 머리 위 닻 (창 아랫변·창 면·화면 윗변 중 하나)에 줄을 쏨 (주사위 없이). 닻이 없으면 false
    pub(super) fn start_hang_down(&mut self, world: &World) -> bool {
        if self.standing.is_none() {
            return false;
        }
        let size = self.cfg.size;
        let (gx, gy) = self.hang_grip();
        let x = self.body.x + self.facing * gx * size;
        // 다 내려왔을 때 손발 높이 — 닻은 그보다 HANG_MIN_LINE 위
        let lowest = self.body.y - web::HANG_HEAD_CLEAR.1 * size - (1.0 - gy) * size - web::HANG_MIN_LINE * size;
        let pick = self.web.rng.f64();
        let lines = web::hang_anchors(world, x, lowest, pick);
        if lines.is_empty() {
            return false;
        }
        let line = lines[self.web.rng.usize(..lines.len())];
        self.web.hang_ready_at = self.clock + behavior::range(&mut self.web.rng, web::HANG_CALM);
        self.web.chain_left = 0;
        self.web.hang_floor = self.body.y;
        self.begin_shoot(WebAim::Hang(line));
        true
    }

    /// 🙃 거꾸로 매달리기 한 틱: 창을 따라 닻을 옮기고 (창이 닫히면 놓음), 단계대로 올라감 → 내려옴 → 살랑 → 내려앉기·다시 올라감
    fn step_hang(&mut self, world: &World, phase: WebPhase, t: f64) {
        let line = match phase {
            WebPhase::HangRise { line, .. } | WebPhase::HangSlide { line, .. } | WebPhase::Hang { line, .. } | WebPhase::HangClimb { line, .. } => line,
            _ => return,
        };
        let Some(line) = line.follow(world) else {
            self.start_fall();
            return;
        };
        let size = self.cfg.size;
        let ease = |u: f64| u * u * (3.0 - 2.0 * u);
        self.body.vx = 0.0;
        self.body.vy = 0.0;
        match phase {
            WebPhase::HangRise { from, .. } => {
                let (gx, drop) = self.swing_hand_offset();
                let to = (line.x - self.facing * gx, line.y + drop);
                let u = (t / web::HANG_RISE_SECS).min(1.0);
                let k = 1.0 - (1.0 - u) * (1.0 - u);
                self.body.x = from.0 + (to.0 - from.0) * k;
                self.body.y = from.1 + (to.1 - from.1) * k;
                if u < 1.0 {
                    self.state = State::Web { phase: WebPhase::HangRise { line, from }, t };
                    return;
                }
                // 뒤집어 손발로 줄을 잡음 — 이제 머리가 아래
                let from_y = self.hang_feet_y(line.y);
                let k = behavior::range(&mut self.web.rng, web::HANG_HEAD_CLEAR);
                let to_y = (self.web.hang_floor - k * size).max(from_y);
                self.place_hanging(line, from_y, 0.0);
                self.state = State::Web { phase: WebPhase::HangSlide { line, from_y, to_y }, t: 0.0 };
            }
            WebPhase::HangSlide { from_y, to_y, .. } => {
                let u = (t / web::HANG_SLIDE_SECS).min(1.0);
                self.place_hanging(line, from_y + (to_y - from_y) * ease(u), 0.0);
                if u < 1.0 {
                    self.state = State::Web { phase: WebPhase::HangSlide { line, from_y, to_y }, t };
                    return;
                }
                let len = behavior::range(&mut self.web.rng, web::HANG_SECS);
                let greet = self.hang_greet_dir();
                let climb = self.web.rng.f64() < web::HANG_CLIMB_CHANCE;
                self.state = State::Web { phase: WebPhase::Hang { line, len, greet, climb }, t: 0.0 };
            }
            WebPhase::Hang { len, mut greet, climb, .. } => {
                if let Some(dir) = greet.filter(|_| t >= web::HANG_GREET_AT) {
                    self.facing = dir;
                    self.speech = Some(Speech::HangHey);
                    greet = None;
                }
                let sway = web::HANG_SWAY * size * (std::f64::consts::TAU * t / web::HANG_SWAY_SECS).sin();
                let y = self.body.y;
                self.place_hanging(line, y, sway);
                if t < len {
                    self.state = State::Web { phase: WebPhase::Hang { line, len, greet, climb }, t };
                } else if climb {
                    self.state = State::Web { phase: WebPhase::HangClimb { line, from_y: y }, t: 0.0 };
                } else {
                    // 몸을 뒤집어 사뿐 내려앉음 (머리 높이가 낮아 영웅 착지·콰당 없음)
                    self.place_hanging(line, y, 0.0);
                    self.thrown = false;
                    self.gentle_drop = true;
                    self.state = State::Air { anim: Anim::Fall };
                }
            }
            WebPhase::HangClimb { from_y, .. } => {
                let top = self.hang_feet_y(line.y);
                let u = (t / web::HANG_CLIMB_SECS).min(1.0);
                self.place_hanging(line, from_y + (top - from_y) * ease(u), 0.0);
                if u < 1.0 {
                    self.state = State::Web { phase: WebPhase::HangClimb { line, from_y }, t };
                    return;
                }
                // 다 올라가 놓고 떠남 (높이서 떨어지니 영웅 착지)
                self.thrown = false;
                self.gentle_drop = false;
                self.state = State::Air { anim: Anim::Fall };
            }
            _ => {}
        }
    }

    /// 🙃 맨 위 손발이 닻 바로 아래 (sway만큼 옆으로 흔들려) 오게, 발 y(feet_y — 거꾸로라 머리)에 몸을 둠
    fn place_hanging(&mut self, line: HangLine, feet_y: f64, sway: f64) {
        let gx = self.hang_grip().0 * self.cfg.size;
        self.body.x = line.x - self.facing * gx + sway;
        self.body.y = feet_y;
    }

    /// 🙃 매달린 채 인사할 쪽 (±1): HANG_GREET_CHANCE로, 가까운 다른 펫 (가장 가까운 것) 쪽, 없으면 가까운 커서 쪽. 안 하면 None
    fn hang_greet_dir(&mut self) -> Option<f64> {
        if self.web.rng.f64() >= web::HANG_GREET_CHANCE {
            return None;
        }
        let reach = web::HANG_GREET_REACH * self.cfg.size;
        let (x, y) = (self.body.x, self.body.y);
        let pal = self
            .web
            .pets
            .iter()
            .filter(|s| s.key != self.social_key && (s.x - x).abs() > 1.0 && (s.x - x).hypot(s.y - y) <= reach)
            .min_by(|a, b| (a.x - x).abs().total_cmp(&(b.x - x).abs()))
            .map(|s| s.x);
        let cursor = Some(self.cursor).filter(|c| (c.0 - x).abs() > 1.0 && (c.0 - x).hypot(c.1 - y) <= reach).map(|c| c.0);
        pal.or(cursor).map(|tx| (tx - x).signum())
    }

    /// 🐛 닿는 거리에 쉬는 동물이 있으면 주사위: 고치 말기
    fn maybe_cocoon(&mut self, dt: f64, world: &World) -> bool {
        if self.clock < self.web.cocoon_ready_at {
            return false;
        }
        let pets = self.cocoon_targets(world);
        if pets.is_empty() || self.web.rng.f64() >= web::COCOON_RATE * self.power() * dt {
            return false;
        }
        let k = self.web.rng.usize(..pets.len());
        self.start_cocoon(&pets[k], world)
    }

    /// 🐛 고치를 말 수 있는 동물: 거미인간만, 같은 모니터 COCOON_REACH 안에 디딤판 위에서 쉬거나 걷는 동물 (들 수 있는 — lift_ok)
    pub(super) fn cocoon_targets(&self, world: &World) -> Vec<PetSpot> {
        if !self.cfg.slings_webs {
            return Vec::new();
        }
        let size = self.cfg.size;
        let (lo, hi) = (web::COCOON_REACH.0 * size, web::COCOON_REACH.1 * size);
        let wa = world.work_area_at(self.body.x, self.body.y - 1.0);
        self.web
            .pets
            .iter()
            .filter(|s| s.key != self.social_key && s.liftable && s.owner.is_some())
            .filter(|s| (lo..=hi).contains(&(s.x - self.body.x).hypot(s.y - self.body.y)) && world.work_area_at(s.x, s.y - 1.0) == wa)
            .copied()
            .collect()
    }

    /// 🐛 이 동물 몸 둘레로 고치 줄을 쏘기 시작함 (주사위 없이)
    pub(super) fn start_cocoon(&mut self, spot: &PetSpot, _world: &World) -> bool {
        self.facing = if spot.x >= self.body.x { 1.0 } else { -1.0 };
        self.web.cocoon_ready_at = self.clock + behavior::range(&mut self.web.rng, web::COCOON_CALM);
        self.pacing.after_beat = None;
        self.clear_hover();
        self.speech = Some(Speech::Thwip);
        self.state = State::Web { phase: WebPhase::Wrap { key: spot.key, feet: (spot.x, spot.y), size: spot.size, shot: 0 }, t: 0.0 };
        true
    }

    /// 🐛 고치 줄 한 번 (WEAVE_SHOT_SECS): 첫 줄이 닿는 순간 그 동물을 감음 (sync_cocoons), 다 쐈으면 "hehe" 하고 쉼
    fn step_wrap(&mut self, dt: f64, key: u64, feet: (f64, f64), size: f64, shot: u32, t: f64) {
        let land = 0.5 * web::WEAVE_SHOT_SECS;
        if shot == 0 && t >= land && t - dt < land {
            self.web.cocoon_pet = Some(key);
        }
        if t < web::WEAVE_SHOT_SECS {
            self.state = State::Web { phase: WebPhase::Wrap { key, feet, size, shot }, t };
        } else if shot + 1 < web::COCOON_SHOTS {
            self.state = State::Web { phase: WebPhase::Wrap { key, feet, size, shot: shot + 1 }, t: 0.0 };
        } else {
            self.speech = Some(Speech::Hehe);
            self.state = self.rest_beat();
        }
    }

    /// 🐛 감고 있는 동물 (social_key) — 첫 줄이 닿은 뒤 (shot ≥ 1)만
    pub(super) fn wrapping(&self) -> Option<u64> {
        match self.state {
            State::Web { phase: WebPhase::Wrap { key, shot, .. }, .. } if shot >= 1 => Some(key),
            _ => None,
        }
    }

    /// 🐛 감던 동물을 못 감았거나 풀려났음: 줄을 거두고 쉼
    pub(super) fn lose_wrap(&mut self) {
        if matches!(self.state, State::Web { phase: WebPhase::Wrap { .. }, .. }) {
            self.state = self.rest_beat();
        }
    }

    /// 🕸️ 한가할 때 주사위 (손님이면 자주): 가까운 모서리에 거미집
    fn maybe_spin_web(&mut self, dt: f64, world: &World) -> bool {
        let rate = if self.guest.is_some() { web::WEB_RATE_GUEST } else { web::WEB_RATE_HOME };
        if self.clock < self.web.web_ready_at || self.web.rng.f64() >= rate * dt {
            return false;
        }
        self.start_web_spin(world)
    }

    /// 🕸️ 가까운 모서리 하나를 골라 (orb_odds로 🕸️⭕ 손이 닿는 아무 자리에 둥근 거미집) 줄을 쏨 (주사위 없이). 칠 자리가 없으면 false
    pub(super) fn start_web_spin(&mut self, world: &World) -> bool {
        let size = self.cfg.size;
        let at = (self.body.x, self.body.y - 0.6 * size);
        let orb = if self.web.rng.f64() < self.web.orb_odds { self.orb_web(world, at) } else { None };
        let spot = match orb {
            Some(spot) => spot,
            None => {
                let spots = web::corner_spots(world, at, web::WEB_REACH * size, size);
                if spots.is_empty() {
                    return false;
                }
                let spot = spots[self.web.rng.usize(..spots.len())];
                self.sized_web(spot, world)
            }
        };
        let calm = if self.guest.is_some() { web::WEB_CALM_GUEST } else { web::WEB_CALM_HOME };
        self.web.web_ready_at = self.clock + behavior::range(&mut self.web.rng, calm);
        self.facing = if spot.x >= self.body.x { 1.0 } else { -1.0 };
        self.begin_shoot(WebAim::Corner(spot));
        true
    }

    /// 🕸️ 칠 거미집의 크기 (몸집과 상관없이 WEB_RADIUS_LOGICAL — 창 모서리는 창 짧은 변의 WEB_WINDOW_FIT배까지)와 짤 줄 횟수·시간을 정함
    pub(super) fn sized_web(&mut self, spot: WebSpin, world: &World) -> WebSpin {
        // 크기는 몸집·연령대와 상관없이 (논리 px × 화면 배율) — 🌱 연령대는 짜는 줄 수만 (shot_range)
        let mut size = behavior::range(&mut self.web.rng, web::WEB_RADIUS_LOGICAL) * self.cfg.dpi;
        if let Some(r) = spot.window.and_then(|id| world.windows.iter().find(|w| w.id == id)).map(|w| w.rect) {
            size = size.min(web::WEB_WINDOW_FIT * r.width().min(r.height()));
        }
        self.woven(WebSpin { size, ..spot })
    }

    /// 🕸️ 거미집 크기에 맞춰 짤 줄 횟수·시간을 정함 (🌱 연령대 shot_range 안)
    fn woven(&self, spot: WebSpin) -> WebSpin {
        let k = spot.size / self.cfg.dpi / web::WEB_RADIUS_PER_SHOT;
        let (lo, hi) = web::shot_range(self.power());
        let shots = ((2.0 + k).round() as u32).clamp(lo, hi);
        WebSpin { shots, build: f64::from(shots - 1) * web::WEAVE_SHOT_SECS, ..spot }
    }

    /// 🕸️⭕ 손(at)이 닿는 아무 자리에 칠 둥근 거미집 (반지름 ORB_RADIUS_LOGICAL — 몸집과 상관없음): 손에서 orb_reach 안을 ORB_TRIES번 찔러 보고
    /// web::orb_fit이 맞는 첫 자리 (화면 위면 window None, 창 위면 그 창). 그 화면에 둥근 거미집이 MAX_ORBS개면 None
    fn orb_web(&mut self, world: &World, at: (f64, f64)) -> Option<WebSpin> {
        let size = self.cfg.size;
        let wa = world.work_area_at(at.0, at.1);
        let orbs = self.known_webs().iter().filter(|w| w.corner.is_orb() && world.work_area_at(w.x, w.y) == wa).count();
        if orbs >= web::MAX_ORBS {
            return None;
        }
        let radius = behavior::range(&mut self.web.rng, web::ORB_RADIUS_LOGICAL) * self.cfg.dpi;
        for _ in 0..web::ORB_TRIES {
            let ang = self.web.rng.f64() * std::f64::consts::TAU;
            let d = (0.25 + 0.75 * self.web.rng.f64()) * web::orb_reach(size, radius);
            let (x, y) = (at.0 + d * ang.cos(), at.1 + d * ang.sin());
            if let Some(window) = web::orb_fit(world, at, x, y, radius) {
                return Some(self.woven(WebSpin { x, y, corner: web::Corner::Center, size: radius, window, shots: 1, build: 0.0 }));
            }
        }
        None
    }

    /// 🌉 한가할 때 주사위: 그네
    fn maybe_web_swing(&mut self, dt: f64, world: &World) -> bool {
        if self.clock < self.web.swing_ready_at || self.web.rng.f64() >= web::SWING_RATE * self.power() * dt {
            return false;
        }
        self.start_web_swing(world)
    }

    /// 🌉 앞(안 되면 뒤) 위로 닻을 찾아 줄을 쏨 (주사위 없이). 닻이 없으면 false
    pub(super) fn start_web_swing(&mut self, world: &World) -> bool {
        let (gx, drop) = self.swing_hand_offset();
        let floor = world.work_area_at(self.body.x, self.body.y - 1.0).bottom;
        for facing in [self.facing, -self.facing] {
            let hand = (self.body.x + facing * gx, self.body.y - drop);
            if let Some(anchor) = web::swing_anchor(world, hand, facing, self.cfg.size, drop, floor) {
                self.web.swing_ready_at = self.clock + behavior::range(&mut self.web.rng, web::SWING_CALM);
                let (lo, hi) = web::chain_range(self.power());
                self.web.chain_left = self.web.rng.u32(lo..=hi) - 1;
                self.facing = facing;
                self.begin_shoot(WebAim::Anchor(anchor));
                return true;
            }
        }
        false
    }

    /// 🌉🌉 그네를 놓고 날아가는 중 (update_web이 매 틱): CHAIN_DELAY가 지나면 앞 위로 닻을 찾아 줄을 쏘고 지금 자리·속도 그대로 진자를 이어 감.
    /// 시한(CHAIN_WINDOW)이 지나거나, 던져졌거나, 거꾸로 날거나, 자리 비움이면 잇기를 그만둠. 이었으면 true
    fn try_chain_swing(&mut self, world: &World) -> bool {
        if self.clock < self.web.chain_at {
            return false;
        }
        let dir = self.web.chain_dir;
        if self.clock > self.web.chain_until || self.thrown || self.inside.is_some() || self.is_user_away() || self.body.vx * dir <= 0.0 {
            self.web.chain_left = 0;
            return false;
        }
        let (gx, drop) = self.swing_hand_offset();
        let hand = (self.body.x + dir * gx, self.body.y - drop);
        let floor = world.work_area_at(self.body.x, self.body.y - 1.0).bottom;
        let Some(anchor) = web::chain_anchor(world, hand, dir, self.cfg.size, drop, floor) else { return false };
        let theta = -anchor.angle;
        let omega = web::rope_omega(anchor.len, theta, self.body.vx, self.body.vy, dir);
        if omega <= 0.0 {
            return false;
        }
        self.web.chain_left -= 1;
        self.facing = dir;
        self.standing = None;
        self.speech = Some(if self.web.rng.f64() < web::CHAIN_WOOHOO_CHANCE { Speech::WebSwing } else { Speech::Thwip });
        self.state = State::Web { phase: WebPhase::Swing { anchor, theta, omega }, t: 0.0 };
        true
    }

    /// 🪝 닿는 거리에 물건이나 동물이 있으면 주사위: 끌어오기
    fn maybe_pull(&mut self, dt: f64, world: &World) -> bool {
        if self.clock < self.web.pull_ready_at {
            return false;
        }
        let (objs, pets) = self.pull_targets(world);
        if objs.is_empty() && pets.is_empty() {
            return false;
        }
        if self.web.rng.f64() >= web::PULL_RATE * self.power() * dt {
            return false;
        }
        let k = self.web.rng.usize(..objs.len() + pets.len());
        if k < objs.len() {
            self.start_pull_object(&objs[k], world)
        } else {
            self.start_pull_pet(&pets[k - objs.len()], world)
        }
    }

    /// 끌어올 수 있는 것: 같은 모니터, PULL_REACH 안의 바닥 물건 (💪 거대한 것도 — 무거워서 조금만 끌려옴)과 들 수 있는 동물
    fn pull_targets(&self, world: &World) -> (Vec<ObjectSpot>, Vec<PetSpot>) {
        let size = self.cfg.size;
        let (lo, hi) = (web::PULL_REACH.0 * size, web::PULL_REACH.1 * size);
        let wa = world.work_area_at(self.body.x, self.body.y - 1.0);
        let ok = |x: f64, y: f64| {
            let d = (x - self.body.x).hypot(y - self.body.y);
            (lo..=hi).contains(&d) && world.work_area_at(x, y - 1.0) == wa
        };
        let objs = self.play.spots.iter().filter(|s| ok(s.x, s.y)).copied().collect();
        let pets = self.web.pets.iter().filter(|s| s.key != self.social_key && s.liftable && s.owner.is_some() && ok(s.x, s.y)).copied().collect();
        (objs, pets)
    }

    /// 끌어온 것이 떨어질 자리: 반은 앞, 반은 머리 너머 뒤 (화면 안으로)
    fn pull_landing(&mut self, from_x: f64, world: &World) -> (f64, f64) {
        let size = self.cfg.size;
        let dir = if from_x >= self.body.x { 1.0 } else { -1.0 };
        self.facing = dir;
        let x = if self.web.rng.bool() { self.body.x + dir * web::PULL_LAND_AHEAD * size } else { self.body.x - dir * web::FLING_LAND_BEHIND * size };
        let wa = world.work_area_at(self.body.x, self.body.y - 1.0);
        (x.clamp(wa.left + 0.5 * size, wa.right - 0.5 * size), self.body.y)
    }

    /// 🪝 이 물건에 줄을 쏨 (주사위 없이). 친구가 연결된 끝이나 던질 만한 동물이 있으면 WHIRL_CHANCE로 🌀 휘둘러 던지기
    /// (동물이 있으면 WHIRL_AT_PET_SHARE로 그 동물에게, 아니면 친구 끝 너머로)
    pub(super) fn start_pull_object(&mut self, spot: &ObjectSpot, world: &World) -> bool {
        let sides = self.friend_sides(world);
        let pets = self.whirl_pet_targets(world);
        if (!sides.is_empty() || !pets.is_empty()) && self.web.rng.f64() < web::WHIRL_CHANCE {
            if !pets.is_empty() && (sides.is_empty() || self.web.rng.f64() < web::WHIRL_AT_PET_SHARE) {
                let target = pets[self.web.rng.usize(..pets.len())];
                return self.start_whirl_at_pet(spot, &target);
            }
            let side = sides[self.web.rng.usize(..sides.len())];
            return self.start_whirl_object(spot, side);
        }
        let to = if spot.giant { self.giant_landing(spot) } else { self.pull_landing(spot.x, world) };
        self.web.pull_ready_at = self.clock + behavior::range(&mut self.web.rng, web::PULL_CALM);
        self.begin_shoot(WebAim::Object { id: spot.id, at: (spot.x, spot.y - spot.half_w), to, heavy: spot.giant });
        true
    }

    /// 💪 끌려온 거대한 것이 떨어질 자리: 그 쪽에서 거미인간 쪽으로 GIANT_PULL_MAX까지만, 거미인간과 GIANT_PULL_GAP 틈을 두고 (머리 너머로는 못 넘김)
    fn giant_landing(&mut self, spot: &ObjectSpot) -> (f64, f64) {
        let size = self.cfg.size;
        let dir = if spot.x >= self.body.x { 1.0 } else { -1.0 };
        self.facing = dir;
        let stop = self.body.x + dir * (spot.half_w + web::GIANT_PULL_GAP * size);
        let shift = ((spot.x - stop) * dir).clamp(0.0, web::GIANT_PULL_MAX * size);
        (spot.x - dir * shift, spot.y)
    }

    /// 🌀 휘두른 물건을 던질 만한 동물: 같은 모니터, 몸 WHIRL_PET_REACH배 안의 디딤판 위 다른 동물
    fn whirl_pet_targets(&self, world: &World) -> Vec<PetSpot> {
        let size = self.cfg.size;
        let (lo, hi) = (web::WHIRL_PET_REACH.0 * size, web::WHIRL_PET_REACH.1 * size);
        let wa = world.work_area_at(self.body.x, self.body.y - 1.0);
        self.web
            .pets
            .iter()
            .filter(|s| s.key != self.social_key && s.owner.is_some())
            .filter(|s| (lo..=hi).contains(&(s.x - self.body.x).abs()) && world.work_area_at(s.x, s.y - 1.0) == wa)
            .copied()
            .collect()
    }

    /// 🌀 이 물건에 줄을 쏨 — 닿으면 휘둘러 그 동물에게 던짐 (주사위 없이)
    pub(super) fn start_whirl_at_pet(&mut self, spot: &ObjectSpot, target: &PetSpot) -> bool {
        let side = if target.x >= self.body.x { Side::Right } else { Side::Left };
        self.start_whirl_object(spot, side);
        self.web.whirl_at = Some(target.key);
        true
    }

    /// 🌀 이 물건에 줄을 쏨 — 닿으면 휘둘러 side 끝 너머로 던짐 (주사위 없이)
    pub(super) fn start_whirl_object(&mut self, spot: &ObjectSpot, side: Side) -> bool {
        self.web.whirl_at = None;
        self.facing = if spot.x >= self.body.x { 1.0 } else { -1.0 };
        self.web.pull_ready_at = self.clock + behavior::range(&mut self.web.rng, web::PULL_CALM);
        self.begin_shoot(WebAim::Lasso { id: spot.id, at: (spot.x, spot.y - spot.half_w), half: spot.half_w, side, heavy: spot.giant });
        true
    }

    /// 🪝 이 동물에 줄을 쏨 (주사위 없이)
    pub(super) fn start_pull_pet(&mut self, spot: &PetSpot, world: &World) -> bool {
        let to = self.pull_landing(spot.x, world);
        self.web.pull_ready_at = self.clock + behavior::range(&mut self.web.rng, web::PULL_CALM);
        self.begin_shoot(WebAim::Pet { key: spot.key, at: (spot.x, spot.y - 0.5 * spot.size), to });
        true
    }

    /// 줄 쏘기 시작 ("thwip!")
    fn begin_shoot(&mut self, aim: WebAim) {
        self.web.exit_side = None;
        self.web.exit_out = None;
        self.pacing.after_beat = None;
        self.clear_hover();
        self.speech = Some(Speech::Thwip);
        self.state = State::Web { phase: WebPhase::Shoot { aim, hit: false }, t: 0.0 };
    }

    /// 줄 놀이 한 틱
    fn step_web(&mut self, dt: f64, world: &World, phase: WebPhase, t: f64) {
        let airborne = matches!(
            phase,
            WebPhase::Zip { .. } | WebPhase::Swing { .. } | WebPhase::Crawl { .. } | WebPhase::Dangle { .. } | WebPhase::HangRise { .. } | WebPhase::HangSlide { .. } | WebPhase::Hang { .. } | WebPhase::HangClimb { .. }
        );
        // 🌉🚪 친구 화면으로 나가는 그네는 끊지 않음 (집이 그리운 손님도 이 길로 나감)
        if self.web.exit_side.is_none() && self.romp_interrupted() {
            if airborne {
                self.web.chain_left = 0;
                self.start_fall();
            } else {
                self.state = self.idle_state();
            }
            return;
        }
        if !airborne && !self.follow_anchor(world) {
            self.start_fall();
            return;
        }
        let t = t + dt;
        match phase {
            WebPhase::Shoot { aim, hit } => self.step_shoot(world, aim, hit, t),
            WebPhase::Zip { anchor, from } => self.step_zip(anchor, from, t),
            WebPhase::Swing { anchor, theta, omega } => self.step_web_swing(dt, anchor, theta, omega, t),
            WebPhase::Pull { tip } => {
                self.state = if t < web::PULL_SECS { State::Web { phase: WebPhase::Pull { tip }, t } } else { self.rest_beat() };
            }
            WebPhase::Weave { spot, shot } => self.step_weave(spot, shot, t),
            WebPhase::Whirl { id, from, half, dir, heavy } => self.step_whirl(world, id, from, half, dir, heavy, t),
            WebPhase::Drag { id, from, side } => self.step_drag(dt, world, id, from, side, t),
            WebPhase::Wrap { key, feet, size, shot } => self.step_wrap(dt, key, feet, size, shot, t),
            WebPhase::Crawl { web, from, k, left } => self.step_crawl(world, web, from, k, left, t),
            WebPhase::Dangle { web, rel, bounce, len } => self.step_dangle(web, rel, bounce, len, t),
            WebPhase::HangRise { .. } | WebPhase::HangSlide { .. } | WebPhase::Hang { .. } | WebPhase::HangClimb { .. } => self.step_hang(world, phase, t),
        }
    }

    /// 줄이 뻗어 나가 닿는 순간 목적대로: 거미집이 핌 · 그네로 떠오름 · 끌어옴
    fn step_shoot(&mut self, world: &World, aim: WebAim, hit: bool, t: f64) {
        if !hit && t >= web::SHOOT_SECS {
            match aim {
                WebAim::Corner(spot) => {
                    self.web.out = Some(spot);
                    self.speech = Some(Speech::Web);
                    // 🕸️ 한 번에 피지 않고 줄을 몇 번 더 쏴서 짬
                    if spot.shots > 1 {
                        self.state = State::Web { phase: WebPhase::Weave { spot, shot: 1 }, t: 0.0 };
                        return;
                    }
                }
                WebAim::Anchor(anchor) => {
                    self.standing = None;
                    self.poop.stepping_off();
                    self.state = State::Web { phase: WebPhase::Zip { anchor, from: (self.body.x, self.body.y) }, t: 0.0 };
                    return;
                }
                WebAim::Object { id, at, to, heavy } => {
                    let g = crate::physics::THROW_GRAVITY_LOGICAL * self.cfg.dpi;
                    let secs = if heavy { web::GIANT_YANK_FLIGHT_SECS } else { web::YANK_FLIGHT_SECS };
                    let (vx, vy) = web::yank_velocity((at.0, self.object_floor(at)), to, secs, g);
                    self.play.event = Some(ObjectPlay::Yank { id, vx, vy });
                    self.speech = Some(Speech::Gotcha);
                    self.state = State::Web { phase: WebPhase::Pull { tip: at }, t: 0.0 };
                    return;
                }
                // 줄이 닿는 순간에도 바닥에 있어야 낚음 (그새 누가 집었거나 치웠으면 줄만 뻗었다 거둠)
                WebAim::Lasso { id, .. } if !self.play.spots.iter().any(|s| s.id == id) => {}
                // 💪 거대한 것을 받는 먼 친구 끝: 휘두르지 않고 끌고 감
                WebAim::Lasso { id, at, half, side, heavy: true } if self.drags_to(side, world) => {
                    self.facing = visit::dir(side);
                    self.speech = Some(Speech::Gotcha);
                    self.juggle.carry = Some(id);
                    self.juggle.giant = true;
                    self.juggle.giant_half = half;
                    self.juggle.present_pending = false;
                    self.state = State::Web { phase: WebPhase::Drag { id, from: (at.0, at.1 + half), side }, t: 0.0 };
                    return;
                }
                WebAim::Lasso { id, at, half, side, heavy } => {
                    self.facing = visit::dir(side);
                    self.speech = Some(Speech::Gotcha);
                    self.state = State::Web { phase: WebPhase::Whirl { id, from: at, half, dir: self.facing, heavy }, t: 0.0 };
                    return;
                }
                // 🕸️🧗 거미집에 붙어 기기 시작 (그새 거미집이 사라졌으면 줄만 뻗었다 거둠)
                WebAim::Perch { web, k, .. } if self.known_webs().iter().any(|w| w.id == web) => {
                    self.standing = None;
                    self.poop.stepping_off();
                    let (lo, hi) = web::WEB_CRAWL_STEPS;
                    let left = self.web.rng.u32(lo..=hi);
                    self.state = State::Web { phase: WebPhase::Crawl { web, from: (self.body.x, self.body.y), k, left }, t: 0.0 };
                    return;
                }
                WebAim::Perch { .. } => {}
                WebAim::Hang(line) => {
                    self.standing = None;
                    self.poop.stepping_off();
                    self.state = State::Web { phase: WebPhase::HangRise { line, from: (self.body.x, self.body.y) }, t: 0.0 };
                    return;
                }
                WebAim::Pet { key, at, to } => {
                    let spot = self.web.pets.iter().find(|s| s.key == key).copied();
                    let from = spot.map_or(at, |s| (s.x, s.y));
                    let (vx, vy) = web::yank_velocity(from, to, web::YANK_FLIGHT_SECS, GRAVITY);
                    self.web.yank_pet = Some((key, vx, vy));
                    self.speech = Some(Speech::Gotcha);
                    self.state = State::Web { phase: WebPhase::Pull { tip: at }, t: 0.0 };
                    return;
                }
            }
        }
        let hit = hit || t >= web::SHOOT_SECS;
        self.state = if t < web::SHOOT_SECS + web::SHOOT_HOLD_SECS { State::Web { phase: WebPhase::Shoot { aim, hit }, t } } else { self.rest_beat() };
    }

    /// 🕸️ 거미집 짜는 줄 한 번: WEAVE_SHOT_SECS가 지나면 다음 줄 (쏘는 그림을 처음부터), 다 쐈으면 쉼
    fn step_weave(&mut self, spot: WebSpin, shot: u32, t: f64) {
        if t < web::WEAVE_SHOT_SECS {
            self.state = State::Web { phase: WebPhase::Weave { spot, shot }, t };
        } else if shot + 1 < spot.shots {
            self.state = State::Web { phase: WebPhase::Weave { spot, shot: shot + 1 }, t: 0.0 };
        } else {
            self.state = self.rest_beat();
        }
    }

    /// 끌어올 물건의 바닥 y: 바닥 물건 목록에 있으면 그 y, 없으면 줄이 붙은 점 아래 (물건 반 폭 몫 — 어림)
    fn object_floor(&self, at: (f64, f64)) -> f64 {
        self.play.spots.iter().filter(|s| (s.x - at.0).abs() < 1.0).map(|s| s.y).next().unwrap_or(at.1)
    }

    /// 줄을 당겨 그네 시작 자리로 떠오름 (충돌 없이)
    pub(super) fn step_zip(&mut self, anchor: Anchor, from: (f64, f64), t: f64) {
        let (gx, drop) = self.swing_hand_offset();
        let hand = web::pendulum_hand((anchor.x, anchor.y), anchor.len, -anchor.angle, self.facing);
        let to = (hand.0 - self.facing * gx, hand.1 + drop);
        let u = (t / web::ZIP_SECS).min(1.0);
        let ease = 1.0 - (1.0 - u) * (1.0 - u);
        self.body.x = from.0 + (to.0 - from.0) * ease;
        self.body.y = from.1 + (to.1 - from.1) * ease;
        self.body.vx = 0.0;
        self.body.vy = 0.0;
        if u < 1.0 {
            self.state = State::Web { phase: WebPhase::Zip { anchor, from }, t };
            return;
        }
        self.speech = Some(Speech::WebSwing);
        self.state = State::Web { phase: WebPhase::Swing { anchor, theta: -anchor.angle, omega: 0.0 }, t: 0.0 };
    }

    /// 진자 한 틱: 반대편으로 RELEASE_FRAC만큼 올라가면 (또는 시한이 지나면) 놓고 날아감 — 더 이을 그네가 있으면 다음 줄 쏠 때를 잡음
    fn step_web_swing(&mut self, dt: f64, anchor: Anchor, theta: f64, omega: f64, t: f64) {
        let (theta, omega) = web::pendulum_step(theta, omega, anchor.len, GRAVITY, dt);
        let (gx, drop) = self.swing_hand_offset();
        let hand = web::pendulum_hand((anchor.x, anchor.y), anchor.len, theta, self.facing);
        self.body.x = hand.0 - self.facing * gx;
        self.body.y = hand.1 + drop;
        let (vx, vy) = web::release_velocity(anchor.len, theta, omega, self.facing);
        let released = (theta >= web::RELEASE_FRAC * anchor.angle && omega > 0.0) || t >= web::SWING_MAX_SECS;
        if !released {
            self.body.vx = vx;
            self.body.vy = vy;
            self.state = State::Web { phase: WebPhase::Swing { anchor, theta, omega }, t };
            return;
        }
        self.standing = None;
        self.body.vx = vx;
        self.body.vy = vy;
        self.thrown = false;
        self.gentle_drop = false;
        // 🌉🚪 친구 화면으로 나가는 그네: 이제 날아가다 그 끝을 넘으면 넘어감 (web_exit_out)
        if let Some(side) = self.web.exit_side.take() {
            self.web.exit_out = Some((side, self.clock + web::EXIT_FLY_SECS));
        }
        if self.web.chain_left > 0 {
            self.web.chain_at = self.clock + behavior::range(&mut self.web.rng, web::CHAIN_DELAY);
            self.web.chain_until = self.web.chain_at + web::CHAIN_WINDOW;
            self.web.chain_dir = self.facing;
        }
        self.state = State::Air { anim: Anim::Fall };
    }

    /// 🌀 휘두르기 한 틱: whirl_secs(heavy)가 지나면 지금 자리(앱이 마지막으로 옮긴 곳)에서 화면 끝 너머 바닥 높이로 포물선을 그리게 놓음 ("yeet!")
    fn step_whirl(&mut self, world: &World, id: u64, from: (f64, f64), half: f64, dir: f64, heavy: bool, t: f64) {
        if t < web::whirl_secs(heavy) {
            self.state = State::Web { phase: WebPhase::Whirl { id, from, half, dir, heavy }, t };
            return;
        }
        let size = self.cfg.size;
        let at = self.carry_point().unwrap_or((self.body.x, self.body.y - 2.0 * size));
        let edge = if dir > 0.0 { world.bounds.right } else { world.bounds.left };
        // 던질 동물이 아직 있으면 그 몸 가운데로, 아니면 친구 끝 너머 바닥 높이로
        let at_pet = self.web.whirl_at.take().and_then(|k| self.web.pets.iter().find(|s| s.key == k).copied());
        let to = at_pet.map_or((edge + dir * web::WHIRL_PAST * size, self.body.y), |s| (s.x, s.y - 0.5 * s.size));
        let to = if heavy { self.throw_target(to, dir) } else { to };
        let g = crate::physics::THROW_GRAVITY_LOGICAL * self.cfg.dpi;
        let (vx, vy) = web::yank_velocity(at, to, web::whirl_flight_secs(to.0 - at.0, size), g);
        self.juggle.event = Some(JuggleEvent::Release { id, vx, vy });
        self.speech = Some(Speech::Yeet);
        self.state = self.rest_beat();
    }

    /// 💪 거대한 것을 던질 자리: 가로로 GIANT_THROW_REACH 안이면 그대로 (가까운 친구 끝 너머·동물), 더 멀면 그 쪽 GIANT_THROW_REACH 앞 바닥
    fn throw_target(&self, to: (f64, f64), dir: f64) -> (f64, f64) {
        let reach = web::GIANT_THROW_REACH * self.cfg.size;
        if (to.0 - self.body.x) * dir <= reach {
            to
        } else {
            (self.body.x + dir * reach, self.body.y)
        }
    }

    /// 🌀 휘두르는 물건 가운데 자리 (물리 px): 처음 WHIRL_REEL_SECS 동안 낚은 자리에서 머리 위 원으로 감아올림 (💪 heavy면 작게·천천히)
    fn whirl_spot(&self, from: (f64, f64), dir: f64, heavy: bool, t: f64) -> (f64, f64) {
        let size = self.cfg.size;
        let (gx, gy) = self.cfg.grip.web_whirl.at(0.0);
        let gx = if self.facing < 0.0 { -gx } else { gx };
        let (_, top) = self.window_pos();
        let on = web::whirl_point((self.body.x + gx * size, top + gy * size), size, dir, t, heavy);
        let u = (t / web::WHIRL_REEL_SECS).min(1.0);
        let ease = 1.0 - (1.0 - u) * (1.0 - u);
        (from.0 + (on.0 - from.0) * ease, from.1 + (on.1 - from.1) * ease)
    }

    /// 💪 이 끝(side)으로 거대한 것을 끌고 감: 그 친구가 거대한 것을 받고 (giant_sides), 던져서는 안 닿게 멂 (GIANT_THROW_REACH 너머 — 가까우면 휘둘러 던짐)
    fn drags_to(&self, side: Side, world: &World) -> bool {
        let edge = if visit::dir(side) > 0.0 { world.bounds.right } else { world.bounds.left };
        (edge - self.body.x).abs() > web::GIANT_THROW_REACH * self.cfg.size && self.giant_sides(world).contains(&side)
    }

    /// 💪 끌기 한 틱: 친구 끝으로 천천히 걸음 (DRAG_PACE). 화면 끝에 닿으면 끌던 채 나가고, 끝이 막혔거나 너무 오래 걸리면 그 자리에 놓음
    fn step_drag(&mut self, dt: f64, world: &World, id: u64, from: (f64, f64), side: Side, t: f64) {
        let dir = visit::dir(side);
        self.facing = dir;
        if t > web::DRAG_MAX_SECS {
            self.let_go_drag(id);
            return;
        }
        let Some(blocked) = self.juggle_walk(world, dir, self.cfg.walk_speed * web::DRAG_PACE * dt) else { return };
        if !blocked {
            self.state = State::Web { phase: WebPhase::Drag { id, from, side }, t };
            return;
        }
        let open = match side {
            Side::Left => self.exit_edges.left,
            _ => self.exit_edges.right,
        };
        let piece = self.standing.and_then(|st| world.piece_at(st.owner, self.body.x)).copied();
        if open && piece.is_some_and(|p| visit::screen_edge(world, &p, dir) == Some(side)) {
            // 끌던 채 나감 — 못 들어가 돌아오면 집에 내려놓음
            self.juggle.present_pending = true;
            self.start_exit(side);
        } else {
            self.let_go_drag(id);
        }
    }

    /// 💪 끌던 거대한 것을 그 자리에 놓음 (앱이 떨어뜨림)
    fn let_go_drag(&mut self, id: u64) {
        self.juggle.carry = None;
        self.juggle.present_pending = false;
        self.juggle.event = Some(JuggleEvent::Release { id, vx: 0.0, vy: 0.0 });
        self.state = self.rest_beat();
    }

    /// 💪 매 틱 (update_web 처음): 줄 놀이 밖에서 끄는 거대한 것 — 놓쳤으면 잊고, 친구 화면에 끌고 들어와 (또는 못 가 돌아와) 섰으면
    /// 줄을 풀어 그 자리에 내려놓음 ("tada!")
    fn settle_drag(&mut self) {
        if self.juggle.carry.is_none() || matches!(self.state, State::Web { .. }) {
            return;
        }
        if !self.carry_ok() {
            self.juggle.carry = None;
            self.juggle.present_pending = false;
            return;
        }
        if self.juggle.present_pending && self.standing.is_some() && matches!(self.state, State::Move { .. } | State::Idle { .. } | State::Turn { .. }) {
            if let Some(id) = self.juggle.carry.take() {
                self.juggle.event = Some(JuggleEvent::Release { id, vx: 0.0, vy: 0.0 });
            }
            self.juggle.present_pending = false;
            self.speech = Some(Speech::Tada);
        }
    }

    /// 줄에 매달아 끄는 중인 단계 (carry_ok가 부름)
    pub(super) fn drags(&self, phase: WebPhase) -> bool {
        matches!(phase, WebPhase::Drag { .. })
    }

    /// 💪 끄는 거대한 것 바닥 가운데 자리 (물리 px — 앱의 carry_at은 물건 바닥 y를 받음): 몸 뒤 DRAG_GAP 틈을 둔 바닥.
    /// 낚은 뒤 처음 WHIRL_REEL_SECS 동안은 낚은 자리에서 끌려옴. 거미인간이 거대한 것을 끄는 중이 아니면 None
    pub(super) fn drag_point(&self) -> Option<(f64, f64)> {
        if !self.cfg.slings_webs || self.juggle.carry.is_none() || !self.carry_ok() {
            return None;
        }
        let behind = self.juggle.giant_half + web::DRAG_GAP * self.cfg.size;
        let at = (self.body.x - self.facing * behind, self.body.y - crate::object::CARRY_SINK_MAX * self.cfg.dpi);
        let State::Web { phase: WebPhase::Drag { from, .. }, t } = self.state else { return Some(at) };
        let u = (t / web::WHIRL_REEL_SECS).min(1.0);
        let ease = 1.0 - (1.0 - u) * (1.0 - u);
        Some((from.0 + (at.0 - from.0) * ease, from.1 + (at.1 - from.1) * ease))
    }

    /// 💪 끄는 줄: 엉덩이(실젖 — 몸 뒤 위)에서 거대한 것 가운데로
    fn drag_line(&self) -> Option<WebLine> {
        let (x, y) = self.drag_point()?;
        let size = self.cfg.size;
        let from = (self.body.x - self.facing * web::SPINNERET.0 * size, self.body.y - web::SPINNERET.1 * size);
        Some(WebLine { from, to: (x, y - self.juggle.giant_half) })
    }

    /// 🌀 휘두르는 중인 물건 id (carried_object가 부름)
    pub(super) fn whirled_object(&self) -> Option<u64> {
        match self.state {
            State::Web { phase: WebPhase::Whirl { id, .. }, .. } => Some(id),
            _ => None,
        }
    }

    /// 🌀 휘두르는 물건 바닥 자리 (carry_point가 부름 — 앱의 carry_at은 물건 바닥 y를 받음)
    pub(super) fn whirl_carry_point(&self) -> Option<(f64, f64)> {
        let State::Web { phase: WebPhase::Whirl { from, half, dir, heavy, .. }, t } = self.state else { return None };
        let (x, y) = self.whirl_spot(from, dir, heavy, t);
        Some((x, y + half))
    }

    /// 🌀 휘두르던 물건을 사용자가 낚아챘거나 물건이 사라졌음: "hmph!" 하고 그만둠. 휘두르던 중이었으면 true
    pub(super) fn lose_whirl(&mut self) -> bool {
        let dragging = matches!(self.state, State::Web { phase: WebPhase::Drag { .. }, .. });
        if self.whirled_object().is_none() && !dragging {
            return false;
        }
        if dragging {
            self.juggle.carry = None;
        }
        self.speech = Some(Speech::Hmph);
        self.state = self.rest_beat();
        true
    }

    /// 줄 놀이 그림
    pub(super) fn web_anim(&self, phase: WebPhase) -> Anim {
        match phase {
            WebPhase::Shoot { .. } | WebPhase::Weave { .. } | WebPhase::Wrap { .. } => Anim::Shoot,
            WebPhase::Zip { .. } | WebPhase::Swing { .. } => Anim::WebSwing,
            WebPhase::Pull { .. } => Anim::WebPull,
            WebPhase::Whirl { .. } => Anim::WebWhirl,
            WebPhase::Drag { .. } => Anim::Walk,
            WebPhase::Crawl { .. } => Anim::Climb,
            WebPhase::Dangle { .. } | WebPhase::HangRise { .. } => Anim::WebSwing,
            WebPhase::HangSlide { .. } | WebPhase::Hang { .. } | WebPhase::HangClimb { .. } => Anim::HangDown,
        }
    }

    /// 줄에 매달려 공중에 있음 (떠오르는 중·진자·🕸️🧗 거미집 위·🙃 거꾸로 매달리기)
    pub(super) fn web_airborne(&self) -> bool {
        matches!(
            self.state,
            State::Web {
                phase: WebPhase::Zip { .. }
                    | WebPhase::Swing { .. }
                    | WebPhase::Crawl { .. }
                    | WebPhase::Dangle { .. }
                    | WebPhase::HangRise { .. }
                    | WebPhase::HangSlide { .. }
                    | WebPhase::Hang { .. }
                    | WebPhase::HangClimb { .. },
                ..
            }
        )
    }
}

/// 🪝 앱이 매 틱 동물 update 뒤에 (sync_lifts가 부름): 거미인간이 끌어온 동물을 날려 보냄
pub(super) fn sync_yanks<T: AsRef<Pet> + AsMut<Pet>>(pets: &mut [T]) {
    for i in 0..pets.len() {
        let Some((key, vx, vy)) = pets[i].as_mut().web.yank_pet.take() else { continue };
        if let Some(j) = pets.iter().position(|p| p.as_ref().social_key == key) {
            if j != i {
                pets[j].as_mut().be_yanked(vx, vy);
            }
        }
    }
}
