//! 캐릭터 스타일 (동글동글 / 픽셀) — 그림만 다르다. 자라기·돌보기·친구·행동은 모두 같다 (구르기만 픽셀 그림에 없어 고르지 않는다).
//!
//! 픽셀 표시 크기 규칙: 한 칸은 언제나 **물리 px의 정수배**라서 어느 배율 모니터에서나 칸이 고르고 또렷하다.
//!   1) 칸은 언제나 논리 3 px — 전 종, 전 단계 같다 ("동물이 너무 크다"는 피드백으로 teenager·adult ×4를 ×3으로 내렸다). 픽셀 시트는 종마다 실루엣에 맞춰
//!      캔버스를 채우므로 동글 `stage_size`를 곱하지 않는다 (곱하면 칸 크기가 들쭉날쭉해진다). 표시 크기는 72 / 84 / 96 / 96 / 108,
//!      공룡만 칸 수가 적어 60 / 72 / 84 / 84 / 96 (pet.json `frame_sizes`가 정한다, 칸 크기는 같아 점이 고르다)
//!   2) 거기에 화면 배율(dpi)을 곱해 가까운 정수 물리 px로 — 125%: 3.75 → 4 / 150%: 4.5 → 5 / 200%: 6
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

/// 단계 → (한 칸의 칸 수, 기본 칸 크기 논리 px): 24×3 = 72, 28×3 = 84, 32×3 = 96, 32×3 = 96, 36×3 = 108 (tools/draw_pixel.py STAGE_GRID, ui/common.js와 같다)
pub fn pixel_grid(stage: Stage) -> (u32, u32) {
    match stage {
        Stage::Baby => (24, 3),
        Stage::Kinder => (28, 3),
        Stage::Child => (32, 3),
        Stage::Teenager => (32, 3),
        Stage::Adult => (36, 3),
    }
}

/// 칸 한 변 (논리 px, 정수): 단계 기본 배율 — 동물과 상관없다
pub fn dot_logical(stage: Stage) -> u32 {
    pixel_grid(stage).1
}

/// 칸 한 변 (물리 px, 정수): 논리 칸 × 화면 배율을 가까운 정수로 (최소 1)
pub fn dot_physical(stage: Stage, dpi: f64) -> u32 {
    let dpi = if dpi.is_finite() && dpi > 0.0 { dpi } else { 1.0 };
    ((f64::from(dot_logical(stage)) * dpi).round() as u32).max(1)
}

/// 동물 크기와 상관없이 같은 칸(동물과 같은 3 논리 px)으로 그리는 세계 그림 (손 음식·선물·물건·눈사람·침 — 동글도 크기가 고정이다)
pub const WORLD_DOT: u32 = 3;

/// 세계 그림 창 크기를 적는 단위: 호출하는 쪽의 한 변은 "칸 수 × 4" (12칸 = 48 — 동글 창 크기와 같은 숫자로 적는다)
const WORLD_SIDE_PER_DOT: f64 = 4.0;

/// 세계 그림 칸 한 변 (물리 px, 정수)
pub fn world_dot_physical(dpi: f64) -> u32 {
    let dpi = if dpi.is_finite() && dpi > 0.0 { dpi } else { 1.0 };
    ((f64::from(WORLD_DOT) * dpi).round() as u32).max(1)
}

/// 세계 그림 창의 논리 크기: side(칸 수 × 4)를 칸이 정수 물리 px인 크기로
pub fn world_side(side: f64, dpi: f64) -> f64 {
    let dpi = if dpi.is_finite() && dpi > 0.0 { dpi } else { 1.0 };
    (side / WORLD_SIDE_PER_DOT).round() * f64::from(world_dot_physical(dpi)) / dpi
}

/// 픽셀 집 그림 한 변 (칸) — 칸 크기는 그 단계 기본 칸(`pixel_grid`). 문은 시트 한 칸 전체가 들어가 동물을 줄이지 않는다 (tools/draw_pixel_world.py HOUSE_DOTS)
pub fn pixel_house_dots(stage: Stage) -> u32 {
    match stage {
        Stage::Baby => 38,
        Stage::Kinder => 42,
        Stage::Child | Stage::Teenager => 48,
        Stage::Adult => 54,
    }
}

/// 픽셀 집 그림 한 변 (논리 px, 물리 px에서 정확한 정수)
pub fn pixel_house_side(stage: Stage, dpi: f64) -> f64 {
    let dpi = if dpi.is_finite() && dpi > 0.0 { dpi } else { 1.0 };
    f64::from(pixel_house_dots(stage) * dot_physical(stage, dpi)) / dpi
}

/// 픽셀 동물의 몸 크기 배율 (`한 변 논리 px` / 128). frame_dots = 그 단계 시트의 칸 수 (pet.json).
/// 128 × 배율 × dpi = frame_dots × 칸 물리 px — 물리 px에서 정확히 정수
pub fn pixel_body_scale(frame_dots: u32, stage: Stage, dpi: f64) -> f64 {
    let dpi = if dpi.is_finite() && dpi > 0.0 { dpi } else { 1.0 };
    f64::from(frame_dots * dot_physical(stage, dpi)) / dpi / BASE_SIDE
}

/// 사용자가 정한 크기(size배)의 칸 한 변 (물리 px): 칸이 정수라 크기는 계단으로 변한다
pub fn dot_physical_at(stage: Stage, dpi: f64, size: f64) -> u32 {
    let size = if size.is_finite() && size > 0.0 { size } else { 1.0 };
    ((f64::from(dot_physical(stage, dpi)) * size).round() as u32).max(1)
}

/// 사용자가 정한 크기의 픽셀 몸 크기 배율
pub fn pixel_body_scale_at(frame_dots: u32, stage: Stage, dpi: f64, size: f64) -> f64 {
    pixel_body_scale(frame_dots, stage, dpi) * f64::from(dot_physical_at(stage, dpi, size)) / f64::from(dot_physical(stage, dpi))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn user_size_keeps_pixel_dots_whole_and_at_least_one() {
        assert_eq!(dot_physical_at(Stage::Adult, 1.0, 1.0), dot_physical(Stage::Adult, 1.0));
        for size in [0.5, 0.75, 1.0, 1.25, 1.5] {
            for dpi in [1.0, 1.25, 1.5, 2.0] {
                assert!(dot_physical_at(Stage::Baby, dpi, size) >= 1);
                let plain = pixel_body_scale(pixel_grid(Stage::Baby).0, Stage::Baby, dpi);
                let want = plain * f64::from(dot_physical_at(Stage::Baby, dpi, size)) / f64::from(dot_physical(Stage::Baby, dpi));
                assert_eq!(pixel_body_scale_at(pixel_grid(Stage::Baby).0, Stage::Baby, dpi, size), want);
            }
        }
        assert!(dot_physical_at(Stage::Adult, 2.0, 1.5) > dot_physical_at(Stage::Adult, 2.0, 1.0));
        assert!(dot_physical_at(Stage::Adult, 2.0, 0.5) < dot_physical_at(Stage::Adult, 2.0, 1.0));
    }

    #[test]
    fn default_display_sizes_at_100_percent() {
        let sides: Vec<f64> = Stage::ALL.iter().map(|&s| pixel_body_scale(pixel_grid(s).0, s, 1.0) * 128.0).collect();
        assert_eq!(sides, [72.0, 84.0, 96.0, 96.0, 108.0]);
    }

    /// 전 종 같은 칸: 표에 동물별 크기가 끼어들 자리가 없다 (동글 산타 1.65도 teenager 칸 3, 몸 96)
    #[test]
    fn every_species_gets_the_same_dot_for_its_stage() {
        assert_eq!(Stage::ALL.map(dot_logical), [3, 3, 3, 3, 3]);
    }

    #[test]
    fn every_dot_is_a_whole_number_of_physical_pixels_on_any_monitor() {
        for dpi in [1.0, 1.25, 1.5, 1.75, 2.0, 2.25, 2.5, 3.0, 0.75] {
            for stage in Stage::ALL {
                let (n, _) = pixel_grid(stage);
                let phys = 128.0 * pixel_body_scale(n, stage, dpi) * dpi;
                let dot = phys / f64::from(n);
                assert!((dot - dot.round()).abs() < 1e-9, "{stage:?} @{dpi}: {dot}");
                assert_eq!(dot.round() as u32, dot_physical(stage, dpi));
            }
        }
    }

    #[test]
    fn fractional_monitor_scale_rounds_to_the_nearest_physical_dot() {
        assert_eq!(dot_physical(Stage::Baby, 1.25), 4); // 3.75
        assert_eq!(dot_physical(Stage::Adult, 1.25), 4);
        assert_eq!(dot_physical(Stage::Baby, 1.5), 5); // 4.5
        assert_eq!(dot_physical(Stage::Adult, 1.5), 5);
        assert_eq!(dot_physical(Stage::Baby, 1.75), 5); // 5.25
        assert_eq!(dot_physical(Stage::Adult, 2.0), 6);
        assert_eq!(dot_physical(Stage::Baby, f64::NAN), 3);
    }

    #[test]
    fn world_art_and_houses_are_whole_physical_dots() {
        for dpi in [1.0, 1.25, 1.5, 1.75, 2.0, 2.5, 3.0] {
            let dot = f64::from(world_dot_physical(dpi));
            for side in [48.0, 64.0, 80.0, 96.0, 128.0] {
                let phys = world_side(side, dpi) * dpi;
                assert!((phys / dot - side / 4.0).abs() < 1e-9, "{side} @{dpi}");
            }
            for stage in Stage::ALL {
                let phys = pixel_house_side(stage, dpi) * dpi;
                assert!((phys - f64::from(pixel_house_dots(stage) * dot_physical(stage, dpi))).abs() < 1e-9);
            }
        }
        assert_eq!(world_side(48.0, 1.0), 36.0);
        assert_eq!(Stage::ALL.map(|s| pixel_house_side(s, 1.0)), [114.0, 126.0, 144.0, 144.0, 162.0]);
    }

    /// 번들 집 그림의 칸 수가 표와 같고, 문에 그 단계 시트 한 칸이 그대로 들어간다
    #[test]
    fn bundled_pixel_houses_match_the_table_and_fit_a_whole_frame() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../ui/house_pixel");
        for kind in ["house", "haunted", "xmas"] {
            for stage in Stage::ALL {
                let path = dir.join(format!("{kind}_{}.json", stage.name()));
                let json: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
                assert_eq!(json["dots"].as_u64(), Some(u64::from(pixel_house_dots(stage))), "{path:?}");
                let (n, _) = pixel_grid(stage);
                assert!(json["door"]["w"].as_u64().unwrap() >= u64::from(n), "{path:?}");
                assert!(json["door"]["h"].as_u64().unwrap() >= u64::from(n), "{path:?}");
            }
        }
    }

    /// 공룡 픽셀 시트는 칸 수만 적다 (pet.json frame_sizes): 칸은 그대로 3이고 몸은 60/72/84/84/96 px, 집 문에도 그대로 들어간다
    #[test]
    fn bundled_dinos_are_smaller_by_dots_not_by_dot_size() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../ui/pets_pixel");
        for pid in ["dino_yellow", "dino_green", "dino_purple"] {
            let json: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(dir.join(pid).join("pet.json")).unwrap()).unwrap();
            let sides = Stage::ALL.map(|s| pixel_body_scale(json["frame_sizes"][s.name()].as_u64().unwrap() as u32, s, 1.0) * 128.0);
            assert_eq!(sides, [60.0, 72.0, 84.0, 84.0, 96.0], "{pid}");
        }
    }

    /// 시트 칸 수가 표와 달라도 (pet.json이 정한다) 칸은 정수 물리 px
    #[test]
    fn body_scale_follows_the_sheets_own_frame_size() {
        assert_eq!(pixel_body_scale(40, Stage::Adult, 1.0) * 128.0, 120.0);
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
