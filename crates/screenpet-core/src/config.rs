use std::collections::HashMap;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::care::Care;

const FILE: &str = "config.json";

#[derive(Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Config {
    /// 마지막으로 고른 동물 폴더 이름
    pub pet: Option<String>,
    /// 동물별 돌보기 상태 (없으면 새끼부터)
    #[serde(default)]
    pub care: HashMap<String, Care>,
}

impl Config {
    /// 없어진 동물을 다른 동물로 옮긴다 (선택과 돌보기 상태). 대상에 이미 기록이 있으면 그쪽을 남긴다.
    pub fn rename_pet(&mut self, from: &str, to: &str) {
        if self.pet.as_deref() == Some(from) {
            self.pet = Some(to.to_string());
        }
        if let Some(care) = self.care.remove(from) {
            self.care.entry(to.to_string()).or_insert(care);
        }
    }
}

pub fn load(dir: &Path) -> Config {
    std::fs::read_to_string(dir.join(FILE))
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

pub fn save(dir: &Path, c: &Config) -> std::io::Result<()> {
    std::fs::create_dir_all(dir)?;
    std::fs::write(dir.join(FILE), serde_json::to_string_pretty(c)?)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(name: &str) -> std::path::PathBuf {
        let d = std::env::temp_dir().join(format!("screenpet-test-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        d
    }

    #[test]
    fn missing_file_gives_default() {
        assert_eq!(load(&temp_dir("missing")), Config::default());
    }

    #[test]
    fn save_then_load_roundtrip() {
        let d = temp_dir("roundtrip");
        let c = Config { pet: Some("rabbit".into()), ..Config::default() };
        save(&d, &c).unwrap();
        assert_eq!(load(&d), c);
    }

    #[test]
    fn old_config_without_care_still_loads() {
        let d = temp_dir("oldcfg");
        std::fs::create_dir_all(&d).unwrap();
        std::fs::write(d.join("config.json"), r#"{"pet": "cat"}"#).unwrap();
        let c = load(&d);
        assert_eq!(c.pet.as_deref(), Some("cat"));
        assert!(c.care.is_empty());
    }

    #[test]
    fn care_per_pet_roundtrips() {
        use crate::care::{Care, Stage};
        let d = temp_dir("care");
        let mut c = Config { pet: Some("dog".into()), ..Config::default() };
        c.care.insert("dog".into(), Care { stage: Stage::Child, stamina: 12.0, fullness: 0.3, locked: true });
        save(&d, &c).unwrap();
        assert_eq!(load(&d), c);
    }

    #[test]
    fn rename_pet_moves_selection_and_care() {
        use crate::care::{Care, Stage};
        let child = Care { stage: Stage::Child, ..Care::default() };
        let mut c = Config { pet: Some("beaver".into()), ..Config::default() };
        c.care.insert("beaver".into(), child.clone());
        c.rename_pet("beaver", "bear");
        assert_eq!(c.pet.as_deref(), Some("bear"));
        assert_eq!(c.care.get("bear"), Some(&child));
        assert!(!c.care.contains_key("beaver"));
    }

    #[test]
    fn rename_pet_keeps_existing_target_care_and_other_selection() {
        use crate::care::{Care, Stage};
        let adult = Care { stage: Stage::Adult, ..Care::default() };
        let mut c = Config { pet: Some("cat".into()), ..Config::default() };
        c.care.insert("beaver".into(), Care::default());
        c.care.insert("bear".into(), adult.clone());
        c.rename_pet("beaver", "bear");
        assert_eq!(c.pet.as_deref(), Some("cat"));
        assert_eq!(c.care.get("bear"), Some(&adult));
        assert!(!c.care.contains_key("beaver"));
    }

    #[test]
    fn corrupt_file_gives_default() {
        let d = temp_dir("corrupt");
        std::fs::create_dir_all(&d).unwrap();
        std::fs::write(d.join("config.json"), "not json").unwrap();
        assert_eq!(load(&d), Config::default());
    }
}
