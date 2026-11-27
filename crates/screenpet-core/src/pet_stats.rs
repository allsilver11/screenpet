//! 📊 익명 펫 통계 모으기 (spec 2026-11-23-pet-stats) — 설정(`config.json` `pet_stats`)에 남는 하루치 기록과 보낼 보고.
//! 동의(`consent`)가 `Some(true)`일 때만 셈: 답하기 전(None)이나 안 보낸다고 했으면(Some(false)) 아무것도 모으지 않음.
//! 날짜(앱의 그 지역 날짜)가 바뀌면 지난날 기록이 보고 하나가 되고 (더 새 날이 오면 바뀜), 릴레이가 `stats_ok`로 받으면 지움.
//! 담는 것: 그날 키운 종류들, 새로 고른 종류와 횟수, 바꾼 것(from→to) 횟수, 앱 버전, 익명 번호(`anon`) — 이름·네트워크 id·친구는 없음.
//! 익명 번호: 동의하면 한 번 무작위로 만들어 설정에 둠 (네트워크 키·친구 id와 상관없음). 누구인지는 모르지만
//! 서로 다른 사람끼리는 구분해 세게 함. 안 보내기로 하면 지우고, 다시 켜면 새로 만듦.

use screenpet_proto::pet_stats::{self as ps, Change, StatsReport};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

/// 하루치 기록 (그날 안에서만 늘어남)
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct DayTally {
    /// 날짜 (`YYYY-MM-DD`, 지역 날짜)
    pub day: String,
    /// 그날 키운 종류들
    pub kept: BTreeSet<String>,
    /// 새로 고른 종류 → 횟수
    pub chosen: BTreeMap<String, u32>,
    /// 바꾼 것들 (같은 쌍은 합침)
    pub changed: Vec<Change>,
}

/// 📊 통계 장부: 동의, 오늘 기록, 아직 못 보낸 지난날 보고
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct StatsBook {
    /// 보내도 되는지: None = 아직 묻지 않음 (처음 켤 때 묻는 창), Some(답)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub consent: Option<bool>,
    /// 오늘 기록 (동의했을 때만)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub today: Option<DayTally>,
    /// 보낼 지난날 보고 (릴레이가 받으면 지움)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pending: Option<StatsReport>,
    /// 새로 설치해 고른 첫 펫을 아직 못 셈 (답이 다음 실행에 와도 그때 "새로 고름"으로 셈)
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub first_pick: bool,
    /// 이 설치의 익명 번호 (동의했을 때만, 소문자 16진수 16글자 — 끄면 지움)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub anon: Option<String>,
}

/// 새 익명 번호: 64비트 무작위를 소문자 16진수 16글자로 (어떤 id에서도 만들지 않음)
fn new_anon() -> String {
    format!("{:016x}", fastrand::u64(..))
}

impl StatsBook {
    /// 아무것도 없음 (설정에 쓰지 않음)
    pub fn is_empty(&self) -> bool {
        *self == StatsBook::default()
    }

    /// 보내기로 했음
    pub fn on(&self) -> bool {
        self.consent == Some(true)
    }

    /// 묻는 창의 답이나 ⚙️ 설정 체크를 적음. 안 보내기로 하면 모아 둔 것도 지움. 바뀌었으면 true
    pub fn set_consent(&mut self, yes: bool) -> bool {
        let before = self.clone();
        self.consent = Some(yes);
        if yes {
            self.ensure_anon();
        } else {
            self.today = None;
            self.pending = None;
            self.anon = None;
        }
        *self != before
    }

    /// 익명 번호가 없거나 모양이 틀리면 새로 만들고, 번호 없는 보낼 보고(옛 버전이 만든 것)에도 붙임. 바뀌었으면 true
    fn ensure_anon(&mut self) -> bool {
        let mut changed = false;
        if !self.anon.as_deref().is_some_and(ps::is_anon) {
            self.anon = Some(new_anon());
            changed = true;
        }
        if let Some(p) = self.pending.as_mut().filter(|p| p.anon.is_none()) {
            p.anon = self.anon.clone();
            changed = true;
        }
        changed
    }

    /// 새로 설치한 실행임: 아직 답이 없으면 첫 펫을 답할 때 "새로 고름"으로 셀 수 있게 적어 둠 (이미 답한 설치는 아님)
    pub fn mark_first_pick(&mut self) {
        if self.consent.is_none() {
            self.first_pick = true;
        }
    }

    /// 묻는 창의 답이나 ⚙️ 설정 체크: 동의를 적고, 보내기로 했고 첫 펫을 아직 못 셌으면 지금 키우는 종류를 "새로 고름"으로 셈.
    /// 첫 펫 표시는 어느 답이든 지움. 바뀌었으면 true
    pub fn answer(&mut self, yes: bool, today: &str, version: &str, species: &[&str]) -> bool {
        let first = std::mem::take(&mut self.first_pick);
        let mut changed = self.set_consent(yes) || first;
        if yes && first {
            for s in species {
                changed |= self.note_chosen(today, version, s);
            }
        }
        changed
    }

    /// 날짜를 맞춤: 익명 번호를 챙기고, 오늘 기록이 지난날이면 보고로 바꾸고 (이틀보다 더 지난 날은 릴레이도 안 받으니 버림) 오늘을 새로 엶.
    /// 동의 안 했으면 아무 일도 없음. 바뀌었으면 true
    pub fn roll(&mut self, today: &str, version: &str) -> bool {
        if !self.on() || ps::day_number(today).is_none() {
            return false;
        }
        let made = self.ensure_anon();
        if self.today.as_ref().is_some_and(|t| t.day == today) {
            return made;
        }
        if let Some(old) = self.today.take() {
            let fresh = ps::day_number(&old.day).zip(ps::day_number(today)).is_some_and(|(o, t)| o < t && t - o <= ps::LATE_DAYS);
            if fresh {
                self.pending = Some(StatsReport { day: old.day, v: version.to_string(), kept: old.kept.into_iter().collect(), chosen: old.chosen, changed: old.changed, anon: self.anon.clone() });
            }
        }
        // 보낼 보고도 너무 지났으면 버림
        if self.pending.as_ref().is_some_and(|p| ps::day_number(&p.day).zip(ps::day_number(today)).is_none_or(|(p, t)| t - p > ps::LATE_DAYS)) {
            self.pending = None;
        }
        self.today = Some(DayTally { day: today.to_string(), ..DayTally::default() });
        true
    }

    /// 오늘 기록 (날짜를 맞춘 뒤 — 동의 안 했으면 None)
    fn tally(&mut self, today: &str, version: &str) -> Option<&mut DayTally> {
        self.roll(today, version);
        if !self.on() {
            return None;
        }
        self.today.as_mut().filter(|t| t.day == today)
    }

    /// 지금 키우는 종류들을 오늘 기록에 넣음 (상한까지). 바뀌었으면 true
    pub fn note_kept(&mut self, today: &str, version: &str, species: &[&str]) -> bool {
        let rolled = self.roll(today, version);
        let Some(t) = self.tally(today, version) else { return rolled };
        let before = t.kept.len();
        for s in species {
            if t.kept.len() < ps::MAX_KEPT {
                t.kept.insert(s.to_string());
            }
        }
        rolled || t.kept.len() != before
    }

    /// 새로 골랐음 (펫 추가·처음 고르기). 바뀌었으면 true
    pub fn note_chosen(&mut self, today: &str, version: &str, species: &str) -> bool {
        let Some(t) = self.tally(today, version) else { return false };
        if !t.chosen.contains_key(species) && t.chosen.len() >= ps::MAX_CHOSEN {
            return false;
        }
        let n = t.chosen.entry(species.to_string()).or_insert(0);
        *n = (*n + 1).min(ps::MAX_COUNT);
        if t.kept.len() < ps::MAX_KEPT {
            t.kept.insert(species.to_string());
        }
        true
    }

    /// 종류를 바꿨음 (`from` → `to`). 같으면 아무 일도 없음. 바뀌었으면 true
    pub fn note_changed(&mut self, today: &str, version: &str, from: &str, to: &str) -> bool {
        if from == to {
            return false;
        }
        let Some(t) = self.tally(today, version) else { return false };
        match t.changed.iter().position(|c| c.from == from && c.to == to) {
            Some(i) => t.changed[i].n = (t.changed[i].n + 1).min(ps::MAX_COUNT),
            None if t.changed.len() < ps::MAX_CHANGED => t.changed.push(Change { from: from.into(), to: to.into(), n: 1 }),
            None => return false,
        }
        if t.kept.len() < ps::MAX_KEPT {
            t.kept.insert(to.to_string());
        }
        true
    }

    /// 보낼 보고 (동의했을 때만)
    pub fn report(&self) -> Option<&StatsReport> {
        self.pending.as_ref().filter(|_| self.on())
    }

    /// 릴레이가 `day` 보고를 받았음: 그 보고를 지움. 지웠으면 true
    pub fn sent(&mut self, day: &str) -> bool {
        if self.pending.as_ref().is_some_and(|p| p.day == day) {
            self.pending = None;
            true
        } else {
            false
        }
    }
}

/// 동의·하루치·날짜 바뀜 시험임
#[cfg(test)]
mod tests {
    use super::*;

    const V: &str = "0.7.4";

    fn yes() -> StatsBook {
        let mut b = StatsBook::default();
        b.set_consent(true);
        b
    }

    #[test]
    fn nothing_is_gathered_or_reported_before_consent() {
        let mut b = StatsBook::default();
        assert!(!b.note_kept("2026-11-22", V, &["cat"]));
        assert!(!b.note_chosen("2026-11-22", V, "cat"));
        assert!(!b.note_changed("2026-11-22", V, "cat", "bat"));
        assert!(!b.roll("2026-11-23", V));
        assert!(b.report().is_none() && b.is_empty());
        b.set_consent(false);
        assert!(!b.note_kept("2026-11-22", V, &["cat"]));
        assert_eq!(b, StatsBook { consent: Some(false), ..StatsBook::default() });
    }

    #[test]
    fn a_day_of_choosing_changing_and_several_pets_becomes_yesterdays_report() {
        let mut b = yes();
        assert!(b.note_kept("2026-11-22", V, &["cat", "penguin"]));
        assert!(!b.note_kept("2026-11-22", V, &["cat", "penguin"]), "nothing new");
        assert!(b.note_chosen("2026-11-22", V, "rabbit"));
        assert!(b.note_changed("2026-11-22", V, "cat", "bat"));
        assert!(b.note_changed("2026-11-22", V, "cat", "bat"));
        assert!(!b.note_changed("2026-11-22", V, "bat", "bat"));
        assert!(b.report().is_none(), "nothing to send during the day");
        assert!(b.roll("2026-11-23", "0.7.5"));
        let r = b.report().unwrap();
        assert_eq!(r.day, "2026-11-22");
        assert_eq!(r.v, "0.7.5");
        assert_eq!(r.kept, vec!["bat", "cat", "penguin", "rabbit"]);
        assert_eq!(r.chosen, BTreeMap::from([("rabbit".to_string(), 1)]));
        assert_eq!(r.changed, vec![Change { from: "cat".into(), to: "bat".into(), n: 2 }]);
        assert_eq!(b.today.as_ref().unwrap().day, "2026-11-23", "a fresh day");
        assert!(b.today.as_ref().unwrap().kept.is_empty());
        assert!(!b.sent("2026-11-21") && b.report().is_some());
        assert!(b.sent("2026-11-22") && b.report().is_none());
    }

    #[test]
    fn a_newer_day_replaces_an_unsent_report_and_stale_days_are_dropped() {
        let mut b = yes();
        b.note_kept("2026-11-20", V, &["cat"]);
        b.note_kept("2026-11-21", V, &["bat"]);
        b.note_kept("2026-11-22", V, &["penguin"]);
        assert_eq!(b.report().unwrap().kept, vec!["bat"], "only the last finished day");
        b.roll("2026-11-25", V);
        assert!(b.report().is_none(), "three days on: too late for the relay");
        b.note_kept("2026-11-25", V, &["cat"]);
        b.roll("2026-11-27", V);
        assert_eq!(b.report().unwrap().day, "2026-11-25", "two days on is still fine");
        assert!(!b.roll("not a day", V));
    }

    #[test]
    fn the_first_pet_of_a_new_install_counts_as_chosen_even_if_the_answer_comes_a_run_later() {
        let mut b = StatsBook::default();
        b.mark_first_pick();
        // 창을 X로 닫음: 답 없이 설정에 남아 다음 실행으로
        let mut b: StatsBook = serde_json::from_str(&serde_json::to_string(&b).unwrap()).unwrap();
        assert!(b.first_pick && b.report().is_none());
        assert!(b.answer(true, "2026-11-23", V, &["cat", "penguin"]));
        let t = b.today.clone().unwrap();
        assert_eq!(t.chosen, BTreeMap::from([("cat".to_string(), 1), ("penguin".to_string(), 1)]));
        assert!(!b.first_pick, "only once");
        assert!(!b.answer(true, "2026-11-23", V, &["cat"]), "nothing new");
        assert_eq!(b.today.as_ref().unwrap().chosen["cat"], 1);
        // 이미 답한 설치(업데이트한 사용자)는 첫 펫으로 치지 않음
        let mut old = yes();
        old.mark_first_pick();
        assert!(!old.first_pick);
        // 안 보낸다고 하면 그 표시도 지움
        let mut no = StatsBook::default();
        no.mark_first_pick();
        no.answer(false, "2026-11-23", V, &["cat"]);
        assert_eq!(no, StatsBook { consent: Some(false), ..StatsBook::default() });
    }

    #[test]
    fn consenting_makes_a_random_anon_number_that_rides_every_report_and_saying_no_deletes_it() {
        let b = yes();
        let id = b.anon.clone().expect("made on yes");
        assert!(ps::is_anon(&id), "{id}");
        // 설정에 남아 다음 실행에도 같은 번호
        let mut b: StatsBook = serde_json::from_str(&serde_json::to_string(&b).unwrap()).unwrap();
        assert_eq!(b.anon.as_deref(), Some(id.as_str()));
        b.set_consent(true);
        assert_eq!(b.anon.as_deref(), Some(id.as_str()), "saying yes again keeps it");
        b.note_kept("2026-11-22", V, &["cat"]);
        b.roll("2026-11-23", V);
        assert_eq!(b.report().unwrap().anon.as_deref(), Some(id.as_str()));
        // 끄면 번호도 지우고, 다시 켜면 새 번호
        assert!(b.set_consent(false));
        assert_eq!(b.anon, None);
        b.set_consent(true);
        let again = b.anon.clone().unwrap();
        assert!(ps::is_anon(&again) && again != id);
        assert_ne!(yes().anon, yes().anon, "random per install");
        assert_eq!(StatsBook::default().anon, None, "nothing before an answer");
    }

    #[test]
    fn an_install_that_said_yes_before_anon_numbers_gets_one_and_its_unsent_report_too() {
        // 옛 설정: 동의했고 보낼 보고가 남았지만 번호가 없음
        let old = r#"{"consent":true,"today":{"day":"2026-11-23"},"pending":{"day":"2026-11-22","v":"0.7.4","kept":["cat"]}}"#;
        let mut b: StatsBook = serde_json::from_str(old).unwrap();
        assert_eq!(b.anon, None);
        assert!(b.roll("2026-11-23", V), "a number is made: save it");
        let id = b.anon.clone().unwrap();
        assert!(ps::is_anon(&id));
        assert_eq!(b.report().unwrap().anon.as_deref(), Some(id.as_str()));
        assert!(!b.roll("2026-11-23", V), "only once");
    }

    #[test]
    fn saying_no_drops_what_was_gathered_and_counts_are_capped() {
        let mut b = yes();
        for _ in 0..100 {
            b.note_chosen("2026-11-22", V, "cat");
        }
        for i in 0..40 {
            b.note_changed("2026-11-22", V, "cat", &format!("x{i}"));
        }
        let t = b.today.clone().unwrap();
        assert_eq!(t.chosen["cat"], ps::MAX_COUNT);
        assert_eq!(t.changed.len(), ps::MAX_CHANGED);
        assert!(t.kept.len() <= ps::MAX_KEPT);
        b.roll("2026-11-23", V);
        assert!(b.set_consent(false));
        assert!(b.report().is_none() && b.today.is_none() && b.pending.is_none());
        let json = serde_json::to_string(&b).unwrap();
        assert_eq!(json, r#"{"consent":false}"#);
    }
}
