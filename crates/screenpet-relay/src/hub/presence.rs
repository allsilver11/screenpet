//! 접속과 친구: 연결 받아들이기, 로그인, 친구 목록·온라인 알림, 친구 코드, 설치 저장소.

use super::*;

/// 이보다 오래 걸린 저장은 `save_slow`로 적음
pub(super) const SLOW_SAVE: Duration = Duration::from_millis(500);

impl Hub {
    /// `ip`에서 온 새 연결을 셈. 한도를 넘으면 Err임
    ///
    /// 인증 전 연결은 공격자에게 공짜라 주소·네트워크·전체마다 작은 풀을 따로 둠.
    /// 풀이 차도 새 연결을 거절하지 않고 가장 오래된 것을 내보냄 (붙잡고 있으려면 왕복마다 다시 열어야 함).
    #[cfg(test)]
    pub(crate) fn admit_ip(&self, ip: &str) -> Result<Admission, Refused> {
        self.admit(ip, self.conn_id())
    }

    /// `admit_ip`와 같지만 accept에서 받은 연결 번호 `conn`을 씀 (로그의 `conn=`이 처음부터 같게)
    pub(crate) fn admit(&self, ip: &str, conn: u64) -> Result<Admission, Refused> {
        let lim = &self.limits;
        let net = share_bucket(ip);
        let mut guard = self.state();
        let st = &mut *guard;
        if !st.control.relay {
            bump(&self.counters.conns_refused_relay_off);
            return Err(Refused::RelayOff);
        }
        if st.control.banned_ips.contains(ip) {
            bump(&self.counters.conns_refused_banned);
            return Err(Refused::Banned);
        }
        let by_ip = st.ip_conns.get(ip).copied().unwrap_or_default();
        let by_net = st.net_conns.get(&net).copied().unwrap_or_default();
        if by_ip.total >= lim.max_conns_per_ip || by_net.total >= lim.max_conns_per_network {
            bump(&self.counters.conns_refused_limit);
            return Err(Refused::Busy);
        }
        if st.all_conns.total >= lim.max_conns {
            // Room for the next try, if any of them isn't a real install.
            st.evict_unauthed(&self.counters, |_| true);
            bump(&self.counters.conns_refused_limit);
            return Err(Refused::Busy);
        }
        if by_ip.unauthed >= lim.max_unauthed_per_ip {
            st.evict_unauthed(&self.counters, |u| u.ip == ip);
        }
        if st.net_conns.get(&net).is_some_and(|c| c.unauthed >= lim.max_unauthed_per_network) {
            st.evict_unauthed(&self.counters, |u| u.net == net);
        }
        if st.all_conns.unauthed >= lim.max_unauthed {
            st.evict_unauthed(&self.counters, |_| true);
        }
        bump(&self.counters.conns_admitted);
        st.all_conns.admit();
        st.ip_conns.entry(ip.to_string()).or_default().admit();
        st.net_conns.entry(net.clone()).or_default().admit();
        let net_queued = st.net_queued.entry(net.clone()).or_default().clone();
        let kill: Arc<OnceLock<&'static str>> = Arc::default();
        st.unauthed.insert(conn, Unauthed { ip: ip.to_string(), net, kill: kill.clone() });
        Ok(Admission { conn, kill, net_queued })
    }

    /// `admit_ip`로 들인 연결이 끝남
    pub fn release_ip(&self, ip: &str, conn: u64) {
        let net = share_bucket(ip);
        let mut st = self.state();
        st.no_longer_unauthed(conn);
        st.all_conns.release();
        if st.ip_conns.get_mut(ip).is_some_and(Conns::release) {
            st.ip_conns.remove(ip);
        }
        if st.net_conns.get_mut(&net).is_some_and(Conns::release) {
            st.net_conns.remove(&net);
            st.net_queued.remove(&net);
        }
    }

    /// 믿는 프록시가 클라이언트 주소를 안 알려 주면 한 번만 경고함
    pub fn warn_proxy_without_address(&self) {
        bump(&self.counters.proxy_without_address);
        if !self.proxy_warned.swap(true, Ordering::Relaxed) {
            // 프록시 설정을 볼 것: 그 뒤 클라이언트가 모두 한 주소의 한도를 나눠 씀
            log!(Warn, "hub", "proxy_no_address");
        }
    }

    /// 올바르고 겹치지 않는 정규 친구 키만 남김 (`own` 제외, 최대 `max_friends`)
    pub(super) fn clean_friends(&self, own: &str, keys: &[String]) -> Vec<String> {
        let mut out: Vec<String> = Vec::new();
        for k in keys.iter().filter_map(|k| proto::norm_key(k)) {
            if k != own && !out.contains(&k) && out.len() < self.limits.max_friends {
                out.push(k);
            }
        }
        out
    }

    /// 인증된 설치가 접속함. 같은 키의 이전 연결은 내보냄. `welcome` 메시지를 돌려줌 (`login`·`login_refused`를 적음)
    pub(crate) fn join(
        &self,
        key: &str,
        id: &str,
        friends: &[String],
        ip: &str,
        conn: u64,
        link: Link,
    ) -> Result<ServerMsg, &'static str> {
        let result = self.join_inner(key, id, friends, ip, conn, link);
        match &result {
            Ok((ServerMsg::Welcome { online, .. }, new, n)) => log!(
                Info, "hub", "login",
                "conn" => Cid(conn), "peer" => short(key), "ip" => mask_ip(ip), "v" => proto::PROTOCOL_VERSION,
                "new" => u8::from(*new), "friends" => *n, "online" => online.len(),
            ),
            Ok(_) => {}
            Err(reason) => log!(Warn, "hub", "login_refused", "conn" => Cid(conn), "peer" => short(key), "ip" => mask_ip(ip), "reason" => reason),
        }
        result.map(|(m, _, _)| m)
    }

    /// `join`의 본체: (`welcome`, 새 설치인지, 알린 친구 수)
    fn join_inner(
        &self,
        key: &str,
        id: &str,
        friends: &[String],
        ip: &str,
        conn: u64,
        link: Link,
    ) -> Result<(ServerMsg, bool, usize), &'static str> {
        let friends = self.clean_friends(key, friends);
        let now = unix_now();
        let mut st = self.state();
        if !st.unauthed.contains_key(&conn) {
            return Err("busy"); // evicted just now
        }
        if !st.control.relay {
            return Err("relay_off");
        }
        if st.control.banned_keys.contains(key) || st.control.banned_ips.contains(ip) {
            bump(&self.counters.logins_refused_banned);
            return Err("banned");
        }
        let known = st.store_ref().get(key).is_some();
        if !known {
            if !st.control.registration {
                bump(&self.counters.registrations_refused_closed);
                return Err("registration_closed");
            }
            // (A full store is handled by `register_with`, which first evicts
            // a stale friendless install.)
            // Each network gets its own budget; the global ceiling is a
            // looser backstop.
            if !allow(&mut st.ip_registrations, &network_bucket(ip), self.limits.registrations_per_ip_per_hour, HOUR, MAX_RATE_KEYS)
                || !allow(&mut st.net_registrations, &share_bucket(ip), self.limits.registrations_per_network_per_day, DAY, MAX_RATE_KEYS)
                || !allow_window(&mut st.global_registrations, self.limits.registrations_per_hour, HOUR)
            {
                bump(&self.counters.registrations_refused);
                return Err("busy");
            }
        }
        if let Some(old) = st.peers.remove(key) {
            bump(&self.counters.logins_replaced);
            // Two copies of one install fighting over the relay show up here (e.g. a copied identity)
            log!(
                Warn, "hub", "login_replaced",
                "conn" => Cid(conn), "peer" => short(key), "ip" => mask_ip(ip), "old_conn" => Cid(old.conn), "old_ip" => mask_ip(&old.ip),
            );
            old.link.kick("replaced");
            for ch in old.channels {
                st.close_channel(ch, Some(key));
            }
        }
        let n_friends = friends.len();
        let registered = {
            let st = &mut *st;
            let peers = &st.peers;
            st.store.as_mut().expect("store present").register_with(key, id, friends, now, &|k| peers.contains_key(k))
        };
        if !registered {
            bump(&self.counters.store_full);
            return Err("full");
        }
        if !known {
            bump(&self.counters.registrations);
        }
        let (code, code_ttl) = if st.control.friendcode {
            let (code, ttl, _) = st.store().code(key, now, self.limits.code_ttl.as_secs()).expect("registered");
            (code, ttl)
        } else {
            (NO_CODE.to_string(), 0)
        };
        let place = st.weathers.want(DEFAULT_PLACE, Instant::now()).expect("built-in");
        let weather = st.weathers.reading(place);
        let peer = Peer { conn, id: id.to_string(), ip: ip.to_string(), since: now, link, channels: HashSet::new(), weather: place, board_at: None };
        // Queued now, written after the welcome (old apps ignore it)
        if let Some(r) = weather {
            peer.send(weather_msg(place, r));
        }
        st.peers.insert(key.to_string(), peer);
        st.no_longer_unauthed(conn);
        let online = st.mutual_online(key);
        for f in &online {
            st.peers[f].send(ServerMsg::Presence { key: key.to_string(), online: true });
        }
        Ok((ServerMsg::Welcome { code, code_ttl, online, proof: None }, !known, n_friends))
    }

    /// 연결이 끝남 (이미 새 연결로 바뀌었으면 아무것도 안 함)
    pub(crate) fn leave(&self, key: &str, conn: u64) {
        let mut st = self.state();
        if st.peers.get(key).is_none_or(|p| p.conn != conn) {
            return; // already replaced by a newer connection
        }
        let online = st.mutual_online(key);
        let peer = st.peers.remove(key).expect("checked");
        log!(Info, "hub", "logout", "conn" => Cid(conn), "peer" => short(key), "channels" => peer.channels.len());
        for ch in peer.channels {
            st.close_channel(ch, Some(key));
        }
        for f in online {
            if let Some(p) = st.peers.get(&f) {
                p.send(ServerMsg::Presence { key: key.to_string(), online: false });
            }
        }
        st.store().touch(key, unix_now());
    }

    /// `key`의 친구 목록을 바꿈. 서로 친구 상태가 바뀐 친구에게 알림
    pub(crate) fn set_friends(&self, key: &str, conn: u64, keys: &[String]) -> Option<ServerMsg> {
        let friends = self.clean_friends(key, keys);
        let mut st = self.state();
        if !Self::is_current(&st, key, conn) {
            return None;
        }
        let before: HashSet<String> = st.mutual_online(key).into_iter().collect();
        st.store().set_friends(key, friends);
        let after = st.mutual_online(key);
        let after_set: HashSet<String> = after.iter().cloned().collect();
        for f in after_set.symmetric_difference(&before) {
            if let Some(p) = st.peers.get(f) {
                p.send(ServerMsg::Presence { key: key.to_string(), online: after_set.contains(f) });
            }
        }
        let n = st.store_ref().get(key).map_or(0, |i| i.friends.len());
        log!(Info, "presence", "friends", "conn" => Cid(conn), "peer" => short(key), "n" => n, "online" => after.len());
        Some(ServerMsg::Online { keys: after })
    }

    /// 친구 코드를 씀. 성공하면 `key`가 주인에게 `grant_ttl` 동안 채널 하나를 열 수 있음 (코드는 로그에 안 적음)
    pub(crate) fn redeem(&self, key: &str, conn: u64, ip: &str, code: &str) -> Option<ServerMsg> {
        let reply = self.redeem_inner(key, conn, ip, code);
        match &reply {
            Some(ServerMsg::Redeemed { key: target, .. }) => log!(Info, "presence", "redeem_ok", "conn" => Cid(conn), "peer" => short(key), "to" => short(target)),
            Some(ServerMsg::RedeemFailed { reason, .. }) => log!(Warn, "presence", "redeem_fail", "conn" => Cid(conn), "peer" => short(key), "reason" => reason),
            _ => {}
        }
        reply
    }

    /// `redeem`의 본체
    fn redeem_inner(&self, key: &str, conn: u64, ip: &str, code: &str) -> Option<ServerMsg> {
        let fail = |reason: &str| Some(ServerMsg::RedeemFailed { code: code.to_string(), reason: reason.into() });
        let mut st = self.state();
        if !Self::is_current(&st, key, conn) {
            return None;
        }
        if !st.control.friendcode {
            return fail("disabled");
        }
        if !allow(&mut st.ip_redeems, ip, self.limits.redeems_per_ip_per_min, MINUTE, MAX_RATE_KEYS) {
            return fail("rate");
        }
        // One answer for "no such code", "owner offline" and "your own code":
        // a guess learns nothing about which codes exist.
        let Some(norm) = proto::normalize_code(code) else { return fail("unknown") };
        // Global budget, charged only for well-formed codes. Each network
        // (IPv4 /24, IPv6 /48) has a fair share (a tenth of the budget, never
        // more than the per-IP cap) reserved inside it: within its share a
        // network is not refused by the soft budget, so one heavy source
        // cannot starve the rest. A hard ceiling of twice the budget applies
        // to everyone.
        let per_min = self.limits.redeems_per_min;
        let share = (per_min / 10).min(self.limits.redeems_per_ip_per_min).max(1);
        let light = allow(&mut st.ip_wellformed, &share_bucket(ip), share, MINUTE, MAX_RATE_KEYS);
        expire(&mut st.global_redeems, MINUTE);
        let used = st.global_redeems.len();
        if used >= 2 * per_min || (!light && used >= per_min) {
            return fail("rate");
        }
        st.global_redeems.push_back(Instant::now());
        let now = unix_now();
        let Some(target) = st.store_ref().by_code(&norm, now, self.limits.code_ttl.as_secs()).map(str::to_string) else {
            return fail("unknown");
        };
        let Some(id) = st.peers.get(&target).map(|p| p.id.clone()).filter(|_| target != key) else {
            return fail("unknown");
        };
        // A code that keeps being redeemed is being guessed or spread: cap
        // distinct requesters per code (a new code starts fresh; a requester
        // retrying counts once).
        let per_code = format!("{target} {norm}");
        if !st.target_redeems.contains_key(&per_code)
            && !(allow(&mut st.net_new_codes, &share_bucket(ip), self.limits.new_codes_per_network_per_hour, HOUR, MAX_RATE_KEYS)
                && allow(&mut st.net_new_codes, &wide_bucket(ip), WIDE_NEW_CODES * self.limits.new_codes_per_network_per_hour, HOUR, MAX_RATE_KEYS))
        {
            return fail("rate");
        }
        if !allow_distinct(&mut st.target_redeems, &per_code, key, self.limits.redeems_per_target_per_hour, HOUR, MAX_RATE_KEYS) {
            return fail("unknown");
        }
        st.store().mark_active(key);
        st.store().mark_active(&target);
        let until = Instant::now() + self.limits.grant_ttl;
        let t = Instant::now();
        st.grants.retain(|g| g.until > t && !(g.from == key && g.to == target));
        if st.grants.iter().filter(|g| g.from == key).count() >= 4 {
            let oldest = st.grants.iter().position(|g| g.from == key).expect("counted");
            st.grants.remove(oldest);
        }
        st.grants.push(Grant { from: key.to_string(), to: target.clone(), until });
        Some(ServerMsg::Redeemed { code: norm, key: target, id })
    }

    /// 사용자가 새 친구 코드를 원함 (코드가 꺼져 있으면 `----`)
    pub(crate) fn new_code(&self, key: &str, conn: u64) -> Option<ServerMsg> {
        let mut st = self.state();
        if !Self::is_current(&st, key, conn) {
            return None;
        }
        if !st.control.friendcode {
            return Some(ServerMsg::Code { code: NO_CODE.to_string(), code_ttl: 0 }); // the apps wait for an answer
        }
        let now = unix_now();
        log!(Info, "presence", "new_code", "conn" => Cid(conn), "peer" => short(key));
        st.store().rotate(key, now);
        let (code, code_ttl, _) = st.store().code(key, now, self.limits.code_ttl.as_secs())?;
        Some(ServerMsg::Code { code, code_ttl })
    }

    /// 코드가 만료돼 바뀌었으면 새 코드를 줌. 아니면 None임
    pub(crate) fn rotated_code(&self, key: &str, conn: u64) -> Option<ServerMsg> {
        let mut st = self.state();
        if !Self::is_current(&st, key, conn) || !st.control.friendcode {
            return None;
        }
        let (code, code_ttl, rotated) = st.store().code(key, unix_now(), self.limits.code_ttl.as_secs())?;
        rotated.then_some(ServerMsg::Code { code, code_ttl })
    }

    /// 오래된 설치를 잊음. 접속 중인 설치는 먼저 본 시각을 새로 적음 (재시작 뒤에도 살아 있게)
    pub(crate) fn prune_store(&self) {
        let now = unix_now();
        let mut guard = self.state();
        let st = &mut *guard;
        let store = st.store.as_mut().expect("store present");
        for key in st.peers.keys() {
            store.touch(key, now);
        }
        let peers = &st.peers;
        store.prune_with(now, &|k| peers.contains_key(k));
    }

    /// 저장소가 바뀌었으면 씀. 스냅숏만 잠금 안에서 뜨고 쓰기는 밖에서 함
    pub(crate) fn save(&self) {
        let snapshot = self.state().store().snapshot();
        if let Some(snap) = snapshot {
            let started = Instant::now();
            match crate::store::write_snapshot(&snap) {
                Err(e) => {
                    log!(Error, "store", "save_fail", "took_ms" => took_ms(started), "err" => e);
                    bump(&self.counters.save_errors);
                    self.state().store().mark_dirty();
                }
                Ok(()) if started.elapsed() >= SLOW_SAVE => log!(Warn, "store", "save_slow", "took_ms" => took_ms(started)),
                Ok(()) => {}
            }
        }
    }
}
