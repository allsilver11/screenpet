//! 🐾🐾🐾 내 펫 여러 마리 (최대 `MAX_OWN_PETS`) — 설정의 `pets` 목록과 이사·더하기·빼기·종류 바꾸기
//! 돌보기 상태(`Care`)는 종류가 아니라 펫 하나하나(`uid`)에 붙음. 첫 펫 = 대표 (창 라벨 `pet`)이고,
//! 옛 버전으로 되돌려도 첫 펫이 그대로 보이게 옛 필드 `pet`·`care[첫 펫 종류]`도 계속 씀 (`sync_legacy`)
//! 💤 쉬게 하기: 기록은 남기고 화면에서만 빠짐 (`resting` — 돌보기가 멈추고 대표가 되지 않음, 3마리 칸은 그대로 차지).
//! 나온 펫이 늘 한 마리는 있음 (`rest_pet`·`remove_pet`이 막고, 모두 쉬는 설정이면 `settle_pets`가 첫 펫을 깨움)
use serde::{Deserialize, Serialize};

use crate::care::Care;
use crate::config::Config;

/// 한 사람이 같이 키우는 펫 수 상한
pub const MAX_OWN_PETS: usize = 3;

/// 내 펫 한 마리 (설정에 남음)
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OwnPet {
    /// 고유 id (설치 안에서만 씀 — 친구에게 보내지 않음)
    pub uid: String,
    /// 동물 종류 (폴더 이름)
    pub species: String,
    /// 이 펫의 돌보기 상태
    #[serde(default)]
    pub care: Care,
    /// 💤 쉬는 중 (창이 없고 돌보기가 멈춤 — 꺼져 있으면 저장하지 않음)
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub resting: bool,
}

/// 펫 목록을 읽되 잘못된 항목은 건너뜀 — 하나 때문에 설정 전체를 버리지 않음
/// (목록이 아니면 — 객체·숫자·null — 통째로 없는 것으로. 먼저 Value로 다 읽어 나머지 설정은 그대로 읽힘)
pub(crate) fn lenient_pets<'de, D: serde::Deserializer<'de>>(d: D) -> Result<Vec<OwnPet>, D::Error> {
    let serde_json::Value::Array(items) = serde_json::Value::deserialize(d)? else { return Ok(Vec::new()) };
    Ok(items.into_iter().filter_map(|v| serde_json::from_value::<OwnPet>(v).ok()).collect())
}

impl Config {
    /// 시작할 때 펫 목록을 맞춤: 없으면 옛 `pet` + `care[pet]`에서 한 마리를 만들고 (이사), 모르는 종류는 fallback으로,
    /// 비었거나 겹친 uid는 새로, 상한을 넘으면 자름. 바뀌었으면 true (저장해야 함)
    pub fn settle_pets(&mut self, known: impl Fn(&str) -> bool, fallback: &str, mut new_uid: impl FnMut() -> String) -> bool {
        let mut changed = false;
        if self.pets.is_empty() {
            let species = self.pet.clone().filter(|p| known(p)).unwrap_or_else(|| fallback.to_string());
            let care = self.care.get(&species).cloned().unwrap_or_default();
            // 다른 동물의 예전 기록은 따로 남겨 둠 (그 동물을 들이면 한 번 돌아옴 — add_pet)
            if self.old_care.is_empty() {
                self.old_care = self.care.iter().filter(|(k, _)| **k != species).map(|(k, v)| (k.clone(), v.clone())).collect();
            }
            self.pets.push(OwnPet { uid: new_uid(), species, care, resting: false });
            changed = true;
        }
        if self.pets.len() > MAX_OWN_PETS {
            self.pets.truncate(MAX_OWN_PETS);
            changed = true;
        }
        let mut seen: Vec<String> = Vec::new();
        for p in &mut self.pets {
            if !known(&p.species) {
                p.species = fallback.to_string();
                changed = true;
            }
            if p.uid.is_empty() || seen.contains(&p.uid) {
                p.uid = new_uid();
                changed = true;
            }
            seen.push(p.uid.clone());
        }
        // 💤 모두 쉬는 중이면 첫 펫을 깨움 (나온 펫이 늘 한 마리는 있음)
        if self.out_count() == 0 {
            if let Some(p) = self.pets.first_mut() {
                p.resting = false;
                changed = true;
            }
        }
        changed
    }

    /// 대표 펫 = 쉬지 않는 첫 펫 ("pet" 창·📱 내 다른 컴퓨터와 나누기·옛 필드) — settle_pets 뒤로는 늘 있음
    pub fn lead_pet(&self) -> Option<&OwnPet> {
        self.out_pets().next().or(self.pets.first())
    }

    /// 💤 화면에 나와 있는 (쉬지 않는) 펫들, 설정 차례대로
    pub fn out_pets(&self) -> impl Iterator<Item = &OwnPet> {
        self.pets.iter().filter(|p| !p.resting)
    }

    /// 💤 나와 있는 펫 수
    pub fn out_count(&self) -> usize {
        self.out_pets().count()
    }

    /// 💤 나와 있는 펫 중 몇째인지 (= 내 펫 슬롯 자리 — 슬롯은 나온 펫을 설정 차례대로 둠). 쉬는 중이거나 없으면 None
    pub fn out_position(&self, uid: &str) -> Option<usize> {
        self.out_pets().position(|p| p.uid == uid)
    }

    /// 메뉴 이름의 차례 (설정 자리, 전체 펫 수) — 쉬어도 바뀌지 않음 (tray::own_pet_name)
    pub fn pet_ordinal(&self, uid: &str) -> Option<(usize, usize)> {
        self.pets.iter().position(|p| p.uid == uid).map(|i| (i, self.pets.len()))
    }

    /// 📊 나와 있는 펫들의 종류 (그날 키운 종류로 셈 — 쉬는 펫은 빼고)
    pub fn out_species(&self) -> Vec<&str> {
        self.out_pets().map(|p| p.species.as_str()).collect()
    }

    /// 💤 이 펫을 쉬게 할 수 있음: 나와 있고, 다른 펫이 하나 이상 나와 있음
    pub fn can_rest(&self, uid: &str) -> bool {
        self.own_pet(uid).is_some_and(|p| !p.resting) && self.out_count() >= 2
    }

    /// 💤 쉬게 함 (기록은 그대로). 못 하면 false (can_rest)
    pub fn rest_pet(&mut self, uid: &str) -> bool {
        if !self.can_rest(uid) {
            return false;
        }
        match self.own_pet_mut(uid) {
            Some(p) => {
                p.resting = true;
                true
            }
            None => false,
        }
    }

    /// 💤 쉬던 펫을 다시 데려옴 (돌보기는 쉬게 한 그때 그대로). 쉬는 중이 아니거나 없으면 false
    pub fn wake_pet(&mut self, uid: &str) -> bool {
        match self.own_pet_mut(uid) {
            Some(p) if p.resting => {
                p.resting = false;
                true
            }
            _ => false,
        }
    }

    /// 🐾🐾🐾 나와 있는 펫마다 dt초만큼 배고파짐 (쉬는 펫은 멈춤 — 다시 나와도 쉰 시간은 셈하지 않음)
    pub fn tick_care(&mut self, dt: f64) {
        for p in self.pets.iter_mut().filter(|p| !p.resting) {
            p.care.tick(dt);
        }
    }

    /// uid의 펫
    pub fn own_pet(&self, uid: &str) -> Option<&OwnPet> {
        self.pets.iter().find(|p| p.uid == uid)
    }

    /// uid의 펫 (바꿀 수 있게)
    pub fn own_pet_mut(&mut self, uid: &str) -> Option<&mut OwnPet> {
        self.pets.iter_mut().find(|p| p.uid == uid)
    }

    /// 펫을 더 들임 (맨 뒤). 꽉 찼으면 None. 지금 그 종류를 키우는 펫이 없고 예전 기록(`old_care[종류]` — 펫 목록이 생기기 전에
    /// 키운 것)이 있으면 그 기록으로 한 번 데려옴 (예전에 키우던 아이 — 그 뒤로는 그 펫 것), 아니면 아기부터. 들인 자리를 돌려줌
    pub fn add_pet(&mut self, species: &str, uid: String) -> Option<usize> {
        if self.pets.len() >= MAX_OWN_PETS || uid.is_empty() || self.own_pet(&uid).is_some() {
            return None;
        }
        let raised = self.pets.iter().any(|p| p.species == species);
        let care = if raised { Care::default() } else { self.old_care.remove(species).unwrap_or_default() };
        self.pets.push(OwnPet { uid, species: species.to_string(), care, resting: false });
        Some(self.pets.len() - 1)
    }

    /// 펫을 보내 줌 (목록에서 뺌). 마지막 한 마리·나와 있는 마지막 펫 (💤 나머지는 쉬는 중)·없는 펫이면 false
    pub fn remove_pet(&mut self, uid: &str) -> bool {
        if self.pets.len() <= 1 {
            return false;
        }
        let Some(i) = self.pets.iter().position(|p| p.uid == uid) else { return false };
        if !self.pets[i].resting && self.out_count() <= 1 {
            return false;
        }
        self.pets.remove(i);
        true
    }

    /// 펫의 종류를 바꿈 — 키운 만큼은 그 펫에 그대로 남음. 바뀌었으면 true
    pub fn set_pet_species(&mut self, uid: &str, species: &str) -> bool {
        match self.own_pet_mut(uid) {
            Some(p) if p.species != species => {
                p.species = species.to_string();
                true
            }
            _ => false,
        }
    }

    /// 옛 필드를 대표 펫(💤 쉬지 않는 첫 펫)으로 맞춤 (저장 직전): `pet` = 대표 종류, `care` = 남은 예전 기록 + 대표 돌보기 —
    /// 옛 버전으로 되돌려도 첫 펫이 그대로 보임. `care`는 늘 새로 만듦 (보내 준 첫 펫·바꾸기 전 종류의 복사본이 남지 않음)
    pub fn sync_legacy(&mut self) {
        if let Some(lead) = self.lead_pet().cloned() {
            self.pet = Some(lead.species.clone());
            self.care = self.old_care.clone();
            self.care.insert(lead.species.clone(), lead.care.clone());
        }
    }

    /// 📱 내 다른 컴퓨터가 나눈 기록을 `care`에 받은 뒤: 지금 키우는 종류가 아닌 기록은 예전 기록으로 남김 (sync_legacy가 지우지 않게)
    pub fn keep_old_records(&mut self) {
        let kept: Vec<(String, Care)> = self.care.iter().filter(|(k, _)| !self.pets.iter().any(|p| &p.species == *k)).map(|(k, v)| (k.clone(), v.clone())).collect();
        self.old_care.extend(kept);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::care::Stage;

    /// 테스트: 아는 동물
    fn known(id: &str) -> bool {
        ["cat", "heumul", "rabbit"].contains(&id)
    }

    /// 테스트: 차례로 u1, u2, … 를 내는 uid 만들기
    fn uids() -> impl FnMut() -> String {
        let mut n = 0;
        move || {
            n += 1;
            format!("u{n}")
        }
    }

    #[test]
    fn an_old_config_moves_into_one_pet_with_its_growth() {
        let mut c: Config = serde_json::from_str(r#"{"pet":"rabbit","care":{"rabbit":{"stage":"adult","stamina":3.0,"fullness":0.5,"locked":true}}}"#).unwrap();
        assert!(c.pets.is_empty());
        assert!(c.settle_pets(known, "cat", uids()));
        assert_eq!(c.pets.len(), 1);
        let lead = c.lead_pet().unwrap();
        assert_eq!((lead.uid.as_str(), lead.species.as_str(), lead.care.stage, lead.care.locked), ("u1", "rabbit", Stage::Adult, true));
        // 다시 맞춰도 그대로
        assert!(!c.settle_pets(known, "cat", uids()));
    }

    #[test]
    fn a_first_run_or_unknown_pet_starts_with_the_fallback() {
        let mut c = Config::default();
        c.settle_pets(known, "cat", uids());
        assert_eq!(c.lead_pet().unwrap().species, "cat");
        let mut c: Config = serde_json::from_str(r#"{"pet":"dragon"}"#).unwrap();
        c.settle_pets(known, "cat", uids());
        assert_eq!(c.lead_pet().unwrap().species, "cat");
        // 목록 안의 모르는 종류도 (키운 만큼은 그대로)
        let mut c: Config = serde_json::from_str(r#"{"pets":[{"uid":"a","species":"dragon","care":{"stage":"child","stamina":0.0,"fullness":1.0,"locked":false}}]}"#).unwrap();
        assert!(c.settle_pets(known, "cat", uids()));
        assert_eq!((c.pets[0].species.as_str(), c.pets[0].care.stage), ("cat", Stage::Child));
    }

    #[test]
    fn three_of_the_same_kind_grow_separately() {
        let mut c = Config::default();
        c.settle_pets(known, "heumul", uids());
        assert_eq!(c.add_pet("heumul", "b".into()), Some(1));
        assert_eq!(c.add_pet("heumul", "c".into()), Some(2));
        let first = c.pets[0].uid.clone();
        c.own_pet_mut(&first).unwrap().care.stage = Stage::Adult;
        c.own_pet_mut("c").unwrap().care.stage = Stage::Child;
        let stages: Vec<Stage> = c.pets.iter().map(|p| p.care.stage).collect();
        assert_eq!(stages, vec![Stage::Adult, Stage::Baby, Stage::Child]);
        // 저장했다 읽어도 따로
        let back: Config = serde_json::from_str(&serde_json::to_string(&c).unwrap()).unwrap();
        assert_eq!(back.pets, c.pets);
    }

    #[test]
    fn at_most_three_pets() {
        let mut c = Config::default();
        c.settle_pets(known, "cat", uids());
        assert!(c.add_pet("cat", "b".into()).is_some());
        assert!(c.add_pet("rabbit", "c".into()).is_some());
        assert_eq!(c.add_pet("heumul", "d".into()), None);
        assert_eq!(c.pets.len(), MAX_OWN_PETS);
        // 같은 uid·빈 uid는 안 됨
        let mut c = Config::default();
        c.settle_pets(known, "cat", uids());
        assert_eq!(c.add_pet("cat", "u1".into()), None);
        assert_eq!(c.add_pet("cat", String::new()), None);
        // 파일에 넷이 적혀 있어도 셋까지만, 겹친 uid는 새로
        let mut c: Config = serde_json::from_str(r#"{"pets":[{"uid":"a","species":"cat"},{"uid":"a","species":"cat"},{"uid":"","species":"cat"},{"uid":"d","species":"cat"}]}"#).unwrap();
        assert!(c.settle_pets(known, "cat", uids()));
        let ids: Vec<&str> = c.pets.iter().map(|p| p.uid.as_str()).collect();
        assert_eq!(ids, vec!["a", "u1", "u2"]);
    }

    #[test]
    fn the_last_pet_cannot_be_let_go() {
        let mut c = Config::default();
        c.settle_pets(known, "cat", uids());
        assert!(!c.remove_pet("u1"));
        c.add_pet("rabbit", "b".into());
        assert!(!c.remove_pet("nobody"));
        assert!(c.remove_pet("u1"));
        assert_eq!(c.lead_pet().unwrap().uid, "b");
        assert!(!c.remove_pet("b"));
    }

    #[test]
    fn the_old_fields_keep_following_the_first_pet() {
        let mut c = Config::default();
        c.settle_pets(known, "cat", uids());
        c.pets[0].care.stage = Stage::Teenager;
        c.add_pet("rabbit", "b".into());
        c.sync_legacy();
        let text = serde_json::to_string(&c).unwrap();
        // 옛 버전이 읽는 모양: pet과 care[pet]
        let old: serde_json::Value = serde_json::from_str(&text).unwrap();
        assert_eq!(old["pet"], "cat");
        assert_eq!(old["care"]["cat"]["stage"], "teenager");
        // 첫 펫이 바뀌면 옛 필드도 따라감
        c.remove_pet("u1");
        c.sync_legacy();
        assert_eq!(c.pet.as_deref(), Some("rabbit"));
        assert_eq!(c.care.get("rabbit").map(|k| k.stage), Some(Stage::Baby));
    }

    #[test]
    fn changing_the_kind_keeps_the_growth_and_an_old_record_comes_back_when_added() {
        let mut c: Config = serde_json::from_str(r#"{"pet":"cat","care":{"cat":{"stage":"adult","stamina":0.0,"fullness":1.0,"locked":false},"rabbit":{"stage":"child","stamina":0.0,"fullness":1.0,"locked":false}}}"#).unwrap();
        c.settle_pets(known, "cat", uids());
        // 예전에 키우던 토끼를 들이면 그 기록으로
        c.add_pet("rabbit", "b".into());
        assert_eq!(c.own_pet("b").unwrap().care.stage, Stage::Child);
        // 이미 키우는 종류를 하나 더 들이면 아기부터
        c.add_pet("rabbit", "c".into());
        assert_eq!(c.own_pet("c").unwrap().care.stage, Stage::Baby);
        // 종류를 바꿔도 키운 만큼은 그 펫에 남음
        assert!(c.set_pet_species("u1", "heumul"));
        assert!(!c.set_pet_species("u1", "heumul"));
        assert_eq!(c.own_pet("u1").unwrap().care.stage, Stage::Adult);
    }

    #[test]
    fn one_broken_pet_in_the_file_is_skipped() {
        let c: Config = serde_json::from_str(r#"{"pets":[{"uid":"a","species":"cat"},{"uid":7},"x"],"housed":true}"#).unwrap();
        assert_eq!(c.pets.len(), 1);
        assert!(c.housed);
        let c: Config = serde_json::from_str(r#"{"pets":"lots","housed":true}"#).unwrap();
        assert!(c.pets.is_empty() && c.housed);
        // 펫 목록이 없는 기본 설정은 그 칸을 쓰지 않음
        assert!(!serde_json::to_string(&Config::default()).unwrap().contains("pets"));
    }

    #[test]
    fn retired_kinds_in_the_list_move_like_the_old_field() {
        let mut c: Config = serde_json::from_str(r#"{"pets":[{"uid":"a","species":"beaver"},{"uid":"b","species":"dog"}]}"#).unwrap();
        c.retire_pets();
        let kinds: Vec<&str> = c.pets.iter().map(|p| p.species.as_str()).collect();
        assert_eq!(kinds, vec!["cat", "cat"]);
        // 남겨 둔 예전 기록도 같이 옮김
        let mut c: Config = serde_json::from_str(r#"{"old_care":{"dog":{"stage":"child","stamina":0.0,"fullness":1.0,"locked":false}}}"#).unwrap();
        c.retire_pets();
        assert_eq!(c.old_care.get("cat").map(|k| k.stage), Some(Stage::Child));
    }

    // ---- 독립 리뷰에서 찾은 것 (2026-11-23) ----

    #[test]
    fn a_pets_object_instead_of_a_list_keeps_the_rest_of_the_config() {
        let k = "11".repeat(32);
        let text = format!(r#"{{"pets": {{"a": 1}}, "housed": true, "friends": [{{"id": "f", "name": "F", "public_key": "{k}", "side": "left"}}]}}"#);
        let c: Config = serde_json::from_str(&text).expect("the config still loads");
        assert!(c.pets.is_empty() && c.housed);
        assert_eq!(c.friends.len(), 1, "friends are not lost");
        for odd in [r#"{"pets": 7, "housed": true}"#, r#"{"pets": null, "housed": true}"#, r#"{"pets": "x", "housed": true}"#] {
            let c: Config = serde_json::from_str(odd).expect(odd);
            assert!(c.pets.is_empty() && c.housed, "{odd}");
        }
    }

    #[test]
    fn an_old_record_comes_back_only_once_and_never_a_copy_of_a_current_pet() {
        let mut c: Config = serde_json::from_str(r#"{"pet":"cat","care":{"cat":{"stage":"adult","stamina":0.0,"fullness":1.0,"locked":false},"rabbit":{"stage":"child","stamina":0.0,"fullness":1.0,"locked":false}}}"#).unwrap();
        c.settle_pets(known, "cat", uids());
        // 첫 펫(어른 고양이)을 토끼로 바꾼 뒤 고양이를 들여도 어른 고양이 복사본이 아니라 아기
        c.set_pet_species("u1", "heumul");
        c.sync_legacy();
        c.add_pet("cat", "b".into());
        assert_eq!(c.own_pet("b").unwrap().care.stage, Stage::Baby, "the first pet's own growth is not copied");
        // 예전 토끼 기록은 한 번만 돌아옴
        c.add_pet("rabbit", "c".into());
        assert_eq!(c.own_pet("c").unwrap().care.stage, Stage::Child);
        c.remove_pet("c");
        c.sync_legacy();
        c.add_pet("rabbit", "d".into());
        assert_eq!(c.own_pet("d").unwrap().care.stage, Stage::Baby, "consumed");
    }

    #[test]
    fn letting_the_first_pet_go_drops_its_growth_from_the_old_fields() {
        let mut c = Config::default();
        c.settle_pets(known, "cat", uids());
        c.pets[0].care.stage = Stage::Adult;
        c.add_pet("rabbit", "b".into());
        c.sync_legacy();
        assert!(c.remove_pet("u1"));
        c.sync_legacy();
        assert!(!c.care.contains_key("cat"), "키운 기록도 없어져요");
        // 다시 고양이를 들이면 아기부터
        c.add_pet("cat", "e".into());
        assert_eq!(c.own_pet("e").unwrap().care.stage, Stage::Baby);
    }

    #[test]
    fn changing_the_first_pets_kind_keeps_an_old_record_of_the_new_kind() {
        let mut c: Config = serde_json::from_str(r#"{"pet":"cat","care":{"rabbit":{"stage":"child","stamina":0.0,"fullness":1.0,"locked":false}}}"#).unwrap();
        c.settle_pets(known, "cat", uids());
        c.set_pet_species("u1", "rabbit");
        c.sync_legacy();
        c.set_pet_species("u1", "cat");
        c.sync_legacy();
        // 예전 토끼 기록은 덮이지 않고, 토끼를 들이면 돌아옴
        c.add_pet("rabbit", "b".into());
        assert_eq!(c.own_pet("b").unwrap().care.stage, Stage::Child);
        // 저장했다 읽어도 남은 옛 기록은 그대로
        let mut c2: Config = serde_json::from_str(r#"{"pet":"cat","care":{"rabbit":{"stage":"child","stamina":0.0,"fullness":1.0,"locked":false}}}"#).unwrap();
        c2.settle_pets(known, "cat", uids());
        c2.sync_legacy();
        let mut back: Config = serde_json::from_str(&serde_json::to_string(&c2).unwrap()).unwrap();
        back.settle_pets(known, "cat", uids());
        back.add_pet("rabbit", "z".into());
        assert_eq!(back.own_pet("z").unwrap().care.stage, Stage::Child);
    }

    // ---- 💤 쉬게 하기 (2026-11-27) ----

    /// 테스트: 고양이(u1)·토끼(b)·흐물이(c) 셋
    fn three() -> Config {
        let mut c = Config::default();
        c.settle_pets(known, "cat", uids());
        c.add_pet("rabbit", "b".into());
        c.add_pet("heumul", "c".into());
        c
    }

    #[test]
    fn resting_is_saved_only_when_on_and_old_configs_load() {
        let mut c = three();
        assert!(!serde_json::to_string(&c).unwrap().contains("resting"), "not written while false");
        assert!(c.rest_pet("b"));
        let text = serde_json::to_string(&c).unwrap();
        assert!(text.contains(r#""resting":true"#));
        let back: Config = serde_json::from_str(&text).unwrap();
        assert!(back.own_pet("b").unwrap().resting);
        let old: Config = serde_json::from_str(r#"{"pets":[{"uid":"a","species":"cat"}]}"#).unwrap();
        assert!(!old.pets[0].resting);
    }

    #[test]
    fn a_pet_rests_only_while_another_stays_out() {
        let mut c = Config::default();
        c.settle_pets(known, "cat", uids());
        assert!(!c.can_rest("u1") && !c.rest_pet("u1"), "the only pet");
        c.add_pet("rabbit", "b".into());
        assert!(c.can_rest("u1") && c.can_rest("b"));
        assert!(c.rest_pet("b"));
        assert!(!c.rest_pet("b"), "already resting");
        assert!(!c.can_rest("u1") && !c.rest_pet("u1"), "the last pet out");
        assert!(!c.rest_pet("nobody"));
        assert_eq!(c.out_count(), 1);
    }

    #[test]
    fn a_resting_pet_keeps_its_record_and_comes_back_as_it_was() {
        let mut c = three();
        {
            let b = c.own_pet_mut("b").unwrap();
            b.care = Care { stage: Stage::Teenager, stamina: 2.5, fullness: 0.4, locked: true };
        }
        let before = c.own_pet("b").unwrap().care.clone();
        assert!(c.rest_pet("b"));
        // 쉬는 동안엔 배고파지지 않음 (나온 펫만)
        let out_before = c.own_pet("u1").unwrap().care.fullness;
        c.tick_care(600.0);
        assert_eq!(c.own_pet("b").unwrap().care, before, "frozen while resting");
        assert!(c.own_pet("u1").unwrap().care.fullness < out_before, "out pets still get hungry");
        assert!(!c.wake_pet("u1"), "not resting");
        assert!(!c.wake_pet("nobody"));
        assert!(c.wake_pet("b"));
        assert_eq!(c.own_pet("b").unwrap().care, before);
        assert!(!c.own_pet("b").unwrap().resting);
    }

    #[test]
    fn resting_pets_still_fill_the_three_places() {
        let mut c = three();
        c.rest_pet("b");
        c.rest_pet("c");
        assert_eq!(c.add_pet("cat", "d".into()), None);
        assert_eq!(c.pets.len(), MAX_OWN_PETS);
    }

    #[test]
    fn the_lead_is_the_first_pet_out_and_the_old_fields_follow_it() {
        let mut c = three();
        assert!(c.rest_pet("u1"));
        assert_eq!(c.lead_pet().unwrap().uid, "b");
        c.sync_legacy();
        assert_eq!(c.pet.as_deref(), Some("rabbit"));
        let out: Vec<&str> = c.out_pets().map(|p| p.uid.as_str()).collect();
        assert_eq!(out, vec!["b", "c"]);
        // 다시 데려오면 설정 차례대로 맨 앞 (나온 펫 중 0번째)
        assert_eq!(c.out_position("u1"), None, "resting");
        c.wake_pet("u1");
        assert_eq!(c.out_position("u1"), Some(0));
        assert_eq!(c.out_position("c"), Some(2));
        assert_eq!(c.lead_pet().unwrap().uid, "u1");
    }

    #[test]
    fn ordinals_stay_the_same_while_a_pet_rests() {
        let mut c = three();
        c.rest_pet("b");
        assert_eq!(c.pet_ordinal("c"), Some((2, 3)), "still the third");
        assert_eq!(c.pet_ordinal("b"), Some((1, 3)));
        assert_eq!(c.pet_ordinal("nobody"), None);
    }

    #[test]
    fn the_last_pet_out_cannot_be_let_go_but_a_resting_one_can() {
        let mut c = three();
        c.rest_pet("b");
        c.rest_pet("c");
        assert!(!c.remove_pet("u1"), "would leave only resting pets");
        assert!(c.remove_pet("b"));
        assert_eq!(c.pets.len(), 2);
        assert_eq!(c.lead_pet().unwrap().uid, "u1");
    }

    #[test]
    fn if_every_pet_rests_the_first_wakes_at_start() {
        let mut c: Config = serde_json::from_str(r#"{"pets":[{"uid":"a","species":"cat","resting":true},{"uid":"b","species":"rabbit","resting":true}]}"#).unwrap();
        assert!(c.settle_pets(known, "cat", uids()));
        assert!(!c.pets[0].resting && c.pets[1].resting);
        assert!(!c.settle_pets(known, "cat", uids()), "settled");
    }

    #[test]
    fn only_pets_out_count_as_kept_kinds() {
        let mut c = three();
        c.rest_pet("b");
        assert_eq!(c.out_species(), vec!["cat", "heumul"]);
    }
}
