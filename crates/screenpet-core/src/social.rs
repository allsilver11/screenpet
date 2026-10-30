//! 한 화면의 동물끼리 알아보기 (v0.3.4): 가까이 마주치면 서로 인사하고, 가끔 한 마리가 술래가 되어 쫓아가며 논다.
//! 게임 루프가 매 틱 (모든 동물의 update 전에) 화면 위 동물 전부(내 동물 + 손님)를 넘겨 `Social::tick`을 부른다.
//! 짝짓기·거리·같은 디딤판 판정·쿨다운·술래 고르기는 여기서, 실제 동작은 brain_social.rs (Pet).
//!
//! - 한 동물은 한 번에 한 놀이만. 짝은 앞에서부터 차례로 (O(n²), 동물 ≤ 11마리).
//! - 사용자 손길·먹이·집 가기·손님의 집 생각은 Pet 쪽에서 놀이를 끊고, 여기선 짝이 놀이를 벗어나면 남은 쪽도 풀어 준다.
//! - 틱마다 할당하지 않는다 (스냅샷 Vec을 다시 쓴다; 쿨다운 표는 놀이가 끝날 때만 넣는다).

use std::collections::HashMap;

use crate::behavior::{self, Personality};
use crate::brain::social_moves::{GreetKind, SocialRole};
use crate::brain::Pet;
use crate::surfaces::Owner;

/// 인사하는 거리 (몸 = 두 동물 스프라이트 크기의 평균)
pub const GREET_BODIES: f64 = 2.0;
/// 술래 놀이를 바로 시작할 수 있는 거리
pub const PLAY_BODIES: f64 = 5.0;
/// 잡았다고 보는 거리 (몸이 맞닿을 즈음)
pub const CATCH_BODIES: f64 = 0.6;
/// 같은 디딤판으로 보는 높이 차 (px)
const SAME_LEVEL_PX: f64 = 4.0;
/// 인사 시간
pub const GREET_SECS: (f64, f64) = (1.5, 2.5);
/// 자는 친구 쳐다보기
pub const PEEK_SECS: f64 = 1.5;
/// 인사 뒤 술래 놀이로 이어질 확률 (보통 성격 기준 — 성격에 따라 eagerness배)
pub const GREET_PLAY_CHANCE: f64 = 0.4;
/// 놀이 거리 안에 있을 때 1초에 술래 놀이를 시작할 확률 (보통 성격 기준)
pub const PLAY_RATE: f64 = 0.02;
/// 술래 놀이 시간 (못 잡으면 끝)
pub const CHASE_SECS: (f64, f64) = (5.0, 10.0);
/// 잡은 뒤 하트
pub const CAUGHT_SECS: f64 = 1.5;
/// 잡은 뒤 한 번 술래를 바꿔 더 놀 확률
pub const SWAP_CHANCE: f64 = 0.3;
/// 새침한 고양이가 "흥" 하고 인사할 확률
pub const ALOOF_HMPH_CHANCE: f64 = 0.5;
/// 같은 짝이 다시 인사하기까지 (초)
pub const GREET_COOLDOWN: (f64, f64) = (60.0, 90.0);
/// 같은 짝이 놀고 난 뒤 다시 어울리기까지 (초)
pub const PLAY_COOLDOWN: (f64, f64) = (120.0, 180.0);
/// 인사 뒤 놀이 확률 상한
const MAX_PLAY_CHANCE: f64 = 0.9;
/// 공룡 쿵쾅이 닿는 거리 (같은 바닥 — 나는 동물은 직선 거리)
pub const STOMP_BODIES: f64 = 5.0;
/// 쿵쾅에 흔들린 걷는 친구가 데굴 넘어질 확률 (나머지는 흔들리기만)
pub const TUMBLE_CHANCE: f64 = 0.35;

/// 놀고 싶은 정도 — 술래가 될 가능성과 놀이 확률의 배율
pub fn eagerness(p: Personality) -> f64 {
    match p {
        Personality::Friendly => 1.0,
        Personality::Playful => 2.0,
        Personality::Aloof => 0.3,
    }
}

/// 인사가 끝난 뒤 술래 놀이로 이어질 확률 — 더 놀고 싶은 쪽을 따른다
pub fn play_chance(a: Personality, b: Personality) -> f64 {
    (GREET_PLAY_CHANCE * eagerness(a).max(eagerness(b))).min(MAX_PLAY_CHANCE)
}

/// 술래 고르기: a가 술래면 true (놀고 싶은 정도에 비례)
pub fn pick_chaser_a(rng: &mut fastrand::Rng, a: Personality, b: Personality) -> bool {
    let (ea, eb) = (eagerness(a), eagerness(b));
    rng.f64() * (ea + eb) < ea
}

/// 한 틱의 동물 모습
#[derive(Debug, Clone, Copy)]
struct Snap {
    key: u64,
    x: f64,
    y: f64,
    /// 서 있는 디딤판 (떠 있으면 None)
    owner: Option<Owner>,
    /// 떠서 나는 중 (나는 동물)
    flying: bool,
    size: f64,
    free: bool,
    asleep: bool,
    personality: Personality,
    role: Option<SocialRole>,
    /// 이번 틱에 이미 놀이에 들어가 있음
    engaged: bool,
}

impl Snap {
    fn of(p: &Pet) -> Self {
        Snap {
            key: p.social_key(),
            x: p.body.x,
            y: p.body.y,
            owner: p.standing.map(|s| s.owner),
            flying: p.cfg.flies && p.standing.is_none(),
            size: p.cfg.size,
            free: p.social_free(),
            asleep: p.social_asleep(),
            personality: p.cfg.personality,
            role: p.social_role(),
            engaged: false,
        }
    }

    fn at(&self) -> (f64, f64) {
        (self.x, self.y)
    }
}

/// 두 동물이 몸 bodies개 거리 안: 둘 다 걸으면 같은 디딤판(같은 높이) 위 가로 거리, 나는 동물이 끼면 직선 거리
fn near(a: &Snap, b: &Snap, bodies: f64) -> bool {
    let reach = bodies * (a.size + b.size) / 2.0;
    if a.flying || b.flying {
        return (a.x - b.x).hypot(a.y - b.y) <= reach;
    }
    a.owner.is_some() && a.owner == b.owner && (a.y - b.y).abs() <= SAME_LEVEL_PX && (a.x - b.x).abs() <= reach
}

/// 잡았다: 가로로 몸이 맞닿고 높이도 비슷 (나는 동물은 몸 하나 차이까지)
fn caught(a: &Snap, b: &Snap) -> bool {
    let body = (a.size + b.size) / 2.0;
    (a.x - b.x).abs() <= CATCH_BODIES * body && (a.y - b.y).abs() <= body
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum Phase {
    /// 서로 인사 중
    Greet { remaining: f64 },
    /// a가 자는 b를 쳐다보는 중 (b는 그대로 잔다)
    Peek { remaining: f64 },
    /// chaser가 쫓는 중
    Chase { chaser: u64, remaining: f64 },
    /// 잡았다 — 하트. swap이면 끝난 뒤 술래를 바꿔 한 번 더
    Caught { chaser: u64, remaining: f64, swap: bool },
}

#[derive(Debug, Clone, Copy)]
struct Link {
    a: u64,
    b: u64,
    phase: Phase,
    /// 술래 놀이를 했음 (끝나면 긴 쿨다운)
    played: bool,
    /// 이미 술래를 한 번 바꿨음
    swapped: bool,
}

/// 화면 위 동물들의 어울림 코디네이터
pub struct Social {
    links: Vec<Link>,
    /// 짝(작은 번호, 큰 번호) → 다시 어울릴 수 있는 시각 (clock)
    cooldown: HashMap<(u64, u64), f64>,
    clock: f64,
    rng: fastrand::Rng,
    snap: Vec<Snap>,
}

fn pair(a: u64, b: u64) -> (u64, u64) {
    (a.min(b), a.max(b))
}

impl Social {
    pub fn new(seed: u64) -> Self {
        Self { links: Vec::new(), cooldown: HashMap::new(), clock: 0.0, rng: fastrand::Rng::with_seed(seed), snap: Vec::new() }
    }

    /// 지금 놀이 중인 짝 수 (테스트·디버그용)
    pub fn active(&self) -> usize {
        self.links.len()
    }

    /// 매 틱 (동물 update 전에) 화면 위 동물 전부로 부른다
    pub fn tick<T: AsRef<Pet> + AsMut<Pet>>(&mut self, dt: f64, pets: &mut [T]) {
        self.clock += dt;
        self.snap.clear();
        self.snap.extend(pets.iter().map(|p| Snap::of(p.as_ref())));
        self.stomps(pets);
        self.step_links(dt, pets);
        self.find_new(dt, pets);
    }

    /// 공룡이 쿵쾅을 시작했으면 같은 바닥 STOMP_BODIES 안의 친구들이 흔들린다 (걷는 친구는 TUMBLE_CHANCE로 데굴).
    /// 흔들린 친구는 이번 틱에 새 놀이를 시작하지 않는다
    fn stomps<T: AsRef<Pet> + AsMut<Pet>>(&mut self, pets: &mut [T]) {
        for i in 0..self.snap.len() {
            if !pets[i].as_mut().take_stomp() {
                continue;
            }
            let dino = self.snap[i];
            if dino.owner.is_none() {
                continue;
            }
            for j in 0..self.snap.len() {
                let other = self.snap[j];
                if j == i || !near(&dino, &other, STOMP_BODIES) {
                    continue;
                }
                let tumble = !other.flying && self.rng.f64() < TUMBLE_CHANCE;
                if pets[j].as_mut().feel_stomp(dino.x, tumble) {
                    self.snap[j] = Snap { engaged: true, ..Snap::of(pets[j].as_ref()) };
                }
            }
        }
    }

    fn index(&self, key: u64) -> Option<usize> {
        self.snap.iter().position(|s| s.key == key)
    }

    fn cooling(&self, a: u64, b: u64) -> bool {
        self.cooldown.get(&pair(a, b)).is_some_and(|&until| self.clock < until)
    }

    fn step_links<T: AsRef<Pet> + AsMut<Pet>>(&mut self, dt: f64, pets: &mut [T]) {
        let mut i = 0;
        while i < self.links.len() {
            let link = self.links[i];
            let (ia, ib) = (self.index(link.a), self.index(link.b));
            let role = |idx: Option<usize>| idx.and_then(|k| self.snap[k].role);
            let valid = match link.phase {
                Phase::Greet { .. } | Phase::Caught { .. } => {
                    role(ia) == Some(SocialRole::Greet) && role(ib) == Some(SocialRole::Greet)
                }
                Phase::Peek { .. } => role(ia) == Some(SocialRole::Greet) && ib.is_some(),
                Phase::Chase { chaser, .. } => {
                    let (ic, ir) = if chaser == link.a { (ia, ib) } else { (ib, ia) };
                    role(ic) == Some(SocialRole::Chase) && role(ir) == Some(SocialRole::Flee)
                }
            };
            let (Some(ia), Some(ib), true) = (ia, ib, valid) else {
                self.finish(i, pets);
                continue;
            };
            self.snap[ia].engaged = true;
            if !matches!(link.phase, Phase::Peek { .. }) {
                self.snap[ib].engaged = true;
            }
            let (sa, sb) = (self.snap[ia], self.snap[ib]);
            pets[ia].as_mut().set_play_target(sb.x, sb.y);
            if !matches!(link.phase, Phase::Peek { .. }) {
                pets[ib].as_mut().set_play_target(sa.x, sa.y);
            }
            let next = match link.phase {
                Phase::Greet { remaining } => {
                    let remaining = remaining - dt;
                    if remaining > 0.0 {
                        Some(Phase::Greet { remaining })
                    } else if self.rng.f64() < play_chance(sa.personality, sb.personality) {
                        let a_chases = pick_chaser_a(&mut self.rng, sa.personality, sb.personality);
                        Some(self.start_chase(pets, ia, ib, a_chases))
                    } else {
                        None
                    }
                }
                Phase::Peek { remaining } => {
                    let remaining = remaining - dt;
                    (remaining > 0.0).then_some(Phase::Peek { remaining })
                }
                Phase::Chase { chaser, remaining } => {
                    if caught(&sa, &sb) {
                        let swap = !link.swapped && self.rng.f64() < SWAP_CHANCE;
                        // 도망가던 쪽이 "hehe!"
                        pets[ia].as_mut().start_greet(sb.x, GreetKind::Caught { speak: chaser != link.a }, CAUGHT_SECS);
                        pets[ib].as_mut().start_greet(sa.x, GreetKind::Caught { speak: chaser != link.b }, CAUGHT_SECS);
                        Some(Phase::Caught { chaser, remaining: CAUGHT_SECS, swap })
                    } else {
                        let remaining = remaining - dt;
                        (remaining > 0.0).then_some(Phase::Chase { chaser, remaining })
                    }
                }
                Phase::Caught { chaser, remaining, swap } => {
                    let remaining = remaining - dt;
                    if remaining > 0.0 {
                        Some(Phase::Caught { chaser, remaining, swap })
                    } else if swap {
                        self.links[i].swapped = true;
                        // 이번엔 잡혔던 쪽이 술래
                        Some(self.start_chase(pets, ia, ib, chaser != link.a))
                    } else {
                        None
                    }
                }
            };
            match next {
                Some(phase) => {
                    self.links[i].phase = phase;
                    if matches!(phase, Phase::Chase { .. }) {
                        self.links[i].played = true;
                    }
                    i += 1;
                }
                None => self.finish(i, pets),
            }
        }
    }

    /// 술래 놀이 시작 (a_chases면 ia가 술래)
    fn start_chase<T: AsMut<Pet>>(&mut self, pets: &mut [T], ia: usize, ib: usize, a_chases: bool) -> Phase {
        let (ic, ir) = if a_chases { (ia, ib) } else { (ib, ia) };
        let secs = behavior::range(&mut self.rng, CHASE_SECS);
        let (c_at, r_at) = (self.snap[ic].at(), self.snap[ir].at());
        pets[ic].as_mut().start_chase(r_at, secs);
        pets[ir].as_mut().start_flee(c_at, secs);
        Phase::Chase { chaser: self.snap[ic].key, remaining: secs }
    }

    /// 놀이 i를 끝낸다: 아직 놀이 중인 쪽은 풀어 주고, 짝에 쿨다운을 건다
    fn finish<T: AsRef<Pet> + AsMut<Pet>>(&mut self, i: usize, pets: &mut [T]) {
        let link = self.links.swap_remove(i);
        let peek = matches!(link.phase, Phase::Peek { .. });
        for key in [Some(link.a), (!peek).then_some(link.b)].into_iter().flatten() {
            if let Some(k) = self.index(key) {
                let pet = pets[k].as_mut();
                if pet.social_role().is_some() {
                    pet.end_social();
                }
                self.snap[k].engaged = true;
            }
        }
        let secs = behavior::range(&mut self.rng, if link.played { PLAY_COOLDOWN } else { GREET_COOLDOWN });
        // 오래된 쿨다운은 이때만 정리한다 (틱마다 하지 않는다)
        let now = self.clock;
        self.cooldown.retain(|_, until| *until > now);
        self.cooldown.insert(pair(link.a, link.b), now + secs);
    }

    /// 새로 마주친 짝: 인사 (자는 친구는 쳐다보기만), 또는 가끔 바로 술래 놀이
    fn find_new<T: AsRef<Pet> + AsMut<Pet>>(&mut self, dt: f64, pets: &mut [T]) {
        let n = self.snap.len();
        for i in 0..n {
            for j in (i + 1)..n {
                let (a, b) = (self.snap[i], self.snap[j]);
                if a.engaged || b.engaged || !(a.free || b.free) || self.cooling(a.key, b.key) {
                    continue;
                }
                if !a.free || !b.free {
                    // 한쪽이 자고 있으면 깨어 있는 쪽이 살짝 쳐다본다
                    let (w, s, iw) = if a.free { (a, b, i) } else { (b, a, j) };
                    if s.asleep && near(&w, &s, GREET_BODIES) {
                        pets[iw].as_mut().start_greet(s.x, GreetKind::Peek, PEEK_SECS);
                        self.snap[iw].engaged = true;
                        self.links.push(Link { a: w.key, b: s.key, phase: Phase::Peek { remaining: PEEK_SECS }, played: false, swapped: false });
                    }
                    continue;
                }
                if near(&a, &b, GREET_BODIES) {
                    let secs = behavior::range(&mut self.rng, GREET_SECS);
                    let ka = self.greet_kind(a.personality);
                    let kb = self.greet_kind(b.personality);
                    pets[i].as_mut().start_greet(b.x, ka, secs);
                    pets[j].as_mut().start_greet(a.x, kb, secs);
                    self.snap[i].engaged = true;
                    self.snap[j].engaged = true;
                    self.links.push(Link { a: a.key, b: b.key, phase: Phase::Greet { remaining: secs }, played: false, swapped: false });
                } else if near(&a, &b, PLAY_BODIES) {
                    let rate = PLAY_RATE * eagerness(a.personality).max(eagerness(b.personality));
                    if self.rng.f64() < rate * dt {
                        let a_chases = pick_chaser_a(&mut self.rng, a.personality, b.personality);
                        let phase = self.start_chase(pets, i, j, a_chases);
                        self.snap[i].engaged = true;
                        self.snap[j].engaged = true;
                        self.links.push(Link { a: a.key, b: b.key, phase, played: true, swapped: false });
                    }
                }
            }
        }
    }

    /// 테스트: 바로 술래 놀이를 시작한다 (a_chases면 i가 술래)
    #[cfg(test)]
    pub(crate) fn start_play<T: AsRef<Pet> + AsMut<Pet>>(&mut self, pets: &mut [T], i: usize, j: usize, a_chases: bool) {
        self.snap.clear();
        self.snap.extend(pets.iter().map(|p| Snap::of(p.as_ref())));
        let phase = self.start_chase(pets, i, j, a_chases);
        self.links.push(Link { a: self.snap[i].key, b: self.snap[j].key, phase, played: true, swapped: false });
    }

    fn greet_kind(&mut self, p: Personality) -> GreetKind {
        if p == Personality::Aloof && self.rng.f64() < ALOOF_HMPH_CHANCE {
            GreetKind::Hmph
        } else {
            GreetKind::Hello
        }
    }
}

#[cfg(test)]
#[path = "social_tests.rs"]
mod tests;
