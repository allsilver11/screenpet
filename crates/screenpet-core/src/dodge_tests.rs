use super::*;

const AREA: Rect = Rect { left: 0.0, top: 0.0, right: 1600.0, bottom: 900.0 };

fn pet_at(x: f64) -> PetBox {
    PetBox { x, y: AREA.bottom, half_w: 38.0, height: 100.0 }
}

fn playing(seed: u64) -> Dodge {
    let mut d = Dodge::new(AREA, 1.0, seed);
    assert!(d.start());
    d
}

/// 동물을 늘 똥이 없는 쪽으로 옮기며 secs초 돌린다 (끝나면 그 결과)
fn run_dodging(d: &mut Dodge, secs: f64) -> Option<Over> {
    let dt = 1.0 / 60.0;
    for _ in 0..(secs / dt) as usize {
        // 판 양 끝 중 가까운 똥이 적은 쪽
        let near = |x: f64| d.poops().iter().filter(|p| (p.x - x).abs() < 200.0).count();
        let x = if near(100.0) <= near(1500.0) { 100.0 } else { 1500.0 };
        if let Some(o) = d.tick(dt, Some(pet_at(x))) {
            return Some(o);
        }
    }
    None
}

#[test]
fn nothing_happens_before_start_or_after_the_end() {
    let mut d = Dodge::new(AREA, 1.0, 1);
    assert_eq!(d.phase(), Phase::Ready);
    assert_eq!(d.tick(5.0, Some(pet_at(800.0))), None);
    assert!(d.poops().is_empty());
    assert!(d.start());
    assert!(!d.start(), "already playing");
}

#[test]
fn nothing_falls_during_the_grace() {
    let mut d = playing(1);
    for _ in 0..50 {
        d.tick(0.016, Some(pet_at(800.0)));
    }
    assert!(d.poops().is_empty());
    d.tick(0.3, Some(pet_at(800.0)));
    assert!(!d.poops().is_empty());
    // 위에서 나타난다
    assert!(d.poops().iter().all(|p| p.y < AREA.top + 100.0 && p.x >= AREA.left && p.x <= AREA.right));
}

#[test]
fn standing_still_gets_hit_and_counts_what_fell() {
    let mut d = playing(7);
    let mut over = None;
    for _ in 0..60 * 120 {
        if let Some(o) = d.tick(1.0 / 60.0, Some(pet_at(800.0))) {
            over = Some(o);
            break;
        }
    }
    let o = over.expect("aimed poops hit a pet that never moves");
    assert!(o.hit);
    assert_eq!(o.score, d.dodged());
    assert_eq!(d.phase(), Phase::Over);
    // 끝난 판은 더 움직이지 않는다
    let ys: Vec<f64> = d.poops().iter().map(|p| p.y).collect();
    assert_eq!(d.tick(1.0, Some(pet_at(800.0))), None);
    assert_eq!(ys, d.poops().iter().map(|p| p.y).collect::<Vec<_>>());
}

#[test]
fn poops_reaching_the_floor_are_dodged() {
    let mut d = playing(3);
    // 동물은 판 안이지만 바닥 아래 — 아무것도 맞지 않는다
    let ghost = PetBox { x: 800.0, y: AREA.bottom + 1000.0, half_w: 0.0, height: 0.0 };
    for _ in 0..60 * 10 {
        assert_eq!(d.tick(1.0 / 60.0, Some(ghost)), None);
    }
    assert!(d.dodged() > 5, "{}", d.dodged());
    assert!(d.poops().iter().all(|p| p.y - p.r < AREA.bottom));
}

#[test]
fn a_fast_poop_cannot_tunnel_through_the_pet() {
    let mut d = playing(1);
    d.poops.push(Poop { id: 99, x: 800.0, y: 700.0, r: 17.0, vy: 50_000.0 });
    d.spawn_in = 100.0;
    let o = d.tick(0.1, Some(pet_at(800.0))).expect("hit");
    assert!(o.hit);
}

#[test]
fn a_poop_beside_the_pet_misses() {
    let mut d = playing(1);
    d.poops.push(Poop { id: 99, x: 800.0 + 38.0 + 17.0 + 5.0, y: 700.0, r: 17.0, vy: 2000.0 });
    d.spawn_in = 100.0;
    assert_eq!(d.tick(0.2, Some(pet_at(800.0))), None);
    assert_eq!(d.dodged(), 1);
}

#[test]
fn leaving_the_area_ends_the_game_without_a_hit() {
    let mut d = playing(1);
    d.tick(0.5, Some(pet_at(800.0)));
    assert_eq!(d.tick(0.016, None), Some(Over { score: 0, hit: false }));
    let mut d = playing(1);
    assert_eq!(d.tick(0.016, Some(pet_at(AREA.right + 1.0))), Some(Over { score: 0, hit: false }));
}

#[test]
fn it_gets_harder_over_time() {
    let mut d = playing(1);
    let (v0, e0) = (d.fall_speed(), d.spawn_every());
    d.t = 60.0;
    assert!(d.fall_speed() > 2.0 * v0 && d.spawn_every() < 0.3 * e0);
    d.t = 10_000.0;
    assert_eq!(d.fall_speed(), FALL_MAX);
    assert!(d.spawn_every() >= SPAWN_MIN * 0.5);
}

#[test]
fn wider_screens_rain_more_and_hidpi_scales() {
    let narrow = Dodge::new(Rect::new(0.0, 0.0, 800.0, 900.0), 1.0, 1);
    let wide = Dodge::new(Rect::new(0.0, 0.0, 3200.0, 900.0), 1.0, 1);
    assert!(wide.spawn_every() < narrow.spawn_every());
    // 같은 논리 크기의 2배 화면: 간격은 같고 속도는 2배
    let hidpi = Dodge::new(Rect::new(0.0, 0.0, 3200.0, 1800.0), 2.0, 1);
    let base = Dodge::new(AREA, 1.0, 1);
    assert_eq!(hidpi.spawn_every(), base.spawn_every());
    assert_eq!(hidpi.fall_speed(), 2.0 * base.fall_speed());
}

#[test]
fn a_careful_player_lasts_and_scores() {
    let mut d = playing(5);
    let o = run_dodging(&mut d, 20.0);
    assert!(o.is_none() || d.dodged() > 0, "{o:?}");
    assert!(d.dodged() >= 10, "{}", d.dodged());
}

#[test]
fn bad_dt_is_ignored_and_memory_is_bounded() {
    let mut d = playing(1);
    assert_eq!(d.tick(f64::NAN, Some(pet_at(800.0))), None);
    assert_eq!(d.tick(-1.0, Some(pet_at(800.0))), None);
    assert!(d.poops().is_empty());
    // 아주 긴 틱 한 번: 상한까지만 떨어뜨린다
    let ghost = PetBox { x: 800.0, y: AREA.bottom + 1000.0, half_w: 0.0, height: 0.0 };
    d.t = 1000.0;
    d.tick(1000.0, Some(ghost));
    assert!(d.poops().len() <= MAX_POOPS);
}

#[test]
fn restart_clears_the_board() {
    let mut d = playing(7);
    while d.tick(1.0 / 60.0, Some(pet_at(800.0))).is_none() {}
    assert!(d.start());
    assert_eq!((d.phase(), d.dodged(), d.poops().len()), (Phase::Play, 0, 0));
}

#[test]
fn score_asks_roundtrip_and_reject_junk() {
    assert_eq!(score_ask(42), "dodge:42");
    assert_eq!(parse_score_ask("dodge:42"), Some(42));
    assert_eq!(parse_score_ask(&score_ask(u32::MAX)), Some(MAX_SCORE));
    for junk in ["dodge:", "dodge:-1", "dodge:1e3", "dodge: 4", "dodge:99999999", "invite", "busy", "Dodge:3"] {
        assert_eq!(parse_score_ask(junk), None, "{junk}");
    }
}

#[test]
fn ranking_sorts_by_score_with_me_first_on_ties() {
    let r = ranking("나", 12, &[("민수".into(), 30), ("앤".into(), 12), ("가람".into(), 3)]);
    let names: Vec<_> = r.iter().map(|x| (x.name.as_str(), x.score, x.me)).collect();
    assert_eq!(names, [("민수", 30, false), ("나", 12, true), ("앤", 12, false), ("가람", 3, false)]);
}
