//! 물건 (쓰레기 장난감, phase 3): 뇌도 성장도 없는 물리 덩어리.
//! 스스로 움직이지 않고 — 가끔 부는 바람에만 조금 구름 — 잡아 끌고 던질 수 있고, 친구가 연결된 화면 끝으로 세게 던지면 그 화면으로 날아감
//! 던지면 제 모양 그대로 빙글빙글 돌며 날아가고, 떨어진 뒤에도 굴러가다 마찰로 멈춰 가까운 면으로 누움 (모양과 무게는 tumble.rs).
//! 던지기 전에 포스트잇을 붙일 수 있음: 받는 사람이 누르면 10초 보이고 영영 사라짐
//! 대부분의 시간엔 잠들어 있음 (`is_asleep`): 앱은 창도 웹뷰도 건드리지 않음
//! 🕸️ 거미집에 걸림 (spec 5부): 날거나 떨어지거나 구르다 보이는 거미집 안으로 들어오면 무조건 걸려 매달림 (Phase::Snagged) —
//! "팅~" 출렁 (web::snag_spring) 뒤 살랑살랑, 거미집이 사라지면 떨어지고 옮겨 가면 따라감, 잡아 끌면 빠지고 web::SNAG_IMMUNE_SECS 동안은 다시 안 걸림.
//! 거미집 자리는 앱이 틱마다 set_webs로 알려 줌. 사용자가 잡은 것·동물이 든 것·회오리 속인 것은 안 걸림

use std::collections::VecDeque;

use crate::collide::{rebound, Face};
use crate::physics::{clamp_speed, step_air, throw_velocity, Body, THROW_GRAVITY_LOGICAL, THROW_LIFT, VELOCITY_WINDOW};
use crate::tumble::{self, heft_of, shape_of, throw_spin, wobble_spin};
use crate::visit::{self, ExitEdges, Fling, Side};
use crate::web::{self, WebSpot};
use crate::surfaces::Owner;
use crate::world::World;

/// 물건 창 한 변 (논리 px). 그림은 128 칸에 그려 이 크기로 줄임 (tools/draw_objects.py)
pub const OBJECT_LOGICAL_SIZE: f64 = 96.0;
/// 한 사람이 만들어 둘 수 있는 물건 수 — 넘치면 가장 오래된 것이 뿅 사라짐
pub const MAX_OWN_OBJECTS: usize = 10;
/// 친구에게서 받아 화면에 있을 수 있는 물건 수 — 넘치면 가장 오래된 것부터
pub const MAX_FOREIGN_OBJECTS: usize = 20;
/// 🪨💩 거대한 것은 화면에 이만큼만 (내 것·받은 것 합쳐) — 화면만 한 창이 쌓이면 앱이 느려짐
pub const MAX_GIANTS: usize = 3;

/// 친구에게서 들어온 거대한 것이 솟을 수 있는 높이: 머리 위 남은 자리의 이만큼까지
const GIANT_ARRIVAL_HEADROOM: f64 = 0.8;

/// 거대한 것이 하나 더 들어옴: 지금 있는 거대한 것들(오래된 순)에서 뿅 사라질 하나
pub fn giant_to_drop(giants_oldest_first: &[u64]) -> Option<u64> {
    (giants_oldest_first.len() >= MAX_GIANTS).then(|| giants_oldest_first[0])
}
/// 화면에 있을 수 있는 똥 수 (내 동물·손님·친구가 던진 것 모두) — 넘치면 가장 오래된 똥이 사라짐. 쓰레기 수와 따로 셈
pub const MAX_POOPS: usize = 10;
/// 포스트잇을 눌러 읽는 시간 (초). 그 뒤엔 다시 볼 수 없음
pub const NOTE_READ_SECS: f64 = 10.0;
/// 바람이 부는 간격 (초, 물건마다 따로)
pub const GUST_EVERY_SECS: (f64, f64) = (30.0, 90.0);
/// 🪨💩 거대한 물건은 아기 동물의 이만큼 배 (한 변)
pub const GIANT_TIMES: f64 = 15.0;
/// 아기 고양이의 종 배율 (ui/pets/cat/pet.json stage_size.baby — tools/test_draw_objects.py가 맞춰 봄)
pub const BABY_CAT_STAGE_SIZE: f64 = 0.91;
/// 거대한 물건은 작업 영역 높이의 이만큼을 넘지 않음
pub const GIANT_MAX_SCREEN: f64 = 0.8;

/// 거대한 물건 칸의 한 변 (논리 px): 아기 고양이 한 변(128 × 아기 배율 × stage_size ≈ 52.4)의 15배 ≈ 786
pub fn giant_side() -> f64 {
    GIANT_TIMES * crate::brain::PET_LOGICAL_SIZE * crate::care::Stage::Baby.scale() * BABY_CAT_STAGE_SIZE
}

/// 거대한 물건의 그림 배율: 15배, 단 작업 영역 높이(논리 px)의 80%까지. 높이를 모르면 그냥 15배
pub fn giant_scale(work_area_height: f64) -> f64 {
    let side = if work_area_height.is_finite() && work_area_height > 0.0 {
        giant_side().min(GIANT_MAX_SCREEN * work_area_height)
    } else {
        giant_side()
    };
    side / OBJECT_LOGICAL_SIZE
}

/// 몸 폭 절반 / 창 크기
const HALF_WIDTH_RATIO: f64 = 0.3;
/// 이보다 세게 떨어지면 살짝 튀어 오름 (논리 px/s)
const BOUNCE_MIN_VY: f64 = 350.0;
/// 빗자루에 쓸린 물건의 최소 가로 속도와 뜨는 속도 (논리 px/s)
const SWEEP_MIN_VX: f64 = 240.0;
const SWEEP_POP: f64 = 160.0;
/// 🧹 거대한 것은 무거워서 이 배만큼만 밀림
const GIANT_SWEEP: f64 = 0.4;
/// 이만큼 안에서 이만큼 안 움직이고 놓으면 클릭 (논리 px, 초)
const CLICK_SLOP: f64 = 6.0;
const CLICK_SECS: f64 = 0.5;

/// 물건 종류 (그림 이름과 같음)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ObjectKind {
    Can,
    Paper,
    Stone,
    Glasses,
    Computer,
    Milk,
    Shirt,
    Umbrella,
    /// 💩 동물이 싼 똥 (poop.rs): 둘둘 쌓인 큰 똥, 작은 똥, 토끼의 알갱이. 메뉴에 없고 포스트잇을 못 붙임
    Poop,
    Pink,
    Rainbow,
    Pellets,
    /// 🪨 거대한 돌 — 메뉴로 꺼냄. 아기 동물의 15배 (화면 높이의 80%까지), 바람에 꿈쩍 않고 포스트잇을 못 붙임
    GiantStone,
    /// 💩 거대한 똥 — 메뉴로 꺼냄. 동물이 싼 게 아니라 쓰레기처럼 치울 때까지 남고, 빗자루에 쓸림
    GiantPoop,
    /// 💩 거대한 핑크 똥·무지개 똥: 메뉴의 "거대한 똥"이 갈색·핑크·무지개 중 하나로 나옴 (menu_pick)
    GiantPink,
    GiantRainbow,
    /// ⛄ 쌓인 눈을 커서로 문질러 뭉친 눈덩이 (snowball.rs) — 동물도 발밑 눈으로 뭉쳐 던짐. 메뉴에 없고 포스트잇을 못 붙임.
    /// 던지면 닿은 곳에서 퍽 부서지고 (동물에 맞으면 "brr!"), 살며시 놓으면 쉬다가 몇 분 뒤 녹음. 친구에게 던질 수 있음 (옛 버전은 모르는 물건이라 버림)
    Snowball,
    /// 📌 흐물이가 작업표시줄(Dock)에서 끌어낸 앱 아이콘 장난감 (taskbar.rs): 그림은 열린 앱 하나의 아이콘을 베낀 것 (앱이 창에 넣음 — 그림 파일이 없음).
    /// 🔓 (v0.6.5, 사용자 결정) 친구에게 던지거나 들고 갈 수 있음 — 그때 아이콘 그림 칸이 같이 감 (icon::to_wire). ALL에는 없어 이름만 오면 받지 않고,
    /// 그림까지 확인될 때만 받음 (accept_toy_toss·accept_toy_carry). 메뉴에 없고 포스트잇을 못 붙임. 한 번에 하나, TOY_LIFE_SECS 뒤 저절로 사라짐
    AppIcon,
}

/// 📌 장난감의 이름 (창 스크립트·웹뷰, 🔓 v0.6.5부터 아이콘 그림과 함께 통신에도 — 그림 파일 목록 tools/draw_objects.py NAMES에는 없음)
pub const TOY_ID: &str = "app_icon";
/// 화면에 있을 수 있는 장난감 수
pub const MAX_TOYS: usize = 1;

impl ObjectKind {
    /// 모든 물건 종류 (메뉴·그림 순서)
    pub const ALL: [ObjectKind; 17] = [
        ObjectKind::Can,
        ObjectKind::Paper,
        ObjectKind::Stone,
        ObjectKind::Glasses,
        ObjectKind::Computer,
        ObjectKind::Milk,
        ObjectKind::Shirt,
        ObjectKind::Umbrella,
        ObjectKind::Poop,
        ObjectKind::Pink,
        ObjectKind::Rainbow,
        ObjectKind::Pellets,
        ObjectKind::GiantStone,
        ObjectKind::GiantPoop,
        ObjectKind::GiantPink,
        ObjectKind::GiantRainbow,
        ObjectKind::Snowball,
    ];
    /// `물건 ▸` 메뉴로 꺼내는 쓰레기 (친구에게 10개씩 던지는 것도 이 중에서)
    pub const TRASH: [ObjectKind; 8] = [
        ObjectKind::Can,
        ObjectKind::Paper,
        ObjectKind::Stone,
        ObjectKind::Glasses,
        ObjectKind::Computer,
        ObjectKind::Milk,
        ObjectKind::Shirt,
        ObjectKind::Umbrella,
    ];
    /// 🪨💩 거대한 물건 (메뉴에서 쓰레기 다음)
    pub const GIANTS: [ObjectKind; 4] = [ObjectKind::GiantStone, ObjectKind::GiantPoop, ObjectKind::GiantPink, ObjectKind::GiantRainbow];
    /// `물건 ▸` 메뉴 순서: 쓰레기, 거대한 물건
    pub const MENU: [ObjectKind; 10] = [
        ObjectKind::Can,
        ObjectKind::Paper,
        ObjectKind::Stone,
        ObjectKind::Glasses,
        ObjectKind::Computer,
        ObjectKind::Milk,
        ObjectKind::Shirt,
        ObjectKind::Umbrella,
        ObjectKind::GiantStone,
        ObjectKind::GiantPoop,
    ];

    /// 📌 작업표시줄에서 꺼낸 앱 아이콘 장난감 — 친구에게는 아이콘 그림과 함께만 감 (🔓 v0.6.5)
    pub fn is_toy(self) -> bool {
        self == ObjectKind::AppIcon
    }

    /// ⛄ 눈덩이: 던지면 닿은 곳에서 부서지고, 저절로 녹음
    pub fn is_snowball(self) -> bool {
        self == ObjectKind::Snowball
    }

    /// 거대한 돌·똥
    pub fn is_giant(self) -> bool {
        matches!(self, ObjectKind::GiantStone | ObjectKind::GiantPoop | ObjectKind::GiantPink | ObjectKind::GiantRainbow)
    }

    /// 메뉴에서 꺼낼 때 실제로 나오는 것 (roll: 0..1): "거대한 똥"은 갈색·핑크·무지개가 1/3씩. 나머지는 그대로
    pub fn menu_pick(self, roll: f64) -> ObjectKind {
        match self {
            ObjectKind::GiantPoop if roll >= 2.0 / 3.0 => ObjectKind::GiantRainbow,
            ObjectKind::GiantPoop if roll >= 1.0 / 3.0 => ObjectKind::GiantPink,
            kind => kind,
        }
    }

    /// 거대한 똥 (갈색·핑크·무지개)
    pub fn is_giant_poop(self) -> bool {
        matches!(self, ObjectKind::GiantPoop | ObjectKind::GiantPink | ObjectKind::GiantRainbow)
    }

    /// 포스트잇을 붙일 수 있음: 쓰레기만 (똥은 사라지고, 거대한 것은 쪽지 창이 그림을 덮음)
    pub fn takes_postit(self) -> bool {
        !self.is_poop() && !self.is_giant() && !self.is_toy() && !self.is_snowball()
    }

    /// 똥임: 수를 따로 세고, 저절로 사라지고, 바람에 구르지 않음
    pub fn is_poop(self) -> bool {
        matches!(self, ObjectKind::Poop | ObjectKind::Pink | ObjectKind::Rainbow | ObjectKind::Pellets)
    }

    /// 그림 크기 배율: 쓰레기는 96 논리 px 칸의 절반 (똥은 싼 동물을 따라 set_scale로 정함), 거대한 것은 15배 (화면을 알면 scale_on)
    pub fn base_scale(self) -> f64 {
        if self.is_giant() {
            giant_scale(f64::NAN)
        } else if self.is_poop() || self.is_toy() {
            1.0
        } else {
            0.5
        }
    }

    /// 화면에 놓을 때의 배율: size = 크기 조절 창의 쓰레기 배율, work_area_height = 놓일 작업 영역 높이 (논리 px).
    /// 쓰레기만 size를 따르고, 거대한 것은 화면 높이에 맞춤
    pub fn scale_on(self, size: f64, work_area_height: f64) -> f64 {
        if self.is_giant() {
            giant_scale(work_area_height)
        } else if self.is_poop() || self.is_toy() {
            self.base_scale()
        } else {
            self.base_scale() * size
        }
    }

    /// 통신과 그림 파일(ui/objects/<id>.png)에 쓰는 이름
    pub fn id(self) -> &'static str {
        match self {
            ObjectKind::Can => "can",
            ObjectKind::Paper => "paper",
            ObjectKind::Stone => "stone",
            ObjectKind::Glasses => "glasses",
            ObjectKind::Computer => "computer",
            ObjectKind::Milk => "milk",
            ObjectKind::Shirt => "shirt",
            ObjectKind::Umbrella => "umbrella",
            ObjectKind::Poop => "poop",
            ObjectKind::Pink => "pink",
            ObjectKind::Rainbow => "rainbow",
            ObjectKind::Pellets => "pellets",
            ObjectKind::GiantStone => "giant_stone",
            ObjectKind::GiantPoop => "giant_poop",
            ObjectKind::GiantPink => "giant_pink",
            ObjectKind::GiantRainbow => "giant_rainbow",
            ObjectKind::Snowball => "snowball",
            ObjectKind::AppIcon => TOY_ID,
        }
    }

    /// 메뉴 글자
    pub fn label(self) -> &'static str {
        match self {
            ObjectKind::Can => "🥫 찌그러진 캔",
            ObjectKind::Paper => "📰 구겨진 신문지",
            ObjectKind::Stone => "🪨 돌멩이",
            ObjectKind::Glasses => "👓 금 간 안경",
            ObjectKind::Computer => "💻 고장난 컴퓨터",
            ObjectKind::Milk => "🥛 빈 우유팩",
            ObjectKind::Shirt => "👕 누더기 티셔츠",
            ObjectKind::Umbrella => "☂️ 부러진 우산",
            ObjectKind::Poop | ObjectKind::Pink | ObjectKind::Rainbow | ObjectKind::Pellets => "💩 똥",
            ObjectKind::GiantStone => "🪨 거대한 돌",
            ObjectKind::GiantPoop => "💩 거대한 똥",
            ObjectKind::GiantPink => "💩 거대한 핑크 똥",
            ObjectKind::GiantRainbow => "💩 거대한 무지개 똥",
            ObjectKind::Snowball => "⛄ 눈덩이",
            ObjectKind::AppIcon => "📌 앱 아이콘 장난감",
        }
    }

    /// 아는 이름만 (친구가 보낸 글자 — 그대로 파일 이름·창 스크립트에 쓰이므로 목록과 정확히 같아야 함)
    pub fn from_id(id: &str) -> Option<ObjectKind> {
        if id == "dropping" {
            // 예전의 초록 똥 (옛 버전 친구가 보낸다)
            return Some(ObjectKind::Poop);
        }
        ObjectKind::ALL.into_iter().find(|k| k.id() == id || k.wire_id() == id)
    }

    /// 친구가 던진 이름과 색 표시 → 종류 (색은 똥에만, 모르는 색은 그냥 똥)
    pub fn wire_kind(id: &str, look: Option<&str>) -> Option<ObjectKind> {
        match (ObjectKind::from_id(id)?, look) {
            (ObjectKind::Poop, Some("pink")) => Some(ObjectKind::Pink),
            (ObjectKind::Poop, Some("rainbow")) => Some(ObjectKind::Rainbow),
            (ObjectKind::Poop, Some("giant")) => Some(ObjectKind::GiantPoop),
            (ObjectKind::Poop, Some("giant_pink")) => Some(ObjectKind::GiantPink),
            (ObjectKind::Poop, Some("giant_rainbow")) => Some(ObjectKind::GiantRainbow),
            (ObjectKind::Stone, Some("giant")) => Some(ObjectKind::GiantStone),
            (kind, _) => Some(kind),
        }
    }

    /// 똥 색과 거대함 (Toss::look으로 보냄 — 옛 버전 친구는 몰라서 갈색 똥·보통 돌로 받음)
    pub fn look(self) -> Option<&'static str> {
        match self {
            ObjectKind::Pink => Some("pink"),
            ObjectKind::Rainbow => Some("rainbow"),
            ObjectKind::GiantStone | ObjectKind::GiantPoop => Some("giant"),
            // 옛 버전(0.6.2까지) 친구는 이 색을 몰라 보통 갈색 똥으로 받는다
            ObjectKind::GiantPink => Some("giant_pink"),
            ObjectKind::GiantRainbow => Some("giant_rainbow"),
            _ => None,
        }
    }

    /// 친구에게 던질 때의 이름: 티셔츠는 양말 자리를 물려받아, 옛 버전 친구는 양말로 받음
    pub fn wire_id(self) -> &'static str {
        // 옛 버전 친구는 무지개·거대함을 몰라 그냥 똥·돌로 받는다 (Toss::look으로 알린다)
        if self == ObjectKind::Shirt {
            "sock"
        } else if self == ObjectKind::GiantStone {
            "stone"
        } else if self.look().is_some() {
            "poop"
        } else {
            self.id()
        }
    }

    /// 한 번의 바람에 가는 거리 (논리 px, 최소·최대) — 무거운 것은 거의 안 감
    fn gust_reach(self) -> (f64, f64) {
        match self {
            ObjectKind::Can => (80.0, 180.0),
            ObjectKind::Paper => (60.0, 170.0),
            ObjectKind::Shirt => (40.0, 120.0),
            ObjectKind::Glasses => (20.0, 60.0),
            ObjectKind::Milk => (30.0, 90.0),
            ObjectKind::Umbrella => (30.0, 80.0),
            ObjectKind::Stone => (8.0, 24.0),
            ObjectKind::Computer => (6.0, 18.0),
            // 바닥에 붙어 있다 (start_gust가 막는다)
            ObjectKind::Poop | ObjectKind::Pink | ObjectKind::Rainbow | ObjectKind::Pellets => (0.0, 0.0),
            // 너무 무겁다 (start_gust가 막는다)
            ObjectKind::GiantStone | ObjectKind::GiantPoop | ObjectKind::GiantPink | ObjectKind::GiantRainbow => (0.0, 0.0),
            // 납작한 네모: 캔보다 덜 구른다
            ObjectKind::AppIcon => (20.0, 60.0),
            // 동글동글하지만 눈이 붙어 조금만
            ObjectKind::Snowball => (10.0, 40.0),
        }
    }
}

/// 물건에 붙은 포스트잇
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Postit {
    #[default]
    None,
    /// 내가 붙였음 — 눌러도 읽히지 않고, 떼거나 던져 보낼 수 있음
    Mine,
    /// 받았음, 아직 안 읽음 — 누르면 읽음
    Unread,
    /// 읽는 중 — 글자는 이미 웹뷰로 갔고 여기엔 남아 있지 않음
    Reading,
    /// 똥·거대한 물건: 포스트잇을 붙일 수 없음 (똥은 저절로 사라지면 받은 글자도 사라지니까, 거대한 것은 쪽지 창이 그림을 덮으니까)
    Never,
}

/// 포인터를 놓은 결과
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Release {
    Click,
    Throw,
}

/// 던져서 화면 끝을 넘어갔음 (이 물건은 이 화면에서 끝남)
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Exit {
    /// 넘어간 화면 끝
    pub side: Side,
    /// 날아가던 모양 (받는 화면이 이어서 날림)
    pub fling: Fling,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum Phase {
    Held,
    Air,
    /// 디딤판 위: settle이 없으면 구르는 중, 있으면 그 각도로 눕는 중 (마찰은 논리 px/s² × dpi)
    Ground { friction: f64, settle: Option<f64> },
    /// 잠듦 — 다음 바람이나 세계 변화까지 할 일 없음
    Rest,
    /// 🤹 동물(흐물이)이 머리 위에 들고 있음 — 자리는 앱이 틱마다 carry_at으로 옮김 (물리 없음)
    Carried,
    /// 🌀 회오리 속에서 돎 — 자리는 앱이 틱마다 whirl_at으로 옮김 (물리 없음). seen = 마지막으로 옮긴 시각 (물건 시계)
    Whirled { seen: f64 },
    /// 🕸️ 거미집(web = 앱의 거미집 번호)에 걸려 매달림 — rel = 거미집 모서리에서 물건 가운데까지, dir = 들어온 쪽, t = 걸린 뒤 흐른 시간
    Snagged { web: u64, rel: (f64, f64), dir: (f64, f64), t: f64 },
    /// ✨ Leviosa에 둥둥 떠 있음 — base_y = 떠오른 바닥 높이, t = 뜬 뒤 흐른 시간, len = 전체, rise = 높이 (물리 px). 끝나면 그 자리에서 내려앉음
    Floating { base_y: f64, t: f64, len: f64, rise: f64 },
}

/// 🌀 회오리가 이만큼(초) 옮기지 않으면 그 자리에서 떨어짐 (태풍이 끝나도 잃어버리지 않게)
const WHIRL_LOST_SECS: f64 = 0.3;

/// 든 물건 바닥이 손 자리보다 이만큼 (그림 높이 배) 아래 — 두 손이 물건 아랫부분을 감쌈. 거대한 것은 이 px까지만 (논리 px)
const CARRY_SINK: f64 = 0.2;
/// 든 물건이 손 아래로 내려가는 최대 거리 (논리 px)
pub const CARRY_SINK_MAX: f64 = 10.0;

/// 화면 위 물건 하나 (쓰레기·똥·장난감)
#[derive(Debug, Clone)]
pub struct Object {
    /// 그림 크기 배율 (1 = 96 논리 px 칸)
    scale: f64,
    /// 물건 종류
    pub kind: ObjectKind,
    /// x = 바닥 중심, y = 바닥 (화면 아래로 +)
    pub body: Body,
    dpi: f64,
    phase: Phase,
    /// 디딘 곳과 기준점에서 떨어진 거리
    standing: Option<(Owner, f64)>,
    /// 그림 각도 (도, 시계 방향이 +)와 회전 (도/s). 잠들 때 각도를 한 바퀴 안으로 접음
    angle: f64,
    spin: f64,
    /// 던진 비행 — 이때만 화면 끝을 넘어감 (바람·낙하는 넘어가지 않음)
    thrown: bool,
    /// 놓는 순간 아래로 던졌음 (visit::aimed_down) — 바닥에 닿으면 넘어감
    thrown_down: bool,
    gust_in: f64,
    rng: fastrand::Rng,
    clock: f64,
    /// 잡은 자리 (몸 - 커서), 잡은 시각, 가장 멀리 움직인 거리, 커서 기록 (시각, x, y)
    grab: (f64, f64),
    grabbed_at: f64,
    moved: f64,
    grabbed_from: (f64, f64),
    samples: Vec<(f64, f64, f64)>,
    postit: Postit,
    note: Option<String>,
    read_left: f64,
    note_ended: bool,
    /// ⚾ 캐치볼로 주고받은 횟수 (처음 던진 것 = 0 — 친구에게 보내는 Toss에 실림)
    rally: u32,
    /// ⚾ 바쁜 친구(집중·하우스)가 받지 않고 되돌려 보낸 것 — 흐물이가 받아치지 않음 (혼자 랠리를 세지 않게)
    returned: bool,
    /// ⚾ 친구 화면 끝에서 날아 들어왔음 (그 끝) — 흐물이가 받으면 이쪽으로 되던짐
    from: Option<Side>,
    /// 🐱 고양이가 툭 밀어 미끄러지는 중 — 끝을 넘어 떨어지면 던진 것처럼 동물에 맞음 (nudge)
    knocked: bool,
    /// ⛄ 던진 눈덩이가 무언가에 닿아 퍽 부서졌음 (take_burst로 한 번 꺼냄 — 앱이 치움)
    burst: bool,
    /// 🧹 빗자루에 밀려 날아가는 중 — 눈덩이도 내려앉을 때 부서지지 않음
    swept: bool,
    /// ⚾🖱️ 사용자가 커서로 던져 날아가는 중 (흐물이가 받아 커서 쪽으로 되던짐 — cursor_rally.rs)
    by_user: bool,
    /// ⚾🖱️ 동물이 커서 쪽으로 돌려보낸 것 (되던짐·콩·놀자) — 날아가는 중이나 내려앉고 RALLY_GRAB_SECS 안에 잡으면 횟수가 이어짐
    cursor_rally: bool,
    /// 마지막으로 디딤판에 내려앉은 시각 (물건 시계 — 날아가는 중이면 None)
    landed_at: Option<f64>,
    /// 🕸️ 화면의 거미집 (앱이 set_webs로)
    webs: Vec<WebSpot>,
    /// 🕸️ 이 시각(물건 시계)까지는 거미집에 안 걸림 (막 빠진 것)
    snag_immune_until: f64,
    /// 🕸️ 걸린 것을 잡아 빼는 중 (놓을 때 다시 안 걸리는 시간을 셈)
    unsnagging: bool,
}

impl Object {
    /// 그림 크기 배율 (똥은 싼 동물 크기를 따름, 웹뷰도 같은 값으로 그림)
    pub fn set_scale(&mut self, scale: f64) {
        self.scale = scale;
    }

    /// 그림 크기 배율
    pub fn scale(&self) -> f64 {
        self.scale
    }

    /// (x, y)에서 떨어지기 시작함. seed로 바람 시계를 정함
    pub fn new(kind: ObjectKind, x: f64, y: f64, dpi: f64, seed: u64) -> Object {
        let mut rng = fastrand::Rng::with_seed(seed);
        let gust_in = next_gust(&mut rng);
        Object {
            kind,
            body: Body { x, y, vx: 0.0, vy: 0.0 },
            dpi: dpi.max(0.01),
            scale: kind.base_scale(),
            phase: Phase::Air,
            standing: None,
            angle: 0.0,
            spin: 0.0,
            thrown: false,
            thrown_down: false,
            gust_in,
            rng,
            clock: 0.0,
            grab: (0.0, 0.0),
            grabbed_at: 0.0,
            moved: 0.0,
            grabbed_from: (x, y),
            samples: Vec::new(),
            postit: if kind.takes_postit() { Postit::None } else { Postit::Never },
            note: None,
            read_left: 0.0,
            note_ended: false,
            rally: 0,
            returned: false,
            from: None,
            knocked: false,
            burst: false,
            swept: false,
            by_user: false,
            cursor_rally: false,
            landed_at: None,
            webs: Vec::new(),
            snag_immune_until: f64::NEG_INFINITY,
            unsnagging: false,
        }
    }

    /// ⚾ 캐치볼로 주고받은 횟수 (처음 던진 것 = 0). 받는 앱이 Toss의 값을 적고, 흐물이가 되던질 때 앱이 하나 늘려 적음
    pub fn set_rally(&mut self, n: u32) {
        self.rally = n.min(crate::juggle::MAX_RALLY);
    }

    /// ⚾ 주고받은 횟수 (처음 던진 것 = 0)
    pub fn rally(&self) -> u32 {
        self.rally
    }

    /// ⚾ 바쁜 친구가 되돌려 보낸 것으로 표시함: 랠리 횟수를 지우고 흐물이가 받아치지 않음
    pub fn mark_returned(&mut self) {
        self.returned = true;
        self.rally = 0;
    }

    /// ⚾ 친구 화면 끝에서 던져져 날아 들어오는 중이면 흐물이가 받을 수 있게 (id = 앱의 물건 id). 되돌아온 것은 None
    pub fn incoming(&self, id: u64) -> Option<crate::juggle::Incoming> {
        // ⛄ 눈덩이는 받지 않는다 (닿으면 퍽)
        if self.returned || self.kind.is_snowball() || !self.in_flight() {
            return None;
        }
        // ⚾🖱️ 사용자가 커서로 던진 것: 오는 쪽이 날아온 끝 (거대한 것은 받지 않음 — notice_incoming이 거름)
        if self.by_user {
            let from = if self.body.vx < 0.0 { Side::Right } else { Side::Left };
            return Some(crate::juggle::Incoming { id, x: self.body.x, y: self.body.y, vx: self.body.vx, vy: self.body.vy, rally: self.rally, from, giant: self.kind.is_giant(), user: true });
        }
        let from = self.from.filter(|s| !visit::is_vertical(*s))?;
        if !self.in_flight() {
            return None;
        }
        Some(crate::juggle::Incoming { id, x: self.body.x, y: self.body.y, vx: self.body.vx, vy: self.body.vy, rally: self.rally, from, giant: self.kind.is_giant(), user: false })
    }

    /// 친구 화면에서 던져져 side 끝으로 날아 들어옴 (높이·속도를 이어 받음). fling은 다듬어져 있어야 함 (accept_toss)
    pub fn flown_in(kind: ObjectKind, world: &World, side: Side, fling: Fling, dpi: f64, seed: u64) -> Object {
        let mut o = Object::new(kind, 0.0, 0.0, dpi, seed);
        let (size, hw, dpi) = (o.size(), o.half_w(), o.dpi);
        let b = world.bounds;
        let g = THROW_GRAVITY_LOGICAL * dpi;
        o.body = if visit::is_vertical(side) {
            let x = visit::vertical_entry_x(world, side, Some(fling.height), hw, &mut o.rng);
            if side == Side::Up {
                Body { x, y: b.top + size, vx: fling.vx * dpi, vy: (fling.vy * dpi).max(0.0) }
            } else {
                // 아래 끝에서: 적어도 몸 둘 반은 튀어 오른다
                let pop = -(2.0 * g * 2.5 * size).sqrt();
                Body { x, y: b.bottom, vx: fling.vx * dpi, vy: (fling.vy * dpi).min(pop) }
            }
        } else {
            let (x, inward) = match side {
                Side::Left => (b.left + hw, 1.0),
                _ => (b.right - hw, -1.0),
            };
            let wa = world.work_area_at(x, b.bottom);
            let y = (wa.top + fling.height * wa.height()).clamp(wa.top + size, wa.bottom.max(wa.top + size));
            Body { x, y, vx: inward * fling.vx.abs() * dpi, vy: fling.vy * dpi }
        };
        if kind.is_giant() {
            // 🪨💩 거대한 것은 몸이 화면만 해서, 아래에서 몸 2.5배를 튀어 오르거나 위로 던져진 채 오면 곧장 위 끝을 넘어
            // 위아래 두 화면 사이를 0.1초마다 오간다 (화면 정중앙에서 깜빡깜빡). 가장 높이 솟아도 머리가 위 끝 안에 있게
            let room = ((o.body.y - size - b.top) * GIANT_ARRIVAL_HEADROOM).max(0.0);
            o.body.vy = o.body.vy.max(-(2.0 * g * room).sqrt());
        }
        o.thrown = true;
        o.from = Some(side);
        o.thrown_down = o.body.vy > 0.0 && visit::arrival_dives(o.body.vy, o.rng.f64());
        // 회전은 통신으로 오지 않는다: 들어오는 속도로 새로 돈다
        o.spin = o.spin_of_throw();
        o
    }

    /// 지금 속도로 던져졌을 때의 회전
    fn spin_of_throw(&mut self) -> f64 {
        let dir = if self.rng.bool() { 1.0 } else { -1.0 };
        throw_spin(self.kind, self.body.vx / self.dpi, self.body.vy / self.dpi, dir)
    }

    /// 그림 한 변 (물리 px)
    fn size(&self) -> f64 {
        OBJECT_LOGICAL_SIZE * self.dpi * self.scale
    }

    /// 몸 폭 절반 (물리 px)
    pub fn half_w(&self) -> f64 {
        self.size() * HALF_WIDTH_RATIO
    }

    /// 화면 배율을 바꿈 (모니터를 옮겼음 — 0.01 아래로는 내리지 않음)
    pub fn set_dpi(&mut self, dpi: f64) {
        self.dpi = dpi.max(0.01);
    }

    /// 화면 배율
    pub fn dpi(&self) -> f64 {
        self.dpi
    }

    /// 그림 각도 (도, 시계 방향이 +)
    pub fn angle(&self) -> f64 {
        self.angle
    }

    /// 회전 (도/s)
    pub fn spin(&self) -> f64 {
        self.spin
    }

    /// 잡혀 있음
    pub fn is_held(&self) -> bool {
        self.phase == Phase::Held
    }

    /// 땅에서 구르는 중 (눕기 전)
    #[cfg(test)]
    pub fn is_rolling(&self) -> bool {
        matches!(self.phase, Phase::Ground { settle: None, .. })
    }

    /// 던져져 날아가는 중 (동물에 부딪힐 수 있음). 떨어지거나 구르거나 들린 것은 아님
    pub fn in_flight(&self) -> bool {
        self.phase == Phase::Air && self.thrown
    }

    /// 그려진 크기의 부딪힘 상자 (반 폭, 높이, 물리 px) — 똥은 작게 그려짐
    pub fn hit_box(&self) -> (f64, f64) {
        let hw = self.half_w();
        (hw, 2.0 * hw)
    }

    /// 동물에 부딪혀 되튐 (away = 맞은 몸에서 멀어지는 쪽). 비행은 이어지지만 아래 친구를 노린 것은 잊음
    pub fn bounce_off(&mut self, face: Face, away: f64) {
        let (vx, vy) = rebound(face, self.body.vx, self.body.vy, away, self.dpi);
        self.body.vx = vx;
        self.body.vy = vy;
        self.thrown_down = false;
        self.spin *= -tumble::WALL_SPIN_KEEP;
    }

    /// 그림이 도는 축의 화면 자리 (물리 px): 창은 이곳을 가운데로 놓음. 가장 낮은 점이 언제나 body.y에 옴
    pub fn pivot(&self) -> (f64, f64) {
        let shape = shape_of(self.kind);
        (self.body.x + shape.pivot_offset_x() * self.scale * self.dpi, self.body.y - shape.drop_at(self.angle) * self.scale * self.dpi)
    }

    /// 화면 점 (x, y) (물리 px)에 그림이 그려져 있음: 축을 중심으로 돌린 그림을 되돌려 칸 좌표로 보고 볼록 껍질 안인지 봄
    pub fn covers(&self, x: f64, y: f64) -> bool {
        let shape = shape_of(self.kind);
        let k = OBJECT_LOGICAL_SIZE / 128.0 * self.scale * self.dpi;
        if k <= 0.0 || !k.is_finite() {
            return false;
        }
        let (px, py) = self.pivot();
        let (dx, dy) = (x - px, y - py);
        // 웹뷰는 시계 방향으로 angle만큼 돌려 그린다 — 거꾸로 돌린다
        let (sin, cos) = self.angle.to_radians().sin_cos();
        let (ux, uy) = (dx * cos + dy * sin, -dx * sin + dy * cos);
        shape.contains(shape.pivot.0 + ux / k, shape.pivot.1 + uy / k)
    }

    /// 창이 (x, y)의 클릭을 아래(바탕·다른 앱)로 보내야 함: 거대한 물건의 투명한 곳만 — 창이 화면을 크게 덮으니까.
    /// 들고 있는 동안은 아님. 작은 물건은 늘 받음
    /// 📌 장난감도: 바닥(작업표시줄 바로 위)에 놓일 때 창 아래쪽 투명한 곳이 작업표시줄을 막지 않게
    pub fn passes_clicks_at(&self, x: f64, y: f64) -> bool {
        // 날아가는 거대한 것은 모든 클릭을 보낸다: 모양이 커서 밑을 계속 지나가 클릭 통과가 깜빡거리지 않게 (Windows는 바꿀 때마다 창을 다시 그린다)
        let flying = self.kind.is_giant() && self.phase == Phase::Air;
        (self.kind.is_giant() || self.kind.is_toy()) && !self.is_held() && (flying || !self.covers(x, y))
    }

    /// 🤹 동물이 들고 있음
    pub fn is_carried(&self) -> bool {
        self.phase == Phase::Carried
    }

    /// 동물이 주울 수 있게 바닥에 놓여 있음 (굴러가는 중이어도) — 들렸거나 날거나 누가 들고 있으면 None
    pub fn spot(&self, id: u64) -> Option<crate::juggle::ObjectSpot> {
        if !matches!(self.phase, Phase::Ground { .. } | Phase::Rest) {
            return None;
        }
        Some(crate::juggle::ObjectSpot { id, x: self.body.x, y: self.body.y, owner: self.owner()?, half_w: self.half_w(), giant: self.kind.is_giant(), local: self.kind.is_toy() })
    }

    /// ⚾🖱️ 사용자가 커서로 던져 날아가는 중 (동물이 콩 받아칠 수 있음)
    pub fn is_user_throw(&self) -> bool {
        self.by_user && self.in_flight()
    }

    /// ⚾🖱️ 동물이 커서 쪽으로 돌려보냈음 (앱이 되던진 뒤 부름): 잡으면 횟수가 이어짐
    pub fn mark_cursor_rally(&mut self) {
        self.cursor_rally = true;
        self.landed_at = None;
    }

    /// ⚾🖱️ 커서 랠리가 아직 살아 있음: 동물이 돌려보냈고, 날아가는 중이거나 내려앉은 지 RALLY_GRAB_SECS 안
    fn cursor_rally_live(&self) -> bool {
        self.cursor_rally && self.landed_at.is_none_or(|t| self.clock - t <= crate::cursor_rally::RALLY_GRAB_SECS)
    }

    /// ⚾🖱️ 동물이 머리로 콩 받아쳐 (vx, vy)로 돌려보냄 (물리 px/s): n번째로 적고 커서 랠리로 표시함 (던진 것처럼 날아감)
    pub fn boop_back(&mut self, vx: f64, vy: f64, n: u32) {
        if self.is_held() {
            return;
        }
        self.launch(vx, vy);
        self.set_rally(n);
        self.mark_cursor_rally();
    }

    /// 동물 손에 들려 (hx, hy)에 손이 있음: 바닥이 손 조금 아래 (똑바로 세워 듦). 사용자가 잡고 있으면 그대로
    pub fn carry_at(&mut self, hx: f64, hy: f64) {
        if self.is_held() {
            return;
        }
        self.by_user = false;
        let (_, height) = self.hit_box();
        self.phase = Phase::Carried;
        self.standing = None;
        self.from = None;
        self.thrown = false;
        self.thrown_down = false;
        self.angle = 0.0;
        self.spin = 0.0;
        self.body = Body { x: hx, y: hy + (CARRY_SINK * height).min(CARRY_SINK_MAX * self.dpi), vx: 0.0, vy: 0.0 };
    }

    /// 들고 있던 동물이 (vx, vy)로 던졌음 (물리 px/s) — 그 뒤는 사람이 던진 것과 같음 (화면 끝을 넘으면 친구에게, 동물에 맞으면 콩)
    pub fn launch(&mut self, vx: f64, vy: f64) {
        if self.is_held() {
            return;
        }
        self.from = None;
        self.by_user = false;
        self.cursor_rally = false;
        self.landed_at = None;
        self.body.vx = vx;
        self.body.vy = vy;
        self.standing = None;
        self.phase = Phase::Air;
        self.thrown = true;
        self.swept = false;
        self.thrown_down = visit::aimed_down(vx, vy, self.dpi);
        self.spin = self.spin_of_throw();
    }

    /// 🐱 바닥에 놓인 것을 dir(±1) 쪽으로 툭 밂: 바닥을 따라 reach(물리 px)쯤 미끄러지고, 디딤판 끝을 넘으면 떨어짐 —
    /// 떨어지는 동안은 던진 것처럼 동물에 맞음 (친구 화면으로는 넘어가지 않음). 바닥에 있지 않으면 false
    pub fn nudge(&mut self, dir: f64, reach: f64) -> bool {
        if !matches!(self.phase, Phase::Ground { .. } | Phase::Rest) || reach <= 0.0 {
            return false;
        }
        let friction = heft_of(self.kind).friction * self.dpi;
        self.body.vx = dir.signum() * (2.0 * friction * reach).sqrt();
        self.spin = heft_of(self.kind).rolling_spin(self.body.vx / self.dpi);
        self.knocked = true;
        self.phase = Phase::Ground { friction, settle: None };
        true
    }

    /// 🦖 쿵! 에 바닥에 놓인 것이 vy(논리 px/s, 위로)로 톡 튀어 올랐다가 그 자리에 내려앉음 (던진 것이 아님). 바닥에 있지 않으면 false
    pub fn pop(&mut self, vy: f64) -> bool {
        if !matches!(self.phase, Phase::Ground { .. } | Phase::Rest) {
            return false;
        }
        self.standing = None;
        self.thrown = false;
        self.knocked = false;
        self.body.vx = 0.0;
        self.body.vy = -vy.abs() * self.dpi;
        let dir = if self.rng.bool() { 1.0 } else { -1.0 };
        self.spin = dir * wobble_spin(vy.abs());
        self.phase = Phase::Air;
        true
    }

    /// ✨ Leviosa: 바닥에 놓인 것이 rise (물리 px)만큼 천천히 떠올라 살랑 떠 있다가 secs초 뒤 그 자리에 내려앉음 (던진 게 아님).
    /// 바닥에 있지 않으면 false
    pub fn levitate(&mut self, rise: f64, secs: f64) -> bool {
        if !matches!(self.phase, Phase::Ground { .. } | Phase::Rest) || rise <= 0.0 || secs <= 0.0 {
            return false;
        }
        self.standing = None;
        self.thrown = false;
        self.thrown_down = false;
        self.knocked = false;
        self.landed_at = None;
        self.body.vx = 0.0;
        self.body.vy = 0.0;
        self.spin = 0.0;
        self.phase = Phase::Floating { base_y: self.body.y, t: 0.0, len: secs, rise };
        true
    }

    /// 🪶 막 만든 장난감 아이콘을 지금 자리 (바닥)에서 바로 둥둥 띄움 (Leviosa로 작업표시줄에서 뽑은 것 — 앱 deliver_toy). 잡혀 있으면 false
    pub fn float_up(&mut self, rise: f64, secs: f64) -> bool {
        if matches!(self.phase, Phase::Held) || rise <= 0.0 || secs <= 0.0 {
            return false;
        }
        self.phase = Phase::Rest;
        self.levitate(rise, secs)
    }

    /// ✨ Leviosa에 떠 있음
    pub fn is_floating(&self) -> bool {
        matches!(self.phase, Phase::Floating { .. })
    }

    /// 들고 있던 동물이 놓쳤음 (잡혔거나 다른 일로): 그 자리에서 떨어짐
    pub fn let_fall(&mut self) {
        if self.phase == Phase::Carried {
            self.start_fall();
        }
    }

    /// 🌀 돌풍에 날리거나 회오리에 빨려 들 수 있음: 바닥에 놓인 작은 것 (거대한 것·잡은 것·동물이 든 것·나는 중은 아님)
    pub fn is_loose(&self) -> bool {
        matches!(self.phase, Phase::Ground { .. } | Phase::Rest) && !self.kind.is_giant()
    }

    /// 🌀 허리케인급 회오리만 들어 올릴 수 있음: 바닥에 놓인 거대한 것 (돌풍엔 그대로)
    pub fn is_heavy_loose(&self) -> bool {
        matches!(self.phase, Phase::Ground { .. } | Phase::Rest) && self.kind.is_giant()
    }

    /// 🌀 회오리 속에서 도는 중
    pub fn is_whirled(&self) -> bool {
        matches!(self.phase, Phase::Whirled { .. })
    }

    /// 🌀 회오리가 (x, y = 바닥 가운데, 바닥)에 붙들어 angle(도)로 돌림. 바닥에 놓였거나 이미 도는 것만 (아니면 false).
    /// 거대한 것은 허리케인급만 고름 (typhoon.rs)
    pub fn whirl_at(&mut self, x: f64, y: f64, angle: f64) -> bool {
        if !self.is_loose() && !self.is_heavy_loose() && !self.is_whirled() {
            return false;
        }
        self.phase = Phase::Whirled { seen: self.clock };
        self.standing = None;
        self.thrown = false;
        self.thrown_down = false;
        self.knocked = false;
        self.from = None;
        self.by_user = false;
        self.cursor_rally = false;
        self.landed_at = None;
        self.body = Body { x, y, vx: 0.0, vy: 0.0 };
        self.angle = angle;
        self.spin = 0.0;
        true
    }

    /// 🌀 돌풍·회오리가 (vx, vy, 물리 px/s)로 휙 날림 — 던진 게 아니라 친구 화면으로는 넘어가지 않음 (친구에게는 launch).
    /// 바닥에 놓였거나 회오리 속인 작은 것만 (아니면 false)
    pub fn blow(&mut self, vx: f64, vy: f64) -> bool {
        if !self.is_loose() && !self.is_whirled() {
            return false;
        }
        self.standing = None;
        self.thrown = false;
        self.thrown_down = false;
        self.knocked = false;
        self.landed_at = None;
        self.body.vx = vx;
        self.body.vy = vy;
        self.spin = self.spin_of_throw();
        self.phase = Phase::Air;
        true
    }

    /// 다음 바람이나 세계 변화까지 할 일이 없음 (창도 웹뷰도 건드리지 않음)
    pub fn is_asleep(&self) -> bool {
        self.phase == Phase::Rest && self.postit != Postit::Reading
    }

    /// 놓인 디딤판 (공중이면 None)
    pub fn owner(&self) -> Option<Owner> {
        self.standing.map(|(o, _)| o)
    }

    /// 똥이 다 말라 사라질 때임 (들고 있는 동안은 기다림)
    /// 📌 장난감도 TOY_LIFE_SECS 뒤 저절로 사라짐 (흐물이가 도로 넣지 못했으면 — 쌓이지 않게)
    pub fn expired(&self) -> bool {
        let life = if self.kind.is_poop() {
            crate::poop::LIFE_SECS
        } else if self.kind.is_toy() {
            crate::taskbar::TOY_LIFE_SECS
        } else if self.kind.is_snowball() {
            crate::snowball::MELT_SECS
        } else {
            return false;
        };
        self.clock >= life && !self.is_held() && !self.is_carried()
    }

    /// 바닥에 놓인 똥의 자리 (동물이 알아챔). 들렸거나 날아가는 중이면 None
    pub fn poop_spot(&self, id: u64) -> Option<crate::poop::PoopSpot> {
        if !self.kind.is_poop() || !matches!(self.phase, Phase::Ground { .. } | Phase::Rest) {
            return None;
        }
        Some(crate::poop::PoopSpot { id, x: self.body.x, y: self.body.y, owner: self.owner()? })
    }

    /// 빗자루(vx, vy = 빗자루 속도, 물리 px/s)가 바닥의 물건을 쓸었음: 그 쪽으로 던진 것과 같음 — 친구가 연결된 화면 끝을 넘으면 평소처럼 날아감
    /// 살살 쓸어도 조금은 밀리고 살짝 뜸. 거대한 것은 무거워서 느리게, 눈덩이는 부서지지 않고 굴러감. 날아가는 중·들려 있음·동물이 든 것·
    /// 포스트잇을 읽는 중이면 그대로 (내려앉아야 다시 쓸림). 쓸렸으면 true
    pub fn sweep(&mut self, vx: f64, vy: f64) -> bool {
        let reading = self.postit == Postit::Reading;
        if reading || !matches!(self.phase, Phase::Ground { .. } | Phase::Rest) || (vx == 0.0 && vy == 0.0) {
            return false;
        }
        let (mut vx, mut vy) = throw_velocity(vx, vy, self.dpi, THROW_LIFT);
        if vx != 0.0 && vx.abs() < SWEEP_MIN_VX * self.dpi {
            vx = vx.signum() * SWEEP_MIN_VX * self.dpi;
        }
        if self.kind.is_giant() {
            vx *= GIANT_SWEEP;
            vy *= GIANT_SWEEP;
        }
        self.thrown_down = visit::aimed_down(vx, vy, self.dpi);
        if !self.thrown_down {
            vy = vy.min(-SWEEP_POP * self.dpi);
        }
        self.body.vx = vx;
        self.body.vy = vy;
        self.standing = None;
        self.thrown = true;
        self.swept = true;
        self.phase = Phase::Air;
        self.spin = self.spin_of_throw();
        true
    }

    /// ⛄ 던진 눈덩이가 퍽 부서짐 (동물에 맞았거나 내려앉았음): 더는 날지도 맞지도 않음. 앱은 take_burst로 알고 치움
    pub fn splat(&mut self) {
        self.burst = true;
        self.thrown = false;
        self.thrown_down = false;
        self.body.vx = 0.0;
        self.body.vy = 0.0;
        self.spin = 0.0;
        self.phase = Phase::Rest;
    }

    /// ⛄ 방금 퍽 부서졌음 (한 번만 참)
    pub fn take_burst(&mut self) -> bool {
        std::mem::take(&mut self.burst)
    }

    /// 윗면에 앉아 있는 창
    pub fn ridden_window(&self) -> Option<u64> {
        match self.owner() {
            Some(Owner::Window(id) | Owner::Inside(id)) => Some(id),
            _ => None,
        }
    }

    // ---- 포인터 ----

    /// 잡았음 (cx, cy = 커서)
    pub fn pointer_down(&mut self, cx: f64, cy: f64) {
        self.from = None;
        // 🕸️ 걸린 것을 잡아 뺌 — 놓은 뒤 잠깐은 다시 안 걸림
        self.unsnagging = self.is_snagged();
        if self.unsnagging {
            self.snag_immune_until = self.clock + web::SNAG_IMMUNE_SECS;
        }
        // ⚾🖱️ 동물이 돌려보낸 것을 금방 잡았으면 횟수가 이어짐 (아니면 새로 1부터)
        if !self.cursor_rally_live() {
            self.rally = 0;
        }
        self.cursor_rally = false;
        self.by_user = false;
        self.phase = Phase::Held;
        self.standing = None;
        self.thrown = false;
        self.body.vx = 0.0;
        self.body.vy = 0.0;
        self.spin = 0.0;
        self.grab = (self.body.x - cx, self.body.y - cy);
        self.grabbed_at = self.clock;
        self.grabbed_from = (cx, cy);
        self.moved = 0.0;
        self.samples.clear();
        self.samples.push((self.clock, cx, cy));
    }

    /// 끄는 중 커서 움직임: 잡은 자리를 따라 옮기고 던지는 속도를 기록함
    pub fn pointer_move(&mut self, cx: f64, cy: f64) {
        if !self.is_held() {
            return;
        }
        self.body.x = cx + self.grab.0;
        self.body.y = cy + self.grab.1;
        self.moved = self.moved.max((cx - self.grabbed_from.0).hypot(cy - self.grabbed_from.1));
        self.samples.push((self.clock, cx, cy));
        let keep_from = self.clock - VELOCITY_WINDOW;
        self.samples.retain(|s| s.0 >= keep_from);
    }

    /// 놓았음: 거의 안 움직이고 금방 놓으면 클릭(그 자리에 내려앉음), 아니면 던지기
    pub fn pointer_up(&mut self) -> Release {
        if !self.is_held() {
            return Release::Throw;
        }
        self.phase = Phase::Air;
        if std::mem::take(&mut self.unsnagging) {
            self.snag_immune_until = self.clock + web::SNAG_IMMUNE_SECS;
        }
        if self.moved < CLICK_SLOP * self.dpi && self.clock - self.grabbed_at < CLICK_SECS {
            return Release::Click;
        }
        let (vx, vy) = crate::physics::release_velocity(self.samples.iter());
        let (vx, vy) = throw_velocity(vx, vy, self.dpi, THROW_LIFT);
        self.body.vx = vx;
        self.body.vy = vy;
        self.thrown = true;
        self.swept = false;
        self.thrown_down = visit::aimed_down(vx, vy, self.dpi);
        self.spin = self.spin_of_throw();
        // ⚾🖱️ 커서로 던짐: 흐물이가 받으러 옴 (거대한 것·눈덩이는 아님)
        self.by_user = !self.kind.is_giant() && !self.kind.is_snowball();
        self.landed_at = None;
        Release::Throw
    }

    // ---- 매 틱 ----

    /// gusts_ok = 바람이 불어도 됨 (object::gusts_ok). 던져서 화면 끝을 넘어가면 Exit
    pub fn tick(&mut self, dt: f64, world: &World, edges: ExitEdges, gusts_ok: bool) -> Option<Exit> {
        self.clock += dt;
        self.tick_note(dt);
        match self.phase {
            Phase::Held | Phase::Carried => None,
            Phase::Whirled { seen } => {
                if self.clock - seen > WHIRL_LOST_SECS {
                    self.start_fall();
                }
                None
            }
            Phase::Snagged { web, rel, dir, t } => {
                self.tick_snag(dt, web, rel, dir, t);
                None
            }
            Phase::Floating { base_y, t, len, rise } => {
                let t = t + dt;
                if t >= len {
                    self.body.y = base_y;
                    self.start_fall();
                } else {
                    self.body.y = base_y - crate::wizard::float_height(t, len, rise);
                    self.phase = Phase::Floating { base_y, t, len, rise };
                }
                None
            }
            Phase::Air => {
                let v = (self.body.vx, self.body.vy);
                let exit = self.tick_air(dt, world, edges);
                if exit.is_none() {
                    self.try_snag(v);
                }
                exit
            }
            Phase::Ground { friction, settle } => {
                self.tick_ground(dt, world, friction, settle);
                if self.body.vx != 0.0 {
                    self.try_snag((self.body.vx, 0.0));
                }
                None
            }
            Phase::Rest => {
                self.gust_in -= dt;
                if self.gust_in <= 0.0 {
                    self.gust_in = next_gust(&mut self.rng);
                    if gusts_ok {
                        let dir = if self.rng.bool() { 1.0 } else { -1.0 };
                        self.start_gust(dir);
                    }
                }
                None
            }
        }
    }

    /// 🕸️ 앱이 (틱마다) 이 화면의 거미집을 알려 줌 (물리 px)
    pub fn set_webs(&mut self, webs: &[WebSpot]) {
        self.webs.clear();
        self.webs.extend_from_slice(webs);
    }

    /// 🕸️ 거미집에 걸려 매달려 있음
    pub fn is_snagged(&self) -> bool {
        matches!(self.phase, Phase::Snagged { .. })
    }

    /// 🕸️ 그림 가운데의 바닥에서 높이 (물리 px)
    fn mid_height(&self) -> f64 {
        0.5 * self.hit_box().1
    }

    /// 🕸️ 날거나 떨어지거나 구르는 중 (v = 이번 틱 속도): 가운데가 보이는 거미집 안이면 무조건 걸림 — 막 빠진 것은 안 걸림
    fn try_snag(&mut self, v: (f64, f64)) {
        if self.webs.is_empty() || self.clock < self.snag_immune_until || !matches!(self.phase, Phase::Air | Phase::Ground { .. }) {
            return;
        }
        let (cx, cy) = (self.body.x, self.body.y - self.mid_height());
        let Some(w) = self.webs.iter().find(|w| !w.hidden && web::in_web(cx, cy, w.x, w.y, w.corner, w.size)).copied() else { return };
        self.phase = Phase::Snagged { web: w.id, rel: (cx - w.x, cy - w.y), dir: web::snag_dir(v.0, v.1), t: 0.0 };
        self.standing = None;
        self.thrown = false;
        self.thrown_down = false;
        self.knocked = false;
        self.swept = false;
        self.by_user = false;
        self.cursor_rally = false;
        self.landed_at = None;
        self.body.vx = 0.0;
        self.body.vy = 0.0;
        self.spin = 0.0;
    }

    /// 🕸️ 매달린 한 틱: 거미집 자리를 따라가며 "팅~" 출렁 뒤 살랑살랑. 거미집이 사라졌으면 그 자리에서 떨어짐
    fn tick_snag(&mut self, dt: f64, id: u64, rel: (f64, f64), dir: (f64, f64), t: f64) {
        let Some(w) = self.webs.iter().find(|w| w.id == id).copied() else {
            self.snag_immune_until = self.clock + web::SNAG_IMMUNE_SECS;
            self.start_fall();
            return;
        };
        let t = t + dt;
        let amp = web::SNAG_STRETCH_OBJECT * self.hit_box().1;
        let (sx, sy) = web::snag_spring(dir, amp, t);
        let sway = web::snag_sway(amp, t);
        let mid = self.mid_height();
        self.body.x = w.x + rel.0 + sx + sway;
        self.body.y = w.y + rel.1 + sy + mid;
        self.phase = Phase::Snagged { web: id, rel, dir, t };
    }

    /// 바람이 붊 (dir = ±1). 잠들어 있을 때만. 던졌을 때와 같은 구르기를 살살
    pub fn start_gust(&mut self, dir: f64) {
        if self.phase != Phase::Rest || self.kind.gust_reach().1 <= 0.0 {
            return;
        }
        let heft = heft_of(self.kind);
        let (lo, hi) = self.kind.gust_reach();
        let reach = lo + self.rng.f64() * (hi - lo);
        let friction = heft.friction * tumble::GUST_EASE * self.dpi;
        self.body.vx = dir * (2.0 * friction * reach * self.dpi).sqrt();
        self.spin = heft.rolling_spin(self.body.vx / self.dpi);
        self.phase = Phase::Ground { friction, settle: None };
    }

    /// 떨어지기 시작함 (디딤판을 놓음)
    fn start_fall(&mut self) {
        self.standing = None;
        self.body.vy = 0.0;
        self.thrown = false;
        self.phase = Phase::Air;
    }

    /// 디딤판 위 한 틱: 구르다 마찰로 멈추고 settle 각도로 누움
    fn tick_ground(&mut self, dt: f64, world: &World, friction: f64, mut settle: Option<f64>) {
        let anchor = self.standing.and_then(|(owner, rel_x)| world.anchor(owner).map(|(ax, _)| (owner, rel_x, ax)));
        let Some((owner, rel_x, ax)) = anchor else {
            self.start_fall();
            return;
        };
        let v0 = self.body.vx;
        let mut v = (v0.abs() - friction * dt).max(0.0) * v0.signum();
        if v0 == 0.0 {
            v = 0.0;
        }
        let (lo, hi) = (world.bounds.left + self.half_w(), world.bounds.right - self.half_w());
        let mut x = ax + rel_x + v * dt;
        if x < lo || x > hi {
            x = x.clamp(lo, hi.max(lo));
            v = 0.0;
        }
        self.body.x = x;
        self.body.vx = v;
        let Some(piece) = world.piece_at(owner, x).copied() else {
            // 디딤판 끝을 지나 돌면서 떨어진다 (바람 때문이어도 — 화면 끝은 위에서 막았다)
            self.start_fall();
            // 🐱 고양이가 밀어 떨어뜨렸다: 떨어지며 아래 동물에 맞을 수 있다
            self.thrown = std::mem::take(&mut self.knocked);
            return;
        };
        self.body.y = piece.y;
        self.standing = Some((owner, x - ax));
        let lying = match settle {
            None => {
                self.roll(dt);
                if v == 0.0 && self.spin.abs() < tumble::SETTLE_SPIN {
                    settle = Some(shape_of(self.kind).rest_angle(self.angle));
                }
                false
            }
            Some(target) => self.flop_toward(dt, target),
        };
        if v == 0.0 {
            self.knocked = false;
        }
        if v == 0.0 && lying {
            self.angle = (self.angle + 180.0).rem_euclid(360.0) - 180.0;
            self.spin = 0.0;
            self.phase = Phase::Rest;
        } else {
            self.phase = Phase::Ground { friction, settle };
        }
    }

    /// 굴러가는 한 틱: 땅이 회전을 구르는 빠르기로 맞추고, 모서리를 넘을 땐 느려졌다 넘어가며 빨라짐
    fn roll(&mut self, dt: f64) {
        let heft = heft_of(self.kind);
        let rolling = heft.rolling_spin(self.body.vx / self.dpi);
        self.spin += (rolling - self.spin) * (1.0 - (-heft.grip * dt).exp());
        self.spin -= tumble::CLUNK * shape_of(self.kind).slope_at(self.angle) * dt;
        self.angle += self.spin * dt;
    }

    /// 눕는 한 틱: 면 쪽으로 넘어가 바닥에 부딪히고, 살짝 되튀다 멈춤 (바닥을 뚫고 지나가지 않음). 다 누웠으면 true
    fn flop_toward(&mut self, dt: f64, target: f64) -> bool {
        let off = self.angle - target;
        if off.abs() < 0.2 && self.spin.abs() < tumble::TINY_SPIN {
            self.angle = target;
            self.spin = 0.0;
            return true;
        }
        let side = if off != 0.0 { off.signum() } else { self.spin.signum() };
        self.spin -= side * tumble::FLOP * dt;
        let next = self.angle + self.spin * dt;
        if self.spin * side < 0.0 && (next - target) * side <= 0.0 {
            self.angle = target;
            self.spin = -self.spin * tumble::REBOUND;
            if self.spin.abs() < tumble::STOP_SPIN {
                self.spin = 0.0;
                return true;
            }
            return false;
        }
        self.angle = next;
        false
    }

    /// 공중 한 틱: 날다가 화면 끝을 넘으면 Exit, 디딤판에 닿으면 착지
    fn tick_air(&mut self, dt: f64, world: &World, edges: ExitEdges) -> Option<Exit> {
        if let Some(exit) = self.fling_out(dt, world, edges) {
            return Some(exit);
        }
        let prev_y = self.body.y;
        let g = THROW_GRAVITY_LOGICAL * self.dpi;
        let (size, hw) = (self.size(), self.half_w());
        let vx_before = self.body.vx;
        step_air(&mut self.body, dt, &world.bounds, hw, size, g);
        if self.body.vx * vx_before < 0.0 {
            // 벽에 부딪혔다: 반대로 돈다
            self.spin *= -tumble::WALL_SPIN_KEEP;
        }
        self.angle += self.spin * dt;
        // 아래 친구에게 던졌으면 창 윗면을 지나 바닥까지 간다
        let to_ground = self.thrown && self.thrown_down && edges.down;
        let landing = world.fall_landing(self.body.x, prev_y, self.body.y, self.body.vy, to_ground);
        if let Some(s) = landing {
            self.body.y = s.y;
            if self.thrown && self.kind.is_snowball() && !std::mem::take(&mut self.swept) {
                // ⛄ 던진 눈덩이는 닿은 곳에서 퍽 (빗자루에 밀린 것은 그냥 내려앉음)
                self.splat();
                return None;
            }
            self.swept = false;
            self.thrown = false;
            if self.body.vy > BOUNCE_MIN_VY * self.dpi {
                self.bounce();
                return None;
            }
            let landed_at = self.body.vy;
            self.body.vy = 0.0;
            self.by_user = false;
            self.landed_at = Some(self.clock);
            let ax = world.anchor(s.owner).map_or(0.0, |(ax, _)| ax);
            self.standing = Some((s.owner, self.body.x - ax));
            let friction = heft_of(self.kind).friction * self.dpi;
            self.phase = if self.spin == 0.0 {
                // 살며시 내려앉았다: 그 자세로 살짝 까딱하고 만다. 공중에서 붙잡아 기운 채 놓았으면 가까운 면으로 눕는다 (처음 모습 0도는 그대로 둔다)
                let dir = if self.rng.bool() { 1.0 } else { -1.0 };
                self.spin = dir * wobble_spin(landed_at / self.dpi);
                let settle = if self.angle == 0.0 { 0.0 } else { shape_of(self.kind).rest_angle(self.angle) };
                Phase::Ground { friction, settle: Some(settle) }
            } else {
                Phase::Ground { friction, settle: None }
            };
        } else if self.body.y > world.bounds.bottom + size * 4.0 {
            // 화면 밖으로 사라짐 → 위에서 다시 떨어진다
            let b = world.bounds;
            self.body = Body { x: self.body.x.clamp(b.left + hw, (b.right - hw).max(b.left + hw)), y: b.top + size, vx: 0.0, vy: 0.0 };
        }
        None
    }

    /// 바닥을 치고 튀어 오름: 도는 물건이면 회전이 구르는 쪽으로 쏠리고, 가벼운 것은 제멋대로 튐
    fn bounce(&mut self) {
        let heft = heft_of(self.kind);
        self.body.vy *= -heft.bounce;
        self.body.vx *= heft.bounce_vx;
        if self.spin != 0.0 {
            let rolling = heft.rolling_spin(self.body.vx / self.dpi);
            let kick = (self.rng.f64() * 2.0 - 1.0) * heft.jitter;
            self.spin = (self.spin * heft.bounce_spin + rolling * (1.0 - heft.bounce_spin) + kick).clamp(-heft.max_spin, heft.max_spin);
            if self.spin == 0.0 {
                // 회전이 딱 0이면 살며시 내려앉은 것으로 치므로, 구르던 물건은 0이 되지 않게 한다
                self.spin = f64::MIN_POSITIVE;
            }
        }
    }

    /// 던진 비행이 친구가 연결된 화면 끝을 넘으면 그대로 넘어감 (동물과 같은 규칙, brain_fling.rs)
    fn fling_out(&mut self, dt: f64, world: &World, edges: ExitEdges) -> Option<Exit> {
        if !self.thrown {
            return None;
        }
        let b = world.bounds;
        let (hw, size, dpi) = (self.half_w(), self.size(), self.dpi);
        let Body { x, y, vx, vy } = self.body;
        // next_y는 이번 틱의 중력까지 더한다: 이 틱에 바닥을 지나면 내려앉기 전에 넘어가야 한다
        let (next_x, next_y) = (x + vx * dt, y + (vy + THROW_GRAVITY_LOGICAL * dpi * dt) * dt);
        // 위/아래 끝은 이 줄의 끝 (모니터마다 작업표시줄·크기가 달라도 위아래에 다른 모니터가 없으면 넘어간다)
        let col = visit::column_bounds(world, x);
        let exit = if next_x < b.left + hw && vx < 0.0 && edges.left {
            Some(Side::Left)
        } else if next_x > b.right - hw && vx > 0.0 && edges.right {
            Some(Side::Right)
        } else if vy < 0.0 && edges.up && col.is_some_and(|c| next_y < c.top + size) {
            Some(Side::Up)
        } else if edges.down && col.is_some_and(|c| next_y >= c.bottom) && visit::falls_through(self.thrown_down, vy, visit::FLING_DOWN_MIN_SPEED, dpi) {
            Some(Side::Down)
        } else {
            None
        };
        let side = exit?;
        let fling = visit::fling_across(world, side, &self.body, dpi)?;
        self.thrown = false;
        Some(Exit { side, fling })
    }

    /// 세계가 바뀌었음 (창이 움직임·사라짐 등): 앉은 창을 따라가거나 떨어지기 시작함. 잠든 물건에만 일이 있음. 움직였으면 true
    pub fn reseat(&mut self, world: &World) -> bool {
        if self.phase != Phase::Rest {
            return false;
        }
        let Some((owner, rel_x)) = self.standing else {
            self.start_fall();
            return true;
        };
        let Some((ax, _)) = world.anchor(owner) else {
            self.start_fall();
            return true;
        };
        let x = ax + rel_x;
        let Some(piece) = world.piece_at(owner, x).copied() else {
            self.body.x = x;
            self.start_fall();
            return true;
        };
        let moved = x != self.body.x || piece.y != self.body.y;
        self.body.x = x;
        self.body.y = piece.y;
        moved
    }

    // ---- 포스트잇 ----

    /// 포스트잇 상태
    pub fn postit(&self) -> Postit {
        self.postit
    }

    /// 물건에 노란 쪽지 그림이 붙어 있음
    pub fn has_sticker(&self) -> bool {
        matches!(self.postit, Postit::Mine | Postit::Unread)
    }

    /// 내가 쓴 포스트잇을 붙임 (다듬은 글자가 비었거나 친구 것이 붙어 있으면 false)
    pub fn attach_note(&mut self, raw: &str) -> bool {
        if !matches!(self.postit, Postit::None | Postit::Mine) {
            return false;
        }
        let Some(note) = visit::sanitize_note(raw) else { return false };
        self.note = Some(note);
        self.postit = Postit::Mine;
        true
    }

    /// 내가 붙인 포스트잇을 뗌
    pub fn peel_note(&mut self) -> bool {
        if self.postit != Postit::Mine {
            return false;
        }
        self.note = None;
        self.postit = Postit::None;
        true
    }

    /// 친구가 붙여 보낸 포스트잇 (accept_toss로 다듬은 글자)
    pub fn receive_note(&mut self, note: Option<String>) {
        if self.postit == Postit::Never {
            return;
        }
        self.postit = if note.is_some() { Postit::Unread } else { Postit::None };
        self.note = note;
    }

    /// 눌렀음: 받은 포스트잇이면 글자를 딱 한 번 내주고 10초를 재기 시작함. 글자는 여기서 지움 — 다시 읽을 수 없음
    pub fn read_note(&mut self) -> Option<String> {
        if self.postit != Postit::Unread {
            return None;
        }
        self.postit = Postit::Reading;
        self.read_left = NOTE_READ_SECS;
        self.note.take()
    }

    /// 던져 보낼 때 같이 갈 글자 (읽기 시작한 포스트잇은 따라가지 않음)
    pub fn note_to_send(&self) -> Option<String> {
        if self.has_sticker() {
            self.note.clone()
        } else {
            None
        }
    }

    /// 포스트잇을 읽는 중이면 남은 시간을 줄이고, 다 되면 영영 지움
    fn tick_note(&mut self, dt: f64) {
        if self.postit != Postit::Reading {
            return;
        }
        self.read_left -= dt;
        if self.read_left <= 0.0 {
            self.postit = Postit::None;
            self.note_ended = true;
        }
    }

    /// 읽는 10초가 끝났음 (한 번만 참)
    pub fn take_note_ended(&mut self) -> bool {
        std::mem::take(&mut self.note_ended)
    }
}

#[cfg(test)]
impl Object {
    /// 테스트: 커서로 던진 것처럼 (vx, vy)로 날아감
    pub(crate) fn force_user_throw(&mut self, vx: f64, vy: f64) {
        self.force_throw(vx, vy);
        self.by_user = true;
    }

    /// 테스트: 지금 자리에서 (vx, vy)로 던져져 날아감
    pub(crate) fn force_throw(&mut self, vx: f64, vy: f64) {
        self.body.vx = vx;
        self.body.vy = vy;
        self.standing = None;
        self.phase = Phase::Air;
        self.thrown = true;
        self.swept = false;
    }
}

/// 바람이 불어도 되는 때: 모두 숨겨 두지 않았고 사용자가 자리에 있음
pub fn gusts_ok(hidden: bool, away: bool) -> bool {
    !hidden && !away
}

/// 다음 바람까지 시간을 뽑음 (GUST_EVERY_SECS 사이)
fn next_gust(rng: &mut fastrand::Rng) -> f64 {
    GUST_EVERY_SECS.0 + rng.f64() * (GUST_EVERY_SECS.1 - GUST_EVERY_SECS.0)
}

/// 친구가 던져 보낸 물건을 다듬은 것
#[derive(Debug, Clone, PartialEq)]
pub struct AcceptedToss {
    /// 물건 종류
    pub kind: ObjectKind,
    /// 날아 들어오는 모양
    pub fling: Fling,
    /// 포스트잇 글 (다듬은 것)
    pub note: Option<String>,
    /// 바쁜 친구가 되돌려 보낸 것임 (앱이 Toss.bounced로 채움 — 흐물이가 받아치지 않음)
    pub returned: bool,
}

/// fling이 없거나 이상할 때: 옆에서 살짝 포물선으로 들어옴
const DEFAULT_FLING: Fling = Fling { height: 0.4, vx: 400.0, vy: -200.0 };

/// 친구가 보낸 것을 다듬음: 모르는 종류는 버리고, 움직임은 범위 안으로, 글자는 편지처럼 (보이지 않는 글자 없이 80자까지)
pub fn accept_toss(kind: &str, fling: Option<Fling>, note: Option<&str>) -> Option<AcceptedToss> {
    let kind = ObjectKind::from_id(kind)?;
    let fling = fling.and_then(Fling::sanitized).unwrap_or(DEFAULT_FLING);
    let (vx, vy) = clamp_speed(fling.vx, fling.vy, visit::MAX_FLING_SPEED);
    // 똥·거대한 것엔 포스트잇이 없다 (옛 버전·고친 앱이 보내도)
    let note = note.filter(|_| kind.takes_postit()).and_then(visit::sanitize_note);
    Some(AcceptedToss { kind, fling: Fling { height: fling.height, vx, vy }, note, returned: false })
}

/// 📌 친구가 던진 앱 아이콘 장난감: 아이콘 그림 (한 변, base64 RGBA)이 엄격한 확인을 지나야만 받음 (icon::accept_wire). 포스트잇은 없음
pub fn accept_toy_toss(fling: Option<Fling>, icon: Option<(u32, &str)>) -> Option<(AcceptedToss, crate::icon::IconImage)> {
    let (size, text) = icon?;
    let icon = crate::icon::accept_wire(size, text)?;
    let fling = fling.and_then(Fling::sanitized).unwrap_or(DEFAULT_FLING);
    let (vx, vy) = clamp_speed(fling.vx, fling.vy, visit::MAX_FLING_SPEED);
    Some((AcceptedToss { kind: ObjectKind::AppIcon, fling: Fling { height: fling.height, vx, vy }, note: None, returned: false }, icon))
}

/// 📌 친구의 흐물이가 들고 온 앱 아이콘 장난감 (Visit.carry): 그림이 확인될 때만
pub fn accept_toy_carry(icon: Option<(u32, &str)>) -> Option<crate::icon::IconImage> {
    let (size, text) = icon?;
    crate::icon::accept_wire(size, text)
}

/// 🤹 친구의 흐물이가 들고 온 물건 (Visit.carry): 던진 것처럼 이름·색을 확인함.
/// 💪 거대한 것은 영웅이 들고 옴 (v0.7.1 — 보내는 앱은 GIANT_CARRY_MIN_VERSION 친구에게만 실음, 크기는 받는 앱이 제 화면 높이로 정함)
pub fn accept_carry(kind: &str, look: Option<&str>) -> Option<ObjectKind> {
    ObjectKind::wire_kind(kind, look)
}

/// 물건이 어디서 왔나
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Origin {
    /// 내가 만들었음 (최대 MAX_OWN_OBJECTS개)
    Own,
    /// 친구에게서 날아왔음 (최대 20개)
    Foreign,
    /// 똥: 어디서 왔든 최대 MAX_POOPS개
    Poop,
    /// 📌 작업표시줄에서 꺼낸 장난감: 최대 MAX_TOYS개 (쓰레기 수에 들지 않음)
    Toy,
    /// ⛄ 눈덩이: 어디서 왔든 최대 MAX_SNOWBALLS개 (쓰레기 수에 들지 않음)
    Snow,
}

/// 화면 위 물건 수 지키기: 넘치면 그 쪽에서 가장 오래된 것을 내보냄
#[derive(Debug, Default)]
pub struct Roster {
    own: VecDeque<u64>,
    foreign: VecDeque<u64>,
    poops: VecDeque<u64>,
    toys: VecDeque<u64>,
    snow: VecDeque<u64>,
}

impl Roster {
    /// id를 더함. 넘쳐서 치워야 할 물건이 있으면 돌려줌
    pub fn add(&mut self, id: u64, origin: Origin) -> Option<u64> {
        let (list, max) = match origin {
            Origin::Own => (&mut self.own, MAX_OWN_OBJECTS),
            Origin::Foreign => (&mut self.foreign, MAX_FOREIGN_OBJECTS),
            Origin::Poop => (&mut self.poops, MAX_POOPS),
            Origin::Toy => (&mut self.toys, MAX_TOYS),
            Origin::Snow => (&mut self.snow, crate::snowball::MAX_SNOWBALLS),
        };
        list.push_back(id);
        if list.len() > max {
            list.pop_front()
        } else {
            None
        }
    }

    /// id를 뺌. 있었으면 true
    pub fn remove(&mut self, id: u64) -> bool {
        for list in [&mut self.own, &mut self.foreign, &mut self.poops, &mut self.toys, &mut self.snow] {
            if let Some(i) = list.iter().position(|&x| x == id) {
                list.remove(i);
                return true;
            }
        }
        false
    }

    /// 내 물건 수
    pub fn own_count(&self) -> usize {
        self.own.len()
    }

    /// 친구가 보낸 물건 수
    pub fn foreign_count(&self) -> usize {
        self.foreign.len()
    }

    /// 테스트: 똥 수
    #[cfg(test)]
    pub fn poop_count(&self) -> usize {
        self.poops.len()
    }

    /// 모두 비움
    pub fn clear(&mut self) {
        self.own.clear();
        self.foreign.clear();
        self.poops.clear();
        self.toys.clear();
        self.snow.clear();
    }
}

/// 받았지만 아직 창을 만들지 않은 물건 — 앱이 틱마다 하나씩 꺼냄 (창 만들기는 메인 스레드를 기다려서, 한꺼번에 오면 모든 동물이 멈춤).
/// 기다리는 것까지 합쳐 받은 물건 20개임: 넘치면 가장 오래된 것이 먼저 — 이미 화면에 있는 물건, 없으면 가장 오래 기다린 것
#[derive(Debug)]
pub struct TossInbox<T> {
    queue: VecDeque<T>,
}

impl<T> Default for TossInbox<T> {
    /// 빈 줄
    fn default() -> Self {
        Self { queue: VecDeque::new() }
    }
}

impl<T> TossInbox<T> {
    /// 줄에 세움. 화면의 물건을 치워야 하면 그 id를 돌려줌 (로스터에서는 이미 뺐음)
    pub fn push(&mut self, item: T, roster: &mut Roster) -> Option<u64> {
        let mut evicted = None;
        if roster.foreign_count() + self.queue.len() >= MAX_FOREIGN_OBJECTS {
            evicted = roster.foreign.pop_front();
            if evicted.is_none() {
                self.queue.pop_front();
            }
        }
        self.queue.push_back(item);
        evicted
    }

    /// 맨 앞 것을 꺼냄
    pub fn pop(&mut self) -> Option<T> {
        self.queue.pop_front()
    }

    /// 기다리는 수
    pub fn len(&self) -> usize {
        self.queue.len()
    }

    /// 기다리는 것이 없음
    pub fn is_empty(&self) -> bool {
        self.queue.is_empty()
    }

    /// 모두 버림
    pub fn clear(&mut self) {
        self.queue.clear();
    }
}

/// 친구 한 명에게 던져 보내는 예산: 10개까지 한 번에, 그 뒤엔 6초에 하나 (받는 쪽 net의 제한과 같음 — 넘치면 받는 쪽이 버리므로 미리 튕김)
#[derive(Debug, Clone, Copy)]
pub struct TossBudget {
    tokens: f64,
    at: f64,
}

const TOSS_BURST: f64 = 50.0;
const TOSS_EVERY_SECS: f64 = 1.2;

impl Default for TossBudget {
    /// 처음엔 가득 참 (TOSS_BURST개)
    fn default() -> Self {
        TossBudget { tokens: TOSS_BURST, at: 0.0 }
    }
}

impl TossBudget {
    /// now까지 다시 찬 예산 (TOSS_EVERY_SECS에 하나, TOSS_BURST까지)
    fn refilled(&self, now: f64) -> f64 {
        (self.tokens + (now - self.at).max(0.0) / TOSS_EVERY_SECS).min(TOSS_BURST)
    }

    /// 하나 던질 예산이 있음
    pub fn can_spend(&self, now: f64) -> bool {
        self.refilled(now) >= 1.0
    }

    /// 예산 하나를 씀
    pub fn spend(&mut self, now: f64) {
        self.tokens = (self.refilled(now) - 1.0).max(0.0);
        self.at = now;
    }
}

/// 메뉴 한 번에 던지는 물건 수
const VOLLEY_SIZE: u32 = 10;
/// 연달아 눌러 쌓을 수 있는 횟수
const MAX_VOLLEYS: u32 = 5;

/// 친구 한 명에게 "물건 10개 던지기"로 기다리는 수: 다시 눌러도 10개까지만 기다림
#[derive(Debug, Default, Clone, Copy)]
pub struct Volley {
    left: u32,
}

impl Volley {
    /// 누를 때마다 10개씩, 다섯 번까지 쌓임
    pub fn request(&mut self) {
        self.left = (self.left + VOLLEY_SIZE).min(VOLLEY_SIZE * MAX_VOLLEYS);
    }

    /// 예산이 되는 만큼 예산을 쓰고, 지금 던질 수를 돌려줌
    pub fn release(&mut self, budget: &mut TossBudget, now: f64) -> u32 {
        let mut sent = 0;
        while self.left > 0 && budget.can_spend(now) {
            budget.spend(now);
            self.left -= 1;
            sent += 1;
        }
        sent
    }

    /// 기다리는 것을 다 던졌음
    pub fn is_done(&self) -> bool {
        self.left == 0
    }
}

/// 던질 물건 하나: 쓰레기 중 아무거나 (똥은 빼고), 들어오는 높이와 속도도 제각각
pub fn volley_piece(rng: &mut fastrand::Rng) -> (ObjectKind, Fling) {
    let kind = ObjectKind::TRASH[rng.usize(..ObjectKind::TRASH.len())];
    let fling = Fling { height: 0.1 + rng.f64() * 0.6, vx: 300.0 + rng.f64() * 500.0, vy: -(100.0 + rng.f64() * 400.0) };
    (kind, fling)
}

#[cfg(test)]
#[path = "object_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "object_snag_tests.rs"]
mod snag_tests;
