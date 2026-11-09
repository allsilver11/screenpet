use std::collections::HashMap;

use serde::{Deserialize, Serialize};

pub use crate::behavior::Personality;
use crate::care::Stage;
use crate::style;

pub const REQUIRED_ANIMS: [&str; 21] = [
    "walk", "run", "roll", "idle", "look", "sleep", "hop", "fall", "dragged", "land", "sit_front", "wave", "tilt", "belly", "stretch", "heart",
    "tickle", "annoyed", "hungry", "eat", "level_up",
];

/// 특정 동물만 가진 동작 — 있으면 검증한다 (behavior::Special::anim_name, 벽 타기 climb/slide, 개구리 bigjump,
/// 기린 neckstretch — 있으면 구르는 대신 목을 늘인다, 올려다보기 lookup — 없으면 그냥 쳐다본다(look)).
/// 개구리의 폴짝(hop)은 모든 동물의 필수 동작이라 REQUIRED_ANIMS에 이미 있다. 순서는 tools/sprite_spec.py OPTIONAL_ANIMS와 같다
/// 공룡 fire·roar·stomp — 있으면 쉬거나 간질일 때 셋 중 하나 (Special::Fire/Roar/Stomp, 쿵쾅은 친구들을 흔든다).
pub const OPTIONAL_ANIMS: [&str; 11] = ["arch", "huff", "climb", "slide", "bigjump", "neckstretch", "lookup", "fire", "roar", "stomp", "surprise"];

/// 이동 방식 (pet.json `locomotion`, 없으면 walk)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Locomotion {
    /// 디딤판 위를 걷는다
    #[default]
    Walk,
    /// 날아다닌다 (유령·새) — 쉴 때 가끔 창 윗면·바닥에 내려앉는다
    Fly,
    /// 폴짝폴짝 뛰어다닌다 (개구리) — 가끔 크게 뛰어 창 위로, 벽은 폴짝폴짝 오른다 (hop.rs)
    Hop,
}

/// 사는 집 그림 (pet.json `house`, 없으면 보통 집)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HouseKind {
    /// 보통 집 (ui/house/house_<stage>)
    House,
    /// 호박·유령의 귀신의 집 (ui/house/haunted_<stage>)
    Haunted,
    /// 루돌프·산타의 눈 덮인 크리스마스 벽돌집 (ui/house/xmas_<stage>, 굴뚝 연기는 ui/house.js가 피운다)
    Xmas,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AnimSpec {
    pub row: u32,
    pub frames: u32,
    pub fps: f64,
    #[serde(rename = "loop", default = "yes")]
    pub looping: bool,
    /// 앞모습 동작 — 웹뷰는 좌우 반전하지 않는다
    #[serde(default)]
    pub front: bool,
    /// 초당 회전 각도 (roll 전용, 0이면 회전 없음)
    #[serde(default)]
    pub spin: f64,
}

fn yes() -> bool {
    true
}

/// 동물별 좋아하는 음식 (음식 고르기 버튼·트레이 메뉴에 쓴다)
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FavoriteFood {
    pub emoji: String,
    pub name: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct PetManifest {
    pub name: String,
    pub frame_size: u32,
    /// 단계마다 칸 크기가 다른 시트 (단계 이름 → 칸 px, 없는 단계는 frame_size) — 픽셀 그림은 24·28·32·32·36
    #[serde(default)]
    pub frame_sizes: HashMap<String, u32>,
    pub walk_speed: f64,
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
    /// 걷는 동물이 걷기·달리기 토막 중 깡총거리는 몫 0~1 (토끼 0.5, 없으면 0 — 개구리는 locomotion hop이라 늘 폴짝)
    #[serde(default)]
    pub hop_share: f64,
    /// 사는 집 (없으면 보통 집)
    #[serde(default)]
    pub house: Option<HouseKind>,
    /// 가끔 머리 위에 작은 눈구름을 부른다 (눈사람)
    #[serde(default)]
    pub snow_cloud: bool,
    /// 빠른 나는 동물 (새) — locomotion fly일 때만 뜻이 있다. 모르는 칸이라 옛 앱은 그냥 무시한다
    #[serde(default)]
    pub swift: bool,
    /// 칸마다 머리 꼭대기 (단계 → 동작 → 칸별 [x, y, 너비, 기울기] 칸 px — 그림 생성기가 적는다)
    #[serde(default)]
    pub head: HashMap<String, HashMap<String, Vec<[f64; 4]>>>,
    /// 기린 목 늘이기 층 (단계 → {cut, tile, ext} — 그리는 건 ui/main.js). 코어는 있는지만 본다
    #[serde(default)]
    pub neck_stretch: Option<serde_json::Value>,
}

/// head 표가 없는 동물의 매달리는 자리: 그림 가운데, 윗변에서 조금 아래
const DEFAULT_HANG_GRIP: (f64, f64) = (0.0, 0.06);
/// 머리 꼭대기는 모자 입구보다 입구 너비의 이만큼 위
const HEAD_TOP_ABOVE_BRIM: f64 = 0.15;

impl PetManifest {
    /// 커서에 매달린 그림(dragged 칸들의 평균)에서 커서 끝이 닿을 머리 꼭대기: (그림 가운데에서 오른쪽, 그림 윗변에서 아래) — 한 변에 대한 비율.
    /// 표의 y는 모자 입구라 머리 위쪽으로 조금 올린다. ui/main.js hangOrigin과 같은 계산
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

/// 픽셀 그림의 pet.json (ui/pets_pixel/<id>/pet.json): 그림에 관한 것만 읽는다
#[derive(Deserialize)]
struct PixelJson {
    #[serde(default)]
    frame_sizes: HashMap<String, u32>,
    animations: HashMap<String, AnimSpec>,
    #[serde(default)]
    head: HashMap<String, HashMap<String, Vec<[f64; 4]>>>,
    #[serde(default)]
    neck_stretch: Option<serde_json::Value>,
}

/// required는 있어야 하고, 있는 동작(required·optional)은 칸 수와 속도가 0보다 커야 한다
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

/// 픽셀 그림의 pet.json을 동글 동물(base) 위에 얹는다: 시트 칸·동작 표·머리 표만 그림 것이고 이름·속도·성격·크기·먹이·집은 모두 base 그대로 (스타일은 겉모습뿐).
/// 칸 크기는 frame_sizes, 없는 단계는 기본 표(style::pixel_grid). roll은 없어도 된다 (없으면 no_roll)
pub fn parse_pixel_manifest(json: &str, base: &PetManifest) -> Result<PetManifest, String> {
    let p: PixelJson = serde_json::from_str(json).map_err(|e| e.to_string())?;
    let mut frame_sizes = HashMap::new();
    for stage in Stage::ALL {
        let name = stage.name();
        let size = p.frame_sizes.get(name).copied().unwrap_or(style::pixel_grid(stage).0);
        if size == 0 {
            return Err(format!("frame size of {name} must be > 0"));
        }
        frame_sizes.insert(name.to_string(), size);
    }
    let required: Vec<&str> = REQUIRED_ANIMS.into_iter().filter(|a| *a != "roll").collect();
    check_anims(&p.animations, &required, &["roll"])?;
    check_anims(&p.animations, &[], &OPTIONAL_ANIMS)?;
    Ok(PetManifest { frame_sizes, animations: p.animations, head: p.head, neck_stretch: p.neck_stretch, ..base.clone() })
}

impl PetManifest {
    pub fn has_anim(&self, name: &str) -> bool {
        self.animations.contains_key(name)
    }

    /// 이 단계 시트의 칸 크기 (px)
    pub fn frame_size_at(&self, stage: &str) -> u32 {
        self.frame_sizes.get(stage).copied().unwrap_or(self.frame_size)
    }

    /// 구르기 그림이 없다 (픽셀 그림) — 구르기를 고르지 않는다
    pub fn no_roll(&self) -> bool {
        !self.has_anim("roll")
    }

    /// 이 동물의 특별 동작. 구르기가 없는 그림에서 목 늘이기는 층 그림(neck_stretch)이 있을 때만 — 없으면 기린도 그냥 안 한다
    pub fn special(&self) -> Option<crate::behavior::Special> {
        let special = crate::behavior::Special::from_manifest(|n| self.has_anim(n));
        special.filter(|s| !(s.replaces_roll() && self.no_roll() && self.neck_stretch.is_none()))
    }

    /// 벽 타기 (뒷모습 climb + slide 둘 다 있어야 한다)
    pub fn can_climb(&self) -> bool {
        self.has_anim("climb") && self.has_anim("slide")
    }

    pub fn flies(&self) -> bool {
        self.locomotion == Locomotion::Fly
    }

    pub fn hops(&self) -> bool {
        self.locomotion == Locomotion::Hop
    }

    /// 걷기·달리기 토막 중 깡총거리는 몫 (폴짝 동물·나는 동물은 0 — 개구리는 늘 폴짝, 유령·새는 난다)
    pub fn hop_share(&self) -> f64 {
        if self.locomotion == Locomotion::Walk { self.hop_share } else { 0.0 }
    }

    /// 위를 올려다본다 (pet.json에 lookup이 있으면 — 없으면 그냥 쳐다본다)
    pub fn can_look_up(&self) -> bool {
        self.has_anim("lookup")
    }

    /// 사는 집 그림: pet.json `house`, 없으면 보통 집
    pub fn house_kind(&self) -> HouseKind {
        self.house.unwrap_or(HouseKind::House)
    }

    /// 이 동물의 단계별 크기 배율 (care::Stage::scale에 곱한다)
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

    /// 2배 칸 단계 (frame_sizes): 표도 칸 px로 2배라 비율로 재는 값은 128 칸일 때와 같다 — 표시 크기는 칸과 상관없다
    #[test]
    fn a_doubled_stage_sheet_keeps_the_same_hang_grip() {
        let json = r#"{"name":"t","frame_size":128,"frame_sizes":{"adult":256},"walk_speed":60,"animations":{},
            "head":{"baby":{"dragged":[[70,30,40,-4],[74,32,40,4]]},"adult":{"dragged":[[140,60,80,-4],[148,64,80,4]]}}}"#;
        let m: super::PetManifest = serde_json::from_str(json).unwrap();
        assert_eq!((m.frame_size_at("baby"), m.frame_size_at("adult")), (128, 256));
        assert_eq!(m.hang_grip("baby"), m.hang_grip("adult"));
    }

    use super::*;

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

    /// 새: swift는 나는 동물에게만 뜻이 있다 (걷는 동물에 적혀 있어도 PetConfig::swift는 꺼진다). 없으면 false
    #[test]
    fn swift_defaults_off_and_only_counts_for_flyers() {
        assert!(!parse_manifest(&full_json()).unwrap().swift);
        let walker = full_json().replace(r#""walk_speed": 60,"#, r#""walk_speed": 60, "swift": true,"#);
        let m = parse_manifest(&walker).unwrap();
        assert!(m.swift && !crate::brain::PetConfig::from_manifest(&m, 1.0, 1.0, "teenager").swift);
        let flyer = walker.replace(r#""swift": true,"#, r#""swift": true, "locomotion": "fly","#);
        assert!(crate::brain::PetConfig::from_manifest(&parse_manifest(&flyer).unwrap(), 1.0, 1.0, "teenager").swift);
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

    #[test]
    fn parses_giraffe_neckstretch() {
        let json = full_json().replace(r#""animations": {"#, r#""animations": {"neckstretch": {"row": 21, "frames": 3, "fps": 2},"#);
        let m = parse_manifest(&json).unwrap();
        assert!(m.has_anim("neckstretch"));
        assert_eq!(crate::behavior::Special::from_manifest(|n| m.has_anim(n)), Some(crate::behavior::Special::NeckStretch));
        assert_eq!(OPTIONAL_ANIMS[5], "neckstretch");
        let bad = full_json().replace(r#""animations": {"#, r#""animations": {"neckstretch": {"row": 21, "frames": 0, "fps": 2},"#);
        assert!(parse_manifest(&bad).is_err());
    }

    /// 올려다보기: 선택 동작의 맨 끝 — 있으면 can_look_up, 없으면 그냥 쳐다본다
    #[test]
    fn parses_lookup_as_the_last_optional_anim() {
        assert_eq!(OPTIONAL_ANIMS[6], "lookup");
        assert_eq!(OPTIONAL_ANIMS[10], "surprise"); // 레서판다 깜짝 (v0.5.5)
        assert!(!parse_manifest(&full_json()).unwrap().can_look_up());
        let json = full_json().replace(r#""animations": {"#, r#""animations": {"lookup": {"row": 21, "frames": 3, "fps": 3},"#);
        assert!(parse_manifest(&json).unwrap().can_look_up());
        let bad = full_json().replace(r#""animations": {"#, r#""animations": {"lookup": {"row": 21, "frames": 0, "fps": 3},"#);
        assert!(parse_manifest(&bad).is_err());
    }

    /// 번들 pet.json: 모든 동물이 올려다본다
    #[test]
    fn bundled_pets_look_up() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../ui/pets");
        for id in crate::test_util::species() {
            let json = std::fs::read_to_string(dir.join(&id).join("pet.json")).unwrap();
            assert!(parse_manifest(&json).unwrap().can_look_up(), "{id}");
        }
    }

    /// 공룡: lookup 뒤의 fire·roar·stomp → 공룡 특별 동작 (셋 중 하나를 고른다)
    #[test]
    fn parses_dino_anims_as_the_dino_special() {
        assert_eq!(OPTIONAL_ANIMS[7..10], ["fire", "roar", "stomp"]);
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

    /// 번들 호박은 벽을 타고, 같은 할로윈 유령은 난다 (벽 없음)
    #[test]
    fn bundled_pumpkin_climbs_and_the_ghost_does_not() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../ui/pets");
        let load = |id: &str| parse_manifest(&std::fs::read_to_string(dir.join(id).join("pet.json")).unwrap()).unwrap();
        assert!(load("pumpkin").can_climb());
        assert!(!load("ghost").can_climb());
    }

    /// 번들 공룡 셋: 걷고, 벽을 타고, 공룡 동작을 모두 갖고, 좋아하는 음식이 저마다 다르다
    #[test]
    fn bundled_dinos() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../ui/pets");
        for (id, food) in [("dino_yellow", "🍉"), ("dino_green", "🥦"), ("dino_purple", "🍇")] {
            let json = std::fs::read_to_string(dir.join(id).join("pet.json")).unwrap();
            let m = parse_manifest(&json).unwrap();
            assert!(["fire", "roar", "stomp"].iter().all(|a| m.has_anim(a)), "{id}");
            assert!(crate::behavior::Special::from_manifest(|n| m.has_anim(n)).is_some_and(|s| s.is_dino()), "{id}");
            assert!(m.can_climb() && m.locomotion == Locomotion::Walk, "{id}");
            assert!(json.contains(food), "{id}");
        }
    }

    /// 번들 산타·눈사람은 고양이처럼 벽을 탄다. 눈사람만 구름을 부르고, 둘 다 구르지 않는다 (썰매 · 배 미끄럼)
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

    /// 번들 다람쥐는 고양이처럼 벽을 타고, 걷다가 가끔 깡총거리고, 도토리를 좋아한다
    #[test]
    fn bundled_squirrel_climbs_and_hops_now_and_then() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../ui/pets/squirrel/pet.json");
        let m = parse_manifest(&std::fs::read_to_string(path).unwrap()).unwrap();
        assert!(m.can_climb() && m.locomotion == Locomotion::Walk);
        assert!(m.hop_share() > 0.0 && m.hop_share() < 1.0);
        assert_eq!(m.favorite_food.as_ref().map(|f| f.name.as_str()), Some("도토리"));
    }

    /// 개구리 (아트 에이전트가 ui/pets/frog를 만들면 그대로 읽힌다 — 없으면 건너뛴다)
    #[test]
    fn bundled_frog_hops_and_climbs_if_present() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../ui/pets/frog/pet.json");
        let Ok(json) = std::fs::read_to_string(path) else { return };
        let m = parse_manifest(&json).unwrap();
        assert_eq!(m.locomotion, Locomotion::Hop);
        assert!(m.has_anim("bigjump") && m.can_climb());
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

    /// 토끼는 걷는 시간의 절반쯤 깡총거린다
    #[test]
    fn bundled_rabbit_hops_half_the_time() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../ui/pets/rabbit/pet.json");
        let m = parse_manifest(&std::fs::read_to_string(path).unwrap()).unwrap();
        assert_eq!(m.locomotion, Locomotion::Walk);
        assert_eq!(m.hop_share(), 0.5);
        assert!(!m.hops() && !m.has_anim("bigjump"));
    }

    /// 곰·꿀벌·강아지는 v0.5.5에서 빠졌다 — 번들에 없고, 그들만 쓰던 동작(wag 살랑·sting 쏘기)도 선택 동작이 아니다
    #[test]
    fn bear_bee_and_dog_are_no_longer_bundled() {
        let pets = crate::test_util::species();
        for gone in ["bear", "bee", "dog"] {
            assert!(!pets.iter().any(|p| p == gone), "{gone}");
        }
        assert!(!OPTIONAL_ANIMS.contains(&"wag") && !OPTIONAL_ANIMS.contains(&"sting"));
    }

    /// 할로윈 호박: 걷는 동물인데 걷기·달리기는 늘 통통 (hop_share 1.0), bigjump 그림 없음 (벽 타기는 번들 pet.json의 climb·slide로 — bundled_pumpkin_climbs_and_the_ghost_does_not)
    fn check_pumpkin(m: &PetManifest) {
        assert_eq!(m.locomotion, Locomotion::Walk);
        assert_eq!(m.hop_share(), 1.0);
        assert!(!m.flies() && !m.hops() && !m.has_anim("bigjump"));
    }

    /// 할로윈 유령: 둥실 날아다닌다 (locomotion fly), 벽은 타지 않는다
    fn check_ghost(m: &PetManifest) {
        assert_eq!(m.locomotion, Locomotion::Fly);
        assert!(m.flies() && !m.can_climb());
        assert!(m.walk_speed < 60.0, "ghost floats slowly: {}", m.walk_speed);
    }

    #[test]
    fn halloween_manifest_fields_parse() {
        let pumpkin = full_json().replace(r#""walk_speed": 60,"#, r#""walk_speed": 60, "hop_share": 1.0, "personality": "playful", "stage_size": {"adult": 0.9},"#);
        let m = parse_manifest(&pumpkin).unwrap();
        check_pumpkin(&m);
        assert_eq!((m.personality, m.size_mult(Stage::Adult)), (Personality::Playful, 0.9));
        let ghost = full_json().replace(r#""walk_speed": 60,"#, r#""walk_speed": 40, "locomotion": "fly", "stage_size": {"adult": 0.85},"#);
        let m = parse_manifest(&ghost).unwrap();
        check_ghost(&m);
        assert_eq!(m.size_mult(Stage::Adult), 0.85);
    }

    /// 호박·유령 (아트 에이전트가 ui/pets/{pumpkin,ghost}를 만들면 그대로 읽힌다 — 없으면 건너뛴다)
    #[test]
    fn bundled_halloween_pets_if_present() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../ui/pets");
        if let Ok(json) = std::fs::read_to_string(dir.join("pumpkin/pet.json")) {
            let m = parse_manifest(&json).unwrap();
            check_pumpkin(&m);
            assert!(m.favorite_food.is_some());
        }
        if let Ok(json) = std::fs::read_to_string(dir.join("ghost/pet.json")) {
            let m = parse_manifest(&json).unwrap();
            check_ghost(&m);
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
        assert!(parse_manifest(&with("castle")).is_err());
        // 루돌프·산타의 크리스마스 벽돌집
        assert_eq!(parse_manifest(&with("xmas")).unwrap().house_kind(), HouseKind::Xmas);
    }

    /// 번들 pet.json: 호박·유령은 귀신의 집, 루돌프·산타는 크리스마스 벽돌집, 나머지는 보통 집
    #[test]
    fn bundled_pets_pick_their_house() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../ui/pets");
        for (id, kind) in [("pumpkin", HouseKind::Haunted), ("ghost", HouseKind::Haunted), ("cat", HouseKind::House),
                           ("rudolph", HouseKind::Xmas), ("santa", HouseKind::Xmas), ("snowman", HouseKind::Xmas)] {
            if let Ok(json) = std::fs::read_to_string(dir.join(id).join("pet.json")) {
                assert_eq!(parse_manifest(&json).unwrap().house_kind(), kind, "{id}");
            }
        }
    }

    /// 픽셀 pet.json: 동글 필수 동작에서 roll만 빠진 표
    fn pixel_json(extra: &str) -> String {
        let anims: Vec<String> = REQUIRED_ANIMS
            .iter()
            .filter(|a| **a != "roll")
            .enumerate()
            .map(|(i, a)| format!(r#""{a}": {{"row": {i}, "frames": 2, "fps": 4}}"#))
            .collect();
        format!(r#"{{{extra} "animations": {{{}}}}}"#, anims.join(","))
    }

    #[test]
    fn round_manifest_frame_size_is_the_same_on_every_stage() {
        let m = parse_manifest(&full_json()).unwrap();
        assert!(m.frame_sizes.is_empty());
        assert_eq!((m.frame_size_at("baby"), m.frame_size_at("adult")), (64, 64));
        assert!(!m.no_roll(), "round pets always have a roll");
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
    fn pixel_manifest_only_brings_its_art_everything_else_stays_the_round_pets() {
        let base = parse_manifest(&full_json().replace(r#""walk_speed": 60,"#, r#""walk_speed": 60, "personality": "aloof", "stage_size": {"adult": 1.3}, "hop_share": 0.5,"#)).unwrap();
        let m = parse_pixel_manifest(&pixel_json(r#""name": "other", "walk_speed": 999, "personality": "playful", "frame_sizes": {"baby": 24, "kinder": 28, "child": 32, "teenager": 32, "adult": 36},"#), &base).unwrap();
        assert_eq!((m.name.as_str(), m.walk_speed, m.personality, m.size_mult(Stage::Adult), m.hop_share), ("고양이", 60.0, Personality::Aloof, 1.3, 0.5));
        assert_eq!((m.frame_size_at("baby"), m.frame_size_at("kinder"), m.frame_size_at("adult")), (24, 28, 36));
        assert!(m.no_roll() && !m.has_anim("roll"));
    }

    /// 배로 미끄러지는 동물: 픽셀 pet.json에 roll이 있으면 구른다 (spin은 픽셀 pet.json에 없다 — 돌지 않는다)
    #[test]
    fn a_pixel_belly_slide_keeps_its_roll_without_spin() {
        let base = parse_manifest(&full_json()).unwrap();
        let json = pixel_json("").replace(r#""animations": {"#, r#""animations": {"roll": {"row": 21, "frames": 2, "fps": 4},"#);
        let m = parse_pixel_manifest(&json, &base).unwrap();
        assert!(!m.no_roll() && m.has_anim("roll"));
        assert_eq!(m.animations["roll"].spin, 0.0);
    }

    #[test]
    fn pixel_frame_sizes_default_to_the_grid() {
        let base = parse_manifest(&full_json()).unwrap();
        let some = parse_pixel_manifest(&pixel_json(r#""frame_sizes": {"baby": 20, "adult": 40},"#), &base).unwrap();
        assert_eq!((some.frame_size_at("baby"), some.frame_size_at("adult")), (20, 40));
        assert_eq!(some.frame_size_at("kinder"), 28, "default grid for a stage it does not list");
        let none = parse_pixel_manifest(&pixel_json(""), &base).unwrap();
        assert_eq!((none.frame_size_at("baby"), none.frame_size_at("child"), none.frame_size_at("teenager"), none.frame_size_at("adult")), (24, 32, 32, 36));
        assert!(parse_pixel_manifest(&pixel_json(r#""frame_sizes": {"baby": 0},"#), &base).is_err());
    }

    #[test]
    fn pixel_manifest_is_validated_like_a_round_one() {
        let base = parse_manifest(&full_json()).unwrap();
        assert!(parse_pixel_manifest("{", &base).is_err());
        assert!(parse_pixel_manifest(&pixel_json("").replace(r#""sleep""#, r#""nap""#), &base).unwrap_err().contains("sleep"));
        assert!(parse_pixel_manifest(&pixel_json("").replace(r#""frames": 2"#, r#""frames": 0"#), &base).is_err());
        let bad_roll = pixel_json("").replace(r#""animations": {"#, r#""animations": {"roll": {"row": 9, "frames": 0, "fps": 4},"#);
        assert!(parse_pixel_manifest(&bad_roll, &base).is_err());
    }

    #[test]
    fn hang_grip_measures_in_the_stages_own_dots() {
        let base = parse_manifest(&full_json()).unwrap();
        let json = pixel_json(r#""frame_sizes": {"baby": 24, "adult": 36}, "head": {"baby": {"dragged": [[15, 6, 8, 0]]}, "adult": {"dragged": [[18, 9, 8, 0]]}},"#);
        let m = parse_pixel_manifest(&json, &base).unwrap();
        let (x, y) = m.hang_grip("baby");
        assert!((x - (15.0 / 24.0 - 0.5)).abs() < 1e-9 && (y - (6.0 - 1.2) / 24.0).abs() < 1e-9, "{x} {y}");
        let (x, _) = m.hang_grip("adult");
        assert!(x.abs() < 1e-9, "{x}");
    }

    /// 기린: 목 늘이기 층이 있으면 픽셀에서도 늘이고, 없으면 그 동작은 없다. 구르기는 어느 쪽도 없다
    #[test]
    fn pixel_giraffe_keeps_its_neck_stretch_only_with_layers() {
        use crate::behavior::Special;
        let base = parse_manifest(&full_json().replace(r#""animations": {"#, r#""animations": {"neckstretch": {"row": 21, "frames": 3, "fps": 2},"#)).unwrap();
        assert_eq!(base.special(), Some(Special::NeckStretch), "round always");
        let anim = |extra: &str| pixel_json(extra).replace(r#""animations": {"#, r#""animations": {"neckstretch": {"row": 21, "frames": 3, "fps": 2},"#);
        assert_eq!(parse_pixel_manifest(&anim(""), &base).unwrap().special(), None);
        let with = parse_pixel_manifest(&anim(r#""neck_stretch": {"baby": {"cut": 9, "tile": 3, "ext": 12}},"#), &base).unwrap();
        assert_eq!(with.special(), Some(Special::NeckStretch));
    }

    /// PNG 머리에서 (너비, 높이)
    fn png_size(path: &std::path::Path) -> (u32, u32) {
        let b = std::fs::read(path).unwrap_or_else(|e| panic!("{path:?}: {e}"));
        (u32::from_be_bytes([b[16], b[17], b[18], b[19]]), u32::from_be_bytes([b[20], b[21], b[22], b[23]]))
    }

    /// 번들 픽셀 그림 (ui/pets_pixel — 있는 동물만): 동글 동물 위에 올라가고, 구르기가 없고, 다섯 단계 시트가 칸 표에 맞는다
    #[test]
    fn bundled_pixel_pets_fit_their_sheets() {
        let ui = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../ui");
        let Ok(dir) = std::fs::read_dir(ui.join("pets_pixel")) else { return };
        for entry in dir {
            let path = entry.unwrap().path();
            let Ok(json) = std::fs::read_to_string(path.join("pet.json")) else { continue };
            let id = path.file_name().unwrap().to_string_lossy().into_owned();
            let base = parse_manifest(&std::fs::read_to_string(ui.join("pets").join(&id).join("pet.json")).unwrap()).unwrap_or_else(|e| panic!("{id}: {e}"));
            let m = parse_pixel_manifest(&json, &base).unwrap_or_else(|e| panic!("{id}: {e}"));
            // 도는 구르기는 도트로 돌리면 무너진다 — 돌지 않는 배 미끄럼(펭귄·눈사람)만 픽셀에도 있다 (tools/draw_pixel.py PIXEL_ROLL)
            let slides = matches!(id.as_str(), "penguin" | "snowman");
            assert_eq!(m.no_roll(), !slides, "{id}: only the belly sliders carry a roll in pixel");
            if let Some(roll) = m.animations.get("roll") {
                assert_eq!((roll.spin, base.animations["roll"].spin), (0.0, 0.0), "{id}: a pixel roll never spins");
            }
            for stage in Stage::ALL {
                let (name, sheet) = (stage.name(), path.join(format!("sheet_{}.png", stage.name())));
                if !sheet.is_file() {
                    continue; // 아직 그리는 중 — 앱은 다섯 장이 다 있어야 픽셀로 쓴다 (build.rs)
                }
                let size = m.frame_size_at(name);
                let (w, h) = png_size(&sheet);
                assert!(w % size == 0 && h % size == 0, "{id} {name}: {w}x{h} is not whole frames of {size}");
                for (anim, spec) in &m.animations {
                    assert!((spec.row + 1) * size <= h && spec.frames * size <= w, "{id} {name} {anim} falls off the sheet");
                }
                assert_eq!(m.head.get(name).is_some(), base.head.contains_key(name), "{id} {name}: head table");
            }
        }
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
