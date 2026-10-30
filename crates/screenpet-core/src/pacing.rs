//! 게임 루프 박자: "일한 뒤 16 ms 자기"는 일한 시간만큼 매 틱 늦어지고 들쭉날쭉하다.
//! 대신 정해진 시각(지난 마감 + 한 주기)까지 잔다 — 늦은 만큼 다음 잠을 줄여 평균 주기를 지킨다.
//! 한 주기 넘게 밀렸으면 (절전·디버거 등) 몰아서 따라잡지 않고 지금부터 다시 센다.

use std::time::{Duration, Instant};

pub struct Pacer {
    period: Duration,
    next: Instant,
}

impl Pacer {
    pub fn new(period: Duration, now: Instant) -> Self {
        Self { period, next: now + period }
    }

    /// 이번 틱 일을 마친 시각 now → 다음 틱까지 잘 시간 (늦었으면 0)
    pub fn wait(&mut self, now: Instant) -> Duration {
        let wait = self.next.saturating_duration_since(now);
        self.next += self.period;
        if self.next < now {
            self.next = now + self.period;
        }
        wait
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const P: Duration = Duration::from_millis(16);

    fn ms(n: u64) -> Duration {
        Duration::from_millis(n)
    }

    #[test]
    fn sleeps_until_the_deadline_not_a_fixed_period() {
        let t0 = Instant::now();
        let mut p = Pacer::new(P, t0);
        // 일에 3 ms 걸렸으면 13 ms만 잔다
        assert_eq!(p.wait(t0 + ms(3)), ms(13));
        // 다음 마감은 32 ms (잠이 1 ms 늦게 깼어도 그 늦음을 다음 잠에서 뺀다)
        assert_eq!(p.wait(t0 + ms(17) + ms(2)), ms(13));
        assert_eq!(p.wait(t0 + ms(48)), ms(0));
    }

    #[test]
    fn a_little_late_catches_up_but_far_behind_resyncs() {
        let t0 = Instant::now();
        let mut p = Pacer::new(P, t0);
        // 마감(16)보다 5 ms 늦음 → 바로 다음 틱, 다음 마감은 32
        assert_eq!(p.wait(t0 + ms(21)), Duration::ZERO);
        assert_eq!(p.wait(t0 + ms(22)), ms(10));
        // 마감(48)보다 한참 늦음 → 바로, 그다음은 지금 + 16 (몰아 돌지 않는다)
        assert_eq!(p.wait(t0 + ms(500)), Duration::ZERO);
        assert_eq!(p.wait(t0 + ms(501)), ms(15));
    }
}
