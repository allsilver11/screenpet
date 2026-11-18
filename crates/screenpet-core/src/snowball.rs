//! ⛄ 문질러서 눈덩이 뭉치기 (v0.6.5) — 순수 계산. 앱(game_loop)이 매 틱 커서와 "커서 바로 아래 보이는 눈 더미가 있음"을 넘김.
//!
//! 날씨 창은 마우스가 통과하므로 클릭은 쓰지 않음: 쌓인 눈 바로 위에서 커서를 좌우로 빠르게 문지르면 (TURNS번 되돌아서기가
//! WINDOW_SECS 안에, 한 획마다 MIN_STROKE~MAX_STROKE를 오감) 그 자리에서 눈덩이가 뭉쳐짐. 평소처럼 움직이는 커서는 거의 걸리지 않음:
//! 짧게 떨리는 것(TURN_SLOP 안), 길게 휘젓는 것(MAX_STROKE 넘게), 느린 왕복(WINDOW_SECS 넘게)은 세지 않음.
//! 눈덩이는 물건(object.rs ObjectKind::Snowball)이고 화면에 MAX_SNOWBALLS개까지, MELT_SECS 뒤 스르륵 녹음.

/// 뭉치려면 되돌아서는 횟수
pub const TURNS: usize = 3;
/// 그 되돌아서기들이 이 시간 안에 있어야 함 (초)
pub const WINDOW_SECS: f64 = 1.5;
/// 한 획은 적어도 이만큼 가야 함 (논리 px)
pub const MIN_STROKE: f64 = 24.0;
/// 한 획이 이보다 길면 문지르기가 아니라 휘젓기임 (논리 px)
pub const MAX_STROKE: f64 = 320.0;
/// 반대쪽으로 이만큼 돌아와야 되돌아선 것으로 봄 (논리 px — 손 떨림은 세지 않음)
pub const TURN_SLOP: f64 = 6.0;
/// 한 번 뭉친 뒤 이만큼은 다시 뭉치지 않음 (초)
pub const COOLDOWN_SECS: f64 = 1.5;
/// 눈 더미 윗면에서 커서가 위아래로 이만큼 안이어야 함 (논리 px)
pub const REACH_Y: f64 = 28.0;
/// 문지른 폭 바깥으로 이만큼까지 눈을 모음 (논리 px)
pub const REACH_X: f64 = 24.0;
/// 눈이 이만큼(칸 높이의 합, 논리 px)은 있어야 뭉쳐짐
pub const PACK_MIN_SNOW: f32 = 4.0;
/// 화면에 있을 수 있는 눈덩이 수 — 넘치면 가장 오래된 것이 뿅
pub const MAX_SNOWBALLS: usize = 5;
/// 눈덩이가 저절로 녹아 사라지기까지 (초)
pub const MELT_SECS: f64 = 180.0;

/// 커서 문지르기를 알아채는 것 (앱이 하나 가짐)
#[derive(Debug, Clone, Default)]
pub struct Rub {
    clock: f64,
    /// 지난 틱의 커서 x (눈 위가 아니었으면 None)
    last_x: Option<f64>,
    /// 지금 획의 쪽 (±1, 0 = 아직 모름)
    dir: f64,
    /// 지금 획이 시작한 x와 그 쪽으로 가장 멀리 간 x
    start_x: f64,
    far_x: f64,
    /// 되돌아선 (시각, x)
    turns: Vec<(f64, f64)>,
    cooldown_until: f64,
}

impl Rub {
    /// 새로 만듦
    pub fn new() -> Self {
        Self::default()
    }

    /// 매 틱: cursor = 커서 (물리 px), over_snow = 커서 바로 아래 보이는 눈 더미가 있음 (REACH_Y 안).
    /// 뭉쳐졌으면 문지른 폭 (왼쪽, 오른쪽 — 물리 px)
    pub fn tick(&mut self, dt: f64, cursor: (f64, f64), dpi: f64, over_snow: bool) -> Option<(f64, f64)> {
        self.clock += dt.max(0.0);
        let x = cursor.0;
        if !over_snow || self.clock < self.cooldown_until || !x.is_finite() {
            self.reset();
            return None;
        }
        let dpi = dpi.max(0.01);
        let Some(prev) = self.last_x.replace(x) else {
            self.start_x = x;
            self.far_x = x;
            self.dir = 0.0;
            return None;
        };
        let dx = x - prev;
        if self.dir == 0.0 {
            if dx.abs() > 0.0 {
                self.dir = dx.signum();
                self.far_x = x;
            }
        } else if (x - self.far_x) * self.dir > 0.0 {
            self.far_x = x;
        } else if (self.far_x - x) * self.dir >= TURN_SLOP * dpi {
            self.turn(x, dpi);
        }
        let clock = self.clock;
        self.turns.retain(|&(t, _)| clock - t <= WINDOW_SECS);
        if self.turns.len() < TURNS {
            return None;
        }
        let (lo, hi) = self.turns.iter().fold((x, x), |(lo, hi), &(_, tx)| (lo.min(tx), hi.max(tx)));
        self.reset();
        self.cooldown_until = self.clock + COOLDOWN_SECS;
        Some((lo, hi))
    }

    /// far_x에서 되돌아섰음: 획 길이가 알맞으면 되돌아서기로 셈, 아니면 처음부터 다시 셈
    fn turn(&mut self, x: f64, dpi: f64) {
        let stroke = (self.far_x - self.start_x).abs();
        if (MIN_STROKE * dpi..=MAX_STROKE * dpi).contains(&stroke) {
            self.turns.push((self.clock, self.far_x));
        } else {
            self.turns.clear();
        }
        self.start_x = self.far_x;
        self.far_x = x;
        self.dir = -self.dir;
    }

    /// 세던 것을 잊음 (쿨다운은 그대로)
    fn reset(&mut self) {
        self.last_x = None;
        self.dir = 0.0;
        self.turns.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const DT: f64 = 1.0 / 60.0;

    /// 테스트: xs를 차례로 지나며 (한 점에 steps 틱씩 고르게) 뭉쳐진 폭을 모음
    fn rub(r: &mut Rub, xs: &[f64], secs_per_leg: f64, over: bool, dpi: f64) -> Vec<(f64, f64)> {
        let mut got = Vec::new();
        let steps = (secs_per_leg / DT).round().max(1.0) as usize;
        let mut at = xs[0];
        if let Some(s) = r.tick(DT, (at, 500.0), dpi, over) {
            got.push(s);
        }
        for &to in &xs[1..] {
            for k in 1..=steps {
                let x = at + (to - at) * k as f64 / steps as f64;
                if let Some(s) = r.tick(DT, (x, 500.0), dpi, over) {
                    got.push(s);
                }
            }
            at = to;
        }
        got
    }

    #[test]
    fn quick_rubbing_over_snow_packs_a_snowball_where_it_was_rubbed() {
        let mut r = Rub::new();
        let got = rub(&mut r, &[400.0, 460.0, 400.0, 460.0, 400.0, 460.0], 0.2, true, 1.0);
        assert_eq!(got.len(), 1, "{got:?}");
        let (lo, hi) = got[0];
        assert!((399.0..=401.0).contains(&lo) && (459.0..=461.0).contains(&hi), "{got:?}");
    }

    #[test]
    fn nothing_happens_away_from_snow() {
        let mut r = Rub::new();
        assert!(rub(&mut r, &[400.0, 460.0, 400.0, 460.0, 400.0, 460.0], 0.2, false, 1.0).is_empty());
    }

    #[test]
    fn ordinary_movement_does_not_pack() {
        // 한쪽으로 쭉
        let mut r = Rub::new();
        assert!(rub(&mut r, &[0.0, 1800.0], 1.0, true, 1.0).is_empty());
        // 느린 왕복 (되돌아서기 사이가 길다)
        let mut r = Rub::new();
        assert!(rub(&mut r, &[400.0, 460.0, 400.0, 460.0, 400.0, 460.0], 0.8, true, 1.0).is_empty());
        // 손 떨림
        let mut r = Rub::new();
        assert!(rub(&mut r, &[400.0, 404.0, 400.0, 404.0, 400.0, 404.0, 400.0, 404.0], 0.05, true, 1.0).is_empty());
        // 길게 휘젓기
        let mut r = Rub::new();
        assert!(rub(&mut r, &[100.0, 900.0, 100.0, 900.0, 100.0, 900.0], 0.2, true, 1.0).is_empty());
        // 짧은 획 둘에 긴 획 하나가 끼면 처음부터 다시 셈
        let mut r = Rub::new();
        assert!(rub(&mut r, &[400.0, 460.0, 400.0, 1000.0, 940.0], 0.2, true, 1.0).is_empty());
    }

    #[test]
    fn leaving_the_snow_mid_rub_starts_over() {
        let mut r = Rub::new();
        assert!(rub(&mut r, &[400.0, 460.0, 400.0], 0.2, true, 1.0).is_empty());
        assert!(rub(&mut r, &[400.0, 410.0], 0.05, false, 1.0).is_empty());
        assert!(rub(&mut r, &[400.0, 460.0, 400.0], 0.2, true, 1.0).is_empty(), "only two turns since coming back");
    }

    #[test]
    fn one_snowball_at_a_time_then_a_short_cooldown() {
        let mut r = Rub::new();
        let legs = [400.0, 460.0, 400.0, 460.0, 400.0, 460.0, 400.0, 460.0, 400.0];
        assert_eq!(rub(&mut r, &legs, 0.12, true, 1.0).len(), 1, "keeps rubbing inside the cooldown");
        r.tick(COOLDOWN_SECS, (400.0, 500.0), 1.0, true);
        assert_eq!(rub(&mut r, &legs[..7], 0.12, true, 1.0).len(), 1);
    }

    #[test]
    fn strokes_are_measured_in_logical_pixels() {
        // 2배 화면: 40 물리 px는 20 논리 px라 너무 짧다
        let mut r = Rub::new();
        assert!(rub(&mut r, &[400.0, 440.0, 400.0, 440.0, 400.0, 440.0], 0.2, true, 2.0).is_empty());
        let mut r = Rub::new();
        assert_eq!(rub(&mut r, &[400.0, 500.0, 400.0, 500.0, 400.0, 500.0], 0.2, true, 2.0).len(), 1);
    }
}
