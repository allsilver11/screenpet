//! 📖 공부 방해 (2026-11-20 마법사 spec 추가 요청) — social.rs의 자식 모듈 (Social의 비공개 필드에 접근). 펫 쪽 몸짓은 brain_romp.rs (Pose),
//! 헤르미온느 쪽은 brain_study.rs (shush · close_book).
//!
//! 헤르미온느가 앉아 책을 읽는 동안 (Snap.reading) 같은 디딤판 조각, 몸 PESTER_BODIES개 안에 한가한 해리·해그리드가 있으면 가끔 (1초에 PESTER_RATE):
//! 1. 🚶 다가감 (Approach): 헤르미온느 옆으로 걸어감 (APPROACH_SECS 안에 못 가면 그만)
//! 2. 😜 방해 (Bother): "psst!" (Speech::Pester) 하고 BOTHER_SECS 동안 귀엽게 — 🫵 콕콕 (갸웃하며 다가갔다 물러났다) · 👋 책 앞에서 손 흔들기 · 🐸 폴짝폴짝
//! 3. 반응 (React): PLAY_CHANCE로 책을 덮고 "Oh, alright!" → 같이 놂 (🏁 달리기 시합 — 자리가 없으면 술래잡기, 방해한 쪽이 도망),
//!    아니면 "Shh! I'm studying!" 하고 계속 읽음 — 방해한 쪽은 시무룩 (annoyed) 하다 물러남
//! 헤르미온느가 그사이 책을 그만 읽거나 방해꾼이 빠지면 (잡힘 등) 그만둠. 헤르미온느마다 PESTER_COOLDOWN 동안 다시 안 함.
//! 주사위는 따로 (다른 놀이의 주사위 순서는 그대로).

use super::romp::{fit, race_course, same_piece, RompKind};
use super::*;
use crate::brain::romp_moves::Pose;
use crate::brain::{Anim, Speech};
use crate::wizard::Hogwarts;

/// 방해할 수 있을 때 1초에 시작할 확률 (책 읽는 동안 몇 번쯤)
pub const PESTER_RATE: f64 = 0.05;
/// 방해하러 오는 거리 (두 몸 크기 평균 배, 같은 디딤판 조각)
pub const PESTER_BODIES: f64 = 10.0;
/// 책을 덮고 같이 놀 확률 (나머지는 "Shh!")
pub const PLAY_CHANCE: f64 = 0.5;
/// 같은 헤르미온느를 다시 방해하기까지 (초)
pub const PESTER_COOLDOWN: (f64, f64) = (60.0, 120.0);
/// 🚶 다가가는 빠르기 (걷기 속도 배), 시간 상한 (초)
const APPROACH_PACE: f64 = 1.2;
const APPROACH_SECS: f64 = 10.0;
/// 🚶 헤르미온느 옆 간격 (두 몸 반폭 합 배)
const BESIDE: f64 = 0.85;
/// 😜 방해 시간 (초)
const BOTHER_SECS: f64 = 2.4;
/// 😜 콕콕·폴짝 박자 (초)
const BEAT_SECS: f64 = 0.4;
/// 😜 콕 다가가는 거리 (몸 크기 배)
const POKE_REACH: f64 = 0.15;
/// 😜 폴짝 높이 (몸 크기 배)
const HOP_LIFT: f64 = 0.2;
/// 😤 "Shh!" 뒤 시무룩한 시간 (초)
const HUSHED_SECS: f64 = 1.4;

/// 방해 몸짓
#[derive(Debug, Clone, Copy, PartialEq)]
enum Tease {
    /// 🫵 콕콕
    Poke,
    /// 👋 책 앞에서 손 흔들기
    Wave,
    /// 🐸 폴짝폴짝
    Hop,
}

/// 방해 단계
#[derive(Debug, Clone, Copy, PartialEq)]
enum Step {
    /// 🚶 옆으로
    Approach,
    /// 😜 방해
    Bother,
    /// 😤 "Shh!"에 시무룩
    Hushed,
}

/// 하는 중인 방해 하나
#[derive(Debug, Clone, Copy)]
struct Pest {
    /// 방해꾼 · 헤르미온느 (social_key)
    pest: u64,
    reader: u64,
    /// 설 자리
    x: f64,
    tease: Tease,
    step: Step,
    /// 이 단계에 든 뒤 흐른 시간
    t: f64,
}

/// 코디네이터의 기억
pub(super) struct SocialPester {
    pests: Vec<Pest>,
    /// 헤르미온느 → 다시 방해받을 수 있는 시각 (clock)
    cooldown: HashMap<u64, f64>,
    rng: fastrand::Rng,
    /// 1초 확률 (테스트에서 바꿈)
    rate: f64,
    /// 책을 덮고 놀 확률 (테스트에서 바꿈)
    play: f64,
    /// 지금까지 시작한 방해 수
    started: usize,
}

impl SocialPester {
    /// seed로 주사위를 따로 둠
    pub(super) fn new(seed: u64) -> Self {
        SocialPester { pests: Vec::new(), cooldown: HashMap::new(), rng: fastrand::Rng::with_seed(seed ^ 0x7368_6821), rate: PESTER_RATE, play: PLAY_CHANCE, started: 0 }
    }
}

impl Social {
    /// 새 방해: 책 읽는 헤르미온느마다 같은 조각 가까이 한가한 해리·해그리드가 있으면 주사위
    pub(super) fn find_pester<T: AsRef<Pet> + AsMut<Pet>>(&mut self, dt: f64, pets: &mut [T]) {
        let n = self.snap.len();
        for r in 0..n {
            let reader = self.snap[r];
            if !reader.reading || reader.hogwarts != Some(Hogwarts::Hermione) || cooling(&self.pester.cooldown, &reader.key, self.clock) || self.pester.pests.iter().any(|p| p.reader == reader.key) {
                continue;
            }
            let Some(i) = (0..n).find(|&i| {
                let s = &self.snap[i];
                i != r
                    && matches!(s.hogwarts, Some(Hogwarts::Harry | Hogwarts::Hagrid))
                    && s.romp_free
                    && !s.engaged
                    && same_piece(&reader, s)
                    && (s.x - reader.x).abs() <= PESTER_BODIES * (s.size + reader.size) / 2.0
            }) else {
                continue;
            };
            if self.pester.rng.f64() >= self.pester.rate * dt {
                continue;
            }
            let s = self.snap[i];
            // 다가오던 쪽 옆에 (자리가 없으면 반대쪽)
            let side = if s.x <= reader.x { -1.0 } else { 1.0 };
            let gap = BESIDE * (s.half_w + reader.half_w);
            let near = fit(&s, reader.x + side * gap);
            let x = if (near - reader.x).abs() >= 0.5 * gap { near } else { fit(&s, reader.x - side * gap) };
            let tease = [Tease::Poke, Tease::Wave, Tease::Hop][self.pester.rng.usize(0..3)];
            pets[i].as_mut().start_romp(Pose::go(Anim::Walk, x, APPROACH_PACE));
            self.snap[i].engaged = true;
            self.pester.pests.push(Pest { pest: s.key, reader: reader.key, x, tease, step: Step::Approach, t: 0.0 });
            self.pester.started += 1;
        }
    }

    /// 하는 중인 방해들 한 틱: 방해꾼이 빠졌거나 헤르미온느가 책을 그만 읽었으면 그만둠
    pub(super) fn step_pesters<T: AsRef<Pet> + AsMut<Pet>>(&mut self, dt: f64, pets: &mut [T]) {
        let mut k = 0;
        while k < self.pester.pests.len() {
            let mut p = self.pester.pests[k];
            let (ip, ir) = (self.index(p.pest), self.index(p.reader));
            let pest_in = ip.is_some_and(|i| pets[i].as_ref().in_romp());
            let reading = ir.is_some_and(|i| self.snap[i].reading);
            let (Some(ip), Some(ir)) = (ip, ir) else {
                self.end_pest_at(k, pets);
                continue;
            };
            // 😤 시무룩 단계는 헤르미온느가 계속 읽든 말든 끝까지
            if !pest_in || (!reading && p.step != Step::Hushed) {
                self.end_pest_at(k, pets);
                continue;
            }
            self.snap[ip].engaged = true;
            p.t += dt;
            match self.step_pest(&mut p, pets, ip, ir) {
                Some(()) => {
                    self.pester.pests[k] = p;
                    k += 1;
                }
                None => {
                    // 놀이로 넘어갔거나 끝남 — 방해꾼은 이미 다음 일로
                    self.pester.pests.swap_remove(k);
                    let wait = behavior::range(&mut self.pester.rng, PESTER_COOLDOWN);
                    cool(&mut self.pester.cooldown, p.reader, self.clock, wait);
                }
            }
        }
    }

    /// 방해 k를 그만둠: 방해꾼을 쉬게 하고, 헤르미온느에 쿨다운
    fn end_pest_at<T: AsRef<Pet> + AsMut<Pet>>(&mut self, k: usize, pets: &mut [T]) {
        let p = self.pester.pests.swap_remove(k);
        if let Some(i) = self.index(p.pest) {
            pets[i].as_mut().end_romp();
            self.snap[i].engaged = true;
        }
        let wait = behavior::range(&mut self.pester.rng, PESTER_COOLDOWN);
        cool(&mut self.pester.cooldown, p.reader, self.clock, wait);
    }

    /// 방해 한 틱 (ip = 방해꾼, ir = 헤르미온느 자리). 끝나면 None (끝낼 때 방해꾼은 이미 쉬거나 놀이 중)
    fn step_pest<T: AsRef<Pet> + AsMut<Pet>>(&mut self, p: &mut Pest, pets: &mut [T], ip: usize, ir: usize) -> Option<()> {
        let (s, r) = (self.snap[ip], self.snap[ir]);
        let toward = if r.x >= s.x { 1.0 } else { -1.0 };
        match p.step {
            Step::Approach => {
                pets[ip].as_mut().set_romp_pose(Pose::go(Anim::Walk, p.x, APPROACH_PACE));
                if (s.x - p.x).abs() < 2.0 {
                    p.step = Step::Bother;
                    p.t = 0.0;
                    pets[ip].as_mut().say_now(Speech::Pester);
                } else if p.t >= APPROACH_SECS {
                    pets[ip].as_mut().end_romp();
                    return None;
                }
            }
            Step::Bother => {
                let beat = (p.t / BEAT_SECS) as u32;
                let pose = match p.tease {
                    // 콕: 박자마다 살짝 다가갔다 물러남 (갸웃)
                    Tease::Poke if beat % 2 == 0 => Pose::go(Anim::Tilt, fit(&s, p.x + toward * POKE_REACH * s.size), f64::INFINITY).facing(toward),
                    Tease::Poke => Pose::go(Anim::Look, p.x, f64::INFINITY).facing(toward),
                    Tease::Wave => Pose::hold(Anim::Wave),
                    Tease::Hop => {
                        let k = (p.t / BEAT_SECS).fract();
                        Pose { lift: HOP_LIFT * (std::f64::consts::PI * k).sin(), ..Pose::hold(Anim::Hop).facing(toward) }
                    }
                };
                pets[ip].as_mut().set_romp_pose(pose);
                if p.t >= BOTHER_SECS {
                    if self.pester.rng.f64() < self.pester.play {
                        pets[ir].as_mut().close_book();
                        self.snap[ir].engaged = true;
                        self.play_after_pester(pets, ip, ir);
                        return None;
                    }
                    pets[ir].as_mut().shush();
                    p.step = Step::Hushed;
                    p.t = 0.0;
                }
            }
            Step::Hushed => {
                pets[ip].as_mut().set_romp_pose(Pose::hold(Anim::Annoyed).facing(toward));
                if p.t >= HUSHED_SECS {
                    pets[ip].as_mut().end_romp();
                    return None;
                }
            }
        }
        Some(())
    }

    /// 책을 덮은 헤르미온느 (ir)와 방해꾼 (ip)이 같이 놂: 🏁 달리기 시합 (자리가 있으면), 아니면 술래잡기 (헤르미온느가 술래)
    fn play_after_pester<T: AsRef<Pet> + AsMut<Pet>>(&mut self, pets: &mut [T], ip: usize, ir: usize) {
        let (a, b) = (self.snap[ir], self.snap[ip]);
        if race_course(&a, &b).is_some() && self.pester.rng.bool() && self.start_romp(pets, RompKind::Race, ir, ip) {
            return;
        }
        let phase = self.start_chase(pets, ir, ip, true);
        // 이번 틱의 짝 놀이 차례 (step_links)가 새 역할을 보게
        self.snap[ir].role = pets[ir].as_ref().social_role();
        self.snap[ip].role = pets[ip].as_ref().social_role();
        self.links.push(Link { a: a.key, b: b.key, phase, played: true, swapped: false });
    }

    /// 테스트: 1초 확률
    #[cfg(test)]
    pub(crate) fn set_pester_rate(&mut self, rate: f64) {
        self.pester.rate = rate;
    }

    /// 테스트: 책을 덮고 놀 확률
    #[cfg(test)]
    pub(crate) fn set_pester_play(&mut self, p: f64) {
        self.pester.play = p;
    }

    /// 테스트: 지금까지 시작한 방해 수
    #[cfg(test)]
    pub(crate) fn pesters_started(&self) -> usize {
        self.pester.started
    }
}
