//! 사다리타기 — 그림 쪽 순수 로직 (spec `2026-11-04-screenpet-ladder-design.md` §3.1).
//!
//! 결과는 사다리 모양이 아니라 **균등 무작위 순열**에서 나옴: 가로줄을 무작위로 긋는 사다리는 가까운 칸으로 갈 확률이 높아
//! 공정하지 않음. 그래서 먼저 순열 π(위 칸 i → 결과 칸 π(i))를 뽑고, 홀짝 전치 정렬로 그 순열이 되는 가로줄을 만든 뒤
//! 결과를 바꾸지 않는 장식 가로줄 쌍을 섞음
//!
//! 난수는 `FnMut() -> u32`로 받음 — core는 해시를 모름. 참가자끼리 같은 결과를 보려면 같은 수열에서 같은 결과가 나와야 하므로
//! 여기 알고리즘은 **판을 넘어 고정**임 (테스트의 고정 벡터가 지킴). `fastrand`를 쓰지 않음

use serde::{Deserialize, Serialize};

/// 한 판의 인원 (방장 포함). net `screenpet_net::ladder`와 같은 값이어야 함 (net `tests/ladder.rs`가 확인함)
pub const MIN_PLAYERS: usize = 2;
/// 한 판의 최대 인원
pub const MAX_PLAYERS: usize = 10;
/// 사다리 최소 높이 (줄 수)
pub const MIN_ROWS: usize = 8;
/// 결과 칸 글자 수 상한
pub const RESULT_MAX_CHARS: usize = 12;
/// 장식 가로줄 쌍을 놓아 보는 횟수 = 세로줄 수 × 이 값
const DECOY_TRIES_PER_COLUMN: usize = 2;
/// 칸 너비 (논리 px) — 기본과 하한
const COLUMN_WIDTH: f64 = 72.0;
/// 칸 너비 하한 (논리 px) — 이보다 좁으면 아기 펫·이름표가 겹침
const MIN_COLUMN_WIDTH: f64 = 48.0;
/// 창이 이보다 좁으면 칸을 줄임 — 적어도 이만큼 칸이 있는 폭으로 잡음
const MIN_COLUMNS_WIDE: usize = 4;
/// 창 좌우 여백 합 (논리 px) — 양 끝 칸의 이름표(두 칸 폭까지 씀)가 창 밖으로 나가지 않게 (tools/test_ladder_ui.py가 이 값으로 잼)
pub const WINDOW_MARGIN: f64 = 60.0;

/// 창 안쪽 높이를 정할 때 작업 영역에서 뺄 몫 (제목 표시줄·테두리, 논리 px)
const WINDOW_CHROME: f64 = 40.0;
/// 아무리 작은 화면이어도 창 안쪽 높이는 이만큼 (판이 그려질 자리)
const MIN_WINDOW_HEIGHT: f64 = 300.0;

/// 결과 칸의 기분 — 펫이 만세(좋음)·시무룩(나쁨)·그냥 앉기(없음)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Mood {
    Good,
    Bad,
    #[default]
    #[serde(rename = "none")]
    Neutral,
}

impl Mood {
    /// 친구가 보낸 값: 모르는 값은 없음
    pub fn from_wire(raw: &str) -> Mood {
        match raw {
            "good" => Mood::Good,
            "bad" => Mood::Bad,
            _ => Mood::Neutral,
        }
    }

    /// 링크로 보내는 글자 ("good" / "bad" / "none")
    pub fn to_wire(self) -> &'static str {
        match self {
            Mood::Good => "good",
            Mood::Bad => "bad",
            Mood::Neutral => "none",
        }
    }
}

/// `rows[r]`에 든 `c` = 그 줄에서 세로줄 `c`와 `c+1`을 잇는 가로줄 (오름차순, 한 줄 안에서 서로 이웃하지 않음)
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Ladder {
    /// 세로줄 수 (= 인원)
    pub columns: usize,
    /// 줄마다 가로줄이 잇는 세로줄 (`rows[r]`의 `c` = `c`와 `c+1`을 이음)
    pub rows: Vec<Vec<usize>>,
}

/// `[0, bound)` 균등 정수. 거부 샘플링이라 나머지 연산 치우침이 없음 (`bound`는 1 이상)
pub fn uniform(next: &mut impl FnMut() -> u32, bound: u32) -> u32 {
    assert!(bound > 0, "bound must be positive");
    // 2^32를 bound로 나눈 나머지만큼 맨 아래 값을 버린다 (u32 범위에서 계산: (2^32 - bound) % bound)
    let threshold = bound.wrapping_neg() % bound;
    loop {
        let x = next();
        if x >= threshold {
            return x % bound;
        }
    }
}

/// Fisher–Yates. `perm[i]` = 위 칸 `i`가 닿는 결과 칸
pub fn permutation(n: usize, next: &mut impl FnMut() -> u32) -> Vec<usize> {
    let mut perm: Vec<usize> = (0..n).collect();
    for i in (1..n).rev() {
        let j = uniform(next, (i + 1) as u32) as usize;
        perm.swap(i, j);
    }
    perm
}

/// `perm`이 되는 사다리. 홀짝 전치 정렬의 교환이 진짜 가로줄이고, 빈 줄과 장식 쌍(같은 자리에 연달아 두 줄 — 서로 상쇄)으로
/// `MIN_ROWS` 이상 채움. 장식 위치도 `next`에서만 뽑음
pub fn build(perm: &[usize], next: &mut impl FnMut() -> u32) -> Ladder {
    let n = perm.len();
    let mut rows: Vec<Vec<usize>> = Vec::new();
    // 세로줄 c에 선 사람이 가야 할 결과 칸. 다 정렬되면 모두 제자리에 닿는다
    let mut want = perm.to_vec();
    for round in 0..n {
        let mut row = Vec::new();
        let mut c = round % 2;
        while c + 1 < n {
            if want[c] > want[c + 1] {
                want.swap(c, c + 1);
                row.push(c);
            }
            c += 2;
        }
        if !row.is_empty() {
            rows.push(row);
        }
    }
    // 빈 줄은 어디 끼워도 결과가 같다
    while rows.len() < MIN_ROWS {
        let at = uniform(next, rows.len() as u32 + 1) as usize;
        rows.insert(at, Vec::new());
    }
    // 장식 쌍: 줄 r과 r+1 모두 c-1·c·c+1 자리가 비어 있을 때만 (그래야 다른 가로줄과 따로 놀아 서로 상쇄된다)
    if n >= 2 {
        for _ in 0..n * DECOY_TRIES_PER_COLUMN {
            let r = uniform(next, rows.len() as u32 - 1) as usize;
            let c = uniform(next, n as u32 - 1) as usize;
            if free_around(&rows[r], c) && free_around(&rows[r + 1], c) {
                insert_sorted(&mut rows[r], c);
                insert_sorted(&mut rows[r + 1], c);
            }
        }
    }
    Ladder { columns: n, rows }
}

/// 이 줄에서 c 자리에 가로줄을 놓아도 이웃과 붙지 않음
fn free_around(row: &[usize], c: usize) -> bool {
    !row.iter().any(|&x| x + 1 >= c && x <= c + 1)
}

/// 정렬을 지키며 c를 넣음
fn insert_sorted(row: &mut Vec<usize>, c: usize) {
    let at = row.partition_point(|&x| x < c);
    row.insert(at, c);
}

/// 위 칸 `start`에서 내려간 길: 맨 위와 각 줄을 지난 뒤의 세로줄 (길이 = 줄 수 + 1, 마지막이 결과 칸)
pub fn trace(ladder: &Ladder, start: usize) -> Vec<usize> {
    let mut col = start;
    let mut path = Vec::with_capacity(ladder.rows.len() + 1);
    path.push(col);
    for row in &ladder.rows {
        if row.contains(&col) {
            col += 1;
        } else if col > 0 && row.contains(&(col - 1)) {
            col -= 1;
        }
        path.push(col);
    }
    path
}

/// 결과 칸 글자 정리: 앞뒤 공백을 떼고, 비었거나 제어 문자(줄바꿈·탭 포함)가 있거나 12자를 넘으면 None
pub fn clean_result(raw: &str) -> Option<String> {
    let s = raw.trim();
    if s.is_empty() || s.chars().any(char::is_control) || s.chars().count() > RESULT_MAX_CHARS {
        return None;
    }
    Some(s.to_string())
}

/// 같은 표시 이름이 또 나오면 뒤에 " ②", " ③" … (처음 것은 그대로)
pub fn disambiguate(names: &[String]) -> Vec<String> {
    let mut out = Vec::with_capacity(names.len());
    for (i, name) in names.iter().enumerate() {
        let nth = names[..i].iter().filter(|n| *n == name).count() + 1;
        if nth == 1 {
            out.push(name.clone());
        } else {
            out.push(format!("{name} {}", circled(nth)));
        }
    }
    out
}

/// ② … ⑳, 그 너머는 (21) 같은 괄호 숫자
fn circled(n: usize) -> String {
    if (1..=20).contains(&n) {
        char::from_u32(0x2460 + n as u32 - 1).map(String::from).unwrap_or_default()
    } else {
        format!("({n})")
    }
}

/// 칸 너비 (논리 px): 기본 72, 작업 영역 폭에 다 안 들어가면 줄임 (하한 48). 칸이 적어도 4칸 폭은 잡음
pub fn column_width(players: usize, work_width: f64) -> f64 {
    let cols = players.max(MIN_COLUMNS_WIDE) as f64;
    let fit = ((work_width - WINDOW_MARGIN) / cols).floor();
    COLUMN_WIDTH.min(fit).max(MIN_COLUMN_WIDTH)
}

/// 창 안쪽 높이 (논리 px): 원하는 높이가 작업 영역에 안 들어가면 줄임 (하한 `MIN_WINDOW_HEIGHT`)
pub fn window_height(wanted: f64, work_height: f64) -> f64 {
    wanted.min((work_height - WINDOW_CHROME).max(MIN_WINDOW_HEIGHT))
}

/// 한 축에서 창이 작업 영역 안에 들어가는 위치. 들어가면 그대로, 영역보다 크면 시작 쪽에 맞춤 (제목 표시줄이 보이게)
pub fn keep_on_screen(pos: f64, len: f64, area_start: f64, area_len: f64) -> f64 {
    pos.min(area_start + area_len - len).max(area_start)
}

/// 친구가 보낸 글자를 화면용으로: 보이지 않는 글자(방향 바꾸기 등)를 뺌. 남는 글자가 없으면 "?".
/// 표시에만 씀 — 지문과 시드는 받은 글자 그대로 해시함
pub fn shown_text(raw: &str, max: usize) -> String {
    let s = crate::config::clean_text(raw, max);
    if s.is_empty() {
        "?".to_string()
    } else {
        s
    }
}

/// 공정성 지문 24비트 → SAS 이모지 번호 4개 (앞에서부터 6비트씩)
pub fn print_indices(bits24: u32) -> [usize; 4] {
    [18, 12, 6, 0].map(|shift| ((bits24 >> shift) & 63) as usize)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 테스트용 결정적 수열 (splitmix32 비슷한 섞기 — 고정 벡터는 아래 따로)
    fn seq(seed: u64) -> impl FnMut() -> u32 {
        let mut s = seed.wrapping_mul(0x9E37_79B9_7F4A_7C15).wrapping_add(1);
        move || {
            s = s.wrapping_add(0x9E37_79B9_7F4A_7C15);
            let mut z = s;
            z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
            z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
            (z ^ (z >> 31)) as u32
        }
    }

    /// 테스트: 시드로 n명의 순열과 사다리를 만듦
    fn ladder_for(n: usize, seed: u64) -> (Vec<usize>, Ladder) {
        let mut next = seq(seed);
        let perm = permutation(n, &mut next);
        let ladder = build(&perm, &mut next);
        (perm, ladder)
    }

    #[test]
    fn every_ladder_draws_its_permutation() {
        for n in MIN_PLAYERS..=MAX_PLAYERS {
            for seed in 0..2000 {
                let (perm, ladder) = ladder_for(n, seed);
                assert_eq!(ladder.columns, n);
                assert!(ladder.rows.len() >= MIN_ROWS, "n {n} seed {seed}");
                for row in &ladder.rows {
                    assert!(row.windows(2).all(|w| w[0] + 1 < w[1]), "adjacent or unsorted rungs {row:?}");
                    assert!(row.iter().all(|&c| c + 1 < n));
                }
                for (start, &want) in perm.iter().enumerate() {
                    let path = trace(&ladder, start);
                    assert_eq!(path.len(), ladder.rows.len() + 1);
                    assert_eq!(*path.last().unwrap(), want, "n {n} seed {seed} start {start}");
                }
            }
        }
    }

    #[test]
    fn decoys_make_ladders_busier_than_the_sort_alone() {
        let rungs: usize = (0..200).map(|s| ladder_for(5, s).1.rows.iter().map(Vec::len).sum::<usize>()).sum();
        // 5명 정렬만이면 평균 교환 수는 n(n-1)/4 = 5. 장식이 더해져야 한다
        assert!(rungs as f64 / 200.0 > 6.0, "{rungs}");
    }

    #[test]
    fn three_player_permutations_are_uniform() {
        let runs = 60_000;
        let mut counts = std::collections::HashMap::new();
        for seed in 0..runs {
            *counts.entry(ladder_for(3, seed).0).or_insert(0usize) += 1;
        }
        assert_eq!(counts.len(), 6);
        let expected = runs as f64 / 6.0;
        let chi2: f64 = counts.values().map(|&c| (c as f64 - expected).powi(2) / expected).sum();
        // 자유도 5, 유의수준 0.001
        assert!(chi2 < 20.52, "chi2 {chi2} {counts:?}");
    }

    #[test]
    fn ten_player_first_column_lands_uniformly() {
        let runs = 20_000;
        let mut counts = [0usize; 10];
        for seed in 0..runs {
            let (_, ladder) = ladder_for(10, seed);
            counts[*trace(&ladder, 0).last().unwrap()] += 1;
        }
        let expected = runs as f64 / 10.0;
        let chi2: f64 = counts.iter().map(|&c| (c as f64 - expected).powi(2) / expected).sum();
        // 자유도 9, 유의수준 0.001
        assert!(chi2 < 27.88, "chi2 {chi2} {counts:?}");
    }

    #[test]
    fn uniform_rejects_the_biased_bottom() {
        // bound 3: 2^32 % 3 = 1 → 0 하나만 버린다
        let mut vals = vec![0u32, 1, u32::MAX].into_iter();
        let mut next = move || vals.next().unwrap();
        assert_eq!(uniform(&mut next, 3), 1);
        assert_eq!(uniform(&mut next, 3), u32::MAX % 3);
        // 2의 거듭제곱은 아무것도 버리지 않는다
        let mut zero = || 0u32;
        assert_eq!(uniform(&mut zero, 4), 0);
        assert_eq!(uniform(&mut zero, 1), 0);
        let mut next = seq(7);
        assert!((0..10_000).all(|_| uniform(&mut next, 7) < 7));
    }

    /// 판을 넘어 같은 수열에서 같은 사다리가 나와야 참가자끼리 결과가 같음. 이 값이 바뀌면 프로토콜이 깨진 것임
    #[test]
    fn fixed_vector() {
        let mut i = 0u32;
        let mut next = move || {
            i = i.wrapping_add(1);
            i.wrapping_mul(2_654_435_761)
        };
        let perm = permutation(5, &mut next);
        let ladder = build(&perm, &mut next);
        assert_eq!(perm, FIXED_PERM);
        assert_eq!(ladder.rows, FIXED_ROWS.iter().map(|r| r.to_vec()).collect::<Vec<_>>());
    }

    const FIXED_PERM: [usize; 5] = [4, 0, 3, 2, 1];
    const FIXED_ROWS: [&[usize]; 8] = [&[], &[], &[0, 2], &[1, 3], &[0], &[0, 2], &[], &[1, 3]];

    #[test]
    fn results_are_cleaned() {
        assert_eq!(clean_result("  당첨  ").as_deref(), Some("당첨"));
        assert_eq!(clean_result(""), None);
        assert_eq!(clean_result("   "), None);
        assert_eq!(clean_result("커피\t쏘기"), None);
        assert_eq!(clean_result("줄\n바꿈"), None);
        assert_eq!(clean_result("열두글자열두글자열두글자").as_deref(), Some("열두글자열두글자열두글자"));
        assert_eq!(clean_result("열세글자열세글자열세글자!"), None);
        assert_eq!(clean_result("🍕 피자").as_deref(), Some("🍕 피자"));
    }

    #[test]
    fn same_names_get_numbers() {
        let names: Vec<String> = ["나비", "나비", "곰", "나비"].iter().map(|s| s.to_string()).collect();
        assert_eq!(disambiguate(&names), ["나비", "나비 ②", "곰", "나비 ③"]);
        assert_eq!(circled(21), "(21)");
    }

    #[test]
    fn tall_windows_shrink_to_the_work_area() {
        assert_eq!(window_height(540.0, 1040.0), 540.0);
        // 1366×768 150% 배율: 작업 영역 약 480 논리 px → 제목 표시줄 몫을 빼고
        assert_eq!(window_height(540.0, 480.0), 440.0);
        assert_eq!(window_height(170.0, 480.0), 170.0, "small windows stay as they are");
        assert_eq!(window_height(540.0, 100.0), 300.0, "the board still needs some height");
    }

    #[test]
    fn a_grown_window_is_pulled_back_on_screen() {
        // 초대 창(250)이 있던 자리에서 게임 창(540)으로 커졌다 — 아래가 잘리면 위로 올린다
        assert_eq!(keep_on_screen(259.0, 540.0, 0.0, 728.0), 188.0);
        // 들어가면 사용자가 옮긴 자리 그대로
        assert_eq!(keep_on_screen(100.0, 540.0, 0.0, 728.0), 100.0);
        // 왼쪽·위로 나갔으면 안으로, 보조 모니터(음수 좌표)도 같다
        assert_eq!(keep_on_screen(-50.0, 300.0, 0.0, 1280.0), 0.0);
        assert_eq!(keep_on_screen(-1000.0, 300.0, -1920.0, 1920.0), -1000.0);
        assert_eq!(keep_on_screen(-100.0, 300.0, -1920.0, 1920.0), -300.0);
        // 작업 영역보다 크면 시작 부분(제목 표시줄과 닫기 단추)이 보이게
        assert_eq!(keep_on_screen(50.0, 900.0, 0.0, 728.0), 0.0);
    }

    #[test]
    fn hidden_characters_never_reach_the_screen() {
        assert_eq!(shown_text("\u{202E}Bob\u{200B}", 20), "Bob");
        assert_eq!(shown_text("나비", 20), "나비");
        assert_eq!(shown_text("\u{200B}\u{3164}", 20), "?", "nothing visible is left");
        // 보이지 않는 글자로만 다른 두 이름은 같은 이름이라 번호로 구분된다
        let names = [shown_text("나비", 20), shown_text("나\u{200B}비", 20)];
        assert_eq!(disambiguate(&names), ["나비", "나비 ②"]);
    }

    #[test]
    fn columns_shrink_to_fit_small_screens() {
        assert_eq!(column_width(3, 1920.0), 72.0);
        assert_eq!(column_width(10, 1920.0), 72.0);
        assert_eq!(column_width(10, 600.0), 54.0);
        assert_eq!(column_width(10, 300.0), 48.0);
    }

    #[test]
    fn print_bits_split_into_four_emoji() {
        assert_eq!(print_indices(0), [0, 0, 0, 0]);
        assert_eq!(print_indices(0xFF_FFFF), [63, 63, 63, 63]);
        assert_eq!(print_indices(0b000001_000010_000011_000100), [1, 2, 3, 4]);
    }

    #[test]
    fn moods_round_trip() {
        for m in [Mood::Good, Mood::Bad, Mood::Neutral] {
            assert_eq!(Mood::from_wire(m.to_wire()), m);
        }
        assert_eq!(Mood::from_wire("great"), Mood::Neutral);
        assert_eq!(serde_json::to_string(&Mood::Neutral).unwrap(), "\"none\"");
    }
}
