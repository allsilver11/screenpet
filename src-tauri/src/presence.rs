//! 곁에 있기와 하루 리듬의 앱 쪽 (v0.3.7): 1초에 한 번 지역 시각과 유휴 시간을 읽어 코어(screenpet_core::presence)에 넘기고,
//! 수면 모자·집중 고리를 웹뷰에 알림. 게임 루프는 매 틱 tick을 부르지만 그 사이엔 시각 비교 한 번뿐임.
//! 🐾🐾🐾 내 펫이 여럿이면 모두 한 번에 (잠·자리 비움·집중 — 코어 Rhythm::tick_all).

use std::time::{Duration, Instant};

use chrono::{Datelike, Timelike};
use screenpet_core::brain::Pet;
use screenpet_core::own_pets::OwnPet;
use screenpet_core::platform::WindowSource;
use screenpet_core::presence::{DailyMarks, FocusLength, LocalMoment, Overlay, Rhythm, RhythmInput, RhythmPet};
use tauri::{AppHandle, Emitter, Manager};

use crate::house;
use crate::slots::Slot;
use crate::AppState;

/// 지역 시각과 유휴 시간을 읽는 주기
const EVERY: Duration = Duration::from_secs(1);
/// 첫 알림은 웹뷰가 듣기 전에 지나갔을 수 있음 — 달라지지 않아도 이따금 다시 보냄
const RESEND_EVERY: Duration = Duration::from_secs(10);

/// 곁에 있기·하루 리듬 상태임 (게임 루프가 가짐)
pub struct Presence {
    rhythm: Rhythm,
    start: Instant,
    next: Instant,
    last: Option<Overlay>,
    last_sent: Instant,
    /// 📱 방금 읽은 (초, 유휴 시간) — 게임 루프가 꺼내 네트워크에 넘김 (어느 컴퓨터로 놀지)
    sample: Option<(f64, Option<f64>)>,
}

impl Presence {
    /// 집중 메뉴를 꺼진 상태로 맞추고 새로 만듦
    pub fn new(app: &AppHandle, length: FocusLength) -> Self {
        crate::tray::sync_focus(&app.state::<AppState>().menu, false, length);
        let now = Instant::now();
        Self { rhythm: Rhythm::default(), start: now, next: now, last: None, last_sent: now, sample: None }
    }

    /// 사용자가 자리를 비웠음 (1초에 한 번 tick이 유휴 시간을 본 결과)
    pub fn is_away(&self) -> bool {
        self.rhythm.is_away()
    }

    /// 1초에 한 번 읽은 (초, 유휴 시간), 새것이 있으면
    pub fn take_sample(&mut self) -> Option<(f64, Option<f64>)> {
        self.sample.take()
    }

    /// 시작 뒤 지난 초
    fn secs(&self) -> f64 {
        self.start.elapsed().as_secs_f64()
    }

    /// 집중 상태를 트레이 메뉴에 맞추고 다음 틱에 고리를 바로 다시 보내게 함
    fn after_menu(&mut self, app: &AppHandle, length: FocusLength) {
        crate::tray::sync_focus(&app.state::<AppState>().menu, self.rhythm.focus().is_some(), length);
        // 고리를 바로 보이거나 치움
        self.next = Instant::now();
    }

    /// 메뉴 "집중 시작" — 켜고 끄기 (🐾🐾🐾 내 펫 모두 한 번에)
    pub fn toggle(&mut self, app: &AppHandle, length: FocusLength, own: &mut [Slot]) {
        let mut pets: Vec<&mut Pet> = own.iter_mut().map(|s| &mut s.pet).collect();
        if self.rhythm.focus().is_some() {
            self.rhythm.stop_focus_all(&mut pets);
        } else {
            self.rhythm.start_focus_all(length, self.secs(), &mut pets);
        }
        self.after_menu(app, length);
    }

    /// 메뉴 길이 — 켜져 있으면 새 길이로 다시 시작 (같은 길이를 다시 눌렀으면 하던 대로)
    pub fn set_length(&mut self, app: &AppHandle, length: FocusLength, own: &mut [Slot]) {
        if self.rhythm.focus().is_some_and(|f| f.length != length) {
            let mut pets: Vec<&mut Pet> = own.iter_mut().map(|s| &mut s.pet).collect();
            self.rhythm.start_focus_all(length, self.secs(), &mut pets);
        }
        self.after_menu(app, length);
    }

    /// 게임 루프 스레드에서 매 틱. own = 내 펫 슬롯들, pets = 설정의 펫 목록 (돌보기 — uid로 짝지음). DailyMarks가 바뀌었으면 true (설정에 저장)
    pub fn tick(&mut self, app: &AppHandle, now: Instant, source: &mut dyn WindowSource, own: &mut [Slot], pets: &mut [OwnPet], marks: &mut DailyMarks) -> bool {
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
        let mut cares: std::collections::HashMap<&str, &mut screenpet_core::care::Care> = pets.iter_mut().map(|p| (p.uid.as_str(), &mut p.care)).collect();
        let mut list: Vec<RhythmPet<'_>> = Vec::new();
        for s in own.iter_mut() {
            let Slot { pet, world, role, .. } = s;
            let crate::slots::Role::Own(uid) = role else { continue };
            if let Some(care) = cares.remove(uid.as_str()) {
                list.push(RhythmPet { pet, world, care });
            }
        }
        let out = self.rhythm.tick_all(&input, &mut list, marks);
        if out.house_wave {
            let _ = app.emit_to(house::LABEL, "house-wave", ());
        }
        let overlay = self.rhythm.overlay(&input);
        if self.last != Some(overlay) || now - self.last_sent >= RESEND_EVERY {
            self.last = Some(overlay);
            self.last_sent = now;
            for s in own.iter() {
                let _ = app.emit_to(s.label.as_str(), "pet-presence", overlay);
            }
        }
        out.marks_changed
    }
}
