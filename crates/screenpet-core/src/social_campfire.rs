//! 🔥 호그와트 셋의 모닥불 (2026-11-20 마법사 spec 추가 요청) — social.rs의 자식 모듈 (Social의 비공개 필드에 접근). 펫 쪽 몸짓은 brain_romp.rs (Pose),
//! 불 그림은 덮개 창 (Pose.fire → Pet::magic_fx → MagicFx::Campfire → ui/magic.js).
//!
//! 같은 화면의 같은 디딤판 조각, 해그리드에게서 몸 CAMPFIRE_BODIES개 안에 한가한 해그리드·해리·헤르미온느가 다 있으면 곧 (1초에 CAMPFIRE_RATE):
//! 1. 🥳 반가움 (Joy): 서로 쪽을 보고 폴짝폴짝 (heart 그림이 뜸) + 하트, 번갈아 "yay!!" (Speech::Reunion)
//! 2. 🚶 자리로 (Gather): 불 자리 (셋 가운데) 둘레로 걸어감 — 한쪽에 해그리드, 다른 쪽에 둘
//! 3. 🔥 불 피우기 (Light): 해그리드가 불 쪽으로 내밀며 (offer) "Fire's up!" — 불이 LIGHT_SECS 동안 커짐, 둘은 쳐다봄
//! 4. 💬 도란도란 (Chat): 셋이 앉아 (sit_front) CHAT_SECS 동안 LINE_SECS마다 번갈아 한마디 (Fireside — 해그리드는 가끔 "Rock cake?"),
//!    가끔 (LAUGH_CHANCE) 다 같이 "Ha ha!"
//! 5. 🌑 불 끄기 (Douse): 해그리드 "Off ter bed!" — 불이 DOUSE_SECS 동안 줄어듦
//! 6. 👋 흩어지기 (Scatter): "see ya!" 하며 양쪽으로 달려감
//! 끝나면 셋마다 CAMPFIRE_COOLDOWN 동안 다시 안 함. 누가 빠지면 (잡힘·친구 화면 등 — 몸짓을 안 받음) 모두 풀어 주고 불도 꺼짐.
//! 주사위는 따로 (다른 놀이의 주사위 순서는 그대로).

use super::romp::{fit, same_piece};
use super::*;
use crate::brain::romp_moves::Pose;
use crate::brain::{Anim, Speech};
use crate::wizard::Hogwarts;

/// 모일 수 있을 때 1초에 시작할 확률 (높음 — 셋이 한가해지면 곧)
pub const CAMPFIRE_RATE: f64 = 0.25;
/// 모일 수 있는 거리 (해그리드 몸 크기 배, 같은 디딤판 조각)
pub const CAMPFIRE_BODIES: f64 = 12.0;
/// 셋이 다시 모닥불을 피우기까지 (초)
pub const CAMPFIRE_COOLDOWN: (f64, f64) = (300.0, 480.0);
/// 🥳 반가워하는 시간 (초)
const JOY_SECS: f64 = 2.4;
/// 🥳 폴짝 빠르기 (초당 번) · 높이 (몸 크기 배)
const JOY_HOPS: f64 = 1.8;
const JOY_LIFT: f64 = 0.25;
/// 🥳 하트 간격 (초)
const JOY_HEART_SECS: f64 = 0.8;
/// 🥳 "yay!!" 사이 (초)
const JOY_LINE_SECS: f64 = 0.5;
/// 🚶 자리로 가는 빠르기 (걷기 속도 배), 시간 상한 (초)
const GATHER_PACE: f64 = 1.3;
const GATHER_SECS: f64 = 9.0;
/// 🚶 불에서 자리까지 (몸 크기 배 — 셋 중 큰 쪽): 해그리드 쪽 하나, 다른 쪽 둘
const SEAT_NEAR: f64 = 0.95;
const SEAT_FAR: f64 = 1.75;
/// 🔥 불 피우는 시간 (초)
const LIGHT_SECS: f64 = 2.0;
/// 💬 도란도란 시간 (초)
pub const CHAT_SECS: (f64, f64) = (20.0, 40.0);
/// 💬 말 사이 (초)
const LINE_SECS: f64 = 2.4;
/// 💬 한마디 뒤 다 같이 웃을 확률
const LAUGH_CHANCE: f64 = 0.3;
/// 💬 해그리드가 "Rock cake?" 할 확률
const ROCK_CAKE_CHANCE: f64 = 0.35;
/// 🌑 불 끄는 시간 (초)
const DOUSE_SECS: f64 = 1.5;
/// 👋 흩어지는 시간 (초), 간격 (몸 크기 배 — 불에서), 빠르기 (걷기 속도 배), "see ya!" 사이 (초)
const SCATTER_SECS: f64 = 1.6;
const SCATTER_GAP: f64 = 3.0;
const SCATTER_PACE: f64 = 1.8;
const SEE_YA_SECS: f64 = 0.3;

/// 모닥불 단계
#[derive(Debug, Clone, Copy, PartialEq)]
enum Step {
    /// 🥳 반가워 폴짝
    Joy,
    /// 🚶 둘레로
    Gather,
    /// 🔥 불 피우기
    Light,
    /// 💬 도란도란
    Chat,
    /// 🌑 불 끄기
    Douse,
    /// 👋 흩어짐
    Scatter,
}

/// 하는 중인 모닥불 하나
#[derive(Debug, Clone, Copy)]
struct Campfire {
    /// 해그리드 · 해리 · 헤르미온느 (social_key)
    keys: [u64; 3],
    /// 불 자리 x
    cx: f64,
    /// 해그리드가 앉을 쪽 (±1 — 둘은 반대쪽)
    side: f64,
    /// 몸 크기 (셋 중 가장 큰)
    size: f64,
    step: Step,
    /// 이 단계에 든 뒤 흐른 시간
    t: f64,
    /// 이 단계 길이 (💬 주사위)
    len: f64,
    /// 센 것: 말·하트 수
    n: u32,
    /// 💬 마지막으로 말한 자리 (0…2, 없으면 3)
    last: usize,
}

/// 코디네이터의 기억
pub(super) struct SocialCampfire {
    fires: Vec<Campfire>,
    /// 펫 → 다시 모닥불을 피울 수 있는 시각 (clock)
    cooldown: HashMap<u64, f64>,
    rng: fastrand::Rng,
    /// 1초 확률 (테스트에서 바꿈)
    rate: f64,
    /// 지금까지 시작한 모닥불 수
    started: usize,
}

impl SocialCampfire {
    /// seed로 주사위를 따로 둠
    pub(super) fn new(seed: u64) -> Self {
        SocialCampfire { fires: Vec::new(), cooldown: HashMap::new(), rng: fastrand::Rng::with_seed(seed ^ 0x6669_7265), rate: CAMPFIRE_RATE, started: 0 }
    }
}

/// x에서 to 쪽 (±1, 같으면 fallback)
fn face(x: f64, to: f64, fallback: f64) -> f64 {
    if (to - x).abs() > 0.5 {
        (to - x).signum()
    } else {
        fallback
    }
}

impl Social {
    /// 새 모닥불: 한가한 해그리드 곁 같은 조각에 한가한 해리·헤르미온느가 있으면 주사위 (한 틱에 하나)
    pub(super) fn find_campfire<T: AsRef<Pet> + AsMut<Pet>>(&mut self, dt: f64, pets: &mut [T]) {
        let n = self.snap.len();
        if n < 3 {
            return;
        }
        let now = self.clock;
        let ok = |s: &Snap, cd: &HashMap<u64, f64>| s.romp_free && !s.engaged && !cooling(cd, &s.key, now);
        let pick = |who: Hogwarts, near: Option<&Snap>, cd: &HashMap<u64, f64>| {
            (0..n).find(|&i| {
                let s = &self.snap[i];
                s.hogwarts == Some(who) && ok(s, cd) && near.is_none_or(|h| same_piece(h, s) && (s.x - h.x).abs() <= CAMPFIRE_BODIES * h.size.max(s.size))
            })
        };
        let cd = &self.campfire.cooldown;
        let Some(h) = pick(Hogwarts::Hagrid, None, cd) else { return };
        let hagrid = self.snap[h];
        let (Some(a), Some(b)) = (pick(Hogwarts::Harry, Some(&hagrid), cd), pick(Hogwarts::Hermione, Some(&hagrid), cd)) else { return };
        if self.campfire.rng.f64() >= self.campfire.rate * dt {
            return;
        }
        self.start_campfire(pets, [h, a, b]);
    }

    /// 모닥불을 시작함: 서로 쪽을 보고 반가워함
    fn start_campfire<T: AsRef<Pet> + AsMut<Pet>>(&mut self, pets: &mut [T], ix: [usize; 3]) {
        let size = ix.iter().map(|&i| self.snap[i].size).fold(0.0, f64::max);
        let mut cx = ix.iter().map(|&i| self.snap[i].x).sum::<f64>() / 3.0;
        let hx = self.snap[ix[0]].x;
        // 해그리드는 원래 있던 쪽에, 둘은 반대쪽에
        let side = face(cx, hx, 1.0);
        // 자리가 조각 안에 들게
        if let Some((lo, hi)) = self.snap[ix[0]].ground {
            let half = SEAT_FAR * size + size;
            if hi - lo > 2.0 * half {
                cx = cx.clamp(lo + half, hi - half);
            }
        }
        for &i in &ix {
            let s = self.snap[i];
            pets[i].as_mut().start_romp(Pose::hold(Anim::Heart).facing(face(s.x, cx, s.facing)));
            self.snap[i].engaged = true;
        }
        let keys = ix.map(|i| self.snap[i].key);
        self.campfire.fires.push(Campfire { keys, cx, side, size, step: Step::Joy, t: 0.0, len: 0.0, n: 0, last: 3 });
        self.campfire.started += 1;
    }

    /// 하는 중인 모닥불들 한 틱: 누가 빠졌으면 모두 풀어 줌 (불도 꺼짐)
    pub(super) fn step_campfires<T: AsRef<Pet> + AsMut<Pet>>(&mut self, dt: f64, pets: &mut [T]) {
        let mut k = 0;
        while k < self.campfire.fires.len() {
            let mut f = self.campfire.fires[k];
            let ix = f.keys.map(|key| self.index(key));
            let live = ix.iter().all(|i| i.is_some_and(|i| pets[i].as_ref().in_romp()));
            if !live {
                self.end_campfire_at(k, pets);
                continue;
            }
            let ix = ix.map(|i| i.unwrap_or(0));
            for &i in &ix {
                self.snap[i].engaged = true;
            }
            f.t += dt;
            let going = self.step_fire(&mut f, pets, ix);
            self.campfire.fires[k] = f;
            if going {
                k += 1;
            } else {
                self.end_campfire_at(k, pets);
            }
        }
    }

    /// 모닥불 k를 끝냄: 아직 몸짓을 받는 쪽은 쉬게 하고 (불도 꺼짐), 셋마다 쿨다운
    fn end_campfire_at<T: AsRef<Pet> + AsMut<Pet>>(&mut self, k: usize, pets: &mut [T]) {
        let f = self.campfire.fires.swap_remove(k);
        let now = self.clock;
        for key in f.keys {
            if let Some(i) = self.index(key) {
                pets[i].as_mut().end_romp();
                self.snap[i].engaged = true;
            }
            let wait = behavior::range(&mut self.campfire.rng, CAMPFIRE_COOLDOWN);
            cool(&mut self.campfire.cooldown, key, now, wait);
        }
    }

    /// 다음 단계로 (시간·센 것을 0부터)
    fn next_fire_step(f: &mut Campfire, step: Step) {
        f.step = step;
        f.t = 0.0;
        f.n = 0;
    }

    /// 자리 (0 = 해그리드, 1 = 해리, 2 = 헤르미온느)의 앉을 x
    fn seat(f: &Campfire, k: usize) -> f64 {
        match k {
            0 => f.cx + f.side * SEAT_NEAR * f.size,
            1 => f.cx - f.side * SEAT_NEAR * f.size,
            _ => f.cx - f.side * SEAT_FAR * f.size,
        }
    }

    /// 모닥불 한 틱 (ix = 해그리드·해리·헤르미온느 자리). 끝나면 false
    fn step_fire<T: AsRef<Pet> + AsMut<Pet>>(&mut self, f: &mut Campfire, pets: &mut [T], ix: [usize; 3]) -> bool {
        let fire_at = |level: f64| Some((f.cx, level.clamp(0.0, 1.0)));
        match f.step {
            Step::Joy => {
                let mid = ix.iter().map(|&i| self.snap[i].x).sum::<f64>() / 3.0;
                for (k, &i) in ix.iter().enumerate() {
                    let s = self.snap[i];
                    let phase = f.t * JOY_HOPS + k as f64 * 0.33;
                    let lift = JOY_LIFT * (std::f64::consts::PI * phase).sin().abs();
                    pets[i].as_mut().set_romp_pose(Pose { lift, ..Pose::hold(Anim::Heart).facing(face(s.x, mid, s.facing)) });
                }
                if f.n < 3 && f.t >= JOY_LINE_SECS * f64::from(f.n) {
                    let i = ix[f.n as usize];
                    pets[i].as_mut().say_now(Speech::Reunion);
                    pets[i].as_mut().romp_heart();
                    f.n += 1;
                } else if f.n >= 3 && f.t >= JOY_HEART_SECS * f64::from(f.n - 2) && f.t < JOY_SECS {
                    for &i in &ix {
                        pets[i].as_mut().romp_heart();
                    }
                    f.n += 1;
                }
                if f.t >= JOY_SECS {
                    Self::next_fire_step(f, Step::Gather);
                }
            }
            Step::Gather => {
                let mut there = true;
                for (k, &i) in ix.iter().enumerate() {
                    let s = self.snap[i];
                    let x = fit(&s, Self::seat(f, k));
                    there &= (s.x - x).abs() < 2.0;
                    pets[i].as_mut().set_romp_pose(Pose::go(Anim::Walk, x, GATHER_PACE));
                }
                if there || f.t >= GATHER_SECS {
                    Self::next_fire_step(f, Step::Light);
                    pets[ix[0]].as_mut().say_now(Speech::LightFire);
                }
            }
            Step::Light => {
                for (k, &i) in ix.iter().enumerate() {
                    let s = self.snap[i];
                    let toward = face(s.x, f.cx, -f.side);
                    let pose = if k == 0 { Pose { fire: fire_at(f.t / LIGHT_SECS), ..Pose::hold(Anim::Offer).facing(toward) } } else { Pose::hold(Anim::Look).facing(toward) };
                    pets[i].as_mut().set_romp_pose(pose);
                }
                if f.t >= LIGHT_SECS {
                    Self::next_fire_step(f, Step::Chat);
                    f.len = behavior::range(&mut self.campfire.rng, CHAT_SECS);
                }
            }
            Step::Chat => {
                for (k, &i) in ix.iter().enumerate() {
                    let pose = Pose { fire: if k == 0 { fire_at(1.0) } else { None }, ..Pose::hold(Anim::SitFront) };
                    pets[i].as_mut().set_romp_pose(pose);
                }
                if f.t >= LINE_SECS * f64::from(f.n + 1) && f.t < f.len {
                    // 바로 앞에 말한 쪽은 빼고 고름
                    let mut who = self.campfire.rng.usize(0..3);
                    if who == f.last {
                        who = (who + 1 + self.campfire.rng.usize(0..2)) % 3;
                    }
                    let line = if who == 0 && self.campfire.rng.f64() < ROCK_CAKE_CHANCE { Speech::RockCake } else { Speech::Fireside };
                    if f.n > 0 && self.campfire.rng.f64() < LAUGH_CHANCE {
                        // 앞의 말에 다 같이 웃음 (말한 쪽 빼고) — 이번 차례는 웃음
                        for (k, &i) in ix.iter().enumerate() {
                            if k != f.last {
                                pets[i].as_mut().say_now(Speech::HaHa);
                            }
                        }
                    } else {
                        pets[ix[who]].as_mut().say_now(line);
                        f.last = who;
                    }
                    f.n += 1;
                }
                if f.t >= f.len {
                    Self::next_fire_step(f, Step::Douse);
                    pets[ix[0]].as_mut().say_now(Speech::Douse);
                }
            }
            Step::Douse => {
                for (k, &i) in ix.iter().enumerate() {
                    let s = self.snap[i];
                    let toward = face(s.x, f.cx, -f.side);
                    let pose = if k == 0 { Pose { fire: fire_at(1.0 - f.t / DOUSE_SECS), ..Pose::hold(Anim::Idle).facing(toward) } } else { Pose::hold(Anim::Idle).facing(toward) };
                    pets[i].as_mut().set_romp_pose(pose);
                }
                if f.t >= DOUSE_SECS {
                    Self::next_fire_step(f, Step::Scatter);
                }
            }
            Step::Scatter => {
                for (k, &i) in ix.iter().enumerate() {
                    let s = self.snap[i];
                    let away = if k == 0 { f.side } else { -f.side };
                    let gap = if k == 2 { SCATTER_GAP + 1.0 } else { SCATTER_GAP };
                    pets[i].as_mut().set_romp_pose(Pose::go(Anim::Run, fit(&s, f.cx + away * gap * f.size), SCATTER_PACE));
                }
                if f.n < 3 && f.t >= SEE_YA_SECS * f64::from(f.n) {
                    pets[ix[f.n as usize]].as_mut().say_now(Speech::SeeYa);
                    f.n += 1;
                }
                if f.t >= SCATTER_SECS && f.n >= 3 {
                    return false;
                }
            }
        }
        true
    }

    /// 테스트: 지금까지 시작한 모닥불 수
    #[cfg(test)]
    pub(crate) fn campfires_started(&self) -> usize {
        self.campfire.started
    }
}
