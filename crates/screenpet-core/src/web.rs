//! 🕷️ 거미인간 줄 놀이 (pet.json `web_slinger` — 거미인간): 숫자와 순수 계산. 동물의 상태는 brain_web.rs, 그림은 src-tauri trail.rs.
//!
//! - 거미집: 한가할 때 가까운 모서리(작업 영역 네 모서리 + 앞에 가려지지 않은 창의 위 두 모서리)를 향해 줄을 쏘고, 줄이 닿으면 거미집이 핌.
//!   손님으로 친구 화면에 가 있으면 자주 (WEB_RATE_GUEST), 집에선 가끔 (WEB_RATE_HOME). 거미집은 앱이 WEB_LIFE_SECS 뒤 흐리게 지움.
//!   크기는 몸집과 상관없이 제각각 (WEB_RADIUS_LOGICAL)이고, 줄을 여러 번 휙휙 쏴서 짬 (WEAVE_SHOTS·weave_target — 앱이 살·고리를 차례로 그림).
//!   🌱 크기·줄 수와 그네·끌어오기·고치 빈도는 연령대로 자람 (크기는 몸집과 상관없이 WEB_RADIUS_LOGICAL, 줄 수는 shot_range — 아기 작게 2~4번 → 어른 크게 5~8번)
//!   창 모서리 거미집은 창을 따라 옮겨 가고, 앞 창에 가려지면 숨고, 창이 닫히면 사라짐 (web_on_window).
//! - 🕸️⭕ 둥근 거미집 (Corner::Center): 거미집 칠 때 ORB_CHANCE로 구석 대신 손이 닿는 (ORB_REACH) 아무 자리에 —
//!   화면이나 창 위 (orb_fit: 그 화면·창 안에 다 들고, 바탕화면이면 어떤 창과도, 창 위면 앞 창과 안 겹침), 반지름 ORB_RADIUS_LOGICAL,
//!   한 화면에 MAX_ORBS개까지. 사방으로 살 → 고리 차례로 짬 (weave_target). 창 위 것은 창 왼쪽 위 기준 자리로 따라감 (orb_on_window).
//!   걸리기·빗자루·거미집 놀이 판정은 원 전체 (in_web·perch_point·broom::sweeps_web)
//! - 그네: 머리 위 창 아랫변이나 화면 윗변에 줄을 붙여 몸을 끌어올린 뒤 진자처럼 흔들려 반대편에서 놓고 날아감.
//!   🌉🌉 줄 잇기: 날아가는 중에 앞 위로 새 줄을 쏴 (chain_anchor — 손까지 딱 닿는 길이, 지금 속도의 접선 몫으로 rope_omega) 또 흔들림.
//!   한 번에 CHAIN_SWINGS번 (🌱 chain_range — 연령대로), 닻 댈 자리가 없으면 그냥 떨어져 영웅 착지
//! - 끌어오기: 바닥의 물건이나 다른 동물에 줄을 붙여 휙 — 반은 앞에 떨어지게, 반은 머리 너머 뒤로
//! - 🌀 휘둘러 던지기: 친구가 연결된 화면 끝이 있으면 (WHIRL_CHANCE) 끌어오는 대신 물건을 줄로 낚아 머리 위로 빙빙 돌리다가
//!   그 끝 너머로 휙 — 끝을 넘은 물건은 앱의 던지기처럼 친구 모니터로 날아감 (whirl_point·whirl_flight_secs)
//! - 💪 거대한 것도: 무거워서 조금만 끌려오고 (GIANT_PULL_MAX, 머리 너머로는 못 넘김), 더 천천히·작게·덜 돌다 (whirl_secs(true)·GIANT_WHIRL_*)
//!   가까이 (GIANT_THROW_REACH)까지만 던짐 — 친구 끝이 그 안이면 넘어가고, 멀면 그 앞 바닥에.
//!   거대한 것을 받는 친구 끝이 멀면 실젖 (SPINNERET)에서 나온 줄에 매달아 몸 뒤 (DRAG_GAP)로 끌며 천천히 (DRAG_PACE) 걸어 나감
//! - 영웅 착지: 높이서 떨어져 내려서면 콰당 대신 한 손 짚고 쭉 (HERO_FALL_SIZES)
//! - 🐛 고치 말기: 가까운 다른 동물 몸 둘레로 줄을 COCOON_SHOTS번 쏴서 칭칭 (cocoon_target) — 감긴 동물은 COCOON_SECS 버둥대다 뿅
//! - 🕸️ 거미집에 걸리기: 거미인간이 아닌 동물의 몸이 거미집 사분원 안 (in_web)으로 들어가면 SNARE_CHANCE로 걸려 SNARE_SECS 버둥댐 —
//!   빠져나오면 그 거미집은 찢어짐 (앱이 지움). 빠져나온 뒤 SNARE_IMMUNE_SECS 동안은 다시 안 걸림. 거미집 자리는 앱이 WebSpot으로 알려 줌.
//!   거미인간은 (자기 것이든 다른 거미인간 것이든) 절대 안 걸림
//! - 🕸️ 출렁: 걸린 물건·동물은 탁 멈추지 않고 "팅~" — 들어온 쪽으로 더 늘어났다 감쇠 진동 (snag_spring, SNAG_SPRING_SECS) 뒤 살랑살랑 (snag_sway)
//! - 🕸️📦 물건도 걸림: 보이는 거미집 안으로 들어온 물건은 무조건 걸려 매달림 (object.rs — 풀린 뒤 SNAG_IMMUNE_SECS 동안은 안 걸림)
//! - 🕸️🧗 거미집 놀이: 거미인간이 가까운 보이는 거미집 (WEB_PLAY_REACH)에 줄을 쏴 올라가 살·고리 자리 (perch_point)를 기어 다니다가
//!   매달려 출렁, 뚝 떨어져 (높으면 영웅 착지) — 거미집은 안 찢음
//! - 🙃 거꾸로 매달리기: 몇 분에 한 번 (HANG_RATE × 연령대 세기) 바로 머리 위 (창 아랫변·창 면 아무 데·화면 윗변 — hang_anchors)에 줄을 붙여
//!   올라간 뒤 뒤집혀 줄을 타고 스르륵 내려와, 머리가 바닥에서 몸 HANG_HEAD_CLEAR배 높이에서 HANG_SECS 살랑 (HANG_SWAY) — 그 뒤 사뿐 내려앉거나 다시 올라가 놓음.
//!   창에 붙은 줄은 창을 따라가고 (HangLine::follow), 창이 닫히면 놓음
//! - 🌉🚪 그네로 친구 화면 가기: 친구가 연결된 끝 바로 안쪽 위에 줄을 붙여 (exit_anchor) 뒤로 당겨 올랐다가 끝 너머로 휙
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
/// 거미집 반지름 (몸 크기 배) — 모서리 고를 때의 기본값 (실제로 칠 때는 WEB_RADIUS_LOGICAL에서 고름)
pub const WEB_SIZE: f64 = 0.8;
/// 실제로 칠 구석 거미집 반지름 (논리 px — 화면 배율을 곱함): 몸집·연령대와 상관없이 작은 것부터 아주 큰 것까지 제각각
pub const WEB_RADIUS_LOGICAL: (f64, f64) = (70.0, 460.0);
/// 거미집 반지름 (논리 px)이 이만큼 늘 때마다 짜는 줄을 한 번 더 쏨
pub const WEB_RADIUS_PER_SHOT: f64 = 70.0;
/// 창 모서리 거미집은 창의 짧은 변의 이만큼까지만
pub const WEB_WINDOW_FIT: f64 = 0.7;
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

/// 🕸️⭕ 거미집을 칠 때 구석 대신 둥근 거미집을 칠 몫
pub const ORB_CHANCE: f64 = 0.2;
/// 🕸️⭕ 둥근 거미집 반지름 (논리 px — 화면 배율을 곱함): 몸집과 상관없이, 화면 가운데를 가리니 구석 것보다는 작게
pub const ORB_RADIUS_LOGICAL: (f64, f64) = (60.0, 260.0);
/// 🕸️⭕ 한 화면에 둥근 거미집은 이만큼까지
pub const MAX_ORBS: usize = 3;
/// 🕸️⭕ 둥근 거미집 가운데를 고르는 거리 (손에서, 몸 크기 배 — 큰 거미집이면 orb_reach가 더 늘림)
pub const ORB_REACH: f64 = 6.0;

/// 🕸️⭕ 둥근 거미집 가운데를 고르는 최대 거리 (물리 px): 몸 ORB_REACH배, 거미집이 크면 반지름의 1.5배까지
pub fn orb_reach(body: f64, radius: f64) -> f64 {
    (ORB_REACH * body).max(1.5 * radius)
}
/// 🕸️⭕ 둥근 거미집 자리를 찾아보는 횟수 (다 안 맞으면 구석 거미집)
pub const ORB_TRIES: u32 = 24;
/// 🕸️⭕ 둥근 거미집 짜는 줄이 노리는 자리: (한 바퀴의 몫 0..1, 반지름의 몫) — 사방 살 끝 넷 → 안쪽 고리 차례 (첫 줄은 가운데)
const ORB_WEAVE_AIMS: [(f64, f64); 8] = [(0.0, 1.0), (0.5, 1.0), (0.25, 1.0), (0.75, 1.0), (0.125, 0.75), (0.625, 0.75), (0.375, 0.5), (0.875, 0.5)];
/// 🕸️⭕🧗 둥근 거미집에서 기어 다니는 자리 (한 바퀴의 몫, 반지름 몫)
const ORB_PERCH_AIMS: [(f64, f64); 7] = [(0.1, 0.6), (0.35, 0.45), (0.6, 0.7), (0.85, 0.5), (0.25, 0.8), (0.5, 0.3), (0.75, 0.8)];

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

/// 🌉🌉 줄 잇기: 한 번 탈 때 이어 타는 그네 수 (첫 그네 포함, 청소년 기준 — 연령대는 chain_range)
pub const CHAIN_SWINGS: (u32, u32) = (2, 5);
/// 🌉🌉 어른이 되어도 이어 타는 그네는 이만큼까지
pub const CHAIN_SWINGS_MAX: u32 = 6;
/// 🌉🌉 놓고 날아간 뒤 다음 줄을 쏘기까지 (꼭대기 언저리에서 쏨)
pub const CHAIN_DELAY: (f64, f64) = (0.15, 0.3);
/// 🌉🌉 그 뒤 이만큼 안에 닻을 못 찾으면 잇기를 그만두고 그냥 떨어짐
pub const CHAIN_WINDOW: f64 = 0.5;
/// 🌉🌉 공중에서 잇는 줄의 시작 각도 후보 (도, 큰 것부터 — 화면 안에 드는 첫 것)
const CHAIN_ANGLES_DEG: [f64; 4] = [40.0, 34.0, 28.0, 22.0];
/// 🌉🌉 이어 쏠 때 "thwip!" 대신 신나서 "woohoo!" (web_swing 말)를 할 몫
pub const CHAIN_WOOHOO_CHANCE: f64 = 0.35;

/// 🌱 연령대 세기(stage_power)에 맞춘 이어 탈 그네 수 범위 — 청소년(1.0)은 CHAIN_SWINGS, 아기 (2, 3) → 어른 (2, 6)
pub fn chain_range(power: f64) -> (u32, u32) {
    let lo = CHAIN_SWINGS.0;
    let hi = ((f64::from(CHAIN_SWINGS.1) * power).round() as u32).clamp(lo, CHAIN_SWINGS_MAX);
    (lo, hi)
}

/// 끌어오기를 할 빈도 (닿는 거리에 무언가 있을 때, 초당 확률)
pub const PULL_RATE: f64 = 1.0 / 15.0;
/// 끌어온 뒤 다시 하기까지
pub const PULL_CALM: (f64, f64) = (12.0, 30.0);
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

/// 🌀 친구가 연결된 끝이나 던질 만한 동물이 있을 때 물건을 끌어오는 대신 휘둘러 던질 몫
pub const WHIRL_CHANCE: f64 = 0.75;
/// 🌀 던질 만한 동물과 친구 끝이 둘 다 있을 때 동물에게 던질 몫
pub const WHIRL_AT_PET_SHARE: f64 = 0.5;
/// 🌀 휘두른 물건을 던질 동물까지의 가로 거리 (몸 크기 배): 가까운 끝, 먼 끝
pub const WHIRL_PET_REACH: (f64, f64) = (2.0, 14.0);
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

/// 💪 거대한 것이 끌려오는 시간 (짧고 낮은 포물선 — 멀리 안 감)
pub const GIANT_YANK_FLIGHT_SECS: f64 = 0.45;
/// 💪 거대한 것이 한 번에 끌려오는 거리 (몸 크기 배까지)
pub const GIANT_PULL_MAX: f64 = 2.5;
/// 💪 끌려온 거대한 것과 거미인간 사이에 남기는 틈 (몸 크기 배)
pub const GIANT_PULL_GAP: f64 = 0.6;
/// 💪 거대한 것을 휘두르는 시간 (더 천천히)
pub const GIANT_WHIRL_SECS: f64 = 2.0;
/// 💪 거대한 것을 휘두르는 바퀴 수 (덜 돎)
pub const GIANT_WHIRL_TURNS: f64 = 1.25;
/// 💪 거대한 것을 휘두르는 원 크기 (보통 원의 배 — 더 작게)
pub const GIANT_WHIRL_SCALE: f64 = 0.6;
/// 💪 거대한 것을 던지는 가로 거리 (몸 크기 배까지) — 친구 끝이나 동물이 더 멀면 그 앞 바닥에 떨어짐
pub const GIANT_THROW_REACH: f64 = 6.0;

/// 💪 끄는 거대한 것과 몸 사이 틈 (몸 크기 배)
pub const DRAG_GAP: f64 = 0.3;
/// 💪 거대한 것을 끌며 걷는 빠르기 (걷기의 배 — 무거워서 천천히)
pub const DRAG_PACE: f64 = 0.7;
/// 💪 이만큼 끌어도 친구 끝에 못 닿으면 그 자리에 놓음 (초)
pub const DRAG_MAX_SECS: f64 = 40.0;
/// 💪 끄는 줄이 나오는 엉덩이(실젖) 자리: 몸 가운데에서 뒤로, 발에서 위로 (몸 크기 배)
pub const SPINNERET: (f64, f64) = (0.35, 0.4);

/// 🙃 거꾸로 매달리기 빈도 (한가할 때 초당 확률 — 연령대 세기를 곱함): 몇 분에 한 번
pub const HANG_RATE: f64 = 1.0 / 150.0;
/// 🙃 매달린 뒤 다시 매달리기까지
pub const HANG_CALM: (f64, f64) = (90.0, 240.0);
/// 🙃 다 내려왔을 때 머리 (그림 아래 끝)가 바닥에서 이만큼 (몸 크기 배) — 이보다 낮게 놓으니 내려앉을 때 영웅 착지·콰당 없이 사뿐
pub const HANG_HEAD_CLEAR: (f64, f64) = (1.0, 1.5);
/// 🙃 다 내려왔을 때도 닻에서 손발까지 줄이 이만큼 (몸 크기 배)은 남음 — 그보다 낮은 닻은 안 씀
pub const HANG_MIN_LINE: f64 = 1.0;
/// 🙃 줄을 당겨 닻 아래로 올라가는 시간
pub const HANG_RISE_SECS: f64 = 0.5;
/// 🙃 뒤집혀 줄을 타고 스르륵 내려오는 시간
pub const HANG_SLIDE_SECS: f64 = 1.4;
/// 🙃 매달려 있는 시간
pub const HANG_SECS: (f64, f64) = (3.0, 6.0);
/// 🙃 살랑살랑 흔들리는 폭 (몸 크기 배, 한쪽)
pub const HANG_SWAY: f64 = 0.06;
/// 🙃 살랑살랑 한 번 오가는 시간
pub const HANG_SWAY_SECS: f64 = 2.4;
/// 🙃 가까운 펫·커서가 있을 때 그쪽을 보며 인사할 몫
pub const HANG_GREET_CHANCE: f64 = 0.6;
/// 🙃 인사할 펫·커서까지의 거리 (몸 크기 배)
pub const HANG_GREET_REACH: f64 = 6.0;
/// 🙃 매달린 뒤 이만큼 지나 인사함
pub const HANG_GREET_AT: f64 = 0.8;
/// 🙃 끝에 내려앉지 않고 다시 올라가 놓을 몫 (높이서 놓으니 영웅 착지)
pub const HANG_CLIMB_CHANCE: f64 = 0.35;
/// 🙃 줄을 타고 다시 올라가는 시간
pub const HANG_CLIMB_SECS: f64 = 1.2;

/// 🙃 거꾸로 매달린 줄의 닻 (물리 px) — 창에 붙었으면 그 창 왼쪽 위에서의 자리 (rel)로 창을 따라감
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HangLine {
    /// 닻 x
    pub x: f64,
    /// 닻 y
    pub y: f64,
    /// 붙은 창 (없으면 화면 윗변)
    pub window: Option<u64>,
    /// 창 왼쪽 위에서 닻까지
    pub rel: (f64, f64),
}

impl HangLine {
    /// 창을 따라 옮긴 닻. 붙은 창이 사라졌으면 None (줄을 놓음)
    pub fn follow(&self, world: &World) -> Option<HangLine> {
        let Some(id) = self.window else { return Some(*self) };
        let r = world.windows.iter().find(|w| w.id == id)?.rect;
        Some(HangLine { x: r.left + self.rel.0, y: r.top + self.rel.1, ..*self })
    }
}

/// 🙃 x 바로 위의 닻 후보 (lowest보다 위, 같은 모니터): 그 x를 덮는 창의 아랫변 · 그 창 면 아무 데 (창 위쪽 ~ 아랫변/lowest 사이) · 화면 윗변.
/// 창 면 자리는 pick (0..1)으로 고름
pub fn hang_anchors(world: &World, x: f64, lowest: f64, pick: f64) -> Vec<HangLine> {
    let wa = world.work_area_at(x, lowest);
    let mut out = Vec::new();
    for w in &world.windows {
        let r = w.rect;
        if !(r.left < x && x < r.right) || r.top >= lowest || r.bottom <= wa.top {
            continue;
        }
        if r.bottom <= lowest {
            out.push(HangLine { x, y: r.bottom, window: Some(w.id), rel: (x - r.left, r.height()) });
        }
        let top = r.top.max(wa.top);
        let low = r.bottom.min(lowest);
        if low - top > 1.0 {
            let y = top + (0.15 + 0.7 * pick) * (low - top);
            out.push(HangLine { x, y, window: Some(w.id), rel: (x - r.left, y - r.top) });
        }
    }
    if wa.top <= lowest {
        out.push(HangLine { x, y: wa.top, window: None, rel: (0.0, 0.0) });
    }
    out
}

/// 이만큼 (몸 크기 배) 넘게 떨어져 내려서면 영웅 착지
pub const HERO_FALL_SIZES: f64 = 1.6;
/// 영웅 착지 자세로 있는 시간 (hero_land 3칸 8fps + 잠깐)
pub const HERO_SECS: f64 = 0.9;

/// 🐛 고치를 말 빈도 (닿는 거리에 쉬는 동물이 있을 때, 초당 확률) — 끌어오기와 비슷하게 가끔
pub const COCOON_RATE: f64 = 1.0 / 15.0;
/// 🐛 고치를 만 뒤 다시 말기까지
pub const COCOON_CALM: (f64, f64) = (20.0, 45.0);
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

/// 🕸️ "팅~" 출렁이는 시간 (그 뒤는 살랑살랑)
pub const SNAG_SPRING_SECS: f64 = 0.8;
/// 🕸️ 출렁 떨림 (Hz)
const SNAG_HZ: f64 = 3.0;
/// 🕸️ 출렁이 줄어드는 시간 상수 (초)
const SNAG_DECAY_SECS: f64 = 0.22;
/// 🕸️ 출렁 뒤 살랑살랑 한 번 (초)
const SNAG_SWAY_SECS: f64 = 2.2;
/// 🕸️ 걸린 물건이 늘어나는 거리 (물건 크기 배) / 살랑살랑 폭
pub const SNAG_STRETCH_OBJECT: f64 = 0.45;
/// 🕸️ 걸린 동물이 늘어나는 거리 (몸 크기 배)
pub const SNAG_STRETCH_PET: f64 = 0.25;
/// 🕸️ 살랑살랑 폭 (늘어나는 거리 배)
pub const SNAG_SWAY: f64 = 0.12;
/// 🕸️📦 거미집에서 빠진 물건이 다시 안 걸리는 시간 (초)
pub const SNAG_IMMUNE_SECS: f64 = 1.0;

/// 🕸️🧗 이만큼 (몸 크기 배) 안의 보이는 거미집이면 놀러 감
pub const WEB_PLAY_REACH: f64 = 8.0;
/// 🕸️🧗 가까이 거미집이 있을 때 1초에 놀러 갈 확률
pub const WEB_PLAY_RATE: f64 = 1.0 / 25.0;
/// 🕸️🧗 한 번 논 뒤 쉬는 시간 (초)
pub const WEB_PLAY_CALM: (f64, f64) = (30.0, 60.0);
/// 🕸️🧗 기어 다니는 자리 수 (처음 붙은 자리 빼고)
pub const WEB_CRAWL_STEPS: (u32, u32) = (2, 4);
/// 🕸️🧗 한 자리에서 다음 자리까지 기는 시간 (초)
pub const WEB_CRAWL_SECS: f64 = 0.7;
/// 🕸️🧗 다 기고 매달려 있는 시간 (초)
pub const WEB_DANGLE_SECS: (f64, f64) = (1.0, 2.0);
/// 🕸️🧗 매달려 있다가 통통 출렁일 몫
pub const WEB_BOUNCE_CHANCE: f64 = 0.5;
/// 🕸️🧗 기어 다니는 자리 (부채꼴 각도 몫 0..1, 반지름 몫) — 살·고리 위
const PERCH_AIMS: [(f64, f64); 7] = [(0.5, 0.7), (0.2, 0.5), (0.8, 0.5), (0.5, 0.4), (0.3, 0.75), (0.7, 0.75), (0.5, 0.55)];

/// 🌉🚪 그네로 나갈 때 닻을 끝에서 안쪽으로 이만큼 (몸 크기 배)
pub const EXIT_ANCHOR_IN: f64 = 0.5;
/// 🌉🚪 놓은 뒤 이 시간 (초) 안에 끝을 넘어야 친구 화면으로 감 (아니면 그냥 떨어짐)
pub const EXIT_FLY_SECS: f64 = 2.5;
/// 🌉🚪 친구 화면에서 날아 들어온 뒤 줄을 쏠 닻을 찾는 시간 (초 — 없으면 그냥 떨어져 영웅 착지)
pub const ARRIVAL_CHAIN_WINDOW: f64 = 1.0;

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
    /// 모서리 점 (둥근 거미집이면 가운데)
    pub x: f64,
    pub y: f64,
    /// 어느 모서리 (사분원이 그 안쪽으로 — Center면 둥근 거미집)
    pub corner: Corner,
    /// 반지름
    pub size: f64,
    /// 앞 창에 가려짐 (안 걸림)
    pub hidden: bool,
}

/// 🕸️ 점 (px, py)가 모서리 (x, y)에서 안쪽으로 반지름 radius만큼 펼쳐진 거미집 사분원 안 (모서리 바깥쪽은 아님).
/// 둥근 거미집 (Corner::Center)이면 가운데 (x, y)의 원 전체
pub fn in_web(px: f64, py: f64, x: f64, y: f64, corner: Corner, radius: f64) -> bool {
    if !(radius > 0.0) {
        return false;
    }
    if corner.is_orb() {
        return (px - x).hypot(py - y) <= radius;
    }
    let right = matches!(corner, Corner::TopLeft | Corner::BottomLeft);
    let down = matches!(corner, Corner::TopLeft | Corner::TopRight);
    let x_in = if right { px >= x } else { px <= x };
    let y_in = if down { py >= y } else { py <= y };
    x_in && y_in && (px - x).hypot(py - y) <= radius
}

/// 거미집이 붙은 모서리 (그림이 모서리 안쪽으로 펼쳐짐) — Center는 🕸️⭕ 둥근 거미집 (x·y가 가운데, 사방으로). 웹뷰엔 snake_case 이름 ("center")
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Corner {
    TopLeft,
    TopRight,
    BottomLeft,
    BottomRight,
    Center,
}

impl Corner {
    /// 🕸️⭕ 둥근 거미집
    pub fn is_orb(self) -> bool {
        self == Corner::Center
    }

    /// 사분원이 펼쳐지는 쪽 (가로, 세로 ±1). 둥근 거미집은 (0, 0)
    fn inward(self) -> (f64, f64) {
        match self {
            Corner::TopLeft => (1.0, 1.0),
            Corner::TopRight => (-1.0, 1.0),
            Corner::BottomLeft => (1.0, -1.0),
            Corner::BottomRight => (-1.0, -1.0),
            Corner::Center => (0.0, 0.0),
        }
    }
}

/// 🕸️⭕ 가운데 (x, y) 반지름 r인 원 쪽 위치 (한 바퀴의 몫 frac, 반지름의 몫 rad)
fn orb_point(x: f64, y: f64, r: f64, frac: f64, rad: f64) -> (f64, f64) {
    let ang = std::f64::consts::TAU * frac;
    (x + r * rad * ang.cos(), y + r * rad * ang.sin())
}

/// 새 거미집 하나 (Pet::take_web으로 한 번 꺼냄)
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WebSpin {
    /// 모서리 점 x (물리 px — 둥근 거미집이면 가운데)
    pub x: f64,
    /// 모서리 점 y (물리 px — 둥근 거미집이면 가운데)
    pub y: f64,
    /// 어느 모서리 (Center = 둥근 거미집)
    pub corner: Corner,
    /// 거미집 반지름 (물리 px)
    pub size: f64,
    /// 창 모서리·창 위 둥근 거미집이면 그 창 id (창을 따라가고 닫히면 앱이 지움), 화면 것이면 None
    pub window: Option<u64>,
    /// 줄을 쏘아 짜는 횟수 (1이면 한 번에 핌)
    pub shots: u32,
    /// 다 짜는 데 걸리는 시간 (초) — 앱이 그동안 살·고리를 차례로 그림 (0이면 바로 핌)
    pub build: f64,
}

/// 🕸️ 거미집을 짜는 k번째 줄 (1부터 — 0번째는 모서리·가운데)이 노리는 자리 (물리 px): 부채꼴 살 끝 → 안쪽 고리 차례.
/// 둥근 거미집은 사방의 살 끝 → 안쪽 고리 (ORB_WEAVE_AIMS)
pub fn weave_target(spot: &WebSpin, k: u32) -> (f64, f64) {
    let i = k.max(1) as usize - 1;
    if spot.corner.is_orb() {
        let (frac, rad) = ORB_WEAVE_AIMS[i % ORB_WEAVE_AIMS.len()];
        return orb_point(spot.x, spot.y, spot.size, frac, rad);
    }
    let (frac, rad) = WEAVE_AIMS[i % WEAVE_AIMS.len()];
    let (sx, sy) = spot.corner.inward();
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

/// 원 (x, y, radius)이 사각형 r에 걸침 (가장 가까운 점이 반지름 안 — 닿기만 하는 건 아님)
fn disk_hits(r: &Rect, x: f64, y: f64, radius: f64) -> bool {
    let (nx, ny) = (x.clamp(r.left, r.right), y.clamp(r.top, r.bottom));
    (nx - x).hypot(ny - y) < radius
}

/// 원 (x, y, radius)이 사각형 r 안에 다 들어감
fn disk_inside(r: &Rect, x: f64, y: f64, radius: f64) -> bool {
    x - radius >= r.left && x + radius <= r.right && y - radius >= r.top && y + radius <= r.bottom
}

/// 🕸️⭕ 가운데 (x, y) 반지름 radius인 둥근 거미집을 칠 수 있나: at(손 자리)이 있는 작업 영역 안에 다 들어가야 함.
/// 가운데가 창 위면 (가장 앞 창) 그 창 안에 다 들고 그보다 앞 창들과 안 겹쳐야 함 → Some(Some(창 id)),
/// 바탕화면이면 어떤 창과도 안 겹쳐야 함 → Some(None). 안 되면 None
pub fn orb_fit(world: &World, at: (f64, f64), x: f64, y: f64, radius: f64) -> Option<Option<u64>> {
    let wa = world.work_area_at(at.0, at.1);
    if !(radius > 0.0) || !disk_inside(&wa, x, y, radius) {
        return None;
    }
    match world.windows.iter().position(|w| inside(&w.rect, x, y)) {
        None => (!world.windows.iter().any(|w| disk_hits(&w.rect, x, y, radius))).then_some(None),
        Some(k) => {
            let w = &world.windows[k];
            let covered = world.windows[..k].iter().any(|f| disk_hits(&f.rect, x, y, radius));
            (disk_inside(&w.rect, x, y, radius) && !covered).then_some(Some(w.id))
        }
    }
}

/// 🕸️⭕ 창(window) 위 둥근 거미집을 지금 창 목록(앞 → 뒤)에 맞춤: 가운데 = 창 왼쪽 위 + offset (창을 옮기면 따라감).
/// 앞 창이 원에 걸치면 숨고, 창이 없거나 작아져 원이 창 밖으로 나가면 지움
pub fn orb_on_window(windows: &[WindowInfo], window: u64, offset: (f64, f64), radius: f64) -> WebAt {
    let Some(k) = windows.iter().position(|w| w.id == window) else { return WebAt::Gone };
    let r = windows[k].rect;
    let (x, y) = (r.left + offset.0, r.top + offset.1);
    if !disk_inside(&r, x, y, radius) {
        return WebAt::Gone;
    }
    if windows[..k].iter().any(|f| disk_hits(&f.rect, x, y, radius)) {
        WebAt::Hidden(x, y)
    } else {
        WebAt::Shown(x, y)
    }
}

/// 🕸️⭕ 창 위 거미집 점의 그 창 왼쪽 위 기준 자리 (앱이 처음에 한 번 재 둠 — orb_on_window의 offset). 창이 없으면 (0, 0)
pub fn window_offset(windows: &[WindowInfo], w: &WebSpin) -> (f64, f64) {
    w.window.and_then(|id| windows.iter().find(|x| x.id == id)).map_or((0.0, 0.0), |x| (w.x - x.rect.left, w.y - x.rect.top))
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

/// 🌉🌉 공중에서 이을 닻: 날아가는 손(hand)에서 dir 쪽 위의 창 아랫변 (가까운 것부터) 또는 화면 윗변 — 같은 모니터만.
/// 줄 길이는 손까지 딱 (그래서 순간이동 없이 지금 자리에서 진자가 이어짐), SWING_MIN_LEN..SWING_MAX_LEN이고
/// 맨 아래에서도 발이 바닥(floor)보다 SWING_CLEAR 위. 시작 각도는 CHAIN_ANGLES_DEG 중 닻과 놓는 자리가 화면 안에 드는 첫 것. 없으면 None
pub fn chain_anchor(world: &World, hand: (f64, f64), dir: f64, size: f64, drop: f64, floor: f64) -> Option<Anchor> {
    let wa = world.work_area_at(hand.0, hand.1);
    if !(wa.left..=wa.right).contains(&hand.0) || !(wa.top..wa.bottom).contains(&hand.1) {
        return None;
    }
    let (min, max) = (SWING_MIN_LEN * size, SWING_MAX_LEN * size);
    let mut tops: Vec<(f64, Option<Rect>)> =
        world.windows.iter().filter(|w| w.rect.bottom < hand.1 && w.rect.bottom > wa.top).map(|w| (w.rect.bottom, Some(w.rect))).collect();
    tops.sort_by(|a, b| b.0.total_cmp(&a.0));
    tops.push((wa.top, None));
    for (top, win) in tops {
        let dy = hand.1 - top;
        let longest = (floor - SWING_CLEAR * size - drop - top).min(max);
        for deg in CHAIN_ANGLES_DEG {
            let angle = deg.to_radians();
            let len = dy / angle.cos();
            if len < min || len > longest {
                continue;
            }
            let ax = hand.0 + dir * dy * angle.tan();
            let far = ax + dir * len * (RELEASE_FRAC * angle).sin();
            let held = win.is_none_or(|r| r.left < ax && ax < r.right);
            let room = (wa.left + 0.5 * size..=wa.right - 0.5 * size).contains(&ax) && (wa.left + size..=wa.right - size).contains(&far);
            if held && room {
                return Some(Anchor { x: ax, y: top, len, angle });
            }
        }
    }
    None
}

/// 🌉🌉 날아가던 몸에 줄이 팽팽해질 때의 각속도: 속도 (vx, vy)의 접선 몫 ÷ 줄 길이 (dir 쪽이 +, 줄 방향 몫은 줄이 받아냄)
pub fn rope_omega(len: f64, theta: f64, vx: f64, vy: f64, dir: f64) -> f64 {
    if len <= 0.0 {
        return 0.0;
    }
    (dir * vx * theta.cos() - vy * theta.sin()) / len
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

/// 🌀 휘두르는 시간: 보통 WHIRL_SECS, 💪 거대한 것 (heavy)은 GIANT_WHIRL_SECS
pub fn whirl_secs(heavy: bool) -> f64 {
    if heavy {
        GIANT_WHIRL_SECS
    } else {
        WHIRL_SECS
    }
}

/// 🌀 휘두르는 물건 가운데 자리: 손(hand) 위 WHIRL_LIFT에 가운데를 둔 납작한 원을 t초에 따라 돎 (dir 쪽으로 던질 방향).
/// whirl_secs(heavy)에 원 꼭대기에 와서 dir 쪽으로 움직임. 💪 heavy면 원이 GIANT_WHIRL_SCALE배, GIANT_WHIRL_TURNS바퀴만
pub fn whirl_point(hand: (f64, f64), size: f64, dir: f64, t: f64, heavy: bool) -> (f64, f64) {
    let (scale, turns) = if heavy { (GIANT_WHIRL_SCALE, GIANT_WHIRL_TURNS) } else { (1.0, WHIRL_TURNS) };
    let (rx, ry) = (scale * WHIRL_RADIUS * size, scale * WHIRL_RADIUS * WHIRL_FLAT * size);
    let left = 1.0 - (t / whirl_secs(heavy)).clamp(0.0, 1.0);
    let phi = -std::f64::consts::FRAC_PI_2 - dir * turns * std::f64::consts::TAU * left;
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

/// 🕸️ "팅~" 출렁 (물리 px): 걸린 뒤 t초에 들어온 쪽 dir (단위 벡터)으로 amp까지 늘어났다 되돌아오며 감쇠 진동. SNAG_SPRING_SECS 뒤 0
pub fn snag_spring(dir: (f64, f64), amp: f64, t: f64) -> (f64, f64) {
    if !(0.0..SNAG_SPRING_SECS).contains(&t) {
        return (0.0, 0.0);
    }
    let k = amp * (-t / SNAG_DECAY_SECS).exp() * (std::f64::consts::TAU * SNAG_HZ * t).sin();
    (dir.0 * k, dir.1 * k)
}

/// 🕸️ 출렁 뒤 살랑살랑 (가로, 물리 px): amp × SNAG_SWAY 폭으로 SNAG_SWAY_SECS마다 한 번. 출렁 중이면 0
pub fn snag_sway(amp: f64, t: f64) -> f64 {
    if t < SNAG_SPRING_SECS {
        return 0.0;
    }
    amp * SNAG_SWAY * (std::f64::consts::TAU * (t - SNAG_SPRING_SECS) / SNAG_SWAY_SECS).sin()
}

/// 🕸️ 걸려 들어온 쪽 (단위 벡터): 속도 (vx, vy) 쪽, 거의 멈춰 있으면 아래
pub fn snag_dir(vx: f64, vy: f64) -> (f64, f64) {
    let v = vx.hypot(vy);
    if v.is_finite() && v > 1.0 {
        (vx / v, vy / v)
    } else {
        (0.0, 1.0)
    }
}

/// 🕸️🧗 거미집 w에서 k번째로 기어 갈 자리 (물리 px — 살·고리 위, 사분원 안. 둥근 거미집이면 원 안 사방)
pub fn perch_point(w: &WebSpot, k: u32) -> (f64, f64) {
    if w.corner.is_orb() {
        let (frac, rad) = ORB_PERCH_AIMS[k as usize % ORB_PERCH_AIMS.len()];
        return orb_point(w.x, w.y, w.size, frac, rad);
    }
    let (frac, rad) = PERCH_AIMS[k as usize % PERCH_AIMS.len()];
    let (sx, sy) = w.corner.inward();
    let ang = std::f64::consts::FRAC_PI_2 * frac;
    let r = w.size * rad;
    (w.x + sx * r * ang.cos(), w.y + sy * r * ang.sin())
}

/// 🌉🚪 그네로 dir 쪽 화면 끝을 넘어갈 닻: 그 끝에서 EXIT_ANCHOR_IN 안쪽, 손(hand) 위의 창 아랫변 (그 x를 덮는 것, 가까운 것부터) 또는 화면 윗변.
/// 줄 길이는 swing_anchor와 같고, 시작 각도는 SWING_ANGLE_DEG부터 줄여 가며 진자 시작 자리 (닻 뒤 안쪽)가 화면 안에 드는 첫 것.
/// 반대편으로 흔들려 놓으면 끝 너머로 날아감. drop = 손에서 발까지 (물리 px). 없으면 None
pub fn exit_anchor(world: &World, hand: (f64, f64), dir: f64, size: f64, drop: f64, floor: f64) -> Option<Anchor> {
    let wa = world.work_area_at(hand.0, hand.1);
    let ax = if dir > 0.0 { wa.right - EXIT_ANCHOR_IN * size } else { wa.left + EXIT_ANCHOR_IN * size };
    let min = SWING_MIN_LEN * size;
    let mut tops: Vec<f64> =
        world.windows.iter().filter(|w| w.rect.bottom <= hand.1 - min && w.rect.bottom > wa.top && w.rect.left < ax && ax < w.rect.right).map(|w| w.rect.bottom).collect();
    tops.sort_by(|a, b| b.total_cmp(a));
    tops.push(wa.top);
    for top in tops {
        let len = (floor - SWING_CLEAR * size - drop - top).min(SWING_MAX_LEN * size);
        if len < min {
            continue;
        }
        for deg in [SWING_ANGLE_DEG, 42.0, 36.0, 30.0, 24.0] {
            let angle = f64::to_radians(deg);
            let start = ax - dir * len * angle.sin();
            if (wa.left + size..=wa.right - size).contains(&start) {
                return Some(Anchor { x: ax, y: top, len, angle });
            }
        }
    }
    None
}

#[cfg(test)]
#[path = "web_orb_tests.rs"]
mod orb_tests;

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

    /// 🌉🌉 공중의 손에서 잇는 닻: 앞 위 (화면 윗변이나 창 아랫변), 줄은 손에 딱 닿는 길이 (SWING_MAX_LEN 안), 맨 아래서도 발이 바닥 위, 놓는 자리도 화면 안
    #[test]
    fn a_chain_anchor_is_up_ahead_at_rope_length_from_the_airborne_hand() {
        let w = world(vec![]);
        let (size, drop, floor) = (100.0, 90.0, 1040.0);
        let hand = (700.0, 520.0);
        for dir in [1.0, -1.0] {
            let a = chain_anchor(&w, (if dir > 0.0 { hand.0 } else { 1920.0 - hand.0 }, hand.1), dir, size, drop, floor).unwrap();
            let hx = if dir > 0.0 { hand.0 } else { 1920.0 - hand.0 };
            assert_eq!(a.y, 0.0);
            assert!((a.x - hx) * dir > 0.0, "앞 {a:?}");
            assert!(((a.x - hx).hypot(a.y - hand.1) - a.len).abs() < 1e-6, "손에서 줄 길이 그대로 {a:?}");
            let th = -a.angle;
            let (px, py) = pendulum_hand((a.x, a.y), a.len, th, dir);
            assert!((px - hx).abs() < 1e-6 && (py - hand.1).abs() < 1e-6, "지금 각도가 시작 각도");
            assert!(a.len <= SWING_MAX_LEN * size && a.len >= SWING_MIN_LEN * size);
            assert!(a.y + a.len + drop <= floor - SWING_CLEAR * size + 1e-6);
            let far = a.x + dir * a.len * (RELEASE_FRAC * a.angle).sin();
            assert!(size <= far && far <= 1920.0 - size);
        }
        // 창 아랫변이 머리 위 앞에 있으면 그 창에
        let w2 = world(vec![win(1, 600.0, 50.0, 1500.0, 200.0)]);
        assert_eq!(chain_anchor(&w2, hand, 1.0, size, drop, floor).unwrap().y, 200.0);
        // 화면 끝 가까이: 없음
        assert!(chain_anchor(&w, (1700.0, 520.0), 1.0, size, drop, floor).is_none());
        // 너무 낮으면 (줄이 SWING_MAX_LEN보다 길어야 함) 없음
        assert!(chain_anchor(&w, (700.0, 900.0), 1.0, size, 20.0, floor).is_none());
        // 앞 속도가 크면 각속도는 진행 쪽 +
        assert!(rope_omega(600.0, -0.5, 500.0, 100.0, 1.0) > 0.0 && rope_omega(600.0, -0.5, -500.0, 100.0, -1.0) > 0.0);
        assert!(rope_omega(600.0, -0.5, -500.0, 0.0, 1.0) < 0.0, "뒤로 가는 중이면 −");
        // 🌱 이을 횟수 범위: 청소년은 CHAIN_SWINGS, 아기는 적게, 어른은 조금 더
        assert_eq!(chain_range(1.0), CHAIN_SWINGS);
        assert!(chain_range(0.6).1 < CHAIN_SWINGS.1 && chain_range(1.2).1 > CHAIN_SWINGS.1);
        assert!(chain_range(0.6).0 >= 2);
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
            let end = whirl_point(hand, size, dir, WHIRL_SECS, false);
            assert!((end.0 - hand.0).abs() < 1e-6, "꼭대기 {end:?}");
            assert!((end.1 - (hand.1 - (WHIRL_LIFT + WHIRL_RADIUS * WHIRL_FLAT) * size)).abs() < 1e-6);
            let before = whirl_point(hand, size, dir, WHIRL_SECS - 0.01, false);
            assert!((end.0 - before.0) * dir > 0.0, "던질 쪽으로 움직이며 끝남");
            for k in 0..=100 {
                let (x, y) = whirl_point(hand, size, dir, WHIRL_SECS * k as f64 / 100.0, false);
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

    /// 🕸️ "팅~": 들어온 쪽으로 먼저 늘어났다가 반대로 되돌아오며 줄어들고, 끝나면 0 — 그 뒤 살랑살랑은 아주 작게
    #[test]
    fn the_snag_spring_overshoots_along_the_entry_then_dies_out_into_a_faint_sway() {
        let amp = 40.0;
        let samples: Vec<(f64, f64)> = (0..=100).map(|k| snag_spring((0.6, 0.8), amp, k as f64 * 0.01)).collect();
        assert_eq!(samples[0], (0.0, 0.0), "처음엔 제자리");
        let first = samples.iter().position(|v| v.0.abs() > 1.0).unwrap();
        assert!(samples[first].0 > 0.0 && samples[first].1 > 0.0, "들어온 쪽으로 먼저 늘어남");
        assert!(samples.iter().any(|v| v.1 < -1.0), "되돌아오며 반대로도 출렁");
        let peak = |a: usize, b: usize| samples[a..b].iter().map(|v| v.0.hypot(v.1)).fold(0.0, f64::max);
        assert!(peak(0, 20) > 0.5 * amp && peak(60, 80) < 0.15 * amp, "줄어듦");
        assert!(peak(0, 101) <= amp + 1e-9);
        assert_eq!(snag_spring((1.0, 0.0), amp, SNAG_SPRING_SECS), (0.0, 0.0));
        assert_eq!(snag_sway(amp, 0.3), 0.0, "출렁 중엔 살랑 없음");
        let sway: f64 = (0..300).map(|k| snag_sway(amp, SNAG_SPRING_SECS + k as f64 * 0.01).abs()).fold(0.0, f64::max);
        assert!(sway > 0.0 && sway <= SNAG_SWAY * amp + 1e-9, "{sway}");
        assert_eq!(snag_dir(0.0, 0.0), (0.0, 1.0));
        assert_eq!(snag_dir(-300.0, 0.0), (-1.0, 0.0));
    }

    /// 🕸️🧗 기어 다닐 자리는 모두 그 거미집 사분원 안
    #[test]
    fn perch_points_lie_inside_the_web() {
        for corner in [Corner::TopLeft, Corner::TopRight, Corner::BottomLeft, Corner::BottomRight] {
            let w = WebSpot { id: 1, x: 900.0, y: 500.0, corner, size: 300.0, hidden: false };
            let pts: Vec<(f64, f64)> = (0..10).map(|k| perch_point(&w, k)).collect();
            assert!(pts.iter().all(|&(x, y)| in_web(x, y, w.x, w.y, w.corner, w.size)), "{corner:?} {pts:?}");
            assert!(pts.windows(2).all(|p| p[0] != p[1]), "자리가 바뀜");
        }
    }

    /// 🌉🚪 나갈 닻은 그 끝 바로 안쪽 위, 진자 시작 자리는 안쪽 화면 안 — 반대편에서 놓으면 끝 너머
    #[test]
    fn the_exit_anchor_sits_just_inside_the_edge_and_the_swing_carries_past_it() {
        let w = world(vec![]);
        let size = 128.0;
        let hand = (1880.0, 1040.0 - 100.0);
        let a = exit_anchor(&w, hand, 1.0, size, 60.0, 1040.0).expect("닻");
        assert_eq!((a.x, a.y), (1920.0 - EXIT_ANCHOR_IN * size, 0.0));
        let start = a.x - a.len * a.angle.sin();
        assert!((size..=1920.0 - size).contains(&start));
        assert!(a.x + a.len * (RELEASE_FRAC * a.angle).sin() > 1920.0, "놓는 자리가 끝 너머");
        let l = exit_anchor(&w, (40.0, 940.0), -1.0, size, 60.0, 1040.0).expect("왼쪽");
        assert_eq!(l.x, EXIT_ANCHOR_IN * size);
        // 천장이 너무 낮으면 없음
        let low = World::new(vec![], vec![Rect::new(0.0, 0.0, 1920.0, 300.0)], 128.0);
        assert!(exit_anchor(&low, (1880.0, 250.0), 1.0, size, 60.0, 300.0).is_none());
    }
}
