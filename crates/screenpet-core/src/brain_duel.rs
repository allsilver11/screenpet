//! ⚔️ 해리·볼드모트 결투 + 🦌 해리의 패트로누스 (디멘터가 올 때) — brain.rs의 자식 모듈. 숫자는 wizard.rs, 그림은 덮개 창 (magic.rs → ui/magic.js).
//!
//! - ⚔️ 결투 (State::Duel — cast 그림): sync_duels (앱이 매 틱 — sync_lifts가 부름)가 주문을 아는 밝은 쪽 마법사 (해리 빨강·헤르미온느 금빛 —
//!   Stupefy가 아닌 누구나)와 Stupefy를 아는 펫 (초록 — 볼드모트)이 둘 다 한가하고 가까이 (DUEL_RANGE·DUEL_RISE) 있으면 함께 시작: 서로 마주 보고 지팡이를 겨눔
//!   ("Expelliarmus!" / "You again, Potter?") → 빨강·초록 빛줄기가 뻗어 가운데에서 맞부딪혀 (wizard::duel_clash) 밀고 당김 → 펑! 둘 다 뒤로 튕김.
//!   한쪽이 끊기면 (잡힘·볼일) 다른 쪽도 그만. 빛줄기는 빨강 쪽이 내놓음 (magic_fx → MagicFx::Duel)
//! - 🦌 패트로누스: sync_patronus가 patronus 펫 (해리) 곁 (PATRONUS_REACH)에 디멘터가 오거나, 같은 화면 (PATRONUS_SEE)에서 디멘터가
//!   다른 펫을 시무룩하게 하면 그 디멘터에게 "Expecto Patronum!" (State::Cast — brain_spell.rs) → 맞는 때 지팡이 끝에서 은빛 사슴이
//!   디멘터를 따라 달려가 (STAG_RUN_SECS) 닿으면 디멘터가 도망 (be_repelled — brain_dementor.rs), 사슴은 그 너머로 달려가며 흐려짐
//! - 훅: magic_fx (덮개 창에 그릴 마법 효과 모두 — ⚡ 빛줄기·⚔️ 결투·🐍 나기니·🦌 사슴·🥶 서리)
//!
//! 주사위는 자기 주사위(DuelMood.rng)로 굴림 — 평소 동작과 다른 동물의 주사위 순서는 그대로.

use super::spell_moves::SpellTarget;
use super::*;
use crate::magic::{BeamColor, MagicFx};
use crate::wizard::{self as wz, Spell};

/// ⚔️ 결투 하나 (두 펫이 같은 것을 하나씩 가짐 — 지팡이 끝은 물리 px)
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Duel {
    /// 상대 (social_key)
    pub foe: u64,
    /// 빨강 쪽 (해리 — 빛줄기를 내놓는 쪽)
    pub red: bool,
    /// 내 지팡이 끝 · 상대 지팡이 끝
    pub me: (f64, f64),
    pub foe_tip: (f64, f64),
    /// 펑까지 (초)
    pub len: f64,
    /// 밝은 쪽 빛줄기 색 (해리 빨강 · 헤르미온느 금빛)
    pub light: BeamColor,
}

/// 🦌 달리는 은빛 사슴 (물리 px)
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Stag {
    /// 쫓는 디멘터 (social_key)
    pub key: u64,
    /// 나온 자리 (지팡이 끝) · 디멘터 자리 (sync_patronus가 따라 바꿈)
    pub from: (f64, f64),
    pub to: (f64, f64),
    /// 나온 뒤 흐른 시간
    pub t: f64,
    /// 달리는 쪽 (±1 — 디멘터가 바로 위에 있을 때)
    pub dir: f64,
    /// 디멘터에게 닿았음
    pub hit: bool,
}

impl Stag {
    /// 지금 자리 (닿은 뒤엔 그 너머로)
    pub fn pos(&self) -> (f64, f64) {
        let k = wz::stag_progress(self.t);
        (self.from.0 + (self.to.0 - self.from.0) * k, self.from.1 + (self.to.1 - self.from.1) * k)
    }
}

/// 결투·패트로누스의 기억
#[derive(Debug, Clone)]
pub struct DuelMood {
    rng: fastrand::Rng,
    /// ⚔️ 이 시각(clock)부터 결투할 수 있음
    pub(super) ready_at: f64,
    /// 🦌 이 시각(clock)부터 패트로누스를 쓸 수 있음
    pub(super) patronus_ready_at: f64,
    /// 🦌 달리는 사슴
    pub(super) stag: Option<Stag>,
    /// 🦌 사슴에 닿은 디멘터 (key, 해리 x — sync_patronus가 한 번 꺼냄)
    repel: Option<(u64, f64)>,
}

impl DuelMood {
    /// 시드로 난수를 따로 둠. 첫 결투는 DUEL_FIRST_SECS 사이에서
    pub(super) fn new(seed: u64) -> Self {
        let mut rng = fastrand::Rng::with_seed(seed ^ 0x6475_656c);
        let ready_at = behavior::range(&mut rng, wz::DUEL_FIRST_SECS);
        DuelMood { rng, ready_at, patronus_ready_at: 0.0, stag: None, repel: None }
    }
}

impl Pet {
    /// ⚔️ 결투하는 쪽: Some(false) = 초록 (Stupefy — 볼드모트), Some(true) = 밝은 쪽 (그 밖에 주문을 아는 마법사 — 해리·헤르미온느), 아니면 None
    pub fn duel_side(&self) -> Option<bool> {
        if self.cfg.spells.has(Spell::Stupefy) {
            Some(false)
        } else if !self.cfg.spells.is_empty() {
            Some(true)
        } else {
            None
        }
    }

    /// ⚔️ 밝은 쪽 빛줄기 색: Expelliarmus를 알면 빨강 (해리), 아니면 금빛 (헤르미온느)
    pub fn duel_color(&self) -> BeamColor {
        if self.cfg.spells.has(Spell::Expelliarmus) {
            BeamColor::Red
        } else {
            BeamColor::Gold
        }
    }

    /// ⚔️ 지금 결투 중인 상대 (social_key)
    pub fn dueling(&self) -> Option<u64> {
        match self.state {
            State::Duel { duel, .. } => Some(duel.foe),
            _ => None,
        }
    }

    /// 🦌 달리는 은빛 사슴
    pub fn stag(&self) -> Option<Stag> {
        self.duel.stag
    }

    /// 한가해서 결투·패트로누스를 시작할 수 있음
    fn wizard_free(&self) -> bool {
        self.play_free() && !self.romp_interrupted() && !self.is_user_away() && !self.is_away()
    }

    /// ⚔️ 결투를 시작할 수 있음
    fn duel_ready(&self) -> bool {
        self.duel_side().is_some() && self.wizard_free() && self.clock >= self.duel.ready_at
    }

    /// 🦌 패트로누스를 쓸 수 있음
    fn patronus_ready(&self) -> bool {
        self.cfg.patronus && self.wizard_free() && self.clock >= self.duel.patronus_ready_at && self.duel.stag.is_none()
    }

    /// x 쪽을 볼 때의 지팡이 끝
    fn wand_tip_toward(&self, x: f64) -> (f64, f64) {
        let f = if (x - self.body.x).abs() > 1.0 { (x - self.body.x).signum() } else { self.facing };
        self.wand_tip_facing(f)
    }

    /// ⚔️ 결투 시작: 상대 쪽을 보고 지팡이를 겨눔 (빨강 "Expelliarmus!" · 초록 "You again, Potter?")
    fn begin_duel(&mut self, duel: Duel) {
        self.pacing.after_beat = None;
        self.clear_hover();
        if (duel.foe_tip.0 - self.body.x).abs() > 1.0 {
            self.facing = (duel.foe_tip.0 - self.body.x).signum();
        }
        self.speech = Some(if duel.red { Speech::Expelliarmus } else { Speech::DuelCall });
        self.state = State::Duel { duel, t: 0.0 };
    }

    /// ⚔️ 결투를 그만둠 (상대가 끊겼음): 쉬고, 다음 결투는 DUEL_CALM_SECS 뒤
    fn end_duel(&mut self) {
        self.duel.ready_at = self.clock + behavior::range(&mut self.duel.rng, wz::DUEL_CALM_SECS);
        self.state = self.rest_beat();
    }

    /// ⚔️ 펑! 뒤로·위로 튕겨 나감 ("whoa!" — 사뿐 떨어짐)
    fn duel_pop(&mut self) {
        self.duel.ready_at = self.clock + behavior::range(&mut self.duel.rng, wz::DUEL_CALM_SECS);
        let dpi = self.cfg.dpi;
        self.poop.stepping_off();
        self.standing = None;
        self.body.vx = -self.facing * wz::DUEL_BOUNCE_VX * dpi;
        self.body.vy = wz::DUEL_BOUNCE_VY * dpi;
        self.thrown = false;
        self.gentle_drop = true;
        self.speech = Some(Speech::Whoa);
        self.state = State::Air { anim: Anim::Fall };
    }

    /// ⚔️ 결투 한 틱: 디딤판을 따라 지팡이를 겨눈 채 버티다가 다 되면 펑. 처리했으면 true
    pub(super) fn update_duel(&mut self, dt: f64, world: &World) -> bool {
        let State::Duel { duel, t } = self.state else { return false };
        if !self.follow_anchor(world) {
            self.duel.ready_at = self.clock + behavior::range(&mut self.duel.rng, wz::DUEL_CALM_SECS);
            self.start_fall();
            return true;
        }
        if self.romp_interrupted() {
            self.end_duel();
            return true;
        }
        let t = t + dt;
        if t >= duel.len {
            self.duel_pop();
        } else {
            self.state = State::Duel { duel, t };
        }
        true
    }

    /// 🦌 디멘터 (key, 몸 가운데 x·y)에게 "Expecto Patronum!" (맞는 때 사슴이 나감 — release_stag)
    fn begin_patronus(&mut self, key: u64, x: f64, y: f64) {
        self.duel.patronus_ready_at = self.clock + behavior::range(&mut self.duel.rng, wz::PATRONUS_CALM_SECS);
        self.begin_cast_at(Spell::Patronus, SpellTarget::Pet { key, x }, (x, y));
    }

    /// 🦌 지팡이 끝에서 은빛 사슴이 디멘터 (key, aim)에게 달려 나감
    pub(super) fn release_stag(&mut self, key: u64, aim: (f64, f64)) {
        let from = self.wand_tip();
        let dir = if (aim.0 - from.0).abs() > 1.0 { (aim.0 - from.0).signum() } else { self.facing };
        self.duel.stag = Some(Stag { key, from, to: aim, t: 0.0, dir, hit: false });
    }

    /// 매 틱 (상태와 상관없이): 사슴이 달려 디멘터에게 닿으면 알리고 (repel), 다 달리면 사라짐
    pub(super) fn duel_tick(&mut self, dt: f64) {
        let Some(mut s) = self.duel.stag else { return };
        s.t += dt;
        if !s.hit && s.t >= wz::STAG_RUN_SECS {
            s.hit = true;
            self.duel.repel = Some((s.key, self.body.x));
        }
        self.duel.stag = if s.t < wz::STAG_SECS { Some(s) } else { None };
    }

    /// ✨ 덮개 창에 그릴 마법 효과 모두 (앱이 매 틱 — trail.rs): ⚡ 빛줄기 · ⚔️ 결투 (빨강 쪽만) · 🐍 나기니 · 🦌 사슴 · 🥶 서리
    pub fn magic_fx(&self, out: &mut Vec<MagicFx>) {
        self.spell_fx(out);
        self.dark_fx(out);
        self.dementor_fx(out);
        if let State::Duel { duel, t } = self.state {
            if duel.red {
                let k = wz::duel_clash(t, duel.len);
                let (r, g) = (duel.me, duel.foe_tip);
                let clash = (r.0 + (g.0 - r.0) * k, r.1 + (g.1 - r.1) * k);
                out.push(MagicFx::Duel { red: r, green: g, clash, reach: wz::duel_reach(t), width: wz::DUEL_WIDTH * self.cfg.size, light: duel.light });
            }
        }
        if let Some(s) = self.duel.stag {
            let (x, y) = s.pos();
            out.push(MagicFx::Stag { x, y, size: wz::STAG_SIZE * self.cfg.size, dir: s.dir, alpha: wz::stag_alpha(s.t) });
        }
    }
}

/// ⚔️ 앱이 매 틱 동물 update 뒤에 (sync_lifts가 부름): 상대가 끊긴 결투를 그만두고, 한가한 빨강·초록 둘이 가까이 있으면 결투를 시작함
pub(super) fn sync_duels<T: AsRef<Pet> + AsMut<Pet>>(pets: &mut [T]) {
    let find = |pets: &[T], key: u64| pets.iter().position(|p| p.as_ref().social_key == key);
    for i in 0..pets.len() {
        let Some(foe) = pets[i].as_ref().dueling() else { continue };
        let me = pets[i].as_ref().social_key;
        let ok = find(pets, foe).is_some_and(|j| j != i && pets[j].as_ref().dueling() == Some(me));
        if !ok {
            pets[i].as_mut().end_duel();
        }
    }
    for i in 0..pets.len() {
        if pets[i].as_ref().duel_side() != Some(true) || !pets[i].as_ref().duel_ready() {
            continue;
        }
        for j in 0..pets.len() {
            let (a, b) = (pets[i].as_ref(), pets[j].as_ref());
            if j == i || b.duel_side() != Some(false) || !b.duel_ready() {
                continue;
            }
            let big = a.cfg.size.max(b.cfg.size);
            let (dx, dy) = ((a.body.x - b.body.x).abs(), (a.body.y - b.body.y).abs());
            if dx > wz::DUEL_RANGE * big || dx < wz::DUEL_MIN * big || dy > wz::DUEL_RISE * big {
                continue;
            }
            let (red_tip, green_tip) = (a.wand_tip_toward(b.body.x), b.wand_tip_toward(a.body.x));
            let (red_key, green_key) = (a.social_key, b.social_key);
            let light = a.duel_color();
            let len = behavior::range(&mut pets[i].as_mut().duel.rng, wz::DUEL_SECS);
            pets[i].as_mut().begin_duel(Duel { foe: green_key, red: true, me: red_tip, foe_tip: green_tip, len, light });
            pets[j].as_mut().begin_duel(Duel { foe: red_key, red: false, me: green_tip, foe_tip: red_tip, len, light });
            break;
        }
    }
}

/// 🦌 앱이 매 틱 동물 update 뒤에 (sync_lifts가 부름): 디멘터가 오면 패트로누스를 쓰고, 달리는 사슴이 디멘터를 따라가게 하고, 닿은 디멘터를 쫓음
pub(super) fn sync_patronus<T: AsRef<Pet> + AsMut<Pet>>(pets: &mut [T]) {
    let center = |p: &Pet| (p.body.x, p.body.y - 0.5 * p.cfg.size);
    let dementors: Vec<(u64, (f64, f64), f64)> = pets.iter().map(|p| p.as_ref()).filter(|p| p.dementor_active()).map(|p| (p.social_key, center(p), p.cfg.size)).collect();
    // 닿은 디멘터를 쫓음 (사슴이 나온 해리 쪽 반대로)
    for i in 0..pets.len() {
        let Some((key, from_x)) = pets[i].as_mut().duel.repel.take() else { continue };
        if let Some(j) = pets.iter().position(|p| p.as_ref().social_key == key) {
            pets[j].as_mut().be_repelled(from_x);
        }
    }
    if dementors.is_empty() {
        return;
    }
    let victims: Vec<(u64, (f64, f64))> = pets.iter().map(|p| p.as_ref()).filter(|p| !p.cfg.dementor && !p.is_away()).map(|p| (p.social_key, center(p))).collect();
    let dist = |a: (f64, f64), b: (f64, f64)| (a.0 - b.0).hypot(a.1 - b.1);
    for p in pets.iter_mut() {
        let p = p.as_mut();
        // 달리는 사슴은 디멘터를 따라감
        if let Some(mut s) = p.duel.stag.filter(|s| !s.hit) {
            if let Some(&(_, at, _)) = dementors.iter().find(|d| d.0 == s.key) {
                s.to = at;
                p.duel.stag = Some(s);
            }
        }
        if !p.patronus_ready() {
            continue;
        }
        let (me, size) = (center(p), p.cfg.size);
        let pick = dementors
            .iter()
            .filter(|&&(key, at, dsize)| {
                let d = dist(me, at);
                key != p.social_key
                    && (d <= wz::PATRONUS_REACH * size
                        || (d <= wz::PATRONUS_SEE * size && victims.iter().any(|&(vk, v)| vk != key && dist(v, at) <= wz::GLOOM_REACH * dsize)))
            })
            .min_by(|a, b| dist(me, a.1).total_cmp(&dist(me, b.1)))
            .copied();
        if let Some((key, at, _)) = pick {
            p.begin_patronus(key, at.0, at.1);
        }
    }
}
