//! 채널 길잡이: 채널 열기·닫기, 친구 프레임 넘기기, 대역폭과 큐 한도.

use super::*;

/// 각 네트워크가 지키는 전체 바이트 예산 몫
pub(super) const NET_BYTES_SHARE: f64 = 1.0 / 16.0;
/// 전체 큐 한도를 넘어도 한 네트워크가 쓸 수 있는 몫 (모두 합쳐 두 배에서 멈춤)
pub(super) const NET_QUEUE_SHARE: usize = 64;

impl Hub {
    /// `key`에서 `to`로 채널을 엶. 서로 친구이거나 코드로 받은 허가가 있어야 함 (허가는 여기서 씀)
    pub(crate) fn open(&self, key: &str, conn: u64, req: u32, to: &str) -> Option<ServerMsg> {
        let fail = |reason: &str| Some(ServerMsg::OpenFailed { req, reason: reason.into() });
        let mut st = self.state();
        if !Self::is_current(&st, key, conn) {
            return None;
        }
        let Some(to) = proto::norm_key(to) else { return fail("bad_key") };
        if to == key {
            return fail("self");
        }
        // Allowed at all? Asked first: a stranger learns nothing about whether
        // a key is online.
        let now = Instant::now();
        let grant = st.grants.iter().position(|g| g.from == key && g.to == to && g.until > now);
        let mutual = st.store_ref().mutual(key, &to);
        if !mutual && grant.is_none() {
            return fail("not_friends");
        }
        let Some(target) = st.peers.get(&to) else { return fail("offline") };
        let target_conn = target.conn;
        let max = self.limits.max_channels;
        if st.peers[key].channels.len() >= max || target.channels.len() >= max {
            return fail("busy");
        }
        let between = st.channels.values().filter(|c| c.ends.iter().any(|e| e.0 == key) && c.ends.iter().any(|e| e.0 == to)).count();
        if between >= self.limits.max_channels_per_pair {
            return fail("busy");
        }
        if let (false, Some(i)) = (mutual, grant) {
            st.grants.remove(i);
        }
        let ch = loop {
            let ch = st.next_ch;
            st.next_ch = st.next_ch.checked_add(1).unwrap_or(1);
            if ch != 0 && !st.channels.contains_key(&ch) {
                break ch;
            }
        };
        let (rate, burst) = (self.limits.chan_msgs_per_sec, self.limits.chan_msgs_burst);
        st.channels.insert(
            ch,
            Channel { ends: [(key.to_string(), conn), (to.clone(), target_conn)], buckets: [Bucket::new(rate, burst), Bucket::new(rate, burst)] },
        );
        let id = st.peers[key].id.clone();
        st.peers.get_mut(key).expect("current").channels.insert(ch);
        let target = st.peers.get_mut(&to).expect("online");
        target.channels.insert(ch);
        target.send(ServerMsg::Incoming { ch, from: key.to_string(), id });
        Some(ServerMsg::Opened { req, ch })
    }

    /// `key`가 자기 채널을 닫음
    pub(crate) fn close(&self, key: &str, conn: u64, ch: u32) {
        let mut st = self.state();
        let mine = st.channels.get(&ch).is_some_and(|c| c.ends.iter().any(|e| e.0 == key && e.1 == conn));
        if mine {
            st.close_channel(ch, Some(key));
        }
    }

    /// 바이너리 메시지 하나(채널 머리 + 프레임)를 채널 반대편에 넘김
    ///
    /// 모르는 채널은 무시함. 속도·대역폭을 넘거나 받는 쪽 큐가 차면 채널을 닫음.
    pub(crate) fn forward(&self, key: &str, conn: u64, ip: &str, msg: Vec<u8>) {
        let ch = u32::from_be_bytes([msg[0], msg[1], msg[2], msg[3]]);
        let len = msg.len();
        let mut guard = self.state();
        let st = &mut *guard;
        let Some(c) = st.channels.get_mut(&ch) else { return };
        let Some(side) = c.ends.iter().position(|e| e.0 == key && e.1 == conn) else { return };
        let rate_ok = c.buckets[side].take(1.0);
        let other = c.ends[1 - side].0.clone();
        if !rate_ok || !self.bandwidth_allows(st, ip, len as f64) {
            bump(&self.counters.channels_closed_rate);
            st.close_channel(ch, None);
            return;
        }
        let delivered = st.peers.get(&other).is_some_and(|p| self.enqueue_frame(&p.link, msg));
        if delivered {
            bump(&self.counters.frames_forwarded);
            self.counters.bytes_forwarded.fetch_add(len as u64, Ordering::Relaxed);
        } else {
            bump(&self.counters.channels_closed_queue);
            st.close_channel(ch, None);
        }
    }

    /// `ip`에서 `n`바이트를 더 보내도 되는지 봄
    ///
    /// 주소 한도는 딱딱함. 전체 예산은 네트워크 몫 안에서는 막지 않아, 한 곳이 다 써도 자기 채널만 닫힘.
    pub(super) fn bandwidth_allows(&self, st: &mut State, ip: &str, n: f64) -> bool {
        let lim = &self.limits;
        if !bucket_in(&mut st.ip_bytes, ip, lim.ip_bytes_per_sec, lim.ip_bytes_burst, MAX_RATE_KEYS).take(n) || !st.global_bytes_hard.take(n) {
            return false;
        }
        let (rate, burst) = (lim.global_bytes_per_sec * NET_BYTES_SHARE, lim.global_bytes_burst * NET_BYTES_SHARE);
        let in_share = bucket_in(&mut st.net_bytes, &share_bucket(ip), rate, burst, MAX_RATE_KEYS).take(n);
        st.global_bytes.take(n) || in_share
    }

    /// 허브 시작 뒤 지난 밀리초
    pub(super) fn now_ms(&self) -> u64 {
        self.started.elapsed().as_millis() as u64
    }

    /// 친구 프레임을 `link` 큐에 넣음. 안 들어가면 false임
    ///
    /// 큐가 찬 것만으로는 그 채널만 잃음. 큐가 움직이지 않을 때만 `slow`로 끊음.
    pub(super) fn enqueue_frame(&self, link: &Link, msg: Vec<u8>) -> bool {
        let len = msg.len();
        let lim = &self.limits;
        let queued = link.queued.load(Ordering::Relaxed);
        if queued == 0 {
            link.drained_at.store(self.now_ms(), Ordering::Relaxed);
        }
        if queued + len > lim.max_outbox_bytes {
            let still = self.now_ms().saturating_sub(link.drained_at.load(Ordering::Relaxed));
            if still >= lim.slow_after.as_millis() as u64 {
                let _ = link.kill.set("slow");
            }
            return false;
        }
        // The total caps server memory; one network's stuck receivers using it
        // up stop only that network, until the ceiling of twice the cap.
        let total = self.queued_total.load(Ordering::Relaxed) + len;
        let net = link.net_queued.load(Ordering::Relaxed) + len;
        if total > lim.max_total_outbox_bytes && (net > lim.max_total_outbox_bytes / NET_QUEUE_SHARE || total > 2 * lim.max_total_outbox_bytes) {
            return false;
        }
        // Counted first: the connection thread may take it off at once.
        link.queued.fetch_add(len, Ordering::Relaxed);
        link.net_queued.fetch_add(len, Ordering::Relaxed);
        self.queued_total.fetch_add(len, Ordering::Relaxed);
        if link.out.try_send(Out::Binary(msg)).is_err() {
            self.dequeued(link, len);
            return false;
        }
        true
    }

    /// `link` 큐에서 `len`바이트가 나감
    pub(crate) fn dequeued(&self, link: &Link, len: usize) {
        link.queued.fetch_sub(len, Ordering::Relaxed);
        link.net_queued.fetch_sub(len, Ordering::Relaxed);
        self.queued_total.fetch_sub(len, Ordering::Relaxed);
        link.drained_at.store(self.now_ms(), Ordering::Relaxed);
    }
}
