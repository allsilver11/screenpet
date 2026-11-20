//! 🕷️ 거미인간 줄 놀이 (pet.json `web_slinger` — 거미인간): 숫자와 순수 계산. 동물의 상태는 brain_web.rs, 그림은 src-tauri trail.rs.
//!
//! - 거미집: 한가할 때 가까운 모서리(작업 영역 네 모서리 + 앞에 가려지지 않은 창의 위 두 모서리)를 향해 줄을 쏘고, 줄이 닿으면 거미집이 핌.
//!   손님으로 친구 화면에 가 있으면 자주 (WEB_RATE_GUEST), 집에선 가끔 (WEB_RATE_HOME). 거미집은 앱이 WEB_LIFE_SECS 뒤 흐리게 지움.
//!   크기는 제각각 (WEB_SIZE_RANGE)이고, 줄을 여러 번 휙휙 쏴서 짬 (WEAVE_SHOTS·weave_target — 앱이 살·고리를 차례로 그림).
//!   🌱 크기·줄 수와 그네·끌어오기·고치 빈도는 연령대로 자람 (stage_power × WEB_SIZE_RANGE, shot_range — 아기 작게 2~4번 → 어른 크게 5~8번)
//!   창 모서리 거미집은 창을 따라 옮겨 가고, 앞 창에 가려지면 숨고, 창이 닫히면 사라짐 (web_on_window).
//! - 그네: 머리 위 창 아랫변이나 화면 윗변에 줄을 붙여 몸을 끌어올린 뒤 진자처럼 흔들려 반대편에서 놓고 날아감
//! - 끌어오기: 바닥의 물건이나 다른 동물에 줄을 붙여 휙 — 반은 앞에 떨어지게, 반은 머리 너머 뒤로
//! - 🌀 휘둘러 던지기: 친구가 연결된 화면 끝이 있으면 (WHIRL_CHANCE) 끌어오는 대신 물건을 줄로 낚아 머리 위로 빙빙 돌리다가
//!   그 끝 너머로 휙 — 끝을 넘은 물건은 앱의 던지기처럼 친구 모니터로 날아감 (whirl_point·whirl_flight_secs)
//! - 영웅 착지: 높이서 떨어져 내려서면 콰당 대신 한 손 짚고 쭉 (HERO_FALL_SIZES)
//! - 🐛 고치 말기: 가까운 다른 동물 몸 둘레로 줄을 COCOON_SHOTS번 쏴서 칭칭 (cocoon_target) — 감긴 동물은 COCOON_SECS 버둥대다 뿅
//! - 🕸️ 거미집에 걸리기: 거미인간이 아닌 동물의 몸이 거미집 사분원 안 (in_web)으로 들어가면 SNARE_CHANCE로 걸려 SNARE_SECS 버둥댐 —
//!   빠져나오면 그 거미집은 찢어짐 (앱이 지움). 빠져나온 뒤 SNARE_IMMUNE_SECS 동안은 다시 안 걸림. 거미집 자리는 앱이 WebSpot으로 알려 줌
//!
//! 단위: 자리·길이는 물리 px, 시간은 초.

use serde::Serialize;

use crate::geom::{Rect, WindowInfo};
use crate::world::World;

/// 줄이 손에서 목표까지 뻗어 나가는 시간 (shoot 3칸 10fps)
pub const SHOOT_SECS: f64 = 0.3;
/// 줄이 닿은 뒤 뻗은 채 있는 시간
pub const SHOOT_HOLD_SECS: f64 = 0.35;
/// 앱을 켜거나 친구 화면에 들어오고 처음 놀기까지
pub const FIRST_READY_SECS: f64 = 6.0;

/// 집에서 거미집을 칠 빈도 (초당 확률)
pub const WEB_RATE_HOME: f64 = 1.0 / 30.0;
/// 손님으로 친구 화면에 가 있을 때 거미집을 칠 빈도 (초당 확률) — 여기저기 자주
pub const WEB_RATE_GUEST: f64 = 1.0 / 10.0;
/// 거미집을 친 뒤 다시 치기까지 (집)
pub const WEB_CALM_HOME: (f64, f64) = (40.0, 100.0);
/// 거미집을 친 뒤 다시 치기까지 (손님)
pub const WEB_CALM_GUEST: (f64, f64) = (8.0, 20.0);
/// 모서리를 고르는 거리 (몸 크기 배)
pub const WEB_REACH: f64 = 9.0;
/// 거미집 반지름 (몸 크기 배) — 모서리 고를 때의 기본값 (실제로 칠 때는 WEB_SIZE_RANGE에서 고름)
pub const WEB_SIZE: f64 = 0.8;
/// 실제로 칠 거미집 반지름 (몸 크기 배): 작은 것부터 꽤 큰 것까지 제각각
pub const WEB_SIZE_RANGE: (f64, f64) = (0.6, 2.6);
/// 창 모서리 거미집은 창의 짧은 변의 이만큼까지만
pub const WEB_WINDOW_FIT: f64 = 0.6;
/// 거미집 하나를 짜려고 줄을 쏘는 횟수 (작은 것 ~ 큰 것)
pub const WEAVE_SHOTS: (u32, u32) = (4, 7);
/// 🌱 연령대 세기(stage_power)에 맞춘 줄 횟수 범위 — 청소년(1.0)은 WEAVE_SHOTS 그대로, 아기 (2, 4) → 어른 (5, 8)
pub fn shot_range(power: f64) -> (u32, u32) {
    let lo = ((f64::from(WEAVE_SHOTS.0) * power).round() as u32).max(2);
    let hi = ((f64::from(WEAVE_SHOTS.1) * power).round() as u32).max(lo);
    (lo, hi)
}
/// 거미집 짜는 줄 한 번 (쏘고 닿아 있는 시간)
pub const WEAVE_SHOT_SECS: f64 = 0.32;
/// 짜는 줄이 노리는 자리: (모서리 각의 몫 0..1, 반지름의 몫) — 두 번째 줄부터 차례로 (첫 줄은 모서리)
const WEAVE_AIMS: [(f64, f64); 6] = [(0.0, 1.0), (1.0, 1.0), (0.5, 1.0), (0.25, 0.7), (0.75, 0.7), (0.5, 0.45)];
/// 이보다 작은 창의 모서리에는 치지 않음 (몸 크기 배, 가로·세로)
pub const MIN_WINDOW: f64 = 2.0;
/// 앱이 거미집을 남겨 두는 시간 (그 뒤 흐려져 사라짐)
pub const WEB_LIFE_SECS: f64 = 480.0;
/// 한 화면에 남는 거미집 수 상한 (넘으면 오래된 것부터)
pub const MAX_WEBS: usize = 12;

/// 그네를 탈 빈도 (초당 확률)
pub const SWING_RATE: f64 = 1.0 / 20.0;
/// 그네를 탄 뒤 다시 타기까지
pub const SWING_CALM: (f64, f64) = (20.0, 50.0);
/// 그네 시작 각도 (도, 수직에서 뒤로)
pub const SWING_ANGLE_DEG: f64 = 48.0;
/// 반대편에서 이만큼 (시작 각도의 몫) 올라갔을 때 놓음
pub const RELEASE_FRAC: f64 = 0.8;
/// 줄이 짧으면 (몸 크기 배) 그네를 타지 않음
pub const SWING_MIN_LEN: f64 = 2.5;
/// 진자 맨 아래에서도 발이 바닥보다 이만큼 (몸 크기 배) 위
pub const SWING_CLEAR: f64 = 0.35;
/// 줄을 당겨 그네 시작 자리로 몸을 끌어올리는 시간
pub const ZIP_SECS: f64 = 0.35;
/// 줄은 이보다 길지 않음 (몸 크기 배) — 높은 화면에서도 한 번에 화면을 가로지를 만큼
pub const SWING_MAX_LEN: f64 = 7.0;
/// 그네가 이만큼 넘게 걸리면 그 자리에서 놓음 (안전 시한)
pub const SWING_MAX_SECS: f64 = 6.0;

/// 끌어오기를 할 빈도 (닿는 거리에 무언가 있을 때, 초당 확률)
pub const PULL_RATE: f64 = 1.0 / 40.0;
/// 끌어온 뒤 다시 하기까지
pub const PULL_CALM: (f64, f64) = (30.0, 75.0);
/// 끌어올 수 있는 거리 (몸 크기 배): 가까운 끝, 먼 끝
pub const PULL_REACH: (f64, f64) = (1.4, 8.0);
/// 끌어당기는 그림 (web_pull) 시간 — 줄이 손으로 감겨 들어옴
pub const PULL_SECS: f64 = 0.5;
/// 끌어온 것이 날아오는 시간
pub const YANK_FLIGHT_SECS: f64 = 0.55;
/// 앞에 떨어지게 끌어올 때: 몸 가운데에서 앞으로 (몸 크기 배)
pub const PULL_LAND_AHEAD: f64 = 0.65;
/// 머리 너머 뒤로 던질 때: 몸 가운데에서 뒤로 (몸 크기 배)
pub const FLING_LAND_BEHIND: f64 = 2.4;

/// 🌀 친구가 연결된 끝이 있을 때 물건을 끌어오는 대신 휘둘러 던질 몫
pub const WHIRL_CHANCE: f64 = 0.6;
/// 🌀 줄이 닿은 뒤 휘두르는 시간 (감아올리기 포함 — 끝나면 놓음)
pub const WHIRL_SECS: f64 = 1.4;
/// 🌀 바닥의 물건을 머리 위 원으로 감아올리는 시간
pub const WHIRL_REEL_SECS: f64 = 0.25;
/// 🌀 휘두르는 바퀴 수 (마지막에 원 꼭대기에서 던질 쪽으로 나감)
pub const WHIRL_TURNS: f64 = 2.5;
/// 🌀 휘두르는 원의 가로 반지름 (몸 크기 배)
pub const WHIRL_RADIUS: f64 = 1.1;
/// 🌀 옆에서 본 원의 납작함 (세로 반지름 ÷ 가로 반지름 — 머리 위에서 눕혀 도는 올가미)
pub const WHIRL_FLAT: f64 = 0.3;
/// 🌀 원 가운데가 손보다 이만큼 위 (몸 크기 배)
pub const WHIRL_LIFT: f64 = 0.45;
/// 🌀 던진 물건이 바닥 높이에 닿을 자리: 화면 끝에서 이만큼 너머 (몸 크기 배)
pub const WHIRL_PAST: f64 = 3.0;
/// 🌀 날아가는 시간: 가장 짧게, 가장 길게
pub const WHIRL_FLIGHT_SECS: (f64, f64) = (0.6, 1.3);
/// 🌀 몸 크기 하나만큼 멀 때마다 더 나는 시간
pub const WHIRL_FLIGHT_PER_SIZE: f64 = 0.04;

/// 이만큼 (몸 크기 배) 넘게 떨어져 내려서면 영웅 착지
pub const HERO_FALL_SIZES: f64 = 1.6;
/// 영웅 착지 자세로 있는 시간 (hero_land 3칸 8fps + 잠깐)
pub const HERO_SECS: f64 = 0.9;

/// 🐛 고치를 말 빈도 (닿는 거리에 쉬는 동물이 있을 때, 초당 확률) — 끌어오기와 비슷하게 가끔
pub const COCOON_RATE: f64 = 1.0 / 40.0;
/// 🐛 고치를 만 뒤 다시 말기까지
pub const COCOON_CALM: (f64, f64) = (45.0, 100.0);
/// 🐛 고치를 말 수 있는 거리 (몸 크기 배): 가까운 끝, 먼 끝
pub const COCOON_REACH: (f64, f64) = (1.0, 6.0);
/// 🐛 고치를 말려고 쏘는 줄 수 (한 줄 = WEAVE_SHOT_SECS, 줄마다 실 한 겹)
pub const COCOON_SHOTS: u32 = 4;
/// 🐛 다 감긴 뒤 버둥대는 시간 (그 뒤 뿅 풀려남)
pub const COCOON_SECS: (f64, f64) = (3.5, 5.5);
/// 🐛 고치 줄이 노리는 몸 둘레 자리: (가로 — 몸 크기 배, 발에서 위로 — 몸 크기 배) 차례로
const COCOON_AIMS: [(f64, f64); 5] = [(-0.3, 0.35), (0.3, 0.6), (-0.25, 0.8), (0.3, 0.3), (0.0, 0.55)];
/// 🕸️ 거미집 안으로 들어갈 때마다 걸릴 몫
pub const SNARE_CHANCE: f64 = 0.6;
/// 🕸️ 걸려서 버둥대는 시간 (그 뒤 찢고 빠져나옴)
pub const SNARE_SECS: (f64, f64) = (3.0, 5.0);
/// 🕸️ 빠져나온 뒤 다시 안 걸리는 시간
pub const SNARE_IMMUNE_SECS: f64 = 8.0;
/// 🐛🕸️ 버둥대며 말하는 간격 ("mmph!" / "help!")
pub const STUCK_TALK_SECS: f64 = 1.6;

/// 🐛 고치를 마는 k번째 줄 (0부터)이 노리는 자리 (물리 px): 발(feet) 위 몸 둘레를 번갈아. size = 그 동물 몸 크기
pub fn cocoon_target(feet: (f64, f64), size: f64, k: u32) -> (f64, f64) {
    let (dx, up) = COCOON_AIMS[k as usize % COCOON_AIMS.len()];
    (feet.0 + dx * size, feet.1 - up * size)
}

/// 🕸️ 화면에 있는 거미집 하나 (앱이 Pet::set_webs로 알려 줌 — 물리 px)
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WebSpot {
    /// 앱의 거미집 번호 (찢어지면 Pet::take_torn_web이 돌려줌)
    pub id: u64,
    /// 모서리 점
    pub x: f64,
    pub y: f64,
    /// 어느 모서리 (사분원이 그 안쪽으로)
    pub corner: Corner,
    /// 반지름
    pub size: f64,
    /// 앞 창에 가려짐 (안 걸림)
    pub hidden: bool,
}

/// 🕸️ 점 (px, py)가 모서리 (x, y)에서 안쪽으로 반지름 radius만큼 펼쳐진 거미집 사분원 안 (모서리 바깥쪽은 아님)
pub fn in_web(px: f64, py: f64, x: f64, y: f64, corner: Corner, radius: f64) -> bool {
    if !(radius > 0.0) {
        return false;
    }
    let right = matches!(corner, Corner::TopLeft | Corner::BottomLeft);
    let down = matches!(corner, Corner::TopLeft | Corner::TopRight);
    let x_in = if right { px >= x } else { px <= x };
    let y_in = if down { py >= y } else { py <= y };
    x_in && y_in && (px - x).hypot(py - y) <= radius
}

/// 거미집이 붙은 모서리 (그림이 모서리 안쪽으로 펼쳐짐)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Corner {
    TopLeft,
    TopRight,
    BottomLeft,
    BottomRight,
}

/// 새 거미집 하나 (Pet::take_web으로 한 번 꺼냄)
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WebSpin {
    /// 모서리 점 x (물리 px)
    pub x: f64,
    /// 모서리 점 y (물리 px)
    pub y: f64,
    /// 어느 모서리
    pub corner: Corner,
    /// 거미집 반지름 (물리 px)
    pub size: f64,
    /// 창 모서리면 그 창 id (창이 움직이거나 닫히면 앱이 지움), 화면 모서리면 None
    pub window: Option<u64>,
    /// 줄을 쏘아 짜는 횟수 (1이면 한 번에 핌)
    pub shots: u32,
    /// 다 짜는 데 걸리는 시간 (초) — 앱이 그동안 살·고리를 차례로 그림 (0이면 바로 핌)
    pub build: f64,
}

/// 🕸️ 거미집을 짜는 k번째 줄 (1부터 — 0번째는 모서리)이 노리는 자리 (물리 px): 부채꼴 살 끝 → 안쪽 고리 차례
pub fn weave_target(spot: &WebSpin, k: u32) -> (f64, f64) {
    let (frac, rad) = WEAVE_AIMS[(k.max(1) as usize - 1) % WEAVE_AIMS.len()];
    let (sx, sy) = match spot.corner {
        Corner::TopLeft => (1.0, 1.0),
        Corner::TopRight => (-1.0, 1.0),
        Corner::BottomLeft => (1.0, -1.0),
        Corner::BottomRight => (-1.0, -1.0),
    };
    let ang = std::f64::consts::FRAC_PI_2 * frac;
    let r = spot.size * rad;
    (spot.x + sx * r * ang.cos(), spot.y + sy * r * ang.sin())
}

/// 지금 뻗어 있는 줄 한 가닥 (물리 px) — 앱이 덮개 창에 그림
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WebLine {
    /// 손 자리
    pub from: (f64, f64),
    /// 줄 끝
    pub to: (f64, f64),
}

/// 그네 닻 (줄을 붙인 점)
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Anchor {
    /// 닻 x (물리 px)
    pub x: f64,
    /// 닻 y (물리 px) — 창 아랫변 또는 화면 윗변
    pub y: f64,
    /// 줄 길이 (닻 → 손, 물리 px)
    pub len: f64,
    /// 시작 각도 (rad, 수직에서 진행 반대쪽으로 — 늘 양수)
    pub angle: f64,
}

/// 점이 이 창 사각형 안 (가장자리 제외)
fn inside(r: &Rect, x: f64, y: f64) -> bool {
    r.left < x && x < r.right && r.top < y && y < r.bottom
}

/// 창 모서리 거미집이 지금 있을 곳 (web_on_window)
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum WebAt {
    /// 그 창 모서리 (x, y)에 보임
    Shown(f64, f64),
    /// 그 창 모서리 (x, y)가 앞 창에 가려져 숨김 (가림이 걷히면 다시 보임)
    Hidden(f64, f64),
    /// 창이 닫혔거나 최소화됨 — 지움
    Gone,
}

/// 🕸️ 창(window) 위 모서리(corner)에 친 거미집을 지금 창 목록(앞 → 뒤)에 맞춤: 창을 옮기거나 크기를 바꾸면 새 모서리로 따라가고,
/// 앞 창이 그 모서리를 가리면 숨고, 창이 없으면 지움
pub fn web_on_window(windows: &[WindowInfo], window: u64, corner: Corner) -> WebAt {
    let Some(k) = windows.iter().position(|w| w.id == window) else { return WebAt::Gone };
    let r = windows[k].rect;
    let x = if matches!(corner, Corner::TopLeft | Corner::BottomLeft) { r.left } else { r.right };
    let y = if matches!(corner, Corner::TopLeft | Corner::TopRight) { r.top } else { r.bottom };
    let inward = (if x == r.left { 1.0 } else { -1.0 }, if y == r.top { 1.0 } else { -1.0 });
    let covered = windows[..k].iter().any(|f| inside(&f.rect, x + inward.0, y + inward.1) || inside(&f.rect, x, y + inward.1));
    if covered {
        WebAt::Hidden(x, y)
    } else {
        WebAt::Shown(x, y)
    }
}

/// at(손 자리)에서 reach(물리 px) 안의 거미집 자리: 작업 영역 네 모서리 + 앞의 창에 가려지지 않은 창 위 두 모서리.
/// 같은 모니터(at이 있는 작업 영역)만. size = 몸 크기 (물리 px)
pub fn corner_spots(world: &World, at: (f64, f64), reach: f64, size: f64) -> Vec<WebSpin>
{
    let wa = world.work_area_at(at.0, at.1);
    let web = WEB_SIZE * size;
    let mut out = vec![
        WebSpin { x: wa.left, y: wa.top, corner: Corner::TopLeft, size: web, window: None, shots: 1, build: 0.0 },
        WebSpin { x: wa.right, y: wa.top, corner: Corner::TopRight, size: web, window: None, shots: 1, build: 0.0 },
        WebSpin { x: wa.left, y: wa.bottom, corner: Corner::BottomLeft, size: web, window: None, shots: 1, build: 0.0 },
        WebSpin { x: wa.right, y: wa.bottom, corner: Corner::BottomRight, size: web, window: None, shots: 1, build: 0.0 },
    ];
    for (k, w) in world.windows.iter().enumerate() {
        let r = w.rect;
        if r.width() < MIN_WINDOW * size || r.height() < MIN_WINDOW * size {
            continue;
        }
        for (x, corner) in [(r.left, Corner::TopLeft), (r.right, Corner::TopRight)] {
            let on_screen = wa.left <= x && x <= wa.right && wa.top <= r.top && r.top < wa.bottom;
            // 앞의 창이 그 모서리를 가리면 안 보임 (창 목록은 앞 → 뒤)
            let covered = world.windows[..k].iter().any(|f| inside(&f.rect, x, r.top + 1.0) || inside(&f.rect, x + if corner == Corner::TopLeft { 1.0 } else { -1.0 }, r.top + 1.0));
            if on_screen && !covered {
                out.push(WebSpin { x, y: r.top, corner, size: web, window: Some(w.id), shots: 1, build: 0.0 });
            }
        }
    }
    out.retain(|s| (s.x - at.0).hypot(s.y - at.1) <= reach);
    out
}

/// 그네 닻: 손(hand)에서 facing 쪽 위의 창 아랫변 (손보다 SWING_MIN_LEN 넘게 위, 가장 가까운 것부터) 또는 화면 윗변.
/// 줄 길이는 진자 맨 아래에서 발이 바닥(floor)보다 SWING_CLEAR 위에 오게 (SWING_MAX_LEN까지) — 그래서 먼저 줄을 당겨 떠오름 (Zip).
/// 시작 각도는 SWING_ANGLE_DEG부터 줄여 가며 닻과 반대편에서 놓는 자리가 모두 화면 안에 드는 것. drop = 손에서 발까지 (물리 px). 없으면 None
pub fn swing_anchor(world: &World, hand: (f64, f64), facing: f64, size: f64, drop: f64, floor: f64) -> Option<Anchor> {
    let wa = world.work_area_at(hand.0, hand.1);
    let min = SWING_MIN_LEN * size;
    let mut tops: Vec<(f64, Option<Rect>)> =
        world.windows.iter().filter(|w| w.rect.bottom <= hand.1 - min && w.rect.bottom > wa.top).map(|w| (w.rect.bottom, Some(w.rect))).collect();
    tops.sort_by(|a, b| b.0.total_cmp(&a.0));
    tops.push((wa.top, None));
    for (top, win) in tops {
        let len = (floor - SWING_CLEAR * size - drop - top).min(SWING_MAX_LEN * size);
        if len < min {
            continue;
        }
        for deg in [SWING_ANGLE_DEG, 42.0, 36.0, 30.0, 24.0] {
            let angle = deg.to_radians();
            let ax = hand.0 + facing * len * angle.sin();
            let far = ax + facing * len * (RELEASE_FRAC * angle).sin();
            let held = win.is_none_or(|r| r.left < ax && ax < r.right);
            let room = (wa.left + 0.5 * size..=wa.right - 0.5 * size).contains(&ax) && (wa.left + size..=wa.right - size).contains(&far);
            if held && room {
                return Some(Anchor { x: ax, y: top, len, angle });
            }
        }
    }
    None
}

/// 진자 한 걸음 (θ'' = −g/L sinθ, 반암시적 오일러): (θ, ω) → 다음 (θ, ω)
pub fn pendulum_step(theta: f64, omega: f64, len: f64, gravity: f64, dt: f64) -> (f64, f64) {
    if len <= 0.0 {
        return (theta, omega);
    }
    let omega = omega - gravity / len * theta.sin() * dt;
    (theta + omega * dt, omega)
}

/// 진자의 손 자리: 닻에서 len, θ (facing 쪽이 +)
pub fn pendulum_hand(anchor: (f64, f64), len: f64, theta: f64, facing: f64) -> (f64, f64) {
    (anchor.0 + facing * len * theta.sin(), anchor.1 + len * theta.cos())
}

/// 놓는 순간의 속도 (물리 px/s): 접선 방향
pub fn release_velocity(len: f64, theta: f64, omega: f64, facing: f64) -> (f64, f64) {
    (facing * len * omega * theta.cos(), -len * omega * theta.sin())
}

/// 🌀 휘두르는 물건 가운데 자리: 손(hand) 위 WHIRL_LIFT에 가운데를 둔 납작한 원을 t초에 따라 돎 (dir 쪽으로 던질 방향).
/// WHIRL_SECS에 원 꼭대기에 와서 dir 쪽으로 움직임
pub fn whirl_point(hand: (f64, f64), size: f64, dir: f64, t: f64) -> (f64, f64) {
    let (rx, ry) = (WHIRL_RADIUS * size, WHIRL_RADIUS * WHIRL_FLAT * size);
    let left = 1.0 - (t / WHIRL_SECS).clamp(0.0, 1.0);
    let phi = -std::f64::consts::FRAC_PI_2 - dir * WHIRL_TURNS * std::f64::consts::TAU * left;
    (hand.0 + rx * phi.cos(), hand.1 - WHIRL_LIFT * size + ry * phi.sin())
}

/// 🌀 휘둘러 던진 물건이 가로 dx (물리 px)를 나는 시간 — 멀수록 길게 (WHIRL_FLIGHT_SECS 안)
pub fn whirl_flight_secs(dx: f64, size: f64) -> f64 {
    let (lo, hi) = WHIRL_FLIGHT_SECS;
    (lo + WHIRL_FLIGHT_PER_SIZE * dx.abs() / size.max(1.0)).clamp(lo, hi)
}

/// from에서 to로 secs 동안 포물선으로 날아가는 처음 속도 (중력 gravity, 물리 px/s²)
pub fn yank_velocity(from: (f64, f64), to: (f64, f64), secs: f64, gravity: f64) -> (f64, f64) {
    let t = secs.max(0.05);
    ((to.0 - from.0) / t, (to.1 - from.1 - 0.5 * gravity * t * t) / t)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn world(windows: Vec<WindowInfo>) -> World {
        World::new(windows, vec![Rect::new(0.0, 0.0, 1920.0, 1040.0)], 128.0)
    }

    fn win(id: u64, l: f64, t: f64, r: f64, b: f64) -> WindowInfo {
        WindowInfo { id, rect: Rect::new(l, t, r, b) }
    }

    /// 🕸️ 창 모서리 거미집: 창을 옮기거나 크기를 바꾸면 그 모서리를 따라가고, 창이 닫히면 사라지고, 앞 창에 가려지면 숨음
    #[test]
    fn a_window_web_follows_its_corner_hides_when_covered_and_goes_when_the_window_does() {
        let w = vec![win(1, 300.0, 300.0, 1000.0, 800.0)];
        assert_eq!(web_on_window(&w, 1, Corner::TopLeft), WebAt::Shown(300.0, 300.0));
        assert_eq!(web_on_window(&w, 1, Corner::TopRight), WebAt::Shown(1000.0, 300.0));
        // 옮김·크기 바꿈: 새 모서리
        let moved = vec![win(1, 500.0, 120.0, 1400.0, 900.0)];
        assert_eq!(web_on_window(&moved, 1, Corner::TopLeft), WebAt::Shown(500.0, 120.0));
        assert_eq!(web_on_window(&moved, 1, Corner::TopRight), WebAt::Shown(1400.0, 120.0));
        // 닫힘·최소화 (목록에 없음)
        assert_eq!(web_on_window(&[win(2, 0.0, 0.0, 100.0, 100.0)], 1, Corner::TopLeft), WebAt::Gone);
        // 앞 창(목록에서 앞)이 그 모서리를 가리면 숨음 — 다른 모서리는 그대로
        let covered = vec![win(2, 200.0, 200.0, 600.0, 600.0), win(1, 300.0, 300.0, 1000.0, 800.0)];
        assert_eq!(web_on_window(&covered, 1, Corner::TopLeft), WebAt::Hidden(300.0, 300.0));
        assert_eq!(web_on_window(&covered, 1, Corner::TopRight), WebAt::Shown(1000.0, 300.0));
    }

    #[test]
    fn corners_are_the_screen_corners_and_uncovered_window_tops_within_reach() {
        let w = world(vec![win(2, 900.0, 200.0, 1300.0, 700.0), win(1, 300.0, 300.0, 1000.0, 800.0)]);
        let all = corner_spots(&w, (960.0, 520.0), 1e9, 100.0);
        let has = |x: f64, y: f64, c: Corner, win: Option<u64>| all.iter().any(|s| s.x == x && s.y == y && s.corner == c && s.window == win);
        assert!(has(0.0, 0.0, Corner::TopLeft, None) && has(1920.0, 1040.0, Corner::BottomRight, None));
        assert!(has(900.0, 200.0, Corner::TopLeft, Some(2)) && has(1300.0, 200.0, Corner::TopRight, Some(2)));
        assert!(has(300.0, 300.0, Corner::TopLeft, Some(1)));
        assert!(!all.iter().any(|s| s.window == Some(1) && s.corner == Corner::TopRight), "앞 창(2)에 가려진 모서리: {all:?}");
        assert!(all.iter().all(|s| (s.size - WEB_SIZE * 100.0).abs() < 1e-9));
        let near = corner_spots(&w, (960.0, 520.0), 500.0, 100.0);
        assert!(!near.is_empty() && near.iter().all(|s| (s.x - 960.0).hypot(s.y - 520.0) <= 500.0), "{near:?}");
        // 작은 창 모서리에는 치지 않음
        let tiny = world(vec![win(3, 500.0, 500.0, 600.0, 560.0)]);
        assert!(corner_spots(&tiny, (550.0, 600.0), 1e9, 100.0).iter().all(|s| s.window.is_none()));
    }

    #[test]
    fn the_swing_anchor_is_up_ahead_on_the_screen_top_or_a_window_bottom_and_never_lets_the_feet_hit_the_floor() {
        let w = world(vec![]);
        let (size, drop, floor) = (100.0, 90.0, 1040.0);
        let hand = (600.0, floor - drop);
        let a = swing_anchor(&w, hand, 1.0, size, drop, floor).unwrap();
        assert_eq!(a.y, 0.0);
        assert!(a.x > hand.0 && a.len >= SWING_MIN_LEN * size);
        assert!(a.y + a.len + drop <= floor - SWING_CLEAR * size + 1e-6, "맨 아래에서도 바닥 위 {a:?}");
        let left = swing_anchor(&w, (1300.0, hand.1), -1.0, size, drop, floor).unwrap();
        assert!(left.x < 1300.0);
        assert!(swing_anchor(&w, hand, -1.0, size, drop, floor).is_none(), "왼쪽으로는 화면 밖");
        // 머리 위 창 아랫변에 붙임
        let w2 = world(vec![win(1, 500.0, 100.0, 1400.0, 500.0)]);
        let b = swing_anchor(&w2, hand, 1.0, size, drop, floor).unwrap();
        assert_eq!(b.y, 500.0, "{b:?}");
        // 화면 끝 가까이에서 그쪽으로는 못 탐
        assert!(swing_anchor(&w, (1880.0, floor - drop), 1.0, size, drop, floor).is_none());
    }

    #[test]
    fn a_pendulum_released_from_rest_swings_through_the_bottom_to_the_other_side() {
        let (len, g) = (600.0, 2500.0);
        let (mut th, mut om) = (-0.8, 0.0);
        let mut peak = th;
        for _ in 0..600 {
            (th, om) = pendulum_step(th, om, len, g, 1.0 / 120.0);
            peak = peak.max(th);
        }
        assert!(peak > 0.75, "에너지가 거의 보존됨 {peak}");
        let (hx, hy) = pendulum_hand((100.0, 0.0), len, 0.0, 1.0);
        assert!((hx - 100.0).abs() < 1e-9 && (hy - len).abs() < 1e-9);
        let (vx, vy) = release_velocity(len, 0.5, 2.0, -1.0);
        assert!(vx < 0.0 && vy < 0.0, "앞(왼쪽) 위로 날아감");
    }

    #[test]
    fn the_whirl_circles_above_the_hand_and_ends_at_the_top_heading_the_throw_way() {
        let (hand, size) = ((500.0, 600.0), 100.0);
        for dir in [1.0, -1.0] {
            let end = whirl_point(hand, size, dir, WHIRL_SECS);
            assert!((end.0 - hand.0).abs() < 1e-6, "꼭대기 {end:?}");
            assert!((end.1 - (hand.1 - (WHIRL_LIFT + WHIRL_RADIUS * WHIRL_FLAT) * size)).abs() < 1e-6);
            let before = whirl_point(hand, size, dir, WHIRL_SECS - 0.01);
            assert!((end.0 - before.0) * dir > 0.0, "던질 쪽으로 움직이며 끝남");
            for k in 0..=100 {
                let (x, y) = whirl_point(hand, size, dir, WHIRL_SECS * k as f64 / 100.0);
                assert!((x - hand.0).abs() <= WHIRL_RADIUS * size + 1e-6 && y <= hand.1 - (WHIRL_LIFT - WHIRL_RADIUS * WHIRL_FLAT) * size + 1e-6);
            }
        }
        assert_eq!(whirl_flight_secs(0.0, size), WHIRL_FLIGHT_SECS.0);
        assert_eq!(whirl_flight_secs(1e6, size), WHIRL_FLIGHT_SECS.1);
        assert!(whirl_flight_secs(800.0, size) > whirl_flight_secs(-300.0, size));
    }

    #[test]
    fn a_yank_lands_where_it_was_aimed() {
        let g = 1700.0;
        let (from, to, t) = ((800.0, 1040.0), (500.0, 1040.0), YANK_FLIGHT_SECS);
        let (vx, vy) = yank_velocity(from, to, t, g);
        let (x, y) = (from.0 + vx * t, from.1 + vy * t + 0.5 * g * t * t);
        assert!((x - to.0).abs() < 1e-6 && (y - to.1).abs() < 1e-6);
        assert!(vy < 0.0, "위로 솟았다가 내려옴");
    }
}
