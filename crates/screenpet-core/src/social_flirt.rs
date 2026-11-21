//! 💕 짱구의 작업 (짱구 spec 2026-11-21) — social.rs의 자식 모듈 (Social의 비공개 필드에 접근). 짱구 쪽 장난은 brain_jjanggu.rs.
//!
//! 헤르미온느가 한가할 때 같은 디딤판 조각, 몸 FLIRT_BODIES개 안에 한가한 짱구가 있으면 가끔 (1초에 FLIRT_RATE):
//! 1. 🏃 쪼르르 (Approach): 짱구가 헤르미온느 옆으로 달려감 (헤르미온느는 제자리에서 짱구 쪽을 봄 — APPROACH_SECS 안에 못 가면 그만)
//! 2. 💕 작업 (Woo): "Hey pretty~" (Speech::Flirt) 하고 WOO_SECS 동안 볼에 손 대고 쑥스럽게 (flirt)
//! 3. 반응 (React): SMILE_CHANCE로 웃어 줌 (heart, "Oh! Thank you~") — 아니면 흥! 고개 돌려 뾰로통 (annoyed, "Hmph!").
//!    짱구는 어느 쪽이든 쑥스러운 채 (굴하지 않음) REACT_SECS 뒤 둘 다 풀려남
//! 둘 중 하나가 빠지면 (잡힘 등) 그만둠. 헤르미온느마다 FLIRT_COOLDOWN 동안 다시 안 함. 주사위는 따로 (다른 놀이의 주사위 순서는 그대로).

use super::romp::{fit, same_piece};
use super::*;
use crate::brain::romp_moves::Pose;
use crate::brain::{Anim, Speech};
use crate::wizard::Hogwarts;

/// 작업할 수 있을 때 1초에 시작할 확률
pub const FLIRT_RATE: f64 = 0.02;
/// 달려오는 거리 (두 몸 크기 평균 배, 같은 디딤판 조각)
pub const FLIRT_BODIES: f64 = 12.0;
/// 웃어 줄 확률 (나머지는 "Hmph!")
pub const SMILE_CHANCE: f64 = 0.5;
/// 같은 헤르미온느에게 다시 작업하기까지 (초)
pub const FLIRT_COOLDOWN: (f64, f64) = (70.0, 150.0);
/// 🏃 달려가는 빠르기 (걷기 속도 배), 시간 상한 (초)
const APPROACH_PACE: f64 = 2.2;
const APPROACH_SECS: f64 = 10.0;
/// 🏃 헤르미온느 옆 간격 (두 몸 반폭 합 배)
const BESIDE: f64 = 0.9;
/// 💕 작업 시간 · 반응 시간 (초)
const WOO_SECS: f64 = 2.2;
const REACT_SECS: f64 = 2.4;

/// 작업 단계
#[derive(Debug, Clone, Copy, PartialEq)]
enum Step {
    /// 🏃 옆으로
    Approach,
    /// 💕 "Hey pretty~"
    Woo,
    /// 헤르미온느 반응 (smiled = 웃어 줌)
    React { smiled: bool },
}

/// 하는 중인 작업 하나
#[derive(Debug, Clone, Copy)]
struct Woo {
    /// 짱구 · 헤르미온느 (social_key)
    rascal: u64,
    lady: u64,
    /// 설 자리
    x: f64,
    step: Step,
    /// 이 단계에 든 뒤 흐른 시간
    t: f64,
}

/// 코디네이터의 기억
pub(super) struct SocialFlirt {
    woos: Vec<Woo>,
    /// 헤르미온느 → 다시 작업받을 수 있는 시각 (clock)
    cooldown: HashMap<u64, f64>,
    rng: fastrand::Rng,
    /// 1초 확률 (테스트에서 바꿈)
    rate: f64,
    /// 웃어 줄 확률 (테스트에서 바꿈)
    smile: f64,
    /// 지금까지 시작한 작업 수
    started: usize,
}

impl SocialFlirt {
    /// seed로 주사위를 따로 둠
    pub(super) fn new(seed: u64) -> Self {
        SocialFlirt { woos: Vec::new(), cooldown: HashMap::new(), rng: fastrand::Rng::with_seed(seed ^ 0x666c_6972), rate: FLIRT_RATE, smile: SMILE_CHANCE, started: 0 }
    }
}

impl Social {
    /// 새 작업: 한가한 헤르미온느마다 같은 조각 가까이 한가한 짱구가 있으면 주사위
    pub(super) fn find_flirt<T: AsRef<Pet> + AsMut<Pet>>(&mut self, dt: f64, pets: &mut [T]) {
        let n = self.snap.len();
        for h in 0..n {
            let lady = self.snap[h];
            if lady.hogwarts != Some(Hogwarts::Hermione) || !lady.romp_free || lady.engaged || cooling(&self.flirt.cooldown, &lady.key, self.clock) {
                continue;
            }
            let Some(i) = (0..n).find(|&i| {
                let s = &self.snap[i];
                i != h && s.rascal && s.romp_free && !s.engaged && same_piece(&lady, s) && (s.x - lady.x).abs() <= FLIRT_BODIES * (s.size + lady.size) / 2.0
            }) else {
                continue;
            };
            if self.flirt.rng.f64() >= self.flirt.rate * dt {
                continue;
            }
            let s = self.snap[i];
            // 달려오던 쪽 옆에 (자리가 없으면 반대쪽)
            let side = if s.x <= lady.x { -1.0 } else { 1.0 };
            let gap = BESIDE * (s.half_w + lady.half_w);
            let near = fit(&s, lady.x + side * gap);
            let x = if (near - lady.x).abs() >= 0.5 * gap { near } else { fit(&s, lady.x - side * gap) };
            pets[i].as_mut().start_romp(Pose::go(Anim::Run, x, APPROACH_PACE));
            pets[h].as_mut().start_romp(Pose::hold(Anim::Idle).facing(if s.x < lady.x { -1.0 } else { 1.0 }));
            self.snap[i].engaged = true;
            self.snap[h].engaged = true;
            self.flirt.woos.push(Woo { rascal: s.key, lady: lady.key, x, step: Step::Approach, t: 0.0 });
            self.flirt.started += 1;
        }
    }

    /// 하는 중인 작업들 한 틱: 둘 중 하나가 빠졌으면 그만둠
    pub(super) fn step_flirts<T: AsRef<Pet> + AsMut<Pet>>(&mut self, dt: f64, pets: &mut [T]) {
        let mut k = 0;
        while k < self.flirt.woos.len() {
            let mut w = self.flirt.woos[k];
            let (ir, il) = (self.index(w.rascal), self.index(w.lady));
            let both = matches!((ir, il), (Some(a), Some(b)) if pets[a].as_ref().in_romp() && pets[b].as_ref().in_romp());
            let (Some(ir), Some(il), true) = (ir, il, both) else {
                self.end_woo_at(k, pets, ir, il);
                continue;
            };
            self.snap[ir].engaged = true;
            self.snap[il].engaged = true;
            w.t += dt;
            if self.step_woo(&mut w, pets, ir, il) {
                self.flirt.woos[k] = w;
                k += 1;
            } else {
                self.end_woo_at(k, pets, Some(ir), Some(il));
            }
        }
    }

    /// 작업 k를 끝냄: 둘 다 쉬게 하고 헤르미온느에 쿨다운
    fn end_woo_at<T: AsRef<Pet> + AsMut<Pet>>(&mut self, k: usize, pets: &mut [T], ir: Option<usize>, il: Option<usize>) {
        let w = self.flirt.woos.swap_remove(k);
        for i in [ir, il].into_iter().flatten() {
            pets[i].as_mut().end_romp();
            self.snap[i].engaged = true;
        }
        let wait = behavior::range(&mut self.flirt.rng, FLIRT_COOLDOWN);
        cool(&mut self.flirt.cooldown, w.lady, self.clock, wait);
    }

    /// 작업 한 틱 (ir = 짱구, il = 헤르미온느 자리). 끝나면 false
    fn step_woo<T: AsRef<Pet> + AsMut<Pet>>(&mut self, w: &mut Woo, pets: &mut [T], ir: usize, il: usize) -> bool {
        let (r, l) = (self.snap[ir], self.snap[il]);
        let toward = if l.x >= r.x { 1.0 } else { -1.0 };
        match w.step {
            Step::Approach => {
                pets[ir].as_mut().set_romp_pose(Pose::go(Anim::Run, w.x, APPROACH_PACE));
                pets[il].as_mut().set_romp_pose(Pose::hold(Anim::Idle).facing(-toward));
                if (r.x - w.x).abs() < 2.0 {
                    w.step = Step::Woo;
                    w.t = 0.0;
                    pets[ir].as_mut().say_now(Speech::Flirt);
                } else if w.t >= APPROACH_SECS {
                    return false;
                }
            }
            Step::Woo => {
                pets[ir].as_mut().set_romp_pose(Pose::hold(Anim::Flirt).facing(toward));
                pets[il].as_mut().set_romp_pose(Pose::hold(Anim::Idle).facing(-toward));
                if w.t >= WOO_SECS {
                    let smiled = self.flirt.rng.f64() < self.flirt.smile;
                    pets[il].as_mut().say_now(if smiled { Speech::Flattered } else { Speech::BrushOff });
                    if smiled {
                        pets[ir].as_mut().romp_heart();
                    }
                    w.step = Step::React { smiled };
                    w.t = 0.0;
                }
            }
            Step::React { smiled } => {
                pets[ir].as_mut().set_romp_pose(Pose::hold(Anim::Flirt).facing(toward));
                // 웃어 주면 하트를 안고 짱구 쪽, 아니면 고개를 홱 돌려 뾰로통
                let pose = if smiled { Pose::hold(Anim::Heart).facing(-toward) } else { Pose::hold(Anim::Annoyed).facing(toward) };
                pets[il].as_mut().set_romp_pose(pose);
                if w.t >= REACT_SECS {
                    return false;
                }
            }
        }
        true
    }

    /// 테스트: 1초 확률
    #[cfg(test)]
    pub(crate) fn set_flirt_rate(&mut self, rate: f64) {
        self.flirt.rate = rate;
    }

    /// 테스트: 웃어 줄 확률
    #[cfg(test)]
    pub(crate) fn set_flirt_smile(&mut self, p: f64) {
        self.flirt.smile = p;
    }

    /// 테스트: 지금까지 시작한 작업 수
    #[cfg(test)]
    pub(crate) fn flirts_started(&self) -> usize {
        self.flirt.started
    }
}
