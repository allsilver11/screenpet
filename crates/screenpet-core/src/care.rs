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

    /// 다음 단계까지 필요한 체력 (adult는 없음). 합 265: 배고플 때마다(22.5분) 밥만 먹이면 27끼 ≈ 10시간, 특식만 먹이면 15끼 ≈ 5.6시간
    pub fn need(self) -> Option<f64> {
        match self {
            Stage::Baby => Some(30.0),
            Stage::Kinder => Some(50.0),
            Stage::Child => Some(75.0),
            Stage::Teenager => Some(110.0),
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

    pub const ALL: [Stage; 5] = [Stage::Baby, Stage::Kinder, Stage::Child, Stage::Teenager, Stage::Adult];

    /// `name`의 반대 (모르는 이름이면 None)
    pub fn from_name(name: &str) -> Option<Stage> {
        Stage::ALL.into_iter().find(|s| s.name() == name)
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

    /// 메뉴에 보이는 한국어 이름
    pub fn korean(self) -> &'static str {
        match self {
            Stage::Baby => "아기",
            Stage::Kinder => "유치원",
            Stage::Child => "어린이",
            Stage::Teenager => "청소년",
            Stage::Adult => "어른",
        }
    }

    /// 몇 번째 단계인지 (1..=STAGES)
    pub fn step(self) -> u8 {
        match self {
            Stage::Baby => 1,
            Stage::Kinder => 2,
            Stage::Child => 3,
            Stage::Teenager => 4,
            Stage::Adult => 5,
        }
    }
}

/// 성장 단계 수
pub const STAGES: u8 = 5;

/// 메뉴의 성장 단계 한 줄 ("🌱 성장: 어린이 (3/5단계)", 성장 멈춤이면 " 🔒").
/// 다음 단계까지 남은 체력은 일부러 숨긴다
pub fn growth_label(stage: Stage, locked: bool) -> String {
    let lock = if locked { " 🔒" } else { "" };
    format!("🌱 성장: {} ({}/{STAGES}단계){lock}", stage.korean(), stage.step())
}

/// 간식·물이 올려 주는 포만감
const TREAT_FILL: f64 = 0.1;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Food {
    /// 간식 — 언제든 받는다. 체력을 조금 주므로 틈틈이 주면 간식으로도 자란다
    Snack,
    Meal,
    /// 동물별 좋아하는 음식
    Favorite,
    /// 물 — 언제든 받는다 (성장 없음)
    Water,
    /// 🍱 특식 — 밥처럼 채우고 체력은 두 배. 개수 제한 없음
    Special,
}

/// 배고플 때 누르면 뜨는 음식 고르기 버튼 (ui/index.html·house.html #picker와 같은 순서)
pub const PICKER_FOODS: [Food; 3] = [Food::Meal, Food::Favorite, Food::Special];

impl Food {
    /// 끼니: 밥, 좋아하는 음식, 특식 — 배부르면 안 먹고, 고르기 버튼에 뜨고, 그릇으로 가서 먹는다
    pub fn is_meal(self) -> bool {
        matches!(self, Food::Meal | Food::Favorite | Food::Special)
    }

    /// 메뉴·음식 고르기 버튼의 이름 (직렬화 이름과 같다)
    pub fn from_name(name: &str) -> Option<Food> {
        match name {
            "snack" => Some(Food::Snack),
            "meal" => Some(Food::Meal),
            "favorite" => Some(Food::Favorite),
            "water" => Some(Food::Water),
            "special" => Some(Food::Special),
            _ => None,
        }
    }

    /// 간식·물: 배불러도 받고, 포만감은 조금만 오른다 (체력은 음식마다 — 간식은 조금, 물은 없음)
    pub fn is_treat(self) -> bool {
        !self.is_meal()
    }

    fn stamina(self) -> f64 {
        match self {
            Food::Water => 0.0,
            Food::Snack => 2.0,
            Food::Meal => 10.0,
            Food::Favorite => 15.0,
            Food::Special => 20.0,
        }
    }

    fn fill(self, fullness: f64) -> f64 {
        match self {
            Food::Snack | Food::Water => (fullness + TREAT_FILL).min(1.0),
            Food::Meal | Food::Special => 1.0,
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

    /// 점심시간: 배고프지 않았으면 이제 배고프다 (하루 한 번, presence.rs)
    pub fn lunch_hunger(&mut self) {
        self.fullness = self.fullness.min(HUNGRY_BELOW - 0.01);
    }

    pub fn feed(&mut self, food: Food) -> FeedResult {
        // 끼니만 배부르면 안 먹는다. 간식·물은 언제든
        if food.is_meal() && self.fullness > FULL_ABOVE {
            return FeedResult::Refused;
        }
        self.fullness = food.fill(self.fullness);
        // 물은 체력이 없으니 성장 판정도 없다 (예전 저장의 넘치는 체력으로 자라지 않게)
        if self.locked || food.stamina() == 0.0 {
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
        assert!((c.fullness - 0.2).abs() < 1e-9 && c.stamina == 2.0);
        let mut c = hungry_care();
        c.feed(Food::Meal);
        assert_eq!((c.fullness, c.stamina), (1.0, 10.0));
        let mut c = hungry_care();
        c.feed(Food::Favorite);
        assert!((c.fullness - 0.8).abs() < 1e-9 && c.stamina == 15.0);
    }

    #[test]
    fn water_adds_no_stamina_and_is_never_refused() {
        // 배불러도 받는다 — 포만감만 조금 오르고 체력·성장은 없다
        let mut c = Care::default();
        for _ in 0..100 {
            assert_eq!(c.feed(Food::Water), FeedResult::Ate { leveled_up: false });
        }
        assert_eq!((c.stage, c.stamina, c.fullness), (Stage::Baby, 0.0, 1.0));
        let mut c = hungry_care();
        c.feed(Food::Water);
        assert!((c.fullness - 0.2).abs() < 1e-9 && c.stamina == 0.0);
        // 레벨업 직전이어도 물로는 자라지 않는다
        let mut c = Care { stamina: 29.9, ..Care::default() };
        assert_eq!(c.feed(Food::Water), FeedResult::Ate { leveled_up: false });
        assert_eq!((c.stage, c.stamina), (Stage::Baby, 29.9));
    }

    #[test]
    fn a_snack_adds_two_stamina_and_only_a_little_fullness() {
        let mut c = hungry_care();
        assert_eq!(c.feed(Food::Snack), FeedResult::Ate { leveled_up: false });
        assert_eq!(c.stamina, 2.0);
        assert!((c.fullness - 0.2).abs() < 1e-9);
    }

    #[test]
    fn a_snack_grows_even_when_full() {
        let mut c = Care::default();
        assert_eq!(c.feed(Food::Snack), FeedResult::Ate { leveled_up: false });
        assert_eq!((c.stamina, c.fullness), (2.0, 1.0));
    }

    #[test]
    fn a_locked_pet_gets_no_stamina_from_a_snack() {
        let mut c = Care { locked: true, stamina: 29.0, ..Care::default() };
        assert_eq!(c.feed(Food::Snack), FeedResult::Ate { leveled_up: false });
        assert_eq!((c.stage, c.stamina), (Stage::Baby, 29.0));
    }

    #[test]
    fn a_snack_can_level_up() {
        // 아기는 배불러도 간식 15개면 자란다 (필요 30, 간식 하나에 2)
        let mut c = Care::default();
        for i in 1..=15 {
            assert_eq!(c.feed(Food::Snack), FeedResult::Ate { leveled_up: i == 15 }, "snack {i}");
        }
        assert_eq!((c.stage, c.stamina, c.fullness), (Stage::Kinder, 0.0, 1.0));
    }

    #[test]
    fn a_snack_is_much_slower_than_meals_all_the_way_to_adult() {
        let mut c = Care::default();
        let mut snacks = 0;
        while c.stage != Stage::Adult {
            c.feed(Food::Snack);
            snacks += 1;
        }
        assert_eq!(snacks, 15 + 25 + 38 + 55);
        // 어른은 더 자라지 않는다
        assert_eq!(c.feed(Food::Snack), FeedResult::Ate { leveled_up: false });
        assert_eq!(c.stage, Stage::Adult);
    }

    #[test]
    fn meals_favorites_and_special_meals_are_real_meals() {
        for food in [Food::Meal, Food::Favorite, Food::Special] {
            assert!(food.is_meal() && !food.is_treat());
            let mut c = hungry_care();
            c.feed(food);
            assert!(c.stamina > 2.0, "{food:?}");
        }
        for food in [Food::Snack, Food::Water] {
            assert!(food.is_treat() && !food.is_meal());
        }
        let mut c = hungry_care();
        c.feed(Food::Meal);
        assert_eq!(c.stamina, 10.0);
    }

    #[test]
    fn hungry_picker_offers_meal_favorite_and_special() {
        assert_eq!(PICKER_FOODS, [Food::Meal, Food::Favorite, Food::Special]);
        // 자라게 해도 간식은 끼니가 아니다 — 고르기 버튼에 없다
        assert!(PICKER_FOODS.iter().all(|f| f.is_meal()) && !PICKER_FOODS.contains(&Food::Snack));
        // 펫 창·하우스 창의 고르기 버튼이 같은 음식을 같은 순서로
        let names: Vec<String> = PICKER_FOODS.iter().map(|f| serde_json::to_value(f).unwrap().as_str().unwrap().to_string()).collect();
        for html in [include_str!("../../../ui/index.html"), include_str!("../../../ui/house.html")] {
            let picker = html.lines().find(|l| l.contains("id=\"picker\"")).expect("picker");
            let foods: Vec<&str> = picker.split("data-food=\"").skip(1).map(|r| r.split('"').next().unwrap()).collect();
            assert_eq!(foods, names);
        }
    }

    #[test]
    fn picker_buttons_fit_the_narrowest_windows() {
        // 버튼 28px, 간격 4px (ui/style.css #picker) — 아기 때도 펫 창은 128px, 하우스 창은 100px 이상이다
        let css = include_str!("../../../ui/style.css");
        assert!(css.contains("#picker {") && css.contains("gap: 4px;") && css.contains("width: 28px;"));
        let width = PICKER_FOODS.len() * 28 + (PICKER_FOODS.len() - 1) * 4;
        assert!(include_str!("../../../src-tauri/src/slots.rs").contains("(PET_LOGICAL_SIZE * body_scale.max(1.0),"));
        assert!(include_str!("../../../src-tauri/src/house.rs").contains("pub const MIN_WIDTH: f64 = 100.0;"));
        assert!(width <= 100, "{width}");
    }

    #[test]
    fn foods_serialize_snake_case() {
        let names: Vec<String> = [Food::Snack, Food::Meal, Food::Favorite, Food::Water, Food::Special].iter().map(|f| serde_json::to_string(f).unwrap()).collect();
        assert_eq!(names, ["\"snack\"", "\"meal\"", "\"favorite\"", "\"water\"", "\"special\""]);
    }

    #[test]
    fn food_names_match_the_serialized_names() {
        for food in [Food::Snack, Food::Meal, Food::Favorite, Food::Water, Food::Special] {
            let name = serde_json::to_value(food).unwrap();
            assert_eq!(Food::from_name(name.as_str().unwrap()), Some(food));
        }
        assert_eq!(Food::from_name("banana"), None);
    }

    #[test]
    fn the_tray_offers_the_special_meal_by_hand_and_in_the_bowl() {
        // feed:<hand|auto>:<food> — 트레이가 메뉴 이름을 Food::from_name으로 읽는다
        let tray = include_str!("../../../src-tauri/src/tray.rs");
        assert!(tray.contains(r#"format!("feed:{tag}:special"), "특식 🍱""#));
        assert!(tray.contains("&[&meal, &fav, &special, &snack]"));
        assert!(tray.contains("Food::from_name(food)"));
        assert!(include_str!("../../../src-tauri/src/main.rs").contains("Food::from_name(&food)"));
    }

    #[test]
    fn a_special_meal_is_twice_a_meal_and_fills_like_one() {
        assert!(Food::Special.is_meal() && !Food::Special.is_treat());
        let mut c = hungry_care();
        assert_eq!(c.feed(Food::Special), FeedResult::Ate { leveled_up: false });
        assert_eq!((c.fullness, c.stamina), (1.0, 20.0));
    }

    #[test]
    fn a_special_meal_is_refused_above_nine_tenths_full() {
        let mut c = Care { fullness: 0.91, ..Care::default() };
        assert_eq!(c.feed(Food::Special), FeedResult::Refused);
        assert_eq!((c.fullness, c.stamina), (0.91, 0.0));
        c.fullness = 0.9;
        assert_eq!(c.feed(Food::Special), FeedResult::Ate { leveled_up: false });
    }

    #[test]
    fn a_locked_pet_gets_no_stamina_from_a_special_meal() {
        let mut c = Care { locked: true, ..hungry_care() };
        assert_eq!(c.feed(Food::Special), FeedResult::Ate { leveled_up: false });
        assert_eq!((c.stage, c.stamina, c.fullness), (Stage::Baby, 0.0, 1.0));
    }

    #[test]
    fn a_baby_needs_two_special_meals() {
        let mut c = Care::default();
        for i in 1..=2 {
            c.fullness = 0.0;
            assert_eq!(c.feed(Food::Special), FeedResult::Ate { leveled_up: i == 2 }, "meal {i}");
        }
        assert_eq!((c.stage, c.stamina), (Stage::Kinder, 0.0));
    }

    /// 배고플 때마다 이 음식만 먹여 어른이 될 때까지의 끼니 수
    fn meals_to_adult(food: Food) -> u32 {
        let mut c = Care::default();
        let mut meals = 0;
        while c.stage != Stage::Adult {
            c.fullness = 0.0;
            c.feed(food);
            meals += 1;
        }
        meals
    }

    #[test]
    fn growing_up_takes_ten_hours_on_meals_and_about_half_on_special_meals() {
        let total: f64 = [Stage::Baby, Stage::Kinder, Stage::Child, Stage::Teenager].iter().filter_map(|s| s.need()).sum();
        assert_eq!(total, 265.0);
        // 가득 찬 뒤 배고파지기까지 22.5분
        let minutes_a_meal = (1.0 - HUNGRY_BELOW) * EMPTY_SECS / 60.0;
        assert_eq!(minutes_a_meal, 22.5);
        assert_eq!(meals_to_adult(Food::Meal), 27);
        assert_eq!(meals_to_adult(Food::Special), 15);
        assert!((f64::from(meals_to_adult(Food::Meal)) * minutes_a_meal / 60.0 - 10.0).abs() < 0.2);
        assert!(f64::from(meals_to_adult(Food::Special)) * minutes_a_meal / 60.0 < 6.0);
    }

    #[test]
    fn an_old_save_above_the_new_need_grows_one_stage_on_the_next_growth_meal() {
        // 예전 필요량(60/100/150/220)으로 쌓아 둔 체력
        let old = Care { stage: Stage::Baby, stamina: 59.0, fullness: 0.0, locked: false };
        // 먹이기 전엔, 또 물로는 자라지 않는다
        let mut c = old.clone();
        c.tick(10.0);
        assert_eq!(c.feed(Food::Water), FeedResult::Ate { leveled_up: false });
        assert_eq!((c.stage, c.stamina), (Stage::Baby, 59.0));
        // 간식도 이제 체력을 주므로 다음 간식에 한 단계
        let mut c = old.clone();
        assert_eq!(c.feed(Food::Snack), FeedResult::Ate { leveled_up: true });
        assert_eq!((c.stage, c.stamina), (Stage::Kinder, 0.0));
        // 다음 밥에 한 단계만, 남은 체력은 넘어가지 않는다
        let mut c = old.clone();
        assert_eq!(c.feed(Food::Meal), FeedResult::Ate { leveled_up: true });
        assert_eq!((c.stage, c.stamina), (Stage::Kinder, 0.0));
        c.fullness = 0.0;
        assert_eq!(c.feed(Food::Meal), FeedResult::Ate { leveled_up: false });
        assert_eq!((c.stage, c.stamina), (Stage::Kinder, 10.0));
        // 저장 파일을 읽어도 그대로, 어른 직전도 한 단계
        let json = serde_json::to_string(&Care { stage: Stage::Teenager, stamina: 219.0, fullness: 0.0, locked: false }).unwrap();
        let mut c: Care = serde_json::from_str(&json).unwrap();
        assert_eq!(c.stamina, 219.0);
        assert_eq!(c.feed(Food::Special), FeedResult::Ate { leveled_up: true });
        assert_eq!((c.stage, c.stamina), (Stage::Adult, 0.0));
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
    fn baby_needs_three_meals() {
        let mut c = Care::default();
        for i in 1..=3 {
            c.fullness = 0.0;
            let r = c.feed(Food::Meal);
            assert_eq!(r, FeedResult::Ate { leveled_up: i == 3 }, "meal {i}");
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
        assert_eq!(s.map(Stage::need), [Some(30.0), Some(50.0), Some(75.0), Some(110.0), None]);
        assert_eq!(s.map(Stage::name), ["baby", "kinder", "child", "teenager", "adult"]);
    }

    #[test]
    fn growth_label_shows_korean_stage_and_step_but_no_progress() {
        let s = [Stage::Baby, Stage::Kinder, Stage::Child, Stage::Teenager, Stage::Adult];
        assert_eq!(s.map(Stage::korean), ["아기", "유치원", "어린이", "청소년", "어른"]);
        assert_eq!(s.map(Stage::step), [1, 2, 3, 4, 5]);
        assert_eq!(growth_label(Stage::Child, false), "🌱 성장: 어린이 (3/5단계)");
        assert_eq!(growth_label(Stage::Baby, true), "🌱 성장: 아기 (1/5단계) 🔒");
        assert_eq!(growth_label(Stage::Adult, false), "🌱 성장: 어른 (5/5단계)");
    }
}
