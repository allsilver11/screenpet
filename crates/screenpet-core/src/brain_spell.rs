//! ✨ 마법사 주문 (pet.json `spells` + cast 그림 — PetConfig::spells, 해리·헤르미온느) + Leviosa에 둥둥 뜨기 (누구나) — brain.rs의 자식 모듈.
//! 숫자는 wizard.rs.
//!
//! - 한가할 때 가끔 SPELL_REACH 안의 대상을 골라 그쪽을 보고 지팡이를 휙 (State::Cast — cast 그림, 맞는 때 CAST_HIT부터 반짝이):
//!   · Expelliarmus: 물건을 든 펫이 있으면 그 펫이 떨어뜨림 (sync_spells → be_disarmed — 물건은 앱이 떨어뜨림), 아니면 바닥의 작은 물건을 휙 날림 (ObjectPlay::Yank)
//!   · Wingardium Leviosa: 작은 펫 (시전자 크기의 FLOAT_PET_MAX배까지, 들 수 있는 때)이면 그 펫이 둥둥 (be_levitated → State::Float),
//!     물건이면 앱이 띄움 (ObjectPlay::Levitate). 책벌레(헤르미온느)가 아니면 FIZZLE_CHANCE로 "Leviosaaa?" 실패
//!   · Reparo: 바닥의 작은 물건 하나를 반짝 치움 (ObjectPlay::Vanish — 손님일 땐 친구 물건을 치우지 않음)
//!   📌 장난감 아이콘·거대한 것은 노리지 않음.
//! - ✨ 둥둥 (State::Float): 몸은 디딤판에 둔 채 창만 wizard::float_height만큼 떠올라 살랑 → 내려옴 (dragged 그림 + 반짝이). 디딤판이 사라지면 떨어짐
//! - 훅: casting (지금 거는 주문 — 나중에 결투·패트로누스가 봄)
//!
//! 주사위는 자기 주사위(SpellMood.rng)로 굴림 — 평소 동작과 다른 동물의 주사위 순서는 그대로.

use super::hoist_moves::PetSpot;
use super::*;
use crate::play::ObjectPlay;
use crate::wizard::{self as wz, Spell};

/// 주문의 대상
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SpellTarget {
    /// 다른 펫 (social_key, 발 x)
    Pet { key: u64, x: f64 },
    /// 바닥의 물건 (id, 바닥 가운데 x)
    Object { id: u64, x: f64 },
}

/// 지금 거는 주문 (State::Cast)
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Cast {
    pub spell: Spell,
    pub target: SpellTarget,
    /// 실패한 주문 ("Leviosaaa?" — 아무 일도 안 일어남)
    pub fizzle: bool,
    /// 이미 맞았음
    pub hit: bool,
}

/// 주문에 맞은 펫에게 할 일 (sync_spells가 그 펫에게)
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PetSpell {
    /// 든 물건을 떨어뜨림
    Disarm { key: u64 },
    /// secs초 동안 둥둥
    Levitate { key: u64, secs: f64 },
}

/// 주문의 기억
#[derive(Debug, Clone)]
pub struct SpellMood {
    rng: fastrand::Rng,
    /// 이 시각(clock)부터 주문을 걸 수 있음
    pub(super) ready_at: f64,
    /// 같은 화면의 다른 동물 (notice_pets가 채움)
    pub(super) pets: Vec<PetSpot>,
    /// 주문에 맞은 펫에게 할 일 (sync_spells가 한 번 꺼냄)
    pub(super) pet_event: Option<PetSpell>,
}

impl SpellMood {
    /// 시드로 난수를 따로 둠. 첫 주문은 SPELL_FIRST_SECS 뒤부터
    pub(super) fn new(seed: u64) -> Self {
        SpellMood { rng: fastrand::Rng::with_seed(seed ^ 0x7370_656c), ready_at: wz::SPELL_FIRST_SECS, pets: Vec::new(), pet_event: None }
    }
}

impl Pet {
    /// ✨ 지금 거는 주문 (지팡이를 휘두르는 동안 — 결투·다른 펫이 봄)
    pub fn casting(&self) -> Option<Spell> {
        match self.state {
            State::Cast { cast, .. } => Some(cast.spell),
            _ => None,
        }
    }

    /// 반짝이 (주문이 맞은 뒤 지팡이 끝·둥둥 뜬 동안 — 웹뷰가 뿌림)
    pub(super) fn spell_sparkle(&self) -> bool {
        match self.state {
            State::Cast { cast, t } => !cast.fizzle && t >= wz::CAST_HIT,
            State::Float { .. } => true,
            _ => false,
        }
    }

    /// 창을 띄우는 높이 (물리 px — Leviosa에 떠 있는 동안)
    pub(super) fn float_lift(&self) -> f64 {
        match self.state {
            State::Float { t, len, rise } => wz::float_height(t, len, rise),
            _ => 0.0,
        }
    }

    /// 매 틱 (빗자루 뒤): 거는 중이면 그 한 틱, 아니면 가끔 대상을 골라 주문. 처리했으면 true
    pub(super) fn update_spell(&mut self, dt: f64, world: &World) -> bool {
        if self.cfg.spells.is_empty() {
            return false;
        }
        if let State::Cast { cast, t } = self.state {
            self.tick_cast(world, cast, t + dt);
            return true;
        }
        if self.clock < self.spell.ready_at || !self.play_free() || self.romp_interrupted() || self.is_user_away() {
            return false;
        }
        if self.spell.rng.f64() >= wz::SPELL_RATE * self.power() * dt {
            return false;
        }
        self.start_spell()
    }

    /// 대상 하나를 골라 주문을 시작함 (없으면 false): 물건을 든 펫이 있으면 Expelliarmus를 먼저, 아니면 펫·물건 중 아무거나
    pub(super) fn start_spell(&mut self) -> bool {
        let spells = self.cfg.spells;
        let guest = self.guest.is_some();
        let (x, y, size) = (self.body.x, self.body.y, self.cfg.size);
        let reach = wz::SPELL_REACH * size;
        let near = |px: f64, py: f64| (px - x).abs() <= reach && (py - y).abs() <= reach && (px - x).abs() > 1.0;
        let mut options: Vec<(Spell, SpellTarget)> = Vec::new();
        let mut disarm: Vec<(Spell, SpellTarget)> = Vec::new();
        for p in self.spell.pets.iter().filter(|p| p.owner.is_some() && near(p.x, p.y)) {
            let target = SpellTarget::Pet { key: p.key, x: p.x };
            if spells.has(Spell::Expelliarmus) && p.carrying {
                disarm.push((Spell::Expelliarmus, target));
            }
            if spells.has(Spell::Leviosa) && p.liftable && p.size <= wz::FLOAT_PET_MAX * size {
                options.push((Spell::Leviosa, target));
            }
        }
        let pet_options = options.len();
        for o in self.play.spots.iter().filter(|o| !o.local && !o.giant && near(o.x, o.y)) {
            let target = SpellTarget::Object { id: o.id, x: o.x };
            for spell in [Spell::Expelliarmus, Spell::Leviosa, Spell::Reparo] {
                // 손님은 친구 화면의 물건을 치우지 않음 (Reparo)
                if spells.has(spell) && !(spell == Spell::Reparo && guest) {
                    options.push((spell, target));
                }
            }
        }
        let rng = &mut self.spell.rng;
        let pick = if !disarm.is_empty() {
            disarm[rng.usize(..disarm.len())]
        } else if options.is_empty() {
            return false;
        } else if pet_options > 0 && (pet_options == options.len() || rng.f64() < wz::PET_TARGET_CHANCE) {
            options[rng.usize(..pet_options)]
        } else {
            options[pet_options + rng.usize(..options.len() - pet_options)]
        };
        let (spell, target) = pick;
        self.begin_cast(spell, target);
        true
    }

    /// 대상 쪽을 보고 지팡이를 들어 주문을 외침 (실패할지도 여기서 정함)
    pub(super) fn begin_cast(&mut self, spell: Spell, target: SpellTarget) {
        self.pacing.after_beat = None;
        self.clear_hover();
        let tx = match target {
            SpellTarget::Pet { x, .. } | SpellTarget::Object { x, .. } => x,
        };
        if (tx - self.body.x).abs() > 1.0 {
            self.facing = (tx - self.body.x).signum();
        }
        let fizzle = spell == Spell::Leviosa && !self.cfg.bookworm && self.spell.rng.f64() < wz::FIZZLE_CHANCE;
        self.speech = Some(match spell {
            Spell::Expelliarmus => Speech::Expelliarmus,
            Spell::Leviosa => Speech::Leviosa,
            Spell::Reparo => Speech::Reparo,
        });
        self.state = State::Cast { cast: Cast { spell, target, fizzle, hit: false }, t: 0.0 };
    }

    /// 주문 한 틱: CAST_HIT에 맞히고, CAST_SECS 뒤 쉼
    fn tick_cast(&mut self, world: &World, cast: Cast, t: f64) {
        if !self.follow_anchor(world) {
            self.start_fall();
            return;
        }
        if self.romp_interrupted() {
            self.state = self.idle_state();
            return;
        }
        let mut cast = cast;
        if !cast.hit && t >= wz::CAST_HIT {
            cast.hit = true;
            self.spell_hits(cast);
        }
        if t < wz::CAST_SECS {
            self.state = State::Cast { cast, t };
            return;
        }
        self.spell.ready_at = self.clock + behavior::range(&mut self.spell.rng, wz::SPELL_CALM_SECS);
        self.state = self.rest_beat();
    }

    /// 주문이 맞음: 펫이면 sync_spells에, 물건이면 앱에 알림. 실패면 "Leviosaaa?"
    fn spell_hits(&mut self, cast: Cast) {
        if cast.fizzle {
            self.speech = Some(Speech::Fizzle);
            return;
        }
        let dpi = self.cfg.dpi;
        match (cast.spell, cast.target) {
            (Spell::Expelliarmus, SpellTarget::Pet { key, .. }) => self.spell.pet_event = Some(PetSpell::Disarm { key }),
            (Spell::Leviosa, SpellTarget::Pet { key, .. }) => {
                let secs = behavior::range(&mut self.spell.rng, wz::FLOAT_SECS);
                self.spell.pet_event = Some(PetSpell::Levitate { key, secs });
            }
            (Spell::Expelliarmus, SpellTarget::Object { id, .. }) => {
                self.play.event = Some(ObjectPlay::Yank { id, vx: self.facing * wz::DISARM_VX * dpi, vy: wz::DISARM_VY * dpi });
            }
            (Spell::Leviosa, SpellTarget::Object { id, .. }) => {
                let secs = behavior::range(&mut self.spell.rng, wz::FLOAT_SECS);
                self.play.event = Some(ObjectPlay::Levitate { id, rise: wz::FLOAT_RISE * self.cfg.size, secs });
            }
            (Spell::Reparo, SpellTarget::Object { id, .. }) => self.play.event = Some(ObjectPlay::Vanish { id }),
            (Spell::Reparo, SpellTarget::Pet { .. }) => {}
        }
    }

    /// ✨ Expelliarmus에 맞음: 든 물건을 놓침 ("hmph!" — 물건은 앱이 떨어뜨림)
    pub fn be_disarmed(&mut self) -> bool {
        if self.carried_object().is_none() {
            return false;
        }
        self.lose_carry();
        true
    }

    /// ✨ Leviosa에 맞음: 들 수 있는 때 (서서 쉬거나 걷거나 자는 중)면 secs초 동안 둥둥. 떴으면 true
    pub fn be_levitated(&mut self, secs: f64) -> bool {
        if !self.lift_ok() || self.cfg.flies {
            return false;
        }
        self.pacing.after_beat = None;
        self.clear_hover();
        self.speech = Some(Speech::Whoa);
        self.state = State::Float { t: 0.0, len: secs, rise: wz::FLOAT_RISE * self.cfg.size };
        true
    }

    /// 둥둥 한 틱 (누구나): 디딤판을 따라가고, 다 내려오면 쉼. 처리했으면 true
    pub(super) fn update_float(&mut self, dt: f64, world: &World) -> bool {
        let State::Float { t, len, rise } = self.state else { return false };
        if !self.follow_anchor(world) {
            self.start_fall();
            return true;
        }
        let t = t + dt;
        self.state = if t < len { State::Float { t, len, rise } } else { self.rest_beat() };
        true
    }
}

/// ✨ 앱이 매 틱 동물 update 뒤에 (sync_lifts가 부름): 주문에 맞은 동물이 든 것을 놓치거나 둥둥 뜸
pub(super) fn sync_spells<T: AsRef<Pet> + AsMut<Pet>>(pets: &mut [T]) {
    for i in 0..pets.len() {
        let Some(ev) = pets[i].as_mut().spell.pet_event.take() else { continue };
        let key = match ev {
            PetSpell::Disarm { key } | PetSpell::Levitate { key, .. } => key,
        };
        let Some(j) = pets.iter().position(|p| p.as_ref().social_key == key) else { continue };
        if j == i {
            continue;
        }
        let target = pets[j].as_mut();
        match ev {
            PetSpell::Disarm { .. } => {
                target.be_disarmed();
            }
            PetSpell::Levitate { secs, .. } => {
                target.be_levitated(secs);
            }
        }
    }
}
