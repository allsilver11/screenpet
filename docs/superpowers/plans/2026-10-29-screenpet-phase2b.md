# ScreenPet 2B (밥 주기·성장) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 배고프면 밥 3종 중 골라 먹이고, 숨겨진 체력으로 baby→adult 5단계 성장(크기·비율 변화), 레벨 고정·리셋.

**Architecture:** 순수 로직 `care.rs`(포만감·체력·단계·음식)와 동물별 저장(`config.rs`)을 core에 두고, `brain.rs`는 배고픔·먹기·거절·성장 연출 상태와 음식 요청 플래그만 갖는다. 게임 루프가 `Care`를 소유해 시간을 흘리고 저장하며, 단계 배율로 펫 크기·창 크기를 바꾼다. 그림은 `draw_pets.py`가 단계별 비율로 5장 시트를 만든다.

**Tech Stack:** Rust (core, Tauri 2), Python/Pillow/numpy, 바닐라 JS.

**Spec:** `docs/superpowers/specs/2026-10-29-screenpet-phase2b-design.md` (2A spec 위에 쌓음)

## Global Constraints

- 포만감 30분에 1.0 감소(앱 켜진 동안만), 배고픔 < 0.25, 배부름(거절) > 0.9.
- 음식: snack 체력+3 포만+0.4 / meal 체력+10 포만=1.0 / favorite 체력+15 포만+0.7 (먹은 뒤 heart 애교). 좋아하는 것: 고양이 🐟 생선, 강아지 🦴 뼈다귀, 토끼 🥕 당근, 비버 🪵 나뭇가지.
- 단계 배율·필요 체력: baby 0.45/60, kinder 0.60/100, child 0.78/150, teenager 1.00/220, adult 1.15/—. 단계가 오르면 체력 0.
- 레벨 고정: 체력·성장만 멈춤. 리셋: baby·체력 0·포만 1.0. 동물별 따로 저장. 설정에 기록 없으면 baby.
- 배고플 때: 쉬기 진입 50% `hungry` 3~5초, 배고픔 말풍선 15~25초마다(평소 대사 대신), 짧은 클릭 → 음식 고르기.
- 먹기 `eat` 2.5초 → favorite면 `heart` 2초. 거절: "full~" + `tilt` 2초. 성장: `level_up` 2초 + "yay!".
- 필수 동작 21개 = 2A 18개 + `hungry eat level_up` (모두 앞모습), 선택 `arch`(고양이)·`wag`(강아지).
- 시트: `sheet_<stage>.png` 5장 + `sheet.png`(= teenager). 단계 이름·다음 모습은 UI 어디에도 표시하지 않는다.
- 펫 창: 너비 `128 × max(1, 배율)`, 높이 `36 + 128 × 배율` 논리 px.
- 리눅스에서 src-tauri 컴파일 불가 (CI 검증). cargo 앞 `. "$HOME/.cargo/env" &&`. 커밋 끝 `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`. 브랜치 `feat/phase2b`.

---

### Task 1: 돌보기 로직 + 저장 (care.rs, config.rs)

**Files:**
- Create: `crates/screenpet-core/src/care.rs`
- Modify: `crates/screenpet-core/src/lib.rs` (`pub mod care;`), `crates/screenpet-core/src/config.rs`

**Interfaces — Produces:**
```rust
// care.rs
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)] #[serde(rename_all = "snake_case")]
pub enum Stage { Baby, Kinder, Child, Teenager, Adult }
impl Stage { pub fn scale(self) -> f64; pub fn need(self) -> Option<f64>; pub fn next(self) -> Option<Stage>; pub fn name(self) -> &'static str }
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)] #[serde(rename_all = "snake_case")]
pub enum Food { Snack, Meal, Favorite }
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FeedResult { Refused, Ate { leveled_up: bool } }
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Care { pub stage: Stage, pub stamina: f64, pub fullness: f64, pub locked: bool }   // Default = Baby, 0, 1.0, false
impl Care { pub fn tick(&mut self, dt: f64); pub fn is_hungry(&self) -> bool; pub fn feed(&mut self, food: Food) -> FeedResult; pub fn reset(&mut self) }
// config.rs
pub struct Config { pub pet: Option<String>, #[serde(default)] pub care: HashMap<String, Care> }
```

- [ ] **Step 1: 실패 테스트** — `care.rs` 하단

```rust
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
```
(`reset`은 고정 여부를 유지한다 — "처음부터 다시 키우기"는 성장만 되돌리고 사용자가 켠 "성장 멈추기"는 그대로.)

`config.rs` 테스트 추가:
```rust
    #[test]
    fn old_config_without_care_still_loads() {
        let d = temp_dir("oldcfg");
        std::fs::create_dir_all(&d).unwrap();
        std::fs::write(d.join("config.json"), r#"{"pet": "cat"}"#).unwrap();
        let c = load(&d);
        assert_eq!(c.pet.as_deref(), Some("cat"));
        assert!(c.care.is_empty());
    }

    #[test]
    fn care_per_pet_roundtrips() {
        use crate::care::{Care, Stage};
        let d = temp_dir("care");
        let mut c = Config { pet: Some("dog".into()), ..Config::default() };
        c.care.insert("dog".into(), Care { stage: Stage::Child, stamina: 12.0, fullness: 0.3, locked: true });
        save(&d, &c).unwrap();
        assert_eq!(load(&d), c);
    }
```
기존 `save_then_load_roundtrip`의 `Config { pet: Some("rabbit".into()) }` → `Config { pet: Some("rabbit".into()), ..Config::default() }`.

- [ ] **Step 2: 실패 확인** — `cargo test -p screenpet-core care config` → FAIL

- [ ] **Step 3: 구현** — `care.rs` 상단

```rust
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
```
(adult는 `need()`가 None이므로 체력이 계속 쌓여도 무해하다.)

`config.rs`: `use std::collections::HashMap;`, `use crate::care::Care;`,
```rust
#[derive(Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Config {
    /// 마지막으로 고른 동물 폴더 이름
    pub pet: Option<String>,
    /// 동물별 돌보기 상태 (없으면 새끼부터)
    #[serde(default)]
    pub care: HashMap<String, Care>,
}
```

- [ ] **Step 4: 통과·커밋** — `cargo test -p screenpet-core` 통과·경고 없음 → `feat(core): care — fullness, hidden stamina, five growth stages, foods`

---

### Task 2: 배고픔·먹기·거절·성장 연출 (brain.rs)

**Files:** Modify `crates/screenpet-core/src/brain.rs`

**Interfaces — Produces:**
```rust
pub enum Anim { ..., Arch, Wag, Hungry, Eat, LevelUp }        // 끝에 추가, 셋 다 앞모습
pub enum Speech { Chatter, Tickle, Grumpy, Hiss, Hungry, Full, LevelUp }   // serde: hungry, full, level_up
pub enum State { ..., Hungry { remaining: f64 }, Eating { remaining: f64, favorite: bool, level_up: bool }, LevelUp { remaining: f64 } }
impl Pet {
    pub fn set_hungry(&mut self, hungry: bool);
    /// 짧은 클릭으로 음식 고르기를 원하면 true를 한 번 돌려준다
    pub fn take_food_request(&mut self) -> bool;
    /// 먹이 결과 반영. 공중·드래그 중이면 무시하고 false
    pub fn feed_result(&mut self, result: FeedResult, food: Food) -> bool;
}
```
2A의 manifest `REQUIRED_ANIMS`는 Task 3에서 21개로 늘린다 — 이 태스크에선 `Anim` 이름 교차 검사 테스트(`every_anim_name_is_a_required_manifest_animation`)가 깨지므로, 그 테스트의 기대 목록에 새 3개가 REQUIRED에 들어갈 것을 반영하는 대신 **이 태스크에서 manifest.rs `REQUIRED_ANIMS`도 21개로 함께 늘리고**, 번들 `ui/pets/*/pet.json` 검증 테스트(`bundled_placeholder_manifests_are_valid`)는 Task 3에서 그림을 다시 만들 때까지 `#[ignore = "re-enabled in phase 2B Task 3 (new hungry/eat/level_up rows)"]`로 둔다. (Task 3 마지막 단계에서 ignore 제거.)

- [ ] **Step 1: 실패 테스트** — brain.rs 끝 새 모듈

```rust
#[cfg(test)]
mod care_tests {
    use super::tests::{cfg, place, w1, world};
    use super::*;
    use crate::care::{FeedResult, Food};

    const DT: f64 = 1.0 / 60.0;

    fn grounded(seed: u64) -> (Pet, World) {
        let w = world(vec![w1()]);
        let mut p = Pet::new(cfg(), 0.0, 0.0, seed);
        place(&mut p, &w, Owner::Window(1), 800.0, State::Idle { remaining: 100.0, look: false });
        (p, w)
    }

    #[test]
    fn hungry_pets_often_sulk_when_resting() {
        let w = world(vec![w1()]);
        let n = 2000;
        let sulks = (0..n)
            .filter(|&seed| {
                let mut p = Pet::new(cfg(), 0.0, 0.0, seed);
                p.set_hungry(true);
                place(&mut p, &w, Owner::Window(1), 800.0, State::Land { remaining: 0.001 });
                p.update(DT, &w);
                matches!(p.state, State::Hungry { .. })
            })
            .count() as f64
            / n as f64;
        assert!((0.45..=0.55).contains(&sulks), "{sulks}");
        assert_eq!(Pet::new(cfg(), 0.0, 0.0, 1).render().flip, false);
    }

    #[test]
    fn hungry_speech_replaces_chatter() {
        let (mut p, w) = grounded(1);
        p.set_hungry(true);
        let mut said = Vec::new();
        for _ in 0..(60 * 60) {
            p.update(DT, &w);
            if let Some(s) = p.take_speech() {
                said.push(s);
            }
        }
        assert!(said.len() >= 2);
        assert!(said.iter().all(|s| *s == Speech::Hungry), "{said:?}");
    }

    #[test]
    fn clicking_a_hungry_pet_asks_for_food_instead_of_a_trick() {
        let (mut p, _w) = grounded(1);
        p.set_hungry(true);
        p.pointer_down(800.0, 350.0);
        p.pointer_up(800.0, 350.0);
        assert!(p.take_food_request());
        assert!(!p.take_food_request(), "only once");
        assert!(matches!(p.state, State::Hungry { .. }));
        let (mut p, _w) = grounded(1);
        p.pointer_down(800.0, 350.0);
        p.pointer_up(800.0, 350.0);
        assert!(!p.take_food_request(), "not hungry → normal click reaction");
    }

    #[test]
    fn eating_then_moving_or_hearting() {
        for (food, then_heart) in [(Food::Meal, false), (Food::Favorite, true)] {
            let (mut p, w) = grounded(1);
            assert!(p.feed_result(FeedResult::Ate { leveled_up: false }, food));
            assert_eq!(p.render().anim, Anim::Eat);
            for _ in 0..(60 * 2 + 40) {
                p.update(DT, &w);
            }
            if then_heart {
                assert_eq!(p.state, State::Trick { kind: Trick::Heart, remaining: p_remaining(&p) });
            } else {
                assert!(matches!(p.state, State::Move { .. }), "{:?}", p.state);
            }
        }
    }

    fn p_remaining(p: &Pet) -> f64 {
        match p.state {
            State::Trick { remaining, .. } => remaining,
            _ => -1.0,
        }
    }

    #[test]
    fn eating_then_level_up_celebrates() {
        let (mut p, w) = grounded(1);
        p.feed_result(FeedResult::Ate { leveled_up: true }, Food::Meal);
        for _ in 0..(60 * 2 + 40) {
            p.update(DT, &w);
        }
        assert!(matches!(p.state, State::LevelUp { .. }), "{:?}", p.state);
        assert_eq!(p.render().anim, Anim::LevelUp);
        assert_eq!(p.take_speech(), Some(Speech::LevelUp));
        for _ in 0..(60 * 3) {
            p.update(DT, &w);
        }
        assert!(!matches!(p.state, State::LevelUp { .. }));
    }

    #[test]
    fn refusing_says_full_and_tilts() {
        let (mut p, _w) = grounded(1);
        assert!(p.feed_result(FeedResult::Refused, Food::Meal));
        assert_eq!(p.take_speech(), Some(Speech::Full));
        assert!(matches!(p.state, State::Trick { kind: Trick::Tilt, .. }));
    }

    #[test]
    fn airborne_pets_ignore_food() {
        let w = world(vec![w1()]);
        let mut p = Pet::new(cfg(), 800.0, 100.0, 1);
        p.update(DT, &w);
        assert!(!p.feed_result(FeedResult::Ate { leveled_up: true }, Food::Meal));
        assert!(matches!(p.state, State::Air { .. }));
    }

    #[test]
    fn new_front_animations() {
        for a in [Anim::Hungry, Anim::Eat, Anim::LevelUp] {
            assert!(a.is_front(), "{a:?}");
        }
        assert_eq!(serde_json::to_string(&Anim::LevelUp).unwrap(), "\"level_up\"");
        assert_eq!(serde_json::to_string(&Speech::LevelUp).unwrap(), "\"level_up\"");
    }
}
```
기존 테스트 갱신: `front_animations_are_the_tricks_and_tickle` → `all` 배열 끝에 `Anim::Hungry, Anim::Eat, Anim::LevelUp` 추가, 기대 front 목록 끝에도 같은 셋 추가. `every_anim_name_is_a_required_manifest_animation`의 `all` 배열: 필수 21개 순서(`... heart, tickle, annoyed, hungry, eat, level_up`)에 맞게 `Anim::Hungry, Anim::Eat, Anim::LevelUp`을 `Anim::Annoyed` 다음, `Anim::Arch` 앞에 둔다.

- [ ] **Step 2: 실패 확인** — `cargo test -p screenpet-core care_tests` → FAIL

- [ ] **Step 3: 구현**

manifest.rs: `REQUIRED_ANIMS`를 `[&str; 21]`로, 끝에 `"hungry", "eat", "level_up"` 추가. `bundled_placeholder_manifests_are_valid`에 `#[ignore = "re-enabled in phase 2B Task 3 (new hungry/eat/level_up rows)"]`.

brain.rs:
1. import: `use crate::care::{FeedResult, Food};`
2. 상수:
```rust
const HUNGRY_REST_CHANCE: f64 = 0.5;
const HUNGRY_SECS: (f64, f64) = (3.0, 5.0);
const HUNGRY_TALK_SECS: (f64, f64) = (15.0, 25.0);
const EAT_SECS: f64 = 2.5;
const AFTER_FAVORITE_HEART_SECS: f64 = 2.0;
const LEVEL_UP_SECS: f64 = 2.0;
const REFUSE_TILT_SECS: f64 = 2.0;
```
3. `Anim` 끝: `Hungry, Eat, LevelUp` (Wag 다음). `is_front`에 `| Anim::Hungry | Anim::Eat | Anim::LevelUp`. 
   **주의**: serde 이름 순서가 교차 검사와 일치해야 하므로 `every_anim_name...` 테스트의 배열 순서만 바꾸면 된다(enum 선언 순서와 무관).
4. `Speech` 끝: `Hungry, Full, LevelUp`.
5. `State` 끝(Special 다음):
```rust
    /// 배고파서 시무룩
    Hungry { remaining: f64 },
    /// 냠냠 — 끝나면 favorite면 하트, level_up이면 성장 연출
    Eating { remaining: f64, favorite: bool, level_up: bool },
    /// 성장 축하
    LevelUp { remaining: f64 },
```
6. `Pet` 필드: `hungry: bool`, `food_request: bool` (new에서 false).
7. 공개 메서드:
```rust
    pub fn set_hungry(&mut self, hungry: bool) {
        self.hungry = hungry;
        if !hungry && matches!(self.state, State::Hungry { .. }) {
            self.state = self.idle_state();
        }
    }

    pub fn take_food_request(&mut self) -> bool {
        std::mem::take(&mut self.food_request)
    }

    pub fn feed_result(&mut self, result: FeedResult, food: Food) -> bool {
        let grounded = self.standing.is_some() && !matches!(self.state, State::Air { .. } | State::Pressed { .. } | State::Dragged);
        if !grounded {
            return false;
        }
        self.state = match result {
            FeedResult::Refused => {
                self.speech = Some(Speech::Full);
                State::Trick { kind: Trick::Tilt, remaining: REFUSE_TILT_SECS }
            }
            FeedResult::Ate { leveled_up } => State::Eating { remaining: EAT_SECS, favorite: food == Food::Favorite, level_up: leveled_up },
        };
        true
    }
```
8. `pointer_up`의 `State::Pressed { grounded: true, .. }` 팔 맨 앞:
```rust
                if self.hungry {
                    self.food_request = true;
                    self.state = State::Hungry { remaining: behavior::range(&mut self.rng, HUNGRY_SECS) };
                    return;
                }
```
9. `update_chatter`: 배고프면 대사 종류와 간격이 바뀐다 — `self.speech = Some(Speech::Chatter); self.chatter_timer = behavior::range(&mut self.rng, CHATTER_SECS);` 두 줄을
```rust
            self.speech = Some(if self.hungry { Speech::Hungry } else { Speech::Chatter });
            self.chatter_timer = behavior::range(&mut self.rng, if self.hungry { HUNGRY_TALK_SECS } else { CHATTER_SECS });
```
   그리고 제외 목록에 `| State::Eating { .. } | State::LevelUp { .. }` 추가.
10. `rest_state` 맨 앞 (special 검사보다 먼저):
```rust
        if self.hungry && self.rng.f64() < HUNGRY_REST_CHANCE {
            return State::Hungry { remaining: behavior::range(&mut self.rng, HUNGRY_SECS) };
        }
```
11. `update_ground` 팔:
```rust
            State::Hungry { remaining } => {
                let remaining = remaining - dt;
                self.state = if remaining > 0.0 { State::Hungry { remaining } } else { self.move_state(true) };
            }
            State::Eating { remaining, favorite, level_up } => {
                let remaining = remaining - dt;
                self.state = if remaining > 0.0 {
                    State::Eating { remaining, favorite, level_up }
                } else if level_up {
                    self.speech = Some(Speech::LevelUp);
                    State::LevelUp { remaining: LEVEL_UP_SECS }
                } else if favorite {
                    State::Trick { kind: Trick::Heart, remaining: AFTER_FAVORITE_HEART_SECS }
                } else {
                    self.move_state(true)
                };
            }
            State::LevelUp { remaining } => {
                let remaining = remaining - dt;
                self.state = if remaining > 0.0 { State::LevelUp { remaining } } else { self.move_state(true) };
            }
```
12. `render`: `State::Hungry { .. } => Anim::Hungry, State::Eating { .. } => Anim::Eat, State::LevelUp { .. } => Anim::LevelUp,`
13. `react_to_tickle`: `State::Eating { .. } | State::LevelUp { .. } => return,` (먹는 중·축하 중엔 간질 무시).

(`eating_then_moving_or_hearting`의 favorite 단언은 `State::Trick { kind: Trick::Heart, .. }`인지 확인하는 의도다 — `p_remaining` 헬퍼 대신 `assert!(matches!(p.state, State::Trick { kind: Trick::Heart, .. }))`로 써도 된다.)

- [ ] **Step 4: 통과·커밋** — `cargo test -p screenpet-core` 통과 (번들 테스트 1개 ignored), 10분 시뮬레이션 그대로 통과, windows-gnu check → `feat(core): hungry sulking, eating, refusing and level-up celebration`

---

### Task 3: 그림 — 새 동작 3개 + 단계별 비율 시트

**Files:** Modify `tools/draw_pets.py`, `tools/sprite_spec.py`, `tools/test_draw_pets.py`, `crates/screenpet-core/src/manifest.rs` (ignore 제거), `docs/sprites.md`; Regenerate `ui/pets/*/` (`sheet.png`, `sheet_{baby,kinder,child,teenager,adult}.png`, `pet.json`).

**Interfaces:**
- Consumes: Task 2의 `REQUIRED_ANIMS` 21개 순서, `Speech` serde 이름 (`hungry`, `full`, `level_up`).
- Produces: `sprite_spec.ANIMS` 21개, `sprite_spec.STAGES = ["baby", "kinder", "child", "teenager", "adult"]`, `sprite_spec.STAGE_SCALE = {...}` (Rust `Stage::scale`와 같은 값), `SPEECH` 각 동물에 `"hungry": ["hungry...", "food?"], "full": ["full~"], "level_up": ["yay!", "♪♪"]` 추가, `pet.json`에 `"favorite_food": {"emoji": "🐟", "name": "생선"}` (강아지 🦴 뼈다귀, 토끼 🥕 당근, 비버 🪵 나뭇가지), `"stages": ["baby", ..., "adult"]`.

요구사항:
1. `ANIMS` 끝에 `("hungry", 3, 3, True, True)`, `("eat", 4, 6, True, True)`, `("level_up", 3, 6, False, True)`.
2. 포즈 (앞모습, 기존 `_front` 뼈대 재사용):
   - `hungry`: 시무룩하게 앉기 — 눈썹 처짐(︶ 모양 눈 위 짧은 사선), 입 작게 ‿ 뒤집힘, 한 앞발로 배 쓰다듬기(프레임마다 발 위치 2px), 머리 위 오른쪽에 생각 풍선(작은 원 2개 + 큰 원 안에 🍙 모양: 흰 삼각 밥 + 검은 김 띠).
   - `eat`: 앞에 놓인 그릇(낮은 반원, 갈색 외곽선, 안에 밥알) 위로 고개 숙여 냠냠 — 프레임마다 머리 3px 위아래, 입 열림/닫힘, 볼 부풀기, 그릇에서 작은 부스러기.
   - `level_up`: 만세 — 양팔 위로, 눈 ^^, 입 크게 웃음, 주변 반짝이(4각 별 3~4개, 노랑) 프레임마다 위치·크기 변화, 프레임 2는 1px 점프.
3. **단계별 비율** (모든 동작에 적용): 모듈 전역 또는 인자로 `stage`를 받아 머리 크기·몸·다리 비율을 바꾼다.

| 단계 | 머리 배율 | 몸 배율 | 다리 길이 | 외곽선 굵기(128 프레임 기준) |
|---|---|---|---|---|
| baby | 1.25 | 0.80 | 0.60 | 3.4 / 0.45 비율을 고려해 표시 크기에서 ≥ 2px → 약 4.6 |
| kinder | 1.15 | 0.88 | 0.75 | 약 3.8 |
| child | 1.07 | 0.94 | 0.88 | 약 3.4 |
| teenager | 1.00 | 1.00 | 1.00 | 현재 값 (기준) |
| adult | 0.93 | 1.06 | 1.10 | 현재 값 |

   비율을 바꾼 뒤에도 프레임 128 안, 가장자리 1px 투명, 서 있는 동작은 발 y ≥ 118, roll 공 원 안 규칙을 모두 지킨다 (필요하면 단계별로 전체를 살짝 축소). baby는 "아기"로 읽혀야 한다 — 머리가 몸보다 확실히 크고 다리가 짧다.
4. 출력: 종마다 `sheet_<stage>.png` 5장 + `sheet.png` (= teenager와 동일 파일 내용). `pet.json`은 하나 (레이아웃 같음).
5. 미리보기: `tools/preview/stages_<id>.png` (한 동물의 5단계 × 대표 동작 walk·sit_front·hungry·eat·level_up), `tools/preview/all_light.png`(teenager 전체), `all_dark.png`.
6. 테스트 (`test_draw_pets.py`): 기존 테스트를 모든 단계 시트에 대해 돌리도록 확장(시간이 길면 대표 단계 baby·teenager·adult만), 추가:
```python
def test_stage_sheets_and_manifest(tmp_path):
    import json
    from sprite_spec import STAGES
    draw_pets.build(tmp_path)
    for pid in ("cat", "dog", "rabbit", "beaver"):
        for st in STAGES:
            assert (tmp_path / pid / f"sheet_{st}.png").is_file(), (pid, st)
        assert (tmp_path / pid / "sheet.png").read_bytes() == (tmp_path / pid / "sheet_teenager.png").read_bytes()
        m = json.loads((tmp_path / pid / "pet.json").read_text(encoding="utf-8"))
        assert m["stages"] == STAGES
        assert m["favorite_food"]["emoji"]
        for key in ("hungry", "full", "level_up"):
            assert m["speech"][key], (pid, key)


def test_baby_head_is_relatively_bigger_than_adult(tmp_path):
    draw_pets.build(tmp_path)
    row = [a for a, *_ in ANIMS].index("sit_front")
    def head_ratio(st):
        cell = Image.open(tmp_path / "cat" / f"sheet_{st}.png").crop((0, row * FRAME, FRAME, (row + 1) * FRAME))
        a = cell.getchannel("A").point(lambda v: 255 if v > 128 else 0)
        widths = [sum(1 for x in range(FRAME) if a.getpixel((x, y))) for y in range(FRAME)]
        top = next(y for y, w in enumerate(widths) if w)
        bottom = max(y for y, w in enumerate(widths) if w)
        mid = top + (bottom - top) // 2
        return max(widths[top:mid]) / max(1, max(widths[mid:bottom + 1]))  # 위 절반(머리) 폭 / 아래 절반(몸) 폭
    assert head_ratio("baby") > head_ratio("teenager") > head_ratio("adult")
```
7. manifest.rs `bundled_placeholder_manifests_are_valid`의 `#[ignore]` 제거 → 통과.
8. `docs/sprites.md`: 21동작 표, 단계 시트 규칙 (`sheet_<stage>.png`, 없으면 `sheet.png`로 대체), `favorite_food`.
9. Read로 `stages_cat.png`, `all_light.png`를 직접 보고 다듬기 (최대 4회). 커밋 `feat(art): growth-stage sheets, hungry/eat/level_up poses`.

---

### Task 4: 앱 연결 — 돌보기 루프, 크기 변화, 음식 고르기, 트레이

**Files:** Modify `src-tauri/src/game_loop.rs`, `src-tauri/src/main.rs`, `src-tauri/src/tray.rs`, `src-tauri/src/pets.rs`(필요 시), `ui/index.html`, `ui/main.js`, `ui/style.css`, `README.md`

**Interfaces:**
- Consumes: `screenpet_core::care::{Care, Food, FeedResult, Stage}`, `config::{load, save, Config}` (care 맵), `Pet::{set_hungry, take_food_request, feed_result}`.
- Produces: 이벤트 `"pet-stage"` payload `{ "stage": "baby", "scale": 0.45 }`, `"food-picker"` payload `{ "emoji": "🐟", "name": "생선" }`(좋아하는 음식), 커맨드 `feed(food: String)`, `current_stage() -> { stage, scale }`.

1. **game_loop.rs**
   - `Input`에 `Feed(Food)`, `ToggleLock`, `ResetGrowth` 추가. `SetPet`에 `care: Care` 필드 추가 (교체할 동물의 저장된 돌보기 상태).
   - `run`은 시작 인자로 현재 동물의 `Care`와 저장 콜백을 받는다: `spawn(app, rx, walk_speed, special, care: Care, pet_id: String)`. 루프 안에서 `care.tick(dt)`; 100ms 갱신마다 `pet.set_hungry(care.is_hungry())`.
   - 저장: `fn save_care(app: &AppHandle, pet_id: &str, care: &Care)` — `AppState.config_dir`의 config를 load → `care` 맵에 넣기 → save (실패는 eprintln). 60초마다, 그리고 Feed/ToggleLock/ResetGrowth/SetPet 직전(이전 동물)에 호출.
   - 크기: `PET_LOGICAL_SIZE * STAGE_SCALE` 상수 대신 `care.stage.scale()`. 단계가 바뀔 때(먹고 성장, 리셋, 동물 교체) `apply_stage(...)`:
```rust
fn window_logical_size(stage_scale: f64) -> (f64, f64) {
    (PET_LOGICAL_SIZE * stage_scale.max(1.0), BUBBLE_SPACE + PET_LOGICAL_SIZE * stage_scale)
}
```
     `window.set_size(LogicalSize::new(w, h))`, `pet.cfg = pet_config(dpi_scale, logical_walk, special, stage_scale)` (size = 128 × stage × dpi), `app.emit("pet-stage", StagePayload { stage: stage.name(), scale })`.
   - 창 위치: `let (sx, sy) = pet.window_pos();` 창 너비(물리) `win_w = PET_LOGICAL_SIZE * stage_scale.max(1.0) * dpi`; `pos = (pet.body.x - win_w / 2.0, sy - BUBBLE_SPACE * dpi)`.
   - Feed 처리: `let result = care.feed(food); if pet.feed_result(result, food) { if let FeedResult::Ate { leveled_up: true } = result { /* 크기는 축하가 시작될 때 */ pending_stage = true; } save_care(..) } else { /* 공중 — 결과 되돌리기 */ care = before; }` — 공중이면 먹지 않은 것으로 하기 위해 `let before = care.clone();`를 먼저 저장해 되돌린다.
   - 성장 크기 적용 시점: `pending_stage`이고 `pet.render().anim == Anim::LevelUp`가 된 틱에 `apply_stage`.
   - `take_food_request()`가 true면 `app.emit("food-picker", favorite_payload)`.
   - ToggleLock: `care.locked = !care.locked`, 저장. ResetGrowth: `care.reset()`, 저장, `apply_stage`.
2. **main.rs**
   - `AppState`에 `favorite: ...` 불필요 — pets 매니페스트에서 읽는다. `PetEntry`/`PetManifest`에 `favorite_food`가 필요하면 manifest.rs에 `#[serde(default)] pub favorite_food: Option<FavoriteFood>` (`emoji: String, name: String`) 추가 (core 변경 — 테스트 1개: 파싱).
   - setup: `config::load` → 현재 동물 `care` (없으면 default) → `spawn(..., care, current.clone())`.
   - `select_pet`: 저장된 care를 config에서 읽어 `Input::SetPet { walk_speed, special, care }`. 이전 동물 저장은 game loop가 SetPet 처리 전에 한다. `Config { pet: Some(id), ..loaded }`로 저장(care 맵 보존!).
   - 커맨드:
```rust
#[tauri::command]
fn feed(state: State<AppState>, food: String) {
    let food = match food.as_str() {
        "snack" => Food::Snack,
        "meal" => Food::Meal,
        "favorite" => Food::Favorite,
        _ => return,
    };
    let _ = state.tx.lock().unwrap().send(Input::Feed(food));
}
```
     `generate_handler!`에 `feed` 추가. 현재 단계는 이벤트로 보내고, 웹뷰 초기 로드를 위해 `current_stage` 커맨드: AppState에 `stage: Mutex<(String, f64)>`를 두고 game loop가 apply_stage 때 갱신.
3. **tray.rs** — 기존 동물 목록 아래:
   - `Submenu "밥 주기"`: `feed:snack` "간식 🍪", `feed:meal` "밥 🍚", `feed:favorite` "<좋아하는 것> <emoji>" (현재 동물 기준 — 동물 교체 시 `set_text`로 갱신).
   - `CheckMenuItem` `lock` "성장 멈추기" (현재 동물의 locked로 체크, 교체 시 갱신).
   - `Submenu "처음부터 다시 키우기"` 안에 `MenuItem` `reset` "정말 처음부터 키울까요? (새끼로 돌아가요)".
   - 이벤트: `feed:*` → `Input::Feed`, `lock` → `Input::ToggleLock`, `reset` → `Input::ResetGrowth`. (Tauri 2: `Submenu::with_items(app, "밥 주기", true, &[&item1, &item2, &item3])`, `CheckMenuItem::set_checked`, `MenuItem::set_text`.)
4. **웹뷰**
   - `index.html`: `<div id="picker" hidden><button data-food="snack">🍪</button><button data-food="meal">🍚</button><button data-food="favorite" id="fav">🐟</button></div>`.
   - `style.css`: `#picker { position:absolute; top:2px; left:50%; transform:translateX(-50%); display:flex; gap:4px; }`, 버튼은 28×28 둥근 흰 원 + 2px 갈색 테두리, emoji 16px, hover 시 1.1배.
   - `main.js`:
     - `let stage = { stage: "teenager", scale: 1 };`, `loadPet(id)`가 `pets/${id}/sheet_${stage.stage}.png`를 먼저 시도하고 실패하면 `sheet.png`.
     - `#pet`의 크기·위치를 단계 배율에 맞춤: `const side = 128 * stage.scale; petEl.style.width = side + "px"; petEl.style.left = (window.innerWidth - side) / 2 + "px";` (CSS `#pet`의 `width:100%` → 위 JS가 지정, `left:0` 유지 초기값), `display = side`.
     - `listen("pet-stage", e => { stage = e.payload; loadPet(currentId); })`, 초기화 때 `stage = await invoke("current_stage")`.
     - `listen("food-picker", e => { favBtn.textContent = e.payload.emoji; show picker 6초 })`, 버튼 클릭 → `invoke("feed", { food })` 후 숨김. 버튼 `mousedown`은 `stopPropagation` (펫 드래그 시작 방지).
   - `#bubble`과 `#picker`가 동시에 보이면 picker가 우선(bubble 숨김).
5. **README** 체크리스트:
```markdown
- [ ] 처음 실행하면 아주 작은 아기 동물
- [ ] 한참(약 22분) 지나면 배고파함: 시무룩 + 🍙 생각 풍선 + "hungry..." 말풍선
- [ ] 배고플 때 클릭 → 머리 위에 🍪 🍚 (좋아하는 것) 버튼 → 고르면 냠냠
- [ ] 배부를 때 트레이 밥 주기 → "full~" 하고 갸웃
- [ ] 여러 번 먹이면 "yay!" 만세하며 한 단계 커짐 (트레이에 단계 표시 없음)
- [ ] 트레이 "성장 멈추기" 켜면 먹어도 안 큼, "처음부터 다시 키우기" → 아기로
- [ ] 동물마다 성장 따로 저장, 앱 재시작 후에도 유지
```
6. 검증: `cargo test` (core에 favorite_food 파싱 테스트 포함) 통과, `node --check ui/main.js`, JSON 유효. Tauri API(`Submenu::with_items`, `MenuItem::set_text`, `set_size`)는 `~/.cargo/registry/src/*/tauri-2.*/` 소스로 대조. 커밋 `feat(app): feeding, growth stages and tray care menu`.
