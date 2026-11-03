//! 눈사람의 작은 눈구름 ⛄☁️ — brain.rs의 자식 모듈 (비공개 필드에 접근).
//!
//! pet.json `snow_cloud`가 있는 동물은 몇 분에 한 번, 쉬는 박자에 머리 위로 작은 구름을 불러 10~20초 눈을 맞으며 행복하게 올려다본다.
//! 달력·날씨 토글과 상관없다. 구름 그림은 웹뷰가 `render.cloud` 동안 머리 위에 띄운다 — 창과 함께 움직인다.
//! 집 안·잡힌 동안·떠나는 길·커서에 매달린 동안엔 없다.

use super::*;

/// 처음 구름이 뜰 때까지 (앱을 켜자마자 뜨지는 않게, 그래도 금방 보이게)
pub const CLOUD_FIRST_WAIT_SECS: (f64, f64) = (60.0, 180.0);
/// 구름이 사라진 뒤 다음 구름까지
pub const CLOUD_WAIT_SECS: (f64, f64) = (180.0, 360.0);
/// 구름이 떠 있는 시간
pub const CLOUD_SECS: (f64, f64) = (10.0, 20.0);
/// 구름이 뜰 때 올려다보는 시간
const CLOUD_LOOK_SECS: (f64, f64) = (2.0, 3.5);

#[derive(Debug, Clone, Copy, Default)]
pub(super) struct SnowCloud {
    /// 다음 구름까지 남은 시간 (None: 아직 주사위를 굴리지 않음 — 구름 없는 동물의 난수 순서는 그대로)
    pub(super) wait: Option<f64>,
    left: f64,
}

impl Pet {
    /// 구름을 띄우면 안 되는 때
    fn cloud_hidden(&self) -> bool {
        self.housed || self.is_held() || self.in_transit() || self.poofing() || self.cursor_through() || matches!(self.state, State::HomeLeap { .. })
    }

    /// 지금 머리 위에 구름이 떠 있다 (웹뷰 render.cloud)
    pub(super) fn snow_cloud_on(&self) -> bool {
        self.cloud.left > 0.0 && !self.cloud_hidden()
    }

    /// 구름이 떠 있는 동안엔 더 자주 올려다본다
    pub(super) fn snow_cloud_up(&self) -> bool {
        self.cloud.left > 0.0
    }

    pub(super) fn tick_snow_cloud(&mut self, dt: f64) {
        if !self.cfg.snow_cloud {
            self.cloud = SnowCloud::default();
            return;
        }
        if self.cloud_hidden() {
            if self.cloud.left > 0.0 {
                self.end_snow_cloud();
            }
            return;
        }
        if self.cloud.left > 0.0 {
            self.cloud.left -= dt;
            if self.cloud.left <= 0.0 {
                self.end_snow_cloud();
            }
            return;
        }
        let wait = match self.cloud.wait {
            Some(w) => (w - dt).max(0.0),
            None => behavior::range(&mut self.rng, CLOUD_FIRST_WAIT_SECS),
        };
        self.cloud.wait = Some(wait);
        // 시간이 다 되면 다음 쉬는 박자를 기다린다
        if wait > 0.0 || !matches!(self.state, State::Idle { .. }) || self.standing.is_none() || self.control.is_some() {
            return;
        }
        self.cloud.left = behavior::range(&mut self.rng, CLOUD_SECS);
        if self.cfg.looks_up {
            self.gaze_up = true;
            self.state = State::Idle { remaining: behavior::range(&mut self.rng, CLOUD_LOOK_SECS), look: true };
        }
    }

    fn end_snow_cloud(&mut self) {
        self.cloud.left = 0.0;
        self.cloud.wait = Some(behavior::range(&mut self.rng, CLOUD_WAIT_SECS));
    }
}
