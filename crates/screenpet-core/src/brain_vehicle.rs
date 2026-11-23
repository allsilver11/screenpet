//! 🚗 탈것 타기의 Pet 동작 — brain.rs의 자식 모듈 (비공개 필드에 접근). 탈것 자체와 숫자는 vehicle.rs (Garage).
//!
//! - `notice_vehicles`: 앱이 update 전에 탈것 자리를 넘김. 한가한 펫이 같은 디딤판의 가까운 빈 탈것을 처음 보면
//!   BOARD_CHANCE로 걸어감 (State::ToVehicle). 다 오면 `boarding`이 Some — 차고가 태우거나 (`take_seat`) 그만두게 함 (`cancel_boarding`)
//! - 🛻 트럭 운전 펫이 부르면 (`invite_ride`) 주사위 없이 짐칸으로 걸어감
//! - 타는 동안 (State::Ride): 차고가 틱마다 좌석 자리로 옮김 (`seat_at`). 그림은 운전석은 옆모습 idle, 짐칸은 앞모습 sit_front —
//!   문·짐칸 벽 아래로 가려지는 다리 몫은 render.seat (웹뷰가 그림 아래를 자름). 한동안 안 옮겨 주면 떨어짐
//! - 내림 (`leave_vehicle`) 은 폴짝, 쿵·잡힘 (`fling_off`) 은 휙 튕겨 나감. 걷다 마주친 펫은 `startle` (폴짝 "whoa!"), 쿵 맞으면 `bonk`
//! - 내 펫이 친구가 연결된 끝까지 몰고 가면 (`may_drive_out`) 탈것째 사라짐 (`drove_out` → take_exit)
//! - 사용자가 펫을 탈것 위에 떨구면 차고의 offer_drop이 `just_dropped`를 보고 태움
//! - 🎮 조작 모드: 운전석이면 `steering`으로 차고가 키대로 몲 (←/→ 운전, ↑ 빵빵, 🚁 방향키 상하좌우). Space = 폴짝 내림.
//!   점프해서 빈 탈것 위에 내려앉으면 올라탐 (`may_hop_in` — 막 내린 뒤 REHOP_SECS 동안은 안 탐)
//!
//! 주사위는 자기 주사위(VehicleMood.rng)로 굴림 — 평소 동작과 다른 동물의 주사위 순서는 그대로.

use super::*;
use crate::vehicle::{self, Obstacle, VehicleSpot};

/// 좌석을 이만큼(초) 안 옮겨 주면 탈것이 없어진 것으로 보고 떨어짐
const SEAT_STALE_SECS: f64 = 0.5;
/// 다 왔다고 보는 거리 (물리 px)
const ARRIVE_PX: f64 = 4.0;
/// 내릴 때 폴짝 (크기 128 기준 px/s: 옆으로, 위로)
const HOP_OFF: (f64, f64) = (140.0, 520.0);
/// 마주쳐 놀라 폴짝 (크기 128 기준 px/s: 뒤로, 위로)
const STARTLE_HOP: (f64, f64) = (110.0, 560.0);
/// 🎮 폴짝 내린 뒤 이만큼(초)은 내려앉아도 다시 타지 않음
const REHOP_SECS: f64 = 1.5;

/// 탈것 타기의 기억
#[derive(Debug, Clone)]
pub struct VehicleMood {
    rng: fastrand::Rng,
    /// 이 시각(clock)까지는 새로 타러 가지 않음
    calm_until: f64,
    /// 주사위를 굴린 탈것과 다시 굴릴 수 있는 시각
    seen: Vec<(u64, f64)>,
    /// 걸어갈 자리 (탈것 가운데 — notice_vehicles가 틱마다 고침)
    target_x: f64,
    /// 다 와서 태워 주기를 기다림
    arrived: bool,
    /// 짐칸에 불렸음 (빈 운전석이 아니어도 감)
    invited: bool,
    /// 좌석: 가려지는 높이 (물리 px), 앞모습
    cover: f64,
    front: bool,
    /// 마지막으로 좌석에 옮겨 준 시각
    seated_at: f64,
    /// 급정거에 쏠리는 중
    lurch: bool,
    /// 🎮 이 틱에 ↑를 막 누름 (운전석 — 차고가 빵빵)
    honk: bool,
    /// 🎮 이 시각(clock)까지는 내려앉아도 다시 타지 않음
    rehop_at: f64,
    /// 테스트: 주사위·쉬는 때 없이 늘 타러 감
    pub(crate) eager: bool,
}

impl VehicleMood {
    /// 시드로 난수를 따로 둠
    pub(super) fn new(seed: u64) -> Self {
        VehicleMood { rng: fastrand::Rng::with_seed(seed ^ 0x6361_7273), calm_until: 0.0, seen: Vec::new(), target_x: 0.0, arrived: false, invited: false, cover: 0.0, front: false, seated_at: 0.0, lurch: false, honk: false, rehop_at: 0.0, eager: false }
    }
}

impl Pet {
    /// 새로 탈것을 타러 갈 수 있음: 스스로 걷거나 쉬는 중, 볼일·조작·잡힘 없음, 내린 뒤 쉬는 때가 지났음
    pub fn may_board_now(&self) -> bool {
        matches!(self.state, State::Move { kind: Move::Walk | Move::Run, .. } | State::Idle { .. } | State::Turn { .. })
            && self.standing.is_some()
            && !self.cfg.flies
            && self.inside.is_none()
            && !self.housed
            && !self.has_errand()
            && !self.presence_quiet()
            && !self.is_heading_out()
            && !self.is_controlled()
            && !self.tickling
            && self.hover_since.is_none()
            && !self.thrown
            && self.juggle.carry.is_none()
            && self.home_trip.is_none()
            && (self.vehicle.eager || self.clock >= self.vehicle.calm_until)
    }

    /// 매 틱 update 전에: 탈것 자리. 걸어가는 중이면 목적지를 고치고, 한가하면 가까운 빈 탈것을 타러 갈지 정함
    pub fn notice_vehicles(&mut self, spots: &[VehicleSpot], world: &World) {
        if let State::ToVehicle { id, .. } = self.state {
            let invited = self.vehicle.invited;
            let spot = spots.iter().find(|s| s.id == id && (s.driver_free || (invited && s.bed_free)));
            match spot.filter(|s| self.standing.is_some_and(|st| Some(st.owner) == s.owner)) {
                Some(s) => self.vehicle.target_x = self.reachable_x(s.x, world).unwrap_or(self.body.x),
                None => self.cancel_boarding(),
            }
            return;
        }
        if !self.may_board_now() {
            return;
        }
        let Some(st) = self.standing else { return };
        let (x, y, clock, reach) = (self.body.x, self.body.y, self.clock, vehicle::NOTICE_REACH * self.cfg.size);
        let seen = &self.vehicle.seen;
        let eager = self.vehicle.eager;
        let near = |s: &&VehicleSpot| {
            s.driver_free && s.owner == Some(st.owner) && (s.y - y).abs() <= 4.0 && (s.x - x).abs() <= reach && (eager || !seen.iter().any(|&(id, until)| id == s.id && clock < until))
        };
        let Some(spot) = spots.iter().filter(near).min_by(|a, b| (a.x - x).abs().total_cmp(&(b.x - x).abs())).copied() else { return };
        self.vehicle.seen.retain(|&(_, until)| clock < until);
        self.vehicle.seen.push((spot.id, clock + vehicle::SAME_VEHICLE_SECS));
        if !self.vehicle.eager && self.vehicle.rng.f64() >= vehicle::BOARD_CHANCE {
            return;
        }
        let Some(to) = self.reachable_x(spot.x, world) else { return };
        self.start_to_vehicle(spot.id, to, false);
    }

    /// 🛻 트럭 운전 펫이 짐칸에 타라고 부름: 한가하면 걸어감 (주사위 없음). 갔으면 true
    pub fn invite_ride(&mut self, id: u64) -> bool {
        if !self.may_board_now() {
            return false;
        }
        let x = self.body.x;
        self.start_to_vehicle(id, x, true);
        true
    }

    /// 같은 디딤판 조각에서 x 쪽으로 걸어갈 수 있는 가장 가까운 자리 (조각 밖이면 None)
    fn reachable_x(&self, x: f64, world: &World) -> Option<f64> {
        let st = self.standing?;
        let piece = world.piece_at(st.owner, self.body.x)?;
        let (lo, hi) = (piece.x_start + self.cfg.half_w, piece.x_end - self.cfg.half_w);
        if lo > hi {
            return None;
        }
        let to = x.clamp(lo, hi);
        ((to - x).abs() <= self.cfg.size).then_some(to)
    }

    /// 탈것(id)으로 걸어가기 시작함
    fn start_to_vehicle(&mut self, id: u64, to: f64, invited: bool) {
        self.pacing.after_beat = None;
        self.clear_hover();
        self.poop.stepping_off();
        self.vehicle.target_x = to;
        self.vehicle.arrived = false;
        self.vehicle.invited = invited;
        self.state = State::ToVehicle { id, t: 0.0 };
    }

    /// 다 걸어와 태워 주기를 기다리는 탈것 (차고가 태우거나 그만두게 함)
    pub fn boarding(&self) -> Option<u64> {
        match self.state {
            State::ToVehicle { id, .. } if self.vehicle.arrived => Some(id),
            _ => None,
        }
    }

    /// 타러 가던 것을 그만둠 (자리가 없거나 탈것이 가 버림)
    pub fn cancel_boarding(&mut self) {
        if matches!(self.state, State::ToVehicle { .. }) {
            self.vehicle.arrived = false;
            self.vehicle.invited = false;
            self.state = self.idle_state();
        }
    }

    /// 타고 있는 탈것과 좌석 (0 = 운전석, 1·2 = 짐칸)
    pub fn riding(&self) -> Option<(u64, u8)> {
        match self.state {
            State::Ride { id, seat } => Some((id, seat)),
            _ => None,
        }
    }

    /// 좌석에 앉음 (차고가 부름). front = 앞모습으로 앉음 (짐칸), cover = 가려지는 높이 (물리 px). 앉았으면 true
    pub fn take_seat(&mut self, id: u64, seat: u8, front: bool, cover: f64) -> bool {
        if self.is_held() || self.housed || self.is_away() || matches!(self.state, State::Exit { .. } | State::Farewell { .. }) {
            return false;
        }
        self.keep_growth_of_interrupted_meal();
        self.pacing.after_beat = None;
        self.clear_hover();
        self.poop.stepping_off();
        self.inside = None;
        self.standing = None;
        self.body.vx = 0.0;
        self.body.vy = 0.0;
        self.thrown = false;
        self.vehicle.arrived = false;
        self.vehicle.invited = false;
        self.vehicle.front = front;
        self.vehicle.cover = cover;
        self.vehicle.seated_at = self.clock;
        self.vehicle.lurch = false;
        self.state = State::Ride { id, seat };
        true
    }

    /// 좌석 자리 (물리 px: 발) 로 옮김 — 차고가 매 틱. lurch = 급정거에 쏠리는 중
    pub fn seat_at(&mut self, x: f64, y: f64, facing: f64, cover: f64, lurch: bool) {
        if !matches!(self.state, State::Ride { .. }) {
            return;
        }
        self.body.x = x;
        self.body.y = y;
        self.facing = facing;
        self.vehicle.cover = cover;
        self.vehicle.lurch = lurch;
        self.vehicle.seated_at = self.clock;
    }

    /// 좌석에서 폴짝 내림 (dir 쪽으로) — 한동안 다시 타러 가지 않음
    pub fn leave_vehicle(&mut self, dir: f64) {
        if !matches!(self.state, State::Ride { .. }) {
            return;
        }
        self.after_ride();
        self.body.vx = dir * self.scaled(HOP_OFF.0);
        self.body.vy = -self.scaled(HOP_OFF.1);
        if dir.abs() > 0.0 {
            self.facing = dir.signum();
        }
        self.gentle_drop = true;
        self.state = State::Air { anim: Anim::Hop };
    }

    /// 좌석에서 휙 튕겨 나감 (쿵·급정거·사용자가 탈것을 잡음 — 물리 px/s)
    pub fn fling_off(&mut self, vx: f64, vy: f64) {
        if !matches!(self.state, State::Ride { .. }) {
            return;
        }
        self.after_ride();
        self.body.vx = vx;
        self.body.vy = vy;
        if vx.abs() > 1.0 {
            self.facing = vx.signum();
        }
        self.gentle_drop = true;
        self.speech = Some(Speech::Whoa);
        self.state = State::Air { anim: Anim::Fall };
    }

    /// 내린 뒤 공통: 좌석 기억을 지우고 쉬는 때를 정함
    fn after_ride(&mut self) {
        self.standing = None;
        self.thrown = false;
        self.vehicle.cover = 0.0;
        self.vehicle.lurch = false;
        self.vehicle.honk = false;
        self.vehicle.rehop_at = self.clock + REHOP_SECS;
        self.vehicle.calm_until = self.clock + behavior::range(&mut self.vehicle.rng, vehicle::CALM_SECS);
    }

    /// 달려오던 탈것이 끼익 멈춤: 놀라 폴짝 "whoa!" (dir = 탈것이 오던 쪽 — 그쪽으로 조금 밀림)
    pub fn startle(&mut self, dir: f64) {
        self.speech = Some(Speech::Whoa);
        if !self.lift_ok() && !matches!(self.state, State::ToVehicle { .. }) {
            return;
        }
        self.pacing.after_beat = None;
        self.clear_hover();
        self.poop.stepping_off();
        self.standing = None;
        self.facing = -dir;
        self.body.vx = dir * self.scaled(STARTLE_HOP.0);
        self.body.vy = -self.scaled(STARTLE_HOP.1);
        self.gentle_drop = true;
        self.thrown = false;
        self.state = State::Air { anim: Anim::Hop };
    }

    /// 탈것에 쿵 맞아 (vx, vy)로 날아감 (물리 px/s)
    pub fn bonk(&mut self, vx: f64, vy: f64) {
        if self.is_held() || self.housed || self.is_away() || self.standing.is_none() {
            return;
        }
        self.keep_growth_of_interrupted_meal();
        self.pacing.after_beat = None;
        self.clear_hover();
        self.poop.stepping_off();
        self.standing = None;
        self.inside = None;
        self.body.vx = vx;
        self.body.vy = vy;
        if vx.abs() > 1.0 {
            self.facing = -vx.signum();
        }
        self.thrown = false;
        self.gentle_drop = true;
        self.speech = Some(Speech::Oof);
        self.state = State::Air { anim: Anim::Fall };
    }

    /// 차고가 시킨 말풍선 ("vroom!", "beep beep!", "hop in!")
    pub fn vehicle_say(&mut self, s: Speech) {
        self.speech = Some(s);
    }

    /// 탈것 앞을 막을 수 있는 펫 (디딤판 위에 서 있고 타고 있지 않음)
    pub fn vehicle_obstacle(&self) -> Option<Obstacle> {
        if self.standing.is_none() || self.housed || self.is_away() || self.is_held() || matches!(self.state, State::Ride { .. }) {
            return None;
        }
        Some(Obstacle { key: self.social_key, x: self.body.x, y: self.body.y, size: self.cfg.size, half_w: self.cfg.half_w })
    }

    /// 🎮 조작 모드로 운전석에 앉아 있으면 (지금 키, 이 틱에 ↑를 막 누름) — 차고가 탈것을 키대로 몲
    pub fn steering(&self) -> Option<(control_moves::ControlInput, bool)> {
        match self.state {
            State::Ride { seat: 0, .. } => self.control_input().map(|k| (k, self.vehicle.honk)),
            _ => None,
        }
    }

    /// 🎮 조작 모드로 점프해 내려오는 중 — 빈 탈것 위에 내려앉으면 차고가 태움
    pub fn may_hop_in(&self) -> bool {
        self.is_controlled()
            && matches!(self.state, State::Air { .. })
            && self.body.vy > 0.0
            && !self.is_held()
            && !self.cfg.flies
            && self.inside.is_none()
            && !self.housed
            && self.clock >= self.vehicle.rehop_at
    }

    /// 운전해서 친구 화면(side)으로 갈 수 있음: 내 펫이고 그 끝에 친구가 연결돼 있음
    pub fn may_drive_out(&self, side: Side) -> bool {
        let open = match side {
            Side::Left => self.exit_edges.left,
            Side::Right => self.exit_edges.right,
            _ => false,
        };
        open && self.guest.is_none() && !self.housed && self.home_trip.is_none()
    }

    /// 탈것째 친구 화면으로 나갔음 (take_exit가 side를 돌려줌)
    pub fn drove_out(&mut self, side: Side) {
        if matches!(self.state, State::Ride { .. }) {
            self.vehicle.cover = 0.0;
            self.vanish(side);
        }
    }

    /// 화면 끝(side)에서 걸어 들어오는 중이면 그 끝 (탈것째 돌아올 때 그 끝에 탈것을 꺼냄)
    pub fn entering_from(&self) -> Option<Side> {
        match self.state {
            State::Enter { side, .. } => Some(side),
            _ => None,
        }
    }

    /// 사용자가 방금 놓은 펫 (공중) — 탈것 위에 떨궜으면 차고가 태움
    pub fn just_dropped(&self) -> bool {
        matches!(self.state, State::Air { .. }) && self.inside.is_none() && !self.housed
    }

    /// 탈것 한 틱 (update에서 — 이 틱을 여기서 처리했으면 true): 걸어가기, 좌석에 앉아 있기
    pub(super) fn update_vehicle(&mut self, dt: f64, world: &World) -> bool {
        match self.state {
            State::ToVehicle { id, t } => {
                self.walk_to_vehicle(dt, world, id, t);
                true
            }
            State::Ride { .. } => {
                // 🎮 조작 모드: Space = 폴짝 내림, ↑ = 빵빵 (차고가 운전석의 키로 몲)
                self.vehicle.honk = false;
                if let Some((keys, prev)) = self.ride_keys(dt) {
                    if keys.jump && !prev.jump {
                        self.leave_vehicle(self.facing);
                        return true;
                    }
                    self.vehicle.honk = keys.up && !prev.up;
                }
                // 집에 가라는 말을 들었음 / 탈것이 사라짐: 내림
                if self.home_trip.is_some() {
                    self.leave_vehicle(-self.facing);
                } else if self.clock - self.vehicle.seated_at > SEAT_STALE_SECS {
                    self.after_ride();
                    self.start_fall();
                }
                if self.vehicle.lurch {
                    self.wobble_timer = self.wobble_timer.max(2.0 * dt);
                }
                true
            }
            _ => false,
        }
    }

    /// 탈것 쪽으로 걸어감 (다 오면 멈춰 기다림 — 차고가 태움)
    fn walk_to_vehicle(&mut self, dt: f64, world: &World, id: u64, t: f64) {
        let t = t + dt;
        if t > vehicle::APPROACH_SECS {
            self.cancel_boarding();
            return;
        }
        let dx = self.vehicle.target_x - self.body.x;
        if dx.abs() <= ARRIVE_PX {
            if !self.follow_anchor(world) {
                self.start_fall();
                return;
            }
            self.vehicle.arrived = true;
        } else {
            let step = (self.cfg.walk_speed * Move::Run.speed_mult() * dt).min(dx.abs());
            self.facing = dx.signum();
            if self.juggle_walk(world, self.facing, step).is_none() {
                return;
            }
        }
        if matches!(self.state, State::ToVehicle { .. }) {
            self.state = State::ToVehicle { id, t };
        }
    }

    /// 탈것 그림: 걸어가는 중은 달리기, 운전석은 옆모습, 짐칸은 앞모습
    pub(super) fn vehicle_anim(&self) -> Anim {
        match self.state {
            State::ToVehicle { .. } if self.vehicle.arrived => Anim::Idle,
            State::ToVehicle { .. } if self.cfg.hops => self.hop_anim(Move::Run),
            State::ToVehicle { .. } => Anim::Run,
            _ if self.vehicle.front => Anim::SitFront,
            _ => Anim::Idle,
        }
    }

    /// 좌석에서 가려지는 그림 아래 몫 (0..1 — render.seat)
    pub(super) fn seat_share(&self) -> f64 {
        if matches!(self.state, State::Ride { .. }) && self.cfg.size > 0.0 {
            (self.vehicle.cover / self.cfg.size).clamp(0.0, 0.9)
        } else if let (State::Lifted { .. }, Some(cover)) = (self.state, self.hoist.cozy) {
            // 🧔 해그리드 사이드카 통에 가려짐 (brain_hagrid.rs)
            cover
        } else {
            0.0
        }
    }
}
