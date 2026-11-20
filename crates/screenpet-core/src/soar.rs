//! 🦸 슈퍼인간 낮게 날기의 숫자 (동작은 brain_soar.rs)
//!
//! - 걷기·달리기 토막의 FLY_SHARE는 날기 (토막마다 한 번 정함 — 걷기는 가끔). 바닥(땅·창 윗면)보다 LIFT (몸 크기 몫) 위를 떠서 감 —
//!   높이는 토막마다 정하고 초당 LIFT_RATE (몸 크기)씩 천천히 오르내림
//! - 빠르기 (걷기 속도의 배): SPEED_SECS마다 새 목표 — DASH_CHANCE로 돌진 DASH, 아니면 느긋한 CRUISE. 목표로 초당 SPEED_EASE 몫씩 천천히 다가감.
//!   FAST_ANIM 배보다 빠르면 fly_fast 그림
//! - 🔄 공중제비: 날 때 초당 FLIP_RATE로 FLIP_SECS 동안 한 바퀴 (FLIP_RISE만큼 더 솟음)
//! - 디딤판 끝: 날고 있으면 EDGE_GLIDE_CHANCE, 걷고 있으면 WALK_GLIDE_CHANCE로 활공해 내려감 (아니면 돌아섬) — 디딤판이 사라져도 활공.
//!   활공: 앞으로 GLIDE_VX (걷기 배), 살짝 솟았다가 (GLIDE_VY) 중력 GLIDE_GRAVITY 몫, 내려가는 빠르기는 GLIDE_MAX_VY (몸 크기/초)까지
//! - 착지는 늘 사뿐 (콰당 없음, hover 그림 SOFT_LAND_SECS) — 날기를 마치고 내려앉을 때 가끔 SOFT_LINE_CHANCE로 "touchdown!"
//! - 친구 화면을 오갈 때 TRANSIT_SPEED (걷기 배)로 날아서 · 💪 들고 다닐 땐 CARRY_PACE배 빠르게 날아서

/// 걷기·달리기 토막 중 날기 몫 (나머지는 걷기 — 가끔만)
pub const FLY_SHARE: f64 = 0.85;
/// 바닥 위로 뜨는 높이 범위 (몸 크기 몫 — 토막마다 고름)
pub const LIFT: (f64, f64) = (0.3, 0.8);
/// 뜨고 내려앉는 빠르기 (몸 크기/초)
pub const LIFT_RATE: f64 = 1.0;
/// 이보다 높이 떠 있으면 쉬기·돌아서기 그림 대신 hover (몸 크기 몫)
pub const HOVER_MIN: f64 = 0.05;
/// 느긋한 비행 빠르기 범위 (걷기 속도의 배)
pub const CRUISE: (f64, f64) = (1.2, 1.8);
/// 빠른 돌진 빠르기 범위 (걷기 속도의 배 — 달리기 2.5배보다 빠름)
pub const DASH: (f64, f64) = (3.3, 4.0);
/// 새 빠르기 목표를 고를 때 돌진 몫
pub const DASH_CHANCE: f64 = 0.45;
/// 빠르기 목표를 바꾸는 간격 (초)
pub const SPEED_SECS: (f64, f64) = (1.5, 4.0);
/// 목표 빠르기로 다가가는 몫 (초당 — 작을수록 천천히 바뀜)
pub const SPEED_EASE: f64 = 0.8;
/// 이 배보다 빠르면 fly_fast 그림
pub const FAST_ANIM: f64 = 2.6;
/// 🔄 날 때 공중제비 빈도 (초당 — 🌱 power를 곱함)
pub const FLIP_RATE: f64 = 0.06;
/// 🔄 공중제비 한 바퀴 시간 (초)
pub const FLIP_SECS: f64 = 0.7;
/// 🔄 공중제비 때 더 솟는 높이 (몸 크기 몫)
pub const FLIP_RISE: f64 = 0.25;
/// 🔄 공중제비 때 "wheee!" 하는 몫
pub const FLIP_LINE_CHANCE: f64 = 0.5;
/// 날아오를 때 "up, up and away!" 하는 몫
pub const TAKE_OFF_LINE_CHANCE: f64 = 0.3;
/// 날다가 디딤판 끝에서 활공해 내려가는 몫 (나머지는 돌아서 날아감)
pub const EDGE_GLIDE_CHANCE: f64 = 0.7;
/// 걷다가 디딤판 끝에서 활공해 내려가는 몫 (나머지는 돌아섬 — 뚝 떨어지지 않음)
pub const WALK_GLIDE_CHANCE: f64 = 0.6;
/// 활공 앞 빠르기 (걷기 속도의 배 — 🌱 soft_power를 곱함)
pub const GLIDE_VX: f64 = 2.8;
/// 활공을 시작할 때 살짝 솟는 빠르기 (몸 크기/초, 위가 -)
pub const GLIDE_VY: f64 = -0.6;
/// 활공 중력 (보통 중력의 몫 — 🌱 soft_power로 나눔)
pub const GLIDE_GRAVITY: f64 = 0.1;
/// 활공으로 내려가는 가장 큰 빠르기 (몸 크기/초)
pub const GLIDE_MAX_VY: f64 = 2.2;
/// 사뿐 착지 (hover 그림) 시간 (초)
pub const SOFT_LAND_SECS: f64 = 0.45;
/// 내려앉을 때 "touchdown!" 하는 몫
pub const SOFT_LINE_CHANCE: f64 = 0.15;
/// 친구 화면을 날아서 오가는 빠르기 (걷기 속도의 배)
pub const TRANSIT_SPEED: f64 = 2.2;
/// 친구 화면에서 날아 들어올 때 높이 (몸 크기 몫)
pub const ENTER_LIFT: f64 = 0.55;
/// 💪 물건을 들고 날아 다니는 빠르기 (흐물이 들고 걷기 juggle::WANDER_PACE의 배)
pub const CARRY_PACE: f64 = 2.0;
