//! 🦎 조작 모드 벽 붙기 (Space Space) — brain.rs의 자식 모듈. 조작 모드 한 틱 (brain_control.rs update_control)이 부름.
//!
//! - Space를 DOUBLE_TAP_SECS 안에 두 번 (첫 Space는 평소 점프 — 공중에서 붙음, 아직 바닥이어도 붙음): 보는 쪽 몸 앞에서 CLING_REACH 안에
//!   화면 끝 (작업 영역 옆변 — Screen) 이나 창 옆면 (Window — 발이 그 창 높이 안) 이 있으면 가까운 것에 붙고, 없으면 그 자리 화면 유리 (Glass)에 붙음.
//!   어느 펫이든 (원래 나는 동물 빼고). 그림은 climb (뒷모습), climb 그림이 없으면 dragged (매달린 모습).
//! - 붙어 있는 동안 (몸은 State::Air 그대로 — 중력 없음, 키 없으면 그 자리):
//!   - 유리: 방향키 4방향 (behavior::CLIMB_SPEED, Shift CLING_BOOST배). 그 x 줄의 작업 영역 안만 (크기 다른 모니터 사이 빈 곳·작업표시줄 뒤 안 감).
//!     ↓로 바닥·창 윗면에 닿으면 내려섬, ↑로 창 윗변을 넘으면 그 창 위에 올라섬.
//!   - 화면 끝·창 옆면: ↑/↓ 벽 따라 오르내림, 벽 반대쪽 방향키 = 유리로 옮겨 감 (떨어지지 않음), 벽 쪽 방향키 = 아무것도 안 함.
//!     창 옆면은 창을 따라감 (사라지면 떨어짐), 끝까지 오르면 창 위에 올라섬, 창 아래 끝을 지나면 유리로. 화면 끝은 맨 위에서 멈춤.
//!   - Space = 놓음 (벽이면 반대쪽으로 폴짝, 유리면 그대로 떨어짐) — 이 Space는 다음 Space Space로 세지 않음.
//! - 차례: 붙어 있으면 이것이 먼저. ↑↑만 특기가 있는 펫 (날기·빗자루·웹스윙 — brain_feat.rs)에게 넘김 (벽에서 떨어져 특기 시작),
//!   ↓↓는 그냥 내려가기 (창에서 뛰어내리기 아님). 날기·빗자루·그네 중엔 Space Space가 붙지 않음 (그네는 첫 Space로 놓으니 둘째로 붙음),
//!   🦇 오토바이는 내리고 붙음.
//! - 조작 모드가 끝나거나 볼일 (잡기·먹이·하우스 등)이 끼면 놓고 떨어짐 (drop_cling). 친구 화면에 간 내 펫도 같은 키 비트로 됨.

use super::control_moves::{ControlInput, Taps};
use super::feat_moves::Feat;
use super::*;

/// 벽을 찾는 거리 (몸 앞에서 가로로, 몸 크기 배)
pub const CLING_REACH: f64 = 2.5;
/// Shift를 누르면 이만큼 빨리 김
pub const CLING_BOOST: f64 = 2.0;

/// 붙어 있는 곳
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ClingWall {
    /// 화면 유리 (아무 데나)
    Glass,
    /// 화면 끝 (작업 영역 옆변) — dir = 벽이 있는 쪽 (±1)
    Screen { dir: f64 },
    /// 창(id) 옆면 — dir = 벽이 있는 쪽 (±1), rel_y = 창 윗변에서 발까지
    Window { id: u64, dir: f64, rel_y: f64 },
}

impl Pet {
    /// 지금 붙어 있는 곳 (조작 모드가 아니거나 안 붙었으면 None)
    pub(super) fn cling_on(&self) -> Option<ClingWall> {
        self.control.and_then(|c| c.cling)
    }

    /// 붙은 곳을 바꿈 (조작 모드가 아니면 아무것도 안 함)
    fn set_cling(&mut self, wall: Option<ClingWall>) {
        if let Some(c) = self.control.as_mut() {
            c.cling = wall;
        }
    }

    /// 붙은 그림: climb (뒷모습), 그 그림이 없는 펫은 dragged
    fn cling_anim(&self) -> Anim {
        if self.cfg.can_climb {
            Anim::Climb
        } else {
            Anim::Dragged
        }
    }

    /// 기는 빠르기 (px/s — run = Shift)
    fn cling_speed(&self, run: bool) -> f64 {
        self.scaled(behavior::CLIMB_SPEED) * if run { CLING_BOOST } else { 1.0 }
    }

    /// 놓고 떨어짐 (조작 끝·볼일) — 붙어 있지 않으면 아무것도 안 함
    pub(super) fn drop_cling(&mut self) {
        if self.cling_on().is_none() {
            return;
        }
        self.set_cling(None);
        if self.standing.is_none() && matches!(self.state, State::Air { .. }) {
            self.body.vx = 0.0;
            self.body.vy = 0.0;
            self.state = State::Air { anim: Anim::Fall };
        }
    }

    /// Space Space: 보는 쪽 가까운 벽이나 유리에 붙음. 날기·빗자루·그네 중·원래 나는 동물·벽 타기 중이면 false
    pub(super) fn start_cling(&mut self, world: &World) -> bool {
        if self.cfg.flies || matches!(self.feat_on(), Some(Feat::Fly | Feat::Broom | Feat::Swing)) {
            return false;
        }
        if matches!(self.state, State::Climb { .. } | State::Slide | State::WallClimb { .. }) || !(self.standing.is_some() || matches!(self.state, State::Air { .. })) {
            return false;
        }
        // 🦇 오토바이는 두고 붙음
        self.clear_feat();
        let wall = self.find_cling_wall(world);
        let half_w = self.cfg.half_w;
        let x = match wall {
            ClingWall::Glass => self.body.x,
            ClingWall::Screen { dir } => {
                let wa = world.work_area_at(self.body.x, self.body.y - 1.0);
                (if dir > 0.0 { wa.right } else { wa.left }) - dir * half_w
            }
            ClingWall::Window { id, dir, .. } => {
                let r = world.window(id).expect("found window").rect;
                if dir > 0.0 { r.left - half_w } else { r.right + half_w }
            }
        };
        let (x, y) = self.feat_fly_clamp(world, x, self.body.y);
        self.standing = None;
        self.body = Body { x, y, vx: 0.0, vy: 0.0 };
        let wall = match wall {
            ClingWall::Window { id, dir, .. } => {
                let top = world.window(id).expect("found window").rect.top;
                ClingWall::Window { id, dir, rel_y: y - top }
            }
            w => w,
        };
        self.soar.gliding = false;
        self.broom.descending = false;
        self.bike.jumping = false;
        self.set_cling(Some(wall));
        self.state = State::Air { anim: self.cling_anim() };
        true
    }

    /// 보는 쪽 몸 앞 CLING_REACH 안의 가장 가까운 화면 끝·창 옆면 (발이 그 창 높이 안), 없으면 유리
    fn find_cling_wall(&self, world: &World) -> ClingWall {
        let (x, y, dir) = (self.body.x, self.body.y, self.facing);
        let front = x + dir * self.cfg.half_w;
        let reach = CLING_REACH * self.cfg.size;
        let near = |side_x: f64| {
            let gap = (side_x - front) * dir;
            (-1.0..=reach).contains(&gap).then_some(gap)
        };
        let mut best: Option<(f64, ClingWall)> = None;
        let wa = world.work_area_at(x, y - 1.0);
        if let Some(gap) = near(if dir > 0.0 { wa.right } else { wa.left }) {
            best = Some((gap, ClingWall::Screen { dir }));
        }
        for w in &world.windows {
            let r = w.rect;
            if !(r.top < y - 1.0 && y <= r.bottom + 1.0) {
                continue;
            }
            if let Some(gap) = near(if dir > 0.0 { r.left } else { r.right }) {
                if best.is_none_or(|(g, _)| gap < g) {
                    best = Some((gap, ClingWall::Window { id: w.id, dir, rel_y: y - r.top }));
                }
            }
        }
        best.map_or(ClingWall::Glass, |(_, wall)| wall)
    }

    /// 벽에 붙어 있으면 그 한 틱 (update_control — 특기보다 먼저). 이번 틱을 다 처리했으면 true.
    /// ↑↑이고 특기가 날기·빗자루·그네면 놓고 false (update_feat가 이어서 특기를 켬)
    pub(super) fn update_wall_cling(&mut self, dt: f64, world: &World, input: ControlInput, taps: Taps) -> bool {
        let Some(wall) = self.cling_on() else { return false };
        if self.standing.is_some() || !matches!(self.state, State::Air { .. }) {
            self.set_cling(None);
            return false;
        }
        if taps.double_up && matches!(self.feat_kind(), Some(Feat::Fly | Feat::Broom | Feat::Swing)) {
            self.drop_cling();
            return false;
        }
        if taps.jump {
            self.set_cling(None);
            // 이 Space는 다음 Space Space의 첫 번째로 세지 않음
            if let Some(c) = self.control.as_mut() {
                c.last_jump = f64::NEG_INFINITY;
            }
            match wall {
                ClingWall::Glass => {
                    self.body.vx = 0.0;
                    self.body.vy = 0.0;
                    self.state = State::Air { anim: Anim::Fall };
                }
                ClingWall::Screen { dir } | ClingWall::Window { dir, .. } => self.start_hop(-dir),
            }
            return true;
        }
        let v = self.cling_speed(input.run);
        match wall {
            ClingWall::Glass => self.cling_glass(dt, world, input, v),
            ClingWall::Screen { dir } => {
                if input.dir() == -dir {
                    self.set_cling(Some(ClingWall::Glass));
                    self.cling_glass(dt, world, input, v);
                } else {
                    let (x, prev_y) = (self.body.x, self.body.y);
                    let (x, y) = self.feat_fly_clamp(world, x, prev_y + input.vertical() * v * dt);
                    self.cling_move(world, input, x, prev_y, y);
                }
            }
            ClingWall::Window { id, dir, rel_y } => self.cling_window(dt, world, input, v, id, dir, rel_y),
        }
        true
    }

    /// 🦎 유리 위 한 걸음: 가로·세로 따로 (가로로 그 x 줄 안에 못 들어가면 가로만 멈춤), 세로는 그 줄 안으로
    fn cling_glass(&mut self, dt: f64, world: &World, input: ControlInput, v: f64) {
        let (dx, dy) = (input.dir(), input.vertical());
        if dx != 0.0 {
            self.facing = dx;
        }
        let (ox, prev_y) = (self.body.x, self.body.y);
        let b = world.bounds;
        let half_w = self.cfg.half_w;
        let x = (ox + dx * v * dt).clamp(b.left + half_w, (b.right - half_w).max(b.left + half_w));
        let fits = self.feat_fly_room(world, x).is_some_and(|(top, bottom)| top - 0.5 <= prev_y && prev_y <= bottom + 0.5);
        let x = if fits { x } else { ox };
        let (x, y) = self.feat_fly_clamp(world, x, prev_y + dy * v * dt);
        self.cling_move(world, input, x, prev_y, y);
    }

    /// 🦎 (x, y)로 옮김: ↓로 바닥·창 윗면에 닿았으면 내려서고, 위로 가다 창 윗변을 넘었으면 그 위에 올라섬 (유리일 때). 아니면 붙은 채
    fn cling_move(&mut self, world: &World, input: ControlInput, x: f64, prev_y: f64, y: f64) {
        self.body = Body { x, y, vx: 0.0, vy: 0.0 };
        if input.down {
            if let Some(s) = world.fall_landing(x, prev_y, y + 0.5, 0.0, false) {
                self.cling_stand(world, s);
                return;
            }
        }
        if y < prev_y && self.cling_on() == Some(ClingWall::Glass) {
            let over = world.surfaces.iter().filter(|s| matches!(s.owner, Owner::Window(_)) && s.contains_x(x) && y <= s.y && s.y < prev_y).max_by(|a, b| a.y.total_cmp(&b.y)).copied();
            if let Some(s) = over {
                self.cling_stand(world, s);
                return;
            }
        }
        self.state = State::Air { anim: self.cling_anim() };
    }

    /// 🦎 디딤판 s에 내려서고 벽에서 떨어짐 (누르고 있던 ↑는 뗄 때까지 점프로 안 씀 — quiet_up)
    fn cling_stand(&mut self, world: &World, s: Surface) {
        let Some((ax, _)) = world.anchor(s.owner) else { return };
        self.body = Body { x: self.body.x, y: s.y, vx: 0.0, vy: 0.0 };
        self.standing = Some(Standing { owner: s.owner, rel_x: self.body.x - ax });
        self.set_cling(None);
        if let Some(c) = self.control.as_mut() {
            c.quiet_up = true;
        }
        self.state = State::Land { remaining: LAND_SECS };
    }

    /// 🦎 창 옆면 한 걸음: 창을 따라감 (사라지면 떨어짐). ↑/↓ 오르내림, 끝까지 오르면 창 위에 올라섬, 아래 끝을 지나면 유리로,
    /// 벽 반대쪽 방향키 = 유리로
    #[allow(clippy::too_many_arguments)]
    fn cling_window(&mut self, dt: f64, world: &World, input: ControlInput, v: f64, id: u64, dir: f64, rel_y: f64) {
        let Some(rect) = world.window(id).map(|w| w.rect) else {
            self.drop_cling();
            return;
        };
        let half_w = self.cfg.half_w;
        let x = if dir > 0.0 { rect.left - half_w } else { rect.right + half_w };
        if input.dir() == -dir {
            self.body.x = x;
            self.body.y = rect.top + rel_y.clamp(0.0, rect.height());
            self.set_cling(Some(ClingWall::Glass));
            self.cling_glass(dt, world, input, v);
            return;
        }
        let prev_y = rect.top + rel_y;
        let rel_y = rel_y + input.vertical() * v * dt;
        if rel_y <= 0.0 {
            // 끝까지 오름: 창 위 모서리 안쪽에 올라섬 (가려져 디딜 데가 없으면 맨 위에서 멈춤)
            let on_x = if dir > 0.0 { rect.left + half_w } else { rect.right - half_w };
            if let Some(s) = world.piece_at(Owner::Window(id), on_x).copied() {
                self.body.x = on_x;
                self.cling_stand(world, s);
                return;
            }
        }
        if rel_y > rect.height() {
            self.body = Body { x, y: rect.bottom, vx: 0.0, vy: 0.0 };
            self.set_cling(Some(ClingWall::Glass));
            self.cling_glass(dt, world, input, v);
            return;
        }
        let rel_y = rel_y.max(0.0);
        let (x, y) = self.feat_fly_clamp(world, x, rect.top + rel_y);
        self.set_cling(Some(ClingWall::Window { id, dir, rel_y: y - rect.top }));
        self.cling_move(world, input, x, prev_y, y);
    }
}
