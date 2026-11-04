//! 탑 쌓기 코디네이터 (social.rs의 자식 모듈) — 움직임은 brain_tower.rs.
//!
//! - 올라가는 길: 마주 오다 만났을 때 (MeetKind::Climb), 걷거나 달리다 탑 밑동으로 들어왔을 때 (TOWER_CLIMB_CHANCE), 위에서 살며시 떨어졌을 때 (HIT_MIN_SPEED 아래 — 그 이상은 collide의 부딪힘이라 둘이 함께 일어나지 않는다).
//! - 언제나 맨 위 동물 위에만 선다. 나는 동물은 끼지 않는다. 최대 MAX_TOWER층, 탑 꼭대기가 작업 영역 위로 나가면 안 된다 (실제 몸 크기로 잰다).
//! - rides는 언제나 아래에서 위 순서다 (태운 쪽의 짝이 먼저 들어간다): 한 번 훑으면 무너짐이 위까지 번지고, carry가 한 틱 늦음 없이 탑 전체를 옮긴다.
//! - 어느 층이 빠지면 (잡힘·던짐·데굴·떠남·집·창 닫힘·크기 바뀜·내려옴) 그 위가 전부 평소처럼 떨어지고, 그 아래는 그대로다.
//! - 손님도 이 화면 주인의 코디네이터가 움직인다 — 통신할 것이 없다.

use super::*;

/// 탑 최대 높이 (층, 맨 아래 포함)
pub const MAX_TOWER: usize = 5;
/// 걷거나 달리다 탑 밑동으로 들어온 동물이 올라갈 확률 (한 번 마주칠 때마다 한 번, 안 하면 MEET_PASS_COOLDOWN 뒤에 다시)
pub const TOWER_CLIMB_CHANCE: f64 = 0.75;
/// 놓자마자 발이 이만큼(몸 크기 배) 머리 속에 있어도 머리에 내려놓은 것으로 친다
const HEAD_DROP_ZONE: f64 = 0.25;

/// 머리 위에 서 있는 짝. 크기가 바뀌면 끝난다
#[derive(Debug, Clone, Copy)]
pub(super) struct Ride {
    rider: u64,
    carrier: u64,
    rider_size: f64,
    carrier_size: f64,
}

fn position<T: AsRef<Pet>>(pets: &[T], key: u64) -> Option<usize> {
    pets.iter().position(|p| p.as_ref().social_key() == key)
}

impl Social {
    /// 탑을 살핀다 (매 틱 맨 먼저): 빠진 층 위를 떨어뜨린다. 아래에서 위로 한 번 훑으면 위까지 번진다
    pub(super) fn step_rides<T: AsRef<Pet> + AsMut<Pet>>(&mut self, pets: &mut [T]) {
        let mut i = 0;
        while i < self.rides.len() {
            let ride = self.rides[i];
            let (ir, ic) = (position(pets, ride.rider), position(pets, ride.carrier));
            let holds = match (ir, ic) {
                (Some(ir), Some(ic)) => {
                    let (r, c) = (pets[ir].as_ref(), pets[ic].as_ref());
                    r.is_on_head() && c.can_carry() && r.cfg.size == ride.rider_size && c.cfg.size == ride.carrier_size
                }
                _ => false,
            };
            if holds {
                i += 1;
                continue;
            }
            self.rides.remove(i);
            if let Some(ir) = ir {
                pets[ir].as_mut().fall_off();
            }
            if let Some(ic) = ic {
                pets[ic].as_mut().set_has_rider(false);
            }
            // 막 내려온 짝은 서로 부딪히거나 바로 다시 올라가지 않는다
            let (key, now) = (pair(ride.rider, ride.carrier), self.clock);
            self.hit_cooldown.retain(|_, until| *until > now);
            self.hit_cooldown.insert(key, now + HIT_PAIR_COOLDOWN);
            self.meet_cooldown.retain(|_, until| *until > now);
            self.meet_cooldown.insert(key, now + MEET_PASS_COOLDOWN);
        }
        for ride in &self.rides {
            if let Some(ir) = position(pets, ride.rider) {
                pets[ir].as_mut().keep_riding();
            }
        }
    }

    /// 동물 update 뒤에 부른다: 탑을 아래에서 위로 옮긴다 (창을 옮기기 전 — 층마다 한 틱씩 밀리지 않는다)
    pub fn carry<T: AsRef<Pet> + AsMut<Pet>>(&mut self, pets: &mut [T]) {
        for ride in &self.rides {
            let (Some(ir), Some(ic)) = (position(pets, ride.rider), position(pets, ride.carrier)) else { continue };
            if pets[ir].as_ref().is_on_head() {
                let (x, y) = pets[ic].as_ref().head_top();
                pets[ir].as_mut().ride_at(x, y);
            }
        }
    }

    /// 이 동물이 선 탑의 맨 위
    pub(super) fn top_of(&self, key: u64) -> u64 {
        let mut top = key;
        while let Some(r) = self.rides.iter().find(|r| r.carrier == top) {
            top = r.rider;
        }
        top
    }

    /// 이 동물 아래까지 포함한 층 수 (자기 포함)
    fn levels(&self, key: u64) -> usize {
        let (mut n, mut at) = (1, key);
        while let Some(r) = self.rides.iter().find(|r| r.rider == at) {
            n += 1;
            at = r.carrier;
        }
        n
    }

    /// 지금 탑 높이 (테스트·디버그용)
    pub fn tower_height(&self, key: u64) -> usize {
        self.levels(self.top_of(key))
    }

    /// host 머리 위에 몸 크기 rider_size가 설 수 있다: 맨 위이고, 층 수와 높이 제한 안
    pub(super) fn can_host<T: AsRef<Pet>>(&self, pets: &[T], host: usize, rider_size: f64) -> bool {
        let p = pets[host].as_ref();
        p.can_carry() && !p.has_rider() && self.levels(p.social_key()) < MAX_TOWER && p.head_top().1 - rider_size >= p.ceiling()
    }

    /// 올라섰다
    fn board<T: AsRef<Pet> + AsMut<Pet>>(&mut self, pets: &mut [T], rider: usize, host: usize) {
        pets[host].as_mut().feel_rider();
        pets[rider].as_mut().begin_ride();
        let head = pets[host].as_ref().head_top();
        pets[rider].as_mut().ride_at(head.0, head.1);
        let (r, c) = (pets[rider].as_ref(), pets[host].as_ref());
        self.rides.push(Ride { rider: r.social_key(), carrier: c.social_key(), rider_size: r.cfg.size, carrier_size: c.cfg.size });
        for k in [rider, host] {
            if k < self.snap.len() {
                self.snap[k] = Snap { engaged: true, ..Snap::of(pets[k].as_ref()) };
            }
        }
    }

    /// 머리로 뛰어오르는 길을 맨 위 머리에 맞추고, 닿으면 올라서거나 (자리가 없어졌으면) 그대로 떨어진다
    pub(super) fn step_climbs<T: AsRef<Pet> + AsMut<Pet>>(&mut self, pets: &mut [T]) {
        let mut i = 0;
        while i < self.climbs.len() {
            let (jumper, base) = self.climbs[i];
            let (Some(ij), it) = (self.index(jumper), self.index(self.top_of(base))) else {
                self.climbs.swap_remove(i);
                continue;
            };
            if pets[ij].as_mut().take_head_bump() {
                self.climbs.swap_remove(i);
                let size = pets[ij].as_ref().cfg.size;
                match it.filter(|&it| it != ij && self.can_host(pets, it, size)) {
                    Some(it) => {
                        self.board(pets, ij, it);
                        pets[ij].as_mut().say_on_top();
                    }
                    None => pets[ij].as_mut().fall_off(),
                }
                continue;
            }
            if !pets[ij].as_ref().is_climbing() {
                self.climbs.swap_remove(i);
                continue;
            }
            if let Some(it) = it {
                let head = pets[it].as_ref().head_top();
                pets[ij].as_mut().aim_climb(head);
            }
            i += 1;
        }
    }

    /// 머리로 뛰어오르기 시작한다 (base = 탑 밑동 또는 혼자인 친구). 자리가 없으면 false
    pub(super) fn start_climb<T: AsRef<Pet> + AsMut<Pet>>(&mut self, pets: &mut [T], jumper: usize, base: usize, crouch: f64) -> bool {
        let size = self.snap[jumper].size;
        let Some(top) = self.index(self.top_of(self.snap[base].key)) else { return false };
        if !self.can_host(pets, top, size) {
            return false;
        }
        let head = pets[top].as_ref().head_top();
        let x = pets[jumper].as_ref().body.x;
        pets[jumper].as_mut().start_vault(MeetKind::Climb, Some(head), x, 0.0, crouch);
        self.climbs.push((self.snap[jumper].key, self.snap[base].key));
        true
    }

    /// 걷거나 달리는 동물이 탑 밑동으로 들어왔다: 맨 위로 뛰어오른다 (한 번 마주칠 때 한 번 정한다)
    pub(super) fn tower_bumps<T: AsRef<Pet> + AsMut<Pet>>(&mut self, pets: &mut [T]) {
        if self.rides.is_empty() {
            return;
        }
        let n = self.snap.len();
        for i in 0..n {
            for j in 0..n {
                let (a, b) = (self.snap[i], self.snap[j]);
                let key = pair(a.key, b.key);
                let (dx, body) = (b.x - a.x, (a.size + b.size) / 2.0);
                if i == j
                    || a.engaged
                    || !a.free
                    || !a.walking
                    || !b.carrying
                    || b.riding
                    || a.carrying
                    || !near(&a, &b, MEET_BODIES)
                    || dx.abs() < MEET_MIN_BODIES * body
                    || dx * a.facing <= 0.0
                    || self.meet_cooldown.get(&key).is_some_and(|&until| self.clock < until)
                {
                    continue;
                }
                let now = self.clock;
                self.meet_cooldown.retain(|_, until| *until > now);
                let crouch = if a.run_carry > 0.0 { CROUCH_SECS * RUN_CROUCH } else { CROUCH_SECS };
                let started = self.meet_rng.f64() < self.tower_chance && self.start_climb(pets, i, j, crouch);
                self.meet_cooldown.insert(key, now + if started { behavior::range(&mut self.meet_rng, MEET_COOLDOWN) } else { MEET_PASS_COOLDOWN });
                if started {
                    self.snap[i].engaged = true;
                    let greet = behavior::range(&mut self.meet_rng, GREET_COOLDOWN);
                    self.cooldown.insert(key, now + greet);
                }
            }
        }
    }

    /// 떨어지던 동물의 발이 맨 위 동물 머리에 살며시 닿았으면 올라선다 (자는 친구는 그대로 잔다).
    /// 그냥 놓은 동물은 높은 데서 떨어져 빨라졌어도 올라선다. 던진 것이 HIT_MIN_SPEED 이상이면 여기서 보지 않는다 — thrown_hits가 튕겨 낸다. 탑 가운데 층은 맨 위가 아니라서 그냥 지나가거나 부딪힌다.
    /// 나는 동물은 끼지 않는다
    pub(super) fn perches<T: AsRef<Pet> + AsMut<Pet>>(&mut self, dt: f64, pets: &mut [T]) {
        for i in 0..self.snap.len() {
            let a = self.snap[i];
            if !a.perching {
                continue;
            }
            let Body { vx, vy, .. } = pets[i].as_ref().body;
            let dpi = pets[i].as_ref().cfg.dpi;
            if vy < 0.0 || (!pets[i].as_ref().is_gentle_drop() && collide::logical_speed(vx, vy, dpi) >= HIT_MIN_SPEED) {
                continue;
            }
            let Some(was) = self.prev.iter().find(|s| s.key == a.key).copied() else { continue };
            if (a.x - was.x).hypot(a.y - was.y) > 2.0 * vx.hypot(vy) * dt + a.size {
                continue;
            }
            for j in 0..self.snap.len() {
                if j == i || !self.can_host(pets, j, a.size) {
                    continue;
                }
                let b = self.snap[j];
                let Some(b_was) = self.prev.iter().find(|s| s.key == b.key) else { continue };
                let head = pets[j].as_ref().head_top();
                let rise = b.y - head.1;
                let rel_prev = (was.x - b_was.x, was.y - b_was.y);
                let rel_now = (a.x - b.x, a.y - b.y);
                let crossed = collide::entry(rel_prev, rel_now, b.half_w, rise, HEAD_RISE * a.size) == Some(collide::Face::Top);
                let set_down = was.held && rel_now.0.abs() < b.half_w && -rise <= rel_now.1 && rel_now.1 <= -rise + HEAD_DROP_ZONE * b.size;
                if crossed || set_down {
                    self.board(pets, i, j);
                    break;
                }
            }
        }
    }

    /// 테스트: 바로 올라선다
    #[cfg(test)]
    pub(crate) fn force_board<T: AsRef<Pet> + AsMut<Pet>>(&mut self, pets: &mut [T], rider: usize, host: usize) {
        self.board(pets, rider, host);
    }
}
