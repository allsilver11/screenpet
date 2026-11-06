//! 💩 똥피하기 — 화면 위에서 똥이 떨어지고, 방향키로 내 동물을 움직여 피한다. 한 번 맞으면 끝, 점수 = 바닥까지 떨어진 똥 수.
//!
//! - 판은 한 모니터의 작업 영역 (물리 px). 동물은 조작 모드로 움직인다 (걷기·점프·창 위에 오르기 그대로).
//! - 시간이 갈수록 똥이 빨라지고 자주 떨어진다. 몇 개에 하나는 동물 머리 위를 노린다 (가만히 서 있으면 맞는다).
//! - 동물이 판 밖(다른 모니터·친구 화면)으로 나가면 그 자리에서 끝난다.
//! - 친구 랭킹: 최고 기록만 친구 링크의 "부탁"(`dodge:<점수>`)으로 주고받는다. 예전 판은 모르는 부탁을 무시한다.
//!
//! 시간은 `tick`이 받는 dt(초)만 더한다.

use crate::brain::Pet;
use crate::geom::Rect;

/// 똥 반지름 (논리 px)
pub const POOP_R: f64 = 17.0;
/// 처음 떨어지는 속도 (논리 px/s), 1초마다 이만큼 빨라진다, 가장 빠른 속도
pub const FALL_START: f64 = 240.0;
pub const FALL_GAIN: f64 = 9.0;
pub const FALL_MAX: f64 = 950.0;
/// 속도 흔들림 (배, ±)
const FALL_JITTER: f64 = 0.2;
/// 처음 떨어지는 간격 (초, 판 폭 SPAWN_WIDTH 기준), 1초마다 줄어드는 양, 가장 짧은 간격
pub const SPAWN_START: f64 = 0.8;
const SPAWN_GAIN: f64 = 0.011;
pub const SPAWN_MIN: f64 = 0.14;
/// 이 폭(논리 px)일 때 위 간격 그대로 — 넓은 화면엔 더 자주 떨어진다 (폭 비율, 0.5~2배)
const SPAWN_WIDTH: f64 = 1600.0;
/// 시작하고 이만큼은 떨어지지 않는다 (초)
pub const GRACE_SECS: f64 = 1.0;
/// 이 확률로 동물 머리 위를 노린다 (가만히 서 있지 못하게), 노릴 때 흩어지는 폭 (동물 반 폭 배)
const AIM_CHANCE: f64 = 0.3;
const AIM_SPREAD: f64 = 2.0;
/// 맞음 판정은 그림보다 조금 작게 (똥 반지름 배, 동물 반 폭 배)
const HIT_POOP: f64 = 0.6;
const HIT_PET: f64 = 0.75;
/// 동물 머리 높이 (몸 크기 배 — 그림의 머리 끝)
const PET_HEIGHT: f64 = 0.8;
/// 한 판의 똥 수 상한 (메모리가 끝없이 늘지 않게 — 넘으면 새로 떨어뜨리지 않는다)
const MAX_POOPS: usize = 200;
/// 점수·기록 상한
pub const MAX_SCORE: u32 = 999_999;
/// 친구에게 최고 기록을 알리는 부탁 머리
pub const ASK_PREFIX: &str = "dodge:";

/// 맞을 몸: 발 가운데 (x, y), 반 폭, 높이 (물리 px)
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PetBox {
    pub x: f64,
    pub y: f64,
    pub half_w: f64,
    pub height: f64,
}

impl PetBox {
    pub fn of(pet: &Pet) -> PetBox {
        PetBox { x: pet.body.x, y: pet.body.y, half_w: pet.cfg.half_w, height: pet.cfg.size * PET_HEIGHT }
    }
}

/// 떨어지는 똥 하나 (가운데, 물리 px)
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Poop {
    pub id: u64,
    pub x: f64,
    pub y: f64,
    pub r: f64,
    /// 떨어지는 속도 (물리 px/s)
    pub vy: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Phase {
    /// 시작 전 (시작 카드)
    Ready,
    Play,
    /// 끝 (결과 카드)
    Over,
}

/// 끝난 판
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Over {
    pub score: u32,
    /// 맞았다 (아니면 판 밖으로 나갔다)
    pub hit: bool,
}

pub struct Dodge {
    area: Rect,
    dpi: f64,
    phase: Phase,
    /// 이번 판이 시작된 뒤 (초)
    t: f64,
    spawn_in: f64,
    poops: Vec<Poop>,
    dodged: u32,
    next_id: u64,
    rng: fastrand::Rng,
}

impl Dodge {
    /// area = 판 (작업 영역, 물리 px), dpi = 그 모니터 배율
    pub fn new(area: Rect, dpi: f64, seed: u64) -> Dodge {
        Dodge { area, dpi: dpi.max(0.1), phase: Phase::Ready, t: 0.0, spawn_in: GRACE_SECS, poops: Vec::new(), dodged: 0, next_id: 0, rng: fastrand::Rng::with_seed(seed) }
    }

    pub fn area(&self) -> Rect {
        self.area
    }

    pub fn phase(&self) -> Phase {
        self.phase
    }

    pub fn poops(&self) -> &[Poop] {
        &self.poops
    }

    /// 이번 판에 피한 똥
    pub fn dodged(&self) -> u32 {
        self.dodged
    }

    /// 새 판 (시작 전·끝난 뒤에만)
    pub fn start(&mut self) -> bool {
        if self.phase == Phase::Play {
            return false;
        }
        self.phase = Phase::Play;
        self.t = 0.0;
        self.spawn_in = GRACE_SECS;
        self.poops.clear();
        self.dodged = 0;
        true
    }

    /// 지금 떨어지는 속도 (물리 px/s)
    fn fall_speed(&self) -> f64 {
        (FALL_START + FALL_GAIN * self.t).min(FALL_MAX) * self.dpi
    }

    /// 지금 떨어지는 간격 (초) — 넓은 판일수록 자주
    fn spawn_every(&self) -> f64 {
        let base = (SPAWN_START - SPAWN_GAIN * self.t).max(SPAWN_MIN);
        let width = self.area.width() / self.dpi;
        base * (SPAWN_WIDTH / width.max(1.0)).clamp(0.5, 2.0)
    }

    /// dt초 지났고 동물은 pet (판 밖이면 None). 이번 틱에 끝났으면 Some
    pub fn tick(&mut self, dt: f64, pet: Option<PetBox>) -> Option<Over> {
        if self.phase != Phase::Play || !dt.is_finite() || dt <= 0.0 {
            return None;
        }
        let Some(pet) = pet.filter(|p| p.x >= self.area.left && p.x < self.area.right) else {
            return Some(self.finish(false));
        };
        self.t += dt;
        // 떨어뜨리기
        self.spawn_in -= dt;
        while self.spawn_in <= 0.0 {
            self.spawn_in += self.spawn_every();
            self.spawn(&pet);
        }
        // 떨어지기 — 이번 틱에 지나간 길(앞 자리~지금 자리) 전체로 맞음을 본다 (빠르거나 틱이 길어도 뚫고 지나가지 않게)
        let floor = self.area.bottom;
        let mut hit = false;
        for p in &mut self.poops {
            let from = p.y;
            p.y += p.vy * dt;
            hit |= touches(p, from, &pet);
        }
        if hit {
            return Some(self.finish(true));
        }
        let before = self.poops.len();
        self.poops.retain(|p| p.y - p.r < floor);
        self.dodged = (self.dodged + (before - self.poops.len()) as u32).min(MAX_SCORE);
        None
    }

    fn spawn(&mut self, pet: &PetBox) {
        if self.poops.len() >= MAX_POOPS {
            return;
        }
        let r = POOP_R * self.dpi;
        let (lo, hi) = (self.area.left + r, (self.area.right - r).max(self.area.left + r));
        let x = if self.rng.f64() < AIM_CHANCE {
            pet.x + (self.rng.f64() * 2.0 - 1.0) * AIM_SPREAD * pet.half_w
        } else {
            lo + self.rng.f64() * (hi - lo)
        };
        let vy = self.fall_speed() * (1.0 + (self.rng.f64() * 2.0 - 1.0) * FALL_JITTER);
        self.poops.push(Poop { id: self.next_id, x: x.clamp(lo, hi), y: self.area.top - r, r, vy });
        self.next_id += 1;
    }

    fn finish(&mut self, hit: bool) -> Over {
        self.phase = Phase::Over;
        Over { score: self.dodged, hit }
    }
}

/// 똥이 from에서 p.y까지 떨어지는 동안 동물 몸에 닿았다
fn touches(p: &Poop, from: f64, pet: &PetBox) -> bool {
    let r = p.r * HIT_POOP;
    let near_x = (p.x - pet.x).abs() < pet.half_w * HIT_PET + r;
    let (top, bottom) = (pet.y - pet.height, pet.y);
    near_x && from - r < bottom && p.y + r > top
}

/// 친구에게 보낼 최고 기록 부탁
pub fn score_ask(best: u32) -> String {
    format!("{ASK_PREFIX}{}", best.min(MAX_SCORE))
}

/// 친구가 보낸 최고 기록 (아니면 None — 다른 부탁)
pub fn parse_score_ask(ask: &str) -> Option<u32> {
    let digits = ask.strip_prefix(ASK_PREFIX)?;
    if digits.is_empty() || digits.len() > 7 || !digits.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    digits.parse::<u32>().ok().map(|n| n.min(MAX_SCORE))
}

/// 랭킹 한 줄
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct Rank {
    pub name: String,
    pub score: u32,
    /// 나
    pub me: bool,
}

/// 나 + 기록이 있는 친구들, 높은 점수부터 (같으면 나 먼저, 그다음 이름 순)
pub fn ranking(my_name: &str, my_best: u32, friends: &[(String, u32)]) -> Vec<Rank> {
    let mut out = vec![Rank { name: my_name.to_string(), score: my_best, me: true }];
    out.extend(friends.iter().map(|(n, s)| Rank { name: n.clone(), score: *s, me: false }));
    out.sort_by(|a, b| b.score.cmp(&a.score).then(b.me.cmp(&a.me)).then(a.name.cmp(&b.name)));
    out
}

#[cfg(test)]
#[path = "dodge_tests.rs"]
mod tests;
