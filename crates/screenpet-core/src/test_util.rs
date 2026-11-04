//! 여러 테스트 파일이 같이 쓰는 도우미 (테스트 빌드에만 들어간다).

use crate::brain::PetConfig;
use crate::care::Stage;

/// ui/pets에 있는 동물 전부 (이름순). 새 동물을 더하면 테스트가 저절로 그 동물도 본다 — pet.json이 없는 폴더는 없어야 한다
pub(crate) fn species() -> Vec<String> {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../ui/pets");
    let mut out: Vec<String> = std::fs::read_dir(&dir)
        .unwrap_or_else(|e| panic!("{dir:?}: {e}"))
        .map(|e| e.unwrap().path())
        .filter(|path| path.is_dir())
        .map(|path| {
            assert!(path.join("pet.json").is_file(), "{path:?} has no pet.json");
            path.file_name().unwrap().to_string_lossy().into_owned()
        })
        .collect();
    assert!(!out.is_empty(), "no pets in {dir:?}");
    out.sort();
    out
}

/// 번들 pet.json 그대로의 설정 (게임이 쓰는 PetConfig::from_manifest, 청소년, 화면 배율 1)
pub(crate) fn bundled() -> Vec<(String, PetConfig)> {
    species()
        .into_iter()
        .map(|id| {
            let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../ui/pets").join(&id).join("pet.json");
            let json = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{path:?}: {e}"));
            let m = crate::manifest::parse_manifest(&json).unwrap_or_else(|e| panic!("{id}: {e:?}"));
            let c = PetConfig::from_manifest(&m, 1.0, m.size_mult(Stage::Teenager), Stage::Teenager.name());
            (id, c)
        })
        .collect()
}

/// `while`·`loop` 테스트의 안전장치: 회귀가 나면 테스트가 멈춰 버리지 않고 실패하게 한다.
/// 넉넉하게 시뮬레이션 10분(60fps)까지만 돈다.
pub(crate) struct LoopLimit(usize);

impl LoopLimit {
    pub(crate) fn new() -> Self {
        LoopLimit(60 * 600)
    }

    #[track_caller]
    pub(crate) fn step(&mut self) {
        assert!(self.0 > 0, "loop did not finish within the step limit");
        self.0 -= 1;
    }
}
