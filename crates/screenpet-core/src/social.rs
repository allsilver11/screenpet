//! 한 화면의 동물끼리 알아보기 (v0.3.4): 가까이 마주치면 서로 인사하고, 가끔 한 마리가 술래가 되어 쫓아가며 논다.
//! 게임 루프가 매 틱 (모든 동물의 update 전에) 화면 위 동물 전부(내 동물 + 손님)를 넘겨 `Social::tick`을 부른다.
//! 짝짓기·거리·같은 디딤판 판정·쿨다운·술래 고르기는 여기서, 실제 동작은 brain_social.rs (Pet).
//!
//! - 한 동물은 한 번에 한 놀이만. 짝은 앞에서부터 차례로 (O(n²), 동물 ≤ 11마리).
//! - 사용자 손길·먹이·집 가기·손님의 집 생각은 Pet 쪽에서 놀이를 끊고, 여기선 짝이 놀이를 벗어나면 남은 쪽도 풀어 준다.
//! - 틱마다 할당하지 않는다 (스냅샷 Vec을 다시 쓴다; 쿨다운 표는 놀이가 끝날 때만 넣는다).

use std::collections::HashMap;
use std::hash::Hash;

use crate::behavior::{self, Personality};
use crate::collide::{self, Hit, HIT_FAST_SPEED, HIT_MIN_SPEED, HIT_PAIR_COOLDOWN};
use crate::object::Object;
use crate::physics::{Body, GRAVITY};
use crate::brain::meet_moves::{MeetKind, CROUCH_SECS, HEAD_RISE};
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

/// 마주 오던 둘이 만났다고 보는 거리 (인사 거리와 같아서 인사보다 먼저 본다, 이보다 가까우면 이미 지나치는 중)
pub const MEET_BODIES: f64 = GREET_BODIES;
pub const MEET_MIN_BODIES: f64 = 0.4;
/// 마주칠 때 밟기·튀기·지나치기·올라서기 중 하나를 할 확률 (한 번의 마주침에 한 번만 정한다, 나머지는 평소대로 인사)
pub const MEET_CHANCE: f64 = 0.85;
/// 만남 고르기의 몫: 머리 밟고 넘기가 가장 흔하고, 밟고 폴짝 되튀기·높이 튀기·지나치기·올라서기는 가끔 (합 1)
pub const MEET_KINDS: [(MeetKind, f64); 5] =
    [(MeetKind::Stomp, 0.45), (MeetKind::BounceBack, 0.13), (MeetKind::HighBounce, 0.12), (MeetKind::Pass, 0.15), (MeetKind::Climb, 0.15)];
/// 한쪽만 다가가 닿았을 때 (친구가 서 있거나, 앞서 가거나, 막 돌아서는 중): 만남을 할 확률과 그 몫.
/// 다가간 쪽만 뛴다 (지나치기는 없다 — 둘 다 뛰어야 한다)
pub const ONE_WAY_MEET_CHANCE: f64 = 0.7;
pub const ONE_WAY_KINDS: [(MeetKind, f64); 4] = [(MeetKind::Stomp, 0.55), (MeetKind::BounceBack, 0.3), (MeetKind::HighBounce, 0.05), (MeetKind::Climb, 0.1)];
/// 달리던 동물은 만나는 거리가 run_carry의 이만큼 더 멀다 (일찍 뛰어 친구를 뚫고 지나가지 않게)
const RUN_REACH: f64 = 0.5;
/// 그 짝이 다시 만남을 정하기까지 (초): 했으면 / 안 했으면
pub const MEET_COOLDOWN: (f64, f64) = (15.0, 30.0);
pub const MEET_PASS_COOLDOWN: f64 = 20.0;
/// 내려설 때 친구와 떨어지는 거리 (몸 크기 평균의 배, 두 몸 반 폭에 더한다)
const MEET_GAP: f64 = 0.15;
/// 지나치기: 낮은 쪽이 솟는 높이와, 높은 쪽 발이 낮은 쪽 머리 위로 지나가는 여유 (몸 크기 배)
const PASS_LOW_RISE: f64 = 0.25;
const PASS_MARGIN: f64 = 0.2;
/// 달리던 동물의 웅크림 (평소의 배)
const RUN_CROUCH: f64 = 0.4;

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
    /// 스스로 걷는 중이고 걷는 곳이 어딘지 (마주침, brain_meet.rs)
    walking: bool,
    facing: f64,
    half_w: f64,
    span: Option<(f64, f64)>,
    /// 던져져 날아가는 중 (brain_hit.rs)
    flung: bool,
    /// 들려 있다
    held: bool,
    /// 떨어지는 중이라 머리 위에 내려설 수 있다 (social_tower.rs)
    perching: bool,
    /// 머리 위에 친구를 태웠다 / 자기가 머리 위에 있다
    carrying: bool,
    riding: bool,
    /// 달리는 중이면 뛰어 내릴 때 더 가는 거리 (아니면 0)
    run_carry: f64,
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
            walking: p.walks_on_surface(),
            facing: p.facing,
            half_w: p.cfg.half_w,
            span: p.walk_span(),
            flung: p.is_flung(),
            held: p.is_held(),
            perching: p.can_perch(),
            carrying: p.has_rider(),
            riding: p.is_on_head(),
            run_carry: p.run_carry(),
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

/// 둘이 같은 디딤판에서 서로 쪽으로 걷다가 막 만났다
fn head_on(a: &Snap, b: &Snap) -> bool {
    let (dx, body) = (b.x - a.x, (a.size + b.size) / 2.0);
    !a.carrying && !b.carrying && a.free && b.free && a.walking && b.walking && a.span.is_some() && a.span == b.span && near(a, b, MEET_BODIES) && dx.abs() >= MEET_MIN_BODIES * body && dx * a.facing > 0.0 && dx * b.facing < 0.0
}

/// a가 스스로 걷거나 달리다 같은 조각 위 친구 b 바로 앞에 왔다 — b는 서 있든 걷든 달리든 한가하기만 하면 된다
/// (떠서 나는 b는 아니다; 내려앉은 나는 동물은 된다). 달리던 a는 조금 더 멀리서 본다
fn walks_into(a: &Snap, b: &Snap) -> bool {
    let Some((lo, hi)) = a.span else { return false };
    let (dx, body) = (b.x - a.x, (a.size + b.size) / 2.0);
    let reach = MEET_BODIES * body + RUN_REACH * a.run_carry;
    !a.carrying
        && !b.carrying
        && !b.riding
        && a.free
        && b.free
        && a.walking
        && !b.flying
        && b.owner.is_some()
        && a.owner == b.owner
        && (a.y - b.y).abs() <= SAME_LEVEL_PX
        && (lo..=hi).contains(&b.x)
        && dx.abs() <= reach
        && dx.abs() >= MEET_MIN_BODIES * body
        && dx * a.facing > 0.0
}

/// 이 짝이 아직 쉬는 중
fn cooling<K: Hash + Eq>(cooldowns: &HashMap<K, f64>, key: &K, now: f64) -> bool {
    cooldowns.get(key).is_some_and(|&until| now < until)
}

/// 이 짝을 secs 동안 쉬게 한다. 오래된 쿨다운은 이때만 정리한다 (틱마다 하지 않는다)
fn cool<K: Hash + Eq>(cooldowns: &mut HashMap<K, f64>, key: K, now: f64, secs: f64) {
    cooldowns.retain(|_, until| *until > now);
    cooldowns.insert(key, now + secs);
}

/// jumper가 친구 너머 같은 조각에 내려설 자리 (자리가 없으면 None)
fn landing_past(jumper: &Snap, other: &Snap) -> Option<f64> {
    let (lo, hi) = jumper.span?;
    let gap = MEET_GAP * (jumper.size + other.size) / 2.0;
    let x = other.x + jumper.facing * (jumper.half_w + other.half_w + gap);
    // 달리던 동물은 더 멀리 내린다 — 자리가 없으면 그냥 친구 바로 너머에
    let far = x + jumper.facing * jumper.run_carry;
    let fits = |x: f64| lo + jumper.half_w <= x && x <= hi - jumper.half_w;
    [far, x].into_iter().find(|&x| fits(x))
}

/// jumper가 친구 머리를 밟고 되튀어 내려설, 온 쪽의 자리 (자리가 없으면 None)
fn landing_back(jumper: &Snap, other: &Snap) -> Option<f64> {
    let (lo, hi) = jumper.span?;
    let gap = MEET_GAP * (jumper.size + other.size) / 2.0;
    let x = other.x - jumper.facing * (jumper.half_w + other.half_w + gap);
    (lo + jumper.half_w <= x && x <= hi - jumper.half_w).then_some(x)
}

/// 0 ≤ roll < 1을 kinds의 몫대로 만남 하나로 바꾼다
fn pick_meet(kinds: &[(MeetKind, f64)], roll: f64) -> MeetKind {
    let mut left = roll;
    for &(kind, share) in kinds {
        if left < share {
            return kind;
        }
        left -= share;
    }
    kinds[0].0
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
    /// 머리를 밟으러 뛰는 중인 짝 (뛰는 쪽, 밟힐 쪽)
    meets: Vec<(u64, u64)>,
    /// 짝 → 다시 만남을 정할 수 있는 시각 (clock)
    meet_cooldown: HashMap<(u64, u64), f64>,
    /// 만남은 따로 굴린다 (인사·술래 고르기에 영향이 없다)
    meet_rng: fastrand::Rng,
    meet_chance: f64,
    /// 한쪽만 다가가 닿았을 때 만남을 할 확률 (테스트에서 바꾼다)
    one_way_chance: f64,
    /// 테스트: 이 만남만 고른다
    meet_only: Option<MeetKind>,
    /// 지난 틱의 모습 (날다가 처음 닿는 순간을 본다) — snap과 번갈아 쓴다
    prev: Vec<Snap>,
    /// 짝 → 다시 부딪힐 수 있는 시각 (clock). 부딪힐 때만 넣고 정리한다
    hit_cooldown: HashMap<(u64, u64), f64>,
    /// 머리 위에 서 있는 짝들, 아래에서 위 순서 (social_tower.rs)
    rides: Vec<tower::Ride>,
    /// 머리로 뛰어오르는 중 (뛰는 쪽, 탑 밑동)
    climbs: Vec<(u64, u64)>,
    /// 탑 밑동으로 들어온 동물이 올라갈 확률 (테스트에서 바꾼다)
    tower_chance: f64,
    /// (물건 번호, 동물) → 다시 부딪힐 수 있는 시각
    object_cooldown: HashMap<(u64, u64), f64>,
}

fn pair(a: u64, b: u64) -> (u64, u64) {
    (a.min(b), a.max(b))
}

impl Social {
    pub fn new(seed: u64) -> Self {
        Self { links: Vec::new(), cooldown: HashMap::new(), clock: 0.0, rng: fastrand::Rng::with_seed(seed), snap: Vec::new(), meets: Vec::new(), meet_cooldown: HashMap::new(), meet_rng: fastrand::Rng::with_seed(seed ^ 0x6d65_6574), meet_chance: MEET_CHANCE, one_way_chance: ONE_WAY_MEET_CHANCE, meet_only: None, prev: Vec::new(), hit_cooldown: HashMap::new(), rides: Vec::new(), climbs: Vec::new(), tower_chance: tower::TOWER_CLIMB_CHANCE, object_cooldown: HashMap::new() }
    }

    /// 지금 놀이 중인 짝 수 (테스트용)
    #[cfg(test)]
    pub fn active(&self) -> usize {
        self.links.len()
    }

    /// 매 틱 (동물 update 전에) 화면 위 동물 전부로 부른다
    pub fn tick<T: AsRef<Pet> + AsMut<Pet>>(&mut self, dt: f64, pets: &mut [T]) {
        self.clock += dt;
        self.step_rides(pets);
        std::mem::swap(&mut self.snap, &mut self.prev);
        self.snap.clear();
        self.snap.extend(pets.iter().map(|p| Snap::of(p.as_ref())));
        self.step_climbs(pets);
        self.perches(dt, pets);
        self.stomps(pets);
        self.thrown_hits(dt, pets);
        self.step_meets(pets);
        self.step_links(dt, pets);
        self.find_head_ons(pets);
        self.tower_bumps(pets);
        self.find_new(dt, pets);
    }

    /// 뛰는 동물이 머리를 밟으면 밟힌 쪽이 반응한다. 뛰는 쪽이 못 오게 됐으면 (잡힘 등) 기다리던 쪽을 풀어 준다
    fn step_meets<T: AsRef<Pet> + AsMut<Pet>>(&mut self, pets: &mut [T]) {
        let mut i = 0;
        while i < self.meets.len() {
            let (jumper, target) = self.meets[i];
            let (ij, it) = (self.index(jumper), self.index(target));
            // 밟힌 쪽이 없어졌어도 꺼내 둔다 — 남겨 두면 나중의 올라서기가 곧바로 끝난다
            if let Some(ij) = ij {
                if pets[ij].as_mut().take_head_bump() {
                    if let Some(it) = it {
                        pets[it].as_mut().feel_head_stomp();
                    }
                }
            }
            let flying = ij.is_some_and(|k| pets[k].as_ref().is_meeting());
            if flying {
                i += 1;
                continue;
            }
            if let Some(it) = it {
                pets[it].as_mut().end_brace();
            }
            self.meets.swap_remove(i);
        }
    }

    /// 마주 오다 만난 짝, 또는 걷거나 달리던 동물이 서 있는·앞서 가는 친구 앞에 닿은 짝: 한 번 닿을 때 한 번만 정한다 —
    /// 대개 머리 밟고 넘기, 가끔 밟고 폴짝 되튀기·높이 튀기·지나치기·올라서기. 나머지는 그대로 (곧 인사로 이어진다).
    /// 한 짝의 인사 쿨다운 중에도 일어난다 (인사 없이 걷던 둘이 그냥 겹쳐 지나가지 않게)
    fn find_head_ons<T: AsRef<Pet> + AsMut<Pet>>(&mut self, pets: &mut [T]) {
        let n = self.snap.len();
        for i in 0..n {
            for j in (i + 1)..n {
                let (a, b) = (self.snap[i], self.snap[j]);
                let key = pair(a.key, b.key);
                if a.engaged || b.engaged || cooling(&self.meet_cooldown, &key, self.clock) {
                    continue;
                }
                // 마주 오기면 둘 중 누가 뛸지 굴리고, 한쪽만 다가갔으면 다가간 쪽이 뛴다
                let (mover, chance) = if head_on(&a, &b) {
                    (None, self.meet_chance)
                } else if walks_into(&a, &b) {
                    (Some((i, j)), self.one_way_chance)
                } else if walks_into(&b, &a) {
                    (Some((j, i)), self.one_way_chance)
                } else {
                    continue;
                };
                let now = self.clock;
                let started = self.meet_rng.f64() < chance
                    && match mover {
                        None => self.start_meet(pets, i, j),
                        Some((m, o)) => self.start_one_way(pets, m, o),
                    };
                let wait = if started { behavior::range(&mut self.meet_rng, MEET_COOLDOWN) } else { MEET_PASS_COOLDOWN };
                cool(&mut self.meet_cooldown, key, now, wait);
                if started {
                    self.snap[i].engaged = true;
                    self.snap[j].engaged = true;
                    // 내리자마자 인사하지 않는다
                    let greet = behavior::range(&mut self.meet_rng, GREET_COOLDOWN);
                    self.cooldown.insert(key, now + greet);
                }
            }
        }
    }

    /// 고른 만남을 시작한다. 내려설 자리가 없으면 false
    fn start_meet<T: AsRef<Pet> + AsMut<Pet>>(&mut self, pets: &mut [T], i: usize, j: usize) -> bool {
        let (ij, it) = if self.meet_rng.bool() { (i, j) } else { (j, i) };
        let kind = self.meet_only.unwrap_or_else(|| pick_meet(&MEET_KINDS, self.meet_rng.f64()));
        let (jump, other) = (self.snap[ij], self.snap[it]);
        // 달리던 동물은 웅크림이 짧다 (속도가 이어져 보이게)
        let crouch = if jump.run_carry > 0.0 || other.run_carry > 0.0 { CROUCH_SECS * RUN_CROUCH } else { CROUCH_SECS };
        if kind == MeetKind::Climb {
            return self.start_climb(pets, ij, it, crouch);
        }
        if kind == MeetKind::Pass {
            let (Some(high_x), Some(low_x)) = (landing_past(&jump, &other), landing_past(&other, &jump)) else { return false };
            // 높은 쪽 발은 낮은 쪽 머리 위로 지나간다. 낮은 쪽이 그만큼 늦게 떠서 꼭대기(가운데)가 같은 때에 온다
            let low = PASS_LOW_RISE * other.size;
            let high = low + HEAD_RISE * other.size + PASS_MARGIN * other.size;
            let air = |rise: f64| 2.0 * (2.0 * rise / GRAVITY).sqrt();
            pets[ij].as_mut().start_vault(kind, None, high_x, high, crouch);
            pets[it].as_mut().start_vault(kind, None, low_x, low, crouch + (air(high) - air(low)) / 2.0);
            return true;
        }
        // 뛰는 쪽 너머(되튀기면 온 쪽)에 자리가 없으면 친구가 뛴다 (둘 다 없으면 그만)
        let land = |a: &Snap, b: &Snap| if kind == MeetKind::BounceBack { landing_back(a, b) } else { landing_past(a, b) };
        let (ij, it, x) = match (land(&jump, &other), land(&other, &jump)) {
            (Some(x), _) => (ij, it, x),
            (None, Some(x)) => (it, ij, x),
            (None, None) => return false,
        };
        self.stomp_head(pets, ij, it, kind, x, crouch)
    }

    /// 한쪽만 다가가 닿았다: 다가간 mover가 친구 머리를 밟고 넘거나, 밟고 폴짝 되튀거나, 가끔 높이 튀거나 올라선다.
    /// 친구는 서서 밟히기를 기다린다. 넘어갈 자리가 없으면 되튀고, 되튈 자리도 없으면 그만
    fn start_one_way<T: AsRef<Pet> + AsMut<Pet>>(&mut self, pets: &mut [T], mover: usize, other: usize) -> bool {
        let kind = match self.meet_only.unwrap_or_else(|| pick_meet(&ONE_WAY_KINDS, self.meet_rng.f64())) {
            MeetKind::Pass => MeetKind::Stomp,
            k => k,
        };
        let (jump, target) = (self.snap[mover], self.snap[other]);
        let crouch = if jump.run_carry > 0.0 { CROUCH_SECS * RUN_CROUCH } else { CROUCH_SECS };
        if kind == MeetKind::Climb && self.start_climb(pets, mover, other, crouch) {
            return true;
        }
        let (past, back) = (landing_past(&jump, &target), landing_back(&jump, &target));
        let (kind, x) = match (kind, past, back) {
            (MeetKind::BounceBack, _, Some(x)) => (MeetKind::BounceBack, x),
            (MeetKind::BounceBack | MeetKind::Climb, Some(x), _) => (MeetKind::Stomp, x),
            (k, Some(x), _) => (k, x),
            (_, None, Some(x)) => (MeetKind::BounceBack, x),
            (_, None, None) => return false,
        };
        self.stomp_head(pets, mover, other, kind, x, crouch)
    }

    /// jumper가 target 머리를 밟고 land_x에 내려선다. target은 기다린다
    fn stomp_head<T: AsRef<Pet> + AsMut<Pet>>(&mut self, pets: &mut [T], jumper: usize, target: usize, kind: MeetKind, land_x: f64, crouch: f64) -> bool {
        let t = self.snap[target];
        pets[jumper].as_mut().start_vault(kind, Some((t.x, t.y - HEAD_RISE * t.size)), land_x, 0.0, crouch);
        pets[target].as_mut().start_brace();
        self.meets.push((self.snap[jumper].key, t.key));
        true
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

    /// 던진 동물이 이번 틱에 다른 동물에 처음 닿았으면 되튀고, 맞은 쪽이 반응한다 (맞을 수 없는 동물은 그대로 지나간다).
    /// 던진 동물이 없으면 할 일이 없다
    fn thrown_hits<T: AsRef<Pet> + AsMut<Pet>>(&mut self, dt: f64, pets: &mut [T]) {
        for i in 0..self.snap.len() {
            if !self.snap[i].flung {
                continue;
            }
            let a = self.snap[i];
            let Some(from) = self.prev.iter().find(|s| s.key == a.key).map(Snap::at) else { continue };
            // 화면 밖에서 위로 되돌아오거나 다른 화면에서 막 들어온 건 날아온 길이 아니다
            let Body { vx, vy, .. } = pets[i].as_ref().body;
            if (a.x - from.0).hypot(a.y - from.1) > 2.0 * vx.hypot(vy) * dt + a.size {
                continue;
            }
            for j in 0..self.snap.len() {
                if j == i || !pets[j].as_ref().hit_feels() {
                    continue;
                }
                let b = self.snap[j];
                let Body { vx, vy, .. } = pets[i].as_ref().body;
                let dpi = pets[i].as_ref().cfg.dpi;
                let key = pair(a.key, b.key);
                if cooling(&self.hit_cooldown, &key, self.clock) {
                    continue;
                }
                // 그냥 놓은 동물이 머리 위로 떨어지면 튕기지 않고 올라선다 (perches). 탑이 꽉 찼으면 평소처럼 튕긴다
                let settles = pets[i].as_ref().is_gentle_drop() && a.perching && self.can_host(pets, j, a.size);
                let Some((face, away, fast)) = self.touch(b, from, a.at(), a.half_w, HEAD_RISE * a.size, vx, vy, dpi).filter(|&(face, ..)| !(settles && face == collide::Face::Top)) else { continue };
                pets[i].as_mut().bounce_off(face, away);
                pets[j].as_mut().feel_hit(Hit { face, vx, vy, dir: -away, fast, poop: false });
                self.snap[j] = Snap { engaged: true, ..Snap::of(pets[j].as_ref()) };
                cool(&mut self.hit_cooldown, key, self.clock, HIT_PAIR_COOLDOWN);
            }
        }
    }

    /// 움직이는 몸(from → now, 발 기준)이 이번 틱에 동물 b에 처음, 충분히 빠르게 닿았다: 들어온 쪽, 멀어지는 쪽, 빠른 충돌인가
    #[allow(clippy::too_many_arguments)]
    fn touch(&self, b: Snap, from: (f64, f64), now: (f64, f64), half_w: f64, height: f64, vx: f64, vy: f64, dpi: f64) -> Option<(collide::Face, f64, bool)> {
        let speed = collide::logical_speed(vx, vy, dpi);
        if speed < HIT_MIN_SPEED {
            return None;
        }
        // 맞는 쪽도 움직인다 (걷기·달리기): 서로에 대한 상대 자리로 본다
        let was = self.prev.iter().find(|s| s.key == b.key)?;
        let rel_prev = (from.0 - was.x, from.1 - was.y);
        let rel_now = (now.0 - b.x, now.1 - b.y);
        let face = collide::entry(rel_prev, rel_now, half_w + b.half_w, HEAD_RISE * b.size, height)?;
        Some((face, collide::away_from(rel_prev.0, vx, face), speed >= HIT_FAST_SPEED))
    }

    /// 던진 물건이 이번 틱에 동물에 처음 닿았으면 되튀고, 맞은 쪽이 반응한다 (똥이면 "ew!"). 게임 루프가 tick 바로 뒤에 물건마다 부른다 (날아가는 물건이 없으면 할 일이 없다)
    pub fn object_hits<T: AsRef<Pet> + AsMut<Pet>>(&mut self, id: u64, object: &mut Object, dt: f64, pets: &mut [T]) {
        if !object.in_flight() {
            return;
        }
        for j in 0..self.snap.len().min(pets.len()) {
            if !pets[j].as_ref().hit_feels() {
                continue;
            }
            let b = self.snap[j];
            let key = (id, b.key);
            if cooling(&self.object_cooldown, &key, self.clock) {
                continue;
            }
            let Body { x, y, vx, vy } = object.body;
            let (half_w, height) = object.hit_box();
            let Some((face, away, fast)) = self.touch(b, (x - vx * dt, y - vy * dt), (x, y), half_w, height, vx, vy, object.dpi()) else { continue };
            object.bounce_off(face, away);
            pets[j].as_mut().feel_hit(Hit { face, vx, vy, dir: -away, fast, poop: object.kind.is_poop() });
            cool(&mut self.object_cooldown, key, self.clock, HIT_PAIR_COOLDOWN);
        }
    }

    fn index(&self, key: u64) -> Option<usize> {
        self.snap.iter().position(|s| s.key == key)
    }

    fn greeted_lately(&self, a: u64, b: u64) -> bool {
        cooling(&self.cooldown, &pair(a, b), self.clock)
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
        cool(&mut self.cooldown, pair(link.a, link.b), self.clock, secs);
    }

    /// 새로 마주친 짝: 인사 (자는 친구는 쳐다보기만), 또는 가끔 바로 술래 놀이
    fn find_new<T: AsRef<Pet> + AsMut<Pet>>(&mut self, dt: f64, pets: &mut [T]) {
        let n = self.snap.len();
        for i in 0..n {
            for j in (i + 1)..n {
                let (a, b) = (self.snap[i], self.snap[j]);
                if a.engaged || b.engaged || !(a.free || b.free) || self.greeted_lately(a.key, b.key) {
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

#[path = "social_tower.rs"]
pub mod tower;

#[cfg(test)]
#[path = "social_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "social_meet_tests.rs"]
mod meet_tests;

#[cfg(test)]
#[path = "social_meet_sim_tests.rs"]
mod meet_sim_tests;
