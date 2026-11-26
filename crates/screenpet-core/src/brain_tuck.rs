//! 🎒 가방에 숨기 (2026-11-20 spec 추가 요청) — brain.rs의 자식 모듈. 숫자는 wizard.rs (TUCK_* · HIDE_*).
//!
//! 해리·헤르미온느·볼드모트가 한 화면에 모이면 평소 결투 (해리 vs 볼드모트)보다 먼저:
//! 1. try_tuck (sync_duels가 볼드모트와의 결투를 막 시작하려 할 때 부름): 가방 주인 (pet.json `bag_cleaner` — 헤르미온느)이 한가하고, 곁 (TUCK_REACH)에
//!    빨강 빛줄기 마법사 (Expelliarmus — 해리)가 한가하고, 볼드모트가 결투 거리에 있으면 (둘 중 하나만 결투 쉬는 때가 지났어도 — 헤르미온느가 끼어듦)
//!    TUCK_CHANCE로 헤르미온느 "In you go!" — 가방을 벌려 들고 (bag_stuff, Duel.hold = HIDE_SECS)
//!    해리는 State::Tucked로 TUCK_SECS 동안 작아지며 가방 쪽으로 쏙 (render.tuck — 웹뷰가 줄이며 흐리게) → 그 뒤 헤르미온느 (금빛)·볼드모트의 긴 결투 한 판 (brain_duel.rs — 가방을 든 채 달아나고 쫓음)
//! 2. sync_tucks (sync_lifts가 부름): 가방 속 해리는 가방 (헤르미온느 앞)을 따라다님. 헤르미온느의 가방 결투가 끝나면 (펑·끊김·헤르미온느가 떠남)
//!    해리가 "Phew!" 하며 튀어나와 내려섬 (TUCK_POP), 다음 결투는 DUEL_CALM_SECS 뒤
//! 3. 해리가 불려 가면 (간질이기·먹이·집 가기 등) 혼자 튀어나옴. 길어도 TUCK_MAX_SECS면 나옴
//!
//! 주사위는 결투 주사위 (DuelMood.rng)로 굴림 — 평소 동작과 다른 동물의 주사위 순서는 그대로.

use super::duel_moves::Duel;
use crate::duel_bout::Bout;
use super::*;
use crate::magic::BeamColor;
use crate::wizard as wz;

/// 🎒 가방이 몸 앞으로 나온 자리 (몸 크기 배 — 해리가 이리로 쏙)
const BAG_AHEAD: f64 = 0.3;

impl Pet {
    /// 🎒 가방 속에 숨어 있으면 가방 주인 (social_key)
    pub fn tucked_in(&self) -> Option<u64> {
        match self.state {
            State::Tucked { owner, .. } => Some(owner),
            _ => None,
        }
    }

    /// 🎒 웹뷰가 그림을 줄이는 몫 (0 = 그대로 … 1 = 다 들어가 안 보임)
    pub(super) fn tuck_shown(&self) -> f64 {
        match self.state {
            State::Tucked { t, .. } => (t / wz::TUCK_SECS).clamp(0.0, 1.0),
            _ => 0.0,
        }
    }

    /// 🎒 가방 주인 (owner)의 가방으로 쏙 들어가기 시작함
    fn begin_tuck(&mut self, owner: u64) {
        self.pacing.after_beat = None;
        self.clear_hover();
        self.body.vx = 0.0;
        self.body.vy = 0.0;
        self.state = State::Tucked { owner, t: 0.0, from: self.body.x };
    }

    /// 🎒 가방에서 "Phew!" 하며 튀어나옴 (at = 나올 자리 — 없으면 제자리, side = 튀어 갈 쪽 ±1). 다음 결투는 DUEL_CALM_SECS 뒤
    fn pop_out(&mut self, at: Option<(f64, f64)>, side: f64) {
        if let Some((x, y)) = at {
            self.body.x = x;
            self.body.y = y;
        }
        self.duel.ready_at = self.clock + behavior::range(&mut self.duel.rng, wz::DUEL_CALM_SECS);
        let dpi = self.cfg.dpi;
        self.poop.stepping_off();
        self.standing = None;
        self.body.vx = side * wz::TUCK_POP_VX * dpi;
        self.body.vy = wz::TUCK_POP_VY * dpi;
        self.thrown = false;
        self.gentle_drop = true;
        self.speech = Some(Speech::PoppedOut);
        self.state = State::Air { anim: Anim::Hop };
    }

    /// 🎒 가방 속 한 틱 (자리는 sync_tucks가 옮김): 불려 가거나 너무 오래면 혼자 튀어나옴. 처리했으면 true
    pub(super) fn update_tucked(&mut self, dt: f64) -> bool {
        let State::Tucked { owner, t, from } = self.state else { return false };
        if self.romp_interrupted() || self.social_interrupted() || t >= wz::TUCK_MAX_SECS {
            let side = self.facing;
            self.pop_out(None, side);
            return true;
        }
        self.body.vx = 0.0;
        self.body.vy = 0.0;
        self.state = State::Tucked { owner, t: t + dt, from };
        true
    }
}

/// 🎒 a 곁 (가로 TUCK_REACH, 세로 DUEL_RISE — 몸 크기 배, 큰 쪽)에 b가 있음
fn beside(a: &Pet, b: &Pet) -> bool {
    let big = a.cfg.size.max(b.cfg.size);
    (a.body.x - b.body.x).abs() <= wz::TUCK_REACH * big && (a.body.y - b.body.y).abs() <= wz::DUEL_RISE * big
}

/// 🎒 가방 숨기기에 낄 수 있음: 한가하거나 (wizard_free) 잠깐 하던 짓은 멈추고 — 재주, 숨는 쪽 (hidee)은 졸기도, 가방 주인은 📖 책 읽기도 (책을 탁 덮음).
/// 밝은 쪽 마법사만
fn tuck_free(p: &Pet, hidee: bool) -> bool {
    if p.duel_side() != Some(true) {
        return false;
    }
    if p.wizard_free() {
        return true;
    }
    let brief = match p.state {
        State::Trick { .. } => true,
        State::Sleep { .. } => hidee,
        State::Study { act: super::study_moves::StudyAct::Read { .. }, .. } => !hidee,
        _ => false,
    };
    brief
        && p.standing.is_some()
        && !p.cfg.flies
        && !p.romp_interrupted()
        && !p.is_user_away()
        && !p.is_away()
        && !p.thrown
        && p.hover_since.is_none()
        && !p.rider_on
        && p.juggle.carry.is_none()
}

/// 🎒 a와 b가 결투 거리 안 (가로 DUEL_RANGE·세로 DUEL_RISE — 너무 붙었으면 가방을 벌리는 동안 물러남, update_duel)
fn in_range(a: &Pet, b: &Pet) -> bool {
    let big = a.cfg.size.max(b.cfg.size);
    (a.body.x - b.body.x).abs() <= wz::DUEL_RANGE * big && (a.body.y - b.body.y).abs() <= wz::DUEL_RISE * big
}

/// 🎒 sync_duels가 결투를 막 시작하려 할 때 (상대 = 볼드모트 v): 한가한 (tuck_free) 헤르미온느가 볼드모트와 결투 거리에 있고 그 곁 (TUCK_REACH)에
/// 한가한 해리가 있으면 TUCK_CHANCE로 해리를 가방에 숨기고 대신 결투를 시작함 (둘 중 하나만 결투 쉬는 때가 지났어도 — 헤르미온느가 끼어듦).
/// 주사위는 결투가 시작될 때 한 번만 (볼드모트 결투 주사위). 시작했으면 true
pub(super) fn try_tuck<T: AsRef<Pet> + AsMut<Pet>>(pets: &mut [T], v: usize) -> bool {
    let n = pets.len();
    let pick = (0..n).find_map(|m| {
        let a = pets[m].as_ref();
        if m == v || !a.cfg.bag_cleaner || !tuck_free(a, false) || a.study.bag_on || !in_range(a, pets[v].as_ref()) {
            return None;
        }
        let h = (0..n).find(|&h| {
            let b = pets[h].as_ref();
            h != m && h != v && tuck_free(b, true) && b.duel_color() == BeamColor::Red && beside(a, b) && (a.clock >= a.duel.ready_at || b.clock >= b.duel.ready_at)
        })?;
        Some((m, h))
    });
    let Some((m, h)) = pick else { return false };
    if pets[v].as_mut().duel.rng.f64() >= wz::TUCK_CHANCE {
        return false;
    }
    let (b, g) = (pets[m].as_ref(), pets[v].as_ref());
    let (tip, foe_tip) = (b.wand_tip_toward(g.body.x), g.wand_tip_toward(b.body.x));
    let (me_key, foe_key, light) = (b.social_key, g.social_key, b.duel_color());
    let (me_at, foe_at, me_size, foe_size) = ((b.body.x, b.body.y), (g.body.x, g.body.y), b.cfg.size, g.cfg.size);
    let plan = Bout::plan(&mut pets[m].as_mut().duel.rng, light);
    let duel = Duel { foe: foe_key, red: true, me: tip, foe_tip, foe_at, foe_size, len: plan.len, light, hold: wz::HIDE_SECS, bag: true };
    pets[v].as_mut().begin_duel(Duel { foe: me_key, red: false, me: foe_tip, foe_tip: tip, foe_at: me_at, foe_size: me_size, bag: false, ..duel }, plan.clone());
    pets[m].as_mut().begin_duel(duel, plan);
    pets[h].as_mut().begin_tuck(me_key);
    true
}

/// 🎒 앱이 매 틱 동물 update 뒤에 (sync_lifts가 부름): 가방 속 펫은 가방 (주인 앞)으로 쏙 들어가 따라다니고,
/// 주인의 가방 결투가 끝났거나 주인이 없으면 튀어나옴
pub(super) fn sync_tucks<T: AsRef<Pet> + AsMut<Pet>>(pets: &mut [T]) {
    for i in 0..pets.len() {
        let State::Tucked { owner, t, from } = pets[i].as_ref().state else { continue };
        let j = pets.iter().position(|p| p.as_ref().social_key == owner);
        let holding = j.filter(|&j| matches!(pets[j].as_ref().state, State::Duel { duel, .. } if duel.bag));
        match holding {
            Some(j) => {
                let o = pets[j].as_ref();
                let bag = o.body.x + o.facing * BAG_AHEAD * o.cfg.size;
                let k = (t / wz::TUCK_SECS).clamp(0.0, 1.0);
                let (x, y) = (from + (bag - from) * k, o.body.y);
                let p = pets[i].as_mut();
                p.body.x = x;
                p.body.y = y;
            }
            None => {
                let at = j.map(|j| {
                    let o = pets[j].as_ref();
                    (o.body.x + o.facing * BAG_AHEAD * o.cfg.size, o.body.y, o.facing)
                });
                let p = pets[i].as_mut();
                let side = at.map_or(-p.facing, |a| a.2);
                p.pop_out(at.map(|a| (a.0, a.1)), side);
            }
        }
    }
}
