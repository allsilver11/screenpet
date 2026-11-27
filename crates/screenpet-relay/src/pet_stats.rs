//! 📊 익명 펫 통계 저장 (`stats.json`, spec 2026-11-23-pet-stats). 날짜(보고하는 앱의 지역 날짜)마다 합계를 둠:
//! 종류별 키운 설치 수·새로 고른 수·바꾼 쌍·앱 버전, 그리고 그날 보고한 익명 번호와 종류마다 키운 번호 (2026-11-27 정책:
//! 앱이 무작위로 만든 번호라 누구인지는 모르지만 기간 동안 서로 다른 사람 수를 셈). 키·주소와 번호를 잇는 값은 파일에 없음.
//! 같은 익명 번호는 그날 한 번만 셈 (파일에 남으니 다시 켜도). 번호는 모양을 엄격히 검사하고 하루 `MAX_DAY_PEOPLE`개까지만 적음.
//! 같은 설치가 같은 날 두 번 보내 부풀리지 않게 그날 보낸 설치를 기억하지만 **메모리에만**: 날마다 새로 뽑은
//! 비밀(메모리에만)로 만든 키 해시라 공개키 목록(`relay.json`)으로 다시 만들 수 없고, 그날이 보고를 받는 동안
//! (`ACCEPT_DAYS`)만 두며, 릴레이를 다시 켜면 잊음 (그만큼 중복 막기는 약해짐 — 일부러).
//! 파일은 보고마다가 아니라 몇 분마다 한꺼번에(`Hub::save_stats`) 쓰고 `.bak`은 두지 않음: 두 파일을 비교해 보고 하나를 떼어 볼 수 없게.
//! 받는 것은 모두 다시 검사함: 날짜·크기·종류 이름(모르면 other)·횟수 상한. `hub::pet_stats`가 `Hub::stats` 잠금 안에서 씀.

use crate::store::write_private;
use screenpet_proto::pet_stats::{self as ps, Change, StatsDay, StatsReport};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::io;
use std::path::Path;

/// 데이터 폴더 안 통계 파일 이름
pub const STATS_FILE: &str = "stats.json";
/// 하루당 주소 묶음 하나에서 받는 보고 수 (메모리에만 셈)
pub const PER_IP: u32 = 20;
/// 하루당 받는 보고 수
pub const MAX_REPORTS: usize = 20_000;
/// 해시에 섞는 꼬리표
const HASH_LABEL: &[u8] = b"screenpet-stats/2";

/// 보고를 세지 않은 까닭임 (로그의 reason)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Refuse {
    /// 날짜가 이상하거나 너무 지났거나 미래임
    Day,
    /// 칸이 너무 많음
    Size,
    /// 이 설치가 그날 이미 보냄
    Dup,
    /// 이 주소의 하루 한도 (다음에 다시 보낼 수 있음)
    Ip,
    /// 그날 받을 만큼 받음 (다음에 다시 보낼 수 있음)
    Full,
    /// 익명 번호 모양이 틀림
    Id,
}

impl Refuse {
    /// 로그 이유 글을 돌려줌
    pub fn as_str(&self) -> &'static str {
        match self {
            Refuse::Day => "day",
            Refuse::Size => "size",
            Refuse::Dup => "dup",
            Refuse::Ip => "ip",
            Refuse::Full => "full",
            Refuse::Id => "id",
        }
    }

    /// 다시 보내도 소용없음: `stats_ok`로 답해 앱이 버리게 함. 아니면 답하지 않아 앱이 다음 연결에 다시 보냄
    pub fn is_final(&self) -> bool {
        matches!(self, Refuse::Day | Refuse::Size | Refuse::Dup | Refuse::Id)
    }
}

/// `stats.json`에 쓰는 내용임: 날짜별 합계뿐 (예전 파일의 다른 칸은 읽을 때 버림)
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
struct Data {
    /// 날짜 → 그날 합계
    days: BTreeMap<String, StatsDay>,
}

/// 통계 저장소임: 합계(파일), 저장할 게 있는지, 그리고 메모리에만 있는 중복 막기
#[derive(Debug, Default)]
pub struct PetStats {
    data: Data,
    dirty: bool,
    /// 날짜 → 그날 해시 비밀 (메모리에만, 처음 쓸 때 뽑음)
    secrets: HashMap<String, [u8; 32]>,
    /// 날짜 → 그날 보낸 설치의 해시 (메모리에만)
    seen: HashMap<String, HashSet<[u8; 16]>>,
    /// (날짜, 주소 묶음) → 보고 수 (메모리에만)
    ip_counts: HashMap<(String, String), u32>,
}

/// 파일에서 읽은 하루의 번호 칸 검사: 번호 모양·상한·겹침·자리 범위 중 하나라도 틀리면 그날 번호와 자리를 모두 버림 (합계는 둠)
fn tidy_people(day: &mut StatsDay) {
    let unique: HashSet<&String> = day.people.iter().collect();
    let ok = day.people.len() <= ps::MAX_DAY_PEOPLE
        && unique.len() == day.people.len()
        && day.people.iter().all(|p| ps::is_anon(p))
        && day.keepers.values().all(|v| v.iter().all(|i| usize::from(*i) < day.people.len()));
    if !ok {
        day.people.clear();
        day.keepers.clear();
    }
}

/// 이름 → 횟수 표에 더하되 횟수는 상한까지
fn add_capped(map: &mut BTreeMap<String, u32>, name: &str, n: u32) {
    let v = map.entry(name.to_string()).or_insert(0);
    *v = v.saturating_add(n).min(ps::MAX_COUNT);
}

impl PetStats {
    /// 그날 그 키의 해시: 그날 비밀(메모리에만)을 섞어 공개키만으로는 만들 수 없음
    fn key_hash(&mut self, day: &str, key: &str) -> [u8; 16] {
        let secret = *self.secrets.entry(day.to_string()).or_insert_with(screenpet_crypto::random32);
        let h = screenpet_crypto::blake2s(&[HASH_LABEL, &secret, day.as_bytes(), key.as_bytes()]);
        h[..16].try_into().expect("16 bytes")
    }

    /// 오래된 날 합계(`KEEP_DAYS`)와 더는 보고를 안 받는 날의 비밀·해시·주소 수(`ACCEPT_DAYS`)를 지움. 합계가 줄면 저장할 것으로 표시
    pub fn prune(&mut self, now: u64) {
        let today = ps::utc_day(now);
        let keep = |day: &str, within: i64| ps::day_number(day).is_some_and(|n| today - n < within);
        let days = self.data.days.len();
        self.data.days.retain(|d, _| keep(d, ps::KEEP_DAYS));
        self.secrets.retain(|d, _| keep(d, ps::ACCEPT_DAYS + 1));
        self.seen.retain(|d, _| keep(d, ps::ACCEPT_DAYS + 1));
        self.ip_counts.retain(|(d, _), _| keep(d, ps::ACCEPT_DAYS + 1));
        self.dirty |= days != self.data.days.len();
    }

    /// `key`(인증된 설치)가 주소 묶음 `ip`에서 보낸 보고를 그날 합계에 더하거나 안 세는 까닭을 말함 (저장은 나중에 한꺼번에)
    pub fn submit(&mut self, key: &str, ip: &str, r: &StatsReport, now: u64) -> Result<(), Refuse> {
        self.prune(now);
        let today = ps::utc_day(now);
        let day = ps::day_number(&r.day).filter(|n| (today - ps::ACCEPT_DAYS..=today).contains(n)).ok_or(Refuse::Day)?;
        let day = ps::day_string(day);
        if r.kept.len() > ps::MAX_KEPT || r.chosen.len() > ps::MAX_CHOSEN || r.changed.len() > ps::MAX_CHANGED {
            return Err(Refuse::Size);
        }
        let anon = r.anon.as_deref();
        if anon.is_some_and(|a| !ps::is_anon(a)) {
            return Err(Refuse::Id);
        }
        let hash = self.key_hash(&day, key);
        let seen = self.seen.get(&day);
        let known_anon = |a: &str| self.data.days.get(&day).is_some_and(|d| d.people.iter().any(|p| p == a));
        if seen.is_some_and(|s| s.contains(&hash)) || anon.is_some_and(known_anon) {
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
        // 익명 번호는 하루 상한까지만 적고 (넘으면 합계에만), 종류마다 그 번호의 자리를 적음
        let slot = anon.filter(|_| entry.people.len() < ps::MAX_DAY_PEOPLE).map(|a| {
            entry.people.push(a.to_string());
            (entry.people.len() - 1) as u16
        });
        for s in kept {
            *entry.kept.entry(s.to_string()).or_insert(0) += 1;
            if let Some(i) = slot {
                entry.keepers.entry(s.to_string()).or_default().push(i);
            }
        }
        for (s, n) in chosen.into_iter().filter(|(_, n)| *n > 0) {
            *entry.chosen.entry(s).or_insert(0) += n;
        }
        for ((from, to), n) in pairs.into_iter().filter(|(_, n)| *n > 0) {
            // 쌍이 너무 많아지면 새 쌍은 other→other로
            let known = entry.changed.iter().any(|c| c.from == from && c.to == to);
            let (from, to) = if !known && entry.changed.len() >= ps::MAX_DAY_PAIRS { (ps::OTHER, ps::OTHER) } else { (from, to) };
            match entry.changed.iter_mut().find(|c| c.from == from && c.to == to) {
                Some(c) => c.n += n,
                None => entry.changed.push(Change { from: from.into(), to: to.into(), n }),
            }
        }
        let mut v = ps::version_or_other(&r.v);
        if !entry.versions.contains_key(&v) && entry.versions.len() >= ps::MAX_DAY_VERSIONS {
            v = ps::OTHER.to_string();
        }
        *entry.versions.entry(v).or_insert(0) += 1;
        self.seen.entry(day).or_default().insert(hash);
        *self.ip_counts.entry(ip_key).or_insert(0) += 1;
        self.dirty = true;
        Ok(())
    }

    /// `before`(없으면 가장 최신, 날짜 모양이 아니면 빈 쪽)보다 앞선 날들을 최신 날부터, `stats_days` JSON이 `REPLY_MAX`를 넘지 않을 만큼 줌.
    /// 하루는 늘 하나는 들어감 (익명 번호가 가득한 하루는 `REPLY_MAX`를 넘을 수 있지만 메시지 한도 `MAX_FRAME` 안). 더 앞선 날이 남았으면 true
    pub fn days_page(&mut self, now: u64, before: Option<&str>) -> (Vec<StatsDay>, bool) {
        self.prune(now);
        // 날짜 모양이 아닌 `before`는 빈 쪽 (글자 비교로 엉뚱한 날을 주지 않게)
        if before.is_some_and(|b| ps::day_number(b).is_none()) {
            return (Vec::new(), false);
        }
        let mut older = self.data.days.iter().rev().filter(|(d, _)| before.is_none_or(|b| d.as_str() < b)).map(|(_, v)| v);
        // 메시지 틀(`{"t":"stats_days","days":[],"more":false}`)과 쉼표 몫
        let mut size = 48;
        let mut page = Vec::new();
        for day in older.by_ref() {
            let n = serde_json::to_string(day).map_or(usize::MAX, |s| s.len() + 1);
            if !page.is_empty() && size + n > ps::REPLY_MAX {
                return (page, true);
            }
            size += n;
            page.push(day.clone());
        }
        (page, false)
    }

    /// 지난 스냅숏 뒤 바뀐 게 있으면 파일 바이트를 돌려줌 (합계만)
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

    /// `stats.json`을 읽음. 없거나 깨졌으면 빈 통계 (날짜 이름이 이상한 칸과 모르는 칸은 버림, 번호 칸이 망가진 날은 번호만 버림)
    pub fn load(dir: &Path) -> PetStats {
        let Ok(bytes) = std::fs::read(dir.join(STATS_FILE)) else { return PetStats::default() };
        match serde_json::from_slice::<Data>(&bytes) {
            Ok(mut data) => {
                data.days.retain(|d, _| ps::day_number(d).is_some());
                for (d, v) in data.days.iter_mut() {
                    v.day = d.clone();
                    tidy_people(v);
                }
                PetStats { data, ..PetStats::default() }
            }
            Err(_) => {
                screenpet_log::log!(Error, "stats", "load_fail", "file" => STATS_FILE);
                PetStats::default()
            }
        }
    }

    /// 비공개 임시 파일(fsync) → 이름 바꾸기 → 폴더 fsync. `.bak`은 두지 않음 (예전 것이 있으면 지움)
    pub fn save(dir: &Path, bytes: &[u8]) -> io::Result<()> {
        let tmp = dir.join(format!("{STATS_FILE}.tmp"));
        write_private(&tmp, bytes)?;
        std::fs::rename(&tmp, dir.join(STATS_FILE))?;
        let _ = std::fs::remove_file(dir.join(format!("{STATS_FILE}.bak")));
        if let Ok(d) = std::fs::File::open(dir) {
            let _ = d.sync_all();
        }
        Ok(())
    }
}

/// 합계·중복·상한·이름·지우기·쪽·저장 시험임
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
            anon: None,
        }
    }

    /// `i`번째 익명 번호 (16진수 16글자)
    fn anon(i: usize) -> String {
        format!("{i:016x}")
    }

    /// 익명 번호를 단 보고
    fn with_anon(r: StatsReport, id: &str) -> StatsReport {
        StatsReport { anon: Some(id.to_string()), ..r }
    }

    #[test]
    fn anon_numbers_tell_people_apart_per_day_and_kind_old_reports_still_count() {
        let mut s = PetStats::default();
        let (a, b) = (anon(0xa), anon(0xb));
        s.submit("k1", "1.1.1.1", &with_anon(report(DAY, &["cat", "penguin", "Cat"], &[], &[]), &a), NOW).unwrap();
        s.submit("k2", "1.1.1.2", &with_anon(report(DAY, &["cat"], &[], &[]), &b), NOW).unwrap();
        s.submit("k3", "1.1.1.3", &report(DAY, &["bat"], &[], &[]), NOW).unwrap();
        s.submit("k1", "1.1.1.1", &with_anon(report("2026-11-23", &["cat"], &[], &[]), &a), NOW).unwrap();
        let (days, _) = all_pages(&mut s, NOW);
        let d = &days[1];
        assert_eq!(d.reports, 3, "a report without a number still counts in the totals");
        assert_eq!(d.people, vec![a.clone(), b.clone()]);
        assert_eq!(d.keepers, BTreeMap::from([("cat".into(), vec![0, 1]), ("other".into(), vec![0]), ("penguin".into(), vec![0])]));
        assert_eq!(d.kept["bat"], 1);
        assert_eq!((days[0].people.clone(), days[0].keepers["cat"].clone()), (vec![a], vec![0]), "the same number on another day");
    }

    #[test]
    fn the_same_anon_number_counts_once_a_day_even_from_another_key_or_after_a_restart() {
        let dir = tmp_dir("anon-dup");
        let mut s = PetStats::default();
        let r = with_anon(report(DAY, &["cat"], &[], &[]), &anon(7));
        s.submit("k1", "1.1.1.1", &r, NOW).unwrap();
        assert_eq!(s.submit("k2", "2.2.2.2", &r, NOW), Err(Refuse::Dup));
        PetStats::save(&dir, &s.snapshot().unwrap()).unwrap();
        let mut back = PetStats::load(&dir);
        assert_eq!(back.submit("k1", "1.1.1.1", &r, NOW), Err(Refuse::Dup), "the file remembers the day's numbers");
        assert_eq!(back.days_page(NOW, None).0[0].reports, 1);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_malformed_anon_number_is_refused_for_good() {
        let mut s = PetStats::default();
        for bad in ["ABCDEF0123456789", "abc", "0123456789abcdef0", "0123456789abcdeg", "", "../../etc/passwd"] {
            assert_eq!(s.submit("k", "1.1.1.1", &with_anon(report(DAY, &["cat"], &[], &[]), bad), NOW), Err(Refuse::Id), "{bad}");
        }
        assert!(Refuse::Id.is_final());
        assert_eq!(Refuse::Id.as_str(), "id");
        assert!(s.days_page(NOW, None).0.is_empty());
    }

    #[test]
    fn a_day_keeps_only_so_many_numbers_and_still_fits_one_message() {
        let mut s = PetStats::default();
        let kept: Vec<&str> = ps::SPECIES.iter().take(ps::MAX_KEPT).copied().collect();
        for i in 0..ps::MAX_DAY_PEOPLE + 5 {
            let r = with_anon(report(DAY, &kept, &[], &[]), &anon(i));
            s.submit(&format!("p{i}"), &format!("10.{}.{}.1", i / 200, i % 200), &r, NOW).unwrap();
        }
        fill_day(&mut s, DAY, NOW);
        let (days, more) = s.days_page(NOW, None);
        let d = &days[0];
        assert!(!more);
        assert_eq!(d.people.len(), ps::MAX_DAY_PEOPLE, "the rest count in the totals only");
        assert!(d.reports as usize > ps::MAX_DAY_PEOPLE + 5);
        assert!(d.keepers.values().all(|v| v.len() <= ps::MAX_DAY_PEOPLE && v.iter().all(|i| (*i as usize) < d.people.len())));
        let size = serde_json::to_string(&screenpet_proto::relay::ServerMsg::StatsDays { days: days.clone(), more }).unwrap().len();
        assert!(size + 4096 <= screenpet_proto::MAX_FRAME, "the fullest day alone fits one message: {size}");
    }

    /// 쪽을 모두 넘겨 모은 날들 (최신 먼저)과 쪽마다의 JSON 크기
    fn all_pages(s: &mut PetStats, now: u64) -> (Vec<StatsDay>, Vec<usize>) {
        let (mut all, mut sizes, mut before) = (Vec::new(), Vec::new(), None::<String>);
        loop {
            let (days, more) = s.days_page(now, before.as_deref());
            sizes.push(serde_json::to_string(&screenpet_proto::relay::ServerMsg::StatsDays { days: days.clone(), more }).unwrap().len());
            before = days.last().map(|d| d.day.clone());
            all.extend(days);
            if !more {
                return (all, sizes);
            }
            assert!(sizes.len() < 200, "paging must end");
        }
    }

    #[test]
    fn reports_add_up_per_day_without_who_sent_them() {
        let mut s = PetStats::default();
        assert!(s.submit("k1", "1.1.1.1", &report(DAY, &["cat", "penguin"], &[("penguin", 1)], &[("cat", "bat", 2)]), NOW).is_ok());
        assert!(s.submit("k2", "1.1.1.2", &report(DAY, &["cat"], &[], &[("cat", "bat", 1)]), NOW).is_ok());
        assert!(s.submit("k1", "1.1.1.1", &report("2026-11-23", &["bat"], &[], &[]), NOW).is_ok(), "another day is another report");
        let (days, _) = all_pages(&mut s, NOW);
        assert_eq!(days.iter().map(|d| d.day.as_str()).collect::<Vec<_>>(), vec!["2026-11-23", DAY], "newest first");
        let d = &days[1];
        assert_eq!(d.reports, 2);
        assert_eq!(d.kept, BTreeMap::from([("cat".into(), 2), ("penguin".into(), 1)]));
        assert_eq!(d.chosen, BTreeMap::from([("penguin".into(), 1)]));
        assert_eq!(d.changed, vec![Change { from: "cat".into(), to: "bat".into(), n: 3 }]);
        assert_eq!(d.versions, BTreeMap::from([("0.7.4".into(), 2)]));
    }

    #[test]
    fn the_saved_file_holds_only_totals_nothing_tied_to_a_key() {
        let mut s = PetStats::default();
        let keys: Vec<String> = (0..5u8).map(|i| screenpet_crypto::to_hex(&[i; 32])).collect();
        for (i, k) in keys.iter().enumerate() {
            s.submit(k, "1.1.1.1", &with_anon(report(DAY, &["cat"], &[], &[]), &anon(0xfeed_0000 + i)), NOW).unwrap();
        }
        let file = String::from_utf8(s.snapshot().unwrap()).unwrap();
        let v: serde_json::Value = serde_json::from_str(&file).unwrap();
        assert_eq!(v.as_object().unwrap().keys().collect::<Vec<_>>(), vec!["days"], "no seen hashes on disk: {file}");
        for k in &keys {
            assert!(!file.contains(k.as_str()) && !file.contains(&k[..16]), "{file}");
            // 비밀 없이 키에서 만들 수 있는 어떤 해시도 없음 (예전 모양: 꼬리표·날짜·키)
            let old = screenpet_crypto::to_hex(&screenpet_crypto::blake2s(&[b"screenpet-stats/1", DAY.as_bytes(), k.as_bytes()])[..16]);
            assert!(!file.contains(&old));
        }
        assert!(!file.contains("1.1.1.1"));
        // 16진수 덩어리는 앱이 보낸 익명 번호뿐 (키에서 만든 값 없음)
        let mut blobs: Vec<String> = file.split('"').filter(|w| w.len() >= 16 && w.chars().all(|c| c.is_ascii_hexdigit())).map(String::from).collect();
        blobs.sort();
        assert_eq!(blobs, (0..5).map(|i| anon(0xfeed_0000 + i)).collect::<Vec<_>>(), "{file}");
    }

    #[test]
    fn the_same_install_counts_once_a_day_while_the_relay_runs() {
        let mut s = PetStats::default();
        let r = report(DAY, &["cat"], &[], &[]);
        assert!(s.submit("k1", "1.1.1.1", &r, NOW).is_ok());
        assert_eq!(s.submit("k1", "2.2.2.2", &r, NOW + 60), Err(Refuse::Dup), "even from another address");
        assert_eq!(s.days_page(NOW, None).0[0].reports, 1);
        // 해시는 날마다 새 비밀로 만듦 (메모리에만) — 같은 키도 날이 다르면 다른 값, 다른 저장소면 다른 값
        let other = PetStats::default();
        assert_ne!(s.key_hash(DAY, "k1"), s.key_hash("2026-11-23", "k1"));
        let mut other = other;
        assert_ne!(s.key_hash(DAY, "k1"), other.key_hash(DAY, "k1"), "a restart forgets who sent (weaker dedupe on purpose)");
    }

    #[test]
    fn bad_days_and_big_reports_are_final_busy_ones_can_retry() {
        let mut s = PetStats::default();
        let ok = report(DAY, &["cat"], &[], &[]);
        for bad in ["2026-11-24", "2026-11-19", "2026-02-30", "yesterday", ""] {
            assert_eq!(s.submit("k", "1.1.1.1", &report(bad, &["cat"], &[], &[]), NOW), Err(Refuse::Day), "{bad}");
        }
        assert!(s.submit("k0", "1.1.1.1", &report("2026-11-20", &[], &[], &[]), NOW).is_ok(), "three days behind UTC is fine (UTC-12 local day, two days late)");
        let many: Vec<String> = (0..=ps::MAX_KEPT).map(|i| format!("x{i}")).collect();
        assert_eq!(s.submit("k", "1.1.1.1", &StatsReport { kept: many, ..ok.clone() }, NOW), Err(Refuse::Size));
        let big = StatsReport { chosen: (0..=ps::MAX_CHOSEN).map(|i| (format!("x{i}"), 1)).collect(), ..ok.clone() };
        assert_eq!(s.submit("k", "1.1.1.1", &big, NOW), Err(Refuse::Size));
        let big = StatsReport { changed: (0..=ps::MAX_CHANGED).map(|_| Change { from: "a".into(), to: "b".into(), n: 1 }).collect(), ..ok.clone() };
        assert_eq!(s.submit("k", "1.1.1.1", &big, NOW), Err(Refuse::Size));
        assert!(s.submit("new", "1.1.1.1", &ok, NOW).is_ok(), "a first-day install counts (it reports right after its first midnight)");
        for i in 0..PER_IP {
            assert!(s.submit(&format!("ip{i}"), "9.9.9.9", &ok, NOW).is_ok());
        }
        assert_eq!(s.submit("late", "9.9.9.9", &ok, NOW), Err(Refuse::Ip));
        assert!(s.submit("late", "9.9.9.9", &report("2026-11-23", &[], &[], &[]), NOW).is_ok(), "per day");
        for r in [Refuse::Day, Refuse::Size, Refuse::Dup] {
            assert!(r.is_final(), "{r:?}: answered, the app drops it");
        }
        for r in [Refuse::Ip, Refuse::Full] {
            assert!(!r.is_final(), "{r:?}: not answered, the app tries again on its next connection");
        }
    }

    #[test]
    fn odd_names_become_other_and_counts_are_capped() {
        let mut s = PetStats::default();
        let r = StatsReport {
            v: "<b>hi</b>".into(),
            ..report(DAY, &["cat", "Cat", "dragon", "<img>"], &[("unicorn", 5), ("phoenix", 999), ("cat", u32::MAX)], &[("cat", "dragon", u32::MAX), ("x", "y", 3), ("z", "w", 0)])
        };
        assert!(s.submit("k", "1.1.1.1", &r, NOW).is_ok());
        let d = &s.days_page(NOW, None).0[0];
        assert_eq!(d.kept, BTreeMap::from([("cat".into(), 1), ("other".into(), 1)]), "other once per report");
        assert_eq!(d.chosen, BTreeMap::from([("cat".into(), ps::MAX_COUNT), ("other".into(), ps::MAX_COUNT)]));
        assert_eq!(
            d.changed,
            vec![Change { from: "cat".into(), to: "other".into(), n: ps::MAX_COUNT }, Change { from: "other".into(), to: "other".into(), n: 3 }]
        );
        assert_eq!(d.versions, BTreeMap::from([("other".into(), 1)]));
    }

    /// 하루를 가장 크게 채움 (쌍·버전·종류 모두 상한까지)
    fn fill_day(s: &mut PetStats, day: &str, now: u64) {
        let sp = ps::SPECIES;
        let mut n = 0;
        'outer: for a in sp {
            for b in sp {
                if n >= ps::MAX_DAY_PAIRS + 3 {
                    break 'outer;
                }
                let kept: Vec<&str> = sp.iter().skip(n % 12).take(ps::MAX_KEPT).copied().collect();
                let chosen: Vec<(&str, u32)> = sp.iter().skip(n % 4).take(ps::MAX_CHOSEN).map(|s| (*s, ps::MAX_COUNT)).collect();
                let r = StatsReport { v: format!("{}.{}.{}", 100 + n, 100 + n, 100 + n), ..report(day, &kept, &chosen, &[(a, b, ps::MAX_COUNT)]) };
                s.submit(&format!("k{n}"), &format!("10.0.{}.{}", n / 200, n % 200), &r, now).unwrap();
                n += 1;
            }
        }
    }

    #[test]
    fn a_day_holds_only_so_many_pairs_and_versions() {
        let mut s = PetStats::default();
        fill_day(&mut s, DAY, NOW);
        let d = &s.days_page(NOW, None).0[0];
        assert!(d.changed.len() <= ps::MAX_DAY_PAIRS + 1, "{}", d.changed.len());
        assert!(d.changed.iter().any(|c| c.from == "other" && c.to == "other" && c.n >= 3));
        assert!(d.versions.len() <= ps::MAX_DAY_VERSIONS + 1);
        assert!(d.versions["other"] >= 3);
    }

    #[test]
    fn ninety_full_days_come_in_pages_under_the_size_limit() {
        let mut s = PetStats::default();
        // 날마다 그날 시각으로 채움 (보고는 그날 근처에만 받으니)
        for i in 0..ps::KEEP_DAYS {
            let now = NOW - (i as u64) * 86_400;
            fill_day(&mut s, &ps::day_string(ps::utc_day(now)), now);
        }
        let (days, sizes) = all_pages(&mut s, NOW);
        assert_eq!(days.len(), ps::KEEP_DAYS as usize, "every day, once");
        assert!(days.windows(2).all(|w| w[0].day > w[1].day), "newest first, no repeats");
        assert!(sizes.len() > 1, "more than one page: {sizes:?}");
        assert!(sizes.iter().all(|n| *n <= ps::REPLY_MAX), "{sizes:?}");
        assert!(ps::REPLY_MAX * 2 <= screenpet_proto::MAX_FRAME);
        assert_eq!(s.days_page(NOW, Some("1999-01-01")), (vec![], false));
        assert_eq!(s.days_page(NOW, Some("nonsense")).0.len(), 0);
    }

    #[test]
    fn old_days_and_in_memory_dedupe_are_dropped_and_a_prune_is_saved() {
        let mut s = PetStats::default();
        s.submit("k", "1.1.1.1", &report(DAY, &["cat"], &[], &[]), NOW).unwrap();
        s.snapshot().unwrap();
        let later = NOW + 4 * 86_400;
        s.prune(later);
        assert_eq!(s.days_page(later, None).0.len(), 1, "the total stays");
        assert!(s.seen.is_empty() && s.secrets.is_empty() && s.ip_counts.is_empty(), "who sent it is forgotten once the day takes no more reports");
        assert_eq!(s.days_page(NOW + 88 * 86_400, None).0.len(), 1, "89 days old: still kept");
        s.prune(NOW + 89 * 86_400);
        assert!(s.days_page(NOW + 89 * 86_400, None).0.is_empty(), "90 days old: gone (the last 90 days are kept)");
        assert!(s.snapshot().is_some(), "a prune alone gets written (an idle relay forgets too)");
    }

    #[test]
    fn it_survives_a_restart_without_a_backup_file() {
        let dir = tmp_dir("restart");
        let mut s = PetStats::default();
        s.submit("k1", "1.1.1.1", &report(DAY, &["cat"], &[], &[]), NOW).unwrap();
        s.submit("k2", "1.1.1.2", &report(DAY, &["bat"], &[], &[]), NOW).unwrap();
        std::fs::write(dir.join(format!("{STATS_FILE}.bak")), b"old").unwrap();
        PetStats::save(&dir, &s.snapshot().unwrap()).unwrap();
        assert!(s.snapshot().is_none(), "clean after a snapshot");
        assert!(!dir.join(format!("{STATS_FILE}.bak")).exists(), "no backup copy to diff against");
        let mut back = PetStats::load(&dir);
        assert_eq!(back.days_page(NOW, None).0[0].reports, 2);
        std::fs::write(dir.join(STATS_FILE), br#"{"days":{"nope":{"reports":5},"2026-11-22":{"reports":2}}, "seen":{"2026-11-22":["ab"]}}"#).unwrap();
        let mut odd = PetStats::load(&dir);
        let days = odd.days_page(NOW, None).0;
        assert_eq!((days.len(), days[0].day.as_str(), days[0].reports), (1, DAY, 2), "bad day names are dropped, day is filled in, old seen is ignored");
        assert!(odd.snapshot().is_none());
        // 망가진 번호 칸은 그날 번호만 버림 (합계는 둠)
        std::fs::write(
            dir.join(STATS_FILE),
            br#"{"days":{"2026-11-22":{"reports":2,"people":["0000000000000001","BAD"],"keepers":{"cat":[0]}},"2026-11-21":{"reports":1,"people":["0000000000000001"],"keepers":{"cat":[3]}},"2026-11-20":{"reports":1,"people":["0000000000000001"],"keepers":{"cat":[0]}}}}"#,
        )
        .unwrap();
        let days = PetStats::load(&dir).days_page(NOW, None).0;
        assert_eq!(days.iter().map(|d| (d.reports, d.people.len(), d.keepers.len())).collect::<Vec<_>>(), vec![(2, 0, 0), (1, 0, 0), (1, 1, 1)]);
        std::fs::write(dir.join(STATS_FILE), b"{torn").unwrap();
        assert!(PetStats::load(&dir).days_page(NOW, None).0.is_empty(), "a torn file: empty stats");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
