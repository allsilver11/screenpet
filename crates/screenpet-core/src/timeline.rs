//! 내 동물 타임라인 — 최근 한 시간 동안 어디에 얼마나 있었는지, 무엇을 먹었는지.
//!
//! 메모리에만 둔다: 저장하지 않고 친구에게 보내지도 않는다.
//! 시간은 `tick`이 받는 dt(초)만 더한다 — 벽시계는 웹뷰가 "몇 초 전"에서 계산한다.
//!
//! - 자리는 셋뿐이다: 집에서 놀기(내 화면) / 하우스 / 친구네. 한 번 바뀌면 몇 분씩 머무는 것들이라 바뀌는 순간 새 칸이 된다.
//! - 자리를 모를 때(`None` — 친구 화면으로 넘어가는 사이)는 앞 칸이 이어진다.
//! - 친구가 받지 않아(바쁨·꽉 참) 곧바로 팅겨 돌아온 방문(`visit::QUICK_DECLINE_SECS` 안)은 지우고 앞 칸에 붙인다.
//! - 틱 사이가 길어도(메뉴가 열려 게임 루프가 멈춤·컴퓨터 잠자기) 그 시간은 지금 자리에 그대로 쌓는다 — 같은 자리는 쪼개지 않는다.
//! - 먹은 것은 시간이 아니라 횟수로 센다 (먹기는 몇 초뿐이다).

use std::collections::VecDeque;

use serde::Serialize;

use crate::care::Food;
use crate::manifest::FavoriteFood;
use crate::visit::QUICK_DECLINE_SECS;

/// 보여 주는 기간 (초)
pub const WINDOW_SECS: f64 = 3600.0;
/// 칸·먹은 것 수 상한 (오래된 것부터 버린다 — 메모리가 끝없이 늘지 않게)
pub const MAX_ENTRIES: usize = 400;

/// 내 동물이 있던 곳
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Place {
    /// 내 화면에서 노는 중 (바닥·창 위·창 안 어디든)
    Play,
    /// 하우스 안 (하우스에서 나와 문 앞에서 먹고 다시 들어갈 참인 것도)
    House,
    /// 친구 화면에 놀러 감 (친구 id)
    Friend { id: String },
}

impl Place {
    /// 목록·막대에 쓰는 이모지 (Windows 10 글꼴에 있는 오래된 것만)
    pub fn emoji(&self) -> &'static str {
        match self {
            Place::Play => "🎈",
            Place::House => "🏠",
            Place::Friend { .. } => "🐾",
        }
    }

    /// 한 줄 이름. friend_name = 친구 id → 보일 이름 (모르면 None)
    pub fn label(&self, friend_name: &dyn Fn(&str) -> Option<String>) -> String {
        match self {
            Place::Play => "집에서 놀기".into(),
            Place::House => "하우스".into(),
            Place::Friend { id } => match friend_name(id) {
                Some(n) => format!("{n}네 놀러 감"),
                None => "친구네 놀러 감".into(),
            },
        }
    }
}

/// 내 동물 상태 → 자리. away_friend = 내 동물이 가 있는 친구 id (화면을 떠났는데 아직 모르면 None — 앞 자리로 센다)
pub fn place_of(housed: bool, away: bool, away_friend: Option<&str>) -> Option<Place> {
    if housed {
        Some(Place::House)
    } else if away {
        away_friend.map(|id| Place::Friend { id: id.to_string() })
    } else {
        Some(Place::Play)
    }
}

/// 먹은 것 한 가지의 이모지·이름 (메뉴 글자와 같다). 좋아하는 음식은 그때 동물의 것
fn food_name(food: Food, favorite: &FavoriteFood) -> (String, String) {
    let (e, n) = match food {
        Food::Meal => ("🍚", "밥"),
        Food::Special => ("🍱", "특식"),
        Food::Snack => ("🍪", "간식"),
        Food::Water => ("💧", "물"),
        Food::Favorite => return (favorite.emoji.clone(), favorite.name.clone()),
    };
    (e.into(), n.into())
}

/// 목록 순서: 밥, 좋아하는 음식, 특식, 간식, 물
fn food_rank(food: Food) -> u8 {
    match food {
        Food::Meal => 0,
        Food::Favorite => 1,
        Food::Special => 2,
        Food::Snack => 3,
        Food::Water => 4,
    }
}

/// 한 칸: 타임라인 시계(초)로 [start, end)
#[derive(Debug, Clone, PartialEq)]
pub struct Segment {
    pub place: Place,
    pub start: f64,
    pub end: f64,
}

/// 먹은 때 (타임라인 시계 초)
#[derive(Debug, Clone, PartialEq)]
struct Meal {
    at: f64,
    rank: u8,
    emoji: String,
    name: String,
}

#[derive(Debug, Clone, Default)]
pub struct Timeline {
    /// 타임라인 시계 (초, 처음 0)
    now: f64,
    segments: VecDeque<Segment>,
    meals: VecDeque<Meal>,
}

impl Timeline {
    pub fn new() -> Timeline {
        Timeline::default()
    }

    /// dt초 지났고 지금 자리는 place (모르면 None — 앞 칸이 이어진다)
    pub fn tick(&mut self, place: Option<Place>, dt: f64) {
        if !dt.is_finite() || dt <= 0.0 {
            return;
        }
        self.now += dt;
        match place {
            // 새 자리: 바로 새 칸 (앞 칸은 여기서 끝난다)
            Some(p) if self.segments.back().is_none_or(|s| s.place != p) => {
                let mut start = self.now - dt;
                // 친구네에서 곧바로 팅겨 왔다 — 그 방문은 없던 것으로 (앞 칸이 같은 자리면 이어지고, 아니면 새 칸이 그때부터)
                if self.segments.back().is_some_and(|s| matches!(s.place, Place::Friend { .. }) && s.end - s.start < QUICK_DECLINE_SECS) {
                    start = self.segments.pop_back().map_or(start, |s| s.start);
                    if let Some(last) = self.segments.back_mut().filter(|s| s.place == p) {
                        last.end = self.now;
                        self.prune();
                        return;
                    }
                }
                self.segments.push_back(Segment { place: p, start, end: self.now });
            }
            // 원래 자리이거나 모름: 앞 칸이 이어진다 (아직 칸이 없으면 아무것도 안 한다)
            _ => self.extend(),
        }
        self.prune();
    }

    /// 내 동물이 먹었다 (배불러 안 먹은 것은 부르지 않는다)
    pub fn ate(&mut self, food: Food, favorite: &FavoriteFood) {
        let (emoji, name) = food_name(food, favorite);
        self.meals.push_back(Meal { at: self.now, rank: food_rank(food), emoji, name });
        self.prune();
    }

    /// 마지막 칸을 지금까지 늘린다
    fn extend(&mut self) {
        if let Some(last) = self.segments.back_mut() {
            last.end = self.now;
        }
    }

    /// 기간 밖·상한 넘는 것을 버린다
    fn prune(&mut self) {
        let cut = self.now - WINDOW_SECS;
        while self.segments.front().is_some_and(|s| s.end <= cut) || self.segments.len() > MAX_ENTRIES {
            self.segments.pop_front();
        }
        while self.meals.front().is_some_and(|m| m.at <= cut) || self.meals.len() > MAX_ENTRIES {
            self.meals.pop_front();
        }
    }

    /// 최근 WINDOW_SECS 안의 칸들 (오래된 것부터). 첫 칸은 기간 시작에서 잘린다
    pub fn spans(&self) -> Vec<Span> {
        let cut = self.now - WINDOW_SECS;
        self.segments
            .iter()
            .filter(|s| s.end > cut)
            .map(|s| Span { place: s.place.clone(), ago_start: self.now - s.start.max(cut), ago_end: self.now - s.end })
            .collect()
    }

    /// 자리별 머문 시간 (초, 많은 것부터 — 같으면 먼저 나온 자리 먼저)
    pub fn totals(&self) -> Vec<(Place, f64)> {
        let mut out: Vec<(Place, f64)> = Vec::new();
        for s in self.spans() {
            let secs = s.ago_start - s.ago_end;
            match out.iter_mut().find(|(p, _)| *p == s.place) {
                Some((_, t)) => *t += secs,
                None => out.push((s.place, secs)),
            }
        }
        out.sort_by(|a, b| b.1.total_cmp(&a.1));
        out
    }

    /// 창에 보낼 내용. friend_name = 친구 id → 보일 이름
    pub fn view(&self, friend_name: &dyn Fn(&str) -> Option<String>) -> TimelineView {
        let totals = self.totals();
        let row_of = |p: &Place| totals.iter().position(|(q, _)| q == p).unwrap_or(0);
        let spans: Vec<SpanView> = self.spans().into_iter().map(|s| SpanView { row: row_of(&s.place), ago_start: s.ago_start, ago_end: s.ago_end }).collect();
        // 마지막 칸이 지금 자리
        let now = spans.last().map(|s| s.row);
        // 먹은 것: 종류별 횟수 (정해진 순서) + 먹은 때들 (오래된 것부터)
        let mut foods: Vec<(u8, FoodCount)> = Vec::new();
        for m in &self.meals {
            match foods.iter_mut().find(|(_, f)| f.name == m.name) {
                Some((_, f)) => f.count += 1,
                None => foods.push((m.rank, FoodCount { emoji: m.emoji.clone(), name: m.name.clone(), count: 1 })),
            }
        }
        foods.sort_by_key(|(rank, _)| *rank);
        TimelineView {
            window_secs: WINDOW_SECS,
            rows: totals.iter().map(|(p, secs)| Row { emoji: p.emoji(), label: p.label(friend_name), secs: *secs }).collect(),
            spans,
            now,
            foods: foods.into_iter().map(|(_, f)| f).collect(),
            meals: self.meals.iter().map(|m| MealView { emoji: m.emoji.clone(), name: m.name.clone(), ago: self.now - m.at }).collect(),
        }
    }
}

/// 보여 줄 한 칸: 지금으로부터 몇 초 전에 시작해 몇 초 전에 끝났는지 (지금 칸이면 ago_end = 0)
#[derive(Debug, Clone, PartialEq)]
pub struct Span {
    pub place: Place,
    pub ago_start: f64,
    pub ago_end: f64,
}

/// 타임라인 창 내용 (웹뷰 `timeline-state` 이벤트·`timeline_state` 커맨드)
#[derive(Debug, Clone, Default, Serialize)]
pub struct TimelineView {
    /// 보여 주는 기간 (초)
    pub window_secs: f64,
    /// 자리별 머문 시간 (많은 것부터) — 색은 이 순번으로
    pub rows: Vec<Row>,
    /// 칸들 (오래된 것부터)
    pub spans: Vec<SpanView>,
    /// 지금 있는 자리의 rows 순번
    pub now: Option<usize>,
    /// 먹은 것 종류별 횟수 (밥, 좋아하는 음식, 특식, 간식, 물 순)
    pub foods: Vec<FoodCount>,
    /// 먹은 때들 (오래된 것부터)
    pub meals: Vec<MealView>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Row {
    pub emoji: &'static str,
    pub label: String,
    /// 머문 시간 (초)
    pub secs: f64,
}

#[derive(Debug, Clone, Serialize)]
pub struct SpanView {
    /// rows 순번
    pub row: usize,
    /// 지금으로부터 몇 초 전 시작 / 끝
    pub ago_start: f64,
    pub ago_end: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct FoodCount {
    pub emoji: String,
    pub name: String,
    pub count: u32,
}

#[derive(Debug, Clone, Serialize)]
pub struct MealView {
    pub emoji: String,
    pub name: String,
    /// 몇 초 전
    pub ago: f64,
}

#[cfg(test)]
#[path = "timeline_tests.rs"]
mod tests;
