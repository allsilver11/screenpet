use std::path::Path;
use std::time::{Duration, Instant};

use super::*;
use crate::geom::{Rect, WindowInfo};
use crate::world::World;
use crate::test_util::idle;

fn gift(id: &str) -> Gift {
    Gift::parse(id).expect("known gift")
}

fn world_with(windows: Vec<WindowInfo>) -> World {
    World::new(windows, vec![Rect::new(0.0, 0.0, 1920.0, 1040.0)], 128.0)
}

fn window(id: u64, left: f64, top: f64, right: f64, bottom: f64) -> WindowInfo {
    WindowInfo { id, rect: Rect::new(left, top, right, bottom) }
}

// ---- 도감 목록 ----

#[test]
fn every_species_has_about_six_gifts_and_only_dinos_share() {
    for s in crate::test_util::species() {
        let list = list_for(&s).unwrap_or_else(|| panic!("no gift list for {s}"));
        assert!((5..=7).contains(&list.items.len()), "{s}: {}", list.items.len());
    }
    assert_eq!(list_for("dino_green").map(|l| l.key), list_for("dino_purple").map(|l| l.key));
    assert_eq!(list_for("dino_green").map(|l| l.key), list_for("dino_yellow").map(|l| l.key));
    assert_eq!(list_for("persian").map(|l| l.key), Some("cat")); // 페르시안도 고양이 선물 (털실·깃털…)
    assert_ne!(list_for("rudolph").map(|l| l.key), list_for("santa").map(|l| l.key));
    assert_ne!(list_for("pumpkin").map(|l| l.key), list_for("ghost").map(|l| l.key));
    assert_eq!(list_for("squirrel").map(|l| (l.key, l.emoji, l.title)), Some(("squirrel", "🐿️", "다람쥐")));
    assert!(list_for("squirrel").unwrap().items.iter().any(|i| i.id == "squirrel_acorn")); // 겨울 대비 도토리
    assert_eq!(list_for("bird").map(|l| (l.key, l.emoji)), Some(("bird", "🐦")));
    assert!(list_for("bird").unwrap().items.iter().any(|i| i.id == "bird_gourdseed")); // 흥부네 제비의 박씨
    assert_eq!(list_for("penguin").map(|l| (l.key, l.emoji)), Some(("penguin", "🐧")));
    assert!(list_for("penguin").unwrap().items.iter().any(|i| i.id == "penguin_pebble")); // 펭귄이 고백할 때 건네는 조약돌
    assert_eq!(list_for("redpanda").map(|l| (l.key, l.emoji, l.title)), Some(("redpanda", "🐾", "레서판다")));
    assert!(list_for("redpanda").unwrap().items.iter().any(|i| i.id == "redpanda_bamboo"));
    assert!(list_for("dragon").is_none());
}

#[test]
fn ids_are_unique_ascii_and_never_end_like_a_shiny_variant() {
    let mut seen = std::collections::HashSet::new();
    for list in CATALOG {
        assert!(!list.title.is_empty() && !list.speech.is_empty(), "{}", list.key);
        for item in list.items {
            assert!(seen.insert(item.id), "duplicate {}", item.id);
            assert!(item.id.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_'), "{}", item.id);
            assert!(!item.id.ends_with(SHINY_SUFFIX), "{}", item.id);
            assert!(!item.name.is_empty());
        }
    }
}

#[test]
fn every_gift_has_a_normal_and_a_shiny_picture() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../ui/gifts");
    for list in CATALOG {
        for item in list.items {
            for name in [item.id.to_string(), format!("{}{SHINY_SUFFIX}", item.id)] {
                assert!(dir.join(format!("{name}.png")).is_file(), "missing ui/gifts/{name}.png (python tools/draw_gifts.py)");
            }
        }
    }
}

// ---- 받은 id 확인 ----

#[test]
fn parse_accepts_only_exact_known_ids() {
    assert_eq!(Gift::parse("cat_yarn").map(|g| (g.id, g.shiny)), Some(("cat_yarn", false)));
    assert_eq!(Gift::parse("cat_yarn_shiny").map(|g| (g.id, g.shiny)), Some(("cat_yarn", true)));
    for bad in ["", "_shiny", "cat_yarn_shiny_shiny", "CAT_YARN", " cat_yarn", "cat_yarn ", "../../evil", "cat_yarn.png", "nope", "cat_yarn\u{0}"] {
        assert!(Gift::parse(bad).is_none(), "{bad:?}");
    }
    assert!(Gift::parse(&"x".repeat(100_000)).is_none());
}

#[test]
fn wire_form_roundtrips_for_every_gift() {
    for list in CATALOG {
        for item in list.items {
            for shiny in [false, true] {
                let g = Gift { id: item.id, shiny };
                assert_eq!(Gift::parse(&g.wire()), Some(g));
            }
        }
    }
}

#[test]
fn a_visiting_pet_carries_only_its_own_kind_of_gift() {
    assert!(accept_visit_gift(Some("rabbit_carrot"), "rabbit").is_some());
    assert!(accept_visit_gift(Some("rabbit_carrot_shiny"), "rabbit").is_some());
    assert!(accept_visit_gift(Some("dino_fossil"), "dino_purple").is_some());
    assert!(accept_visit_gift(Some("rabbit_carrot"), "cat").is_none(), "another species' gift");
    assert!(accept_visit_gift(Some("rabbit_carrot"), "dragon").is_none(), "unknown species");
    assert!(accept_visit_gift(Some("unknown_item"), "rabbit").is_none());
    assert!(accept_visit_gift(None, "rabbit").is_none());
    // 곰·꿀벌·강아지는 v0.5.5에서 빠졌다: 예전 버전 친구의 그 동물이 들고 온 선물은 받지 않는다 (그림이 없다)
    for (gift, species) in [("dog_ball", "dog"), ("bear_honey", "bear"), ("bee_flower_shiny", "bee")] {
        assert!(Gift::parse(gift).is_none(), "{gift}");
        assert!(accept_visit_gift(Some(gift), species).is_none(), "{gift}");
        assert!(list_for(species).is_none() && Gift::roll(species, &mut fastrand::Rng::with_seed(1)).is_none(), "{species}");
    }
}

// ---- 고르기 ----

#[test]
fn roll_picks_from_the_species_list_and_shiny_is_rare() {
    let mut rng = fastrand::Rng::with_seed(7);
    let list = list_for("frog").unwrap();
    let mut shiny = 0;
    let n = 20_000;
    let mut seen = std::collections::HashSet::new();
    for _ in 0..n {
        let g = Gift::roll("frog", &mut rng).unwrap();
        assert!(list.items.iter().any(|i| i.id == g.id));
        seen.insert(g.id);
        shiny += g.shiny as u32;
    }
    assert_eq!(seen.len(), list.items.len(), "every item can drop");
    let rate = shiny as f64 / n as f64;
    assert!((SHINY_CHANCE * 0.7..SHINY_CHANCE * 1.3).contains(&rate), "{rate}");
    assert!(Gift::roll("dragon", &mut rng).is_none());
}

#[test]
fn only_some_visits_carry_a_gift() {
    let mut rng = fastrand::Rng::with_seed(3);
    let n = 10_000;
    let carried = (0..n).filter(|_| roll_visit_gift("cat", &mut rng).is_some()).count();
    let rate = carried as f64 / n as f64;
    assert!((VISIT_GIFT_CHANCE * 0.8..VISIT_GIFT_CHANCE * 1.2).contains(&rate), "{rate}");
    assert!(roll_visit_gift("dragon", &mut rng).is_none());
}

// ---- 언제 떨어뜨리나 ----

#[test]
fn timer_starts_between_twenty_and_sixty_minutes() {
    let mut rng = fastrand::Rng::with_seed(1);
    for _ in 0..500 {
        let t = GiftTimer::new(&mut rng, false);
        assert!((20.0 * 60.0..=60.0 * 60.0).contains(&t.remaining()), "{}", t.remaining());
    }
}

#[test]
fn timer_only_runs_while_awake_and_faster_when_well_fed() {
    let mut rng = fastrand::Rng::with_seed(2);
    let mut asleep = GiftTimer::new(&mut rng, false);
    let before = asleep.remaining();
    assert!(!asleep.tick(1000.0, false, 1.0));
    assert_eq!(asleep.remaining(), before, "does not count while asleep");

    let mut fed = GiftTimer { remaining: 100.0 };
    let mut hungry = GiftTimer { remaining: 100.0 };
    fed.tick(10.0, true, mood_rate(0.9));
    hungry.tick(10.0, true, mood_rate(0.1));
    assert!(fed.remaining() < 100.0 - 10.0, "well fed runs faster: {}", fed.remaining());
    assert!(hungry.remaining() > 100.0 - 10.0, "hungry runs slower: {}", hungry.remaining());
    assert_eq!(mood_rate(0.5), 1.0);
}

#[test]
fn due_timer_stays_due_until_rearmed() {
    let mut rng = fastrand::Rng::with_seed(4);
    let mut t = GiftTimer { remaining: 1.0 };
    assert!(t.tick(2.0, true, 1.0));
    assert!(t.tick(0.0, false, 1.0), "keeps waiting for a good moment");
    t.rearm(&mut rng, false);
    assert!(t.remaining() >= 20.0 * 60.0);
    assert!(!t.tick(1.0, true, 1.0));
}

#[test]
fn fast_preview_timer_is_seconds() {
    let mut rng = fastrand::Rng::with_seed(4);
    assert!(GiftTimer::new(&mut rng, true).remaining() < 60.0);
}

// ---- 친구 선물 횟수 제한 ----

#[test]
fn friend_limit_is_per_friend_and_recovers() {
    let t0 = Instant::now();
    let mut l = FriendLimit::default();
    assert!(l.allow("a", t0));
    assert!(!l.allow("a", t0 + Duration::from_secs(1)));
    assert!(!l.allow("a", t0 + FRIEND_GIFT_EVERY - Duration::from_secs(1)));
    assert!(l.allow("b", t0 + Duration::from_secs(1)), "another friend has its own limit");
    assert!(l.allow("a", t0 + FRIEND_GIFT_EVERY));
}

#[test]
fn refused_attempts_do_not_push_the_limit_back() {
    let t0 = Instant::now();
    let mut l = FriendLimit::default();
    assert!(l.allow("a", t0));
    for s in 1..100 {
        assert!(!l.allow("a", t0 + Duration::from_secs(s)));
    }
    assert!(l.allow("a", t0 + FRIEND_GIFT_EVERY), "spamming must not extend the wait");
}

// ---- 화면 위 선물 ----

#[test]
fn own_gift_lands_where_the_pet_stands_and_rides_its_window() {
    let mut w = world_with(vec![window(1, 100.0, 500.0, 900.0, 900.0)]);
    let mut f = GiftField::default();
    let t = f.add(gift("cat_yarn"), None, 300.0, 499.0).unwrap();
    f.tick(0.05, &w);
    let g = f.get(t).unwrap();
    assert_eq!((g.x, g.y), (300.0, 500.0));
    assert_eq!(f.ridden_windows().collect::<Vec<_>>(), vec![1]);
    w.patch_windows(&[(1, Rect::new(200.0, 400.0, 1000.0, 800.0))]);
    f.tick(0.016, &w);
    let g = f.get(t).unwrap();
    assert_eq!((g.x, g.y), (400.0, 400.0), "follows the window it sits on");
}

#[test]
fn gift_falls_to_the_ground_when_its_window_closes() {
    let w = world_with(vec![window(1, 100.0, 500.0, 900.0, 900.0)]);
    let mut f = GiftField::default();
    let t = f.add(gift("cat_yarn"), None, 300.0, 499.0).unwrap();
    f.tick(0.05, &w);
    let gone = world_with(vec![]);
    for _ in 0..200 {
        f.tick(0.016, &gone);
    }
    let g = f.get(t).unwrap();
    assert_eq!((g.x, g.y), (300.0, 1040.0));
    assert_eq!(f.ridden_windows().count(), 0);
}

#[test]
fn gift_falls_when_a_window_covers_its_spot() {
    let w = world_with(vec![window(1, 100.0, 500.0, 900.0, 900.0)]);
    let mut f = GiftField::default();
    let t = f.add(gift("cat_yarn"), None, 300.0, 499.0).unwrap();
    f.tick(0.05, &w);
    let covered = world_with(vec![window(2, 200.0, 300.0, 500.0, 700.0), window(1, 100.0, 500.0, 900.0, 900.0)]);
    for _ in 0..200 {
        f.tick(0.016, &covered);
    }
    let g = f.get(t).unwrap();
    assert_eq!((g.x, g.y), (300.0, 1040.0), "drops past the covering window to the ground");
}

#[test]
fn gift_from_a_flying_pet_falls_to_the_first_surface() {
    let w = world_with(vec![window(1, 100.0, 500.0, 900.0, 900.0)]);
    let mut f = GiftField::default();
    let t = f.add(gift("bird_feather"), None, 300.0, 200.0).unwrap();
    f.tick(0.016, &w);
    assert!(f.get(t).unwrap().y < 500.0, "still falling");
    for _ in 0..200 {
        f.tick(0.016, &w);
    }
    assert_eq!(f.get(t).unwrap().y, 500.0);
}

#[test]
fn gift_outside_every_screen_comes_back_onto_the_nearest_ground() {
    let w = world_with(vec![]);
    let mut f = GiftField::default();
    let t = f.add(gift("cat_yarn"), None, 5000.0, 100.0).unwrap();
    for _ in 0..300 {
        f.tick(0.016, &w);
    }
    let g = f.get(t).unwrap();
    assert_eq!(g.y, 1040.0);
    assert!(g.x <= 1920.0, "{}", g.x);
}

#[test]
fn ground_gift_stays_when_a_maximized_window_covers_the_ground() {
    let w = world_with(vec![]);
    let mut f = GiftField::default();
    let t = f.add(gift("cat_yarn"), None, 300.0, 1039.0).unwrap();
    f.tick(0.05, &w);
    assert_eq!(f.get(t).unwrap().y, 1040.0);
    let max = world_with(vec![window(1, 0.0, 0.0, 1920.0, 1040.0)]);
    f.tick(0.016, &max);
    assert_eq!(f.get(t).unwrap().y, 1040.0);
}

#[test]
fn uncollected_gift_vanishes_after_thirty_minutes() {
    let w = world_with(vec![]);
    let mut f = GiftField::default();
    let t = f.add(gift("cat_yarn"), None, 300.0, 1039.0).unwrap();
    assert!(f.tick(LIFE_SECS - 1.0, &w).is_empty());
    assert_eq!(f.tick(2.0, &w), vec![t]);
    assert!(f.get(t).is_none());
    assert!(f.is_empty());
    assert_eq!(LIFE_SECS, 30.0 * 60.0);
}

#[test]
fn collecting_takes_the_gift_once() {
    let mut f = GiftField::default();
    let t = f.add(gift("cat_yarn_shiny"), Some("친구".into()), 300.0, 100.0).unwrap();
    let (g, from) = f.collect(t).unwrap();
    assert_eq!((g.id, g.shiny, from.as_deref()), ("cat_yarn", true, Some("친구")));
    assert!(f.collect(t).is_none(), "double click counts once");
}

#[test]
fn screen_holds_a_limited_number_of_gifts_and_friends_cannot_fill_it() {
    let mut f = GiftField::default();
    for i in 0..MAX_FRIEND_GIFTS {
        assert!(f.add(gift("rabbit_carrot"), Some("친구".into()), i as f64, 100.0).is_some());
    }
    assert!(f.add(gift("rabbit_carrot"), Some("친구".into()), 0.0, 100.0).is_none(), "friend cap");
    for i in MAX_FRIEND_GIFTS..MAX_GIFTS {
        assert!(f.add(gift("cat_yarn"), None, i as f64, 100.0).is_some(), "own gifts still fit");
    }
    assert!(!f.has_room());
    assert!(f.add(gift("cat_yarn"), None, 0.0, 100.0).is_none(), "total cap");
    assert_eq!(f.len(), MAX_GIFTS);
    // 하나 주우면 다시 자리가 난다
    assert!(f.collect(1).is_some());
    assert!(f.has_room());
}

// ---- 도감 ----

#[test]
fn book_counts_shiny_and_notes_who_gave_it() {
    let mut b = GiftBook::default();
    assert!(b.is_empty());
    b.add(gift("cat_yarn"), None);
    b.add(gift("cat_yarn"), None);
    b.add(gift("cat_yarn_shiny"), Some("  달\u{202E}이  "));
    let e = b.entry("cat_yarn").unwrap();
    assert_eq!((e.n, e.shiny, e.from.as_deref()), (2, 1, Some("달이")));
    // 내 동물이 준 건 친구 이름을 지우지 않는다
    b.add(gift("cat_yarn"), None);
    assert_eq!(b.entry("cat_yarn").unwrap().from.as_deref(), Some("달이"));
    assert!(b.entry("rabbit_carrot").is_none());
}

#[test]
fn book_counts_do_not_overflow_and_friend_name_is_bounded() {
    let mut b = GiftBook::default();
    b.add(gift("cat_yarn"), Some(&"가".repeat(500)));
    b.items.get_mut("cat_yarn").unwrap().n = u32::MAX;
    b.add(gift("cat_yarn"), None);
    let e = b.entry("cat_yarn").unwrap();
    assert_eq!(e.n, u32::MAX);
    assert_eq!(e.from.as_ref().unwrap().chars().count(), crate::config::MAX_NAME_CHARS);
    // 반짝이 수도 넘치지 않는다
    b.items.get_mut("cat_yarn").unwrap().shiny = u32::MAX;
    b.add(gift("cat_yarn_shiny"), None);
    let e = b.entry("cat_yarn").unwrap();
    assert_eq!((e.n, e.shiny), (u32::MAX, u32::MAX));
}

#[test]
fn book_view_shows_all_sections_with_silhouettes_for_missing_items() {
    let mut b = GiftBook::default();
    b.add(gift("cat_yarn"), None);
    b.add(gift("cat_feather_shiny"), Some("달이"));
    let v = book_view(&b);
    assert_eq!(v.sections.len(), CATALOG.len());
    assert_eq!(v.total, CATALOG.iter().map(|l| l.items.len()).sum::<usize>());
    assert_eq!((v.collected, v.shiny), (2, 1));
    let cat = v.sections.iter().find(|s| s.key == "cat").unwrap();
    assert_eq!(cat.collected, 2);
    let yarn = cat.items.iter().find(|i| i.id == "cat_yarn").unwrap();
    assert_eq!((yarn.count, yarn.shiny, yarn.image.as_str()), (1, 0, "cat_yarn"));
    assert_eq!(yarn.name, "털실 뭉치");
    // 반짝이만 있으면 반짝이 그림으로
    let feather = cat.items.iter().find(|i| i.id == "cat_feather").unwrap();
    assert_eq!((feather.count, feather.shiny, feather.image.as_str()), (0, 1, "cat_feather_shiny"));
    assert_eq!(feather.from.as_deref(), Some("달이"));
    // 아직 못 모은 것은 이름도 비밀
    let bone = cat.items.iter().find(|i| i.id == "cat_fishbone").unwrap();
    assert_eq!((bone.count, bone.shiny, bone.name), (0, 0, UNKNOWN_NAME));
}

#[test]
fn book_roundtrips_and_is_not_written_while_empty() {
    use crate::config::Config;
    let plain = serde_json::to_string(&Config::default()).unwrap();
    assert!(!plain.contains("gifts"), "{plain}");
    let old: Config = serde_json::from_str(r#"{"pet":"cat"}"#).unwrap();
    assert!(old.gifts.is_empty());
    let mut c = Config::default();
    c.gifts.add(gift("rabbit_carrot_shiny"), Some("달이"));
    let back: Config = serde_json::from_str(&serde_json::to_string(&c).unwrap()).unwrap();
    assert_eq!(back, c);
}

#[test]
fn one_bad_book_entry_or_a_bad_book_does_not_lose_the_config() {
    use crate::config::Config;
    let c: Config = serde_json::from_str(r#"{"pet":"cat","gifts":{"cat_yarn":{"n":3},"rabbit_carrot":"oops","cat_feather":{"n":-1}}}"#).unwrap();
    assert_eq!(c.pet.as_deref(), Some("cat"));
    assert_eq!(c.gifts.entry("cat_yarn").map(|e| e.n), Some(3));
    assert!(c.gifts.entry("rabbit_carrot").is_none());
    assert!(c.gifts.entry("cat_feather").is_none());
    let c: Config = serde_json::from_str(r#"{"pet":"rabbit","gifts":[1,2,3]}"#).unwrap();
    assert_eq!(c.pet.as_deref(), Some("rabbit"));
    assert!(c.gifts.is_empty());
}

#[test]
fn gifts_wait_for_a_calm_moment() {
    use crate::brain::Anim;
    for a in [Anim::Walk, Anim::Idle, Anim::SitFront, Anim::Hop] {
        assert!(calm_anim(a), "{a:?}");
    }
    for a in [Anim::Sleep, Anim::Fall, Anim::Dragged, Anim::Eat, Anim::Climb, Anim::LevelUp, Anim::Roll] {
        assert!(!calm_anim(a), "{a:?}");
    }
}

#[test]
fn speech_is_english_and_short() {
    for list in CATALOG {
        assert!(list.speech.is_ascii() && list.speech.len() <= 24, "{}", list.speech);
    }
    assert!(SHINY_SPEECH.chars().count() <= 24 && COLLECT_SPEECH.is_ascii());
    assert_eq!(collect_speech(Some("Dan")), "from Dan!");
    assert_eq!(collect_speech(None), COLLECT_SPEECH);
    assert_eq!(collect_speech(Some(" D\u{202E}an ")), "from Dan!", "another person's name is cleaned");
    assert_eq!(collect_speech(Some(" \u{200B} ")), COLLECT_SPEECH);
}

// ---- 어디에 떨어뜨리나 (진짜 동물로) ----

fn pet_cfg(flies: bool) -> crate::brain::PetConfig {
    crate::brain::PetConfig { flies, ..crate::test_util::pet_cfg() }
}

/// 선물을 떨어뜨려도 되는 때가 올 때까지 돌린다
fn run_until_calm(p: &mut crate::brain::Pet, w: &World) {
    for _ in 0..3600 {
        p.update(1.0 / 60.0, w);
        if drop_spot(p, w).is_some() {
            return;
        }
    }
    panic!("never calm: {:?}", p.render().anim);
}

#[test]
fn a_standing_pet_drops_in_front_of_itself_onto_its_surface() {
    let w = world_with(vec![]);
    let mut p = crate::brain::Pet::new(pet_cfg(false), 800.0, 900.0, 5);
    assert!(drop_spot(&p, &w).is_none(), "still in the air");
    run_until_calm(&mut p, &w);
    let (x, y) = drop_spot(&p, &w).unwrap();
    assert_eq!(y, 1039.0);
    assert!((x - (p.body.x + p.facing * 0.4 * 128.0)).abs() < 1e-6, "{x} vs {}", p.body.x);
    // 그 자리에서 바닥으로 내려앉는다
    let mut f = GiftField::default();
    let t = f.add(gift("cat_yarn"), None, x, y).unwrap();
    f.tick(0.05, &w);
    assert_eq!(f.get(t).unwrap().y, 1040.0);
}

#[test]
fn no_gift_while_held_housed_or_focusing() {
    let w = world_with(vec![]);
    let mut p = crate::brain::Pet::new(pet_cfg(false), 800.0, 900.0, 5);
    run_until_calm(&mut p, &w);
    p.set_focus(true);
    assert!(drop_spot(&p, &w).is_none(), "focus buddy stays quiet");
    p.set_focus(false);
    assert!(drop_spot(&p, &w).is_some());
    let (x, y) = (p.body.x, p.body.y - 40.0);
    p.pointer_down(x, y);
    p.pointer_move(x + 100.0, y - 200.0);
    assert!(p.is_held());
    assert!(drop_spot(&p, &w).is_none(), "held");
    let mut housed = crate::brain::Pet::new(pet_cfg(false), 800.0, 900.0, 5);
    run_until_calm(&mut housed, &w);
    housed.enter_house();
    assert!(drop_spot(&housed, &w).is_none(), "in the house");
}

#[test]
fn a_flying_pet_drops_from_where_it_flies() {
    let w = world_with(vec![]);
    let mut p = crate::brain::Pet::new(pet_cfg(true), 800.0, 500.0, 9);
    run_until_calm(&mut p, &w);
    p.standing = None;
    p.body.x = 640.0;
    p.body.y = 500.0;
    p.state = idle();
    let (x, y) = drop_spot(&p, &w).unwrap();
    assert_eq!(x, 640.0);
    assert!(y >= 500.0, "from where it flies, not above: {y}");
}

#[test]
fn ground_gift_comes_back_when_its_monitor_shrinks() {
    let wide = World::new(vec![], vec![Rect::new(0.0, 0.0, 3840.0, 1040.0)], 128.0);
    let mut f = GiftField::default();
    let t = f.add(gift("cat_yarn"), None, 3000.0, 1039.0).unwrap();
    f.tick(0.05, &wide);
    assert_eq!(f.get(t).unwrap().y, 1040.0);
    f.tick(0.016, &world_with(vec![]));
    let g = f.get(t).unwrap();
    assert!(g.x <= 1920.0, "off screen at {}", g.x);
    assert_eq!(g.y, 1040.0);
}
