//! 📊 익명 펫 통계: 하루 보고 받기 (누구나, 연결당 날마다 한 번), 날짜별 합계 주기 (편지함 독자만).
//!
//! 통계는 자기 잠금(`Hub::stats`)을 씀: 편지함처럼 잎 잠금이라 잡은 채 다른 잠금을 잡지 않음.

use super::*;
use screenpet_proto::pet_stats::StatsReport;

impl Hub {
    /// 통계 잠금을 잡음. 잎 잠금이라 잡은 채 다른 잠금을 잡지 말 것
    pub(super) fn stats(&self) -> MutexGuard<'_, crate::pet_stats::PetStats> {
        self.stats.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// 통계가 바뀌었으면 `stats.json`을 씀 (실패면 `save_fail`을 적고 다음에 다시). 걸린 밀리초를 돌려줌
    fn save_stats(&self) -> u128 {
        let started = Instant::now();
        let _order = self.stats_save.lock().unwrap_or_else(|e| e.into_inner());
        let snap = self.stats().snapshot();
        if let (Some(bytes), Some(dir)) = (snap, self.data_dir.as_deref()) {
            if let Err(e) = crate::pet_stats::PetStats::save(dir, &bytes) {
                log!(Error, "stats", "save_fail", "took_ms" => took_ms(started), "err" => e);
                self.stats().mark_dirty();
            }
        }
        took_ms(started)
    }

    /// `key`의 지난날 보고를 받음. 셌든 안 셌든 `stats_ok`로 답함 (다시 보내도 소용없음).
    /// `stats_in`(날짜·종류 수·took_ms)이나 `stats_refused`(까닭)를 적음. 종류 이름은 안 적음
    pub(crate) fn stats_report(&self, key: &str, conn: u64, ip: &str, report: &StatsReport) -> Option<ServerMsg> {
        let result = {
            let st = self.state();
            if !Self::is_current(&st, key, conn) {
                return None;
            }
            let seen_days = st.store_ref().get(key).map_or(0, |i| i.seen_days);
            self.stats().submit(key, ip, seen_days, report, unix_now())
        };
        match result {
            Ok(()) => {
                let took = self.save_stats();
                log!(Info, "stats", "stats_in", "conn" => Cid(conn), "day" => report.day.as_str(), "kept" => report.kept.len(), "took_ms" => took);
            }
            Err(r) => log!(Warn, "stats", "stats_refused", "conn" => Cid(conn), "reason" => r.as_str()),
        }
        // 날짜는 앱이 보낸 것을 그대로 돌려줌 (앱이 자기 보고와 맞춰 봄 — 길이만 자름)
        Some(ServerMsg::StatsOk { day: report.day.chars().take(screenpet_proto::pet_stats::MAX_TEXT).collect() })
    }

    /// 날짜별 합계를 줌 (편지함 독자만, 아니면 None임)
    pub(crate) fn stats_days(&self, key: &str, conn: u64) -> Option<ServerMsg> {
        let st = self.state();
        if !(Self::is_current(&st, key, conn) && Self::is_reader(&st, key)) {
            return None;
        }
        let days = self.stats().days(unix_now());
        log!(Info, "stats", "stats_list", "conn" => Cid(conn), "peer" => short(key), "n" => days.len());
        Some(ServerMsg::StatsDays { days })
    }
}
