//! 🕷️ 거미인간 줄 놀이 — 거미집·그네·영웅 착지·끌어오기·🌀 휘둘러 던지기 (pet.json `web_slinger` + WEB_ANIMS — PetConfig::slings_webs) — brain.rs의 자식 모듈. 숫자·계산은 web.rs.
//!
//! - 🕸️ 거미집: 모서리를 보고 줄을 쏨 (shoot) → 줄 끝이 닿는 순간 take_web이 WebSpin을 한 번 돌려줌 (앱이 그 모서리에 거미집을 그림).
//!   크기는 제각각 (sized_web)이고, 첫 줄 뒤에도 거미집 여기저기로 줄을 몇 번 더 휙휙 쏴서 짬 (Weave — 앱이 그동안 살·고리를 차례로 그림).
//!   손님으로 친구 화면에 가 있으면 자주, 집에서는 가끔.
//! - 🌉 그네: 머리 위 창 아랫변·화면 윗변에 줄을 붙이고 (shoot) → 줄을 당겨 몸이 떠오름 (Zip) → 진자 (web_swing) → 반대편에서 놓고 날아감.
//!   🌉🌉 줄 잇기: 놓고 날아가는 중 (State::Air) 꼭대기 언저리에서 앞 위로 새 줄을 쏴 ("thwip!" / 가끔 "woohoo!") Zip 없이
//!   지금 자리·속도 그대로 진자를 이어 감 (try_chain_swing) — 한 번에 web::chain_range번, 닻이 없으면 그냥 떨어져 영웅 착지.
//! - 🦸 영웅 착지: 몸 크기 HERO_FALL_SIZES배 넘게 떨어져 내려서면 콰당 대신 hero_land ("nailed it!") — land_or_recover가 부름.
//! - 🪝 끌어오기: 바닥의 물건(ObjectPlay::Yank — 앱이 날림)이나 다른 동물(sync_lifts가 be_yanked)에 줄을 붙여 휙 (web_pull).
//!   반은 앞에 떨어지게 끌어오고 반은 머리 너머 뒤로 던짐.
//! - 🌀 휘둘러 던지기: 내 동물이고 친구가 연결된 끝이 있으면 (friend_sides) 물건은 자주 줄로 낚아 머리 위로 빙빙 (web_whirl) →
//!   그 끝 너머로 휙. 도는 동안은 흐물이가 든 물건처럼 carried_object·carry_point로 앱이 옮기고, 놓을 때 JuggleEvent::Release
//!   (앱이 날림 — 끝을 넘으면 친구 모니터로). 낚아채이면 (lose_carry) "hmph!", 끼어들리면 줄을 놓아 물건이 떨어짐.
//! - 🐛 고치 말기: 닿는 거리에 쉬는 다른 동물(친구 동물 포함)이 있으면 가끔 (COCOON_RATE) 그 몸 둘레로 줄을 COCOON_SHOTS번 쏴서 (Wrap)
//!   칭칭 — 첫 줄이 닿으면 sync_cocoons가 그 동물을 감고 (brain_snare.rs), 다 감으면 "hehe".
//! - 지금 뻗은 줄은 web_line (앱이 덮개 창에 그림).
//!
//! 주사위는 자기 주사위(WebMood.rng)로 굴림 — 평소 동작과 다른 동물의 주사위 순서는 그대로.

use super::hoist_moves::PetSpot;
use super::*;
use crate::juggle::{JuggleEvent, ObjectSpot};
use crate::play::ObjectPlay;
use crate::web::{self, Anchor, WebLine, WebSpin};

/// 줄을 쏜 목적
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum WebAim {
    /// 🕸️ 모서리에 거미집
    Corner(WebSpin),
    /// 🌉 그네 닻
    Anchor(Anchor),
    /// 🪝 바닥의 물건 (at: 줄이 붙는 점, to: 떨어질 자리)
    Object { id: u64, at: (f64, f64), to: (f64, f64) },
    /// 🪝 다른 동물 (key = social_key)
    Pet { key: u64, at: (f64, f64), to: (f64, f64) },
    /// 🌀 바닥의 물건을 낚아 휘둘러 side 끝 너머로 (at: 줄이 붙는 점 = 물건 가운데, half: 물건 반 폭)
    Lasso { id: u64, at: (f64, f64), half: f64, side: Side },
}

impl WebAim {
    /// 줄 끝이 닿을 점 (물리 px)
    pub fn tip(&self) -> (f64, f64) {
        match *self {
            WebAim::Corner(s) => (s.x, s.y),
            WebAim::Anchor(a) => (a.x, a.y),
            WebAim::Object { at, .. } | WebAim::Pet { at, .. } | WebAim::Lasso { at, .. } => at,
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
    /// 🌀 줄 끝의 물건을 머리 위로 휘두름 (from: 낚은 순간 물건 가운데, half: 물건 반 폭, dir: 던질 쪽 ±1)
    Whirl { id: u64, from: (f64, f64), half: f64, dir: f64 },
    /// 🐛 다른 동물(key)을 고치로 감는 shot번째 줄 (web::cocoon_target으로 노림 — feet: 그 동물 발, size: 그 몸 크기)
    Wrap { key: u64, feet: (f64, f64), size: f64, shot: u32 },
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
    /// 새 거미집 (take_web으로 한 번)
    out: Option<WebSpin>,
    /// 끌어온 동물 (social_key, vx, vy) — sync_lifts가 한 번 꺼냄
    pub(super) yank_pet: Option<(u64, f64, f64)>,
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
            out: None,
            yank_pet: None,
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

    /// 지금 뻗어 있는 줄 (손 → 줄 끝, 물리 px). 줄 놀이 중이 아니면 None
    pub fn web_line(&self) -> Option<WebLine> {
        let State::Web { phase, t } = self.state else { return None };
        let hand = self.web_hand();
        let lerp = |a: (f64, f64), b: (f64, f64), u: f64| (a.0 + (b.0 - a.0) * u, a.1 + (b.1 - a.1) * u);
        let to = match phase {
            WebPhase::Shoot { aim, hit } => lerp(hand, aim.tip(), if hit { 1.0 } else { (t / web::SHOOT_SECS).min(1.0) }),
            WebPhase::Zip { anchor, .. } | WebPhase::Swing { anchor, .. } => (anchor.x, anchor.y),
            WebPhase::Pull { tip } => lerp(tip, hand, (t / web::PULL_SECS).min(1.0)),
            WebPhase::Weave { spot, shot } => lerp(hand, web::weave_target(&spot, shot), (t / (0.5 * web::WEAVE_SHOT_SECS)).min(1.0)),
            WebPhase::Whirl { from, dir, .. } => self.whirl_spot(from, dir, t),
            WebPhase::Wrap { feet, size, shot, .. } => lerp(hand, web::cocoon_target(feet, size, shot), (t / (0.5 * web::WEAVE_SHOT_SECS)).min(1.0)),
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
        matches!(self.state, State::Land { .. }) && self.clock < self.web.hero_until
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

    /// 손 자리 (물리 px): 지금 그림의 grip (shoot·web_swing·web_pull·web_whirl), 왼쪽을 보면 좌우 반전
    fn web_hand(&self) -> (f64, f64) {
        let g = &self.cfg.grip;
        let (gx, gy) = match self.state {
            State::Web { phase: WebPhase::Shoot { .. } | WebPhase::Weave { .. } | WebPhase::Wrap { .. }, t } => g.shoot.at(t),
            State::Web { phase: WebPhase::Zip { .. } | WebPhase::Swing { .. }, t } => g.web_swing.at(t),
            State::Web { phase: WebPhase::Pull { .. }, t } => g.web_pull.at(t),
            State::Web { phase: WebPhase::Whirl { .. }, t } => g.web_whirl.at(t),
            _ => g.shoot.at(1.0),
        };
        let gx = if self.facing < 0.0 { -gx } else { gx };
        let (_, top) = self.window_pos();
        (self.body.x + gx * self.cfg.size, top + gy * self.cfg.size)
    }

    /// 그네 그림의 손에서 발까지 (가로 — 진행 쪽 +, 세로 — 아래 +, 물리 px)
    fn swing_hand_offset(&self) -> (f64, f64) {
        let (gx, gy) = self.cfg.grip.web_swing.at(0.0);
        (gx * self.cfg.size, (1.0 - gy) * self.cfg.size)
    }

    /// update에서 (다른 동물 놀이 뒤). 이번 틱을 여기서 처리했으면 true
    pub(super) fn update_web(&mut self, dt: f64, world: &World) -> bool {
        if !self.cfg.slings_webs {
            return false;
        }
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
        self.maybe_pull(dt, world) || self.maybe_cocoon(dt, world) || self.maybe_spin_web(dt, world) || self.maybe_web_swing(dt, world)
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

    /// 🕸️ 가까운 모서리 하나를 골라 줄을 쏨 (주사위 없이). 모서리가 없으면 false
    pub(super) fn start_web_spin(&mut self, world: &World) -> bool {
        let size = self.cfg.size;
        let at = (self.body.x, self.body.y - 0.6 * size);
        let spots = web::corner_spots(world, at, web::WEB_REACH * size, size);
        if spots.is_empty() {
            return false;
        }
        let spot = spots[self.web.rng.usize(..spots.len())];
        let spot = self.sized_web(spot, world);
        let calm = if self.guest.is_some() { web::WEB_CALM_GUEST } else { web::WEB_CALM_HOME };
        self.web.web_ready_at = self.clock + behavior::range(&mut self.web.rng, calm);
        self.facing = if spot.x >= self.body.x { 1.0 } else { -1.0 };
        self.begin_shoot(WebAim::Corner(spot));
        true
    }

    /// 🕸️ 칠 거미집의 크기 (WEB_SIZE_RANGE — 창 모서리는 창 짧은 변의 WEB_WINDOW_FIT배까지)와 짤 줄 횟수·시간을 정함
    pub(super) fn sized_web(&mut self, spot: WebSpin, world: &World) -> WebSpin {
        // 🌱 거미집 크기·줄 수는 연령대로 자람 (아기 작게 2~4번 → 어른 크게 5~8번, brain_stage.rs)
        let p = self.power();
        let k = behavior::range(&mut self.web.rng, web::WEB_SIZE_RANGE) * p;
        let mut size = k * self.cfg.size;
        if let Some(r) = spot.window.and_then(|id| world.windows.iter().find(|w| w.id == id)).map(|w| w.rect) {
            size = size.min(web::WEB_WINDOW_FIT * r.width().min(r.height()));
        }
        let k = size / self.cfg.size;
        let (lo, hi) = web::shot_range(p);
        let shots = ((2.0 + 1.5 * k).round() as u32).clamp(lo, hi);
        WebSpin { size, shots, build: f64::from(shots - 1) * web::WEAVE_SHOT_SECS, ..spot }
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

    /// 끌어올 수 있는 것: 같은 모니터, PULL_REACH 안의 바닥 물건 (거대한 것 빼고)과 들 수 있는 동물
    fn pull_targets(&self, world: &World) -> (Vec<ObjectSpot>, Vec<PetSpot>) {
        let size = self.cfg.size;
        let (lo, hi) = (web::PULL_REACH.0 * size, web::PULL_REACH.1 * size);
        let wa = world.work_area_at(self.body.x, self.body.y - 1.0);
        let ok = |x: f64, y: f64| {
            let d = (x - self.body.x).hypot(y - self.body.y);
            (lo..=hi).contains(&d) && world.work_area_at(x, y - 1.0) == wa
        };
        let objs = self.play.spots.iter().filter(|s| !s.giant && ok(s.x, s.y)).copied().collect();
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

    /// 🪝 이 물건에 줄을 쏨 (주사위 없이). 친구가 연결된 끝이 있으면 WHIRL_CHANCE로 🌀 휘둘러 던지기 (끝은 그중 아무 데나)
    pub(super) fn start_pull_object(&mut self, spot: &ObjectSpot, world: &World) -> bool {
        let sides = self.friend_sides(world);
        if !sides.is_empty() && self.web.rng.f64() < web::WHIRL_CHANCE {
            let side = sides[self.web.rng.usize(..sides.len())];
            return self.start_whirl_object(spot, side);
        }
        let to = self.pull_landing(spot.x, world);
        self.web.pull_ready_at = self.clock + behavior::range(&mut self.web.rng, web::PULL_CALM);
        self.begin_shoot(WebAim::Object { id: spot.id, at: (spot.x, spot.y - spot.half_w), to });
        true
    }

    /// 🌀 이 물건에 줄을 쏨 — 닿으면 휘둘러 side 끝 너머로 던짐 (주사위 없이)
    pub(super) fn start_whirl_object(&mut self, spot: &ObjectSpot, side: Side) -> bool {
        self.facing = if spot.x >= self.body.x { 1.0 } else { -1.0 };
        self.web.pull_ready_at = self.clock + behavior::range(&mut self.web.rng, web::PULL_CALM);
        self.begin_shoot(WebAim::Lasso { id: spot.id, at: (spot.x, spot.y - spot.half_w), half: spot.half_w, side });
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
        self.pacing.after_beat = None;
        self.clear_hover();
        self.speech = Some(Speech::Thwip);
        self.state = State::Web { phase: WebPhase::Shoot { aim, hit: false }, t: 0.0 };
    }

    /// 줄 놀이 한 틱
    fn step_web(&mut self, dt: f64, world: &World, phase: WebPhase, t: f64) {
        let airborne = matches!(phase, WebPhase::Zip { .. } | WebPhase::Swing { .. });
        if self.romp_interrupted() {
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
            WebPhase::Shoot { aim, hit } => self.step_shoot(aim, hit, t),
            WebPhase::Zip { anchor, from } => self.step_zip(anchor, from, t),
            WebPhase::Swing { anchor, theta, omega } => self.step_web_swing(dt, anchor, theta, omega, t),
            WebPhase::Pull { tip } => {
                self.state = if t < web::PULL_SECS { State::Web { phase: WebPhase::Pull { tip }, t } } else { self.rest_beat() };
            }
            WebPhase::Weave { spot, shot } => self.step_weave(spot, shot, t),
            WebPhase::Whirl { id, from, half, dir } => self.step_whirl(world, id, from, half, dir, t),
            WebPhase::Wrap { key, feet, size, shot } => self.step_wrap(dt, key, feet, size, shot, t),
        }
    }

    /// 줄이 뻗어 나가 닿는 순간 목적대로: 거미집이 핌 · 그네로 떠오름 · 끌어옴
    fn step_shoot(&mut self, aim: WebAim, hit: bool, t: f64) {
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
                WebAim::Object { id, at, to } => {
                    let g = crate::physics::THROW_GRAVITY_LOGICAL * self.cfg.dpi;
                    let (vx, vy) = web::yank_velocity((at.0, self.object_floor(at)), to, web::YANK_FLIGHT_SECS, g);
                    self.play.event = Some(ObjectPlay::Yank { id, vx, vy });
                    self.speech = Some(Speech::Gotcha);
                    self.state = State::Web { phase: WebPhase::Pull { tip: at }, t: 0.0 };
                    return;
                }
                // 줄이 닿는 순간에도 바닥에 있어야 낚음 (그새 누가 집었거나 치웠으면 줄만 뻗었다 거둠)
                WebAim::Lasso { id, .. } if !self.play.spots.iter().any(|s| s.id == id) => {}
                WebAim::Lasso { id, at, half, side } => {
                    self.facing = visit::dir(side);
                    self.speech = Some(Speech::Gotcha);
                    self.state = State::Web { phase: WebPhase::Whirl { id, from: at, half, dir: self.facing }, t: 0.0 };
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
    fn step_zip(&mut self, anchor: Anchor, from: (f64, f64), t: f64) {
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
        if self.web.chain_left > 0 {
            self.web.chain_at = self.clock + behavior::range(&mut self.web.rng, web::CHAIN_DELAY);
            self.web.chain_until = self.web.chain_at + web::CHAIN_WINDOW;
            self.web.chain_dir = self.facing;
        }
        self.state = State::Air { anim: Anim::Fall };
    }

    /// 🌀 휘두르기 한 틱: WHIRL_SECS가 지나면 지금 자리(앱이 마지막으로 옮긴 곳)에서 화면 끝 너머 바닥 높이로 포물선을 그리게 놓음 ("yeet!")
    fn step_whirl(&mut self, world: &World, id: u64, from: (f64, f64), half: f64, dir: f64, t: f64) {
        if t < web::WHIRL_SECS {
            self.state = State::Web { phase: WebPhase::Whirl { id, from, half, dir }, t };
            return;
        }
        let size = self.cfg.size;
        let at = self.carry_point().unwrap_or((self.body.x, self.body.y - 2.0 * size));
        let edge = if dir > 0.0 { world.bounds.right } else { world.bounds.left };
        let to = (edge + dir * web::WHIRL_PAST * size, self.body.y);
        let g = crate::physics::THROW_GRAVITY_LOGICAL * self.cfg.dpi;
        let (vx, vy) = web::yank_velocity(at, to, web::whirl_flight_secs(to.0 - at.0, size), g);
        self.juggle.event = Some(JuggleEvent::Release { id, vx, vy });
        self.speech = Some(Speech::Yeet);
        self.state = self.rest_beat();
    }

    /// 🌀 휘두르는 물건 가운데 자리 (물리 px): 처음 WHIRL_REEL_SECS 동안 낚은 자리에서 머리 위 원으로 감아올림
    fn whirl_spot(&self, from: (f64, f64), dir: f64, t: f64) -> (f64, f64) {
        let size = self.cfg.size;
        let (gx, gy) = self.cfg.grip.web_whirl.at(0.0);
        let gx = if self.facing < 0.0 { -gx } else { gx };
        let (_, top) = self.window_pos();
        let on = web::whirl_point((self.body.x + gx * size, top + gy * size), size, dir, t);
        let u = (t / web::WHIRL_REEL_SECS).min(1.0);
        let ease = 1.0 - (1.0 - u) * (1.0 - u);
        (from.0 + (on.0 - from.0) * ease, from.1 + (on.1 - from.1) * ease)
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
        let State::Web { phase: WebPhase::Whirl { from, half, dir, .. }, t } = self.state else { return None };
        let (x, y) = self.whirl_spot(from, dir, t);
        Some((x, y + half))
    }

    /// 🌀 휘두르던 물건을 사용자가 낚아챘거나 물건이 사라졌음: "hmph!" 하고 그만둠. 휘두르던 중이었으면 true
    pub(super) fn lose_whirl(&mut self) -> bool {
        if self.whirled_object().is_none() {
            return false;
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
        }
    }

    /// 줄에 매달려 공중에 있음 (떠오르는 중·진자)
    pub(super) fn web_airborne(&self) -> bool {
        matches!(self.state, State::Web { phase: WebPhase::Zip { .. } | WebPhase::Swing { .. }, .. })
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
