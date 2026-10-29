use std::collections::HashMap;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::care::Care;

const FILE: &str = "config.json";
const TMP_FILE: &str = "config.json.tmp";
const BAK_FILE: &str = "config.json.bak";

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

/// 설정을 읽는다. 없으면 기본값. 깨진 파일은 `config.json.bak`으로 남겨 두고 기본값으로 시작한다.
pub fn load(dir: &Path) -> Config {
    let path = dir.join(FILE);
    let Ok(text) = std::fs::read_to_string(&path) else { return Config::default() };
    match serde_json::from_str(&text) {
        Ok(c) => c,
        Err(e) => {
            let bak = dir.join(BAK_FILE);
            match std::fs::copy(&path, &bak) {
                Ok(_) => eprintln!("config {} is corrupt ({e}); kept a copy at {}, starting fresh", path.display(), bak.display()),
                Err(be) => eprintln!("config {} is corrupt ({e}); backup failed ({be}), starting fresh", path.display()),
            }
            Config::default()
        }
    }
}

/// 임시 파일에 다 쓴 뒤 이름을 바꿔 덮어쓴다 — 쓰다 꺼져도 이전 설정이 남는다
pub fn save(dir: &Path, c: &Config) -> std::io::Result<()> {
    std::fs::create_dir_all(dir)?;
    let tmp = dir.join(TMP_FILE);
    std::fs::write(&tmp, serde_json::to_string_pretty(c)?)?;
    std::fs::rename(&tmp, dir.join(FILE))
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
        assert_eq!(std::fs::read_to_string(d.join("config.json.bak")).unwrap(), "not json");
    }

    #[test]
    fn corrupt_backup_keeps_the_latest_bad_file() {
        let d = temp_dir("corrupt2");
        std::fs::create_dir_all(&d).unwrap();
        std::fs::write(d.join("config.json"), "bad 1").unwrap();
        load(&d);
        std::fs::write(d.join("config.json"), "bad 2").unwrap();
        load(&d);
        assert_eq!(std::fs::read_to_string(d.join("config.json.bak")).unwrap(), "bad 2");
    }

    #[test]
    fn save_is_atomic_and_leaves_no_temp_file() {
        let d = temp_dir("atomic");
        save(&d, &Config { pet: Some("cat".into()), ..Config::default() }).unwrap();
        let c = Config { pet: Some("dog".into()), ..Config::default() };
        save(&d, &c).unwrap();
        assert!(!d.join("config.json.tmp").exists());
        assert_eq!(load(&d), c);
        assert!(!d.join("config.json.bak").exists());
    }
}
