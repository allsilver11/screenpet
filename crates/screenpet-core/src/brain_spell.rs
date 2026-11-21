//! ✨ 마법사 주문 (pet.json `spells` + cast 그림 — PetConfig::spells, 해리·헤르미온느·볼드모트) + Leviosa에 둥둥 뜨기 (누구나) — brain.rs의 자식 모듈.
//! 숫자는 wizard.rs.
//!
//! - 한가할 때 가끔 SPELL_REACH 안의 대상을 골라 그쪽을 보고 지팡이를 휙 (State::Cast — cast 그림, 맞는 때 CAST_HIT부터 반짝이):
//!   · Expelliarmus: 물건을 든 펫이 있으면 그 펫이 떨어뜨림 (sync_spells → be_disarmed — 물건은 앱이 떨어뜨림), 아니면 바닥의 작은 물건을 휙 날림 (ObjectPlay::Yank)
//!   · Wingardium Leviosa: 작은 펫 (시전자 크기의 FLOAT_PET_MAX배까지, 들 수 있는 때)이면 그 펫이 둥둥 (be_levitated → State::Float),
//!     물건이면 앱이 띄움 (ObjectPlay::Levitate). 책벌레(헤르미온느)가 아니면 FIZZLE_CHANCE로 "Leviosaaa?" 실패
//!   · Reparo: 바닥의 작은 물건 하나를 반짝 치움 (ObjectPlay::Vanish — 손님일 땐 친구 물건을 치우지 않음)
//!   · ⚡ Stupefy (볼드모트): 초록 빛줄기 (spell_fx → 덮개 창) — 들 수 있는 때의 펫이면 💫 잠깐 기절 (be_stunned, brain_dark.rs), 물건이면 휙 날림
//!   · 🐍 Serpensortia·🦌 Patronus는 여기서 고르지 않음 — brain_dark.rs (나기니)·brain_duel.rs (디멘터가 올 때)가 begin_cast_at으로 부름
//!   · 🪶 Leviosa로 앱 아이콘 (0.7.2): 내 화면 아래 바닥에서 장난감 아이콘을 만들 수 있으면 (앱 set_icon_lift_ok — 흐물이 Toys.ready와 같음)
//!     가끔 (ICON_LIFT_CHANCE) 발 앞 바닥을 보고 "Wingardium Leviosa!" — 흐물이처럼 JuggleEvent::Pull로 아이콘을 달라고 하고, 앱이 그 자리
//!     (icon_lift_wanted)에 장난감 아이콘 물건을 만들어 바로 둥둥 띄움 (icon_lifted — 내려앉은 뒤엔 보통 장난감 아이콘).
//!     못 받으면 (icon_lift_failed · ICON_WAIT_SECS 넘김) 곁의 보통 물건을 띄움
//!   · 💡 Lumos (주문을 아는 마법사 누구나 — 주문 주사위가 맞으면 LUMOS_CHANCE): 지팡이를 들고 LUMOS_SECS 동안 끝에 밝은 불빛
//!     (MagicFx::Glow), 맞는 때 앱에 한 번 알림 (take_lumos — 🌗 lumos.rs 화면 밝히기)
//!   · 🌑 Nox (Lumos의 반대 — 그다음 몫 NOX_CHANCE): "Nox!" 지팡이 끝 불이 훅 꺼지며 (잠깐 흐려지는 Glow) 작은 검은 연기 (MagicFx::Puff),
//!     맞는 때 앱에 한 번 알림 (take_nox — 🌗 lumos.rs 화면 어둡게)
//!   📌 장난감 아이콘·거대한 것은 노리지 않음.
//! - ✨ 둥둥 (State::Float): 몸은 디딤판에 둔 채 창만 wizard::float_height만큼 떠올라 살랑 → 내려옴 (dragged 그림 + 반짝이). 디딤판이 사라지면 떨어짐
//! - 훅: casting (지금 거는 주문 — 다른 펫이 봄) · wand_tip (지팡이 끝 — 빛줄기·사슴 자리)
//!
//! 주사위는 자기 주사위(SpellMood.rng)로 굴림 — 평소 동작과 다른 동물의 주사위 순서는 그대로.

use super::hoist_moves::PetSpot;
use super::*;
use crate::magic::{BeamColor, MagicFx};
use crate::play::ObjectPlay;
use crate::wizard::{self as wz, Spell};

/// 주문의 대상
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SpellTarget {
    /// 다른 펫 (social_key, 발 x)
    Pet { key: u64, x: f64 },
    /// 바닥의 물건 (id, 바닥 가운데 x)
    Object { id: u64, x: f64 },
    /// 발밑 바닥의 한 자리 (🐍 나기니가 나올 곳 · 💡 Lumos · 🌑 Nox)
    Floor { x: f64 },
    /// 🪶 작업표시줄에서 뽑아 띄울 앱 아이콘 자리 (바닥 x)
    Icon { x: f64 },
}

/// 🪶 앱 아이콘을 띄울 자리 (물리 px — 앱이 바닥 (x, y)에 장난감 아이콘을 만들어 rise만큼 secs초 띄움)
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct IconLift {
    pub x: f64,
    pub y: f64,
    pub rise: f64,
    pub secs: f64,
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
    /// 노리는 자리 (물리 px — ⚡ 빛줄기·🦌 사슴이 가는 곳)
    pub aim: (f64, f64),
    /// 휘두른 뒤 지팡이를 들고 있는 시간 (초 — 💡 Lumos·🌑 Nox, 아니면 0)
    pub hold: f64,
}

/// 주문에 맞은 펫에게 할 일 (sync_spells가 그 펫에게)
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PetSpell {
    /// 든 물건을 떨어뜨림
    Disarm { key: u64 },
    /// secs초 동안 둥둥
    Levitate { key: u64, secs: f64 },
    /// ⚡ secs초 동안 💫 기절
    Stun { key: u64, secs: f64 },
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
    /// 💡🪶 Lumos·아이콘 띄우기 주사위 (평소 주문 주사위 순서는 그대로)
    extra_rng: fastrand::Rng,
    /// 💡 Lumos가 막 켜짐 (앱이 take_lumos로 한 번 꺼냄)
    lumos_lit: bool,
    /// 🌑 Nox로 막 꺼짐 (앱이 take_nox로 한 번 꺼냄)
    nox_out: bool,
    /// 🪶 지금 장난감 아이콘을 만들 수 있음 (앱이 매 틱 — 내 펫만)
    icon_ok: bool,
    /// 🪶 기다리는 아이콘 자리와 그만 기다릴 시각 (clock)
    icon_wait: Option<(IconLift, f64)>,
}

impl SpellMood {
    /// 시드로 난수를 따로 둠. 첫 주문은 SPELL_FIRST_SECS 뒤부터
    pub(super) fn new(seed: u64) -> Self {
        SpellMood {
            rng: fastrand::Rng::with_seed(seed ^ 0x7370_656c),
            ready_at: wz::SPELL_FIRST_SECS,
            pets: Vec::new(),
            pet_event: None,
            extra_rng: fastrand::Rng::with_seed(seed ^ 0x6c75_6d6f),
            lumos_lit: false,
            nox_out: false,
            icon_ok: false,
            icon_wait: None,
        }
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
            State::Duel { .. } => match self.duel_view() {
                Some((_, crate::duel_bout::Act::Beam { u, .. })) => wz::duel_reach(u) >= 1.0,
                Some((_, crate::duel_bout::Act::Shoot { u, .. })) => u >= crate::duel_bout::SHOT_FIRE,
                _ => false,
            },
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

    /// 지팡이 끝 (물리 px — cast 마지막 칸의 pet.json grip.cast, 없으면 앞 위): ⚡ 빛줄기·⚔️ 결투·🦌 사슴이 나가는 자리
    pub(super) fn wand_tip(&self) -> (f64, f64) {
        self.wand_tip_facing(self.facing)
    }

    /// facing (±1) 쪽을 볼 때의 지팡이 끝 (물리 px)
    pub(super) fn wand_tip_facing(&self, facing: f64) -> (f64, f64) {
        let g = self.cfg.grip.cast;
        let (gx, gy) = if g.n > 0 { g.pts[g.n - 1] } else { (0.26, 0.62) };
        let size = self.cfg.size;
        (self.body.x + facing * gx * size, self.body.y - size + gy * size)
    }

    /// ⚡ Stupefy 빛줄기 (맞는 때 조금 앞부터 휘두르기가 끝날 때까지 — 덮개 창이 그림) · 💡 Lumos 불빛 (맞는 때부터 들고 있는 동안) ·
    /// 🌑 Nox 연기 (맞는 때부터 끝까지 — 처음 NOX_SNUFF_SECS는 꺼져 가는 불빛도)
    pub(super) fn spell_fx(&self, out: &mut Vec<MagicFx>) {
        if let State::Cast { cast, t } = self.state {
            if cast.spell == Spell::Lumos && t >= wz::CAST_HIT {
                let end = wz::CAST_SECS + cast.hold;
                let level = ((t - wz::CAST_HIT) / wz::LUMOS_RAMP_SECS).min((end - t) / wz::LUMOS_FADE_SECS).clamp(0.0, 1.0);
                let (x, y) = self.wand_tip();
                out.push(MagicFx::Glow { x, y, size: wz::LUMOS_GLOW * self.cfg.size, level });
            }
            if cast.spell == Spell::Nox && t >= wz::CAST_HIT {
                let (x, y) = self.wand_tip();
                let since = t - wz::CAST_HIT;
                let snuff = 1.0 - since / wz::NOX_SNUFF_SECS;
                if snuff > 0.0 {
                    out.push(MagicFx::Glow { x, y, size: wz::LUMOS_GLOW * self.cfg.size * 0.6, level: 0.8 * snuff });
                }
                let k = (since / (wz::CAST_SECS + cast.hold - wz::CAST_HIT)).clamp(0.0, 1.0);
                out.push(MagicFx::Puff { x, y, size: wz::NOX_PUFF * self.cfg.size, k });
            }
            if cast.spell == Spell::Stupefy && !cast.fizzle && t >= wz::CAST_HIT - wz::BOLT_LEAD_SECS {
                out.push(MagicFx::Bolt { from: self.wand_tip(), to: cast.aim, color: BeamColor::Green, width: wz::BOLT_WIDTH * self.cfg.size });
            }
        }
    }

    /// 매 틱 (빗자루 뒤): 거는 중이면 그 한 틱, 아니면 가끔 대상을 골라 주문. 처리했으면 true
    pub(super) fn update_spell(&mut self, dt: f64, world: &World) -> bool {
        if let State::Cast { cast, t } = self.state {
            self.tick_cast(world, cast, t + dt);
            return true;
        }
        if self.cfg.spells.is_empty() {
            return false;
        }
        if self.clock < self.spell.ready_at || !self.play_free() || self.romp_interrupted() || self.is_user_away() {
            return false;
        }
        if self.spell.rng.f64() >= wz::SPELL_RATE * self.power() * dt {
            return false;
        }
        // 💡 가끔은 대상 없이 Lumos, 🌑 그다음 몫은 Nox
        let roll = self.spell.extra_rng.f64();
        if roll < wz::LUMOS_CHANCE {
            return self.begin_lumos();
        }
        if roll < wz::LUMOS_CHANCE + wz::NOX_CHANCE {
            return self.begin_nox();
        }
        self.start_spell()
    }

    /// 💡 "Lumos!" — 지팡이를 들고 LUMOS_SECS 동안 끝에 불을 밝힘 (주문을 모르면 false)
    pub(super) fn begin_lumos(&mut self) -> bool {
        if self.cfg.spells.is_empty() {
            return false;
        }
        let hold = behavior::range(&mut self.spell.extra_rng, wz::LUMOS_SECS);
        let tip = self.wand_tip();
        self.begin_cast_at(Spell::Lumos, SpellTarget::Floor { x: self.body.x }, tip);
        if let State::Cast { cast, t } = self.state {
            self.state = State::Cast { cast: Cast { hold, ..cast }, t };
        }
        true
    }

    /// 💡 Lumos가 막 켜졌음 (앱이 매 틱 한 번 꺼냄 — 🌗 이 화면을 잠깐 라이트 모드로, lumos.rs)
    pub fn take_lumos(&mut self) -> bool {
        std::mem::take(&mut self.spell.lumos_lit)
    }

    /// 🌑 "Nox!" — 지팡이 끝 불을 끄고 NOX_HOLD_SECS 동안 작은 검은 연기 (주문을 모르면 false)
    pub(super) fn begin_nox(&mut self) -> bool {
        if self.cfg.spells.is_empty() {
            return false;
        }
        let tip = self.wand_tip();
        self.begin_cast_at(Spell::Nox, SpellTarget::Floor { x: self.body.x }, tip);
        if let State::Cast { cast, t } = self.state {
            self.state = State::Cast { cast: Cast { hold: wz::NOX_HOLD_SECS, ..cast }, t };
        }
        true
    }

    /// 🌑 Nox로 막 꺼졌음 (앱이 매 틱 한 번 꺼냄 — 🌗 이 화면을 잠깐 다크 모드로, lumos.rs)
    pub fn take_nox(&mut self) -> bool {
        std::mem::take(&mut self.spell.nox_out)
    }

    /// 🪶 앱이 매 틱 (내 펫만): 지금 장난감 아이콘을 만들 수 있나. 기다리던 아이콘이 ICON_WAIT_SECS 넘게 안 오면 보통 물건을 띄움
    pub fn set_icon_lift_ok(&mut self, ok: bool) {
        self.spell.icon_ok = ok;
        if self.spell.icon_wait.is_some_and(|(_, until)| self.clock >= until) {
            self.icon_lift_failed();
        }
    }

    /// 🪶 앱이 아이콘을 만들어 띄울 자리 (기다리는 동안만)
    pub fn icon_lift_wanted(&self) -> Option<IconLift> {
        self.spell.icon_wait.map(|(lift, _)| lift)
    }

    /// 🪶 앱이 장난감 아이콘 (id)을 만들어 띄웠음
    pub fn icon_lifted(&mut self, _id: u64) {
        self.spell.icon_wait = None;
    }

    /// 🪶 아이콘을 못 받았음: 곁 (SPELL_REACH)의 보통 물건 하나를 대신 띄움 (없으면 그만)
    pub fn icon_lift_failed(&mut self) {
        let Some((lift, _)) = self.spell.icon_wait.take() else { return };
        let reach = wz::SPELL_REACH * self.cfg.size;
        let pick = self
            .play
            .spots
            .iter()
            .filter(|o| !o.local && !o.giant && (o.x - self.body.x).abs() <= reach && (o.y - self.body.y).abs() <= reach)
            .min_by(|a, b| (a.x - lift.x).abs().total_cmp(&(b.x - lift.x).abs()))
            .map(|o| o.id);
        if let Some(id) = pick {
            self.play.event = Some(ObjectPlay::Levitate { id, rise: lift.rise, secs: lift.secs });
        }
    }

    /// 🪶 화면 아래 바닥에서 발 앞을 보고 "Wingardium Leviosa!" — 작업표시줄 아이콘을 달라고 함 (아이콘을 만들 수 없거나 Leviosa를 모르면 false)
    pub(super) fn begin_icon_lift(&mut self) -> bool {
        let on_bottom = self.standing.is_some_and(|s| matches!(s.owner, Owner::Ground(_)));
        if !self.spell.icon_ok || !self.cfg.spells.has(Spell::Leviosa) || !on_bottom || self.guest.is_some() || self.spell.icon_wait.is_some() {
            return false;
        }
        let size = self.cfg.size;
        let x = self.body.x + self.facing * wz::ICON_AHEAD * size;
        self.begin_cast_at(Spell::Leviosa, SpellTarget::Icon { x }, (x, self.body.y - 0.3 * size));
        let State::Cast { cast, .. } = self.state else { return false };
        if !cast.fizzle {
            let secs = behavior::range(&mut self.spell.extra_rng, wz::FLOAT_SECS);
            let lift = IconLift { x, y: self.body.y, rise: wz::FLOAT_RISE * size, secs };
            self.spell.icon_wait = Some((lift, self.clock + wz::ICON_WAIT_SECS));
            self.spell.icon_ok = false;
            self.juggle.event = Some(crate::juggle::JuggleEvent::Pull);
        }
        true
    }

    /// 대상 하나를 골라 주문을 시작함 (없으면 false): 물건을 든 펫이 있으면 Expelliarmus를 먼저, 아니면 펫·물건 중 아무거나
    pub(super) fn start_spell(&mut self) -> bool {
        // 🪶 가끔 작업표시줄 앱 아이콘을 뽑아 띄움 (만들 수 있을 때만)
        if self.spell.icon_ok && self.cfg.spells.has(Spell::Leviosa) && self.spell.extra_rng.f64() < wz::ICON_LIFT_CHANCE && self.begin_icon_lift() {
            return true;
        }
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
            if spells.has(Spell::Stupefy) && p.liftable {
                options.push((Spell::Stupefy, target));
            }
        }
        let pet_options = options.len();
        for o in self.play.spots.iter().filter(|o| !o.local && !o.giant && near(o.x, o.y)) {
            let target = SpellTarget::Object { id: o.id, x: o.x };
            for spell in [Spell::Expelliarmus, Spell::Leviosa, Spell::Reparo, Spell::Stupefy] {
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

    /// 대상 쪽을 보고 지팡이를 들어 주문을 외침 — 노리는 자리는 본 펫·물건에서 (펫은 몸 가운데, 물건은 바닥 조금 위)
    pub(super) fn begin_cast(&mut self, spell: Spell, target: SpellTarget) {
        let aim = match target {
            SpellTarget::Pet { key, x } => self.spell.pets.iter().find(|p| p.key == key).map_or((x, self.body.y - 0.5 * self.cfg.size), |p| (p.x, p.y - 0.5 * p.size)),
            SpellTarget::Object { id, x } => self.play.spots.iter().find(|o| o.id == id).map_or((x, self.body.y - 0.2 * self.cfg.size), |o| (o.x, o.y - o.half_w.min(0.4 * self.cfg.size))),
            SpellTarget::Floor { x } | SpellTarget::Icon { x } => (x, self.body.y),
        };
        self.begin_cast_at(spell, target, aim);
    }

    /// 대상 쪽을 보고 지팡이를 들어 주문을 외침 (aim = 노리는 자리, 물리 px — 실패할지도 여기서 정함)
    pub(super) fn begin_cast_at(&mut self, spell: Spell, target: SpellTarget, aim: (f64, f64)) {
        self.pacing.after_beat = None;
        self.clear_hover();
        let tx = match target {
            SpellTarget::Pet { x, .. } | SpellTarget::Object { x, .. } | SpellTarget::Floor { x } | SpellTarget::Icon { x } => x,
        };
        if (tx - self.body.x).abs() > 1.0 {
            self.facing = (tx - self.body.x).signum();
        }
        let fizzle = spell == Spell::Leviosa && !self.cfg.bookworm && self.spell.rng.f64() < wz::FIZZLE_CHANCE;
        self.speech = Some(match spell {
            Spell::Expelliarmus => Speech::Expelliarmus,
            Spell::Leviosa => Speech::Leviosa,
            Spell::Reparo => Speech::Reparo,
            Spell::Stupefy => Speech::Stupefy,
            Spell::Serpensortia => Speech::Nagini,
            Spell::Patronus => Speech::Patronus,
            Spell::Lumos => Speech::Lumos,
            Spell::Nox => Speech::Nox,
        });
        self.state = State::Cast { cast: Cast { spell, target, fizzle, hit: false, aim, hold: 0.0 }, t: 0.0 };
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
        if t < wz::CAST_SECS + cast.hold {
            self.state = State::Cast { cast, t };
            return;
        }
        self.spell.ready_at = self.clock + behavior::range(&mut self.spell.rng, wz::SPELL_CALM_SECS);
        // 😈 맞히고 나면 반쯤 "hehehe"
        if cast.spell == Spell::Stupefy && self.cfg.dark_lord && self.spell.rng.bool() {
            self.speech = Some(Speech::Hehehe);
        }
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
            (Spell::Stupefy, SpellTarget::Pet { key, .. }) => {
                let secs = behavior::range(&mut self.spell.rng, wz::FAINT_SECS);
                self.spell.pet_event = Some(PetSpell::Stun { key, secs });
            }
            (Spell::Stupefy, SpellTarget::Object { id, .. }) => {
                self.play.event = Some(ObjectPlay::Yank { id, vx: self.facing * wz::DISARM_VX * dpi, vy: wz::DISARM_VY * dpi });
            }
            (Spell::Serpensortia, SpellTarget::Floor { x }) => self.summon_nagini(x),
            (Spell::Patronus, SpellTarget::Pet { key, .. }) => self.release_stag(key, cast.aim),
            (Spell::Lumos, _) => self.spell.lumos_lit = true,
            (Spell::Nox, _) => self.spell.nox_out = true,
            _ => {}
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
            PetSpell::Disarm { key } | PetSpell::Levitate { key, .. } | PetSpell::Stun { key, .. } => key,
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
            PetSpell::Stun { secs, .. } => {
                target.be_stunned(secs);
            }
        }
    }
}
