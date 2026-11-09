use std::collections::HashMap;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::care::Care;
use crate::food::FeedMode;
use crate::gift::GiftBook;
use crate::presence::{DailyMarks, FocusLength};
use crate::style::PetStyle;
use crate::visit::{ExitEdges, Side};

/// 내 이름을 정하지 않았을 때 친구들에게 보이는 이름
pub const DEFAULT_OWNER_NAME: &str = "친구";

const FILE: &str = "config.json";
const TMP_FILE: &str = "config.json.tmp";
const BAK_FILE: &str = "config.json.bak";

#[derive(Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Config {
    /// 파일을 읽지 못해 기본값으로 돈다 — 이 실행은 파일을 덮어쓰지 않는다 (친구 목록을 지키려고)
    #[serde(skip)]
    pub keep_file: bool,
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
    #[serde(default, deserialize_with = "lenient")]
    pub feed_mode: FeedMode,
    /// 친구들에게 보이는 내 이름 (친구 화면의 내 동물 이름표). 없으면 컴퓨터 사용자 이름
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub owner_name: Option<String>,
    /// 🌨️ 날씨 효과를 메뉴에서 켰다 (기본은 꺼짐)
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub weather_on: bool,
    /// 🎁 주운 선물 (도감). 잘못된 줄은 건너뛴다
    #[serde(default, skip_serializing_if = "GiftBook::is_empty")]
    pub gifts: GiftBook,
    /// 💩 똥피하기 내 최고 기록 (피한 똥 수). 모르는 값이면 0
    #[serde(default, skip_serializing_if = "is_zero", deserialize_with = "lenient")]
    pub dodge_best: u32,
    /// 📏 펫 크기 (%). 없으면 100
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "lenient")]
    pet_size_pct: Option<u32>,
    /// 📏 물건 크기 (%). 없으면 100
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "lenient")]
    object_size_pct: Option<u32>,
    /// 🐾 커서 장난을 메뉴에서 껐다 (기본은 켜짐)
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub cursor_play_off: bool,
    /// 💩 똥 싸기를 메뉴에서 껐다 (기본은 켜짐)
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub poop_off: bool,
    /// ⏱️ 집중 친구 길이 (켜져 있는지는 남기지 않는다). 모르는 값이면 25/5
    #[serde(default, skip_serializing_if = "FocusLength::is_short", deserialize_with = "lenient")]
    pub focus_length: FocusLength,
    /// 오늘의 아침·점심 인사를 했는지 (앱을 다시 켜도 하루 한 번)
    #[serde(default, skip_serializing_if = "DailyMarks::is_empty")]
    pub daily: DailyMarks,
    /// 캐릭터 스타일 (동글동글 / 픽셀, 그림만 다르다). 모르는 값이면 동글동글
    #[serde(default, skip_serializing_if = "PetStyle::is_round", deserialize_with = "lenient")]
    pub style: PetStyle,
    /// 메뉴 "사다리 초대 받기"를 껐다 (끄면 초대 창 없이 자동으로 거절)
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub ladder_invites_off: bool,
    /// 가장 최근 사다리타기 한 판의 결과 표 (메뉴 "지난 사다리 결과"). 망가진 값이면 없는 것으로
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "lenient")]
    pub last_ladder: Option<LastLadder>,
}

/// 지난 사다리타기 결과 (보여 주기만 한다 — 이름은 그때 화면에 보인 이름)
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LastLadder {
    /// 방장 이름 (내가 방장이면 내 이름)
    pub host: String,
    /// 끝난 시각 (유닉스 초)
    pub at: i64,
    /// 공정성 지문 이모지 4개
    pub print: String,
    pub rows: Vec<LastLadderRow>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LastLadderRow {
    pub name: String,
    pub result: String,
    /// "good" | "bad" | "none"
    pub mood: String,
    #[serde(default)]
    pub me: bool,
}

/// 내가 정하는 이름(내 이름·친구 별명)의 최대 글자 수
pub const MAX_NAME_CHARS: usize = 20;

/// 글자에 보이지 않게 끼어드는 것: 제어 문자, 방향 바꾸기(U+202A–202E, U+2066–2069, U+061C), 폭 없는 글자(U+200B–200F,
/// U+2060–2064, U+180E), BOM(U+FEFF), 소프트 하이픈, 빈 칸처럼 보이는 채움 글자(한글 U+3164·U+115F·U+1160·U+FFA0, 점자
/// U+2800), 태그 문자(U+E0000–E007F), 줄·문단 구분자(U+2028·U+2029)
fn is_hidden_char(c: char) -> bool {
    c.is_control()
        || matches!(c, '\u{200B}'..='\u{200F}' | '\u{202A}'..='\u{202E}' | '\u{2060}'..='\u{2064}' | '\u{2066}'..='\u{2069}' | '\u{FEFF}' | '\u{061C}' | '\u{180E}' | '\u{00AD}' | '\u{3164}' | '\u{115F}' | '\u{1160}' | '\u{FFA0}' | '\u{2800}' | '\u{E0000}'..='\u{E007F}' | '\u{2028}' | '\u{2029}')
}

/// 다른 사람이 정했거나 내가 친 이름을 화면용으로: 보이지 않는 글자를 빼고, 앞뒤 공백을 지우고, max자까지.
/// 공백뿐이면 빈 글자 (부르는 쪽이 기본 이름으로)
pub fn clean_text(raw: &str, max: usize) -> String {
    let clean: String = raw.chars().filter(|&c| !is_hidden_char(c)).collect();
    let name: String = clean.trim().chars().take(max).collect();
    name.trim_end().to_string()
}

/// 내가 정하는 이름: 보이지 않는 글자를 빼고 앞뒤 공백을 지운 뒤 20자까지. 비면 None (= 기본 이름으로)
pub fn sanitize_name(raw: &str) -> Option<String> {
    let name = clean_text(raw, MAX_NAME_CHARS);
    (!name.is_empty()).then_some(name)
}

/// 값이 깨졌으면 기본값으로 — 설정 전체를 버리지 않는다
fn lenient<'de, T: serde::de::DeserializeOwned + Default, D: serde::Deserializer<'de>>(d: D) -> Result<T, D::Error> {
    Ok(serde_json::from_value(serde_json::Value::deserialize(d)?).unwrap_or_default())
}

/// 📏 크기 조절 슬라이더의 범위와 기본값 (%)
pub const SIZE_PCT_RANGE: (u32, u32) = (50, 150);
pub const SIZE_PCT_DEFAULT: u32 = 100;

fn clean_size_pct(pct: Option<u32>) -> u32 {
    pct.unwrap_or(SIZE_PCT_DEFAULT).clamp(SIZE_PCT_RANGE.0, SIZE_PCT_RANGE.1)
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
    /// 💩 똥피하기 그 친구의 최고 기록 (친구가 알려 준 것 — 받은 적 없으면 None)
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "lenient")]
    pub dodge_best: Option<u32>,
    /// 📱 내 다른 컴퓨터: 연결 코드가 맞아 둘이 나눠 가진 비밀. 연결될 때마다 서로 이 비밀을 증명해야 친구 목록을 나눈다.
    /// 컴퓨터는 둘까지라 이 값이 있는 친구는 하나뿐이다 (net이 정한다). 깨진 값은 연결 없음으로 읽는다
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "lenient")]
    pub device_secret: Option<DeviceSecret>,
    /// 이 친구 id가 자기 다른 컴퓨터라고 소개했다 (그 친구를 지우면 같이 지워진다)
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "lenient")]
    pub via: Option<String>,
    /// 짝맞추기 없이 (내 다른 컴퓨터나 친구의 소개로) 들어왔고, 아직 상대가 나를 아는지 못 봤다
    #[serde(default, skip_serializing_if = "std::ops::Not::not", deserialize_with = "lenient")]
    pub pending: bool,
    /// 확인 못 한 채로 들어온 때 (유닉스 초) — 일주일이 지나도 확인 못 하면 net이 지운다. 껐다 켜도 이어 센다
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "lenient")]
    pub pending_since: Option<u64>,
}

/// 📱 내 다른 컴퓨터와 나눈 비밀 (파일에는 hex 64자). Debug로 찍어도 값은 나오지 않는다
#[derive(Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeviceSecret(#[serde(with = "hex32")] pub [u8; 32]);

impl std::fmt::Debug for DeviceSecret {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("DeviceSecret(..)")
    }
}

fn is_zero(n: &u32) -> bool {
    *n == 0
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
    Up,
    Down,
}

/// 이름이 바뀐 동물 (예전 이름, 지금 이름) — 불러올 때 옮긴다 (retire_pets)
pub const RENAMED_PETS: [(&str, &str); 1] = [("beaver", "bear")];
/// 더 이상 없는 동물 (v0.5.5: 곰·꿀벌·강아지) — 고른 동물이면 RETIRED_TO로 바꾸고, 키운 기록은 RETIRED_TO에 넘긴다
pub const RETIRED_PETS: [&str; 3] = ["bear", "bee", "dog"];
pub const RETIRED_TO: &str = "cat";

/// 돌보기 기록 중 더 많이 큰 쪽 (단계, 같으면 체력)
fn further_grown(a: &Care, b: &Care) -> bool {
    let rank = |c: &Care| crate::care::Stage::ALL.iter().position(|s| *s == c.stage).unwrap_or(0);
    (rank(a), a.stamina) > (rank(b), b.stamina)
}

impl Config {
    /// 이름이 바뀌었거나 없어진 동물을 지금 동물로 옮긴다 (`load`가 부른다). 없어진 동물이 골라져 있으면 고양이로 바꾸고,
    /// 그 동물들의 기록 중 가장 많이 큰 것을 (고양이 기록보다 더 컸을 때만) 고양이에게 넘긴다 — 키운 만큼은 잃지 않는다.
    pub fn retire_pets(&mut self) {
        for (from, to) in RENAMED_PETS {
            self.rename_pet(from, to);
        }
        for gone in RETIRED_PETS {
            if self.pet.as_deref() == Some(gone) {
                self.pet = Some(RETIRED_TO.to_string());
            }
            if let Some(care) = self.care.remove(gone) {
                match self.care.get(RETIRED_TO) {
                    Some(have) if !further_grown(&care, have) => {}
                    _ => {
                        self.care.insert(RETIRED_TO.to_string(), care);
                    }
                }
            }
        }
    }

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
                self.friends.push(Friend { id: id.to_string(), name: name.to_string(), public_key, side, nickname: None, dodge_best: None, device_secret: None, via: None, pending: false, pending_since: None });
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

    pub fn pet_size_pct(&self) -> u32 {
        clean_size_pct(self.pet_size_pct)
    }

    pub fn object_size_pct(&self) -> u32 {
        clean_size_pct(self.object_size_pct)
    }

    /// 펫 크기 배율 (내 화면에만, 1.0 = 기본)
    pub fn pet_size(&self) -> f64 {
        f64::from(self.pet_size_pct()) / 100.0
    }

    /// 쓰레기 크기 배율 (내 화면에만, 1.0 = 기본)
    pub fn object_size(&self) -> f64 {
        f64::from(self.object_size_pct()) / 100.0
    }

    /// 크기 조절 창의 값을 넣는다 (범위 밖은 안으로, 기본값은 파일에 안 남긴다)
    pub fn set_sizes(&mut self, pet: u32, object: u32) {
        let keep = |pct: u32| Some(clean_size_pct(Some(pct))).filter(|&p| p != SIZE_PCT_DEFAULT);
        self.pet_size_pct = keep(pet);
        self.object_size_pct = keep(object);
    }

    /// 친구들에게 보일 내 이름: 정한 이름, 없으면 "친구" (컴퓨터 사용자 이름은 같은 망에 알리지 않는다)
    pub fn my_name(&self) -> String {
        self.owner_name.as_deref().and_then(sanitize_name).unwrap_or_else(|| DEFAULT_OWNER_NAME.to_string())
    }

    /// 💩 똥피하기 한 판 끝: 최고 기록이면 남기고 true
    pub fn record_dodge(&mut self, score: u32) -> bool {
        let score = score.min(crate::dodge::MAX_SCORE);
        if score <= self.dodge_best {
            return false;
        }
        self.dodge_best = score;
        true
    }

    /// 친구가 알려 준 똥피하기 최고 기록. 바뀌었으면 true (모르는 친구는 무시)
    pub fn set_friend_dodge_best(&mut self, id: &str, score: u32) -> bool {
        let score = Some(score.min(crate::dodge::MAX_SCORE));
        match self.friends.iter_mut().find(|f| f.id == id) {
            Some(f) if f.dodge_best != score => {
                f.dodge_best = score;
                true
            }
            _ => false,
        }
    }

    /// 📱 친구가 어디서 왔는지(소개한 친구, 확인 전)를 맞춘다 (net이 아는 대로). 바뀌었으면 true (모르는 친구는 무시)
    pub fn set_friend_links(&mut self, id: &str, via: Option<&str>, pending: bool) -> bool {
        let changed = match self.friends.iter_mut().find(|f| f.id == id) {
            Some(f) if (f.via.as_deref(), f.pending) != (via, pending) => {
                f.via = via.map(str::to_string);
                f.pending = pending;
                true
            }
            _ => false,
        };
        // 소개받은 컴퓨터는 그 사람이 있는 끝에
        if let Some(side) = via.and_then(|v| self.friend(v)).map(|f| f.side).filter(|_| changed) {
            self.set_friend_side(id, side);
        }
        changed
    }

    /// 📱 친구 목록을 linked::Member로 (connected = 연결된 때, 클수록 나중)
    fn members(&self, connected: impl Fn(&str) -> Option<f64>) -> Vec<crate::linked::Member<'_>> {
        self.friends.iter().map(|f| crate::linked::Member { id: &f.id, via: f.via.as_deref(), connected: connected(&f.id) }).collect()
    }

    /// 📱 이 친구가 속한 사람(처음 친구가 된 컴퓨터)의 id — 친구가 소개한 그 친구의 다른 컴퓨터면 그 친구
    pub fn person_id<'a>(&'a self, id: &'a str) -> &'a str {
        crate::linked::person_id(&self.members(|_| None), id)
    }

    /// 📱 그 사람의 보일 이름 (별명, 없으면 처음 친구가 정한 이름) — 다른 컴퓨터에서 온 동물도 같은 이름표
    pub fn person_name(&self, id: &str) -> Option<&str> {
        self.friend(self.person_id(id)).map(Friend::shown_name)
    }

    /// 📱 친구를 사람으로 묶는다 (메뉴 한 줄씩). connected = 연결된 때 (None = 연결 안 됨)
    pub fn people(&self, connected: impl Fn(&str) -> Option<f64>) -> Vec<crate::linked::Person> {
        crate::linked::group(&self.members(connected))
    }

    /// 📱 a와 b는 한 사람의 서로 다른 컴퓨터다
    pub fn same_person(&self, a: &str, b: &str) -> bool {
        a != b && self.friend(a).is_some() && self.friend(b).is_some() && self.person_id(a) == self.person_id(b)
    }

    /// 📱 친구 id의 동물에게 보낼 말을 실을 링크: 그 id가 연결돼 있으면 그대로, 아니면 그 사람의 연결된 다른 컴퓨터
    /// (대기 중인 컴퓨터의 동물은 노는 컴퓨터를 거쳐 왔다). 아무도 없으면 그 id
    pub fn reach<'a>(&'a self, id: &'a str, connected: impl Fn(&str) -> bool) -> &'a str {
        if connected(id) {
            return id;
        }
        let person = self.person_id(id);
        self.friends.iter().map(|f| f.id.as_str()).find(|f| *f != id && self.person_id(f) == person && connected(f)).unwrap_or(id)
    }

    /// 💩 똥피하기 순위에 넣을 친구: 사람마다 한 줄, 그 사람 컴퓨터들 중 가장 좋은 기록
    pub fn dodge_friends(&self) -> Vec<(String, u32)> {
        self.people(|_| None)
            .iter()
            .filter_map(|p| {
                let best = p.members.iter().filter_map(|id| self.friend(id)?.dodge_best).max()?;
                Some((self.person_name(&p.id)?.to_string(), best))
            })
            .collect()
    }

    /// 📱 내 다른 컴퓨터와 나눈 비밀을 맞춘다 (net이 아는 대로, None = 연결 안 됨). 바뀌었으면 true (모르는 친구는 무시)
    pub fn set_friend_device_secret(&mut self, id: &str, secret: Option<[u8; 32]>) -> bool {
        let secret = secret.map(DeviceSecret);
        match self.friends.iter_mut().find(|f| f.id == id) {
            Some(f) if f.device_secret != secret => {
                f.device_secret = secret;
                true
            }
            _ => false,
        }
    }

    /// 📱 연결된 내 다른 컴퓨터 (id, 비밀) — 여럿 적혀 있으면 (손으로 고친 파일) 처음 것만
    pub fn linked_device(&self) -> Option<(&str, [u8; 32])> {
        self.friends.iter().find_map(|f| Some((f.id.as_str(), f.device_secret?.0)))
    }

    /// 확인 못 한 친구가 들어온 때를 맞춘다 (net이 아는 대로). 바뀌었으면 true (모르는 친구는 무시)
    pub fn set_friend_pending_since(&mut self, id: &str, since: Option<u64>) -> bool {
        match self.friends.iter_mut().find(|f| f.id == id) {
            Some(f) if f.pending_since != since => {
                f.pending_since = since;
                true
            }
            _ => false,
        }
    }

    pub fn remove_friend(&mut self, id: &str) -> bool {
        let before = self.friends.len();
        self.friends.retain(|f| f.id != id);
        self.friends.len() != before
    }

    /// 친구가 붙은 끝 — 📱 한 사람의 컴퓨터 둘은 같은 끝에
    pub fn set_friend_side(&mut self, id: &str, side: Side) -> bool {
        let person = self.person_id(id).to_string();
        let members: Vec<String> = self.people(|_| None).into_iter().find(|p| p.id == person).map_or_else(|| vec![id.to_string()], |p| p.members);
        let mut changed = false;
        for f in self.friends.iter_mut().filter(|f| members.contains(&f.id)) {
            if f.side != side {
                f.side = side;
                changed = true;
            }
        }
        changed
    }

    /// 지금 연결된(connected) 친구가 있는 화면 끝
    pub fn exit_edges(&self, connected: impl Fn(&str) -> bool) -> ExitEdges {
        let on = |side| self.friends.iter().any(|f| f.side == side && connected(&f.id));
        ExitEdges { left: on(Side::Left), right: on(Side::Right), up: on(Side::Up), down: on(Side::Down) }
    }

    /// 놀러 온 손님(주인 owner)을 더 보낼 수 있는 친구: 연결된 내 친구 중 그 주인을 뺀 친구들 (손님 우클릭 "<친구>에게 보내기")
    pub fn send_on_targets(&self, owner: &str, connected: impl Fn(&str) -> bool) -> Vec<&Friend> {
        self.friends.iter().filter(|f| f.id != owner && connected(&f.id)).collect()
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
    let parsed = match std::fs::read_to_string(&path) {
        Ok(text) => serde_json::from_str::<Config>(&text).map_err(|e| e.to_string()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Config::default(),
        Err(e) if e.kind() == std::io::ErrorKind::InvalidData => Err(e.to_string()),
        // 권한·잠금 등: 파일은 멀쩡할 수 있다. 기본값으로 돌되 덮어쓰지 않는다
        Err(e) => {
            eprintln!("config {} cannot be read ({e}); running on defaults without saving", path.display());
            return Config { keep_file: true, ..Config::default() };
        }
    };
    match parsed {
        Ok(mut c) => {
            c.retire_pets();
            c
        }
        Err(e) => {
            let bak = dir.join(BAK_FILE);
            match std::fs::copy(&path, &bak) {
                Ok(_) => {
                    eprintln!("config {} is corrupt ({e}); kept a copy at {}, starting fresh", path.display(), bak.display());
                    Config::default()
                }
                // 백업이 없으면 덮어쓰지 않는다
                Err(be) => {
                    eprintln!("config {} is corrupt ({e}); backup failed ({be}), running on defaults without saving", path.display());
                    Config { keep_file: true, ..Config::default() }
                }
            }
        }
    }
}

/// 임시 파일에 다 쓴 뒤 이름을 바꿔 덮어쓴다 — 쓰다 꺼져도 이전 설정이 남는다
pub fn save(dir: &Path, c: &Config) -> std::io::Result<()> {
    if c.keep_file {
        return Err(std::io::Error::other("the config file could not be read at start; not overwriting it"));
    }
    std::fs::create_dir_all(dir)?;
    let tmp = dir.join(TMP_FILE);
    {
        use std::io::Write;
        let mut f = std::fs::File::create(&tmp)?;
        f.write_all(serde_json::to_string_pretty(c)?.as_bytes())?;
        // 이름을 바꾸기 전에 디스크까지 — 꺼져도 빈 파일이 남지 않는다
        f.sync_all()?;
    }
    std::fs::rename(&tmp, dir.join(FILE))
}

const CRASH_FILE: &str = "crash.log";
/// 충돌 기록이 이보다 커지면 처음부터 다시 쓴다
const CRASH_LOG_MAX: u64 = 64 * 1024;

/// 충돌 내용을 config 폴더의 `crash.log`에 덧붙인다 (배포판에는 콘솔이 없다). 실패는 무시한다 — 이미 죽어 가는 중이다
pub fn append_crash_log(dir: &Path, text: &str) {
    use std::io::Write;
    let path = dir.join(CRASH_FILE);
    let keep = std::fs::metadata(&path).is_ok_and(|m| m.len() < CRASH_LOG_MAX);
    let _ = std::fs::create_dir_all(dir);
    let file = std::fs::OpenOptions::new().create(true).write(true).append(keep).truncate(!keep).open(&path);
    if let Ok(mut f) = file {
        let _ = writeln!(f, "{text}");
    }
}

/// 장기 X25519 비밀키 파일 (config.json과 따로 — 설정 파일은 비밀이 아닌 것만 갖는다)
const KEY_FILE: &str = "identity.key";
const KEY_TMP_FILE: &str = "identity.key.tmp";
const KEY_BAK_FILE: &str = "identity.key.bak";

/// 이 설치의 비밀키(친구들이 고정한 신원)를 config 폴더의 `identity.key`(hex 64자)에서 읽는다.
/// 없거나 내용이 깨졌으면 new_key()로 만들어 저장한다 (깨진 파일은 `identity.key.bak`으로 남기고 — 못 남기면 Err — 새로 쓴다. 그러면 친구들과 다시 짝맞추기가 필요하다).
/// 그 밖의 읽기 에러(권한 등)는 그대로 Err — 멀쩡할 수 있는 키를 덮어쓰지 않는다 (그 실행에서는 친구 기능을 끈다).
///
/// 권한: Unix에서는 폴더를 0o700, 파일을 0o600으로 만든다. Windows에서는 config 폴더(%APPDATA% 아래)가
/// 기본으로 그 사용자 전용 ACL을 가지므로 보통 방식으로 만든다.
pub fn load_or_create_identity_key(dir: &Path, new_key: impl FnOnce() -> [u8; 32]) -> std::io::Result<[u8; 32]> {
    let path = dir.join(KEY_FILE);
    match std::fs::read(&path) {
        Ok(bytes) => {
            if let Some(k) = std::str::from_utf8(&bytes).ok().and_then(|t| hex32::decode(t.trim())) {
                lock_down(&path);
                return Ok(k);
            }
            let bak = dir.join(KEY_BAK_FILE);
            std::fs::copy(&path, &bak)?;
            lock_down(&bak);
            eprintln!("identity key {} is corrupt; kept a copy at {}, making a new one", path.display(), bak.display());
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

/// 복사나 백업으로 남도 읽을 수 있게 된 비밀키는 읽을 때 다시 잠근다. 못 잠가도 키는 쓴다 (친구를 잃는 것보다 낫다)
#[cfg(unix)]
fn lock_down(path: &Path) {
    use std::os::unix::fs::PermissionsExt;
    let Ok(meta) = std::fs::metadata(path) else { return };
    if meta.permissions().mode() & 0o077 != 0 {
        if let Err(e) = std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600)) {
            eprintln!("identity key {} is readable by others and could not be locked: {e}", path.display());
        }
    }
}

/// Windows: %APPDATA% 아래 파일은 그 사용자 전용 ACL을 물려받는다
#[cfg(not(unix))]
fn lock_down(_path: &Path) {}

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

    #[test]
    fn sizes_default_to_100_percent_and_stay_in_range() {
        let mut c: Config = serde_json::from_str("{}").unwrap();
        assert_eq!((c.pet_size_pct(), c.object_size_pct()), (SIZE_PCT_DEFAULT, SIZE_PCT_DEFAULT));
        assert_eq!((c.pet_size(), c.object_size()), (1.0, 1.0));
        c.set_sizes(10, 900);
        assert_eq!((c.pet_size_pct(), c.object_size_pct()), SIZE_PCT_RANGE);
        c.set_sizes(125, 75);
        assert_eq!((c.pet_size(), c.object_size()), (1.25, 0.75));
    }

    #[test]
    fn sizes_survive_a_restart_and_default_is_not_written() {
        let mut c: Config = serde_json::from_str("{}").unwrap();
        c.set_sizes(125, 75);
        let saved = serde_json::to_string(&c).unwrap();
        let back: Config = serde_json::from_str(&saved).unwrap();
        assert_eq!((back.pet_size_pct(), back.object_size_pct()), (125, 75));
        c.set_sizes(SIZE_PCT_DEFAULT, SIZE_PCT_DEFAULT);
        let saved = serde_json::to_string(&c).unwrap();
        assert!(!saved.contains("size_pct"), "{saved}");
    }

    #[test]
    fn a_broken_size_in_the_file_does_not_lose_the_config() {
        let c: Config = serde_json::from_str(r#"{"pet_size_pct":"huge","object_size_pct":9000,"housed":true}"#).unwrap();
        assert!(c.housed);
        assert_eq!(c.pet_size_pct(), SIZE_PCT_DEFAULT);
        assert_eq!(c.object_size_pct(), SIZE_PCT_RANGE.1);
    }

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
        let mut c = Config { pet: Some("rabbit".into()), ..Config::default() };
        c.care.insert("rabbit".into(), Care { stage: Stage::Child, stamina: 12.0, fullness: 0.3, locked: true });
        save(&d, &c).unwrap();
        assert_eq!(load(&d), c);
    }

    // ---- 없어진 동물 (곰·꿀벌·강아지) → 고양이, 키운 만큼은 그대로 ----

    fn cfg_with(pet: &str, care: &[(&str, crate::care::Care)]) -> Config {
        let mut c = Config { pet: Some(pet.into()), owner_name: Some("민지".into()), ..Config::default() };
        c.care.extend(care.iter().map(|(id, cr)| (id.to_string(), cr.clone())));
        c
    }

    #[test]
    fn retired_pets_move_to_the_cat_with_their_growth() {
        use crate::care::{Care, Stage};
        let grown = Care { stage: Stage::Teenager, stamina: 40.0, fullness: 0.4, locked: true };
        for gone in ["bear", "bee", "dog"] {
            let mut c = cfg_with(gone, &[(gone, grown.clone())]);
            c.retire_pets();
            assert_eq!(c.pet.as_deref(), Some(RETIRED_TO), "{gone}");
            assert_eq!(c.care.get("cat"), Some(&grown), "{gone}: growth is carried over");
            assert!(!c.care.contains_key(gone), "{gone}");
            assert_eq!(c.owner_name.as_deref(), Some("민지"), "the rest of the config is kept");
        }
        assert_eq!(RETIRED_TO, "cat");
    }

    #[test]
    fn retired_pets_keep_the_furthest_grown_care() {
        use crate::care::{Care, Stage};
        let baby = Care { stage: Stage::Baby, stamina: 20.0, ..Care::default() };
        let child = Care { stage: Stage::Child, stamina: 5.0, ..Care::default() };
        let child_more = Care { stage: Stage::Child, stamina: 30.0, ..Care::default() };
        let adult = Care { stage: Stage::Adult, ..Care::default() };
        // 고양이가 더 어리면 없어진 동물의 기록을 받는다
        let mut c = cfg_with("dog", &[("dog", child.clone()), ("cat", baby.clone())]);
        c.retire_pets();
        assert_eq!((c.pet.as_deref(), c.care.get("cat")), (Some("cat"), Some(&child)));
        // 고양이가 더 컸으면 고양이 기록을 남긴다
        let mut c = cfg_with("bear", &[("bear", child.clone()), ("cat", adult.clone())]);
        c.retire_pets();
        assert_eq!(c.care.get("cat"), Some(&adult));
        // 같은 단계면 체력이 더 많은 쪽 · 여러 마리면 가장 많이 큰 하나
        let mut c = cfg_with("rabbit", &[("bee", child_more.clone()), ("dog", child.clone()), ("cat", child.clone())]);
        c.retire_pets();
        assert_eq!(c.pet.as_deref(), Some("rabbit"), "a remaining pet stays selected");
        assert_eq!(c.care.get("cat"), Some(&child_more));
        assert!(!c.care.contains_key("bee") && !c.care.contains_key("dog"));
        assert_eq!(c.care.len(), 1);
    }

    #[test]
    fn retiring_leaves_remaining_pets_alone() {
        use crate::care::{Care, Stage};
        let teen = Care { stage: Stage::Teenager, ..Care::default() };
        let mut c = cfg_with("rabbit", &[("rabbit", teen.clone())]);
        let before = serde_json::to_string(&c).unwrap();
        c.retire_pets();
        assert_eq!(serde_json::to_string(&c).unwrap(), before);
        let mut none = Config::default();
        none.retire_pets();
        assert_eq!(none, Config::default(), "a new install keeps no pet chosen");
    }

    #[test]
    fn a_very_old_beaver_becomes_a_cat_too() {
        use crate::care::{Care, Stage};
        let kinder = Care { stage: Stage::Kinder, ..Care::default() };
        let mut c = cfg_with("beaver", &[("beaver", kinder.clone())]);
        c.retire_pets();
        assert_eq!((c.pet.as_deref(), c.care.get("cat")), (Some("cat"), Some(&kinder)));
        assert_eq!(c.care.len(), 1);
    }

    #[test]
    fn loading_an_old_config_retires_bear_bee_and_dog() {
        let d = temp_dir("retired");
        std::fs::create_dir_all(&d).unwrap();
        std::fs::write(
            d.join("config.json"),
            r#"{"pet": "bee", "owner_name": "민지", "care": {"bee": {"stage": "child", "stamina": 12.0, "fullness": 0.5, "locked": false}}}"#,
        )
        .unwrap();
        let c = load(&d);
        assert_eq!(c.pet.as_deref(), Some("cat"));
        assert_eq!(c.care.get("cat").map(|c| (c.stage, c.stamina)), Some((crate::care::Stage::Child, 12.0)));
        assert_eq!(c.owner_name.as_deref(), Some("민지"));
    }

    #[test]
    fn rename_pet_moves_selection_and_care() {
        use crate::care::{Care, Stage};
        let child = Care { stage: Stage::Child, ..Care::default() };
        let mut c = Config { pet: Some("beaver".into()), ..Config::default() };
        c.care.insert("beaver".into(), child.clone());
        c.rename_pet("beaver", "rabbit");
        assert_eq!(c.pet.as_deref(), Some("rabbit"));
        assert_eq!(c.care.get("rabbit"), Some(&child));
        assert!(!c.care.contains_key("beaver"));
    }

    #[test]
    fn rename_pet_keeps_existing_target_care_and_other_selection() {
        use crate::care::{Care, Stage};
        let adult = Care { stage: Stage::Adult, ..Care::default() };
        let mut c = Config { pet: Some("cat".into()), ..Config::default() };
        c.care.insert("beaver".into(), Care::default());
        c.care.insert("rabbit".into(), adult.clone());
        c.rename_pet("beaver", "rabbit");
        assert_eq!(c.pet.as_deref(), Some("cat"));
        assert_eq!(c.care.get("rabbit"), Some(&adult));
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

    #[cfg(unix)]
    #[test]
    fn an_identity_key_left_readable_by_others_is_locked_down_on_load() {
        use std::os::unix::fs::PermissionsExt;
        let d = temp_dir("identity_key_perms");
        let k = load_or_create_identity_key(&d, || key(0x11)).unwrap();
        std::fs::set_permissions(d.join("identity.key"), std::fs::Permissions::from_mode(0o644)).unwrap();
        assert_eq!(load_or_create_identity_key(&d, || panic!("no new key")).unwrap(), k);
        assert_eq!(std::fs::metadata(d.join("identity.key")).unwrap().permissions().mode() & 0o777, 0o600);
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
    fn invisible_look_alikes_are_hidden_too() {
        // 채움 글자(한글·반각·점자)와 태그 문자, 줄·문단 구분자: 보이지 않는 이름을 만들 수 있다
        for c in ['\u{3164}', '\u{115F}', '\u{1160}', '\u{FFA0}', '\u{2800}', '\u{E0000}', '\u{E0041}', '\u{E007F}', '\u{2028}', '\u{2029}'] {
            assert_eq!(sanitize_name(&format!("{c}{c}")), None, "U+{:X}", c as u32);
            assert_eq!(sanitize_name(&format!("da{c}nni")), Some("danni".into()), "U+{:X}", c as u32);
        }
        // 보통 한글 자모와 이모지는 그대로
        assert_eq!(sanitize_name("ㅎㅎ 한글 🐱"), Some("ㅎㅎ 한글 🐱".into()));
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

    /// 친구 X(왼쪽), X가 소개한 X의 다른 컴퓨터 x2, 다른 친구 y
    fn with_two_computers() -> Config {
        let mut c = Config::default();
        c.upsert_friend("x", "Xavier", key(1));
        c.upsert_friend("y", "Yuna", key(2));
        c.set_friend_side("x", Side::Left);
        c.upsert_friend("x2", "x on the laptop", key(3));
        c.set_friend_links("x2", Some("x"), false);
        c
    }

    #[test]
    fn a_friends_other_computer_is_the_same_person_with_the_same_name_and_side() {
        let mut c = with_two_computers();
        assert_eq!(c.friend("x2").unwrap().side, Side::Left, "it sits where its person sits");
        assert_eq!(c.person_id("x2"), "x");
        assert_eq!(c.person_id("y"), "y");
        assert_eq!(c.person_name("x2"), Some("Xavier"));
        assert_eq!(c.person_name("nobody"), None);
        c.set_friend_nickname("x", "자비");
        assert_eq!(c.person_name("x2"), Some("자비"), "the nickname is for the person");
        // 화면 끝은 둘 다
        assert!(c.set_friend_side("x", Side::Up));
        assert_eq!((c.friend("x").unwrap().side, c.friend("x2").unwrap().side), (Side::Up, Side::Up));
        assert!(c.set_friend_side("x2", Side::Down));
        assert_eq!((c.friend("x").unwrap().side, c.friend("x2").unwrap().side), (Side::Down, Side::Down));
        assert_eq!(c.friend("y").unwrap().side, Side::Left, "others stay");
        let people = c.people(|id| (id == "x2").then_some(1.0));
        assert_eq!(people.len(), 2);
        assert_eq!((people[0].id.as_str(), people[0].target.as_str(), people[0].connected), ("x", "x2", true));
    }

    #[test]
    fn messages_for_a_persons_pet_go_to_whichever_of_their_computers_is_connected() {
        let c = with_two_computers();
        assert_eq!(c.reach("x2", |id| id == "x2"), "x2");
        assert_eq!(c.reach("x2", |id| id == "x"), "x", "x2 plays elsewhere: its pet is reached through x");
        assert_eq!(c.reach("x", |id| id == "x2"), "x2");
        assert_eq!(c.reach("x", |_| false), "x");
        assert_eq!(c.reach("y", |id| id == "x"), "y", "never someone else");
        // 한 사람이 보낸 다른 컴퓨터의 동물만 (소개한 쪽이든 소개받은 쪽이든)
        assert!(c.same_person("x", "x2") && c.same_person("x2", "x"));
        assert!(!c.same_person("x", "y") && !c.same_person("x", "x") && !c.same_person("x", "nobody"));
    }

    #[test]
    fn the_dodge_ranking_has_one_line_per_person_with_their_best() {
        let mut c = with_two_computers();
        c.set_friend_dodge_best("x", 10);
        c.set_friend_dodge_best("x2", 30);
        c.set_friend_dodge_best("y", 20);
        assert_eq!(c.dodge_friends(), vec![("Xavier".to_string(), 30), ("Yuna".to_string(), 20)]);
    }

    #[test]
    fn exit_edges_need_a_connected_friend_on_that_side() {
        let mut c = Config::default();
        c.upsert_friend("r", "R", key(1));
        c.upsert_friend("l", "L", key(2));
        let none = |_: &str| false;
        assert_eq!(c.exit_edges(none), ExitEdges::default());
        assert_eq!(c.exit_edges(|id| id == "r"), ExitEdges { left: false, right: true, ..Default::default() });
        assert_eq!(c.exit_edges(|_| true), ExitEdges { left: true, right: true, ..Default::default() });
        c.set_friend_side("l", Side::Right);
        assert_eq!(c.exit_edges(|_| true), ExitEdges { left: false, right: true, ..Default::default() });
        let ids: Vec<&str> = c.friends_at(Side::Right, |id| id == "l").iter().map(|f| f.id.as_str()).collect();
        assert_eq!(ids, ["l"]);
        assert!(c.friends_at(Side::Left, |_| true).is_empty());
    }

    #[test]
    fn friends_can_sit_above_or_below_and_old_configs_still_load() {
        let mut c = Config::default();
        c.upsert_friend("u", "U", key(1));
        c.upsert_friend("d", "D", key(2));
        assert!(c.set_friend_side("u", Side::Up) && c.set_friend_side("d", Side::Down));
        assert_eq!(c.exit_edges(|_| true), ExitEdges { up: true, down: true, ..Default::default() });
        assert_eq!(c.exit_edges(|id| id == "d"), ExitEdges { down: true, ..Default::default() });
        assert_eq!(c.friends_at(Side::Up, |_| true).len(), 1);
        let text = serde_json::to_string(&c).unwrap();
        assert!(text.contains(r#""side":"up""#) && text.contains(r#""side":"down""#), "{text}");
        let back: Config = serde_json::from_str(&text).unwrap();
        assert_eq!(back.friend("u").unwrap().side, Side::Up);
        assert_eq!(back.friend("d").unwrap().side, Side::Down);
        // 새 친구는 예전처럼 왼쪽/오른쪽에 붙는다
        c.upsert_friend("n", "N", key(3));
        assert!(matches!(c.friend("n").unwrap().side, Side::Left | Side::Right));
    }

    #[test]
    fn corrupt_file_gives_default() {
        let d = temp_dir("corrupt");
        std::fs::create_dir_all(&d).unwrap();
        std::fs::write(d.join("config.json"), "not json").unwrap();
        assert_eq!(load(&d), Config::default());
        assert_eq!(std::fs::read_to_string(d.join("config.json.bak")).unwrap(), "not json");
    }

    /// 읽기 권한 없는 파일로 읽기 실패를 만든다 (root로 돌면 읽히므로 건너뛴다)
    #[cfg(unix)]
    fn unreadable(d: &Path, name: &str, text: &str) -> bool {
        use std::os::unix::fs::PermissionsExt;
        std::fs::create_dir_all(d).unwrap();
        let path = d.join(name);
        std::fs::write(&path, text).unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o000)).unwrap();
        std::fs::read(&path).is_err()
    }

    #[cfg(unix)]
    #[test]
    fn a_config_that_cannot_be_read_is_never_overwritten() {
        use std::os::unix::fs::PermissionsExt;
        let d = temp_dir("unreadable");
        if !unreadable(&d, "config.json", "{\"pet\":\"rabbit\"}") {
            return;
        }
        let c = load(&d);
        assert_eq!(c.pet, None);
        assert!(c.keep_file, "this run must not write over the friends it could not read");
        assert!(save(&d, &c).is_err());
        std::fs::set_permissions(d.join("config.json"), std::fs::Permissions::from_mode(0o600)).unwrap();
        assert_eq!(load(&d).pet.as_deref(), Some("rabbit"));
    }

    #[test]
    fn crashes_are_appended_and_the_log_stays_small() {
        let d = temp_dir("crashlog");
        append_crash_log(&d, "one");
        append_crash_log(&d, "two");
        assert_eq!(std::fs::read_to_string(d.join("crash.log")).unwrap(), "one\ntwo\n");
        std::fs::write(d.join("crash.log"), "x".repeat(CRASH_LOG_MAX as usize)).unwrap();
        append_crash_log(&d, "three");
        assert_eq!(std::fs::read_to_string(d.join("crash.log")).unwrap(), "three\n");
    }

    #[test]
    fn a_config_that_is_not_text_counts_as_corrupt() {
        let d = temp_dir("notutf8");
        std::fs::create_dir_all(&d).unwrap();
        std::fs::write(d.join("config.json"), [0xff, 0xfe, 0x00]).unwrap();
        let c = load(&d);
        assert!(!c.keep_file);
        assert_eq!(std::fs::read(d.join("config.json.bak")).unwrap(), [0xff, 0xfe, 0x00]);
    }

    #[test]
    fn a_corrupt_identity_key_is_kept_as_a_backup_before_a_new_one_is_made() {
        let d = temp_dir("corruptkey");
        std::fs::create_dir_all(&d).unwrap();
        std::fs::write(d.join("identity.key"), "not a key").unwrap();
        let k = load_or_create_identity_key(&d, || [7u8; 32]).unwrap();
        assert_eq!(k, [7u8; 32]);
        assert_eq!(std::fs::read_to_string(d.join("identity.key.bak")).unwrap(), "not a key");
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
        let c = Config { pet: Some("rabbit".into()), ..Config::default() };
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
    fn weather_is_off_unless_turned_on_and_old_files_still_load() {
        let old: Config = serde_json::from_str(r#"{"pet":"cat","weather_off":true}"#).unwrap();
        assert!(!old.weather_on);
        assert!(!serde_json::to_string(&old).unwrap().contains("weather_on"), "not written while it is the default");
        let on = Config { weather_on: true, ..Default::default() };
        let back: Config = serde_json::from_str(&serde_json::to_string(&on).unwrap()).unwrap();
        assert!(back.weather_on);
    }

    #[test]
    fn feed_mode_defaults_to_hand_and_roundtrips() {
        use crate::food::FeedMode;
        let old: Config = serde_json::from_str(r#"{"pet":"cat"}"#).unwrap();
        assert_eq!(old.feed_mode, FeedMode::Hand);
        let bad: Config = serde_json::from_str(r#"{"pet":"cat","feed_mode":"teleport"}"#).unwrap();
        assert_eq!((bad.pet.as_deref(), bad.feed_mode), (Some("cat"), FeedMode::Hand));
        let dir = temp_dir("feedmode");
        let c = Config { pet: Some("rabbit".into()), feed_mode: FeedMode::Auto, ..Config::default() };
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

    #[test]
    fn a_guest_can_be_sent_on_to_my_connected_friends_except_its_owner() {
        let mut c = Config::default();
        c.upsert_friend("owner", "O", key(1));
        c.upsert_friend("b", "B", key(2));
        c.upsert_friend("off", "Off", key(3));
        c.upsert_friend("d", "D", key(4));
        let ids: Vec<&str> = c.send_on_targets("owner", |id| id != "off").iter().map(|f| f.id.as_str()).collect();
        assert_eq!(ids, ["b", "d"]);
        assert!(c.send_on_targets("owner", |_| false).is_empty());
    }

    // ---- 내 이름·친구 별명 ----

    #[test]
    fn hidden_format_chars_are_stripped_and_blank_names_rejected() {
        // 방향 바꾸기(RLO 등)·폭 없는 글자·BOM은 뺀다
        assert_eq!(sanitize_name("da\u{202E}nn\u{200B}i\u{FEFF}"), Some("danni".into()));
        assert_eq!(sanitize_name("\u{2066}x\u{2069}\u{200F}"), Some("x".into()));
        // 보이지 않는 글자·공백뿐이면 없는 이름
        assert_eq!(sanitize_name("\u{200B}\u{3000} \u{200D}"), None);
        assert_eq!(clean_text(" \u{202A}Ann\u{202C} ", 64), "Ann");
        assert_eq!(clean_text(&"b".repeat(100), 64).chars().count(), 64);
        assert_eq!(clean_text("\u{200B} \t", 64), "");
    }

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
        assert_eq!(c.my_name(), DEFAULT_OWNER_NAME);
        assert!(c.set_owner_name("  danni  "));
        assert_eq!(c.owner_name.as_deref(), Some("danni"));
        assert_eq!(c.my_name(), "danni");
        assert!(!c.set_owner_name("danni"), "unchanged");
        assert!(c.set_owner_name("   "));
        assert_eq!(c.owner_name, None);
        assert_eq!(c.my_name(), DEFAULT_OWNER_NAME);
        // 손으로 고친 설정 파일의 긴 이름도 화면엔 20자까지만
        c.owner_name = Some("b".repeat(50));
        assert_eq!(c.my_name(), "b".repeat(20));
    }

    #[test]
    fn dodge_records_keep_the_best_and_survive_a_roundtrip() {
        let mut c = Config::default();
        assert!(c.record_dodge(12));
        assert!(!c.record_dodge(7), "lower score");
        assert!(!c.record_dodge(12), "same score");
        assert_eq!(c.dodge_best, 12);
        c.upsert_friend("a", "Ann", key(1));
        assert!(c.set_friend_dodge_best("a", 30));
        assert!(!c.set_friend_dodge_best("a", 30));
        assert!(!c.set_friend_dodge_best("nobody", 3));
        // 이름이 바뀌어도 기록은 남는다
        c.upsert_friend("a", "Annie", key(1));
        assert_eq!(c.friend("a").unwrap().dodge_best, Some(30));
        let back: Config = serde_json::from_str(&serde_json::to_string(&c).unwrap()).unwrap();
        assert_eq!((back.dodge_best, back.friends[0].dodge_best), (12, Some(30)));
        // 기록이 없으면 저장 파일에도 없고, 잘못된 값은 0 / None
        assert!(!serde_json::to_string(&Config::default()).unwrap().contains("dodge"));
        let bad: Config = serde_json::from_str(r#"{"pet":"cat","dodge_best":"lots"}"#).unwrap();
        assert_eq!(bad.dodge_best, 0);
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
    fn focus_length_and_daily_marks_roundtrip_and_old_files_still_load() {
        let old: Config = serde_json::from_str(r#"{"pet":"cat"}"#).unwrap();
        assert_eq!(old.focus_length, FocusLength::Short);
        assert_eq!(old.daily, DailyMarks::default());
        let plain = serde_json::to_string(&Config::default()).unwrap();
        assert!(!plain.contains("focus_length") && !plain.contains("daily"), "not written while default: {plain}");
        let c = Config { focus_length: FocusLength::Long, daily: DailyMarks { morning: Some(5), lunch: None }, ..Config::default() };
        let back: Config = serde_json::from_str(&serde_json::to_string(&c).unwrap()).unwrap();
        assert_eq!(back, c);
        let bad: Config = serde_json::from_str(r#"{"pet":"cat","focus_length":"forever"}"#).unwrap();
        assert_eq!((bad.pet.as_deref(), bad.focus_length), (Some("cat"), FocusLength::Short));
    }

    #[test]
    fn style_defaults_to_round_and_old_or_odd_files_still_load() {
        let old: Config = serde_json::from_str(r#"{"pet":"cat"}"#).unwrap();
        assert_eq!(old.style, PetStyle::Round);
        assert!(!serde_json::to_string(&Config::default()).unwrap().contains("style"), "not written while default");
        let c = Config { style: PetStyle::Pixel, ..Config::default() };
        let text = serde_json::to_string(&c).unwrap();
        assert!(text.contains(r#""style":"pixel""#), "{text}");
        assert_eq!(serde_json::from_str::<Config>(&text).unwrap(), c);
        for bad in [r#""sparkly""#, "3", "null", "{}"] {
            let odd: Config = serde_json::from_str(&format!(r#"{{"pet":"cat","style":{bad}}}"#)).unwrap();
            assert_eq!((odd.pet.as_deref(), odd.style), (Some("cat"), PetStyle::Round), "{bad}");
        }
    }

    #[test]
    fn ladder_settings_default_off_the_file_and_round_trip() {
        let old: Config = serde_json::from_str(r#"{"pet":"cat"}"#).unwrap();
        assert!(!old.ladder_invites_off);
        assert_eq!(old.last_ladder, None);
        let plain = serde_json::to_string(&Config::default()).unwrap();
        assert!(!plain.contains("ladder"), "{plain}");
        let last = LastLadder {
            host: "방장".into(),
            at: 1_793_000_000,
            print: "🐱🍎🎈🌙".into(),
            rows: vec![LastLadderRow { name: "나".into(), result: "커피".into(), mood: "bad".into(), me: true }],
        };
        let c = Config { ladder_invites_off: true, last_ladder: Some(last.clone()), ..Config::default() };
        let back: Config = serde_json::from_str(&serde_json::to_string(&c).unwrap()).unwrap();
        assert!(back.ladder_invites_off);
        assert_eq!(back.last_ladder, Some(last));
        let torn: Config = serde_json::from_str(r#"{"pet":"cat","last_ladder":{"host":3}}"#).unwrap();
        assert_eq!(torn.last_ladder, None);
    }

    #[test]
    fn cursor_play_is_on_unless_turned_off() {
        let old: Config = serde_json::from_str(r#"{"pet":"cat"}"#).unwrap();
        assert!(!old.cursor_play_off);
        let on = serde_json::to_string(&Config::default()).unwrap();
        assert!(!on.contains("cursor_play_off"));
        let off = Config { cursor_play_off: true, ..Config::default() };
        let back: Config = serde_json::from_str(&serde_json::to_string(&off).unwrap()).unwrap();
        assert!(back.cursor_play_off);
    }

    #[test]
    fn pooping_is_on_unless_turned_off() {
        let old: Config = serde_json::from_str(r#"{"pet":"cat"}"#).unwrap();
        assert!(!old.poop_off);
        let on = serde_json::to_string(&Config::default()).unwrap();
        assert!(!on.contains("poop_off"));
        let off = Config { poop_off: true, ..Config::default() };
        let back: Config = serde_json::from_str(&serde_json::to_string(&off).unwrap()).unwrap();
        assert!(back.poop_off);
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

    #[test]
    fn linked_device_secrets_roundtrip_and_old_friends_still_load() {
        let d = temp_dir("devices");
        let mut c = Config { install_id: Some("me".into()), ..Config::default() };
        c.upsert_friend("home", "Ann", key(1));
        c.upsert_friend("pal", "Bo", key(2));
        c.upsert_friend("pal2", "Bo", key(3));
        assert!(c.set_friend_device_secret("home", Some([7; 32])));
        assert!(!c.set_friend_device_secret("home", Some([7; 32])), "unchanged");
        assert!(!c.set_friend_device_secret("nobody", Some([7; 32])));
        assert!(c.set_friend_links("pal2", Some("pal"), true));
        assert!(!c.set_friend_links("pal2", Some("pal"), true), "unchanged");
        assert!(!c.set_friend_links("nobody", None, false));
        save(&d, &c).unwrap();
        let text = std::fs::read_to_string(d.join("config.json")).unwrap();
        // 연결 안 된 친구는 예전과 같은 모양으로 저장된다
        assert_eq!(text.matches("\"device_secret\"").count(), 1, "{text}");
        assert_eq!(text.matches("\"pending\"").count(), 1, "{text}");
        let back = load(&d);
        assert_eq!(back, c);
        let pal2 = back.friend("pal2").unwrap();
        assert_eq!((pal2.device_secret, pal2.via.as_deref(), pal2.pending), (None, Some("pal"), true));
        assert_eq!(back.friend("home").unwrap().device_secret, Some(DeviceSecret([7; 32])));
        assert_eq!(back.linked_device(), Some(("home", [7; 32])));
        // 비밀은 Debug로도 찍히지 않는다
        assert!(!format!("{back:?}").contains("7, 7"), "the secret is never printed");
        assert!(c.set_friend_device_secret("home", None), "unlinked");
        assert_eq!(c.linked_device(), None);
        // 예전 친구 항목 (예전 판의 "device": true 표시도): 연결 없음
        let old = format!(r#"{{"friends": [{{"id": "a", "name": "A", "public_key": "{}", "side": "left", "device": true}}]}}"#, "01".repeat(32));
        let old: Config = serde_json::from_str(&old).unwrap();
        let a = old.friend("a").unwrap();
        assert_eq!((a.device_secret, a.via.as_deref(), a.pending), (None, None, false));
    }

    #[test]
    fn bad_device_values_are_dropped_not_the_friend() {
        let k = "01".repeat(32);
        for secret in [r#""yes""#, r#""0102""#, "5", &format!(r#""{}""#, "zz".repeat(32))] {
            let text = format!(r#"{{"friends": [{{"id": "a", "name": "A", "public_key": "{k}", "side": "left", "device_secret": {secret}, "via": 5, "pending": []}}]}}"#);
            let c: Config = serde_json::from_str(&text).unwrap();
            let a = c.friend("a").expect("the friend stays");
            assert_eq!((a.device_secret, a.via.as_deref(), a.pending), (None, None, false), "{secret}");
        }
    }

    #[test]
    fn when_an_unconfirmed_friend_came_in_is_kept_and_old_or_bad_values_load_as_unknown() {
        let d = temp_dir("pending-since");
        let mut c = Config { install_id: Some("me".into()), ..Config::default() };
        c.upsert_friend("pal2", "Bo", key(3));
        c.upsert_friend("pal", "Bo", key(2));
        assert!(c.set_friend_pending_since("pal2", Some(1_700_000_000)));
        assert!(!c.set_friend_pending_since("pal2", Some(1_700_000_000)), "unchanged");
        assert!(!c.set_friend_pending_since("nobody", Some(1)));
        save(&d, &c).unwrap();
        let text = std::fs::read_to_string(d.join("config.json")).unwrap();
        assert_eq!(text.matches("\"pending_since\"").count(), 1, "{text}");
        let back = load(&d);
        assert_eq!(back, c);
        assert_eq!(back.friend("pal2").unwrap().pending_since, Some(1_700_000_000));
        assert_eq!(back.friend("pal").unwrap().pending_since, None);
        assert!(c.set_friend_pending_since("pal2", None), "confirmed");
        let k = "01".repeat(32);
        let text = format!(r#"{{"friends": [{{"id": "a", "name": "A", "public_key": "{k}", "side": "left", "pending": true, "pending_since": "soon"}}]}}"#);
        let c: Config = serde_json::from_str(&text).unwrap();
        assert_eq!(c.friend("a").expect("the friend stays").pending_since, None);
    }
}
