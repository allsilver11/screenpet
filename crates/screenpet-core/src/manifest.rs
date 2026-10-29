use std::collections::HashMap;

use serde::{Deserialize, Serialize};

pub use crate::behavior::Personality;
use crate::care::Stage;

pub const REQUIRED_ANIMS: [&str; 21] = [
    "walk", "run", "roll", "idle", "look", "sleep", "hop", "fall", "dragged", "land", "sit_front", "wave", "tilt", "belly", "stretch", "heart",
    "tickle", "annoyed", "hungry", "eat", "level_up",
];

/// 특정 동물만 가진 동작 — 있으면 검증한다 (behavior::Special::anim_name, 벽 타기 climb/slide)
pub const OPTIONAL_ANIMS: [&str; 5] = ["arch", "wag", "huff", "climb", "slide"];

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
}

pub fn parse_manifest(json: &str) -> Result<PetManifest, String> {
    let m: PetManifest = serde_json::from_str(json).map_err(|e| e.to_string())?;
    if m.frame_size == 0 {
        return Err("frame_size must be > 0".into());
    }
    if m.walk_speed <= 0.0 {
        return Err("walk_speed must be > 0".into());
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
        assert!(OPTIONAL_ANIMS.ends_with(&["huff", "climb", "slide"]));
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
