use super::*;

/// 한 틱 (60fps)
const DT: f64 = 1.0 / 60.0;

fn run(t: &mut Timeline, place: Option<Place>, secs: f64) {
    for _ in 0..(secs / DT).round() as usize {
        t.tick(place.clone(), DT);
    }
}

fn friend(id: &str) -> Place {
    Place::Friend { id: id.into() }
}

fn milk() -> FavoriteFood {
    FavoriteFood { emoji: "🥛".into(), name: "우유".into() }
}

/// 칸들을 (자리, 길이 초 반올림)으로
fn shape(t: &Timeline) -> Vec<(Place, i64)> {
    t.spans().into_iter().map(|s| (s.place, (s.ago_start - s.ago_end).round() as i64)).collect()
}

#[test]
fn empty_timeline_has_nothing() {
    let t = Timeline::new();
    assert!(t.spans().is_empty());
    let v = t.view(&|_| None);
    assert!(v.rows.is_empty() && v.foods.is_empty() && v.meals.is_empty() && v.now.is_none());
}

#[test]
fn places_follow_one_another() {
    let mut t = Timeline::new();
    run(&mut t, Some(Place::Play), 120.0);
    run(&mut t, Some(Place::House), 300.0);
    run(&mut t, Some(Place::Play), 60.0);
    assert_eq!(shape(&t), vec![(Place::Play, 120), (Place::House, 300), (Place::Play, 60)]);
    let last = t.spans().pop().unwrap();
    assert!(last.ago_end.abs() < 1e-6, "지금 칸은 지금 끝난다");
}

#[test]
fn a_change_shows_at_once() {
    // 기다리지 않는다 — 한 틱 만에 새 칸
    let mut t = Timeline::new();
    run(&mut t, Some(Place::Play), 60.0);
    t.tick(Some(friend("a")), DT);
    let v = t.view(&|_| None);
    assert_eq!(v.rows.len(), 2);
    assert_eq!(v.rows[v.now.unwrap()].label, "친구네 놀러 감");
}

#[test]
fn unknown_moments_continue_the_current_place() {
    let mut t = Timeline::new();
    // 처음엔 모른다 — 아무 칸도 없다
    run(&mut t, None, 3.0);
    assert!(t.spans().is_empty());
    run(&mut t, Some(Place::Play), 30.0);
    // 화면을 나갔는데 아직 어느 친구인지 모른다
    run(&mut t, None, 1.0);
    run(&mut t, Some(friend("a")), 30.0);
    assert_eq!(shape(&t), vec![(Place::Play, 31), (friend("a"), 30)]);
}

#[test]
fn hops_through_friends_are_separate_rows() {
    let mut t = Timeline::new();
    run(&mut t, Some(Place::Play), 60.0);
    run(&mut t, Some(friend("a")), 480.0);
    run(&mut t, Some(friend("b")), 360.0);
    run(&mut t, Some(Place::Play), 60.0);
    assert_eq!(shape(&t), vec![(Place::Play, 60), (friend("a"), 480), (friend("b"), 360), (Place::Play, 60)]);
}

#[test]
fn totals_add_up_each_place() {
    let mut t = Timeline::new();
    run(&mut t, Some(Place::Play), 100.0);
    run(&mut t, Some(Place::House), 300.0);
    run(&mut t, Some(Place::Play), 250.0);
    let totals: Vec<(Place, i64)> = t.totals().into_iter().map(|(p, s)| (p, s.round() as i64)).collect();
    assert_eq!(totals, vec![(Place::Play, 350), (Place::House, 300)]);
}

#[test]
fn only_the_last_hour_is_kept() {
    let mut t = Timeline::new();
    run(&mut t, Some(Place::House), 1800.0);
    t.ate(Food::Meal, &milk());
    run(&mut t, Some(Place::Play), 3700.0);
    // 하우스 칸과 그때 먹은 밥은 기간 밖으로 다 나갔고, 놀기 칸은 기간 시작에서 잘린다
    assert_eq!(shape(&t), vec![(Place::Play, 3600)]);
    assert!(t.view(&|_| None).foods.is_empty());
    run(&mut t, Some(Place::House), 1200.0);
    assert_eq!(shape(&t), vec![(Place::Play, 2400), (Place::House, 1200)]);
    let sum: f64 = t.totals().iter().map(|(_, s)| s).sum();
    assert!((sum - WINDOW_SECS).abs() < 1e-3);
    assert_eq!(t.segments.len(), 2, "버린 칸은 메모리에서도 사라진다");
    assert!(t.meals.is_empty());
}

#[test]
fn a_long_tick_never_splits_a_place() {
    // 메뉴가 열려 게임 루프가 10초 멈췄다 — 같은 자리는 한 줄, 멈춘 시간도 그 자리
    let mut t = Timeline::new();
    run(&mut t, Some(Place::Play), 60.0);
    t.tick(Some(Place::Play), 10.0);
    run(&mut t, Some(Place::Play), 60.0);
    assert_eq!(shape(&t), vec![(Place::Play, 130)]);
    // 멈춘 사이 친구네로 갔으면 그 틱부터 친구네
    t.tick(Some(friend("a")), 8.0);
    run(&mut t, Some(friend("a")), 2.0);
    assert_eq!(shape(&t), vec![(Place::Play, 130), (friend("a"), 10)]);
}

#[test]
fn bad_dt_is_ignored() {
    let mut t = Timeline::new();
    run(&mut t, Some(Place::Play), 10.0);
    t.tick(Some(Place::House), f64::NAN);
    t.tick(Some(Place::House), -1.0);
    t.tick(Some(Place::House), 0.0);
    assert_eq!(shape(&t), vec![(Place::Play, 10)]);
}

#[test]
fn many_changes_never_grow_memory_without_bound() {
    let mut t = Timeline::new();
    for _ in 0..1000 {
        run(&mut t, Some(Place::Play), 0.5);
        run(&mut t, Some(Place::House), 0.5);
        t.ate(Food::Snack, &milk());
    }
    assert!(t.segments.len() <= MAX_ENTRIES && t.meals.len() <= MAX_ENTRIES);
}

#[test]
fn labels_read_naturally() {
    let names = |id: &str| (id == "a").then(|| "민수".to_string());
    assert_eq!(Place::Play.label(&names), "집에서 놀기");
    assert_eq!(Place::House.label(&names), "하우스");
    assert_eq!(friend("a").label(&names), "민수네 놀러 감");
    assert_eq!(friend("zz").label(&names), "친구네 놀러 감");
}

#[test]
fn state_becomes_a_place() {
    assert_eq!(place_of(false, false, None), Some(Place::Play));
    assert_eq!(place_of(true, false, None), Some(Place::House));
    assert_eq!(place_of(false, true, Some("a")), Some(friend("a")));
    assert_eq!(place_of(false, true, None), None, "어느 친구인지 모르면 앞 자리로");
}

#[test]
fn foods_are_counted_by_kind_in_menu_order() {
    let mut t = Timeline::new();
    run(&mut t, Some(Place::Play), 10.0);
    t.ate(Food::Snack, &milk());
    t.ate(Food::Meal, &milk());
    run(&mut t, Some(Place::Play), 10.0);
    t.ate(Food::Snack, &milk());
    t.ate(Food::Favorite, &milk());
    t.ate(Food::Water, &milk());
    t.ate(Food::Special, &milk());
    let v = t.view(&|_| None);
    let foods: Vec<(String, u32)> = v.foods.iter().map(|f| (format!("{} {}", f.emoji, f.name), f.count)).collect();
    assert_eq!(foods, vec![("🍚 밥".into(), 1), ("🥛 우유".into(), 1), ("🍱 특식".into(), 1), ("🍪 간식".into(), 2), ("💧 물".into(), 1)]);
    // 먹은 때: 오래된 것부터, 몇 초 전
    assert_eq!(v.meals.len(), 6);
    assert!((v.meals[0].ago - 10.0).abs() < 0.1 && v.meals[5].ago.abs() < 1e-6);
    assert_eq!(v.meals[0].name, "간식");
}

#[test]
fn view_rows_and_spans_share_colors() {
    let mut t = Timeline::new();
    run(&mut t, Some(Place::Play), 100.0);
    run(&mut t, Some(friend("a")), 300.0);
    run(&mut t, Some(Place::Play), 50.0);
    let names = |id: &str| (id == "a").then(|| "민수".to_string());
    let v = t.view(&names);
    let labels: Vec<&str> = v.rows.iter().map(|r| r.label.as_str()).collect();
    assert_eq!(labels, vec!["민수네 놀러 감", "집에서 놀기"]);
    let rows: Vec<usize> = v.spans.iter().map(|s| s.row).collect();
    assert_eq!(rows, vec![1, 0, 1]);
    assert_eq!(v.now, Some(1));
}

#[test]
fn a_bounced_visit_leaves_no_row() {
    // 바쁜 친구에게 갔다가 곧바로 팅겨 왔다 — 놀기 한 줄
    let mut t = Timeline::new();
    run(&mut t, Some(Place::Play), 60.0);
    run(&mut t, Some(friend("busy")), 1.0);
    run(&mut t, Some(Place::Play), 30.0);
    assert_eq!(shape(&t), vec![(Place::Play, 91)]);
    // 하우스에서 바로 보냈는데 팅겨서 놀기로 돌아왔다 — 하우스, 놀기
    let mut t = Timeline::new();
    run(&mut t, Some(Place::House), 60.0);
    run(&mut t, Some(friend("busy")), 1.0);
    run(&mut t, Some(Place::Play), 30.0);
    assert_eq!(shape(&t), vec![(Place::House, 60), (Place::Play, 31)], "빈틈 없이 이어진다");
    // 제대로 논 방문은 남는다
    let mut t = Timeline::new();
    run(&mut t, Some(Place::Play), 60.0);
    run(&mut t, Some(friend("a")), QUICK_DECLINE_SECS + 1.0);
    run(&mut t, Some(Place::Play), 30.0);
    assert_eq!(shape(&t).len(), 3);
}
