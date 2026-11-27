//! 🐛🕸️ 거미인간에게 걸리기 — 고치에 감긴 동물·거미집에 걸린 동물 (State::Stuck) — brain.rs의 자식 모듈. 숫자·계산은 web.rs.
//!
//! - 🐛 고치: 거미인간이 줄을 여러 번 쏴서 (brain_web.rs WebPhase::Wrap) 감으면 sync_cocoons가 be_cocooned를 부름 →
//!   못 움직이고 버둥버둥 (render.wobble, "mmph!") — 실은 줄마다 한 겹 (render.cocoon, 웹뷰가 하얀 띠로 그림) → COCOON_SECS 뒤 뿅 ("free!").
//! - 🕸️ 거미집: 앱이 화면의 거미집을 set_webs로 알려 줌 → 거미인간이 아닌 동물의 몸 (가운데·발)이 거미집 안으로 막 들어오면
//!   SNARE_CHANCE로 걸려 버둥버둥 ("help!") → SNARE_SECS 뒤 찢고 빠져나옴 (take_torn_web — 앱이 그 거미집을 지움).
//!   빠져나온 뒤 SNARE_IMMUNE_SECS 동안은 안 걸림. 가려진 거미집은 안 걸림. 걸린 거미집이 사라지면 그냥 풀려남.
//! - 🕸️🖱️ 손으로: 사용자가 끌어다 거미집 위에 놓으면 (snare_on_drop — 나는 동물도) 또는 던져서 날아가다 거미집에 들어가면 물건처럼 무조건 걸림.
//! - 🕸️ "팅~": 걸리는 순간 그림이 들어온 쪽으로 늘어났다 출렁 (web::snag_spring — 몸 자리는 그대로, snare_offset을 window_pos에 더함) 뒤 살랑살랑.
//! - 거미인간은 어느 거미인간의 거미집에도 안 걸림 (snare_ok·maybe_snare — cfg.slings_webs면 모두). 놀러 올라가는 건 brain_web.rs 거미집 놀이.
//! - 클릭: 걸린 동물을 누르면 바로 풀어 줌 ("thanks!" — pointer_down이 free_stuck을 부름. 거미집은 찢어짐).
//!
//! 주사위는 자기 주사위(SnareMood.rng)로 굴림 — 평소 동작의 주사위 순서는 그대로.

use super::*;
use crate::web::{self, WebSpot};

/// 무엇에 걸렸나
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum StuckKind {
    /// 🐛 거미인간(by = social_key)의 고치
    Cocoon { by: u64 },
    /// 🕸️ 거미집 (web = 앱의 거미집 번호)
    Snare { web: u64 },
}

/// 고치·거미집에 걸리기의 기억
#[derive(Debug, Clone)]
pub struct SnareMood {
    rng: fastrand::Rng,
    /// 거미집 안으로 들어갈 때 걸릴 몫 (보통 SNARE_CHANCE — 테스트가 바꿈)
    pub(super) chance: f64,
    /// 화면의 거미집 (앱이 set_webs로)
    webs: Vec<WebSpot>,
    /// 지난번에 몸이 들어 있던 거미집 번호 (막 들어온 것만 주사위)
    inside: Vec<u64>,
    /// 이 시각(clock)까지는 안 걸림
    immune_until: f64,
    /// 찢고 나온 거미집 — take_torn_web이 한 번 꺼냄
    torn: Option<u64>,
    /// 다음 버둥 말까지
    talk_in: f64,
    /// 🕸️ 걸려 들어온 쪽 (단위 벡터 — 출렁 방향)
    spring_dir: (f64, f64),
}

impl SnareMood {
    /// 시드로 난수를 따로 둠
    pub(super) fn new(seed: u64) -> Self {
        SnareMood {
            rng: fastrand::Rng::with_seed(seed ^ 0x736e_6172),
            chance: web::SNARE_CHANCE,
            webs: Vec::new(),
            inside: Vec::new(),
            immune_until: f64::NEG_INFINITY,
            torn: None,
            talk_in: 0.0,
            spring_dir: (0.0, 1.0),
        }
    }
}

impl Pet {
    /// 🕸️ 앱이 (틱마다·바뀔 때) 이 화면의 거미집을 알려 줌 (물리 px)
    pub fn set_webs(&mut self, webs: &[WebSpot]) {
        self.snare.webs.clear();
        self.snare.webs.extend_from_slice(webs);
    }

    /// 🕸️ 걸렸다가 찢고 나온 거미집 번호 (한 번) — 앱이 그 거미집을 지움
    pub fn take_torn_web(&mut self) -> Option<u64> {
        self.snare.torn.take()
    }

    /// 🐛🕸️ 고치에 감겼거나 거미집에 걸려 있음
    pub fn is_stuck(&self) -> bool {
        matches!(self.state, State::Stuck { .. })
    }

    /// update에서: 걸려 있으면 버둥 한 틱, 아니면 거미집에 막 들어왔는지 봄. 이번 틱을 여기서 처리했으면 true
    pub(super) fn update_stuck(&mut self, dt: f64, world: &World) -> bool {
        if let State::Stuck { kind, t, remaining } = self.state {
            self.step_stuck(dt, world, kind, t, remaining);
            return true;
        }
        self.maybe_snare()
    }

    /// 🕸️ 걸릴 수 있는 때: 거미인간·나는 동물이 아니고, 걷기·쉬기·떨어지기·기어오르기 같은 평소 몸짓 중 (잡힘·하우스·창 안·볼일·조작·먹기 빼고)
    fn snare_ok(&self) -> bool {
        matches!(
            self.state,
            State::Move { .. }
                | State::Idle { .. }
                | State::Turn { .. }
                | State::Land { .. }
                | State::Air { .. }
                | State::Climb { .. }
                | State::Slide
                | State::WallClimb { .. }
                | State::Tumble { .. }
                | State::Panic { .. }
        ) && !self.cfg.slings_webs
            && !self.cfg.flies
            && !self.housed
            && self.inside.is_none()
            && self.home_trip.is_none()
            && !self.has_errand()
            && !self.is_heading_out()
            && !self.is_controlled()
            && !self.is_user_away()
            && !self.is_eating()
            && !self.rider_on
            && self.juggle.carry.is_none()
    }

    /// 🕸️ 몸 (가운데·발)이 들어 있는 보이는 거미집 번호들
    fn webs_touching(&self) -> Vec<u64> {
        let size = self.cfg.size;
        let points = [(self.body.x, self.body.y - 0.5 * size), (self.body.x, self.body.y - 1.0)];
        self.snare
            .webs
            .iter()
            .filter(|w| !w.hidden && points.iter().any(|&(px, py)| web::in_web(px, py, w.x, w.y, w.corner, w.size)))
            .map(|w| w.id)
            .collect()
    }

    /// 🕸️ 거미집 안으로 막 들어왔으면 (지난번엔 밖) 하나마다 chance로 걸림 — 손에서 던져져 나는 중이면 (thrown) 들어 있기만 해도 무조건. 걸렸으면 true
    fn maybe_snare(&mut self) -> bool {
        if self.cfg.slings_webs || self.snare.webs.is_empty() {
            self.snare.inside.clear();
            return false;
        }
        let flung = self.thrown && matches!(self.state, State::Air { .. });
        let now = self.webs_touching();
        let fresh: Vec<u64> = if flung { now.clone() } else { now.iter().copied().filter(|id| !self.snare.inside.contains(id)).collect() };
        self.snare.inside = now;
        if fresh.is_empty() || self.clock < self.snare.immune_until || !self.snare_ok() {
            return false;
        }
        for id in fresh {
            if flung || self.snare.rng.f64() < self.snare.chance {
                self.get_snared(id);
                return true;
            }
        }
        false
    }

    /// 🕸️🖱️ 사용자가 끌어 놓은 순간 (pointer_up): 몸이 보이는 거미집 안이면 물건처럼 무조건 걸림 — 나는 동물도. 거미인간·막 빠져나온 동물은 빼고. 걸렸으면 true
    pub(super) fn snare_on_drop(&mut self) -> bool {
        if self.cfg.slings_webs || self.clock < self.snare.immune_until || self.is_controlled() || self.rider_on || self.juggle.carry.is_some() {
            return false;
        }
        let now = self.webs_touching();
        let Some(&id) = now.first() else { return false };
        self.snare.inside = now;
        self.get_snared(id);
        true
    }

    /// 🕸️ 거미집(id)에 걸림 ("help!") — 그 자리에서 멈춤
    fn get_snared(&mut self, id: u64) {
        // 디딤판 위면 걷던 쪽, 공중이면 날던 쪽으로 출렁
        self.snare.spring_dir = if self.standing.is_some() { (self.facing.signum(), 0.0) } else { web::snag_dir(self.body.vx, self.body.vy) };
        self.snare_hold_still();
        self.speech = Some(Speech::Help);
        let remaining = behavior::range(&mut self.snare.rng, web::SNARE_SECS);
        self.state = State::Stuck { kind: StuckKind::Snare { web: id }, t: 0.0, remaining };
    }

    /// 🐛 거미인간(by)의 고치에 감김 ("mmph!") — 들 수 있는 동물만 (lift_ok, 조작 중 빼고). 감겼으면 true
    pub(super) fn be_cocooned(&mut self, by: u64) -> bool {
        if !self.lift_ok() || self.is_controlled() || self.social_key == by {
            return false;
        }
        self.snare_hold_still();
        self.speech = Some(Speech::Mmph);
        let wrap = f64::from(web::COCOON_SHOTS - 1) * web::WEAVE_SHOT_SECS;
        let remaining = wrap + behavior::range(&mut self.snare.rng, web::COCOON_SECS);
        self.state = State::Stuck { kind: StuckKind::Cocoon { by }, t: 0.0, remaining };
        true
    }

    /// 걸리는 순간: 하던 일·속도를 잊고 멈춤
    fn snare_hold_still(&mut self) {
        self.pacing.after_beat = None;
        self.clear_hover();
        self.thrown = false;
        self.body.vx = 0.0;
        self.body.vy = 0.0;
        self.snare.talk_in = web::STUCK_TALK_SECS;
    }

    /// 버둥 한 틱: 디딤판을 따라가고, 가끔 "mmph!"/"help!", 시간이 다 되면 빠져나옴. 걸린 거미집이 사라지거나 볼일이 생기면 그냥 풀림
    fn step_stuck(&mut self, dt: f64, world: &World, kind: StuckKind, t: f64, remaining: f64) {
        if self.standing.is_some() && !self.follow_anchor(world) {
            self.standing = None;
        }
        self.body.vx = 0.0;
        self.body.vy = 0.0;
        let web_gone = matches!(kind, StuckKind::Snare { web } if !self.snare.webs.iter().any(|w| w.id == web));
        if web_gone || self.housed || self.has_errand() || self.is_heading_out() || self.is_controlled() {
            self.snare_let_go(None);
            return;
        }
        let (t, remaining) = (t + dt, remaining - dt);
        if remaining <= 0.0 {
            self.escape(Speech::BrokeFree);
            return;
        }
        self.snare.talk_in -= dt;
        if self.snare.talk_in <= 0.0 {
            self.snare.talk_in = web::STUCK_TALK_SECS;
            self.speech = Some(if matches!(kind, StuckKind::Cocoon { .. }) { Speech::Mmph } else { Speech::Help });
        }
        self.state = State::Stuck { kind, t, remaining };
    }

    /// 빠져나옴 (speech) — 거미집이었으면 찢음 (take_torn_web)
    fn escape(&mut self, speech: Speech) {
        let torn = match self.state {
            State::Stuck { kind: StuckKind::Snare { web }, .. } => Some(web),
            _ => None,
        };
        if let Some(id) = torn {
            self.snare.torn = Some(id);
            self.snare.webs.retain(|w| w.id != id);
            self.snare.inside.retain(|&w| w != id);
        }
        self.snare_let_go(Some(speech));
    }

    /// 풀려남: 한동안 안 걸리고, 디딤판 위면 잠깐 쉬고 공중이면 떨어짐
    fn snare_let_go(&mut self, speech: Option<Speech>) {
        self.snare.immune_until = self.clock + web::SNARE_IMMUNE_SECS;
        if speech.is_some() {
            self.speech = speech;
        }
        if self.standing.is_some() {
            self.state = self.rest_beat();
        } else {
            self.start_fall();
        }
    }

    /// 🐛🕸️ 사용자가 눌러 풀어 줌 ("thanks!" — 거미집은 찢어짐). 걸려 있었으면 true
    pub(super) fn free_stuck(&mut self) -> bool {
        if !self.is_stuck() {
            return false;
        }
        self.escape(Speech::Thanks);
        true
    }

    /// 걸린 동안의 그림: 고치는 서 있는 그림 (웹뷰가 실 띠로 덮음), 거미집은 잡힌 그림으로 버둥
    pub(super) fn stuck_anim(kind: StuckKind) -> Anim {
        match kind {
            StuckKind::Cocoon { .. } => Anim::Idle,
            StuckKind::Snare { .. } => Anim::Dragged,
        }
    }

    /// 🕸️ 거미집에 걸린 동안 그림이 몸 자리에서 비켜난 만큼 (물리 px): "팅~" 출렁 뒤 살랑살랑. 아니면 (0, 0)
    pub(super) fn snare_offset(&self) -> (f64, f64) {
        let State::Stuck { kind: StuckKind::Snare { .. }, t, .. } = self.state else { return (0.0, 0.0) };
        let amp = web::SNAG_STRETCH_PET * self.cfg.size;
        let (sx, sy) = web::snag_spring(self.snare.spring_dir, amp, t);
        (sx + web::snag_sway(amp, t), sy)
    }

    /// 🕸️ 화면의 보이는·가려진 거미집 모두 (앱이 set_webs로 알려 준 것 — 거미집 놀이가 봄)
    pub(super) fn known_webs(&self) -> &[WebSpot] {
        &self.snare.webs
    }

    /// 🐛 고치 실 감긴 겹 수 (render.cocoon): 감긴 순간 한 겹, 줄이 닿을 때마다 한 겹씩 COCOON_SHOTS까지
    pub(super) fn cocoon_bands(&self) -> u8 {
        match self.state {
            State::Stuck { kind: StuckKind::Cocoon { .. }, t, .. } => (1 + (t / web::WEAVE_SHOT_SECS) as u32).min(web::COCOON_SHOTS) as u8,
            _ => 0,
        }
    }
}

/// 🐛 앱이 매 틱 동물 update 뒤에 (sync_lifts가 부름): 거미인간의 첫 고치 줄이 닿았으면 그 동물을 감고 (못 감으면 거미인간이 그만둠),
/// 감던 동물이 그새 풀려났으면 (클릭) 거미인간도 줄을 거둠
pub(super) fn sync_cocoons<T: AsRef<Pet> + AsMut<Pet>>(pets: &mut [T]) {
    let find = |pets: &[T], key: u64| pets.iter().position(|p| p.as_ref().social_key == key);
    for i in 0..pets.len() {
        let spider = pets[i].as_ref().social_key;
        if let Some(key) = pets[i].as_mut().web.cocoon_pet.take() {
            let ok = find(pets, key).is_some_and(|j| j != i && pets[j].as_mut().be_cocooned(spider));
            if !ok {
                pets[i].as_mut().lose_wrap();
            }
            continue;
        }
        if let Some(key) = pets[i].as_ref().wrapping() {
            let held = find(pets, key).is_some_and(|j| matches!(pets[j].as_ref().state, State::Stuck { kind: StuckKind::Cocoon { by }, .. } if by == spider));
            if !held {
                pets[i].as_mut().lose_wrap();
            }
        }
    }
}
