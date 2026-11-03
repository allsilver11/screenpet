//! 계절 날씨 (v0.3.9, 일 년 내내) — 달력으로 날씨를 정하고, 창 윗면과 바닥에 쌓이는 눈·꽃잎·낙엽 더미를 센다. 그림은 앱이 그린다 (ui/weather.js).
//!
//! CPU를 아끼려고 알갱이 하나하나를 세지 않는다: 떨어지는 알갱이는 웹뷰의 그림일 뿐이고, 더미는 여기서 1초에 한 번
//! 작은 눈덩이를 무작위 칸에 얹어 키운다 (쌓이는 속도와 녹는 속도만 맞춘다). 앱은 장면이 바뀌었을 때만 웹뷰에 보낸다.
//!
//! 좌표: 세계는 물리 px. 장면(Scene)은 모니터별 논리 px, 그 모니터의 작업 영역 왼쪽 위가 (0, 0). 더미 높이는 논리 px.

use std::time::{Duration, Instant};

use serde::Serialize;

use crate::geom::{Rect, WindowInfo};
use crate::surfaces::Owner;
use crate::world::World;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Weather {
    /// 11월의 가벼운 눈
    LightSnow,
    Snow,
    Rain,
    /// 5~8월의 소나기 (잠깐씩만, showers)
    Shower,
    /// 3·4월의 벚꽃잎
    Petals,
    /// 9~11월의 낙엽
    Leaves,
}

impl Weather {
    pub fn is_snow(self) -> bool {
        matches!(self, Weather::LightSnow | Weather::Snow)
    }

    /// 펫이 자꾸 올려다보는 날씨
    pub fn looks_up(self) -> bool {
        matches!(self, Weather::Petals | Weather::Leaves)
    }

    fn rates(self) -> Rates {
        match self {
            Weather::LightSnow => Rates { grow: 0.012, melt: 0.003, cap: 5.0 },
            Weather::Snow => Rates { grow: 0.02, melt: 0.004, cap: 9.0 },
            // 비는 쌓이지 않고, 남아 있던 더미를 빨리 녹인다
            Weather::Rain | Weather::Shower => Rates { grow: 0.0, melt: 0.05, cap: 0.0 },
            Weather::Petals => Rates { grow: 0.008, melt: 0.004, cap: 3.5 },
            Weather::Leaves => Rates { grow: 0.008, melt: 0.004, cap: 4.0 },
        }
    }
}

/// 더미 높이 (논리 px, 초당)
struct Rates {
    grow: f32,
    melt: f32,
    cap: f32,
}

/// 지역 날짜의 날씨 (여름의 Shower는 하루 중 잠깐씩만이다, weather_at): 3·4월 벚꽃잎, 5~8월 소나기, 9·10월 낙엽,
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

/// 그날의 소나기들: [시작, 끝) 초. 날짜로만 정해져서 앱을 다시 켜도 같다. 시간대를 똑같이 나눈 칸마다 하나씩이라 겹치지 않는다
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

/// 지금 달력이 말하는 날씨 (secs: 자정부터 초). None이면 소나기 사이의 맑은 때 — 앱은 창을 닫아 둔다
pub fn weather_at(year: i32, month: u32, day: u32, secs: u32) -> Option<Weather> {
    let weather = season(month, day);
    if weather != Weather::Shower || showers(year, month, day).iter().any(|&(start, end)| start <= secs && secs < end) {
        Some(weather)
    } else {
        None
    }
}

/// 개발용 미리 보기 값 (환경 변수 SCREENPET_WEATHER): snow·light_snow·rain·shower·petals·leaves·off. 그 밖의 값은 미리 보기 아님 (None).
/// shower는 달력과 달리 쉬지 않고 내린다
pub fn parse_preview(raw: &str) -> Option<Option<Weather>> {
    match raw.trim().to_ascii_lowercase().as_str() {
        "snow" => Some(Some(Weather::Snow)),
        "light_snow" => Some(Some(Weather::LightSnow)),
        "rain" => Some(Some(Weather::Rain)),
        "shower" => Some(Some(Weather::Shower)),
        "petals" => Some(Some(Weather::Petals)),
        "leaves" => Some(Some(Weather::Leaves)),
        "off" => Some(None),
        _ => None,
    }
}

/// 지금 보일 날씨. 미리 보기는 달력과 메뉴를 넘어서고, 트레이로 모두 숨겨 둔 동안엔 아무것도 없다
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

/// 작업 영역 밖으로 이만큼(물리 px) 넘어야 전체 화면으로 본다 — 최대화한 창은 작업 영역에 딱 맞고, 작업표시줄은 이보다 높다
const FULLSCREEN_OVERHANG: f64 = 16.0;

/// 이 모니터의 맨 앞 창(가운데를 덮는 첫 창)이 작업표시줄까지 덮는 전체 화면인가.
/// 작업표시줄을 자동으로 숨긴 화면에선 최대화와 구별되지 않아 false다
pub fn fullscreen_over(windows: &[WindowInfo], wa: &Rect) -> bool {
    let (cx, cy) = ((wa.left + wa.right) / 2.0, (wa.top + wa.bottom) / 2.0);
    let front = windows.iter().find(|w| w.rect.left <= cx && cx < w.rect.right && w.rect.top <= cy && cy < w.rect.bottom);
    let Some(r) = front.map(|w| w.rect) else { return false };
    let covers = r.left <= wa.left + 1.0 && r.top <= wa.top + 1.0 && r.right >= wa.right - 1.0 && r.bottom >= wa.bottom - 1.0;
    let overhang = (wa.left - r.left).max(wa.top - r.top).max(r.right - wa.right).max(r.bottom - wa.bottom);
    covers && overhang >= FULLSCREEN_OVERHANG
}

/// 이 모니터의 날씨를 멈춰 둘 때: 사용자가 자리를 비웠거나 전체 화면 앱이 덮고 있다 (창을 숨기고 웹뷰 타이머도 멈춘다)
pub fn overlay_paused(away: bool, windows: &[WindowInfo], wa: &Rect) -> bool {
    away || fullscreen_over(windows, wa)
}

/// 날씨 창을 다시 만들어 보는 첫 간격과 상한
const RETRY_FIRST: Duration = Duration::from_secs(5);
const RETRY_MAX: Duration = Duration::from_secs(60);

/// 날씨 창 만들기가 실패하면 다음 시도까지 기다린다 (5초, 10초, … 1분). 장면 주기(5 Hz)로 되풀이하지 않게
#[derive(Debug, Clone, Copy, Default)]
pub struct OverlayRetry {
    wait: Option<Duration>,
    next: Option<Instant>,
}

impl OverlayRetry {
    pub fn ready(&self, now: Instant) -> bool {
        self.next.is_none_or(|at| now >= at)
    }

    pub fn failed(&mut self, now: Instant) {
        let wait = self.wait.map_or(RETRY_FIRST, |w| (w * 2).min(RETRY_MAX));
        self.wait = Some(wait);
        self.next = Some(now + wait);
    }

    pub fn reset(&mut self) {
        *self = Self::default();
    }
}

/// 더미 한 칸의 폭 (물리 px)
const CELL: f64 = 20.0;
/// 한 번에 얹는 눈덩이 높이 — 그 칸에 이만큼, 양옆 칸에 절반씩 (칸 수로 치면 이 값의 2배가 쌓인다)
const GRAIN: f32 = 0.6;
const GRAIN_TOTAL: f32 = GRAIN * 2.0;
/// 더미를 세는 창 수 (앞쪽 창부터, 바닥은 따로)
const MAX_PILES: usize = 16;
/// 바닥 평균 높이가 이만큼(논리 px)이면 눈이 쌓인 것이다 — 눈 오는 날 약 4분, 가벼운 눈 약 6분
const SETTLED_DEPTH: f32 = 3.0;
/// 바닥 평균 높이가 이만큼(논리 px)은 있어야 아직 눈이 남은 것이다
const SNOW_LEFT_DEPTH: f32 = 0.5;

/// 창 하나(또는 바닥 하나)의 더미
struct Pile {
    owner: Owner,
    left: f64,
    right: f64,
    y: f64,
    cells: Vec<f32>,
}

impl Pile {
    fn new(owner: Owner, left: f64, right: f64, y: f64) -> Self {
        let n = ((right - left) / CELL).ceil().max(1.0) as usize;
        Self { owner, left, right, y, cells: vec![0.0; n] }
    }

    fn average(&self) -> f32 {
        self.cells.iter().sum::<f32>() / self.cells.len() as f32
    }

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

/// 보이는 디딤판 조각 (앞쪽 창에 가려진 곳은 빠진다)
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
    pub kind: Weather,
    /// 더미의 그림 (쌓인 뒤 비가 오면 녹는 동안 눈 모양 그대로)
    pub pile_kind: Option<Weather>,
    /// 알갱이가 내려앉는 곳: [왼쪽, 오른쪽, 높이]
    pub tops: Vec<[f32; 3]>,
    pub piles: Vec<PileView>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct PileView {
    pub x: f32,
    pub y: f32,
    pub cell: f32,
    /// 칸별 높이 (0.1 논리 px 단위)
    pub h: Vec<u8>,
    /// 보이는 조각: [왼쪽, 오른쪽]
    pub clips: Vec<[f32; 2]>,
}

pub struct PileField {
    piles: Vec<Pile>,
    tops: Vec<Top>,
    pile_kind: Option<Weather>,
    /// 다음 틱으로 넘기는 눈덩이 몫
    carry: f32,
    /// 쌓이는 속도 배수 (미리 보기는 빨리)
    grow_mult: f32,
    rng: fastrand::Rng,
}

impl PileField {
    pub fn new(seed: u64, grow_mult: f32) -> Self {
        Self { piles: Vec::new(), tops: Vec::new(), pile_kind: None, carry: 0.0, grow_mult, rng: fastrand::Rng::with_seed(seed) }
    }

    /// 지금 세계에 맞춘다: 옮기거나 크기가 바뀐 창의 더미는 사라지고 (윗면 전체가 가려졌거나 닫힌 창도), 나머지는 그대로
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

    /// 시간을 dt초 보낸다 (앱은 1초에 한 번). 녹이고, 쌓이는 날씨면 보이는 조각에 눈덩이를 얹는다
    pub fn tick(&mut self, dt: f64, weather: Weather) {
        let rates = weather.rates();
        let piles_up = rates.grow > 0.0;
        if piles_up && self.pile_kind.is_some_and(|k| k.is_snow() != weather.is_snow()) {
            // 눈이 꽃잎·낙엽으로, 낙엽이 눈으로: 다른 더미를 새로 시작한다
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

    /// 눈이 바닥에 한동안 내려 쌓였다 (눈사람을 만들 만큼)
    pub fn snow_settled(&self) -> bool {
        self.ground_snow_at_least(SETTLED_DEPTH)
    }

    /// 바닥에 눈이 아직 남아 있다 (눈사람이 서 있을 만큼)
    pub fn snow_on_ground(&self) -> bool {
        self.ground_snow_at_least(SNOW_LEFT_DEPTH)
    }

    fn ground_snow_at_least(&self, depth: f32) -> bool {
        self.pile_kind.is_some_and(Weather::is_snow) && self.piles.iter().any(|p| matches!(p.owner, Owner::Ground(_)) && p.average() >= depth)
    }

    /// 모니터(작업 영역 wa, 배율 scale) 하나의 장면. 그 모니터에 걸친 것만
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
        Scene { kind, pile_kind: self.pile_kind, tops, piles }
    }
}

#[cfg(test)]
#[path = "weather_tests.rs"]
mod tests;
