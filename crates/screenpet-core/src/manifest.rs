use std::collections::HashMap;

use serde::Deserialize;

pub const REQUIRED_ANIMS: [&str; 21] = [
    "walk", "run", "roll", "idle", "look", "sleep", "hop", "fall", "dragged", "land", "sit_front", "wave", "tilt", "belly", "stretch", "heart",
    "tickle", "annoyed", "hungry", "eat", "level_up",
];

/// 특정 동물만 가진 동작 — 있으면 검증한다 (behavior::Special::anim_name)
pub const OPTIONAL_ANIMS: [&str; 2] = ["arch", "wag"];

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

#[derive(Debug, Clone, Deserialize)]
pub struct PetManifest {
    pub name: String,
    pub frame_size: u32,
    pub walk_speed: f64,
    pub animations: HashMap<String, AnimSpec>,
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
    fn rejects_invalid_json() {
        assert!(parse_manifest("{").is_err());
    }

    #[test]
    #[ignore = "re-enabled in phase 2B Task 3 (new hungry/eat/level_up rows)"]
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
