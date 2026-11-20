//! 🦇 박쥐인간 오토바이·박쥐 신호의 숫자 (동작은 brain_bike.rs)
//!
//! - 걷기·달리기 토막의 RIDE_SHARE는 오토바이 (토막마다 한 번 정함) — 빠르기는 걷기 속도의 BIKE_SPEED배 (달리기 2.5배보다 빠름)
//! - 타는 동안 초당 WHEELIE_RATE로 윌리 (WHEELIE_SECS 동안 뒷바퀴로만)
//! - 디딤판 끝: EDGE_JUMP_CHANCE로 붕 뛰어내리고 아니면 DRIFT_SECS 드리프트로 멈춤. 토막 끝에도 STOP_DRIFT_CHANCE로 드리프트
//! - 앞 동물에 부딪치면 (BUMP_REACH) 그 동물이 (BUMP_VX, BUMP_VY)로 살짝 튕김 — 같은 동물은 BUMP_COOLDOWN_SECS 동안 다시 안 튕김
//! - 🔦 박쥐 신호: SIGNAL_FIRST_SECS 뒤부터, 쉬거나 걸을 때 초당 SIGNAL_RATE로 켜고 SIGNAL_SECS 동안 비춤, 다음은 SIGNAL_COOLDOWN_SECS 뒤
//! - 💪 거대한 것: 힘주기 HERO_STRAIN_SECS 뒤 늘 들어 올림

/// 걷기·달리기 토막 중 오토바이 토막 몫
pub const RIDE_SHARE: f64 = 0.5;
/// 오토바이 빠르기 (걷기 속도 cfg.walk_speed의 배 — 달리기는 2.5배)
pub const BIKE_SPEED: f64 = 3.2;
/// 오토바이에 오를 때 "vroom!" 하는 몫
pub const VROOM_CHANCE: f64 = 0.45;
/// 타는 동안 윌리를 시작하는 빈도 (초당)
pub const WHEELIE_RATE: f64 = 0.25;
/// 윌리 길이 (초)
pub const WHEELIE_SECS: (f64, f64) = (1.0, 2.2);
/// 디딤판 끝에서 붕 뛰어내리는 몫 (나머지는 드리프트로 멈춤)
pub const EDGE_JUMP_CHANCE: f64 = 0.6;
/// 뛰어내릴 때 앞으로 가는 빠르기 (오토바이 빠르기의 배)
pub const EDGE_JUMP_VX: f64 = 0.75;
/// 드리프트로 멈춰 있는 시간 (초)
pub const DRIFT_SECS: f64 = 0.7;
/// 오토바이 토막이 끝날 때 드리프트로 멈추는 몫
pub const STOP_DRIFT_CHANCE: f64 = 0.35;
/// 앞 동물과 부딪치는 거리: 몸 반 폭 + 그 동물 크기의 이 몫 (가로, 물리 px)
pub const BUMP_REACH: f64 = 0.3;
/// 같은 바닥으로 치는 높이 차 (물리 px)
pub const BUMP_LEVEL_PX: f64 = 4.0;
/// 부딪친 동물이 튕기는 빠르기 (논리 px/s — dpi를 곱함)
pub const BUMP_VX: f64 = 140.0;
pub const BUMP_VY: f64 = 260.0;
/// 같은 동물을 다시 튕기기까지 (초)
pub const BUMP_COOLDOWN_SECS: f64 = 4.0;
/// 🔦 처음 박쥐 신호를 켤 수 있게 되는 때 (clock 초)
pub const SIGNAL_FIRST_SECS: f64 = 90.0;
/// 🔦 켤 수 있을 때 켜는 빈도 (초당)
pub const SIGNAL_RATE: f64 = 0.05;
/// 🔦 비추는 시간 (초) — 앱의 빛줄기도 이만큼
pub const SIGNAL_SECS: f64 = 5.0;
/// 🔦 다음 신호까지 (초)
pub const SIGNAL_COOLDOWN_SECS: (f64, f64) = (150.0, 300.0);
/// 🔦 조명 렌즈 자리 (몸 크기 몫 — 그림 왼쪽 위에서, 오른쪽을 볼 때): tools/draw_bat.py SIGNAL_LENS (96, 84) / 128
pub const SIGNAL_FROM: (f64, f64) = (96.0 / 128.0, 84.0 / 128.0);
/// 💪 거대한 것에 힘주는 시간 (초 — 흐물이 juggle::STRAIN_SECS보다 훨씬 짧음)
pub const HERO_STRAIN_SECS: f64 = 0.5;
