//! ⚔️ 해리·볼드모트 결투 + 🦌 해리의 패트로누스 (디멘터가 올 때) — brain.rs의 자식 모듈. 숫자는 wizard.rs, 그림은 덮개 창 (magic.rs → ui/magic.js).
//!
//! - ⚔️ 결투 (State::Duel): sync_duels (앱이 매 틱 — sync_lifts가 부름)가 주문을 아는 밝은 쪽 마법사 (해리 빨강·헤르미온느 금빛 —
//!   Stupefy가 아닌 누구나)와 Stupefy를 아는 펫 (초록 — 볼드모트)이 둘 다 한가하고 가까이 (DUEL_RANGE·DUEL_RISE) 있으면 함께 시작:
//!   🗡️ 0.7.2부터 약 15초 한 판 (duel_bout.rs가 짠 Bout을 둘이 하나씩 들고 같은 시계로 따라 함 — Act):
//!   빛줄기 맞부딪힘 (wizard::duel_clash — 진 쪽이 맞음) → 저주 주고받기 (⚡ 빛줄기 Bolt가 날아감, 맞으면 튕겨 💫 2초 / 폴짝·숙여 피함) →
//!   한쪽이 달아나다 뒤돌아 쏘고 다른 쪽은 쫓음 (디딤판 끝까지) → … → 둘 다 숨 고르고 서로 반대쪽으로 걸어가 흩어짐 (다음은 DUEL_CALM_SECS 뒤).
//!   저주: 해리 "Expelliarmus!" (빨강 — 지팡이가 날아감 MagicFx::Wand) · 헤르미온느 "Petrificus Totalus!" (금빛 — 통나무처럼 뻣뻣하게 쿵, render.topple) ·
//!   볼드모트 "Stupefy!"·"Avada Kedavra!" 번갈아 (초록 — 튕겨 기절, 해롭지 않은 장난).
//!   한쪽이 끊기면 (잡힘·볼일) 다른 쪽도 그만. 맞부딪힘 빛줄기는 빨강 쪽이 내놓음 (magic_fx → MagicFx::Duel), 저주 빛줄기는 쏜 쪽이
//! - 🎒 셋이 모이면 (해리·헤르미온느·볼드모트) 헤르미온느가 해리를 가방에 숨기고 대신 결투 (Duel.hold·bag — brain_tuck.rs가 시작·해리를 맡음)
//! - 🦌 패트로누스: sync_patronus가 patronus 펫 (해리) 곁 (PATRONUS_REACH)에 디멘터가 오거나, 같은 화면 (PATRONUS_SEE)에서 디멘터가
//!   다른 펫을 시무룩하게 하면 그 디멘터에게 "Expecto Patronum!" (State::Cast — brain_spell.rs) → 맞는 때 지팡이 끝에서 은빛 사슴이
//!   디멘터를 따라 달려가 (STAG_RUN_SECS) 닿으면 디멘터가 도망 (be_repelled — brain_dementor.rs), 사슴은 그 너머로 달려가며 흐려짐
//! - 훅: magic_fx (덮개 창에 그릴 마법 효과 모두 — ⚡ 빛줄기·⚔️ 결투·🐍 나기니·🦌 사슴·🥶 서리)
//!
//! 주사위는 자기 주사위(DuelMood.rng)로 굴림 — 평소 동작과 다른 동물의 주사위 순서는 그대로.

use super::spell_moves::SpellTarget;
use super::*;
use crate::duel_bout::{self as bout, Act, Bout, Curse, Dodge, Side};
use crate::magic::{BeamColor, MagicFx};
use crate::wizard::{self as wz, Spell};

/// ⚔️ 결투 하나 (두 펫이 같은 것을 하나씩 가짐 — 지팡이 끝은 물리 px)
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Duel {
    /// 상대 (social_key)
    pub foe: u64,
    /// 빨강 쪽 (해리 — 빛줄기를 내놓는 쪽)
    pub red: bool,
    /// 내 지팡이 끝 · 상대 지팡이 끝 (sync_duels가 매 틱 새로 — 움직여도 따라감)
    pub me: (f64, f64),
    pub foe_tip: (f64, f64),
    /// 상대 발 자리 · 몸 크기 (sync_duels가 매 틱 새로)
    pub foe_at: (f64, f64),
    pub foe_size: f64,
    /// 한 판 길이 (숨 고르기까지, 초 — Bout.len)
    pub len: f64,
    /// 밝은 쪽 빛줄기 색 (해리 빨강 · 헤르미온느 금빛)
    pub light: BeamColor,
    /// 🎒 빛줄기 앞에 기다리는 시간 (초 — 헤르미온느가 해리를 가방에 숨기는 동안, 아니면 0)
    pub hold: f64,
    /// 🎒 가방을 든 쪽 (헤르미온느 — 결투 내내 가방 속에 해리가 있음)
    pub bag: bool,
}

impl Duel {
    /// 빛줄기 시간: 기다림 (hold) 뒤부터 흐른 시간
    pub fn beam_t(&self, t: f64) -> f64 {
        (t - self.hold).max(0.0)
    }

    /// 내 쪽 (red = 밝은 쪽)
    pub fn side(&self) -> Side {
        if self.red {
            Side::Light
        } else {
            Side::Dark
        }
    }
}

/// 🪄 저주를 외치는 말
fn curse_speech(c: Curse) -> Speech {
    match c {
        Curse::Expelliarmus => Speech::Expelliarmus,
        Curse::Petrificus => Speech::Petrificus,
        Curse::Stupefy => Speech::Stupefy,
        Curse::AvadaKedavra => Speech::AvadaKedavra,
    }
}

/// 할 일이 바뀐 때를 알아보는 열쇠 (합 번호·종류)
fn act_key(b: &Bout, t: f64, act: Act) -> (usize, u8) {
    let step = b.steps.iter().rposition(|s| t >= s.at).unwrap_or(0);
    let kind = match act {
        Act::Beam { .. } => 0,
        Act::Shoot { .. } => 1,
        Act::Ready => 2,
        Act::Dodge { .. } => 3,
        Act::Hit { .. } => 4,
        Act::Run { away: true } => 5,
        Act::Run { away: false } => 6,
        Act::Rest => 7,
        Act::Done => 8,
    };
    (step, kind)
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
    pub(super) rng: fastrand::Rng,
    /// ⚔️ 이 시각(clock)부터 결투할 수 있음
    pub(super) ready_at: f64,
    /// 🦌 이 시각(clock)부터 패트로누스를 쓸 수 있음
    pub(super) patronus_ready_at: f64,
    /// 🦌 달리는 사슴
    pub(super) stag: Option<Stag>,
    /// 🦌 사슴에 닿은 디멘터 (key, 해리 x — sync_patronus가 한 번 꺼냄)
    repel: Option<(u64, f64)>,
    /// ⚔️ 지금 결투 한 판 (상대도 같은 것을 가짐)
    pub(super) bout: Option<Bout>,
    /// ⚔️ 마지막으로 한 일 (act_key — 바뀔 때 한 번 말함)
    said: (usize, u8),
}

impl DuelMood {
    /// 시드로 난수를 따로 둠. 첫 결투는 DUEL_FIRST_SECS 사이에서
    pub(super) fn new(seed: u64) -> Self {
        let mut rng = fastrand::Rng::with_seed(seed ^ 0x6475_656c);
        let ready_at = behavior::range(&mut rng, wz::DUEL_FIRST_SECS);
        DuelMood { rng, ready_at, patronus_ready_at: 0.0, stag: None, repel: None, bout: None, said: (0, 0) }
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

    /// 테스트: 마법사 혼자 하는 일 (빗자루·스니치·주문·책·가방·나기니·결투·패트로누스·해그리드 놀이)을 끔
    #[cfg(test)]
    pub(crate) fn hush_wizardry(&mut self) {
        self.broom.ready_at = f64::INFINITY;
        self.broom.snitch_ready_at = f64::INFINITY;
        self.spell.ready_at = f64::INFINITY;
        self.study.read_ready_at = f64::INFINITY;
        self.study.bag_ready_at = f64::INFINITY;
        self.dark.nagini_ready_at = f64::INFINITY;
        self.duel.ready_at = f64::INFINITY;
        self.duel.patronus_ready_at = f64::INFINITY;
        self.hagrid.ready_at = f64::INFINITY;
    }

    /// 🏰 호그와트 친구 셋 중 누구인지 (해그리드 > 헤르미온느 > 해리 순으로 봄)
    pub fn hogwarts(&self) -> Option<wz::Hogwarts> {
        if self.cfg.gamekeeper {
            Some(wz::Hogwarts::Hagrid)
        } else if self.cfg.bookworm {
            Some(wz::Hogwarts::Hermione)
        } else if self.cfg.patronus {
            Some(wz::Hogwarts::Harry)
        } else {
            None
        }
    }

    /// 🦌 달리는 은빛 사슴
    pub fn stag(&self) -> Option<Stag> {
        self.duel.stag
    }

    /// 한가해서 결투·패트로누스를 시작할 수 있음
    pub(super) fn wizard_free(&self) -> bool {
        self.play_free() && !self.romp_interrupted() && !self.is_user_away() && !self.is_away()
    }

    /// ⚔️ 결투를 시작할 수 있음
    pub(super) fn duel_ready(&self) -> bool {
        self.duel_side().is_some() && self.wizard_free() && self.clock >= self.duel.ready_at
    }

    /// 🦌 패트로누스를 쓸 수 있음
    fn patronus_ready(&self) -> bool {
        self.cfg.patronus && self.wizard_free() && self.clock >= self.duel.patronus_ready_at && self.duel.stag.is_none()
    }

    /// x 쪽을 볼 때의 지팡이 끝
    pub(super) fn wand_tip_toward(&self, x: f64) -> (f64, f64) {
        let f = if (x - self.body.x).abs() > 1.0 { (x - self.body.x).signum() } else { self.facing };
        self.wand_tip_facing(f)
    }

    /// ⚔️ 결투 시작: 상대 쪽을 보고 지팡이를 겨눔 (밝은 쪽은 첫 저주 이름 — 해리 "Expelliarmus!"·헤르미온느 "Petrificus Totalus!" ·
    /// 초록 "You again, Potter?" · 🎒 가방 든 쪽 "In you go!"), 한 판 (b)을 받아 둠
    pub(super) fn begin_duel(&mut self, duel: Duel, b: Bout) {
        self.pacing.after_beat = None;
        self.clear_hover();
        if (duel.foe_tip.0 - self.body.x).abs() > 1.0 {
            self.facing = (duel.foe_tip.0 - self.body.x).signum();
        }
        self.speech = Some(if duel.bag {
            Speech::InYouGo
        } else if duel.red {
            curse_speech(bout::curse_of(Side::Light, duel.light, 0))
        } else {
            Speech::DuelCall
        });
        self.duel.bout = Some(b);
        self.duel.said = (0, 0);
        self.state = State::Duel { duel, t: 0.0 };
    }

    /// ⚔️ 결투를 그만둠 (상대가 끊겼음): 쉬고, 다음 결투는 DUEL_CALM_SECS 뒤
    fn end_duel(&mut self) {
        self.duel.ready_at = self.clock + behavior::range(&mut self.duel.rng, wz::DUEL_CALM_SECS);
        self.duel.bout = None;
        self.state = self.rest_beat();
    }

    /// ⚔️ 한 판이 다 끝남: 상대 반대쪽을 보고 조금 걸어가 흩어짐. 다음 결투는 DUEL_CALM_SECS 뒤
    fn part_from_duel(&mut self, foe_x: f64) {
        self.duel.ready_at = self.clock + behavior::range(&mut self.duel.rng, wz::DUEL_CALM_SECS);
        self.duel.bout = None;
        if (foe_x - self.body.x).abs() > 1.0 {
            self.facing = -(foe_x - self.body.x).signum();
        }
        self.state = State::Move { kind: Move::Walk, remaining: behavior::range(&mut self.duel.rng, wz::DUEL_PART_SECS) };
    }

    /// ⚔️ 지금 결투에서 할 일 (🎒 가방을 벌리는 동안은 None)
    pub(super) fn duel_view(&self) -> Option<(Duel, Act)> {
        let State::Duel { duel, t } = self.state else { return None };
        if t < duel.hold {
            return None;
        }
        let b = self.duel.bout.as_ref()?;
        Some((duel, b.act(duel.side(), duel.beam_t(t))))
    }

    /// ⚔️ 결투 중 디딤판 위로 dx만큼 (디딤판 끝에서 멈춤 — 떨어지지 않게)
    fn duel_step(&mut self, dx: f64, world: &World) {
        let Some(st) = self.standing else { return };
        let half = self.cfg.half_w;
        let x = self.body.x + dx;
        let x = match world.piece_at(st.owner, self.body.x) {
            Some(p) if p.x_end - p.x_start > 2.0 * half => x.clamp(p.x_start + half, p.x_end - half),
            _ => self.body.x,
        };
        self.walk_by(x - self.body.x, world);
    }

    /// ⚔️ 결투 한 틱: 디딤판을 따라 한 판 (Bout)이 시키는 대로 — 겨누기·쏘기·피하기·맞아 튕기기·달아나기·쫓기·숨 고르기, 다 되면 흩어짐. 처리했으면 true
    pub(super) fn update_duel(&mut self, dt: f64, world: &World) -> bool {
        let State::Duel { duel, t } = self.state else { return false };
        if !self.follow_anchor(world) {
            self.duel.ready_at = self.clock + behavior::range(&mut self.duel.rng, wz::DUEL_CALM_SECS);
            self.duel.bout = None;
            self.start_fall();
            return true;
        }
        if self.romp_interrupted() {
            self.end_duel();
            return true;
        }
        let t = t + dt;
        let toward = if (duel.foe_at.0 - self.body.x).abs() > 1.0 { (duel.foe_at.0 - self.body.x).signum() } else { self.facing };
        if t < duel.hold {
            self.facing = toward;
            self.state = State::Duel { duel, t };
            return true;
        }
        let Some(b) = self.duel.bout.as_ref() else {
            self.end_duel();
            return true;
        };
        let bt = duel.beam_t(t);
        let act = b.act(duel.side(), bt);
        let key = act_key(b, bt, act);
        let size = self.cfg.size;
        if key != self.duel.said {
            self.duel.said = key;
            self.speech = match act {
                Act::Shoot { curse, .. } => Some(curse_speech(curse)),
                Act::Hit { curse, .. } if curse.stiff() => Some(Speech::Petrified),
                Act::Hit { .. } => Some(Speech::Dazed),
                Act::Dodge { .. } => Some(Speech::Dodged),
                Act::Run { away: true } => Some(Speech::DuelFlee),
                Act::Run { away: false } => Some(Speech::DuelChase),
                Act::Rest => Some(Speech::DuelRest),
                _ => self.speech,
            };
        }
        match act {
            Act::Done => {
                self.part_from_duel(duel.foe_at.0);
                return true;
            }
            Act::Hit { h, .. } => {
                // 뒤로 튕김 (상대 쪽을 본 채)
                self.facing = toward;
                let (k1, _) = bout::hit_shape(h);
                let (k0, _) = bout::hit_shape((h - dt).max(0.0));
                self.duel_step(-toward * bout::KNOCK_DIST * size * (k1 - k0), world);
            }
            Act::Run { away: true } => {
                self.facing = -toward;
                self.duel_step(-toward * bout::RUN_SPEED * size * dt, world);
            }
            Act::Run { away: false } => {
                self.facing = toward;
                if (duel.foe_at.0 - self.body.x).abs() > bout::CHASE_GAP * size.max(duel.foe_size) {
                    self.duel_step(toward * bout::CHASE_SPEED * size * dt, world);
                }
            }
            _ => self.facing = toward,
        }
        self.state = State::Duel { duel, t };
        true
    }

    /// ⚔️ 결투 그림: 🎒 가방 벌리기 · 겨누기·쏘기 (cast) · 기다림 (idle) · 폴짝 (hop)·숙이기 (land) · 맞아 튕김 (fall)·💫 누움 (sleep)·일어남 (land) ·
    /// 뻣뻣 (idle — 웹뷰가 render.topple만큼 눕힘) · 달리기 (run)
    pub(super) fn duel_anim(&self) -> Anim {
        let State::Duel { duel, t } = self.state else { return Anim::Idle };
        let Some((_, act)) = self.duel_view() else {
            return if duel.bag && t < duel.hold { Anim::BagStuff } else { Anim::Cast };
        };
        match act {
            Act::Beam { .. } => Anim::Cast,
            Act::Shoot { u, .. } if u < bout::SHOT_SECS => Anim::Cast,
            Act::Dodge { kind: Dodge::Hop, .. } => Anim::Hop,
            Act::Dodge { kind: Dodge::Duck, .. } => Anim::Land,
            Act::Hit { curse, .. } if curse.stiff() => Anim::Idle,
            Act::Hit { h, .. } if h < bout::KNOCK_SECS => Anim::Fall,
            Act::Hit { h, .. } if bout::hit_dazed(h) => Anim::Sleep,
            Act::Hit { .. } => Anim::Land,
            Act::Run { .. } => Anim::Run,
            _ => Anim::Idle,
        }
    }

    /// 💫 결투에서 맞아 기절·뻣뻣해 누워 있음 (render.thud)
    pub(super) fn duel_dazed(&self) -> bool {
        matches!(self.duel_view(), Some((_, Act::Hit { h, .. })) if bout::hit_dazed(h))
    }

    /// 🪨 Petrificus에 맞아 통나무처럼 누운 각도 (도 — 상대 반대쪽으로 넘어짐, 0이면 서 있음 — render.topple)
    pub(super) fn duel_topple(&self) -> f64 {
        match self.duel_view() {
            Some((_, Act::Hit { curse, h })) if curse.stiff() => -self.facing * 90.0 * bout::hit_shape(h).1,
            _ => 0.0,
        }
    }

    /// 결투 중 창을 띄우는 높이 (물리 px): 맞아 튕길 때 · 폴짝 피할 때
    pub(super) fn duel_lift(&self) -> f64 {
        let size = self.cfg.size;
        match self.duel_view() {
            Some((_, Act::Hit { h, .. })) if h < bout::KNOCK_SECS => (std::f64::consts::PI * h / bout::KNOCK_SECS).sin() * bout::KNOCK_RISE * size,
            Some((_, Act::Dodge { kind: Dodge::Hop, u })) => (std::f64::consts::PI * (u / bout::DODGE_SECS).min(1.0)).sin() * bout::HOP_RISE * size,
            _ => 0.0,
        }
    }

    /// ⚔️ 결투 효과: 맞부딪힘 빛줄기 (빨강 쪽만) · 날아가는 저주 빛줄기 (쏜 쪽) · Expelliarmus에 날아간 지팡이 (맞은 쪽)
    fn duel_fx(&self, out: &mut Vec<MagicFx>) {
        let Some((duel, act)) = self.duel_view() else { return };
        let size = self.cfg.size;
        match act {
            Act::Beam { u, len } if duel.red => {
                let k = wz::duel_clash(u, len);
                let (r, g) = (duel.me, duel.foe_tip);
                let clash = (r.0 + (g.0 - r.0) * k, r.1 + (g.1 - r.1) * k);
                out.push(MagicFx::Duel { red: r, green: g, clash, reach: wz::duel_reach(u), width: wz::DUEL_WIDTH * size, light: duel.light });
            }
            Act::Shoot { curse, u, dodge } if u >= bout::SHOT_FIRE - wz::BOLT_LEAD_SECS && u <= bout::SHOT_FIRE + bout::BOLT_FLY_SECS + 0.05 => {
                let from = self.wand_tip_toward(duel.foe_at.0);
                let (fx, fy, fs) = (duel.foe_at.0, duel.foe_at.1, duel.foe_size);
                let aim = match dodge {
                    None => (fx, fy - 0.5 * fs),
                    Some(Dodge::Hop) => (fx, fy - 0.2 * fs),
                    Some(Dodge::Duck) => (fx, fy - 0.85 * fs),
                };
                // 피하면 그 너머까지 날아감
                let end = if dodge.is_some() { (aim.0 + (aim.0 - from.0) * 0.6, aim.1 + (aim.1 - from.1) * 0.6) } else { aim };
                let k = ((u - bout::SHOT_FIRE) / bout::BOLT_FLY_SECS).clamp(0.0, 1.0);
                let lerp = |k: f64| (from.0 + (end.0 - from.0) * k, from.1 + (end.1 - from.1) * k);
                out.push(MagicFx::Bolt { from: lerp((k - 0.45).max(0.0)), to: lerp(k.max(0.04)), color: curse.color(), width: wz::BOLT_WIDTH * size * 1.2 });
            }
            Act::Hit { curse, h } if curse.disarms() => {
                let hand = (self.body.x + self.facing * 0.26 * size, self.body.y - 0.55 * size);
                let land = (self.body.x + self.facing * 1.1 * size, self.body.y - 0.04 * size);
                let back = bout::KNOCK_SECS + bout::HIT_FAINT_SECS;
                let k = if h < back { (h / wz::WAND_FLY_SECS).min(1.0) } else { 1.0 - ((h - back) / bout::GETUP_SECS).min(1.0) };
                out.push(MagicFx::Wand { from: hand, to: land, k, size: wz::WAND_SIZE * size });
            }
            _ => {}
        }
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

    /// ✨ 덮개 창에 그릴 마법 효과 모두 (앱이 매 틱 — trail.rs): ⚡ 빛줄기 · ⚔️ 결투 (빨강 쪽만) · 🐍 나기니 · 🦌 사슴 · 🥶 서리 · 🔥 모닥불
    pub fn magic_fx(&self, out: &mut Vec<MagicFx>) {
        self.spell_fx(out);
        self.dark_fx(out);
        self.dementor_fx(out);
        self.hagrid_fx(out);
        // 🔥 해그리드가 피운 모닥불 (social_campfire.rs가 몸짓 Pose.fire로 줌)
        if let Some((x, level)) = self.romp.fire.filter(|_| self.in_romp()) {
            out.push(MagicFx::Campfire { x, y: self.body.y, size: wz::CAMPFIRE_SIZE * self.cfg.size, level });
        }
        self.duel_fx(out);
        if let Some(s) = self.duel.stag {
            let (x, y) = s.pos();
            out.push(MagicFx::Stag { x, y, size: wz::STAG_SIZE * self.cfg.size, dir: s.dir, alpha: wz::stag_alpha(s.t) });
        }
    }
}

/// ⚔️ a (밝은 쪽)와 b (초록)가 결투할 거리: 가로 DUEL_MIN … DUEL_RANGE, 세로 DUEL_RISE 안 (몸 크기 배 — 큰 쪽)
pub(super) fn duel_reachable(a: &Pet, b: &Pet) -> bool {
    let big = a.cfg.size.max(b.cfg.size);
    let (dx, dy) = ((a.body.x - b.body.x).abs(), (a.body.y - b.body.y).abs());
    dx <= wz::DUEL_RANGE * big && dx >= wz::DUEL_MIN * big && dy <= wz::DUEL_RISE * big
}

/// ⚔️ 앱이 매 틱 동물 update 뒤에 (sync_lifts가 부름): 상대가 끊긴 결투를 그만두고, 한가한 빨강·초록 둘이 가까이 있으면 결투를 시작함
/// (🎒 해리·헤르미온느·볼드모트가 다 있으면 가방 숨기기 결투가 먼저 — brain_tuck.rs)
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
    // 서로의 자리를 새로 알려 줌 (달아나고 쫓고 튕겨도 빛줄기·저주가 따라감)
    for i in 0..pets.len() {
        let State::Duel { mut duel, t } = pets[i].as_ref().state else { continue };
        let Some(j) = find(pets, duel.foe).filter(|&j| j != i) else { continue };
        let (me, foe) = (pets[i].as_ref(), pets[j].as_ref());
        duel.foe_at = (foe.body.x, foe.body.y);
        duel.foe_size = foe.cfg.size;
        duel.foe_tip = foe.wand_tip_toward(me.body.x);
        duel.me = me.wand_tip_toward(foe.body.x);
        pets[i].as_mut().state = State::Duel { duel, t };
    }
    // 🎒 셋이 모였으면 헤르미온느가 해리를 가방에 숨기고 대신 결투 (brain_tuck.rs) — 평소 결투보다 먼저
    super::tuck_moves::start_tucks(pets);
    for i in 0..pets.len() {
        if pets[i].as_ref().duel_side() != Some(true) || !pets[i].as_ref().duel_ready() {
            continue;
        }
        for j in 0..pets.len() {
            let (a, b) = (pets[i].as_ref(), pets[j].as_ref());
            if j == i || b.duel_side() != Some(false) || !b.duel_ready() || !duel_reachable(a, b) {
                continue;
            }
            let (red_tip, green_tip) = (a.wand_tip_toward(b.body.x), b.wand_tip_toward(a.body.x));
            let (red_key, green_key) = (a.social_key, b.social_key);
            let light = a.duel_color();
            let (red_at, green_at, red_size, green_size) = ((a.body.x, a.body.y), (b.body.x, b.body.y), a.cfg.size, b.cfg.size);
            let plan = Bout::plan(&mut pets[i].as_mut().duel.rng, light);
            let len = plan.len;
            let duel = Duel { foe: green_key, red: true, me: red_tip, foe_tip: green_tip, foe_at: green_at, foe_size: green_size, len, light, hold: 0.0, bag: false };
            pets[j].as_mut().begin_duel(Duel { foe: red_key, red: false, me: green_tip, foe_tip: red_tip, foe_at: red_at, foe_size: red_size, ..duel }, plan.clone());
            pets[i].as_mut().begin_duel(duel, plan);
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
