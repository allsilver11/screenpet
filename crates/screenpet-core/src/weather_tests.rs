use super::*;
use crate::surfaces::Owner;
use crate::world::World;

fn win(id: u64, l: f64, t: f64, r: f64, b: f64) -> WindowInfo {
    WindowInfo { id, rect: Rect::new(l, t, r, b) }
}

const WA: Rect = Rect { left: 0.0, top: 0.0, right: 1920.0, bottom: 1040.0 };

fn world(windows: Vec<WindowInfo>) -> World {
    World::new(windows, vec![WA], 128.0)
}

fn field(windows: Vec<WindowInfo>) -> (PileField, World) {
    let w = world(windows);
    let mut f = PileField::new(1, 1.0);
    f.sync(&w);
    (f, w)
}

/// 쌓이는 날씨에서 secs초 (1초 틱)
fn run(f: &mut PileField, weather: Weather, secs: usize) {
    for _ in 0..secs {
        f.tick(1.0, weather);
    }
}

fn depth(f: &PileField, owner: Owner) -> f32 {
    f.piles.iter().find(|p| p.owner == owner).map_or(0.0, Pile::average)
}

#[test]
fn seasons_follow_the_brief() {
    // 11월: 날마다 번갈아 눈·비
    assert_eq!(season(11, 1), Some(Weather::LightSnow));
    assert_eq!(season(11, 2), Some(Weather::Rain));
    assert_eq!(season(11, 3), Some(Weather::LightSnow));
    assert_eq!(season(11, 30), Some(Weather::Rain));
    for m in [12, 1, 2] {
        assert_eq!(season(m, 15), Some(Weather::Snow), "{m}");
    }
    assert_eq!(season(3, 1), Some(Weather::Petals));
    assert_eq!(season(3, 31), Some(Weather::Petals));
    for m in 4..=10 {
        for d in [1, 2, 15, 30] {
            assert_eq!(season(m, d), None, "{m}/{d}");
        }
    }
}

#[test]
fn preview_values_are_parsed_and_anything_else_means_no_preview() {
    assert_eq!(parse_preview("snow"), Some(Some(Weather::Snow)));
    assert_eq!(parse_preview(" Light_Snow "), Some(Some(Weather::LightSnow)));
    assert_eq!(parse_preview("rain"), Some(Some(Weather::Rain)));
    assert_eq!(parse_preview("petals"), Some(Some(Weather::Petals)));
    assert_eq!(parse_preview("off"), Some(None));
    assert_eq!(parse_preview(""), None);
    assert_eq!(parse_preview("hail"), None);
}

#[test]
fn wanted_is_nothing_when_off_hidden_or_out_of_season() {
    let snow = Some(Weather::Snow);
    assert_eq!(wanted(snow, None, false, false), snow);
    assert_eq!(wanted(snow, None, true, false), None, "menu toggle off");
    assert_eq!(wanted(snow, None, false, true), None, "windows hidden");
    assert_eq!(wanted(None, None, false, false), None, "out of season");
    // 미리 보기는 달력과 메뉴를 넘어선다 — 숨겨 둔 동안은 그래도 꺼진다
    let rain = Some(Some(Weather::Rain));
    assert_eq!(wanted(None, rain, true, false), Some(Weather::Rain));
    assert_eq!(wanted(None, rain, false, true), None);
    assert_eq!(wanted(snow, Some(None), false, false), None, "preview off wins over the calendar");
}

#[test]
fn a_snowman_stays_for_an_hour_of_snowy_mood() {
    assert!(snowman_stays(Some(Weather::Snow), 0.0));
    assert!(snowman_stays(Some(Weather::LightSnow), SNOWMAN_SECS - 1.0));
    assert!(!snowman_stays(Some(Weather::Snow), SNOWMAN_SECS));
    assert!(!snowman_stays(Some(Weather::Rain), 10.0), "rain day");
    assert!(!snowman_stays(Some(Weather::Petals), 10.0));
    assert!(!snowman_stays(None, 10.0));
}

#[test]
fn only_a_window_past_the_work_area_counts_as_fullscreen() {
    // 전체 화면: 작업표시줄까지 덮는다
    assert!(fullscreen_over(&[win(1, 0.0, 0.0, 1920.0, 1080.0)], &WA));
    // 최대화: 작업 영역에 딱 맞는다
    assert!(!fullscreen_over(&[win(1, 0.0, 0.0, 1920.0, 1040.0)], &WA));
    // 작은 창, 창 없음
    assert!(!fullscreen_over(&[win(1, 100.0, 100.0, 900.0, 800.0)], &WA));
    assert!(!fullscreen_over(&[], &WA));
    // 전체 화면 창 앞에 보통 창이 떠 있으면 (가운데를 덮으면) 전체 화면을 보는 중이 아니다
    assert!(!fullscreen_over(&[win(2, 500.0, 300.0, 1400.0, 800.0), win(1, 0.0, 0.0, 1920.0, 1080.0)], &WA));
    // 다른 모니터의 전체 화면은 상관없다
    assert!(!fullscreen_over(&[win(1, 1920.0, 0.0, 3840.0, 1080.0)], &WA));
}

#[test]
fn snow_builds_on_window_tops_and_the_ground_up_to_a_cap() {
    let (mut f, _) = field(vec![win(1, 200.0, 300.0, 1000.0, 800.0)]);
    run(&mut f, Weather::Snow, 300);
    assert!(depth(&f, Owner::Window(1)) > 1.0, "{}", depth(&f, Owner::Window(1)));
    assert!(depth(&f, Owner::Ground(0)) > 1.0);
    run(&mut f, Weather::Snow, 3000);
    let top = f.piles.iter().flat_map(|p| &p.cells).fold(0.0f32, |a, &b| a.max(b));
    assert!(top <= 9.0, "cap: {top}");
}

#[test]
fn rain_never_piles_up() {
    let (mut f, _) = field(vec![win(1, 200.0, 300.0, 1000.0, 800.0)]);
    run(&mut f, Weather::Rain, 600);
    assert_eq!(depth(&f, Owner::Ground(0)), 0.0);
    assert!(f.scene(Weather::Rain, &WA, 1.0).piles.is_empty());
}

#[test]
fn piles_melt_away_in_the_rain_and_keep_their_look_meanwhile() {
    let (mut f, _) = field(vec![]);
    run(&mut f, Weather::LightSnow, 400);
    let before = depth(&f, Owner::Ground(0));
    assert!(before > 1.0);
    run(&mut f, Weather::Rain, 10);
    let mid = depth(&f, Owner::Ground(0));
    assert!(mid < before && mid > 0.0, "{before} -> {mid}");
    assert_eq!(f.scene(Weather::Rain, &WA, 1.0).pile_kind, Some(Weather::LightSnow));
    run(&mut f, Weather::Rain, 300);
    assert_eq!(depth(&f, Owner::Ground(0)), 0.0);
    assert!(f.scene(Weather::Rain, &WA, 1.0).piles.is_empty());
}

#[test]
fn snow_turning_to_petals_starts_a_new_pile() {
    let (mut f, _) = field(vec![]);
    run(&mut f, Weather::Snow, 300);
    assert!(depth(&f, Owner::Ground(0)) > 1.0);
    f.tick(1.0, Weather::Petals);
    assert!(depth(&f, Owner::Ground(0)) < 0.5);
    assert_eq!(f.scene(Weather::Petals, &WA, 1.0).pile_kind, Some(Weather::Petals));
}

#[test]
fn a_moved_or_closed_window_loses_its_pile_but_the_others_keep_theirs() {
    let a = win(1, 200.0, 300.0, 700.0, 800.0);
    let b = win(2, 900.0, 300.0, 1400.0, 800.0);
    let (mut f, _) = field(vec![a, b]);
    run(&mut f, Weather::Snow, 300);
    assert!(depth(&f, Owner::Window(1)) > 1.0 && depth(&f, Owner::Window(2)) > 1.0);
    // 창 1을 옮겼다
    f.sync(&world(vec![win(1, 210.0, 300.0, 710.0, 800.0), b]));
    assert_eq!(depth(&f, Owner::Window(1)), 0.0);
    assert!(depth(&f, Owner::Window(2)) > 1.0);
    assert!(depth(&f, Owner::Ground(0)) > 1.0);
    // 창 2를 닫았다
    f.sync(&world(vec![win(1, 210.0, 300.0, 710.0, 800.0)]));
    assert!(!f.piles.iter().any(|p| p.owner == Owner::Window(2)));
}

#[test]
fn a_covered_part_of_a_window_top_gathers_nothing_and_is_clipped_in_the_scene() {
    // 창 2가 창 1 윗면의 오른쪽 절반을 가린다
    let back = win(1, 200.0, 400.0, 1000.0, 800.0);
    let front = win(2, 600.0, 200.0, 1400.0, 700.0);
    let (mut f, _) = field(vec![front, back]);
    run(&mut f, Weather::Snow, 300);
    let pile = f.piles.iter().find(|p| p.owner == Owner::Window(1)).unwrap();
    let (left, right) = pile.cells.split_at(pile.cells.len() / 2);
    assert!(left.iter().sum::<f32>() > 1.0);
    // 경계 칸은 옆 칸에서 조금 넘칠 수 있다
    assert_eq!(right[1..].iter().sum::<f32>(), 0.0);
    let scene = f.scene(Weather::Snow, &WA, 1.0);
    let view = scene.piles.iter().find(|p| p.y == 400.0).unwrap();
    assert_eq!(view.clips, vec![[200.0, 600.0]]);
}

#[test]
fn scene_is_in_logical_pixels_local_to_the_monitor_and_skips_other_monitors() {
    let left = Rect::new(0.0, 0.0, 1920.0, 1040.0);
    let right = Rect::new(1920.0, 0.0, 5760.0, 2080.0);
    let w = World::new(vec![win(1, 2120.0, 400.0, 3120.0, 1400.0)], vec![left, right], 128.0);
    let mut f = PileField::new(1, 1.0);
    f.sync(&w);
    let scene = f.scene(Weather::Snow, &right, 2.0);
    assert_eq!(scene.kind, Weather::Snow);
    assert!(scene.tops.contains(&[100.0, 600.0, 200.0]), "{:?}", scene.tops);
    assert!(scene.tops.contains(&[0.0, 1920.0, 1040.0]), "own ground: {:?}", scene.tops);
    assert_eq!(scene.tops.len(), 2);
    let on_left = f.scene(Weather::Snow, &left, 1.0);
    assert_eq!(on_left.tops, vec![[0.0, 1920.0, 1040.0]]);
}

#[test]
fn scene_serializes_for_the_webview() {
    let (mut f, _) = field(vec![]);
    run(&mut f, Weather::Petals, 400);
    let v = serde_json::to_value(f.scene(Weather::Petals, &WA, 1.0)).unwrap();
    assert_eq!(v["kind"], "petals");
    assert_eq!(v["pile_kind"], "petals");
    assert_eq!(v["tops"][0][2], 1040.0);
    let pile = &v["piles"][0];
    assert_eq!(pile["y"], 1040.0);
    assert!(pile["h"].as_array().unwrap().iter().any(|h| h.as_u64().unwrap() > 0));
}

#[test]
fn snow_settles_on_the_ground_only_after_a_while() {
    for weather in [Weather::Snow, Weather::LightSnow] {
        let (mut f, _) = field(vec![]);
        run(&mut f, weather, 60);
        assert!(!f.snow_settled(), "{weather:?}: a minute is not a while");
        run(&mut f, weather, 900);
        assert!(f.snow_settled(), "{weather:?}: {}", depth(&f, Owner::Ground(0)));
    }
    let (mut f, _) = field(vec![]);
    run(&mut f, Weather::Petals, 2000);
    assert!(!f.snow_settled(), "petals are not snow");
}

#[test]
fn preview_multiplier_makes_piles_show_up_quickly() {
    let w = world(vec![]);
    let mut f = PileField::new(1, 10.0);
    f.sync(&w);
    run(&mut f, Weather::Snow, 30);
    assert!(f.snow_settled());
}

#[test]
fn only_so_many_tops_are_tracked_and_the_ground_is_always_one() {
    let many: Vec<WindowInfo> = (0..40).map(|i| win(i + 1, 50.0 * i as f64, 200.0 + 10.0 * i as f64, 50.0 * i as f64 + 300.0, 900.0)).collect();
    let (f, _) = field(many);
    assert!(f.piles.len() <= MAX_PILES + 1, "{}", f.piles.len());
    assert!(f.piles.iter().any(|p| p.owner == Owner::Ground(0)));
}

#[test]
fn an_unchanged_world_makes_an_equal_scene() {
    // 게임 루프는 바뀐 장면만 보낸다
    let (mut f, w) = field(vec![win(1, 200.0, 300.0, 1000.0, 800.0)]);
    run(&mut f, Weather::Snow, 100);
    let a = f.scene(Weather::Snow, &WA, 1.0);
    f.sync(&w);
    assert_eq!(a, f.scene(Weather::Snow, &WA, 1.0));
}

#[test]
fn overlays_pause_while_the_user_is_away_or_under_a_fullscreen_app() {
    let fullscreen = [win(1, -20.0, -20.0, 1940.0, 1100.0)];
    assert!(!overlay_paused(false, &[], &WA));
    assert!(overlay_paused(true, &[], &WA), "away");
    assert!(overlay_paused(false, &fullscreen, &WA), "fullscreen");
    assert!(overlay_paused(true, &fullscreen, &WA));
}

#[test]
fn a_failed_overlay_is_retried_later_and_later() {
    let t0 = std::time::Instant::now();
    let after = |s: u64| t0 + std::time::Duration::from_secs(s);
    let mut r = OverlayRetry::default();
    assert!(r.ready(t0), "first try right away");
    r.failed(t0);
    assert!(!r.ready(t0));
    assert!(!r.ready(t0 + std::time::Duration::from_millis(200)), "not at the scene rate");
    assert!(!r.ready(after(4)));
    assert!(r.ready(after(5)));
    r.failed(after(5));
    assert!(!r.ready(after(14)));
    assert!(r.ready(after(15)), "twice as long");
    // 상한: 1분
    for _ in 0..20 {
        r.failed(t0);
    }
    assert!(!r.ready(after(59)));
    assert!(r.ready(after(60)));
    // 성공했거나 모니터가 바뀌면 처음부터
    r.reset();
    assert!(r.ready(t0));
    r.failed(t0);
    assert!(r.ready(after(5)));
}
