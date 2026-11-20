//! weather.rs 테스트 — 계절·실제 날씨 고르기, 더미 쌓이기·녹기, 장면 만들기, 창 만들기 재시도
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
    // 달마다, 날마다 전부
    const DAYS: [u32; 12] = [31, 29, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];
    for (i, &days) in DAYS.iter().enumerate() {
        let m = i as u32 + 1;
        for d in 1..=days {
            let cycle = (d - 1) % 3;
            let want = match m {
                3 | 4 => Weather::Petals,
                5..=8 => Weather::Shower,
                9 | 10 => Weather::Leaves,
                11 => [Weather::Leaves, Weather::LightSnow, Weather::Rain][cycle as usize],
                _ => [Weather::Snow, Weather::Snow, Weather::Rain][cycle as usize],
            };
            assert_eq!(season(m, d), want, "{m}/{d}");
        }
    }
    // 사용자의 말 그대로 몇 개
    assert_eq!(season(11, 1), Weather::Leaves);
    assert_eq!(season(11, 2), Weather::LightSnow);
    assert_eq!(season(11, 3), Weather::Rain);
    assert_eq!(season(11, 30), Weather::Rain);
    assert_eq!(season(12, 3), Weather::Rain);
    assert_eq!(season(1, 1), Weather::Snow);
    assert_eq!(season(4, 30), Weather::Petals);
    assert_eq!(season(5, 1), Weather::Shower);
    assert_eq!(season(10, 31), Weather::Leaves);
}

#[test]
fn outside_summer_the_clock_does_not_matter() {
    // 9월은 🌀 태풍이 올 수 있어 따로 봄 (typhoons_come_only_in_their_windows)
    // 한겨울 눈 오는 날은 함박눈 칸이 있을 수 있어 따로 봄 (heavy_snow_visits_some_deep_winter_snow_days_for_a_stretch)
    for (m, d) in [(3, 1), (4, 30), (10, 31), (11, 1), (11, 2), (11, 3), (12, 25), (1, 3), (2, 28)] {
        for secs in [0, 3 * 3600, 12 * 3600, 86_399] {
            let heavy = heavy_snows(2027, m, d).iter().any(|&(a, b)| a <= secs && secs < b);
            let want = if heavy { Weather::HeavySnow } else { season(m, d) };
            assert_eq!(weather_at(2027, m, d, secs), Some(want), "{m}/{d} {secs}");
        }
    }
}

/// 여름날 전부 (윤년 포함)
fn summer_days() -> Vec<(i32, u32, u32)> {
    let mut days = Vec::new();
    for y in [2027, 2028] {
        for (m, n) in [(5, 31), (6, 30), (7, 31), (8, 31)] {
            days.extend((1..=n).map(|d| (y, m, d)));
        }
    }
    days
}

#[test]
fn a_summer_day_has_a_few_short_showers_and_is_mostly_clear() {
    for (y, m, d) in summer_days() {
        let list = showers(y, m, d);
        assert!((3..=5).contains(&list.len()), "{y}-{m}-{d}: {list:?}");
        let mut last_end = 0;
        for &(start, end) in &list {
            let len = end - start;
            assert!((180..=480).contains(&len), "3 to 8 minutes: {list:?}");
            assert!(start >= last_end, "in order, no overlap: {list:?}");
            assert!(end <= 86_400, "stays inside the day: {list:?}");
            last_end = end;
        }
        let wet: u32 = list.iter().map(|&(s, e)| e - s).sum();
        assert!(wet <= 40 * 60, "mostly clear: {wet}s");
    }
}

#[test]
fn showers_are_the_same_after_a_restart_and_differ_between_days() {
    let days = summer_days();
    for &(y, m, d) in &days {
        assert_eq!(showers(y, m, d), showers(y, m, d));
    }
    let differ = days.windows(2).filter(|p| showers(p[0].0, p[0].1, p[0].2) != showers(p[1].0, p[1].1, p[1].2)).count();
    assert_eq!(differ, days.len() - 1);
}

#[test]
fn it_only_rains_inside_the_shower_windows() {
    for (y, m, d) in summer_days() {
        let list = showers(y, m, d);
        // 🌀 태풍 칸은 태풍이 이김 (typhoons_come_only_in_their_windows)
        let storm = typhoons(y, m, d);
        let calm = |secs: u32| !storm.iter().any(|&(s, e)| s <= secs && secs < e);
        let at = |secs: u32| weather_at(y, m, d, secs).filter(|_| calm(secs));
        for &(start, end) in &list {
            for (secs, want) in [(start, Weather::Shower), (end - 1, Weather::Shower), (end, Weather::GreenLeaves), (start - 1, Weather::GreenLeaves)] {
                if calm(secs) {
                    assert_eq!(at(secs), Some(want), "소나기 사이엔 푸른 잎 {secs}");
                }
            }
        }
        // 분마다 세어 봐도 소나기는 잠깐뿐
        let wet = (0..1440).filter(|min| at(min * 60) == Some(Weather::Shower)).count();
        assert!(wet <= 45, "{y}-{m}-{d}: {wet} minutes");
        if storm.is_empty() {
            assert!(wet > 0, "{y}-{m}-{d}");
        }
    }
}

#[test]
fn leaves_pile_up_like_petals_and_rain_clears_them() {
    let (mut f, _) = field(vec![win(1, 100.0, 300.0, 500.0, 800.0)]);
    run(&mut f, Weather::Leaves, 400);
    assert!(depth(&f, Owner::Window(1)) > 1.0);
    assert!(depth(&f, Owner::Ground(0)) > 1.0);
    let v = serde_json::to_value(f.scene(Weather::Leaves, &WA, 1.0)).unwrap();
    assert_eq!(v["kind"], "leaves");
    assert_eq!(v["pile_kind"], "leaves");
    assert!(!f.snow_settled(), "leaves are not snow");
    assert!(!f.snow_on_ground());
    run(&mut f, Weather::Leaves, 3000);
    assert!(f.piles.iter().flat_map(|p| &p.cells).all(|&c| c <= 4.0), "low cap");
    run(&mut f, Weather::Rain, 300);
    assert!(f.scene(Weather::Rain, &WA, 1.0).piles.is_empty());
}

#[test]
fn a_new_kind_of_pile_starts_over_on_the_cycle_days() {
    let (mut f, _) = field(vec![]);
    run(&mut f, Weather::Leaves, 400);
    let leaves = depth(&f, Owner::Ground(0));
    assert!(leaves > 1.0);
    f.tick(1.0, Weather::LightSnow);
    assert!(depth(&f, Owner::Ground(0)) < leaves / 2.0, "snow does not sit on an old leaf pile");
    run(&mut f, Weather::Snow, 300);
    f.tick(1.0, Weather::Leaves);
    assert_eq!(f.scene(Weather::Leaves, &WA, 1.0).pile_kind, Some(Weather::Leaves));
}

#[test]
fn a_shower_never_piles_up_and_clears_what_is_left() {
    let (mut f, _) = field(vec![]);
    run(&mut f, Weather::Shower, 300);
    assert_eq!(depth(&f, Owner::Ground(0)), 0.0);
    assert_eq!(f.scene(Weather::Shower, &WA, 1.0).pile_kind, None);
    let v = serde_json::to_value(f.scene(Weather::Shower, &WA, 1.0)).unwrap();
    assert_eq!(v["kind"], "shower");
    run(&mut f, Weather::Petals, 400);
    run(&mut f, Weather::Shower, 300);
    assert!(f.scene(Weather::Shower, &WA, 1.0).piles.is_empty());
}

#[test]
fn pets_look_up_at_petals_and_leaves_only() {
    assert!(Weather::Petals.looks_up() && Weather::Leaves.looks_up() && Weather::GreenLeaves.looks_up());
    for w in [Weather::Snow, Weather::LightSnow, Weather::Rain, Weather::Shower] {
        assert!(!w.looks_up(), "{w:?}");
    }
}

#[test]
fn a_snowman_needs_real_snow_on_the_ground() {
    let (mut f, w) = field(vec![]);
    assert!(!f.snow_on_ground());
    run(&mut f, Weather::Snow, 300);
    assert!(f.snow_on_ground());
    // 비가 다 녹이면 끝
    run(&mut f, Weather::Rain, 300);
    assert!(!f.snow_on_ground());
    run(&mut f, Weather::Snow, 300);
    assert!(f.snow_on_ground());
    // 바닥이 달라져 더미가 사라져도 끝
    f.sync(&World::new(vec![], vec![Rect::new(0.0, 0.0, 1600.0, 900.0)], 128.0));
    assert!(!f.snow_on_ground());
    f.sync(&w);
    assert!(!f.snow_on_ground());
}

#[test]
fn preview_values_are_parsed_and_anything_else_means_no_preview() {
    assert_eq!(parse_preview("snow"), Some(Some(Weather::Snow)));
    assert_eq!(parse_preview(" Light_Snow "), Some(Some(Weather::LightSnow)));
    assert_eq!(parse_preview("rain"), Some(Some(Weather::Rain)));
    assert_eq!(parse_preview("petals"), Some(Some(Weather::Petals)));
    assert_eq!(parse_preview("leaves"), Some(Some(Weather::Leaves)));
    assert_eq!(parse_preview("shower"), Some(Some(Weather::Shower)));
    assert_eq!(parse_preview("green_leaves"), Some(Some(Weather::GreenLeaves)));
    assert_eq!(parse_preview("off"), Some(None));
    assert_eq!(parse_preview(""), None);
    assert_eq!(parse_preview("hail"), None);
}

#[test]
fn wanted_is_nothing_when_off_hidden_or_between_showers() {
    let snow = Some(Weather::Snow);
    assert_eq!(wanted(snow, None, false, false), snow);
    assert_eq!(wanted(snow, None, true, false), None, "menu toggle off");
    assert_eq!(wanted(snow, None, false, true), None, "windows hidden");
    assert_eq!(wanted(None, None, false, false), None, "clear between showers");
    // 미리 보기는 달력과 메뉴를 넘어선다 — 숨겨 둔 동안은 그래도 꺼진다
    let rain = Some(Some(Weather::Rain));
    assert_eq!(wanted(None, rain, true, false), Some(Weather::Rain));
    assert_eq!(wanted(None, rain, false, true), None);
    assert_eq!(wanted(snow, Some(None), false, false), None, "preview off wins over the calendar");
}

#[test]
fn a_snowman_stays_for_an_hour_of_snowy_mood() {
    assert!(snowman_stays(Some(Weather::Snow), 0.0, true));
    assert!(snowman_stays(Some(Weather::LightSnow), SNOWMAN_SECS - 1.0, true));
    assert!(!snowman_stays(Some(Weather::Snow), SNOWMAN_SECS, true));
    assert!(!snowman_stays(Some(Weather::Snow), 10.0, false), "no snow left on the ground");
    assert!(!snowman_stays(Some(Weather::Rain), 10.0, true), "rain day");
    assert!(!snowman_stays(Some(Weather::Shower), 10.0, true));
    assert!(!snowman_stays(Some(Weather::Petals), 10.0, true));
    assert!(!snowman_stays(Some(Weather::Leaves), 10.0, true));
    assert!(!snowman_stays(None, 10.0, true));
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
fn every_change_of_weather_clears_the_old_piles_at_once() {
    // 🧹 눈이 오다 태풍이 되면 눈 더미가 그대로 남던 문제: 날씨 종류가 바뀌면 (비·태풍 포함) 예전 더미를 싹 치움
    let cases = [
        (Weather::Snow, Weather::Typhoon),
        (Weather::Snow, Weather::Rain),
        (Weather::HeavySnow, Weather::Shower),
        (Weather::Leaves, Weather::Typhoon),
        (Weather::Petals, Weather::Rain),
        (Weather::GreenLeaves, Weather::Shower),
        (Weather::LightSnow, Weather::Petals),
        (Weather::Leaves, Weather::Snow),
    ];
    for (from, to) in cases {
        let (mut f, _) = field(vec![win(1, 200.0, 400.0, 1000.0, 800.0)]);
        run(&mut f, from, 400);
        let before = depth(&f, Owner::Ground(0)) + depth(&f, Owner::Window(1));
        assert!(before > 1.0, "{from:?}");
        f.tick(1.0, to);
        let after = depth(&f, Owner::Ground(0)) + depth(&f, Owner::Window(1));
        assert!(after < 0.1, "{from:?} → {to:?}: 한 번에 다 치움 (새 날씨가 막 얹은 것만): {after}");
        if to.rates().grow == 0.0 {
            assert!(!f.has_piles() && f.scene(to, &WA, 1.0).piles.is_empty(), "{from:?} → {to:?}");
            assert_eq!(f.pile_kind(), None, "{from:?} → {to:?}: 쌓이지 않는 날씨엔 더미 그림도 없음");
            assert!(!f.snow_on_ground());
        }
    }
}

#[test]
fn the_same_weather_keeps_its_piles() {
    for kind in [Weather::Snow, Weather::Leaves, Weather::Petals, Weather::GreenLeaves] {
        let (mut f, _) = field(vec![]);
        run(&mut f, kind, 400);
        let before = depth(&f, Owner::Ground(0));
        f.tick(1.0, kind);
        assert!(depth(&f, Owner::Ground(0)) >= before - 0.01, "{kind:?}: 그대로 쌓임");
    }
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

#[test]
fn the_broom_clears_only_the_pile_it_passes_over() {
    let (mut f, _) = field(vec![win(1, 200.0, 400.0, 1000.0, 800.0)]);
    run(&mut f, Weather::Snow, 300);
    let (top, ground) = (depth(&f, Owner::Window(1)), depth(&f, Owner::Ground(0)));
    assert!(top > 0.5 && ground > 0.5);
    // 창 윗면 높이, 왼쪽 절반만
    assert!(f.sweep(200.0, 600.0, 410.0, 40.0) > 0.0);
    let pile = f.piles.iter().find(|p| p.owner == Owner::Window(1)).unwrap();
    let (left, right) = pile.cells.split_at(pile.cells.len() / 2);
    assert_eq!(left.iter().sum::<f32>(), 0.0);
    assert!(right.iter().sum::<f32>() > 0.0);
    assert_eq!(depth(&f, Owner::Ground(0)), ground, "the floor is far below");
    // 더미 없는 높이는 아무것도 쓸지 않는다
    assert_eq!(f.sweep(0.0, 1920.0, 100.0, 40.0), 0.0);
    // 다시 쓸면 쓸 것이 없다
    assert_eq!(f.sweep(200.0, 600.0, 410.0, 40.0), 0.0);
}

#[test]
fn only_piling_weather_can_be_swept_to_a_friend() {
    for kind in [Weather::LightSnow, Weather::Snow, Weather::HeavySnow, Weather::Petals, Weather::Leaves] {
        assert_eq!(parse_flurry_kind(&flurry_kind(kind)), Some(kind));
    }
    for raw in ["flurry:rain", "flurry:shower", "flurry:off", "flurry:", "flurry:lava", "can", "snow"] {
        assert_eq!(parse_flurry_kind(raw), None, "{raw}");
    }
}

#[test]
fn any_fullscreen_sees_every_monitor_and_ignores_maximized_windows() {
    let areas = [Rect::new(0.0, 0.0, 1920.0, 1040.0), Rect::new(1920.0, 0.0, 3840.0, 1040.0)];
    let full = WindowInfo { id: 1, rect: Rect::new(1920.0, 0.0, 3840.0, 1080.0) };
    let maximized = WindowInfo { id: 2, rect: Rect::new(0.0, 0.0, 1920.0, 1040.0) };
    assert!(any_fullscreen(&[full], &areas));
    assert!(!any_fullscreen(&[maximized], &areas));
    assert!(!any_fullscreen(&[], &areas));
}

#[test]
fn clear_all_empties_every_pile_at_once() {
    let (mut f, _) = field(vec![win(1, 200.0, 400.0, 1000.0, 800.0)]);
    run(&mut f, Weather::Leaves, 300);
    assert!(depth(&f, Owner::Window(1)) > 0.5 && depth(&f, Owner::Ground(0)) > 0.5);
    assert!(f.clear_all(), "치운 게 있음");
    assert!(f.piles.iter().flat_map(|p| &p.cells).all(|&c| c == 0.0));
    assert!(f.scene(Weather::Leaves, &WA, 1.0).piles.is_empty());
    assert!(!f.clear_all(), "두 번째엔 치울 게 없음");
}

#[test]
fn piles_are_wiped_clean_every_hour_by_themselves() {
    let (mut f, _) = field(vec![win(1, 200.0, 400.0, 1000.0, 800.0)]);
    run(&mut f, Weather::Leaves, (AUTO_CLEAR_SECS as usize) - 10);
    assert!(depth(&f, Owner::Ground(0)) > 0.5, "한 시간 전엔 그대로");
    run(&mut f, Weather::Leaves, 10);
    assert!(depth(&f, Owner::Ground(0)) < 0.1, "한 시간이 되면 싹 치움");
    // 다시 쌓이고, 또 한 시간 뒤에 치움
    run(&mut f, Weather::Leaves, 300);
    assert!(depth(&f, Owner::Ground(0)) > 0.5);
}

#[test]
fn clearing_by_hand_restarts_the_hour() {
    let (mut f, _) = field(vec![win(1, 200.0, 400.0, 1000.0, 800.0)]);
    run(&mut f, Weather::Leaves, 3000);
    f.clear_all();
    run(&mut f, Weather::Leaves, 1000);
    assert!(depth(&f, Owner::Ground(0)) > 0.5, "손으로 치운 뒤 한 시간을 새로 셈");
}

#[test]
fn snow_under_the_cursor_is_measured_and_packed_only_where_rubbed() {
    let (mut f, _) = field(vec![win(1, 200.0, 400.0, 1000.0, 800.0)]);
    run(&mut f, Weather::Snow, 300);
    let near = f.snow_amount(400.0, 500.0, 410.0, 28.0);
    assert!(near > 0.5, "{near}");
    assert_eq!(f.snow_amount(400.0, 500.0, 300.0, 28.0), 0.0, "too far above the window top");
    let before = depth(&f, Owner::Window(1));
    let packed = f.pack_snow(400.0, 500.0, 410.0, 28.0);
    assert!((packed - near).abs() < 1e-3, "{packed} vs {near}");
    assert_eq!(f.snow_amount(400.0, 500.0, 410.0, 28.0), 0.0, "the rubbed cells are bare");
    assert!(depth(&f, Owner::Window(1)) > 0.0 && depth(&f, Owner::Window(1)) < before, "the rest of the top keeps its snow");
}

#[test]
fn only_snow_can_be_packed() {
    let (mut f, _) = field(vec![win(1, 200.0, 400.0, 1000.0, 800.0)]);
    run(&mut f, Weather::Leaves, 300);
    assert!(f.amount_in(400.0, 500.0, 410.0, 28.0) > 0.0, "leaves are there");
    assert_eq!(f.snow_amount(400.0, 500.0, 410.0, 28.0), 0.0);
    assert_eq!(f.pack_snow(400.0, 500.0, 410.0, 28.0), 0.0);
    assert!(f.amount_in(400.0, 500.0, 410.0, 28.0) > 0.0, "nothing was taken");
}

#[test]
fn the_pile_under_a_foot_is_its_kind_and_depth() {
    let (mut f, _) = field(vec![win(1, 200.0, 400.0, 1000.0, 800.0)]);
    assert_eq!(f.pile_under(500.0, 400.0, 20.0), None, "nothing has fallen yet");
    run(&mut f, Weather::Leaves, 300);
    let (kind, depth) = f.pile_under(500.0, 400.0, 20.0).unwrap();
    assert_eq!(kind, Weather::Leaves);
    assert!(depth > 0.5, "{depth}");
    assert_eq!(f.pile_under(500.0, 300.0, 20.0), None, "standing on nothing there");
    assert_eq!(f.pile_under(100.0, 400.0, 20.0), None, "beside the window");
}

#[test]
fn petals_and_green_leaves_pile_up_exactly_like_leaves() {
    let b = Weather::Leaves.rates();
    for kind in [Weather::Petals, Weather::GreenLeaves] {
        let a = kind.rates();
        assert_eq!((a.grow, a.melt, a.cap), (b.grow, b.melt, b.cap), "{kind:?}: 같은 빠르기·같은 높이");
        let (mut f, _) = field(vec![win(1, 200.0, 400.0, 1000.0, 800.0)]);
        run(&mut f, kind, 400);
        assert!(depth(&f, Owner::Window(1)) > 1.0 && depth(&f, Owner::Ground(0)) > 1.0, "{kind:?}: 창 위와 바닥에 쌓임");
        let (under, d) = f.pile_under(1400.0, WA.bottom, 20.0).expect("발밑 더미");
        assert_eq!(under, kind);
        assert!(d > 0.3, "{d}");
        assert_eq!(serde_json::to_value(f.scene(kind, &WA, 1.0)).unwrap()["pile_kind"], kind.name());
        assert!(f.clear_all(), "쓰레기 다 치우기로 치움");
        run(&mut f, kind, (AUTO_CLEAR_SECS as usize) - 10);
        assert!(depth(&f, Owner::Ground(0)) > 0.5);
        run(&mut f, kind, 10);
        assert!(depth(&f, Owner::Ground(0)) < 0.1, "{kind:?}: 한 시간마다 저절로 치움");
    }
}

#[test]
fn a_different_kind_of_leaf_or_petal_starts_a_new_pile_but_snow_keeps_piling() {
    for (from, to) in [(Weather::Leaves, Weather::Petals), (Weather::Petals, Weather::GreenLeaves), (Weather::GreenLeaves, Weather::Leaves)] {
        let (mut f, _) = field(vec![]);
        run(&mut f, from, 400);
        let before = depth(&f, Owner::Ground(0));
        f.tick(1.0, to);
        assert!(depth(&f, Owner::Ground(0)) < before / 2.0, "{from:?} → {to:?}: 예전 더미에 새 그림을 입히지 않음");
        assert_eq!(f.pile_kind(), Some(to));
    }
    let (mut f, _) = field(vec![]);
    run(&mut f, Weather::Snow, 300);
    let snow = depth(&f, Owner::Ground(0));
    f.tick(1.0, Weather::LightSnow);
    assert!(depth(&f, Owner::Ground(0)) > snow / 2.0, "눈에서 가벼운 눈은 이어 쌓임");
}

#[test]
fn green_leaves_are_named_for_settings_and_friends_and_swept_over_as_a_flurry() {
    assert_eq!(Weather::GreenLeaves.name(), "green_leaves");
    assert_eq!(serde_json::to_value(Weather::GreenLeaves).unwrap(), "green_leaves");
    assert_eq!(parse_flurry_kind(&flurry_kind(Weather::GreenLeaves)), Some(Weather::GreenLeaves));
    assert_eq!(parse_flurry_kind("flurry:pine_needles"), None, "모르는 이름은 버림");
}

/// 7~9월 날짜 전부 (윤년 포함)
fn typhoon_days() -> Vec<(i32, u32, u32)> {
    let mut days = Vec::new();
    for y in [2027, 2028] {
        for (m, n) in [(7, 31), (8, 31), (9, 30)] {
            days.extend((1..=n).map(|d| (y, m, d)));
        }
    }
    days
}

#[test]
fn typhoons_visit_some_late_summer_days_for_a_few_minutes_while_awake() {
    let days = typhoon_days();
    let stormy = days.iter().filter(|&&(y, m, d)| !typhoons(y, m, d).is_empty()).count();
    assert!(stormy * 4 >= days.len() && stormy * 4 <= days.len() * 3, "가끔 (대략 이틀에 하루): {stormy}/{}", days.len());
    for &(y, m, d) in &days {
        let list = typhoons(y, m, d);
        assert_eq!(list, typhoons(y, m, d), "다시 켜도 같음");
        assert!(list.len() <= 2, "{list:?}");
        let mut last_end = 0;
        for &(start, end) in &list {
            assert!((300..=600).contains(&(end - start)), "5~10분: {list:?}");
            assert!(start >= 9 * 3600 && end <= 22 * 3600, "깨어 있는 시간: {list:?}");
            assert!(start >= last_end, "겹치지 않음: {list:?}");
            last_end = end;
        }
    }
    for (m, d) in [(1, 5), (4, 2), (6, 30), (10, 1), (11, 3), (12, 24)] {
        for y in [2027, 2028] {
            assert!(typhoons(y, m, d).is_empty(), "{y}-{m}-{d} 태풍 철 아님");
        }
    }
}

#[test]
fn typhoons_come_only_in_their_windows() {
    for (y, m, d) in typhoon_days() {
        let list = typhoons(y, m, d);
        for &(start, end) in &list {
            assert_eq!(weather_at(y, m, d, start), Some(Weather::Typhoon));
            assert_eq!(weather_at(y, m, d, end - 1), Some(Weather::Typhoon));
            assert_ne!(weather_at(y, m, d, end), Some(Weather::Typhoon));
        }
        let windy = (0..1440).filter(|min| weather_at(y, m, d, min * 60) == Some(Weather::Typhoon)).count();
        assert!(windy <= 20, "{y}-{m}-{d}: {windy} minutes");
        if list.is_empty() {
            assert_eq!(windy, 0);
        }
        // 태풍이 지나가면 그 달의 날씨로 돌아옴
        let calm = (0..86_400).step_by(600).find(|s| !list.iter().any(|&(a, b)| a <= *s && *s < b)).unwrap();
        let back = weather_at(y, m, d, calm).unwrap();
        assert!(matches!(back, Weather::Shower | Weather::GreenLeaves | Weather::Leaves), "{back:?}");
    }
}

#[test]
fn a_typhoon_never_piles_up_and_sweeps_old_piles_away() {
    let (mut f, _) = field(vec![]);
    run(&mut f, Weather::Leaves, 400);
    assert!(depth(&f, Owner::Ground(0)) > 1.0 && f.has_piles());
    run(&mut f, Weather::Typhoon, 60);
    assert_eq!(depth(&f, Owner::Ground(0)), 0.0);
    assert!(!f.has_piles(), "다 날아감 — 돌풍에 흩날릴 것도 없음");
    assert_eq!(f.pile_kind(), None);
    assert!(!Weather::Typhoon.looks_up());
    assert!(!Weather::Typhoon.is_snow());
    assert!(Weather::Typhoon.is_stormy());
    assert!(!Weather::Rain.is_stormy());
}

#[test]
fn typhoon_is_named_for_settings_and_previews_but_never_swept_to_a_friend() {
    assert_eq!(Weather::Typhoon.name(), "typhoon");
    assert_eq!(serde_json::to_value(Weather::Typhoon).unwrap(), "typhoon");
    assert_eq!(parse_preview("typhoon"), Some(Some(Weather::Typhoon)));
    assert_eq!(parse_flurry_kind("flurry:typhoon"), None, "쌓이지 않으니 쓸어 보내지 않음");
}

#[test]
fn a_typhoon_scene_carries_the_wind_and_the_funnel_but_other_scenes_do_not() {
    let (f, _) = field(vec![]);
    let calm = serde_json::to_value(f.scene(Weather::Rain, &WA, 1.0)).unwrap();
    assert!(calm.get("wind").is_none() && calm.get("tornado").is_none(), "예전 장면 그대로: {calm}");
    let mut storm = f.scene(Weather::Typhoon, &WA, 1.0);
    storm.wind = Some(-1.2);
    storm.tornado = Some([300.0, 1040.0, 80.0, 2.0]);
    let v = serde_json::to_value(storm).unwrap();
    assert_eq!(v["kind"], "typhoon");
    assert!((v["wind"].as_f64().unwrap() + 1.2).abs() < 1e-6);
    assert_eq!(v["tornado"][0], 300.0);
    assert_eq!(v["tornado"][3], 2.0, "🌀 크기 (허리케인급은 더 큼, 예전 웹뷰는 넷째 값을 모르고 넘김)");
}

#[test]
fn heavy_snow_is_snow_that_piles_faster_and_higher() {
    let (heavy, snow) = (Weather::HeavySnow.rates(), Weather::Snow.rates());
    assert!(heavy.grow > snow.grow && heavy.cap > snow.cap, "펑펑: 더 빨리, 더 높이");
    assert!(Weather::HeavySnow.is_snow() && !Weather::HeavySnow.looks_up() && !Weather::HeavySnow.is_stormy());
    let (mut f, _) = field(vec![win(1, 200.0, 400.0, 1000.0, 800.0)]);
    let (mut g, _) = field(vec![win(1, 200.0, 400.0, 1000.0, 800.0)]);
    run(&mut f, Weather::HeavySnow, 300);
    run(&mut g, Weather::Snow, 300);
    assert!(depth(&f, Owner::Ground(0)) > depth(&g, Owner::Ground(0)) * 1.3, "보통 눈보다 훨씬 많이 쌓임");
    assert!(f.snow_on_ground() && f.pack_snow(0.0, 1920.0, WA.bottom, 40.0) > 0.0, "눈처럼 뭉침");
    run(&mut f, Weather::HeavySnow, 3000);
    let top = f.piles.iter().flat_map(|p| &p.cells).fold(0.0f32, |a, &c| a.max(c));
    assert!(top > 9.0 && top <= heavy.cap, "보통 눈보다 높은 끝: {top}");
    assert!(snowman_stays(Some(Weather::HeavySnow), 0.0, true));
}

#[test]
fn heavy_snow_piles_into_bumpy_drifts_not_a_flat_slab() {
    let (mut f, _) = field(vec![]);
    run(&mut f, Weather::HeavySnow, 3000);
    let cells = &f.piles.iter().find(|p| p.owner == Owner::Ground(0)).unwrap().cells;
    let (lo, hi) = cells.iter().fold((f32::MAX, 0.0f32), |(lo, hi), &c| (lo.min(c), hi.max(c)));
    assert!(hi - lo >= 5.0, "언덕과 골이 있음: {lo}..{hi}");
    assert!(hi <= Weather::HeavySnow.rates().cap);
    // 언덕은 칸 하나짜리 뾰족이가 아니라 여러 칸에 걸친 둔덕 (이웃 칸끼리는 크게 안 다름)
    let jump = cells.windows(2).fold(0.0f32, |a, w| a.max((w[0] - w[1]).abs()));
    assert!(jump < 3.0, "이웃 칸 차이: {jump}");
    // 같은 자리는 다시 쌓여도 같은 모양 (치우고 다시 쌓아도 언덕 자리가 그대로)
    let mut g = PileField::new(2, 1.0);
    g.sync(&world(vec![]));
    run(&mut g, Weather::HeavySnow, 3000);
    let other = &g.piles.iter().find(|p| p.owner == Owner::Ground(0)).unwrap().cells;
    let tallest = |c: &Vec<f32>| c.iter().enumerate().fold((0, 0.0f32), |a, (i, &v)| if v > a.1 { (i, v) } else { a }).0;
    assert!(tallest(cells).abs_diff(tallest(other)) <= 3, "언덕 자리는 화면 자리로 정해짐");
}

#[test]
fn light_normal_and_heavy_snow_keep_the_same_pile() {
    for (from, to) in [(Weather::Snow, Weather::HeavySnow), (Weather::HeavySnow, Weather::LightSnow), (Weather::LightSnow, Weather::HeavySnow)] {
        let (mut f, _) = field(vec![]);
        run(&mut f, from, 300);
        let before = depth(&f, Owner::Ground(0));
        f.tick(1.0, to);
        assert!(depth(&f, Owner::Ground(0)) > before / 2.0, "{from:?} → {to:?}: 이어 쌓임");
    }
    let (mut f, _) = field(vec![]);
    run(&mut f, Weather::HeavySnow, 300);
    f.tick(1.0, Weather::Leaves);
    assert!(depth(&f, Owner::Ground(0)) < 0.5, "낙엽은 새 더미");
}

#[test]
fn heavy_snow_is_named_for_settings_previews_and_friends() {
    assert_eq!(Weather::HeavySnow.name(), "heavy_snow");
    assert_eq!(serde_json::to_value(Weather::HeavySnow).unwrap(), "heavy_snow");
    assert_eq!(parse_preview("heavy_snow"), Some(Some(Weather::HeavySnow)));
    assert_eq!(flurry_kind(Weather::HeavySnow), "flurry:heavy_snow");
    assert_eq!(parse_flurry_kind("flurry:heavy_snow"), Some(Weather::HeavySnow));
}

/// 12~2월 날짜 전부 (윤년 포함)
fn winter_days() -> Vec<(i32, u32, u32)> {
    let mut days = Vec::new();
    for y in [2027, 2028] {
        for (m, n) in [(12, 31), (1, 31), (2, if y == 2028 { 29 } else { 28 })] {
            days.extend((1..=n).map(|d| (y, m, d)));
        }
    }
    days
}

#[test]
fn heavy_snow_visits_some_deep_winter_snow_days_for_a_stretch() {
    let days = winter_days();
    let snowy: Vec<_> = days.iter().filter(|&&(_, m, d)| season(m, d) == Weather::Snow).collect();
    let heavy = snowy.iter().filter(|&&&(y, m, d)| !heavy_snows(y, m, d).is_empty()).count();
    assert!(heavy * 6 >= snowy.len() && heavy * 3 <= snowy.len() * 2, "가끔: {heavy}/{}", snowy.len());
    for &(y, m, d) in &days {
        let list = heavy_snows(y, m, d);
        assert_eq!(list, heavy_snows(y, m, d), "다시 켜도 같음");
        if season(m, d) != Weather::Snow {
            assert!(list.is_empty(), "{y}-{m}-{d}: 눈 오는 날에만");
        }
        let mut last_end = 0;
        for &(start, end) in &list {
            assert!((20 * 60..=50 * 60).contains(&(end - start)), "한동안: {list:?}");
            assert!(start >= 8 * 3600 && end <= 23 * 3600, "깨어 있는 시간: {list:?}");
            assert!(start >= last_end, "겹치지 않음: {list:?}");
            last_end = end;
            assert_eq!(weather_at(y, m, d, start), Some(Weather::HeavySnow));
            assert_eq!(weather_at(y, m, d, end - 1), Some(Weather::HeavySnow));
            assert_eq!(weather_at(y, m, d, end), Some(Weather::Snow), "지나가면 보통 눈");
        }
    }
    for (y, m, d) in [(2027, 11, 2), (2027, 3, 1), (2027, 7, 1), (2027, 10, 1)] {
        assert!(heavy_snows(y, m, d).is_empty(), "{y}-{m}-{d} 한겨울 아님");
    }
}
