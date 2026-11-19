//! typhoon.rs 테스트 — 돌풍 박자, 회오리가 돌아다니고 빨아들이고 내던지기, 친구 화면까지 닿는 포물선, 태풍 동안의 회오리 차례

use super::*;
use crate::physics::{MAX_THROW_SPEED_LOGICAL, THROW_GRAVITY_LOGICAL};

const DT: f64 = 1.0 / 60.0;
const WA: Rect = Rect { left: 0.0, top: 0.0, right: 1920.0, bottom: 1040.0 };

/// 땅 위 x에 놓인 물건 하나
fn ground_thing(id: u64, x: f64) -> Loose {
    Loose { thing: Thing::Object(id), x, y: WA.bottom, cross: None }
}

/// 회오리를 x 자리에 멈춰 세움 (테스트가 자리를 정함)
fn parked(x: f64, seed: u64) -> Tornado {
    let mut t = Tornado::new(WA, WA, 1.0, seed);
    t.park(x);
    t
}

/// secs초 동안 돌림: 틱마다 나온 것들
fn spin(t: &mut Tornado, loose: &[Loose], secs: f64) -> Vec<Whirl> {
    let mut all = Vec::new();
    let mut out = Vec::new();
    for _ in 0..(secs / DT) as usize {
        out.clear();
        t.step(DT, loose, &mut out);
        all.extend(out.iter().copied());
    }
    all
}

fn flings(all: &[Whirl]) -> Vec<(Thing, f64, f64, bool)> {
    all.iter()
        .filter_map(|w| match *w {
            Whirl::Fling { thing, vx, vy, cross } => Some((thing, vx, vy, cross)),
            Whirl::Hold { .. } => None,
        })
        .collect()
}

#[test]
fn gusts_come_every_few_seconds_mostly_one_way_and_blow_harder_than_the_steady_wind() {
    let mut g = Gusts::new(7);
    let calm = g.wind().abs();
    assert!(calm > 0.3, "태풍은 늘 바람이 붊: {calm}");
    let mut starts = Vec::new();
    let mut peak: f64 = 0.0;
    let mut clock = 0.0;
    for _ in 0..(600.0 / DT) as usize {
        clock += DT;
        if let Some(gust) = g.tick(DT) {
            assert!((0.3..=1.0).contains(&gust.strength), "{gust:?}");
            assert!(gust.dir == 1.0 || gust.dir == -1.0);
            starts.push((clock, gust.dir));
        }
        peak = peak.max(g.wind().abs());
        assert!(g.wind().abs() <= MAX_WIND + 1e-9);
    }
    // 10분에 3~7초 간격 + 돌풍 길이 → 대략 60~140번
    assert!((50..=160).contains(&starts.len()), "{}", starts.len());
    let gaps: Vec<f64> = starts.windows(2).map(|p| p[1].0 - p[0].0).collect();
    assert!(gaps.iter().all(|&gap| gap >= GUST_EVERY.0), "{gaps:?}");
    assert!(peak > calm + 0.3, "돌풍은 더 셈: {peak} vs {calm}");
    let main = starts.iter().filter(|s| s.1 == starts[0].1).count();
    assert!(main * 2 > starts.len(), "대개 한쪽으로: {main}/{}", starts.len());
}

#[test]
fn a_pet_mostly_braces_tumbles_or_gets_blown_off_its_feet_harder_in_stronger_gusts() {
    let count = |strength: f64, want: Stagger| (0..1000).filter(|i| gust_reaction(strength, *i as f64 / 1000.0) == want).count();
    for want in [Stagger::Lean, Stagger::Tumble, Stagger::Blown] {
        assert!(count(1.0, want) > 100, "{want:?} 자주 나옴");
    }
    assert!(count(1.0, Stagger::Blown) > count(0.3, Stagger::Blown) * 2);
    assert!(count(0.3, Stagger::Lean) > count(1.0, Stagger::Lean));
}

#[test]
fn a_gust_blows_loose_things_sideways_and_up_with_the_wind() {
    for roll in [0.0, 0.5, 0.99] {
        let (vx, vy) = blow_velocity(1.0, 1.0, 1.5, roll);
        assert!(vx > 300.0 * 1.5 && vy < 0.0, "{vx} {vy}");
        let (lx, _) = blow_velocity(-1.0, 0.3, 1.0, roll);
        assert!(lx < 0.0 && lx.abs() < vx.abs() / 1.5);
    }
}

#[test]
fn on_side_by_side_monitors_the_tornado_wanders_over_to_the_next_one_and_back() {
    let left = Rect::new(0.0, 0.0, 1920.0, 1040.0);
    let right = Rect::new(1920.0, 0.0, 3840.0, 1080.0);
    let bounds = Rect::new(0.0, 0.0, 3840.0, 1080.0);
    let mut t = Tornado::new(left, bounds, 1.0, 5);
    let mut out = Vec::new();
    let (mut on_left, mut on_right) = (false, false);
    for _ in 0..(300.0 / DT) as usize {
        t.set_areas(&[left, right]);
        t.step(DT, &[], &mut out);
        assert!(t.x() >= bounds.left && t.x() <= bounds.right, "{}", t.x());
        assert!(t.area() == left || t.area() == right);
        assert!(t.x() >= t.area().left - 1.0 && t.x() <= t.area().right + 1.0, "자리는 지금 모니터 안: {} {:?}", t.x(), t.area());
        assert_eq!(t.ground(), t.area().bottom, "그 모니터 바닥을 따라감");
        on_left |= t.area() == left;
        on_right |= t.area() == right;
    }
    assert!(on_left && on_right, "듀얼 모니터면 옆 모니터로 넘어감");
}

#[test]
fn a_tornado_does_not_jump_to_a_monitor_that_is_not_next_to_it() {
    let left = Rect::new(0.0, 0.0, 1920.0, 1040.0);
    let far = Rect::new(2500.0, 0.0, 4420.0, 1040.0);
    let mut t = Tornado::new(left, Rect::new(0.0, 0.0, 4420.0, 1040.0), 1.0, 5);
    let mut out = Vec::new();
    for _ in 0..(120.0 / DT) as usize {
        t.set_areas(&[left, far]);
        t.step(DT, &[], &mut out);
        assert_eq!(t.area(), left);
    }
}

#[test]
fn the_tornado_roams_its_whole_monitor_changing_speed_and_direction_but_never_leaves_it() {
    let area = Rect::new(1920.0, 0.0, 3840.0, 1080.0);
    let bounds = Rect::new(0.0, 0.0, 3840.0, 1080.0);
    let mut t = Tornado::new(area, bounds, 1.0, 3);
    assert_eq!(t.ground(), 1080.0);
    let (mut lo, mut hi) = (f64::MAX, f64::MIN);
    let mut turns = 0;
    let mut last_dir = t.vx().signum();
    let mut out = Vec::new();
    for _ in 0..(90.0 / DT) as usize {
        t.step(DT, &[], &mut out);
        assert!(t.x() >= area.left && t.x() <= area.right, "{}", t.x());
        lo = lo.min(t.x());
        hi = hi.max(t.x());
        if t.vx().signum() != last_dir && t.vx() != 0.0 {
            turns += 1;
            last_dir = t.vx().signum();
        }
    }
    assert!(hi - lo > 1000.0, "여기저기 돌아다님: {lo}..{hi}");
    assert!(turns >= 6, "자주 방향을 바꿈: {turns}");
    assert!(out.is_empty());
}

#[test]
fn things_near_the_funnel_get_sucked_in_spun_upward_and_flung_out() {
    let mut t = parked(900.0, 1);
    let near = ground_thing(1, 900.0 + 0.8 * PULL_RADIUS);
    let far = ground_thing(2, 900.0 + PULL_RADIUS + 60.0);
    let high = Loose { y: WA.bottom - PULL_HEIGHT - 40.0, ..ground_thing(3, 900.0) };
    let all = spin(&mut t, &[near, far, high], SPIN_SECS.1 + 0.2);
    let holds: Vec<(f64, f64)> = all.iter().filter_map(|w| match *w {
        Whirl::Hold { thing: Thing::Object(1), x, y, .. } => Some((x, y)),
        _ => None,
    }).collect();
    assert!(holds.len() as f64 >= SPIN_SECS.0 / DT - 2.0, "{}", holds.len());
    assert!(all.iter().all(|w| w.thing() == Thing::Object(1)), "먼 것·높은 것은 그대로");
    // 깔때기를 따라 빙글빙글 올라감: 양쪽을 오가고, 점점 높이
    let top = holds.iter().map(|h| h.1).fold(f64::MAX, f64::min);
    assert!(WA.bottom - top > 0.6 * LIFT, "높이 올라감: {top}");
    assert!(holds.iter().any(|h| h.0 < 900.0) && holds.iter().any(|h| h.0 > 900.0), "둘레를 돎");
    assert!(holds.iter().all(|h| (h.0 - 900.0).abs() <= 0.8 * PULL_RADIUS + 1.0));
    let f = flings(&all);
    assert_eq!(f.len(), 1, "한 번 내던짐");
    let (_, vx, vy, cross) = f[0];
    assert!(!cross, "친구 쪽 끝이 없음");
    assert!(vx.abs() >= 300.0 && vy < -250.0, "{vx} {vy}");
    assert_eq!(t.riders(), 0);
}

#[test]
fn a_thing_that_was_just_flung_is_not_caught_again_right_away() {
    let mut t = parked(900.0, 2);
    let thing = ground_thing(1, 950.0);
    let mut out = Vec::new();
    while !out.iter().any(|w| matches!(w, Whirl::Fling { .. })) {
        out.clear();
        t.step(DT, &[thing], &mut out);
    }
    let soon = spin(&mut t, &[thing], RECATCH_SECS - 0.3);
    assert!(soon.is_empty(), "곧장 다시 빨려 들지 않음");
    let later = spin(&mut t, &[thing], 0.6);
    assert!(!later.is_empty(), "잠깐 뒤엔 다시");
}

#[test]
fn a_rider_that_goes_missing_is_let_go_without_a_fling() {
    let mut t = parked(900.0, 3);
    let thing = ground_thing(1, 950.0);
    spin(&mut t, &[thing], 0.5);
    assert!(t.holds(Thing::Object(1)));
    // 사용자가 잡았거나 치웠음 — 후보에서 빠짐
    let gone = spin(&mut t, &[], 1.0);
    assert!(gone.is_empty());
    assert_eq!(t.riders(), 0);
}

#[test]
fn only_so_many_ride_at_once() {
    let mut t = parked(900.0, 4);
    let many: Vec<Loose> = (0..MAX_RIDERS as u64 + 5).map(|i| ground_thing(i, 900.0 + i as f64 * 10.0)).collect();
    spin(&mut t, &many, 0.3);
    assert_eq!(t.riders(), MAX_RIDERS);
}

/// 포물선을 따라 날려 보고 그 끝 x에 닿는지 (바닥 floor에 닿기 전)
fn reaches(from: (f64, f64), (vx, vy): (f64, f64), edge_x: f64, floor: f64, dpi: f64) -> bool {
    let g = THROW_GRAVITY_LOGICAL * dpi;
    let (mut x, mut y, mut vy) = (from.0, from.1, vy);
    for _ in 0..2000 {
        x += vx * DT;
        vy += g * DT;
        y += vy * DT;
        if (vx > 0.0 && x >= edge_x) || (vx < 0.0 && x <= edge_x) {
            return true;
        }
        if y > floor {
            return false;
        }
    }
    false
}

#[test]
fn a_cross_throw_reaches_the_edge_before_landing_within_the_throw_speed_limit() {
    for dpi in [1.0, 1.5, 2.0] {
        for d in [200.0, 600.0, 1000.0] {
            for h in [0.0, 120.0, 240.0] {
                let from = (2000.0 * dpi, (1000.0 - h) * dpi);
                for edge in [from.0 + d * dpi, from.0 - d * dpi] {
                    let v = cross_throw(from, edge, 1000.0 * dpi, dpi).unwrap_or_else(|| panic!("{dpi} {d} {h}"));
                    assert!(v.0.hypot(v.1) <= MAX_THROW_SPEED_LOGICAL * dpi, "{v:?}");
                    assert!(v.1 < 0.0 && v.0.signum() == (edge - from.0).signum());
                    assert!(reaches(from, v, edge, 1000.0 * dpi, dpi), "{dpi} {d} {h} {v:?}");
                }
            }
        }
    }
    assert_eq!(cross_throw((100.0, 1000.0), 3000.0, 1000.0, 1.0), None, "너무 멀면 못 던짐");
}

#[test]
fn near_a_friend_edge_most_flings_go_over_to_the_friend() {
    let mut crossed = 0;
    let mut local = 0;
    for seed in 0..40 {
        let mut t = parked(1500.0, seed);
        let thing = Loose { cross: Some(Side::Right), ..ground_thing(1, 1500.0) };
        for (_, vx, vy, cross) in flings(&spin(&mut t, &[thing], SPIN_SECS.1 + 0.1)) {
            if cross {
                crossed += 1;
                assert!(vx > 0.0);
                let from = t.last_fling_from().unwrap();
                assert!(reaches(from, (vx, vy), WA.right, WA.bottom, 1.0), "{from:?} {vx} {vy}");
            } else {
                local += 1;
            }
        }
    }
    assert!(crossed > 40 / 2, "자주 넘어감: {crossed} vs {local}");
    assert!(local > 0, "가끔은 이 화면에");
}

#[test]
fn pets_cross_to_a_friend_at_most_once_in_a_while_but_objects_keep_going() {
    let mut t = parked(1600.0, 9);
    let pet = Loose { thing: Thing::Pet(7), x: 1600.0, y: WA.bottom, cross: Some(Side::Right) };
    let mut pet_cross = Vec::new();
    let mut clock = 0.0;
    let mut out = Vec::new();
    for _ in 0..(PET_CROSS_GAP * 2.0 / DT) as usize {
        clock += DT;
        out.clear();
        t.step(DT, &[pet], &mut out);
        for w in &out {
            if let Whirl::Fling { cross: true, .. } = w {
                pet_cross.push(clock);
            }
        }
    }
    assert!(!pet_cross.is_empty() && pet_cross.len() <= 2, "{pet_cross:?}");
    assert!(pet_cross.windows(2).all(|p| p[1] - p[0] >= PET_CROSS_GAP));
    // 물건은 날마다 예산(앱의 TossBudget)에만 걸림
    let mut t = parked(1600.0, 9);
    let obj = Loose { cross: Some(Side::Right), ..ground_thing(1, 1600.0) };
    let objects = flings(&spin(&mut t, &[obj], PET_CROSS_GAP * 2.0)).iter().filter(|f| f.3).count();
    assert!(objects >= 3 && objects > pet_cross.len(), "{objects}");
}

#[test]
fn a_storm_brings_tornadoes_one_after_another_and_lets_riders_go_when_one_dies() {
    let mut s = Storm::new(5);
    let mut out = Vec::new();
    let mut seen = 0;
    let mut had = false;
    let mut released = 0;
    let thing = ground_thing(1, 960.0);
    for _ in 0..(400.0 / DT) as usize {
        out.clear();
        // 회오리 자리 바로 옆에 늘 물건 하나
        let loose = s.tornado().map(|t| vec![Loose { x: t.x() + 20.0, ..thing }]).unwrap_or_default();
        s.tick(DT, &[WA], WA, 1.0, &loose, &mut out);
        let has = s.tornado().is_some();
        if has && !had {
            seen += 1;
        }
        if had && !has {
            released += out.iter().filter(|w| matches!(w, Whirl::Fling { .. })).count();
        }
        had = has;
    }
    assert!(seen >= 3, "회오리가 자꾸 옴: {seen}");
    assert!(released > 0, "사라지는 회오리는 든 것을 놓아줌");
    // 모니터가 없으면 회오리도 없음
    let mut s = Storm::new(5);
    for _ in 0..(30.0 / DT) as usize {
        s.tick(DT, &[], WA, 1.0, &[], &mut out);
    }
    assert!(s.tornado().is_none());
}

#[test]
fn a_funnel_just_past_a_monitor_edge_still_shows_on_that_monitor() {
    let wa = Rect::new(0.0, 0.0, 1920.0, 1040.0);
    assert!(straddles(1960.0, &wa, 1.0), "오른쪽 끝에 걸침");
    assert!(straddles(-40.0, &wa, 1.0), "왼쪽 끝에 걸침");
    assert!(!straddles(900.0, &wa, 1.0), "안쪽은 제 모니터가 그림");
    assert!(!straddles(2400.0, &wa, 1.0), "멀리 있으면 안 보임");
    assert!(straddles(2100.0, &wa, 2.0) && !straddles(2100.0, &wa, 1.0), "배율만큼 넓음");
}
