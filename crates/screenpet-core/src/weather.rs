//! 계절 날씨 (v0.3.9, 일 년 내내) — 달력으로 날씨를 정하고, 창 윗면과 바닥에 쌓이는 눈·꽃잎·낙엽·푸른 잎 더미를 셈. 그림은 앱이 그림 (ui/weather.js).
//! ❄️ 함박눈 (v0.6.8, 12~2월 눈 오는 날 가끔·메뉴): 큰 눈송이가 펑펑, 눈과 같은 더미에 더 빨리·높이 쌓임
//! 🌀 태풍 (v0.6.8, 7~9월 가끔·메뉴): 쌓이지 않고 더미를 날려 버림. 돌풍·회오리 계산은 typhoon.rs
//! 메뉴에서 날씨를 직접 고르면 (real_weather::WeatherSource) 달력과 상관없이 그 날씨가 계속 옴
//!
//! CPU를 아끼려고 알갱이 하나하나를 세지 않음: 떨어지는 알갱이는 웹뷰의 그림일 뿐이고, 더미는 여기서 1초에 한 번
//! 작은 눈덩이를 무작위 칸에 얹어 키움 (쌓이는 속도와 녹는 속도만 맞춤). 앱은 장면이 바뀌었을 때만 웹뷰에 보냄
//!
//! 좌표: 세계는 물리 px. 장면(Scene)은 모니터별 논리 px, 그 모니터의 작업 영역 왼쪽 위가 (0, 0). 더미 높이는 논리 px.

use std::time::{Duration, Instant};

use serde::Serialize;

use crate::geom::{Rect, WindowInfo};
use crate::surfaces::Owner;
use crate::world::World;

/// 날씨 종류 (계절·실제 날씨에서 고르거나 메뉴에서 직접 고름)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Weather {
    /// 11월의 가벼운 눈
    LightSnow,
    Snow,
    /// ❄️ 함박눈 — 큰 눈송이가 펑펑, 눈보다 빨리·높이 쌓임 (12~2월 눈 오는 날 가끔, heavy_snows). 예전 판은 모르는 이름이라 버림
    HeavySnow,
    Rain,
    /// 5~8월의 소나기 (잠깐씩만, showers)
    Shower,
    /// 3·4월의 벚꽃잎
    Petals,
    /// 9~11월의 낙엽
    Leaves,
    /// 5~8월 소나기 사이의 푸른 잎 (예전 판은 모르는 이름이라 버림)
    GreenLeaves,
    /// 🌀 7~9월 가끔 오는 태풍 — 센 비·돌풍·회오리 (typhoon.rs). 쌓이지 않아 친구에게 쓸어 보내지도 않음
    Typhoon,
}

impl Weather {
    /// 눈임 (가벼운 눈·함박눈 포함)
    pub fn is_snow(self) -> bool {
        matches!(self, Weather::LightSnow | Weather::Snow | Weather::HeavySnow)
    }

    /// 펫이 자꾸 올려다보는 날씨
    pub fn looks_up(self) -> bool {
        matches!(self, Weather::Petals | Weather::Leaves | Weather::GreenLeaves)
    }

    /// 돌풍과 회오리가 부는 날씨 (태풍)
    pub fn is_stormy(self) -> bool {
        self == Weather::Typhoon
    }

    /// 더미 그림의 갈래: 눈(가벼운 눈·함박눈 포함)은 하나로 이어 쌓이고, 꽃잎·낙엽·푸른 잎은 서로 다른 더미임
    fn pile_family(self) -> u8 {
        match self {
            Weather::LightSnow | Weather::Snow | Weather::HeavySnow => 0,
            Weather::Petals => 1,
            Weather::Leaves => 2,
            Weather::GreenLeaves => 3,
            Weather::Rain | Weather::Shower | Weather::Typhoon => 4,
        }
    }

    /// 설정·통신에 쓰는 이름 (parse_preview와 같음)
    pub fn name(self) -> &'static str {
        match self {
            Weather::LightSnow => "light_snow",
            Weather::Snow => "snow",
            Weather::HeavySnow => "heavy_snow",
            Weather::Rain => "rain",
            Weather::Shower => "shower",
            Weather::Petals => "petals",
            Weather::Leaves => "leaves",
            Weather::GreenLeaves => "green_leaves",
            Weather::Typhoon => "typhoon",
        }
    }

    /// 쌓이는 빠르기·녹는 빠르기·최대 높이
    fn rates(self) -> Rates {
        match self {
            Weather::LightSnow => Rates { grow: 0.012, melt: 0.003, cap: 5.0 },
            Weather::Snow => Rates { grow: 0.02, melt: 0.004, cap: 9.0 },
            // 함박눈은 훨씬 빨리, 훨씬 높이 쌓임
            Weather::HeavySnow => Rates { grow: 0.045, melt: 0.004, cap: 16.0 },
            // 비는 쌓이지 않고, 남아 있던 더미를 빨리 녹인다
            Weather::Rain | Weather::Shower => Rates { grow: 0.0, melt: 0.05, cap: 0.0 },
            // 🌀 태풍은 쌓이지 않고 남은 더미를 바람이 금방 날려 버림
            Weather::Typhoon => Rates { grow: 0.0, melt: 0.25, cap: 0.0 },
            // 꽃잎·푸른 잎은 낙엽과 똑같이 쌓임
            Weather::Petals | Weather::Leaves | Weather::GreenLeaves => Rates { grow: 0.008, melt: 0.004, cap: 4.0 },
        }
    }
}

/// 더미 높이 (논리 px, 초당)
struct Rates {
    grow: f32,
    melt: f32,
    cap: f32,
}

/// 🧹 친구 화면으로 쓸어 보내는 더미의 이름: 물건 던지기 통로로 감 (예전 판은 모르는 물건이라 그냥 버림)
pub fn flurry_kind(kind: Weather) -> String {
    format!("flurry:{}", kind.name())
}

/// 받은 물건 이름이 쓸어 보낸 더미면 그 날씨 (쌓이는 날씨만)
pub fn parse_flurry_kind(raw: &str) -> Option<Weather> {
    let kind = parse_preview(raw.strip_prefix("flurry:")?)??;
    (kind.rates().cap > 0.0).then_some(kind)
}

/// 지역 날짜의 날씨 (여름의 Shower는 하루 중 잠깐씩만이고 그 사이엔 푸른 잎임, weather_at): 3·4월 벚꽃잎, 5~8월 소나기, 9·10월 낙엽,
/// 11월은 날마다 낙엽·가벼운 눈·비 차례로, 12~2월은 눈 이틀에 비 하루
pub fn season(month: u32, day: u32) -> Weather {
    let cycle = day.saturating_sub(1) % 3;
    match month {
        3 | 4 => Weather::Petals,
        5..=8 => Weather::Shower,
        9 | 10 => Weather::Leaves,
        11 => match cycle {
            0 => Weather::Leaves,
            1 => Weather::LightSnow,
            _ => Weather::Rain,
        },
        _ if cycle == 2 => Weather::Rain,
        _ => Weather::Snow,
    }
}

/// 소나기가 오는 시간대 (자정부터 초): 깨어 있는 시간에 몰아, 켜 둔 동안 볼 수 있게
const SHOWER_HOURS: (u32, u32) = (8 * 3600, 23 * 3600);
/// 하루 소나기 수
const SHOWERS_PER_DAY: (u32, u32) = (3, 5);
/// 소나기 한 번의 길이 (초): 3~8분
const SHOWER_SECS: (u32, u32) = (180, 480);

/// 그날의 소나기들: [시작, 끝) 초. 날짜로만 정해져서 앱을 다시 켜도 같음. 시간대를 똑같이 나눈 칸마다 하나씩이라 겹치지 않음
pub fn showers(year: i32, month: u32, day: u32) -> Vec<(u32, u32)> {
    let mut rng = fastrand::Rng::with_seed((year as i64 * 10_000 + month as i64 * 100 + day as i64) as u64);
    let count = rng.u32(SHOWERS_PER_DAY.0..=SHOWERS_PER_DAY.1);
    let slot = (SHOWER_HOURS.1 - SHOWER_HOURS.0) / count;
    (0..count)
        .map(|i| {
            let len = rng.u32(SHOWER_SECS.0..=SHOWER_SECS.1);
            let start = SHOWER_HOURS.0 + i * slot + rng.u32(0..=slot - len);
            (start, start + len)
        })
        .collect()
}

/// 🌀 태풍이 오는 달
const TYPHOON_MONTHS: std::ops::RangeInclusive<u32> = 7..=9;
/// 태풍이 오는 시간대 (자정부터 초)
const TYPHOON_HOURS: (u32, u32) = (9 * 3600, 22 * 3600);
/// 태풍이 오는 날의 몫 (그날 하루 1~2번)
const TYPHOON_DAY_CHANCE: f64 = 0.5;
const TYPHOONS_PER_DAY: (u32, u32) = (1, 2);
/// 태풍 한 번의 길이 (초): 5~10분
const TYPHOON_SECS: (u32, u32) = (300, 600);

/// 7~9월 그날의 🌀 태풍들: [시작, 끝) 초. 날짜로만 정해져서 앱을 다시 켜도 같음. 대략 이틀에 하루, 하루 1~2번 (겹치지 않음)
pub fn typhoons(year: i32, month: u32, day: u32) -> Vec<(u32, u32)> {
    if !TYPHOON_MONTHS.contains(&month) {
        return Vec::new();
    }
    let seed = (year as i64 * 10_000 + month as i64 * 100 + day as i64) as u64;
    let mut rng = fastrand::Rng::with_seed(seed.wrapping_mul(0x9e37_79b9_7f4a_7c15) ^ 0x7479_7068);
    if rng.f64() >= TYPHOON_DAY_CHANCE {
        return Vec::new();
    }
    let count = rng.u32(TYPHOONS_PER_DAY.0..=TYPHOONS_PER_DAY.1);
    let slot = (TYPHOON_HOURS.1 - TYPHOON_HOURS.0) / count;
    (0..count)
        .map(|i| {
            let len = rng.u32(TYPHOON_SECS.0..=TYPHOON_SECS.1);
            let start = TYPHOON_HOURS.0 + i * slot + rng.u32(0..=slot - len);
            (start, start + len)
        })
        .collect()
}

/// ❄️ 함박눈이 오는 시간대 (자정부터 초)
const HEAVY_SNOW_HOURS: (u32, u32) = (8 * 3600, 23 * 3600);
/// 함박눈이 오는 날의 몫 (12~2월 눈 오는 날 중)
const HEAVY_SNOW_DAY_CHANCE: f64 = 0.35;
const HEAVY_SNOWS_PER_DAY: (u32, u32) = (1, 2);
/// 함박눈 한 번의 길이 (초): 20~50분
const HEAVY_SNOW_SECS: (u32, u32) = (20 * 60, 50 * 60);

/// 12~2월 눈 오는 날의 ❄️ 함박눈들: [시작, 끝) 초. 날짜로만 정해져서 앱을 다시 켜도 같음. 눈 오는 날의 셋에 하루쯤, 하루 1~2번 (겹치지 않음)
pub fn heavy_snows(year: i32, month: u32, day: u32) -> Vec<(u32, u32)> {
    if !matches!(month, 12 | 1 | 2) || season(month, day) != Weather::Snow {
        return Vec::new();
    }
    let seed = (year as i64 * 10_000 + month as i64 * 100 + day as i64) as u64;
    let mut rng = fastrand::Rng::with_seed(seed.wrapping_mul(0x9e37_79b9_7f4a_7c15) ^ 0x6865_6176);
    if rng.f64() >= HEAVY_SNOW_DAY_CHANCE {
        return Vec::new();
    }
    let count = rng.u32(HEAVY_SNOWS_PER_DAY.0..=HEAVY_SNOWS_PER_DAY.1);
    let slot = (HEAVY_SNOW_HOURS.1 - HEAVY_SNOW_HOURS.0) / count;
    (0..count)
        .map(|i| {
            let len = rng.u32(HEAVY_SNOW_SECS.0..=HEAVY_SNOW_SECS.1);
            let start = HEAVY_SNOW_HOURS.0 + i * slot + rng.u32(0..=slot - len);
            (start, start + len)
        })
        .collect()
}

/// 지금 달력이 말하는 날씨 (secs: 자정부터 초). 7~9월 태풍 칸이면 태풍, 12~2월 함박눈 칸이면 함박눈, 여름 소나기 사이의 맑은 때는 푸른 잎이 날림
pub fn weather_at(year: i32, month: u32, day: u32, secs: u32) -> Option<Weather> {
    if typhoons(year, month, day).iter().any(|&(start, end)| start <= secs && secs < end) {
        return Some(Weather::Typhoon);
    }
    if heavy_snows(year, month, day).iter().any(|&(start, end)| start <= secs && secs < end) {
        return Some(Weather::HeavySnow);
    }
    let weather = season(month, day);
    if weather != Weather::Shower || showers(year, month, day).iter().any(|&(start, end)| start <= secs && secs < end) {
        Some(weather)
    } else {
        Some(Weather::GreenLeaves)
    }
}

/// 개발용 미리 보기 값 (환경 변수 SCREENPET_WEATHER): snow·light_snow·heavy_snow·rain·shower·petals·leaves·green_leaves·typhoon·off. 그 밖의 값은 미리 보기 아님 (None).
/// shower는 달력과 달리 쉬지 않고 내림
pub fn parse_preview(raw: &str) -> Option<Option<Weather>> {
    match raw.trim().to_ascii_lowercase().as_str() {
        "snow" => Some(Some(Weather::Snow)),
        "light_snow" => Some(Some(Weather::LightSnow)),
        "heavy_snow" => Some(Some(Weather::HeavySnow)),
        "rain" => Some(Some(Weather::Rain)),
        "shower" => Some(Some(Weather::Shower)),
        "petals" => Some(Some(Weather::Petals)),
        "leaves" => Some(Some(Weather::Leaves)),
        "green_leaves" => Some(Some(Weather::GreenLeaves)),
        "typhoon" => Some(Some(Weather::Typhoon)),
        "off" => Some(None),
        _ => None,
    }
}

/// 지금 보일 날씨. 미리 보기는 달력과 메뉴를 넘어서고, 트레이로 모두 숨겨 둔 동안엔 아무것도 없음
pub fn wanted(season: Option<Weather>, preview: Option<Option<Weather>>, menu_off: bool, hidden: bool) -> Option<Weather> {
    if hidden {
        return None;
    }
    match preview {
        Some(p) => p,
        None if menu_off => None,
        None => season,
    }
}

/// 눈사람이 서 있는 시간 (초)
pub const SNOWMAN_SECS: f64 = 3600.0;

/// 눈사람이 계속 서 있는가: 눈 오는 날, 바닥에 눈이 남아 있고, 한 시간 안
pub fn snowman_stays(weather: Option<Weather>, age_secs: f64, snow_on_ground: bool) -> bool {
    weather.is_some_and(Weather::is_snow) && snow_on_ground && age_secs < SNOWMAN_SECS
}

/// 작업 영역 밖으로 이만큼(물리 px) 넘어야 전체 화면으로 봄 — 최대화한 창은 작업 영역에 딱 맞고, 작업표시줄은 이보다 높음
const FULLSCREEN_OVERHANG: f64 = 16.0;

/// 이 모니터의 맨 앞 창(가운데를 덮는 첫 창)이 작업표시줄까지 덮는 전체 화면인가.
/// 작업표시줄을 자동으로 숨긴 화면에선 최대화와 구별되지 않아 false임
pub fn fullscreen_over(windows: &[WindowInfo], wa: &Rect) -> bool {
    let (cx, cy) = ((wa.left + wa.right) / 2.0, (wa.top + wa.bottom) / 2.0);
    let front = windows.iter().find(|w| w.rect.left <= cx && cx < w.rect.right && w.rect.top <= cy && cy < w.rect.bottom);
    let Some(r) = front.map(|w| w.rect) else { return false };
    let covers = r.left <= wa.left + 1.0 && r.top <= wa.top + 1.0 && r.right >= wa.right - 1.0 && r.bottom >= wa.bottom - 1.0;
    let overhang = (wa.left - r.left).max(wa.top - r.top).max(r.right - wa.right).max(r.bottom - wa.bottom);
    covers && overhang >= FULLSCREEN_OVERHANG
}

/// 어느 모니터든 전체 화면 앱이 덮고 있음 (방해하면 안 되는 창을 미룰 때)
pub fn any_fullscreen(windows: &[WindowInfo], work_areas: &[Rect]) -> bool {
    work_areas.iter().any(|wa| fullscreen_over(windows, wa))
}

/// 이 모니터의 날씨를 멈춰 둘 때: 사용자가 자리를 비웠거나 전체 화면 앱이 덮고 있음 (창을 숨기고 웹뷰 타이머도 멈춤)
pub fn overlay_paused(away: bool, windows: &[WindowInfo], wa: &Rect) -> bool {
    away || fullscreen_over(windows, wa)
}

/// 날씨 창을 다시 만들어 보는 첫 간격과 상한
const RETRY_FIRST: Duration = Duration::from_secs(5);
const RETRY_MAX: Duration = Duration::from_secs(60);

/// 날씨 창 만들기가 실패하면 다음 시도까지 기다림 (5초, 10초, … 1분). 장면 주기(5 Hz)로 되풀이하지 않게
#[derive(Debug, Clone, Copy, Default)]
pub struct OverlayRetry {
    wait: Option<Duration>,
    next: Option<Instant>,
}

impl OverlayRetry {
    /// 지금 다시 시도해도 됨
    pub fn ready(&self, now: Instant) -> bool {
        self.next.is_none_or(|at| now >= at)
    }

    /// 실패: 기다림을 두 배로 늘림 (RETRY_MAX까지)
    pub fn failed(&mut self, now: Instant) {
        let wait = self.wait.map_or(RETRY_FIRST, |w| (w * 2).min(RETRY_MAX));
        self.wait = Some(wait);
        self.next = Some(now + wait);
    }

    /// 성공: 기다림을 지움
    pub fn reset(&mut self) {
        *self = Self::default();
    }
}

/// 더미 한 칸의 폭 (물리 px)
const CELL: f64 = 20.0;
/// 한 번에 얹는 눈덩이 높이 — 그 칸에 이만큼, 양옆 칸에 절반씩 (칸 수로 치면 이 값의 2배가 쌓임)
const GRAIN: f32 = 0.6;
const GRAIN_TOTAL: f32 = GRAIN * 2.0;
/// 더미를 세는 창 수 (앞쪽 창부터, 바닥은 따로)
const MAX_PILES: usize = 16;
/// 바닥 평균 높이가 이만큼(논리 px)이면 눈이 쌓인 것임 — 눈 오는 날 약 4분, 가벼운 눈 약 6분
const SETTLED_DEPTH: f32 = 3.0;
/// 바닥 평균 높이가 이만큼(논리 px)은 있어야 아직 눈이 남은 것임
const SNOW_LEFT_DEPTH: f32 = 0.5;
/// 발이 이만큼(물리 px) 안이면 그 디딤판 위에 선 것임
const SAME_SURFACE: f64 = 4.0;
/// 더미를 이만큼(초, 날씨가 켜져 있고 자리에 있는 시간)마다 저절로 싹 치움
pub const AUTO_CLEAR_SECS: f64 = 3600.0;

/// 창 하나(또는 바닥 하나)의 더미
struct Pile {
    owner: Owner,
    left: f64,
    right: f64,
    y: f64,
    cells: Vec<f32>,
}

impl Pile {
    /// 빈 더미를 만듦 (CELL 폭 칸들)
    fn new(owner: Owner, left: f64, right: f64, y: f64) -> Self {
        let n = ((right - left) / CELL).ceil().max(1.0) as usize;
        Self { owner, left, right, y, cells: vec![0.0; n] }
    }

    /// 칸 평균 높이
    fn average(&self) -> f32 {
        self.cells.iter().sum::<f32>() / self.cells.len() as f32
    }

    /// x 칸에 알갱이 하나를 쌓음 (양옆에 반씩, cap까지)
    fn drop_grain(&mut self, x: f64, cap: f32) {
        let i = (((x - self.left) / CELL) as usize).min(self.cells.len() - 1);
        self.cells[i] = (self.cells[i] + GRAIN).min(cap);
        for n in [i.wrapping_sub(1), i + 1] {
            if let Some(c) = self.cells.get_mut(n) {
                *c = (*c + GRAIN / 2.0).min(cap);
            }
        }
    }
}

/// 보이는 디딤판 조각 (앞쪽 창에 가려진 곳은 빠짐)
#[derive(Clone, Copy)]
struct Top {
    owner: Owner,
    x0: f64,
    x1: f64,
    y: f64,
}

/// 웹뷰에 보내는 한 모니터의 장면
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Scene {
    /// 내리는 날씨
    pub kind: Weather,
    /// 더미의 그림 (쌓인 뒤 비가 오면 녹는 동안 눈 모양 그대로)
    pub pile_kind: Option<Weather>,
    /// 알갱이가 내려앉는 곳: [왼쪽, 오른쪽, 높이]
    pub tops: Vec<[f32; 3]>,
    /// 더미들
    pub piles: Vec<PileView>,
    /// 🌀 태풍 바람 (부호 = 방향, 0.7…1.6 — 돌풍에 커짐). 태풍이 아니면 없음 (예전 장면 그대로)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub wind: Option<f32>,
    /// 🌀 회오리 [바닥 가운데 x, 바닥 y, 가로 빠르기] (논리 px, px/s — 웹뷰가 사이를 이어 그림). 이 모니터에 없으면 없음
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tornado: Option<[f32; 3]>,
}

/// 한 디딤판의 더미 그림 (모니터 작업 영역 기준 논리 px)
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct PileView {
    /// 왼쪽 x
    pub x: f32,
    /// 윗면 y
    pub y: f32,
    /// 칸 폭
    pub cell: f32,
    /// 칸별 높이 (0.1 논리 px 단위)
    pub h: Vec<u8>,
    /// 보이는 조각: [왼쪽, 오른쪽]
    pub clips: Vec<[f32; 2]>,
}

/// 창·바닥에 쌓이는 눈·꽃잎·낙엽 더미들
pub struct PileField {
    piles: Vec<Pile>,
    tops: Vec<Top>,
    pile_kind: Option<Weather>,
    /// 다음 틱으로 넘기는 눈덩이 몫
    carry: f32,
    /// 쌓이는 속도 배수 (미리 보기는 빨리)
    grow_mult: f32,
    /// 마지막으로 싹 치운 뒤 흐른 시간 (초)
    since_clear: f64,
    rng: fastrand::Rng,
}

impl PileField {
    /// 빈 더미 밭. grow_mult는 쌓이는 속도 배수
    pub fn new(seed: u64, grow_mult: f32) -> Self {
        Self { piles: Vec::new(), tops: Vec::new(), pile_kind: None, carry: 0.0, grow_mult, since_clear: 0.0, rng: fastrand::Rng::with_seed(seed) }
    }

    /// 지금 세계에 맞춤: 옮기거나 크기가 바뀐 창의 더미는 사라지고 (윗면 전체가 가려졌거나 닫힌 창도), 나머지는 그대로
    pub fn sync(&mut self, world: &World) {
        self.tops.clear();
        let mut old = std::mem::take(&mut self.piles);
        let mut windows = 0;
        for s in &world.surfaces {
            let edge = match s.owner {
                Owner::Window(id) => world.window(id).map(|w| (w.rect.left, w.rect.right)),
                Owner::Ground(i) => world.work_areas.get(i).map(|wa| (wa.left, wa.right)),
                Owner::Inside(_) => None,
            };
            let Some((left, right)) = edge else { continue };
            let known = self.piles.iter().any(|p| p.owner == s.owner);
            if !known {
                if matches!(s.owner, Owner::Window(_)) {
                    if windows == MAX_PILES {
                        continue;
                    }
                    windows += 1;
                }
                let kept = old.iter().position(|p| p.owner == s.owner && p.left == left && p.right == right && p.y == s.y);
                self.piles.push(kept.map_or_else(|| Pile::new(s.owner, left, right, s.y), |i| old.swap_remove(i)));
            }
            self.tops.push(Top { owner: s.owner, x0: s.x_start, x1: s.x_end, y: s.y });
        }
    }

    /// 시간을 dt초 보냄 (앱은 1초에 한 번). 녹이고, 쌓이는 날씨면 보이는 조각에 눈덩이를 얹음
    pub fn tick(&mut self, dt: f64, weather: Weather) {
        self.since_clear += dt;
        if self.since_clear >= AUTO_CLEAR_SECS {
            self.clear_all();
        }
        let rates = weather.rates();
        let piles_up = rates.grow > 0.0;
        if piles_up && self.pile_kind.is_some_and(|k| k.pile_family() != weather.pile_family()) {
            // 눈이 꽃잎·낙엽으로, 낙엽이 눈이나 꽃잎으로: 다른 더미를 새로 시작함 (가벼운 눈·눈·함박눈은 이어 쌓음)
            self.piles.iter_mut().for_each(|p| p.cells.fill(0.0));
        }
        if piles_up {
            self.pile_kind = Some(weather);
        }
        let dt = dt as f32;
        for c in self.piles.iter_mut().flat_map(|p| &mut p.cells) {
            *c = (*c - rates.melt * dt).max(0.0);
        }
        if !piles_up {
            return;
        }
        let width: f64 = self.tops.iter().map(|t| t.x1 - t.x0).sum();
        self.carry += (width / CELL) as f32 * rates.grow * self.grow_mult * dt / GRAIN_TOTAL;
        while self.carry >= 1.0 {
            self.carry -= 1.0;
            let mut at = self.rng.f64() * width;
            let Some(top) = self.tops.iter().find(|t| {
                let w = t.x1 - t.x0;
                let hit = at < w;
                if !hit {
                    at -= w;
                }
                hit
            }) else {
                continue;
            };
            let x = top.x0 + at;
            if let Some(p) = self.piles.iter_mut().find(|p| p.owner == top.owner) {
                p.drop_grain(x, rates.cap);
            }
        }
    }

    /// 눈이 바닥에 한동안 내려 쌓였음 (눈사람을 만들 만큼)
    pub fn snow_settled(&self) -> bool {
        self.ground_snow_at_least(SETTLED_DEPTH)
    }

    /// 바닥에 눈이 아직 남아 있음 (눈사람이 서 있을 만큼)
    pub fn snow_on_ground(&self) -> bool {
        self.ground_snow_at_least(SNOW_LEFT_DEPTH)
    }

    /// 바닥 눈이 depth 이상 쌓였음 (눈사람 만들기용)
    fn ground_snow_at_least(&self, depth: f32) -> bool {
        self.pile_kind.is_some_and(Weather::is_snow) && self.piles.iter().any(|p| matches!(p.owner, Owner::Ground(_)) && p.average() >= depth)
    }

    /// 모니터(작업 영역 wa, 배율 scale) 하나의 장면. 그 모니터에 걸친 것만
    /// 지금 더미의 그림
    pub fn pile_kind(&self) -> Option<Weather> {
        self.pile_kind
    }

    /// 🧹 [x0, x1]를 지나며 높이 y(±reach_y)의 보이는 더미를 씀: 쓴 양(칸 높이의 합, 논리 px)
    pub fn sweep(&mut self, x0: f64, x1: f64, y: f64, reach_y: f64) -> f32 {
        self.take_in(x0, x1, y, reach_y, true)
    }

    /// [x0, x1]의 높이 y(±reach_y)에 보이는 더미 양 (칸 높이의 합, 논리 px). take면 그 칸들을 비움
    fn take_in(&mut self, x0: f64, x1: f64, y: f64, reach_y: f64, take: bool) -> f32 {
        let tops = &self.tops;
        let mut found = 0.0;
        for p in self.piles.iter_mut().filter(|p| (p.y - y).abs() <= reach_y) {
            if !tops.iter().any(|t| t.owner == p.owner && t.x1 > x0 && t.x0 < x1) {
                continue;
            }
            let left = p.left;
            for (i, c) in p.cells.iter_mut().enumerate() {
                let x = left + (i as f64 + 0.5) * CELL;
                if x >= x0 && x <= x1 {
                    found += *c;
                    if take {
                        *c = 0.0;
                    }
                }
            }
        }
        found
    }

    /// [x0, x1]의 높이 y(±reach_y)에 보이는 더미 양 — 비우지 않음
    fn amount_in(&mut self, x0: f64, x1: f64, y: f64, reach_y: f64) -> f32 {
        self.take_in(x0, x1, y, reach_y, false)
    }

    /// ⛄ [x0, x1]의 높이 y(±reach_y)에 보이는 눈 양 (눈 더미가 아니면 0)
    pub fn snow_amount(&mut self, x0: f64, x1: f64, y: f64, reach_y: f64) -> f32 {
        if !self.pile_kind.is_some_and(Weather::is_snow) {
            return 0.0;
        }
        self.amount_in(x0, x1, y, reach_y)
    }

    /// ⛄ 눈덩이를 뭉침: [x0, x1]의 눈을 모아 그 칸들을 비움. 모은 양 (눈 더미가 아니면 0, 아무것도 안 비움)
    pub fn pack_snow(&mut self, x0: f64, x1: f64, y: f64, reach_y: f64) -> f32 {
        if !self.pile_kind.is_some_and(Weather::is_snow) {
            return 0.0;
        }
        self.take_in(x0, x1, y, reach_y, true)
    }

    /// 발밑 더미: x 둘레 ±half_w, 발 높이 y(±SAME_SURFACE) 디딤판의 보이는 더미 그림과 평균 높이 (논리 px). 더미가 없으면 None
    pub fn pile_under(&self, x: f64, y: f64, half_w: f64) -> Option<(Weather, f32)> {
        let kind = self.pile_kind?;
        let (x0, x1) = (x - half_w, x + half_w);
        let p = self.piles.iter().find(|p| (p.y - y).abs() <= SAME_SURFACE && x >= p.left && x <= p.right)?;
        if !self.tops.iter().any(|t| t.owner == p.owner && t.x0 <= x && x <= t.x1) {
            return None;
        }
        let (sum, n) = p.cells.iter().enumerate().fold((0.0f32, 0u32), |(sum, n), (i, &c)| {
            let cx = p.left + (i as f64 + 0.5) * CELL;
            if cx >= x0 && cx <= x1 { (sum + c, n + 1) } else { (sum, n) }
        });
        let depth = if n == 0 { 0.0 } else { sum / n as f32 };
        (depth >= 0.05).then_some((kind, depth))
    }

    /// 보이는 만큼 쌓인 더미가 어딘가 있음 (🌀 돌풍에 흩날릴 것)
    pub fn has_piles(&self) -> bool {
        self.piles.iter().flat_map(|p| &p.cells).any(|&c| c >= 0.05)
    }

    /// 🧹 모든 더미를 싹 치움 (쓰레기 다 치우기·한 시간마다): 치운 게 있었으면 true. 한 시간을 새로 셈
    pub fn clear_all(&mut self) -> bool {
        self.since_clear = 0.0;
        let any = self.piles.iter().flat_map(|p| &p.cells).any(|&c| c > 0.0);
        self.piles.iter_mut().for_each(|p| p.cells.fill(0.0));
        any
    }

    /// 한 모니터(wa)의 장면을 만듦 (scale = 화면 배율, 논리 px로 바꿈)
    pub fn scene(&self, kind: Weather, wa: &Rect, scale: f64) -> Scene {
        let lx = |x: f64| ((x.clamp(wa.left, wa.right) - wa.left) / scale).round() as f32;
        let ly = |y: f64| ((y - wa.top) / scale).round() as f32;
        let on_screen = |t: &&Top| t.x1 > wa.left && t.x0 < wa.right && t.y > wa.top && t.y <= wa.bottom;
        let tops = self.tops.iter().filter(on_screen).map(|t| [lx(t.x0), lx(t.x1), ly(t.y)]).collect();
        let piles = self
            .piles
            .iter()
            .filter(|p| p.cells.iter().any(|&c| c >= 0.05))
            .filter_map(|p| {
                let clips: Vec<[f32; 2]> = self.tops.iter().filter(|t| t.owner == p.owner).filter(on_screen).map(|t| [lx(t.x0), lx(t.x1)]).collect();
                (!clips.is_empty()).then(|| PileView {
                    x: ((p.left - wa.left) / scale).round() as f32,
                    y: ly(p.y),
                    cell: (CELL / scale) as f32,
                    h: p.cells.iter().map(|&c| (c * 10.0).round().min(255.0) as u8).collect(),
                    clips,
                })
            })
            .collect();
        Scene { kind, pile_kind: self.pile_kind, tops, piles, wind: None, tornado: None }
    }
}

#[cfg(test)]
#[path = "weather_tests.rs"]
mod tests;
