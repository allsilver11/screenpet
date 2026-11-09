//! 📱 한 사람의 두 컴퓨터 (내 다른 컴퓨터).
//!
//! - **친구 쪽에서는 한 사람.** 친구 X와 X가 소개한 X의 다른 컴퓨터(`via = X`)는 메뉴에서 한 줄이다:
//!   X의 이름(별명도 함께), 둘 중 하나라도 연결돼 있으면 ●. 보내기·편지·데리고 오기·던지기는
//!   지금 노는 컴퓨터(연결된 쪽, 둘 다면 나중에 연결된 쪽, 아무도 없으면 X)로 간다. [`group`]
//! - **내 쪽에서는 한 번에 하나만 논다.** 대기 중인 컴퓨터에서 사용자가 다시 쓰기 시작하면
//!   (한동안 안 쓰다가 몇 초 동안 계속 쓰면) 이 컴퓨터로 놀겠다고 알린다. [`ClaimWatch`]
//!   두 컴퓨터를 번갈아 만져도 왔다 갔다 하지 않게: 대기가 된 뒤 한동안(`IDLE_BEFORE_CLAIM_SECS`)
//!   안 쓴 적이 있어야 하고, 그 뒤 `CLAIM_AFTER_SECS` 동안 계속 써야 한다.
//! - **펫과 성장은 두 컴퓨터가 같다.** 노는 컴퓨터가 고른 펫과 모든 펫의 돌보기 상태를 보내고
//!   ([`shared_care`]), 쉬는 컴퓨터가 받아 합친다 ([`adopt_shared`]: 아는 동물만, 값은 다듬어서).

use std::collections::HashMap;

use crate::care::{Care, Stage};

/// 대기 중인 컴퓨터를 이만큼(초) 안 쓴 적이 있어야 다시 쓸 때 저절로 넘어온다
pub const IDLE_BEFORE_CLAIM_SECS: f64 = 30.0;
/// … 그리고 이만큼 계속 써야 한다
pub const CLAIM_AFTER_SECS: f64 = 4.0;
/// 유휴 시간이 이보다 짧으면 지금 쓰고 있는 것 (1초에 한 번 보므로 넉넉히)
pub const USING_BELOW_SECS: f64 = crate::presence::BACK_BELOW_SECS;

/// 대기 중인 이 컴퓨터를 사용자가 다시 쓰기 시작했는지 본다 (앱이 1초에 한 번 부른다)
#[derive(Debug, Clone, Copy, Default)]
pub struct ClaimWatch {
    was_idle: bool,
    using_from: Option<f64>,
}

impl ClaimWatch {
    /// now = 초 (늘어나기만), idle = 마지막 입력 뒤 초 (None = 알 수 없음), standby = 이 컴퓨터가 대기 중.
    /// true = 지금 이 컴퓨터로 놀겠다고 알린다
    pub fn observe(&mut self, now: f64, idle: Option<f64>, standby: bool) -> bool {
        if !standby {
            *self = ClaimWatch::default();
            return false;
        }
        let Some(idle) = idle.filter(|i| i.is_finite() && *i >= 0.0) else { return false };
        if idle >= IDLE_BEFORE_CLAIM_SECS {
            self.was_idle = true;
            self.using_from = None;
            return false;
        }
        if !self.was_idle {
            return false;
        }
        if idle >= USING_BELOW_SECS {
            self.using_from = None;
            return false;
        }
        let from = *self.using_from.get_or_insert(now - idle);
        if now - from >= CLAIM_AFTER_SECS {
            *self = ClaimWatch::default();
            return true;
        }
        false
    }
}

/// 내 동물·물건을 보낼 수 있는 곳: 연결됐고, 대기 중인 내 다른 컴퓨터(standby_device)가 아님.
/// (대기 중인 컴퓨터의 펫을 데려오기, 그 펫이 집에 가기는 이 규칙과 상관없다)
pub fn playable(id: &str, connected: bool, standby_device: Option<&str>) -> bool {
    connected && standby_device != Some(id)
}

/// 친구 하나 (설정 순서대로): id, 소개한 친구, 연결된 때 (클수록 나중, None = 연결 안 됨)
#[derive(Debug, Clone, Copy)]
pub struct Member<'a> {
    pub id: &'a str,
    pub via: Option<&'a str>,
    pub connected: Option<f64>,
}

/// 메뉴의 한 줄 = 한 사람
#[derive(Debug, Clone, PartialEq)]
pub struct Person {
    /// 처음 친구가 된 컴퓨터 (이름·별명·화면 끝·연결 끊기는 이 id로)
    pub id: String,
    /// 그 사람의 컴퓨터들 (id가 먼저)
    pub members: Vec<String>,
    /// 둘 중 하나라도 연결됨
    pub connected: bool,
    /// 보내기·편지·데리고 오기·던지기가 갈 컴퓨터
    pub target: String,
}

/// 이 친구가 속한 사람(처음 친구)의 id: 소개받은 컴퓨터면 소개한 친구 (한 다리만 — 소개한 친구도 소개받은 것이면 따로)
pub fn person_id<'a>(members: &[Member<'a>], id: &'a str) -> &'a str {
    let via = members.iter().find(|m| m.id == id).and_then(|m| m.via);
    match via.and_then(|v| members.iter().find(|m| m.id == v)) {
        Some(p) if p.via.is_none() && p.id != id => p.id,
        _ => id,
    }
}

/// 친구들을 사람으로 묶는다 (처음 친구의 순서대로)
pub fn group(members: &[Member]) -> Vec<Person> {
    let mut people: Vec<Person> = Vec::new();
    let mut at: Vec<Option<f64>> = Vec::new();
    for m in members.iter().filter(|m| person_id(members, m.id) == m.id) {
        people.push(Person { id: m.id.to_string(), members: vec![m.id.to_string()], connected: false, target: m.id.to_string() });
        at.push(m.connected);
    }
    for m in members.iter().filter(|m| person_id(members, m.id) != m.id) {
        let p = person_id(members, m.id);
        if let Some(i) = people.iter().position(|q| q.id == p) {
            people[i].members.push(m.id.to_string());
            // 둘 다 연결돼 있으면 나중에 연결된 쪽 (지금 놀기 시작한 컴퓨터)
            if m.connected.is_some_and(|t| at[i].is_none_or(|u| t > u)) {
                people[i].target = m.id.to_string();
                at[i] = m.connected;
            }
        }
    }
    for (p, t) in people.iter_mut().zip(&at) {
        p.connected = t.is_some();
    }
    people
}

/// 📱 한 번에 나누는 펫 수 (보내기·받기 모두 — 지금 동물보다 넉넉히)
pub const MAX_SHARED_PETS: usize = 64;

/// 📱 두 컴퓨터가 나누는 한 동물의 돌보기 상태 (선 위의 모양 그대로 — 받은 것은 [`clean_care`]로 다듬는다)
#[derive(Debug, Clone, PartialEq)]
pub struct SharedCare {
    pub pet: String,
    pub stage: String,
    pub stamina: f64,
    pub fullness: f64,
    pub locked: bool,
}

/// 받은 돌보기 상태를 다듬는다: 모르는 단계나 숫자가 아닌 값이면 None,
/// 포만감은 0~1, 체력은 0~그 단계에서 크는 데 필요한 만큼 (어른은 0)
pub fn clean_care(c: &SharedCare) -> Option<Care> {
    let stage = Stage::from_name(&c.stage)?;
    if !c.stamina.is_finite() || !c.fullness.is_finite() {
        return None;
    }
    let need = stage.need().unwrap_or(0.0);
    Some(Care { stage, stamina: c.stamina.clamp(0.0, need), fullness: c.fullness.clamp(0.0, 1.0), locked: c.locked })
}

/// 내 모든 펫의 돌보기 상태를 보낼 모양으로 (펫 이름 순서 — 늘 같은 순서, 많아야 `MAX_SHARED_PETS`)
pub fn shared_care(care: &HashMap<String, Care>) -> Vec<SharedCare> {
    let mut ids: Vec<&String> = care.keys().collect();
    ids.sort();
    ids.into_iter()
        .take(MAX_SHARED_PETS)
        .map(|id| {
            let c = &care[id];
            SharedCare { pet: id.clone(), stage: c.stage.name().to_string(), stamina: c.stamina, fullness: c.fullness, locked: c.locked }
        })
        .collect()
}

/// [`adopt_shared`]의 결과
#[derive(Debug, Clone, PartialEq)]
pub struct Adopted {
    /// 이 동물로 바꾼다 (아는 동물이고 지금 동물과 다를 때만)
    pub switch_to: Option<String>,
    /// 돌보기 상태가 하나라도 바뀌었다
    pub changed: bool,
}

/// 📱 노는 컴퓨터가 보낸 펫과 돌보기 상태를 내 설정(`care`, 지금 동물 `current`)에 합친다.
/// 아는 동물(`known`)의 멀쩡한 값만 덮어쓰고 (같은 동물이 두 번 오면 처음 것), 보내지 않은 동물은 그대로 둔다.
/// 앞의 `MAX_SHARED_PETS`개만 읽는다
pub fn adopt_shared(care: &mut HashMap<String, Care>, current: &str, pet: Option<&str>, shared: &[SharedCare], known: impl Fn(&str) -> bool) -> Adopted {
    let mut seen: Vec<&str> = Vec::new();
    let mut changed = false;
    for s in shared.iter().take(MAX_SHARED_PETS) {
        if seen.contains(&s.pet.as_str()) {
            continue;
        }
        seen.push(&s.pet);
        let Some(c) = clean_care(s).filter(|_| known(&s.pet)) else { continue };
        if care.get(&s.pet) != Some(&c) {
            care.insert(s.pet.clone(), c);
            changed = true;
        }
    }
    let switch_to = pet.filter(|p| *p != current && known(p)).map(str::to_string);
    Adopted { switch_to, changed }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn m<'a>(id: &'a str, via: Option<&'a str>, connected: Option<f64>) -> Member<'a> {
        Member { id, via, connected }
    }

    #[test]
    fn a_friend_and_the_computer_it_introduced_are_one_person() {
        let list = [m("x", None, None), m("y", None, Some(1.0)), m("x2", Some("x"), Some(5.0))];
        let people = group(&list);
        assert_eq!(people.len(), 2);
        assert_eq!(people[0], Person { id: "x".into(), members: vec!["x".into(), "x2".into()], connected: true, target: "x2".into() });
        assert_eq!(people[1], Person { id: "y".into(), members: vec!["y".into()], connected: true, target: "y".into() });
        assert_eq!(person_id(&list, "x2"), "x");
        assert_eq!(person_id(&list, "x"), "x");
        assert_eq!(person_id(&list, "nobody"), "nobody");
    }

    #[test]
    fn the_target_is_the_connected_computer_then_the_one_connected_last_then_the_first() {
        let target = |a: Option<f64>, b: Option<f64>| group(&[m("x", None, a), m("x2", Some("x"), b)])[0].target.clone();
        assert_eq!(target(None, None), "x");
        assert_eq!(target(Some(1.0), None), "x");
        assert_eq!(target(None, Some(1.0)), "x2");
        assert_eq!(target(Some(1.0), Some(2.0)), "x2");
        assert_eq!(target(Some(3.0), Some(2.0)), "x");
        assert!(!group(&[m("x", None, None), m("x2", Some("x"), None)])[0].connected);
    }

    #[test]
    fn an_introducer_that_is_gone_or_introduced_itself_leaves_the_computer_on_its_own() {
        let list = [m("d", Some("gone"), None), m("a", Some("b"), None), m("b", Some("c"), None), m("c", None, None)];
        let people = group(&list);
        let ids: Vec<&str> = people.iter().map(|p| p.id.as_str()).collect();
        assert_eq!(ids, ["d", "a", "c"], "b joins c; a (introduced by b, itself introduced) stays alone");
        assert_eq!(people[2].members, ["c", "b"]);
    }

    #[test]
    fn my_computer_on_standby_is_closed_for_my_pet_and_things() {
        assert!(playable("x", true, None));
        assert!(!playable("x", false, None));
        assert!(playable("x", true, Some("mine")), "others stay open");
        assert!(!playable("mine", true, Some("mine")), "my other computer on standby is not a place to go");
        assert!(playable("mine", true, None), "when it plays (we can't both), or we can't reach each other");
    }

    #[test]
    fn a_standby_computer_claims_only_after_a_quiet_spell_and_a_few_seconds_of_use() {
        let mut w = ClaimWatch::default();
        // 계속 쓰고 있었다 (저쪽이 방금 가져갔다): 저절로 되찾지 않는다
        for t in 0..20 {
            assert!(!w.observe(f64::from(t), Some(0.5), true), "no ping-pong at {t}");
        }
        // 한동안 안 쓰다가 …
        assert!(!w.observe(20.0, Some(IDLE_BEFORE_CLAIM_SECS), true));
        // …잠깐 건드린 것으로는 아니다
        assert!(!w.observe(21.0, Some(0.2), true));
        assert!(!w.observe(22.0, Some(1.2), true));
        assert!(!w.observe(23.0, Some(5.0), true), "a pause starts over");
        // 몇 초 동안 계속 쓰면 넘어온다
        assert!(!w.observe(24.0, Some(0.1), true));
        assert!(!w.observe(26.0, Some(0.3), true));
        assert!(w.observe(28.0, Some(0.2), true));
        // 한 번 넘어오면 다시 조용한 때가 있어야 한다
        assert!(!w.observe(33.0, Some(0.2), true));
    }

    #[test]
    fn a_computer_that_plays_never_claims_and_forgets_the_quiet_spell() {
        let mut w = ClaimWatch::default();
        assert!(!w.observe(0.0, Some(100.0), true));
        assert!(!w.observe(1.0, Some(0.1), false), "playing: nothing to claim");
        for t in 2..10 {
            assert!(!w.observe(f64::from(t), Some(0.1), true), "the quiet spell before playing doesn't count");
        }
        assert!(!w.observe(10.0, None, true), "unknown idle time");
        assert!(!w.observe(11.0, Some(f64::NAN), true));
    }

    fn sc(pet: &str, stage: &str, stamina: f64, fullness: f64, locked: bool) -> SharedCare {
        SharedCare { pet: pet.into(), stage: stage.into(), stamina, fullness, locked }
    }

    #[test]
    fn shared_care_is_clamped_like_a_fed_pet_and_nonsense_is_dropped() {
        use crate::care::Stage;
        assert_eq!(clean_care(&sc("rabbit", "child", 12.0, 0.3, true)), Some(Care { stage: Stage::Child, stamina: 12.0, fullness: 0.3, locked: true }));
        // 범위 밖: 포만감 0~1, 체력 0~그 단계에 필요한 만큼
        assert_eq!(clean_care(&sc("rabbit", "baby", 1e9, 7.0, false)), Some(Care { stage: Stage::Baby, stamina: 30.0, fullness: 1.0, locked: false }));
        assert_eq!(clean_care(&sc("rabbit", "kinder", -5.0, -1.0, false)), Some(Care { stage: Stage::Kinder, stamina: 0.0, fullness: 0.0, locked: false }));
        assert_eq!(clean_care(&sc("rabbit", "adult", 40.0, 0.5, false)).map(|c| c.stamina), Some(0.0), "an adult has nothing left to grow");
        assert_eq!(clean_care(&sc("rabbit", "giant", 1.0, 0.5, false)), None, "unknown stage");
        assert_eq!(clean_care(&sc("rabbit", "baby", f64::NAN, 0.5, false)), None);
        assert_eq!(clean_care(&sc("rabbit", "baby", 1.0, f64::INFINITY, false)), None);
    }

    #[test]
    fn my_care_goes_out_sorted_and_bounded() {
        use crate::care::Stage;
        let mut care = HashMap::new();
        care.insert("rabbit".to_string(), Care { stage: Stage::Adult, ..Care::default() });
        care.insert("cat".to_string(), Care::default());
        let out = shared_care(&care);
        assert_eq!(out.iter().map(|c| c.pet.as_str()).collect::<Vec<_>>(), ["cat", "rabbit"], "same order every time");
        assert_eq!(out[1], sc("rabbit", "adult", 0.0, 1.0, false));
        let many: HashMap<String, Care> = (0..200).map(|i| (format!("p{i:03}"), Care::default())).collect();
        assert_eq!(shared_care(&many).len(), MAX_SHARED_PETS);
    }

    #[test]
    fn adopting_shared_state_overwrites_known_pets_keeps_the_rest_and_switches_only_to_a_known_pet() {
        use crate::care::Stage;
        let known = |p: &str| ["cat", "rabbit", "bird"].contains(&p);
        let mut care = HashMap::new();
        care.insert("cat".to_string(), Care::default());
        care.insert("bird".to_string(), Care { stage: Stage::Teenager, ..Care::default() });
        let shared = [sc("cat", "child", 3.0, 0.2, false), sc("unicorn", "adult", 0.0, 1.0, false), sc("rabbit", "giant", 0.0, 1.0, false), sc("cat", "adult", 0.0, 1.0, false)];
        let got = adopt_shared(&mut care, "cat", Some("rabbit"), &shared, known);
        assert_eq!(got, Adopted { switch_to: Some("rabbit".into()), changed: true });
        assert_eq!(care["cat"], Care { stage: Stage::Child, stamina: 3.0, fullness: 0.2, locked: false }, "the first entry for a pet wins");
        assert_eq!(care["bird"].stage, Stage::Teenager, "pets the other computer did not send stay");
        assert!(!care.contains_key("unicorn"), "a species this build doesn't know is left out");
        assert!(!care.contains_key("rabbit"), "a broken entry is left out");
        // 같은 동물이면 바꾸지 않는다, 모르는 동물로도 바꾸지 않는다
        assert_eq!(adopt_shared(&mut care, "cat", Some("cat"), &[], known), Adopted { switch_to: None, changed: false });
        assert_eq!(adopt_shared(&mut care, "cat", Some("unicorn"), &[], known), Adopted { switch_to: None, changed: false });
        assert_eq!(adopt_shared(&mut care, "cat", None, &[sc("cat", "child", 3.0, 0.2, false)], known), Adopted { switch_to: None, changed: false }, "nothing new");
        let too_many: Vec<SharedCare> = (0..MAX_SHARED_PETS).map(|_| sc("bird", "baby", 0.0, 1.0, false)).chain([sc("cat", "baby", 0.0, 1.0, false)]).collect();
        adopt_shared(&mut care, "cat", None, &too_many, known);
        assert_eq!(care["cat"].stage, Stage::Child, "only the first MAX_SHARED_PETS entries are read");
    }
}
