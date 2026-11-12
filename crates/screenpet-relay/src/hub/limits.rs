//! 속도·자원 한도: 토큰 버킷, 키 수가 묶인 미닫이 창 표, 주소를 네트워크로 묶는 규칙.
//!
//! 허브 상태(`State`) 안의 표를 다루는 순수 함수들이라 잠금은 부르는 쪽이 잡음.

use super::*;

/// 토큰 버킷: 초마다 `rate`, 최대 `burst`
#[derive(Debug, Clone)]
pub(crate) struct Bucket {
    tokens: f64,
    rate: f64,
    burst: f64,
    last: Instant,
}

impl Default for Bucket {
    /// 비어 있고 차지 않는 버킷
    fn default() -> Bucket {
        Bucket::new(0.0, 0.0)
    }
}

impl Bucket {
    /// `rate`/초로 차고 `burst`까지 담는 꽉 찬 버킷을 만듦
    pub fn new(rate: f64, burst: f64) -> Bucket {
        Bucket { tokens: burst, rate, burst, last: Instant::now() }
    }
    /// 지금쯤 `burst`까지 찼을지 봄 (그러면 잊어도 잃는 것 없음)
    pub(super) fn is_full(&self) -> bool {
        self.tokens + self.last.elapsed().as_secs_f64() * self.rate >= self.burst
    }
    /// `n`만큼 꺼냄. 모자라면 false임
    pub fn take(&mut self, n: f64) -> bool {
        let now = Instant::now();
        self.tokens = (self.tokens + now.duration_since(self.last).as_secs_f64() * self.rate).min(self.burst);
        self.last = now;
        if self.tokens >= n {
            self.tokens -= n;
            true
        } else {
            false
        }
    }
}

/// 최대 `max_keys`개인 표에서 `key`의 버킷을 줌. 차면 다 찬 버킷부터, 그다음 가장 꽉 찬 것을 지움
pub(super) fn bucket_in<'a>(table: &'a mut HashMap<String, Bucket>, key: &str, rate: f64, burst: f64, max_keys: usize) -> &'a mut Bucket {
    if !table.contains_key(key) && table.len() >= max_keys {
        table.retain(|_, b| !b.is_full());
        while table.len() >= max_keys {
            let fullest = table.iter().max_by(|a, b| a.1.tokens.total_cmp(&b.1.tokens)).map(|(k, _)| k.clone());
            match fullest {
                Some(k) => table.remove(&k),
                None => break,
            };
        }
    }
    table.entry(key.to_string()).or_insert_with(|| Bucket::new(rate, burst))
}

/// 넓은 네트워크(`wide_bucket`)가 코드 표에 더할 수 있는 네트워크 몫 수 (/48은 싸고 /32는 비쌈)
pub(super) const WIDE_NEW_CODES: usize = 4;

/// 속도 표 하나의 최대 키 수. 차면 오래된 키부터 내보냄 (새로 온 쪽을 통째로 거절하지 않음)
pub(super) const MAX_RATE_KEYS: usize = 8192;

/// 속도 표에서 시각이 붙은 것
pub(super) trait Stamped {
    /// 기록 시각
    fn at(&self) -> Instant;
}
impl Stamped for Instant {
    /// 그 시각 자체임
    fn at(&self) -> Instant {
        *self
    }
}
impl Stamped for (Instant, String) {
    /// 앞의 시각임
    fn at(&self) -> Instant {
        self.0
    }
}

/// 한 네트워크가 속도 표에 만들 수 있는 키 수 (IPv4 /24마다 주소, IPv6 /48마다 /64). 넘으면 네트워크 키를 같이 씀
pub(super) const MAX_KEYS_PER_PARENT: usize = 16;

/// 주소 키를 감싸는 네트워크: IPv4 → /24, IPv6 /64 → /48
pub(super) fn parent_of(key: &str) -> Option<String> {
    if key.parse::<std::net::Ipv4Addr>().is_ok() {
        return Some(network_bucket(key));
    }
    let prefix = key.strip_suffix("::/64")?;
    let segs: Vec<&str> = prefix.split(':').collect();
    (segs.len() == 4).then(|| format!("{}:{}:{}::/48", segs[0], segs[1], segs[2]))
}

/// 코드 쓰기 몫을 세는 묶음: IPv4 /24, IPv6 /48
pub(super) fn share_bucket(ip: &str) -> String {
    parent_of(ip).unwrap_or_else(|| ip.to_string())
}

/// 한 주인의 할당 전체쯤: IPv4 /16, IPv6 /32
pub(super) fn wide_bucket(ip: &str) -> String {
    if let Ok(v4) = ip.parse::<std::net::Ipv4Addr>() {
        let o = v4.octets();
        return format!("{}.{}.0.0/16", o[0], o[1]);
    }
    let mut segs = ip.split(':');
    match (segs.next(), segs.next()) {
        (Some(a), Some(b)) => format!("{a}:{b}::/32"),
        _ => ip.to_string(),
    }
}

/// `key`를 셀 이름: 자기 자신, 또는 그 네트워크가 이미 `MAX_KEYS_PER_PARENT`개를 가졌으면 네트워크
pub(super) fn capped_key<V>(table: &HashMap<String, VecDeque<V>>, key: &str) -> String {
    if table.contains_key(key) {
        return key.to_string();
    }
    match parent_of(key) {
        Some(parent) if table.keys().filter(|k| parent_of(k).as_deref() == Some(parent.as_str())).count() >= MAX_KEYS_PER_PARENT => parent,
        _ => key.to_string(),
    }
}

/// 최대 `max`개인 표에 `key` 자리를 냄: 오래된 키부터, 그다음 가장 오래 쉰 키
pub(super) fn make_room<V: Stamped>(table: &mut HashMap<String, VecDeque<V>>, key: &str, window: Duration, max: usize) {
    if table.contains_key(key) || table.len() < max {
        return;
    }
    prune_table(table, window);
    while table.len() >= max {
        let oldest = table.iter().min_by_key(|(_, q)| q.back().map(Stamped::at)).map(|(k, _)| k.clone());
        match oldest {
            Some(k) => table.remove(&k),
            None => break,
        };
    }
}

/// `window`보다 오래된 기록을 버림
pub(super) fn expire(q: &mut VecDeque<Instant>, window: Duration) {
    let now = Instant::now();
    while q.front().is_some_and(|t| now.duration_since(*t) >= window) {
        q.pop_front();
    }
}

/// 미닫이 창 하나: `window` 안에 `limit`보다 적으면 세고 true임
pub(super) fn allow_window(q: &mut VecDeque<Instant>, limit: usize, window: Duration) -> bool {
    expire(q, window);
    if q.len() >= limit {
        return false;
    }
    q.push_back(Instant::now());
    true
}

/// 키마다 미닫이 창 (표는 최대 `max_keys`개). 오래된 키는 `prune_table`이 버림
pub(super) fn allow(table: &mut HashMap<String, VecDeque<Instant>>, key: &str, limit: usize, window: Duration, max_keys: usize) -> bool {
    let key = capped_key(table, key);
    make_room(table, &key, window, max_keys);
    allow_window(table.entry(key).or_default(), limit, window)
}

/// `who`가 `key`에 셀 수 있는지 봄: 이미 창 안의 요청자이거나 서로 다른 요청자가 `limit`보다 적으면 true (한 번만 셈)
///
/// 보안상 중요: 살아 있는 기록은 내보내지 않음. 표가 살아 있는 키로 차면 *새* 키만 거절함.
pub(super) fn allow_distinct(
    table: &mut HashMap<String, VecDeque<(Instant, String)>>,
    key: &str,
    who: &str,
    limit: usize,
    window: Duration,
    max_keys: usize,
) -> bool {
    let now = Instant::now();
    if !table.contains_key(key) && table.len() >= max_keys {
        prune_table(table, window);
        if table.len() >= max_keys {
            return false;
        }
    }
    let q = table.entry(key.to_string()).or_default();
    while q.front().is_some_and(|(t, _)| now.duration_since(*t) >= window) {
        q.pop_front();
    }
    if q.iter().any(|(_, w)| w == who) {
        return true;
    }
    if q.len() >= limit {
        return false;
    }
    q.push_back((now, who.to_string()));
    true
}

/// 가장 새 기록도 `window`보다 오래된 키를 버림
pub(super) fn prune_table<V: Stamped>(table: &mut HashMap<String, VecDeque<V>>, window: Duration) {
    let now = Instant::now();
    table.retain(|_, q| q.back().is_some_and(|t| now.duration_since(t.at()) < window));
}

/// 새 설치 예산 묶음: IPv4는 /24 (IPv6는 이미 /64로 옴)
pub(super) fn network_bucket(ip: &str) -> String {
    match ip.parse::<std::net::Ipv4Addr>() {
        Ok(v4) => {
            let o = v4.octets();
            format!("{}.{}.{}.0/24", o[0], o[1], o[2])
        }
        Err(_) => ip.to_string(),
    }
}

pub(super) const HOUR: Duration = Duration::from_secs(3600);
pub(super) const MINUTE: Duration = Duration::from_secs(60);
pub(super) const DAY: Duration = Duration::from_secs(24 * 3600);

impl Hub {
    /// 메모리에만 있는 것을 정리함: 만료된 허가, 속도 표
    pub(crate) fn maintain(&self) {
        let mut st = self.state();
        let now = Instant::now();
        st.grants.retain(|g| g.until > now);
        prune_table(&mut st.ip_redeems, MINUTE);
        expire(&mut st.global_redeems, MINUTE);
        prune_table(&mut st.ip_registrations, HOUR);
        prune_table(&mut st.net_registrations, DAY);
        expire(&mut st.global_registrations, HOUR);
        prune_table(&mut st.target_redeems, HOUR);
        prune_table(&mut st.net_new_codes, HOUR);
        prune_table(&mut st.ip_wellformed, MINUTE);
        st.ip_bytes.retain(|_, b| !b.is_full());
        st.net_bytes.retain(|_, b| !b.is_full());
    }
}
