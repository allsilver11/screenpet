//! 다른 동물들도 더 놀기 (v0.6.3)의 짝 맞추기 — social.rs의 자식 모듈 (Social의 비공개 필드에 접근). 숫자는 play.rs.
//!
//! - 🙌 하이파이브: 마주 오다 만난 짝(head_on)이 HIGH_FIVE_CHANCE로 — 손이 닿을 만큼 다가가 마주 보고, 둘 다 준비되면 같이 폴짝 짝!
//!   그 짝은 HIGH_FIVE_COOLDOWN 동안 다시 하지 않음 (밟고 넘기·인사도 잠깐 쉼).

use super::*;
use crate::play::{self};

/// 놀이 코디네이터의 기억
pub(super) struct SocialPlay {
    /// 하이파이브 중인 짝 (아직 뛰기 전)
    fives: Vec<(u64, u64)>,
    five_cooldown: HashMap<(u64, u64), f64>,
    rng: fastrand::Rng,
    /// 하이파이브 확률 (테스트에서 바꿈)
    pub(super) five_chance: f64,
}

impl SocialPlay {
    /// seed로 주사위를 따로 둠
    pub(super) fn new(seed: u64) -> Self {
        SocialPlay { fives: Vec::new(), five_cooldown: HashMap::new(), rng: fastrand::Rng::with_seed(seed ^ 0x6869_6635), five_chance: play::HIGH_FIVE_CHANCE }
    }
}

impl Social {
    /// 🙌 마주 오다 만난 짝(i, j)이 하이파이브를 하기로 했으면 시작하고 true
    pub(super) fn try_high_five<T: AsRef<Pet> + AsMut<Pet>>(&mut self, pets: &mut [T], i: usize, j: usize) -> bool {
        let (a, b) = (self.snap[i], self.snap[j]);
        let key = pair(a.key, b.key);
        if !head_on(&a, &b) || cooling(&self.play.five_cooldown, &key, self.clock) {
            return false;
        }
        let now = self.clock;
        if self.play.rng.f64() >= self.play.five_chance {
            // 이번 마주침엔 안 한다 (닿아 있는 동안 틱마다 다시 굴리지 않는다)
            cool(&mut self.play.five_cooldown, key, now, MEET_PASS_COOLDOWN);
            return false;
        }
        let Some((lo, hi)) = a.span else { return false };
        let (left, right) = if a.x <= b.x { (i, j) } else { (j, i) };
        let (l, r) = (self.snap[left], self.snap[right]);
        let mid = (l.x + r.x) / 2.0;
        let gap = play::FIVE_GAP * (l.half_w + r.half_w);
        let (lx, rx) = (mid - gap / 2.0, mid + gap / 2.0);
        if lx < lo + l.half_w || rx > hi - r.half_w {
            return false;
        }
        let speak_left = self.play.rng.bool();
        pets[left].as_mut().start_high_five(lx, 1.0, speak_left);
        pets[right].as_mut().start_high_five(rx, -1.0, !speak_left);
        self.play.fives.push((l.key, r.key));
        let wait = behavior::range(&mut self.play.rng, play::HIGH_FIVE_COOLDOWN);
        cool(&mut self.play.five_cooldown, key, now, wait);
        // 내리자마자 밟고 넘거나 인사하지 않는다
        let calm = behavior::range(&mut self.meet_rng, MEET_COOLDOWN);
        cool(&mut self.meet_cooldown, key, now, calm);
        let greet = behavior::range(&mut self.meet_rng, GREET_COOLDOWN);
        self.cooldown.insert(key, now + greet);
        self.snap[i].engaged = true;
        self.snap[j].engaged = true;
        true
    }

    /// 🙌 준비된 짝은 같이 뛰고, 짝이 사라진 쪽은 풀어 줌
    fn step_fives<T: AsRef<Pet> + AsMut<Pet>>(&mut self, pets: &mut [T]) {
        let mut k = 0;
        while k < self.play.fives.len() {
            let (a, b) = self.play.fives[k];
            let (ia, ib) = (self.index(a), self.index(b));
            let live = |i: Option<usize>| i.filter(|&i| pets[i].as_ref().in_high_five());
            match (live(ia), live(ib)) {
                (Some(ia), Some(ib)) => {
                    self.snap[ia].engaged = true;
                    self.snap[ib].engaged = true;
                    if pets[ia].as_ref().high_five_ready() && pets[ib].as_ref().high_five_ready() {
                        pets[ia].as_mut().jump_high_five();
                        pets[ib].as_mut().jump_high_five();
                        self.play.fives.swap_remove(k);
                    } else {
                        k += 1;
                    }
                }
                (x, y) => {
                    for i in [x, y].into_iter().flatten() {
                        pets[i].as_mut().drop_high_five();
                    }
                    self.play.fives.swap_remove(k);
                }
            }
        }
    }

    /// 🙌 하이파이브 짝을 맞춤 (social.rs가 틱마다)
    pub(super) fn step_play<T: AsRef<Pet> + AsMut<Pet>>(&mut self, pets: &mut [T]) {
        self.step_fives(pets);
    }

    /// 테스트: 하이파이브 확률을 바꿈
    #[cfg(test)]
    pub(crate) fn set_high_five_chance(&mut self, p: f64) {
        self.play.five_chance = p;
    }
}
