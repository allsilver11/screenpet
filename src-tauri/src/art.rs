//! 세계 그림의 스타일 (집·문간 동물 말고 전부: 소품·손 음식·선물·도감·물건·침·날씨) — 내가 고른 캐릭터 스타일을 따른다.
//! 창을 만들 때 `style_script`로 알려 주고, 이미 떠 있는 창은 `art-style` 이벤트로 바꾼다. 픽셀이면 창 크기는 칸이 물리 px의 정수배가 되게 맞춘다.

use std::sync::atomic::{AtomicBool, Ordering};

use screenpet_core::style::{self, PetStyle};

static PIXEL: AtomicBool = AtomicBool::new(false);

pub fn set(style: PetStyle) {
    PIXEL.store(style == PetStyle::Pixel, Ordering::Relaxed);
}

pub fn is_pixel() -> bool {
    PIXEL.load(Ordering::Relaxed)
}

/// 동글이면 아무것도 넣지 않는다 (창이 예전과 똑같다)
pub fn style_script() -> &'static str {
    if is_pixel() {
        "window.ART_STYLE = \"pixel\";"
    } else {
        ""
    }
}

/// 언제나 teenager 칸(4)으로 그리는 창의 논리 크기. 동글이면 그대로
pub fn fit((w, h): (f64, f64), dpi: f64) -> (f64, f64) {
    if is_pixel() {
        (style::world_side(w, dpi), style::world_side(h, dpi))
    } else {
        (w, h)
    }
}
