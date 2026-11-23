//! 🐾🐾🐾 내 펫 여러 마리 (최대 `MAX_OWN_PETS`) — 설정의 `pets` 목록과 이사·더하기·빼기·종류 바꾸기
//! 돌보기 상태(`Care`)는 종류가 아니라 펫 하나하나(`uid`)에 붙음. 첫 펫 = 대표 (창 라벨 `pet`)이고,
//! 옛 버전으로 되돌려도 첫 펫이 그대로 보이게 옛 필드 `pet`·`care[첫 펫 종류]`도 계속 씀 (`sync_legacy`)
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
            self.pets.push(OwnPet { uid: new_uid(), species, care });
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
        changed
    }

    /// 첫 펫 (대표) — settle_pets 뒤로는 늘 있음
    pub fn lead_pet(&self) -> Option<&OwnPet> {
        self.pets.first()
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
        self.pets.push(OwnPet { uid, species: species.to_string(), care });
        Some(self.pets.len() - 1)
    }

    /// 펫을 보내 줌 (목록에서 뺌). 마지막 한 마리이거나 없는 펫이면 false
    pub fn remove_pet(&mut self, uid: &str) -> bool {
        if self.pets.len() <= 1 {
            return false;
        }
        let Some(i) = self.pets.iter().position(|p| p.uid == uid) else { return false };
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

    /// 옛 필드를 첫 펫으로 맞춤 (저장 직전): `pet` = 첫 펫 종류, `care` = 남은 예전 기록 + 첫 펫 돌보기 —
    /// 옛 버전으로 되돌려도 첫 펫이 그대로 보임. `care`는 늘 새로 만듦 (보내 준 첫 펫·바꾸기 전 종류의 복사본이 남지 않음)
    pub fn sync_legacy(&mut self) {
        if let Some(lead) = self.pets.first() {
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
}
