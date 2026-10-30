//! 밥그릇·물그릇·꽃·쿠키 (phase 3 먹이 바꾸기) — 먹을 때 동물 앞 바닥에 잠깐 놓이는 작은 소품.
//! 앱은 Pet::take_prop이 돌려주는 PropEvent대로 작은 투명 창(ui/prop.html)을 띄우고 거둔다.

use serde::{Deserialize, Serialize};

use crate::care::Food;

/// 그릇이 놓이는 거리: 몸 중심에서 바라보는 쪽으로 (크기 128 기준 px)
pub const BOWL_GAP: f64 = 75.0;
/// 그릇까지 이 시간(초) 안에 못 가면 그 자리에서 먹는다
pub const FOOD_TIMEOUT_SECS: f64 = 8.0;
/// 쿠키는 입 높이에: 발에서 몸 크기의 이 비율만큼 위
pub const COOKIE_RISE: f64 = 0.3;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Prop {
    /// 밥그릇 (밥·좋아하는 음식)
    FoodBowl,
    /// 물그릇
    WaterBowl,
    /// 간식 — 입가에 잠깐
    Cookie,
    /// 꿀벌의 밥 — 바닥에 피어난 작은 꽃
    Flower,
}

impl Prop {
    /// 이 음식을 먹을 때 놓이는 소품 (꿀벌은 밥 대신 꽃 — `flower`는 꿀벌만: PetConfig::stings.
    /// 유령처럼 날기만 하는 동물은 보통 밥그릇)
    pub fn for_food(food: Food, flower: bool) -> Prop {
        match food {
            Food::Snack => Prop::Cookie,
            Food::Water => Prop::WaterBowl,
            Food::Meal | Food::Favorite if flower => Prop::Flower,
            Food::Meal | Food::Favorite => Prop::FoodBowl,
        }
    }
}

/// 앱에 보내는 소품 변화. (x, y) = 소품 아래 가운데 (물리 px, 디딤판 높이)
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum PropEvent {
    Show { prop: Prop, x: f64, y: f64 },
    /// 다 먹었다 — 흐려지며 사라진다
    Hide,
}

/// 먹이 주는 방법 (트레이·우클릭 "밥 주기"에서 고른다, 설정에 저장)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FeedMode {
    /// 직접 주기 — 커서 끝의 음식을 동물 입에 갖다 댄다
    #[default]
    Hand,
    /// 자동 주기 — 그릇(꿀벌은 꽃)이 놓이고 동물이 가서 먹는다
    Auto,
}

impl FeedMode {
    /// 이 음식을 손으로 주는지 (물은 늘 물그릇)
    pub fn by_hand(self, food: Food) -> bool {
        self == FeedMode::Hand && food != Food::Water
    }
}

/// 손에 든 음식이 이만큼(몸 크기 비율) 입 가까이 오면 받아먹는다
pub const HAND_REACH: f64 = 0.35;
/// 입 높이 (발에서 몸 크기의 이 비율 위) — 걷는 동물 (나는 동물은 sting::FACE_HEIGHT)
pub const MOUTH_HEIGHT: f64 = 0.45;
/// 손에 든 음식을 이 시간(초)이 지나면 저절로 거둔다
pub const HAND_TIMEOUT_SECS: f64 = 30.0;
/// 음식 그림 가운데가 커서 끝에서 오른쪽 아래로 이만큼 (논리 px) — 손에 쥔 것처럼
pub const HAND_OFFSET: f64 = 14.0;

/// 손에 든 음식 그림 (ui/effects/<name>.png, tools/draw_props.py). 좋아하는 음식은 동물마다
pub fn hand_icon(food: Food, pet_id: &str) -> &'static str {
    match food {
        Food::Snack => "hand_cookie",
        Food::Favorite => match pet_id {
            "cat" => "hand_fish",
            "dog" => "hand_bone",
            "rabbit" => "hand_carrot",
            "bear" => "hand_honey",
            "bee" => "hand_nectar",
            "frog" => "hand_fly",
            "pumpkin" => "hand_candy",
            "ghost" => "hand_lollipop",
            "rudolph" => "hand_apple",
            "santa" => "hand_milk",
            "duck" => "hand_bread",
            "giraffe" => "hand_leaf",
            "dino_yellow" => "hand_watermelon",
            "dino_green" => "hand_broccoli",
            "dino_purple" => "hand_grapes",
            _ => "hand_meal",
        },
        Food::Meal | Food::Water => "hand_meal",
    }
}

/// 음식(food)이 입(mouth)에 닿았는지 — 몸 크기(size)의 HAND_REACH 반경
pub fn in_reach(mouth: (f64, f64), food: (f64, f64), size: f64) -> bool {
    (food.0 - mouth.0).hypot(food.1 - mouth.1) <= HAND_REACH * size
}

/// 손에 든 음식 — 버튼 누름(가장자리)과 시간 재기
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HandInput {
    None,
    /// 왼쪽 버튼을 새로 눌렀다 (닿아 있으면 주기, 빈 곳이면 거두기)
    Click,
    /// 오른쪽 버튼·Esc·30초 — 거둔다 (먹지 않음)
    Cancel,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HandFeed {
    pub food: Food,
    elapsed: f64,
    /// 지난 틱의 왼쪽·오른쪽 버튼·Esc (처음엔 눌린 것으로 쳐서, 메뉴를 고른 그 누름은 세지 않는다)
    was: (bool, bool, bool),
}

impl HandFeed {
    pub fn new(food: Food) -> Self {
        Self { food, elapsed: 0.0, was: (true, true, true) }
    }

    pub fn tick(&mut self, dt: f64, left: bool, right: bool, esc: bool) -> HandInput {
        self.elapsed += dt;
        let pressed = (left && !self.was.0, right && !self.was.1, esc && !self.was.2);
        self.was = (left, right, esc);
        if pressed.1 || pressed.2 || self.elapsed >= HAND_TIMEOUT_SECS - 1e-9 {
            HandInput::Cancel
        } else if pressed.0 {
            HandInput::Click
        } else {
            HandInput::None
        }
    }
}

/// 그릇 x: 몸 중심(x)에서 바라보는 쪽(facing)으로 gap만큼, 동물이 설 수 있는 범위 [lo, hi] 안으로.
/// 범위가 비었으면(좁은 디딤판) 지금 자리
pub fn bowl_x(x: f64, facing: f64, gap: f64, lo: f64, hi: f64) -> f64 {
    if lo > hi {
        return x;
    }
    (x + facing.signum() * gap).clamp(lo, hi)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bowl_goes_in_front_and_stays_on_the_surface() {
        assert_eq!(bowl_x(500.0, 1.0, 75.0, 100.0, 900.0), 575.0);
        assert_eq!(bowl_x(500.0, -1.0, 75.0, 100.0, 900.0), 425.0);
        assert_eq!(bowl_x(880.0, 1.0, 75.0, 100.0, 900.0), 900.0);
        assert_eq!(bowl_x(120.0, -1.0, 75.0, 100.0, 900.0), 100.0);
        assert_eq!(bowl_x(500.0, 1.0, 75.0, 600.0, 580.0), 500.0);
    }

    #[test]
    fn props_per_food() {
        assert_eq!(Prop::for_food(Food::Meal, false), Prop::FoodBowl);
        assert_eq!(Prop::for_food(Food::Favorite, false), Prop::FoodBowl);
        assert_eq!(Prop::for_food(Food::Meal, true), Prop::Flower);
        assert_eq!(Prop::for_food(Food::Favorite, true), Prop::Flower);
        assert_eq!(Prop::for_food(Food::Water, false), Prop::WaterBowl);
        assert_eq!(Prop::for_food(Food::Water, true), Prop::WaterBowl);
        assert_eq!(Prop::for_food(Food::Snack, true), Prop::Cookie);
    }

    #[test]
    fn feed_mode_defaults_to_hand_and_serializes_snake_case() {
        assert_eq!(FeedMode::default(), FeedMode::Hand);
        assert_eq!(serde_json::to_string(&FeedMode::Auto).unwrap(), "\"auto\"");
        assert_eq!(serde_json::from_str::<FeedMode>("\"hand\"").unwrap(), FeedMode::Hand);
        // 물은 늘 그릇, 나머지는 모드대로
        assert!(FeedMode::Hand.by_hand(Food::Meal) && FeedMode::Hand.by_hand(Food::Favorite) && FeedMode::Hand.by_hand(Food::Snack));
        assert!(!FeedMode::Hand.by_hand(Food::Water));
        assert!(!FeedMode::Auto.by_hand(Food::Meal));
    }

    #[test]
    fn hand_icons_per_food_and_species() {
        assert_eq!(hand_icon(Food::Meal, "cat"), "hand_meal");
        assert_eq!(hand_icon(Food::Snack, "dog"), "hand_cookie");
        let favs: Vec<_> = ["cat", "dog", "rabbit", "bear", "bee", "frog", "pumpkin", "ghost", "unknown"].iter().map(|id| hand_icon(Food::Favorite, id)).collect();
        assert_eq!(favs, ["hand_fish", "hand_bone", "hand_carrot", "hand_honey", "hand_nectar", "hand_fly", "hand_candy", "hand_lollipop", "hand_meal"]);
        // 할로윈 동물도 밥·간식은 다른 동물과 같다
        assert_eq!(hand_icon(Food::Meal, "ghost"), "hand_meal");
        assert_eq!(hand_icon(Food::Snack, "pumpkin"), "hand_cookie");
        // 크리스마스: 루돌프는 사과, 산타는 우유 (밥·간식은 같다)
        assert_eq!((hand_icon(Food::Favorite, "rudolph"), hand_icon(Food::Favorite, "santa")), ("hand_apple", "hand_milk"));
        assert_eq!(hand_icon(Food::Snack, "santa"), "hand_cookie");
        assert_eq!((hand_icon(Food::Favorite, "duck"), hand_icon(Food::Favorite, "giraffe")), ("hand_bread", "hand_leaf"));
        assert_eq!(hand_icon(Food::Meal, "giraffe"), "hand_meal");
        assert_eq!(
            ["dino_yellow", "dino_green", "dino_purple"].map(|id| hand_icon(Food::Favorite, id)),
            ["hand_watermelon", "hand_broccoli", "hand_grapes"]
        );
        assert_eq!(hand_icon(Food::Snack, "dino_green"), "hand_cookie");
    }

    #[test]
    fn food_is_in_reach_near_the_mouth() {
        // 크기 128 → 반경 44.8
        assert!(in_reach((500.0, 300.0), (530.0, 320.0), 128.0));
        assert!(!in_reach((500.0, 300.0), (545.0, 300.0), 128.0));
        assert!(in_reach((500.0, 300.0), (520.0, 300.0), 64.0));
        assert!(!in_reach((500.0, 300.0), (525.0, 300.0), 64.0));
    }

    #[test]
    fn hand_feed_cancels_clicks_and_times_out() {
        // 메뉴를 누른 버튼이 아직 눌려 있어도 클릭으로 치지 않는다
        let mut h = HandFeed::new(Food::Meal);
        assert_eq!(h.tick(0.016, true, false, false), HandInput::None);
        assert_eq!(h.tick(0.016, false, false, false), HandInput::None);
        assert_eq!(h.tick(0.016, true, false, false), HandInput::Click);
        assert_eq!(h.tick(0.016, true, false, false), HandInput::None, "held is not another click");
        assert_eq!(h.food, Food::Meal);
        let mut h = HandFeed::new(Food::Snack);
        h.tick(0.016, false, false, false);
        assert_eq!(h.tick(0.016, false, true, false), HandInput::Cancel);
        let mut h = HandFeed::new(Food::Snack);
        h.tick(0.016, false, false, false);
        assert_eq!(h.tick(0.016, false, false, true), HandInput::Cancel);
        // 30초가 지나면 저절로 거둔다
        let mut h = HandFeed::new(Food::Favorite);
        let mut out = HandInput::None;
        let mut t = 0.0;
        while out == HandInput::None && t < 40.0 {
            out = h.tick(0.1, false, false, false);
            t += 0.1;
        }
        assert_eq!(out, HandInput::Cancel);
        assert!((t - HAND_TIMEOUT_SECS).abs() < 0.2, "{t}");
    }

    #[test]
    fn events_serialize_for_the_webview() {
        let show = PropEvent::Show { prop: Prop::WaterBowl, x: 1.0, y: 2.0 };
        assert_eq!(serde_json::to_string(&show).unwrap(), r#"{"kind":"show","prop":"water_bowl","x":1.0,"y":2.0}"#);
        assert_eq!(serde_json::to_string(&PropEvent::Hide).unwrap(), r#"{"kind":"hide"}"#);
    }
}
