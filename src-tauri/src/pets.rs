//! 빌드 때 포함된 동물 목록임: `build.rs`가 만든 `BUNDLED_PETS`/`BUNDLED_PIXEL_PETS`의 `pet.json`을 읽어 [`PetEntry`]로 만듦.
//! 진입점 `load_pets`는 앱 시작 때 `main.rs`가 부르고, 스타일별 그림 고르기는 `PetEntry`가 함.
use screenpet_core::manifest::{parse_manifest, parse_pixel_manifest, PetManifest};
use screenpet_core::style::PetStyle;

include!(concat!(env!("OUT_DIR"), "/pets.rs"));
include!(concat!(env!("OUT_DIR"), "/pets_pixel.rs"));

/// 고를 수 있는 동물 하나임 (동글 매니페스트와, 있으면 픽셀 그림)
#[derive(Clone)]
pub struct PetEntry {
    /// ui/pets 아래 폴더 이름
    pub id: String,
    /// 동글 매니페스트
    pub manifest: PetManifest,
    /// 픽셀 그림 (ui/pets_pixel/<id>, 동글 manifest 위에 그림만 얹은 것) — 아직 없는 동물은 None이고 픽셀 스타일에서도 동글로 보임
    pub pixel: Option<PetManifest>,
}

impl PetEntry {
    /// 원하는 스타일로 보여 줄 수 있으면 그 스타일, 그림이 없으면 동글
    pub fn shown_style(&self, wanted: PetStyle) -> PetStyle {
        if wanted == PetStyle::Pixel && self.pixel.is_some() {
            PetStyle::Pixel
        } else {
            PetStyle::Round
        }
    }

    /// 이 스타일의 그림 정보 (shown_style로 고른 값을 넘김)
    pub fn art(&self, style: PetStyle) -> &PetManifest {
        match (style, &self.pixel) {
            (PetStyle::Pixel, Some(m)) => m,
            _ => &self.manifest,
        }
    }
}

/// 빌드 시 포함된 동물 중 매니페스트가 올바른 것만.
pub fn load_pets() -> Vec<PetEntry> {
    BUNDLED_PETS
        .iter()
        .filter_map(|(id, json)| match parse_manifest(json) {
            Ok(manifest) => {
                let pixel = BUNDLED_PIXEL_PETS.iter().find(|(pid, _)| pid == id).and_then(|(_, px)| {
                    parse_pixel_manifest(px, &manifest).map_err(|e| eprintln!("skip pixel art of {id}: {e}")).ok()
                });
                Some(PetEntry { id: id.to_string(), manifest, pixel })
            }
            Err(e) => {
                eprintln!("skip pet {id}: {e}");
                None
            }
        })
        .collect()
}
