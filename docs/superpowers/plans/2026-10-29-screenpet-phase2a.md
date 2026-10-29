# ScreenPet 2A Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 동물을 활발하게(걷기·뛰기·구르기, 애교 6종, 랜덤 클릭 반응) 만들고 그림을 128px 말랑 카와이 스타일 16동작으로 전면 교체한다.

**Architecture:** 다음 행동을 고르는 확률·시간 수치를 새 `behavior.rs`(순수 함수 + `fastrand::Rng`)로 분리하고, `brain.rs` 상태 머신은 `Move{kind}`/`Trick{kind}` 상태로 확장한다. 그림은 `tools/draw_pets.py`가 도형 합집합을 블러+임계값으로 녹인 실루엣에 굵은 외곽선을 입혀 4종 × 16동작 시트를 생성한다. 웹뷰는 창 크기에 맞춰 그리고, `roll`만 연속 회전시킨다.

**Tech Stack:** Rust (screenpet-core, Tauri 2 app), Python 3 + Pillow, 바닐라 JS, GitHub Actions.

**Spec:** `docs/superpowers/specs/2026-10-29-screenpet-phase2a-design.md` (v1 spec: `2026-10-28-desktop-pets-design.md`)

## Global Constraints

- 수치 (spec §2): 걷기 4~10초, 쉬기 1.5~3초, 잠 확률 8%·6~12초, 이동 선택 걷기 55% / 뛰기 30% / 구르기 15%, 뛰기 ×2.5·2~5초, 구르기 ×2.0·2~4초, 이동 종료 시 30% Turn, 쉬기 중 애교 30%·2~4초, 클릭 애교 2초, 걷기 Edge 80% Hop, 뛰기 Edge 항상 점프(vx ×1.8), 구르기 Edge 속도 유지 낙하, Blocked/Wall은 항상 Turn.
- 걷기 속도 (`pet.json` `walk_speed`): 고양이 90 / 강아지 105 / 토끼 120 / 비버 60.
- 필수 동작 16개, 이 순서 (Rust `REQUIRED_ANIMS` = Python `ANIMS` = 시트 행 순서): `walk run roll idle look sleep hop fall dragged land sit_front wave tilt belly stretch heart`.
- 앞모습 동작(`sit_front wave tilt belly stretch heart`)은 좌우 반전하지 않는다.
- 프레임 128×128, `frame_size: 128`, 표시 128 논리 px (확대 없음).
- `roll` 공: 중심 (64, 88), 반지름 38(외곽선 포함) 안. `spin = walk_speed × 2.0 / 38 × 180/π` (deg/s). 웹뷰 회전축 `transform-origin: 50% 68.75%`.
- 스타일: 말랑 카와이 — 녹인 한 덩어리 실루엣, 외곽선 RGB (132,96,92) 약 3px, 큰 머리, 점 눈+하이라이트, 빗금 볼터치. 시안: `design-preview` 브랜치 `design-preview/kawaii-all-light.png`.
- 고양이는 사용자 고양이 무늬 (흰 바탕 + 치즈색 머리·귀·등·꼬리 줄무늬, 이마 흰 줄, 고양이 왼쪽 볼 치즈 점).
- 크기 배율: `STAGE_SCALE = 1.0` (teenager). 2B에서 바꿀 자리만 만든다.
- 리눅스 서버: `src-tauri`는 컴파일 불가 (CI가 검증). cargo 명령 앞에 `. "$HOME/.cargo/env" &&`.
- 커밋 메시지 끝: `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`. 브랜치 `feat/phase2a`. 푸시는 컨트롤러가 한다.

## File Structure

```
crates/screenpet-core/src/
  behavior.rs   (신규) Move, Trick, Rest, ClickReaction, pick_* 함수, 수치 상수
  brain.rs      State::Move/Trick, Anim 16종, 이동별 Edge 규칙, 애교·클릭 반응, render 앞모습 반전 금지
  manifest.rs   REQUIRED_ANIMS 16, AnimSpec.front / spin
  lib.rs        pub mod behavior;
tools/
  sprite_spec.py   ANIMS 16 (name, frames, fps, loop, front), FRAME=128, ROLL_* 상수, roll_spin()
  draw_pets.py     (신규) 카와이 엔진 + 종별 설정 + 포즈 → ui/pets/*/sheet.png, pet.json
  test_draw_pets.py (신규)
  pack_sheet.py    128 규격, front/spin 반영
  gen_placeholder.py  삭제
ui/
  main.js, style.css   창 크기 기준 표시, roll 회전, flip은 transform으로
  pets/{cat,dog,rabbit,beaver}/   재생성
src-tauri/src/game_loop.rs   STAGE_SCALE, 창 크기 설정
docs/sprites.md, README.md
```

---

### Task 1: 행동 규칙 (behavior.rs + brain.rs)

**Files:**
- Create: `crates/screenpet-core/src/behavior.rs`
- Modify: `crates/screenpet-core/src/lib.rs`, `crates/screenpet-core/src/brain.rs`

**Interfaces:**
- Consumes: 기존 `Pet`, `State`, `World`, `EndKind`, `Body`.
- Produces:
```rust
// behavior.rs
pub enum Move { Walk, Run, Roll }                       // Copy, Eq, Debug
impl Move { pub fn speed_mult(self) -> f64; pub fn secs(self) -> (f64, f64) }
pub enum Trick { SitFront, Wave, Tilt, Belly, Stretch, Heart }
pub const TRICKS: [Trick; 6];
pub enum Rest { Idle, Trick(Trick) }
pub enum ClickReaction { Hop, Trick(Trick) }
pub fn pick_move(rng: &mut fastrand::Rng) -> Move;
pub fn pick_rest(rng: &mut fastrand::Rng) -> Rest;
pub fn pick_trick(rng: &mut fastrand::Rng) -> Trick;
pub fn pick_click(rng: &mut fastrand::Rng) -> ClickReaction;
pub fn range(rng: &mut fastrand::Rng, (lo, hi): (f64, f64)) -> f64;
pub const IDLE_SECS, SLEEP_SECS, SLEEP_CHANCE, TRICK_SECS, TRICK_CHANCE, MOVE_END_TURN_CHANCE,
          WALK_EDGE_HOP_CHANCE, RUN_LEAP_VX_MULT, CLICK_TRICK_SECS;
// brain.rs
#[serde(rename_all = "snake_case")] pub enum Anim { Walk, Run, Roll, Idle, Look, Sleep, Hop, Fall, Dragged, Land,
                                                      SitFront, Wave, Tilt, Belly, Stretch, Heart }
impl Anim { pub fn is_front(self) -> bool }
pub enum State { Move { kind: Move, remaining: f64 }, Idle{..}, Sleep{..}, Turn{..}, Land{..},
                 Trick { kind: Trick, remaining: f64 }, Air{..}, Pressed{..}, Dragged }
```
`State::Walk`는 없어진다 (`State::Move { kind: Move::Walk, .. }`).

- [ ] **Step 1: behavior.rs 실패 테스트 작성** — `crates/screenpet-core/src/behavior.rs` 하단 (파일 상단은 Step 3에서), `lib.rs`에 `pub mod behavior;` 추가

```rust
#[cfg(test)]
mod tests {
    use super::*;

    fn share<T: PartialEq>(n: usize, mut f: impl FnMut(&mut fastrand::Rng) -> T, want: T) -> f64 {
        let mut rng = fastrand::Rng::with_seed(7);
        (0..n).filter(|_| f(&mut rng) == want).count() as f64 / n as f64
    }

    #[test]
    fn move_mix_is_55_30_15() {
        assert!((0.52..=0.58).contains(&share(20_000, pick_move, Move::Walk)));
        assert!((0.27..=0.33).contains(&share(20_000, pick_move, Move::Run)));
        assert!((0.13..=0.17).contains(&share(20_000, pick_move, Move::Roll)));
    }

    #[test]
    fn rest_is_trick_30_percent() {
        let mut rng = fastrand::Rng::with_seed(3);
        let n = 20_000;
        let tricks = (0..n).filter(|_| matches!(pick_rest(&mut rng), Rest::Trick(_))).count() as f64 / n as f64;
        assert!((0.27..=0.33).contains(&tricks), "{tricks}");
    }

    #[test]
    fn every_trick_and_click_reaction_appears() {
        let mut rng = fastrand::Rng::with_seed(5);
        for t in TRICKS {
            assert!((0..600).any(|_| pick_trick(&mut rng) == t), "{t:?}");
        }
        let mut seen = [false; 4];
        for _ in 0..400 {
            let i = match pick_click(&mut rng) {
                ClickReaction::Hop => 0,
                ClickReaction::Trick(Trick::Heart) => 1,
                ClickReaction::Trick(Trick::Wave) => 2,
                ClickReaction::Trick(Trick::Tilt) => 3,
                ClickReaction::Trick(t) => panic!("click gave {t:?}"),
            };
            seen[i] = true;
        }
        assert_eq!(seen, [true; 4]);
    }

    #[test]
    fn move_speeds_and_durations() {
        assert_eq!((Move::Walk.speed_mult(), Move::Run.speed_mult(), Move::Roll.speed_mult()), (1.0, 2.5, 2.0));
        assert_eq!((Move::Walk.secs(), Move::Run.secs(), Move::Roll.secs()), ((4.0, 10.0), (2.0, 5.0), (2.0, 4.0)));
    }

    #[test]
    fn range_stays_inside() {
        let mut rng = fastrand::Rng::with_seed(1);
        assert!((0..1000).all(|_| (1.5..=3.0).contains(&range(&mut rng, (1.5, 3.0)))));
    }
}
```

- [ ] **Step 2: 실패 확인** — `cargo test -p screenpet-core behavior` → FAIL (타입 없음)

- [ ] **Step 3: behavior.rs 구현** (파일 상단)

```rust
//! 다음 행동 고르기 — 이동 방식, 쉬기/애교, 클릭 반응. 확률·시간 수치는 모두 여기에 둔다.

pub const IDLE_SECS: (f64, f64) = (1.5, 3.0);
pub const SLEEP_SECS: (f64, f64) = (6.0, 12.0);
pub const SLEEP_CHANCE: f64 = 0.08;
pub const TRICK_SECS: (f64, f64) = (2.0, 4.0);
pub const TRICK_CHANCE: f64 = 0.3;
pub const MOVE_END_TURN_CHANCE: f64 = 0.3;
pub const WALK_EDGE_HOP_CHANCE: f64 = 0.8;
pub const RUN_LEAP_VX_MULT: f64 = 1.8;
pub const CLICK_TRICK_SECS: f64 = 2.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Move {
    Walk,
    Run,
    Roll,
}

impl Move {
    pub fn speed_mult(self) -> f64 {
        match self {
            Move::Walk => 1.0,
            Move::Run => 2.5,
            Move::Roll => 2.0,
        }
    }

    pub fn secs(self) -> (f64, f64) {
        match self {
            Move::Walk => (4.0, 10.0),
            Move::Run => (2.0, 5.0),
            Move::Roll => (2.0, 4.0),
        }
    }
}

/// 앞모습 애교 동작
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Trick {
    SitFront,
    Wave,
    Tilt,
    Belly,
    Stretch,
    Heart,
}

pub const TRICKS: [Trick; 6] = [Trick::SitFront, Trick::Wave, Trick::Tilt, Trick::Belly, Trick::Stretch, Trick::Heart];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Rest {
    Idle,
    Trick(Trick),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClickReaction {
    /// v1의 하트 + 제자리 점프
    Hop,
    Trick(Trick),
}

pub fn pick_move(rng: &mut fastrand::Rng) -> Move {
    let r = rng.f64();
    if r < 0.55 {
        Move::Walk
    } else if r < 0.85 {
        Move::Run
    } else {
        Move::Roll
    }
}

pub fn pick_rest(rng: &mut fastrand::Rng) -> Rest {
    if rng.f64() < TRICK_CHANCE {
        Rest::Trick(pick_trick(rng))
    } else {
        Rest::Idle
    }
}

pub fn pick_trick(rng: &mut fastrand::Rng) -> Trick {
    TRICKS[rng.usize(..TRICKS.len())]
}

pub fn pick_click(rng: &mut fastrand::Rng) -> ClickReaction {
    match rng.usize(..4) {
        0 => ClickReaction::Hop,
        1 => ClickReaction::Trick(Trick::Heart),
        2 => ClickReaction::Trick(Trick::Wave),
        _ => ClickReaction::Trick(Trick::Tilt),
    }
}

pub fn range(rng: &mut fastrand::Rng, (lo, hi): (f64, f64)) -> f64 {
    lo + rng.f64() * (hi - lo)
}
```

- [ ] **Step 4: 통과 확인** — `cargo test -p screenpet-core behavior` → 5 passed. 커밋 `feat(core): behavior picks for moves, tricks and click reactions`

- [ ] **Step 5: brain.rs 새 테스트 작성 + 기존 테스트 갱신** (brain.rs `mod tests` / `mod input_tests`)

`mod tests`에서 `use super::*;` 아래에 `use crate::behavior::{Move, Trick};` 추가. 기존 테스트 수정:
- 모든 `State::Walk { remaining: X }` → `State::Move { kind: Move::Walk, remaining: X }`.
- `blocked_end_turns_around` 마지막 단언: `assert!(matches!(p.state, State::Move { .. }));`
- `window_edge_turns_or_hops_about_half_each`를 아래로 교체:
```rust
    #[test]
    fn walking_to_a_window_edge_hops_about_80_percent() {
        let w = world(vec![w1()]);
        let (mut hops, mut turns) = (0, 0);
        for seed in 0..400 {
            let mut p = Pet::new(cfg(), 0.0, 0.0, seed);
            place(&mut p, &w, Owner::Window(1), 1100.0 - 38.0 - 0.5, State::Move { kind: Move::Walk, remaining: 100.0 });
            p.facing = 1.0;
            p.update(DT, &w);
            match p.state {
                State::Air { anim: Anim::Hop } => hops += 1,
                State::Turn { .. } => turns += 1,
                s => panic!("unexpected {s:?}"),
            }
        }
        assert!((280..=360).contains(&hops), "hops={hops}");
        assert_eq!(hops + turns, 400);
    }
```
- `expired_walk_becomes_idle_or_turn` 단언 → `assert!(matches!(p.state, State::Idle { .. } | State::Turn { .. } | State::Trick { .. }));`
- `expired_idle_becomes_sleep_or_walk` 단언 → `assert!(matches!(p.state, State::Sleep { .. } | State::Move { .. }));`

새 테스트 (`mod tests`에 추가):
```rust
    #[test]
    fn running_moves_two_and_a_half_times_faster() {
        let w = world(vec![]);
        let mut p = Pet::new(cfg(), 0.0, 0.0, 1);
        place(&mut p, &w, Owner::Ground(0), 900.0, State::Move { kind: Move::Run, remaining: 100.0 });
        p.facing = 1.0;
        p.update(DT, &w);
        assert!((p.body.x - (900.0 + 60.0 * 2.5 * DT)).abs() < 1e-9);
    }

    #[test]
    fn running_off_a_window_edge_always_leaps_far() {
        let w = world(vec![w1()]);
        for seed in 0..50 {
            let mut p = Pet::new(cfg(), 0.0, 0.0, seed);
            place(&mut p, &w, Owner::Window(1), 1100.0 - 38.0 - 0.5, State::Move { kind: Move::Run, remaining: 100.0 });
            p.facing = 1.0;
            p.update(DT, &w);
            assert_eq!(p.state, State::Air { anim: Anim::Hop }, "seed {seed}");
            assert!(p.body.vx > 300.0);
        }
    }

    #[test]
    fn rolling_off_a_window_edge_keeps_rolling_speed() {
        let w = world(vec![w1()]);
        let mut p = Pet::new(cfg(), 0.0, 0.0, 1);
        place(&mut p, &w, Owner::Window(1), 1100.0 - 38.0 - 0.5, State::Move { kind: Move::Roll, remaining: 100.0 });
        p.facing = 1.0;
        p.update(DT, &w);
        assert_eq!(p.state, State::Air { anim: Anim::Roll });
        assert_eq!((p.body.vx, p.body.vy), (120.0, 0.0));
        assert!(p.standing.is_none());
    }

    #[test]
    fn blocked_end_turns_for_every_move_kind() {
        let w = world(vec![win(2, 900.0, 300.0, 1300.0, 600.0), w1()]);
        for kind in [Move::Walk, Move::Run, Move::Roll] {
            let mut p = Pet::new(cfg(), 0.0, 0.0, 1);
            place(&mut p, &w, Owner::Window(1), 900.0 - 38.0 - 0.5, State::Move { kind, remaining: 100.0 });
            p.facing = 1.0;
            p.update(DT, &w);
            assert!(matches!(p.state, State::Turn { .. }), "{kind:?}");
        }
    }

    #[test]
    fn tricks_face_front_and_end_by_moving() {
        let w = world(vec![]);
        let mut p = Pet::new(cfg(), 0.0, 0.0, 1);
        place(&mut p, &w, Owner::Ground(0), 900.0, State::Trick { kind: Trick::Wave, remaining: 0.5 });
        p.facing = -1.0;
        assert_eq!(p.render(), RenderState { anim: Anim::Wave, flip: false, heart: false });
        run(&mut p, &w, 40);
        assert!(matches!(p.state, State::Move { .. }));
    }

    #[test]
    fn covered_during_trick_falls() {
        let w = world(vec![w1()]);
        let mut p = Pet::new(cfg(), 0.0, 0.0, 1);
        place(&mut p, &w, Owner::Window(1), 800.0, State::Trick { kind: Trick::Belly, remaining: 3.0 });
        p.update(DT, &world(vec![win(2, 700.0, 300.0, 900.0, 600.0), w1()]));
        assert_eq!(p.state, State::Air { anim: Anim::Fall });
    }

    #[test]
    fn side_animations_still_flip() {
        let mut p = Pet::new(cfg(), 0.0, 0.0, 1);
        p.facing = -1.0;
        p.state = State::Move { kind: Move::Roll, remaining: 1.0 };
        assert_eq!(p.render(), RenderState { anim: Anim::Roll, flip: true, heart: false });
    }

    #[test]
    fn front_animation_set_is_exactly_the_six_tricks() {
        let front: Vec<Anim> = [Anim::Walk, Anim::Run, Anim::Roll, Anim::Idle, Anim::Look, Anim::Sleep, Anim::Hop, Anim::Fall,
            Anim::Dragged, Anim::Land, Anim::SitFront, Anim::Wave, Anim::Tilt, Anim::Belly, Anim::Stretch, Anim::Heart]
            .into_iter().filter(|a| a.is_front()).collect();
        assert_eq!(front, vec![Anim::SitFront, Anim::Wave, Anim::Tilt, Anim::Belly, Anim::Stretch, Anim::Heart]);
        assert_eq!(serde_json::to_string(&Anim::SitFront).unwrap(), "\"sit_front\"");
    }

    /// 10분 시뮬레이션 — "계속 자기만 한다" 회귀 방지 (spec §6)
    #[test]
    fn ten_minutes_are_mostly_moving_and_rarely_sleeping() {
        let w = world(vec![]);
        let (mut moving, mut sleeping, mut total) = (0usize, 0usize, 0usize);
        for seed in 0..20 {
            let mut p = Pet::new(PetConfig { size: 128.0, half_w: 38.4, walk_speed: 90.0 }, 960.0, 128.0, seed);
            for _ in 0..(60 * 600) {
                p.update(DT, &w);
                match p.render().anim {
                    Anim::Walk | Anim::Run | Anim::Roll => moving += 1,
                    Anim::Sleep => sleeping += 1,
                    _ => {}
                }
                total += 1;
            }
        }
        let (m, s) = (moving as f64 / total as f64, sleeping as f64 / total as f64);
        assert!((0.65..=0.75).contains(&m), "moving {m:.3}");
        assert!(s <= 0.10, "sleeping {s:.3}");
    }
```

`mod input_tests`: `use crate::behavior::Trick;` 추가. `click_on_grounded_pet_is_happy_and_lands_back`와 `tiny_move_still_counts_as_click`를 아래로 교체:
```rust
    #[test]
    fn click_on_grounded_pet_reacts_with_hop_or_trick() {
        let w = world(vec![w1()]);
        let mut seen = std::collections::HashSet::new();
        for seed in 0..80 {
            let mut p = Pet::new(cfg(), 0.0, 0.0, seed);
            place(&mut p, &w, Owner::Window(1), 800.0, idle());
            p.pointer_down(800.0, 350.0);
            p.pointer_up(800.0, 350.0);
            match p.state {
                State::Air { anim: Anim::Hop } => {
                    assert!(p.render().heart);
                    run(&mut p, &w, 120);
                    assert_eq!(p.standing.unwrap().owner, Owner::Window(1));
                    seen.insert("hop");
                }
                State::Trick { kind, remaining } => {
                    assert_eq!(remaining, 2.0);
                    assert!(p.standing.is_some());
                    seen.insert(match kind {
                        Trick::Heart => "heart",
                        Trick::Wave => "wave",
                        Trick::Tilt => "tilt",
                        k => panic!("unexpected {k:?}"),
                    });
                }
                s => panic!("unexpected {s:?}"),
            }
        }
        assert_eq!(seen.len(), 4);
    }

    #[test]
    fn tiny_move_still_counts_as_click() {
        let w = world(vec![w1()]);
        let mut p = Pet::new(cfg(), 0.0, 0.0, 1);
        place(&mut p, &w, Owner::Window(1), 800.0, idle());
        p.pointer_down(800.0, 350.0);
        p.pointer_move(803.0, 351.0);
        p.pointer_up(803.0, 351.0);
        assert!(matches!(p.state, State::Air { anim: Anim::Hop } | State::Trick { .. }));
    }
```

- [ ] **Step 6: 실패 확인** — `cargo test -p screenpet-core brain` → FAIL (컴파일 에러: `State::Move`, `Anim::Run` 등 없음)

- [ ] **Step 7: brain.rs 구현**

1. import·상수: `use crate::behavior::{self, ClickReaction, Move, Rest, Trick};` 추가. `WALK_SECS IDLE_SECS SLEEP_SECS SLEEP_CHANCE WALK_END_TURN_CHANCE EDGE_HOP_CHANCE` 상수 삭제 (behavior로 이동). `fn range` 메서드 삭제 (→ `behavior::range(&mut self.rng, ..)`).
2. `Anim`:
```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Anim {
    Walk,
    Run,
    Roll,
    Idle,
    Look,
    Sleep,
    Hop,
    Fall,
    Dragged,
    Land,
    SitFront,
    Wave,
    Tilt,
    Belly,
    Stretch,
    Heart,
}

impl Anim {
    /// 앞모습 동작 — 좌우 반전하지 않는다.
    pub fn is_front(self) -> bool {
        matches!(self, Anim::SitFront | Anim::Wave | Anim::Tilt | Anim::Belly | Anim::Stretch | Anim::Heart)
    }
}

impl From<Move> for Anim {
    fn from(m: Move) -> Self {
        match m {
            Move::Walk => Anim::Walk,
            Move::Run => Anim::Run,
            Move::Roll => Anim::Roll,
        }
    }
}

impl From<Trick> for Anim {
    fn from(t: Trick) -> Self {
        match t {
            Trick::SitFront => Anim::SitFront,
            Trick::Wave => Anim::Wave,
            Trick::Tilt => Anim::Tilt,
            Trick::Belly => Anim::Belly,
            Trick::Stretch => Anim::Stretch,
            Trick::Heart => Anim::Heart,
        }
    }
}
```
3. `State`: `Walk { remaining: f64 }` → `Move { kind: Move, remaining: f64 }`, `Trick { kind: Trick, remaining: f64 }` 추가 (Land 다음).
4. `render`:
```rust
        let anim = match self.state {
            State::Move { kind, .. } => kind.into(),
            State::Trick { kind, .. } => kind.into(),
            State::Idle { look: true, .. } => Anim::Look,
            State::Idle { .. } | State::Turn { .. } => Anim::Idle,
            State::Sleep { .. } => Anim::Sleep,
            State::Land { .. } => Anim::Land,
            State::Air { anim } => anim,
            State::Pressed { grounded: true, .. } => Anim::Idle,
            State::Pressed { .. } | State::Dragged => Anim::Dragged,
        };
        RenderState { anim, flip: self.facing < 0.0 && !anim.is_front(), heart: self.heart_timer > 0.0 }
```
5. `pointer_up`의 `State::Pressed { grounded: true, .. }` 팔:
```rust
            State::Pressed { grounded: true, .. } => match behavior::pick_click(&mut self.rng) {
                ClickReaction::Hop => {
                    self.heart_timer = HEART_SECS;
                    self.standing = None;
                    self.body.vx = 0.0;
                    self.body.vy = HAPPY_VY;
                    self.state = State::Air { anim: Anim::Hop };
                }
                ClickReaction::Trick(kind) => {
                    self.state = State::Trick { kind, remaining: behavior::CLICK_TRICK_SECS };
                }
            },
```
6. `update_ground`의 `match self.state` 본문 교체:
```rust
        match self.state {
            State::Move { kind, remaining } => {
                let speed = self.cfg.walk_speed * kind.speed_mult();
                let new_x = self.body.x + self.facing * speed * dt;
                let (hit, end) = if self.facing > 0.0 {
                    (new_x + self.cfg.half_w > piece.x_end, piece.end_kind)
                } else {
                    (new_x - self.cfg.half_w < piece.x_start, piece.start_kind)
                };
                if hit {
                    match (end, kind) {
                        (EndKind::Edge, Move::Walk) if self.rng.f64() < behavior::WALK_EDGE_HOP_CHANCE => {
                            self.start_hop(self.facing)
                        }
                        (EndKind::Edge, Move::Run) => {
                            self.start_hop(self.facing);
                            self.body.vx *= behavior::RUN_LEAP_VX_MULT;
                        }
                        (EndKind::Edge, Move::Roll) => {
                            self.standing = None;
                            self.body.vx = self.facing * speed;
                            self.body.vy = 0.0;
                            self.state = State::Air { anim: Anim::Roll };
                        }
                        _ => self.state = State::Turn { remaining: TURN_PAUSE },
                    }
                    return;
                }
                self.standing = Some(Standing { owner: st.owner, rel_x: st.rel_x + (new_x - self.body.x) });
                self.body.x = new_x;
                let remaining = remaining - dt;
                self.state = if remaining > 0.0 {
                    State::Move { kind, remaining }
                } else if self.rng.f64() < behavior::MOVE_END_TURN_CHANCE {
                    State::Turn { remaining: TURN_PAUSE }
                } else {
                    self.rest_state()
                };
            }
            State::Idle { remaining, look } => {
                let remaining = remaining - dt;
                self.state = if remaining > 0.0 {
                    State::Idle { remaining, look }
                } else if self.rng.f64() < behavior::SLEEP_CHANCE {
                    State::Sleep { remaining: behavior::range(&mut self.rng, behavior::SLEEP_SECS) }
                } else {
                    self.move_state(true)
                };
            }
            State::Trick { kind, remaining } => {
                let remaining = remaining - dt;
                self.state = if remaining > 0.0 { State::Trick { kind, remaining } } else { self.move_state(true) };
            }
            State::Sleep { remaining } => {
                let remaining = remaining - dt;
                self.state = if remaining > 0.0 { State::Sleep { remaining } } else { self.idle_state() };
            }
            State::Turn { remaining } => {
                let remaining = remaining - dt;
                self.state = if remaining > 0.0 {
                    State::Turn { remaining }
                } else {
                    self.facing = -self.facing;
                    self.move_state(false)
                };
            }
            State::Land { remaining } => {
                let remaining = remaining - dt;
                self.state = if remaining > 0.0 { State::Land { remaining } } else { self.rest_state() };
            }
            State::Air { .. } | State::Pressed { .. } | State::Dragged => {}
        }
```
7. 헬퍼 (기존 `idle_state` 교체, `range` 삭제):
```rust
    fn idle_state(&mut self) -> State {
        State::Idle { remaining: behavior::range(&mut self.rng, behavior::IDLE_SECS), look: self.rng.bool() }
    }

    /// 이동 시작. may_flip이면 절반 확률로 방향을 바꾼다 (Turn 직후엔 이미 바꿨으므로 false).
    fn move_state(&mut self, may_flip: bool) -> State {
        if may_flip && self.rng.bool() {
            self.facing = -self.facing;
        }
        let kind = behavior::pick_move(&mut self.rng);
        State::Move { kind, remaining: behavior::range(&mut self.rng, kind.secs()) }
    }

    /// 이동이 끝난 뒤: 옆모습 쉬기 또는 앞모습 애교
    fn rest_state(&mut self) -> State {
        match behavior::pick_rest(&mut self.rng) {
            Rest::Idle => self.idle_state(),
            Rest::Trick(kind) => State::Trick { kind, remaining: behavior::range(&mut self.rng, behavior::TRICK_SECS) },
        }
    }
```
`pointer_up`의 잠 깨우기 팔(`was_sleeping: true`)은 그대로 `self.idle_state()`.

- [ ] **Step 8: 통과 확인** — `cargo test -p screenpet-core` → 전체 통과, 경고 없음, `cargo check -p screenpet-core --target x86_64-pc-windows-gnu` 통과.
  10분 시뮬레이션이 범위를 벗어나면 **상수를 바꾸지 말고** 실제 비율을 보고서에 적고 DONE_WITH_CONCERNS로 보고한다 (수치는 spec이 정한 값).

- [ ] **Step 9: 커밋** — `feat(core): run/roll moves, front tricks, random click reactions`

---

### Task 2: 카와이 그림 파이프라인 (16동작 × 4종, 128px)

**Files:**
- Create: `tools/draw_pets.py`, `tools/test_draw_pets.py`
- Modify: `tools/sprite_spec.py`, `tools/pack_sheet.py`, `tools/test_pack_sheet.py`, `crates/screenpet-core/src/manifest.rs`, `docs/sprites.md`
- Delete: `tools/gen_placeholder.py`
- Regenerate (커밋): `ui/pets/{cat,dog,rabbit,beaver}/{sheet.png,pet.json}`

**Interfaces:**
- Consumes: Task 1의 `Anim` serde 이름 (snake_case 16개).
- Produces: 16동작 매니페스트 (`front`, `spin` 포함), `sprite_spec.ANIMS` 5-튜플 `(name, frames, fps, loop, front)`, `sprite_spec.FRAME = 128`, `sprite_spec.ROLL_CENTER = (64, 88)`, `sprite_spec.ROLL_RADIUS = 38`, `sprite_spec.roll_spin(walk_speed) -> float`, `sprite_spec.manifest(name, walk_speed, frame_counts) -> dict`.

- [ ] **Step 1: Rust 매니페스트 — 실패 테스트**

`manifest.rs`:
- `REQUIRED_ANIMS`를 `[&str; 16] = ["walk", "run", "roll", "idle", "look", "sleep", "hop", "fall", "dragged", "land", "sit_front", "wave", "tilt", "belly", "stretch", "heart"]`로.
- 테스트 추가:
```rust
    #[test]
    fn front_and_spin_parse_with_defaults() {
        let json = full_json().replace(r#""wave": {"row": 11, "frames": 2, "fps": 4}"#, r#""wave": {"row": 11, "frames": 2, "fps": 4, "front": true}"#)
            .replace(r#""roll": {"row": 2, "frames": 2, "fps": 4}"#, r#""roll": {"row": 2, "frames": 2, "fps": 4, "spin": 271.4}"#);
        let m = parse_manifest(&json).unwrap();
        assert!(m.animations["wave"].front);
        assert!(!m.animations["walk"].front);
        assert_eq!(m.animations["roll"].spin, 271.4);
        assert_eq!(m.animations["walk"].spin, 0.0);
    }
```
- 기존 `loop_false_is_respected` 테스트의 `"hop": {"row": 4 ...}`를 hop의 새 행 6으로: `r#""hop": {"row": 6, "frames": 2, "fps": 4}"#` → `..."loop": false}` 로 갱신.

`brain.rs` `mod tests`에 교차 검증 추가:
```rust
    #[test]
    fn every_anim_name_is_a_required_manifest_animation() {
        let all = [Anim::Walk, Anim::Run, Anim::Roll, Anim::Idle, Anim::Look, Anim::Sleep, Anim::Hop, Anim::Fall, Anim::Dragged,
            Anim::Land, Anim::SitFront, Anim::Wave, Anim::Tilt, Anim::Belly, Anim::Stretch, Anim::Heart];
        let names: Vec<String> = all.iter().map(|a| serde_json::to_string(a).unwrap().trim_matches('"').to_string()).collect();
        assert_eq!(names, crate::manifest::REQUIRED_ANIMS);
    }
```
Run `cargo test -p screenpet-core manifest` → `front_and_spin` FAIL (필드 없음), `bundled_placeholder_manifests_are_valid` FAIL (ui/pets가 8동작).

- [ ] **Step 2: Rust 매니페스트 구현** — `AnimSpec`에:
```rust
    /// 앞모습 동작 — 웹뷰는 좌우 반전하지 않는다
    #[serde(default)]
    pub front: bool,
    /// 초당 회전 각도 (roll 전용, 0이면 회전 없음)
    #[serde(default)]
    pub spin: f64,
```
(번들 테스트는 Step 7 재생성 후 통과)

- [ ] **Step 3: sprite_spec.py 교체**

```python
"""스프라이트 시트 규격 — 앱(src/manifest.rs REQUIRED_ANIMS)과 같은 순서를 유지한다."""

import math

FRAME = 128
# roll 공: 이 점을 축으로 웹뷰가 회전시킨다 (ui/style.css transform-origin 50% 68.75%)
ROLL_CENTER = (64, 88)
ROLL_RADIUS = 38
ROLL_SPEED_MULT = 2.0  # brain behavior Move::Roll.speed_mult()

# (동작, 기본 프레임 수, fps, 반복, 앞모습)
ANIMS = [
    ("walk", 4, 8, True, False),
    ("run", 4, 12, True, False),
    ("roll", 2, 4, True, False),
    ("idle", 2, 2, True, False),
    ("look", 4, 3, True, False),
    ("sleep", 2, 1, True, False),
    ("hop", 3, 10, False, False),
    ("fall", 2, 8, True, False),
    ("dragged", 2, 4, True, False),
    ("land", 2, 10, False, False),
    ("sit_front", 4, 4, True, True),
    ("wave", 4, 6, True, True),
    ("tilt", 3, 3, True, True),
    ("belly", 4, 4, True, True),
    ("stretch", 3, 3, False, True),
    ("heart", 3, 4, True, True),
]


def roll_spin(walk_speed):
    """구르기 속도에서 미끄러짐 없는 회전 속도 (deg/s)."""
    return round(walk_speed * ROLL_SPEED_MULT / ROLL_RADIUS * 180 / math.pi, 1)


def manifest(name, walk_speed, frame_counts):
    """frame_counts: {동작: 프레임 수}. 행 번호는 ANIMS 순서."""
    animations = {}
    for row, (anim, _, fps, loop, front) in enumerate(ANIMS):
        spec = {"row": row, "frames": frame_counts[anim], "fps": fps}
        if not loop:
            spec["loop"] = False
        if front:
            spec["front"] = True
        if anim == "roll":
            spec["spin"] = roll_spin(walk_speed)
        animations[anim] = spec
    return {"name": name, "frame_size": FRAME, "walk_speed": walk_speed, "animations": animations}
```

- [ ] **Step 4: pack_sheet.py / test_pack_sheet.py 갱신**
  - `pack_sheet.py`: `for anim, default_n, _, _ in ANIMS` → `for anim, default_n, *_ in ANIMS`, `for row, (anim, _, _, _) in enumerate(ANIMS)` → `for row, (anim, *_) in enumerate(ANIMS)`. 독스트링의 `64x64` → `128x128`.
  - `test_pack_sheet.py`: 튜플 언패킹을 `for anim, n, *_ in ANIMS` 형태로, 기대값 갱신 — `m["animations"]["walk"] == {"row": 0, "frames": 4, "fps": 8}`, `m["animations"]["hop"]["loop"] is False`, 추가 단언 `m["animations"]["wave"]["front"] is True`, `m["animations"]["roll"]["spin"] == roll_spin(55)` (`from sprite_spec import roll_spin`). 움직임 보존 테스트의 기대 오프셋 계산이 `FRAME`을 쓰는지 확인하고 하드코딩된 64가 있으면 `FRAME`으로.
  - Run `cd tools && python -m pytest -q test_pack_sheet.py` → 통과.

- [ ] **Step 5: draw_pets.py — 엔진 + 종별 설정 + 16포즈**

출발점: 승인된 시안 코드 `/tmp/claude-1000/-home-jde-personal/10c5a648-04e7-4a85-90aa-9a43294dc740/scratchpad/kawaii3.py` (엔진 함수 `melt/grow/aa/paint/stroke/blush/dot_eye/mouth_w`, 종별 앞모습·옆모습). 이것을 복사해 아래 구조로 정리한다. 시안의 모습(`design-preview` 브랜치 `design-preview/kawaii-all-light.png`)이 **기준**이고, 정리 과정에서 모습이 바뀌면 안 된다.

구조 (한 파일, 섹션 주석으로 구분):
1. **엔진**: `K=4` 슈퍼샘플, `Canvas` (S=512), 도형 생성기 `E/T/L`, `melt(mask, r=2.4)`, `grow`, `aa`, `paint(img, mask, color, clip=None)`, `stroke`, 얼굴 부품 `dot_eye(closed=False)`, `blush`, `mouth_w`, 효과 부품 `heart_shape(cx, cy, r)`, `question_mark(cx, cy)`, `zzz(cx, cy)`, 사용자 좌표계 배율 `SC`(종별, 발 기준 (64,126) 축소).
2. **종별 설정** `SPECIES = {id: Species(name, walk_speed, scale, base, colors, ears_front, ears_side, tail_front, tail_side, marks_front, marks_side, face_extra)}` — 시안의 값을 그대로. walk_speed: cat 90, dog 105, rabbit 120, beaver 60. scale: rabbit 0.8, 나머지 0.94.
3. **포즈 뼈대** — 포즈 함수는 `(species, frame_index) -> PIL.Image(128×128 RGBA)`. 같은 뼈대를 종별 설정으로 칠한다:

| 동작 | 프레임 | 뼈대 |
|---|---|---|
| walk | 4 | 옆모습 (시안 side), 다리 교차 4단계, 몸 1.5px 위아래 |
| run | 4 | 옆모습, 몸을 앞으로 기울이고 길게, 앞뒤 다리를 크게 벌림↔모음 (질주), 귀 뒤로 |
| roll | 2 | 공처럼 만 몸: 중심 (64,88) 반지름 ≤ 38 원 안에 머리·몸·꼬리 감김, 눈 감은 웃음(^^), 2프레임은 볼터치만 다름 |
| idle | 2 | 옆모습 앉기 (엉덩이 내리고 앞발 세움), 숨쉬기 1px |
| look | 4 | 옆모습 앉기 + 고개를 위/앞/아래/앞으로 (머리만 이동) |
| sleep | 2 | **확실히 엎드린 자세** (사용자 요청): 옆모습으로 배를 바닥에 깔고 납작하게 엎드림 — 몸 높이는 서 있을 때의 절반 이하(실루엣 위쪽이 y ≥ 70), 앞발을 앞으로 뻗고 그 위에 턱을 올림, 꼬리는 몸에 붙여 바닥에 내림, 눈 감은 곡선(︶), `z` 1~2개 (프레임마다 숨쉬기 1px + z 위치 변화) |
| hop | 3 | 웅크림 → 쭉 뻗음 → 다리 모음 |
| fall | 2 | 팔다리 벌림, 놀란 눈(동그란 흰자), 2프레임 팔다리 흔들림 |
| dragged | 2 | 목덜미 잡혀 대롱대롱: 다리 아래로 늘어짐, 좌우 3px 흔들림 |
| land | 2 | 납작 찌그러짐 (세로 0.85) → 복귀 |
| sit_front | 4 | 시안 front sit, 마지막 프레임 눈 감김(깜빡) |
| wave | 4 | 시안 front wave, 팔 높이 위/중간 반복, 젤리 발바닥 |
| tilt | 3 | 앞모습 앉기 + 머리 12° 기울임 (프레임 0: 8°, 1: 12°, 2: 12°), 머리 옆 `?` |
| belly | 4 | 앞모습으로 누워 배 보이기: 등 대고 누움, 네 발 위로, 발 꼬물(프레임마다 발끝 2px), 꼬리 옆 |
| stretch | 3 | 앞발 앞으로 쭉·엉덩이 들기 (옆이 아니라 정면 쪽으로 엎드린 앞모습) → 하품(입 벌림) → 복귀 앉기 |
| heart | 3 | 앞모습 앉기 + 양발 모아 가슴 앞, 작은 하트 2~3개가 머리 위로 떠오름 (프레임마다 위로 6px, 크기 변화) |

4. **시트 출력** `build(out_root: Path)`: 종마다 `sheet.png` (가로 = 최대 프레임 수 × 128, 세로 = 16 × 128) + `pet.json` (`sprite_spec.manifest`). `main()`은 `ui/pets/`로 출력하고 `tools/preview/`에 4종 × 16동작 전체 미리보기 `all_light.png`(흰 배경), `all_dark.png`(어두운 배경)를 저장 (preview 폴더는 `.gitignore`에 추가).

규칙:
- 모든 프레임은 128×128 안에서 **가장자리 1px 투명** (잘림 없음), 서 있는 포즈는 발이 y 120~127 사이에 닿는다 (`roll`·`fall`·`dragged`·`hop` 제외).
- 옆모습은 오른쪽을 본다.
- 하트·물음표·z는 프레임 안에 그린다.
- 반투명 요소(볼터치 등)는 `Image.alpha_composite`로 얹는다 (직접 그리면 알파 구멍이 생김 — 시안에서 겪은 버그).

- [ ] **Step 6: test_draw_pets.py 작성·실행**

```python
from pathlib import Path

from PIL import Image

import draw_pets
from sprite_spec import ANIMS, FRAME, ROLL_CENTER, ROLL_RADIUS

STANDING = {"walk", "run", "idle", "look", "sleep", "land", "sit_front", "wave", "tilt", "stretch", "heart"}


def test_sleep_is_lying_flat(tmp_path):
    draw_pets.build(tmp_path)
    row = [a for a, *_ in ANIMS].index("sleep")
    for pid in ("cat", "dog", "rabbit", "beaver"):
        cell = Image.open(tmp_path / pid / "sheet.png").crop((0, row * FRAME, FRAME, (row + 1) * FRAME))
        # z 글자는 제외하고 몸만: 불투명한 큰 덩어리의 위쪽이 낮아야 한다
        alpha = cell.getchannel("A").point(lambda v: 255 if v > 200 else 0)
        body_top = min(y for y in range(FRAME) if sum(alpha.getpixel((x, y)) > 0 for x in range(FRAME)) >= 20)
        assert body_top >= 70, f"{pid} sleep body top {body_top} — 엎드린 자세가 아님"


def test_every_species_every_animation(tmp_path):
    draw_pets.build(tmp_path)
    for pid in ("cat", "dog", "rabbit", "beaver"):
        sheet = Image.open(tmp_path / pid / "sheet.png")
        assert sheet.height == FRAME * len(ANIMS)
        for row, (anim, n, *_) in enumerate(ANIMS):
            for col in range(n):
                cell = sheet.crop((col * FRAME, row * FRAME, (col + 1) * FRAME, (row + 1) * FRAME))
                bbox = cell.getchannel("A").getbbox()
                assert bbox is not None, f"{pid} {anim} {col} empty"
                assert bbox[0] >= 1 and bbox[1] >= 1 and bbox[2] <= FRAME - 1 and bbox[3] <= FRAME - 1, f"{pid} {anim} {col} clipped {bbox}"
                if anim in STANDING:
                    assert bbox[3] >= 118, f"{pid} {anim} {col} floats (bottom {bbox[3]})"


def test_roll_ball_fits_rotation_circle(tmp_path):
    draw_pets.build(tmp_path)
    cx, cy = ROLL_CENTER
    row = [a for a, *_ in ANIMS].index("roll")
    for pid in ("cat", "dog", "rabbit", "beaver"):
        cell = Image.open(tmp_path / pid / "sheet.png").crop((0, row * FRAME, FRAME, (row + 1) * FRAME))
        alpha = cell.getchannel("A")
        for y in range(FRAME):
            for x in range(FRAME):
                if alpha.getpixel((x, y)) > 40:
                    assert (x - cx) ** 2 + (y - cy) ** 2 <= (ROLL_RADIUS + 1) ** 2, f"{pid} roll pixel ({x},{y}) outside circle"
```
Run `cd tools && python -m pytest -q` → 전체 통과.

- [ ] **Step 7: 생성·시각 확인**
  - `python tools/draw_pets.py` → `ui/pets/*` 재생성, `tools/preview/all_light.png`, `all_dark.png`.
  - **Read 도구로 미리보기 두 장을 열어 직접 본다.** 확인 항목: 시안과 같은 스타일·무늬, 16동작이 각 동작답게 읽히는지(특히 run·roll·belly·stretch·heart·tilt), 잘림·구멍·빈 칸 없음, 어두운 배경에서도 외곽선이 보임. 이상하면 고치고 다시 생성 (최대 3회 반복, 남은 아쉬움은 보고서에 적는다).
  - `git rm tools/gen_placeholder.py`.
  - `cargo test -p screenpet-core` → 전체 통과 (번들 매니페스트 16동작 검증 포함).

- [ ] **Step 8: docs/sprites.md 갱신** — 규격 128×128, 16동작 표(동작·권장 장수·내용·앞모습 여부), roll 공 원 규칙 (중심 (64,88), 반지름 38), 앞모습 동작은 반전되지 않음, 코드 생성기 `python tools/draw_pets.py`, AI 교체는 `pack_sheet.py` (동작별 PNG → 시트).

- [ ] **Step 9: 커밋** — `feat(art): kawaii 128px sprites, 16 animations for 4 species`

---

### Task 3: 웹뷰 표시 + 크기 배율 + README

**Files:**
- Modify: `ui/main.js`, `ui/style.css`, `src-tauri/src/game_loop.rs`, `README.md`

**Interfaces:**
- Consumes: `pet-render` payload `{anim, flip, heart}` (flip은 앞모습이면 이미 false), 매니페스트 `spin`.
- Produces: 없음 (최종 화면).

- [ ] **Step 1: `ui/style.css`의 `#pet` 규칙 교체, `#pet.flip` 규칙 삭제**

```css
#pet {
  width: 100%;
  height: 100%;
  background-repeat: no-repeat;
  image-rendering: auto;
  /* roll 공 중심 (64,88)/128 — tools/sprite_spec.py ROLL_CENTER */
  transform-origin: 50% 68.75%;
  cursor: grab;
}
```

- [ ] **Step 2: `ui/main.js` 수정**
  - `const DISPLAY = 128;`와 `let scale = 2;` 삭제, `let display = 128;` 추가.
  - `loadPet`:
```js
async function loadPet(id) {
  manifest = await (await fetch(`pets/${id}/pet.json`)).json();
  display = petEl.clientWidth || 128;
  const scale = display / manifest.frame_size;
  const img = new Image();
  img.src = `pets/${id}/sheet.png`;
  await img.decode();
  petEl.style.backgroundImage = `url(pets/${id}/sheet.png)`;
  petEl.style.backgroundSize = `${img.width * scale}px ${img.height * scale}px`;
  animStart = performance.now();
}
```
  - `draw`:
```js
function draw(now) {
  if (manifest) {
    const spec = manifest.animations[render.anim];
    const elapsed = now - animStart;
    const f = frameIndex(spec, elapsed);
    petEl.style.backgroundPosition = `${-f * display}px ${-spec.row * display}px`;
    // 좌우 반전 공간 안에서 돌리므로 왼쪽으로 갈 땐 자동으로 반대로 구른다
    const parts = [];
    if (render.flip) parts.push("scaleX(-1)");
    if (spec.spin) parts.push(`rotate(${((elapsed / 1000) * spec.spin) % 360}deg)`);
    petEl.style.transform = parts.length ? parts.join(" ") : "none";
  }
  requestAnimationFrame(draw);
}
```
  - `pet-render` 리스너에서 `petEl.classList.toggle("flip", render.flip);` 줄 삭제.
  - Run `node --check ui/main.js`.

- [ ] **Step 3: `src-tauri/src/game_loop.rs` 크기 배율**
  - 상수 추가 (PET_LOGICAL_SIZE 아래):
```rust
/// 성장 단계 배율 (teenager = 1.0). 2B에서 단계별 값으로 바뀐다.
const STAGE_SCALE: f64 = 1.0;
```
  - `pet_config`:
```rust
fn pet_config(scale: f64, logical_walk: f64) -> PetConfig {
    let size = PET_LOGICAL_SIZE * STAGE_SCALE * scale;
    PetConfig { size, half_w: size * HALF_WIDTH_RATIO, walk_speed: logical_walk * scale }
}
```
  - `run`에서 `let mut size = PET_LOGICAL_SIZE * scale;` → `let mut size = pet_config(scale, walk_speed).size;`, 그리고 window를 얻은 직후:
```rust
    let logical_side = PET_LOGICAL_SIZE * STAGE_SCALE;
    let _ = window.set_size(tauri::LogicalSize::new(logical_side, logical_side));
```
  (Tauri 2: `WebviewWindow::set_size<S: Into<Size>>(&self, size: S) -> tauri::Result<()>`, `LogicalSize<f64>`는 `Into<Size>`.)

- [ ] **Step 4: README.md 수동 테스트 체크리스트에 추가** (기존 목록 뒤)
```markdown
- [ ] 걷기·뛰기·데구르르 구르기가 섞여 나옴 (대부분 움직이고, 잠은 가끔 짧게)
- [ ] 뛰다가 창 모서리에서 멀리 점프, 구르다가 모서리에서 굴러 떨어짐
- [ ] 구를 때 공처럼 말려 진행 방향으로 회전 (왼쪽으로 갈 땐 반대로)
- [ ] 멈춰 있을 때 가끔 앞모습 애교 (앉아서 바라보기·손 흔들기·고개 갸웃·배 보여주기·기지개·하트)
- [ ] 애교 중에는 왼쪽을 보고 있었어도 얼굴이 정면 (반전 안 됨)
- [ ] 클릭하면 점프+하트 / 하트 뿌뿌 / 손 흔들기 / 갸웃 중 하나
- [ ] 그림 가장자리가 매끈함 (계단 없음)
```
  README의 "임시" 그림 언급이 있으면 "카와이 스타일 그림"으로 바꾸고, 그림 교체 안내를 `docs/sprites.md`로 연결.

- [ ] **Step 5: 확인·커밋**
  - `cargo test` (루트) 통과, `node --check ui/main.js`.
  - Tauri 쪽은 컴파일 불가 — `set_size`/`LogicalSize` 사용을 `~/.cargo/registry/src/*/tauri-2.*/src/webview/webview_window.rs`에서 시그니처 확인.
  - 커밋 `feat(app): render at window size, roll rotation, stage scale hook`

---

## 컨트롤러 후속 (계획 밖, 참고)

- Task 2 후: `tools/preview/all_light.png`·`all_dark.png`를 `design-preview` 브랜치에 올려 사용자에게 링크 전달 (spec §6 시각 확인).
- Task 3 후: `feat/phase2a` 푸시 → GitHub Actions Windows 빌드 → 설치 파일 링크 전달 → 실기 테스트 후 `main` 병합.
