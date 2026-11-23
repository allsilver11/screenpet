//! 🧳 포트키 (pet.json `portkey` — 해리·헤르미온느·볼드모트, PetConfig::portkey) — brain.rs의 자식 모듈. 숫자·짝 열쇠는 portkey.rs.
//!
//! - 앱이 매 틱 놓인 물건 자리를 알려 줌 (notice_portkeys — 꺼낸 물건·똥·거대한 것·쓰레기·📌 장난감 모두, 모두 숨겨 둔 동안엔 빈 목록)
//! - 한가할 때 (하우스·집중·자리 비움·조작 모드·창 안·볼일이 아닐 때) 가끔 같은 디딤판의 물건으로 "Portkey!" 하고 달려감 (State::Portkey — Run).
//!   짝 물건 (같은 열쇠의 다른 물건)이 내 화면에 있거나 새 버전 친구가 연결돼 있을 때만 (set_portkey_friend — 앱이 내 펫에게만)
//! - 손을 대고 (cast 그림, TOUCH_SECS) 빙글 돌며 작아져 사라짐 (Vanish — render.tuck·twirl, 반짝). 물건은 늘 제자리
//! - 사라지면: 내 화면의 짝 옆에 빙글 커지며 나타남 (Appear, "whoa!") · 친구 쪽이면 앱에 물어봄 (take_portkey → AskFriend — 펫은 State::Away).
//!   친구가 거절하거나 늦으면 앱이 portkey_return → 내 화면의 짝으로, 그것도 없으면 제자리에 다시 나타나 "huh?"
//! - 친구 화면에서 온 손님은 앱이 그 화면의 같은 물건 옆에 나타나게 함 (portkey_arrive)
//! - 같은 포트키에 JOIN_REACH 안의 한가한 마법사는 같이 잡고 같은 곳에 나타남 (sync_portkeys — sync_lifts가 부름, 손님도)
//!
//! 주사위는 자기 주사위(PortkeyMood.rng)로 굴림 — 평소 동작과 다른 동물의 주사위 순서는 그대로.

use super::*;
use crate::juggle::ObjectSpot;
use crate::portkey::{self as pk, PortkeyKey, PortkeySpot};

/// 🧳 포트키 단계
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PortkeyPhase {
    /// 물건 (id, 설 자리 x)으로 달려감
    Run { id: u64, x: f64 },
    /// 손을 대고 빙글 돌며 작아져 사라짐
    Vanish { id: u64 },
    /// 짝 물건 옆 (또는 제자리)에 빙글 커지며 나타남 — huh = 텔레포트 안 됨
    Appear { huh: bool },
}

/// 🧳 사라진 뒤 나타날 곳
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PortkeyDest {
    /// 내 화면의 짝 물건 (id)
    Local(u64),
    /// 친구 화면 (앱이 물어봄)
    Friend,
    /// 갈 곳이 없음 — 제자리
    Stay,
}

/// 🧳 앱에 알릴 일 (take_portkey로 한 번 꺼냄)
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PortkeyEvent {
    /// 친구 화면에 같은 물건 (key)이 있는지 물어봐 줘 — 펫은 이미 사라졌음 (State::Away)
    AskFriend { key: PortkeyKey },
}

/// 🧳 포트키의 기억
#[derive(Debug, Clone)]
pub struct PortkeyMood {
    rng: fastrand::Rng,
    /// 이 시각(clock)부터 포트키를 찾을 수 있음
    pub(super) ready_at: f64,
    /// 이 화면에 놓인 물건들 (앱이 매 틱)
    spots: Vec<PortkeySpot>,
    /// 새 버전 친구가 연결돼 있어 친구 쪽 짝을 물어볼 수 있음 (앱이 내 펫에게만)
    friend_ok: bool,
    /// 잡은 포트키 열쇠 · 나타날 곳 · 같이 잡은 차례 (0 = 처음 잡은 마법사)
    key: Option<PortkeyKey>,
    dest: PortkeyDest,
    nth: usize,
    /// 사라진 자리 (디딤판, x — 제자리에 다시 나타날 때)
    origin: Option<(Owner, f64)>,
    /// 앱에 알릴 일
    event: Option<PortkeyEvent>,
}

impl PortkeyMood {
    /// 시드로 난수를 따로 둠. 첫 포트키는 PORTKEY_FIRST_SECS 뒤부터
    pub(super) fn new(seed: u64) -> Self {
        PortkeyMood {
            rng: fastrand::Rng::with_seed(seed ^ 0x706f_7274),
            ready_at: pk::PORTKEY_FIRST_SECS,
            spots: Vec::new(),
            friend_ok: false,
            key: None,
            dest: PortkeyDest::Stay,
            nth: 0,
            origin: None,
            event: None,
        }
    }
}

impl Pet {
    /// 🧳 이 화면에 놓인 물건들 (앱이 매 틱 — 바닥·창 위에 가만히 놓인 것만)
    pub fn notice_portkeys(&mut self, spots: &[PortkeySpot]) {
        self.portkey.spots.clear();
        self.portkey.spots.extend_from_slice(spots);
    }

    /// 🧳 새 버전 친구가 연결돼 있어 친구 화면의 짝을 물어볼 수 있음 (앱이 내 펫에게만 — 손님은 늘 false)
    pub fn set_portkey_friend(&mut self, ok: bool) {
        self.portkey.friend_ok = ok;
    }

    /// 🧳 앱에 알릴 일 (한 번)
    pub fn take_portkey(&mut self) -> Option<PortkeyEvent> {
        self.portkey.event.take()
    }

    /// 🧳 포트키 중 (달려가기·사라지기·나타나기)
    pub fn portkeying(&self) -> bool {
        matches!(self.state, State::Portkey { .. })
    }

    /// 🧳 포트키를 찾을 수 있게 한가함 (쉬는 시간은 따로)
    fn portkey_free(&self) -> bool {
        self.cfg.portkey && self.wizard_free() && !self.is_controlled() && self.standing.is_some()
    }

    /// 🧳 짝이 있는 물건 (내 화면에 같은 물건이 또 있거나 친구 쪽을 물어볼 수 있음)
    fn portkey_twin_ok(&self, s: &PortkeySpot) -> bool {
        self.portkey.friend_ok || pk::has_twin(&self.portkey.spots, s.key, s.spot.id)
    }

    /// 🧳 달려갈 수 있는 물건과 설 자리 (같은 디딤판 PORTKEY_REACH 안, 짝이 있는 것만)
    fn portkey_choices(&self, world: &World) -> Vec<(PortkeySpot, f64)> {
        let reach = pk::PORTKEY_REACH * self.cfg.size;
        let Some(st) = self.standing else { return Vec::new() };
        self.portkey
            .spots
            .iter()
            .filter(|s| s.spot.owner == st.owner && (s.spot.x - self.body.x).abs() <= reach && self.portkey_twin_ok(s))
            .filter_map(|s| self.approach_x(&s.spot, world).map(|x| (*s, x)))
            .collect()
    }

    /// 🧳 포트키를 찾아 달려가기 시작 (주사위 없이 — update_portkey가 굴림). 시작했으면 true
    pub(super) fn start_portkey(&mut self, world: &World) -> bool {
        if !self.portkey_free() || self.clock < self.portkey.ready_at {
            return false;
        }
        let choices = self.portkey_choices(world);
        if choices.is_empty() {
            return false;
        }
        let (s, x) = choices[self.portkey.rng.usize(..choices.len())];
        self.portkey.ready_at = self.clock + behavior::range(&mut self.portkey.rng, pk::PORTKEY_CALM_SECS);
        self.pacing.after_beat = None;
        self.clear_hover();
        self.speech = Some(Speech::Portkey);
        self.state = State::Portkey { phase: PortkeyPhase::Run { id: s.spot.id, x }, t: 0.0 };
        true
    }

    /// 🧳 나타날 곳을 정함: 내 화면의 짝 (여럿이면 아무거나) · 친구 쪽 (짝이 없거나 FRIEND_FIRST) · 없으면 제자리
    fn portkey_dest(&mut self, key: PortkeyKey, id: u64) -> PortkeyDest {
        let twins: Vec<u64> = self.portkey.spots.iter().filter(|s| s.key == key && s.spot.id != id).map(|s| s.spot.id).collect();
        if self.portkey.friend_ok && (twins.is_empty() || self.portkey.rng.f64() < pk::FRIEND_FIRST) {
            return PortkeyDest::Friend;
        }
        match twins.len() {
            0 => PortkeyDest::Stay,
            n => PortkeyDest::Local(twins[self.portkey.rng.usize(..n)]),
        }
    }

    /// 🧳 물건 (id, key)을 잡고 빙글 사라지기 시작 (dest = 나타날 곳, nth = 같이 잡은 차례)
    fn grab_portkey(&mut self, id: u64, key: PortkeyKey, dest: PortkeyDest, nth: usize) {
        self.portkey.key = Some(key);
        self.portkey.dest = dest;
        self.portkey.nth = nth;
        self.portkey.origin = self.standing.map(|st| (st.owner, self.body.x));
        if let Some(s) = self.portkey.spots.iter().find(|s| s.spot.id == id) {
            if (s.spot.x - self.body.x).abs() > 1.0 {
                self.facing = (s.spot.x - self.body.x).signum();
            }
        }
        self.state = State::Portkey { phase: PortkeyPhase::Vanish { id }, t: 0.0 };
    }

    /// 🧳 같이 잡음 (sync_portkeys가 부름): 앞 마법사와 같은 곳으로 — 친구 쪽인데 물어볼 수 없으면 (손님) 내 화면의 짝, 없으면 제자리
    fn join_portkey(&mut self, id: u64, key: PortkeyKey, dest: PortkeyDest, nth: usize) {
        let dest = match dest {
            PortkeyDest::Friend if !self.portkey.friend_ok => self.portkey_dest(key, id),
            d => d,
        };
        self.pacing.after_beat = None;
        self.clear_hover();
        self.speech = Some(Speech::Portkey);
        self.portkey.ready_at = self.clock + behavior::range(&mut self.portkey.rng, pk::PORTKEY_CALM_SECS);
        self.grab_portkey(id, key, dest, nth);
    }

    /// 🧳 포트키 한 틱: 달려가기 → 손 대고 사라지기 → (짝 옆에) 나타나기. 아니면 한가할 때 가끔 시작. 처리했으면 true
    pub(super) fn update_portkey(&mut self, dt: f64, world: &World) -> bool {
        let State::Portkey { phase, t } = self.state else {
            if self.cfg.portkey && self.portkey.rng.f64() < pk::PORTKEY_RATE * self.power() * dt {
                return self.start_portkey(world);
            }
            return false;
        };
        let t = t + dt;
        match phase {
            PortkeyPhase::Run { id, x } => {
                let Some(s) = self.portkey.spots.iter().find(|s| s.spot.id == id).copied() else {
                    // 물건이 치워졌거나 들렸음
                    self.state = self.rest_beat();
                    return true;
                };
                if self.romp_interrupted() {
                    self.state = self.rest_beat();
                    return true;
                }
                let dx = x - self.body.x;
                let step = self.cfg.walk_speed * Move::Run.speed_mult() * dt;
                if dx.abs() > step {
                    self.facing = dx.signum();
                    if self.juggle_walk(world, self.facing, step).is_none() {
                        return true;
                    }
                    self.state = State::Portkey { phase, t };
                } else {
                    if self.juggle_walk(world, dx.signum(), dx.abs()).is_none() {
                        return true;
                    }
                    let dest = self.portkey_dest(s.key, id);
                    self.grab_portkey(id, s.key, dest, 0);
                }
            }
            PortkeyPhase::Vanish { id } => {
                if !self.follow_anchor(world) {
                    self.start_fall();
                    return true;
                }
                if t < pk::TOUCH_SECS + pk::VANISH_SECS {
                    self.state = State::Portkey { phase, t };
                    return true;
                }
                self.portkey_gone(id, world);
            }
            PortkeyPhase::Appear { .. } => {
                if !self.follow_anchor(world) {
                    self.start_fall();
                    return true;
                }
                self.state = if t < pk::APPEAR_SECS { State::Portkey { phase, t } } else { self.rest_beat() };
            }
        }
        true
    }

    /// 🧳 다 사라짐: 정한 곳으로 — 내 화면의 짝 옆 (그새 없어졌으면 제자리) · 친구 쪽이면 앱에 물어보고 화면을 떠남
    fn portkey_gone(&mut self, id: u64, world: &World) {
        let key = self.portkey.key.unwrap_or(PortkeyKey { kind: crate::object::ObjectKind::Paper, look: 0 });
        match self.portkey.dest {
            PortkeyDest::Friend => {
                self.portkey.event = Some(PortkeyEvent::AskFriend { key });
                self.standing = None;
                self.inside = None;
                self.body.vx = 0.0;
                self.body.vy = 0.0;
                self.pacing.after_beat = None;
                self.clear_hover();
                self.state = State::Away;
            }
            PortkeyDest::Local(twin) => match self.portkey.spots.iter().find(|s| s.spot.id == twin && s.spot.id != id).copied() {
                Some(s) => self.appear_next_to(&s.spot, world, false),
                None => self.appear_in_place(world),
            },
            PortkeyDest::Stay => self.appear_in_place(world),
        }
    }

    /// 🧳 친구가 돌려보냈거나 답이 늦음 (앱이 부름 — 친구 쪽으로 사라졌던 내 펫): 내 화면의 짝 옆, 없으면 제자리에 다시 나타나 "huh?"
    pub fn portkey_return(&mut self, world: &World) {
        let key = self.portkey.key;
        let origin_id = self.portkey.origin.and_then(|(owner, x)| {
            self.portkey.spots.iter().filter(|s| Some(s.key) == key && s.spot.owner == owner).min_by(|a, b| (a.spot.x - x).abs().total_cmp(&(b.spot.x - x).abs())).map(|s| s.spot.id)
        });
        let twins: Vec<PortkeySpot> = self.portkey.spots.iter().filter(|s| Some(s.key) == key && Some(s.spot.id) != origin_id).copied().collect();
        if twins.is_empty() {
            self.appear_in_place(world);
        } else {
            let s = twins[self.portkey.rng.usize(..twins.len())];
            self.appear_next_to(&s.spot, world, false);
        }
    }

    /// 🧳 친구 화면에서 포트키로 온 손님 (앱이 부름 — 이 화면의 같은 물건 spot 옆에 나타남, nth = 같이 온 차례)
    pub fn portkey_arrive(&mut self, spot: &ObjectSpot, world: &World, nth: usize) {
        self.portkey.nth = nth;
        self.appear_next_to(spot, world, false);
    }

    /// 🧳 사라졌던 자리에 다시 나타남 ("huh?" — 디딤판이 없어졌으면 위에서 떨어짐)
    fn appear_in_place(&mut self, world: &World) {
        match self.portkey.origin.and_then(|(owner, x)| world.anchor(owner).map(|a| (owner, x, a))) {
            Some((owner, x, (ax, ay))) => {
                self.body = Body { x, y: ay, vx: 0.0, vy: 0.0 };
                self.standing = Some(Standing { owner, rel_x: x - ax });
                self.begin_appear(true);
            }
            None => {
                self.speech = Some(Speech::PortkeyHuh);
                self.start_fall();
            }
        }
    }

    /// 🧳 물건 spot 옆 (거대한 것은 몸을 붙인 옆, 같이 온 차례만큼 더 옆)에 빙글 커지며 나타남. 디딤판이 없으면 그 위에서 떨어짐
    fn appear_next_to(&mut self, spot: &ObjectSpot, world: &World, huh: bool) {
        let size = self.cfg.size;
        let side = if (spot.id + self.portkey.nth as u64) % 2 == 0 { 1.0 } else { -1.0 };
        let gap = spot.half_w + self.cfg.half_w + 0.15 * size + (self.portkey.nth / 2) as f64 * 0.8 * size;
        let Some((ax, ay)) = world.anchor(spot.owner) else {
            self.standing = None;
            self.body = Body { x: spot.x, y: spot.y - size, vx: 0.0, vy: 0.0 };
            self.start_fall();
            return;
        };
        let x = match world.piece_at(spot.owner, spot.x) {
            Some(p) => {
                let (lo, hi) = (p.x_start + self.cfg.half_w, p.x_end - self.cfg.half_w);
                let want = spot.x + side * gap;
                let other = spot.x - side * gap;
                if (lo..=hi).contains(&want) {
                    want
                } else if (lo..=hi).contains(&other) {
                    other
                } else {
                    spot.x.clamp(lo.min(hi), hi.max(lo))
                }
            }
            None => spot.x,
        };
        self.body = Body { x, y: ay, vx: 0.0, vy: 0.0 };
        self.standing = Some(Standing { owner: spot.owner, rel_x: x - ax });
        if (spot.x - x).abs() > 1.0 {
            self.facing = (spot.x - x).signum();
        }
        self.begin_appear(huh);
    }

    /// 🧳 빙글 커지며 나타나기 시작 ("whoa!" · 텔레포트 안 됐으면 "huh?")
    fn begin_appear(&mut self, huh: bool) {
        self.pacing.after_beat = None;
        self.clear_hover();
        self.speech = Some(if huh { Speech::PortkeyHuh } else { Speech::PortkeyLand });
        self.state = State::Portkey { phase: PortkeyPhase::Appear { huh }, t: 0.0 };
    }

    /// 🧳 사라진 몫 (0 = 그대로 … 1 = 안 보임 — render.tuck): 손 댄 뒤 작아지고, 나타날 땐 커짐
    pub(super) fn portkey_tuck(&self) -> f64 {
        match self.state {
            State::Portkey { phase: PortkeyPhase::Vanish { .. }, t } => ((t - pk::TOUCH_SECS) / pk::VANISH_SECS).clamp(0.0, 1.0),
            State::Portkey { phase: PortkeyPhase::Appear { huh: false }, t } => 1.0 - (t / pk::APPEAR_SECS).clamp(0.0, 1.0),
            _ => 0.0,
        }
    }

    /// 🧳 빙글 도는 각도 (도 — render.twirl, 웹뷰가 팽이처럼 좌우로 돌림): 사라질수록 빨리, 나타날수록 천천히
    pub(super) fn portkey_twirl(&self) -> f64 {
        let k = match self.state {
            State::Portkey { phase: PortkeyPhase::Vanish { .. }, .. } | State::Portkey { phase: PortkeyPhase::Appear { huh: false }, .. } => self.portkey_tuck(),
            _ => return 0.0,
        };
        (360.0 * pk::SPIN_TURNS * k * k) % 360.0
    }

    /// 🧳 반짝이 (사라지고 나타나는 동안 — render.sparkle)
    pub(super) fn portkey_sparkle(&self) -> bool {
        self.portkey_tuck() > 0.0
    }

    /// 🧳 포트키 그림: 달려감 (run) · 손 댐 (cast) · 그 밖엔 idle
    pub(super) fn portkey_anim(&self) -> Anim {
        match self.state {
            State::Portkey { phase: PortkeyPhase::Run { .. }, .. } => Anim::Run,
            State::Portkey { phase: PortkeyPhase::Vanish { .. }, t } if t < pk::TOUCH_SECS => Anim::Cast,
            _ => Anim::Idle,
        }
    }
}

/// 🧳 앱이 매 틱 동물 update 뒤에 (sync_lifts가 부름 — 내 펫·손님 모두): 막 포트키를 잡은 마법사 곁 (JOIN_REACH, 같은 디딤판)의
/// 한가한 마법사도 같이 잡음 (같은 곳에 나타남 — 쉬는 시간이어도)
pub(super) fn sync_portkeys<T: AsRef<Pet> + AsMut<Pet>>(pets: &mut [T]) {
    for i in 0..pets.len() {
        let p = pets[i].as_ref();
        let State::Portkey { phase: PortkeyPhase::Vanish { id }, t } = p.state else { continue };
        if t > pk::TOUCH_SECS || p.portkey.nth != 0 {
            continue;
        }
        let (Some(key), dest) = (p.portkey.key, p.portkey.dest) else { continue };
        let Some(s) = p.portkey.spots.iter().find(|s| s.spot.id == id).map(|s| s.spot) else { continue };
        let mut nth = 1;
        for j in 0..pets.len() {
            let q = pets[j].as_ref();
            let near = q.standing.is_some_and(|st| st.owner == s.owner) && (q.body.x - s.x).abs() <= pk::JOIN_REACH * q.cfg.size + s.half_w;
            if j == i || !near || !q.portkey_free() {
                continue;
            }
            pets[j].as_mut().join_portkey(id, key, dest, nth);
            nth += 1;
        }
    }
}
