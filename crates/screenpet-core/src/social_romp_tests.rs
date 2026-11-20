//! 펫끼리 놀기 더 (2026-11-20 spec 1부 — social_romp.rs + brain_romp.rs): 🥊 투닥투닥 · 😈 몰래 찌르기 · 🫸 밀치기 ·
//! 😴 같이 낮잠 · 🏁 달리기 시합 · 💬 수다가 맞는 때 시작하고, 차례대로 움직이고 말하고, 깔끔히 끝나 둘 다 풀려나는지.
//! 그리고 집에서 두 마리가 10분 같이 있으면 새 놀이가 대충 3~6번 보이는지.

use super::*;
use crate::behavior::Move;
use crate::brain::{Anim, Pet, PetConfig, Speech, Standing, State};
use crate::geom::Rect;
use crate::test_util::DT;
use crate::world::World;
use super::romp::ROMP_COOLDOWN;

const GROUND: f64 = 1040.0;
const BODY: f64 = 128.0;

fn cfg() -> PetConfig {
    PetConfig { half_w: 38.4, ..crate::test_util::pet_cfg() }
}

fn floor() -> World {
    World::new(vec![], vec![Rect::new(0.0, 0.0, 1920.0, GROUND)], 128.0)
}

fn at(w: &World, c: PetConfig, x: f64, facing: f64, state: State, seed: u64) -> Pet {
    let owner = Owner::Ground(0);
    let (ax, ay) = w.anchor(owner).unwrap();
    let mut p = Pet::new(c, x, ay, seed);
    p.body.x = x;
    p.body.y = ay;
    p.standing = Some(Standing { owner, rel_x: x - ax });
    p.state = state;
    p.facing = facing;
    p.update(0.0, w);
    p
}

fn resting() -> State {
    State::Idle { remaining: 100.0, look: false }
}

fn walking() -> State {
    State::Move { kind: Move::Walk, remaining: 60.0 }
}

fn sleeping() -> State {
    State::Sleep { remaining: 100.0 }
}

/// 다른 놀이(인사·술래·만남·하이파이브)는 끄고 이 놀이만 곧바로 하는 코디네이터
fn only(kind: RompKind, seed: u64) -> Social {
    let mut s = Social::new(seed);
    s.meet_chance = 0.0;
    s.one_way_chance = 0.0;
    s.set_high_five_chance(0.0);
    s.set_romp_only(Some(kind));
    s.set_romp_rate(1e6);
    // 인사는 쉬는 중으로 (짝 쿨다운을 미리 걸어 둠)
    s.hold_greets(1e9);
    s
}

fn step(s: &mut Social, pets: &mut [Pet], w: &World) {
    s.tick(DT, pets);
    for p in pets.iter_mut() {
        p.update(DT, w);
    }
}

/// 한 틱씩 돌리며 그때그때 본 것을 모음
#[derive(Default)]
struct Seen {
    anims: [Vec<Anim>; 2],
    said: [Vec<Speech>; 2],
    wobble: [bool; 2],
    thud: [bool; 2],
    sparkle: [bool; 2],
    heart: [bool; 2],
    /// 가장 많이 뜬 그림 높이 (px)
    lift: [f64; 2],
    /// 놀이가 끝난 틱 (둘 다 놀이 밖)
    ended: Option<usize>,
}

/// 놀이가 끝날 때까지 (최대 secs초) 돌림
fn play_out(s: &mut Social, pets: &mut [Pet], w: &World, secs: f64) -> Seen {
    let mut seen = Seen::default();
    for k in 0..(secs / DT) as usize {
        step(s, pets, w);
        for (i, p) in pets.iter_mut().enumerate().take(2) {
            let r = p.render();
            if seen.anims[i].last() != Some(&r.anim) {
                seen.anims[i].push(r.anim);
            }
            if let Some(sp) = p.take_speech() {
                seen.said[i].push(sp);
            }
            seen.wobble[i] |= r.wobble;
            seen.thud[i] |= r.thud;
            seen.sparkle[i] |= r.sparkle;
            seen.heart[i] |= r.heart;
            seen.lift[i] = seen.lift[i].max(GROUND - BODY - p.window_pos().1);
        }
        if s.romps_active() == 0 && seen.ended.is_none() && k > 0 {
            seen.ended = Some(k);
            break;
        }
    }
    seen
}

/// 놀이가 끝난 둘: 놀이 상태가 아니고 평소대로 돌아감
fn freed(pets: &[Pet]) -> bool {
    pets.iter().all(|p| !p.in_romp() && p.social_role().is_none())
}

// ---- 🥊 투닥투닥 ----

#[test]
fn a_scuffle_closes_in_rolls_as_one_ball_then_one_thuds_and_the_other_gloats() {
    let w = floor();
    let mut pets = vec![at(&w, cfg(), 800.0, 1.0, resting(), 1), at(&w, cfg(), 800.0 + 2.5 * BODY, -1.0, resting(), 2)];
    let mut s = only(RompKind::Scuffle, 1);
    step(&mut s, &mut pets, &w);
    assert!(pets.iter().all(Pet::in_romp), "둘 다 놀이 시작");
    assert_eq!(s.romps_started(), 1);
    let seen = play_out(&mut s, &mut pets, &w, 12.0);
    assert!(seen.ended.is_some_and(|k| k as f64 * DT < 9.0), "몇 초 안에 끝남");
    for i in 0..2 {
        assert!(seen.anims[i].contains(&Anim::Roll) && seen.anims[i].contains(&Anim::Annoyed), "{:?}", seen.anims[i]);
        assert!(seen.wobble[i], "덜덜");
        assert!(seen.lift[i] > 0.05 * BODY, "빙글 돌며 들썩 {}", seen.lift[i]);
    }
    let said: Vec<Speech> = seen.said.concat();
    assert!(said.contains(&Speech::Scuffle), "{said:?}");
    // 진 쪽만 콰당 💫 + "oof", 이긴 쪽은 손 흔들기·흥
    let loser = (0..2).find(|&i| seen.thud[i]).expect("진 쪽이 털썩");
    let winner = 1 - loser;
    assert!(!seen.thud[winner]);
    assert!(seen.said[loser].contains(&Speech::Oof), "{:?}", seen.said[loser]);
    assert!(seen.anims[winner].iter().any(|a| matches!(a, Anim::Wave | Anim::Huff)), "{:?}", seen.anims[winner]);
    assert!(freed(&pets));
}

#[test]
fn the_same_pair_does_not_romp_again_for_a_while() {
    let w = floor();
    let mut pets = vec![at(&w, cfg(), 800.0, 1.0, resting(), 1), at(&w, cfg(), 800.0 + 2.5 * BODY, -1.0, resting(), 2)];
    let mut s = only(RompKind::Scuffle, 3);
    step(&mut s, &mut pets, &w);
    play_out(&mut s, &mut pets, &w, 12.0);
    assert_eq!(s.romps_started(), 1);
    for _ in 0..((ROMP_COOLDOWN.0 - 1.0) / DT) as usize {
        for p in pets.iter_mut() {
            if !matches!(p.state, State::Idle { .. }) && p.social_role().is_none() && !p.in_romp() {
                p.state = resting();
            }
        }
        step(&mut s, &mut pets, &w);
    }
    assert_eq!(s.romps_started(), 1, "쿨다운 동안은 다시 안 함");
}

// ---- 😈 몰래 찌르기 ----

#[test]
fn a_prankster_sneaks_up_behind_a_sleeper_pokes_and_gets_chased() {
    let w = floor();
    // 자는 친구(1)는 오른쪽을 봄 — 장난꾼(0)은 그 뒤(왼쪽)에서 다가감
    let mut pets = vec![at(&w, cfg(), 700.0, 1.0, resting(), 1), at(&w, cfg(), 700.0 + 2.0 * BODY, 1.0, sleeping(), 2)];
    let mut s = only(RompKind::Prank, 4);
    step(&mut s, &mut pets, &w);
    assert!(pets[0].in_romp(), "장난꾼이 살금살금");
    assert!(matches!(pets[1].state, State::Sleep { .. }), "친구는 그대로 잠");
    // 살금살금: 느리게 걸어감 (걷기 속도의 절반 아래)
    let x0 = pets[0].body.x;
    for _ in 0..30 {
        step(&mut s, &mut pets, &w);
    }
    let speed = (pets[0].body.x - x0) / (30.0 * DT);
    assert!(speed > 0.0 && speed < 0.5 * 60.0 * 0.7 + 1e-9, "살금살금 {speed}");
    assert_eq!(pets[0].render().anim, Anim::Walk);
    let mut tilted = false;
    let mut hopped = false;
    let mut said = [Vec::new(), Vec::new()];
    let mut chase = false;
    for _ in 0..(15.0 / DT) as usize {
        step(&mut s, &mut pets, &w);
        tilted |= pets[0].render().anim == Anim::Tilt;
        hopped |= pets[1].render().anim == Anim::Hop;
        for (i, p) in pets.iter_mut().enumerate() {
            if let Some(sp) = p.take_speech() {
                said[i].push(sp);
            }
        }
        if pets[1].social_role() == Some(SocialRole::Chase) && pets[0].social_role() == Some(SocialRole::Flee) {
            chase = true;
            break;
        }
    }
    assert!(tilted, "콕");
    assert!(hopped, "화들짝 폴짝");
    assert!(said[1].contains(&Speech::Hey), "{:?}", said[1]);
    assert!(said[0].contains(&Speech::Hehe), "{:?}", said[0]);
    assert!(chase, "찔린 쪽이 쫓아감");
    // 쫓기도 몇 초 뒤엔 끝나고 둘 다 풀림
    for _ in 0..(12.0 / DT) as usize {
        step(&mut s, &mut pets, &w);
    }
    assert!(freed(&pets));
}

#[test]
fn a_prank_on_a_sleeper_that_wakes_up_first_is_called_off() {
    let w = floor();
    let mut pets = vec![at(&w, cfg(), 700.0, 1.0, resting(), 1), at(&w, cfg(), 700.0 + 2.0 * BODY, 1.0, sleeping(), 2)];
    let mut s = only(RompKind::Prank, 5);
    step(&mut s, &mut pets, &w);
    assert!(pets[0].in_romp());
    pets[1].state = State::Move { kind: Move::Walk, remaining: 5.0 };
    step(&mut s, &mut pets, &w);
    step(&mut s, &mut pets, &w);
    assert!(!pets[0].in_romp());
    assert_eq!(s.romps_active(), 0);
}

// ---- 🫸 밀치기 ----

#[test]
fn a_shove_pushes_the_friend_about_a_body_away_with_an_oof() {
    let w = floor();
    let mut pets = vec![at(&w, cfg(), 700.0, 1.0, walking(), 1), at(&w, cfg(), 700.0 + 2.0 * BODY, 1.0, resting(), 2)];
    let mut s = only(RompKind::Shove, 6);
    s.set_shove_back(0.0);
    step(&mut s, &mut pets, &w);
    assert!(pets.iter().all(Pet::in_romp));
    let before = pets[1].body.x;
    let seen = play_out(&mut s, &mut pets, &w, 10.0);
    assert!(seen.ended.is_some());
    let slid = pets[1].body.x - before;
    assert!(slid > 0.6 * BODY && slid < 1.5 * BODY, "몸 1개쯤 밀림: {slid}");
    assert_eq!(seen.said[0], vec![Speech::Hey]);
    assert_eq!(seen.said[1], vec![Speech::Oof]);
    assert!(seen.anims[1].contains(&Anim::Fall), "{:?}", seen.anims[1]);
    assert!(freed(&pets));
}

#[test]
fn shoves_go_back_and_forth_at_most_three_times() {
    let w = floor();
    let mut pets = vec![at(&w, cfg(), 600.0, 1.0, resting(), 1), at(&w, cfg(), 600.0 + 2.0 * BODY, 1.0, resting(), 2)];
    let mut s = only(RompKind::Shove, 7);
    s.set_shove_back(1.0);
    step(&mut s, &mut pets, &w);
    let seen = play_out(&mut s, &mut pets, &w, 20.0);
    assert!(seen.ended.is_some());
    let all: Vec<Speech> = seen.said.concat();
    let oofs = all.iter().filter(|&&sp| sp == Speech::Oof).count();
    assert_eq!(oofs, 3, "세 번 주고받음: {:?}", seen.said);
    assert!(all.contains(&Speech::Hey) && all.contains(&Speech::HeyBack), "{all:?}");
    assert!(freed(&pets));
}

// ---- 😴 같이 낮잠 ----

#[test]
fn a_friend_snuggles_up_to_a_sleeper_naps_too_and_wakes_soon_after_it() {
    let w = floor();
    let mut pets = vec![at(&w, cfg(), 600.0, 1.0, resting(), 1), at(&w, cfg(), 600.0 + 3.0 * BODY, -1.0, sleeping(), 2)];
    let mut s = only(RompKind::Nap, 8);
    step(&mut s, &mut pets, &w);
    assert!(pets[0].in_romp(), "다가감");
    let mut heart = false;
    let mut asleep_at = None;
    for k in 0..(10.0 / DT) as usize {
        step(&mut s, &mut pets, &w);
        heart |= pets[0].render().heart;
        if matches!(pets[0].state, State::Sleep { .. }) {
            asleep_at = Some(k);
            break;
        }
    }
    assert!(asleep_at.is_some(), "같이 잠");
    assert!(heart, "하트 한 번");
    let gap = (pets[1].body.x - pets[0].body.x).abs();
    assert!(gap < 1.0 * BODY && gap > 0.4 * BODY, "옆에 붙어서: {gap}");
    // 한참 같이 잠
    for _ in 0..(5.0 / DT) as usize {
        step(&mut s, &mut pets, &w);
        pets[1].state = sleeping();
    }
    assert!(matches!(pets[0].state, State::Sleep { .. }));
    // 먼저 자던 친구가 깸 → 몇 초 뒤 같이 깸 (기지개)
    pets[1].state = State::Trick { kind: crate::behavior::Trick::Stretch, remaining: 2.5 };
    let mut woke = None;
    for k in 0..(5.0 / DT) as usize {
        step(&mut s, &mut pets, &w);
        if !matches!(pets[0].state, State::Sleep { .. }) {
            woke = Some(k as f64 * DT);
            break;
        }
    }
    let woke = woke.expect("같이 깸");
    assert!((0.9..=3.2).contains(&woke), "몇 초 뒤: {woke}");
    assert_eq!(pets[0].render().anim, Anim::Stretch);
    assert_eq!(s.romps_active(), 0);
}

#[test]
fn naps_are_for_friendly_pets_mostly() {
    let w = floor();
    let aloof = PetConfig { personality: crate::behavior::Personality::Aloof, ..cfg() };
    let friendly = cfg();
    let napper = |c: PetConfig, seed: u64| {
        let pets = vec![at(&w, c, 600.0, 1.0, resting(), seed), at(&w, cfg(), 600.0 + 3.0 * BODY, -1.0, sleeping(), seed + 100)];
        let s = Social::new(seed);
        s.romp_weight(&pets, RompKind::Nap, 0, 1)
    };
    assert!(napper(friendly, 1) > 3.0 * napper(aloof, 1));
}

// ---- 🏁 달리기 시합 ----

#[test]
fn a_race_lines_up_says_ready_go_and_ends_with_a_winner_and_a_sore_loser() {
    let w = floor();
    let mut pets = vec![at(&w, cfg(), 500.0, 1.0, resting(), 1), at(&w, cfg(), 500.0 + 2.0 * BODY, -1.0, resting(), 2)];
    let mut s = only(RompKind::Race, 9);
    step(&mut s, &mut pets, &w);
    assert!(pets.iter().all(Pet::in_romp));
    let seen = play_out(&mut s, &mut pets, &w, 25.0);
    assert!(seen.ended.is_some());
    let all: Vec<Speech> = seen.said.concat();
    let ready = all.iter().position(|&sp| sp == Speech::Ready).expect("ready…");
    let go = all.iter().position(|&sp| sp == Speech::Go).expect("go!");
    let _ = (ready, go);
    for i in 0..2 {
        assert!(seen.anims[i].contains(&Anim::Run), "{:?}", seen.anims[i]);
    }
    let winner = (0..2).find(|&i| seen.said[i].contains(&Speech::IWin)).expect("I win!");
    let loser = 1 - winner;
    assert!(seen.sparkle[winner] && !seen.sparkle[loser]);
    assert!(seen.anims[winner].contains(&Anim::Wave));
    assert!(seen.said[loser].contains(&Speech::NoFair));
    assert!(seen.anims[loser].contains(&Anim::Annoyed));
    // 같은 쪽(먼 끝)으로 달려감: 처음 자리보다 둘 다 오른쪽으로 멀리
    assert!(pets.iter().all(|p| p.body.x > 500.0 + 4.0 * BODY), "{} {}", pets[0].body.x, pets[1].body.x);
    assert!(freed(&pets));
}

#[test]
fn no_race_without_room_to_run() {
    // 좁은 창 위: 몸 3개도 못 달림
    let w = World::new(vec![crate::geom::WindowInfo { id: 1, rect: Rect::new(800.0, 600.0, 420.0, 300.0) }], vec![Rect::new(0.0, 0.0, 1920.0, GROUND)], 128.0);
    let owner = Owner::Window(1);
    let (ax, ay) = w.anchor(owner).unwrap();
    let put = |x: f64, seed: u64| {
        let mut p = Pet::new(cfg(), x, ay, seed);
        p.body.x = x;
        p.standing = Some(Standing { owner, rel_x: x - ax });
        p.state = resting();
        p.update(0.0, &w);
        p
    };
    let mut pets = vec![put(900.0, 1), put(1100.0, 2)];
    let mut s = only(RompKind::Race, 10);
    for _ in 0..30 {
        step(&mut s, &mut pets, &w);
    }
    assert_eq!(s.romps_started(), 0);
}

// ---- 💬 수다 ----

#[test]
fn a_chat_trades_two_to_four_lines_face_to_face() {
    for seed in 0..6 {
        let w = floor();
        let mut pets = vec![at(&w, cfg(), 800.0, -1.0, resting(), seed), at(&w, cfg(), 800.0 + 1.5 * BODY, 1.0, resting(), seed + 50)];
        let mut s = only(RompKind::Chat, seed);
        step(&mut s, &mut pets, &w);
        assert!(pets.iter().all(Pet::in_romp));
        let mut order = Vec::new();
        let mut faced = false;
        for _ in 0..(15.0 / DT) as usize {
            step(&mut s, &mut pets, &w);
            faced |= pets[0].facing > 0.0 && pets[1].facing < 0.0;
            for (i, p) in pets.iter_mut().enumerate() {
                if let Some(sp) = p.take_speech() {
                    order.push((i, sp));
                }
            }
            if s.romps_active() == 0 {
                break;
            }
        }
        assert_eq!(s.romps_active(), 0);
        assert!(faced, "서로 봄");
        assert!((2..=4).contains(&order.len()), "{order:?}");
        for (k, &(i, sp)) in order.iter().enumerate() {
            assert_eq!(i, k % 2, "번갈아: {order:?}");
            assert_eq!(sp, if i == 0 { Speech::ChatA } else { Speech::ChatB });
        }
        assert!(freed(&pets));
    }
}

// ---- 누가 할 수 있나 ----

#[test]
fn flyers_busy_and_controlled_pets_do_not_romp() {
    let w = floor();
    for kind in [RompKind::Scuffle, RompKind::Shove, RompKind::Race, RompKind::Chat, RompKind::Prank] {
        // 나는 동물
        let mut pets = vec![at(&w, crate::test_util::flyer(), 800.0, 1.0, resting(), 1), at(&w, cfg(), 800.0 + 2.0 * BODY, -1.0, resting(), 2)];
        let mut s = only(kind, 1);
        for _ in 0..10 {
            step(&mut s, &mut pets, &w);
        }
        assert_eq!(s.romps_started(), 0, "{kind:?} flyer");
        // 조작 중
        let mut pets = vec![at(&w, cfg(), 800.0, 1.0, resting(), 3), at(&w, cfg(), 800.0 + 2.0 * BODY, -1.0, resting(), 4)];
        pets[0].set_control(true);
        let mut s = only(kind, 2);
        for _ in 0..10 {
            step(&mut s, &mut pets, &w);
        }
        assert_eq!(s.romps_started(), 0, "{kind:?} controlled");
        // 너무 멂
        let mut pets = vec![at(&w, cfg(), 300.0, 1.0, resting(), 5), at(&w, cfg(), 300.0 + 7.0 * BODY, -1.0, resting(), 6)];
        let mut s = only(kind, 3);
        for _ in 0..10 {
            step(&mut s, &mut pets, &w);
        }
        assert_eq!(s.romps_started(), 0, "{kind:?} far");
    }
}

#[test]
fn grabbing_one_pet_mid_romp_frees_the_other() {
    let w = floor();
    for kind in [RompKind::Scuffle, RompKind::Shove, RompKind::Race, RompKind::Chat] {
        let mut pets = vec![at(&w, cfg(), 800.0, 1.0, resting(), 1), at(&w, cfg(), 800.0 + 2.0 * BODY, -1.0, resting(), 2)];
        let mut s = only(kind, 11);
        step(&mut s, &mut pets, &w);
        for _ in 0..20 {
            step(&mut s, &mut pets, &w);
        }
        assert!(pets[1].in_romp(), "{kind:?}");
        let p = &mut pets[0];
        p.pointer_down(p.body.x, p.body.y - 60.0);
        step(&mut s, &mut pets, &w);
        step(&mut s, &mut pets, &w);
        assert!(!pets[1].in_romp(), "{kind:?}");
        assert_eq!(s.romps_active(), 0);
        p_release(&mut pets[0]);
    }
}

/// 누른 손을 뗌 (클릭)
fn p_release(p: &mut Pet) {
    let (x, y) = (p.body.x, p.body.y - 60.0);
    p.pointer_up(x, y);
}

#[test]
fn a_pet_left_alone_in_a_romp_wanders_off_by_itself() {
    // 코디네이터가 사라져도 (짝이 화면을 떠나 다른 Social 등) 몸짓이 끊기면 스스로 그만둠
    let w = floor();
    let mut pets = vec![at(&w, cfg(), 800.0, 1.0, resting(), 1), at(&w, cfg(), 800.0 + 2.0 * BODY, -1.0, resting(), 2)];
    let mut s = only(RompKind::Chat, 12);
    step(&mut s, &mut pets, &w);
    assert!(pets[0].in_romp());
    for _ in 0..60 {
        pets[0].update(DT, &w);
    }
    assert!(!pets[0].in_romp());
}

#[test]
fn every_romp_bias_is_neutral_for_now() {
    let p = Pet::new(cfg(), 0.0, 0.0, 1);
    for kind in RompKind::ALL {
        assert_eq!(p.social_bias(kind), 1.0);
    }
}

// ---- 10분 같이 두기 ----

/// 번들 pet.json 그대로 (청소년)
fn species(id: &str) -> PetConfig {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../ui/pets").join(id).join("pet.json");
    let json = std::fs::read_to_string(&path).unwrap();
    let m = crate::manifest::parse_manifest(&json).unwrap();
    PetConfig::from_manifest(&m, 1.0, m.size_mult(crate::care::Stage::Teenager), crate::care::Stage::Teenager.name())
}

/// 두 마리를 바닥에 두고 secs초 앱처럼 돌림: 새 놀이를 몇 번 시작했나
fn ten_minutes(a: &str, b: &str, seed: u64) -> usize {
    let w = floor();
    let (ax, ay) = w.anchor(Owner::Ground(0)).unwrap();
    let put = |c: PetConfig, x: f64, seed: u64| {
        let mut p = Pet::new(c, x, ay, seed);
        p.standing = Some(Standing { owner: Owner::Ground(0), rel_x: x - ax });
        p.state = State::Idle { remaining: 0.5, look: false };
        p.set_exit_edges(false, false);
        p
    };
    let mut pets = vec![put(species(a), 700.0, seed), put(species(b), 1100.0, seed + 31)];
    let mut s = Social::new(seed);
    for _ in 0..(600.0 / DT) as usize {
        step(&mut s, &mut pets, &w);
    }
    s.romps_started()
}

#[test]
fn two_pets_at_home_show_about_three_to_six_new_plays_in_ten_minutes() {
    let pairs = [("cat", "rabbit"), ("duck", "frog"), ("persian", "squirrel"), ("cat", "penguin"), ("rabbit", "duck"), ("frog", "cat")];
    let counts: Vec<usize> = pairs.iter().enumerate().map(|(k, &(a, b))| ten_minutes(a, b, 100 + k as u64)).collect();
    let mean = counts.iter().sum::<usize>() as f64 / counts.len() as f64;
    assert!((3.0..=6.0).contains(&mean), "{counts:?} mean {mean}");
    assert!(counts.iter().all(|&n| n <= 10), "{counts:?}");
}

#[test]
fn greet_tag_and_high_five_happen_more_often_than_before() {
    assert_eq!(GREET_COOLDOWN, (30.0, 50.0));
    assert_eq!(GREET_PLAY_CHANCE, 0.6);
    assert_eq!(PLAY_RATE, 0.05);
    assert_eq!(PLAY_COOLDOWN, (45.0, 90.0));
    assert_eq!(crate::play::HIGH_FIVE_CHANCE, 0.35);
    assert_eq!(crate::play::HIGH_FIVE_COOLDOWN, (60.0, 150.0));
}
