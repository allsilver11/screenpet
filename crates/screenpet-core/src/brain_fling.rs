//! 던져서 친구 화면으로 넘어가기 (phase 3): 던진 동물이 친구가 연결된 화면 가장 바깥 끝에 부딪히면
//! 튕기지 않고 그대로 넘어간다 — 친구 화면은 옆 모니터처럼 높이(작업 영역 비율)와 속도(논리 px/s)를 이어 받는다.
//! 넘어갈 수 있는 끝은 걸어 나갈 때와 같다 (exit_odds > 0: 내 동물은 연결된 친구 쪽, 손님은 집 쪽이나 다른 친구 쪽).
//! brain.rs의 자식 모듈.

use super::*;
use crate::visit::Fling;

impl Pet {
    /// 던진 비행의 다음 x(next_x)가 [left, right] 밖으로 나가면: 그 끝으로 넘어갈 수 있으면 넘어가고 true
    pub(super) fn fling_out(&mut self, world: &World, next_x: f64, left: f64, right: f64) -> bool {
        if !self.thrown || self.home_trip.is_some() || self.inside.is_some() {
            return false;
        }
        let side = if next_x < left && self.body.vx < 0.0 {
            Side::Left
        } else if next_x > right && self.body.vx > 0.0 {
            Side::Right
        } else {
            return false;
        };
        if self.exit_odds(side) <= 0.0 {
            return false;
        }
        let wa = world.work_area_at(self.body.x, self.body.y);
        let height = if wa.height() > 0.0 { (self.body.y - wa.top) / wa.height() } else { 1.0 };
        let dpi = self.cfg.dpi.max(0.01);
        let fling = Fling { height, vx: self.body.vx / dpi, vy: self.body.vy / dpi }.sanitized();
        self.thrown = false;
        self.vanish(side);
        self.fling = fling;
        true
    }

    /// 던져서 넘어간 움직임 (take_exit 뒤에 한 번 꺼낸다)
    pub fn take_fling(&mut self) -> Option<Fling> {
        self.fling.take()
    }

    /// 다른 화면에서 던져져 side 끝으로 날아 들어온다 (높이·속도를 이어 받고, 평소 물리로 떨어져 내려선다 — 나는 동물은 떠서 멈춘다).
    /// fling이 이상하면 평소처럼 걸어 들어온다
    pub fn enter_flung(&mut self, world: &World, side: Side, fling: Fling) {
        if visit::is_vertical(side) {
            self.enter_flung_vertical(world, side, fling);
            return;
        }
        self.enter(world, side);
        let Some(f) = fling.sanitized() else { return };
        let inward = -visit::dir(side);
        let b = world.bounds;
        // 그 끝에 닿은 작업 영역 (없으면 가장 가까운 것)
        let edge_x = match side {
            Side::Left => b.left,
            Side::Right | Side::Up | Side::Down => b.right - 1.0,
        };
        let wa = world.work_area_at(edge_x, b.bottom);
        let x = match side {
            Side::Left => b.left + self.cfg.half_w,
            Side::Right | Side::Up | Side::Down => b.right - self.cfg.half_w,
        };
        let y = (wa.top + f.height * wa.height()).clamp(wa.top + self.cfg.size, wa.bottom.max(wa.top + self.cfg.size));
        let dpi = self.cfg.dpi;
        self.body = Body { x, y, vx: inward * f.vx.abs() * dpi, vy: f.vy * dpi };
        self.standing = None;
        self.flight = None;
        self.hover_at = None;
        self.facing = inward;
        self.state = State::Air { anim: Anim::Fall };
        self.thrown = true;
    }
}
