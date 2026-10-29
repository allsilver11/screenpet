use screenpet_core::manifest::{parse_manifest, PetManifest};

include!(concat!(env!("OUT_DIR"), "/pets.rs"));

#[derive(Clone)]
pub struct PetEntry {
    /// ui/pets 아래 폴더 이름
    pub id: String,
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
