//! 돌보기 — 포만감, 숨겨진 체력, 성장 단계, 음식 (spec 2B §1). 시간은 앱이 켜져 있을 때만 흐른다.

use serde::{Deserialize, Serialize};

/// 가득 찬 포만감이 0이 되는 시간 (초)
const EMPTY_SECS: f64 = 1800.0;
const HUNGRY_BELOW: f64 = 0.25;
const FULL_ABOVE: f64 = 0.9;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Stage {
    Baby,
    Kinder,
    Child,
    Teenager,
    Adult,
}

impl Stage {
    /// 몸 크기 배율 (teenager = v1 크기)
    pub fn scale(self) -> f64 {
        match self {
            Stage::Baby => 0.45,
            Stage::Kinder => 0.60,
            Stage::Child => 0.78,
            Stage::Teenager => 1.00,
            Stage::Adult => 1.15,
        }
    }

    /// 다음 단계까지 필요한 체력 (adult는 없음)
    pub fn need(self) -> Option<f64> {
        match self {
            Stage::Baby => Some(60.0),
            Stage::Kinder => Some(100.0),
            Stage::Child => Some(150.0),
            Stage::Teenager => Some(220.0),
            Stage::Adult => None,
        }
    }

    pub fn next(self) -> Option<Stage> {
        match self {
            Stage::Baby => Some(Stage::Kinder),
            Stage::Kinder => Some(Stage::Child),
            Stage::Child => Some(Stage::Teenager),
            Stage::Teenager => Some(Stage::Adult),
            Stage::Adult => None,
        }
    }

    /// 시트 파일 이름에 쓰는 이름 (`sheet_<name>.png`)
    pub fn name(self) -> &'static str {
        match self {
            Stage::Baby => "baby",
            Stage::Kinder => "kinder",
            Stage::Child => "child",
            Stage::Teenager => "teenager",
            Stage::Adult => "adult",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Food {
    Snack,
    Meal,
    /// 동물별 좋아하는 음식
    Favorite,
}

impl Food {
    fn stamina(self) -> f64 {
        match self {
            Food::Snack => 3.0,
            Food::Meal => 10.0,
            Food::Favorite => 15.0,
        }
    }

    fn fill(self, fullness: f64) -> f64 {
        match self {
            Food::Snack => (fullness + 0.4).min(1.0),
            Food::Meal => 1.0,
            Food::Favorite => (fullness + 0.7).min(1.0),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FeedResult {
    /// 배불러서 안 먹음
    Refused,
    Ate { leveled_up: bool },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Care {
    pub stage: Stage,
    /// 숨겨진 체력 — 화면에 표시하지 않는다
    pub stamina: f64,
    /// 0.0(텅 빔) ~ 1.0(가득)
    pub fullness: f64,
    /// 성장 멈추기
    pub locked: bool,
}

impl Default for Care {
    fn default() -> Self {
        Self { stage: Stage::Baby, stamina: 0.0, fullness: 1.0, locked: false }
    }
}

impl Care {
    pub fn tick(&mut self, dt: f64) {
        self.fullness = (self.fullness - dt / EMPTY_SECS).max(0.0);
    }

    pub fn is_hungry(&self) -> bool {
        self.fullness < HUNGRY_BELOW
    }

    pub fn feed(&mut self, food: Food) -> FeedResult {
        if self.fullness > FULL_ABOVE {
            return FeedResult::Refused;
        }
        self.fullness = food.fill(self.fullness);
        if self.locked {
            return FeedResult::Ate { leveled_up: false };
        }
        self.stamina += food.stamina();
        match (self.stage.need(), self.stage.next()) {
            (Some(need), Some(next)) if self.stamina >= need => {
                self.stage = next;
                self.stamina = 0.0;
                FeedResult::Ate { leveled_up: true }
            }
            _ => FeedResult::Ate { leveled_up: false },
        }
    }

    /// 처음부터 다시 키우기 — 성장 멈추기 설정은 유지
    pub fn reset(&mut self) {
        *self = Care { locked: self.locked, ..Care::default() };
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hungry_care() -> Care {
        Care { fullness: 0.1, ..Care::default() }
    }

    #[test]
    fn starts_as_a_full_baby() {
        let c = Care::default();
        assert_eq!((c.stage, c.stamina, c.fullness, c.locked), (Stage::Baby, 0.0, 1.0, false));
        assert!(!c.is_hungry());
    }

    #[test]
    fn empties_in_thirty_minutes_and_gets_hungry_below_a_quarter() {
        let mut c = Care::default();
        c.tick(0.74 * 1800.0);
        assert!(!c.is_hungry());
        c.tick(0.02 * 1800.0);
        assert!(c.is_hungry());
        c.tick(10_000.0);
        assert_eq!(c.fullness, 0.0);
    }

    #[test]
    fn foods_fill_and_strengthen_differently() {
        let mut c = hungry_care();
        assert_eq!(c.feed(Food::Snack), FeedResult::Ate { leveled_up: false });
        assert!((c.fullness - 0.5).abs() < 1e-9 && c.stamina == 3.0);
        let mut c = hungry_care();
        c.feed(Food::Meal);
        assert_eq!((c.fullness, c.stamina), (1.0, 10.0));
        let mut c = hungry_care();
        c.feed(Food::Favorite);
        assert!((c.fullness - 0.8).abs() < 1e-9 && c.stamina == 15.0);
    }

    #[test]
    fn refuses_when_full() {
        let mut c = Care::default();
        assert_eq!(c.feed(Food::Meal), FeedResult::Refused);
        assert_eq!(c.stamina, 0.0);
    }

    #[test]
    fn grows_through_every_stage_and_stops_at_adult() {
        let mut c = Care::default();
        let mut seen = vec![c.stage];
        for _ in 0..200 {
            c.fullness = 0.0;
            if let FeedResult::Ate { leveled_up: true } = c.feed(Food::Meal) {
                seen.push(c.stage);
                assert_eq!(c.stamina, 0.0);
            }
        }
        assert_eq!(seen, vec![Stage::Baby, Stage::Kinder, Stage::Child, Stage::Teenager, Stage::Adult]);
        assert_eq!(c.stage.need(), None);
    }

    #[test]
    fn baby_needs_six_meals() {
        let mut c = Care::default();
        for i in 1..=6 {
            c.fullness = 0.0;
            let r = c.feed(Food::Meal);
            assert_eq!(r, FeedResult::Ate { leveled_up: i == 6 }, "meal {i}");
        }
        assert_eq!(c.stage, Stage::Kinder);
    }

    #[test]
    fn locked_pets_eat_but_never_grow() {
        let mut c = Care { locked: true, ..hungry_care() };
        for _ in 0..50 {
            c.fullness = 0.0;
            assert_eq!(c.feed(Food::Favorite), FeedResult::Ate { leveled_up: false });
        }
        assert_eq!((c.stage, c.stamina), (Stage::Baby, 0.0));
    }

    #[test]
    fn reset_goes_back_to_a_full_baby_but_keeps_lock() {
        let mut c = Care { stage: Stage::Child, stamina: 40.0, fullness: 0.2, locked: true };
        c.reset();
        assert_eq!(c, Care { locked: true, ..Care::default() });
    }

    #[test]
    fn stage_table() {
        let s = [Stage::Baby, Stage::Kinder, Stage::Child, Stage::Teenager, Stage::Adult];
        assert_eq!(s.map(Stage::scale), [0.45, 0.60, 0.78, 1.00, 1.15]);
        assert_eq!(s.map(Stage::need), [Some(60.0), Some(100.0), Some(150.0), Some(220.0), None]);
        assert_eq!(s.map(Stage::name), ["baby", "kinder", "child", "teenager", "adult"]);
    }
}
