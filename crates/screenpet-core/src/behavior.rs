//! 다음 행동 고르기 — 이동 방식, 쉬기/애교, 클릭 반응. 확률·시간 수치는 모두 여기에 둔다.

use serde::Deserialize;

pub const IDLE_SECS: (f64, f64) = (1.5, 3.0);
pub const SLEEP_SECS: (f64, f64) = (10.0, 20.0);
pub const SLEEP_CHANCE: f64 = 0.12;
pub const TRICK_SECS: (f64, f64) = (3.0, 5.0);
pub const TRICK_CHANCE: f64 = 0.55;
/// 활동 종류가 바뀔 때 사이에 끼우는 옆모습 앉기 (spec 2B §6)
pub const BEAT_SECS: (f64, f64) = (0.6, 1.5);
/// 다음 이동이 직전 이동 방식을 그대로 이어갈 확률
pub const MOVE_KEEP_CHANCE: f64 = 0.5;
/// 애교 뒤 다음 이동이 걷기일 확률
pub const WALK_AFTER_FRONT_CHANCE: f64 = 0.7;
/// 새침한 동물의 애교 확률 (평소 / 배고플 때)
pub const ALOOF_AEGYO_CHANCE: f64 = 0.10;
pub const ALOOF_HUNGRY_AEGYO_CHANCE: f64 = 0.60;
/// 새침한 동물이 애교 대신 클릭을 무시할 때 "..." 말풍선을 띄울 확률
pub const IGNORE_SPEECH_CHANCE: f64 = 0.5;
pub const HUFF_SECS: f64 = 1.5;
/// 장난꾸러기(개구리)가 쉴 때 애교 부릴 확률
pub const PLAYFUL_TRICK_CHANCE: f64 = 0.65;

/// 동물 성격 (pet.json `personality`)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Personality {
    /// 애교 많음 — 2A 동작 그대로
    #[default]
    Friendly,
    /// 새침함 (고양이) — 애교가 드물고 클릭에 흥! 하거나 무시
    Aloof,
    /// 장난꾸러기 (개구리) — 클릭엔 늘 반응하고 쉴 때 애교가 조금 더 많다
    Playful,
}

impl Personality {
    /// 쉬기에 들어갈 때(그리고 새침한 동물이 클릭받을 때) 애교를 부릴 확률
    pub fn trick_chance(self, hungry: bool) -> f64 {
        match (self, hungry) {
            (Personality::Friendly, _) => TRICK_CHANCE,
            (Personality::Playful, _) => PLAYFUL_TRICK_CHANCE,
            (Personality::Aloof, false) => ALOOF_AEGYO_CHANCE,
            (Personality::Aloof, true) => ALOOF_HUNGRY_AEGYO_CHANCE,
        }
    }
}

/// 활동 묶음 — 묶음이 바뀌면 그 사이에 잠깐 앉는다
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Family {
    /// 걷기·달리기
    Move,
    /// 구르기
    Roll,
    /// 앞모습 애교·흥·간질 반응 등
    Front,
    /// 종별 특별 동작
    Special,
    /// 화면 벽 타기 (뒷모습)
    Climb,
}
/// 벽 타는 동물이 새 이동 대신 화면을 기어오를 확률 (spec 2B §7)
pub const CLIMB_CHANCE: f64 = 0.06;
/// 기어오를 높이 — 작업 영역 바닥에서 꼭대기까지 거리의 이 비율 범위
pub const CLIMB_HEIGHT: (f64, f64) = (0.25, 1.0);
/// 크기 128 기준 px/s (cfg.size / 128 배)
pub const CLIMB_SPEED: f64 = 70.0;
pub const SLIDE_SPEED: f64 = 250.0;
pub const WALL_CLIMB_SPEED: f64 = 90.0;
/// 목표 높이에 닿은 뒤 매달려 있는 시간
pub const CLIMB_HANG_SECS: f64 = 0.4;
/// 이 속도(|vx|)보다 빠르게 던져져야 창 옆면에 달라붙는다
pub const WALL_STICK_MIN_VX: f64 = 50.0;
/// 창 꼭대기에 올라설 때 옆면에서 안쪽으로 몸 절반 + 이만큼
pub const WALL_TOP_INSET: f64 = 2.0;

/// 기어오를 목표 발 높이: bottom(작업 영역 바닥) − u × (bottom − top_limit), u ∈ CLIMB_HEIGHT
pub fn climb_target(rng: &mut fastrand::Rng, bottom: f64, top_limit: f64) -> f64 {
    bottom - range(rng, CLIMB_HEIGHT) * (bottom - top_limit)
}

pub const MOVE_END_TURN_CHANCE: f64 = 0.3;
pub const WALK_EDGE_HOP_CHANCE: f64 = 0.8;
pub const RUN_LEAP_VX_MULT: f64 = 1.8;
pub const CLICK_TRICK_SECS: f64 = 2.0;

/// 특정 동물만 가진 특별 동작 (spec §2.7)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Special {
    /// 고양이 하악질 — 등 휘고 꼬리 펑
    Arch,
    /// 강아지 — 혀 내밀고 꼬리 흔들기
    Wag,
    /// 기린 — 구르는 대신 멈춰 서서 목을 평소의 두 배로 쭉 늘였다 줄인다 (쉬거나 간질일 땐 하지 않는다)
    NeckStretch,
    /// 공룡 — 입에서 작은 불꽃을 퐁 (아무것도 태우지 않는다)
    Fire,
    /// 공룡 — 고개 들고 크앙!
    Roar,
    /// 공룡 — 제자리 쿵쾅쿵쾅 (같은 바닥의 친구들이 흔들리거나 데굴 넘어진다, social.rs)
    Stomp,
}

/// 공룡의 세 동작 중 하나를 고르는 몫 (불 · 크앙 · 쿵쾅) — 쉴 때 / 간질일 때 (간질이면 쿵쾅보다 불·크앙)
pub const DINO_REST_SHARE: [f64; 3] = [0.35, 0.35, 0.30];
pub const DINO_TICKLE_SHARE: [f64; 3] = [0.45, 0.40, 0.15];

/// 공룡: 불·크앙·쿵쾅 중 하나 (tickled면 간질 반응 몫)
pub fn pick_dino(rng: &mut fastrand::Rng, tickled: bool) -> Special {
    let share = if tickled { DINO_TICKLE_SHARE } else { DINO_REST_SHARE };
    let u = rng.f64();
    if u < share[0] {
        Special::Fire
    } else if u < share[0] + share[1] {
        Special::Roar
    } else {
        Special::Stomp
    }
}

/// 기린 목 늘이기 길이 (초) — 앱(ui/main.js NECK_*_MS)이 이 시간 안에 늘이고·머물고·줄인다
pub const NECK_STRETCH_SECS: f64 = 3.4;

impl Special {
    pub fn rest_chance(self) -> f64 {
        match self {
            Special::Arch => 0.08,
            Special::Wag => 0.12,
            Special::NeckStretch => 0.0,
            // 공룡: 쉬는 박자의 18%는 셋 중 하나 (pick_dino)
            Special::Fire | Special::Roar | Special::Stomp => 0.18,
        }
    }

    pub fn tickle_chance(self) -> f64 {
        match self {
            Special::Arch => 0.25,
            Special::Wag => 0.30,
            Special::NeckStretch => 0.0,
            Special::Fire | Special::Roar | Special::Stomp => 0.30,
        }
    }

    pub fn secs(self) -> (f64, f64) {
        match self {
            Special::Arch => (2.0, 3.0),
            Special::Wag => (2.0, 4.0),
            Special::NeckStretch => (NECK_STRETCH_SECS, NECK_STRETCH_SECS),
            // 불 뿜기 그림은 한 번 (1초) 뒤 흐뭇한 얼굴로 머문다
            Special::Fire => (1.8, 2.4),
            Special::Roar => (1.5, 2.2),
            // 쿵쾅 한 번 = 0.67초 (stomp 4장 6fps) — 두세 번
            Special::Stomp => (1.4, 2.0),
        }
    }

    pub fn anim_name(self) -> &'static str {
        match self {
            Special::Arch => "arch",
            Special::Wag => "wag",
            Special::NeckStretch => "neckstretch",
            Special::Fire => "fire",
            Special::Roar => "roar",
            Special::Stomp => "stomp",
        }
    }

    /// 구르기 대신 하는 특별 동작 (기린 목 늘이기)
    pub fn replaces_roll(self) -> bool {
        self == Special::NeckStretch
    }

    /// 공룡 동작 — 시작할 때 셋 중 하나를 다시 고른다 (pick_dino)
    pub fn is_dino(self) -> bool {
        matches!(self, Special::Fire | Special::Roar | Special::Stomp)
    }

    /// pet.json에 있는 동작으로 그 동물의 특별 동작을 찾는다
    pub fn from_manifest(has_anim: impl Fn(&str) -> bool) -> Option<Special> {
        [Special::Arch, Special::Wag, Special::NeckStretch, Special::Fire, Special::Roar, Special::Stomp]
            .into_iter()
            .find(|s| has_anim(s.anim_name()))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Move {
    Walk,
    Run,
    Roll,
}

impl Move {
    pub fn family(self) -> Family {
        match self {
            Move::Walk | Move::Run => Family::Move,
            Move::Roll => Family::Roll,
        }
    }

    pub fn speed_mult(self) -> f64 {
        match self {
            Move::Walk => 0.7,
            Move::Run => 2.5,
            Move::Roll => 2.0,
        }
    }

    pub fn secs(self) -> (f64, f64) {
        match self {
            Move::Walk => (4.0, 10.0),
            Move::Run => (2.0, 5.0),
            Move::Roll => (2.0, 4.0),
        }
    }
}

/// 앞모습 애교 동작
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Trick {
    SitFront,
    Wave,
    Tilt,
    Belly,
    Stretch,
    Heart,
}

pub const TRICKS: [Trick; 6] = [Trick::SitFront, Trick::Wave, Trick::Tilt, Trick::Belly, Trick::Stretch, Trick::Heart];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Rest {
    Idle,
    Trick(Trick),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClickReaction {
    /// v1의 하트 + 제자리 점프
    Hop,
    Trick(Trick),
}

/// 짧은 클릭(깨어 있고 배고프지 않을 때)에 대한 반응
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClickOutcome {
    React(ClickReaction),
    /// 새침: 하던 일을 계속한다 (speak면 "..." 말풍선)
    Ignore { speak: bool },
    /// 새침: 흥! 하고 고개 돌리기
    Huff,
}

pub fn pick_click_outcome(rng: &mut fastrand::Rng, personality: Personality, hungry: bool) -> ClickOutcome {
    if personality != Personality::Aloof || rng.f64() < personality.trick_chance(hungry) {
        ClickOutcome::React(pick_click(rng))
    } else if rng.bool() {
        ClickOutcome::Ignore { speak: rng.f64() < IGNORE_SPEECH_CHANCE }
    } else {
        ClickOutcome::Huff
    }
}

/// 다음 이동 방식: 애교 뒤엔 대개 걷기, 아니면 절반은 직전 방식을 이어간다
pub fn pick_next_move(rng: &mut fastrand::Rng, last: Option<Move>, after_front: bool) -> Move {
    if after_front {
        if rng.f64() < WALK_AFTER_FRONT_CHANCE {
            return Move::Walk;
        }
    } else if let Some(last) = last {
        if rng.f64() < MOVE_KEEP_CHANCE {
            return last;
        }
    }
    pick_move(rng)
}

pub fn pick_move(rng: &mut fastrand::Rng) -> Move {
    let r = rng.f64();
    if r < 0.45 {
        Move::Walk
    } else if r < 0.80 {
        Move::Run
    } else {
        Move::Roll
    }
}

pub fn pick_rest(rng: &mut fastrand::Rng, trick_chance: f64) -> Rest {
    if rng.f64() < trick_chance {
        Rest::Trick(pick_trick(rng))
    } else {
        Rest::Idle
    }
}

pub fn pick_trick(rng: &mut fastrand::Rng) -> Trick {
    TRICKS[rng.usize(..TRICKS.len())]
}

pub fn pick_click(rng: &mut fastrand::Rng) -> ClickReaction {
    match rng.usize(..4) {
        0 => ClickReaction::Hop,
        1 => ClickReaction::Trick(Trick::Heart),
        2 => ClickReaction::Trick(Trick::Wave),
        _ => ClickReaction::Trick(Trick::Tilt),
    }
}

pub fn range(rng: &mut fastrand::Rng, (lo, hi): (f64, f64)) -> f64 {
    lo + rng.f64() * (hi - lo)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn share<T: PartialEq>(n: usize, mut f: impl FnMut(&mut fastrand::Rng) -> T, want: T) -> f64 {
        let mut rng = fastrand::Rng::with_seed(7);
        (0..n).filter(|_| f(&mut rng) == want).count() as f64 / n as f64
    }

    #[test]
    fn move_mix_is_45_35_20() {
        assert!((0.42..=0.48).contains(&share(20_000, pick_move, Move::Walk)));
        assert!((0.32..=0.38).contains(&share(20_000, pick_move, Move::Run)));
        assert!((0.17..=0.23).contains(&share(20_000, pick_move, Move::Roll)));
    }

    #[test]
    fn rest_is_trick_55_percent() {
        let mut rng = fastrand::Rng::with_seed(3);
        let n = 20_000;
        let p = Personality::Friendly.trick_chance(false);
        let tricks = (0..n).filter(|_| matches!(pick_rest(&mut rng, p), Rest::Trick(_))).count() as f64 / n as f64;
        assert!((0.52..=0.58).contains(&tricks), "{tricks}");
    }

    #[test]
    fn aloof_trick_chance_is_10_percent_or_60_when_hungry() {
        assert_eq!(Personality::Friendly.trick_chance(true), TRICK_CHANCE);
        assert_eq!(Personality::Aloof.trick_chance(false), 0.10);
        assert_eq!(Personality::Aloof.trick_chance(true), 0.60);
        assert_eq!(Personality::default(), Personality::Friendly);
    }

    /// (애교, 하악, 무시) 비율
    fn click_shares(personality: Personality, hungry: bool) -> (f64, f64, f64) {
        let mut rng = fastrand::Rng::with_seed(11);
        let n = 20_000;
        let (mut react, mut huff, mut ignore) = (0, 0, 0);
        for _ in 0..n {
            match pick_click_outcome(&mut rng, personality, hungry) {
                ClickOutcome::React(_) => react += 1,
                ClickOutcome::Huff => huff += 1,
                ClickOutcome::Ignore { .. } => ignore += 1,
            }
        }
        (react as f64 / n as f64, huff as f64 / n as f64, ignore as f64 / n as f64)
    }

    #[test]
    fn aloof_clicks_are_mostly_huffs_and_ignores() {
        let (react, huff, ignore) = click_shares(Personality::Aloof, false);
        assert!((0.05..=0.15).contains(&react), "{react}");
        assert!((0.38..=0.52).contains(&huff), "{huff}");
        assert!((0.38..=0.52).contains(&ignore), "{ignore}");
        let (react, _, _) = click_shares(Personality::Aloof, true);
        assert!((0.52..=0.68).contains(&react), "hungry {react}");
        assert_eq!(click_shares(Personality::Friendly, false), (1.0, 0.0, 0.0));
        assert_eq!(click_shares(Personality::Playful, false), (1.0, 0.0, 0.0));
        assert_eq!(Personality::Playful.trick_chance(true), PLAYFUL_TRICK_CHANCE);
    }

    #[test]
    fn half_of_ignores_say_so() {
        let mut rng = fastrand::Rng::with_seed(2);
        let (mut said, mut ignores) = (0, 0);
        for _ in 0..20_000 {
            if let ClickOutcome::Ignore { speak } = pick_click_outcome(&mut rng, Personality::Aloof, false) {
                ignores += 1;
                said += speak as usize;
            }
        }
        let share = said as f64 / ignores as f64;
        assert!((0.45..=0.55).contains(&share), "{share}");
    }

    #[test]
    fn move_families() {
        assert_eq!((Move::Walk.family(), Move::Run.family(), Move::Roll.family()), (Family::Move, Family::Move, Family::Roll));
    }

    fn next_share(after_front: bool, last: Option<Move>, want: Move) -> f64 {
        let mut rng = fastrand::Rng::with_seed(9);
        let n = 20_000;
        (0..n).filter(|_| pick_next_move(&mut rng, last, after_front) == want).count() as f64 / n as f64
    }

    #[test]
    fn next_move_keeps_momentum_and_walks_after_tricks() {
        // 50% 그대로 + 50% × pick_move
        let roll_again = next_share(false, Some(Move::Roll), Move::Roll);
        assert!((0.57..=0.63).contains(&roll_again), "{roll_again}");
        let walk_first = next_share(false, None, Move::Walk);
        assert!((0.42..=0.48).contains(&walk_first), "{walk_first}");
        // 애교 뒤: 70% 걷기 + 30% × 45%
        let walk_after_trick = next_share(true, Some(Move::Roll), Move::Walk);
        assert!((0.80..=0.86).contains(&walk_after_trick), "{walk_after_trick}");
    }

    #[test]
    fn every_trick_and_click_reaction_appears() {
        let mut rng = fastrand::Rng::with_seed(5);
        for t in TRICKS {
            assert!((0..600).any(|_| pick_trick(&mut rng) == t), "{t:?}");
        }
        let mut seen = [false; 4];
        for _ in 0..400 {
            let i = match pick_click(&mut rng) {
                ClickReaction::Hop => 0,
                ClickReaction::Trick(Trick::Heart) => 1,
                ClickReaction::Trick(Trick::Wave) => 2,
                ClickReaction::Trick(Trick::Tilt) => 3,
                ClickReaction::Trick(t) => panic!("click gave {t:?}"),
            };
            seen[i] = true;
        }
        assert_eq!(seen, [true; 4]);
    }

    #[test]
    fn move_speeds_and_durations() {
        assert_eq!((Move::Walk.speed_mult(), Move::Run.speed_mult(), Move::Roll.speed_mult()), (0.7, 2.5, 2.0));
        assert_eq!((Move::Walk.secs(), Move::Run.secs(), Move::Roll.secs()), ((4.0, 10.0), (2.0, 5.0), (2.0, 4.0)));
    }

    #[test]
    fn sleep_and_trick_timing() {
        assert_eq!((SLEEP_CHANCE, SLEEP_SECS, TRICK_SECS), (0.12, (10.0, 20.0), (3.0, 5.0)));
        assert_eq!(BEAT_SECS, (0.6, 1.5));
    }

    #[test]
    fn special_numbers_and_lookup() {
        assert_eq!((Special::Arch.rest_chance(), Special::Arch.tickle_chance(), Special::Arch.secs()), (0.08, 0.25, (2.0, 3.0)));
        assert_eq!((Special::Wag.rest_chance(), Special::Wag.tickle_chance(), Special::Wag.secs()), (0.12, 0.30, (2.0, 4.0)));
        assert_eq!(Special::from_manifest(|n| n == "arch"), Some(Special::Arch));
        assert_eq!(Special::from_manifest(|n| n == "wag"), Some(Special::Wag));
        assert_eq!(Special::from_manifest(|_| false), None);
        assert_eq!((Special::Arch.anim_name(), Special::Wag.anim_name()), ("arch", "wag"));
        assert_eq!(Special::from_manifest(|n| n == "neckstretch"), Some(Special::NeckStretch));
        assert_eq!((Special::NeckStretch.anim_name(), Special::NeckStretch.secs()), ("neckstretch", (NECK_STRETCH_SECS, NECK_STRETCH_SECS)));
        assert!(Special::NeckStretch.replaces_roll() && !Special::Arch.replaces_roll() && !Special::Wag.replaces_roll());
    }

    #[test]
    fn dino_specials() {
        // 공룡 pet.json (fire·roar·stomp)이면 공룡 동작 — 시작할 때마다 셋 중 하나를 다시 고른다
        let dino = |n: &str| matches!(n, "fire" | "roar" | "stomp");
        let s = Special::from_manifest(dino).unwrap();
        assert!(s.is_dino() && !Special::Arch.is_dino() && !Special::NeckStretch.is_dino() && !s.replaces_roll());
        assert_eq!([Special::Fire, Special::Roar, Special::Stomp].map(Special::anim_name), ["fire", "roar", "stomp"]);
        for k in [Special::Fire, Special::Roar, Special::Stomp] {
            assert_eq!((k.rest_chance(), k.tickle_chance()), (0.18, 0.30));
            let (lo, hi) = k.secs();
            assert!(1.0 <= lo && lo < hi && hi <= 2.5, "{k:?}");
        }
        assert!((DINO_REST_SHARE.iter().sum::<f64>() - 1.0).abs() < 1e-9 && (DINO_TICKLE_SHARE.iter().sum::<f64>() - 1.0).abs() < 1e-9);
    }

    #[test]
    fn dino_pick_shares() {
        for (tickled, want) in [(false, DINO_REST_SHARE), (true, DINO_TICKLE_SHARE)] {
            for (k, kind) in [Special::Fire, Special::Roar, Special::Stomp].into_iter().enumerate() {
                let got = share(20_000, |r| pick_dino(r, tickled), kind);
                assert!((got - want[k]).abs() < 0.02, "{tickled} {kind:?} {got}");
            }
        }
    }

    #[test]
    fn climb_target_spans_a_quarter_to_the_top() {
        let mut rng = fastrand::Rng::with_seed(4);
        let (bottom, top_limit) = (1040.0, 128.0);
        let range_px = bottom - top_limit;
        let ts: Vec<f64> = (0..5000).map(|_| climb_target(&mut rng, bottom, top_limit)).collect();
        assert!(ts.iter().all(|&t| t >= bottom - range_px && t <= bottom - 0.25 * range_px));
        let (lo, hi) = ts.iter().fold((f64::MAX, f64::MIN), |(lo, hi), &t| (lo.min(t), hi.max(t)));
        assert!(lo < top_limit + 10.0 && hi > bottom - 0.25 * range_px - 10.0, "{lo} {hi}");
        assert_eq!((CLIMB_CHANCE, CLIMB_SPEED, SLIDE_SPEED, WALL_CLIMB_SPEED, CLIMB_HANG_SECS), (0.06, 70.0, 250.0, 90.0, 0.4));
    }

    #[test]
    fn range_stays_inside() {
        let mut rng = fastrand::Rng::with_seed(1);
        assert!((0..1000).all(|_| (1.5..=3.0).contains(&range(&mut rng, (1.5, 3.0)))));
    }
}
