use std::collections::HashMap;

use serde::{Deserialize, Serialize};

pub use crate::behavior::Personality;
use crate::care::Stage;

pub const REQUIRED_ANIMS: [&str; 21] = [
    "walk", "run", "roll", "idle", "look", "sleep", "hop", "fall", "dragged", "land", "sit_front", "wave", "tilt", "belly", "stretch", "heart",
    "tickle", "annoyed", "hungry", "eat", "level_up",
];

/// 특정 동물만 가진 동작 — 있으면 검증한다 (behavior::Special::anim_name, 벽 타기 climb/slide, 꿀벌 sting, 개구리 bigjump,
/// 기린 neckstretch — 있으면 구르는 대신 목을 늘인다).
/// 개구리의 폴짝(hop)은 모든 동물의 필수 동작이라 REQUIRED_ANIMS에 이미 있다
pub const OPTIONAL_ANIMS: [&str; 8] = ["arch", "wag", "huff", "climb", "slide", "sting", "bigjump", "neckstretch"];

/// 이동 방식 (pet.json `locomotion`, 없으면 walk)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Locomotion {
    /// 디딤판 위를 걷는다
    #[default]
    Walk,
    /// 날아다닌다 (꿀벌) — 쉴 때 가끔 창 윗면·바닥에 내려앉는다
    Fly,
    /// 폴짝폴짝 뛰어다닌다 (개구리) — 가끔 크게 뛰어 창 위로, 벽은 폴짝폴짝 오른다 (hop.rs)
    Hop,
}

/// 사는 집 그림 (pet.json `house`, 없으면 sting이 있는 꿀벌은 벌통·나머지는 보통 집)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HouseKind {
    /// 보통 집 (ui/house/house_<stage>)
    House,
    /// 꿀벌 벌통 (ui/house/hive_<stage>)
    Hive,
    /// 호박·유령의 귀신의 집 (ui/house/haunted_<stage>)
    Haunted,
}

impl HouseKind {
    /// 그림 이름 앞부분 (ui/house/<art>_<stage>.png)
    pub fn art(self) -> &'static str {
        match self {
            HouseKind::House => "house",
            HouseKind::Hive => "hive",
            HouseKind::Haunted => "haunted",
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct AnimSpec {
    pub row: u32,
    pub frames: u32,
    pub fps: f64,
    #[serde(rename = "loop", default = "yes")]
    pub looping: bool,
    /// 앞모습 동작 — 웹뷰는 좌우 반전하지 않는다
    #[serde(default)]
    pub front: bool,
    /// 초당 회전 각도 (roll 전용, 0이면 회전 없음)
    #[serde(default)]
    pub spin: f64,
}

fn yes() -> bool {
    true
}

/// 동물별 좋아하는 음식 (음식 고르기 버튼·트레이 메뉴에 쓴다)
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FavoriteFood {
    pub emoji: String,
    pub name: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct PetManifest {
    pub name: String,
    pub frame_size: u32,
    pub walk_speed: f64,
    pub animations: HashMap<String, AnimSpec>,
    /// 성격 (없으면 friendly)
    #[serde(default)]
    pub personality: Personality,
    /// 단계별 몸 크기 배율 (단계 이름 → 배율, 없으면 1.0)
    #[serde(default)]
    pub stage_size: HashMap<String, f64>,
    /// 좋아하는 음식 (없으면 None)
    #[serde(default)]
    pub favorite_food: Option<FavoriteFood>,
    /// 이동 방식 (없으면 walk)
    #[serde(default)]
    pub locomotion: Locomotion,
    /// 걷는 동물이 걷기·달리기 토막 중 깡총거리는 몫 0~1 (토끼 0.5, 없으면 0 — 개구리는 locomotion hop이라 늘 폴짝)
    #[serde(default)]
    pub hop_share: f64,
    /// 사는 집 (없으면 house_kind()가 sting으로 정한다)
    #[serde(default)]
    pub house: Option<HouseKind>,
}

pub fn parse_manifest(json: &str) -> Result<PetManifest, String> {
    let m: PetManifest = serde_json::from_str(json).map_err(|e| e.to_string())?;
    if m.frame_size == 0 {
        return Err("frame_size must be > 0".into());
    }
    if m.walk_speed <= 0.0 {
        return Err("walk_speed must be > 0".into());
    }
    if let Some((stage, v)) = m.stage_size.iter().find(|(_, v)| !(v.is_finite() && **v > 0.0)) {
        return Err(format!("stage_size {stage}: {v} must be finite and > 0"));
    }
    if !(0.0..=1.0).contains(&m.hop_share) {
        return Err(format!("hop_share {} must be within 0..=1", m.hop_share));
    }
    for name in REQUIRED_ANIMS {
        let spec = m.animations.get(name).ok_or_else(|| format!("missing animation: {name}"))?;
        if spec.frames == 0 || spec.fps <= 0.0 {
            return Err(format!("animation {name}: frames and fps must be > 0"));
        }
    }
    for name in OPTIONAL_ANIMS {
        if let Some(spec) = m.animations.get(name) {
            if spec.frames == 0 || spec.fps <= 0.0 {
                return Err(format!("animation {name}: frames and fps must be > 0"));
            }
        }
    }
    Ok(m)
}

impl PetManifest {
    pub fn has_anim(&self, name: &str) -> bool {
        self.animations.contains_key(name)
    }

    /// 벽 타기 (뒷모습 climb + slide 둘 다 있어야 한다)
    pub fn can_climb(&self) -> bool {
        self.has_anim("climb") && self.has_anim("slide")
    }

    pub fn flies(&self) -> bool {
        self.locomotion == Locomotion::Fly
    }

    pub fn hops(&self) -> bool {
        self.locomotion == Locomotion::Hop
    }

    /// 걷기·달리기 토막 중 깡총거리는 몫 (폴짝 동물·나는 동물은 0 — 개구리는 늘 폴짝, 꿀벌은 난다)
    pub fn hop_share(&self) -> f64 {
        if self.locomotion == Locomotion::Walk { self.hop_share } else { 0.0 }
    }

    /// 괴롭히면 화면을 쏜다 (pet.json에 sting이 있는 꿀벌)
    pub fn can_sting(&self) -> bool {
        self.has_anim("sting")
    }

    /// 사는 집 그림: pet.json `house`, 없으면 sting이 있는 동물(꿀벌)은 벌통 (예전 pet.json 호환), 나머지는 보통 집
    pub fn house_kind(&self) -> HouseKind {
        self.house.unwrap_or(if self.can_sting() { HouseKind::Hive } else { HouseKind::House })
    }

    /// 이 동물의 단계별 크기 배율 (care::Stage::scale에 곱한다)
    pub fn size_mult(&self, stage: Stage) -> f64 {
        self.stage_size.get(stage.name()).copied().unwrap_or(1.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn full_json() -> String {
        let anims: Vec<String> = REQUIRED_ANIMS
            .iter()
            .enumerate()
            .map(|(i, a)| format!(r#""{a}": {{"row": {i}, "frames": 2, "fps": 4}}"#))
            .collect();
        format!(r#"{{"name": "고양이", "frame_size": 64, "walk_speed": 60, "animations": {{{}}}}}"#, anims.join(","))
    }

    #[test]
    fn parses_valid_manifest_with_loop_default_true() {
        let m = parse_manifest(&full_json()).unwrap();
        assert_eq!(m.name, "고양이");
        assert!(m.animations["walk"].looping);
    }

    #[test]
    fn loop_false_is_respected() {
        let json = full_json().replace(r#""hop": {"row": 6, "frames": 2, "fps": 4}"#, r#""hop": {"row": 6, "frames": 2, "fps": 4, "loop": false}"#);
        assert!(!parse_manifest(&json).unwrap().animations["hop"].looping);
    }

    #[test]
    fn front_and_spin_parse_with_defaults() {
        let json = full_json().replace(r#""wave": {"row": 11, "frames": 2, "fps": 4}"#, r#""wave": {"row": 11, "frames": 2, "fps": 4, "front": true}"#)
            .replace(r#""roll": {"row": 2, "frames": 2, "fps": 4}"#, r#""roll": {"row": 2, "frames": 2, "fps": 4, "spin": 271.4}"#);
        let m = parse_manifest(&json).unwrap();
        assert!(m.animations["wave"].front);
        assert!(!m.animations["walk"].front);
        assert_eq!(m.animations["roll"].spin, 271.4);
        assert_eq!(m.animations["walk"].spin, 0.0);
    }

    #[test]
    fn rejects_missing_animation() {
        let json = full_json().replace(r#""sleep""#, r#""nap""#);
        assert!(parse_manifest(&json).unwrap_err().contains("sleep"));
    }

    #[test]
    fn rejects_zero_frames_or_bad_speed() {
        assert!(parse_manifest(&full_json().replace(r#""frames": 2"#, r#""frames": 0"#)).is_err());
        assert!(parse_manifest(&full_json().replace(r#""walk_speed": 60"#, r#""walk_speed": 0"#)).is_err());
    }

    #[test]
    fn optional_specials_are_validated_when_present() {
        let with_arch = full_json().replace(r#""animations": {"#, r#""animations": {"arch": {"row": 18, "frames": 3, "fps": 6},"#);
        assert!(parse_manifest(&with_arch).unwrap().has_anim("arch"));
        assert!(!parse_manifest(&full_json()).unwrap().has_anim("wag"));
        let bad = full_json().replace(r#""animations": {"#, r#""animations": {"wag": {"row": 18, "frames": 0, "fps": 8},"#);
        assert!(parse_manifest(&bad).is_err());
    }

    #[test]
    fn climbing_needs_both_climb_and_slide() {
        let add = |extra: &str| full_json().replace(r#""animations": {"#, &format!(r#""animations": {{{extra}"#));
        assert_eq!(OPTIONAL_ANIMS[2..5], ["huff", "climb", "slide"]);
        assert!(!parse_manifest(&full_json()).unwrap().can_climb());
        assert!(!parse_manifest(&add(r#""climb": {"row": 21, "frames": 4, "fps": 8},"#)).unwrap().can_climb());
        let both = add(r#""climb": {"row": 21, "frames": 4, "fps": 8}, "slide": {"row": 22, "frames": 2, "fps": 6},"#);
        assert!(parse_manifest(&both).unwrap().can_climb());
        assert!(parse_manifest(&add(r#""slide": {"row": 22, "frames": 0, "fps": 6},"#)).is_err());
    }

    #[test]
    fn personality_and_stage_size_default_when_missing() {
        let m = parse_manifest(&full_json()).unwrap();
        assert_eq!(m.personality, Personality::Friendly);
        assert!(m.stage_size.is_empty());
        assert_eq!(m.size_mult(Stage::Baby), 1.0);
    }

    #[test]
    fn personality_and_stage_size_parse() {
        let json = full_json().replace(
            r#""walk_speed": 60,"#,
            r#""walk_speed": 60, "personality": "aloof", "stage_size": {"baby": 0.85, "adult": 1.3},"#,
        );
        let m = parse_manifest(&json).unwrap();
        assert_eq!(m.personality, Personality::Aloof);
        assert_eq!((m.size_mult(Stage::Baby), m.size_mult(Stage::Adult), m.size_mult(Stage::Child)), (0.85, 1.3, 1.0));
        assert!(parse_manifest(&full_json().replace(r#""walk_speed": 60,"#, r#""walk_speed": 60, "personality": "grumpy","#)).is_err());
    }

    #[test]
    fn rejects_non_positive_or_non_finite_stage_size() {
        for bad in ["0", "-1.2", "1e999"] {
            let json = full_json().replace(r#""walk_speed": 60,"#, &format!(r#""walk_speed": 60, "stage_size": {{"adult": {bad}}},"#));
            assert!(parse_manifest(&json).is_err(), "{bad}");
        }
    }

    #[test]
    fn bundled_manifests_carry_personality_and_sizes() {
        let load = |id: &str| {
            let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(format!("../../ui/pets/{id}/pet.json"));
            parse_manifest(&std::fs::read_to_string(path).unwrap()).unwrap()
        };
        let (cat, dog, bear, rabbit) = (load("cat"), load("dog"), load("bear"), load("rabbit"));
        assert_eq!((cat.personality, dog.personality), (Personality::Aloof, Personality::Friendly));
        assert_eq!(bear.size_mult(Stage::Teenager), 1.2);
        assert_eq!(rabbit.size_mult(Stage::Baby), 0.85);
        assert_eq!(cat.size_mult(Stage::Adult), 1.0);
    }

    #[test]
    fn favorite_food_parses_and_defaults_to_none() {
        assert_eq!(parse_manifest(&full_json()).unwrap().favorite_food, None);
        let json = full_json().replace(r#""walk_speed": 60,"#, r#""walk_speed": 60, "favorite_food": {"emoji": "🐟", "name": "생선"},"#);
        let fav = parse_manifest(&json).unwrap().favorite_food.unwrap();
        assert_eq!(fav, FavoriteFood { emoji: "🐟".into(), name: "생선".into() });
    }

    #[test]
    fn bundled_manifests_all_have_a_favorite_food() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../ui/pets");
        for entry in std::fs::read_dir(dir).unwrap() {
            let path = entry.unwrap().path().join("pet.json");
            if path.is_file() {
                let m = parse_manifest(&std::fs::read_to_string(&path).unwrap()).unwrap();
                assert!(m.favorite_food.is_some_and(|f| !f.emoji.is_empty() && !f.name.is_empty()), "{path:?}");
            }
        }
    }

    #[test]
    fn locomotion_defaults_to_walk_and_parses_fly() {
        assert_eq!(parse_manifest(&full_json()).unwrap().locomotion, Locomotion::Walk);
        let json = full_json().replace(r#""walk_speed": 60,"#, r#""walk_speed": 60, "locomotion": "fly","#);
        assert_eq!(parse_manifest(&json).unwrap().locomotion, Locomotion::Fly);
        assert!(parse_manifest(&full_json().replace(r#""walk_speed": 60,"#, r#""walk_speed": 60, "locomotion": "swim","#)).is_err());
    }

    #[test]
    fn parses_hop_locomotion_bigjump_and_playful() {
        let json = full_json()
            .replace(r#""walk_speed": 60,"#, r#""walk_speed": 60, "locomotion": "hop", "personality": "playful","#)
            .replace(r#""animations": {"#, r#""animations": {"bigjump": {"row": 21, "frames": 4, "fps": 8, "loop": false},"#);
        let m = parse_manifest(&json).unwrap();
        assert_eq!(m.locomotion, Locomotion::Hop);
        assert!(m.hops() && !m.flies() && m.has_anim("bigjump"));
        assert_eq!(m.personality, Personality::Playful);
        assert_eq!(OPTIONAL_ANIMS[6], "bigjump");
        let bad = full_json().replace(r#""animations": {"#, r#""animations": {"bigjump": {"row": 21, "frames": 0, "fps": 8},"#);
        assert!(parse_manifest(&bad).is_err());
    }

    #[test]
    fn parses_giraffe_neckstretch() {
        let json = full_json().replace(r#""animations": {"#, r#""animations": {"neckstretch": {"row": 21, "frames": 3, "fps": 2},"#);
        let m = parse_manifest(&json).unwrap();
        assert!(m.has_anim("neckstretch"));
        assert_eq!(crate::behavior::Special::from_manifest(|n| m.has_anim(n)), Some(crate::behavior::Special::NeckStretch));
        assert_eq!(OPTIONAL_ANIMS.last(), Some(&"neckstretch"));
        let bad = full_json().replace(r#""animations": {"#, r#""animations": {"neckstretch": {"row": 21, "frames": 0, "fps": 2},"#);
        assert!(parse_manifest(&bad).is_err());
    }

    /// 개구리 (아트 에이전트가 ui/pets/frog를 만들면 그대로 읽힌다 — 없으면 건너뛴다)
    #[test]
    fn bundled_frog_hops_and_climbs_if_present() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../ui/pets/frog/pet.json");
        let Ok(json) = std::fs::read_to_string(path) else { return };
        let m = parse_manifest(&json).unwrap();
        assert_eq!(m.locomotion, Locomotion::Hop);
        assert!(m.has_anim("bigjump") && m.can_climb() && !m.can_sting());
    }

    #[test]
    fn hop_share_defaults_to_zero_and_is_bounded() {
        assert_eq!(parse_manifest(&full_json()).unwrap().hop_share(), 0.0);
        let with = |v: &str| full_json().replace(r#""walk_speed": 60,"#, &format!(r#""walk_speed": 60, "hop_share": {v},"#));
        assert_eq!(parse_manifest(&with("0.5")).unwrap().hop_share(), 0.5);
        assert!(parse_manifest(&with("1.5")).is_err() && parse_manifest(&with("-0.1")).is_err());
        // 폴짝 동물은 늘 폴짝이라 몫을 쓰지 않는다
        let frog = with("0.5").replace(r#""walk_speed": 60,"#, r#""walk_speed": 60, "locomotion": "hop","#);
        assert_eq!(parse_manifest(&frog).unwrap().hop_share(), 0.0);
    }

    /// 토끼는 걷는 시간의 절반쯤 깡총거린다
    #[test]
    fn bundled_rabbit_hops_half_the_time() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../ui/pets/rabbit/pet.json");
        let m = parse_manifest(&std::fs::read_to_string(path).unwrap()).unwrap();
        assert_eq!(m.locomotion, Locomotion::Walk);
        assert_eq!(m.hop_share(), 0.5);
        assert!(!m.hops() && !m.has_anim("bigjump"));
    }

    #[test]
    fn sting_is_an_optional_front_animation() {
        assert!(OPTIONAL_ANIMS.contains(&"sting"));
        assert!(!parse_manifest(&full_json()).unwrap().has_anim("sting"));
        let with = full_json().replace(r#""animations": {"#, r#""animations": {"sting": {"row": 21, "frames": 3, "fps": 8, "loop": false, "front": true},"#);
        let m = parse_manifest(&with).unwrap();
        assert!(m.has_anim("sting") && m.animations["sting"].front);
        let bad = full_json().replace(r#""animations": {"#, r#""animations": {"sting": {"row": 21, "frames": 0, "fps": 8},"#);
        assert!(parse_manifest(&bad).is_err());
    }

    /// 꿀벌 (아트 에이전트가 ui/pets/bee를 만들면 그대로 읽힌다 — 없으면 건너뛴다)
    #[test]
    fn bundled_bee_flies_and_stings_if_present() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../ui/pets/bee/pet.json");
        let Ok(json) = std::fs::read_to_string(path) else { return };
        let m = parse_manifest(&json).unwrap();
        assert_eq!(m.locomotion, Locomotion::Fly);
        assert!(m.has_anim("sting") && m.animations["sting"].front);
        assert!(!m.can_climb());
    }

    /// 할로윈 호박: 걷는 동물인데 걷기·달리기는 늘 통통 (hop_share 1.0), 벽 타기·큰 점프·쏘기 없음
    fn check_pumpkin(m: &PetManifest) {
        assert_eq!(m.locomotion, Locomotion::Walk);
        assert_eq!(m.hop_share(), 1.0);
        assert!(!m.flies() && !m.hops() && !m.can_climb() && !m.can_sting() && !m.has_anim("bigjump"));
    }

    /// 할로윈 유령: 날지만(locomotion fly) 쏘지 않는다 — 꽃·벌통은 꿀벌만 (can_sting)
    fn check_ghost(m: &PetManifest) {
        assert_eq!(m.locomotion, Locomotion::Fly);
        assert!(m.flies() && !m.can_sting() && !m.can_climb());
        assert!(m.walk_speed < 60.0, "ghost floats slower than the bee: {}", m.walk_speed);
    }

    #[test]
    fn halloween_manifest_fields_parse() {
        let pumpkin = full_json().replace(r#""walk_speed": 60,"#, r#""walk_speed": 60, "hop_share": 1.0, "personality": "playful", "stage_size": {"adult": 0.9},"#);
        let m = parse_manifest(&pumpkin).unwrap();
        check_pumpkin(&m);
        assert_eq!((m.personality, m.size_mult(Stage::Adult)), (Personality::Playful, 0.9));
        let ghost = full_json().replace(r#""walk_speed": 60,"#, r#""walk_speed": 40, "locomotion": "fly", "stage_size": {"adult": 0.85},"#);
        let m = parse_manifest(&ghost).unwrap();
        check_ghost(&m);
        assert_eq!(m.size_mult(Stage::Adult), 0.85);
    }

    /// 호박·유령 (아트 에이전트가 ui/pets/{pumpkin,ghost}를 만들면 그대로 읽힌다 — 없으면 건너뛴다)
    #[test]
    fn bundled_halloween_pets_if_present() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../ui/pets");
        if let Ok(json) = std::fs::read_to_string(dir.join("pumpkin/pet.json")) {
            let m = parse_manifest(&json).unwrap();
            check_pumpkin(&m);
            assert!(m.favorite_food.is_some());
        }
        if let Ok(json) = std::fs::read_to_string(dir.join("ghost/pet.json")) {
            let m = parse_manifest(&json).unwrap();
            check_ghost(&m);
            assert!(m.favorite_food.is_some());
        }
    }

    #[test]
    fn house_kind_defaults_to_normal_or_hive_for_stingers_and_parses_field() {
        assert_eq!(parse_manifest(&full_json()).unwrap().house_kind(), HouseKind::House);
        let with = |v: &str| full_json().replace(r#""walk_speed": 60,"#, &format!(r#""walk_speed": 60, "house": "{v}","#));
        assert_eq!(parse_manifest(&with("hive")).unwrap().house_kind(), HouseKind::Hive);
        assert_eq!(parse_manifest(&with("haunted")).unwrap().house_kind(), HouseKind::Haunted);
        assert_eq!(parse_manifest(&with("house")).unwrap().house_kind(), HouseKind::House);
        assert!(parse_manifest(&with("castle")).is_err());
        // 예전 pet.json (house 없음): sting이 있으면 벌통
        let stinger = full_json().replace(r#""animations": {"#, r#""animations": {"sting": {"row": 21, "frames": 3, "fps": 8},"#);
        assert_eq!(parse_manifest(&stinger).unwrap().house_kind(), HouseKind::Hive);
        // 적힌 값이 sting보다 먼저
        let haunted_stinger = stinger.replace(r#""walk_speed": 60,"#, r#""walk_speed": 60, "house": "haunted","#);
        assert_eq!(parse_manifest(&haunted_stinger).unwrap().house_kind(), HouseKind::Haunted);
        assert_eq!((HouseKind::House.art(), HouseKind::Hive.art(), HouseKind::Haunted.art()), ("house", "hive", "haunted"));
    }

    /// 번들 pet.json: 꿀벌은 벌통, 호박·유령은 귀신의 집, 나머지는 보통 집
    #[test]
    fn bundled_pets_pick_their_house() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../ui/pets");
        for (id, kind) in [("bee", HouseKind::Hive), ("pumpkin", HouseKind::Haunted), ("ghost", HouseKind::Haunted), ("cat", HouseKind::House)] {
            if let Ok(json) = std::fs::read_to_string(dir.join(id).join("pet.json")) {
                assert_eq!(parse_manifest(&json).unwrap().house_kind(), kind, "{id}");
            }
        }
    }

    #[test]
    fn rejects_invalid_json() {
        assert!(parse_manifest("{").is_err());
    }

    #[test]
    fn bundled_placeholder_manifests_are_valid() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../ui/pets");
        let mut count = 0;
        for entry in std::fs::read_dir(dir).unwrap() {
            let path = entry.unwrap().path().join("pet.json");
            if path.is_file() {
                parse_manifest(&std::fs::read_to_string(&path).unwrap()).unwrap_or_else(|e| panic!("{path:?}: {e}"));
                count += 1;
            }
        }
        assert!(count >= 4);
    }
}
