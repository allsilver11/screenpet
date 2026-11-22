//! 🛡️🕷️ 무쇠인간과 거미인간 (2026-11-23 요청) — social.rs의 자식 모듈 (Social의 비공개 필드에 접근). 펫 쪽 몸짓은 brain_romp.rs (Pose).
//!
//! 같은 화면 (내 펫 + 친구 펫 손님)의 같은 디딤판 조각, 몸 MENTOR_BODIES개 안에 쉬는 무쇠인간 (Snap.mentor)과 거미인간이 있으면 가끔 둘 중 하나:
//! - 🤗 응원 포옹 (1초에 HUG_RATE): 무쇠인간이 걸어가 (Approach) 거미인간을 꼭 안고 (hug 그림, 하트) 응원 한마디 (Speech::Cheer —
//!   "You got this, kid!" …). 거미인간은 제자리에서 하트 그림으로 쑥스러워함 ("aw, shucks" — Speech::Shucks). HUG_SECS 뒤 풀고 각자.
//! - 🗨️ 졸졸 수다 (1초에 TAG_RATE): 거미인간이 무쇠인간을 졸졸 따라다니며 CHAT_SECS마다 재잘재잘 (Speech::KidChat — 학교·사건·여자친구·영웅의 무게).
//!   무쇠인간은 MOOD_SECS마다 주사위: 들어 줌 (서서 거미인간 쪽을 보고 가끔 "Uh-huh.") 또는 FLEE_CHANCE로 지루해 "Not now, kid." 하고
//!   제트팩으로 낮게 날아 달아남 — 거미인간은 그래도 달려서 쫓아가며 말함. TAG_SECS 뒤 끝.
//! 한 짝은 한 번에 하나만 (포옹과 수다가 겹치지 않음). 끝나면 무쇠인간마다 MENTOR_COOLDOWN 동안 다시 안 함.
//! 둘 중 누가 빠지면 (잡힘 등 — 몸짓을 안 받음) 다른 쪽도 풀어 줌. 주사위는 따로 (다른 놀이의 주사위 순서는 그대로). 새 그림은 무쇠인간 hug 하나.

use super::romp::{fit, same_piece};
use super::*;
use crate::brain::romp_moves::Pose;
use crate::brain::{Anim, Speech};

/// 🤗 포옹: 둘이 쉬고 있을 때 1초에 시작할 확률 (같이 있으면 몇 분에 한 번쯤 — 쿨다운이 더 김)
pub const HUG_RATE: f64 = 0.01;
/// 🗨️ 졸졸 수다: 1초에 시작할 확률
pub const TAG_RATE: f64 = 0.01;
/// 만날 수 있는 거리 (두 몸 크기 평균 배, 같은 디딤판 조각)
pub const MENTOR_BODIES: f64 = 10.0;
/// 한 번 하고 나면 그 무쇠인간이 다시 하기까지 (초)
pub const MENTOR_COOLDOWN: (f64, f64) = (180.0, 360.0);
/// 🗨️ 수다가 이어지는 시간 (초)
pub const TAG_SECS: (f64, f64) = (15.0, 25.0);
/// 🗨️ 무쇠인간이 지루해 달아날 몫 (MOOD_SECS마다)
pub const FLEE_CHANCE: f64 = 0.45;
/// 🗨️ 들어 줄지 달아날지 다시 정하기까지 (초)
const MOOD_SECS: (f64, f64) = (3.0, 5.0);
/// 🗨️ 거미인간의 말 사이 (초)
const CHAT_SECS: f64 = 2.6;
/// 🗨️ 거미인간 말 뒤 무쇠인간이 "Uh-huh." 하기까지 (초)
const NOD_AFTER_SECS: f64 = 1.1;
/// 🗨️ 졸졸 따라가 서는 간격 (몸 크기 배)
const FOLLOW_GAP: f64 = 0.85;
/// 🗨️ 따라 달리는 빠르기 (걷기 속도 배)
const FOLLOW_PACE: f64 = 2.0;
/// 🗨️ 달아나는 거리 (몸 크기 배) · 빠르기 (걷기 속도 배) · 뜬 높이 (몸 크기 배 — 슈퍼인간·무쇠인간 낮게 날기만큼)
const FLEE_BODIES: f64 = 5.0;
const FLEE_PACE: f64 = 1.5;
const FLEE_LIFT: f64 = 0.4;
/// 🤗 다가가는 빠르기 (걷기 속도 배), 시간 상한 (초)
const APPROACH_PACE: f64 = 1.2;
const APPROACH_SECS: f64 = 10.0;
/// 🤗 안을 때 두 몸 가운데 간격 (몸 크기 배 — 꼭 붙음)
const HUG_GAP: f64 = 0.4;
/// 🤗 안고 있는 시간 (초) · 거미인간이 "aw, shucks" 하는 때 · 하트 간격
const HUG_SECS: f64 = 2.8;
const SHUCKS_AT: f64 = 1.5;
const HUG_HEART_SECS: f64 = 0.9;

/// 하는 일
#[derive(Debug, Clone, Copy, PartialEq)]
enum Kind {
    /// 🤗 응원 포옹
    Hug,
    /// 🗨️ 졸졸 수다
    Tag,
}

/// 한 짝 (무쇠인간 · 거미인간)
#[derive(Debug, Clone, Copy)]
struct Duo {
    /// 무쇠인간 · 거미인간 (social_key)
    mentor: u64,
    kid: u64,
    kind: Kind,
    /// 🤗 안으러 가는 중 (아니면 안는 중) · 무쇠인간이 설 자리
    approach: bool,
    x: f64,
    /// 이 단계에 든 뒤 흐른 시간 (🗨️ 수다는 처음부터)
    t: f64,
    /// 🗨️ 이어질 시간
    len: f64,
    /// 🗨️ 들어 줌 (아니면 달아남) · 다시 정하는 때 · 달아날 곳
    listen: bool,
    mood_at: f64,
    flee_x: f64,
    /// 🗨️ 마지막 거미인간 말 뒤 흐른 시간 · 이번 말에 끄덕였음
    since_chat: f64,
    nodded: bool,
    /// 🤗 거미인간이 쑥스러워했음 · 마지막 하트 뒤
    shucked: bool,
    heart_t: f64,
}

/// 코디네이터의 기억
pub(super) struct SocialMentor {
    duos: Vec<Duo>,
    /// 무쇠인간 → 다시 할 수 있는 시각 (clock)
    cooldown: HashMap<u64, f64>,
    rng: fastrand::Rng,
    /// 1초 확률 (테스트에서 바꿈)
    hug_rate: f64,
    tag_rate: f64,
    /// 달아날 몫 (테스트에서 바꿈)
    flee: f64,
    /// 지금까지 시작한 포옹 · 수다 수
    hugs: usize,
    tags: usize,
}

impl SocialMentor {
    /// seed로 주사위를 따로 둠
    pub(super) fn new(seed: u64) -> Self {
        SocialMentor {
            duos: Vec::new(),
            cooldown: HashMap::new(),
            rng: fastrand::Rng::with_seed(seed ^ 0x6875_6721),
            hug_rate: HUG_RATE,
            tag_rate: TAG_RATE,
            flee: FLEE_CHANCE,
            hugs: 0,
            tags: 0,
        }
    }
}

/// a가 b를 보는 쪽 (±1)
fn toward(a: &Snap, b: &Snap) -> f64 {
    if b.x >= a.x {
        1.0
    } else {
        -1.0
    }
}

impl Social {
    /// 새 짝: 쉬는 무쇠인간마다 같은 조각 가까이 쉬는 거미인간이 있으면 주사위 (포옹 또는 수다)
    pub(super) fn find_mentor<T: AsRef<Pet> + AsMut<Pet>>(&mut self, dt: f64, pets: &mut [T]) {
        let n = self.snap.len();
        let now = self.clock;
        let free = |s: &Snap| s.romp_free && s.resting && !s.engaged;
        for m in 0..n {
            let a = self.snap[m];
            if !a.mentor || !free(&a) || cooling(&self.mentor.cooldown, &a.key, now) || self.mentor.duos.iter().any(|d| d.mentor == a.key) {
                continue;
            }
            let Some(k) = (0..n).find(|&k| {
                let b = &self.snap[k];
                k != m && b.spider && free(b) && same_piece(&a, b) && (b.x - a.x).abs() <= MENTOR_BODIES * (a.size + b.size) / 2.0
            }) else {
                continue;
            };
            let roll = self.mentor.rng.f64();
            let kind = if roll < self.mentor.hug_rate * dt {
                Kind::Hug
            } else if roll < (self.mentor.hug_rate + self.mentor.tag_rate) * dt {
                Kind::Tag
            } else {
                continue;
            };
            self.start_mentor(pets, kind, m, k);
        }
    }

    /// 짝 시작 (m = 무쇠인간, k = 거미인간 자리)
    fn start_mentor<T: AsRef<Pet> + AsMut<Pet>>(&mut self, pets: &mut [T], kind: Kind, m: usize, k: usize) {
        let (a, b) = (self.snap[m], self.snap[k]);
        let side = if a.x <= b.x { -1.0 } else { 1.0 };
        let near = fit(&a, b.x + side * HUG_GAP * a.size);
        let x = if (near - b.x).abs() >= 0.5 * HUG_GAP * a.size { near } else { fit(&a, b.x - side * HUG_GAP * a.size) };
        let mut d = Duo {
            mentor: a.key,
            kid: b.key,
            kind,
            approach: kind == Kind::Hug,
            x,
            t: 0.0,
            len: 0.0,
            listen: true,
            mood_at: 0.0,
            flee_x: a.x,
            since_chat: 0.0,
            nodded: false,
            shucked: false,
            heart_t: 0.0,
        };
        match kind {
            Kind::Hug => {
                pets[m].as_mut().start_romp(Pose::go(Anim::Walk, x, APPROACH_PACE));
                pets[k].as_mut().start_romp(Pose::hold(Anim::Look).facing(toward(&b, &a)));
                self.mentor.hugs += 1;
            }
            Kind::Tag => {
                d.len = behavior::range(&mut self.mentor.rng, TAG_SECS);
                d.mood_at = behavior::range(&mut self.mentor.rng, MOOD_SECS);
                pets[m].as_mut().start_romp(Pose::hold(Anim::Idle).facing(toward(&a, &b)));
                pets[k].as_mut().start_romp(Pose::go(Anim::Run, a.x, FOLLOW_PACE));
                pets[k].as_mut().say_now(Speech::KidChat);
                self.mentor.tags += 1;
            }
        }
        self.snap[m].engaged = true;
        self.snap[k].engaged = true;
        self.mentor.duos.push(d);
    }

    /// 하는 중인 짝들 한 틱: 둘 중 누가 빠졌으면 (잡힘 등) 둘 다 풀어 줌
    pub(super) fn step_mentors<T: AsRef<Pet> + AsMut<Pet>>(&mut self, dt: f64, pets: &mut [T]) {
        let mut j = 0;
        while j < self.mentor.duos.len() {
            let mut d = self.mentor.duos[j];
            let (im, ik) = (self.index(d.mentor), self.index(d.kid));
            let both = matches!((im, ik), (Some(m), Some(k)) if pets[m].as_ref().in_romp() && pets[k].as_ref().in_romp());
            let going = both && {
                let (m, k) = (im.unwrap_or(0), ik.unwrap_or(0));
                self.snap[m].engaged = true;
                self.snap[k].engaged = true;
                d.t += dt;
                match d.kind {
                    Kind::Hug => self.step_cheer_hug(&mut d, pets, m, k, dt),
                    Kind::Tag => self.step_tagalong(&mut d, pets, m, k, dt),
                }
            };
            if going {
                self.mentor.duos[j] = d;
                j += 1;
                continue;
            }
            self.mentor.duos.swap_remove(j);
            for i in [im, ik].into_iter().flatten() {
                pets[i].as_mut().end_romp();
                self.snap[i].engaged = true;
            }
            let wait = behavior::range(&mut self.mentor.rng, MENTOR_COOLDOWN);
            cool(&mut self.mentor.cooldown, d.mentor, self.clock, wait);
        }
    }

    /// 🤗 포옹 한 틱 (m = 무쇠인간, k = 거미인간). 끝나면 false
    fn step_cheer_hug<T: AsRef<Pet> + AsMut<Pet>>(&mut self, d: &mut Duo, pets: &mut [T], m: usize, k: usize, dt: f64) -> bool {
        let (a, b) = (self.snap[m], self.snap[k]);
        if d.approach {
            pets[m].as_mut().set_romp_pose(Pose::go(Anim::Walk, d.x, APPROACH_PACE));
            pets[k].as_mut().set_romp_pose(Pose::hold(Anim::Look).facing(toward(&b, &a)));
            if (a.x - d.x).abs() < 2.0 {
                d.approach = false;
                d.t = 0.0;
                pets[m].as_mut().say_now(Speech::Cheer);
                pets[m].as_mut().romp_heart();
            } else if d.t >= APPROACH_SECS {
                return false;
            }
            return true;
        }
        pets[m].as_mut().set_romp_pose(Pose::hold(Anim::Hug).facing(toward(&a, &b)));
        pets[k].as_mut().set_romp_pose(Pose { sparkle: true, ..Pose::hold(Anim::Heart) });
        d.heart_t += dt;
        if d.heart_t >= HUG_HEART_SECS {
            d.heart_t = 0.0;
            pets[m].as_mut().romp_heart();
            pets[k].as_mut().romp_heart();
        }
        if !d.shucked && d.t >= SHUCKS_AT {
            d.shucked = true;
            pets[k].as_mut().say_now(Speech::Shucks);
        }
        d.t < HUG_SECS
    }

    /// 🗨️ 졸졸 수다 한 틱 (m = 무쇠인간, k = 거미인간). 끝나면 false
    fn step_tagalong<T: AsRef<Pet> + AsMut<Pet>>(&mut self, d: &mut Duo, pets: &mut [T], m: usize, k: usize, dt: f64) -> bool {
        let (a, b) = (self.snap[m], self.snap[k]);
        if d.t >= d.len {
            return false;
        }
        // 무쇠인간: 들어 줄지 달아날지
        if d.t >= d.mood_at {
            d.mood_at = d.t + behavior::range(&mut self.mentor.rng, MOOD_SECS);
            let listen = self.mentor.rng.f64() >= self.mentor.flee;
            if !listen {
                d.flee_x = self.mentor_flee_spot(&a, &b);
                pets[m].as_mut().say_now(Speech::NotNow);
            }
            d.listen = listen;
        }
        if d.listen {
            pets[m].as_mut().set_romp_pose(Pose::hold(Anim::Idle).facing(toward(&a, &b)));
        } else {
            if (a.x - d.flee_x).abs() < 2.0 {
                d.flee_x = self.mentor_flee_spot(&a, &b);
            }
            pets[m].as_mut().set_romp_pose(Pose { lift: FLEE_LIFT, ..Pose::go(Anim::Fly, d.flee_x, FLEE_PACE) });
        }
        // 거미인간: 무쇠인간 곁 (지금 있는 쪽)으로 달려가 재잘재잘
        let side = if b.x <= a.x { -1.0 } else { 1.0 };
        let spot = fit(&b, a.x + side * FOLLOW_GAP * a.size.max(b.size));
        if (b.x - spot).abs() > 6.0 {
            pets[k].as_mut().set_romp_pose(Pose::go(Anim::Run, spot, FOLLOW_PACE));
        } else {
            let anim = if ((d.t / 1.3) as u32).is_multiple_of(2) { Anim::Tilt } else { Anim::Wave };
            pets[k].as_mut().set_romp_pose(Pose::hold(anim).facing(toward(&b, &a)));
        }
        d.since_chat += dt;
        if d.since_chat >= CHAT_SECS {
            d.since_chat = 0.0;
            d.nodded = false;
            pets[k].as_mut().say_now(Speech::KidChat);
        } else if d.listen && !d.nodded && d.since_chat >= NOD_AFTER_SECS {
            d.nodded = true;
            pets[m].as_mut().say_now(Speech::UhHuh);
        }
        true
    }

    /// 🗨️ 무쇠인간이 달아날 곳: 거미인간 반대쪽으로 FLEE_BODIES (막혔으면 거미인간을 지나 반대쪽으로)
    fn mentor_flee_spot(&self, a: &Snap, b: &Snap) -> f64 {
        let away = -toward(a, b);
        let far = fit(a, a.x + away * FLEE_BODIES * a.size);
        if (far - a.x).abs() >= 1.5 * a.size {
            far
        } else {
            fit(a, a.x - away * FLEE_BODIES * a.size)
        }
    }

    /// 테스트: 1초 확률 (포옹, 수다)
    #[cfg(test)]
    pub(crate) fn set_mentor_rates(&mut self, hug: f64, tag: f64) {
        self.mentor.hug_rate = hug;
        self.mentor.tag_rate = tag;
    }

    /// 테스트: 무쇠인간이 달아날 몫
    #[cfg(test)]
    pub(crate) fn set_mentor_flee(&mut self, p: f64) {
        self.mentor.flee = p;
    }

    /// 테스트: 지금까지 시작한 (포옹, 수다) 수
    #[cfg(test)]
    pub(crate) fn mentor_started(&self) -> (usize, usize) {
        (self.mentor.hugs, self.mentor.tags)
    }
}
