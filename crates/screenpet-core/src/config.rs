use std::collections::HashMap;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::care::Care;
use crate::food::FeedMode;
use crate::visit::{ExitEdges, Side};

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
    /// 하우스 안에 있음 (동물이 아니라 설치마다 하나) — 다음 실행도 하우스 안에서 시작
    #[serde(default)]
    pub housed: bool,
    /// 이 설치의 고유 id (친구 찾기 알림·친구 목록의 이름표). 처음 실행 때 만든다
    #[serde(default)]
    pub install_id: Option<String>,
    /// 친구 목록 (짝맞추기로 고정한 공개키와 연결된 화면 끝). 잘못된 항목 하나는 건너뛴다
    #[serde(default, deserialize_with = "lenient_friends")]
    pub friends: Vec<Friend>,
    /// 하우스를 끌어다 놓은 자리. 없으면 (또는 그 모니터가 없어졌으면) 주 모니터 오른쪽 아래 구석
    #[serde(default, deserialize_with = "lenient_house_spot")]
    pub house_spot: Option<HouseSpot>,
    /// 먹이 주는 방법 (직접 주기 / 자동 주기). 모르는 값이면 직접 주기
    #[serde(default, deserialize_with = "lenient_feed_mode")]
    pub feed_mode: FeedMode,
    /// 친구들에게 보이는 내 이름 (친구 화면의 내 동물 이름표). 없으면 컴퓨터 사용자 이름
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub owner_name: Option<String>,
}

/// 내가 정하는 이름(내 이름·친구 별명)의 최대 글자 수
pub const MAX_NAME_CHARS: usize = 20;

/// 내가 정하는 이름: 제어 문자를 빼고 앞뒤 공백을 지운 뒤 20자까지. 비면 None (= 기본 이름으로)
pub fn sanitize_name(raw: &str) -> Option<String> {
    let clean: String = raw.chars().filter(|c| !c.is_control()).collect();
    let name: String = clean.trim().chars().take(MAX_NAME_CHARS).collect();
    let name = name.trim_end();
    (!name.is_empty()).then(|| name.to_string())
}

fn lenient_feed_mode<'de, D: serde::Deserializer<'de>>(d: D) -> Result<FeedMode, D::Error> {
    let v = serde_json::Value::deserialize(d)?;
    Ok(serde_json::from_value(v).unwrap_or_default())
}

/// 친구: id(이름표) + 고정된 정적 공개키(신원) + 표시 이름 + 내 화면의 어느 끝에 연결됐는지
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Friend {
    pub id: String,
    pub name: String,
    #[serde(with = "hex32")]
    pub public_key: [u8; 32],
    #[serde(with = "SideDef")]
    pub side: Side,
    /// 내가 붙인 별명 (그 친구 동물의 이름표·메뉴에 이름 대신). 없으면 친구가 정한 이름
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub nickname: Option<String>,
}

impl Friend {
    /// 화면·메뉴에 보일 이름: 내가 붙인 별명, 없으면 친구가 정한 이름
    pub fn shown_name(&self) -> &str {
        self.nickname.as_deref().filter(|n| !n.is_empty()).unwrap_or(&self.name)
    }
}

/// 친구 목록을 읽되 잘못된 항목은 (로그만 남기고) 건너뛴다 — 하나 때문에 설정 전체를 버리지 않는다
fn lenient_friends<'de, D: serde::Deserializer<'de>>(d: D) -> Result<Vec<Friend>, D::Error> {
    let items = Vec::<serde_json::Value>::deserialize(d)?;
    Ok(items
        .into_iter()
        .filter_map(|v| match serde_json::from_value::<Friend>(v) {
            Ok(f) => Some(f),
            Err(e) => {
                eprintln!("skipping a malformed friend in config: {e}");
                None
            }
        })
        .collect())
}

#[derive(Serialize, Deserialize)]
#[serde(remote = "Side", rename_all = "snake_case")]
enum SideDef {
    Left,
    Right,
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

    /// 설치 id가 없으면 만든다. 바뀌었으면 true (저장해야 한다)
    pub fn ensure_install_id(&mut self, new_id: impl FnOnce() -> String) -> bool {
        if self.install_id.as_deref().is_some_and(|id| !id.is_empty()) {
            return false;
        }
        self.install_id = Some(new_id());
        true
    }

    pub fn friend(&self, id: &str) -> Option<&Friend> {
        self.friends.iter().find(|f| f.id == id)
    }

    /// 새 친구가 붙을 끝: 친구가 적은 쪽, 같으면 오른쪽 (첫 친구 오른쪽, 두 번째 왼쪽, 그 뒤 번갈아)
    fn next_side(&self) -> Side {
        let left = self.friends.iter().filter(|f| f.side == Side::Left).count();
        if left < self.friends.len() - left {
            Side::Left
        } else {
            Side::Right
        }
    }

    /// 친구를 더하거나 이름·키를 고친다 (끝은 그대로). 같은 키를 가진 다른 id는 지운다. 바뀌었으면 true
    pub fn upsert_friend(&mut self, id: &str, name: &str, public_key: [u8; 32]) -> bool {
        let before = self.friends.len();
        self.friends.retain(|f| f.id == id || f.public_key != public_key);
        let mut changed = self.friends.len() != before;
        match self.friends.iter_mut().find(|f| f.id == id) {
            Some(f) => {
                if f.name != name || f.public_key != public_key {
                    f.name = name.to_string();
                    f.public_key = public_key;
                    changed = true;
                }
            }
            None => {
                let side = self.next_side();
                self.friends.push(Friend { id: id.to_string(), name: name.to_string(), public_key, side, nickname: None });
                changed = true;
            }
        }
        changed
    }

    /// 친구에게 별명을 붙인다 (비면 지운다 — 친구가 정한 이름으로). 바뀌었으면 true
    pub fn set_friend_nickname(&mut self, id: &str, raw: &str) -> bool {
        let nickname = sanitize_name(raw);
        match self.friends.iter_mut().find(|f| f.id == id) {
            Some(f) if f.nickname != nickname => {
                f.nickname = nickname;
                true
            }
            _ => false,
        }
    }

    /// 친구들에게 보일 내 이름을 정한다 (비면 지운다 — 컴퓨터 사용자 이름으로). 바뀌었으면 true
    pub fn set_owner_name(&mut self, raw: &str) -> bool {
        let name = sanitize_name(raw);
        let changed = self.owner_name != name;
        self.owner_name = name;
        changed
    }

    /// 친구들에게 보일 내 이름: 정한 이름, 없으면 system (컴퓨터 사용자 이름)
    pub fn my_name(&self, system: &str) -> String {
        self.owner_name.as_deref().and_then(sanitize_name).unwrap_or_else(|| system.to_string())
    }

    pub fn remove_friend(&mut self, id: &str) -> bool {
        let before = self.friends.len();
        self.friends.retain(|f| f.id != id);
        self.friends.len() != before
    }

    pub fn set_friend_side(&mut self, id: &str, side: Side) -> bool {
        match self.friends.iter_mut().find(|f| f.id == id) {
            Some(f) if f.side != side => {
                f.side = side;
                true
            }
            _ => false,
        }
    }

    /// 지금 연결된(connected) 친구가 있는 화면 끝
    pub fn exit_edges(&self, connected: impl Fn(&str) -> bool) -> ExitEdges {
        let on = |side| self.friends.iter().any(|f| f.side == side && connected(&f.id));
        ExitEdges { left: on(Side::Left), right: on(Side::Right) }
    }

    /// side 끝에 있는 연결된 친구들
    pub fn friends_at(&self, side: Side, connected: impl Fn(&str) -> bool) -> Vec<&Friend> {
        self.friends.iter().filter(|f| f.side == side && connected(&f.id)).collect()
    }
}

// ---- 하우스 자리 (끌어서 옮긴 집) ----

/// 하우스를 놓은 자리: 어느 모니터의 바닥 어디쯤 (y는 늘 그 모니터 작업 영역의 바닥)
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HouseSpot {
    /// OS가 주는 모니터 이름 (없으면 빈 문자열)
    #[serde(default)]
    pub monitor: String,
    /// 그 모니터의 왼쪽 위 (물리 px) — 이름이 바뀌었을 때 찾는다
    pub monitor_pos: (i32, i32),
    /// 하우스 창 왼쪽 = 그 작업 영역 왼쪽 + offset (물리 px)
    pub offset: f64,
}

impl HouseSpot {
    /// 저장된 모니터를 monitors (이름, 왼쪽 위) 중에서 찾는다: 이름·자리가 모두 맞는 것, 이름이 맞는 것, 자리가 맞는 것 순
    pub fn pick(&self, monitors: &[(&str, (i32, i32))]) -> Option<usize> {
        let named = |name: &str| !self.monitor.is_empty() && name == self.monitor;
        monitors
            .iter()
            .position(|&(n, p)| named(n) && p == self.monitor_pos)
            .or_else(|| monitors.iter().position(|&(n, _)| named(n)))
            .or_else(|| monitors.iter().position(|&(_, p)| p == self.monitor_pos))
    }

    /// 작업 영역 [wa_left, wa_right) 위의 하우스 창 왼쪽 (창 너비 width, 모두 물리 px) — 영역 밖으로 나가지 않는다
    pub fn left_in(&self, wa_left: f64, wa_right: f64, width: f64) -> f64 {
        clamp_house_left(wa_left + self.offset, width, wa_left, wa_right)
    }
}

/// 하우스 창 왼쪽을 작업 영역 안으로 (창이 영역보다 넓으면 왼쪽에 붙인다)
pub fn clamp_house_left(left: f64, width: f64, wa_left: f64, wa_right: f64) -> f64 {
    left.min(wa_right - width).max(wa_left)
}

/// 하우스 자리가 깨졌으면 (로그만 남기고) 구석으로 — 설정 전체를 버리지 않는다
fn lenient_house_spot<'de, D: serde::Deserializer<'de>>(d: D) -> Result<Option<HouseSpot>, D::Error> {
    let v = Option::<serde_json::Value>::deserialize(d)?;
    Ok(v.and_then(|v| match serde_json::from_value::<HouseSpot>(v) {
        Ok(s) => Some(s),
        Err(e) => {
            eprintln!("ignoring a malformed house spot in config: {e}");
            None
        }
    }))
}

/// 32바이트 키를 소문자 hex 64자로
mod hex32 {
    use serde::{de::Error, Deserialize, Deserializer, Serializer};

    pub fn encode(k: &[u8; 32]) -> String {
        k.iter().map(|b| format!("{b:02x}")).collect()
    }

    pub fn decode(s: &str) -> Option<[u8; 32]> {
        if s.len() != 64 || !s.is_ascii() {
            return None;
        }
        let mut k = [0u8; 32];
        for (i, b) in k.iter_mut().enumerate() {
            *b = u8::from_str_radix(&s[2 * i..2 * i + 2], 16).ok()?;
        }
        Some(k)
    }

    pub fn serialize<S: Serializer>(k: &[u8; 32], s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&encode(k))
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<[u8; 32], D::Error> {
        let s = String::deserialize(d)?;
        decode(&s).ok_or_else(|| D::Error::custom("expected 64 hex digits"))
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

/// 장기 X25519 비밀키 파일 (config.json과 따로 — 설정 파일은 비밀이 아닌 것만 갖는다)
const KEY_FILE: &str = "identity.key";
const KEY_TMP_FILE: &str = "identity.key.tmp";

/// 이 설치의 비밀키(친구들이 고정한 신원)를 config 폴더의 `identity.key`(hex 64자)에서 읽는다.
/// 없거나 내용이 깨졌으면 new_key()로 만들어 저장한다 (깨진 파일은 덮어쓴다 — 그러면 친구들과 다시 짝맞추기가 필요하다).
/// 그 밖의 읽기 에러(권한 등)는 그대로 Err — 멀쩡할 수 있는 키를 덮어쓰지 않는다 (그 실행에서는 친구 기능을 끈다).
///
/// 권한: Unix에서는 폴더를 0o700, 파일을 0o600으로 만든다. Windows에서는 config 폴더(%APPDATA% 아래)가
/// 기본으로 그 사용자 전용 ACL을 가지므로 보통 방식으로 만든다.
pub fn load_or_create_identity_key(dir: &Path, new_key: impl FnOnce() -> [u8; 32]) -> std::io::Result<[u8; 32]> {
    let path = dir.join(KEY_FILE);
    match std::fs::read(&path) {
        Ok(bytes) => {
            if let Some(k) = std::str::from_utf8(&bytes).ok().and_then(|t| hex32::decode(t.trim())) {
                return Ok(k);
            }
            eprintln!("identity key {} is corrupt; making a new one", path.display());
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Err(e) => return Err(e),
    }
    let key = new_key();
    create_private_dir(dir)?;
    let tmp = dir.join(KEY_TMP_FILE);
    let _ = std::fs::remove_file(&tmp);
    {
        use std::io::Write;
        let mut f = private_file(&tmp)?;
        f.write_all(hex32::encode(&key).as_bytes())?;
        f.sync_all()?;
    }
    std::fs::rename(&tmp, &path)?;
    Ok(key)
}

#[cfg(unix)]
fn create_private_dir(dir: &Path) -> std::io::Result<()> {
    use std::os::unix::fs::{DirBuilderExt, PermissionsExt};
    std::fs::DirBuilder::new().recursive(true).mode(0o700).create(dir)?;
    std::fs::set_permissions(dir, std::fs::Permissions::from_mode(0o700))
}

#[cfg(not(unix))]
fn create_private_dir(dir: &Path) -> std::io::Result<()> {
    // Windows: %APPDATA% 아래 폴더는 기본으로 그 사용자 전용 ACL을 물려받는다
    std::fs::create_dir_all(dir)
}

#[cfg(unix)]
fn private_file(path: &Path) -> std::io::Result<std::fs::File> {
    use std::os::unix::fs::OpenOptionsExt;
    std::fs::OpenOptions::new().write(true).create_new(true).mode(0o600).open(path)
}

#[cfg(not(unix))]
fn private_file(path: &Path) -> std::io::Result<std::fs::File> {
    // Windows: 사용자 전용 ACL을 가진 config 폴더 안에 보통 방식으로 만든다
    std::fs::OpenOptions::new().write(true).create_new(true).open(path)
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
    fn housed_roundtrips_and_defaults_to_false() {
        let d = temp_dir("housed");
        std::fs::create_dir_all(&d).unwrap();
        std::fs::write(d.join("config.json"), r#"{"pet": "cat", "care": {}}"#).unwrap();
        assert!(!load(&d).housed);
        let c = Config { pet: Some("cat".into()), housed: true, ..Config::default() };
        save(&d, &c).unwrap();
        assert!(load(&d).housed);
    }

    fn key(b: u8) -> [u8; 32] {
        [b; 32]
    }

    #[test]
    fn install_id_is_created_once_and_kept() {
        let mut c = Config::default();
        assert!(c.ensure_install_id(|| "abc".into()));
        assert_eq!(c.install_id.as_deref(), Some("abc"));
        // 이미 있으면 새로 만들지 않는다
        assert!(!c.ensure_install_id(|| panic!("no new id")));
        // 빈 id는 없는 것으로 본다
        c.install_id = Some(String::new());
        assert!(c.ensure_install_id(|| "def".into()));
        assert_eq!(c.install_id.as_deref(), Some("def"));
    }

    #[test]
    fn friends_roundtrip_with_hex_public_keys() {
        let d = temp_dir("friends");
        let mut c = Config { install_id: Some("me".into()), ..Config::default() };
        c.upsert_friend("f1", "민지", key(1));
        save(&d, &c).unwrap();
        let text = std::fs::read_to_string(d.join("config.json")).unwrap();
        assert!(text.contains(&"01".repeat(32)) && text.contains(r#""side": "right""#), "{text}");
        assert_eq!(load(&d), c);
    }

    #[test]
    fn identity_key_is_created_once_in_its_own_file() {
        let d = temp_dir("idkey");
        let k = load_or_create_identity_key(&d, || key(0xab)).unwrap();
        assert_eq!(k, key(0xab));
        assert_eq!(std::fs::read_to_string(d.join("identity.key")).unwrap(), "ab".repeat(32));
        assert_eq!(load_or_create_identity_key(&d, || panic!("no new key")).unwrap(), k);
        assert!(!d.join("identity.key.tmp").exists());
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = |p: &std::path::Path| std::fs::metadata(p).unwrap().permissions().mode() & 0o777;
            assert_eq!(mode(&d.join("identity.key")), 0o600);
            assert_eq!(mode(&d), 0o700);
        }
        // 깨진 파일은 새 키로 바꾼다
        std::fs::write(d.join("identity.key"), "zz").unwrap();
        assert_eq!(load_or_create_identity_key(&d, || key(3)).unwrap(), key(3));
        assert_eq!(load_or_create_identity_key(&d, || panic!("no new key")).unwrap(), key(3));
    }

    #[test]
    fn config_json_never_contains_the_private_key() {
        let d = temp_dir("nosecret");
        let secret = load_or_create_identity_key(&d, || key(0x5c)).unwrap();
        let mut c = Config { install_id: Some("me".into()), pet: Some("cat".into()), ..Config::default() };
        c.upsert_friend("f1", "민지", key(1));
        save(&d, &c).unwrap();
        let text = std::fs::read_to_string(d.join("config.json")).unwrap();
        assert!(!text.contains(&super::hex32::encode(&secret)), "{text}");
        assert!(!text.contains("static_key") && !text.contains(&"5c".repeat(4)), "{text}");
    }

    #[test]
    fn old_config_without_network_fields_loads() {
        let d = temp_dir("oldnet");
        std::fs::create_dir_all(&d).unwrap();
        std::fs::write(d.join("config.json"), r#"{"pet": "cat", "care": {}, "housed": false}"#).unwrap();
        let c = load(&d);
        assert_eq!((c.install_id, c.friends.len()), (None, 0));
    }

    #[test]
    fn a_malformed_friend_is_skipped_not_the_whole_config() {
        let d = temp_dir("badkey");
        std::fs::create_dir_all(&d).unwrap();
        let good = format!(r#"{{"id": "b", "name": "B", "public_key": "{}", "side": "right"}}"#, "02".repeat(32));
        let text = format!(
            r#"{{"pet": "cat", "housed": true, "friends": [{{"id": "a", "name": "A", "public_key": "zz", "side": "left"}}, {good}, {{"id": 5}}, "x"]}}"#
        );
        std::fs::write(d.join("config.json"), text).unwrap();
        let c = load(&d);
        assert_eq!((c.pet.as_deref(), c.housed), (Some("cat"), true));
        assert_eq!(c.friends.len(), 1);
        assert_eq!((c.friends[0].id.as_str(), c.friends[0].public_key), ("b", key(2)));
        assert!(!d.join("config.json.bak").exists());
        // friends가 배열이 아니면 그건 깨진 설정이다
        std::fs::write(d.join("config.json"), r#"{"pet": "cat", "friends": 3}"#).unwrap();
        assert_eq!(load(&d), Config::default());
    }

    #[test]
    fn identity_key_io_errors_other_than_missing_do_not_replace_the_key() {
        let d = temp_dir("idkeyerr");
        // identity.key가 폴더라 읽을 수 없다 (없는 것도, 깨진 것도 아님) — 새로 만들지 않고 에러
        std::fs::create_dir_all(d.join("identity.key")).unwrap();
        assert!(load_or_create_identity_key(&d, || panic!("must not generate")).is_err());
        assert!(d.join("identity.key").is_dir());
    }

    #[test]
    fn friend_sides_start_right_then_left_then_alternate() {
        let mut c = Config::default();
        for (i, id) in ["a", "b", "c", "d"].iter().enumerate() {
            c.upsert_friend(id, id, key(i as u8));
        }
        let sides: Vec<Side> = c.friends.iter().map(|f| f.side).collect();
        assert_eq!(sides, [Side::Right, Side::Left, Side::Right, Side::Left]);
        // 빠진 쪽을 먼저 채운다
        c.remove_friend("b");
        c.upsert_friend("e", "e", key(9));
        assert_eq!(c.friend("e").unwrap().side, Side::Left);
    }

    #[test]
    fn upsert_updates_name_and_key_but_keeps_side() {
        let mut c = Config::default();
        assert!(c.upsert_friend("a", "A", key(1)));
        c.upsert_friend("b", "B", key(2));
        assert!(c.set_friend_side("a", Side::Left));
        assert!(!c.set_friend_side("a", Side::Left));
        assert!(!c.set_friend_side("zz", Side::Left));
        assert!(c.upsert_friend("a", "A2", key(3)));
        assert!(!c.upsert_friend("a", "A2", key(3)));
        let a = c.friend("a").unwrap();
        assert_eq!((a.name.as_str(), a.public_key, a.side), ("A2", key(3), Side::Left));
        // 같은 키를 다른 id가 가져가면 예전 id는 지운다 (net과 같은 규칙)
        c.upsert_friend("c", "C", key(2));
        assert!(c.friend("b").is_none());
        assert_eq!(c.friends.len(), 2);
        assert!(c.remove_friend("a") && !c.remove_friend("a"));
    }

    #[test]
    fn exit_edges_need_a_connected_friend_on_that_side() {
        let mut c = Config::default();
        c.upsert_friend("r", "R", key(1));
        c.upsert_friend("l", "L", key(2));
        let none = |_: &str| false;
        assert_eq!(c.exit_edges(none), ExitEdges::default());
        assert_eq!(c.exit_edges(|id| id == "r"), ExitEdges { left: false, right: true });
        assert_eq!(c.exit_edges(|_| true), ExitEdges { left: true, right: true });
        c.set_friend_side("l", Side::Right);
        assert_eq!(c.exit_edges(|_| true), ExitEdges { left: false, right: true });
        let ids: Vec<&str> = c.friends_at(Side::Right, |id| id == "l").iter().map(|f| f.id.as_str()).collect();
        assert_eq!(ids, ["l"]);
        assert!(c.friends_at(Side::Left, |_| true).is_empty());
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

    // ---- 하우스 자리 (끌어서 옮긴 집) ----

    fn spot(monitor: &str, pos: (i32, i32), offset: f64) -> HouseSpot {
        HouseSpot { monitor: monitor.into(), monitor_pos: pos, offset }
    }

    #[test]
    fn feed_mode_defaults_to_hand_and_roundtrips() {
        use crate::food::FeedMode;
        let old: Config = serde_json::from_str(r#"{"pet":"cat"}"#).unwrap();
        assert_eq!(old.feed_mode, FeedMode::Hand);
        let bad: Config = serde_json::from_str(r#"{"pet":"cat","feed_mode":"teleport"}"#).unwrap();
        assert_eq!((bad.pet.as_deref(), bad.feed_mode), (Some("cat"), FeedMode::Hand));
        let dir = temp_dir("feedmode");
        let c = Config { pet: Some("dog".into()), feed_mode: FeedMode::Auto, ..Config::default() };
        save(&dir, &c).unwrap();
        assert_eq!(load(&dir).feed_mode, FeedMode::Auto);
    }

    #[test]
    fn house_spot_defaults_to_none_and_roundtrips() {
        let d = temp_dir("housespot");
        std::fs::create_dir_all(&d).unwrap();
        std::fs::write(d.join("config.json"), r#"{"pet": "cat", "care": {}, "housed": true}"#).unwrap();
        assert_eq!(load(&d).house_spot, None);
        let c = Config { house_spot: Some(spot(r"\\.\DISPLAY2", (-1920, 0), 300.0)), ..Config::default() };
        save(&d, &c).unwrap();
        assert_eq!(load(&d), c);
    }

    #[test]
    fn a_malformed_house_spot_is_dropped_not_the_whole_config() {
        let d = temp_dir("badspot");
        std::fs::create_dir_all(&d).unwrap();
        std::fs::write(d.join("config.json"), r#"{"pet": "cat", "housed": true, "house_spot": {"offset": "far"}}"#).unwrap();
        let c = load(&d);
        assert_eq!((c.pet.as_deref(), c.housed, c.house_spot), (Some("cat"), true, None));
    }

    #[test]
    fn house_left_is_clamped_inside_the_work_area() {
        // 작업 영역 [0, 1000), 창 너비 100
        assert_eq!(clamp_house_left(400.0, 100.0, 0.0, 1000.0), 400.0);
        assert_eq!(clamp_house_left(-50.0, 100.0, 0.0, 1000.0), 0.0);
        assert_eq!(clamp_house_left(950.0, 100.0, 0.0, 1000.0), 900.0);
        // 다른 모니터 (왼쪽, 음수 좌표)
        assert_eq!(clamp_house_left(-100.0, 100.0, -1920.0, 0.0), -100.0);
        assert_eq!(clamp_house_left(10.0, 100.0, -1920.0, 0.0), -100.0);
        // 창보다 좁은 작업 영역이면 왼쪽에 붙인다
        assert_eq!(clamp_house_left(30.0, 100.0, 0.0, 50.0), 0.0);
    }

    #[test]
    fn house_spot_left_is_offset_from_the_work_area_and_clamped() {
        let s = spot("A", (0, 0), 300.0);
        assert_eq!(s.left_in(1920.0, 3840.0, 128.0), 2220.0);
        let far = spot("A", (0, 0), 5000.0);
        assert_eq!(far.left_in(0.0, 1920.0, 128.0), 1792.0);
    }

    #[test]
    fn house_spot_finds_its_monitor_by_name_and_position() {
        let monitors = [("A", (0, 0)), ("B", (1920, 0)), ("C", (-1920, 0))];
        // 이름과 자리가 모두 맞는 모니터
        assert_eq!(spot("B", (1920, 0), 0.0).pick(&monitors), Some(1));
        // 자리가 바뀌었어도 이름이 같으면 그 모니터
        assert_eq!(spot("C", (-2560, 0), 0.0).pick(&monitors), Some(2));
        // 이름이 바뀌었으면 자리로
        assert_eq!(spot("X", (1920, 0), 0.0).pick(&monitors), Some(1));
        // 이름이 같은 모니터가 둘이면 자리까지 맞는 쪽
        let twins = [("D", (0, 0)), ("D", (1920, 0))];
        assert_eq!(spot("D", (1920, 0), 0.0).pick(&twins), Some(1));
        // 이름 없는 모니터는 이름으로 찾지 않는다
        assert_eq!(spot("", (5, 5), 0.0).pick(&[("", (0, 0))]), None);
        // 없어진 모니터
        assert_eq!(spot("Z", (9999, 0), 0.0).pick(&monitors), None);
    }

    // ---- 내 이름·친구 별명 ----

    #[test]
    fn names_are_trimmed_stripped_of_control_chars_and_capped_at_20() {
        assert_eq!(sanitize_name("  danni \n"), Some("danni".into()));
        assert_eq!(sanitize_name("da\tn\u{7}ni"), Some("danni".into()));
        assert_eq!(sanitize_name(&"가".repeat(30)), Some("가".repeat(MAX_NAME_CHARS)));
        assert_eq!(sanitize_name(&"🐱".repeat(25)).map(|n| n.chars().count()), Some(20));
        // 잘라낸 뒤 끝 공백도 뺀다
        assert_eq!(sanitize_name(&format!("{} x", "a".repeat(19))), Some("a".repeat(19)));
        assert_eq!(sanitize_name(""), None);
        assert_eq!(sanitize_name(" \n\t "), None);
    }

    #[test]
    fn owner_name_overrides_the_system_name_and_empty_resets() {
        let mut c = Config::default();
        assert_eq!(c.my_name("winuser"), "winuser");
        assert!(c.set_owner_name("  danni  "));
        assert_eq!(c.owner_name.as_deref(), Some("danni"));
        assert_eq!(c.my_name("winuser"), "danni");
        assert!(!c.set_owner_name("danni"), "unchanged");
        assert!(c.set_owner_name("   "));
        assert_eq!(c.owner_name, None);
        assert_eq!(c.my_name("winuser"), "winuser");
        // 손으로 고친 설정 파일의 긴 이름도 화면엔 20자까지만
        c.owner_name = Some("b".repeat(50));
        assert_eq!(c.my_name("winuser"), "b".repeat(20));
    }

    #[test]
    fn friend_nickname_takes_precedence_and_clearing_falls_back() {
        let mut c = Config::default();
        c.upsert_friend("a", "Ann", key(1));
        assert_eq!(c.friend("a").unwrap().shown_name(), "Ann");
        assert!(c.set_friend_nickname("a", " 앤 언니 "));
        assert_eq!(c.friend("a").unwrap().shown_name(), "앤 언니");
        // 친구가 이름을 바꿔도 내 별명이 그대로 보인다
        assert!(c.upsert_friend("a", "Annie", key(1)));
        assert_eq!(c.friend("a").unwrap().name, "Annie");
        assert_eq!(c.friend("a").unwrap().shown_name(), "앤 언니");
        assert!(c.set_friend_nickname("a", ""));
        assert_eq!(c.friend("a").unwrap().nickname, None);
        assert_eq!(c.friend("a").unwrap().shown_name(), "Annie");
        assert!(!c.set_friend_nickname("a", ""), "unchanged");
        assert!(!c.set_friend_nickname("nobody", "x"));
    }

    #[test]
    fn owner_name_and_nicknames_roundtrip_and_old_configs_load() {
        let d = temp_dir("names");
        std::fs::create_dir_all(&d).unwrap();
        let k = "01".repeat(32);
        std::fs::write(d.join("config.json"), format!(r#"{{"friends":[{{"id":"a","name":"Ann","public_key":"{k}","side":"left"}}]}}"#)).unwrap();
        let mut c = load(&d);
        assert_eq!((c.owner_name.as_deref(), c.friends[0].nickname.as_deref()), (None, None));
        c.set_owner_name("danni");
        c.set_friend_nickname("a", "앤");
        save(&d, &c).unwrap();
        assert_eq!(load(&d), c);
        // 별명이 없으면 파일에 적지 않는다 (예전 판과 같은 모양)
        c.set_friend_nickname("a", "");
        save(&d, &c).unwrap();
        assert!(!std::fs::read_to_string(d.join("config.json")).unwrap().contains("nickname"));
    }
}
