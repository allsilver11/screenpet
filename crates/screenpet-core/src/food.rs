//! 밥그릇·물그릇·꽃·쿠키 (phase 3 먹이 바꾸기) — 먹을 때 동물 앞 바닥에 잠깐 놓이는 작은 소품.
//! 앱은 Pet::take_prop이 돌려주는 PropEvent대로 작은 투명 창(ui/prop.html)을 띄우고 거둔다.

use serde::Serialize;

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
    /// 이 음식을 먹을 때 놓이는 소품 (나는 동물은 밥 대신 꽃)
    pub fn for_food(food: Food, flies: bool) -> Prop {
        match food {
            Food::Snack => Prop::Cookie,
            Food::Water => Prop::WaterBowl,
            Food::Meal | Food::Favorite if flies => Prop::Flower,
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
    fn events_serialize_for_the_webview() {
        let show = PropEvent::Show { prop: Prop::WaterBowl, x: 1.0, y: 2.0 };
        assert_eq!(serde_json::to_string(&show).unwrap(), r#"{"kind":"show","prop":"water_bowl","x":1.0,"y":2.0}"#);
        assert_eq!(serde_json::to_string(&PropEvent::Hide).unwrap(), r#"{"kind":"hide"}"#);
    }
}
