//! 실제처럼 돌린 마주침 (social.rs + brain_meet.rs): 크기·종이 다른 두 동물(하나는 손님)이 스스로 걷고·쉬고·돌아서며
//! 한 바닥(또는 창 위)을 오갈 때, 서로 쪽으로 걷다 만나는 번마다 머리 밟고 넘기가 얼마나 보이는지.
//! 그리고 걷거나 달리던 동물이 서 있는·걷는·달아나는 친구 앞에 닿을 때 (한쪽만 다가가도) 밟고 넘거나 밟고 폴짝 되튀는지.

use super::*;
use crate::behavior::Move;
use crate::brain::meet_moves::MeetPhase;
use crate::brain::{Pet, PetConfig, Standing, State};
use crate::geom::{Rect, WindowInfo};
use crate::test_util::DT;
use crate::visit::Side;
use crate::world::World;

const GROUND: f64 = 1040.0;
const KINDS: [MeetKind; 5] = [MeetKind::Stomp, MeetKind::HighBounce, MeetKind::Pass, MeetKind::Climb, MeetKind::BounceBack];

/// 머리를 밟는 만남 (밟고 넘기, 밟고 폴짝 되튀기)
fn stomp_like(k: MeetKind) -> bool {
    matches!(k, MeetKind::Stomp | MeetKind::BounceBack)
}

/// a가 스스로 걷거나 달려 b 바로 앞에 왔다: 같은 디딤판(같은 높이), 둘 다 한가함, a가 b 쪽을 보고 만나는 거리 안
/// (b는 서 있든, 마주 오든, 앞서 가든 상관없다 — 판정은 social.rs와 따로 여기서 한다)
fn walks_into(a: &Pet, b: &Pet) -> bool {
    let (Some(sa), Some(sb)) = (a.standing, b.standing) else { return false };
    let dx = b.body.x - a.body.x;
    let body = (a.cfg.size + b.cfg.size) / 2.0;
    a.walks_on_surface()
        && a.social_free()
        && b.social_free()
        && !a.has_rider()
        && !b.has_rider()
        && sa.owner == sb.owner
        && (a.body.y - b.body.y).abs() <= 4.0
        && dx.abs() <= MEET_BODIES * body
        && dx.abs() >= MEET_MIN_BODIES * body
        && dx * a.facing > 0.0
}

/// 만남이 끝났을 때 한 몸이 다른 몸 안에 남아 있다 (머리 위에 선 것은 빼고)
fn left_inside(a: &Pet, b: &Pet) -> bool {
    let both_down = a.standing.is_some() && b.standing.is_some() && a.standing.map(|s| s.owner) == b.standing.map(|s| s.owner);
    both_down && !a.is_on_head() && !b.is_on_head() && (a.body.x - b.body.x).abs() < 0.6 * (a.cfg.half_w + b.cfg.half_w)
}

/// 번들 pet.json 그대로 (게임이 쓰는 설정, 청소년, 배율 1)
fn species(id: &str) -> PetConfig {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../ui/pets").join(id).join("pet.json");
    let json = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{path:?}: {e}"));
    let m = crate::manifest::parse_manifest(&json).unwrap_or_else(|e| panic!("{id}: {e:?}"));
    PetConfig::from_manifest(&m, 1.0, m.size_mult(crate::care::Stage::Teenager), crate::care::Stage::Teenager.name())
}

/// owner 위 x에 막 내려선 동물 — 그다음은 스스로 정한다
fn settle(w: &World, c: PetConfig, owner: Owner, x: f64, seed: u64) -> Pet {
    let (ax, ay) = w.anchor(owner).unwrap();
    let mut p = Pet::new(c, x, ay, seed);
    p.standing = Some(Standing { owner, rel_x: x - ax });
    p.state = State::Idle { remaining: 0.5, look: false };
    // 화면 끝 너머에 이어진 친구가 없다 (손님도 집 생각이 들 때까지는 이 화면에서 논다)
    p.set_exit_edges(false, false);
    p
}

#[derive(Debug, Default, Clone, Copy)]
struct Tally {
    /// 둘이 서로 쪽으로 걷다 만난 번 (head_on이 참이 된 마주침)
    head_ons: usize,
    /// 실제로 고른 만남 (KINDS 순서)
    kinds: [usize; 5],
    /// 한쪽이 걷거나 달려 다른 쪽 앞에 닿은 번 (마주 오기 포함, walks_into)
    contacts: usize,
    /// 그중 마주 오기가 아닌 번 (서 있는·앞서 가는 친구에게 다가감)
    one_way: usize,
    /// 머리를 밟은 만남: 전부 / 마주 오기가 아닌 번에서
    stomps: usize,
    one_way_stomps: usize,
    /// 같은 짝의 만남 사이 가장 짧은 틈 (초)
    min_gap: f64,
    /// 만남이 끝났는데 한 몸이 다른 몸 안에 남은 번
    inside: usize,
}

/// 두 동물(b는 아직 집 생각이 없는 손님)을 secs초 동안 앱처럼 돌린다: 틱마다 Social::tick 뒤 각자 update
fn simulate(w: &World, owner: Owner, a: &str, b: &str, (lo, hi): (f64, f64), secs: f64, seed: u64) -> Tally {
    // 손님은 오른쪽 친구 화면에서 왔다. 집 쪽 끝에서 돌아가면 곧 다시 놀러 온다 (새 방문 = 새 동물)
    let guest = |n: u64| {
        let mut g = settle(w, species(b), owner, lo + 0.7 * (hi - lo), seed + 7 + n);
        g.set_guest(Side::Right, 1e9);
        g
    };
    let mut visits = 0;
    let mut pets = vec![settle(w, species(a), owner, lo + 0.3 * (hi - lo), seed), guest(visits)];
    let mut s = Social::new(seed);
    let mut t = Tally::default();
    t.min_gap = f64::INFINITY;
    let (mut facing_off, mut was_meeting) = (false, false);
    // 지금 닿아 있는 번 (마주 오기가 아니면 true)
    let mut touching: Option<bool> = None;
    let mut last_meet = f64::NEG_INFINITY;
    for tick in 0..(secs / DT) as usize {
        let now = tick as f64 * DT;
        let (sa, sb) = (Snap::of(&pets[0]), Snap::of(&pets[1]));
        // 한 번의 마주침: head_on이 처음 참이 된 때부터 둘이 만나는 거리 밖으로 나갈 때까지
        if head_on(&sa, &sb) && !facing_off {
            facing_off = true;
            t.head_ons += 1;
        } else if !near(&sa, &sb, MEET_BODIES) {
            facing_off = false;
        }
        // 한 번의 닿음: 한쪽이 다른 쪽 앞에 처음 온 때부터 만나는 거리 밖으로 나갈 때까지
        if touching.is_none() && (walks_into(&pets[0], &pets[1]) || walks_into(&pets[1], &pets[0])) {
            let one_way = !head_on(&sa, &sb);
            touching = Some(one_way);
            t.contacts += 1;
            t.one_way += one_way as usize;
        } else if !near(&sa, &sb, MEET_BODIES) && !pets.iter().any(Pet::is_meeting) {
            touching = None;
        }
        let meeting = pets.iter().any(Pet::is_meeting);
        // 달리던 동물은 만나는 거리보다 조금 일찍 뛴다 — 그것도 한 번 닿은 것이다
        if meeting && !was_meeting && touching.is_none() {
            let one_way = !head_on(&sa, &sb);
            touching = Some(one_way);
            t.contacts += 1;
            t.one_way += one_way as usize;
        }
        if meeting && !was_meeting {
            if let Some(k) = pets.iter().find_map(Pet::meet_kind) {
                t.kinds[KINDS.iter().position(|x| *x == k).unwrap()] += 1;
                if stomp_like(k) {
                    t.stomps += 1;
                    t.one_way_stomps += (touching == Some(true)) as usize;
                }
            }
            t.min_gap = t.min_gap.min(now - last_meet);
            last_meet = now;
        }
        if !meeting && was_meeting && left_inside(&pets[0], &pets[1]) {
            t.inside += 1;
        }
        was_meeting = meeting;
        s.tick(DT, &mut pets);
        for p in pets.iter_mut() {
            p.update(DT, w);
        }
        if pets[1].is_away() {
            visits += 1;
            pets[1] = guest(visits);
            // 새 손님은 새 짝이다
            last_meet = f64::NEG_INFINITY;
        }
    }
    t
}

#[test]
fn walking_into_each_other_often_ends_in_a_head_stomp_over() {
    let floor = World::new(vec![], vec![Rect::new(0.0, 0.0, 1920.0, GROUND)], 128.0);
    let window = WindowInfo { id: 1, rect: Rect::new(400.0, 500.0, 1300.0, 1000.0) };
    let desk = World::new(vec![window], vec![Rect::new(0.0, 0.0, 1920.0, GROUND)], 128.0);
    let runs = [
        (&floor, Owner::Ground(0), "cat", "rudolph", (0.0, 1920.0)),
        (&floor, Owner::Ground(0), "rabbit", "santa", (0.0, 1920.0)),
        (&floor, Owner::Ground(0), "dino_green", "duck", (0.0, 1920.0)),
        (&desk, Owner::Window(1), "frog", "persian", (400.0, 1300.0)),
    ];
    let mut all = Tally { min_gap: f64::INFINITY, ..Tally::default() };
    for (i, &(w, owner, a, b, span)) in runs.iter().enumerate() {
        let t = simulate(w, owner, a, b, span, 3600.0, 11 + i as u64);
        eprintln!("{a}+{b}: {t:?}");
        all.head_ons += t.head_ons;
        all.contacts += t.contacts;
        all.one_way += t.one_way;
        all.stomps += t.stomps;
        all.one_way_stomps += t.one_way_stomps;
        all.inside += t.inside;
        all.min_gap = all.min_gap.min(t.min_gap);
        for k in 0..KINDS.len() {
            all.kinds[k] += t.kinds[k];
        }
    }
    let stomps = all.kinds[0];
    let share = stomps as f64 / all.head_ons as f64;
    eprintln!("all: {all:?} stomps per head-on {share:.2}");
    assert!(all.head_ons > 60, "they do walk into each other: {all:?}");
    // 서로 쪽으로 걷다 만나면 세 번에 한 번쯤은 머리를 밟고 넘는다
    assert!(share > 0.25, "stomps per head-on {share:.2}: {all:?}");
    assert!(all.kinds[1..].iter().all(|&k| k < stomps), "the stomp is the usual meet: {all:?}");
    // 한쪽만 다가가도 (서 있는 친구, 앞서 가는 친구) 머리를 밟는다
    let one_way = all.one_way_stomps as f64 / all.one_way as f64;
    let overall = all.stomps as f64 / all.contacts as f64;
    eprintln!("contacts {} (one-way {}), stomp-like per contact {overall:.2}, per one-way contact {one_way:.2}", all.contacts, all.one_way);
    assert!(all.one_way > 30, "pets do walk into a friend who isn't coming their way: {all:?}");
    assert!(one_way > 0.25, "stomps per one-way contact {one_way:.2}: {all:?}");
    // 재미있지만 늘 그러진 않는다: 닿는 번의 셋에 하나 ~ 절반쯤
    assert!((0.3..=0.6).contains(&overall), "stomp-like per contact {overall:.2}: {all:?}");
    // 같은 짝이 쉬지 않고 계속 밟지 않고, 끝나면 몸이 겹쳐 있지 않다
    assert!(all.min_gap >= MEET_COOLDOWN.0 - 1.0, "same pair met again after {:.1}s: {all:?}", all.min_gap);
    assert_eq!(all.inside, 0, "{all:?}");
}

/// 친구가 하던 일
#[derive(Debug, Clone, Copy, PartialEq)]
enum Other {
    /// 서서 쉬는 중
    Standing,
    /// 마주 걸어온다
    Toward,
    /// 같은 쪽으로 앞서 걷는다
    Away,
}

/// 한 번 다가가 본 결과
#[derive(Debug, Default, Clone, Copy)]
struct Trial {
    contact: bool,
    kind: Option<MeetKind>,
    /// 뛴 쪽이 누구였나 (다가간 쪽이면 true)
    mover_jumped: bool,
    /// 밟힌 쪽이 납작해졌다
    squashed: bool,
    /// 밟힌 쪽이 (내려앉은) 나는 동물
    flyer_squashed: bool,
    /// 만남이 끝났을 때 한 몸이 다른 몸 안에 남았다
    inside: bool,
    /// 만남이 4초 안에 끝나지 않았다
    stuck: bool,
}

/// owner 위에서 mover(걷기 또는 달리기)가 오른쪽의 친구에게 다가간다. guest_moves면 다가가는 쪽이 손님이다
#[allow(clippy::too_many_arguments)]
fn approach(w: &World, owner: Owner, (lo, hi): (f64, f64), mover: &str, other: &str, run: bool, how: Other, guest_moves: bool, seed: u64) -> Trial {
    let x = lo + 0.25 * (hi - lo);
    let mut m = settle(w, species(mover), owner, x, seed);
    let gap = 4.0 * (m.cfg.size + species(other).size) / 2.0;
    let mut o = settle(w, species(other), owner, x + gap, seed + 501);
    m.facing = 1.0;
    m.state = State::Move { kind: if run { Move::Run } else { Move::Walk }, remaining: 60.0 };
    match how {
        Other::Standing => o.state = State::Idle { remaining: 30.0, look: false },
        Other::Toward | Other::Away => {
            o.facing = if how == Other::Toward { -1.0 } else { 1.0 };
            o.state = State::Move { kind: Move::Walk, remaining: 60.0 };
        }
    }
    let (mi, oi) = if guest_moves { (1, 0) } else { (0, 1) };
    let mut pets = if guest_moves { vec![o, m] } else { vec![m, o] };
    pets[1].set_guest(Side::Right, 1e9);
    let mut s = Social::new(seed);
    let mut t = Trial::default();
    let mut started: Option<f64> = None;
    let mut was_meeting = false;
    for tick in 0..(10.0 / DT) as usize {
        let now = tick as f64 * DT;
        if t.kind.is_none() && walks_into(&pets[mi], &pets[oi]) {
            t.contact = true;
        }
        s.tick(DT, &mut pets);
        for p in pets.iter_mut() {
            p.update(DT, w);
        }
        let meeting = pets.iter().any(Pet::is_meeting);
        if meeting && started.is_none() {
            // 달리던 동물은 만나는 거리보다 조금 일찍 뛴다
            t.contact = true;
            started = Some(now);
            t.kind = pets.iter().find_map(Pet::meet_kind);
            t.mover_jumped = pets[mi].meet_kind().is_some();
        }
        for p in &pets {
            if matches!(p.state, State::Meet { phase: MeetPhase::Squash, .. }) {
                t.squashed = true;
                t.flyer_squashed |= p.cfg.flies;
            }
        }
        if was_meeting && !meeting {
            t.inside |= left_inside(&pets[0], &pets[1]);
            break;
        }
        was_meeting = meeting;
        if started.is_some_and(|at| now - at > 4.0) {
            t.stuck = true;
            break;
        }
    }
    t
}

/// 걷거나 달리던 동물이 친구 앞에 닿는 장면 여럿: 내 동물·손님이 번갈아 다가가고, 바닥과 창 위.
/// 서 있는 친구에는 내려앉은 새도 끼운다 (새는 걷지 않으니 걷는 친구 자리에는 오리)
fn approaches(run: bool, how: Other) -> Vec<Trial> {
    let floor = World::new(vec![], vec![Rect::new(0.0, 0.0, 1920.0, GROUND)], 128.0);
    let window = WindowInfo { id: 1, rect: Rect::new(400.0, 500.0, 1300.0, 1000.0) };
    let desk = World::new(vec![window], vec![Rect::new(0.0, 0.0, 1920.0, GROUND)], 128.0);
    let pairs = [
        (&floor, Owner::Ground(0), "cat", "rudolph", (0.0, 1920.0)),
        (&floor, Owner::Ground(0), "santa", "rabbit", (0.0, 1920.0)),
        (&floor, Owner::Ground(0), "dino_green", if how == Other::Standing { "bird" } else { "duck" }, (0.0, 1920.0)),
        (&desk, Owner::Window(1), "frog", "persian", (400.0, 1300.0)),
    ];
    let mut out = Vec::new();
    for seed in 0..16u64 {
        for &(w, owner, a, b, span) in &pairs {
            out.push(approach(w, owner, span, a, b, run, how, seed % 2 == 1, 100 + seed));
        }
    }
    out
}

/// 다가간 번마다 밟기가 적당히 자주 나오고, 밟힌 쪽은 납작해지고, 아무도 끼거나 멈추지 않는다
fn check_approaches(name: &str, trials: &[Trial]) {
    let contacts = trials.iter().filter(|t| t.contact).count();
    let stomps: Vec<&Trial> = trials.iter().filter(|t| t.contact && t.kind.is_some_and(stomp_like)).collect();
    let overs = stomps.iter().filter(|t| t.kind == Some(MeetKind::Stomp)).count();
    let hops = stomps.len() - overs;
    let share = stomps.len() as f64 / contacts as f64;
    let by_mover = stomps.iter().filter(|t| t.mover_jumped).count();
    eprintln!("{name}: contacts {contacts}/{} stomp-like {} (over {overs}, hop {hops}, by mover {by_mover}) share {share:.2}", trials.len(), stomps.len());
    assert!(contacts >= trials.len() * 3 / 4, "{name}: they reach each other: {contacts}/{}", trials.len());
    assert!((0.3..=0.8).contains(&share), "{name}: stomp-like per contact {share:.2}");
    assert!(overs >= hops, "{name}: stomping over is the usual one ({overs} over, {hops} hop)");
    assert!(stomps.iter().all(|t| t.squashed), "{name}: the stomped friend goes flat");
    assert!(trials.iter().all(|t| !t.inside), "{name}: nobody is left inside the other");
    assert!(trials.iter().all(|t| !t.stuck), "{name}: every meet ends");
}

#[test]
fn a_walker_stomps_a_standing_friend() {
    let trials = approaches(false, Other::Standing);
    check_approaches("walk -> standing", &trials);
    // 서 있는 쪽은 뛰지 않는다 — 다가간 쪽이 밟는다 (내려앉은 새도 밟힌다)
    assert!(trials.iter().filter(|t| t.kind.is_some_and(stomp_like)).all(|t| t.mover_jumped));
    assert!(trials.iter().any(|t| t.flyer_squashed), "a perched bird can be stomped on too");
}

#[test]
fn a_runner_stomps_a_walker_coming_its_way() {
    check_approaches("run -> walking toward", &approaches(true, Other::Toward));
}

#[test]
fn a_runner_stomps_a_standing_friend() {
    check_approaches("run -> standing", &approaches(true, Other::Standing));
}

#[test]
fn a_runner_stomps_a_friend_walking_ahead() {
    check_approaches("run -> walking ahead", &approaches(true, Other::Away));
}
