//! 날아다니는 동물(꿀벌, spec 2B §10)의 Pet 동작 — brain.rs의 자식 모듈 (비공개 필드에 접근).
//! 비행 수치·경로 계산은 flight.rs, 쏘기 판정은 sting.rs.
//!
//! 나는 동물은 `standing`이 None인 채로 날거나 떠 있고(State::Move = 날기, 그 밖의 쉬기·애교 = 제자리에 떠서),
//! 내려앉았을 때(`standing` = Some)만 디딤판 위 동작(update_ground)을 쓴다. 중력은 받지 않는다.

use super::*;
use crate::flight::{self, Goal, Loop, Style, Wave};
use crate::sting;

/// 쏘기 시작 때 나는 범위 밖이면 이 속도(1/초)로 범위 안으로 미끄러져 들어온다 ("!" 0.5초 안에 거의 다)
const STING_EASE: f64 = 12.0;

impl Pet {
    /// 머리 위 여유 (발에서): 바깥은 말풍선 띠까지, 창 안은 몸만
    fn fly_head(&self) -> f64 {
        if self.inside.is_some() {
            self.cfg.size
        } else {
            self.cfg.size * (1.0 + flight::TOP_SPACE)
        }
    }

    /// 지금 몸이 있을 수 있는 범위: 창 안이면 그 창 안, 아니면 합친 화면
    pub(super) fn fly_limits(&self, world: &World) -> Rect {
        let area = match self.inside.and_then(|s| world.window(s.window)) {
            Some(w) => inside::air_bounds(&w.rect, &world.bounds, true),
            None => world.bounds,
        };
        flight::fly_bounds(&area, self.cfg.half_w, self.fly_head())
    }

    pub(super) fn fly_speed(&self, kind: Move) -> f64 {
        self.cfg.walk_speed * flight::speed_mult(kind)
    }

    pub(super) fn update_flyer(&mut self, dt: f64, world: &World) {
        self.update_fly_inside(dt, world);
        match self.state {
            State::Housed => {}
            State::Enter { remaining, side } => self.update_fly_enter(dt, world, remaining, side),
            _ if self.home_trip.is_some() && !self.is_held() && !matches!(self.state, State::Air { .. }) && !self.celebrating() => {
                self.update_chatter(dt);
                self.update_fly_home(dt, world);
            }
            State::Dragged => {}
            State::Pressed { grounded, .. } => {
                if grounded && self.standing.is_some() && !self.follow_anchor(world) {
                    self.standing = None;
                }
            }
            State::Air { .. } => self.update_fly_air(dt, world),
            State::ToFood { .. } => self.fly_to_food(world),
            State::WantFood { .. } => self.update_fly_want_food(dt, world),
            // 걷는 동물에서 바뀐 경우 등 — 나는 동물은 벽을 타거나 뛰지 않는다
            State::Climb { .. } | State::Slide | State::WallClimb { .. } | State::InnerClimb { .. } | State::HomeLeap { .. } => {
                self.standing = None;
                self.state = self.idle_state();
            }
            _ => {
                self.pause_for_hover();
                self.react_to_tickle();
                if matches!(self.state, State::Sting { .. }) {
                    return;
                }
                self.update_chatter(dt);
                if self.standing.is_some() {
                    self.update_perched(dt, world);
                } else {
                    self.update_flying(dt, world);
                }
            }
        }
    }

    /// 창 안에 놓였는지 보고(놓은 다음 틱), 창이 사라지거나 머문 시간이 다 되거나 집에 가야 하면 창 밖으로 날아 나간다
    fn update_fly_inside(&mut self, dt: f64, world: &World) {
        if std::mem::take(&mut self.drop_check) && matches!(self.state, State::Air { .. }) && self.home_trip.is_none() {
            let target = inside::drop_target(world, self.body.x, self.body.y, self.cfg.half_w, self.cfg.size);
            self.inside = target.map(|id| Stay::new(id, Mode::In, &mut self.rng));
        }
        let Some(mut stay) = self.inside else { return };
        stay.leave_in -= dt;
        self.inside = Some(stay);
        let Some(rect) = world.window(stay.window).map(|w| w.rect) else {
            self.inside = None;
            if self.standing.is_some() {
                self.take_off();
            }
            return;
        };
        if self.is_held() {
            return;
        }
        let free = matches!(self.state, State::Move { .. } | State::Idle { .. } | State::Turn { .. });
        if self.home_trip.is_some() || self.is_homesick() || stay.leave_in <= 0.0 && free {
            // 창 윗면 위로 날아 나간다
            self.inside = None;
            if self.standing.is_some() {
                self.take_off();
            }
            if self.home_trip.is_none() {
                let b = self.fly_limits(world);
                let to = flight::clamp(&b, self.body.x, rect.top - flight::TOP_SPACE * self.cfg.size);
                self.start_flight_styled(Goal::Point(to.0, to.1), Move::Walk, Style::Gentle);
            }
        }
    }

    /// 디딤판을 떠나 제자리에 뜬다 (잠·착지는 떠서 쉬기로)
    pub(super) fn take_off(&mut self) {
        self.standing = None;
        self.hover_at = None;
        self.body.vx = 0.0;
        self.body.vy = 0.0;
        if matches!(self.state, State::Sleep { .. } | State::Annoyed { .. } | State::Land { .. }) {
            self.state = self.idle_state();
        }
    }

    /// 내려앉아 있음: 앉은 자리가 사라지면 떠오르고, 이동할 때가 되면 날아오른다. 그 밖엔 디딤판 위 동작 그대로
    fn update_perched(&mut self, dt: f64, world: &World) {
        let ok = self.follow_anchor(world)
            && self.standing.is_some_and(|st| world.piece_at(st.owner, self.body.x).is_some_and(|p| p.width() >= 2.0 * self.cfg.half_w));
        if !ok {
            self.take_off();
            self.update_flying(dt, world);
            return;
        }
        if let Some((favorite, level_up)) = self.meal_pending.take() {
            self.pacing.after_beat = None;
            self.state = State::Eating { remaining: EAT_SECS, favorite, level_up };
            return;
        }
        if matches!(self.state, State::Move { .. }) {
            self.take_off();
            self.update_flying(dt, world);
            return;
        }
        self.update_ground(dt, world);
        // 쉬기가 끝나 날 때가 됐으면 바로 떠오른다 (디딤판 위에서 "걷지" 않는다)
        if matches!(self.state, State::Move { .. }) {
            self.take_off();
        }
    }

    /// 떠 있음: 날기(Move)는 길을 따라가고, 나머지 활동은 제자리에 떠서 시간만 흘린다
    fn update_flying(&mut self, dt: f64, world: &World) {
        // 먹이를 받았으면 가까운 자리로 내려앉으러 간다
        let to_perch = self.flight.is_some_and(|f| matches!(f.goal, Goal::Perch { .. }));
        if self.meal_pending.is_some() && !to_perch && matches!(self.state, State::Move { .. } | State::Idle { .. } | State::Turn { .. }) {
            self.pacing.after_beat = None;
            self.flight = None;
            self.state = State::Move { kind: Move::Walk, remaining: flight::MAX_FLIGHT_SECS };
        }
        match self.state {
            State::Move { kind, .. } => self.fly_step(dt, world, kind),
            _ => {
                self.flight = None;
                self.hover_in_place(world);
                self.tick_activity(dt, world);
            }
        }
    }

    /// 제자리에 떠서 위아래로 살랑
    pub(super) fn hover_in_place(&mut self, world: &World) {
        let b = self.fly_limits(world);
        let (hx, hy) = self.hover_at.unwrap_or((self.body.x, self.body.y));
        let (hx, hy) = flight::clamp(&b, hx, hy);
        self.hover_at = Some((hx, hy));
        let (x, y) = flight::clamp(&b, hx, hy + flight::bob(self.clock, self.cfg.size));
        self.body = Body { x, y, vx: 0.0, vy: 0.0 };
    }

    /// 날아갈 길을 정해 날기 시작한다 (facing = 가는 쪽). 지점으로는 굽이굽이, 내려앉으러는 완만하게
    pub(super) fn start_flight(&mut self, goal: Goal, kind: Move) {
        let style = if matches!(goal, Goal::Point(..)) { Style::Wander } else { Style::Gentle };
        self.start_flight_styled(goal, kind, style);
    }

    /// 길 모양(style)을 정해 날기 시작한다 — 곡선 자체는 첫 틱에 범위를 보고 만든다 (fly_step)
    fn start_flight_styled(&mut self, goal: Goal, kind: Move, style: Style) {
        let from = (self.body.x, self.body.y);
        let mut f = Flight::new(goal, from, Wave::Arc { height: 0.0 });
        f.style = style;
        if kind == Move::Roll {
            if let Goal::Point(x, y) = goal {
                f.loop_at = Some(flight::LOOP_AT * (x - from.0).hypot(y - from.1));
            }
        }
        if let Goal::Point(x, _) = goal {
            if (x - from.0).abs() > 1.0 {
                self.facing = (x - from.0).signum();
            }
        }
        self.flight = Some(f);
        self.hover_at = None;
        self.state = State::Move { kind, remaining: flight::MAX_FLIGHT_SECS };
    }

    /// 새 날기의 목표: 먹이를 받았으면 가까운 자리, 집이 그리운 손님은 집 쪽 끝, 아니면 아무 데나
    fn plan_flight(&mut self, world: &World, kind: Move) -> bool {
        let (x, y) = (self.body.x, self.body.y);
        let inside = self.inside.map(|s| s.window);
        if self.meal_pending.is_some() {
            // 꽃이 핀 자리로 (그 자리가 사라졌으면 가까운 자리에 꽃을 다시 피운다)
            if let Some((owner, rel_x, _)) = self.meal_spot.filter(|&(owner, rel_x, _)| world.anchor(owner).is_some_and(|(ax, _)| world.piece_at(owner, ax + rel_x).is_some())) {
                self.start_flight(Goal::Perch { owner, rel_x }, Move::Walk);
                return true;
            }
            return match flight::nearest_perch(world, inside, x, y, self.cfg.half_w, self.fly_head()) {
                Some(p) => {
                    let (ax, _) = world.anchor(p.owner).expect("surface owner has anchor");
                    let prop = self.meal_spot.map_or(Prop::Flower, |s| s.2);
                    self.meal_spot = Some((p.owner, p.x - ax, prop));
                    self.show_prop_on(prop, p.owner, p.x - ax, world);
                    self.start_flight(Goal::Perch { owner: p.owner, rel_x: p.x - ax }, Move::Walk);
                    true
                }
                None => false,
            };
        }
        if let (Some(g), None) = (self.guest.filter(|g| g.is_homesick()), inside) {
            let head = self.fly_head();
            if let Some((tx, ty)) = flight::edge_target(&mut self.rng, world, g.home, self.cfg.half_w, head, self.cfg.size) {
                self.start_flight(Goal::Point(tx, ty), kind);
                return true;
            }
        }
        let area = match self.inside.and_then(|s| world.window(s.window)) {
            Some(w) => inside::air_bounds(&w.rect, &world.bounds, true),
            None if self.rng.f64() < flight::SAME_AREA_CHANCE => world.work_area_at(x, y),
            None => world.work_areas[self.rng.usize(..world.work_areas.len())],
        };
        // 공중제비는 굽이굽이 가는 길에서만 — 원·살랑·8자는 그 밖의 날기에서
        let style = if kind == Move::Roll { Style::Wander } else { flight::pick_style(&mut self.rng) };
        let area = if style == Style::Sway && self.inside.is_none() { world.work_area_at(x, y) } else { area };
        let b = flight::fly_bounds(&area, self.cfg.half_w, self.fly_head());
        let mut to = flight::pick_target(&mut self.rng, &b);
        if style == Style::Sway {
            // 좌우로 오가며 가로로는 조금만, 위나 아래로 천천히 옮겨 간다 (넉넉한 쪽으로)
            let reach = flight::SWAY_REACH * self.cfg.size;
            to.0 = (x + (self.rng.f64() * 2.0 - 1.0) * reach).clamp(b.left, b.right);
            let drift = behavior::range(&mut self.rng, flight::SWAY_DRIFT) * self.cfg.size;
            let down = if y - drift < b.top { true } else if y + drift > b.bottom { false } else { self.rng.bool() };
            to.1 = (if down { y + drift } else { y - drift }).clamp(b.top, b.bottom);
        }
        for _ in 0..4 {
            if (to.0 - x).hypot(to.1 - y) >= flight::MIN_FLIGHT * self.cfg.size {
                break;
            }
            to = flight::pick_target(&mut self.rng, &b);
        }
        self.start_flight_styled(Goal::Point(to.0, to.1), kind, style);
        true
    }

    /// 날기 한 틱: 길(또는 공중제비)을 따라가고, 화면 끝에선 나갈지 정하고, 도착하면 떠서 쉬거나 내려앉는다
    fn fly_step(&mut self, dt: f64, world: &World, kind: Move) {
        if self.flight.is_none() && !self.plan_flight(world, kind) {
            // 내려앉을 곳이 하나도 없다 — 먹이는 떠서 먹는다
            self.state = match self.meal_pending.take() {
                Some((favorite, level_up)) => State::Eating { remaining: EAT_SECS, favorite, level_up },
                None => self.rest_beat(),
            };
            return;
        }
        let Some(mut f) = self.flight else { return };
        let to = match f.goal {
            Goal::Point(x, y) => (x, y),
            Goal::Perch { owner, rel_x } => match world.anchor(owner) {
                Some((ax, ay)) => (ax + rel_x, ay),
                None => {
                    // 앉으려던 창이 사라짐
                    self.flight = None;
                    self.state = self.idle_state();
                    return;
                }
            },
            Goal::Home => return,
        };
        let limits = self.fly_limits(world);
        let speed = self.fly_speed(kind);
        if f.path.is_none() {
            // 처음 날 때 곡선을 만들고, 첫 굽이 쪽을 바라본다
            let path = flight::build_path(&mut self.rng, f.style, f.from, to, &limits, self.cfg.size);
            let next = path.points().iter().copied().find(|p| (p.0 - f.from.0).abs() > 1.0).unwrap_or(to);
            if (next.0 - f.from.0).abs() > 1.0 {
                self.facing = (next.0 - f.from.0).signum();
            }
            f.path = Some(path);
        }
        let r = flight::LOOP_RADIUS * self.cfg.size;
        f.elapsed += dt;
        let mut done = false;
        let pos = if let Some(l) = f.looping {
            let theta = l.theta + speed / r * dt;
            if theta >= std::f64::consts::TAU {
                f.looping = None;
                f.loop_at = None;
                l.start
            } else {
                f.looping = Some(Loop { theta, ..l });
                flight::loop_point(l.start, r, theta, self.facing, l.up)
            }
        } else if f.loop_at.is_some_and(|at| f.s >= at) {
            let start = (self.body.x, self.body.y);
            f.looping = Some(Loop { start, theta: 0.0, up: start.1 - 2.0 * r >= limits.top });
            start
        } else {
            f.s += speed * dt;
            let (p, arrived) = match f.path.as_mut() {
                Some(path) => {
                    let boost = path.speed_mult();
                    path.advance(speed * boost * dt, to)
                }
                None => flight::path_point(f.from, to, f.s, f.wave),
            };
            done = arrived;
            // 가로로 가는 쪽을 바라본다 — 충분히 그쪽으로 가고, 지난번 뒤집은 지 조금 지났을 때만 (깜빡깜빡 뒤집지 않게)
            let vx = (p.0 - self.body.x) / dt.max(1e-9);
            if vx * self.facing < 0.0 && vx.abs() > flight::FLIP_SPEED * speed && f.elapsed - f.flipped_at >= flight::FLIP_HOLD {
                self.facing = vx.signum();
                f.flipped_at = f.elapsed;
            }
            p
        };
        // 내려앉는 자리는 범위를 넘어서도 정확히 (창 안 바닥 등)
        let (x, y) = if done && matches!(f.goal, Goal::Perch { .. }) { to } else { flight::clamp(&limits, pos.0, pos.1) };
        self.body = Body { x, y, vx: 0.0, vy: 0.0 };
        self.flight = Some(f);
        if self.fly_edge_exit(world, &mut f) {
            return;
        }
        if done {
            self.flight = None;
            match f.goal {
                Goal::Perch { owner, rel_x } if world.piece_at(owner, x).is_some() => {
                    self.standing = Some(Standing { owner, rel_x });
                    self.state = State::Land { remaining: LAND_SECS };
                }
                _ => {
                    self.hover_at = Some((x, y));
                    self.state = self.rest_beat();
                }
            }
        } else if f.elapsed > flight::MAX_FLIGHT_SECS {
            self.flight = None;
            self.state = self.rest_beat();
        }
    }

    /// 화면 가장 바깥 끝 가까이에서 그쪽으로 날고 있으면 (날기마다 한 번) 친구 화면으로 나갈지 정한다.
    /// 집이 그리운 손님은 집 쪽 끝 가까이에 오기만 하면 나간다. 나가기 시작했으면 true
    fn fly_edge_exit(&mut self, world: &World, f: &mut Flight) -> bool {
        if f.edge_checked || self.inside.is_some() || self.meal_pending.is_some() || !matches!(f.goal, Goal::Point(..)) {
            return false;
        }
        let dir = match self.guest {
            Some(g) if g.is_homesick() => visit::dir(g.home),
            _ => self.facing,
        };
        let Some(side) = flight::edge_ahead(world, self.body.x, self.body.y, self.cfg.half_w, self.cfg.size, dir) else { return false };
        f.edge_checked = true;
        self.flight = Some(*f);
        let chance = visit::exit_chance(side, self.exit_edges, self.guest);
        if chance >= 1.0 || chance > 0.0 && self.rng.f64() < chance {
            self.flight = None;
            self.start_exit(side);
            return true;
        }
        false
    }

    /// 던져짐: 중력 없이 공기 저항으로 느려지다가 멈추면 그 자리에 뜬다 (떨어지지 않는다)
    fn update_fly_air(&mut self, dt: f64, world: &World) {
        let b = self.fly_limits(world);
        let (mut vx, mut vy) = flight::drift(self.body.vx, self.body.vy, dt);
        let (mut x, mut y) = (self.body.x + vx * dt, self.body.y + vy * dt);
        if x < b.left || x > b.right {
            x = x.clamp(b.left, b.right);
            vx = -vx * crate::physics::WALL_DAMPING;
        }
        if y < b.top || y > b.bottom {
            y = y.clamp(b.top, b.bottom);
            vy = -vy * crate::physics::WALL_DAMPING;
        }
        if vx.abs() > 1.0 {
            self.facing = vx.signum();
        }
        self.standing = None;
        if vx.hypot(vy) < flight::SETTLE_SPEED {
            self.body = Body { x, y, vx: 0.0, vy: 0.0 };
            self.hover_at = Some((x, y));
            self.state = self.rest_beat();
        } else {
            self.body = Body { x, y, vx, vy };
        }
    }

    /// 하우스 문까지 활 모양으로 날아가 들어간다
    fn update_fly_home(&mut self, dt: f64, world: &World) {
        let Some(trip) = self.home_trip else { return };
        let Some((_, gy)) = world.anchor(trip.owner) else {
            // 하우스가 있던 모니터가 사라짐 — 그냥 하우스 안으로
            self.enter_house();
            return;
        };
        if self.standing.is_some() {
            self.take_off();
        }
        self.inside = None;
        let to = (trip.door_x, gy);
        let mut f = match self.flight {
            Some(f) if f.goal == Goal::Home => f,
            _ => {
                let from = (self.body.x, self.body.y);
                let len = (to.0 - from.0).hypot(to.1 - from.1);
                let height = (flight::HOME_ARC * len).min(flight::HOME_ARC_MAX * self.cfg.size);
                Flight::new(Goal::Home, from, Wave::Arc { height })
            }
        };
        f.s += self.fly_speed(Move::Run) * dt;
        let ((x, y), done) = flight::path_point(f.from, to, f.s, f.wave);
        if (to.0 - f.from.0).abs() > 1.0 {
            self.facing = (to.0 - f.from.0).signum();
        }
        self.body = Body { x, y, vx: 0.0, vy: 0.0 };
        self.state = State::GoHome;
        if done {
            self.flight = None;
            self.arrive_home(world);
        } else {
            self.flight = Some(f);
        }
    }

    /// 화면 끝(side) 밖으로 수평으로 날아 나간다 — 최소 remaining초, 몸이 다 빠져나갈 때까지
    pub(super) fn update_fly_exit(&mut self, dt: f64, world: &World, remaining: f64, side: Side) {
        self.facing = visit::dir(side);
        self.standing = None;
        self.body.x += self.facing * self.fly_speed(Move::Walk) * dt;
        let remaining = remaining - dt;
        if remaining <= 0.0 && visit::fully_outside(&world.bounds, side, self.body.x, self.cfg.half_w) {
            self.vanish(side);
        } else {
            self.state = State::Exit { remaining, side };
        }
    }

    /// 화면 끝(side) 바로 바깥 공중에 두고 안쪽으로 날아 들어오게 한다
    pub(super) fn enter_flying(&mut self, world: &World, side: Side) {
        let head = self.cfg.size * (1.0 + flight::TOP_SPACE);
        let (x, y) = flight::entry_point(world, side, self.cfg.half_w, head);
        self.body = Body { x, y, vx: 0.0, vy: 0.0 };
        self.standing = None;
        self.flight = None;
        self.hover_at = None;
        self.state = State::Enter { remaining: visit::ENTER_SECS, side };
    }

    fn update_fly_enter(&mut self, dt: f64, world: &World, remaining: f64, side: Side) {
        self.facing = -visit::dir(side);
        self.standing = None;
        self.body.x += self.facing * self.fly_speed(Move::Walk) * dt;
        let remaining = remaining - dt;
        self.state = if remaining <= 0.0 && visit::fully_inside(&world.bounds, side, self.body.x, self.cfg.half_w) {
            State::Move { kind: Move::Walk, remaining: flight::MAX_FLIGHT_SECS }
        } else {
            State::Enter { remaining, side }
        };
    }

    /// 떠서 쉬는 박자가 끝난 뒤: 시무룩 / 어딘가에 내려앉으러 가기(35%) / 떠서 좀 더 쉬기
    pub(super) fn fly_rest_state(&mut self, world: &World) -> State {
        if self.hungry && self.rng.f64() < HUNGRY_REST_CHANCE {
            return State::Hungry { remaining: behavior::range(&mut self.rng, HUNGRY_SECS) };
        }
        if !self.is_homesick() && self.rng.f64() < flight::PERCH_CHANCE {
            let inside = self.inside.map(|s| s.window);
            let head = self.fly_head();
            if let Some(p) = flight::pick_perch(&mut self.rng, world, inside, self.cfg.half_w, head) {
                let (ax, _) = world.anchor(p.owner).expect("surface owner has anchor");
                // 쉬러 갈 땐 굽이굽이 (먹으러 갈 땐 완만하게 곧장)
                self.start_flight_styled(Goal::Perch { owner: p.owner, rel_x: p.x - ax }, Move::Walk, Style::Wander);
                return self.state;
            }
        }
        self.idle_state()
    }

    /// 다음 날기 (방식만 정하고 길은 날기 시작할 때 정한다)
    pub(super) fn fly_move_state(&mut self) -> State {
        self.flight = None;
        let kind = flight::pick_move(&mut self.rng);
        self.next_after_beat(State::Move { kind, remaining: flight::MAX_FLIGHT_SECS })
    }

    /// 떠 있는 나는 동물의 먹이: 배부르면 떠서 갸웃, 간식은 떠서 오물오물, 밥·물은 꽃(그릇)이 피어난 자리에 내려앉아 먹는다
    pub(super) fn feed_flying(&mut self, result: FeedResult, food: Food) -> bool {
        let free = !self.off_surface() && !self.is_held() && !self.in_transit() && self.home_trip.is_none() && !matches!(self.state, State::Sting { .. });
        if !free {
            return false;
        }
        self.pacing.after_beat = None;
        match result {
            FeedResult::Refused => {
                self.speech = Some(Speech::Full);
                self.state = State::Trick { kind: Trick::Tilt, remaining: REFUSE_TILT_SECS };
            }
            FeedResult::Ate { leveled_up } => self.serve(food, leveled_up),
        }
        true
    }

    /// 손에 든 음식으로 날아가 얼굴을 음식에 갖다 댄다 (내려앉아 있었으면 떠오른다)
    fn update_fly_want_food(&mut self, dt: f64, world: &World) {
        let Some((fx, fy)) = self.hand else {
            self.state = self.idle_state();
            return;
        };
        if self.standing.is_some() {
            self.take_off();
        }
        self.flight = None;
        self.hover_at = None;
        let b = self.fly_limits(world);
        let (tx, ty) = flight::clamp(&b, fx, fy + sting::FACE_HEIGHT * self.cfg.size);
        let (dx, dy) = (tx - self.body.x, ty - self.body.y);
        let dist = dx.hypot(dy);
        if (fx - self.body.x).abs() > 1.0 {
            self.facing = (fx - self.body.x).signum();
        }
        let step = self.fly_speed(Move::Walk) * dt;
        let walking = dist > step;
        let (x, y) = if walking { (self.body.x + dx / dist * step, self.body.y + dy / dist * step) } else { (tx, ty) };
        self.body = Body { x, y, vx: 0.0, vy: 0.0 };
        self.state = State::WantFood { walking };
    }

    /// 받은 먹이의 자리를 정한다: 내려앉아 있으면 바라보는 쪽 앞, 떠 있으면 가장 가까운 자리.
    /// 그 자리에 꽃(물은 물그릇)이 피고, 거기로 날아가 내려앉아 먹는다. 앉을 곳이 없으면 떠서 먹는다
    fn fly_to_food(&mut self, world: &World) {
        let Some(trip) = self.food.take() else {
            self.state = self.idle_state();
            return;
        };
        let prop = Prop::for_food(trip.food, true);
        let spot = match self.standing {
            Some(st) => world.anchor(st.owner).zip(world.piece_at(st.owner, self.body.x).copied()).map(|((ax, ay), piece)| {
                let (lo, hi) = (piece.x_start + self.cfg.half_w, piece.x_end - self.cfg.half_w);
                (st.owner, food::bowl_x(self.body.x, self.facing, self.scaled(food::BOWL_GAP), lo, hi) - ax, ay)
            }),
            None => {
                let inside = self.inside.map(|s| s.window);
                flight::nearest_perch(world, inside, self.body.x, self.body.y, self.cfg.half_w, self.fly_head()).and_then(|p| {
                    let (ax, _) = world.anchor(p.owner)?;
                    Some((p.owner, p.x - ax, p.y))
                })
            }
        };
        let Some((owner, rel_x, _)) = spot else {
            self.state = State::Eating { remaining: EAT_SECS, favorite: trip.heart, level_up: trip.level_up };
            return;
        };
        self.show_prop_on(prop, owner, rel_x, world);
        self.meal_spot = Some((owner, rel_x, prop));
        self.meal_pending = Some((trip.heart, trip.level_up));
        if self.standing.is_some() {
            self.take_off();
        }
        self.start_flight(Goal::Perch { owner, rel_x }, Move::Walk);
    }

    /// 지금 쏠 수 있는지: 쏘는 동물, 쿨다운 끝, 하우스·집 가는 길·화면 드나들기 아님
    pub(super) fn can_sting_now(&self) -> bool {
        self.cfg.stings
            && self.clock >= self.sting_ready_at
            && !self.housed
            && self.home_trip.is_none()
            && !self.in_transit()
            && !matches!(self.state, State::Sting { .. })
    }

    /// 화나서 쏘기 시작 (들려 있었으면 손에서 빠져나온다)
    pub(super) fn start_sting(&mut self) {
        self.sting_ready_at = self.clock + sting::COOLDOWN_SECS;
        self.pacing.after_beat = None;
        self.before_press = None;
        self.flight = None;
        self.hover_at = None;
        self.drop_check = false;
        self.shake.clear();
        self.pester.clear();
        self.clear_hover();
        self.body.vx = 0.0;
        self.body.vy = 0.0;
        self.state = State::Sting { phase: StingPhase::Alert, remaining: StingPhase::Alert.secs() };
    }

    /// 쏘기: 제자리에서 (앉아 있으면 앉은 자리를 따라가며) 국면을 넘긴다. 돌진이 끝나는 순간 콕! — 얼굴 위치를 앱에 알린다
    pub(super) fn update_sting(&mut self, dt: f64, world: &World, phase: StingPhase, remaining: f64) {
        if self.standing.is_some() && !self.follow_anchor(world) {
            self.standing = None;
        }
        if self.standing.is_none() {
            // 떠 있으면 살랑이지 않고 그 자리에 멈춰 노려본다. 나는 범위 밖(흔들다 놓친 자리)이면 부드럽게 들어온다
            let b = self.fly_limits(world);
            let (tx, ty) = flight::clamp(&b, self.body.x, self.body.y);
            let k = 1.0 - (-STING_EASE * dt).exp();
            let ease = |from: f64, to: f64| if (to - from).abs() < 0.5 { to } else { from + (to - from) * k };
            self.body = Body { x: ease(self.body.x, tx), y: ease(self.body.y, ty), vx: 0.0, vy: 0.0 };
        }
        let remaining = remaining - dt;
        if remaining > 0.0 {
            self.state = State::Sting { phase, remaining };
            return;
        }
        match phase.next() {
            Some(next) => {
                match phase {
                    StingPhase::Dash => {
                        self.sting_at = Some(sting::face_point(self.body.x, self.body.y, self.cfg.size));
                        self.speech = Some(Speech::Sting);
                    }
                    StingPhase::Stuck => self.speech = Some(Speech::Grumpy),
                    _ => {}
                }
                self.state = State::Sting { phase: next, remaining: next.secs() };
            }
            None => {
                self.hover_at = None;
                self.state = self.move_state(true, world);
            }
        }
    }

    /// 들고 흔드는 중: 1초 안에 600px 넘게 흔들리면 쏘면서 손에서 빠져나온다
    pub(super) fn shake_check(&mut self, cx: f64, cy: f64) {
        if !self.cfg.stings {
            return;
        }
        self.shake.push(self.clock, cx, cy);
        if self.shake.shaken() && self.can_sting_now() {
            self.start_sting();
        }
    }
}
