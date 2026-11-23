//! 📊 익명 펫 통계: 하루 보고 받기 (누구나, 연결당 날마다 한 번), 날짜별 합계를 쪽으로 주기 (편지함 독자만),
//! 몇 분마다 한꺼번에 `stats.json` 쓰기 (보고마다 쓰지 않음 — 두 시점의 파일로 보고 하나를 떼어 볼 수 없게, 자정 무렵 디스크 몰림도 없게).
//!
//! 통계는 자기 잠금(`Hub::stats`)을 씀: 편지함처럼 잎 잠금이라 잡은 채 다른 잠금을 잡지 않음.

use super::*;
use screenpet_proto::pet_stats::StatsReport;

impl Hub {
    /// 통계 잠금을 잡음. 잎 잠금이라 잡은 채 다른 잠금을 잡지 말 것
    pub(super) fn stats(&self) -> MutexGuard<'_, crate::pet_stats::PetStats> {
        self.stats.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// 오래된 날을 지우고, 바뀌었으면 `stats.json`을 씀 (업킵 스레드가 `stats_save_every`마다, 그리고 끝날 때).
    /// 실패면 `save_fail`을 적고 다음에 다시 씀
    pub(crate) fn save_stats(&self) {
        let started = Instant::now();
        let _order = self.stats_save.lock().unwrap_or_else(|e| e.into_inner());
        let snap = {
            let mut stats = self.stats();
            stats.prune(unix_now());
            stats.snapshot()
        };
        if let (Some(bytes), Some(dir)) = (snap, self.data_dir.as_deref()) {
            if let Err(e) = crate::pet_stats::PetStats::save(dir, &bytes) {
                log!(Error, "stats", "save_fail", "took_ms" => took_ms(started), "err" => e);
                self.stats().mark_dirty();
            }
        }
    }

    /// `key`의 지난날 보고를 받음 (저장은 나중에 한꺼번에). 셌거나 다시 보내도 소용없으면 `stats_ok`로 답하고,
    /// 지금만 안 되는 것(주소·하루 한도)은 답하지 않음 — 앱이 다음 연결에 다시 보냄.
    /// `stats_in`(날짜만)이나 `stats_refused`(까닭)를 적음. 종류·수는 안 적음
    pub(crate) fn stats_report(&self, key: &str, conn: u64, ip: &str, report: &StatsReport) -> Option<ServerMsg> {
        let result = {
            let st = self.state();
            if !Self::is_current(&st, key, conn) {
                return None;
            }
            self.stats().submit(key, ip, report, unix_now())
        };
        match result {
            Ok(()) => log!(Info, "stats", "stats_in", "day" => report.day.as_str()),
            Err(r) => {
                log!(Warn, "stats", "stats_refused", "conn" => Cid(conn), "reason" => r.as_str());
                if !r.is_final() {
                    return None;
                }
            }
        }
        // 날짜는 앱이 보낸 것을 그대로 돌려줌 (앱이 자기 보고와 맞춰 봄 — 길이만 자름)
        Some(ServerMsg::StatsOk { day: report.day.chars().take(screenpet_proto::pet_stats::MAX_TEXT).collect() })
    }

    /// `before`보다 앞선 날들의 합계 한 쪽을 줌 (편지함 독자만, 아니면 None임)
    pub(crate) fn stats_days(&self, key: &str, conn: u64, before: Option<&str>) -> Option<ServerMsg> {
        let st = self.state();
        if !(Self::is_current(&st, key, conn) && Self::is_reader(&st, key)) {
            return None;
        }
        let (days, more) = self.stats().days_page(unix_now(), before);
        log!(Info, "stats", "stats_list", "conn" => Cid(conn), "peer" => short(key), "n" => days.len());
        Some(ServerMsg::StatsDays { days, more })
    }
}
