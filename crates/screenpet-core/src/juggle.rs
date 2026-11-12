//! 🤹 물건 들어 던지기 (pet.json `juggler` — 흐물이): 숫자와 순수 계산. 동물의 상태는 brain_juggle.rs, 창은 src-tauri objects.rs.
//!
//! - 줍기: 걷거나 쉬다가 같은 디딤판에 놓인 물건·똥을 처음 알아채면 PICK_CHANCE로 다가가 쪼그려 앉았다가 두 손으로 머리 위로 번쩍.
//! - 던지기: 가까이(AIM_RANGE) 다른 동물이 있으면 그쪽으로 포물선 (AIM_HIT_CHANCE쯤 맞는다 — 맞으면 평소의 콩), 없으면 아무 쪽으로 위로 휙.
//!   던진 물건은 사람이 던진 것과 똑같다: 친구가 연결된 화면 끝을 넘으면 친구에게 날아가고, 동물에 맞으면 콩.
//! - 들고 돌아다니기: 바로 던지지 않고 WANDER_CHANCE로 머리 위에 인 채 WANDER_SECS 동안 화면을 돌아다닌 뒤 던진다.
//! - 배달 장난: 새 버전 친구가 연결된 끝까지 걸어갈 수 있으면 PRANK_CHANCE로 들고 그 친구에게 놀러 가서, 도착하면 앞을 보고 번쩍 든 채
//!   잠깐 섰다가 휙 떨군다 (Visit.carry — 옛 버전 친구에게는 하지 않는다). 못 들어가 돌아오면 들고 와서 집에 떨군다.
//! - 🪨💩 거대한 것: "끙차" 힘을 주다가 GIANT_LIFT_CHANCE로 들어 올려 가까이 툭 던지고, 아니면 너무 무거워 털썩 주저앉았다가 일어나 간다
//!   (물건은 그 자리 그대로). 친구에게 들고 가지 않는다.
//!
//! 주사위는 이 동물만 자기 주사위(JuggleMood.rng)로 굴린다 — 다른 동물의 주사위 순서는 그대로.

use crate::physics::THROW_GRAVITY_LOGICAL;
use crate::surfaces::Owner;

/// 처음 알아챈 물건마다 주우러 갈 확률
pub const PICK_CHANCE: f64 = 0.6;
/// 알아채는 거리 (몸 크기 배, 같은 디딤판 위)
pub const NOTICE_REACH: f64 = 4.0;
/// 같은 디딤판으로 보는 높이 차 (px)
pub const SAME_LEVEL_PX: f64 = 4.0;
/// 같은 물건은 이만큼 다시 주사위를 굴리지 않는다 (초)
pub const SAME_OBJECT_SECS: f64 = 45.0;
/// 한 번 던지고(또는 주저앉고) 나면 이만큼은 다른 물건을 줍지 않는다 (초)
pub const CALM_SECS: (f64, f64) = (12.0, 25.0);
/// 다가가다 이만큼 못 닿으면 그만둔다 (초)
pub const APPROACH_SECS: f64 = 6.0;
/// 쪼그려 앉았다가 머리 위로 번쩍 드는 시간 (그림 lift 4칸 8fps 한 번 + 잠깐)
pub const LIFT_SECS: f64 = 0.6;
/// 든 채 잠깐 서 있는 시간
pub const HOLD_SECS: (f64, f64) = (0.3, 0.8);
/// 바로 던지지 않고 들고 돌아다닐 확률 (배달 장난이 아닐 때)
pub const WANDER_CHANCE: f64 = 0.45;
/// 들고 돌아다니는 시간
pub const WANDER_SECS: (f64, f64) = (4.0, 10.0);
/// 들고 돌아다니는 빠르기 (걷기 배)
pub const WANDER_PACE: f64 = 1.3;
/// 배달 장난: 새 버전 친구가 연결된 끝까지 걸어갈 수 있을 때 주운 것을 들고 놀러 갈 확률
pub const PRANK_CHANCE: f64 = 0.45;
/// 🎯 모니터 너머로 던지기: 친구 끝이 있을 때 주운 것을 그 끝 쪽으로 걸어가 친구 화면으로 휙 던질 확률 (배달 장난 다음 주사위)
pub const LOB_CHANCE: f64 = 0.3;
/// 던지기 전에 화면 끝에서 이만큼 (몸 크기 배) 안으로 다가간다
pub const LOB_DIST: f64 = 5.0;
/// 모니터 너머로 던질 때 위로 솟는 속도 (논리 px/s)
pub const LOB_VY: f64 = 900.0;

/// 🎯 (fx)에서 화면 끝 edge_x 너머로 떨어지게 던지는 속도 (물리 px/s): 높이 솟아 같은 높이로 돌아올 때까지 끝을 넉넉히 넘는다
pub fn lob_velocity(fx: f64, edge_x: f64, dpi: f64) -> (f64, f64) {
    let g = crate::physics::THROW_GRAVITY_LOGICAL * dpi;
    let vy = LOB_VY * dpi;
    let air = 2.0 * vy / g;
    let d = (edge_x - fx).abs();
    let vx = (d * 1.3 / air).max(300.0 * dpi).min(crate::physics::MAX_THROW_SPEED_LOGICAL * dpi * 0.9);
    (vx * (edge_x - fx).signum(), -vy)
}
/// 거대한 것에 "끙차" 힘주는 시간
pub const STRAIN_SECS: f64 = 1.6;
/// 거대한 것을 끝내 들어 올릴 확률 (아니면 털썩 주저앉는다)
pub const GIANT_LIFT_CHANCE: f64 = 0.6;
/// 털썩 주저앉아 있는 시간
pub const SLUMP_SECS: f64 = 2.2;
/// 던지기: 뒤로 젖혔다가 (THROW_RELEASE초에) 놓고, THROW_SECS에 끝난다 (그림 throw 4칸 10fps 한 번)
pub const THROW_SECS: f64 = 0.6;
pub const THROW_RELEASE: f64 = 0.2;
/// 배달: 앞을 보고 번쩍 든 채 서 있다가 (PRESENT_RELEASE초에) 휙 떨구고, PRESENT_SECS에 끝난다 (그림 present 4칸 3fps 한 번)
pub const PRESENT_SECS: f64 = 1.8;
pub const PRESENT_RELEASE: f64 = 1.0;
/// 노릴 수 있는 동물: 가로 거리 (내 몸 크기 배), 높이 차 (내 몸 크기 배)
pub const AIM_RANGE: f64 = 7.0;
pub const AIM_RISE: f64 = 3.0;
/// 노린 동물에 맞히려는 몫 (나머지는 머리 너머로 빗나간다)
pub const AIM_HIT_CHANCE: f64 = 0.5;
/// 빗나갈 땐 노린 동물 너머 이만큼 (그 동물 크기 배)에 떨어지게 던진다
const MISS_BEYOND: (f64, f64) = (1.4, 2.4);
/// 아무도 없을 때의 던지기 (논리 px/s): 가로, 위로
pub const RANDOM_THROW_VX: (f64, f64) = (220.0, 520.0);
pub const RANDOM_THROW_VY: (f64, f64) = (380.0, 650.0);
/// 거대한 것은 무거워서 가까이 툭 (논리 px/s)
pub const GIANT_THROW_VX: (f64, f64) = (60.0, 140.0);
pub const GIANT_THROW_VY: (f64, f64) = (150.0, 250.0);
/// 배달해 떨굴 때 살짝 앞으로·위로 (논리 px/s)
pub const FLICK_VX: (f64, f64) = (60.0, 140.0);
pub const FLICK_VY: f64 = 160.0;

/// 바닥에 놓인 물건 (들렸거나 날거나 누가 들고 있는 것은 아니다) — 앱이 틱마다 넘긴다
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ObjectSpot {
    pub id: u64,
    /// 바닥 가운데 x, 바닥 y (물리 px)
    pub x: f64,
    pub y: f64,
    pub owner: Owner,
    /// 몸 폭 절반 (물리 px)
    pub half_w: f64,
    /// 🪨💩 거대한 것
    pub giant: bool,
    /// 이 컴퓨터 밖으로 나가지 않는 것 (📌 작업표시줄에서 꺼낸 앱 아이콘 장난감 — taskbar.rs): 친구에게 들고 가지 않는다
    pub local: bool,
}

/// 같은 화면의 다른 동물 (발 x, 발 y, 그림 한 변 — 물리 px). 맞을 수 있는 동물만 넘긴다
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Target {
    pub x: f64,
    pub y: f64,
    pub size: f64,
}

/// 앱에 알릴 일 (take_juggle로 한 번 꺼낸다)
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum JuggleEvent {
    /// 손에서 놓았다: 이 속도로 날아간다 (물리 px/s) — 그 뒤는 사람이 던진 것과 같다
    Release { id: u64, vx: f64, vy: f64 },
    /// 📌 작업표시줄에서 장난감을 끌어내는 중: 앱이 열린 앱 아이콘 하나로 장난감을 만들어 `hold_toy`로 쥐여 준다 (못 만들면 그만이다 — taskbar.rs)
    Pull,
    /// 📌 장난감을 작업표시줄에 도로 밀어 넣었다: 앱이 이 물건을 뿅 치운다
    Stow { id: u64 },
    /// ⚾ 친구 화면에서 날아온 것을 받았다 (brain_rally.rs): 앱은 날던 이 물건을 손에 쥐여 준다 (그 뒤는 carried_object로 손 자리를 따라간다)
    Catch { id: u64 },
}

/// ⚾ 캐치볼: 이만큼 주고받으면 끝 ("끝!"). P2P 메시지와 같은 값임
pub use screenpet_proto::MAX_RALLY;
/// ⚾ 받는 그림 catch (4칸 10fps 한 번)
pub const CATCH_SECS: f64 = 0.4;
/// ⚾ 받으러 달려가다 이만큼 지나도 못 받으면 그만둔다
pub const CHASE_SECS: f64 = 4.0;
/// ⚾ 손(머리 위)에서 물건 바닥이 몸 이만큼 안이면 받는다
pub const CATCH_REACH: f64 = 0.6;

/// ⚾ 친구 화면 끝에서 던져져 날아 들어오는 물건 (앱이 틱마다 넘긴다 — notice_incoming)
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Incoming {
    pub id: u64,
    /// 물건 바닥 가운데 (물리 px)와 속도 (물리 px/s)
    pub x: f64,
    pub y: f64,
    pub vx: f64,
    pub vy: f64,
    /// 주고받은 횟수 (처음 던진 것 = 0)
    pub rally: u32,
    /// 날아 들어온 끝 (되던질 쪽)
    pub from: crate::world::Side,
    pub giant: bool,
}

/// (x, y)에서 (vx, vy)로 던져진 것이 내려오며 높이 y_at에 닿는 x (던진 비행 중력 — 물리 px). 그 높이에 닿지 않으면 None
pub fn descent_x(x: f64, y: f64, vx: f64, vy: f64, y_at: f64, dpi: f64) -> Option<(f64, f64)> {
    let g = THROW_GRAVITY_LOGICAL * dpi;
    // y + vy t + g t²/2 = y_at → 큰 근
    let (a, b, c) = (0.5 * g, vy, y - y_at);
    let disc = b * b - 4.0 * a * c;
    if disc < 0.0 || a <= 0.0 {
        return None;
    }
    let t = (-b + disc.sqrt()) / (2.0 * a);
    (t > 0.0).then_some((x + vx * t, t))
}

/// 그림 한 동작의 칸별 손 자리 (그림 가운데에서 오른쪽, 그림 윗변에서 아래 — 한 변에 대한 비율)
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GripAnim {
    pub pts: [(f64, f64); 4],
    pub n: usize,
    pub fps: f64,
    pub looping: bool,
}

/// 표가 없을 때의 손 자리: 머리 위
pub const DEFAULT_GRIP: (f64, f64) = (0.05, 0.1);

impl Default for GripAnim {
    fn default() -> Self {
        GripAnim { pts: [DEFAULT_GRIP; 4], n: 0, fps: 1.0, looping: true }
    }
}

impl GripAnim {
    /// 그 동작을 시작한 지 t초 뒤의 손 자리 (웹뷰 frameIndex와 같은 칸)
    pub fn at(&self, t: f64) -> (f64, f64) {
        if self.n == 0 {
            return DEFAULT_GRIP;
        }
        let k = (t.max(0.0) * self.fps).floor() as usize;
        let i = if self.looping { k % self.n } else { k.min(self.n - 1) };
        self.pts[i]
    }
}

/// 손에 든 물건을 그리는 동작들의 손 자리
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct GripTable {
    pub lift: GripAnim,
    pub hold: GripAnim,
    pub carry: GripAnim,
    pub throw: GripAnim,
    pub present: GripAnim,
    /// 📌 작업표시줄에서 끌어내기 (손이 바닥께) · 도로 밀어 넣기
    pub yank: GripAnim,
    pub stuff: GripAnim,
    /// ⚾ 날아온 것 받기 (머리 위) · 🎣 커서 끝을 잡은 두 손 (모든 칸이 같은 점 — 흔드는 축)
    pub catch: GripAnim,
    pub hang_cursor: GripAnim,
    /// 들고 달리기 (친구에게 들고 가는 길)
    pub carry_run: GripAnim,
}

/// 가장 가까운 노릴 동물: 가로 AIM_RANGE, 높이 AIM_RISE (내 몸 크기 배) 안에서
pub fn nearest_target(from: (f64, f64), size: f64, targets: &[Target]) -> Option<Target> {
    targets
        .iter()
        .filter(|t| (t.x - from.0).abs() <= AIM_RANGE * size && (t.y - from.1).abs() <= AIM_RISE * size && (t.x - from.0).abs() > 1.0)
        .min_by(|a, b| (a.x - from.0).abs().total_cmp(&(b.x - from.0).abs()))
        .copied()
}

/// from(손에 든 물건 바닥)에서 (x, y)에 닿는 포물선: 비행 시간은 거리에 따라 0.5~1.0초. g = 물리 px/s²
fn arc_to(from: (f64, f64), to: (f64, f64), reach: f64, g: f64) -> (f64, f64) {
    let dx = to.0 - from.0;
    let secs = (0.5 + 0.5 * (dx.abs() / reach.max(1.0))).clamp(0.5, 1.0);
    let vx = dx / secs;
    let vy = (to.1 - from.1 - 0.5 * g * secs * secs) / secs;
    (vx, vy)
}

/// 노린 동물에게 던지는 속도 (물리 px/s): hit이면 몸 가운데로, 아니면 머리 너머 바닥으로. beyond = 너머 거리 (그 동물 크기 배)
pub fn aim_at(from: (f64, f64), target: Target, hit: bool, beyond: f64, size: f64, dpi: f64) -> (f64, f64) {
    let g = THROW_GRAVITY_LOGICAL * dpi;
    let dir = if target.x >= from.0 { 1.0 } else { -1.0 };
    let to = if hit { (target.x, target.y - 0.45 * target.size) } else { (target.x + dir * beyond * target.size, target.y) };
    arc_to(from, to, AIM_RANGE * size, g)
}

fn range(rng: &mut fastrand::Rng, (lo, hi): (f64, f64)) -> f64 {
    lo + rng.f64() * (hi - lo)
}

/// 던질 속도 (물리 px/s)와 바라볼 쪽. 거대한 것은 바라보는 쪽으로 가까이 툭, 노릴 동물이 있으면 그쪽으로 (반쯤 맞힌다), 없으면 아무 쪽으로 위로
pub fn throw_velocity(rng: &mut fastrand::Rng, from: (f64, f64), facing: f64, size: f64, dpi: f64, giant: bool, targets: &[Target]) -> (f64, f64, f64) {
    if giant {
        return (facing * range(rng, GIANT_THROW_VX) * dpi, -range(rng, GIANT_THROW_VY) * dpi, facing);
    }
    if let Some(t) = nearest_target(from, size, targets) {
        let hit = rng.f64() < AIM_HIT_CHANCE;
        let beyond = range(rng, MISS_BEYOND);
        let (vx, vy) = aim_at(from, t, hit, beyond, size, dpi);
        return (vx, vy, if t.x >= from.0 { 1.0 } else { -1.0 });
    }
    let dir = if rng.bool() { 1.0 } else { -1.0 };
    (dir * range(rng, RANDOM_THROW_VX) * dpi, -range(rng, RANDOM_THROW_VY) * dpi, dir)
}

/// 배달해 떨구는 속도 (물리 px/s): 바라보는 쪽으로 살짝, 위로 조금
pub fn flick_velocity(rng: &mut fastrand::Rng, facing: f64, dpi: f64) -> (f64, f64) {
    (facing * range(rng, FLICK_VX) * dpi, -FLICK_VY * dpi)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn grip(n: usize, fps: f64, looping: bool) -> GripAnim {
        GripAnim { pts: [(0.0, 0.1), (0.1, 0.2), (0.2, 0.3), (0.3, 0.4)], n, fps, looping }
    }

    #[test]
    fn grip_follows_the_webview_frame_index() {
        let g = grip(4, 10.0, false);
        assert_eq!(g.at(0.0), (0.0, 0.1));
        assert_eq!(g.at(0.15), (0.1, 0.2));
        assert_eq!(g.at(5.0), (0.3, 0.4), "한 번만 도는 동작은 마지막 칸에 머문다");
        let g = grip(2, 10.0, true);
        assert_eq!(g.at(0.25), (0.0, 0.1), "반복");
        assert_eq!(GripAnim::default().at(1.0), DEFAULT_GRIP);
    }

    #[test]
    fn nearest_target_is_within_range_only() {
        let t = |x: f64, y: f64| Target { x, y, size: 100.0 };
        let from = (500.0, 900.0);
        assert_eq!(nearest_target(from, 100.0, &[t(1300.0, 1000.0), t(300.0, 1000.0), t(800.0, 1000.0)]), Some(t(300.0, 1000.0)));
        assert_eq!(nearest_target(from, 100.0, &[t(1300.0, 1000.0)]), None, "너무 멀다");
        assert_eq!(nearest_target(from, 100.0, &[t(600.0, 300.0)]), None, "너무 높다");
    }

    #[test]
    fn a_thrown_arc_reaches_the_aimed_point() {
        let from = (100.0, 800.0);
        let target = Target { x: 500.0, y: 1000.0, size: 100.0 };
        for (hit, want) in [(true, (500.0, 955.0)), (false, (700.0, 1000.0))] {
            let (vx, vy) = aim_at(from, target, hit, 2.0, 100.0, 1.0);
            let g = THROW_GRAVITY_LOGICAL;
            let t = (want.0 - from.0) / vx;
            let y = from.1 + vy * t + 0.5 * g * t * t;
            assert!((y - want.1).abs() < 0.5, "{hit}: {y}");
        }
    }

    #[test]
    fn giants_go_a_short_way_and_random_throws_go_up_either_way() {
        let mut rng = fastrand::Rng::with_seed(1);
        let mut dirs = [0; 2];
        for _ in 0..200 {
            let (vx, vy, _) = throw_velocity(&mut rng, (0.0, 0.0), 1.0, 100.0, 1.0, true, &[]);
            assert!(vx > 0.0 && vx <= GIANT_THROW_VX.1 && vy < 0.0 && -vy <= GIANT_THROW_VY.1);
            let (vx, vy, face) = throw_velocity(&mut rng, (0.0, 0.0), 1.0, 100.0, 1.0, false, &[]);
            assert!(vy <= -RANDOM_THROW_VY.0 && vx.abs() >= RANDOM_THROW_VX.0 && vx.signum() == face);
            dirs[usize::from(vx > 0.0)] += 1;
        }
        assert!(dirs[0] > 60 && dirs[1] > 60, "{dirs:?}");
    }
}
