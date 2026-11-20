//! 🕷️🕷️🕷️ 멀티버스 모임 (2026-11-20 spec 4부) — social.rs의 자식 모듈 (Social의 비공개 필드에 접근). 펫 쪽 몸짓은 brain_romp.rs (Pose).
//!
//! 같은 화면 (내 펫 + 친구 펫 손님 — Social이 모든 slot을 봄)의 같은 디딤판 조각, 몸 MULTIVERSE_BODIES개 안에 한가한 거미인간이
//! MULTIVERSE_MIN마리 (둘) 이상 있으면 곧 (1초에 MULTIVERSE_RATE) 모임:
//! 1. 🚶 모임 (Gather): 가운데로 걸어가 나란히 섬
//! 2. 😲 깜짝 (Gasp): 서로 마주 보며 번갈아 "multiverse?!" / "wait… you too?" / "no way!" (Speech::Multiverse)
//! 3. 🤗 껴안기 (Hug): 한 덩어리로 붙음 (하트) — 둘이면 짝꿍 포옹, 셋 넘으면 무리
//! 4. 🌀 뱅글뱅글 (Spin): 가운데를 두고 SPIN_TURNS바퀴 돎 (자리가 원을 그리고 뒤쪽은 살짝 뜸, 반짝이·하트)
//! 5. 👋 흩어지기 (Scatter): "see ya!" (Speech::SeeYa) 하며 양쪽으로 달려감
//! 끝나면 모인 거미인간마다 MULTIVERSE_COOLDOWN 동안 다시 안 함. 누가 빠지면 (잡힘 등 — 몸짓을 안 받음) 모두 풀어 줌.
//! 주사위는 따로 (다른 놀이의 주사위 순서는 그대로). 새 그림 없이 기존 그림·효과로.

use super::romp::{fit, same_piece};
use super::*;
use crate::brain::romp_moves::Pose;
use crate::brain::{Anim, Speech};

/// 모임이 되는 가장 적은 거미인간 수
pub const MULTIVERSE_MIN: usize = 2;
/// 모일 수 있을 때 1초에 모임을 시작할 확률 (높음 — 셋 다 한가해지면 곧)
pub const MULTIVERSE_RATE: f64 = 0.5;
/// 모일 수 있는 거리 (몸 크기 배, 같은 디딤판 조각에서 첫 거미인간부터)
pub const MULTIVERSE_BODIES: f64 = 14.0;
/// 모인 거미인간이 다시 모이기까지 (초)
pub const MULTIVERSE_COOLDOWN: (f64, f64) = (180.0, 300.0);
/// 🚶 모이는 빠르기 (걷기 속도 배)
const GATHER_PACE: f64 = 1.4;
/// 🚶 모이는 시간 상한 (초)
const GATHER_SECS: f64 = 8.0;
/// 🚶 나란히 선 간격 (몸 크기 배)
const GATHER_GAP: f64 = 0.8;
/// 😲 말 사이 (초)
const GASP_LINE_SECS: f64 = 1.1;
/// 😲 모인 수보다 더 하는 말 수 (주사위 0..=이만큼)
const GASP_EXTRA: u8 = 1;
/// 🤗 붙는 시간 (초)
const HUG_SECS: f64 = 0.6;
/// 🤗 붙은 간격 (몸 크기 배)
const HUG_GAP: f64 = 0.3;
/// 🤗 붙으러 가는 빠르기 (걷기 속도 배)
const HUG_PACE: f64 = 2.5;
/// 🌀 도는 시간 (초)
const SPIN_SECS: f64 = 3.0;
/// 🌀 도는 바퀴 수
const SPIN_TURNS: f64 = 3.0;
/// 🌀 도는 원 반지름 (몸 크기 배)
const SPIN_RADIUS: f64 = 0.35;
/// 🌀 원 뒤쪽에서 뜨는 높이 (몸 크기 배)
const SPIN_LIFT: f64 = 0.15;
/// 🌀 하트 간격 (초)
const SPIN_HEART_SECS: f64 = 1.0;
/// 👋 흩어지는 시간 (초)
const SCATTER_SECS: f64 = 1.6;
/// 👋 흩어지는 간격 (몸 크기 배 — 자리 하나마다)
const SCATTER_GAP: f64 = 2.5;
/// 👋 흩어지는 빠르기 (걷기 속도 배)
const SCATTER_PACE: f64 = 1.8;
/// 👋 "see ya!" 사이 (초)
const SEE_YA_SECS: f64 = 0.25;

/// 모임 단계
#[derive(Debug, Clone, Copy, PartialEq)]
enum Step {
    /// 🚶 가운데로
    Gather,
    /// 😲 번갈아 놀람
    Gasp,
    /// 🤗 붙음
    Hug,
    /// 🌀 뱅글뱅글
    Spin,
    /// 👋 흩어짐
    Scatter,
}

/// 하는 중인 모임 하나
#[derive(Debug, Clone)]
struct Gathering {
    /// 모인 거미인간 (social_key — 처음 x 순서)
    keys: Vec<u64>,
    /// 가운데 x
    cx: f64,
    /// 몸 크기 (모인 것 중 가장 큰)
    size: f64,
    step: Step,
    /// 이 단계에 든 뒤 흐른 시간
    t: f64,
    /// 센 것: 한 말 수·하트 수
    n: u8,
    /// 정해 둔 말 수
    m: u8,
}

/// 코디네이터의 기억
pub(super) struct SocialMultiverse {
    gatherings: Vec<Gathering>,
    /// 거미인간 → 다시 모일 수 있는 시각 (clock)
    cooldown: HashMap<u64, f64>,
    rng: fastrand::Rng,
    /// 1초 확률 (테스트에서 바꿈)
    rate: f64,
    /// 지금까지 시작한 모임 수
    started: usize,
}

impl SocialMultiverse {
    /// seed로 주사위를 따로 둠
    pub(super) fn new(seed: u64) -> Self {
        SocialMultiverse { gatherings: Vec::new(), cooldown: HashMap::new(), rng: fastrand::Rng::with_seed(seed ^ 0x6d75_6c74), rate: MULTIVERSE_RATE, started: 0 }
    }
}

/// k번째 (0부터, n 중) 자리의 가운데에서 몫 (-(n-1)/2 ..= (n-1)/2)
fn slot(k: usize, n: usize) -> f64 {
    k as f64 - (n as f64 - 1.0) / 2.0
}

impl Social {
    /// 새 모임: 한가한 거미인간이 같은 조각 가까이 MULTIVERSE_MIN마리 이상 있으면 주사위 (한 틱에 한 무리만)
    pub(super) fn find_multiverse<T: AsRef<Pet> + AsMut<Pet>>(&mut self, dt: f64, pets: &mut [T]) {
        let n = self.snap.len();
        if n < MULTIVERSE_MIN {
            return;
        }
        let now = self.clock;
        let ok = |s: &Snap, cd: &HashMap<u64, f64>| s.spider && s.romp_free && !s.engaged && !cooling(cd, &s.key, now);
        let Some(first) = (0..n).find(|&i| ok(&self.snap[i], &self.multiverse.cooldown)) else { return };
        let a = self.snap[first];
        let group: Vec<usize> = (0..n)
            .filter(|&j| {
                let b = self.snap[j];
                ok(&b, &self.multiverse.cooldown) && same_piece(&a, &b) && (b.x - a.x).abs() <= MULTIVERSE_BODIES * a.size.max(b.size)
            })
            .collect();
        if group.len() < MULTIVERSE_MIN || self.multiverse.rng.f64() >= self.multiverse.rate * dt {
            return;
        }
        self.start_multiverse(pets, group);
    }

    /// 모임을 시작함: 가운데로 걸어가게 함
    fn start_multiverse<T: AsRef<Pet> + AsMut<Pet>>(&mut self, pets: &mut [T], mut group: Vec<usize>) {
        group.sort_by(|&a, &b| self.snap[a].x.total_cmp(&self.snap[b].x));
        let size = group.iter().map(|&i| self.snap[i].size).fold(0.0, f64::max);
        let mut cx = group.iter().map(|&i| self.snap[i].x).sum::<f64>() / group.len() as f64;
        // 나란히 설 자리가 조각 안에 들게
        if let Some((lo, hi)) = self.snap[group[0]].ground {
            let half = slot(group.len() - 1, group.len()) * SCATTER_GAP * size + size;
            if hi - lo > 2.0 * half {
                cx = cx.clamp(lo + half, hi - half);
            }
        }
        let n = group.len();
        for (k, &i) in group.iter().enumerate() {
            let s = self.snap[i];
            pets[i].as_mut().start_romp(Pose::go(Anim::Walk, fit(&s, cx + slot(k, n) * GATHER_GAP * size), GATHER_PACE));
            self.snap[i].engaged = true;
        }
        let keys = group.iter().map(|&i| self.snap[i].key).collect();
        self.multiverse.gatherings.push(Gathering { keys, cx, size, step: Step::Gather, t: 0.0, n: 0, m: 0 });
        self.multiverse.started += 1;
    }

    /// 하는 중인 모임들 한 틱: 누가 빠졌으면 모두 풀어 줌
    pub(super) fn step_multiverse<T: AsRef<Pet> + AsMut<Pet>>(&mut self, dt: f64, pets: &mut [T]) {
        let mut k = 0;
        while k < self.multiverse.gatherings.len() {
            let mut g = self.multiverse.gatherings[k].clone();
            let ix: Vec<Option<usize>> = g.keys.iter().map(|&key| self.index(key)).collect();
            let live = ix.iter().all(|i| i.is_some_and(|i| pets[i].as_ref().in_romp()));
            if !live {
                self.end_multiverse_at(k, pets);
                continue;
            }
            let ix: Vec<usize> = ix.into_iter().flatten().collect();
            for &i in &ix {
                self.snap[i].engaged = true;
            }
            g.t += dt;
            let going = self.step_gathering(&mut g, pets, &ix);
            self.multiverse.gatherings[k] = g;
            if going {
                k += 1;
            } else {
                self.end_multiverse_at(k, pets);
            }
        }
    }

    /// 모임 k를 끝냄: 아직 몸짓을 받는 쪽은 쉬게 하고, 모인 거미인간마다 쿨다운
    fn end_multiverse_at<T: AsRef<Pet> + AsMut<Pet>>(&mut self, k: usize, pets: &mut [T]) {
        let g = self.multiverse.gatherings.swap_remove(k);
        let now = self.clock;
        for key in g.keys {
            if let Some(i) = self.index(key) {
                pets[i].as_mut().end_romp();
                self.snap[i].engaged = true;
            }
            let wait = behavior::range(&mut self.multiverse.rng, MULTIVERSE_COOLDOWN);
            cool(&mut self.multiverse.cooldown, key, now, wait);
        }
    }

    /// 다음 단계로 (시간·센 것을 0부터)
    fn next_step(g: &mut Gathering, step: Step) {
        g.step = step;
        g.t = 0.0;
        g.n = 0;
    }

    /// 모임 한 틱 (ix = 모인 거미인간 자리, keys 순서). 끝나면 false
    fn step_gathering<T: AsRef<Pet> + AsMut<Pet>>(&mut self, g: &mut Gathering, pets: &mut [T], ix: &[usize]) -> bool {
        let n = ix.len();
        let size = g.size;
        match g.step {
            Step::Gather => {
                let mut there = true;
                for (k, &i) in ix.iter().enumerate() {
                    let s = self.snap[i];
                    let x = fit(&s, g.cx + slot(k, n) * GATHER_GAP * size);
                    there &= (s.x - x).abs() < 2.0;
                    pets[i].as_mut().set_romp_pose(Pose::go(Anim::Walk, x, GATHER_PACE));
                }
                if there || g.t >= GATHER_SECS {
                    Self::next_step(g, Step::Gasp);
                    g.m = n.max(MULTIVERSE_MIN) as u8 + self.multiverse.rng.u8(0..=GASP_EXTRA);
                }
            }
            Step::Gasp => {
                let speaker = usize::from(g.n.max(1) - 1) % n;
                for (k, &i) in ix.iter().enumerate() {
                    let s = self.snap[i];
                    // 말하는 쪽은 옆 친구를, 나머지는 말하는 쪽을 봄
                    let other = if k == speaker { self.snap[ix[if k + 1 < n { k + 1 } else { k - 1 }]].x } else { self.snap[ix[speaker]].x };
                    let face = if other >= s.x { 1.0 } else { -1.0 };
                    let anim = if k == speaker && g.n > 0 { Anim::Wave } else { Anim::Look };
                    pets[i].as_mut().set_romp_pose(Pose::hold(anim).facing(face));
                }
                if g.n < g.m && g.t >= GASP_LINE_SECS * f64::from(g.n) {
                    let who = usize::from(g.n) % n;
                    pets[ix[who]].as_mut().say_now(Speech::Multiverse);
                    g.n += 1;
                } else if g.n >= g.m && g.t >= GASP_LINE_SECS * f64::from(g.m) {
                    Self::next_step(g, Step::Hug);
                    for &i in ix {
                        pets[i].as_mut().romp_heart();
                    }
                }
            }
            Step::Hug => {
                for (k, &i) in ix.iter().enumerate() {
                    let s = self.snap[i];
                    let face = if slot(k, n) <= 0.0 { 1.0 } else { -1.0 };
                    pets[i].as_mut().set_romp_pose(Pose::go(Anim::Heart, fit(&s, g.cx + slot(k, n) * HUG_GAP * size), HUG_PACE).facing(face));
                }
                if g.t >= HUG_SECS {
                    Self::next_step(g, Step::Spin);
                }
            }
            Step::Spin => {
                let turn = std::f64::consts::TAU * SPIN_TURNS * (g.t / SPIN_SECS).min(1.0);
                for (k, &i) in ix.iter().enumerate() {
                    let s = self.snap[i];
                    let phi = std::f64::consts::TAU * k as f64 / n as f64 + turn;
                    let x = fit(&s, g.cx + SPIN_RADIUS * size * phi.cos());
                    // 원의 뒤쪽 (sin > 0)은 살짝 뜸, 도는 쪽을 봄
                    let face = if -phi.sin() >= 0.0 { 1.0 } else { -1.0 };
                    let pose = Pose { lift: SPIN_LIFT * 0.5 * (1.0 + phi.sin()), sparkle: true, ..Pose::go(Anim::Heart, x, f64::INFINITY).facing(face) };
                    pets[i].as_mut().set_romp_pose(pose);
                }
                if g.t >= SPIN_HEART_SECS * f64::from(g.n) {
                    for &i in ix {
                        pets[i].as_mut().romp_heart();
                    }
                    g.n += 1;
                }
                if g.t >= SPIN_SECS {
                    Self::next_step(g, Step::Scatter);
                }
            }
            Step::Scatter => {
                for (k, &i) in ix.iter().enumerate() {
                    let s = self.snap[i];
                    let off = slot(k, n);
                    let pose = if off == 0.0 { Pose::hold(Anim::Wave) } else { Pose::go(Anim::Run, fit(&s, g.cx + off * SCATTER_GAP * size), SCATTER_PACE) };
                    pets[i].as_mut().set_romp_pose(pose);
                }
                if usize::from(g.n) < n && g.t >= SEE_YA_SECS * f64::from(g.n) {
                    pets[ix[usize::from(g.n)]].as_mut().say_now(Speech::SeeYa);
                    g.n += 1;
                }
                if g.t >= SCATTER_SECS && usize::from(g.n) >= n {
                    return false;
                }
            }
        }
        true
    }

    /// 테스트: 지금까지 시작한 멀티버스 모임 수
    #[cfg(test)]
    pub(crate) fn multiverses_started(&self) -> usize {
        self.multiverse.started
    }
}
