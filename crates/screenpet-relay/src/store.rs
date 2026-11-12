//! 릴레이의 저장 데이터임 (데이터 폴더의 `relay.json`): 설치(정적 공개키가 키)마다 스스로 밝힌 설치 id,
//! 닿고 싶은 친구 키들, 지금 친구 코드. 메시지 내용은 절대 저장하지 않음. 릴레이 자신의 정적 키(`relay.key`)도 다룸.
//! 허브가 `State::store`로 들고, 쓰기는 잠금 밖 `write_snapshot`으로 함.

use crate::proto::{self, new_code};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

/// 데이터 파일 이름
const DATA_FILE: &str = "relay.json";
/// 릴레이 키 파일 이름
const KEY_FILE: &str = "relay.key";
/// 이만큼 안 보인 설치는 잊음
const FORGET_AFTER_SECS: u64 = 180 * 24 * 3600;
/// 친구를 둔 적도 코드 사용에 낀 적도 없는 설치는 이만큼 안 보이면 잊음 (등록으로 저장소를 영영 못 채우게)
pub const IDLE_FORGET_SECS: u64 = 24 * 3600;
/// `ESTABLISHED_DAYS`일보다 적게 보인 친구 있는 설치는 이만큼 안 보이면 잊음: 서로만 친구 맺은 키가 반년을 못 삼.
/// 잊혀도 진짜 설치는 손해가 적음: 다음 로그인 때 같은 친구 목록으로 다시 등록됨
const NEW_FRIEND_FORGET_SECS: u64 = 14 * 24 * 3600;
/// 이만큼 서로 다른 날에 보이면 자리 잡은 설치로 봄
const ESTABLISHED_DAYS: u32 = 7;
/// 하루 (초)
const DAY_SECS: u64 = 24 * 3600;
/// `last_seen`만으로는 설치당 최대 이 주기로만 저장소를 더럽힘
const TOUCH_EVERY_SECS: u64 = 600;
/// 저장하는 설치 수 기본 상한
pub const MAX_INSTALLS: usize = 20_000;

/// 지금 유닉스 초를 돌려줌 (시계가 이상하면 0)
pub fn unix_now() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

/// 옛 파일의 `seen_days` 기본값: 자리 잡은 것으로 시작함
fn established_days() -> u32 {
    ESTABLISHED_DAYS
}

/// 저장된 설치 하나임
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Install {
    /// 스스로 밝힌 설치 id 라벨
    pub id: String,
    /// 닿고 싶은 친구 키들
    #[serde(default)]
    pub friends: Vec<String>,
    /// 지금 친구 코드
    pub code: String,
    /// `code`를 준 때 (유닉스 초)
    pub code_at: u64,
    /// 마지막으로 본 때 (유닉스 초)
    #[serde(default)]
    pub last_seen: u64,
    /// 코드를 썼거나 자기 코드가 쓰였음 (서로 친구 있는 설치처럼 둠)
    #[serde(default)]
    pub active: bool,
    /// 이 설치가 보인 서로 다른 날 수. 이 수를 세기 전 파일은 진짜 설치를 담았으므로 자리 잡은 것으로 시작함
    #[serde(default = "established_days")]
    pub seen_days: u32,
}



/// `relay.json` 내용임
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Data {
    /// 설치 키 → 설치
    #[serde(default)]
    installs: HashMap<String, Install>,
}

/// 허브 잠금 밖에서 쓸 데이터 사본임 ([`write_snapshot`] 참고)
pub struct Snapshot {
    dir: PathBuf,
    data: Data,
}

/// 설치 저장소임: 데이터, 코드 색인, 저장할 게 있는지
pub struct Store {
    /// 데이터 폴더 (None이면 메모리만)
    dir: Option<PathBuf>,
    /// 데이터
    data: Data,
    /// 코드 → 설치 키
    by_code: HashMap<String, String>,
    /// 저장할 게 있음
    dirty: bool,
    /// 저장하는 설치 수 딱딱한 상한 (차면 새 키를 거절함)
    pub max_installs: usize,
}

/// 데이터 파일을 읽음. 없거나 찢어졌으면 None임
fn read_data(path: &Path) -> Option<Data> {
    let bytes = std::fs::read(path).ok()?;
    match serde_json::from_slice(&bytes) {
        Ok(d) => Some(d),
        Err(e) => {
            let file = path.file_name().map(|f| f.to_string_lossy().into_owned()).unwrap_or_default();
            screenpet_log::log!(Error, "store", "load_fail", "file" => file, "err" => e);
            None
        }
    }
}

/// `dir`을 만듦 (unix면 0700)
fn private_dir(dir: &Path) -> io::Result<()> {
    std::fs::create_dir_all(dir)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(dir, std::fs::Permissions::from_mode(0o700))?;
    }
    Ok(())
}

impl Store {
    /// 메모리에만 있는 저장소를 만듦 (시험)
    pub fn memory() -> Store {
        Store { dir: None, data: Data::default(), by_code: HashMap::new(), dirty: false, max_installs: MAX_INSTALLS }
    }

    /// `relay.json`, 없으면 백업 `relay.json.bak`, 없으면 빈 것으로 시작함 (로그 남김). 찢어진 파일이 릴레이 시작을 막지 않음
    pub fn open(dir: &Path) -> io::Result<Store> {
        private_dir(dir)?;
        let main = dir.join(DATA_FILE);
        let bak = dir.join(format!("{DATA_FILE}.bak"));
        let data = match read_data(&main) {
            Some(d) => d,
            None => match read_data(&bak) {
                Some(d) => {
                    screenpet_log::log!(Warn, "store", "load_backup", "file" => format!("{DATA_FILE}.bak"));
                    d
                }
                None => {
                    if main.exists() || bak.exists() {
                        screenpet_log::log!(Error, "store", "load_empty");
                    }
                    Data::default()
                }
            },
        };
        let mut store = Store { dir: Some(dir.to_path_buf()), data, by_code: HashMap::new(), dirty: false, max_installs: MAX_INSTALLS };
        store.data.installs.retain(|k, _| proto::norm_key(k).as_deref() == Some(k.as_str()));
        let codes: Vec<(String, String)> = store.data.installs.iter().map(|(k, i)| (i.code.clone(), k.clone())).collect();
        for (code, key) in codes {
            if store.by_code.insert(code, key.clone()).is_some() {
                // Duplicate code in a hand-edited file: give this one a new code.
                store.issue_code(&key, unix_now());
            }
        }
        Ok(store)
    }

    /// 설치 하나를 돌려줌
    pub fn get(&self, key: &str) -> Option<&Install> {
        self.data.installs.get(key)
    }

    /// 본 날 수를 정함 (시험용)
    #[cfg(test)]
    pub fn set_seen_days(&mut self, key: &str, days: u32) {
        if let Some(i) = self.data.installs.get_mut(key) {
            i.seen_days = days;
        }
    }

    /// 저장된 설치 수
    pub fn count(&self) -> usize {
        self.data.installs.len()
    }

    /// 모든 설치를 돌려줌
    pub fn installs(&self) -> impl Iterator<Item = (&String, &Install)> {
        self.data.installs.iter()
    }

    /// `key`의 서로 친구 수
    pub fn mutual_count(&self, key: &str) -> usize {
        self.get(key).map_or(0, |i| i.friends.iter().filter(|f| self.mutual(key, f)).count())
    }

    /// `secs` 동안 안 보인 오프라인 설치들
    pub fn idle(&self, now: u64, secs: u64, online: &dyn Fn(&str) -> bool) -> Vec<String> {
        self.installs().filter(|(k, i)| !online(k) && now.saturating_sub(i.last_seen) >= secs).map(|(k, _)| k.clone()).collect()
    }

    /// 서로 친구로 이어진 `min`명 이상의 무리 중 아무도 `online`이 아닌 것들: 서로만 친구 맺은 키들의 모습임.
    /// (모두 자리를 비운 진짜 친구 무리도 그렇게 보여서 `min`을 받음)
    pub fn islands(&self, min: usize, online: &dyn Fn(&str) -> bool) -> Vec<Vec<String>> {
        let mut seen = std::collections::HashSet::new();
        let mut groups = Vec::new();
        for start in self.data.installs.keys() {
            if !seen.insert(start.clone()) {
                continue;
            }
            let mut group = vec![start.clone()];
            let mut next = 0;
            while next < group.len() {
                let key = group[next].clone();
                next += 1;
                for friend in &self.data.installs[&key].friends {
                    if self.mutual(&key, friend) && seen.insert(friend.clone()) {
                        group.push(friend.clone());
                    }
                }
            }
            if group.len() >= min && !group.iter().any(|k| online(k)) {
                groups.push(group);
            }
        }
        groups
    }

    /// 저장소가 찼는지 봄
    pub fn is_full(&self) -> bool {
        self.data.installs.len() >= self.max_installs
    }

    /// `key`에 새 코드를 주고 색인을 고침 (겹치지 않을 때까지 다시 뽑음)
    fn issue_code(&mut self, key: &str, now: u64) {
        let code = loop {
            let c = new_code();
            if !self.by_code.contains_key(&c) {
                break c;
            }
        };
        if let Some(inst) = self.data.installs.get_mut(key) {
            if self.by_code.get(&inst.code).map(String::as_str) == Some(key) {
                self.by_code.remove(&inst.code);
            }
            inst.code = code.clone();
            inst.code_at = now;
            self.by_code.insert(code, key.to_string());
            self.dirty = true;
        }
    }

    /// 지금 `key`에 새 코드를 줌 (사용자가 새 코드를 원함)
    pub fn rotate(&mut self, key: &str, now: u64) {
        self.issue_code(key, now);
    }

    /// 성공한 인증을 기록함. 새 설치면 (코드와 함께) 만들고, 저장소가 차 있으면 `false`. id 라벨과 친구 목록을 고침
    /// (바뀐 게 있을 때만 씀). 차 있으면 먼저 친구 없고 `online`이 아닌 가장 오래 안 보인 설치를 내보냄
    pub fn register_with(&mut self, key: &str, id: &str, friends: Vec<String>, now: u64, online: &dyn Fn(&str) -> bool) -> bool {
        if self.is_full() && !self.data.installs.contains_key(key) {
            let stalest = |keep: &dyn Fn(&str, &Install) -> bool| {
                let offline = self.data.installs.iter().filter(|(k, i)| !online(k) && !keep(k, i));
                offline.min_by_key(|(_, i)| i.last_seen).map(|(k, _)| k.clone())
            };
            // Friendless first, then friends that are still new.
            let victim = stalest(&|k, i| self.has_friends(k, i)).or_else(|| stalest(&|_, i| i.seen_days >= ESTABLISHED_DAYS));
            if let Some(v) = victim {
                self.remove(&v);
            }
        }
        let full = self.is_full();
        match self.data.installs.get_mut(key) {
            Some(inst) => {
                if inst.id != id || inst.friends != friends {
                    inst.id = id.to_string();
                    inst.friends = friends;
                    self.dirty = true;
                }
                self.touch(key, now);
                true
            }
            None if full => false,
            None => {
                let inst = Install { id: id.to_string(), friends, code: String::new(), code_at: 0, last_seen: now, active: false, seen_days: 1 };
                self.data.installs.insert(key.to_string(), inst);
                self.dirty = true;
                self.issue_code(key, now);
                true
            }
        }
    }

    /// 친구 목록을 바꿈 (바뀌었을 때만 저장소를 더럽힘)
    pub fn set_friends(&mut self, key: &str, friends: Vec<String>) {
        if let Some(inst) = self.data.installs.get_mut(key) {
            if inst.friends != friends {
                inst.friends = friends;
                self.dirty = true;
            }
        }
    }

    /// 이 설치가 코드 사용에 낌: 친구 있는 설치처럼 둠
    pub fn mark_active(&mut self, key: &str) {
        if let Some(inst) = self.data.installs.get_mut(key) {
            if !inst.active {
                inst.active = true;
                self.dirty = true;
            }
        }
    }

    /// 활동을 적음. 저장소는 `TOUCH_EVERY_SECS`마다만 더럽힘
    pub fn touch(&mut self, key: &str, now: u64) {
        if let Some(inst) = self.data.installs.get_mut(key) {
            if now.saturating_sub(inst.last_seen) >= TOUCH_EVERY_SECS {
                if now / DAY_SECS != inst.last_seen / DAY_SECS {
                    inst.seen_days = inst.seen_days.saturating_add(1);
                }
                inst.last_seen = now;
                self.dirty = true;
            }
        }
    }

    /// 설치의 지금 코드와 남은 수명(초)을 돌려줌. `ttl`만큼 지났으면 먼저 바꿈. `rotated` = 새 코드를 줬음
    pub fn code(&mut self, key: &str, now: u64, ttl: u64) -> Option<(String, u64, bool)> {
        let due = self.data.installs.get(key).map(|i| now >= i.code_at.saturating_add(ttl))?;
        if due {
            self.issue_code(key, now);
        }
        let inst = self.data.installs.get(key)?;
        let left = inst.code_at.saturating_add(ttl).saturating_sub(now).max(1);
        Some((inst.code.clone(), left, due))
    }

    /// (맞춘) 코드의 설치 키를 돌려줌. 지금 코드일 때만
    pub fn by_code(&self, code: &str, now: u64, ttl: u64) -> Option<&str> {
        let key = self.by_code.get(code)?;
        let inst = self.data.installs.get(key)?;
        (inst.code == code && now < inst.code_at.saturating_add(ttl)).then_some(key.as_str())
    }

    /// 둘 가치가 있는지 봄: 코드 사용에 꼈거나 *서로* 친구가 하나라도 있음 (한쪽만 밝힌 목록은 안 셈)
    pub fn befriended(&self, key: &str) -> bool {
        self.data.installs.get(key).is_some_and(|i| i.active || i.friends.iter().any(|f| self.mutual(key, f)))
    }

    /// `a`가 `b`를, `b`가 `a`를 목록에 두었는지 봄
    pub fn mutual(&self, a: &str, b: &str) -> bool {
        let wants = |x: &str, y: &str| self.data.installs.get(x).is_some_and(|i| i.friends.iter().any(|f| f == y));
        wants(a, b) && wants(b, a)
    }

    /// 설치를 잊음 (운영자 정리)
    pub fn remove(&mut self, key: &str) -> bool {
        match self.data.installs.remove(key) {
            Some(i) => {
                self.by_code.remove(&i.code);
                self.dirty = true;
                true
            }
            None => false,
        }
    }

    /// 누군가의 친구로 둘 가치가 있는지 봄: `befriended`, 또는 친구 목록이 있는 자리 잡은 설치 (친구가 잊혀도 그대로임)
    fn has_friends(&self, key: &str, inst: &Install) -> bool {
        self.befriended(key) || (inst.seen_days >= ESTABLISHED_DAYS && !inst.friends.is_empty())
    }

    /// `key`를 안 보인 채로 얼마나 두는지 돌려줌
    fn keep_for(&self, key: &str, inst: &Install) -> u64 {
        if !self.has_friends(key, inst) {
            IDLE_FORGET_SECS
        } else if inst.seen_days < ESTABLISHED_DAYS {
            NEW_FRIEND_FORGET_SECS
        } else {
            FORGET_AFTER_SECS
        }
    }

    /// `keep_for`보다 오래 안 보인 설치를 잊음. `online`인 것은 절대 안 잊음.
    /// 친구 관계는 `IDLE_FORGET_SECS` 넘게 안 보인 설치만 보므로 보통은 단순 훑기임
    pub fn prune_with(&mut self, now: u64, online: &dyn Fn(&str) -> bool) {
        let expired: Vec<String> = self
            .data
            .installs
            .iter()
            .filter(|(k, i)| {
                let idle = now.saturating_sub(i.last_seen);
                idle >= IDLE_FORGET_SECS && !online(k) && idle >= self.keep_for(k, i)
            })
            .map(|(k, _)| k.clone())
            .collect();
        for k in expired {
            self.remove(&k);
        }
    }

    /// 마지막으로 본 때를 정함 (시험용)
    #[cfg(test)]
    pub(crate) fn touch_at_for_test(&mut self, key: &str, when: u64) {
        self.data.installs.get_mut(key).unwrap().last_seen = when;
    }

    /// 바뀐 게 있으면 쓸 사본을 돌려줌 (저장소를 깨끗하게 표시함. 쓰기에 실패하면 [`Store::mark_dirty`]를 부를 것)
    pub fn snapshot(&mut self) -> Option<Snapshot> {
        if !self.dirty {
            return None;
        }
        self.dirty = false;
        let dir = self.dir.clone()?;
        Some(Snapshot { dir, data: self.data.clone() })
    }

    /// 쓰기에 실패했을 때 다시 저장하게 표시함
    pub fn mark_dirty(&mut self) {
        self.dirty = true;
    }

}

/// 사본을 씀: 비공개 임시 파일, fsync, 옛 파일은 `relay.json.bak`으로 두고 이름 바꾸기, 폴더 fsync (되는 만큼)
pub fn write_snapshot(snap: &Snapshot) -> io::Result<()> {
    let main = snap.dir.join(DATA_FILE);
    let tmp = snap.dir.join(format!("{DATA_FILE}.tmp"));
    let bytes = serde_json::to_vec(&snap.data).map_err(io::Error::other)?;
    write_private(&tmp, &bytes)?;
    if main.exists() {
        std::fs::rename(&main, snap.dir.join(format!("{DATA_FILE}.bak")))?;
    }
    std::fs::rename(&tmp, &main)?;
    if let Ok(d) = std::fs::File::open(&snap.dir) {
        let _ = d.sync_all();
    }
    Ok(())
}

/// `relay.key`(hex)에서 릴레이의 장기 X25519 개인키를 읽음. 처음 시작이면 만듦
pub fn load_or_create_key(dir: &Path) -> io::Result<[u8; 32]> {
    private_dir(dir)?;
    let path = dir.join(KEY_FILE);
    match std::fs::read_to_string(&path) {
        Ok(s) => {
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600))?;
            }
            proto::hex32(s.trim()).ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "relay.key is corrupt"))
        }
        Err(e) if e.kind() == io::ErrorKind::NotFound => {
            let key = proto::random32();
            let tmp = dir.join(format!("{KEY_FILE}.tmp"));
            write_private(&tmp, proto::to_hex(&key).as_bytes())?;
            std::fs::rename(&tmp, &path)?;
            Ok(key)
        }
        Err(e) => Err(e),
    }
}

/// `bytes`를 새 0600 파일(unix)에 쓰고 fsync함
pub(crate) fn write_private(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let mut opts = std::fs::OpenOptions::new();
    opts.write(true).create(true).truncate(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        opts.mode(0o600);
    }
    let mut f = opts.open(path)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        f.set_permissions(std::fs::Permissions::from_mode(0o600))?; // file may have existed
    }
    f.write_all(bytes)?;
    f.sync_all()
}

/// 코드·친구·정리·저장·용량 시험임
#[cfg(test)]
mod tests {
    use super::*;

    fn k(n: u8) -> String {
        proto::to_hex(&[n; 32])
    }

    impl Store {
        fn register(&mut self, key: &str, id: &str, friends: Vec<String>, now: u64) -> bool {
            self.register_with(key, id, friends, now, &|_| false)
        }

        fn prune(&mut self, now: u64) {
            self.prune_with(now, &|_| false)
        }

        fn flush(&mut self) -> io::Result<()> {
            match self.snapshot() {
                Some(snap) => write_snapshot(&snap),
                None => Ok(()),
            }
        }

        fn establish(&mut self, key: &str) {
            self.data.installs.get_mut(key).unwrap().seen_days = ESTABLISHED_DAYS;
        }
    }

    #[test]
    fn codes_rotate_after_ttl_and_old_codes_stop_working() {
        let mut s = Store::memory();
        s.register(&k(1), "a", vec![], 1000);
        let (c1, left, rotated) = s.code(&k(1), 1000, 100).unwrap();
        assert!(!rotated);
        assert_eq!(left, 100);
        assert_eq!(s.by_code(&c1, 1050, 100), Some(k(1).as_str()));
        let (c2, _, rotated) = s.code(&k(1), 1100, 100).unwrap();
        assert!(rotated);
        assert_ne!(c1, c2);
        assert_eq!(s.by_code(&c1, 1100, 100), None);
        assert_eq!(s.by_code(&c2, 1100, 100), Some(k(1).as_str()));
        // Without rotation having run, an expired code is still refused.
        assert_eq!(s.by_code(&c2, 1200, 100), None);
    }

    #[test]
    fn mutual_needs_both_sides() {
        let mut s = Store::memory();
        s.register(&k(1), "a", vec![k(2)], 0);
        s.register(&k(2), "b", vec![], 0);
        assert!(!s.mutual(&k(1), &k(2)));
        s.set_friends(&k(2), vec![k(1)]);
        assert!(s.mutual(&k(1), &k(2)) && s.mutual(&k(2), &k(1)));
    }

    #[test]
    fn persists_and_reloads() {
        let dir = std::env::temp_dir().join(format!("screenpet-relay-store-{}", proto::to_hex(&proto::random32()[..8])));
        {
            let mut s = Store::open(&dir).unwrap();
            s.register(&k(1), "a", vec![k(2)], unix_now());
            s.flush().unwrap();
        }
        let s = Store::open(&dir).unwrap();
        let i = s.get(&k(1)).unwrap();
        assert_eq!(i.id, "a");
        assert_eq!(i.friends, vec![k(2)]);
        assert_eq!(s.by_code(&i.code.clone(), unix_now(), 3600), Some(k(1).as_str()));
        let key = load_or_create_key(&dir).unwrap();
        assert_eq!(load_or_create_key(&dir).unwrap(), key);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn prune_forgets_long_unseen_installs() {
        let mut s = Store::memory();
        s.register(&k(1), "a", vec![], 0);
        s.register(&k(2), "b", vec![], FORGET_AFTER_SECS);
        s.prune(FORGET_AFTER_SECS + 1);
        assert!(s.get(&k(1)).is_none());
        assert!(s.get(&k(2)).is_some());
    }

    fn tmpdir(tag: &str) -> PathBuf {
        std::env::temp_dir().join(format!("screenpet-relay-{tag}-{}", proto::to_hex(&proto::random32()[..8])))
    }

    #[test]
    fn torn_data_file_falls_back_to_backup_then_empty() {
        let dir = tmpdir("torn");
        {
            let mut s = Store::open(&dir).unwrap();
            s.register(&k(1), "a", vec![], unix_now());
            s.flush().unwrap();
            s.register(&k(2), "b", vec![], unix_now());
            s.flush().unwrap(); // the first state is now the backup
        }
        std::fs::write(dir.join(DATA_FILE), b"{\"installs\": {").unwrap();
        let s = Store::open(&dir).unwrap();
        assert!(s.get(&k(1)).is_some(), "loaded from relay.json.bak");
        std::fs::write(dir.join(format!("{DATA_FILE}.bak")), b"garbage").unwrap();
        let s = Store::open(&dir).unwrap();
        assert_eq!(s.count(), 0, "both torn: start empty instead of refusing");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[cfg(unix)]
    #[test]
    fn data_is_private() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tmpdir("perm");
        let mut s = Store::open(&dir).unwrap();
        s.register(&k(1), "a", vec![], unix_now());
        s.flush().unwrap();
        load_or_create_key(&dir).unwrap();
        let mode = |p: &Path| std::fs::metadata(p).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode(&dir), 0o700);
        assert_eq!(mode(&dir.join(DATA_FILE)), 0o600);
        assert_eq!(mode(&dir.join(KEY_FILE)), 0o600);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn installs_are_capped() {
        let mut s = Store::memory();
        s.max_installs = 2;
        assert!(s.register(&k(1), "a", vec![k(2)], 0));
        assert!(s.register(&k(2), "b", vec![k(1)], 0));
        s.establish(&k(1));
        s.establish(&k(2));
        assert!(!s.register(&k(3), "c", vec![], 0), "full of established mutual friends");
        assert!(s.register(&k(1), "a", vec![k(2)], 0), "known installs still log in");
    }

    #[test]
    fn touch_dirties_at_most_every_ten_minutes() {
        let mut s = Store::memory();
        s.register(&k(1), "a", vec![], 1000);
        s.dirty = false;
        s.touch(&k(1), 1100);
        assert!(!s.dirty);
        s.register(&k(1), "a", vec![], 1200);
        assert!(!s.dirty, "unchanged login is not a write");
        s.touch(&k(1), 1000 + 601);
        assert!(s.dirty);
    }

    #[test]
    fn idle_installs_without_friends_expire_after_a_day() {
        let mut s = Store::memory();
        s.register(&k(1), "lonely", vec![], 0);
        s.register(&k(2), "friendly", vec![k(4)], 0);
        s.register(&k(4), "its friend", vec![k(2)], 0);
        s.register(&k(3), "redeemer", vec![], 0);
        s.mark_active(&k(3));
        s.prune(IDLE_FORGET_SECS + 1);
        assert!(s.get(&k(1)).is_none(), "never befriended, not back within a day");
        assert!(s.get(&k(2)).is_some());
        assert!(s.get(&k(3)).is_some());
    }

    #[test]
    fn keys_befriending_each_other_get_no_long_retention() {
        let mut s = Store::memory();
        s.register(&k(1), "new", vec![k(2)], 0);
        s.register(&k(2), "new", vec![k(1)], 0);
        s.register(&k(3), "old", vec![k(4)], 0);
        s.register(&k(4), "old", vec![k(3)], 0);
        s.establish(&k(3));
        s.establish(&k(4));
        s.prune(NEW_FRIEND_FORGET_SECS - 1);
        assert!(s.get(&k(1)).is_some(), "kept past the friendless day");
        s.prune(NEW_FRIEND_FORGET_SECS);
        assert!(s.get(&k(1)).is_none() && s.get(&k(2)).is_none());
        assert!(s.get(&k(3)).is_some() && s.get(&k(4)).is_some(), "seen on several days");
        s.prune(FORGET_AFTER_SECS);
        assert!(s.get(&k(3)).is_none());
    }

    #[test]
    fn a_full_store_gives_way_to_new_installs_over_new_friends() {
        let mut s = Store::memory();
        s.max_installs = 3;
        s.register(&k(1), "clique", vec![k(2)], 500);
        s.register(&k(2), "clique", vec![k(1)], 600);
        s.register(&k(3), "old", vec![k(4)], 0);
        s.establish(&k(3));
        s.register(&k(7), "lonely", vec![], 650);
        assert_eq!(s.count(), 3, "the stalest friendless install went");
        assert!(s.get(&k(3)).is_some(), "established, so its friend list still counts");
        assert!(s.get(&k(1)).is_none() || s.get(&k(2)).is_none(), "a new friend made room for lonely");
        assert!(!s.register_with(&k(6), "x", vec![], 800, &|key| key == k(7) || key == k(2)), "only established ones left");
    }

    #[test]
    fn islands_are_offline_mutual_groups_of_a_size() {
        let mut s = Store::memory();
        // 1-2-3 a chain, 4-5 a pair, 6 one-sided towards 1.
        s.register(&k(1), "", vec![k(2)], 0);
        s.register(&k(2), "", vec![k(1), k(3)], 0);
        s.register(&k(3), "", vec![k(2)], 0);
        s.register(&k(4), "", vec![k(5)], 0);
        s.register(&k(5), "", vec![k(4)], 0);
        s.register(&k(6), "", vec![k(1)], 0);
        let found = s.islands(3, &|_| false);
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].len(), 3);
        assert_eq!(s.islands(2, &|_| false).len(), 2);
        assert_eq!(s.islands(3, &|key| key == k(2)).len(), 0, "someone in it is online");
        assert_eq!(s.idle(100, 100, &|key| key != k(1)), vec![k(1)]);
        assert!(s.idle(99, 100, &|_| false).is_empty());
    }

    #[test]
    fn days_seen_count_distinct_days() {
        let mut s = Store::memory();
        s.register(&k(1), "a", vec![], 1000);
        assert_eq!(s.get(&k(1)).unwrap().seen_days, 1);
        s.touch(&k(1), 2000);
        assert_eq!(s.get(&k(1)).unwrap().seen_days, 1, "same day");
        s.touch(&k(1), DAY_SECS + 5);
        assert_eq!(s.get(&k(1)).unwrap().seen_days, 2);
    }

    #[test]
    fn older_data_files_still_load() {
        let dir = tmpdir("old");
        std::fs::create_dir_all(&dir).unwrap();
        let file = format!(r#"{{"installs":{{"{}":{{"id":"a","code":"ABCD-1234","code_at":5,"last_seen":7}}}}}}"#, k(1));
        std::fs::write(dir.join(DATA_FILE), file).unwrap();
        let s = Store::open(&dir).unwrap();
        assert_eq!(s.get(&k(1)).unwrap().seen_days, ESTABLISHED_DAYS);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// `seen_days` 전 실제 relay.json의 필드들임
    #[test]
    fn friends_from_an_older_file_keep_their_long_retention() {
        let dir = tmpdir("legacy");
        std::fs::create_dir_all(&dir).unwrap();
        let install = |friend: String, code: &str| serde_json::json!({"id": "a", "friends": [friend], "code": code, "code_at": 5, "last_seen": 0, "active": false});
        let file = serde_json::json!({"installs": {k(1): install(k(2), "ABCD-1234"), k(2): install(k(1), "ABCD-1235")}});
        std::fs::write(dir.join(DATA_FILE), file.to_string()).unwrap();
        let mut s = Store::open(&dir).unwrap();
        s.prune(NEW_FRIEND_FORGET_SECS + 1);
        assert!(s.get(&k(1)).is_some() && s.get(&k(2)).is_some(), "past 14 days");
        s.register(&k(3), "new", vec![], 0);
        assert_eq!(s.get(&k(3)).unwrap().seen_days, 1, "new installs still start new");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn an_established_install_keeps_its_retention_when_its_friend_goes() {
        let mut s = Store::memory();
        s.register(&k(1), "a", vec![k(2)], 0);
        s.register(&k(2), "b", vec![k(1)], 0);
        s.establish(&k(1));
        s.remove(&k(2));
        s.prune(IDLE_FORGET_SECS + 1);
        assert!(s.get(&k(1)).is_some());
        s.prune(FORGET_AFTER_SECS);
        assert!(s.get(&k(1)).is_none());
    }

    #[test]
    fn a_full_store_evicts_the_stalest_friendless_install_first() {
        let mut s = Store::memory();
        s.max_installs = 3;
        s.register(&k(1), "old", vec![], 0);
        s.register(&k(2), "newer", vec![], 1000);
        s.register(&k(3), "redeemer", vec![], 0);
        s.mark_active(&k(3));
        s.establish(&k(3));
        assert!(s.register_with(&k(4), "d", vec![], 2000, &|_| false));
        assert!(s.get(&k(1)).is_none(), "least recently seen friendless install evicted");
        assert!(s.get(&k(4)).is_some());
        // Online or befriended installs are never evicted: then it refuses.
        assert!(!s.register_with(&k(5), "e", vec![], 3000, &|key| key == k(2) || key == k(4)));
    }

    #[test]
    fn only_mutual_friendships_protect_an_install() {
        let mut s = Store::memory();
        s.max_installs = 3;
        s.register(&k(2), "b", vec![k(3)], 0);
        s.register(&k(3), "c", vec![k(2)], 0);
        s.establish(&k(2));
        s.establish(&k(3));
        // Self-declared friend that does not list it back: no protection.
        s.register(&k(1), "a", vec![k(9)], 500);
        assert!(s.register_with(&k(4), "d", vec![], 1000, &|_| false));
        assert!(s.get(&k(1)).is_none(), "one-sided list evicted before older mutual friends");
        assert!(s.get(&k(2)).is_some() && s.get(&k(3)).is_some());
        // Nothing evictable left (k4 online): refuse.
        assert!(!s.register_with(&k(5), "e", vec![], 2000, &|key| key == k(4)));
        // A one-sided list doesn't buy the long retention either.
        let mut s = Store::memory();
        s.register(&k(1), "a", vec![k(9)], 0);
        s.prune(IDLE_FORGET_SECS + 1);
        assert!(s.get(&k(1)).is_none());
    }
}
