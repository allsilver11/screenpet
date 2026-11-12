//! ⌨️ 타자 응원 (흐물이 v0.6.3)

use super::swing_tests::{heumul, resting};
use super::tests::world;
use super::typing_moves::{
    AFTER_SECS, BUDGET_SECS, BUDGET_WINDOW_SECS, MAX_TAPS_PER_SEC, SPEEDY_KPS, SPEEDY_LINE, TAP_SECS, TYPING_HOLD_SECS, TYPING_LINES, YAY_EVERY_SECS,
    YAY_SECS,
};
use super::*;
use crate::test_util::{pet_cfg, DT};

fn run(p: &mut Pet, w: &World, secs: f64, rate: f64) -> Vec<Speech> {
    let mut said = Vec::new();
    let mut t = 0.0;
    while t < secs {
        p.set_typing_rate(rate);
        p.update(DT, w);
        said.extend(p.take_speech());
        t += DT;
    }
    said
}

fn cheering(p: &Pet) -> bool {
    matches!(p.state, State::TypeCheer { .. })
}

#[test]
fn fast_typing_for_two_seconds_makes_it_face_the_screen_and_cheer() {
    let w = world(vec![]);
    let mut p = resting(heumul(), 800.0, 1, &w);
    run(&mut p, &w, TYPING_HOLD_SECS - 0.2, 6.0);
    assert!(!cheering(&p), "2초 전엔 아직");
    let (said, _) = run_text(&mut p, &w, 0.3, 6.0);
    assert!(cheering(&p));
    assert_eq!(p.render().anim, Anim::Type, "키보드 앞에 앉아 친다");
    assert!(Anim::Type.is_front() && Anim::CheerType.is_front(), "앞(화면)을 본다");
    assert_eq!(said, vec![TYPING_LINES[0].to_string()]);
    // 빠른 타자가 이어지는 동안 계속, 타자 대사는 가끔 또
    let (said, _) = run_text(&mut p, &w, 12.0, 6.0);
    assert!(cheering(&p) && said.len() >= 2, "{said:?}");
    // 느려지면 2초 뒤 끝
    run(&mut p, &w, AFTER_SECS - 0.2, 0.5);
    assert!(cheering(&p));
    run(&mut p, &w, 0.4, 0.5);
    assert!(!cheering(&p));
}

#[test]
fn slow_typing_or_short_bursts_or_other_pets_do_nothing() {
    let w = world(vec![]);
    let mut p = resting(heumul(), 800.0, 2, &w);
    run(&mut p, &w, 10.0, 3.0);
    assert!(!cheering(&p), "초당 4타 아래");
    for _ in 0..5 {
        run(&mut p, &w, 1.5, 8.0);
        run(&mut p, &w, 0.5, 0.0);
    }
    assert!(!cheering(&p), "2초를 못 채운 타자");
    let mut cat = resting(pet_cfg(), 800.0, 3, &w);
    run(&mut cat, &w, 5.0, 9.0);
    assert!(!cheering(&cat));
}

#[test]
fn at_most_twenty_seconds_every_three_minutes() {
    let w = world(vec![]);
    let mut p = resting(heumul(), 800.0, 4, &w);
    let mut cheered = 0.0;
    let mut t = 0.0;
    while t < BUDGET_WINDOW_SECS - 5.0 {
        p.set_typing_rate(7.0);
        p.update(DT, &w);
        p.take_speech();
        if cheering(&p) {
            cheered += DT;
        }
        t += DT;
    }
    assert!(cheered <= BUDGET_SECS + 0.1 && cheered >= BUDGET_SECS - 0.5, "{cheered}");
    // 다음 3분 창에선 다시
    run(&mut p, &w, 10.0, 7.0);
    assert!(cheering(&p));
}

#[test]
fn not_while_away_focused_held_or_sleeping() {
    let w = world(vec![]);
    for case in 0..3 {
        let mut p = resting(heumul(), 800.0, 5, &w);
        match case {
            0 => p.set_focus(true),
            1 => p.state = State::Sleep { remaining: 100.0 },
            _ => p.pointer_down(p.body.x, p.body.y - 40.0),
        }
        run(&mut p, &w, 5.0, 8.0);
        assert!(!cheering(&p), "case {case}");
    }
    // 응원 중에 자리 비움이면 그만
    let mut p = resting(heumul(), 800.0, 6, &w);
    run(&mut p, &w, 3.0, 8.0);
    assert!(cheering(&p));
    p.set_user_away(true);
    run(&mut p, &w, 0.2, 8.0);
    assert!(!cheering(&p));
}

/// 매 틱 (초당 키 수, 이번 틱에 새로 눌린 수)를 주고 (틱마다의 모습, 말)을 모은다
fn run_keys(p: &mut Pet, w: &World, secs: f64, rate: f64, mut keys: impl FnMut(usize) -> u32) -> (Vec<RenderState>, Vec<String>) {
    let (mut shown, mut said) = (Vec::new(), Vec::new());
    let mut k = 0;
    while (k as f64) * DT < secs {
        p.set_typing_rate(rate);
        p.typing_keys(keys(k));
        p.update(DT, w);
        p.take_speech();
        said.extend(p.take_speech_text());
        shown.push(p.render());
        k += 1;
    }
    (shown, said)
}

fn run_text(p: &mut Pet, w: &World, secs: f64, rate: f64) -> (Vec<String>, Vec<RenderState>) {
    let (shown, said) = run_keys(p, w, secs, rate, |_| 0);
    (said, shown)
}

/// 응원을 막 시작한 흐물이 (시작 대사는 꺼냈다)
fn cheering_heumul(seed: u64, w: &World) -> Pet {
    let mut p = resting(heumul(), 800.0, seed, w);
    run(&mut p, w, TYPING_HOLD_SECS + 0.1, 6.0);
    assert!(cheering(&p));
    p.take_speech_text();
    p
}

/// 새로 시작한 톡 (손이 바뀌거나 쉬다가 내려간 틱)
fn tap_starts(shown: &[RenderState]) -> Vec<i8> {
    let mut out = Vec::new();
    let mut last = 0;
    for r in shown {
        if r.tap != 0 && r.tap != last {
            out.push(r.tap);
        }
        last = r.tap;
    }
    out
}

#[test]
fn each_key_press_makes_one_short_tap_with_alternating_hands() {
    let w = world(vec![]);
    let mut p = cheering_heumul(11, &w);
    // 키를 안 누르면 두 손은 키보드 위에서 쉰다
    let (shown, _) = run_keys(&mut p, &w, 0.5, 6.0, |_| 0);
    assert!(shown.iter().all(|r| r.anim == Anim::Type && r.tap == 0));
    // 0.25초마다 한 번씩 눌렀다: 누를 때마다 톡 — 왼손, 오른손, 왼손… 톡은 짧고 (TAP_SECS) 그사이엔 쉰다
    let (shown, _) = run_keys(&mut p, &w, 2.0, 6.0, |k| u32::from(k % 15 == 0));
    let starts = tap_starts(&shown);
    assert_eq!(starts.len(), 8, "{starts:?}");
    assert!(starts.windows(2).all(|h| h[0] == -h[1]), "손을 번갈아: {starts:?}");
    let down = shown.iter().filter(|r| r.tap != 0).count() as f64 * DT;
    assert!((down - 8.0 * TAP_SECS).abs() < 8.0 * DT * 1.5, "{down}");
    assert!((0.08..=0.12).contains(&TAP_SECS));
    // 손을 떼면 바로 멈춘다
    let (shown, _) = run_keys(&mut p, &w, 0.3, 6.0, |_| 0);
    assert!(shown[(TAP_SECS / DT) as usize + 1..].iter().all(|r| r.tap == 0));
}

#[test]
fn a_flood_of_keys_is_capped_to_what_the_frames_can_show() {
    let w = world(vec![]);
    let mut p = cheering_heumul(12, &w);
    // 틱마다 12타씩 (말도 안 되게 빠름) 3초: 톡은 초당 MAX_TAPS_PER_SEC를 넘지 않고, 밀린 톡이 손을 뗀 뒤까지 이어지지 않는다
    let (shown, _) = run_keys(&mut p, &w, 3.0, 25.0, |_| 12);
    let starts = tap_starts(&shown);
    assert!(MAX_TAPS_PER_SEC <= 12.0);
    assert!(starts.len() as f64 <= 3.0 * MAX_TAPS_PER_SEC + 1.0 && starts.len() >= 20, "{}", starts.len());
    let (shown, _) = run_keys(&mut p, &w, 0.5, 25.0, |_| 0);
    assert!(tap_starts(&shown).len() <= 2, "밀린 톡은 한두 번이면 끝");
    assert!(shown.last().is_some_and(|r| r.tap == 0));
}

#[test]
fn keys_before_the_cheer_do_nothing_and_are_not_saved_up() {
    let w = world(vec![]);
    let mut p = resting(heumul(), 800.0, 13, &w);
    // 느린 타자 (응원 아님): 키를 눌러도 키보드도 톡도 없다
    let (shown, _) = run_keys(&mut p, &w, 3.0, 2.0, |_| 3);
    assert!(!cheering(&p));
    assert!(shown.iter().all(|r| r.anim != Anim::Type && r.tap == 0));
    // 그동안 누른 키가 응원을 시작할 때 한꺼번에 쏟아지지 않는다
    run(&mut p, &w, TYPING_HOLD_SECS + 0.05, 6.0);
    assert!(cheering(&p));
    let (shown, _) = run_keys(&mut p, &w, 1.0, 6.0, |_| 0);
    assert!(tap_starts(&shown).len() <= 1);
    // 응원이 끝나면 키보드도 없다
    run(&mut p, &w, AFTER_SECS + 0.5, 0.0);
    assert!(!cheering(&p) && p.render().anim != Anim::Type && p.render().tap == 0);
}

#[test]
fn typing_is_the_main_loop_with_a_short_yay_now_and_then() {
    let w = world(vec![]);
    let mut p = cheering_heumul(14, &w);
    let (shown, _) = run_keys(&mut p, &w, 14.0, 6.0, |k| u32::from(k % 8 == 0));
    let yay = shown.iter().filter(|r| r.anim == Anim::CheerType).count() as f64 * DT;
    let typing = shown.iter().filter(|r| r.anim == Anim::Type).count() as f64 * DT;
    assert_eq!(yay + typing, shown.len() as f64 * DT, "응원 중엔 둘 중 하나");
    let bursts = shown.windows(2).filter(|r| r[0].anim == Anim::Type && r[1].anim == Anim::CheerType).count();
    assert_eq!(bursts, (14.0 / YAY_EVERY_SECS) as usize, "{bursts}");
    assert!((yay - bursts as f64 * YAY_SECS).abs() < 3.0 * DT, "{yay}");
    assert!(typing > yay * 5.0);
    // 만세하는 동안엔 톡이 없다
    assert!(shown.iter().filter(|r| r.anim == Anim::CheerType).all(|r| r.tap == 0));
}

#[test]
fn typing_lines_take_turns_now_and_then_and_speedy_typing_gets_its_own() {
    let w = world(vec![]);
    let mut p = resting(heumul(), 800.0, 15, &w);
    run(&mut p, &w, TYPING_HOLD_SECS - 0.1, 6.0);
    let mut said: Vec<(f64, String)> = Vec::new();
    let mut t = 0.0;
    while t < 17.0 {
        p.set_typing_rate(6.0);
        p.update(DT, &w);
        if let Some(s) = p.take_speech_text() {
            said.push((t, s));
        }
        t += DT;
    }
    let lines: Vec<&str> = said.iter().map(|(_, s)| s.as_str()).collect();
    assert!(lines.len() >= 3, "{lines:?}");
    for (i, l) in lines.iter().enumerate() {
        assert_eq!(*l, TYPING_LINES[i % TYPING_LINES.len()], "{lines:?}");
    }
    for gap in said.windows(2).map(|s| s[1].0 - s[0].0) {
        assert!((4.0 - DT..=6.0 + DT).contains(&gap), "가끔만: {gap}");
    }
    assert_eq!(TYPING_LINES, ["Type type!", "So fast!", "Keep going! ⌨️", "Tap tap tap!"]);
    // 아주 빠르면 (초당 8타 이상) 🔥
    let mut p = resting(heumul(), 800.0, 16, &w);
    let (_, said) = run_keys(&mut p, &w, TYPING_HOLD_SECS + 6.5, SPEEDY_KPS, |_| 0);
    assert!(said.len() >= 2 && said.iter().all(|s| s == SPEEDY_LINE), "{said:?}");
    assert_eq!(SPEEDY_LINE, "🔥 Speedy!");
    // 옛 응원 말 ("Cheer up!")은 이제 없다
    assert!(said.iter().all(|s| !s.contains("Cheer")));
}

/// 개인정보: 뇌로 넘어오는 것은 개수뿐 — 키 코드·글자를 받을 자리가 없다
#[test]
fn only_counts_cross_into_the_brain() {
    let src = include_str!("brain_typing.rs");
    assert!(src.contains("pub fn typing_keys(&mut self, presses: u32)"));
    assert!(src.contains("pub fn set_typing_rate(&mut self, keys_per_sec: f64)"));
    let mood = &src[src.index_of("pub struct TypingMood")..];
    let mood = &mood[..mood.find("\n}").unwrap()];
    for banned in ["char", "String", "Vec", "[u8", "&str", "key_code", "vk"] {
        assert!(!mood.contains(banned), "TypingMood mentions {banned}");
    }
    let api: Vec<&str> = src.lines().filter(|l| l.trim_start().starts_with("pub fn")).collect();
    for l in &api {
        assert!(!l.contains("char") && !l.contains("&str") && !l.contains("String") && !l.contains("u8"), "{l}");
    }
}

trait IndexOf {
    fn index_of(&self, pat: &str) -> usize;
}

impl IndexOf for str {
    fn index_of(&self, pat: &str) -> usize {
        self.find(pat).unwrap_or_else(|| panic!("no {pat}"))
    }
}
