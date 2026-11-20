//! 🕷️ 거미인간 추가 (brain_snare.rs): 🐛 고치 말기 — 감긴 펫이 버둥대다 뿅 풀려남 · 🕸️ 거미집에 걸리기 — 걸린 펫이 빠져나오면 그 거미집은 찢어짐

use super::hoist_moves::{notice_pets, sync_lifts};
use super::snare_moves::StuckKind;
use super::swing_tests::resting;
use super::tests::world;
use super::web_moves::WebPhase;
use super::*;
use crate::test_util::{bundled, pet_cfg, LoopLimit, DT};
use crate::web::{self, Corner, WebSpot};

/// 번들 pet.json 그대로의 거미인간
fn spider() -> PetConfig {
    bundled().into_iter().find(|(id, _)| id == "spider").expect("ui/pets/spider").1
}

/// 바닥(x)에 쉬는 거미인간 — 주사위 놀이는 꺼 둠 (start_*로만)
fn calm_spider(x: f64, seed: u64, w: &World) -> Pet {
    let mut p = resting(spider(), x, seed, w);
    p.web.web_ready_at = f64::INFINITY;
    p.web.swing_ready_at = f64::INFINITY;
    p.web.pull_ready_at = f64::INFINITY;
    p.web.cocoon_ready_at = f64::INFINITY;
    p
}

/// 거미인간과 그 왼쪽 gap(몸 크기 배)에 쉬는 고양이
fn spider_and_cat(gap: f64, w: &World) -> Vec<Pet> {
    let size = spider().size;
    let mut pets = vec![calm_spider(900.0, 5, w), resting(pet_cfg(), 900.0 - gap * size, 6, w)];
    pets[1].state = State::Idle { remaining: 100.0, look: false };
    pets
}

/// 앱처럼 한 틱: 다른 동물 보여 주기 → update → 동물 사이 일
fn step_all(pets: &mut [Pet], w: &World) {
    notice_pets(pets);
    for p in pets.iter_mut() {
        p.update(DT, w);
    }
    sync_lifts(pets);
}

/// 화면 오른쪽 아래 모서리의 거미집 (반지름 radius 몸 크기 배)
fn floor_web(id: u64, radius: f64, size: f64) -> WebSpot {
    WebSpot { id, x: 1920.0, y: 1040.0, corner: Corner::BottomRight, size: radius * size, hidden: false }
}

/// 바닥 x에 쉬는 고양이 — 걸리기 주사위는 chance로
fn cat_at(x: f64, seed: u64, chance: f64, w: &World) -> Pet {
    let mut p = resting(pet_cfg(), x, seed, w);
    p.state = State::Idle { remaining: 100.0, look: false };
    p.snare.chance = chance;
    p
}

/// 걸려 있음
fn snared(p: &Pet) -> bool {
    matches!(p.state, State::Stuck { kind: StuckKind::Snare { .. }, .. })
}

/// 고치에 감겨 있음
fn cocooned(p: &Pet) -> bool {
    matches!(p.state, State::Stuck { kind: StuckKind::Cocoon { .. }, .. })
}

#[test]
fn new_speech_keys_and_the_cocoon_render_flag_serialize_by_name() {
    for (s, name) in [(Speech::Mmph, "mmph"), (Speech::Help, "help"), (Speech::Thanks, "thanks"), (Speech::BrokeFree, "broke_free"), (Speech::Hehe, "hehe")] {
        assert_eq!(serde_json::to_string(&s).unwrap(), format!("\"{name}\""));
    }
    let w = world(vec![]);
    let json = serde_json::to_value(resting(pet_cfg(), 500.0, 1, &w).render()).unwrap();
    assert_eq!(json["cocoon"], 0);
}

#[test]
fn in_web_is_the_quarter_disk_inside_the_corner() {
    // 오른쪽 아래 모서리: 왼쪽 위로 펼쳐짐
    assert!(web::in_web(1900.0, 1030.0, 1920.0, 1040.0, Corner::BottomRight, 100.0));
    assert!(!web::in_web(1800.0, 1030.0, 1920.0, 1040.0, Corner::BottomRight, 100.0), "반지름 밖");
    assert!(!web::in_web(1930.0, 1030.0, 1920.0, 1040.0, Corner::BottomRight, 100.0), "모서리 바깥쪽");
    // 창 왼쪽 위 모서리: 오른쪽 아래로 (창 윗면 위에 선 발은 밖)
    assert!(web::in_web(520.0, 420.0, 500.0, 400.0, Corner::TopLeft, 60.0));
    assert!(!web::in_web(520.0, 399.0, 500.0, 400.0, Corner::TopLeft, 60.0));
    assert!(!web::in_web(480.0, 420.0, 500.0, 400.0, Corner::TopLeft, 60.0));
    assert!(!web::in_web(520.0, 420.0, 500.0, 400.0, Corner::TopLeft, 0.0), "반지름 0은 없음");
}

#[test]
fn the_spider_wraps_a_nearby_pet_with_several_lines_and_it_wriggles_then_pops_free() {
    let w = world(vec![]);
    let mut pets = spider_and_cat(3.0, &w);
    notice_pets(&mut pets);
    let targets = pets[0].cocoon_targets(&w);
    let target = targets.iter().find(|s| s.key == pets[1].social_key).copied().expect("가까운 동물을 노림");
    assert!(pets[0].start_cocoon(&target, &w));
    assert_eq!(pets[0].facing, -1.0, "그 동물 쪽을 봄");
    assert_eq!(pets[0].speech, Some(Speech::Thwip));
    let mut tips: Vec<(f64, f64)> = Vec::new();
    let mut bands = Vec::new();
    let (mut mmph, mut hehe, mut stuck_for, mut freed) = (false, false, 0.0, false);
    let feet = (pets[1].body.x, pets[1].body.y);
    let mut guard = LoopLimit::new();
    for _ in 0..(60 * 12) {
        guard.step();
        step_all(&mut pets, &w);
        if let Some(l) = pets[0].web_line() {
            if !tips.iter().any(|t| (t.0 - l.to.0).hypot(t.1 - l.to.1) < 1.0) && matches!(pets[0].state, State::Web { phase: WebPhase::Wrap { .. }, t } if t > 0.5 * web::WEAVE_SHOT_SECS) {
                tips.push(l.to);
            }
        }
        hehe |= pets[0].speech == Some(Speech::Hehe);
        if cocooned(&pets[1]) {
            stuck_for += DT;
            let r = pets[1].render();
            assert!(r.wobble, "버둥버둥");
            bands.push(r.cocoon);
            mmph |= pets[1].speech == Some(Speech::Mmph);
            assert_eq!((pets[1].body.x, pets[1].body.y), feet, "못 움직임");
        } else if stuck_for > 0.0 && !freed {
            freed = true;
            assert_eq!(pets[1].speech, Some(Speech::BrokeFree), "뿅 풀려남");
            assert_eq!(pets[1].render().cocoon, 0);
        }
    }
    assert!(tips.len() >= 3, "줄을 여러 번, 매번 다른 자리로 {tips:?}");
    assert!(tips.iter().all(|t| (t.0 - feet.0).abs() < pets[1].cfg.size && t.1 < feet.1 + 1.0 && t.1 > feet.1 - 1.2 * pets[1].cfg.size), "줄은 그 동물 몸 둘레로 {tips:?}");
    assert!(bands.windows(2).all(|v| v[1] >= v[0]), "실 감김이 늘어남 {bands:?}");
    assert_eq!(bands.first(), Some(&1));
    assert_eq!(*bands.last().unwrap(), web::COCOON_SHOTS as u8);
    assert!(mmph && hehe, "감긴 펫은 mmph!, 거미인간은 hehe");
    assert!(freed);
    assert!(stuck_for >= web::COCOON_SECS.0 && stuck_for <= web::COCOON_SECS.1 + f64::from(web::COCOON_SHOTS) * web::WEAVE_SHOT_SECS + 0.1, "{stuck_for}");
    assert!(!matches!(pets[0].state, State::Web { .. }), "거미인간도 끝냄");
}

#[test]
fn it_only_wraps_free_pets_in_reach_on_its_screen() {
    let w = world(vec![]);
    // 너무 멀면 안 노림
    let mut pets = spider_and_cat(20.0, &w);
    notice_pets(&mut pets);
    assert!(pets[0].cocoon_targets(&w).is_empty());
    // 눌려 있거나 하우스·조작 중인 동물은 안 노림
    for busy in 0..3 {
        let mut pets = spider_and_cat(3.0, &w);
        match busy {
            0 => {
                let (x, y) = (pets[1].body.x, pets[1].body.y - 10.0);
                pets[1].pointer_down(x, y);
            }
            1 => pets[1].housed = true,
            _ => pets[1].state = State::Hoist { phase: hoist_moves::HoistPhase::Slump, t: 0.0 },
        }
        notice_pets(&mut pets);
        assert!(pets[0].cocoon_targets(&w).is_empty(), "바쁜 동물 {busy}");
    }
    // 다른 동물은 고치를 말지 않음
    let mut cats = vec![resting(pet_cfg(), 500.0, 1, &w), resting(pet_cfg(), 800.0, 2, &w)];
    notice_pets(&mut cats);
    assert!(cats[0].cocoon_targets(&w).is_empty());
}

#[test]
fn left_alone_near_a_pet_it_cocoons_it_now_and_then() {
    let w = world(vec![]);
    let mut wrapped = 0;
    for seed in 0..4 {
        let mut pets = spider_and_cat(3.0, &w);
        pets[0] = resting(spider(), 900.0, seed, &w);
        pets[0].web.web_ready_at = f64::INFINITY;
        pets[0].web.swing_ready_at = f64::INFINITY;
        pets[0].web.pull_ready_at = f64::INFINITY;
        pets[0].web.cocoon_ready_at = 0.0;
        for _ in 0..(60 * 300) {
            // 고양이는 그 자리에 쉬게 둠 (돌아다니면 멀어질 수 있음)
            if matches!(pets[1].state, State::Move { .. }) {
                pets[1].state = State::Idle { remaining: 100.0, look: false };
            }
            step_all(&mut pets, &w);
            if cocooned(&pets[1]) {
                wrapped += 1;
                break;
            }
        }
    }
    assert!(wrapped >= 2, "가끔 고치를 맒 {wrapped}");
}

#[test]
fn clicking_a_cocooned_pet_frees_it_at_once() {
    let w = world(vec![]);
    let mut pets = spider_and_cat(3.0, &w);
    notice_pets(&mut pets);
    let target = pets[0].cocoon_targets(&w)[0];
    assert!(pets[0].start_cocoon(&target, &w));
    for _ in 0..30 {
        step_all(&mut pets, &w);
    }
    assert!(cocooned(&pets[1]));
    let (x, y) = (pets[1].body.x, pets[1].body.y - 20.0);
    pets[1].pointer_down(x, y);
    assert!(!cocooned(&pets[1]), "바로 풀림");
    assert_eq!(pets[1].speech, Some(Speech::Thanks));
    assert_eq!(pets[1].render().cocoon, 0);
    pets[1].pointer_up(x, y);
    assert!(!pets[1].is_held());
    // 거미인간은 줄을 거둠 (더 감지 않음)
    for _ in 0..60 {
        step_all(&mut pets, &w);
        assert!(!cocooned(&pets[1]));
    }
}

#[test]
fn a_pet_walking_into_a_floor_web_gets_snared_then_escapes_and_tears_it_once() {
    let w = world(vec![]);
    let size = pet_cfg().size;
    let mut p = cat_at(1920.0 - 5.0 * size, 1, 1.0, &w);
    p.set_webs(&[floor_web(9, 2.5, size)]);
    p.facing = 1.0;
    p.state = State::Move { kind: Move::Walk, remaining: 100.0 };
    let mut guard = LoopLimit::new();
    while !snared(&p) {
        guard.step();
        p.update(DT, &w);
        assert!(p.take_torn_web().is_none());
    }
    assert_eq!(p.speech, Some(Speech::Help));
    let at = (p.body.x, p.body.y);
    assert!(web::in_web(at.0, at.1 - 0.5 * size, 1920.0, 1040.0, Corner::BottomRight, 2.5 * size) || web::in_web(at.0, at.1 - 1.0, 1920.0, 1040.0, Corner::BottomRight, 2.5 * size));
    let mut stuck_for = 0.0;
    let mut torn = Vec::new();
    while snared(&p) {
        guard.step();
        assert!(p.render().wobble, "버둥버둥");
        assert_eq!((p.body.x, p.body.y), at, "못 움직임");
        p.update(DT, &w);
        stuck_for += DT;
        torn.extend(p.take_torn_web());
    }
    assert!(stuck_for >= web::SNARE_SECS.0 - 0.05 && stuck_for <= web::SNARE_SECS.1 + 0.05, "{stuck_for}");
    for _ in 0..60 {
        p.update(DT, &w);
        torn.extend(p.take_torn_web());
    }
    assert_eq!(torn, vec![9], "찢어진 거미집은 한 번");
    assert_eq!(p.speech.is_some(), true);
}

#[test]
fn a_falling_pet_is_caught_in_mid_air_and_drops_after_escaping() {
    let w = world(vec![]);
    let size = pet_cfg().size;
    let mut p = cat_at(1920.0 - 1.0 * size, 2, 1.0, &w);
    p.standing = None;
    p.body.y = 300.0;
    p.state = State::Air { anim: Anim::Fall };
    p.set_webs(&[floor_web(3, 2.5, size)]);
    let mut guard = LoopLimit::new();
    while !snared(&p) {
        guard.step();
        p.update(DT, &w);
        assert!(p.standing.is_none(), "바닥에 닿기 전에 걸림");
    }
    assert!(p.body.y < 1040.0 - 1.0);
    while snared(&p) {
        guard.step();
        p.update(DT, &w);
    }
    assert!(matches!(p.state, State::Air { .. }), "빠져나오면 떨어짐 {:?}", p.state);
    p.set_webs(&[]);
    while p.standing.is_none() {
        guard.step();
        p.update(DT, &w);
    }
}

#[test]
fn spiders_and_hidden_webs_never_snare() {
    let w = world(vec![]);
    let size = spider().size;
    let mut s = calm_spider(1920.0 - 0.6 * size, 1, &w);
    s.snare.chance = 1.0;
    s.set_webs(&[floor_web(1, 2.5, size)]);
    let mut cat = cat_at(1920.0 - 0.6 * size, 1, 1.0, &w);
    cat.set_webs(&[WebSpot { hidden: true, ..floor_web(2, 2.5, size) }]);
    for _ in 0..(60 * 3) {
        s.update(DT, &w);
        cat.update(DT, &w);
        assert!(!snared(&s) && !snared(&cat));
    }
}

#[test]
fn clicking_a_snared_pet_frees_it_and_tears_the_web() {
    let w = world(vec![]);
    let size = pet_cfg().size;
    let mut p = cat_at(1920.0 - 0.6 * size, 1, 1.0, &w);
    p.set_webs(&[floor_web(4, 2.0, size)]);
    p.update(DT, &w);
    assert!(snared(&p));
    let (x, y) = (p.body.x, p.body.y - 20.0);
    p.pointer_down(x, y);
    assert!(!snared(&p));
    assert_eq!(p.speech, Some(Speech::Thanks));
    assert_eq!(p.take_torn_web(), Some(4));
    assert_eq!(p.take_torn_web(), None);
    p.pointer_up(x, y);
    assert!(!p.is_held());
}

#[test]
fn a_freed_pet_is_immune_for_a_while_and_a_vanished_web_lets_go() {
    let w = world(vec![]);
    let size = pet_cfg().size;
    let mut p = cat_at(1920.0 - 0.6 * size, 1, 1.0, &w);
    p.set_webs(&[floor_web(4, 2.0, size)]);
    p.update(DT, &w);
    p.pointer_down(p.body.x, p.body.y - 20.0);
    p.pointer_up(p.body.x, p.body.y - 20.0);
    let _ = p.take_torn_web();
    // 거미인간이 바로 같은 자리에 새 거미집을 쳐도 한동안은 안 걸림
    p.set_webs(&[floor_web(5, 2.0, size)]);
    let mut t = 0.0;
    while t < web::SNARE_IMMUNE_SECS - 0.1 {
        p.update(DT, &w);
        if matches!(p.state, State::Move { .. }) {
            p.state = State::Idle { remaining: 100.0, look: false };
        }
        assert!(!snared(&p), "방금 빠져나온 펫은 안 걸림 {t}");
        t += DT;
    }
    // 걸린 거미집이 사라지면 (빗자루·치우기) 풀려남 — 찢었다고 알리지 않음
    let mut q = cat_at(1920.0 - 0.6 * size, 3, 1.0, &w);
    q.set_webs(&[floor_web(6, 2.0, size)]);
    q.update(DT, &w);
    assert!(snared(&q));
    q.set_webs(&[]);
    q.update(DT, &w);
    assert!(!snared(&q));
    assert_eq!(q.take_torn_web(), None);
}

#[test]
fn only_some_walk_ins_are_caught() {
    let w = world(vec![]);
    let size = pet_cfg().size;
    let mut caught = 0;
    for seed in 0..20 {
        let mut p = cat_at(1920.0 - 0.6 * size, seed, web::SNARE_CHANCE, &w);
        p.set_webs(&[floor_web(1, 2.0, size)]);
        p.update(DT, &w);
        caught += usize::from(snared(&p));
    }
    assert!((4..=18).contains(&caught), "반쯤 넘게 걸림 {caught}/20");
    // 0이면 절대 안 걸림
    let mut p = cat_at(1920.0 - 0.6 * size, 1, 0.0, &w);
    p.set_webs(&[floor_web(1, 2.0, size)]);
    for _ in 0..60 {
        p.update(DT, &w);
        assert!(!snared(&p));
    }
}

/// 🕸️ 걸린 동물도 탁 멈추지 않고 "팅~": 그림이 걸어 들어온 쪽으로 늘어났다가 출렁 (몸 자리는 그대로) — 그 뒤 살랑살랑만
#[test]
fn a_snared_pet_twangs_along_its_entry_then_only_sways() {
    let w = world(vec![]);
    let size = pet_cfg().size;
    let mut p = cat_at(1920.0 - 5.0 * size, 1, 1.0, &w);
    p.set_webs(&[floor_web(9, 2.5, size)]);
    p.facing = 1.0;
    p.state = State::Move { kind: Move::Walk, remaining: 100.0 };
    let mut guard = LoopLimit::new();
    while !snared(&p) {
        guard.step();
        p.update(DT, &w);
    }
    let still = (p.body.x - size / 2.0, p.body.y - size);
    let mut dx = Vec::new();
    let mut dy = Vec::new();
    for _ in 0..(1.5 / DT) as usize {
        p.update(DT, &w);
        if !snared(&p) {
            break;
        }
        let (x, y) = p.window_pos();
        dx.push(x - still.0);
        dy.push(y - still.1);
    }
    let spring = (web::SNAG_SPRING_SECS / DT) as usize - 1;
    let first = dx.iter().position(|v| v.abs() > 1.0).expect("출렁");
    assert!(dx[first] > 0.0, "걸어 들어온 쪽(오른쪽)으로 먼저 {dx:?}");
    assert!(dx[..spring].iter().any(|v| *v < -1.0), "되돌아오며 출렁");
    assert!(dx[..spring].iter().copied().fold(0.0, f64::max) > 0.1 * size);
    assert!(dx[spring + 2..].iter().all(|v| v.abs() < 0.05 * size), "그 뒤 살랑살랑만");
    assert!(dy.iter().all(|v| v.abs() < 1e-6), "옆으로 걸어 들어왔으면 옆으로만");
}

/// 🕸️ 거미인간은 어느 거미인간의 거미집이든 걸어 들어가도 떨어져 들어가도 안 걸림
#[test]
fn spiders_walking_or_falling_into_any_web_are_never_snared() {
    let w = world(vec![]);
    let size = spider().size;
    let mut walker = calm_spider(1920.0 - 5.0 * size, 2, &w);
    walker.snare.chance = 1.0;
    walker.set_webs(&[floor_web(4, 3.0, size)]);
    walker.facing = 1.0;
    walker.state = State::Move { kind: Move::Walk, remaining: 100.0 };
    let mut faller = calm_spider(1920.0 - 1.0 * size, 3, &w);
    faller.snare.chance = 1.0;
    faller.standing = None;
    faller.body.y = 300.0;
    faller.state = State::Air { anim: Anim::Fall };
    faller.set_webs(&[floor_web(4, 3.0, size)]);
    for _ in 0..(60 * 4) {
        walker.update(DT, &w);
        faller.update(DT, &w);
        assert!(!walker.is_stuck() && !faller.is_stuck());
        assert!(walker.take_torn_web().is_none() && faller.take_torn_web().is_none());
    }
}
