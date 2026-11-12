//! 관리 명령 (`admin ...`): 내보내기·차단·스위치·정리. 바꾸는 쪽은 admin 스레드 하나뿐임.

use super::*;

impl Hub {
    /// 스위치·차단 목록을 먼저 디스크에 쓰고 적용함 (저장 못 한 차단을 됐다고 하지 않게)
    ///
    /// 쓰기는 잠금 밖에서 함. 바꾸는 쪽은 admin 스레드 하나뿐임.
    pub(super) fn commit_control(&self, next: Control) -> Result<MutexGuard<'_, State>, String> {
        if let Some(dir) = &self.data_dir {
            next.save(dir).map_err(|e| format!("cannot save control.json, nothing changed: {e}"))?;
        }
        let mut st = self.state();
        self.relay_off.store(!next.relay, Ordering::SeqCst);
        st.control = next;
        Ok(st)
    }

    /// 현재 스위치·차단 목록의 사본
    pub(super) fn control(&self) -> Control {
        self.state().control.clone()
    }

    /// `admin kick`: 접속 중인 설치를 내보냄
    pub(crate) fn admin_kick(&self, who: &str) -> Result<String, String> {
        let mut st = self.state();
        let key = st.resolve(who)?;
        if st.drop_peer(&key, "kicked") {
            Ok(format!("kicked {key}"))
        } else {
            Err(format!("{key} is not online"))
        }
    }

    /// `admin ban`: 키를 막고 접속 중이면 끊음 (한 번도 안 온 키도 막을 수 있음)
    pub(crate) fn admin_ban(&self, who: &str) -> Result<String, String> {
        // A whole key can be banned before it has ever connected.
        let key = match proto::norm_key(who) {
            Some(k) => k,
            None => self.state().resolve(who)?,
        };
        let mut next = self.control();
        next.banned_keys.insert(key.clone());
        let mut st = self.commit_control(next)?;
        let was_online = st.drop_peer(&key, "banned");
        st.grants.retain(|g| g.from != key && g.to != key);
        Ok(format!("banned {key}{}", if was_online { ", disconnected" } else { "" }))
    }

    /// `admin ban-ip`: 주소 묶음을 막고 그 연결을 모두 끊음. 루프백·믿는 프록시는 거절함
    pub(crate) fn admin_ban_ip(&self, ip: &str) -> Result<String, String> {
        let addr: std::net::IpAddr = ip.parse().map_err(|_| format!("not an address: {ip}"))?;
        let addr = addr.to_canonical();
        let bucket = crate::http::bucket(addr);
        // Everyone arrives through these when a forwarded header goes missing.
        if addr.is_loopback() || self.trusted_proxies.iter().any(|p| crate::http::bucket(p.to_canonical()) == bucket) {
            return Err(format!("{ip} is loopback or a trusted proxy: banning it could refuse everyone"));
        }
        let mut next = self.control();
        next.banned_ips.insert(bucket.clone());
        let mut st = self.commit_control(next)?;
        let keys: Vec<String> = st.peers.iter().filter(|(_, p)| p.ip == bucket).map(|(k, _)| k.clone()).collect();
        for key in &keys {
            st.drop_peer(key, "banned");
        }
        for u in st.unauthed.values().filter(|u| u.ip == bucket) {
            let _ = u.kill.set("banned");
        }
        Ok(format!("banned {bucket}, {} disconnected", keys.len()))
    }

    /// `admin unban`: 주소나 키 차단을 풂 (하나만 맞아야 함)
    pub(crate) fn admin_unban(&self, who: &str) -> Result<String, String> {
        let mut next = self.control();
        let ip_key = who.parse::<std::net::IpAddr>().map(|a| crate::http::bucket(a.to_canonical())).unwrap_or_else(|_| who.to_string());
        if next.banned_ips.remove(&ip_key) {
            drop(self.commit_control(next)?);
            return Ok(format!("unbanned {ip_key}"));
        }
        let lower = who.to_ascii_lowercase();
        let matches: Vec<String> = {
            let st = self.state();
            let by_id = st.matching(who);
            let long = who.len() >= MIN_PREFIX;
            next.banned_keys.iter().filter(|k| (long && k.starts_with(&lower)) || by_id.contains(k)).cloned().collect()
        };
        match matches.as_slice() {
            [] => Err(format!("no ban matches {who:?}")),
            [key] => {
                next.banned_keys.remove(key);
                drop(self.commit_control(next)?);
                Ok(format!("unbanned {key}"))
            }
            many => Err(format!("{who:?} matches {} bans, nothing done:\n{}", many.len(), self.state().describe(many))),
        }
    }

    /// `admin bans`: 막힌 키와 주소 목록
    pub(crate) fn admin_bans(&self) -> String {
        let st = self.state();
        let keys: Vec<String> = st.control.banned_keys.iter().cloned().collect();
        let mut out = format!("keys ({}):\n{}\naddresses ({}):\n", keys.len(), st.describe(&keys), st.control.banned_ips.len());
        for ip in &st.control.banned_ips {
            out += &format!("  {ip}\n");
        }
        out
    }

    /// `admin registration on|off`
    pub(crate) fn admin_registration(&self, on: bool) -> Result<String, String> {
        let next = Control { registration: on, ..self.control() };
        drop(self.commit_control(next)?);
        Ok(format!("registration {}", if on { "on" } else { "off" }))
    }

    /// `admin friendcode on|off`: 접속 중인 모두에게 코드(또는 `----`)를 다시 알림
    pub(crate) fn admin_friendcode(&self, on: bool) -> Result<String, String> {
        let next = Control { friendcode: on, ..self.control() };
        let mut guard = self.commit_control(next)?;
        let st = &mut *guard;
        if !on {
            st.grants.clear(); // a code redeemed a moment ago opens nothing now
        }
        // Whoever is online holds a code, or is holding a placeholder.
        let ttl = self.limits.code_ttl.as_secs();
        let now = unix_now();
        let keys: Vec<String> = st.peers.keys().cloned().collect();
        for key in keys {
            let current = if on { st.store().code(&key, now, ttl).map(|(code, left, _)| (code, left)) } else { Some((NO_CODE.to_string(), 0)) };
            if let Some((code, code_ttl)) = current {
                st.peers[&key].send(ServerMsg::Code { code, code_ttl });
            }
        }
        Ok(format!("friendcode {}", if on { "on" } else { "off" }))
    }

    /// `admin relay on|off`: 끄면 연결 스레드가 한 번 돌 때 `relay_off`로 끊음
    pub(crate) fn admin_relay(&self, on: bool) -> Result<String, String> {
        let next = Control { relay: on, ..self.control() };
        drop(self.commit_control(next)?);
        Ok(format!("relay {}", if on { "on" } else { "off" }))
    }

    /// `admin purge`: 설치 하나를 잊음 (접속 중이면 끊음)
    pub(crate) fn admin_purge(&self, who: &str) -> Result<String, String> {
        let mut st = self.state();
        let key = st.resolve(who)?;
        let was_online = st.drop_peer(&key, "purged");
        st.grants.retain(|g| g.from != key && g.to != key);
        st.store().remove(&key);
        Ok(format!("purged {key}{}", if was_online { ", disconnected (it registers again if it comes back)" } else { "" }))
    }

    /// `admin purge-idle`: `days`일 넘게 안 온 설치를 잊음 (`yes`가 아니면 목록만)
    pub(crate) fn admin_purge_idle(&self, days: u64, yes: bool) -> Result<String, String> {
        if days == 0 {
            return Err("days must be at least 1".into());
        }
        let mut guard = self.state();
        let st = &mut *guard;
        let peers = &st.peers;
        let keys = st.store.as_ref().expect("store present").idle(unix_now(), days.saturating_mul(24 * 3600), &|k| peers.contains_key(k));
        Ok(Self::purge_listed(st, keys, yes))
    }

    /// `admin purge-islands`: 바깥과 이어지지 않은 작은 무리를 잊음 (`yes`가 아니면 목록만)
    pub(crate) fn admin_purge_islands(&self, min: u64, yes: bool) -> Result<String, String> {
        if min < 2 {
            return Err("a group needs at least 2 installs".into());
        }
        let mut guard = self.state();
        let st = &mut *guard;
        let peers = &st.peers;
        let groups = st.store.as_ref().expect("store present").islands(min as usize, &|k| peers.contains_key(k));
        Ok(Self::purge_listed(st, groups.into_iter().flatten().collect(), yes))
    }

    /// `keys`(모두 오프라인)를 잊음. `yes`가 false면 무엇을 잊을지만 말함
    pub(super) fn purge_listed(st: &mut State, keys: Vec<String>, yes: bool) -> String {
        let list = st.describe(&keys);
        if !yes {
            return format!("would purge {} installs (add --yes):\n{list}", keys.len());
        }
        for key in &keys {
            st.store().remove(key);
        }
        format!("purged {} installs:\n{list}", keys.len())
    }
}
