# ScreenPet v1 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Windows 바탕화면에서 동물 한 마리가 작업표시줄·창 윗라인 위를 걷고, 드래그해 던지면 창 위에 착지하는 Tauri 앱을 만들고 GitHub Actions로 설치 파일을 빌드한다.

**Architecture:** Cargo 워크스페이스 두 크레이트. `crates/screenpet-core`는 OS 무관 순수 로직(디딤판 계산, 물리, 상태 머신, 매니페스트, 설정) + `platform` 모듈(Win32 구현은 `cfg(windows)`)이라 리눅스에서 `cargo test` 가능. `src-tauri`는 Tauri 2 앱(60Hz 게임 루프 스레드, 트레이, 커맨드)이며 Windows CI에서만 빌드한다. `ui/`는 번들러 없는 정적 웹뷰(스프라이트 렌더링만).

**Tech Stack:** Rust stable, Tauri 2, `windows` crate 0.58, `fastrand` 2, serde, 바닐라 JS, Python 3 + Pillow(스프라이트 도구), GitHub Actions.

**Spec:** `docs/superpowers/specs/2026-10-28-desktop-pets-design.md`

## Global Constraints

- 첫 버전: 한 번에 한 마리, 트레이에서 고양이/강아지/토끼/비버 선택.
- 펫 창 = 동물 크기 투명 창(128×128 논리 px), 항상 위, 테두리·그림자 없음, 작업표시줄 미표시. 전체화면 오버레이 금지.
- 모든 판단은 Rust. 웹뷰는 `{anim, flip, heart}`를 받아 그리기만 한다.
- 창 목록 조회 약 10Hz, 시뮬레이션 60Hz.
- 스프라이트: 프레임 64×64, 화면 128px 표시, 최근접 보간, 오른쪽 보는 그림, 동작 `walk idle look sleep hop fall dragged land`.
- 수치: Walk 2~8초, Idle 3~6초, Sleep 진입 25%·15~40초, Edge 50% Turn/50% Hop, Blocked/Wall은 항상 Turn(0.3초 멈칫), 클릭 판정 5px 미만, 던지기 속도 창 100ms, 중력 2500 px/s².
- Edge = 창 모서리, Blocked = 앞 창에 가려짐, Wall = 화면 끝/바닥 양 끝.
- 발밑이 덮이거나 창이 사라지면 Fall(놀람). 보이는 조각 폭 < 몸 폭이면 Hop.
- 서 있을 때(Walk/Idle/Turn) 클릭 → Happy(하트 + 제자리 점프). Sleep 클릭 → Idle. 공중 클릭 → 속도 0 낙하.
- 이 리눅스 서버에는 Tauri 리눅스 의존성(webkit2gtk)·sudo가 없다 → `src-tauri`는 여기서 컴파일하지 않는다. Windows CI가 컴파일 검증이다.
- 커밋 메시지 끝에 `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`.
- 원격: `git@github.com:allsilver11/screenpet.git` (SSH, `main`).

## File Structure

```
Cargo.toml                         워크스페이스 (default-members = core)
package.json, package-lock.json    @tauri-apps/cli
crates/screenpet-core/
  Cargo.toml
  src/lib.rs                       모듈 선언
  src/geom.rs                      Rect, WindowInfo
  src/surfaces.rs                  Owner, EndKind, Surface, compute_surfaces
  src/world.rs                     World (bounds, anchor, piece_at, landing)
  src/physics.rs                   Body, step_air, clamp_speed
  src/manifest.rs                  PetManifest, parse_manifest
  src/config.rs                    Config load/save
  src/brain.rs                     Pet 상태 머신
  src/platform/mod.rs              WindowSource trait, NullSource, default_source
  src/platform/win32.rs            Win32Source (cfg(windows))
src-tauri/
  Cargo.toml, build.rs, tauri.conf.json, capabilities/default.json, icons/*
  src/main.rs                      앱 조립, 커맨드, AppState, select_pet
  src/pets.rs                      빌드 시 포함된 pet.json 로드
  src/game_loop.rs                 60Hz 루프 스레드
  src/tray.rs                      트레이 메뉴
ui/
  index.html, style.css, main.js   스프라이트 렌더러
  pets/{cat,dog,rabbit,beaver}/{sheet.png,pet.json}
tools/
  sprite_spec.py                   동작 목록·기본 fps 공용 정의
  gen_placeholder.py               임시 픽셀 동물 생성
  pack_sheet.py                    AI 프레임 → 규격 시트
  make_icon.py                     앱 아이콘 원본 생성
  test_pack_sheet.py
docs/sprites.md                    AI 스프라이트 제작 가이드
.github/workflows/build.yml
README.md                          빌드·설치·수동 테스트 체크리스트
```

---

### Task 1: 툴체인 + 워크스페이스 + geom

**Files:**
- Create: `Cargo.toml`, `.gitignore`, `crates/screenpet-core/Cargo.toml`, `crates/screenpet-core/src/lib.rs`, `crates/screenpet-core/src/geom.rs`

**Interfaces:**
- Produces: `screenpet_core::geom::{Rect, WindowInfo}`; `Rect::new(l,t,r,b)`, `Rect::width()`, `Rect::height()`, `Rect::union(&Rect) -> Rect`.

- [ ] **Step 1: Rust 설치 (사용자 영역, sudo 불필요)**

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --profile minimal
. "$HOME/.cargo/env"
rustup target add x86_64-pc-windows-gnu
cargo --version
```
Expected: `cargo 1.x.x` 출력.

- [ ] **Step 2: 워크스페이스 파일 작성**

`Cargo.toml`:
```toml
[workspace]
resolver = "2"
members = ["crates/screenpet-core"]
default-members = ["crates/screenpet-core"]
```
(`src-tauri`는 Task 10에서 members에 추가한다.)

`.gitignore`:
```
target/
node_modules/
__pycache__/
.pytest_cache/
```

`crates/screenpet-core/Cargo.toml`:
```toml
[package]
name = "screenpet-core"
version = "0.1.0"
edition = "2021"

[dependencies]
fastrand = "2"
serde = { version = "1", features = ["derive"] }
serde_json = "1"
```

`crates/screenpet-core/src/lib.rs`:
```rust
pub mod geom;
```

- [ ] **Step 3: 실패하는 테스트 작성** — `crates/screenpet-core/src/geom.rs`

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn width_and_height() {
        let r = Rect::new(10.0, 20.0, 110.0, 70.0);
        assert_eq!(r.width(), 100.0);
        assert_eq!(r.height(), 50.0);
    }

    #[test]
    fn union_covers_both() {
        let a = Rect::new(0.0, 0.0, 100.0, 100.0);
        let b = Rect::new(50.0, -20.0, 300.0, 80.0);
        assert_eq!(a.union(&b), Rect::new(0.0, -20.0, 300.0, 100.0));
    }
}
```

- [ ] **Step 4: 실패 확인** — `cargo test -p screenpet-core` → FAIL (`Rect` 없음)

- [ ] **Step 5: 구현** — 같은 파일 위쪽에:

```rust
/// 가상 화면 좌표(물리 픽셀)의 사각형. right/bottom은 끝(미포함) 좌표.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rect {
    pub left: f64,
    pub top: f64,
    pub right: f64,
    pub bottom: f64,
}

impl Rect {
    pub fn new(left: f64, top: f64, right: f64, bottom: f64) -> Self {
        Self { left, top, right, bottom }
    }

    pub fn width(&self) -> f64 {
        self.right - self.left
    }

    pub fn height(&self) -> f64 {
        self.bottom - self.top
    }

    pub fn union(&self, o: &Rect) -> Rect {
        Rect::new(
            self.left.min(o.left),
            self.top.min(o.top),
            self.right.max(o.right),
            self.bottom.max(o.bottom),
        )
    }
}

/// 다른 앱의 최상위 창 하나.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WindowInfo {
    pub id: u64,
    pub rect: Rect,
}
```

- [ ] **Step 6: 통과 확인** — `cargo test -p screenpet-core` → 2 passed

- [ ] **Step 7: 커밋**
```bash
git add -A && git commit -m "feat(core): workspace scaffold and geometry types"
```

---

### Task 2: 디딤판 계산 (surfaces)

**Files:**
- Create: `crates/screenpet-core/src/surfaces.rs`
- Modify: `crates/screenpet-core/src/lib.rs` (`pub mod surfaces;`)

**Interfaces:**
- Consumes: `geom::{Rect, WindowInfo}`
- Produces:
```rust
pub enum Owner { Window(u64), Ground(usize) }          // Copy, Eq, Hash
pub enum EndKind { Edge, Blocked, Wall }               // Copy, Eq
pub struct Surface { pub owner: Owner, pub y: f64, pub x_start: f64, pub x_end: f64,
                     pub start_kind: EndKind, pub end_kind: EndKind }
impl Surface { pub fn width(&self) -> f64; pub fn contains_x(&self, x: f64) -> bool }
/// windows: 앞(Z-order 위)→뒤 순서. work_areas: 모니터별 작업 영역.
/// min_y보다 위에 있는 창 윗라인은 제외(펫이 화면 밖으로 나가지 않게).
pub fn compute_surfaces(windows: &[WindowInfo], work_areas: &[Rect], min_y: f64) -> Vec<Surface>
```

- [ ] **Step 1: 실패하는 테스트 작성** — `surfaces.rs` 하단

```rust
#[cfg(test)]
mod tests {
    use super::*;

    fn win(id: u64, l: f64, t: f64, r: f64, b: f64) -> WindowInfo {
        WindowInfo { id, rect: Rect::new(l, t, r, b) }
    }

    fn screen() -> Vec<Rect> {
        vec![Rect::new(0.0, 0.0, 1920.0, 1040.0)]
    }

    fn pieces_of(s: &[Surface], owner: Owner) -> Vec<Surface> {
        s.iter().copied().filter(|p| p.owner == owner).collect()
    }

    #[test]
    fn ground_per_work_area_with_walls() {
        let s = compute_surfaces(&[], &screen(), 0.0);
        assert_eq!(s.len(), 1);
        let g = s[0];
        assert_eq!(g.owner, Owner::Ground(0));
        assert_eq!((g.y, g.x_start, g.x_end), (1040.0, 0.0, 1920.0));
        assert_eq!((g.start_kind, g.end_kind), (EndKind::Wall, EndKind::Wall));
    }

    #[test]
    fn single_window_top_has_edges() {
        let s = compute_surfaces(&[win(1, 500.0, 400.0, 1100.0, 900.0)], &screen(), 0.0);
        let p = pieces_of(&s, Owner::Window(1));
        assert_eq!(p.len(), 1);
        assert_eq!((p[0].y, p[0].x_start, p[0].x_end), (400.0, 500.0, 1100.0));
        assert_eq!((p[0].start_kind, p[0].end_kind), (EndKind::Edge, EndKind::Edge));
    }

    #[test]
    fn front_window_splits_back_window_top() {
        let ws = [win(2, 700.0, 300.0, 900.0, 600.0), win(1, 500.0, 400.0, 1100.0, 900.0)];
        let p = pieces_of(&compute_surfaces(&ws, &screen(), 0.0), Owner::Window(1));
        assert_eq!(p.len(), 2);
        assert_eq!((p[0].x_start, p[0].x_end), (500.0, 700.0));
        assert_eq!((p[0].start_kind, p[0].end_kind), (EndKind::Edge, EndKind::Blocked));
        assert_eq!((p[1].x_start, p[1].x_end), (900.0, 1100.0));
        assert_eq!((p[1].start_kind, p[1].end_kind), (EndKind::Blocked, EndKind::Edge));
    }

    #[test]
    fn front_window_covering_left_end() {
        let ws = [win(2, 400.0, 300.0, 600.0, 600.0), win(1, 500.0, 400.0, 1100.0, 900.0)];
        let p = pieces_of(&compute_surfaces(&ws, &screen(), 0.0), Owner::Window(1));
        assert_eq!(p.len(), 1);
        assert_eq!((p[0].x_start, p[0].x_end), (600.0, 1100.0));
        assert_eq!((p[0].start_kind, p[0].end_kind), (EndKind::Blocked, EndKind::Edge));
    }

    #[test]
    fn front_window_below_the_top_line_does_not_cover() {
        let ws = [win(2, 700.0, 450.0, 900.0, 600.0), win(1, 500.0, 400.0, 1100.0, 900.0)];
        let p = pieces_of(&compute_surfaces(&ws, &screen(), 0.0), Owner::Window(1));
        assert_eq!(p.len(), 1);
    }

    #[test]
    fn fully_covered_window_has_no_pieces() {
        let ws = [win(2, 0.0, 0.0, 1920.0, 1040.0), win(1, 500.0, 400.0, 1100.0, 900.0)];
        let p = pieces_of(&compute_surfaces(&ws, &screen(), 0.0), Owner::Window(1));
        assert!(p.is_empty());
    }

    #[test]
    fn back_window_does_not_cover_front_window() {
        let ws = [win(1, 500.0, 400.0, 1100.0, 900.0), win(2, 700.0, 300.0, 900.0, 600.0)];
        let p = pieces_of(&compute_surfaces(&ws, &screen(), 0.0), Owner::Window(1));
        assert_eq!(p.len(), 1);
        assert_eq!((p[0].x_start, p[0].x_end), (500.0, 1100.0));
    }

    #[test]
    fn tops_above_min_y_or_below_screen_are_ignored() {
        let ws = [win(1, 500.0, 50.0, 1100.0, 900.0), win(2, 500.0, 1040.0, 1100.0, 1200.0)];
        let s = compute_surfaces(&ws, &screen(), 128.0);
        assert!(pieces_of(&s, Owner::Window(1)).is_empty());
        assert!(pieces_of(&s, Owner::Window(2)).is_empty());
    }

    #[test]
    fn offscreen_part_is_clipped_as_wall() {
        let ws = [win(1, -300.0, 400.0, 600.0, 900.0)];
        let p = pieces_of(&compute_surfaces(&ws, &screen(), 0.0), Owner::Window(1));
        assert_eq!((p[0].x_start, p[0].x_end), (0.0, 600.0));
        assert_eq!((p[0].start_kind, p[0].end_kind), (EndKind::Wall, EndKind::Edge));
    }
}
```

- [ ] **Step 2: 실패 확인** — `cargo test -p screenpet-core surfaces` → FAIL (컴파일 에러)

- [ ] **Step 3: 구현** — `surfaces.rs` 위쪽:

```rust
use crate::geom::{Rect, WindowInfo};

/// 디딤판이 누구 것인지. Ground의 값은 work_areas 인덱스.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Owner {
    Window(u64),
    Ground(usize),
}

/// 디딤판 조각 끝이 왜 끝났는지.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EndKind {
    /// 창 모서리
    Edge,
    /// 앞쪽 창에 가려짐
    Blocked,
    /// 화면 끝
    Wall,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Surface {
    pub owner: Owner,
    pub y: f64,
    pub x_start: f64,
    pub x_end: f64,
    pub start_kind: EndKind,
    pub end_kind: EndKind,
}

impl Surface {
    pub fn width(&self) -> f64 {
        self.x_end - self.x_start
    }

    pub fn contains_x(&self, x: f64) -> bool {
        x >= self.x_start && x <= self.x_end
    }
}

pub fn compute_surfaces(windows: &[WindowInfo], work_areas: &[Rect], min_y: f64) -> Vec<Surface> {
    let mut out = Vec::new();
    let Some(bounds) = work_areas.iter().copied().reduce(|a, b| a.union(&b)) else {
        return out;
    };

    for (i, w) in windows.iter().enumerate() {
        let y = w.rect.top;
        if y < min_y || y >= bounds.bottom {
            continue;
        }
        // (x_start, x_end, start_kind, end_kind)
        let mut segs = vec![clip(w.rect.left, w.rect.right, &bounds)];
        for front in &windows[..i] {
            let r = front.rect;
            if !(r.top <= y && r.bottom > y) {
                continue;
            }
            segs = segs
                .into_iter()
                .flat_map(|(a, b, ka, kb)| {
                    let mut kept = Vec::new();
                    if r.right <= a || r.left >= b {
                        kept.push((a, b, ka, kb));
                    } else {
                        if r.left > a {
                            kept.push((a, r.left, ka, EndKind::Blocked));
                        }
                        if r.right < b {
                            kept.push((r.right, b, EndKind::Blocked, kb));
                        }
                    }
                    kept
                })
                .collect();
        }
        for (a, b, ka, kb) in segs {
            if b > a {
                out.push(Surface { owner: Owner::Window(w.id), y, x_start: a, x_end: b, start_kind: ka, end_kind: kb });
            }
        }
    }

    for (i, wa) in work_areas.iter().enumerate() {
        out.push(Surface {
            owner: Owner::Ground(i),
            y: wa.bottom,
            x_start: wa.left,
            x_end: wa.right,
            start_kind: EndKind::Wall,
            end_kind: EndKind::Wall,
        });
    }
    out
}

fn clip(left: f64, right: f64, bounds: &Rect) -> (f64, f64, EndKind, EndKind) {
    let (a, ka) = if left < bounds.left { (bounds.left, EndKind::Wall) } else { (left, EndKind::Edge) };
    let (b, kb) = if right > bounds.right { (bounds.right, EndKind::Wall) } else { (right, EndKind::Edge) };
    (a, b, ka, kb)
}
```

- [ ] **Step 4: 통과 확인** — `cargo test -p screenpet-core surfaces` → 9 passed

- [ ] **Step 5: 커밋** — `git add -A && git commit -m "feat(core): compute walkable surfaces from window z-order"`

---

### Task 3: World

**Files:**
- Create: `crates/screenpet-core/src/world.rs`
- Modify: `lib.rs` (`pub mod world;`)

**Interfaces:**
- Consumes: `geom`, `surfaces::{compute_surfaces, Owner, Surface}`
- Produces:
```rust
pub struct World { pub windows: Vec<WindowInfo>, pub work_areas: Vec<Rect>, pub bounds: Rect, pub surfaces: Vec<Surface> }
impl World {
    /// work_areas가 비어 있으면 1920x1080 하나로 대체. min_y = bounds.top + pet_height.
    pub fn new(windows: Vec<WindowInfo>, work_areas: Vec<Rect>, pet_height: f64) -> Self;
    /// 디딤판 주인의 기준점: 창이면 (left, top), 바닥이면 (work_area.left, work_area.bottom).
    pub fn anchor(&self, owner: Owner) -> Option<(f64, f64)>;
    pub fn piece_at(&self, owner: Owner, x: f64) -> Option<&Surface>;
    /// 낙하 중 prev_y→new_y 사이에 통과한 디딤판 중 가장 위의 것.
    pub fn landing(&self, x: f64, prev_y: f64, new_y: f64) -> Option<&Surface>;
}
```

- [ ] **Step 1: 실패하는 테스트**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    fn world() -> World {
        let w1 = WindowInfo { id: 1, rect: Rect::new(500.0, 400.0, 1100.0, 900.0) };
        let w2 = WindowInfo { id: 2, rect: Rect::new(600.0, 700.0, 1000.0, 1000.0) };
        World::new(vec![w1, w2], vec![Rect::new(0.0, 0.0, 1920.0, 1040.0)], 128.0)
    }

    #[test]
    fn empty_work_areas_fall_back_to_default_screen() {
        let w = World::new(vec![], vec![], 128.0);
        assert_eq!(w.bounds, Rect::new(0.0, 0.0, 1920.0, 1080.0));
    }

    #[test]
    fn anchors() {
        let w = world();
        assert_eq!(w.anchor(Owner::Window(1)), Some((500.0, 400.0)));
        assert_eq!(w.anchor(Owner::Ground(0)), Some((0.0, 1040.0)));
        assert_eq!(w.anchor(Owner::Window(99)), None);
    }

    #[test]
    fn piece_at_finds_piece_under_x() {
        let w = world();
        assert!(w.piece_at(Owner::Window(1), 800.0).is_some());
        assert!(w.piece_at(Owner::Window(1), 1200.0).is_none());
    }

    #[test]
    fn landing_picks_first_crossed_surface() {
        let w = world();
        let s = w.landing(800.0, 300.0, 1100.0).unwrap();
        assert_eq!(s.owner, Owner::Window(1));
    }

    #[test]
    fn landing_ignores_surfaces_when_rising_or_not_crossed() {
        let w = world();
        assert!(w.landing(800.0, 500.0, 300.0).is_none());
        assert!(w.landing(800.0, 300.0, 390.0).is_none());
    }

    #[test]
    fn landing_requires_x_inside() {
        let w = world();
        let s = w.landing(200.0, 300.0, 1100.0).unwrap();
        assert_eq!(s.owner, Owner::Ground(0));
    }
}
```

- [ ] **Step 2: 실패 확인** — `cargo test -p screenpet-core world` → FAIL

- [ ] **Step 3: 구현**

```rust
use crate::geom::{Rect, WindowInfo};
use crate::surfaces::{compute_surfaces, Owner, Surface};

pub struct World {
    pub windows: Vec<WindowInfo>,
    pub work_areas: Vec<Rect>,
    pub bounds: Rect,
    pub surfaces: Vec<Surface>,
}

impl World {
    pub fn new(windows: Vec<WindowInfo>, mut work_areas: Vec<Rect>, pet_height: f64) -> Self {
        if work_areas.is_empty() {
            work_areas.push(Rect::new(0.0, 0.0, 1920.0, 1080.0));
        }
        let bounds = work_areas.iter().copied().reduce(|a, b| a.union(&b)).unwrap();
        let surfaces = compute_surfaces(&windows, &work_areas, bounds.top + pet_height);
        Self { windows, work_areas, bounds, surfaces }
    }

    pub fn anchor(&self, owner: Owner) -> Option<(f64, f64)> {
        match owner {
            Owner::Window(id) => self.windows.iter().find(|w| w.id == id).map(|w| (w.rect.left, w.rect.top)),
            Owner::Ground(i) => self.work_areas.get(i).map(|r| (r.left, r.bottom)),
        }
    }

    pub fn piece_at(&self, owner: Owner, x: f64) -> Option<&Surface> {
        self.surfaces.iter().find(|s| s.owner == owner && s.contains_x(x))
    }

    pub fn landing(&self, x: f64, prev_y: f64, new_y: f64) -> Option<&Surface> {
        if new_y <= prev_y {
            return None;
        }
        self.surfaces
            .iter()
            .filter(|s| s.contains_x(x) && prev_y <= s.y && new_y >= s.y)
            .min_by(|a, b| a.y.total_cmp(&b.y))
    }
}
```

- [ ] **Step 4: 통과 확인** — `cargo test -p screenpet-core world` → 6 passed
- [ ] **Step 5: 커밋** — `git commit -am "feat(core): world with anchors and landing detection"` (새 파일이므로 `git add -A` 먼저)

---

### Task 4: 공중 물리

**Files:**
- Create: `crates/screenpet-core/src/physics.rs`
- Modify: `lib.rs` (`pub mod physics;`)

**Interfaces:**
- Produces:
```rust
pub const GRAVITY: f64 = 2500.0;
pub const WALL_DAMPING: f64 = 0.6;
pub const MAX_THROW_SPEED: f64 = 3000.0;
pub struct Body { pub x: f64, pub y: f64, pub vx: f64, pub vy: f64 }   // x = 발 중심, y = 발바닥
/// 중력 적용 후 이동. 좌우 화면 끝에서 튕기고(감쇠), 천장에 머리가 닿으면 멈춘다.
pub fn step_air(b: &mut Body, dt: f64, bounds: &Rect, half_w: f64, height: f64);
pub fn clamp_speed(vx: f64, vy: f64) -> (f64, f64);
```

- [ ] **Step 1: 실패하는 테스트**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    const B: Rect = Rect { left: 0.0, top: 0.0, right: 1000.0, bottom: 1000.0 };

    #[test]
    fn gravity_accelerates_downward() {
        let mut b = Body { x: 500.0, y: 500.0, vx: 0.0, vy: 0.0 };
        step_air(&mut b, 0.1, &B, 38.0, 128.0);
        assert!((b.vy - 250.0).abs() < 1e-9);
        assert!(b.y > 500.0);
    }

    #[test]
    fn bounces_off_left_wall_with_damping() {
        let mut b = Body { x: 40.0, y: 500.0, vx: -1000.0, vy: 0.0 };
        step_air(&mut b, 0.1, &B, 38.0, 128.0);
        assert_eq!(b.x, 38.0);
        assert!((b.vx - 600.0).abs() < 1e-9);
    }

    #[test]
    fn bounces_off_right_wall_with_damping() {
        let mut b = Body { x: 960.0, y: 500.0, vx: 1000.0, vy: 0.0 };
        step_air(&mut b, 0.1, &B, 38.0, 128.0);
        assert_eq!(b.x, 962.0);
        assert!((b.vx + 600.0).abs() < 1e-9);
    }

    #[test]
    fn ceiling_stops_upward_motion() {
        let mut b = Body { x: 500.0, y: 140.0, vx: 0.0, vy: -2000.0 };
        step_air(&mut b, 0.1, &B, 38.0, 128.0);
        assert_eq!(b.y, 128.0);
        assert_eq!(b.vy, 0.0);
    }

    #[test]
    fn clamp_speed_limits_magnitude() {
        let (vx, vy) = clamp_speed(6000.0, 8000.0);
        assert!(((vx * vx + vy * vy).sqrt() - MAX_THROW_SPEED).abs() < 1e-6);
        assert_eq!(clamp_speed(100.0, -50.0), (100.0, -50.0));
    }
}
```

- [ ] **Step 2: 실패 확인** — `cargo test -p screenpet-core physics` → FAIL

- [ ] **Step 3: 구현**

```rust
use crate::geom::Rect;

pub const GRAVITY: f64 = 2500.0;
pub const WALL_DAMPING: f64 = 0.6;
pub const MAX_THROW_SPEED: f64 = 3000.0;

/// x = 발 중심, y = 발바닥 (화면 아래로 +).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Body {
    pub x: f64,
    pub y: f64,
    pub vx: f64,
    pub vy: f64,
}

pub fn step_air(b: &mut Body, dt: f64, bounds: &Rect, half_w: f64, height: f64) {
    b.vy += GRAVITY * dt;
    b.x += b.vx * dt;
    b.y += b.vy * dt;

    if b.x - half_w < bounds.left {
        b.x = bounds.left + half_w;
        b.vx = b.vx.abs() * WALL_DAMPING;
    } else if b.x + half_w > bounds.right {
        b.x = bounds.right - half_w;
        b.vx = -b.vx.abs() * WALL_DAMPING;
    }
    if b.y - height < bounds.top {
        b.y = bounds.top + height;
        b.vy = b.vy.max(0.0);
    }
}

pub fn clamp_speed(vx: f64, vy: f64) -> (f64, f64) {
    let speed = (vx * vx + vy * vy).sqrt();
    if speed <= MAX_THROW_SPEED {
        (vx, vy)
    } else {
        let k = MAX_THROW_SPEED / speed;
        (vx * k, vy * k)
    }
}
```

- [ ] **Step 4: 통과 확인** — `cargo test -p screenpet-core physics` → 5 passed
- [ ] **Step 5: 커밋** — `git add -A && git commit -m "feat(core): airborne physics with wall bounce"`

---

### Task 5: 매니페스트 + 설정

**Files:**
- Create: `crates/screenpet-core/src/manifest.rs`, `crates/screenpet-core/src/config.rs`
- Modify: `lib.rs` (`pub mod manifest; pub mod config;`)

**Interfaces:**
- Produces:
```rust
// manifest.rs
pub const REQUIRED_ANIMS: [&str; 8] = ["walk","idle","look","sleep","hop","fall","dragged","land"];
#[derive(Clone, Deserialize)] pub struct AnimSpec { pub row: u32, pub frames: u32, pub fps: f64, #[serde(rename="loop", default="yes")] pub looping: bool }
#[derive(Clone, Deserialize)] pub struct PetManifest { pub name: String, pub frame_size: u32, pub walk_speed: f64, pub animations: HashMap<String, AnimSpec> }
pub fn parse_manifest(json: &str) -> Result<PetManifest, String>;
// config.rs
#[derive(Default, Serialize, Deserialize, PartialEq, Debug)] pub struct Config { pub pet: Option<String> }
pub fn load(dir: &Path) -> Config;          // 파일 없음/깨짐 → Default
pub fn save(dir: &Path, c: &Config) -> std::io::Result<()>;   // dir 생성 포함, 파일명 config.json
```

- [ ] **Step 1: 실패하는 테스트** — `manifest.rs`

```rust
#[cfg(test)]
mod tests {
    use super::*;

    fn full_json() -> String {
        let anims: Vec<String> = REQUIRED_ANIMS
            .iter()
            .enumerate()
            .map(|(i, a)| format!(r#""{a}": {{"row": {i}, "frames": 2, "fps": 4}}"#))
            .collect();
        format!(r#"{{"name": "고양이", "frame_size": 64, "walk_speed": 60, "animations": {{{}}}}}"#, anims.join(","))
    }

    #[test]
    fn parses_valid_manifest_with_loop_default_true() {
        let m = parse_manifest(&full_json()).unwrap();
        assert_eq!(m.name, "고양이");
        assert!(m.animations["walk"].looping);
    }

    #[test]
    fn loop_false_is_respected() {
        let json = full_json().replace(r#""hop": {"row": 4, "frames": 2, "fps": 4}"#, r#""hop": {"row": 4, "frames": 2, "fps": 4, "loop": false}"#);
        assert!(!parse_manifest(&json).unwrap().animations["hop"].looping);
    }

    #[test]
    fn rejects_missing_animation() {
        let json = full_json().replace(r#""sleep""#, r#""nap""#);
        assert!(parse_manifest(&json).unwrap_err().contains("sleep"));
    }

    #[test]
    fn rejects_zero_frames_or_bad_speed() {
        assert!(parse_manifest(&full_json().replace(r#""frames": 2"#, r#""frames": 0"#)).is_err());
        assert!(parse_manifest(&full_json().replace(r#""walk_speed": 60"#, r#""walk_speed": 0"#)).is_err());
    }

    #[test]
    fn rejects_invalid_json() {
        assert!(parse_manifest("{").is_err());
    }
}
```

`config.rs` 테스트:
```rust
#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(name: &str) -> std::path::PathBuf {
        let d = std::env::temp_dir().join(format!("screenpet-test-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        d
    }

    #[test]
    fn missing_file_gives_default() {
        assert_eq!(load(&temp_dir("missing")), Config::default());
    }

    #[test]
    fn save_then_load_roundtrip() {
        let d = temp_dir("roundtrip");
        let c = Config { pet: Some("rabbit".into()) };
        save(&d, &c).unwrap();
        assert_eq!(load(&d), c);
    }

    #[test]
    fn corrupt_file_gives_default() {
        let d = temp_dir("corrupt");
        std::fs::create_dir_all(&d).unwrap();
        std::fs::write(d.join("config.json"), "not json").unwrap();
        assert_eq!(load(&d), Config::default());
    }
}
```

- [ ] **Step 2: 실패 확인** — `cargo test -p screenpet-core manifest config` (두 번 실행: `... manifest`, `... config`) → FAIL

- [ ] **Step 3: 구현** — `manifest.rs`

```rust
use std::collections::HashMap;

use serde::Deserialize;

pub const REQUIRED_ANIMS: [&str; 8] = ["walk", "idle", "look", "sleep", "hop", "fall", "dragged", "land"];

#[derive(Debug, Clone, Deserialize)]
pub struct AnimSpec {
    pub row: u32,
    pub frames: u32,
    pub fps: f64,
    #[serde(rename = "loop", default = "yes")]
    pub looping: bool,
}

fn yes() -> bool {
    true
}

#[derive(Debug, Clone, Deserialize)]
pub struct PetManifest {
    pub name: String,
    pub frame_size: u32,
    pub walk_speed: f64,
    pub animations: HashMap<String, AnimSpec>,
}

pub fn parse_manifest(json: &str) -> Result<PetManifest, String> {
    let m: PetManifest = serde_json::from_str(json).map_err(|e| e.to_string())?;
    if m.frame_size == 0 {
        return Err("frame_size must be > 0".into());
    }
    if m.walk_speed <= 0.0 {
        return Err("walk_speed must be > 0".into());
    }
    for name in REQUIRED_ANIMS {
        let spec = m.animations.get(name).ok_or_else(|| format!("missing animation: {name}"))?;
        if spec.frames == 0 || spec.fps <= 0.0 {
            return Err(format!("animation {name}: frames and fps must be > 0"));
        }
    }
    Ok(m)
}
```

`config.rs`:
```rust
use std::path::Path;

use serde::{Deserialize, Serialize};

const FILE: &str = "config.json";

#[derive(Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Config {
    /// 마지막으로 고른 동물 폴더 이름
    pub pet: Option<String>,
}

pub fn load(dir: &Path) -> Config {
    std::fs::read_to_string(dir.join(FILE))
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

pub fn save(dir: &Path, c: &Config) -> std::io::Result<()> {
    std::fs::create_dir_all(dir)?;
    std::fs::write(dir.join(FILE), serde_json::to_string_pretty(c)?)
}
```

- [ ] **Step 4: 통과 확인** — `cargo test -p screenpet-core` → 전체 통과 (manifest 5 + config 3 추가)
- [ ] **Step 5: 커밋** — `git add -A && git commit -m "feat(core): pet manifest parsing and config persistence"`

---

### Task 6: 펫 상태 머신 — 땅 위 행동

**Files:**
- Create: `crates/screenpet-core/src/brain.rs`
- Modify: `lib.rs` (`pub mod brain;`)

**Interfaces:**
- Consumes: `World`, `Owner`, `EndKind`, `Body`, `step_air`, `clamp_speed`
- Produces (Task 7, 10이 사용):
```rust
#[derive(Serialize)] #[serde(rename_all="lowercase")]
pub enum Anim { Walk, Idle, Look, Sleep, Hop, Fall, Dragged, Land }
pub enum State { Walk{remaining:f64}, Idle{remaining:f64, look:bool}, Sleep{remaining:f64}, Turn{remaining:f64},
                 Land{remaining:f64}, Air{anim:Anim}, Pressed{start_cx:f64,start_cy:f64,grounded:bool,was_sleeping:bool}, Dragged }
pub struct Standing { pub owner: Owner, pub rel_x: f64 }
pub struct PetConfig { pub size: f64, pub half_w: f64, pub walk_speed: f64 }
#[derive(Serialize)] pub struct RenderState { pub anim: Anim, pub flip: bool, pub heart: bool }
pub struct Pet { pub cfg, pub body: Body, pub state: State, pub standing: Option<Standing>, pub facing: f64, .. }
impl Pet {
  pub fn new(cfg: PetConfig, x: f64, y: f64, seed: u64) -> Self;   // Air{Fall}로 시작
  pub fn update(&mut self, dt: f64, world: &World);
  pub fn render(&self) -> RenderState;
  pub fn window_pos(&self) -> (f64, f64);                            // (x - size/2, y - size)
  // Task 7: pointer_down / pointer_move / pointer_up / is_held
}
```

- [ ] **Step 1: 실패하는 테스트** — `brain.rs` 하단 (Task 7에서 같은 모듈에 테스트 추가)

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::geom::{Rect, WindowInfo};

    const DT: f64 = 1.0 / 60.0;

    pub(super) fn cfg() -> PetConfig {
        PetConfig { size: 128.0, half_w: 38.0, walk_speed: 60.0 }
    }

    pub(super) fn win(id: u64, l: f64, t: f64, r: f64, b: f64) -> WindowInfo {
        WindowInfo { id, rect: Rect::new(l, t, r, b) }
    }

    /// windows는 앞→뒤 순서
    pub(super) fn world(windows: Vec<WindowInfo>) -> World {
        World::new(windows, vec![Rect::new(0.0, 0.0, 1920.0, 1040.0)], 128.0)
    }

    pub(super) fn w1() -> WindowInfo {
        win(1, 500.0, 400.0, 1100.0, 900.0)
    }

    pub(super) fn place(p: &mut Pet, w: &World, owner: Owner, x: f64, state: State) {
        let (ax, ay) = w.anchor(owner).unwrap();
        p.standing = Some(Standing { owner, rel_x: x - ax });
        p.body = Body { x, y: ay, vx: 0.0, vy: 0.0 };
        p.state = state;
    }

    pub(super) fn run(p: &mut Pet, w: &World, ticks: usize) {
        for _ in 0..ticks {
            p.update(DT, w);
        }
    }

    #[test]
    fn falls_and_lands_on_window_top() {
        let w = world(vec![w1()]);
        let mut p = Pet::new(cfg(), 800.0, 200.0, 1);
        run(&mut p, &w, 120);
        assert_eq!(p.standing.unwrap().owner, Owner::Window(1));
        assert_eq!(p.body.y, 400.0);
    }

    #[test]
    fn falls_to_ground_when_nothing_below() {
        let w = world(vec![w1()]);
        let mut p = Pet::new(cfg(), 200.0, 200.0, 1);
        run(&mut p, &w, 120);
        assert_eq!(p.standing.unwrap().owner, Owner::Ground(0));
        assert_eq!(p.body.y, 1040.0);
    }

    #[test]
    fn landing_shows_land_then_idle() {
        let w = world(vec![w1()]);
        let mut p = Pet::new(cfg(), 800.0, 390.0, 1);
        run(&mut p, &w, 10);
        assert!(matches!(p.state, State::Land { .. }));
        run(&mut p, &w, 30);
        assert!(matches!(p.state, State::Idle { .. }));
    }

    #[test]
    fn rides_window_when_it_moves() {
        let w = world(vec![w1()]);
        let mut p = Pet::new(cfg(), 0.0, 0.0, 1);
        place(&mut p, &w, Owner::Window(1), 800.0, State::Idle { remaining: 100.0, look: false });
        let moved = world(vec![win(1, 600.0, 450.0, 1200.0, 950.0)]);
        p.update(DT, &moved);
        assert_eq!((p.body.x, p.body.y), (900.0, 450.0));
    }

    #[test]
    fn falls_when_window_disappears() {
        let w = world(vec![w1()]);
        let mut p = Pet::new(cfg(), 0.0, 0.0, 1);
        place(&mut p, &w, Owner::Window(1), 800.0, State::Idle { remaining: 100.0, look: false });
        p.update(DT, &world(vec![]));
        assert_eq!(p.state, State::Air { anim: Anim::Fall });
        assert!(p.standing.is_none());
    }

    #[test]
    fn falls_when_covered_by_front_window() {
        let w = world(vec![w1()]);
        let mut p = Pet::new(cfg(), 0.0, 0.0, 1);
        place(&mut p, &w, Owner::Window(1), 800.0, State::Idle { remaining: 100.0, look: false });
        let covered = world(vec![win(2, 700.0, 300.0, 900.0, 600.0), w1()]);
        p.update(DT, &covered);
        assert_eq!(p.state, State::Air { anim: Anim::Fall });
    }

    #[test]
    fn blocked_end_turns_around() {
        let w = world(vec![win(2, 900.0, 300.0, 1300.0, 600.0), w1()]);
        let mut p = Pet::new(cfg(), 0.0, 0.0, 1);
        place(&mut p, &w, Owner::Window(1), 840.0, State::Walk { remaining: 100.0 });
        p.facing = 1.0;
        let mut turned = false;
        for _ in 0..60 {
            p.update(DT, &w);
            assert!(!matches!(p.state, State::Air { .. }));
            if matches!(p.state, State::Turn { .. }) {
                turned = true;
                break;
            }
        }
        assert!(turned);
        run(&mut p, &w, 20);
        assert_eq!(p.facing, -1.0);
        assert!(matches!(p.state, State::Walk { .. }));
    }

    #[test]
    fn wall_end_always_turns() {
        let w = world(vec![]);
        for seed in 0..50 {
            let mut p = Pet::new(cfg(), 0.0, 0.0, seed);
            place(&mut p, &w, Owner::Ground(0), 1920.0 - 38.0 - 0.5, State::Walk { remaining: 100.0 });
            p.facing = 1.0;
            p.update(DT, &w);
            assert!(matches!(p.state, State::Turn { .. }), "seed {seed}");
        }
    }

    #[test]
    fn window_edge_turns_or_hops_about_half_each() {
        let w = world(vec![w1()]);
        let (mut hops, mut turns) = (0, 0);
        for seed in 0..400 {
            let mut p = Pet::new(cfg(), 0.0, 0.0, seed);
            place(&mut p, &w, Owner::Window(1), 1100.0 - 38.0 - 0.5, State::Walk { remaining: 100.0 });
            p.facing = 1.0;
            p.update(DT, &w);
            match p.state {
                State::Air { anim: Anim::Hop } => hops += 1,
                State::Turn { .. } => turns += 1,
                s => panic!("unexpected {s:?}"),
            }
        }
        assert!((120..=280).contains(&hops), "hops={hops}");
        assert!((120..=280).contains(&turns), "turns={turns}");
    }

    #[test]
    fn hop_off_edge_leaves_the_window() {
        let w = world(vec![w1()]);
        let mut p = Pet::new(cfg(), 0.0, 0.0, 1);
        place(&mut p, &w, Owner::Window(1), 1061.0, State::Walk { remaining: 100.0 });
        p.facing = 1.0;
        p.start_hop(1.0);
        run(&mut p, &w, 120);
        assert_eq!(p.standing.unwrap().owner, Owner::Ground(0));
    }

    #[test]
    fn narrow_piece_makes_pet_hop_down() {
        let w = world(vec![win(2, 550.0, 300.0, 1300.0, 600.0), w1()]);
        let mut p = Pet::new(cfg(), 0.0, 0.0, 1);
        place(&mut p, &w, Owner::Window(1), 525.0, State::Idle { remaining: 100.0, look: false });
        p.update(DT, &w);
        assert_eq!(p.state, State::Air { anim: Anim::Hop });
    }

    #[test]
    fn expired_walk_becomes_idle_or_turn() {
        let w = world(vec![]);
        let mut p = Pet::new(cfg(), 0.0, 0.0, 3);
        place(&mut p, &w, Owner::Ground(0), 900.0, State::Walk { remaining: 0.001 });
        p.update(DT, &w);
        assert!(matches!(p.state, State::Idle { .. } | State::Turn { .. }));
    }

    #[test]
    fn expired_idle_becomes_sleep_or_walk() {
        let w = world(vec![]);
        let mut p = Pet::new(cfg(), 0.0, 0.0, 3);
        place(&mut p, &w, Owner::Ground(0), 900.0, State::Idle { remaining: 0.001, look: true });
        p.update(DT, &w);
        assert!(matches!(p.state, State::Sleep { .. } | State::Walk { .. }));
    }

    #[test]
    fn render_maps_state_and_facing() {
        let mut p = Pet::new(cfg(), 0.0, 0.0, 1);
        p.state = State::Idle { remaining: 1.0, look: true };
        p.facing = -1.0;
        assert_eq!(p.render(), RenderState { anim: Anim::Look, flip: true, heart: false });
        p.state = State::Turn { remaining: 0.1 };
        assert_eq!(p.render().anim, Anim::Idle);
    }

    #[test]
    fn window_pos_is_top_left_of_sprite() {
        let mut p = Pet::new(cfg(), 0.0, 0.0, 1);
        p.body = Body { x: 800.0, y: 400.0, vx: 0.0, vy: 0.0 };
        assert_eq!(p.window_pos(), (736.0, 272.0));
    }
}
```

- [ ] **Step 2: 실패 확인** — `cargo test -p screenpet-core brain` → FAIL

- [ ] **Step 3: 구현** — `brain.rs` 위쪽

```rust
use std::collections::VecDeque;

use serde::Serialize;

use crate::physics::{clamp_speed, step_air, Body};
use crate::surfaces::{EndKind, Owner};
use crate::world::World;

const WALK_SECS: (f64, f64) = (2.0, 8.0);
const IDLE_SECS: (f64, f64) = (3.0, 6.0);
const SLEEP_SECS: (f64, f64) = (15.0, 40.0);
const SLEEP_CHANCE: f64 = 0.25;
const WALK_END_TURN_CHANCE: f64 = 0.3;
const EDGE_HOP_CHANCE: f64 = 0.5;
const TURN_PAUSE: f64 = 0.3;
const LAND_SECS: f64 = 0.25;
const HOP_VX: f64 = 180.0;
const HOP_VY: f64 = -650.0;
const HAPPY_VY: f64 = -500.0;
const HEART_SECS: f64 = 1.0;
const CLICK_SLOP: f64 = 5.0;
const VELOCITY_WINDOW: f64 = 0.1;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Anim {
    Walk,
    Idle,
    Look,
    Sleep,
    Hop,
    Fall,
    Dragged,
    Land,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum State {
    Walk { remaining: f64 },
    Idle { remaining: f64, look: bool },
    Sleep { remaining: f64 },
    Turn { remaining: f64 },
    Land { remaining: f64 },
    /// Hop / Fall(낙하·던지기) / Happy 점프
    Air { anim: Anim },
    /// 마우스를 눌렀지만 아직 드래그 거리 미만
    Pressed { start_cx: f64, start_cy: f64, grounded: bool, was_sleeping: bool },
    Dragged,
}

/// 서 있는 디딤판과 그 기준점(anchor)으로부터의 x 오프셋.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Standing {
    pub owner: Owner,
    pub rel_x: f64,
}

#[derive(Debug, Clone, Copy)]
pub struct PetConfig {
    /// 스프라이트 한 변 (물리 px)
    pub size: f64,
    /// 몸 폭의 절반 — 디딤판 끝 판정에 사용
    pub half_w: f64,
    /// px/s
    pub walk_speed: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct RenderState {
    pub anim: Anim,
    pub flip: bool,
    pub heart: bool,
}

pub struct Pet {
    pub cfg: PetConfig,
    pub body: Body,
    pub state: State,
    pub standing: Option<Standing>,
    /// +1 오른쪽, -1 왼쪽
    pub facing: f64,
    heart_timer: f64,
    grab_dx: f64,
    grab_dy: f64,
    /// (clock, cursor x, cursor y)
    history: VecDeque<(f64, f64, f64)>,
    clock: f64,
    rng: fastrand::Rng,
}

impl Pet {
    pub fn new(cfg: PetConfig, x: f64, y: f64, seed: u64) -> Self {
        Self {
            cfg,
            body: Body { x, y, vx: 0.0, vy: 0.0 },
            state: State::Air { anim: Anim::Fall },
            standing: None,
            facing: 1.0,
            heart_timer: 0.0,
            grab_dx: 0.0,
            grab_dy: 0.0,
            history: VecDeque::new(),
            clock: 0.0,
            rng: fastrand::Rng::with_seed(seed),
        }
    }

    pub fn update(&mut self, dt: f64, world: &World) {
        self.clock += dt;
        self.heart_timer = (self.heart_timer - dt).max(0.0);
        match self.state {
            State::Dragged => {}
            State::Pressed { grounded, .. } => {
                if grounded {
                    self.follow_anchor(world);
                }
            }
            State::Air { .. } => self.update_air(dt, world),
            _ => self.update_ground(dt, world),
        }
    }

    pub fn render(&self) -> RenderState {
        let anim = match self.state {
            State::Walk { .. } => Anim::Walk,
            State::Idle { look: true, .. } => Anim::Look,
            State::Idle { .. } | State::Turn { .. } => Anim::Idle,
            State::Sleep { .. } => Anim::Sleep,
            State::Land { .. } => Anim::Land,
            State::Air { anim } => anim,
            State::Pressed { grounded: true, .. } => Anim::Idle,
            State::Pressed { .. } | State::Dragged => Anim::Dragged,
        };
        RenderState { anim, flip: self.facing < 0.0, heart: self.heart_timer > 0.0 }
    }

    pub fn window_pos(&self) -> (f64, f64) {
        (self.body.x - self.cfg.size / 2.0, self.body.y - self.cfg.size)
    }

    /// 서 있는 디딤판 기준점을 따라 위치 갱신. 기준점이 사라졌으면 false.
    fn follow_anchor(&mut self, world: &World) -> bool {
        let Some(st) = self.standing else { return false };
        let Some((ax, ay)) = world.anchor(st.owner) else { return false };
        self.body.x = ax + st.rel_x;
        self.body.y = ay;
        true
    }

    fn update_ground(&mut self, dt: f64, world: &World) {
        if !self.follow_anchor(world) {
            self.start_fall();
            return;
        }
        let st = self.standing.unwrap();
        let Some(piece) = world.piece_at(st.owner, self.body.x).copied() else {
            self.start_fall();
            return;
        };
        if piece.width() < 2.0 * self.cfg.half_w && !matches!(self.state, State::Land { .. }) {
            self.start_hop(self.facing);
            return;
        }

        match self.state {
            State::Walk { remaining } => {
                let new_x = self.body.x + self.facing * self.cfg.walk_speed * dt;
                let (hit, kind) = if self.facing > 0.0 {
                    (new_x + self.cfg.half_w > piece.x_end, piece.end_kind)
                } else {
                    (new_x - self.cfg.half_w < piece.x_start, piece.start_kind)
                };
                if hit {
                    if kind == EndKind::Edge && self.rng.f64() < EDGE_HOP_CHANCE {
                        self.start_hop(self.facing);
                    } else {
                        self.state = State::Turn { remaining: TURN_PAUSE };
                    }
                    return;
                }
                self.standing = Some(Standing { owner: st.owner, rel_x: st.rel_x + (new_x - self.body.x) });
                self.body.x = new_x;
                let remaining = remaining - dt;
                self.state = if remaining > 0.0 {
                    State::Walk { remaining }
                } else if self.rng.f64() < WALK_END_TURN_CHANCE {
                    State::Turn { remaining: TURN_PAUSE }
                } else {
                    self.idle_state()
                };
            }
            State::Idle { remaining, look } => {
                let remaining = remaining - dt;
                self.state = if remaining > 0.0 {
                    State::Idle { remaining, look }
                } else if self.rng.f64() < SLEEP_CHANCE {
                    State::Sleep { remaining: self.range(SLEEP_SECS) }
                } else {
                    if self.rng.bool() {
                        self.facing = -self.facing;
                    }
                    State::Walk { remaining: self.range(WALK_SECS) }
                };
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
                    State::Walk { remaining: self.range(WALK_SECS) }
                };
            }
            State::Land { remaining } => {
                let remaining = remaining - dt;
                self.state = if remaining > 0.0 { State::Land { remaining } } else { self.idle_state() };
            }
            State::Air { .. } | State::Pressed { .. } | State::Dragged => {}
        }
    }

    fn update_air(&mut self, dt: f64, world: &World) {
        let prev_y = self.body.y;
        step_air(&mut self.body, dt, &world.bounds, self.cfg.half_w, self.cfg.size);
        if let Some(s) = world.landing(self.body.x, prev_y, self.body.y).copied() {
            let (ax, _) = world.anchor(s.owner).expect("surface owner has anchor");
            self.body = Body { x: self.body.x, y: s.y, vx: 0.0, vy: 0.0 };
            self.standing = Some(Standing { owner: s.owner, rel_x: self.body.x - ax });
            self.state = State::Land { remaining: LAND_SECS };
        } else if self.body.y > world.bounds.bottom + self.cfg.size * 4.0 {
            // 화면 밖으로 사라짐 → 가까운 위치 상단에서 다시 떨어뜨림
            let b = world.bounds;
            self.body = Body {
                x: self.body.x.clamp(b.left + self.cfg.half_w, b.right - self.cfg.half_w),
                y: b.top + self.cfg.size,
                vx: 0.0,
                vy: 0.0,
            };
        }
    }

    pub(crate) fn start_hop(&mut self, dir: f64) {
        self.standing = None;
        self.body.vx = dir * HOP_VX;
        self.body.vy = HOP_VY;
        self.state = State::Air { anim: Anim::Hop };
    }

    fn start_fall(&mut self) {
        self.standing = None;
        self.body.vx = 0.0;
        self.body.vy = 0.0;
        self.state = State::Air { anim: Anim::Fall };
    }

    fn idle_state(&mut self) -> State {
        State::Idle { remaining: self.range(IDLE_SECS), look: self.rng.bool() }
    }

    fn range(&mut self, (lo, hi): (f64, f64)) -> f64 {
        lo + self.rng.f64() * (hi - lo)
    }
}
```

- [ ] **Step 4: 통과 확인** — `cargo test -p screenpet-core brain` → 15 passed
- [ ] **Step 5: 커밋** — `git add -A && git commit -m "feat(core): pet state machine for grounded and airborne behavior"`

---

### Task 7: 펫 상태 머신 — 마우스 입력 (클릭·드래그·던지기)

**Files:**
- Modify: `crates/screenpet-core/src/brain.rs`

**Interfaces:**
- Produces:
```rust
impl Pet {
  pub fn pointer_down(&mut self, cx: f64, cy: f64);
  pub fn pointer_move(&mut self, cx: f64, cy: f64);   // 버튼 누른 동안 매 틱 호출
  pub fn pointer_up(&mut self, cx: f64, cy: f64);
  pub fn is_held(&self) -> bool;                      // Pressed | Dragged
}
```

- [ ] **Step 1: 실패하는 테스트** — `brain.rs`에 두 번째 테스트 모듈 추가

```rust
#[cfg(test)]
mod input_tests {
    use super::tests::{cfg, place, run, w1, world};
    use super::*;

    const DT: f64 = 1.0 / 60.0;

    fn idle() -> State {
        State::Idle { remaining: 100.0, look: false }
    }

    #[test]
    fn click_on_grounded_pet_is_happy_and_lands_back() {
        let w = world(vec![w1()]);
        let mut p = Pet::new(cfg(), 0.0, 0.0, 1);
        place(&mut p, &w, Owner::Window(1), 800.0, idle());
        p.pointer_down(800.0, 350.0);
        assert!(p.is_held());
        p.pointer_up(800.0, 350.0);
        assert!(!p.is_held());
        assert_eq!(p.state, State::Air { anim: Anim::Hop });
        assert!(p.render().heart);
        run(&mut p, &w, 120);
        assert_eq!(p.standing.unwrap().owner, Owner::Window(1));
    }

    #[test]
    fn tiny_move_still_counts_as_click() {
        let w = world(vec![w1()]);
        let mut p = Pet::new(cfg(), 0.0, 0.0, 1);
        place(&mut p, &w, Owner::Window(1), 800.0, idle());
        p.pointer_down(800.0, 350.0);
        p.pointer_move(803.0, 351.0);
        p.pointer_up(803.0, 351.0);
        assert!(p.render().heart);
    }

    #[test]
    fn click_on_sleeping_pet_wakes_it() {
        let w = world(vec![w1()]);
        let mut p = Pet::new(cfg(), 0.0, 0.0, 1);
        place(&mut p, &w, Owner::Window(1), 800.0, State::Sleep { remaining: 30.0 });
        p.pointer_down(800.0, 350.0);
        p.pointer_up(800.0, 350.0);
        assert!(matches!(p.state, State::Idle { .. }));
        assert!(!p.render().heart);
    }

    #[test]
    fn pressed_grounded_pet_keeps_riding_window() {
        let w = world(vec![w1()]);
        let mut p = Pet::new(cfg(), 0.0, 0.0, 1);
        place(&mut p, &w, Owner::Window(1), 800.0, idle());
        p.pointer_down(800.0, 350.0);
        p.update(DT, &world(vec![super::tests::win(1, 600.0, 400.0, 1200.0, 900.0)]));
        assert_eq!(p.body.x, 900.0);
    }

    #[test]
    fn drag_moves_pet_with_cursor_keeping_grab_offset() {
        let w = world(vec![w1()]);
        let mut p = Pet::new(cfg(), 0.0, 0.0, 1);
        place(&mut p, &w, Owner::Window(1), 800.0, idle());
        p.pointer_down(810.0, 350.0);
        p.pointer_move(910.0, 250.0);
        assert_eq!(p.state, State::Dragged);
        assert!(p.standing.is_none());
        assert_eq!((p.body.x, p.body.y), (900.0, 300.0));
        assert_eq!(p.render().anim, Anim::Dragged);
    }

    #[test]
    fn release_throws_with_recent_cursor_velocity() {
        let w = world(vec![w1()]);
        let mut p = Pet::new(cfg(), 0.0, 0.0, 1);
        place(&mut p, &w, Owner::Window(1), 800.0, idle());
        p.pointer_down(800.0, 300.0);
        for i in 1..=6 {
            p.update(DT, &w);
            p.pointer_move(800.0 + 20.0 * i as f64, 300.0);
        }
        p.pointer_up(920.0, 300.0);
        assert_eq!(p.state, State::Air { anim: Anim::Fall });
        assert!((p.body.vx - 1200.0).abs() < 1.0, "vx={}", p.body.vx);
        assert!(p.body.vy.abs() < 1.0);
        assert_eq!(p.facing, 1.0);
    }

    #[test]
    fn click_while_airborne_drops_with_zero_velocity() {
        let w = world(vec![w1()]);
        let mut p = Pet::new(cfg(), 800.0, 200.0, 1);
        p.body.vx = 500.0;
        p.body.vy = 300.0;
        p.pointer_down(800.0, 150.0);
        p.update(DT, &w);
        assert_eq!(p.body.y, 200.0, "held pet must not move");
        p.pointer_up(800.0, 150.0);
        assert_eq!(p.state, State::Air { anim: Anim::Fall });
        assert_eq!((p.body.vx, p.body.vy), (0.0, 0.0));
        assert!(!p.render().heart);
    }

    #[test]
    fn second_pointer_down_while_held_is_ignored() {
        let w = world(vec![w1()]);
        let mut p = Pet::new(cfg(), 0.0, 0.0, 1);
        place(&mut p, &w, Owner::Window(1), 800.0, idle());
        p.pointer_down(800.0, 350.0);
        p.pointer_move(900.0, 350.0);
        p.pointer_down(950.0, 350.0);
        assert_eq!(p.state, State::Dragged);
    }
}
```

- [ ] **Step 2: 실패 확인** — `cargo test -p screenpet-core input_tests` → FAIL (`pointer_down` 없음)

- [ ] **Step 3: 구현** — `impl Pet` 블록에 추가

```rust
    pub fn is_held(&self) -> bool {
        matches!(self.state, State::Pressed { .. } | State::Dragged)
    }

    pub fn pointer_down(&mut self, cx: f64, cy: f64) {
        if self.is_held() {
            return;
        }
        let grounded = self.standing.is_some() && !matches!(self.state, State::Air { .. });
        let was_sleeping = matches!(self.state, State::Sleep { .. });
        self.grab_dx = self.body.x - cx;
        self.grab_dy = self.body.y - cy;
        self.history.clear();
        self.history.push_back((self.clock, cx, cy));
        if !grounded {
            self.standing = None;
            self.body.vx = 0.0;
            self.body.vy = 0.0;
        }
        self.state = State::Pressed { start_cx: cx, start_cy: cy, grounded, was_sleeping };
    }

    pub fn pointer_move(&mut self, cx: f64, cy: f64) {
        match self.state {
            State::Pressed { start_cx, start_cy, .. } => {
                self.record(cx, cy);
                if (cx - start_cx).hypot(cy - start_cy) >= CLICK_SLOP {
                    self.standing = None;
                    self.state = State::Dragged;
                    self.drag_to(cx, cy);
                }
            }
            State::Dragged => {
                self.record(cx, cy);
                self.drag_to(cx, cy);
            }
            _ => {}
        }
    }

    pub fn pointer_up(&mut self, cx: f64, cy: f64) {
        match self.state {
            State::Pressed { grounded: true, was_sleeping: true, .. } => {
                self.state = self.idle_state();
            }
            State::Pressed { grounded: true, .. } => {
                self.heart_timer = HEART_SECS;
                self.standing = None;
                self.body.vx = 0.0;
                self.body.vy = HAPPY_VY;
                self.state = State::Air { anim: Anim::Hop };
            }
            State::Pressed { .. } => self.start_fall(),
            State::Dragged => {
                self.record(cx, cy);
                let (vx, vy) = self.release_velocity();
                let (vx, vy) = clamp_speed(vx, vy);
                if vx.abs() > 50.0 {
                    self.facing = vx.signum();
                }
                self.standing = None;
                self.body.vx = vx;
                self.body.vy = vy;
                self.state = State::Air { anim: Anim::Fall };
            }
            _ => {}
        }
    }

    fn drag_to(&mut self, cx: f64, cy: f64) {
        self.body.x = cx + self.grab_dx;
        self.body.y = cy + self.grab_dy;
    }

    fn record(&mut self, cx: f64, cy: f64) {
        self.history.push_back((self.clock, cx, cy));
        while self.history.len() > 2 && self.clock - self.history[0].0 > VELOCITY_WINDOW {
            self.history.pop_front();
        }
    }

    fn release_velocity(&self) -> (f64, f64) {
        let (Some(&(t0, x0, y0)), Some(&(t1, x1, y1))) = (self.history.front(), self.history.back()) else {
            return (0.0, 0.0);
        };
        let dt = t1 - t0;
        if dt < 1e-6 {
            return (0.0, 0.0);
        }
        ((x1 - x0) / dt, (y1 - y0) / dt)
    }
```

참고: 테스트 `release_throws_with_recent_cursor_velocity`에서 pointer_up이 마지막 위치(920)를 같은 clock으로 한 번 더 기록하므로 속도 계산에 영향 없음(동일 시간·동일 위치).

- [ ] **Step 4: 통과 확인** — `cargo test -p screenpet-core` → 전체 통과
- [ ] **Step 5: 커밋** — `git commit -am "feat(core): click, drag and throw input handling"`

---

### Task 8: platform — WindowSource trait + Win32 구현

**Files:**
- Create: `crates/screenpet-core/src/platform/mod.rs`, `crates/screenpet-core/src/platform/win32.rs`
- Modify: `crates/screenpet-core/Cargo.toml`, `lib.rs` (`pub mod platform;`)

**Interfaces:**
- Produces:
```rust
pub trait WindowSource: Send {
    /// 다른 프로세스의 보이는 최상위 창, 앞→뒤(Z-order) 순.
    fn windows(&mut self) -> Vec<WindowInfo>;
    /// 모니터별 작업 영역 (작업표시줄 제외).
    fn work_areas(&mut self) -> Vec<Rect>;
    /// 전역 커서 위치 (물리 px).
    fn cursor(&mut self) -> (f64, f64);
    fn left_button_down(&mut self) -> bool;
}
pub struct NullSource;                         // 창 없음, 작업영역 없음(World가 기본값 사용), 커서 (0,0), 버튼 false
pub fn default_source() -> Box<dyn WindowSource>;   // windows: Win32Source, 그 외: NullSource
```

- [ ] **Step 1: 실패하는 테스트** — `platform/mod.rs` 하단

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn null_source_is_empty_and_released() {
        let mut s = NullSource;
        assert!(s.windows().is_empty());
        assert!(s.work_areas().is_empty());
        assert!(!s.left_button_down());
    }
}
```

- [ ] **Step 2: 실패 확인** — `cargo test -p screenpet-core platform` → FAIL

- [ ] **Step 3: Cargo.toml에 Windows 전용 의존성 추가**

```toml
[target.'cfg(windows)'.dependencies]
windows = { version = "0.58", features = [
    "Win32_Foundation",
    "Win32_Graphics_Dwm",
    "Win32_Graphics_Gdi",
    "Win32_UI_Input_KeyboardAndMouse",
    "Win32_UI_WindowsAndMessaging",
] }
```

- [ ] **Step 4: 구현** — `platform/mod.rs`

```rust
use crate::geom::{Rect, WindowInfo};

#[cfg(windows)]
mod win32;
#[cfg(windows)]
pub use win32::Win32Source;

pub trait WindowSource: Send {
    /// 다른 프로세스의 보이는 최상위 창, 앞→뒤(Z-order) 순.
    fn windows(&mut self) -> Vec<WindowInfo>;
    /// 모니터별 작업 영역 (작업표시줄 제외).
    fn work_areas(&mut self) -> Vec<Rect>;
    /// 전역 커서 위치 (물리 px).
    fn cursor(&mut self) -> (f64, f64);
    fn left_button_down(&mut self) -> bool;
}

/// 지원하지 않는 OS용. 바닥(기본 화면)만 있는 세계가 된다.
pub struct NullSource;

impl WindowSource for NullSource {
    fn windows(&mut self) -> Vec<WindowInfo> {
        Vec::new()
    }

    fn work_areas(&mut self) -> Vec<Rect> {
        Vec::new()
    }

    fn cursor(&mut self) -> (f64, f64) {
        (0.0, 0.0)
    }

    fn left_button_down(&mut self) -> bool {
        false
    }
}

pub fn default_source() -> Box<dyn WindowSource> {
    #[cfg(windows)]
    return Box::new(Win32Source);
    #[cfg(not(windows))]
    return Box::new(NullSource);
}
```

`platform/win32.rs`:
```rust
use std::ffi::c_void;
use std::mem::size_of;

use windows::Win32::Foundation::{BOOL, HWND, LPARAM, POINT, RECT, TRUE};
use windows::Win32::Graphics::Dwm::{DwmGetWindowAttribute, DWMWA_CLOAKED, DWMWA_EXTENDED_FRAME_BOUNDS};
use windows::Win32::Graphics::Gdi::{EnumDisplayMonitors, GetMonitorInfoW, HDC, HMONITOR, MONITORINFO};
use windows::Win32::UI::Input::KeyboardAndMouse::{GetAsyncKeyState, VK_LBUTTON};
use windows::Win32::UI::WindowsAndMessaging::{
    EnumWindows, GetClassNameW, GetCursorPos, GetWindowLongW, GetWindowTextLengthW, GetWindowThreadProcessId,
    IsIconic, IsWindowVisible, GWL_EXSTYLE, WS_EX_TOOLWINDOW, WS_EX_TRANSPARENT,
};

use super::WindowSource;
use crate::geom::{Rect, WindowInfo};

/// 디딤판이 되면 안 되는 셸 창 클래스
const SHELL_CLASSES: [&str; 4] = ["Progman", "WorkerW", "Shell_TrayWnd", "Shell_SecondaryTrayWnd"];
const MIN_SIZE: f64 = 50.0;

pub struct Win32Source;

impl WindowSource for Win32Source {
    fn windows(&mut self) -> Vec<WindowInfo> {
        let mut hwnds: Vec<HWND> = Vec::new();
        unsafe {
            let _ = EnumWindows(Some(collect_window), LPARAM(&mut hwnds as *mut Vec<HWND> as isize));
        }
        let own_pid = std::process::id();
        hwnds
            .into_iter()
            .filter(|&h| is_candidate(h, own_pid))
            .filter_map(|h| {
                let rect = frame_rect(h)?;
                (rect.width() >= MIN_SIZE && rect.height() >= MIN_SIZE)
                    .then_some(WindowInfo { id: h.0 as usize as u64, rect })
            })
            .collect()
    }

    fn work_areas(&mut self) -> Vec<Rect> {
        let mut list: Vec<Rect> = Vec::new();
        unsafe {
            let _ = EnumDisplayMonitors(HDC::default(), None, Some(collect_monitor), LPARAM(&mut list as *mut Vec<Rect> as isize));
        }
        list
    }

    fn cursor(&mut self) -> (f64, f64) {
        let mut p = POINT::default();
        unsafe {
            let _ = GetCursorPos(&mut p);
        }
        (p.x as f64, p.y as f64)
    }

    fn left_button_down(&mut self) -> bool {
        unsafe { (GetAsyncKeyState(VK_LBUTTON.0 as i32) as u16 & 0x8000) != 0 }
    }
}

unsafe extern "system" fn collect_window(hwnd: HWND, lparam: LPARAM) -> BOOL {
    let list = &mut *(lparam.0 as *mut Vec<HWND>);
    list.push(hwnd);
    TRUE
}

unsafe extern "system" fn collect_monitor(hmon: HMONITOR, _hdc: HDC, _clip: *mut RECT, lparam: LPARAM) -> BOOL {
    let list = &mut *(lparam.0 as *mut Vec<Rect>);
    let mut info = MONITORINFO { cbSize: size_of::<MONITORINFO>() as u32, ..Default::default() };
    if GetMonitorInfoW(hmon, &mut info).as_bool() {
        list.push(to_rect(info.rcWork));
    }
    TRUE
}

fn to_rect(r: RECT) -> Rect {
    Rect::new(r.left as f64, r.top as f64, r.right as f64, r.bottom as f64)
}

fn is_candidate(hwnd: HWND, own_pid: u32) -> bool {
    unsafe {
        if !IsWindowVisible(hwnd).as_bool() || IsIconic(hwnd).as_bool() {
            return false;
        }
        let mut pid = 0u32;
        GetWindowThreadProcessId(hwnd, Some(&mut pid as *mut u32));
        if pid == own_pid {
            return false;
        }
        let ex = GetWindowLongW(hwnd, GWL_EXSTYLE) as u32;
        if ex & (WS_EX_TOOLWINDOW.0 | WS_EX_TRANSPARENT.0) != 0 {
            return false;
        }
        // 제목 없는 창은 대부분 오버레이·보조 창
        if GetWindowTextLengthW(hwnd) == 0 {
            return false;
        }
        // 다른 가상 데스크톱, 일시 중단된 UWP 앱 등
        let mut cloaked = 0u32;
        let res = DwmGetWindowAttribute(hwnd, DWMWA_CLOAKED, &mut cloaked as *mut u32 as *mut c_void, size_of::<u32>() as u32);
        if res.is_ok() && cloaked != 0 {
            return false;
        }
        let mut buf = [0u16; 64];
        let n = GetClassNameW(hwnd, &mut buf).max(0) as usize;
        let class = String::from_utf16_lossy(&buf[..n]);
        !SHELL_CLASSES.contains(&class.as_str())
    }
}

/// 그림자를 뺀 실제 창 테두리
fn frame_rect(hwnd: HWND) -> Option<Rect> {
    let mut r = RECT::default();
    unsafe {
        DwmGetWindowAttribute(hwnd, DWMWA_EXTENDED_FRAME_BOUNDS, &mut r as *mut RECT as *mut c_void, size_of::<RECT>() as u32).ok()?;
    }
    Some(to_rect(r))
}
```

- [ ] **Step 5: 테스트 통과 확인** — `cargo test -p screenpet-core` → 전체 통과

- [ ] **Step 6: Windows 대상 교차 컴파일 검사** — `cargo check -p screenpet-core --target x86_64-pc-windows-gnu`
Expected: `Finished`. 에러가 나면 `windows` 0.58 시그니처(예: `EnumDisplayMonitors` 첫 인자, `GetWindowThreadProcessId` 두 번째 인자 타입)를 에러 메시지에 맞게 고친다. 동작은 바꾸지 않는다.

- [ ] **Step 7: 커밋** — `git add -A && git commit -m "feat(core): platform window source with Win32 implementation"`

---

### Task 9: 스프라이트 도구 + 임시 동물 4종 + 아이콘

**Files:**
- Create: `tools/sprite_spec.py`, `tools/gen_placeholder.py`, `tools/pack_sheet.py`, `tools/make_icon.py`, `tools/test_pack_sheet.py`, `docs/sprites.md`
- Generated (커밋): `ui/pets/{cat,dog,rabbit,beaver}/{sheet.png,pet.json}`

**Interfaces:**
- Produces: `sprite_spec.ANIMS: list[tuple[name, frames, fps, loop]]`, `sprite_spec.FRAME = 64`, `sprite_spec.manifest(name, walk_speed, frame_counts) -> dict`; `pack_sheet.pack(frames_dir: Path, out_dir: Path, name: str, walk_speed: float) -> None`; 출력 `pet.json`은 Task 5의 `parse_manifest`를 통과해야 한다.

- [ ] **Step 1: `tools/sprite_spec.py`**

```python
"""스프라이트 시트 규격 — 앱(src/manifest.rs REQUIRED_ANIMS)과 같은 순서를 유지한다."""

FRAME = 64

# (동작, 기본 프레임 수, fps, 반복)
ANIMS = [
    ("walk", 4, 8, True),
    ("idle", 2, 2, True),
    ("look", 4, 4, True),
    ("sleep", 2, 1, True),
    ("hop", 3, 10, False),
    ("fall", 2, 8, True),
    ("dragged", 2, 4, True),
    ("land", 2, 10, False),
]


def manifest(name, walk_speed, frame_counts):
    """frame_counts: {동작: 프레임 수}. 행 번호는 ANIMS 순서."""
    animations = {}
    for row, (anim, _, fps, loop) in enumerate(ANIMS):
        spec = {"row": row, "frames": frame_counts[anim], "fps": fps}
        if not loop:
            spec["loop"] = False
        animations[anim] = spec
    return {"name": name, "frame_size": FRAME, "walk_speed": walk_speed, "animations": animations}
```

- [ ] **Step 2: 실패하는 테스트** — `tools/test_pack_sheet.py`

```python
import json

from PIL import Image

from pack_sheet import pack
from sprite_spec import ANIMS, FRAME


def make_frames(d, size=(300, 200)):
    for anim, n, _, _ in ANIMS:
        for i in range(n):
            img = Image.new("RGBA", (512, 512), (0, 0, 0, 0))
            img.paste((200, 100, 50, 255), (100, 200, 100 + size[0], 200 + size[1]))
            img.save(d / f"{anim}_{i}.png")


def test_pack_builds_sheet_and_manifest(tmp_path):
    frames, out = tmp_path / "frames", tmp_path / "out"
    frames.mkdir()
    make_frames(frames)
    pack(frames, out, "테스트", 55)

    sheet = Image.open(out / "sheet.png")
    max_frames = max(n for _, n, _, _ in ANIMS)
    assert sheet.size == (FRAME * max_frames, FRAME * len(ANIMS))

    m = json.loads((out / "pet.json").read_text(encoding="utf-8"))
    assert m["name"] == "테스트"
    assert m["walk_speed"] == 55
    assert m["animations"]["walk"] == {"row": 0, "frames": 4, "fps": 8}
    assert m["animations"]["hop"]["loop"] is False


def test_frames_are_bottom_aligned_and_fit(tmp_path):
    frames, out = tmp_path / "frames", tmp_path / "out"
    frames.mkdir()
    make_frames(frames)
    pack(frames, out, "x", 60)
    first = Image.open(out / "sheet.png").crop((0, 0, FRAME, FRAME))
    bbox = first.getchannel("A").getbbox()
    assert bbox[3] == FRAME  # 발이 프레임 바닥에 닿음
    assert bbox[2] - bbox[0] <= FRAME


def test_missing_frame_raises(tmp_path):
    frames = tmp_path / "frames"
    frames.mkdir()
    try:
        pack(frames, tmp_path / "out", "x", 60)
    except FileNotFoundError as e:
        assert "walk_0.png" in str(e)
    else:
        raise AssertionError("expected FileNotFoundError")
```

- [ ] **Step 3: 실패 확인** — `cd tools && python -m pytest -q` → FAIL (`pack_sheet` 없음)

- [ ] **Step 4: `tools/pack_sheet.py`**

```python
"""AI로 만든 동작별 프레임 PNG들을 앱 규격 시트로 합친다.

사용법:
    python tools/pack_sheet.py <프레임폴더> ui/pets/<id> --name 고양이 --walk-speed 60

프레임 폴더에는 `<동작>_<번호>.png` (번호는 0부터) 파일이 있어야 한다.
동작별 프레임 수는 sprite_spec.ANIMS의 기본값보다 많아도 된다(연속된 번호만큼 읽음).
모든 프레임은 투명 배경이어야 하며, 공통 배율로 축소돼 64x64 안에 바닥·가운데 정렬된다.
"""

import argparse
import json
from pathlib import Path

from PIL import Image

from sprite_spec import ANIMS, FRAME, manifest


def _load_frames(frames_dir):
    frames = {}
    for anim, default_n, _, _ in ANIMS:
        imgs = []
        i = 0
        while (frames_dir / f"{anim}_{i}.png").exists():
            imgs.append(Image.open(frames_dir / f"{anim}_{i}.png").convert("RGBA"))
            i += 1
        if not imgs:
            raise FileNotFoundError(f"{frames_dir / f'{anim}_0.png'} 가 없습니다 (기본 {default_n}장 권장)")
        frames[anim] = imgs
    return frames


def pack(frames_dir, out_dir, name, walk_speed):
    frames_dir, out_dir = Path(frames_dir), Path(out_dir)
    frames = _load_frames(frames_dir)
    cropped = {a: [img.crop(img.getchannel("A").getbbox()) for img in imgs] for a, imgs in frames.items()}

    # 모든 프레임에 같은 배율 → 프레임마다 크기가 들쭉날쭉하지 않음
    biggest = max(max(c.width, c.height) for cs in cropped.values() for c in cs)
    scale = FRAME / biggest

    cols = max(len(cs) for cs in cropped.values())
    sheet = Image.new("RGBA", (FRAME * cols, FRAME * len(ANIMS)), (0, 0, 0, 0))
    for row, (anim, _, _, _) in enumerate(ANIMS):
        for col, c in enumerate(cropped[anim]):
            w, h = max(1, round(c.width * scale)), max(1, round(c.height * scale))
            small = c.resize((w, h), Image.LANCZOS)
            sheet.paste(small, (col * FRAME + (FRAME - w) // 2, row * FRAME + FRAME - h), small)

    out_dir.mkdir(parents=True, exist_ok=True)
    sheet.save(out_dir / "sheet.png")
    counts = {a: len(cs) for a, cs in cropped.items()}
    (out_dir / "pet.json").write_text(
        json.dumps(manifest(name, walk_speed, counts), ensure_ascii=False, indent=2), encoding="utf-8"
    )


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("frames_dir")
    ap.add_argument("out_dir")
    ap.add_argument("--name", required=True)
    ap.add_argument("--walk-speed", type=float, default=60)
    args = ap.parse_args()
    pack(args.frames_dir, args.out_dir, args.name, args.walk_speed)


if __name__ == "__main__":
    main()
```

- [ ] **Step 5: 통과 확인** — `cd tools && python -m pytest -q` → 3 passed

- [ ] **Step 6: `tools/gen_placeholder.py`**

```python
"""임시 픽셀 동물 4종 시트 생성. 사용법: python tools/gen_placeholder.py"""

import json
from pathlib import Path

from PIL import Image, ImageDraw

from sprite_spec import ANIMS, FRAME, manifest

OUT = Path(__file__).resolve().parent.parent / "ui" / "pets"
OUTLINE = (40, 30, 30, 255)
WHITE = (255, 255, 255, 255)

SPECIES = {
    "cat": dict(name="고양이", speed=60, body=(245, 166, 35, 255), dark=(190, 110, 20, 255), ears="pointy", tail="long"),
    "dog": dict(name="강아지", speed=70, body=(222, 184, 135, 255), dark=(139, 90, 43, 255), ears="floppy", tail="short"),
    "rabbit": dict(name="토끼", speed=80, body=(245, 245, 245, 255), dark=(255, 170, 190, 255), ears="long", tail="puff"),
    "beaver": dict(name="비버", speed=40, body=(150, 95, 55, 255), dark=(90, 55, 30, 255), ears="round", tail="flat"),
}


def pose(anim, i):
    p = dict(dy=0, squash=0, legs=0, eyes="open", head_dx=0, dangle=False, z=False)
    if anim == "walk":
        p.update(legs=[0, 1, 0, -1][i], dy=[0, 1, 0, 1][i])
    elif anim == "idle":
        p.update(dy=i)
    elif anim == "look":
        p.update(head_dx=[0, 3, 0, -3][i])
    elif anim == "sleep":
        p.update(eyes="closed", squash=3, z=i == 1)
    elif anim == "hop":
        p.update(dy=[2, 8, 5][i], legs=[0, 1, 1][i])
    elif anim == "fall":
        p.update(eyes="wide", legs=[1, -1][i])
    elif anim == "dragged":
        p.update(eyes="wide", dangle=True, head_dx=[-1, 1][i])
    elif anim == "land":
        p.update(squash=[5, 2][i])
    return p


def draw_frame(sp, anim, i):
    p = pose(anim, i)
    img = Image.new("RGBA", (FRAME, FRAME), (0, 0, 0, 0))
    d = ImageDraw.Draw(img)
    body, dark = sp["body"], sp["dark"]
    base = FRAME - 2 - p["dy"]
    leg_len = 10 if p["dangle"] else 6
    top = 38 - p["dy"] + p["squash"]

    for k, lx in enumerate((18, 26, 34, 42)):
        off = p["legs"] * (2 if k % 2 == 0 else -2)
        d.rectangle([lx + off, base - leg_len, lx + off + 4, base], fill=dark, outline=OUTLINE)

    ty = top + 6
    if sp["tail"] == "long":
        d.line([(14, ty + 4), (6, ty - 6), (8, ty - 12)], fill=body, width=4)
    elif sp["tail"] == "short":
        d.ellipse([8, ty - 2, 16, ty + 6], fill=body, outline=OUTLINE)
    elif sp["tail"] == "puff":
        d.ellipse([7, ty, 17, ty + 10], fill=WHITE, outline=OUTLINE)
    elif sp["tail"] == "flat":
        d.ellipse([2, ty + 4, 18, ty + 12], fill=dark, outline=OUTLINE)

    d.ellipse([12 - p["squash"], top, 48 + p["squash"], base - leg_len + 4], fill=body, outline=OUTLINE)

    hx, hy, r = 46 + p["head_dx"], top - 2, 11
    if sp["ears"] == "pointy":
        d.polygon([(hx - 9, hy - 6), (hx - 6, hy - 17), (hx - 1, hy - 9)], fill=body, outline=OUTLINE)
        d.polygon([(hx + 1, hy - 9), (hx + 6, hy - 17), (hx + 9, hy - 6)], fill=body, outline=OUTLINE)
    elif sp["ears"] == "long":
        d.ellipse([hx - 7, hy - 26, hx - 2, hy - 6], fill=body, outline=OUTLINE)
        d.ellipse([hx + 2, hy - 26, hx + 7, hy - 6], fill=body, outline=OUTLINE)
    elif sp["ears"] == "round":
        d.ellipse([hx - 11, hy - 12, hx - 4, hy - 5], fill=dark, outline=OUTLINE)
        d.ellipse([hx + 4, hy - 12, hx + 11, hy - 5], fill=dark, outline=OUTLINE)

    d.ellipse([hx - r, hy - r, hx + r, hy + r], fill=body, outline=OUTLINE)

    if sp["ears"] == "floppy":
        d.ellipse([hx - 13, hy - 6, hx - 6, hy + 8], fill=dark, outline=OUTLINE)
        d.ellipse([hx + 6, hy - 6, hx + 13, hy + 8], fill=dark, outline=OUTLINE)

    ex, ey = hx + 4, hy - 1
    if p["eyes"] == "open":
        d.rectangle([ex, ey, ex + 1, ey + 2], fill=OUTLINE)
    elif p["eyes"] == "closed":
        d.line([(ex - 1, ey + 1), (ex + 2, ey + 1)], fill=OUTLINE)
    else:
        d.ellipse([ex - 1, ey - 1, ex + 3, ey + 3], fill=WHITE, outline=OUTLINE)
    d.rectangle([hx + 9, hy + 2, hx + 10, hy + 3], fill=(230, 90, 110, 255))
    if p["z"]:
        d.text((54, 0), "z", fill=OUTLINE)
    return img


def main():
    cols = max(n for _, n, _, _ in ANIMS)
    for pid, sp in SPECIES.items():
        sheet = Image.new("RGBA", (FRAME * cols, FRAME * len(ANIMS)), (0, 0, 0, 0))
        for row, (anim, n, _, _) in enumerate(ANIMS):
            for i in range(n):
                sheet.paste(draw_frame(sp, anim, i), (i * FRAME, row * FRAME))
        out = OUT / pid
        out.mkdir(parents=True, exist_ok=True)
        sheet.save(out / "sheet.png")
        counts = {a: n for a, n, _, _ in ANIMS}
        (out / "pet.json").write_text(
            json.dumps(manifest(sp["name"], sp["speed"], counts), ensure_ascii=False, indent=2), encoding="utf-8"
        )
        print("wrote", out)


if __name__ == "__main__":
    main()
```

Run: `python tools/gen_placeholder.py` → `wrote .../ui/pets/{cat,dog,rabbit,beaver}` 4줄.
그 다음 `ui/pets/cat/sheet.png`를 Read 도구로 열어 동물 모양이 보이는지 눈으로 확인한다(잘림·빈 프레임 없음).

- [ ] **Step 7: 생성된 pet.json이 Rust 파서를 통과하는지 확인** — `manifest.rs` 테스트 모듈에 추가:

```rust
    #[test]
    fn bundled_placeholder_manifests_are_valid() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../ui/pets");
        let mut count = 0;
        for entry in std::fs::read_dir(dir).unwrap() {
            let path = entry.unwrap().path().join("pet.json");
            if path.is_file() {
                parse_manifest(&std::fs::read_to_string(&path).unwrap()).unwrap_or_else(|e| panic!("{path:?}: {e}"));
                count += 1;
            }
        }
        assert!(count >= 4);
    }
```
Run: `cargo test -p screenpet-core bundled` → PASS

- [ ] **Step 8: `tools/make_icon.py`** (앱/트레이 아이콘 원본, 1024px 발바닥)

```python
"""앱 아이콘 원본 생성: python tools/make_icon.py → tools/icon.png
이후: npx tauri icon tools/icon.png (src-tauri/icons/ 생성)"""

from pathlib import Path

from PIL import Image, ImageDraw

S = 1024
img = Image.new("RGBA", (S, S), (0, 0, 0, 0))
d = ImageDraw.Draw(img)
d.ellipse([40, 40, S - 40, S - 40], fill=(255, 190, 90, 255))
paw = (120, 70, 40, 255)
d.ellipse([312, 470, 712, 840], fill=paw)
for cx, cy in [(300, 380), (430, 270), (594, 270), (724, 380)]:
    d.ellipse([cx - 80, cy - 95, cx + 80, cy + 95], fill=paw)
img.save(Path(__file__).resolve().parent / "icon.png")
```
Run: `python tools/make_icon.py` (아이콘 변환은 Task 10에서 Tauri CLI 설치 후).

- [ ] **Step 9: `docs/sprites.md`** — AI 스프라이트 제작 가이드

````markdown
# 동물 스프라이트 만들기 (AI 생성)

## 규격
- 동작별로 PNG를 따로 만들고 `tools/pack_sheet.py`로 합친다. 파일명: `<동작>_<번호>.png` (0부터).
- 투명 배경, **오른쪽을 보는** 옆모습, 발이 그림 맨 아래에 닿게.
- 크기는 자유(예: 512×512). 도구가 공통 배율로 64×64에 맞춰 바닥·가운데 정렬한다.

| 동작 | 권장 장수 | 내용 |
|---|---|---|
| walk | 4 | 걷기 한 사이클 (다리 교차) |
| idle | 2 | 앉아서 숨쉬기 (살짝 위아래) |
| look | 4 | 두리번거리기 (정면→옆→정면→반대) |
| sleep | 2 | 웅크리고 자기 (호흡) |
| hop | 3 | 도약 준비 → 공중 → 하강 |
| fall | 2 | 놀라서 팔다리 벌리고 떨어짐 |
| dragged | 2 | 목덜미 잡혀 대롱대롱 |
| land | 2 | 착지 찌그러짐 → 복귀 |

## 일관성 팁
1. **기준 그림 먼저**: 동물 하나의 `idle_0`를 마음에 들 때까지 만든 뒤, 그 이미지를 참조 이미지로 넣고 나머지 동작을 만든다.
2. 프롬프트의 스타일 문구를 **모든 프레임에서 그대로 복사**한다.
3. 한 동작의 프레임들은 한 번에 "가로 N칸 스프라이트 시트"로 요청한 뒤 잘라 쓰면 더 일관적이다.
4. 배경 제거가 안 되는 도구면 단색 배경으로 만들고 배경 제거 도구로 투명화한다.

## 프롬프트 템플릿
```
Cute chibi {동물} side view facing right, {동작 설명}, pixel art style,
64x64 sprite, thick dark outline, flat pastel colors, full body, feet touching
the bottom edge, transparent background, consistent character, no text
```
예: `{동물}=orange tabby cat`, `{동작 설명}=walking, left front leg forward (frame 1 of 4)`

## 합치기
```bash
python tools/pack_sheet.py my_frames/cat ui/pets/cat --name 고양이 --walk-speed 60
```
새 동물은 `ui/pets/<새id>/`로 출력하면 다음 빌드부터 트레이 메뉴에 나타난다.
````

- [ ] **Step 10: 커밋**
```bash
git add -A && git commit -m "feat(tools): sprite spec, placeholder pets, sheet packer, icon source"
```

---

### Task 10: Tauri 앱 셸 + 웹뷰 렌더러

**Files:**
- Create: `package.json`, `package-lock.json`, `src-tauri/Cargo.toml`, `src-tauri/build.rs`, `src-tauri/tauri.conf.json`, `src-tauri/capabilities/default.json`, `src-tauri/icons/*`, `src-tauri/src/main.rs`, `src-tauri/src/pets.rs`, `ui/index.html`, `ui/style.css`, `ui/main.js`
- Modify: `Cargo.toml` (members에 `"src-tauri"` 추가)

**Interfaces:**
- Consumes: `screenpet_core::manifest::{parse_manifest, PetManifest}`
- Produces:
  - `pets::PetEntry { pub id: String, pub manifest: PetManifest }` (Clone), `pets::load_pets() -> Vec<PetEntry>`
  - 웹뷰 이벤트 계약: `"pet-render"` payload `{anim: string, flip: bool, heart: bool}`, `"pet-change"` payload `string`(pet id)
  - 커맨드 계약: `pointer_down()`, `current_pet() -> String`

- [ ] **Step 1: Tauri CLI + 아이콘**

`package.json`:
```json
{
  "name": "screenpet",
  "private": true,
  "scripts": {
    "tauri": "tauri"
  },
  "devDependencies": {
    "@tauri-apps/cli": "^2"
  }
}
```
Run:
```bash
npm install
npx tauri icon tools/icon.png -o src-tauri/icons
ls src-tauri/icons
```
Expected: `32x32.png 128x128.png 128x128@2x.png icon.icns icon.ico ...`

- [ ] **Step 2: `src-tauri/Cargo.toml`**

```toml
[package]
name = "screenpet"
version = "0.1.0"
edition = "2021"

[build-dependencies]
tauri-build = { version = "2", features = [] }

[dependencies]
tauri = { version = "2", features = ["tray-icon"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
screenpet-core = { path = "../crates/screenpet-core" }
```

루트 `Cargo.toml`의 `members`를 `["crates/screenpet-core", "src-tauri"]`로 바꾼다 (`default-members`는 core만 유지 → 리눅스 `cargo test`는 core만 빌드).

- [ ] **Step 3: `src-tauri/build.rs`** — `ui/pets/*/pet.json`을 바이너리에 포함

```rust
use std::{env, fs, path::Path};

fn main() {
    let manifest_dir = env::var("CARGO_MANIFEST_DIR").unwrap();
    let pets_dir = Path::new(&manifest_dir).join("../ui/pets");
    println!("cargo:rerun-if-changed={}", pets_dir.display());

    let mut ids: Vec<String> = fs::read_dir(&pets_dir)
        .expect("ui/pets 폴더가 필요합니다")
        .filter_map(|e| e.ok())
        .filter(|e| e.path().join("pet.json").is_file())
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .collect();
    ids.sort();
    assert!(!ids.is_empty(), "ui/pets 안에 pet.json이 있는 동물 폴더가 하나 이상 필요합니다");

    let mut code = String::from("pub const BUNDLED_PETS: &[(&str, &str)] = &[\n");
    for id in &ids {
        let path = pets_dir.join(id).join("pet.json");
        println!("cargo:rerun-if-changed={}", path.display());
        code.push_str(&format!("    ({:?}, include_str!({:?})),\n", id, path.display().to_string()));
    }
    code.push_str("];\n");
    fs::write(Path::new(&env::var("OUT_DIR").unwrap()).join("pets.rs"), code).unwrap();

    tauri_build::build();
}
```

- [ ] **Step 4: `src-tauri/tauri.conf.json`**

```json
{
  "$schema": "https://schema.tauri.app/config/2",
  "productName": "ScreenPet",
  "version": "0.1.0",
  "identifier": "com.allsilver11.screenpet",
  "build": {
    "frontendDist": "../ui"
  },
  "app": {
    "withGlobalTauri": true,
    "windows": [
      {
        "label": "pet",
        "title": "ScreenPet",
        "url": "index.html",
        "width": 128,
        "height": 128,
        "transparent": true,
        "decorations": false,
        "alwaysOnTop": true,
        "skipTaskbar": true,
        "resizable": false,
        "shadow": false,
        "focus": false
      }
    ],
    "security": {
      "csp": null
    }
  },
  "bundle": {
    "active": true,
    "targets": ["msi", "nsis"],
    "icon": [
      "icons/32x32.png",
      "icons/128x128.png",
      "icons/128x128@2x.png",
      "icons/icon.icns",
      "icons/icon.ico"
    ]
  }
}
```

`src-tauri/capabilities/default.json`:
```json
{
  "$schema": "../gen/schemas/desktop-schema.json",
  "identifier": "default",
  "windows": ["pet"],
  "permissions": ["core:default"]
}
```

- [ ] **Step 5: `src-tauri/src/pets.rs`**

```rust
use screenpet_core::manifest::{parse_manifest, PetManifest};

include!(concat!(env!("OUT_DIR"), "/pets.rs"));

#[derive(Clone)]
pub struct PetEntry {
    /// ui/pets 아래 폴더 이름
    pub id: String,
    pub manifest: PetManifest,
}

/// 빌드 시 포함된 동물 중 매니페스트가 올바른 것만.
pub fn load_pets() -> Vec<PetEntry> {
    BUNDLED_PETS
        .iter()
        .filter_map(|(id, json)| match parse_manifest(json) {
            Ok(manifest) => Some(PetEntry { id: id.to_string(), manifest }),
            Err(e) => {
                eprintln!("skip pet {id}: {e}");
                None
            }
        })
        .collect()
}
```

- [ ] **Step 6: 임시 `src-tauri/src/main.rs`** (Task 11에서 완성 — 이 단계는 창이 뜨고 고양이가 그려지는지만)

```rust
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod pets;

#[tauri::command]
fn current_pet() -> String {
    pets::load_pets().first().map(|p| p.id.clone()).unwrap_or_default()
}

#[tauri::command]
fn pointer_down() {}

fn main() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![current_pet, pointer_down])
        .run(tauri::generate_context!())
        .expect("error while running ScreenPet");
}
```

- [ ] **Step 7: 웹뷰** — `ui/index.html`

```html
<!doctype html>
<html>
  <head>
    <meta charset="utf-8" />
    <link rel="stylesheet" href="style.css" />
  </head>
  <body>
    <div id="pet"></div>
    <div id="heart">♥</div>
    <script src="main.js"></script>
  </body>
</html>
```

`ui/style.css`:
```css
html,
body {
  margin: 0;
  width: 100%;
  height: 100%;
  overflow: hidden;
  background: transparent;
  user-select: none;
}

#pet {
  width: 128px;
  height: 128px;
  background-repeat: no-repeat;
  image-rendering: pixelated;
  cursor: grab;
}

#pet.flip {
  transform: scaleX(-1);
}

#heart {
  position: absolute;
  top: 0;
  left: 50%;
  transform: translateX(-50%);
  color: #ff5a8a;
  font-size: 28px;
  opacity: 0;
  pointer-events: none;
}

#heart.show {
  animation: float-up 1s ease-out;
}

@keyframes float-up {
  from {
    top: 28px;
    opacity: 1;
  }
  to {
    top: 0;
    opacity: 0;
  }
}
```

`ui/main.js`:
```js
// 백엔드가 보내는 {anim, flip, heart}를 받아 스프라이트 프레임만 그린다.
const { invoke } = window.__TAURI__.core;
const { listen } = window.__TAURI__.event;

const DISPLAY = 128;
const petEl = document.getElementById("pet");
const heartEl = document.getElementById("heart");

let manifest = null;
let scale = 2;
let render = { anim: "fall", flip: false, heart: false };
let animStart = performance.now();

async function loadPet(id) {
  manifest = await (await fetch(`pets/${id}/pet.json`)).json();
  scale = DISPLAY / manifest.frame_size;
  const img = new Image();
  img.src = `pets/${id}/sheet.png`;
  await img.decode();
  petEl.style.backgroundImage = `url(pets/${id}/sheet.png)`;
  petEl.style.backgroundSize = `${img.width * scale}px ${img.height * scale}px`;
  animStart = performance.now();
}

function frameIndex(spec, elapsedMs) {
  const n = Math.floor((elapsedMs / 1000) * spec.fps);
  return spec.loop === false ? Math.min(n, spec.frames - 1) : n % spec.frames;
}

function draw(now) {
  if (manifest) {
    const spec = manifest.animations[render.anim];
    const f = frameIndex(spec, now - animStart);
    petEl.style.backgroundPosition = `${-f * DISPLAY}px ${-spec.row * DISPLAY}px`;
  }
  requestAnimationFrame(draw);
}

listen("pet-render", (e) => {
  if (e.payload.anim !== render.anim) animStart = performance.now();
  if (e.payload.heart && !render.heart) {
    heartEl.classList.remove("show");
    void heartEl.offsetWidth; // 애니메이션 재시작
    heartEl.classList.add("show");
  }
  render = e.payload;
  petEl.classList.toggle("flip", render.flip);
});

listen("pet-change", (e) => loadPet(e.payload));

petEl.addEventListener("mousedown", (e) => {
  if (e.button === 0) invoke("pointer_down");
});

(async () => {
  await loadPet(await invoke("current_pet"));
  requestAnimationFrame(draw);
})();
```

- [ ] **Step 8: 리눅스에서 가능한 검증**
  - `cargo test` (루트, default-members = core) → 통과 (src-tauri는 빌드 안 됨)
  - `node --check ui/main.js` → 출력 없음
  - `python -c "import json;json.load(open('src-tauri/tauri.conf.json'))"` → 출력 없음

- [ ] **Step 9: 커밋** — `git add -A && git commit -m "feat(app): tauri shell with sprite renderer webview"`

---

### Task 11: 게임 루프 + 트레이 + 앱 조립

**Files:**
- Create: `src-tauri/src/game_loop.rs`, `src-tauri/src/tray.rs`
- Modify: `src-tauri/src/main.rs` (전체 교체)

**Interfaces:**
- Consumes: `Pet`, `PetConfig`, `World`, `platform::default_source`, `config::{load, save, Config}`, `pets::{load_pets, PetEntry}`
- Produces:
```rust
// game_loop.rs
pub enum Input { PointerDown, SetWalkSpeed(f64) }
pub fn spawn(app: AppHandle, rx: Receiver<Input>, walk_speed: f64);
// main.rs
pub struct AppState { pub tx: Mutex<Sender<Input>>, pub current: Mutex<String>, pub pets: Vec<PetEntry>, pub config_dir: PathBuf }
pub fn select_pet(app: &AppHandle, id: &str);
// tray.rs
pub fn build(app: &AppHandle, pets: &[PetEntry], current: &str) -> tauri::Result<()>;
```

- [ ] **Step 1: `src-tauri/src/game_loop.rs`**

```rust
use std::sync::mpsc::Receiver;
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use screenpet_core::brain::{Pet, PetConfig};
use screenpet_core::platform::{default_source, WindowSource};
use screenpet_core::world::World;
use tauri::{AppHandle, Emitter, Manager, PhysicalPosition};

const TICK: Duration = Duration::from_millis(16);
const WORLD_REFRESH: Duration = Duration::from_millis(100);
const MAX_DT: f64 = 0.05;
/// 펫 창 한 변 (논리 px) — tauri.conf.json의 width/height와 같아야 한다.
const PET_LOGICAL_SIZE: f64 = 128.0;
/// 몸 폭 절반 / 스프라이트 크기
const HALF_WIDTH_RATIO: f64 = 0.3;

pub enum Input {
    PointerDown,
    /// 동물 교체 시 걷기 속도 (논리 px/s)
    SetWalkSpeed(f64),
}

pub fn spawn(app: AppHandle, rx: Receiver<Input>, walk_speed: f64) {
    thread::spawn(move || run(app, rx, walk_speed));
}

fn refresh(source: &mut dyn WindowSource, size: f64) -> World {
    World::new(source.windows(), source.work_areas(), size)
}

fn run(app: AppHandle, rx: Receiver<Input>, walk_speed: f64) {
    let Some(window) = app.get_webview_window("pet") else { return };
    let scale = window.scale_factor().unwrap_or(1.0);
    let size = PET_LOGICAL_SIZE * scale;
    let mut source = default_source();
    let mut world = refresh(source.as_mut(), size);

    let seed = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_nanos() as u64).unwrap_or(1);
    let cfg = PetConfig { size, half_w: size * HALF_WIDTH_RATIO, walk_speed: walk_speed * scale };
    let b = world.bounds;
    let mut pet = Pet::new(cfg, b.left + b.width() / 2.0, b.top + size, seed);

    let mut last = Instant::now();
    let mut last_refresh = last;
    let mut last_pos = None;
    let mut last_render = None;

    loop {
        while let Ok(input) = rx.try_recv() {
            match input {
                Input::PointerDown => {
                    let (cx, cy) = source.cursor();
                    pet.pointer_down(cx, cy);
                }
                Input::SetWalkSpeed(speed) => pet.cfg.walk_speed = speed * scale,
            }
        }

        let now = Instant::now();
        let dt = (now - last).as_secs_f64().min(MAX_DT);
        last = now;
        if now - last_refresh >= WORLD_REFRESH {
            world = refresh(source.as_mut(), size);
            last_refresh = now;
        }

        if pet.is_held() {
            let (cx, cy) = source.cursor();
            if source.left_button_down() {
                pet.pointer_move(cx, cy);
            } else {
                pet.pointer_up(cx, cy);
            }
        }
        pet.update(dt, &world);

        let (wx, wy) = pet.window_pos();
        let pos = (wx.round() as i32, wy.round() as i32);
        if last_pos != Some(pos) {
            let _ = window.set_position(PhysicalPosition::new(pos.0, pos.1));
            last_pos = Some(pos);
        }
        let render = pet.render();
        if last_render != Some(render) {
            let _ = app.emit("pet-render", render);
            last_render = Some(render);
        }

        thread::sleep(TICK);
    }
}
```

- [ ] **Step 2: `src-tauri/src/tray.rs`**

```rust
use tauri::menu::{CheckMenuItem, Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::TrayIconBuilder;
use tauri::{AppHandle, Manager, Wry};

use crate::pets::PetEntry;

pub fn build(app: &AppHandle, pets: &[PetEntry], current: &str) -> tauri::Result<()> {
    let mut checks: Vec<(CheckMenuItem<Wry>, String)> = Vec::new();
    for p in pets {
        let item = CheckMenuItem::with_id(app, format!("pet:{}", p.id), &p.manifest.name, true, p.id == current, None::<&str>)?;
        checks.push((item, p.id.clone()));
    }
    let toggle = MenuItem::with_id(app, "toggle", "숨기기 / 보이기", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "종료", true, None::<&str>)?;

    let menu = Menu::new(app)?;
    for (item, _) in &checks {
        menu.append(item)?;
    }
    menu.append(&PredefinedMenuItem::separator(app)?)?;
    menu.append(&toggle)?;
    menu.append(&quit)?;

    TrayIconBuilder::with_id("main")
        .icon(app.default_window_icon().cloned().expect("bundle icon"))
        .tooltip("ScreenPet")
        .menu(&menu)
        .on_menu_event(move |app, event| match event.id().as_ref() {
            "quit" => app.exit(0),
            "toggle" => {
                if let Some(w) = app.get_webview_window("pet") {
                    if w.is_visible().unwrap_or(true) {
                        let _ = w.hide();
                    } else {
                        let _ = w.show();
                    }
                }
            }
            other => {
                if let Some(id) = other.strip_prefix("pet:") {
                    // 체크된 항목을 다시 누르면 체크가 풀리므로 항상 다시 맞춘다
                    for (item, pid) in &checks {
                        let _ = item.set_checked(pid == id);
                    }
                    crate::select_pet(app, id);
                }
            }
        })
        .build(app)?;
    Ok(())
}
```

- [ ] **Step 3: `src-tauri/src/main.rs` 전체 교체**

```rust
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod game_loop;
mod pets;
mod tray;

use std::path::PathBuf;
use std::sync::mpsc::{self, Sender};
use std::sync::Mutex;

use game_loop::Input;
use pets::PetEntry;
use screenpet_core::config::{self, Config};
use tauri::{AppHandle, Emitter, Manager, State};

pub struct AppState {
    pub tx: Mutex<Sender<Input>>,
    pub current: Mutex<String>,
    pub pets: Vec<PetEntry>,
    pub config_dir: PathBuf,
}

#[tauri::command]
fn pointer_down(state: State<AppState>) {
    let _ = state.tx.lock().unwrap().send(Input::PointerDown);
}

#[tauri::command]
fn current_pet(state: State<AppState>) -> String {
    state.current.lock().unwrap().clone()
}

/// 트레이에서 동물을 바꿨을 때: 저장 → 걷기 속도 반영 → 웹뷰에 새 시트 로드 요청
pub fn select_pet(app: &AppHandle, id: &str) {
    let state = app.state::<AppState>();
    let Some(entry) = state.pets.iter().find(|p| p.id == id) else { return };
    *state.current.lock().unwrap() = id.to_string();
    let _ = config::save(&state.config_dir, &Config { pet: Some(id.to_string()) });
    let _ = state.tx.lock().unwrap().send(Input::SetWalkSpeed(entry.manifest.walk_speed));
    let _ = app.emit("pet-change", id.to_string());
}

fn main() {
    let pets = pets::load_pets();
    assert!(!pets.is_empty(), "no valid pets bundled");

    tauri::Builder::default()
        .setup(move |app| {
            let config_dir = app.path().app_config_dir()?;
            let saved = config::load(&config_dir).pet;
            let current = saved
                .filter(|id| pets.iter().any(|p| &p.id == id))
                .unwrap_or_else(|| pets[0].id.clone());
            let walk_speed = pets.iter().find(|p| p.id == current).unwrap().manifest.walk_speed;

            let (tx, rx) = mpsc::channel();
            tray::build(app.handle(), &pets, &current)?;
            app.manage(AppState { tx: Mutex::new(tx), current: Mutex::new(current), pets: pets.clone(), config_dir });
            game_loop::spawn(app.handle().clone(), rx, walk_speed);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![pointer_down, current_pet])
        .run(tauri::generate_context!())
        .expect("error while running ScreenPet");
}
```

- [ ] **Step 4: 리눅스에서 가능한 검증** — `cargo test` → core 전체 통과. `src-tauri` 컴파일은 Task 12의 Windows CI가 검증한다.

- [ ] **Step 5: 커밋** — `git add -A && git commit -m "feat(app): game loop, tray menu and pet selection"`

---

### Task 12: GitHub Actions + README + 푸시

**Files:**
- Create: `.github/workflows/build.yml`, `README.md`

- [ ] **Step 1: `.github/workflows/build.yml`**

```yaml
name: build

on:
  push:
    branches: [main]
    tags: ["v*"]
  pull_request:
  workflow_dispatch:

jobs:
  test:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
        with:
          targets: x86_64-pc-windows-gnu
      - uses: Swatinem/rust-cache@v2
      - name: Core tests
        run: cargo test -p screenpet-core
      - name: Check Win32 code compiles
        run: cargo check -p screenpet-core --target x86_64-pc-windows-gnu
      - uses: actions/setup-python@v5
        with:
          python-version: "3.12"
      - name: Sprite tool tests
        run: |
          pip install pillow pytest
          cd tools && python -m pytest -q

  windows:
    needs: test
    runs-on: windows-latest
    permissions:
      contents: write
    steps:
      - uses: actions/checkout@v4
      - uses: actions/setup-node@v4
        with:
          node-version: 22
          cache: npm
      - uses: dtolnay/rust-toolchain@stable
      - uses: Swatinem/rust-cache@v2
      - run: npm ci
      - name: Build installers
        run: npm run tauri build
      - uses: actions/upload-artifact@v4
        with:
          name: screenpet-windows
          path: |
            target/release/bundle/msi/*.msi
            target/release/bundle/nsis/*.exe
      - name: Attach to release
        if: startsWith(github.ref, 'refs/tags/v')
        uses: softprops/action-gh-release@v2
        with:
          files: |
            target/release/bundle/msi/*.msi
            target/release/bundle/nsis/*.exe
```

- [ ] **Step 2: `README.md`**

````markdown
# ScreenPet 🐾

바탕화면 위를 돌아다니는 동물 친구. 작업표시줄과 창 윗라인 위를 걷고, 잡아서 던지면 창 위에 착지해요.

## 설치 (Windows)
1. GitHub → **Actions** → 최신 `build` 실행 → **Artifacts**의 `screenpet-windows` 다운로드
   (태그 `v*`를 푸시하면 **Releases**에도 올라감)
2. `.exe`(NSIS) 또는 `.msi` 실행
3. "Windows의 PC 보호" 경고가 뜨면 **추가 정보 → 실행** (코드 서명 안 된 앱이라 그래요)

## 사용법
- 트레이 아이콘(발바닥) 메뉴: 동물 선택 / 숨기기·보이기 / 종료
- 클릭: 하트 + 점프 · 자고 있으면 깨어남
- 드래그해서 던지기: 창 윗라인이나 작업표시줄에 착지

## 개발
```bash
cargo test                       # 코어 로직 (리눅스/맥/윈도우 어디서나)
cd tools && python -m pytest     # 스프라이트 도구
npm install && npm run tauri dev # 앱 실행 (Windows)
```
- 코어 로직: `crates/screenpet-core` · 앱: `src-tauri` · 화면: `ui`
- 동물 추가/교체: [docs/sprites.md](docs/sprites.md)

## 수동 테스트 체크리스트 (Windows)
- [ ] 실행하면 동물이 화면 위쪽에서 떨어져 작업표시줄 위에 착지
- [ ] 걷다가 화면 끝에서 뒤돌기
- [ ] 동물을 창 윗라인 위로 던지면 그 위에 착지
- [ ] 창 위를 걷다 모서리에서 뒤돌거나 폴짝 뛰어내림 (반반)
- [ ] 다른 창이 윗라인 일부를 가리면 그 앞에서 멈칫하고 뒤돌기
- [ ] 서 있는 자리에 다른 창을 끌어다 덮으면 놀라며 떨어짐
- [ ] 동물이 탄 창을 드래그해 옮기면 같이 이동
- [ ] 동물이 탄 창을 최소화/닫으면 떨어짐
- [ ] 휙 던지면 화면 벽에서 튕김
- [ ] 클릭하면 하트 + 점프, 자는 중 클릭하면 깸
- [ ] 트레이에서 동물 바꾸기 → 앱 재시작 후에도 유지
- [ ] 숨기기/보이기, 종료
- [ ] 작업표시줄을 위/왼쪽으로 옮겨도 바닥이 맞음
- [ ] 듀얼 모니터에서 다른 화면으로 던지기
- [ ] 디스플레이 배율 125% / 150%에서 위치가 어긋나지 않음
- [ ] 최대화된 창(윗라인이 화면 맨 위)에는 올라가지 않음
````

- [ ] **Step 3: 로컬 최종 검증**
```bash
cargo test
cargo check -p screenpet-core --target x86_64-pc-windows-gnu
(cd tools && python -m pytest -q)
node --check ui/main.js
```
Expected: 전부 통과/에러 없음.

- [ ] **Step 4: 커밋 + 푸시**
```bash
git add -A && git commit -m "ci: windows installer build and core tests; add README"
git push origin main
```

- [ ] **Step 5: CI 결과 확인** — 저장소가 비공개라 이 서버에서는 Actions 상태를 볼 수 없다(`gh` 없음, 토큰 없음). 사용자에게 Actions 탭에서 `build` 결과를 확인해 달라고 요청하고, `windows` 잡이 실패하면 로그의 에러 부분을 받아 고친다(주로 Tauri/`windows` crate API 시그니처). 고친 뒤 다시 푸시 → 성공할 때까지 반복.

- [ ] **Step 6: 사용자에게 설치 파일 다운로드 경로와 README 체크리스트로 실기 테스트를 요청한다.**
