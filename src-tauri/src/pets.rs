//! 빌드 때 포함된 동물 목록임: `build.rs`가 만든 `BUNDLED_PETS`의 `pet.json`을 읽어 [`PetEntry`]로 만듦.
//! 진입점 `load_pets`는 앱 시작 때 `main.rs`가 부름.
use screenpet_core::manifest::{parse_manifest, PetManifest};

include!(concat!(env!("OUT_DIR"), "/pets.rs"));

/// 고를 수 있는 동물 하나임
#[derive(Clone)]
pub struct PetEntry {
    /// ui/pets 아래 폴더 이름
    pub id: String,
    /// 매니페스트
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
