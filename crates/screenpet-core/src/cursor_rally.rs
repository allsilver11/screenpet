//! ⚾🖱️ 커서와 캐치볼 (v0.6.5) — 숫자와 순수 계산. 흐물이의 받기·되던지기는 brain_rally.rs, 다른 동물의 콩 받아치기·먼저 놀자고 하기는
//! brain_cursor_rally.rs, 물건 쪽 기억은 object.rs (by_user·cursor_rally), 창은 src-tauri.
//!
//! - 사용자가 던진 물건(거대한 것·눈덩이 빼고)이 흐물이 쪽으로 날아오면 흐물이가 머리 위로 받아 몇 번째인지 외치고 ("1!") 커서 쪽으로 되던짐
//! - 손 없는 동물은 맞을 때 가끔 (BOOP_CHANCE) 머리로 콩 받아쳐 커서 쪽으로 돌려보냄 ("boop! 2")
//! - 되돌아온 것을 날아가는 중이나 내려앉고 RALLY_GRAB_SECS 안에 다시 잡아 던지면 횟수가 이어짐. 더 오래 두면 조용히 끝 (다음은 1부터)
//! - 가끔 (INVITE_EVERY_SECS에 한 번쯤) 동물이 먼저 근처 물건을 커서 쪽으로 던지거나 밀어 "catch!" — 이 첫 던지기가 1.
//!   사용자가 INVITE_WAIT_SECS 안에 받아 주지 않으면 더 길게 (IGNORED_BACKOFF_SECS) 쉼

use crate::juggle::{self, Target};

/// 되돌아온 것을 내려앉은 뒤 이만큼 안에 잡아야 횟수가 이어짐 (초)
pub const RALLY_GRAB_SECS: f64 = 3.0;
/// 손 없는 동물이 사용자가 던진 것에 맞을 때 콩 받아칠 확률
pub const BOOP_CHANCE: f64 = 0.5;
/// 먼저 놀자고 하는 간격 (초)
pub const INVITE_EVERY_SECS: (f64, f64) = (180.0, 360.0);
/// 놀자고 할 때가 됐을 때 정말 할 확률 (아니면 다음 간격까지)
pub const INVITE_CHANCE: f64 = 0.5;
/// 놀자고 한 뒤 이만큼 안에 사용자가 받아 주지 않으면 무시한 것임 (초)
pub const INVITE_WAIT_SECS: f64 = 15.0;
/// 무시당하면 이만큼 쉼 (초)
pub const IGNORED_BACKOFF_SECS: (f64, f64) = (600.0, 1200.0);
/// 흐물이가 놀자고 집으러 가는 거리 (몸 크기 배)
pub const INVITE_REACH: f64 = 3.0;
/// 손 없는 동물이 놀자고 밀 수 있는 거리 (몸 크기 배)
pub const BOOP_REACH: f64 = 1.2;

/// (from)에서 커서 쪽으로 던지는 속도 (물리 px/s): 커서 점을 지나는 포물선 (너무 빠르면 던지기 상한으로 자름)
pub fn to_cursor_velocity(from: (f64, f64), cursor: (f64, f64), size: f64, dpi: f64) -> (f64, f64) {
    let (vx, vy) = juggle::aim_at(from, Target { x: cursor.0, y: cursor.1, size: 0.0 }, true, 0.0, size, dpi);
    crate::physics::clamp_speed(vx, vy, crate::physics::MAX_THROW_SPEED_LOGICAL * dpi)
}

/// 앱에 알릴 일 (Pet::take_cursor_rally로 한 번 꺼냄)
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum CursorRallyEvent {
    /// 흐물이가 이 물건을 커서 쪽으로 던졌음 (받아 되던졌거나 놀자고 던졌음): 앱이 rally번째로 적고 커서 랠리로 표시함
    Back { id: u64, rally: u32 },
    /// 손 없는 동물이 바닥의 이 물건을 커서 쪽으로 (vx, vy — 물리 px/s) 밀어 놀자고 함: 앱이 Object::boop_back으로 날림
    Boop { id: u64, vx: f64, vy: f64, rally: u32 },
}

/// 먼저 놀자고 하기의 시계 (동물마다 하나)
#[derive(Debug, Clone, Default)]
pub struct Invites {
    /// 이때부터 놀자고 할 수 있음 (처음 보면 정함)
    next_at: Option<f64>,
    /// 놀자고 했고 이때까지 답을 기다림
    waiting_until: Option<f64>,
}

/// lo..hi에서 고르게 뽑음
fn range(rng: &mut fastrand::Rng, (lo, hi): (f64, f64)) -> f64 {
    lo + rng.f64() * (hi - lo)
}

impl Invites {
    /// 지금 놀자고 해도 됨: 때가 됐고 답을 기다리는 중이 아님 (처음 부르면 간격을 정하고 false)
    pub fn due(&mut self, clock: f64, rng: &mut fastrand::Rng) -> bool {
        let at = *self.next_at.get_or_insert_with(|| clock + range(rng, INVITE_EVERY_SECS));
        self.waiting_until.is_none() && clock >= at
    }

    /// 때가 됐지만 주사위가 아니라고 함: 다음 간격까지
    pub fn skip(&mut self, clock: f64, rng: &mut fastrand::Rng) {
        self.next_at = Some(clock + range(rng, INVITE_EVERY_SECS));
    }

    /// 놀자고 했음: 답을 기다림
    pub fn invited(&mut self, clock: f64) {
        self.waiting_until = Some(clock + INVITE_WAIT_SECS);
        self.next_at = Some(f64::INFINITY);
    }

    /// 사용자가 같이 놀았음 (던진 것을 받거나 받아쳤음): 다음은 평소 간격
    pub fn answered(&mut self, clock: f64, rng: &mut fastrand::Rng) {
        if self.waiting_until.take().is_some() || self.next_at.is_none_or(|at| at < clock) {
            self.next_at = Some(clock + range(rng, INVITE_EVERY_SECS));
        }
    }

    /// 매 틱: 답을 기다리다 시간이 넘었으면 무시당한 것 — 길게 쉼
    pub fn tick(&mut self, clock: f64, rng: &mut fastrand::Rng) {
        if self.waiting_until.is_some_and(|until| clock > until) {
            self.waiting_until = None;
            self.next_at = Some(clock + range(rng, IGNORED_BACKOFF_SECS));
        }
    }

    /// 답을 기다리는 중
    pub fn waiting(&self) -> bool {
        self.waiting_until.is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::physics::THROW_GRAVITY_LOGICAL;

    #[test]
    fn a_throw_toward_the_cursor_passes_the_cursor() {
        let from = (500.0, 900.0);
        let (vx, vy) = to_cursor_velocity(from, (900.0, 600.0), 100.0, 1.0);
        let t = (900.0 - from.0) / vx;
        let y = from.1 + vy * t + 0.5 * THROW_GRAVITY_LOGICAL * t * t;
        assert!((y - 600.0).abs() < 1.0, "{y}");
        // 아주 멀어도 던지기 상한을 넘지 않는다
        let (vx, vy) = to_cursor_velocity(from, (5000.0, -3000.0), 100.0, 1.0);
        assert!(vx.hypot(vy) <= crate::physics::MAX_THROW_SPEED_LOGICAL + 1e-6);
    }

    #[test]
    fn invites_come_now_and_then_and_back_off_when_ignored() {
        let mut rng = fastrand::Rng::with_seed(1);
        let mut inv = Invites::default();
        assert!(!inv.due(0.0, &mut rng), "not right away");
        assert!(!inv.due(INVITE_EVERY_SECS.0 - 1.0, &mut rng));
        assert!(inv.due(INVITE_EVERY_SECS.1, &mut rng));
        inv.invited(400.0);
        assert!(inv.waiting() && !inv.due(401.0, &mut rng));
        // 무시당함: 오래 쉰다
        inv.tick(400.0 + INVITE_WAIT_SECS + 1.0, &mut rng);
        assert!(!inv.waiting());
        assert!(!inv.due(400.0 + IGNORED_BACKOFF_SECS.0 - 1.0, &mut rng));
        assert!(inv.due(400.0 + INVITE_WAIT_SECS + IGNORED_BACKOFF_SECS.1 + 1.0, &mut rng));
    }

    #[test]
    fn playing_along_brings_the_usual_wait() {
        let mut rng = fastrand::Rng::with_seed(2);
        let mut inv = Invites::default();
        inv.due(0.0, &mut rng);
        inv.invited(10.0);
        inv.answered(12.0, &mut rng);
        assert!(!inv.waiting());
        inv.tick(100.0, &mut rng);
        assert!(!inv.due(12.0 + INVITE_EVERY_SECS.0 - 1.0, &mut rng));
        assert!(inv.due(12.0 + INVITE_EVERY_SECS.1 + 1.0, &mut rng));
        // 주사위가 아니면 다음 간격까지
        inv.skip(1000.0, &mut rng);
        assert!(!inv.due(1001.0, &mut rng));
    }
}
