//! 다른 동물들도 더 놀기 (v0.6.3)의 짝·줄 맞추기 — social.rs의 자식 모듈 (Social의 비공개 필드에 접근). 숫자는 play.rs.
//!
//! - 🦆 오리 기차: 기차를 부르고 싶은 오리(take_train_wish)가 바닥에서 걸으면 같은 바닥 TRAIN_GATHER_BODIES 안의 한가한 친구를
//!   가까운 순으로 TRAIN_MAX_FOLLOWERS까지 모은다 (나는 동물·자는 동물·들린 동물·볼일 있는 동물·머리 위 탑은 빼고).
//!   틱마다 오리가 지나간 길(빵 부스러기)을 적고, k번째 친구는 그 길을 따라 앞 친구들 몸 간격(TRAIN_SPACING)만큼 뒤 자리로 간다 —
//!   오리가 돌아서면 친구들도 그 자리에 와서 돈다. 오리가 멈추면(시간이 다 됐거나 잡혔다) 다 같이 폴짝 하고 흩어진다.
//! - 🙌 하이파이브: 마주 오다 만난 짝(head_on)이 HIGH_FIVE_CHANCE로 — 손이 닿을 만큼 다가가 마주 보고, 둘 다 준비되면 같이 폴짝 짝!
//!   그 짝은 HIGH_FIVE_COOLDOWN 동안 다시 하지 않는다 (밟고 넘기·인사도 잠깐 쉰다).

use super::*;
use crate::play::{self};

/// 기차 하나
#[derive(Debug, Clone)]
pub(super) struct Train {
    leader: u64,
    /// 따라오는 친구들 (앞에서부터)
    cars: Vec<u64>,
    /// 오리가 지나간 길: (누적 거리, x) — 처음 점은 오리가 출발한 자리
    path: Vec<(f64, f64)>,
    /// 처음 볼 쪽 (출발 자리보다 뒤는 이 쪽의 반대로 이어진 길로 본다)
    start_facing: f64,
}

/// 놀이 코디네이터의 기억
pub(super) struct SocialPlay {
    trains: Vec<Train>,
    /// 하이파이브 중인 짝 (아직 뛰기 전)
    fives: Vec<(u64, u64)>,
    five_cooldown: HashMap<(u64, u64), f64>,
    rng: fastrand::Rng,
    /// 하이파이브 확률 (테스트에서 바꾼다)
    pub(super) five_chance: f64,
}

impl SocialPlay {
    pub(super) fn new(seed: u64) -> Self {
        SocialPlay { trains: Vec::new(), fives: Vec::new(), five_cooldown: HashMap::new(), rng: fastrand::Rng::with_seed(seed ^ 0x6869_6635), five_chance: play::HIGH_FIVE_CHANCE }
    }
}

/// 길 위 누적 거리 d의 x와 그 자리에서 가는 쪽. d가 출발점보다 뒤면 출발 자리에서 처음 볼 쪽의 반대로 이어진 곧은 길
fn along(path: &[(f64, f64)], start_facing: f64, d: f64) -> (f64, f64) {
    let (d0, x0) = path[0];
    if d <= d0 {
        return (x0 - start_facing * (d0 - d), start_facing);
    }
    for w in path.windows(2) {
        let ((da, xa), (db, xb)) = (w[0], w[1]);
        if d <= db {
            let u = if db > da { (d - da) / (db - da) } else { 1.0 };
            return (xa + (xb - xa) * u, (xb - xa).signum());
        }
    }
    let n = path.len();
    let dir = if n >= 2 { (path[n - 1].1 - path[n - 2].1).signum() } else { start_facing };
    (path[n - 1].1, dir)
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

    /// 🙌 준비된 짝은 같이 뛰고, 짝이 사라진 쪽은 풀어 준다
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

    /// 🦆 기차를 부르는 오리가 있으면 친구를 모아 출발하고, 달리는 기차의 줄을 맞춘다
    pub(super) fn step_play<T: AsRef<Pet> + AsMut<Pet>>(&mut self, pets: &mut [T]) {
        self.step_fives(pets);
        self.step_trains(pets);
        for i in 0..self.snap.len() {
            if self.snap[i].engaged || !pets[i].as_mut().take_train_wish() {
                continue;
            }
            self.start_train(pets, i);
        }
    }

    fn start_train<T: AsRef<Pet> + AsMut<Pet>>(&mut self, pets: &mut [T], i: usize) {
        let lead = self.snap[i];
        let Some(owner) = lead.owner else { return };
        let mut cand: Vec<(f64, usize)> = (0..self.snap.len())
            .filter(|&j| j != i)
            .filter(|&j| {
                let s = self.snap[j];
                let reach = play::TRAIN_GATHER_BODIES * (lead.size + s.size) / 2.0;
                !s.engaged
                    && s.free
                    && !s.flying
                    && !s.asleep
                    && !s.held
                    && !s.carrying
                    && !s.riding
                    && s.owner == Some(owner)
                    && (s.y - lead.y).abs() <= SAME_LEVEL_PX
                    && (s.x - lead.x).abs() <= reach
                    && lead.span.is_none_or_contains(s.x)
                    && pets[j].as_ref().can_follow()
            })
            .map(|j| ((self.snap[j].x - lead.x).abs(), j))
            .collect();
        if cand.is_empty() {
            return;
        }
        cand.sort_by(|a, b| a.0.total_cmp(&b.0));
        cand.truncate(play::TRAIN_MAX_FOLLOWERS);
        let secs = behavior::range(&mut self.play.rng, play::TRAIN_SECS);
        pets[i].as_mut().start_lead(secs);
        self.snap[i].engaged = true;
        let mut cars = Vec::new();
        for &(_, j) in &cand {
            pets[j].as_mut().start_follow();
            self.snap[j].engaged = true;
            cars.push(self.snap[j].key);
        }
        self.play.trains.push(Train { leader: lead.key, cars, path: vec![(0.0, lead.x)], start_facing: lead.facing });
    }

    fn step_trains<T: AsRef<Pet> + AsMut<Pet>>(&mut self, pets: &mut [T]) {
        let mut k = 0;
        while k < self.play.trains.len() {
            let leader = self.play.trains[k].leader;
            let Some(il) = self.index(leader).filter(|&il| pets[il].as_ref().is_leading()) else {
                // 오리가 멈췄다 (시간이 다 됐거나 잡혔다): 다 같이 폴짝
                let train = self.play.trains.swap_remove(k);
                for key in train.cars {
                    if let Some(j) = self.index(key) {
                        pets[j].as_mut().end_train();
                        self.snap[j].engaged = true;
                    }
                }
                continue;
            };
            self.snap[il].engaged = true;
            // 따라오다 빠진 친구 (잡혔거나 볼일이 생겼다)
            let snap = &self.snap;
            let alive = |key: &u64| snap.iter().position(|s| s.key == *key).is_some_and(|j| pets[j].as_ref().is_following());
            self.play.trains[k].cars.retain(alive);
            if self.play.trains[k].cars.is_empty() {
                pets[il].as_mut().end_train();
                self.play.trains.swap_remove(k);
                continue;
            }
            let lx = self.snap[il].x;
            let train = &mut self.play.trains[k];
            let &(d, x) = train.path.last().expect("path starts with the leader");
            if (lx - x).abs() > 0.5 {
                train.path.push((d + (lx - x).abs(), lx));
            }
            let total = train.path.last().map_or(0.0, |p| p.0);
            let mut sizes = vec![self.snap[il].size];
            let idx: Vec<usize> = train.cars.iter().filter_map(|key| self.snap.iter().position(|s| s.key == *key)).collect();
            sizes.extend(idx.iter().map(|&j| self.snap[j].size));
            for (slot, &j) in idx.iter().enumerate() {
                let behind = play::train_offset(&sizes, slot + 1);
                let (x, dir) = along(&train.path, train.start_facing, total - behind);
                pets[j].as_mut().set_follow_target(x, dir);
                self.snap[j].engaged = true;
            }
            k += 1;
        }
    }

    /// 테스트: 하이파이브 확률을 바꾼다
    #[cfg(test)]
    pub(crate) fn set_high_five_chance(&mut self, p: f64) {
        self.play.five_chance = p;
    }

    /// 테스트: 달리는 기차 수
    #[cfg(test)]
    pub(crate) fn trains(&self) -> usize {
        self.play.trains.len()
    }
}

trait SpanHas {
    fn is_none_or_contains(&self, x: f64) -> bool;
}

impl SpanHas for Option<(f64, f64)> {
    fn is_none_or_contains(&self, x: f64) -> bool {
        self.map_or(true, |(lo, hi)| (lo..=hi).contains(&x))
    }
}

#[cfg(test)]
mod tests {
    use super::along;

    #[test]
    fn along_the_path_and_behind_the_start() {
        let path = [(0.0, 100.0), (50.0, 150.0), (80.0, 120.0)];
        assert_eq!(along(&path, 1.0, -20.0), (80.0, 1.0));
        assert_eq!(along(&path, 1.0, 25.0), (125.0, 1.0));
        assert_eq!(along(&path, 1.0, 60.0), (140.0, -1.0));
        assert_eq!(along(&path, 1.0, 999.0), (120.0, -1.0));
    }
}
