//! 펭귄의 "안경 번쩍" 👓✨ — brain.rs의 자식 모듈 (비공개 필드에 접근).
//!
//! pet.json `glasses_flash`가 있는 동물은 장난칠 때 안경이 번쩍한다: 웹뷰는 `render.flash`가 켜지는 순간 렌즈만 하얗게 번쩍이고 ✦
//! (ui/main.js PET_FX "play", 누를 때 "click"은 웹뷰가 스스로 한다). 백엔드가 켜는 때:
//!   - 커서 장난에서 커서에 닿은 순간 (brain_cursor.rs)
//!   - 친구와 놀 때 가끔: 반갑게 인사 · 술래가 되어 쫓기 시작 · 잡았다/잡혔다 (brain_social.rs)
//!   - 깨어 쉬거나 걷는 중에 몇 분에 한 번
//!
//! 잘 때·집 안·잡힌 동안·떠나는 길·사라지는 중엔 켜지 않는다. 플래그 없는 동물은 주사위를 굴리지 않는다 (난수 순서 그대로).

use super::*;

/// render.flash가 켜져 있는 시간 (웹뷰는 켜지는 순간만 본다 — 이벤트 한 번은 꼭 지나가게 몇 틱)
pub const FLASH_PULSE_SECS: f64 = 0.3;
/// 같은 번쩍이 겹치지 않게: 한 번 번쩍한 뒤 이만큼은 다시 켜지 않는다 (웹뷰 애니메이션 1.6초)
pub const FLASH_CALM_SECS: f64 = 2.5;
/// 처음 혼자 번쩍할 때까지
pub const FLASH_FIRST_WAIT_SECS: (f64, f64) = (90.0, 240.0);
/// 혼자 번쩍한 뒤 다음까지 (몇 분에 한 번)
pub const FLASH_WAIT_SECS: (f64, f64) = (150.0, 330.0);
/// 친구와 놀 때 번쩍할 확률 (인사·쫓기·잡기 한 번마다)
pub const FLASH_SOCIAL_CHANCE: f64 = 0.4;
/// 안경 반사 (render.glare — 렌즈가 하얗게 차서 눈이 안 보인다, 페이커처럼): 한 번 이만큼 이어지고 (초)
pub const GLARE_SECS: (f64, f64) = (15.0, 45.0);
/// 반사가 걷힌 뒤 다시 반사까지 (초) — 둘이 같아서 깨어 있는 시간의 절반쯤 반사
pub const CLEAR_SECS: (f64, f64) = (15.0, 45.0);

#[derive(Debug, Clone, Copy, Default)]
pub(super) struct GlassesFlash {
    /// render.flash가 켜져 있는 남은 시간
    left: f64,
    /// 다시 켤 수 있을 때까지 남은 시간
    calm: f64,
    /// 다음 혼자 번쩍까지 (None: 아직 주사위를 굴리지 않음)
    wait: Option<f64>,
    /// 지금 렌즈가 반사 중이다 (눈이 안 보인다)
    glare: bool,
    /// 반사가 켜지거나 꺼질 때까지 (None: 아직 주사위를 굴리지 않음)
    glare_left: Option<f64>,
}

#[cfg(test)]
impl GlassesFlash {
    /// 시험용: 혼자 번쩍을 한참 미룬다
    pub(super) fn hold_off(&mut self) {
        self.wait = Some(1e9);
    }

    /// 시험용: 지금부터 한참 반사
    pub(super) fn force_glare(&mut self) {
        self.glare = true;
        self.glare_left = Some(1e9);
    }
}

impl Pet {
    /// 번쩍하면 안 되는 때
    fn flash_hidden(&self) -> bool {
        matches!(self.state, State::Sleep { .. } | State::Away | State::HomeLeap { .. })
            || self.is_user_away()
            || self.housed
            || self.is_held()
            || self.in_transit()
            || self.poofing()
    }

    /// 지금 안경이 번쩍 (웹뷰 render.flash — 켜지는 순간 한 번 번쩍)
    pub(super) fn glasses_flash_on(&self) -> bool {
        self.flash.left > 0.0 && !self.flash_hidden()
    }

    /// 지금 렌즈가 하얗게 반사 중 (웹뷰 render.glare — 켜져 있는 동안 렌즈를 하얗게 채운다)
    pub(super) fn glasses_glare_on(&self) -> bool {
        self.cfg.glasses_flash && self.flash.glare && !self.flash_hidden()
    }

    /// 장난스러운 순간: 안경 번쩍 (플래그 없는 동물·숨는 때·방금 번쩍했으면 그냥 지나간다)
    pub(super) fn glasses_flash(&mut self) {
        if self.cfg.glasses_flash && self.flash.calm <= 0.0 && !self.flash_hidden() {
            self.flash.left = FLASH_PULSE_SECS;
            self.flash.calm = FLASH_CALM_SECS;
        }
    }

    /// 친구와 놀 때 가끔 번쩍 (플래그 있는 동물만 주사위를 굴린다)
    pub(super) fn maybe_glasses_flash(&mut self) {
        if self.cfg.glasses_flash && self.rng.f64() < FLASH_SOCIAL_CHANCE {
            self.glasses_flash();
        }
    }

    pub(super) fn tick_glasses_flash(&mut self, dt: f64) {
        if !self.cfg.glasses_flash {
            return;
        }
        self.flash.left = (self.flash.left - dt).max(0.0);
        self.flash.calm = (self.flash.calm - dt).max(0.0);
        // 반사는 시간으로만 켜고 끈다 (자는 동안·잡힌 동안은 보이지 않을 뿐 시계는 간다)
        let left = match self.flash.glare_left {
            Some(l) => l - dt,
            None => behavior::range(&mut self.rng, CLEAR_SECS),
        };
        self.flash.glare_left = Some(if left > 0.0 {
            left
        } else {
            self.flash.glare = !self.flash.glare;
            behavior::range(&mut self.rng, if self.flash.glare { GLARE_SECS } else { CLEAR_SECS })
        });
        let wait = match self.flash.wait {
            Some(w) => (w - dt).max(0.0),
            None => behavior::range(&mut self.rng, FLASH_FIRST_WAIT_SECS),
        };
        self.flash.wait = Some(wait);
        // 시간이 다 되면 깨어 쉬거나 걷는 박자를 기다린다
        let calm_beat = matches!(self.state, State::Idle { .. } | State::Move { kind: Move::Walk, .. }) && self.standing.is_some() && self.control.is_none();
        if wait > 0.0 || !calm_beat || self.flash_hidden() {
            return;
        }
        self.glasses_flash();
        self.flash.wait = Some(behavior::range(&mut self.rng, FLASH_WAIT_SECS));
    }
}
