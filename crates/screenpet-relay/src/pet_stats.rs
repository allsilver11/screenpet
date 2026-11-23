//! 📊 익명 펫 통계 저장 (`stats.json`, spec 2026-11-23-pet-stats). 날짜(보고하는 앱의 그날)마다 합계만 둠:
//! 종류별 키운 설치 수·새로 고른 수·바꾼 쌍·앱 버전. 누가 보냈는지는 합계에 없음.
//! 같은 설치가 같은 날 두 번 보내 부풀리지 않게 "그날 보낸 설치"의 키 해시(날짜를 섞어 날마다 다름)만 그날이
//! 받을 수 있는 동안(`LATE_DAYS`) 두고 그 뒤 지움. 주소별 하루 한도는 메모리에만. 90일 지난 날은 지움.
//! 받는 것은 모두 다시 검사함: 날짜·크기·종류 이름(모르면 other)·횟수 상한. `hub::pet_stats`가 `Hub::stats` 잠금 안에서 씀.

use crate::store::write_private;
use screenpet_proto::pet_stats::{self as ps, Change, StatsDay, StatsReport};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::io;
use std::path::Path;

/// 데이터 폴더 안 통계 파일 이름
pub const STATS_FILE: &str = "stats.json";
/// 하루당 주소 묶음 하나에서 받는 보고 수 (메모리에만 셈)
pub const PER_IP: u32 = 20;
/// 하루당 받는 보고 수 (= 그날 해시 수 상한)
pub const MAX_REPORTS: usize = 20_000;
/// 설치가 이만큼 서로 다른 날에 보였어야 셈 (방금 만든 키로 부풀리지 못함 — 어제를 보고하니 진짜 설치는 늘 넘음)
pub const MIN_SEEN_DAYS: u32 = 2;
/// 해시에 섞는 꼬리표
const HASH_LABEL: &[u8] = b"screenpet-stats/1";

/// 보고를 세지 않은 까닭임 (로그의 reason)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Refuse {
    /// 날짜가 이상하거나 너무 지났거나 미래임
    Day,
    /// 칸이 너무 많음
    Size,
    /// 너무 새 설치
    New,
    /// 이 설치가 그날 이미 보냄
    Dup,
    /// 이 주소의 하루 한도
    Ip,
    /// 그날 받을 만큼 받음
    Full,
}

impl Refuse {
    /// 로그 이유 글을 돌려줌
    pub fn as_str(&self) -> &'static str {
        match self {
            Refuse::Day => "day",
            Refuse::Size => "size",
            Refuse::New => "new",
            Refuse::Dup => "dup",
            Refuse::Ip => "ip",
            Refuse::Full => "full",
        }
    }
}

/// `stats.json`에 쓰는 내용임
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
struct Data {
    /// 날짜 → 그날 합계
    days: BTreeMap<String, StatsDay>,
    /// 날짜 → 그날 보낸 설치의 키 해시 (그날이 받을 수 있는 동안만)
    seen: BTreeMap<String, BTreeSet<String>>,
}

/// 통계 저장소임: 내용, 저장할 게 있는지, 주소별 하루 보고 수 (메모리)
#[derive(Debug, Default)]
pub struct PetStats {
    data: Data,
    dirty: bool,
    /// (날짜, 주소 묶음) → 보고 수. 저장하지 않음
    ip_counts: HashMap<(String, String), u32>,
}

/// 그날 그 키의 해시 (hex 32자): 날짜가 섞여 다른 날과 이어 볼 수 없음
fn key_hash(day: &str, key: &str) -> String {
    let h = screenpet_crypto::blake2s(&[HASH_LABEL, day.as_bytes(), key.as_bytes()]);
    screenpet_crypto::to_hex(&h[..16])
}

/// 이름 → 횟수 표에 더하되 횟수는 상한까지
fn add_capped(map: &mut BTreeMap<String, u32>, name: &str, n: u32) {
    let v = map.entry(name.to_string()).or_insert(0);
    *v = v.saturating_add(n).min(ps::MAX_COUNT);
}

impl PetStats {
    /// 오래된 날 합계(`KEEP_DAYS`)·받을 수 없게 된 날의 해시와 주소 수(`LATE_DAYS`)를 지움
    fn prune(&mut self, now: u64) {
        let today = ps::utc_day(now);
        let keep = |day: &str, within: i64| ps::day_number(day).is_some_and(|n| today - n < within);
        let (days, seen) = (self.data.days.len(), self.data.seen.len());
        self.data.days.retain(|d, _| keep(d, ps::KEEP_DAYS));
        self.data.seen.retain(|d, _| keep(d, ps::LATE_DAYS + 1));
        self.ip_counts.retain(|(d, _), _| keep(d, ps::LATE_DAYS + 1));
        self.dirty |= days != self.data.days.len() || seen != self.data.seen.len();
    }

    /// `key`(인증된 설치)가 주소 묶음 `ip`에서 보낸 보고를 그날 합계에 더하거나 안 세는 까닭을 말함. `seen_days`는 설치 저장소에서 옴
    pub fn submit(&mut self, key: &str, ip: &str, seen_days: u32, r: &StatsReport, now: u64) -> Result<(), Refuse> {
        self.prune(now);
        let today = ps::utc_day(now);
        let day = ps::day_number(&r.day).filter(|n| (today - ps::LATE_DAYS..=today).contains(n)).ok_or(Refuse::Day)?;
        let day = ps::day_string(day);
        if r.kept.len() > ps::MAX_KEPT || r.chosen.len() > ps::MAX_CHOSEN || r.changed.len() > ps::MAX_CHANGED {
            return Err(Refuse::Size);
        }
        if seen_days < MIN_SEEN_DAYS {
            return Err(Refuse::New);
        }
        let hash = key_hash(&day, key);
        let seen = self.data.seen.get(&day);
        if seen.is_some_and(|s| s.contains(&hash)) {
            return Err(Refuse::Dup);
        }
        if seen.is_some_and(|s| s.len() >= MAX_REPORTS) {
            return Err(Refuse::Full);
        }
        let ip_key = (day.clone(), ip.to_string());
        if self.ip_counts.get(&ip_key).is_some_and(|n| *n >= PER_IP) {
            return Err(Refuse::Ip);
        }
        // 이 보고 안에서 먼저 다듬고 합침 (모르는 이름은 other — 합친 뒤에도 칸마다 상한)
        let kept: BTreeSet<&str> = r.kept.iter().map(|s| ps::species_or_other(s)).collect();
        let mut chosen = BTreeMap::new();
        for (s, n) in &r.chosen {
            add_capped(&mut chosen, ps::species_or_other(s), *n);
        }
        let mut pairs: BTreeMap<(&str, &str), u32> = BTreeMap::new();
        for c in &r.changed {
            let v = pairs.entry((ps::species_or_other(&c.from), ps::species_or_other(&c.to))).or_insert(0);
            *v = (*v + c.n.min(ps::MAX_COUNT)).min(ps::MAX_COUNT);
        }
        let entry = self.data.days.entry(day.clone()).or_insert_with(|| StatsDay { day: day.clone(), ..StatsDay::default() });
        entry.reports += 1;
        for s in kept {
            *entry.kept.entry(s.to_string()).or_insert(0) += 1;
        }
        for (s, n) in chosen.into_iter().filter(|(_, n)| *n > 0) {
            *entry.chosen.entry(s).or_insert(0) += n;
        }
        for ((from, to), n) in pairs.into_iter().filter(|(_, n)| *n > 0) {
            let at = entry.changed.iter().position(|c| c.from == from && c.to == to).or_else(|| {
                // 쌍이 너무 많아지면 새 쌍은 other→other로
                let (from, to) = if entry.changed.len() >= ps::MAX_DAY_PAIRS { (ps::OTHER, ps::OTHER) } else { (from, to) };
                entry.changed.iter().position(|c| c.from == from && c.to == to).or_else(|| {
                    entry.changed.push(Change { from: from.into(), to: to.into(), n: 0 });
                    Some(entry.changed.len() - 1)
                })
            });
            if let Some(i) = at {
                entry.changed[i].n += n;
            }
        }
        let mut v = ps::version_or_other(&r.v);
        if !entry.versions.contains_key(&v) && entry.versions.len() >= ps::MAX_DAY_VERSIONS {
            v = ps::OTHER.to_string();
        }
        *entry.versions.entry(v).or_insert(0) += 1;
        self.data.seen.entry(day).or_default().insert(hash);
        *self.ip_counts.entry(ip_key).or_insert(0) += 1;
        self.dirty = true;
        Ok(())
    }

    /// 날짜별 합계를 오래된 날 먼저 돌려줌 (90일 지난 날은 먼저 지움)
    pub fn days(&mut self, now: u64) -> Vec<StatsDay> {
        self.prune(now);
        self.data.days.values().cloned().collect()
    }

    /// 지난 스냅숏 뒤 바뀐 게 있으면 파일 바이트를 돌려줌
    pub fn snapshot(&mut self) -> Option<Vec<u8>> {
        if !self.dirty {
            return None;
        }
        self.dirty = false;
        serde_json::to_vec(&self.data).ok()
    }

    /// 쓰기에 실패했을 때 다시 저장하게 표시함
    pub fn mark_dirty(&mut self) {
        self.dirty = true;
    }

    /// `stats.json`, 없으면 `.bak`, 없으면 빈 통계를 읽음 (날짜 이름이 이상한 칸은 버림)
    pub fn load(dir: &Path) -> PetStats {
        for name in [STATS_FILE.to_string(), format!("{STATS_FILE}.bak")] {
            if let Ok(bytes) = std::fs::read(dir.join(&name)) {
                if let Ok(mut data) = serde_json::from_slice::<Data>(&bytes) {
                    data.days.retain(|d, _| ps::day_number(d).is_some());
                    data.seen.retain(|d, _| ps::day_number(d).is_some());
                    for (d, v) in data.days.iter_mut() {
                        v.day = d.clone();
                    }
                    return PetStats { data, ..PetStats::default() };
                }
                screenpet_log::log!(Error, "stats", "load_fail", "file" => name);
            }
        }
        PetStats::default()
    }

    /// `letters.json`처럼 씀: 비공개 임시 파일(fsync), 옛 파일은 `.bak`으로 두고 이름 바꾸기, 폴더 fsync
    pub fn save(dir: &Path, bytes: &[u8]) -> io::Result<()> {
        let main = dir.join(STATS_FILE);
        let tmp = dir.join(format!("{STATS_FILE}.tmp"));
        write_private(&tmp, bytes)?;
        if main.exists() {
            std::fs::rename(&main, dir.join(format!("{STATS_FILE}.bak")))?;
        }
        std::fs::rename(&tmp, &main)?;
        if let Ok(d) = std::fs::File::open(dir) {
            let _ = d.sync_all();
        }
        Ok(())
    }
}

/// 합계·중복·상한·이름·지우기·저장 시험임
#[cfg(test)]
mod tests {
    use super::*;

    /// 2026-11-23 12:00 UTC
    const NOW: u64 = 20_780 * 86_400 + 12 * 3600;
    const DAY: &str = "2026-11-22";

    fn tmp_dir(tag: &str) -> std::path::PathBuf {
        let d = std::env::temp_dir().join(format!("screenpet-petstats-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    fn report(day: &str, kept: &[&str], chosen: &[(&str, u32)], changed: &[(&str, &str, u32)]) -> StatsReport {
        StatsReport {
            day: day.into(),
            v: "0.7.4".into(),
            kept: kept.iter().map(|s| s.to_string()).collect(),
            chosen: chosen.iter().map(|(s, n)| (s.to_string(), *n)).collect(),
            changed: changed.iter().map(|(f, t, n)| Change { from: f.to_string(), to: t.to_string(), n: *n }).collect(),
        }
    }

    #[test]
    fn reports_add_up_per_day_without_who_sent_them() {
        let mut s = PetStats::default();
        assert!(s.submit("k1", "1.1.1.1", 5, &report(DAY, &["cat", "penguin"], &[("penguin", 1)], &[("cat", "bat", 2)]), NOW).is_ok());
        assert!(s.submit("k2", "1.1.1.2", 5, &report(DAY, &["cat"], &[], &[("cat", "bat", 1)]), NOW).is_ok());
        assert!(s.submit("k1", "1.1.1.1", 5, &report("2026-11-23", &["bat"], &[], &[]), NOW).is_ok(), "another day is another report");
        let days = s.days(NOW);
        assert_eq!(days.iter().map(|d| d.day.as_str()).collect::<Vec<_>>(), vec![DAY, "2026-11-23"], "oldest first");
        let d = &days[0];
        assert_eq!(d.reports, 2);
        assert_eq!(d.kept, BTreeMap::from([("cat".into(), 2), ("penguin".into(), 1)]));
        assert_eq!(d.chosen, BTreeMap::from([("penguin".into(), 1)]));
        assert_eq!(d.changed, vec![Change { from: "cat".into(), to: "bat".into(), n: 3 }]);
        assert_eq!(d.versions, BTreeMap::from([("0.7.4".into(), 2)]));
        let file = String::from_utf8(s.snapshot().unwrap()).unwrap();
        assert!(!file.contains("k1") && !file.contains("1.1.1.1"), "no keys or addresses on disk: {file}");
    }

    #[test]
    fn the_same_install_counts_once_a_day() {
        let mut s = PetStats::default();
        let r = report(DAY, &["cat"], &[], &[]);
        assert!(s.submit("k1", "1.1.1.1", 5, &r, NOW).is_ok());
        assert_eq!(s.submit("k1", "2.2.2.2", 5, &r, NOW + 60), Err(Refuse::Dup), "even from another address");
        assert_eq!(s.days(NOW)[0].reports, 1);
        assert_ne!(key_hash(DAY, "k1"), key_hash("2026-11-23", "k1"), "a different hash every day");
    }

    #[test]
    fn bad_days_big_reports_new_installs_and_busy_addresses_are_refused() {
        let mut s = PetStats::default();
        let ok = report(DAY, &["cat"], &[], &[]);
        for bad in ["2026-11-24", "2026-11-20", "2026-02-30", "yesterday", ""] {
            assert_eq!(s.submit("k", "1.1.1.1", 5, &report(bad, &["cat"], &[], &[]), NOW), Err(Refuse::Day), "{bad}");
        }
        assert!(s.submit("k0", "1.1.1.1", 5, &report("2026-11-21", &[], &[], &[]), NOW).is_ok(), "two days late is fine (time zones)");
        let many: Vec<String> = (0..=ps::MAX_KEPT).map(|i| format!("x{i}")).collect();
        let big = StatsReport { kept: many, ..ok.clone() };
        assert_eq!(s.submit("k", "1.1.1.1", 5, &big, NOW), Err(Refuse::Size));
        let big = StatsReport { chosen: (0..=ps::MAX_CHOSEN).map(|i| (format!("x{i}"), 1)).collect(), ..ok.clone() };
        assert_eq!(s.submit("k", "1.1.1.1", 5, &big, NOW), Err(Refuse::Size));
        let big = StatsReport { changed: (0..=ps::MAX_CHANGED).map(|_| Change { from: "a".into(), to: "b".into(), n: 1 }).collect(), ..ok.clone() };
        assert_eq!(s.submit("k", "1.1.1.1", 5, &big, NOW), Err(Refuse::Size));
        assert_eq!(s.submit("new", "1.1.1.1", 1, &ok, NOW), Err(Refuse::New));
        for i in 0..PER_IP {
            assert!(s.submit(&format!("ip{i}"), "9.9.9.9", 5, &ok, NOW).is_ok());
        }
        assert_eq!(s.submit("late", "9.9.9.9", 5, &ok, NOW), Err(Refuse::Ip));
        assert!(s.submit("late", "9.9.9.9", 5, &report("2026-11-23", &[], &[], &[]), NOW).is_ok(), "per day");
        assert_eq!(Refuse::Dup.as_str(), "dup");
    }

    #[test]
    fn odd_names_become_other_and_counts_are_capped() {
        let mut s = PetStats::default();
        let r = StatsReport {
            v: "<b>hi</b>".into(),
            ..report(DAY, &["cat", "Cat", "dragon", "<img>"], &[("unicorn", 5), ("phoenix", 999), ("cat", 999)], &[("cat", "dragon", 999), ("x", "y", 3), ("z", "w", 0)])
        };
        assert!(s.submit("k", "1.1.1.1", 5, &r, NOW).is_ok());
        let d = &s.days(NOW)[0];
        assert_eq!(d.kept, BTreeMap::from([("cat".into(), 1), ("other".into(), 1)]), "other once per report");
        assert_eq!(d.chosen, BTreeMap::from([("cat".into(), ps::MAX_COUNT), ("other".into(), ps::MAX_COUNT)]));
        assert_eq!(
            d.changed,
            vec![Change { from: "cat".into(), to: "other".into(), n: ps::MAX_COUNT }, Change { from: "other".into(), to: "other".into(), n: 3 }]
        );
        assert_eq!(d.versions, BTreeMap::from([("other".into(), 1)]));
    }

    #[test]
    fn a_day_holds_only_so_many_pairs_and_versions() {
        let mut s = PetStats::default();
        let sp = ps::SPECIES;
        let mut n = 0;
        'outer: for a in sp {
            for b in sp {
                if n >= ps::MAX_DAY_PAIRS + 3 {
                    break 'outer;
                }
                let r = StatsReport { v: format!("0.{}.0", n), ..report(DAY, &[], &[], &[(a, b, 1)]) };
                s.submit(&format!("k{n}"), &format!("10.0.{}.{}", n / 200, n % 200), 5, &r, NOW).unwrap();
                n += 1;
            }
        }
        let d = &s.days(NOW)[0];
        assert!(d.changed.len() <= ps::MAX_DAY_PAIRS + 1, "{}", d.changed.len());
        assert!(d.changed.iter().any(|c| c.from == "other" && c.to == "other" && c.n >= 3));
        assert!(d.versions.len() <= ps::MAX_DAY_VERSIONS + 1);
        assert!(d.versions["other"] >= 3);
    }

    #[test]
    fn old_days_and_old_hashes_are_dropped() {
        let mut s = PetStats::default();
        s.submit("k", "1.1.1.1", 5, &report(DAY, &["cat"], &[], &[]), NOW).unwrap();
        let later = NOW + 3 * 86_400;
        assert_eq!(s.days(later).len(), 1, "the total stays");
        assert!(s.data.seen.is_empty(), "who sent it is forgotten once the day can no longer take reports");
        assert!(s.ip_counts.is_empty());
        assert_eq!(s.days(NOW + 88 * 86_400).len(), 1, "89 days old: still kept");
        assert!(s.days(NOW + 89 * 86_400).is_empty(), "90 days old: gone (the last 90 days are kept)");
    }

    #[test]
    fn it_survives_a_restart_and_a_torn_file() {
        let dir = tmp_dir("restart");
        let mut s = PetStats::default();
        s.submit("k1", "1.1.1.1", 5, &report(DAY, &["cat"], &[], &[]), NOW).unwrap();
        PetStats::save(&dir, &s.snapshot().unwrap()).unwrap();
        assert!(s.snapshot().is_none(), "clean after a snapshot");
        s.submit("k2", "1.1.1.2", 5, &report(DAY, &["bat"], &[], &[]), NOW).unwrap();
        PetStats::save(&dir, &s.snapshot().unwrap()).unwrap();
        let mut back = PetStats::load(&dir);
        assert_eq!(back.days(NOW)[0].reports, 2);
        assert_eq!(back.submit("k1", "1.1.1.9", 5, &report(DAY, &["cat"], &[], &[]), NOW), Err(Refuse::Dup), "remembered across a restart");
        std::fs::write(dir.join(STATS_FILE), b"{torn").unwrap();
        assert_eq!(PetStats::load(&dir).days(NOW)[0].reports, 1, "falls back to .bak");
        std::fs::write(dir.join(STATS_FILE), br#"{"days":{"nope":{"reports":5},"2026-11-22":{"reports":2}}}"#).unwrap();
        let mut odd = PetStats::load(&dir);
        let days = odd.days(NOW);
        assert_eq!((days.len(), days[0].day.as_str(), days[0].reports), (1, DAY, 2), "bad day names are dropped, day is filled in");
        let _ = std::fs::remove_dir_all(&dir);
        assert!(PetStats::load(&dir).days(NOW).is_empty(), "nothing at all: empty stats");
    }
}
