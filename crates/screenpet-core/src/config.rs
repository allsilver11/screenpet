use std::path::Path;

use serde::{Deserialize, Serialize};

const FILE: &str = "config.json";

#[derive(Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Config {
    /// 마지막으로 고른 동물 폴더 이름
    pub pet: Option<String>,
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
        let c = Config { pet: Some("rabbit".into()) };
        save(&d, &c).unwrap();
        assert_eq!(load(&d), c);
    }

    #[test]
    fn corrupt_file_gives_default() {
        let d = temp_dir("corrupt");
        std::fs::create_dir_all(&d).unwrap();
        std::fs::write(d.join("config.json"), "not json").unwrap();
        assert_eq!(load(&d), Config::default());
    }
}
