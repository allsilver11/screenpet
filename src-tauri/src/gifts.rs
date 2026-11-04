//! 선물과 🎁 선물 도감 (v0.3.7) — 규칙은 screenpet_core::gift, 여기는 창과 말풍선.
//! 선물 하나 = 작은 투명 창 하나 (`prop-gift-<n>`, gift.html). 창이 그림 크기라 그 칸만 누를 수 있고 나머지 화면은 그대로 통과한다.
//! prop- 창이라 트레이 숨기기/보이기도 따른다. 선물이 없으면 1초에 한 번 시계만 본다 — 웹뷰 안에도 멈춰 있을 때 도는 애니메이션이 없다.
//! 주운 선물은 게임 루프만 설정에 적는다 (Input::CollectGift). 도감 창(`book`, book.html)은 그 복사본을 읽기만 한다.

use std::sync::Mutex;
use std::time::{Duration, Instant};

use screenpet_core::brain::Anim;
use screenpet_core::care::Care;
use screenpet_core::gift::{self, BookView, Gift, GiftBook, GiftField, GiftTimer};
use tauri::{AppHandle, Emitter, LogicalSize, Manager, PhysicalPosition, State, WebviewUrl, WebviewWindow, WebviewWindowBuilder};

use crate::game_loop::Input;
use crate::slots::{Role, Slot, OWN_LABEL};
use crate::AppState;

const PREFIX: &str = "prop-gift-";
const BOOK_LABEL: &str = "book";
const BOOK_SIZE: (f64, f64) = (470.0, 600.0);
/// 선물 창 한 변 (논리 px) — 그림(ui/gifts/*.png, tools/draw_gifts.py)과 같다
const SIZE: f64 = 40.0;
/// 픽셀 선물 창 한 변 (12칸 × 4로 적는다 — 칸이 정수 물리 px가 되게 art::fit이 실제 크기로 맞춘다, 100%에서 36 px)
const PIXEL_SIZE: f64 = 48.0;
/// gift.html의 줍기(0.55초)·사라지기(0.6초)가 끝난 뒤 닫는다
const CLOSE_AFTER: Duration = Duration::from_millis(700);
/// 시계·손님 선물을 보는 간격
const CHECK_EVERY: Duration = Duration::from_secs(1);
/// 앱이 멈췄다 돌아와도 시계는 이만큼만 더 간다
const MAX_ELAPSED: f64 = 5.0;
/// 친구 동물이 들어온 뒤 선물을 두고 가기까지 (초)
const GUEST_DROP_SECS: (f64, f64) = (12.0, 40.0);
/// 개발용 숨은 미리 보기: "fast"면 선물이 20초쯤마다 떨어진다. 메뉴에는 없다
const PREVIEW_ENV: &str = "SCREENPET_GIFTS";

/// 도감 창이 읽는 복사본 (커맨드는 게임 루프 밖에서 불린다). 쓰는 건 게임 루프뿐이다
static BOOK: Mutex<Option<GiftBook>> = Mutex::new(None);

/// 친구 동물이 들고 온 선물 (인증된 친구 링크에서, 목록 확인과 횟수 제한을 지났다 — network.rs)
pub struct GuestGift {
    pub visit_id: String,
    pub owner_id: String,
    pub owner_name: String,
    pub gift: Gift,
}

struct LiveWindow {
    token: u64,
    label: String,
    window: WebviewWindow,
    pos: (i32, i32),
    /// 창이 놓인 모니터의 배율 (내 동물이 다른 배율의 모니터에 있을 수 있다)
    dpi: f64,
}

pub struct Gifts {
    field: GiftField,
    windows: Vec<LiveWindow>,
    closing: Vec<(WebviewWindow, Instant)>,
    timer: GiftTimer,
    fast: bool,
    rng: fastrand::Rng,
    pending: Vec<(GuestGift, Instant)>,
    next_check: Instant,
    last_check: Instant,
    next_label: u64,
}

impl Gifts {
    /// 스타일이 바뀌었다: 떠 있는 선물 창을 새 크기로 (아래 가운데는 그대로). 그림은 창이 `art-style`을 듣고 바꾼다
    pub fn restyle(&mut self) {
        for w in &mut self.windows {
            let dpi = w.window.scale_factor().unwrap_or(1.0);
            let Ok(old) = w.window.inner_size() else { continue };
            let side = size(dpi);
            let (cx, by) = (f64::from(w.pos.0) + f64::from(old.width) / 2.0, f64::from(w.pos.1) + f64::from(old.height));
            w.pos = ((cx - side * dpi / 2.0).round() as i32, (by - side * dpi).round() as i32);
            let _ = w.window.set_size(LogicalSize::new(side, side));
            let _ = w.window.set_position(PhysicalPosition::new(w.pos.0, w.pos.1));
        }
    }

    pub fn new(book: &GiftBook, seed: u64) -> Self {
        publish(book);
        let mut rng = fastrand::Rng::with_seed(seed);
        let fast = std::env::var(PREVIEW_ENV).is_ok_and(|v| v == "fast");
        let timer = GiftTimer::new(&mut rng, fast);
        let now = Instant::now();
        Self { field: GiftField::default(), windows: Vec::new(), closing: Vec::new(), timer, fast, rng, pending: Vec::new(), next_check: now + CHECK_EVERY, last_check: now, next_label: 0 }
    }

    /// 선물이 앉아 있는 창들 — 사이 틱에도 경계를 읽어 부드럽게 따라간다
    pub fn ridden_windows(&self) -> impl Iterator<Item = u64> + '_ {
        self.field.ridden_windows()
    }

    /// 친구 동물이 선물을 들고 왔다 — 조금 놀다가 두고 간다 (그 전에 떠나면 없던 일)
    pub fn guest_arrived(&mut self, gift: GuestGift, now: Instant) {
        let (lo, hi) = GUEST_DROP_SECS;
        let due = now + Duration::from_secs_f64(lo + self.rng.f64() * (hi - lo));
        self.pending.push((gift, due));
    }

    /// 매 틱 (게임 루프 스레드 — 창 만들기가 메인 스레드를 기다린다)
    pub fn tick(&mut self, app: &AppHandle, now: Instant, dt: f64, slots: &[Slot], care: &Care) {
        self.closing.retain(|(w, at)| {
            if now >= *at {
                let _ = w.destroy();
                false
            } else {
                true
            }
        });
        let Some(own) = slots.first() else { return };
        if !self.field.is_empty() {
            for token in self.field.tick(dt, &own.world) {
                self.fade_out(token, now);
            }
            self.move_windows();
        }
        if now < self.next_check {
            return;
        }
        self.next_check = now + CHECK_EVERY;
        let elapsed = now.duration_since(self.last_check).as_secs_f64().min(MAX_ELAPSED);
        self.last_check = now;
        if crate::windows_hidden(app) {
            return;
        }
        self.own_drop(app, own, elapsed, care);
        self.guest_drops(app, now, slots);
    }

    /// 깨어 있는 시간이 찼으면 내 동물이 떨어뜨린다 (자리가 나고 하던 일이 끝날 때까지 기다린다)
    fn own_drop(&mut self, app: &AppHandle, own: &Slot, elapsed: f64, care: &Care) {
        let awake = own.on_screen() && own.pet.render().anim != Anim::Sleep;
        if !self.timer.tick(elapsed, awake, gift::mood_rate(care.fullness)) || !self.field.has_room() {
            return;
        }
        let Some((x, y)) = gift::drop_spot(&own.pet, &own.world) else { return };
        if let Some(gift) = Gift::roll(&own.entry.id, &mut self.rng) {
            if self.place(app, gift, None, (x, y), own.dpi) {
                let _ = app.emit_to(OWN_LABEL, "pet-say-text", gift::drop_speech(&own.entry.id, gift));
            }
        }
        self.timer.rearm(&mut self.rng, self.fast);
    }

    /// 때가 된 친구 동물이 선물을 두고 간다
    fn guest_drops(&mut self, app: &AppHandle, now: Instant, slots: &[Slot]) {
        if self.pending.is_empty() {
            return;
        }
        for (g, due) in std::mem::take(&mut self.pending) {
            // 떠났으면 없던 일
            let found = slots.iter().find(|s| matches!(&s.role, Role::Guest(i) if i.owner_id == g.owner_id && i.visit_id == g.visit_id));
            let Some(slot) = found else { continue };
            let spot = if now >= due { gift::drop_spot(&slot.pet, &slot.world) } else { None };
            let Some(spot) = spot else {
                self.pending.push((g, due));
                continue;
            };
            let dpi = slots.first().map_or(slot.dpi, |o| o.dpi);
            if self.place(app, g.gift, Some(g.owner_name), spot, dpi) {
                let _ = app.emit_to(slot.label.as_str(), "pet-say-text", gift::drop_speech(&slot.entry.id, g.gift));
            }
        }
    }

    /// 화면에 놓는다 (자리가 없으면 false)
    fn place(&mut self, app: &AppHandle, gift: Gift, from: Option<String>, (x, y): (f64, f64), dpi: f64) -> bool {
        let Some(token) = self.field.add(gift, from, x, y) else { return false };
        self.next_label += 1;
        let label = format!("{PREFIX}{}", self.next_label);
        // 떨어진 자리의 모니터 배율 (모르면 내 동물의 배율)
        let dpi = app.monitor_from_point(x, y).ok().flatten().map_or(dpi, |m| m.scale_factor());
        let pos = bottom_center(x, y, dpi);
        match spawn_window(app, &label, &gift.wire(), pos, dpi) {
            Some(window) => {
                self.windows.push(LiveWindow { token, label, window, pos, dpi });
                true
            }
            None => {
                self.field.collect(token);
                false
            }
        }
    }

    fn move_windows(&mut self) {
        for w in &mut self.windows {
            let Some(g) = self.field.get(w.token) else { continue };
            let pos = bottom_center(g.x, g.y, w.dpi);
            if pos != w.pos {
                let _ = w.window.set_position(PhysicalPosition::new(pos.0, pos.1));
                w.pos = pos;
                // 다른 모니터로 넘어갔을 수 있다 (움직일 때만 묻는다)
                w.dpi = w.window.scale_factor().unwrap_or(w.dpi);
            }
        }
    }

    fn fade_out(&mut self, token: u64, now: Instant) {
        if let Some(i) = self.windows.iter().position(|w| w.token == token) {
            let w = self.windows.remove(i);
            let _ = w.window.eval("document.body.classList.add('hide')");
            self.closing.push((w.window, now + CLOSE_AFTER));
        }
    }

    /// 선물 창을 눌렀다: 반짝이며 도감에 들어간다. 적었으면 true (부른 쪽이 설정을 저장한다).
    /// 두 번 눌러도 한 번만 센다
    pub fn collect(&mut self, app: &AppHandle, label: &str, book: &mut GiftBook, own: &Slot) -> bool {
        let Some(i) = self.windows.iter().position(|w| w.label == label) else { return false };
        let w = self.windows.remove(i);
        let collected = self.field.collect(w.token);
        let _ = w.window.eval("document.body.classList.add('collect')");
        self.closing.push((w.window, Instant::now() + CLOSE_AFTER));
        let Some((gift, from)) = collected else { return false };
        book.add(gift, from.as_deref());
        publish(book);
        let _ = app.emit_to(BOOK_LABEL, "book-state", gift::book_view(book));
        if own.on_screen() {
            let _ = app.emit_to(OWN_LABEL, "pet-say-text", gift::collect_speech(from.as_deref()));
        }
        true
    }
}

fn publish(book: &GiftBook) {
    *BOOK.lock().unwrap() = Some(book.clone());
}

/// 지금 스타일의 선물 창 한 변 (논리 px)
fn size(dpi: f64) -> f64 {
    if crate::art::is_pixel() {
        crate::art::fit((PIXEL_SIZE, PIXEL_SIZE), dpi).0
    } else {
        SIZE
    }
}

fn bottom_center(x: f64, y: f64, dpi: f64) -> (i32, i32) {
    let side = size(dpi);
    ((x - side * dpi / 2.0).round() as i32, (y - side * dpi).round() as i32)
}

/// image는 Gift::wire — 목록에 있는 id뿐이다 (친구가 보낸 글자는 Gift::parse를 지나면 이 목록의 값으로 바뀐다)
fn spawn_window(app: &AppHandle, label: &str, image: &str, pos: (i32, i32), dpi: f64) -> Option<WebviewWindow> {
    let side = size(dpi);
    let built = WebviewWindowBuilder::new(app, label, WebviewUrl::App("gift.html".into()))
        .title("ScreenPet Gift")
        .initialization_script(&format!("window.GIFT_IMAGE = {image:?}; {}", crate::art::style_script()))
        .inner_size(side, side)
        .accept_first_mouse(true)
        .transparent(true)
        .decorations(false)
        .always_on_top(true)
        .skip_taskbar(true)
        .resizable(false)
        .shadow(false)
        .focused(false)
        .visible(false)
        .build()
        .inspect(crate::overlay::float_everywhere);
    let window = match built {
        Ok(w) => w,
        Err(e) => {
            eprintln!("failed to create gift window: {e}");
            return None;
        }
    };
    let _ = window.set_size(LogicalSize::new(side, side));
    let _ = window.set_position(PhysicalPosition::new(pos.0, pos.1));
    if !crate::windows_hidden(app) {
        let _ = window.show();
    }
    Some(window)
}

/// 선물 창을 눌렀다 — 선물 창이 부른 것만 받는다
#[tauri::command]
pub fn gift_collect(window: WebviewWindow, state: State<AppState>) {
    let label = window.label();
    if label.starts_with(PREFIX) {
        state.send(Input::CollectGift { label: label.to_string() });
    }
}

/// 도감 창의 지금 내용 (첫 로드용 — 그 뒤로는 `book-state` 이벤트)
#[tauri::command]
pub fn book_state(window: WebviewWindow) -> Result<BookView, String> {
    if window.label() != BOOK_LABEL {
        return Err("wrong window".into());
    }
    Ok(gift::book_view(&BOOK.lock().unwrap().clone().unwrap_or_default()))
}

/// 메뉴 "🎁 선물 도감". 이미 열려 있으면 앞으로 가져온다
pub fn show_book(app: &AppHandle) {
    if let Some(window) = app.get_webview_window(BOOK_LABEL) {
        let _ = window.unminimize();
        let _ = window.show();
        let _ = window.set_focus();
        return;
    }
    let built = WebviewWindowBuilder::new(app, BOOK_LABEL, WebviewUrl::App("book.html".into()))
        .title("🎁 선물 도감")
        .initialization_script(crate::art::style_script())
        .inner_size(BOOK_SIZE.0, BOOK_SIZE.1)
        .resizable(true)
        .maximizable(false)
        .always_on_top(true)
        .center()
        .build();
    match built {
        Ok(window) => {
            let _ = window.set_focus();
        }
        Err(e) => eprintln!("failed to open gift book: {e}"),
    }
}
