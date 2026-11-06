//! 곁에 있기와 하루 리듬의 앱 쪽 (v0.3.7): 1초에 한 번 지역 시각과 유휴 시간을 읽어 코어(screenpet_core::presence)에 넘기고,
//! 수면 모자·집중 고리를 웹뷰에 알린다. 게임 루프는 매 틱 tick을 부르지만 그 사이엔 시각 비교 한 번뿐이다.

use std::time::{Duration, Instant};

use chrono::{Datelike, Timelike};
use screenpet_core::brain::Pet;
use screenpet_core::care::Care;
use screenpet_core::platform::WindowSource;
use screenpet_core::presence::{DailyMarks, FocusLength, LocalMoment, Overlay, Rhythm, RhythmInput};
use tauri::{AppHandle, Emitter, Manager};

use crate::house;
use crate::slots::{Slot, OWN_LABEL};
use crate::AppState;

const EVERY: Duration = Duration::from_secs(1);
/// 첫 알림은 웹뷰가 듣기 전에 지나갔을 수 있다 — 달라지지 않아도 이따금 다시 보낸다
const RESEND_EVERY: Duration = Duration::from_secs(10);

pub struct Presence {
    rhythm: Rhythm,
    start: Instant,
    next: Instant,
    last: Option<Overlay>,
    last_sent: Instant,
    /// 📱 방금 읽은 (초, 유휴 시간) — 게임 루프가 꺼내 네트워크에 넘긴다 (어느 컴퓨터로 놀지)
    sample: Option<(f64, Option<f64>)>,
}

impl Presence {
    pub fn new(app: &AppHandle, length: FocusLength) -> Self {
        crate::tray::sync_focus(&app.state::<AppState>().menu, false, length);
        let now = Instant::now();
        Self { rhythm: Rhythm::default(), start: now, next: now, last: None, last_sent: now, sample: None }
    }

    /// 사용자가 자리를 비웠다 (1초에 한 번 tick이 유휴 시간을 본 결과)
    pub fn is_away(&self) -> bool {
        self.rhythm.is_away()
    }

    /// 1초에 한 번 읽은 (초, 유휴 시간), 새것이 있으면
    pub fn take_sample(&mut self) -> Option<(f64, Option<f64>)> {
        self.sample.take()
    }

    fn secs(&self) -> f64 {
        self.start.elapsed().as_secs_f64()
    }

    fn after_menu(&mut self, app: &AppHandle, length: FocusLength) {
        crate::tray::sync_focus(&app.state::<AppState>().menu, self.rhythm.focus().is_some(), length);
        // 고리를 바로 보이거나 치운다
        self.next = Instant::now();
    }

    /// 메뉴 "집중 시작" — 켜고 끄기
    pub fn toggle(&mut self, app: &AppHandle, length: FocusLength, pet: &mut Pet) {
        if self.rhythm.focus().is_some() {
            self.rhythm.stop_focus(pet);
        } else {
            self.rhythm.start_focus(length, self.secs(), pet);
        }
        self.after_menu(app, length);
    }

    /// 메뉴 길이 — 켜져 있으면 새 길이로 다시 시작 (같은 길이를 다시 눌렀으면 하던 대로)
    pub fn set_length(&mut self, app: &AppHandle, length: FocusLength, pet: &mut Pet) {
        if self.rhythm.focus().is_some_and(|f| f.length != length) {
            self.rhythm.start_focus(length, self.secs(), pet);
        }
        self.after_menu(app, length);
    }

    /// 게임 루프 스레드에서 매 틱. DailyMarks가 바뀌었으면 true (설정에 저장)
    pub fn tick(&mut self, app: &AppHandle, now: Instant, source: &mut dyn WindowSource, own: &mut Slot, care: &mut Care, marks: &mut DailyMarks) -> bool {
        if now < self.next {
            return false;
        }
        self.next = now + EVERY;
        let local = chrono::Local::now();
        let input = RhythmInput {
            secs: self.secs(),
            moment: LocalMoment::new(local.num_days_from_ce(), local.hour(), local.minute()),
            idle: source.idle_seconds(),
            visible: !crate::windows_hidden(app),
        };
        self.sample = Some((input.secs, input.idle));
        let out = self.rhythm.tick(&input, &mut own.pet, &own.world, care, marks);
        if out.house_wave {
            let _ = app.emit_to(house::LABEL, "house-wave", ());
        }
        let overlay = self.rhythm.overlay(&input);
        if self.last != Some(overlay) || now - self.last_sent >= RESEND_EVERY {
            self.last = Some(overlay);
            self.last_sent = now;
            let _ = app.emit_to(OWN_LABEL, "pet-presence", overlay);
        }
        out.marks_changed
    }
}
