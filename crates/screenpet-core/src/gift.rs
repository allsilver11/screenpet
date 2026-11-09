//! 선물과 🎁 선물 도감 (v0.3.7) — 창도 웹뷰도 모르는 규칙만 (앱 쪽은 src-tauri/src/gifts.rs).
//!
//! 내 동물은 깨어 있는 시간 20~60분마다(배부르면 더 자주) 자기 종의 선물 하나를 서 있는 곳에 떨어뜨린다. 놀러 간 동물은
//! 주인이 방문에 실어 보낸 선물(Visit의 `gift`)을 친구 화면에 두고 온다. 받은 id는 목록에 있는 그대로일 때만 받고
//! (`Gift::parse`, 파일 이름·창 스크립트에 그대로 쓰이는 값이다), 친구마다 횟수(`FriendLimit`)와 화면 위 개수(`GiftField`)를 제한한다.
//! 주운 선물만 설정에 남는다 (`GiftBook`) — 화면 위 선물은 30분 뒤 사라지고 앱을 끄면 없어진다.

use std::collections::{BTreeMap, HashMap};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

use crate::brain::{Anim, Pet};
use crate::config::{clean_text, MAX_NAME_CHARS};
use crate::surfaces::Owner;
use crate::world::World;

/// 반짝이 그림·전송 이름의 꼬리 (ui/gifts/<id>_shiny.png)
pub const SHINY_SUFFIX: &str = "_shiny";
/// 떨어뜨린 선물이 반짝이일 확률
pub const SHINY_CHANCE: f64 = 0.05;
/// 내 동물을 친구에게 보낼 때 선물을 들려 보낼 확률
pub const VISIT_GIFT_CHANCE: f64 = 0.35;
/// 내 쪽에서 선물을 들려 보내는 최소 간격 (친구 쪽 제한 FRIEND_GIFT_EVERY보다 길어 보통은 거절될 일이 없다)
pub const VISIT_GIFT_EVERY: Duration = Duration::from_secs(10 * 60);
/// 한 친구(링크)에게서 선물을 받는 최소 간격
pub const FRIEND_GIFT_EVERY: Duration = Duration::from_secs(5 * 60);
/// 줍지 않은 선물이 사라질 때까지
pub const LIFE_SECS: f64 = 30.0 * 60.0;
/// 화면 위 선물 수 (창 하나씩이다)
pub const MAX_GIFTS: usize = 6;
/// 그중 친구가 두고 간 선물 수 — 친구가 자리를 다 차지하지 못한다
pub const MAX_FRIEND_GIFTS: usize = 3;
/// 깨어 있는 시간 기준 선물 간격 (초)
const INTERVAL_SECS: (f64, f64) = (20.0 * 60.0, 60.0 * 60.0);
/// 개발용 빠른 미리 보기의 간격 (초)
const FAST_INTERVAL_SECS: (f64, f64) = (15.0, 30.0);
/// 떨어지는 선물의 중력 (물리 px/s²)
const GRAVITY: f64 = 2200.0;
/// 화면 밖에서 돌아올 때 바닥 끝에서 띄우는 거리 (물리 px)
const EDGE_PAD: f64 = 32.0;
/// 도감에서 못 모은 선물의 이름
pub const UNKNOWN_NAME: &str = "???";
/// 반짝이를 떨어뜨릴 때 말
pub const SHINY_SPEECH: &str = "✨ look, shiny!";
/// 주웠을 때 내 동물의 말
pub const COLLECT_SPEECH: &str = "yay!";

#[derive(Debug, PartialEq, Eq)]
pub struct GiftItem {
    /// 그림 파일 이름이자 전송 값 (ui/gifts/<id>.png, tools/draw_gifts.py)
    pub id: &'static str,
    pub name: &'static str,
}

#[derive(Debug)]
pub struct GiftList {
    pub key: &'static str,
    /// 도감의 칸 제목
    pub title: &'static str,
    pub emoji: &'static str,
    /// 이 목록을 쓰는 동물
    pub species: &'static [&'static str],
    /// 떨어뜨릴 때 말 (영어)
    pub speech: &'static str,
    pub items: &'static [GiftItem],
}

const fn item(id: &'static str, name: &'static str) -> GiftItem {
    GiftItem { id, name }
}

pub const CATALOG: &[GiftList] = &[
    GiftList {
        key: "cat",
        title: "고양이",
        emoji: "🐱",
        species: &["cat", "persian"],
        speech: "...for you. hmph.",
        items: &[
            item("cat_fishbone", "생선 뼈"),
            item("cat_yarn", "털실 뭉치"),
            item("cat_feather", "깃털"),
            item("cat_mouse", "쥐 인형"),
            item("cat_bell", "방울 목걸이"),
            item("cat_catnip", "캣닢"),
        ],
    },
    GiftList {
        key: "squirrel",
        title: "다람쥐",
        emoji: "🐿️",
        species: &["squirrel"],
        speech: "chk! for you!",
        items: &[
            item("squirrel_acorn", "도토리"),
            item("squirrel_pinecone", "솔방울"),
            item("squirrel_walnut", "호두"),
            item("squirrel_ginkgo", "노란 은행잎"),
            item("squirrel_fluff", "꼬리 솜털"),
            item("squirrel_mushroom", "작은 버섯"),
        ],
    },
    GiftList {
        key: "rabbit",
        title: "토끼",
        emoji: "🐰",
        species: &["rabbit"],
        speech: "here you go!",
        items: &[
            item("rabbit_carrot", "당근"),
            item("rabbit_clover", "네잎클로버"),
            item("rabbit_lettuce", "상추"),
            item("rabbit_radish", "무"),
            item("rabbit_daisy", "데이지"),
            item("rabbit_strawberry", "딸기"),
        ],
    },
    GiftList {
        key: "frog",
        title: "개구리",
        emoji: "🐸",
        species: &["frog"],
        speech: "ribbit! a gift!",
        items: &[
            item("frog_lilypad", "연잎"),
            item("frog_dragonfly", "잠자리"),
            item("frog_fly", "파리"),
            item("frog_lotus", "연꽃"),
            item("frog_tadpole", "올챙이"),
            item("frog_pebble", "이끼 돌"),
        ],
    },
    GiftList {
        key: "pumpkin",
        title: "호박",
        emoji: "🎃",
        species: &["pumpkin"],
        speech: "trick or treat!",
        items: &[
            item("pumpkin_candycorn", "캔디콘"),
            item("pumpkin_lantern", "작은 초롱"),
            item("pumpkin_seed", "호박씨"),
            item("pumpkin_vine", "호박 덩굴"),
            item("pumpkin_lollipop", "막대사탕"),
            item("pumpkin_hat", "마녀 모자"),
        ],
    },
    GiftList {
        key: "ghost",
        title: "유령",
        emoji: "👻",
        species: &["ghost"],
        speech: "boo! for you~",
        items: &[
            item("ghost_candle", "촛불"),
            item("ghost_key", "낡은 열쇠"),
            item("ghost_web", "거미줄"),
            item("ghost_bat", "박쥐 인형"),
            item("ghost_tombstone", "작은 묘비"),
            item("ghost_moon", "초승달 장식"),
        ],
    },
    GiftList {
        key: "duck",
        title: "오리",
        emoji: "🦆",
        species: &["duck"],
        speech: "quack! for you!",
        items: &[
            item("duck_bread", "빵 부스러기"),
            item("duck_pebble", "조약돌"),
            item("duck_rubber", "고무 오리"),
            item("duck_feather", "오리 깃털"),
            item("duck_shell", "조개껍데기"),
            item("duck_reed", "갈대"),
        ],
    },
    GiftList {
        key: "bird",
        title: "새",
        emoji: "🐦",
        species: &["bird"],
        speech: "chirp! for you!",
        items: &[
            item("bird_gourdseed", "박씨"),
            item("bird_feather", "제비 깃털"),
            item("bird_twig", "둥지 지푸라기"),
            item("bird_berry", "빨간 열매"),
            item("bird_egg", "하늘색 알"),
            item("bird_dandelion", "민들레 홀씨"),
        ],
    },
    GiftList {
        key: "penguin",
        title: "펭귄",
        emoji: "🐧",
        species: &["penguin"],
        speech: "pip! for you!",
        items: &[
            item("penguin_pebble", "반들반들 조약돌"),
            item("penguin_ice", "얼음 조각"),
            item("penguin_fish", "은빛 멸치"),
            item("penguin_snowflake", "눈송이"),
            item("penguin_down", "아기 솜털"),
            item("penguin_shell", "소라 껍데기"),
        ],
    },
    GiftList {
        key: "redpanda",
        title: "레서판다",
        emoji: "🐾",
        species: &["redpanda"],
        speech: "kyu! for you!",
        items: &[
            item("redpanda_bamboo", "대나무잎"),
            item("redpanda_berry", "산딸기"),
            item("redpanda_stick", "작은 나뭇가지"),
            item("redpanda_maple", "빨간 단풍잎"),
            item("redpanda_fluff", "꼬리 털뭉치"),
            item("redpanda_flower", "들꽃"),
        ],
    },
    GiftList {
        key: "giraffe",
        title: "기린",
        emoji: "🦒",
        species: &["giraffe"],
        speech: "from way up high!",
        items: &[
            item("giraffe_acacia", "아카시아 잎"),
            item("giraffe_pod", "콩 꼬투리"),
            item("giraffe_mango", "망고"),
            item("giraffe_cloud", "구름 조각"),
            item("giraffe_nest", "새 둥지"),
            item("giraffe_scarf", "긴 목도리"),
        ],
    },
    GiftList {
        key: "rudolph",
        title: "루돌프",
        emoji: "🦌",
        species: &["rudolph"],
        speech: "jingle! for you!",
        items: &[
            item("rudolph_bell", "금방울"),
            item("rudolph_bulb", "빨간 전구"),
            item("rudolph_snowflake", "눈송이"),
            item("rudolph_star", "트리 별"),
            item("rudolph_moss", "순록 이끼"),
            item("rudolph_ribbon", "빨간 리본"),
        ],
    },
    GiftList {
        key: "santa",
        title: "산타",
        emoji: "🎅",
        species: &["santa"],
        speech: "ho ho ho!",
        items: &[
            item("santa_giftbox", "선물 상자"),
            item("santa_candycane", "지팡이 사탕"),
            item("santa_mitten", "벙어리장갑"),
            item("santa_cookie", "진저 쿠키"),
            item("santa_stocking", "크리스마스 양말"),
            item("santa_coal", "석탄 한 조각"),
        ],
    },
    GiftList {
        key: "snowman",
        title: "눈사람",
        emoji: "⛄",
        species: &["snowman"],
        speech: "brrr! for you!",
        items: &[
            item("snowman_icicle", "고드름"),
            item("snowman_carrot", "여분 당근 코"),
            item("snowman_twig", "나뭇가지 팔"),
            item("snowman_globe", "스노우볼"),
            item("snowman_bucket", "양동이 모자"),
            item("snowman_snowball", "동글 눈덩이"),
        ],
    },
    GiftList {
        key: "dino",
        title: "공룡",
        emoji: "🦖",
        species: &["dino_yellow", "dino_green", "dino_purple"],
        speech: "rawr! for you!",
        items: &[
            item("dino_fossil", "화석"),
            item("dino_eggshell", "알껍데기"),
            item("dino_fern", "고사리"),
            item("dino_amber", "호박석"),
            item("dino_tooth", "공룡 이빨"),
            item("dino_footprint", "발자국 돌"),
        ],
    },
];

/// 그 동물의 선물 목록 (모르는 동물이면 None)
pub fn list_for(species: &str) -> Option<&'static GiftList> {
    CATALOG.iter().find(|l| l.species.contains(&species))
}

fn known_item(id: &str) -> Option<&'static GiftItem> {
    CATALOG.iter().flat_map(|l| l.items).find(|i| i.id == id)
}

/// 선물 하나 — 언제나 목록에 있는 id다 (`id`가 &'static이라 받은 글자를 그대로 들고 있을 수 없다)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Gift {
    pub id: &'static str,
    pub shiny: bool,
}

impl Gift {
    /// 전송 값·그림 이름을 선물로. 목록에 정확히 있는 값만 받는다
    pub fn parse(raw: &str) -> Option<Gift> {
        if let Some(i) = known_item(raw) {
            return Some(Gift { id: i.id, shiny: false });
        }
        let base = raw.strip_suffix(SHINY_SUFFIX)?;
        known_item(base).map(|i| Gift { id: i.id, shiny: true })
    }

    /// 그 동물의 목록에서 하나 고른다 (가끔 반짝이)
    pub fn roll(species: &str, rng: &mut fastrand::Rng) -> Option<Gift> {
        let list = list_for(species)?;
        let item = &list.items[rng.usize(..list.items.len())];
        Some(Gift { id: item.id, shiny: rng.f64() < SHINY_CHANCE })
    }

    /// 전송 값이자 그림 이름 (ui/gifts/<wire>.png)
    pub fn wire(&self) -> String {
        if self.shiny {
            format!("{}{SHINY_SUFFIX}", self.id)
        } else {
            self.id.to_string()
        }
    }
}

/// 친구에게 놀러 보낼 때 들려 보낼 선물 (가끔만)
pub fn roll_visit_gift(species: &str, rng: &mut fastrand::Rng) -> Option<Gift> {
    let gift = Gift::roll(species, rng)?;
    (rng.f64() < VISIT_GIFT_CHANCE).then_some(gift)
}

/// 친구 동물이 들고 온 값: 목록에 있고 그 동물 종의 선물일 때만
pub fn accept_visit_gift(raw: Option<&str>, species: &str) -> Option<Gift> {
    let gift = Gift::parse(raw?)?;
    list_for(species)?.items.iter().any(|i| i.id == gift.id).then_some(gift)
}

/// 친구 하나당 받는 횟수 제한. 거절한 시도는 기다리는 시간을 늘리지 않는다 (친구 수만큼만 자란다 — 인증된 친구만 부른다)
#[derive(Debug, Default)]
pub struct FriendLimit {
    last: HashMap<String, Instant>,
}

impl FriendLimit {
    pub fn allow(&mut self, friend: &str, now: Instant) -> bool {
        if self.last.get(friend).is_some_and(|t| now.duration_since(*t) < FRIEND_GIFT_EVERY) {
            return false;
        }
        self.last.insert(friend.to_string(), now);
        true
    }
}

/// 다음 선물까지 남은 "깨어 있는 시간"
#[derive(Debug, Clone, PartialEq)]
pub struct GiftTimer {
    remaining: f64,
}

/// 기분(배부름)에 따른 시계 빠르기: 배부르면 더 자주, 배고프면 드물게
pub fn mood_rate(fullness: f64) -> f64 {
    if fullness >= 0.7 {
        1.5
    } else if fullness < 0.3 {
        0.5
    } else {
        1.0
    }
}

fn roll_interval(rng: &mut fastrand::Rng, fast: bool) -> f64 {
    let (lo, hi) = if fast { FAST_INTERVAL_SECS } else { INTERVAL_SECS };
    lo + rng.f64() * (hi - lo)
}

impl GiftTimer {
    pub fn new(rng: &mut fastrand::Rng, fast: bool) -> Self {
        Self { remaining: roll_interval(rng, fast) }
    }

    pub fn remaining(&self) -> f64 {
        self.remaining
    }

    /// 깨어 있으면 시계를 돌린다. 때가 됐으면 (떨어뜨리고 rearm할 때까지 계속) true
    pub fn tick(&mut self, dt: f64, awake: bool, rate: f64) -> bool {
        if awake {
            self.remaining -= dt * rate;
        }
        self.remaining <= 0.0
    }

    pub fn rearm(&mut self, rng: &mut fastrand::Rng, fast: bool) {
        self.remaining = roll_interval(rng, fast);
    }
}

/// 화면 위 선물 하나. (x, y) = 아래 가운데 (물리 px)
#[derive(Debug, Clone, PartialEq)]
pub struct LiveGift {
    pub token: u64,
    pub gift: Gift,
    /// 두고 간 친구의 이름 (내 동물이 떨어뜨린 건 None)
    pub from: Option<String>,
    pub x: f64,
    pub y: f64,
    vy: f64,
    /// 앉아 있는 곳과, 그 왼쪽 끝에서의 거리
    on: Option<(Owner, f64)>,
    age: f64,
}

/// 화면 위 선물들: 떨어지고, 앉은 창을 따라가고, 창이 사라지거나 가려지면 다시 떨어진다
#[derive(Debug, Default)]
pub struct GiftField {
    gifts: Vec<LiveGift>,
    last_token: u64,
}

impl GiftField {
    pub fn is_empty(&self) -> bool {
        self.gifts.is_empty()
    }

    pub fn len(&self) -> usize {
        self.gifts.len()
    }

    pub fn has_room(&self) -> bool {
        self.gifts.len() < MAX_GIFTS
    }

    pub fn get(&self, token: u64) -> Option<&LiveGift> {
        self.gifts.iter().find(|g| g.token == token)
    }

    pub fn iter(&self) -> impl Iterator<Item = &LiveGift> {
        self.gifts.iter()
    }

    /// 선물이 앉아 있는 창들 (사이 틱에도 경계를 읽어 부드럽게 따라가려고)
    pub fn ridden_windows(&self) -> impl Iterator<Item = u64> + '_ {
        self.gifts.iter().filter_map(|g| match g.on {
            Some((Owner::Window(id), _)) => Some(id),
            _ => None,
        })
    }

    /// (x, y)에서 떨어지기 시작한다. 자리가 없으면 None
    pub fn add(&mut self, gift: Gift, from: Option<String>, x: f64, y: f64) -> Option<u64> {
        let friends = self.gifts.iter().filter(|g| g.from.is_some()).count();
        if !self.has_room() || (from.is_some() && friends >= MAX_FRIEND_GIFTS) {
            return None;
        }
        self.last_token += 1;
        self.gifts.push(LiveGift { token: self.last_token, gift, from, x, y, vy: 0.0, on: None, age: 0.0 });
        Some(self.last_token)
    }

    pub fn collect(&mut self, token: u64) -> Option<(Gift, Option<String>)> {
        let i = self.gifts.iter().position(|g| g.token == token)?;
        let g = self.gifts.remove(i);
        Some((g.gift, g.from))
    }

    /// 매 틱. 시간이 다 되어 사라진 선물의 token을 돌려준다
    pub fn tick(&mut self, dt: f64, world: &World) -> Vec<u64> {
        let mut gone = Vec::new();
        self.gifts.retain_mut(|g| {
            g.age += dt;
            if g.age >= LIFE_SECS {
                gone.push(g.token);
                return false;
            }
            g.step(dt, world);
            true
        });
        gone
    }
}

impl LiveGift {
    fn step(&mut self, dt: f64, world: &World) {
        if let Some((owner, dx)) = self.on {
            if let Some((ax, ay)) = world.anchor(owner) {
                let x = ax + dx;
                // 바닥은 최대화한 창에 가려져도 그 자리에 그대로 (선물 창은 항상 위에 있다).
                // 모니터가 줄어 화면 밖이 됐으면 가까운 바닥으로 돌아온다
                let on_ground = matches!(owner, Owner::Ground(_));
                if on_ground && !world.work_areas.iter().any(|r| r.left <= x && x <= r.right && r.bottom == ay) {
                    (self.x, self.y) = (x, ay);
                    self.rest_on_nearest_ground(world);
                    return;
                }
                let held = on_ground || world.piece_at(owner, x).is_some();
                if held {
                    (self.x, self.y) = (x, ay);
                    return;
                }
            }
            self.on = None;
            self.vy = 0.0;
        }
        self.vy += GRAVITY * dt;
        let next = self.y + self.vy * dt;
        if let Some(s) = world.landing(self.x, self.y, next) {
            let (owner, y) = (s.owner, s.y);
            let left = world.anchor(owner).map_or(0.0, |(ax, _)| ax);
            self.on = Some((owner, self.x - left));
            self.y = y;
            return;
        }
        self.y = next;
        if self.y > world.bounds.bottom {
            self.rest_on_nearest_ground(world);
        }
    }

    /// 어느 디딤판도 못 만나고 화면 아래로 빠졌다 — 가장 가까운 작업 영역 바닥에 앉는다
    fn rest_on_nearest_ground(&mut self, world: &World) {
        let wa = world.work_area_at(self.x, self.y);
        let i = world.work_areas.iter().position(|r| *r == wa).unwrap_or(0);
        self.x = self.x.clamp(wa.left + EDGE_PAD, (wa.right - EDGE_PAD).max(wa.left + EDGE_PAD));
        self.y = wa.bottom;
        self.on = Some((Owner::Ground(i), self.x - wa.left));
    }
}

/// 선물을 떨어뜨려도 되는 몸짓: 걷거나 쉬는 중 (먹기·잠·끌려가기·벽 타기·장난 도중엔 기다린다)
pub fn calm_anim(anim: Anim) -> bool {
    matches!(anim, Anim::Walk | Anim::Run | Anim::Idle | Anim::Look | Anim::SitFront | Anim::Stretch | Anim::Tilt | Anim::Hop | Anim::LookUp)
}

/// 몸 크기에 대해 선물이 놓이는 앞쪽 거리
const DROP_REACH: f64 = 0.4;

/// 이 동물이 지금 선물을 떨어뜨릴 자리 (아래 가운데, 물리 px). 서 있으면 몸 앞 그 바닥, 나는 동물은 몸 아래 (떨어진다).
/// 들려 있거나·하우스·친구 화면·조작 모드·집중 중·자리 비움·창 안쪽이거나 하던 일이 있으면 None
pub fn drop_spot(pet: &Pet, world: &World) -> Option<(f64, f64)> {
    if pet.is_held() || pet.is_housed() || pet.is_away() || pet.is_controlled() || pet.is_focusing() || pet.is_user_away() || !calm_anim(pet.render().anim) {
        return None;
    }
    let (x, y) = (pet.body.x, pet.body.y);
    match pet.standing {
        Some(st) if matches!(st.owner, Owner::Inside(_)) => None,
        Some(st) => {
            let front = x + pet.facing * DROP_REACH * pet.cfg.size;
            let on_it = world.piece_at(st.owner, front).is_some();
            Some((if on_it { front } else { x }, y - 1.0))
        }
        None if pet.cfg.flies => Some((x, y)),
        None => None,
    }
}

/// 도감의 한 줄
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct BookEntry {
    /// 모은 보통 선물 수
    #[serde(default)]
    pub n: u32,
    /// 모은 반짝이 수
    #[serde(default, skip_serializing_if = "is_zero")]
    pub shiny: u32,
    /// 마지막으로 이 선물을 두고 간 친구
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub from: Option<String>,
}

fn is_zero(n: &u32) -> bool {
    *n == 0
}

/// 🎁 모은 선물 (설정에 남는다). 잘못된 줄 하나, 또는 통째로 잘못돼도 설정 나머지는 살아남는다
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct GiftBook {
    #[serde(deserialize_with = "lenient_items")]
    pub(crate) items: BTreeMap<String, BookEntry>,
}

fn lenient_items<'de, D: serde::Deserializer<'de>>(d: D) -> Result<BTreeMap<String, BookEntry>, D::Error> {
    let serde_json::Value::Object(map) = serde_json::Value::deserialize(d)? else { return Ok(BTreeMap::new()) };
    Ok(map.into_iter().filter_map(|(id, v)| Some((id, serde_json::from_value(v).ok()?))).collect())
}

impl GiftBook {
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    pub fn entry(&self, id: &str) -> Option<&BookEntry> {
        self.items.get(id)
    }

    /// 주운 선물을 적는다. from = 두고 간 친구 (다른 사람이 정한 이름이라 다듬는다)
    pub fn add(&mut self, gift: Gift, from: Option<&str>) {
        let e = self.items.entry(gift.id.to_string()).or_default();
        let count = if gift.shiny { &mut e.shiny } else { &mut e.n };
        *count = count.saturating_add(1);
        if let Some(name) = from.map(|f| clean_text(f, MAX_NAME_CHARS)).filter(|f| !f.is_empty()) {
            e.from = Some(name);
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct BookItem {
    pub id: &'static str,
    pub name: &'static str,
    pub count: u32,
    pub shiny: u32,
    pub from: Option<String>,
    /// 보일 그림 (ui/gifts/<image>.png) — 못 모은 것은 웹뷰가 실루엣으로 칠한다
    pub image: String,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct BookSection {
    pub key: &'static str,
    pub title: &'static str,
    pub emoji: &'static str,
    pub collected: usize,
    pub items: Vec<BookItem>,
}

/// 도감 창에 보낼 것 (`book_state` 커맨드, `book-state` 이벤트)
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct BookView {
    pub sections: Vec<BookSection>,
    /// 하나라도 모은 선물 수
    pub collected: usize,
    pub total: usize,
    /// 반짝이를 모은 선물 수
    pub shiny: usize,
}

pub fn book_view(book: &GiftBook) -> BookView {
    let sections: Vec<BookSection> = CATALOG
        .iter()
        .map(|list| {
            let items: Vec<BookItem> = list
                .items
                .iter()
                .map(|i| {
                    let e = book.entry(i.id).cloned().unwrap_or_default();
                    let image = if e.n == 0 && e.shiny > 0 { format!("{}{SHINY_SUFFIX}", i.id) } else { i.id.to_string() };
                    let name = if e.n > 0 || e.shiny > 0 { i.name } else { UNKNOWN_NAME };
                    BookItem { id: i.id, name, count: e.n, shiny: e.shiny, from: e.from, image }
                })
                .collect();
            let collected = items.iter().filter(|i| i.count > 0 || i.shiny > 0).count();
            BookSection { key: list.key, title: list.title, emoji: list.emoji, collected, items }
        })
        .collect();
    let collected = sections.iter().map(|s| s.collected).sum();
    let total = sections.iter().map(|s| s.items.len()).sum();
    let shiny = sections.iter().flat_map(|s| &s.items).filter(|i| i.shiny > 0).count();
    BookView { sections, collected, total, shiny }
}

/// 주웠을 때 내 동물의 말: 친구가 두고 간 것이면 누가 줬는지 (웹뷰는 textContent로만 쓴다)
pub fn collect_speech(from: Option<&str>) -> String {
    match from.map(|f| clean_text(f, MAX_NAME_CHARS)).filter(|f| !f.is_empty()) {
        Some(name) => format!("from {name}!"),
        None => COLLECT_SPEECH.to_string(),
    }
}

/// 떨어뜨릴 때 말
pub fn drop_speech(species: &str, gift: Gift) -> &'static str {
    if gift.shiny {
        SHINY_SPEECH
    } else {
        list_for(species).map_or("for you!", |l| l.speech)
    }
}

#[cfg(test)]
#[path = "gift_tests.rs"]
mod tests;
