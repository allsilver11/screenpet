//! 🛡️🕷️ 무쇠인간과 거미인간 (social_mentor.rs): 같은 바닥에 둘 다 쉬고 있으면 가끔 —
//! 🤗 응원 포옹 (무쇠인간이 다가가 꼭 안고 "You got this, kid!", 거미인간은 하트 뿅 "aw, shucks") 또는
//! 🗨️ 졸졸 수다 (거미인간이 무쇠인간을 졸졸 따라다니며 학교·사건·여자친구·영웅의 무게 이야기 — 무쇠인간은 들어 주거나 휙 날아 도망, 그래도 따라감).
//! 한 번에 하나만, 끝나면 쿨다운. 둘 중 누가 잡히면 깔끔히 끝남

use super::campfire_tests::{idle_at, quiet, step};
use super::*;
use crate::brain::{Anim, Pet, Speech};
use crate::geom::Rect;
use crate::test_util::DT;
use crate::world::World;

const GROUND: f64 = 1040.0;

fn floor() -> World {
    World::new(vec![], vec![Rect::new(0.0, 0.0, 1920.0, GROUND)], 128.0)
}

/// 쉬는 무쇠인간 (mx)과 거미인간 (sx)
fn duo(w: &World, mx: f64, sx: f64) -> Vec<Pet> {
    vec![idle_at(w, "musoe", mx, 1), idle_at(w, "spider", sx, 2)]
}

/// 포옹만 바로 시작되는 코디네이터
fn hugs() -> Social {
    let mut s = quiet(5);
    s.set_mentor_rates(50.0, 0.0);
    s
}

/// 졸졸 수다만 바로 시작되는 코디네이터 (flee = 무쇠인간이 지루해 도망갈 몫)
fn tags(flee: f64) -> Social {
    let mut s = quiet(6);
    s.set_mentor_rates(0.0, 50.0);
    s.set_mentor_flee(flee);
    s
}

/// 한 틱 돌리고 들은 말을 모음
fn tick(s: &mut Social, pets: &mut [Pet], w: &World, said: &mut Vec<(usize, Speech)>) {
    step(s, pets, w);
    for (i, p) in pets.iter_mut().enumerate() {
        if let Some(sp) = p.take_speech() {
            said.push((i, sp));
        }
    }
}

#[test]
fn mentor_lines_have_their_sheet_names() {
    for (s, name) in [(Speech::Cheer, "cheer"), (Speech::Shucks, "shucks"), (Speech::KidChat, "kid_chat"), (Speech::UhHuh, "uh_huh"), (Speech::NotNow, "not_now")] {
        assert_eq!(serde_json::to_string(&s).unwrap(), format!("\"{name}\""));
    }
}

/// 🤗 무쇠인간이 걸어가 꼭 안고 (hug 그림, 거미인간 쪽을 봄) 응원 한마디 → 거미인간은 하트 (heart 그림) "aw, shucks" → 풀고 각자. 곧바로 또 하지 않음
#[test]
fn musoe_walks_over_hugs_spider_tight_and_cheers_him_on() {
    let w = floor();
    let mut pets = duo(&w, 500.0, 1100.0);
    let mut s = hugs();
    let (mut said, mut hugged, mut closest) = (Vec::new(), false, f64::MAX);
    for _ in 0..(15.0 / DT) as usize {
        tick(&mut s, &mut pets, &w, &mut said);
        closest = closest.min((pets[0].body.x - pets[1].body.x).abs());
        if pets[0].render().anim == Anim::Hug {
            hugged = true;
            assert_eq!(pets[1].render().anim, Anim::Heart, "거미인간은 수줍게 하트");
            let toward = (pets[1].body.x - pets[0].body.x).signum();
            assert_eq!(pets[0].render().flip, toward < 0.0, "거미인간 쪽을 보고 안음");
        }
    }
    assert_eq!(s.mentor_started(), (1, 0));
    assert!(hugged, "안음");
    assert!(closest < 0.6 * pets[0].cfg.size, "꼭 붙음 {closest}");
    assert!((pets[1].body.x - 1100.0).abs() < 1.0, "거미인간은 제자리에서 기다림");
    assert!(said.contains(&(0, Speech::Cheer)), "{said:?}");
    assert!(said.contains(&(1, Speech::Shucks)), "{said:?}");
    assert!(!pets[0].in_romp() && !pets[1].in_romp(), "끝남");
    let mut again = Vec::new();
    for _ in 0..(60.0 / DT) as usize {
        tick(&mut s, &mut pets, &w, &mut again);
    }
    assert_eq!(s.mentor_started(), (1, 0), "쿨다운");
}

/// 둘 다 있고 둘 다 쉴 때만: 거미인간 없이·다른 동물과·걷는 중·멀리 떨어진 다른 디딤판이면 시작 안 함
#[test]
fn it_starts_only_with_both_present_and_resting_nearby() {
    let w = floor();
    let mut s = hugs();
    s.set_mentor_rates(50.0, 50.0);
    let mut alone = vec![idle_at(&w, "musoe", 500.0, 1), idle_at(&w, "cat", 700.0, 2)];
    let mut said = Vec::new();
    for _ in 0..(5.0 / DT) as usize {
        tick(&mut s, &mut alone, &w, &mut said);
    }
    assert_eq!(s.mentor_started(), (0, 0), "고양이와는 안 함");
    let mut two_spiders = vec![idle_at(&w, "spider", 500.0, 1), idle_at(&w, "super", 700.0, 2)];
    for _ in 0..(5.0 / DT) as usize {
        tick(&mut s, &mut two_spiders, &w, &mut said);
    }
    assert_eq!(s.mentor_started(), (0, 0), "슈퍼인간은 무쇠인간이 아님");
    // 걷는 중이면 안 함 (쉴 때만)
    let mut walking = duo(&w, 500.0, 800.0);
    for p in walking.iter_mut() {
        p.state = crate::brain::State::Move { kind: crate::behavior::Move::Walk, remaining: 1e6 };
    }
    s.tick(DT, &mut walking);
    assert_eq!(s.mentor_started(), (0, 0), "걷는 중");
    // 너무 멀면 안 함
    let mut far = duo(&w, 100.0, 1850.0);
    for _ in 0..(3.0 / DT) as usize {
        tick(&mut s, &mut far, &w, &mut said);
    }
    assert_eq!(s.mentor_started(), (0, 0), "멀리");
}

/// 🗨️ 졸졸 수다: 거미인간이 무쇠인간을 따라가 옆에서 재잘재잘 (말이 여러 번) — 무쇠인간이 들어 주면 서서 거미인간 쪽을 보고 "Uh-huh."
#[test]
fn spider_tags_along_chattering_and_musoe_listens() {
    let w = floor();
    let mut pets = duo(&w, 900.0, 300.0);
    let mut s = tags(0.0);
    let (mut said, mut near) = (Vec::new(), 0usize);
    for _ in 0..(12.0 / DT) as usize {
        tick(&mut s, &mut pets, &w, &mut said);
        if pets[1].in_romp() && (pets[0].body.x - pets[1].body.x).abs() < 1.3 * pets[1].cfg.size {
            near += 1;
        }
    }
    assert_eq!(s.mentor_started(), (0, 1));
    assert!(pets[0].in_romp() && pets[1].in_romp(), "아직 수다 중");
    assert!((pets[0].body.x - 900.0).abs() < 1.0, "들어 주는 무쇠인간은 제자리");
    assert!(near as f64 * DT > 6.0, "거미인간이 따라와 곁에 붙음 {near}");
    let chats = said.iter().filter(|l| **l == (1, Speech::KidChat)).count();
    assert!(chats >= 3, "재잘재잘 {said:?}");
    assert!(said.contains(&(0, Speech::UhHuh)), "{said:?}");
    assert!(!said.iter().any(|l| l.1 == Speech::NotNow));
}

/// 🗨️ 무쇠인간이 지루해 "Not now, kid." 하고 날아서 달아나도 거미인간은 계속 쫓아가며 말함. 한참 (TAG_SECS) 뒤 끝나고 쿨다운 — 포옹과 겹치지 않음
#[test]
fn musoe_flies_off_bored_and_spider_keeps_following_until_it_ends() {
    let w = floor();
    let mut pets = duo(&w, 900.0, 700.0);
    let mut s = tags(1.0);
    let (mut said, mut flew, mut followed) = (Vec::new(), false, 0.0f64);
    let start = pets[0].body.x;
    let mut ended_at = None;
    for k in 0..(40.0 / DT) as usize {
        let before = pets[1].body.x;
        tick(&mut s, &mut pets, &w, &mut said);
        if pets[0].in_romp() && pets[0].render().anim == Anim::Fly {
            flew = true;
        }
        if pets[1].in_romp() {
            // 거미인간은 무쇠인간 쪽으로 움직임
            let toward = (pets[0].body.x - before).signum();
            followed += (pets[1].body.x - before) * toward;
        }
        if ended_at.is_none() && !pets[0].in_romp() && !pets[1].in_romp() && k > 10 {
            ended_at = Some(k as f64 * DT);
        }
    }
    assert!(flew, "날아서 달아남");
    assert!((pets[0].body.x - start).abs() > 100.0 || said.iter().filter(|l| l.1 == Speech::NotNow).count() >= 2);
    assert!(said.contains(&(0, Speech::NotNow)), "{said:?}");
    assert!(followed > 300.0, "그래도 쫓아감 {followed}");
    assert!(said.iter().filter(|l| **l == (1, Speech::KidChat)).count() >= 3);
    let t = ended_at.expect("끝남");
    assert!((15.0..=27.0).contains(&t), "한참 하다 끝남 {t}");
    assert_eq!(s.mentor_started(), (0, 1), "쿨다운 — 포옹도 안 겹침");
}

/// 잡으면 깔끔히 끝남: 수다 중 무쇠인간을 잡으면 거미인간도 풀려남, 포옹 중 거미인간을 잡아도 무쇠인간이 풀려남
#[test]
fn grabbing_either_pet_ends_it_cleanly() {
    let w = floor();
    let mut pets = duo(&w, 900.0, 600.0);
    let mut s = tags(0.0);
    let mut said = Vec::new();
    for _ in 0..(2.0 / DT) as usize {
        tick(&mut s, &mut pets, &w, &mut said);
    }
    assert!(pets[0].in_romp() && pets[1].in_romp());
    let (x, y) = (pets[0].body.x, pets[0].body.y - 40.0);
    pets[0].pointer_down(x, y);
    for _ in 0..3 {
        s.tick(DT, &mut pets);
        pets[1].update(DT, &w);
    }
    assert!(!pets[1].in_romp(), "거미인간도 풀려남");
    let mut pets = duo(&w, 500.0, 900.0);
    let mut s = hugs();
    let mut hugging = false;
    for _ in 0..(12.0 / DT) as usize {
        tick(&mut s, &mut pets, &w, &mut said);
        if pets[0].render().anim == Anim::Hug {
            hugging = true;
            break;
        }
    }
    assert!(hugging);
    let (x, y) = (pets[1].body.x, pets[1].body.y - 40.0);
    pets[1].pointer_down(x, y);
    for _ in 0..3 {
        s.tick(DT, &mut pets);
        pets[0].update(DT, &w);
    }
    assert!(!pets[0].in_romp(), "무쇠인간도 풀려남");
}
