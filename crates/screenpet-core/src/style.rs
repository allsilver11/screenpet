//! 캐릭터 스타일 (동글동글 / 픽셀) — 그림만 다르다. 자라기·돌보기·친구·행동은 모두 같다 (구르기만 픽셀 그림에 없어 고르지 않는다).
//!
//! 픽셀 표시 크기 규칙: 한 칸은 언제나 **물리 px의 정수배**라서 어느 배율 모니터에서나 칸이 고르고 또렷하다.
//!   1) 단계 기본 배율(논리 px, ×3 / ×4)에 동물별 크기(`stage_size`)를 곱해 가까운 정수로 — 곰 어른 4 × 1.3 = 5.2 → 5, 토끼 새끼 3 × 0.85 = 2.55 → 3
//!   2) 거기에 화면 배율(dpi)을 곱해 가까운 정수 물리 px로 — 125%: ×3 → 3.75 → 4, ×4 → 5 / 150%: ×3 → 4.5 → 5, ×4 → 6
//!   3) 몸 한 변 = 칸 수 × 칸 물리 px. 논리 크기는 그 값 / dpi (분수일 수 있다 — 물리 px에서는 정확한 정수)
//! 동글 동물은 지금까지처럼 `단계 배율 × stage_size` 그대로.

use serde::{Deserialize, Serialize};

use crate::care::Stage;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PetStyle {
    /// 동글동글 (기본)
    #[default]
    Round,
    /// 픽셀 (ui/pets_pixel)
    Pixel,
}

impl PetStyle {
    pub fn is_round(&self) -> bool {
        *self == PetStyle::Round
    }

    /// 친구가 보낸 값 (`Visit.style`): 정확히 "pixel"일 때만 픽셀, 없거나 모르는 값은 동글
    pub fn from_wire(raw: Option<&str>) -> PetStyle {
        if raw == Some("pixel") {
            PetStyle::Pixel
        } else {
            PetStyle::Round
        }
    }

    /// 친구에게 보낼 값: 동글은 싣지 않는다 (예전 버전과 메시지가 똑같다)
    pub fn to_wire(self) -> Option<String> {
        match self {
            PetStyle::Round => None,
            PetStyle::Pixel => Some("pixel".to_string()),
        }
    }
}

/// teenager(배율 1.0) 스프라이트 한 변 (논리 px) — src-tauri/src/slots.rs PET_LOGICAL_SIZE와 같다
const BASE_SIDE: f64 = 128.0;

/// 단계 → (한 칸의 칸 수, 기본 칸 크기 논리 px): 24×3 = 72, 28×3 = 84, 32×3 = 96, 32×4 = 128, 36×4 = 144 (tools/draw_pixel.py STAGE_GRID, ui/common.js와 같다)
pub fn pixel_grid(stage: Stage) -> (u32, u32) {
    match stage {
        Stage::Baby => (24, 3),
        Stage::Kinder => (28, 3),
        Stage::Child => (32, 3),
        Stage::Teenager => (32, 4),
        Stage::Adult => (36, 4),
    }
}

/// 칸 한 변 (논리 px, 정수): 단계 기본 배율 × 동물별 크기를 가까운 정수로 (최소 1)
pub fn dot_logical(stage: Stage, size_mult: f64) -> u32 {
    let mult = if size_mult.is_finite() && size_mult > 0.0 { size_mult } else { 1.0 };
    ((f64::from(pixel_grid(stage).1) * mult).round() as u32).max(1)
}

/// 칸 한 변 (물리 px, 정수): 논리 칸 × 화면 배율을 가까운 정수로 (최소 1)
pub fn dot_physical(stage: Stage, size_mult: f64, dpi: f64) -> u32 {
    let dpi = if dpi.is_finite() && dpi > 0.0 { dpi } else { 1.0 };
    ((f64::from(dot_logical(stage, size_mult)) * dpi).round() as u32).max(1)
}

/// 픽셀 동물의 몸 크기 배율 (`한 변 논리 px` / 128). frame_dots = 그 단계 시트의 칸 수 (pet.json).
/// 128 × 배율 × dpi = frame_dots × 칸 물리 px — 물리 px에서 정확히 정수
pub fn pixel_body_scale(frame_dots: u32, stage: Stage, size_mult: f64, dpi: f64) -> f64 {
    let dpi = if dpi.is_finite() && dpi > 0.0 { dpi } else { 1.0 };
    f64::from(frame_dots * dot_physical(stage, size_mult, dpi)) / dpi / BASE_SIDE
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_display_sizes_at_100_percent() {
        let sides: Vec<f64> = Stage::ALL.iter().map(|&s| pixel_body_scale(pixel_grid(s).0, s, 1.0, 1.0) * 128.0).collect();
        assert_eq!(sides, [72.0, 84.0, 96.0, 128.0, 144.0]);
    }

    #[test]
    fn species_size_snaps_to_a_whole_logical_dot() {
        // 곰: 어른 1.3 → 5.2 → 5, 사춘기 1.2 → 4.8 → 5, 어린이 1.1 → 3.3 → 3
        assert_eq!(dot_logical(Stage::Adult, 1.3), 5);
        assert_eq!(dot_logical(Stage::Teenager, 1.2), 5);
        assert_eq!(dot_logical(Stage::Child, 1.1), 3);
        // 토끼 새끼 0.85 → 2.55 → 3 (기본 그대로), 아주 작아도 1칸은 남는다
        assert_eq!(dot_logical(Stage::Baby, 0.85), 3);
        assert_eq!(dot_logical(Stage::Baby, 0.01), 1);
        assert_eq!(dot_logical(Stage::Baby, f64::NAN), 3);
    }

    #[test]
    fn every_dot_is_a_whole_number_of_physical_pixels_on_any_monitor() {
        for dpi in [1.0, 1.25, 1.5, 1.75, 2.0, 2.25, 2.5, 3.0, 0.75] {
            for stage in Stage::ALL {
                for mult in [0.85, 1.0, 1.1, 1.2, 1.3] {
                    let (n, _) = pixel_grid(stage);
                    let phys = 128.0 * pixel_body_scale(n, stage, mult, dpi) * dpi;
                    let dot = phys / f64::from(n);
                    assert!((dot - dot.round()).abs() < 1e-9, "{stage:?} {mult} @{dpi}: {dot}");
                    assert_eq!(dot.round() as u32, dot_physical(stage, mult, dpi));
                }
            }
        }
    }

    #[test]
    fn fractional_monitor_scale_rounds_to_the_nearest_physical_dot() {
        assert_eq!(dot_physical(Stage::Baby, 1.0, 1.25), 4); // 3.75
        assert_eq!(dot_physical(Stage::Adult, 1.0, 1.25), 5);
        assert_eq!(dot_physical(Stage::Baby, 1.0, 1.5), 5); // 4.5
        assert_eq!(dot_physical(Stage::Adult, 1.0, 1.5), 6);
        assert_eq!(dot_physical(Stage::Baby, 1.0, 1.75), 5); // 5.25
        assert_eq!(dot_physical(Stage::Adult, 1.0, 2.0), 8);
        assert_eq!(dot_physical(Stage::Baby, 1.0, f64::NAN), 3);
    }

    /// 시트 칸 수가 표와 달라도 (pet.json이 정한다) 칸은 정수 물리 px
    #[test]
    fn body_scale_follows_the_sheets_own_frame_size() {
        assert_eq!(pixel_body_scale(40, Stage::Adult, 1.0, 1.0) * 128.0, 160.0);
    }

    #[test]
    fn wire_value_is_validated_and_round_is_not_sent() {
        assert_eq!(PetStyle::from_wire(Some("pixel")), PetStyle::Pixel);
        for bad in [None, Some("round"), Some("PIXEL"), Some(""), Some("../../etc")] {
            assert_eq!(PetStyle::from_wire(bad), PetStyle::Round, "{bad:?}");
        }
        assert_eq!(PetStyle::Round.to_wire(), None);
        assert_eq!(PetStyle::Pixel.to_wire().as_deref(), Some("pixel"));
    }
}
