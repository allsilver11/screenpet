//! 🌱 연령대별 특기 (2026-11-20 spec 2부 — stage.rs + brain_stage.rs): 연령대 배 표, 모든 연령대가 하지만 제 연령대가 훨씬 자주,
//! 종별 특기의 세기, 펫끼리 놀이 성향, 레벨업 뒤 새 특기 한 줄, 손님도 제 연령대를 앎.

use super::stage_moves::StageAct;
use super::tests::{place, world};
use super::*;
use crate::care::Stage;
use crate::social::RompKind;
use crate::stage::{self as st, StageTrait};
use crate::test_util::{pet_cfg, DT};

/// 이 연령대의 평범한 동물
fn aged(stage: Stage) -> PetConfig {
    PetConfig { stage, ..pet_cfg() }
}

/// 바닥 x에 쉬는 동물 (친구 화면 끝 없음)
fn on_floor(c: PetConfig, x: f64, seed: u64, w: &World) -> Pet {
    let mut p = Pet::new(c, x, 0.0, seed);
    place(&mut p, w, Owner::Ground(0), x, State::Idle { remaining: 0.5, look: false });
    p.set_exit_edges(false, false);
    p
}

/// home에서 두 단계 이상 먼 연령대
fn far_from(home: Stage) -> Stage {
    if home.step() >= 3 {
        Stage::Baby
    } else {
        Stage::Adult
    }
}

#[test]
fn stage_weights_are_one_near_point_three_five_and_far_point_one_five() {
    assert_eq!(st::STAGE_WEIGHTS, [1.0, 0.35, 0.15]);
    assert_eq!(st::stage_weight(Stage::Baby, Stage::Baby), 1.0);
    assert_eq!(st::stage_weight(Stage::Kinder, Stage::Baby), 0.35);
    assert_eq!(st::stage_weight(Stage::Teenager, Stage::Adult), 0.35);
    assert_eq!(st::stage_weight(Stage::Child, Stage::Baby), 0.15);
    assert_eq!(st::stage_weight(Stage::Adult, Stage::Baby), 0.15);
}

#[test]
fn every_trait_is_possible_at_every_stage_but_its_own_stage_does_it_most() {
    for t in StageTrait::ALL {
        let own = t.rate(t.home());
        for s in Stage::ALL {
            let r = t.rate(s);
            assert!(r > 0.0, "{t:?} at {s:?}");
            if s != t.home() {
                assert!(own >= r / 0.35 - 1e-9, "{t:?}: own {own} vs {s:?} {r}");
            }
        }
    }
}

#[test]
fn species_power_grows_with_stage_and_teenagers_stay_as_before() {
    let p: Vec<f64> = Stage::ALL.iter().map(|&s| st::stage_power(s)).collect();
    assert_eq!(p, vec![0.6, 0.75, 0.9, 1.0, 1.2]);
    assert_eq!(st::soft_power(Stage::Teenager), 1.0);
    assert_eq!(st::lead_power(Stage::Teenager), 1.0);
    assert!(st::soft_power(Stage::Baby) > st::stage_power(Stage::Baby), "빠르기·높이는 덜 변함");
    assert!(st::lead_power(Stage::Baby) < st::stage_power(Stage::Baby), "아기 오리는 거의 따라가기만");
}

#[test]
fn social_bias_follows_the_stage_table() {
    let bias = |s: Stage, k: RompKind| Pet::new(aged(s), 0.0, 0.0, 1).social_bias(k);
    // 🧑‍🎤 청소년: 투닥투닥·밀치기 ↑
    assert!(bias(Stage::Teenager, RompKind::Scuffle) > bias(Stage::Child, RompKind::Scuffle));
    assert!(bias(Stage::Teenager, RompKind::Shove) > bias(Stage::Kinder, RompKind::Shove));
    // 🧑 어린이: 찌르기·시합 ↑
    for k in [RompKind::Prank, RompKind::Race] {
        for s in [Stage::Baby, Stage::Kinder, Stage::Teenager, Stage::Adult] {
            assert!(bias(Stage::Child, k) > bias(s, k), "{k:?} {s:?}");
        }
    }
    // 🧓 어른: 낮잠·수다 ↑, 투닥투닥 ↓ · 👶 아기는 싸움을 거의 안 함 (금지가 아니라 배)
    assert!(bias(Stage::Adult, RompKind::Nap) > bias(Stage::Teenager, RompKind::Nap));
    assert!(bias(Stage::Adult, RompKind::Chat) > bias(Stage::Child, RompKind::Chat));
    assert!(bias(Stage::Adult, RompKind::Scuffle) < bias(Stage::Teenager, RompKind::Scuffle) / 2.0);
    let baby_fight = bias(Stage::Baby, RompKind::Scuffle);
    assert!(baby_fight > 0.0 && baby_fight < 0.5);
    // 👶 졸졸 · 🧓 쓰다듬기
    assert!(bias(Stage::Baby, RompKind::Tag) > bias(Stage::Child, RompKind::Tag));
    assert!(bias(Stage::Adult, RompKind::Pat) > bias(Stage::Child, RompKind::Pat));
    assert_eq!(bias(Stage::Child, RompKind::Prank), st::BIAS_FLOOR + st::BIAS_GAIN);
}

/// 한 마리를 바닥에서 secs초 돌림 (같은 주사위 씨앗 둘을 더함)
fn simulate(stage: Stage, secs: f64) -> (Pet, Pet, f64) {
    let w = world(vec![]);
    let mut asleep = 0.0;
    let mut run = |seed: u64| {
        let mut p = on_floor(aged(stage), 900.0, seed, &w);
        for _ in 0..(secs / DT) as usize {
            p.update(DT, &w);
            if matches!(p.state, State::Sleep { .. }) {
                asleep += DT;
            }
            let _ = p.take_speech();
            let _ = p.take_speech_text();
        }
        p
    };
    let a = run(11);
    let b = run(12);
    (a, b, asleep)
}

#[test]
fn each_trait_shows_up_much_more_at_its_own_stage_and_sometimes_elsewhere() {
    let secs = 25.0 * 60.0;
    let runs: Vec<(Stage, Counts, f64)> = Stage::ALL
        .iter()
        .map(|&s| {
            let (a, b, asleep) = simulate(s, secs);
            let counts = StageTrait::ALL.map(|t| a.stage_count(t) + b.stage_count(t));
            (s, Counts(counts), asleep)
        })
        .collect();
    let count = |s: Stage, t: StageTrait| {
        let i = StageTrait::ALL.iter().position(|&x| x == t).unwrap();
        runs.iter().find(|r| r.0 == s).unwrap().1 .0[i]
    };
    for t in [
        StageTrait::Toddle,
        StageTrait::Plop,
        StageTrait::Waah,
        StageTrait::Ooh,
        StageTrait::HappyHop,
        StageTrait::HopSkip,
        StageTrait::Flip,
        StageTrait::Sulk,
        StageTrait::Sprint,
        StageTrait::Stretch,
    ] {
        let (home, far) = (t.home(), far_from(t.home()));
        let (own, distant) = (count(home, t), count(far, t));
        let elsewhere: u32 = Stage::ALL.iter().filter(|&&s| s != home).map(|&s| count(s, t)).sum();
        assert!(own >= 4, "{t:?}: {own} at {home:?}");
        assert!(own > 2 * distant, "{t:?}: {own} at {home:?} vs {distant} at {far:?}");
        assert!(elsewhere > 0, "{t:?} should still happen at other stages");
    }
    // 👶 아기는 더 오래 잠 (깨어 있는 시간 대비)
    let asleep = |s: Stage| runs.iter().find(|r| r.0 == s).unwrap().2;
    assert!(asleep(Stage::Baby) > 1.3 * asleep(Stage::Adult), "baby {} vs adult {}", asleep(Stage::Baby), asleep(Stage::Adult));
}

/// 행동별 센 수 묶음 (테스트 표)
struct Counts([u32; StageTrait::ALL.len()]);

#[test]
fn a_baby_trips_plops_down_and_says_oops() {
    let w = world(vec![]);
    let mut p = on_floor(aged(Stage::Baby), 900.0, 3, &w);
    p.state = p.stage_act(StageAct::Plop, st::PLOP_SECS);
    let (mut anims, mut said, mut wobbled) = (Vec::new(), Vec::new(), false);
    for _ in 0..(60.0 * st::PLOP_SECS) as usize + 5 {
        p.update(DT, &w);
        anims.push(p.render().anim);
        wobbled |= p.render().wobble;
        said.extend(p.take_speech());
    }
    assert!(anims.contains(&Anim::Fall) && anims.contains(&Anim::SitFront), "{anims:?}");
    assert!(said.contains(&Speech::Plop), "{said:?}");
    assert!(wobbled);
    assert!(!matches!(p.state, State::StageAct { .. }), "다시 평소대로: {:?}", p.state);
}

#[test]
fn a_teen_backflips_with_a_roll_in_the_air_and_says_yeah() {
    let w = world(vec![]);
    let mut p = on_floor(aged(Stage::Teenager), 900.0, 3, &w);
    let ground = p.window_pos().1;
    p.state = p.stage_act(StageAct::Flip, st::FLIP_SECS);
    let (mut rolled, mut highest, mut said) = (false, 0.0f64, Vec::new());
    for _ in 0..(60.0 * st::FLIP_SECS) as usize + 5 {
        p.update(DT, &w);
        rolled |= p.render().anim == Anim::Roll;
        highest = highest.max(ground - p.window_pos().1);
        said.extend(p.take_speech());
    }
    assert!(rolled);
    assert!(highest > 0.5 * st::FLIP_RISE * p.cfg.size, "{highest}");
    assert_eq!(said, vec![Speech::Flip]);
    assert_eq!(p.window_pos().1, ground, "다시 바닥");
}

#[test]
fn a_kinder_hops_happily_with_sparkles_and_a_teen_sulks() {
    let w = world(vec![]);
    let mut p = on_floor(aged(Stage::Kinder), 900.0, 3, &w);
    p.state = p.stage_act(StageAct::HappyHop, st::HAPPY_HOP_SECS * 2.0);
    let (mut hops, mut sparkle) = (0, false);
    let mut was_hop = false;
    for _ in 0..60 {
        p.update(DT, &w);
        let r = p.render();
        sparkle |= r.sparkle;
        if r.anim == Anim::Hop && !was_hop {
            hops += 1;
        }
        was_hop = r.anim == Anim::Hop;
    }
    assert_eq!(hops, 2);
    assert!(sparkle);
    let mut t = on_floor(aged(Stage::Teenager), 900.0, 3, &w);
    t.state = t.stage_act(StageAct::Sulk, 2.0);
    t.update(DT, &w);
    assert_eq!(t.render().anim, Anim::Annoyed);
}

#[test]
fn a_lonely_baby_sometimes_cries_but_company_resets_the_clock() {
    let w = world(vec![]);
    let mut p = on_floor(aged(Stage::Baby), 900.0, 5, &w);
    p.stage.alone = st::WAAH_ALONE_SECS + 1.0;
    p.note_company();
    assert_eq!(p.stage.alone, 0.0);
    // 혼자 오래: 쉬는 박자마다 가끔 "waah"
    let mut cried = false;
    for seed in 0..200 {
        let mut q = on_floor(aged(Stage::Baby), 900.0, seed, &w);
        q.stage.alone = st::WAAH_ALONE_SECS + 1.0;
        if let Some(State::StageAct { act: StageAct::Waah, .. }) = q.stage_rest() {
            assert_eq!(q.take_speech(), Some(Speech::Waah));
            assert_eq!(q.stage.alone, 0.0, "한 번 울면 다시 셈");
            cried = true;
        }
    }
    assert!(cried);
}

#[test]
fn after_growing_up_the_pet_announces_its_new_signature_trait() {
    let w = world(vec![]);
    let mut p = on_floor(aged(Stage::Baby), 900.0, 3, &w);
    p.state = State::Eating { remaining: 0.05, favorite: false, level_up: true };
    for _ in 0..10 {
        p.update(DT, &w);
    }
    assert!(matches!(p.state, State::LevelUp { .. }), "{:?}", p.state);
    // 앱이 축하를 시작하며 새 단계 설정을 줌 (apply_own_stage)
    p.cfg = aged(Stage::Kinder);
    let mut text = None;
    for _ in 0..(60.0 * 2.5) as usize {
        p.update(DT, &w);
        text = text.or(p.take_speech_text());
    }
    assert_eq!(text.as_deref(), Some(st::signature_line(Stage::Kinder)));
    assert_eq!(st::signature_line(Stage::Teenager), "new: 🤸 flips!");
    // 메뉴로 단계를 바꾸면 (축하 없음) 알리지 않음
    p.cfg = aged(Stage::Adult);
    for _ in 0..120 {
        p.update(DT, &w);
        assert_eq!(p.take_speech_text(), None);
    }
}

#[test]
fn a_housed_pet_also_announces_after_its_celebration() {
    let mut p = Pet::new(aged(Stage::Child), 0.0, 0.0, 3);
    p.housed = true;
    p.state = State::LevelUp { remaining: 0.1 };
    p.cfg = aged(Stage::Teenager);
    let w = world(vec![]);
    let mut text = None;
    for _ in 0..30 {
        p.update(DT, &w);
        text = text.or(p.take_speech_text());
    }
    assert_eq!(text.as_deref(), Some("new: 🤸 flips!"));
}

#[test]
fn own_and_guest_pets_know_their_stage_from_the_manifest() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../ui/pets/cat/pet.json");
    let m = crate::manifest::parse_manifest(&std::fs::read_to_string(path).unwrap()).unwrap();
    for s in Stage::ALL {
        let c = PetConfig::from_manifest(&m, 1.0, m.size_mult(s), s.name());
        assert_eq!(c.stage, s);
        let w = world(vec![]);
        let guest = Pet::new_entering(c, &w, Side::Left, 1);
        assert_eq!(guest.stage(), s, "손님도 Visit에 실려 온 연령대 그대로");
    }
    assert_eq!(PetConfig::from_manifest(&m, 1.0, 1.0, "???").stage, Stage::Teenager);
    let mut p = Pet::new(aged(Stage::Kinder), 0.0, 0.0, 1);
    p.set_cursor_species("cat");
    assert_eq!(p.species(), "cat");
}

#[test]
fn toddling_babies_walk_slower_and_sprinting_turns_walks_into_runs() {
    let baby = Pet::new(aged(Stage::Baby), 0.0, 0.0, 1);
    assert!((baby.toddle_pace(Move::Walk) - (1.0 - st::TODDLE_SLOW)).abs() < 1e-9);
    assert_eq!(baby.toddle_pace(Move::Run), 1.0);
    let adult = Pet::new(aged(Stage::Adult), 0.0, 0.0, 1);
    assert!(adult.toddle_pace(Move::Walk) > baby.toddle_pace(Move::Walk));
    let w = world(vec![]);
    let mut runs = 0;
    for seed in 0..300 {
        let mut t = on_floor(aged(Stage::Teenager), 900.0, seed, &w);
        if t.stage_sprint(Move::Walk) == Move::Run {
            runs += 1;
        }
    }
    let expect = 300.0 * StageTrait::Sprint.rate(Stage::Teenager);
    assert!((runs as f64 - expect).abs() < 0.5 * expect, "{runs} vs {expect}");
}

#[test]
fn kinder_pets_love_the_cursor_and_children_love_tag() {
    let fan = |s: Stage| Pet::new(aged(s), 0.0, 0.0, 1).cursor_fan();
    assert!(fan(Stage::Kinder) > fan(Stage::Adult) && fan(Stage::Adult) > 1.0);
    let tag = |s: Stage| Pet::new(aged(s), 0.0, 0.0, 1).tag_fan();
    assert!(tag(Stage::Child) > tag(Stage::Baby) && tag(Stage::Baby) > 1.0);
}

// ---- 종별 특기의 세기 ----

#[test]
fn spider_webs_keep_their_size_range_but_get_more_shots_as_it_grows() {
    let w = world(vec![]);
    let spot = crate::web::WebSpin { x: 0.0, y: 1040.0, corner: crate::web::Corner::BottomLeft, size: 0.0, window: None, shots: 0, build: 0.0 };
    let mean = |s: Stage| {
        let mut p = on_floor(PetConfig { slings_webs: true, ..aged(s) }, 900.0, 9, &w);
        let spins: Vec<WebSpinStat> = (0..200).map(|_| WebSpinStat::of(p.sized_web(spot, &w), p.cfg.size)).collect();
        let size = spins.iter().map(|x| x.k).sum::<f64>() / spins.len() as f64;
        let lo = spins.iter().map(|x| x.shots).min().unwrap();
        let hi = spins.iter().map(|x| x.shots).max().unwrap();
        (size, lo, hi)
    };
    let (baby, teen, adult) = (mean(Stage::Baby), mean(Stage::Teenager), mean(Stage::Adult));
    // 크기는 몸집·연령대와 상관없음 (사용자 결정 2026-11-20): 같은 주사위면 같은 크기 — 연령대는 짜는 줄 수만 바꿈
    assert!((baby.0 - teen.0).abs() < 1e-9 && (adult.0 - teen.0).abs() < 1e-9, "{baby:?} {teen:?} {adult:?}");
    assert!(baby.1 >= 2 && baby.2 <= 4, "아기 2~4번: {baby:?}");
    assert!(teen.1 >= crate::web::WEAVE_SHOTS.0 && teen.2 <= crate::web::WEAVE_SHOTS.1, "청소년은 예전 그대로: {teen:?}");
    assert!(adult.2 > teen.2, "어른은 더 많이: {adult:?}");
}

/// 거미집 하나의 크기(몸 크기 배)와 줄 수
struct WebSpinStat {
    k: f64,
    shots: u32,
}

impl WebSpinStat {
    /// 칠 거미집에서 뽑음
    fn of(s: crate::web::WebSpin, body: f64) -> Self {
        WebSpinStat { k: s.size / body, shots: s.shots }
    }
}

#[test]
fn duck_trains_get_longer_and_baby_ducks_rarely_lead() {
    let duck = |s: Stage| Pet::new(PetConfig { leads_train: true, ..aged(s) }, 0.0, 0.0, 1);
    assert_eq!(duck(Stage::Baby).train_followers_max(), 2);
    assert_eq!(duck(Stage::Teenager).train_followers_max(), crate::play::TRAIN_MAX_FOLLOWERS);
    assert_eq!(duck(Stage::Adult).train_followers_max(), 5);
}

#[test]
fn frogs_jump_higher_dinos_roar_more_and_flyers_fly_faster_as_they_grow() {
    let frog = |s: Stage| Pet::new(PetConfig { hops: true, ..aged(s) }, 0.0, 0.0, 1);
    let h = |s: Stage| frog(s).gait(Move::Walk).unwrap().height;
    assert!(h(Stage::Baby) < h(Stage::Teenager) && h(Stage::Teenager) < h(Stage::Adult));
    assert_eq!(h(Stage::Teenager), crate::hop::Gait::new(128.0, 60.0 * Move::Walk.speed_mult(), false).height, "청소년은 예전 그대로");
    let dino = |s: Stage| Pet::new(PetConfig { special: Some(Special::Roar), ..aged(s) }, 0.0, 0.0, 1).special_power(Special::Roar);
    assert!(dino(Stage::Baby) < dino(Stage::Teenager) && dino(Stage::Adult) > 1.0);
    assert_eq!(Pet::new(aged(Stage::Baby), 0.0, 0.0, 1).special_power(Special::Arch), 1.0);
    let fly = |s: Stage| Pet::new(PetConfig { flies: true, ..aged(s) }, 0.0, 0.0, 1).fly_speed(Move::Walk);
    assert!(fly(Stage::Baby) < fly(Stage::Teenager) && fly(Stage::Teenager) < fly(Stage::Adult));
}

#[test]
fn a_teen_with_its_traits_still_mostly_moves_and_rarely_sleeps() {
    let w = world(vec![]);
    let (mut moving, mut sleeping, mut total) = (0usize, 0usize, 0usize);
    for seed in 0..6 {
        let mut p = on_floor(PetConfig { half_w: 38.4, walk_speed: 90.0, ..aged(Stage::Teenager) }, 960.0, seed, &w);
        for _ in 0..(60 * 600) {
            p.update(DT, &w);
            match p.render().anim {
                Anim::Walk | Anim::Run | Anim::Roll => moving += 1,
                Anim::Sleep => sleeping += 1,
                _ => {}
            }
            total += 1;
        }
    }
    let (m, s) = (moving as f64 / total as f64, sleeping as f64 / total as f64);
    assert!(m >= 0.45, "moving {m:.3}");
    assert!(s <= 0.13, "sleeping {s:.3}");
}
