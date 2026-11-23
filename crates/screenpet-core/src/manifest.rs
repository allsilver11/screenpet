//! 📜 동물 정의 (pet.json) — 그림 시트의 동작별 줄·칸·fps, 성격, 걷기 속도, 특기 등을 읽고 검사함
//! parse_manifest로 읽어 PetManifest를 만들고, 앱이 이것으로 brain의 PetConfig를 채움
use std::collections::HashMap;

use serde::{Deserialize, Serialize};

pub use crate::behavior::Personality;
use crate::care::Stage;

/// 모든 동물이 반드시 가져야 하는 동작
pub const REQUIRED_ANIMS: [&str; 21] = [
    "walk", "run", "roll", "idle", "look", "sleep", "hop", "fall", "dragged", "land", "sit_front", "wave", "tilt", "belly", "stretch", "heart",
    "tickle", "annoyed", "hungry", "eat", "level_up",
];

/// 특정 동물만 가진 동작 — 있으면 검증함 (behavior::Special::anim_name, 벽 타기 climb/slide, 폴짝 동물 bigjump,
/// 올려다보기 lookup — 없으면 그냥 쳐다봄(look)). 기린 neckstretch·레서판다 surprise는 그 동물과 함께 없어짐 (v0.7.0).
/// 폴짝(hop)은 모든 동물의 필수 동작이라 REQUIRED_ANIMS에 이미 있음. 순서는 tools/sprite_spec.py OPTIONAL_ANIMS와 같음
/// 공룡 fire·roar·stomp — 있으면 쉬거나 간질일 때 셋 중 하나 (Special::Fire/Roar/Stomp, 쿵쾅은 친구들을 흔듦).
/// 흐물이 panic (만세 줄행랑) · hang·rope (창 옆면 매달리기·오르내리기) · lift·hold·carry·throw·strain·present·slump (물건 들어 던지기)
/// · yank·stuff (📌 작업표시줄에서 장난감 끌어내기·도로 밀어 넣기)
/// · hang_cursor·peek·dance·dance2·catch·draw·cheer_type·type (흐물이 더 놀기, v0.6.3 — ANTICS_ANIMS, type = ⌨️ 키보드 v0.6.4)
/// · paw (🐱 고양이 툭툭 — KNOCK_ANIMS) · cheeks_walk·cheeks_idle·dig (🐿️ 다람쥐 볼 빵빵 — STASH_ANIMS, v0.6.3)
/// · shoot·web_swing·hero_land·web_pull·web_whirl·hang_down (🕷️ 거미인간 — WEB_ANIMS) · ride·wheelie·bike_jump·drift·signal (🦇 박쥐인간 — BIKE_ANIMS)
/// · fly·fly_fast·fly_flip·hover (🦸 슈퍼인간 — SOAR_ANIMS)
/// · broom (🧹 빗자루 — BROOM_ANIMS) · cast (✨ 지팡이 주문 — SPELL_ANIMS) · read·raise_hand (📖🙋 헤르미온느 — BOOK_ANIMS)
/// · bag_walk·bag_drag·bag_stuff (👜 헤르미온느 가방 청소 — BAG_ANIMS)
/// · dragon·sidecar·sidecar_fly·hug·hug_walk·pat·egg·offer (🧔 해그리드 — HAGRID_ANIMS, dragon은 빗자루 대신 용)
/// · suit_up (🛡️ 무쇠인간 투구 장착 — SUIT_ANIMS, behavior::Special::SuitUp).
pub const OPTIONAL_ANIMS: [&str; 65] = [
    "arch", "huff", "climb", "slide", "bigjump", "lookup", "fire", "roar", "stomp", "panic", "hang", "rope", "lift", "hold",
    "carry", "throw", "strain", "present", "slump", "yank", "stuff", "hang_cursor", "peek", "dance", "dance2", "catch", "draw", "cheer_type", "type",
    "carry_run", "paw", "cheeks_walk", "cheeks_idle", "dig", "shoot", "web_swing", "hero_land", "web_pull",
    "web_whirl", "hang_down", "ride", "wheelie", "bike_jump", "drift", "signal", "fly", "fly_fast", "fly_flip", "hover",
    "broom", "cast", "read", "raise_hand", "bag_walk", "bag_drag", "bag_stuff",
    "dragon", "sidecar", "sidecar_fly", "hug", "hug_walk", "pat", "egg", "offer",
    "suit_up",
];
/// 🛡️ 무쇠인간 그림: 🪖 투구 장착 (있으면 behavior::Special::SuitUp — 🚀 제트팩 날기는 SOAR_ANIMS를 같이 씀)
pub const SUIT_ANIMS: [&str; 1] = ["suit_up"];
/// 🧔 해그리드 그림: 🐉 용 타기 · 🏍️ 사이드카 오토바이 · 붕 날기 · 🤗 안기 · 안고 걷기 · 🖐️ 쓰담 · 🥚 알 품기 · 🪨 락 케이크
/// (pet.json `gamekeeper`가 있어도 이게 다 있어야 함 — is_gamekeeper(); 사이드카 자리·안은 손·알 = grip)
pub const HAGRID_ANIMS: [&str; 8] = ["dragon", "sidecar", "sidecar_fly", "hug", "hug_walk", "pat", "egg", "offer"];
/// 🐉 빗자루 대신 탈 용 그림 (broom_rider가 이것만 있어도 빗자루 비행을 용으로 — rides_broom(), dragon_mount())
pub const DRAGON_ANIM: &str = "dragon";
/// 🧹 빗자루 그림: 빗자루에 앉아 날기 (pet.json `broom_rider`가 있어도 이게 있어야 함 — rides_broom())
pub const BROOM_ANIMS: [&str; 1] = ["broom"];
/// ✨ 주문 그림: 지팡이 휙 + 끝에 반짝 (pet.json `spells`가 있어도 이게 있어야 함 — spell_set(), 지팡이 끝 = grip.cast)
pub const SPELL_ANIMS: [&str; 1] = ["cast"];
/// 📖🙋 책벌레 그림: 앉아서 책 읽기 · 손 번쩍 "I know!" (pet.json `bookworm`가 있어도 이게 다 있어야 함 — reads_books())
pub const BOOK_ANIMS: [&str; 2] = ["read", "raise_hand"];
/// 👜 가방 청소 그림: 가방 들고 걷기 · 큰 가방 질질 끌기 · 가방에 쏙 넣기 (pet.json `bag_cleaner`가 있어도 이게 다 있어야 함 — cleans_with_bag(),
/// 가방 끈을 쥔 손 = grip.bag_walk·bag_drag·bag_stuff — 가방은 웹뷰가 그 손에 그림)
pub const BAG_ANIMS: [&str; 3] = ["bag_walk", "bag_drag", "bag_stuff"];
/// 🦸 슈퍼인간 그림: ✈️ 날기 · 빠른 날기 · 🔄 공중제비 · 둥실 (pet.json `super_flier`가 있어도 이게 다 있어야 함 — soars())
pub const SOAR_ANIMS: [&str; 4] = ["fly", "fly_fast", "fly_flip", "hover"];
/// 🦇 박쥐인간 그림: 🏍️ 오토바이 타기 · 윌리 · 오토바이 점프 · 드리프트 · 🔦 박쥐 신호 (pet.json `bike_rider`가 있어도 이게 다 있어야 함 — rides_bike())
pub const BIKE_ANIMS: [&str; 5] = ["ride", "wheelie", "bike_jump", "drift", "signal"];
/// 🕷️ 거미인간 그림: 줄 쏘기 · 줄 그네 · 영웅 착지 · 줄 끌어당기기 · 🌀 줄 휘두르기 · 🙃 거꾸로 매달리기 (pet.json `web_slinger`가 있어도 이게 다 있어야 함 — slings_webs())
pub const WEB_ANIMS: [&str; 6] = ["shoot", "web_swing", "hero_land", "web_pull", "web_whirl", "hang_down"];
/// 🐱 툭툭 그림 (pet.json `knocker`가 있어도 이게 있어야 함 — knocks())
pub const KNOCK_ANIMS: [&str; 1] = ["paw"];
/// 🐿️ 볼 빵빵 그림 (pet.json `stasher`가 있어도 이게 다 있어야 함 — stashes())
pub const STASH_ANIMS: [&str; 3] = ["cheeks_walk", "cheeks_idle", "dig"];
/// 물건 들어 던지기 그림 (pet.json `juggler`가 있어도 이게 다 있어야 함)
pub const JUGGLE_ANIMS: [&str; 7] = ["lift", "hold", "carry", "throw", "strain", "present", "slump"];
/// 📌 작업표시줄 놀이 그림 (pet.json `taskbar_play`가 있어도 이게 다 있어야 하고, 들고 던지기도 해야 함)
pub const TASKBAR_ANIMS: [&str; 2] = ["yank", "stuff"];
/// 흐물이 더 놀기 그림 (v0.6.3): 🎣 커서 매달리기 · 🫣 숨바꼭질 · 🕺 연타 춤 둘 · ⚾ 받기 · 🖍️ 낙서 · ⌨️ 타자 응원 (만세 · 키보드 타자 v0.6.4)
/// — 들고 던지기를 하고 이게 다 있어야 함 (antics())
pub const ANTICS_ANIMS: [&str; 9] = ["hang_cursor", "peek", "dance", "dance2", "catch", "draw", "cheer_type", "type", "carry_run"];

/// 😜 흐물이 장난 하나 (pet.json `pranks` — 흐물이 아닌 동물이 나눠 받는 것)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Prank {
    /// 🙆 같은 바닥의 다른 펫을 머리 위로 번쩍 들어 조금 들고 다니다 내려놓음 (brain_hoist.rs — 들고 던지기 그림을 씀)
    Hoist,
    /// 🫣 창 옆면 뒤에 숨어 머리만 빼꼼 (brain_hide.rs — peek 그림)
    Hide,
}

/// 이동 방식 (pet.json `locomotion`, 없으면 walk)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Locomotion {
    /// 디딤판 위를 걸음
    #[default]
    Walk,
    /// 날아다님 (지금 번들 동물엔 없음 — 슈퍼인간이 쓸 수 있음) — 쉴 때 가끔 창 윗면·바닥에 내려앉음
    Fly,
    /// 폴짝폴짝 뛰어다님 (지금 번들엔 없음 — 개구리는 v0.7.1에 빠짐) — 가끔 크게 뛰어 창 위로, 벽은 폴짝폴짝 오름 (hop.rs)
    Hop,
}

/// 사는 집 그림 (pet.json `house`, 없으면 보통 집)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HouseKind {
    /// 보통 집 (ui/house/house_<stage>)
    House,
    /// 호박의 귀신의 집 (ui/house/haunted_<stage>)
    Haunted,
    /// 루돌프·산타의 눈 덮인 크리스마스 벽돌집 (ui/house/xmas_<stage>, 굴뚝 연기는 ui/house.js가 피움)
    Xmas,
    /// 🏰 해리·헤르미온느·해그리드의 호그와트 성 (ui/house/castle_<stage> — 뾰족 지붕 첨탑 여럿, 불 켜진 작은 창)
    Castle,
    /// 🏚️ 볼드모트·디멘터의 쓰러져 가는 나무집 (ui/house/shack_<stage> — 기울어진 판자, 깨진 창, 내려앉은 지붕)
    Shack,
}

/// 그림 시트의 동작 하나
#[derive(Debug, Clone, Deserialize)]
pub struct AnimSpec {
    /// 시트의 줄 번호
    pub row: u32,
    /// 칸 수
    pub frames: u32,
    /// 초당 칸 수
    pub fps: f64,
    /// 되풀이함 (기본 true)
    #[serde(rename = "loop", default = "yes")]
    pub looping: bool,
    /// 앞모습 동작 — 웹뷰는 좌우 반전하지 않음
    #[serde(default)]
    pub front: bool,
    /// 초당 회전 각도 (roll 전용, 0이면 회전 없음)
    #[serde(default)]
    pub spin: f64,
}

/// serde 기본값: true
fn yes() -> bool {
    true
}

/// 동물별 좋아하는 음식 (음식 고르기 버튼·트레이 메뉴에 씀)
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FavoriteFood {
    /// 이모지
    pub emoji: String,
    /// 이름
    pub name: String,
}

/// pet.json 한 장
#[derive(Debug, Clone, Deserialize)]
pub struct PetManifest {
    /// 동물 이름
    pub name: String,
    /// 칸 한 변 (px)
    pub frame_size: u32,
    /// 단계마다 칸 크기가 다른 시트 (단계 이름 → 칸 px, 없는 단계는 frame_size) — 고해상도 단계는 2배 칸
    #[serde(default)]
    pub frame_sizes: HashMap<String, u32>,
    /// 걷기 속도 (논리 px/s)
    pub walk_speed: f64,
    /// 동작 이름 → 그림 시트 자리
    pub animations: HashMap<String, AnimSpec>,
    /// 성격 (없으면 friendly)
    #[serde(default)]
    pub personality: Personality,
    /// 단계별 몸 크기 배율 (단계 이름 → 배율, 없으면 1.0)
    #[serde(default)]
    pub stage_size: HashMap<String, f64>,
    /// 좋아하는 음식 (없으면 None)
    #[serde(default)]
    pub favorite_food: Option<FavoriteFood>,
    /// 이동 방식 (없으면 walk)
    #[serde(default)]
    pub locomotion: Locomotion,
    /// 걷는 동물이 걷기·달리기 토막 중 깡총거리는 몫 0~1 (토끼 0.5, 없으면 0 — 폴짝 동물은 locomotion hop이라 늘 폴짝)
    #[serde(default)]
    pub hop_share: f64,
    /// 사는 집 (없으면 보통 집)
    #[serde(default)]
    pub house: Option<HouseKind>,
    /// 가끔 머리 위에 작은 눈구름을 부름 (눈사람)
    #[serde(default)]
    pub snow_cloud: bool,
    /// 장난칠 때 안경이 번쩍 (펭귄 — brain_flash.rs, 웹뷰 render.flash)
    #[serde(default)]
    pub glasses_flash: bool,
    /// 겁쟁이 — 다른 동물과 마주치면 반쯤 만세 줄행랑 (흐물이, panic 그림이 있을 때만 — timid())
    #[serde(default)]
    pub timid: bool,
    /// 기운 넘침 — 더 자주 달리고 더 자주 크게 뜀 (흐물이)
    #[serde(default)]
    pub energetic: bool,
    /// 창 옆면에 매달리기 (흐물이, hang·rope 그림이 있을 때만 — rigs())
    #[serde(default)]
    pub rigging: bool,
    /// 바닥의 물건·똥을 주워 머리 위로 들고 던짐 (흐물이, JUGGLE_ANIMS가 다 있을 때만 — juggles())
    #[serde(default)]
    pub juggler: bool,
    /// 😜 흐물이 장난 중 이 동물도 하는 것 (🙆 hoist 다른 펫 번쩍 들기 — 들고 던지기 그림이 있을 때만 · 🫣 hide 창 뒤 숨바꼭질 — peek 그림이 있을 때만).
    /// 흐물이는 antics로 이미 다 함 (hoists·hides)
    #[serde(default)]
    pub pranks: Vec<Prank>,
    /// 📌 작업표시줄(Dock)에서 열린 앱 아이콘 장난감을 끌어내 놀다가 도로 넣음 (흐물이, 들고 던지기 + TASKBAR_ANIMS가 있을 때만 — plays_taskbar())
    #[serde(default)]
    pub taskbar_play: bool,
    /// 🐱 창 윗면의 작은 물건을 툭툭 밀어 떨어뜨림 (고양이, paw 그림이 있을 때만 — knocks())
    #[serde(default)]
    pub knocker: bool,
    /// 🐿️ 바닥의 작은 물건을 볼에 넣어 물음 (다람쥐, STASH_ANIMS가 다 있을 때만 — stashes())
    #[serde(default)]
    pub stasher: bool,
    /// 🕷️ 줄을 쏘아 모서리에 거미집을 치고, 줄 그네를 타고, 영웅 착지를 하고, 물건·친구를 끌어옴 (거미인간, WEB_ANIMS가 다 있을 때만 — slings_webs())
    #[serde(default)]
    pub web_slinger: bool,
    /// 🦇 오토바이를 자주 타고 윌리·점프·드리프트, 가끔 박쥐 신호 조명, 친구 화면도 오토바이로 오감 (박쥐인간, BIKE_ANIMS가 다 있을 때만 — rides_bike())
    #[serde(default)]
    pub bike_rider: bool,
    /// 🦸 걷기 대신 바닥 조금 위를 낮게 날고, 공중제비, 디딤판 끝에서 멀리 활공, 친구 화면도 날아서 오감 (슈퍼인간, SOAR_ANIMS가 다 있을 때만 — soars())
    #[serde(default)]
    pub super_flier: bool,
    /// 🧹 가끔 빗자루를 타고 화면을 날아다니고 (낮게·높게), 친구 화면도 빗자루로 오감 (해리포터·헤르미온느 — BROOM_ANIMS가 있을 때만, rides_broom())
    #[serde(default)]
    pub broom_rider: bool,
    /// ✨ 아는 주문 (해리 expelliarmus·leviosa, 헤르미온느 leviosa·reparo — SPELL_ANIMS가 있을 때만, spell_set())
    #[serde(default)]
    pub spells: Vec<crate::wizard::Spell>,
    /// 🟡 골든 스니치를 빗자루로 쫓아 잡음 (해리 — 빗자루를 탈 때만, seeks())
    #[serde(default)]
    pub seeker: bool,
    /// 📖 자주 앉아서 책을 읽고 🙋 "I know!" (헤르미온느 — BOOK_ANIMS가 다 있을 때만, reads_books())
    #[serde(default)]
    pub bookworm: bool,
    /// 👜 끝없는 구슬 가방으로 바닥의 물건을 주워 치움 (헤르미온느 — BAG_ANIMS가 다 있을 때만, cleans_with_bag())
    #[serde(default)]
    pub bag_cleaner: bool,
    /// 🐍 볼드모트: 가끔 나기니를 불러 발밑을 기게 하고 😈 "hehehe" (SPELL_ANIMS가 있을 때만 — is_dark_lord())
    #[serde(default)]
    pub dark_lord: bool,
    /// 👻 디멘터: 다가가면 펫이 시무룩·배고파지고 화면 구석에 🥶 서리, 🦌 패트로누스에 쫓김 (나는 동물일 때만 — is_dementor())
    #[serde(default)]
    pub dementor: bool,
    /// 🦌🦦 해리 "stag"·헤르미온느 "otter": 디멘터가 오면 "Expecto Patronum!" 은빛 동물 (옛 true는 사슴 — SPELL_ANIMS가 있을 때만, patronus_animal())
    #[serde(default, deserialize_with = "patronus_field")]
    pub patronus: Option<crate::wizard::PatronusAnimal>,
    /// 🧔 해그리드: 🏍️ 사이드카 오토바이·🤗 안아 들기·🖐️ 쓰담·🥚 용알·🪨 락 케이크 (HAGRID_ANIMS가 다 있을 때만 — is_gamekeeper())
    #[serde(default)]
    pub gamekeeper: bool,
    /// 🧳 해리·헤르미온느·볼드모트: 놓인 물건을 포트키로 짝 물건 옆으로 순간이동 (SPELL_ANIMS가 있을 때만 — uses_portkeys())
    #[serde(default)]
    pub portkey: bool,
    /// 🎨 잉크 선으로 그려 몸 색을 고를 수 있음 (흐물이 — 웹뷰가 ink_<stage>.png 자리를 고른 색으로 덮음, core::tint)
    #[serde(default)]
    pub tint: bool,
    /// 물건을 든 동작의 칸별 손 자리 (단계 → 동작 → 칸별 [x, y] 칸 px — 그림 생성기가 적음). 든 물건의 바닥이 여기 옴
    #[serde(default)]
    pub grip: HashMap<String, HashMap<String, Vec<[f64; 2]>>>,
    /// 칸마다 머리 꼭대기 (단계 → 동작 → 칸별 [x, y, 너비, 기울기] 칸 px — 그림 생성기가 적음)
    #[serde(default)]
    pub head: HashMap<String, HashMap<String, Vec<[f64; 4]>>>,
}

/// head 표가 없는 동물의 매달리는 자리: 그림 가운데, 윗변에서 조금 아래
const DEFAULT_HANG_GRIP: (f64, f64) = (0.0, 0.06);
/// 머리 꼭대기는 모자 입구보다 입구 너비의 이만큼 위
const HEAD_TOP_ABOVE_BRIM: f64 = 0.15;

/// 🦌🦦 pet.json `patronus` 읽기: true = 사슴 (옛 값) · "stag"·"otter" · false·null·숫자·모르는 이름은 없음 (다른 특기는 그대로 읽히게)
fn patronus_field<'de, D: serde::Deserializer<'de>>(d: D) -> Result<Option<crate::wizard::PatronusAnimal>, D::Error> {
    use crate::wizard::PatronusAnimal;
    // 어떤 JSON 값이든 받고 (null·숫자·목록도 pet.json 전체를 버리지 않음) 아는 값만 고름
    Ok(match serde_json::Value::deserialize(d)? {
        serde_json::Value::Bool(true) => Some(PatronusAnimal::Stag),
        serde_json::Value::String(n) if n == "stag" => Some(PatronusAnimal::Stag),
        serde_json::Value::String(n) if n == "otter" => Some(PatronusAnimal::Otter),
        _ => None,
    })
}

impl PetManifest {
    /// 커서에 매달린 그림(dragged 칸들의 평균)에서 커서 끝이 닿을 머리 꼭대기: (그림 가운데에서 오른쪽, 그림 윗변에서 아래) — 한 변에 대한 비율.
    /// 표의 y는 모자 입구라 머리 위쪽으로 조금 올림. ui/main.js hangOrigin과 같은 계산
    pub fn hang_grip(&self, stage: &str) -> (f64, f64) {
        let size = f64::from(self.frame_size_at(stage));
        let Some(cells) = self.head.get(stage).and_then(|anims| anims.get("dragged")).filter(|c| !c.is_empty()) else {
            return DEFAULT_HANG_GRIP;
        };
        let n = cells.len() as f64;
        let mean = |i: usize| cells.iter().map(|c| c[i]).sum::<f64>() / n;
        (mean(0) / size - 0.5, (mean(1) - HEAD_TOP_ABOVE_BRIM * mean(2)).max(0.0) / size)
    }
}

/// pet.json을 읽고 검사함 (필수 동작·칸 크기 등). 틀리면 이유를 글로 돌려줌
pub fn parse_manifest(json: &str) -> Result<PetManifest, String> {
    let m: PetManifest = serde_json::from_str(json).map_err(|e| e.to_string())?;
    if m.frame_size == 0 {
        return Err("frame_size must be > 0".into());
    }
    if let Some((stage, _)) = m.frame_sizes.iter().find(|(_, v)| **v == 0) {
        return Err(format!("frame_sizes {stage} must be > 0"));
    }
    if m.walk_speed <= 0.0 {
        return Err("walk_speed must be > 0".into());
    }
    if let Some((stage, v)) = m.stage_size.iter().find(|(_, v)| !(v.is_finite() && **v > 0.0)) {
        return Err(format!("stage_size {stage}: {v} must be finite and > 0"));
    }
    if !(0.0..=1.0).contains(&m.hop_share) {
        return Err(format!("hop_share {} must be within 0..=1", m.hop_share));
    }
    check_anims(&m.animations, &REQUIRED_ANIMS, &OPTIONAL_ANIMS)?;
    Ok(m)
}

/// required는 있어야 하고, 있는 동작(required·optional)은 칸 수와 속도가 0보다 커야 함
fn check_anims(animations: &HashMap<String, AnimSpec>, required: &[&str], optional: &[&str]) -> Result<(), String> {
    for &name in required.iter().chain(optional) {
        match animations.get(name) {
            Some(spec) if spec.frames == 0 || spec.fps <= 0.0 => return Err(format!("animation {name}: frames and fps must be > 0")),
            None if required.contains(&name) => return Err(format!("missing animation: {name}")),
            _ => {}
        }
    }
    Ok(())
}

impl PetManifest {
    /// 이 동작 그림이 있음
    pub fn has_anim(&self, name: &str) -> bool {
        self.animations.contains_key(name)
    }

    /// 이 단계 시트의 칸 크기 (px)
    pub fn frame_size_at(&self, stage: &str) -> u32 {
        self.frame_sizes.get(stage).copied().unwrap_or(self.frame_size)
    }

    /// 이 동물의 특별 동작 (pet.json에 있는 그림으로 찾음)
    pub fn special(&self) -> Option<crate::behavior::Special> {
        crate::behavior::Special::from_manifest(|n| self.has_anim(n))
    }

    /// 벽 타기 (뒷모습 climb + slide 둘 다 있어야 함)
    pub fn can_climb(&self) -> bool {
        self.has_anim("climb") && self.has_anim("slide")
    }

    /// 나는 동물임
    pub fn flies(&self) -> bool {
        self.locomotion == Locomotion::Fly
    }

    /// 폴짝 뛰는 동물임
    pub fn hops(&self) -> bool {
        self.locomotion == Locomotion::Hop
    }

    /// 걷기·달리기 토막 중 깡총거리는 몫 (폴짝 동물·나는 동물은 0 — 폴짝 동물은 늘 폴짝, 나는 동물은 남)
    pub fn hop_share(&self) -> f64 {
        if self.locomotion == Locomotion::Walk { self.hop_share } else { 0.0 }
    }

    /// 위를 올려다봄 (pet.json에 lookup이 있으면 — 없으면 그냥 쳐다봄)
    pub fn can_look_up(&self) -> bool {
        self.has_anim("lookup")
    }

    /// 겁쟁이 (panic 그림이 있어야 함)
    pub fn timid(&self) -> bool {
        self.timid && self.has_anim("panic")
    }

    /// 창 옆면 매달리기 (hang·rope 그림이 있어야 함)
    pub fn rigs(&self) -> bool {
        self.rigging && self.has_anim("hang") && self.has_anim("rope")
    }

    /// 물건 들어 던지기 (JUGGLE_ANIMS 그림이 다 있어야 함)
    pub fn juggles(&self) -> bool {
        self.juggler && JUGGLE_ANIMS.iter().all(|a| self.has_anim(a))
    }

    /// 📌 작업표시줄 놀이 (들고 던지기를 하고 TASKBAR_ANIMS 그림이 다 있어야 함)
    pub fn plays_taskbar(&self) -> bool {
        self.taskbar_play && self.juggles() && TASKBAR_ANIMS.iter().all(|a| self.has_anim(a))
    }

    /// 🐱 툭툭 (KNOCK_ANIMS 그림이 있어야 함)
    pub fn knocks(&self) -> bool {
        self.knocker && KNOCK_ANIMS.iter().all(|a| self.has_anim(a))
    }

    /// 🐿️ 볼 빵빵 (STASH_ANIMS 그림이 다 있어야 함)
    pub fn stashes(&self) -> bool {
        self.stasher && STASH_ANIMS.iter().all(|a| self.has_anim(a))
    }

    /// 🕷️ 줄 쓰기 (WEB_ANIMS 그림이 다 있어야 함)
    pub fn slings_webs(&self) -> bool {
        self.web_slinger && WEB_ANIMS.iter().all(|a| self.has_anim(a))
    }

    /// 🦇 오토바이 타기 (BIKE_ANIMS 그림이 다 있어야 함)
    pub fn rides_bike(&self) -> bool {
        self.bike_rider && BIKE_ANIMS.iter().all(|a| self.has_anim(a))
    }

    /// 🦸 낮게 날기 (SOAR_ANIMS 그림이 다 있어야 함)
    pub fn soars(&self) -> bool {
        self.super_flier && SOAR_ANIMS.iter().all(|a| self.has_anim(a))
    }

    /// 🧹 빗자루 타기 (BROOM_ANIMS 그림 — 🐉 해그리드는 dragon 그림이 있어야 함)
    pub fn rides_broom(&self) -> bool {
        self.broom_rider && (BROOM_ANIMS.iter().all(|a| self.has_anim(a)) || self.dragon_mount())
    }

    /// 🐉 빗자루 대신 용을 탐 (dragon 그림)
    pub fn dragon_mount(&self) -> bool {
        self.has_anim(DRAGON_ANIM)
    }

    /// 🧳 포트키 (SPELL_ANIMS 그림이 있어야 함 — 손을 댈 때 cast)
    pub fn uses_portkeys(&self) -> bool {
        self.portkey && SPELL_ANIMS.iter().all(|a| self.has_anim(a))
    }

    /// 🧔 해그리드 (HAGRID_ANIMS 그림이 다 있어야 함)
    pub fn is_gamekeeper(&self) -> bool {
        self.gamekeeper && HAGRID_ANIMS.iter().all(|a| self.has_anim(a))
    }

    /// ✨ 쓸 수 있는 주문 (SPELL_ANIMS 그림이 없으면 없음)
    pub fn spell_set(&self) -> crate::wizard::SpellSet {
        if SPELL_ANIMS.iter().all(|a| self.has_anim(a)) {
            crate::wizard::SpellSet::of(&self.spells)
        } else {
            crate::wizard::SpellSet::default()
        }
    }

    /// 🟡 스니치 쫓기 (빗자루를 탈 수 있어야 함)
    pub fn seeks(&self) -> bool {
        self.seeker && self.rides_broom()
    }

    /// 📖🙋 책 읽기·"I know!" (BOOK_ANIMS 그림이 다 있어야 함)
    pub fn reads_books(&self) -> bool {
        self.bookworm && BOOK_ANIMS.iter().all(|a| self.has_anim(a))
    }

    /// 👜 가방 청소 (BAG_ANIMS 그림이 다 있어야 함)
    pub fn cleans_with_bag(&self) -> bool {
        self.bag_cleaner && BAG_ANIMS.iter().all(|a| self.has_anim(a))
    }

    /// 🐍 나기니·hehehe (SPELL_ANIMS 그림이 있어야 함)
    pub fn is_dark_lord(&self) -> bool {
        self.dark_lord && SPELL_ANIMS.iter().all(|a| self.has_anim(a))
    }

    /// 👻 디멘터 (나는 동물이어야 함)
    pub fn is_dementor(&self) -> bool {
        self.dementor && self.flies()
    }

    /// 🦌 패트로누스 (SPELL_ANIMS 그림이 있어야 함)
    pub fn casts_patronus(&self) -> bool {
        self.patronus_animal().is_some()
    }

    /// 🦌🦦 패트로누스 동물 (SPELL_ANIMS 그림이 없으면 없음)
    pub fn patronus_animal(&self) -> Option<crate::wizard::PatronusAnimal> {
        self.patronus.filter(|_| SPELL_ANIMS.iter().all(|a| self.has_anim(a)))
    }

    /// 흐물이 더 놀기 (들고 던지기를 하고 ANTICS_ANIMS 그림이 다 있어야 함)
    pub fn antics(&self) -> bool {
        self.juggles() && ANTICS_ANIMS.iter().all(|a| self.has_anim(a))
    }

    /// 🙆 다른 펫 번쩍 들기: 흐물이 (antics) 또는 pranks에 hoist가 있고 들고 던지기를 함
    pub fn hoists(&self) -> bool {
        self.antics() || (self.pranks.contains(&Prank::Hoist) && self.juggles())
    }

    /// 🫣 창 뒤 숨바꼭질: 흐물이 (antics) 또는 pranks에 hide가 있고 peek 그림이 있음
    pub fn hides(&self) -> bool {
        self.antics() || (self.pranks.contains(&Prank::Hide) && self.has_anim("peek"))
    }

    /// 이 단계의 손 자리 표 (칸 px → 한 변에 대한 비율: 그림 가운데에서 오른쪽, 윗변에서 아래). 없는 동작은 머리 위 (juggle::DEFAULT_GRIP)
    pub fn grip_table(&self, stage: &str) -> crate::juggle::GripTable {
        let size = f64::from(self.frame_size_at(stage));
        let anim = |name: &str| {
            let (Some(cells), Some(spec)) = (self.grip.get(stage).and_then(|a| a.get(name)).filter(|c| !c.is_empty()), self.animations.get(name)) else {
                return crate::juggle::GripAnim::default();
            };
            let mut g = crate::juggle::GripAnim { n: cells.len().min(4), fps: spec.fps, looping: spec.looping, ..Default::default() };
            for (i, c) in cells.iter().take(4).enumerate() {
                g.pts[i] = (c[0] / size - 0.5, c[1] / size);
            }
            g
        };
        crate::juggle::GripTable { lift: anim("lift"), hold: anim("hold"), carry: anim("carry"), throw: anim("throw"), present: anim("present"), yank: anim("yank"), stuff: anim("stuff"), catch: anim("catch"), hang_cursor: anim("hang_cursor"), carry_run: anim("carry_run"), shoot: anim("shoot"), web_swing: anim("web_swing"), web_pull: anim("web_pull"), web_whirl: anim("web_whirl"), hang_down: anim("hang_down"), cast: anim("cast"), hug: anim("hug"), hug_walk: anim("hug_walk"), sidecar: anim("sidecar"), sidecar_fly: anim("sidecar_fly"), egg: anim("egg") }
    }

    /// 사는 집 그림: pet.json `house`, 없으면 보통 집
    pub fn house_kind(&self) -> HouseKind {
        self.house.unwrap_or(HouseKind::House)
    }

    /// 이 동물의 단계별 크기 배율 (care::Stage::scale에 곱함)
    pub fn size_mult(&self, stage: Stage) -> f64 {
        self.stage_size.get(stage.name()).copied().unwrap_or(1.0)
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn hang_grip_is_the_mean_head_top_of_the_dragged_cells() {
        let json = r#"{"name":"t","frame_size":128,"walk_speed":60,"animations":{},
            "head":{"baby":{"dragged":[[70,30,40,-4],[74,32,40,4]]}}}"#;
        let m: super::PetManifest = serde_json::from_str(json).unwrap();
        let (x, y) = m.hang_grip("baby");
        assert!((x - 8.0 / 128.0).abs() < 1e-9, "{x}");
        assert!((y - 25.0 / 128.0).abs() < 1e-9, "{y}");
        assert_eq!(m.hang_grip("adult"), super::DEFAULT_HANG_GRIP, "no table for this stage");
    }

    /// 2배 칸 단계 (frame_sizes): 표도 칸 px로 2배라 비율로 재는 값은 128 칸일 때와 같음 — 표시 크기는 칸과 상관없음
    #[test]
    fn a_doubled_stage_sheet_keeps_the_same_hang_grip() {
        let json = r#"{"name":"t","frame_size":128,"frame_sizes":{"adult":256},"walk_speed":60,"animations":{},
            "head":{"baby":{"dragged":[[70,30,40,-4],[74,32,40,4]]},"adult":{"dragged":[[140,60,80,-4],[148,64,80,4]]}}}"#;
        let m: super::PetManifest = serde_json::from_str(json).unwrap();
        assert_eq!((m.frame_size_at("baby"), m.frame_size_at("adult")), (128, 256));
        assert_eq!(m.hang_grip("baby"), m.hang_grip("adult"));
    }

    use super::*;

    /// 테스트: 필수 동작이 다 든 pet.json
    fn full_json() -> String {
        let anims: Vec<String> = REQUIRED_ANIMS
            .iter()
            .enumerate()
            .map(|(i, a)| format!(r#""{a}": {{"row": {i}, "frames": 2, "fps": 4}}"#))
            .collect();
        format!(r#"{{"name": "고양이", "frame_size": 64, "walk_speed": 60, "animations": {{{}}}}}"#, anims.join(","))
    }

    #[test]
    fn parses_valid_manifest_with_loop_default_true() {
        let m = parse_manifest(&full_json()).unwrap();
        assert_eq!(m.name, "고양이");
        assert!(m.animations["walk"].looping);
    }

    #[test]
    fn loop_false_is_respected() {
        let json = full_json().replace(r#""hop": {"row": 6, "frames": 2, "fps": 4}"#, r#""hop": {"row": 6, "frames": 2, "fps": 4, "loop": false}"#);
        assert!(!parse_manifest(&json).unwrap().animations["hop"].looping);
    }

    #[test]
    fn front_and_spin_parse_with_defaults() {
        let json = full_json().replace(r#""wave": {"row": 11, "frames": 2, "fps": 4}"#, r#""wave": {"row": 11, "frames": 2, "fps": 4, "front": true}"#)
            .replace(r#""roll": {"row": 2, "frames": 2, "fps": 4}"#, r#""roll": {"row": 2, "frames": 2, "fps": 4, "spin": 271.4}"#);
        let m = parse_manifest(&json).unwrap();
        assert!(m.animations["wave"].front);
        assert!(!m.animations["walk"].front);
        assert_eq!(m.animations["roll"].spin, 271.4);
        assert_eq!(m.animations["walk"].spin, 0.0);
    }

    #[test]
    fn rejects_missing_animation() {
        let json = full_json().replace(r#""sleep""#, r#""nap""#);
        assert!(parse_manifest(&json).unwrap_err().contains("sleep"));
    }

    #[test]
    fn rejects_zero_frames_or_bad_speed() {
        assert!(parse_manifest(&full_json().replace(r#""frames": 2"#, r#""frames": 0"#)).is_err());
        assert!(parse_manifest(&full_json().replace(r#""walk_speed": 60"#, r#""walk_speed": 0"#)).is_err());
    }

    #[test]
    fn optional_specials_are_validated_when_present() {
        let with_arch = full_json().replace(r#""animations": {"#, r#""animations": {"arch": {"row": 18, "frames": 3, "fps": 6},"#);
        assert!(parse_manifest(&with_arch).unwrap().has_anim("arch"));
        assert!(!parse_manifest(&full_json()).unwrap().has_anim("huff"));
        let bad = full_json().replace(r#""animations": {"#, r#""animations": {"huff": {"row": 18, "frames": 0, "fps": 8},"#);
        assert!(parse_manifest(&bad).is_err());
    }

    #[test]
    fn climbing_needs_both_climb_and_slide() {
        let add = |extra: &str| full_json().replace(r#""animations": {"#, &format!(r#""animations": {{{extra}"#));
        assert_eq!(OPTIONAL_ANIMS[1..4], ["huff", "climb", "slide"]);
        assert!(!parse_manifest(&full_json()).unwrap().can_climb());
        assert!(!parse_manifest(&add(r#""climb": {"row": 21, "frames": 4, "fps": 8},"#)).unwrap().can_climb());
        let both = add(r#""climb": {"row": 21, "frames": 4, "fps": 8}, "slide": {"row": 22, "frames": 2, "fps": 6},"#);
        assert!(parse_manifest(&both).unwrap().can_climb());
        assert!(parse_manifest(&add(r#""slide": {"row": 22, "frames": 0, "fps": 6},"#)).is_err());
    }

    #[test]
    fn personality_and_stage_size_default_when_missing() {
        let m = parse_manifest(&full_json()).unwrap();
        assert_eq!(m.personality, Personality::Friendly);
        assert!(m.stage_size.is_empty());
        assert_eq!(m.size_mult(Stage::Baby), 1.0);
    }

    #[test]
    fn personality_and_stage_size_parse() {
        let json = full_json().replace(
            r#""walk_speed": 60,"#,
            r#""walk_speed": 60, "personality": "aloof", "stage_size": {"baby": 0.85, "adult": 1.3},"#,
        );
        let m = parse_manifest(&json).unwrap();
        assert_eq!(m.personality, Personality::Aloof);
        assert_eq!((m.size_mult(Stage::Baby), m.size_mult(Stage::Adult), m.size_mult(Stage::Child)), (0.85, 1.3, 1.0));
        assert!(parse_manifest(&full_json().replace(r#""walk_speed": 60,"#, r#""walk_speed": 60, "personality": "grumpy","#)).is_err());
    }

    #[test]
    fn rejects_non_positive_or_non_finite_stage_size() {
        for bad in ["0", "-1.2", "1e999"] {
            let json = full_json().replace(r#""walk_speed": 60,"#, &format!(r#""walk_speed": 60, "stage_size": {{"adult": {bad}}},"#));
            assert!(parse_manifest(&json).is_err(), "{bad}");
        }
    }

    #[test]
    fn bundled_manifests_carry_personality_and_sizes() {
        let load = |id: &str| {
            let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(format!("../../ui/pets/{id}/pet.json"));
            parse_manifest(&std::fs::read_to_string(path).unwrap()).unwrap()
        };
        let (cat, rudolph, rabbit) = (load("cat"), load("rudolph"), load("rabbit"));
        assert_eq!((cat.personality, rudolph.personality), (Personality::Aloof, Personality::Friendly));
        assert_eq!(rudolph.size_mult(Stage::Teenager), 0.84);
        assert_eq!(rabbit.size_mult(Stage::Baby), 0.85);
        assert_eq!(cat.size_mult(Stage::Adult), 0.91);
    }

    #[test]
    fn favorite_food_parses_and_defaults_to_none() {
        assert_eq!(parse_manifest(&full_json()).unwrap().favorite_food, None);
        let json = full_json().replace(r#""walk_speed": 60,"#, r#""walk_speed": 60, "favorite_food": {"emoji": "🐟", "name": "생선"},"#);
        let fav = parse_manifest(&json).unwrap().favorite_food.unwrap();
        assert_eq!(fav, FavoriteFood { emoji: "🐟".into(), name: "생선".into() });
    }

    #[test]
    fn bundled_manifests_all_have_a_favorite_food() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../ui/pets");
        for entry in std::fs::read_dir(dir).unwrap() {
            let path = entry.unwrap().path().join("pet.json");
            if path.is_file() {
                let m = parse_manifest(&std::fs::read_to_string(&path).unwrap()).unwrap();
                assert!(m.favorite_food.is_some_and(|f| !f.emoji.is_empty() && !f.name.is_empty()), "{path:?}");
            }
        }
    }

    #[test]
    fn locomotion_defaults_to_walk_and_parses_fly() {
        assert_eq!(parse_manifest(&full_json()).unwrap().locomotion, Locomotion::Walk);
        let json = full_json().replace(r#""walk_speed": 60,"#, r#""walk_speed": 60, "locomotion": "fly","#);
        assert_eq!(parse_manifest(&json).unwrap().locomotion, Locomotion::Fly);
        assert!(parse_manifest(&full_json().replace(r#""walk_speed": 60,"#, r#""walk_speed": 60, "locomotion": "swim","#)).is_err());
    }

    #[test]
    fn parses_hop_locomotion_bigjump_and_playful() {
        let json = full_json()
            .replace(r#""walk_speed": 60,"#, r#""walk_speed": 60, "locomotion": "hop", "personality": "playful","#)
            .replace(r#""animations": {"#, r#""animations": {"bigjump": {"row": 21, "frames": 4, "fps": 8, "loop": false},"#);
        let m = parse_manifest(&json).unwrap();
        assert_eq!(m.locomotion, Locomotion::Hop);
        assert!(m.hops() && !m.flies() && m.has_anim("bigjump"));
        assert_eq!(m.personality, Personality::Playful);
        assert_eq!(OPTIONAL_ANIMS[4], "bigjump");
        let bad = full_json().replace(r#""animations": {"#, r#""animations": {"bigjump": {"row": 21, "frames": 0, "fps": 8},"#);
        assert!(parse_manifest(&bad).is_err());
    }

    /// 기린 목 늘이기·레서판다 깜짝은 그 동물과 함께 없어짐 (v0.7.0) — 옛 pet.json에 남아 있어도 특별 동작이 아님
    #[test]
    fn the_removed_giraffe_and_red_panda_poses_are_no_longer_specials() {
        assert!(!OPTIONAL_ANIMS.contains(&"neckstretch") && !OPTIONAL_ANIMS.contains(&"surprise"));
        let json = full_json().replace(r#""animations": {"#, r#""animations": {"neckstretch": {"row": 21, "frames": 3, "fps": 2}, "surprise": {"row": 22, "frames": 3, "fps": 8},"#);
        assert_eq!(parse_manifest(&json).unwrap().special(), None);
    }

    /// 올려다보기: 선택 동작의 맨 끝 — 있으면 can_look_up, 없으면 그냥 쳐다봄
    #[test]
    fn parses_lookup_as_the_last_optional_anim() {
        assert_eq!(OPTIONAL_ANIMS[5], "lookup");
        assert!(!parse_manifest(&full_json()).unwrap().can_look_up());
        let json = full_json().replace(r#""animations": {"#, r#""animations": {"lookup": {"row": 21, "frames": 3, "fps": 3},"#);
        assert!(parse_manifest(&json).unwrap().can_look_up());
        let bad = full_json().replace(r#""animations": {"#, r#""animations": {"lookup": {"row": 21, "frames": 0, "fps": 3},"#);
        assert!(parse_manifest(&bad).is_err());
    }

    /// 번들 pet.json: 모든 동물이 올려다봄
    #[test]
    fn bundled_pets_look_up() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../ui/pets");
        for id in crate::test_util::species() {
            let json = std::fs::read_to_string(dir.join(&id).join("pet.json")).unwrap();
            assert!(parse_manifest(&json).unwrap().can_look_up(), "{id}");
        }
    }

    /// 공룡: lookup 뒤의 fire·roar·stomp → 공룡 특별 동작 (셋 중 하나를 고름)
    #[test]
    fn parses_dino_anims_as_the_dino_special() {
        assert_eq!(OPTIONAL_ANIMS[6..9], ["fire", "roar", "stomp"]);
        let json = full_json().replace(
            r#""animations": {"#,
            r#""animations": {"fire": {"row": 21, "frames": 4, "fps": 4, "loop": false}, "roar": {"row": 22, "frames": 3, "fps": 4}, "stomp": {"row": 23, "frames": 4, "fps": 6},"#,
        );
        let m = parse_manifest(&json).unwrap();
        let special = crate::behavior::Special::from_manifest(|n| m.has_anim(n)).unwrap();
        assert!(special.is_dino());
        let bad = full_json().replace(r#""animations": {"#, r#""animations": {"roar": {"row": 21, "frames": 0, "fps": 4},"#);
        assert!(parse_manifest(&bad).is_err());
    }

    /// 번들 호박은 벽을 탐. 👻 유령은 v0.7.0에 빠졌음 (설정은 호박으로 옮김)
    #[test]
    fn bundled_pumpkin_climbs_and_the_ghost_is_gone() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../ui/pets");
        let load = |id: &str| parse_manifest(&std::fs::read_to_string(dir.join(id).join("pet.json")).unwrap()).unwrap();
        assert!(load("pumpkin").can_climb());
        assert!(!dir.join("ghost").exists());
    }

    /// 🦆🐸🟣🐱 오리·개구리·보라 공룡·페르시안은 v0.7.1에 빠졌음 (설정은 노란 공룡·다람쥐·초록 공룡·고양이로 옮김 — config::RETIRED_PETS)
    #[test]
    fn the_duck_frog_purple_dino_and_persian_are_gone() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../ui/pets");
        for id in ["duck", "frog", "dino_purple", "persian"] {
            assert!(!dir.join(id).exists(), "{id}");
            assert!(crate::config::replacement_species(id).is_some_and(|to| dir.join(to).join("pet.json").exists()), "{id}");
        }
    }

    /// 번들 공룡 둘: 걷고, 벽을 타고, 공룡 동작을 모두 갖고, 좋아하는 음식이 저마다 다름
    #[test]
    fn bundled_dinos() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../ui/pets");
        for (id, food) in [("dino_yellow", "🍉"), ("dino_green", "🥦")] {
            let json = std::fs::read_to_string(dir.join(id).join("pet.json")).unwrap();
            let m = parse_manifest(&json).unwrap();
            assert!(["fire", "roar", "stomp"].iter().all(|a| m.has_anim(a)), "{id}");
            assert!(crate::behavior::Special::from_manifest(|n| m.has_anim(n)).is_some_and(|s| s.is_dino()), "{id}");
            assert!(m.can_climb() && m.locomotion == Locomotion::Walk, "{id}");
            assert!(json.contains(food), "{id}");
        }
    }

    /// 번들 산타·눈사람은 고양이처럼 벽을 탐. 눈사람만 구름을 부르고, 둘 다 구르지 않음 (썰매 · 배 미끄럼)
    #[test]
    fn bundled_santa_and_snowman_climb() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../ui/pets");
        let load = |id: &str| parse_manifest(&std::fs::read_to_string(dir.join(id).join("pet.json")).unwrap()).unwrap();
        let (santa, snowman) = (load("santa"), load("snowman"));
        assert!(santa.can_climb() && snowman.can_climb());
        assert!(snowman.snow_cloud && !santa.snow_cloud && !load("cat").snow_cloud);
        assert_eq!(snowman.hop_share(), 0.5);
        assert_eq!(snowman.animations["roll"].spin, 0.0);
        assert_eq!(snowman.favorite_food.as_ref().map(|f| f.name.as_str()), Some("빙수"));
    }

    /// 번들 펭귄만 장난칠 때 안경이 번쩍함 (brain_flash.rs)
    #[test]
    fn bundled_penguin_flashes_its_glasses() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../ui/pets");
        let load = |id: &str| parse_manifest(&std::fs::read_to_string(dir.join(id).join("pet.json")).unwrap()).unwrap();
        assert!(load("penguin").glasses_flash);
        assert!(!load("cat").glasses_flash && !load("snowman").glasses_flash);
    }

    /// 번들 흐물이만 겁쟁이(만세 줄행랑)·기운 넘침·창 옆면 매달리기 — 셋 다 그림(panic·hang·rope)이 있음
    #[test]
    fn bundled_heumul_is_timid_energetic_and_rigs_window_sides() {
        assert_eq!(&OPTIONAL_ANIMS[9..12], &["panic", "hang", "rope"]);
        assert_eq!(&OPTIONAL_ANIMS[12..19], &JUGGLE_ANIMS, "물건 들어 던지기 (v0.6.2)");
        assert_eq!(&OPTIONAL_ANIMS[19..21], &TASKBAR_ANIMS, "📌 작업표시줄 놀이 (v0.6.2)");
        assert_eq!(&OPTIONAL_ANIMS[21..30], &ANTICS_ANIMS, "흐물이 더 놀기 (v0.6.3, ⌨️ type v0.6.4)");
        assert_eq!(&OPTIONAL_ANIMS[30..31], &KNOCK_ANIMS, "🐱 툭툭 (v0.6.3)");
        assert_eq!(&OPTIONAL_ANIMS[31..34], &STASH_ANIMS, "🐿️ 볼 빵빵 (v0.6.3)");
        assert_eq!(&OPTIONAL_ANIMS[34..40], &WEB_ANIMS, "🕷️ 거미인간 (🙃 hang_down은 web_whirl 뒤)");
        assert_eq!(&OPTIONAL_ANIMS[40..45], &BIKE_ANIMS, "🦇 박쥐인간");
        assert_eq!(&OPTIONAL_ANIMS[45..49], &SOAR_ANIMS, "🦸 슈퍼인간");
        assert_eq!(&OPTIONAL_ANIMS[49..50], &BROOM_ANIMS, "🧹 빗자루");
        assert_eq!(&OPTIONAL_ANIMS[50..51], &SPELL_ANIMS, "✨ 주문");
        assert_eq!(&OPTIONAL_ANIMS[51..53], &BOOK_ANIMS, "📖🙋 책벌레");
        assert_eq!(&OPTIONAL_ANIMS[53..56], &BAG_ANIMS, "👜 가방 청소");
        assert_eq!(&OPTIONAL_ANIMS[56..64], &HAGRID_ANIMS, "🧔 해그리드");
        assert_eq!(&OPTIONAL_ANIMS[64..], &SUIT_ANIMS, "🛡️ 무쇠인간 투구 장착 — 맨 끝에 이어 붙인다");
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../ui/pets");
        let load = |id: &str| parse_manifest(&std::fs::read_to_string(dir.join(id).join("pet.json")).unwrap()).unwrap();
        let h = load("heumul");
        assert!(h.timid() && h.energetic && h.rigs() && h.juggles() && h.plays_taskbar() && h.antics());
        assert_eq!(h.name, "흐물이");
        assert_eq!(h.personality, Personality::Playful);
        assert!(h.hop_share() >= 0.5 && h.walk_speed >= 100.0);
        assert_eq!(h.favorite_food.as_ref().map(|f| f.emoji.as_str()), Some("🍭"));
        for id in crate::test_util::species().into_iter().filter(|id| id != "heumul") {
            let m = load(&id);
            assert!(!m.timid() && !m.energetic && !m.rigs() && !m.taskbar_play && !m.plays_taskbar() && !m.antics(), "{id}");
            // 💪 영웅 셋 (박쥐인간·슈퍼인간·무쇠인간)과 😜 장난꾸러기 마법사 넷도 물건을 들어 옮김 (흐물이 juggle 틀) — 다른 동물은 안 듦
            let lifts = JUGGLER_PRANKSTERS.contains(&id.as_str());
            assert_eq!((m.juggler, m.juggles()), (lifts, lifts), "{id}");
        }
    }

    /// 🕷️ 거미인간만 줄을 씀 (web_slinger + WEB_ANIMS 그림 여섯). 줄 끝 손 자리(grip)는 shoot·web_swing·web_pull·web_whirl·hang_down
    #[test]
    fn only_the_bundled_spider_slings_webs_and_has_hand_spots_for_the_line() {
        assert_eq!(WEB_ANIMS, ["shoot", "web_swing", "hero_land", "web_pull", "web_whirl", "hang_down"]);
        let ui = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../ui");
        let read = |root: &str, id: &str| std::fs::read_to_string(ui.join(root).join(id).join("pet.json")).unwrap();
        {
            for id in crate::test_util::species() {
                let m = parse_manifest(&read("pets", &id)).unwrap();
                assert_eq!(m.slings_webs(), id == "spider", "{id}");
                assert_eq!(crate::brain::PetConfig::from_manifest(&m, 1.0, 1.0, "teenager").slings_webs, id == "spider", "{id}");
            }
        }
        let s = parse_manifest(&read("pets", "spider")).unwrap();
        assert_eq!(s.name, "거미인간");
        assert!(s.can_climb() && !s.juggles() && !s.antics() && !s.tint);
        for stage in ["baby", "teenager", "adult"] {
            let g = s.grip_table(stage);
            assert_eq!((g.shoot.n, g.web_swing.n, g.web_pull.n, g.web_whirl.n), (3, 2, 4, 3), "{stage}");
            assert!(g.shoot.at(1.0).0 > 0.03, "쏜 손은 몸 앞 (꼬마 영웅의 짧은 팔) {stage}");
            assert!(g.web_swing.at(0.0).1 < 0.2, "그네 손은 머리 위 {stage}");
            assert!((0..3).all(|i| g.web_whirl.pts[i].1 < 0.4), "🌀 휘두르는 손은 머리 위 {stage}");
            assert!(g.hang_down.n == 3 && (0..3).all(|i| g.hang_down.pts[i].1 < 0.35), "🙃 줄 잡은 손발은 그림 위쪽 {stage}");
        }
        // 그림이 하나라도 빠지면 줄을 쓰지 않음
        let partial = read("pets", "spider").replace(r#""web_pull""#, r#""web_pull_x""#);
        assert!(!parse_manifest(&partial).unwrap().slings_webs());
        let no_whirl = read("pets", "spider").replace(r#""web_whirl""#, r#""web_whirl_x""#);
        assert!(!parse_manifest(&no_whirl).unwrap().slings_webs());
        let no_hang = read("pets", "spider").replace(r#""hang_down""#, r#""hang_down_x""#);
        assert!(!parse_manifest(&no_hang).unwrap().slings_webs());
    }

    /// 🦇 박쥐인간만 오토바이를 탐 (bike_rider + BIKE_ANIMS 그림 다섯). 물건 들기(juggle)도 하고 손 자리(grip)는 머리 위
    #[test]
    fn only_the_bundled_bat_rides_a_bike_and_lifts_things_overhead() {
        let ui = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../ui");
        let read = |root: &str, id: &str| std::fs::read_to_string(ui.join(root).join(id).join("pet.json")).unwrap();
        {
            for id in crate::test_util::species() {
                let m = parse_manifest(&read("pets", &id)).unwrap();
                assert_eq!(m.rides_bike(), id == "bat", "{id}");
                assert_eq!(crate::brain::PetConfig::from_manifest(&m, 1.0, 1.0, "teenager").rides_bike, id == "bat", "{id}");
            }
        }
        let b = parse_manifest(&read("pets", "bat")).unwrap();
        assert_eq!(b.name, "박쥐인간");
        assert!(b.can_climb() && b.juggles() && !b.antics() && !b.plays_taskbar() && !b.slings_webs() && !b.tint);
        for stage in ["baby", "teenager", "adult"] {
            let g = b.grip_table(stage);
            assert_eq!((g.lift.n, g.hold.n, g.carry.n, g.throw.n, g.present.n), (4, 2, 4, 4, 4), "{stage}");
            assert!(g.hold.at(0.0).1 < 0.35 && g.carry.at(0.0).1 < 0.35, "머리 위에 듦 {stage}");
        }
        let partial = read("pets", "bat").replace(r#""wheelie""#, r#""wheelie_x""#);
        assert!(!parse_manifest(&partial).unwrap().rides_bike());
    }

    /// 🦸 슈퍼인간·🛡️ 무쇠인간만 낮게 낢 (super_flier + SOAR_ANIMS 그림 넷 — locomotion은 걷기 그대로). 슈퍼인간은 물건 들기(juggle)도 하고 손 자리(grip)는 머리 위
    #[test]
    fn only_the_bundled_super_soars_and_lifts_things_overhead() {
        let ui = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../ui");
        let read = |id: &str| std::fs::read_to_string(ui.join("pets").join(id).join("pet.json")).unwrap();
        for id in crate::test_util::species() {
            let m = parse_manifest(&read(&id)).unwrap();
            let soars = id == "super" || id == "musoe";
            assert_eq!(m.soars(), soars, "{id}");
            assert_eq!(crate::brain::PetConfig::from_manifest(&m, 1.0, 1.0, "teenager").soars, soars, "{id}");
        }
        let s = parse_manifest(&read("super")).unwrap();
        assert_eq!(s.name, "슈퍼인간");
        assert_eq!(s.locomotion, Locomotion::Walk);
        assert!(s.can_climb() && s.juggles() && !s.antics() && !s.plays_taskbar() && !s.slings_webs() && !s.rides_bike() && !s.tint);
        for stage in ["baby", "teenager", "adult"] {
            let g = s.grip_table(stage);
            assert_eq!((g.lift.n, g.hold.n, g.carry.n, g.throw.n, g.present.n), (4, 2, 4, 4, 4), "{stage}");
            assert!(g.hold.at(0.0).1 < 0.35 && g.carry.at(0.0).1 < 0.35, "머리 위에 듦 {stage}");
        }
        let partial = read("super").replace(r#""fly_flip""#, r#""fly_flip_x""#);
        assert!(!parse_manifest(&partial).unwrap().soars());
    }

    /// 💪😜 흐물이 들고 던지기를 하는 다른 동물 (pet.json juggler — 시험용 목록)
    const JUGGLER_PRANKSTERS: [&str; 7] = ["bat", "super", "musoe", "harry", "hermione", "voldemort", "hagrid"];

    /// 😜 흐물이 장난 (🙆 다른 펫 번쩍 들기 · 🫣 창 뒤 숨바꼭질)을 pet.json `pranks` 목록으로 나눠 줌: 영웅 셋 + 마법사 넷.
    /// 흐물이는 예전처럼 antics로 다 함. 그림이 빠지면 (들기는 juggle 그림, 숨기는 peek) 하지 않음
    #[test]
    fn pranks_list_gives_heumul_pranks_to_the_heroes_and_the_wizards() {
        let ui = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../ui");
        let read = |id: &str| std::fs::read_to_string(ui.join("pets").join(id).join("pet.json")).unwrap();
        for id in crate::test_util::species() {
            let m = parse_manifest(&read(&id)).unwrap();
            let prankster = id == "heumul" || JUGGLER_PRANKSTERS.contains(&id.as_str());
            assert_eq!((m.hoists(), m.hides()), (prankster, prankster), "{id}");
            let c = crate::brain::PetConfig::from_manifest(&m, 1.0, 1.0, "teenager");
            assert_eq!((c.hoists, c.hides), (prankster, prankster), "{id}");
            if id != "heumul" {
                assert_eq!(m.pranks, if prankster { vec![Prank::Hoist, Prank::Hide] } else { vec![] }, "{id}");
                assert!(!m.antics() && !m.plays_taskbar(), "{id}: 커서 매달리기·춤·작업표시줄은 흐물이만");
            }
        }
        let bat = read("bat");
        assert!(!parse_manifest(&bat.replace(r#""peek""#, r#""peek_x""#)).unwrap().hides());
        assert!(!parse_manifest(&bat.replace(r#""lift""#, r#""lift_x""#)).unwrap().hoists());
        assert!(parse_manifest(r#"{"name":"x","frame_size":128,"walk_speed":60,"animations":{},"pranks":["boo"]}"#).is_err());
    }

    /// 🛡️ 무쇠인간: 제트팩으로 낮게 날고 (슈퍼인간과 같은 soars) 🪖 투구 장착 특별 동작 (suit_up 그림 — Special::SuitUp).
    /// 💪 영웅이라 물건·거대한 돌·똥도 머리 위로 번쩍 (juggle — 손 자리는 머리 위), 🕷️ 거미인간을 안아 줄 hug 그림
    #[test]
    fn the_bundled_musoe_soars_suits_up_and_lifts_things_overhead() {
        let ui = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../ui");
        let read = |id: &str| std::fs::read_to_string(ui.join("pets").join(id).join("pet.json")).unwrap();
        let m = parse_manifest(&read("musoe")).unwrap();
        assert_eq!(m.name, "무쇠인간");
        assert_eq!(m.locomotion, Locomotion::Walk);
        assert!(m.soars() && m.can_climb() && m.juggles() && !m.slings_webs() && !m.rides_bike() && !m.rides_broom() && !m.tint);
        assert!(m.has_anim("hug") && !m.is_gamekeeper());
        for stage in ["baby", "teenager", "adult"] {
            let g = m.grip_table(stage);
            assert_eq!((g.lift.n, g.hold.n, g.carry.n, g.throw.n, g.present.n), (4, 2, 4, 4, 4), "{stage}");
            assert!(g.hold.at(0.0).1 < 0.35 && g.carry.at(0.0).1 < 0.35, "머리 위에 듦 {stage}");
        }
        assert_eq!(m.special(), Some(crate::behavior::Special::SuitUp));
        for id in crate::test_util::species().into_iter().filter(|id| id != "musoe") {
            assert_ne!(parse_manifest(&read(&id)).unwrap().special(), Some(crate::behavior::Special::SuitUp), "{id}");
        }
        assert_eq!(parse_manifest(&read("musoe").replace(r#""suit_up""#, r#""suit_up_x""#)).unwrap().special(), None);
    }

    /// 🧙 해리포터·헤르미온느만 빗자루를 타고 주문을 씀: 해리는 expelliarmus·leviosa + 🟡 스니치, 헤르미온느는 leviosa·reparo + 📖 책 + 👜 가방
    #[test]
    fn only_the_bundled_wizards_ride_brooms_and_cast_spells() {
        use crate::wizard::Spell;
        let ui = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../ui");
        let read = |id: &str| std::fs::read_to_string(ui.join("pets").join(id).join("pet.json")).unwrap();
        for id in crate::test_util::species() {
            let m = parse_manifest(&read(&id)).unwrap();
            let wizard = id == "harry" || id == "hermione" || id == "voldemort";
            assert_eq!(m.rides_broom(), wizard || id == "hagrid", "{id}");
            assert_eq!(!m.spell_set().is_empty(), wizard, "{id}");
            assert_eq!((m.dragon_mount(), m.is_gamekeeper()), (id == "hagrid", id == "hagrid"), "{id}");
            assert_eq!((m.is_dark_lord(), m.is_dementor(), m.casts_patronus()), (id == "voldemort", id == "dementor", id == "harry" || id == "hermione"), "{id}");
            assert_eq!(m.seeks(), id == "harry", "{id}");
            assert_eq!(m.uses_portkeys(), wizard, "🧳 {id}");
            assert_eq!((m.reads_books(), m.cleans_with_bag()), (id == "hermione", id == "hermione"), "{id}");
            let c = crate::brain::PetConfig::from_manifest(&m, 1.0, 1.0, "teenager");
            assert_eq!((c.rides_broom, c.seeker, c.bookworm, c.bag_cleaner), (wizard || id == "hagrid", id == "harry", id == "hermione", id == "hermione"), "{id}");
            assert_eq!((c.dragon, c.gamekeeper), (id == "hagrid", id == "hagrid"), "{id}");
        }
        let h = parse_manifest(&read("harry")).unwrap();
        assert_eq!(h.name, "해리포터");
        assert!(h.spell_set().has(Spell::Expelliarmus) && h.spell_set().has(Spell::Leviosa) && !h.spell_set().has(Spell::Reparo));
        assert!(h.can_climb() && h.juggles() && !h.soars() && !h.rides_bike() && !h.slings_webs());
        let g = parse_manifest(&read("hermione")).unwrap();
        assert_eq!(g.name, "헤르미온느");
        assert!(g.spell_set().has(Spell::Leviosa) && g.spell_set().has(Spell::Reparo) && !g.spell_set().has(Spell::Expelliarmus));
        assert!(g.can_climb() && g.juggles());
        // 그림 하나라도 없으면 그 특기는 꺼짐
        assert!(!parse_manifest(&read("harry").replace(r#""broom""#, r#""broom_x""#)).unwrap().rides_broom());
        assert!(parse_manifest(&read("harry").replace(r#""cast""#, r#""cast_x""#)).unwrap().spell_set().is_empty());
        assert!(!parse_manifest(&read("hermione").replace(r#""bag_drag""#, r#""bag_drag_x""#)).unwrap().cleans_with_bag());
        // 🐍 볼드모트: Stupefy만 + 나기니·hehehe, 👻 디멘터: 나는 동물 (날지 않으면 디멘터가 아님)
        let v = parse_manifest(&read("voldemort")).unwrap();
        assert_eq!(v.name, "볼드모트");
        assert!(v.spell_set().has(Spell::Stupefy) && !v.spell_set().has(Spell::Expelliarmus) && v.can_climb());
        let c = crate::brain::PetConfig::from_manifest(&v, 1.0, 1.0, "teenager");
        assert!(c.dark_lord && !c.dementor && c.patronus.is_none() && c.grip.cast.n == 3);
        let d = parse_manifest(&read("dementor")).unwrap();
        assert!(d.name == "디멘터" && d.flies() && d.is_dementor() && !d.can_climb() && d.spell_set().is_empty());
        assert!(!parse_manifest(&read("dementor").replace(r#""locomotion": "fly","#, "")).unwrap().is_dementor());
        assert!(!parse_manifest(&read("voldemort").replace(r#""cast""#, r#""cast_x""#)).unwrap().is_dark_lord());
        assert!(crate::brain::PetConfig::from_manifest(&parse_manifest(&read("harry")).unwrap(), 1.0, 1.0, "teenager").patronus.is_some());
    }

    /// 🦌🦦 패트로누스 동물 (pet.json `patronus`): 해리 "stag" 사슴 · 헤르미온느 "otter" 수달 · 옛 true는 사슴 · false·모르는 값·cast 그림 없음은 없음
    #[test]
    fn patronus_animals_come_from_pet_json() {
        use crate::wizard::PatronusAnimal;
        let ui = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../ui");
        let read = |id: &str| std::fs::read_to_string(ui.join("pets").join(id).join("pet.json")).unwrap();
        let animal = |text: &str| crate::brain::PetConfig::from_manifest(&parse_manifest(text).unwrap(), 1.0, 1.0, "teenager").patronus;
        let harry = read("harry");
        assert!(harry.contains(r#""patronus": "stag""#));
        assert_eq!(animal(&harry), Some(PatronusAnimal::Stag));
        assert_eq!(animal(&read("hermione")), Some(PatronusAnimal::Otter));
        assert_eq!(animal(&read("voldemort")), None);
        assert_eq!(animal(&harry.replace(r#""patronus": "stag""#, r#""patronus": true"#)), Some(PatronusAnimal::Stag));
        assert_eq!(animal(&harry.replace(r#""patronus": "stag""#, r#""patronus": "otter""#)), Some(PatronusAnimal::Otter));
        assert_eq!(animal(&harry.replace(r#""patronus": "stag""#, r#""patronus": false"#)), None);
        assert_eq!(animal(&harry.replace(r#""patronus": "stag""#, r#""patronus": "phoenix""#)), None);
        assert_eq!(animal(&harry.replace(r#""cast""#, r#""cast_x""#)), None);
        // 이상한 값은 그 특기만 없고 pet.json은 그대로 읽힘
        for odd in ["null", "3", "[1]", r#"{"a":1}"#] {
            assert_eq!(animal(&harry.replace(r#""patronus": "stag""#, &format!(r#""patronus": {odd}"#))), None, "{odd}");
        }
    }

    /// 🧔 해그리드: 빗자루 대신 🐉 용 (broom_rider + dragon 그림), 사이드카·안기·알·락 케이크 (gamekeeper + HAGRID_ANIMS), 덩치 1.4배, 손 자리
    #[test]
    fn the_bundled_hagrid_rides_a_dragon_and_keeps_the_grounds() {
        let ui = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../ui");
        let read = |id: &str| std::fs::read_to_string(ui.join("pets").join(id).join("pet.json")).unwrap();
        let h = parse_manifest(&read("hagrid")).unwrap();
        assert_eq!(h.name, "해그리드");
        assert!(h.rides_broom() && h.dragon_mount() && h.is_gamekeeper() && h.can_climb() && h.spell_set().is_empty() && h.juggles() && !h.rides_bike());
        let harry = parse_manifest(&read("harry")).unwrap();
        for st in [Stage::Baby, Stage::Teenager, Stage::Adult] {
            let k = h.size_mult(st) / harry.size_mult(st);
            assert!((1.3..=1.5).contains(&k), "덩치 약 1.4배: {k}");
        }
        let g = h.grip_table("teenager");
        assert_eq!((g.hug.n, g.hug_walk.n, g.sidecar.n, g.sidecar_fly.n, g.egg.n), (2, 4, 3, 2, 4));
        assert!(g.sidecar.at(0.0).0 > 0.15, "사이드카는 앞쪽: {:?}", g.sidecar.at(0.0));
        assert!(!parse_manifest(&read("hagrid").replace(r#""egg""#, r#""egg_x""#)).unwrap().is_gamekeeper());
        assert!(!parse_manifest(&read("hagrid").replace(r#""dragon""#, r#""dragon_x""#)).unwrap().rides_broom());
    }

    /// 🐱 툭툭은 고양이, 🐿️ 볼 빵빵은 다람쥐만 (v0.6.3)
    #[test]
    fn bundled_cats_knock_and_the_squirrel_stashes() {
        let ui = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../ui");
        let read = |root: &str, id: &str| std::fs::read_to_string(ui.join(root).join(id).join("pet.json")).unwrap();
        {
            for id in crate::test_util::species() {
                let m = parse_manifest(&read("pets", &id)).unwrap();
                assert_eq!(m.knocks(), id == "cat", "{id}");
                assert_eq!(m.stashes(), id == "squirrel", "{id}");
            }
        }
    }

    /// 🎨 몸 색은 잉크 선으로 그린 흐물이만 (pet.json tint + ink_<stage>.png) — 메뉴 "🎨 흐물이 색"도 그때만 보임
    #[test]
    fn only_bundled_heumul_can_be_tinted() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../ui/pets");
        let load = |id: &str| parse_manifest(&std::fs::read_to_string(dir.join(id).join("pet.json")).unwrap()).unwrap();
        assert!(load("heumul").tint);
        for id in crate::test_util::species().into_iter().filter(|id| id != "heumul") {
            assert!(!load(&id).tint, "{id}");
        }
    }

    /// 번들 다람쥐는 고양이처럼 벽을 타고, 걷다가 가끔 깡총거리고, 도토리를 좋아함
    #[test]
    fn bundled_squirrel_climbs_and_hops_now_and_then() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../ui/pets/squirrel/pet.json");
        let m = parse_manifest(&std::fs::read_to_string(path).unwrap()).unwrap();
        assert!(m.can_climb() && m.locomotion == Locomotion::Walk);
        assert!(m.hop_share() > 0.0 && m.hop_share() < 1.0);
        assert_eq!(m.favorite_food.as_ref().map(|f| f.name.as_str()), Some("도토리"));
    }

    #[test]
    fn hop_share_defaults_to_zero_and_is_bounded() {
        assert_eq!(parse_manifest(&full_json()).unwrap().hop_share(), 0.0);
        let with = |v: &str| full_json().replace(r#""walk_speed": 60,"#, &format!(r#""walk_speed": 60, "hop_share": {v},"#));
        assert_eq!(parse_manifest(&with("0.5")).unwrap().hop_share(), 0.5);
        assert!(parse_manifest(&with("1.5")).is_err() && parse_manifest(&with("-0.1")).is_err());
        // 폴짝 동물은 늘 폴짝이라 몫을 쓰지 않는다
        let frog = with("0.5").replace(r#""walk_speed": 60,"#, r#""walk_speed": 60, "locomotion": "hop","#);
        assert_eq!(parse_manifest(&frog).unwrap().hop_share(), 0.0);
    }

    /// 토끼는 걷는 시간의 절반쯤 깡총거림
    #[test]
    fn bundled_rabbit_hops_half_the_time() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../ui/pets/rabbit/pet.json");
        let m = parse_manifest(&std::fs::read_to_string(path).unwrap()).unwrap();
        assert_eq!(m.locomotion, Locomotion::Walk);
        assert_eq!(m.hop_share(), 0.5);
        assert!(!m.hops() && !m.has_anim("bigjump"));
    }

    /// 곰·꿀벌·강아지는 v0.5.5에서 빠졌음 — 번들에 없고, 그들만 쓰던 동작(wag 살랑·sting 쏘기)도 선택 동작이 아님
    #[test]
    fn bear_bee_and_dog_are_no_longer_bundled() {
        let pets = crate::test_util::species();
        for gone in ["bear", "bee", "dog"] {
            assert!(!pets.iter().any(|p| p == gone), "{gone}");
        }
        assert!(!OPTIONAL_ANIMS.contains(&"wag") && !OPTIONAL_ANIMS.contains(&"sting"));
    }

    /// 할로윈 호박: 걷는 동물인데 걷기·달리기는 늘 통통 (hop_share 1.0), bigjump 그림 없음 (벽 타기는 번들 pet.json의 climb·slide로 — bundled_pumpkin_climbs_and_the_ghost_is_gone)
    fn check_pumpkin(m: &PetManifest) {
        assert_eq!(m.locomotion, Locomotion::Walk);
        assert_eq!(m.hop_share(), 1.0);
        assert!(!m.flies() && !m.hops() && !m.has_anim("bigjump"));
    }

    #[test]
    fn halloween_manifest_fields_parse() {
        let pumpkin = full_json().replace(r#""walk_speed": 60,"#, r#""walk_speed": 60, "hop_share": 1.0, "personality": "playful", "stage_size": {"adult": 0.9},"#);
        let m = parse_manifest(&pumpkin).unwrap();
        check_pumpkin(&m);
        assert_eq!((m.personality, m.size_mult(Stage::Adult)), (Personality::Playful, 0.9));
        // 나는 동물 (유령은 빠졌지만 날기는 남음 — 슈퍼인간용): locomotion fly
        let flyer = full_json().replace(r#""walk_speed": 60,"#, r#""walk_speed": 40, "locomotion": "fly", "stage_size": {"adult": 0.85},"#);
        let m = parse_manifest(&flyer).unwrap();
        assert!(m.locomotion == Locomotion::Fly && m.flies());
        assert_eq!(m.size_mult(Stage::Adult), 0.85);
    }

    /// 번들 호박 (ui/pets/pumpkin — 없으면 건너뜀)
    #[test]
    fn bundled_halloween_pets_if_present() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../ui/pets");
        if let Ok(json) = std::fs::read_to_string(dir.join("pumpkin/pet.json")) {
            let m = parse_manifest(&json).unwrap();
            check_pumpkin(&m);
            assert!(m.favorite_food.is_some());
        }
    }

    #[test]
    fn house_kind_defaults_to_normal_and_parses_field() {
        assert_eq!(parse_manifest(&full_json()).unwrap().house_kind(), HouseKind::House);
        let with = |v: &str| full_json().replace(r#""walk_speed": 60,"#, &format!(r#""walk_speed": 60, "house": "{v}","#));
        // 벌통은 꿀벌과 함께 빠졌다 (v0.5.5)
        assert!(parse_manifest(&with("hive")).is_err());
        assert_eq!(parse_manifest(&with("haunted")).unwrap().house_kind(), HouseKind::Haunted);
        assert_eq!(parse_manifest(&with("house")).unwrap().house_kind(), HouseKind::House);
        assert!(parse_manifest(&with("palace")).is_err());
        // 루돌프·산타의 크리스마스 벽돌집 · 해리·헤르미온느·해그리드의 호그와트 성 · 볼드모트·디멘터의 쓰러져 가는 나무집
        assert_eq!(parse_manifest(&with("xmas")).unwrap().house_kind(), HouseKind::Xmas);
        assert_eq!(parse_manifest(&with("castle")).unwrap().house_kind(), HouseKind::Castle);
        assert_eq!(parse_manifest(&with("shack")).unwrap().house_kind(), HouseKind::Shack);
    }

    /// 번들 pet.json: 호박은 귀신의 집, 루돌프·산타는 크리스마스 벽돌집, 해리·헤르미온느·해그리드는 호그와트 성, 볼드모트·디멘터는 나무집, 나머지는 보통 집
    #[test]
    fn bundled_pets_pick_their_house() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../ui/pets");
        for (id, kind) in [("pumpkin", HouseKind::Haunted), ("cat", HouseKind::House),
                           ("rudolph", HouseKind::Xmas), ("santa", HouseKind::Xmas), ("snowman", HouseKind::Xmas),
                           ("harry", HouseKind::Castle), ("hermione", HouseKind::Castle), ("hagrid", HouseKind::Castle),
                           ("voldemort", HouseKind::Shack), ("dementor", HouseKind::Shack)] {
            let json = std::fs::read_to_string(dir.join(id).join("pet.json")).unwrap_or_else(|e| panic!("{id}: {e}"));
            assert_eq!(parse_manifest(&json).unwrap().house_kind(), kind, "{id}");
        }
    }

    #[test]
    fn round_manifest_frame_size_is_the_same_on_every_stage() {
        let m = parse_manifest(&full_json()).unwrap();
        assert!(m.frame_sizes.is_empty());
        assert_eq!((m.frame_size_at("baby"), m.frame_size_at("adult")), (64, 64));
        assert!(parse_manifest(&full_json().replace(r#""roll""#, r#""rolly""#)).unwrap_err().contains("roll"), "round still needs its roll");
    }

    #[test]
    fn stage_frame_sizes_parse_and_fall_back_to_frame_size() {
        let json = full_json().replace(r#""walk_speed": 60,"#, r#""walk_speed": 60, "frame_sizes": {"baby": 24, "adult": 36},"#);
        let m = parse_manifest(&json).unwrap();
        assert_eq!((m.frame_size_at("baby"), m.frame_size_at("adult"), m.frame_size_at("child")), (24, 36, 64));
        let bad = full_json().replace(r#""walk_speed": 60,"#, r#""walk_speed": 60, "frame_sizes": {"baby": 0},"#);
        assert!(parse_manifest(&bad).is_err());
    }

    #[test]
    fn hang_grip_measures_in_the_stages_own_dots() {
        let json = full_json().replace(
            r#""walk_speed": 60,"#,
            r#""walk_speed": 60, "frame_sizes": {"baby": 24, "adult": 36}, "head": {"baby": {"dragged": [[15, 6, 8, 0]]}, "adult": {"dragged": [[18, 9, 8, 0]]}},"#,
        );
        let m = parse_manifest(&json).unwrap();
        let (x, y) = m.hang_grip("baby");
        assert!((x - (15.0 / 24.0 - 0.5)).abs() < 1e-9 && (y - (6.0 - 1.2) / 24.0).abs() < 1e-9, "{x} {y}");
        let (x, _) = m.hang_grip("adult");
        assert!(x.abs() < 1e-9, "{x}");
    }

    #[test]
    fn rejects_invalid_json() {
        assert!(parse_manifest("{").is_err());
    }

    #[test]
    fn bundled_placeholder_manifests_are_valid() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../ui/pets");
        let mut count = 0;
        for entry in std::fs::read_dir(dir).unwrap() {
            let path = entry.unwrap().path().join("pet.json");
            if path.is_file() {
                parse_manifest(&std::fs::read_to_string(&path).unwrap()).unwrap_or_else(|e| panic!("{path:?}: {e}"));
                count += 1;
            }
        }
        assert!(count >= 4);
    }
}
