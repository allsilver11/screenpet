//! 링크 하나의 세션 구동기임. 전송과 무관함 (프레임 들어오고 프레임 나감).
//!
//! 1. Noise XX 핸드셰이크 (다이얼한 쪽 = initiator) → 상대 정적 공개키.
//! 2. 암호화된 `hello{id, name, friend}`. 그 id의 친구가 정확히 그 키로 고정돼 있고 상대도 우리 키를
//!    고정했으면(`friend: true`) *친구* 링크임. 키가 다른 id나 다른 id 밑의 고정 키는 거절함.
//!    나가는 링크면 id가 다이얼한 id와 같아야 함. 핸드셰이크 + hello는 마감 하나를 같이 씀.
//! 3. 아니면 *낯선* 링크임 (여럿이 같은 id를 주장해도 서로나 친구를 막지 않음). 짝짓기만 할 수 있고
//!    초대자가 다이얼한 링크에서만 됨: `invite{commit}` → `pair_random` → `pair_reveal` → 양쪽에 같은 이모지 6개 SAS →
//!    마감 전에 두 사용자가 `invite_reply{accepted}`로 확인 → 서로 키를 고정함. 거절·시간 초과는 끝임.
//!
//! 진입점: `run_session`(tcp·relay 전송이 부름), `invite`/`invite_at`/`answer_invite`(lib의 `Net`이 부름).

use crate::discovery::{note_dial, pick_candidate};
use crate::noise::{self, Established};
use crate::protocol::{clamp_name, Control, Message, DEVICES_MIN_VERSION, MAX_FRAME, MAX_FRIENDS, MAX_ID, PROTOCOL_VERSION};
use crate::secure::SecureLink;
use crate::transport::{self, FrameLink, FrameReader, Recv};
use crate::{
    Backoff, Conn, Kind, NetEvent, PairStage, Pairing, SessionGuard, Shared, State, INVITED_WINDOW,
    MAX_INVITED_PER_WINDOW, MAX_INVITES_IN, MAX_INVITES_IN_PER_ADDR, MAX_PAIR_ATTEMPTS_PER_ADDR, PRE_AUTH_MAX,
};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Instant;

/// 다이얼이 어떻게 끝났는지임 (후보 기록용)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DialOutcome {
    /// 다이얼한 신원이 답함 (링크를 남겼거나 남긴 것의 중복)
    Proven,
    /// 다이얼한 친구 id가 다른 정적 키로 답함
    WrongKey,
    /// 그 밖 (연결 없음, 핸드셰이크 실패, 거절)
    Failed,
}

/// hello 뒤 링크를 분류한 결과임
enum Reg {
    /// 받아들임
    Ok,
    /// 중복 동점 깨기에서 진 친구 링크
    Duplicate,
    /// 친구 id인데 키가 다름
    WrongKey,
    /// 고정한 친구(키 맞음)가 우리를 고정하지 않았음
    NotMutual,
    /// 그 밖에 거절함
    Reject,
}

/// 제어 메시지를 `Message`로 감쌈
fn ctl(c: Control) -> Message {
    Message::Control(c)
}

/// 이 링크에서 짝짓기가 진행 중인지 봄 (실패로 끝난 것은 아님)
fn pairing_active(c: &Conn) -> bool {
    matches!(&c.kind, Kind::Stranger(Some(p)) if !matches!(p.stage, PairStage::Failed))
}

/// `id`와의 짝짓기(어느 방향이든)가 진행 중인지 봄
pub(crate) fn pairing_with(st: &State, id: &str) -> bool {
    st.conns.values().any(|c| c.peer_id == id && pairing_active(c))
}

/// *우리가* 건 `id`로의 짝짓기가 진행 중인지 봄 (`id`를 주장만 하는 들어온 링크는 우리 초대를 막지 않음)
fn outbound_pairing(st: &State, id: &str) -> bool {
    st.conns.values().any(|c| c.outgoing && c.peer_id == id && pairing_active(c))
}

/// 한 출처 주소에서 들어온 진행 중 짝짓기 수임 (주장한 id가 아니라 주소당 제한이라 무단 점유자가 남의 id를 막지 못함)
fn inbound_pairings_from(st: &State, addr: &str) -> usize {
    st.conns.values().filter(|c| !c.outgoing && c.peer_addr == addr && pairing_active(c)).count()
}

/// 핸드셰이크로 정해진 세션별 사실들임
struct Ctx {
    token: u64,
    link: Arc<SecureLink>,
    dialed: Option<String>,
    remote_key: [u8; 32],
    hs_hash: [u8; 32],
    peer_addr: String,
    unverified: Arc<AtomicBool>,
    surfaced: Arc<AtomicBool>,
}

/// 링크 하나를 닫히거나·시간 초과·잘못 행동하거나·종료될 때까지 구동함.
/// `dialed`는 우리가 연결한 상대 id(나가는 링크)이고 받은 링크면 None임
pub(crate) fn run_session(
    shared: Arc<Shared>,
    frames: Arc<dyn FrameLink>,
    mut reader: Box<dyn FrameReader>,
    dialed: Option<String>,
    guard: SessionGuard,
) -> DialOutcome {
    let t = shared.timing.clone();
    // One deadline for the whole pre-auth phase (handshake + hello).
    let pre_auth = Instant::now() + t.conn_timeout.min(PRE_AUTH_MAX);
    let est = noise::handshake(&shared.static_key, dialed.is_some(), &*frames, &mut *reader, pre_auth, &shared.stop);
    let Some(Established { transport, remote_static, hash }) = est else {
        frames.close();
        cleanup(&shared, None, dialed.as_deref());
        return DialOutcome::Failed;
    };
    // 📱 On standby no friend gets in (my linked computer does): closed before any hello
    if shared.state().standby_refuses_key(&remote_static) {
        frames.close();
        cleanup(&shared, None, dialed.as_deref());
        return DialOutcome::Failed;
    }
    let transport = Arc::new(Mutex::new(transport));
    let Some(link) = SecureLink::start(&shared, frames.clone(), transport.clone()) else {
        frames.close();
        cleanup(&shared, None, dialed.as_deref());
        return DialOutcome::Failed;
    };
    let ctx = Ctx {
        token: shared.token(),
        link: link.clone(),
        dialed: dialed.clone(),
        remote_key: remote_static,
        hs_hash: hash,
        peer_addr: frames.peer_addr(),
        unverified: guard.unverified.clone(),
        surfaced: guard.surfaced.clone(),
    };
    let we_pinned_them = shared.state().friends.iter().any(|f| f.public_key == remote_static);
    link.send(&ctl(Control::Hello {
        id: shared.id.clone(),
        name: shared.name(),
        v: PROTOCOL_VERSION,
        friend: we_pinned_them,
    }));

    let mut outcome = DialOutcome::Failed;
    let mut registered: Option<(String, String)> = None;
    let mut buf = vec![0u8; MAX_FRAME];
    let mut last_recv = Instant::now();
    let mut last_ping = Instant::now();
    let mut closing_since: Option<Instant> = None;
    while !shared.stopping() {
        match reader.recv() {
            Recv::Closed | Recv::Invalid => break,
            Recv::Idle => {}
            Recv::Frame(frame) => {
                last_recv = Instant::now();
                let n = transport.lock().unwrap_or_else(|e| e.into_inner()).read_message(&frame, &mut buf);
                let Ok(n) = n else { break }; // forged, replayed or reordered frame
                let Some(msg) = Message::parse(&buf[..n]) else { break };
                match &registered {
                    None => {
                        let Message::Control(Control::Hello { id, name, friend, v }) = msg else { break };
                        let name = clamp_name(&name);
                        match register(&shared, &ctx, &id, &name, friend, v) {
                            Reg::Ok => {
                                guard.handshake_done();
                                outcome = DialOutcome::Proven;
                                registered = Some((id, name));
                            }
                            Reg::Duplicate | Reg::NotMutual => {
                                outcome = DialOutcome::Proven;
                                break;
                            }
                            Reg::WrongKey => {
                                outcome = DialOutcome::WrongKey;
                                break;
                            }
                            Reg::Reject => break,
                        }
                    }
                    Some((id, name)) => {
                        if !handle(&shared, &ctx, id, name, msg) {
                            break;
                        }
                    }
                }
            }
        }
        let now = Instant::now();
        if now.duration_since(last_recv) > t.conn_timeout || (registered.is_none() && now > pre_auth) {
            break;
        }
        if link.closing() {
            if now.duration_since(*closing_since.get_or_insert(now)) > t.conn_timeout {
                break;
            }
        } else if now.duration_since(last_ping) >= t.ping_interval {
            if !link.send(&ctl(Control::Ping { ask: None, steer: None, toss: None })) {
                break;
            }
            last_ping = now;
        }
        if registered.is_some() && !timers(&shared, &ctx) {
            break;
        }
    }
    link.close();
    cleanup(&shared, Some(ctx.token), dialed.as_deref());
    drop(guard);
    outcome
}

/// 상대 hello 뒤 링크를 분류하고 기록함
fn register(shared: &Shared, ctx: &Ctx, id: &str, name: &str, peer_pinned_us: bool, v: u32) -> Reg {
    if id.is_empty() || id.len() > MAX_ID || id == shared.id {
        return Reg::Reject;
    }
    if ctx.dialed.as_deref().is_some_and(|d| d != id) {
        return Reg::Reject; // answered by someone other than who we dialed
    }
    let outgoing = ctx.dialed.is_some();
    let dialer = if outgoing { shared.id.clone() } else { id.to_string() };
    let now = Instant::now();
    let mut replaced = None;
    let mut first_msg = None;
    {
        let mut st = shared.state();
        let pinned = st.friend_key(id);
        match pinned {
            // A key only a friend's introduction vouches for: whoever this is
            // may still pair for real (as a stranger, SAS-confirmed).
            Some(k) if k != ctx.remote_key && st.introduced_only(id) => {}
            Some(k) if k != ctx.remote_key => {
                // Unproven: whoever answered is not our friend. Silent; the
                // dialed candidate is backed off by the caller.
                return Reg::WrongKey;
            }
            None if st.friends.iter().any(|f| f.public_key == ctx.remote_key) => return Reg::Reject,
            _ => {}
        }
        let friend_link = pinned == Some(ctx.remote_key);
        if friend_link && !peer_pinned_us {
            // The pinned key itself says it does not have us (`pinned` matched
            // `remote_key` above, proven by the Noise handshake). A hello sent
            // just before the peer pinned us can still arrive right after we
            // pinned it; that one means nothing, so only back off. The same for
            // a friend added without pairing (another computer of mine or an
            // introduction): its news of us may not have arrived yet — up to
            // `PENDING_STRIKES` times; then it really doesn't know us.
            if st.just_paired(id, now) {
                st.back_off(Backoff::Id(id.to_string()), shared.timing.peer_timeout, now);
                return Reg::NotMutual;
            }
            if st.pending.contains_key(id) {
                let denied = st.pending_denied.entry(id.to_string()).or_insert(0);
                *denied += 1;
                if *denied < crate::devices::PENDING_STRIKES {
                    st.back_off(Backoff::Id(id.to_string()), shared.timing.peer_timeout, now);
                    return Reg::NotMutual;
                }
                let dropped = crate::devices::drop_pending(shared, &mut st, id);
                drop(st);
                crate::devices::close_dropped(dropped);
                return Reg::NotMutual;
            }
            st.remove_friend(id);
            st.invites_out.remove(id);
            let stale = st.demote_friend_link(id);
            shared.emit(NetEvent::FriendForgotUs { id: id.to_string() });
            let dropped = crate::devices::drop_introduced(shared, &mut st, id);
            drop(st);
            if let Some(l) = stale {
                l.close(); // `announced` stays: Disconnected follows
            }
            crate::devices::close_dropped(dropped);
            return Reg::NotMutual;
        }
        if !friend_link && peer_pinned_us {
            return Reg::Reject; // they think we are friends; we are not: free the slot now
        }
        // A link that ended moments ago still owes its Disconnected: this one
        // takes over its Connected state instead.
        let mut announced = friend_link && !st.friend_links.contains_key(id) && st.pending_gone.remove(id).is_some();
        let kind = if friend_link {
            if let Some(&old_t) = st.friend_links.get(id) {
                // One friend link per id. A newer link from the same dialer
                // replaces a stale one; links dialed by different sides
                // resolve to the one dialed by the smaller id on both peers.
                let min = if shared.id.as_str() < id { shared.id.as_str() } else { id };
                let old = st.conns.get_mut(&old_t).expect("friend link indexed");
                if old.dialer != dialer && dialer != min {
                    return Reg::Duplicate;
                }
                announced = old.announced;
                old.announced = false; // the new link inherits the Connected state
                replaced = Some(old.link.clone());
            }
            shared.release_unverified(&ctx.unverified);
            st.trust_ip(id, &ctx.peer_addr, now);
            st.friend_wait.remove(id);
            st.backoff.remove(&Backoff::Id(id.to_string()));
            // It pinned us too: no longer unconfirmed
            st.pending.remove(id);
            st.pending_denied.remove(id);
            st.friend_links.insert(id.to_string(), ctx.token);
            Kind::Friend
        } else if outgoing {
            // We only dial strangers to deliver an invite.
            let Some(pet) = st.invites_out.remove(id) else { return Reg::Reject };
            let nonce_i = noise::random32();
            first_msg = Some(Control::Invite { pet, commit: noise::to_hex(&noise::commit(&nonce_i)) });
            Kind::Stranger(Some(Pairing {
                stage: PairStage::AwaitRandom { nonce_i },
                local_ok: false,
                remote_ok: false,
                since: now,
                failure_reported: false,
                surfaced: false,
            }))
        } else {
            Kind::Stranger(None)
        };
        // My linked computer proves our secret again on every new link
        let mut dev = crate::devices::DevLink::default();
        if friend_link && v >= DEVICES_MIN_VERSION && st.device.as_ref().is_some_and(|(d, _)| d == id) {
            let nonce = crate::devices::link_nonce();
            dev.mine = Some(nonce);
            first_msg = Some(Control::DeviceNonce { nonce: noise::to_hex(&nonce) });
        }
        let connected = friend_link && !announced;
        st.conns.insert(
            ctx.token,
            Conn {
                peer_id: id.to_string(),
                link: ctx.link.clone(),
                name: name.to_string(),
                v,
                kind,
                announced: announced || connected,
                outgoing,
                dialer,
                remote_key: ctx.remote_key,
                peer_addr: ctx.peer_addr.clone(),
                hs_hash: ctx.hs_hash,
                since: now,
                unverified: ctx.unverified.clone(),
                dev,
            },
        );
        if connected {
            shared.emit(NetEvent::Connected { id: id.to_string(), name: name.to_string() });
        }
    }
    if let Some(old) = replaced {
        old.close();
    }
    if let Some(m) = first_msg {
        if !ctx.link.send(&ctl(m)) {
            return Reg::Reject;
        }
    }
    Reg::Ok
}

/// 짝짓기 링크 `token`의 상대를 고정함 (두 사용자가 시간 안에 확인함).
/// 같은 id를 주장하는 다른 낯선 링크들을 돌려줘 닫게 함. 초대 뒤 친구 목록이 찼으면
/// (짝짓기는 나란히 돌 수 있음) 이 짝짓기는 거절로 끝남
fn pin(shared: &Shared, st: &mut State, token: u64) -> Vec<Arc<SecureLink>> {
    let Some(id) = st.conns.get(&token).map(|c| c.peer_id.clone()) else { return Vec::new() };
    let full = st.friends.len() >= MAX_FRIENDS && !st.is_friend(&id);
    let Some(conn) = st.conns.get_mut(&token) else { return Vec::new() };
    if full {
        let (id, link) = (conn.peer_id.clone(), conn.link.clone());
        if let Kind::Stranger(Some(p)) = &mut conn.kind {
            fail(shared, p, &id, token);
        }
        link.send(&ctl(Control::InviteReply { accepted: false }));
        link.finish();
        return Vec::new();
    }
    conn.kind = Kind::Friend;
    conn.announced = true;
    shared.release_unverified(&conn.unverified);
    let (id, key, name, addr) = (conn.peer_id.clone(), conn.remote_key, conn.name.clone(), conn.peer_addr.clone());
    // Replaces an unconfirmed introduction of this id (`register`): now a plain friend
    st.add_friend(&id, key);
    st.via.remove(&id);
    st.pending.remove(&id);
    st.pending_denied.remove(&id);
    st.paired_at.insert(id.clone(), Instant::now());
    st.trust_ip(&id, &addr, Instant::now());
    st.friend_links.insert(id.clone(), token);
    if st.pending_gone.remove(&id).is_none() {
        shared.emit(NetEvent::Connected { id: id.clone(), name });
    }
    st.conns.iter().filter(|(t, c)| **t != token && c.peer_id == id).map(|(_, c)| c.link.clone()).collect()
}

/// 짝짓기를 실패(끝)로 표시함. 이미 알리지 않았으면 한 번 알림
fn fail(shared: &Shared, p: &mut Pairing, id: &str, token: u64) {
    p.stage = PairStage::Failed;
    if !p.failure_reported {
        p.failure_reported = true;
        if p.surfaced {
            shared.emit(NetEvent::InviteAnswered { from: id.to_string(), accepted: false, pairing: token });
        }
    }
}

/// 짝짓기 마감(`invite_timeout`)이 지났는지 봄
fn expired(shared: &Shared, p: &Pairing, now: Instant) -> bool {
    now.duration_since(p.since) > shared.timing.invite_timeout
}

/// 초대자는 우리 난수에 바로 스스로 답함. SAS만 사람을 기다림.
/// 조용한 상대가 짝짓기 자리를 그 내내 차지하면 안 되므로 늦었는지 봄
fn reveal_overdue(shared: &Shared, p: &Pairing, now: Instant) -> bool {
    matches!(p.stage, PairStage::AwaitReveal { .. }) && now.duration_since(p.since) > shared.timing.conn_timeout.min(PRE_AUTH_MAX)
}

/// 들어온 짝짓기 시도를 기록함. 모든 한도 안이면 `true`임
fn admit_invite(st: &mut State, addr: &str, now: Instant) -> bool {
    st.invited_log.retain(|t| now.duration_since(*t) < INVITED_WINDOW);
    if st.pair_attempts.len() >= 256 && !st.pair_attempts.contains_key(addr) {
        st.pair_attempts.retain(|_, q| q.back().is_some_and(|t| now.duration_since(*t) < INVITED_WINDOW));
        if st.pair_attempts.len() >= 256 {
            let oldest = st.pair_attempts.iter().min_by_key(|(_, q)| q.back().copied()).map(|(a, _)| a.clone());
            if let Some(o) = oldest {
                st.pair_attempts.remove(&o);
            }
        }
    }
    let attempts = st.pair_attempts.entry(addr.to_string()).or_default();
    attempts.retain(|t| now.duration_since(*t) < INVITED_WINDOW);
    let addr_ok = attempts.len() < MAX_PAIR_ATTEMPTS_PER_ADDR;
    if attempts.len() < 2 * MAX_PAIR_ATTEMPTS_PER_ADDR {
        attempts.push_back(now);
    }
    let pairing = st.conns.values().filter(|c| !c.outgoing && pairing_active(c)).count();
    addr_ok && pairing < MAX_INVITES_IN && st.invited_log.len() < MAX_INVITED_PER_WINDOW
}

/// 등록된 링크의 메시지 하나를 처리함. `false`면 링크를 끊음
fn handle(shared: &Shared, ctx: &Ctx, id: &str, name: &str, msg: Message) -> bool {
    let now = Instant::now();
    let mut st = shared.state();
    let Some(conn) = st.conns.get(&ctx.token) else { return false };
    let (outgoing, hs_hash) = (conn.outgoing, conn.hs_hash);
    match msg {
        // A repeated hello on an authenticated friend link (same id) = the friend renamed itself.
        // Unchanged names are ignored; renames and asks share one budget per friend
        Message::Control(Control::Hello { id: hid, name: new_name, v, .. }) => {
            let new_name = clamp_name(&new_name);
            let friend = hid == id && matches!(conn.kind, Kind::Friend) && st.is_friend(id);
            let renamed = friend && conn.name != new_name;
            let Some(c) = st.conns.get_mut(&ctx.token) else { return false };
            if hid == id {
                c.v = v;
            }
            if renamed && st.budget(id, now).asks.take(now) {
                if let Some(c) = st.conns.get_mut(&ctx.token) {
                    c.name = new_name.clone();
                }
                shared.emit(NetEvent::Renamed { id: id.to_string(), name: new_name });
            }
            true
        }
        Message::Control(Control::Ping { ask: None, steer: None, toss: None }) => true,
        // A friend's app request, steering of its visiting pet and/or a thrown object (authenticated
        // friend links only, like visits). Budgeted per friend: reconnecting doesn't refill it
        Message::Control(Control::Ping { ask, steer, toss }) => {
            if !(matches!(conn.kind, Kind::Friend) && st.is_friend(id)) {
                return true;
            }
            let budget = st.budget(id, now);
            if let Some(ask) = ask.filter(|_| budget.asks.take(now)) {
                shared.emit(NetEvent::Asked { from: id.to_string(), ask: clamp_name(&ask) });
            }
            if let Some(steer) = steer.filter(|_| budget.steers.take(now)) {
                shared.emit(NetEvent::Steered { from: id.to_string(), steer });
            }
            if let Some(toss) = toss.filter(|_| budget.tosses.take(now)) {
                shared.emit(NetEvent::Tossed { from: id.to_string(), toss });
            }
            true
        }
        Message::Control(Control::Invite { pet, commit }) => {
            match conn.kind {
                Kind::Friend => return true,
                Kind::Stranger(Some(_)) => return false, // one pairing per link
                Kind::Stranger(None) if outgoing => return false, // invites only on links the inviter dialed
                Kind::Stranger(None) => {}
            }
            let Some(commit) = noise::hex32(&commit) else { return false };
            // A full friend list refuses (before any SAS is shown)
            if st.friends.len() >= MAX_FRIENDS
                || inbound_pairings_from(&st, &ctx.peer_addr) >= MAX_INVITES_IN_PER_ADDR
                || !admit_invite(&mut st, &ctx.peer_addr, now)
            {
                drop(st);
                ctx.link.send(&ctl(Control::InviteReply { accepted: false }));
                ctx.link.finish();
                return true;
            }
            let r_r = noise::random32();
            let conn = st.conns.get_mut(&ctx.token).expect("current conn");
            conn.kind = Kind::Stranger(Some(Pairing {
                stage: PairStage::AwaitReveal { commit, r_r, pet: clamp_name(&pet) },
                local_ok: false,
                remote_ok: false,
                since: now,
                failure_reported: false,
                surfaced: false,
            }));
            drop(st);
            ctx.link.send(&ctl(Control::PairRandom { random: noise::to_hex(&r_r) }))
        }
        Message::Control(Control::PairRandom { random }) => {
            let conn = st.conns.get_mut(&ctx.token).expect("current conn");
            let Kind::Stranger(Some(p)) = &mut conn.kind else { return false };
            let PairStage::AwaitRandom { nonce_i } = p.stage else { return false };
            let Some(r_r) = noise::hex32(&random) else { return false };
            p.stage = PairStage::Confirm;
            p.surfaced = true;
            shared.emit(NetEvent::InviteSent {
                to: id.to_string(),
                sas: noise::sas(&hs_hash, &nonce_i, &r_r),
                pairing: ctx.token,
            });
            drop(st);
            ctx.link.send(&ctl(Control::PairReveal { nonce: noise::to_hex(&nonce_i) }))
        }
        Message::Control(Control::PairReveal { nonce }) => {
            let conn = st.conns.get_mut(&ctx.token).expect("current conn");
            let Kind::Stranger(Some(p)) = &mut conn.kind else { return false };
            let PairStage::AwaitReveal { commit, r_r, pet } = &p.stage else { return false };
            let Some(nonce_i) = noise::hex32(&nonce) else { return false };
            if noise::commit(&nonce_i) != *commit {
                return false; // reveal does not match the commitment: abort pairing
            }
            let sas = noise::sas(&hs_hash, &nonce_i, r_r);
            let pet = pet.clone();
            p.stage = PairStage::Confirm;
            p.surfaced = true;
            ctx.surfaced.store(true, Ordering::SeqCst);
            st.invited_log.push_back(now);
            shared.emit(NetEvent::Invited { from: id.to_string(), name: name.to_string(), pet, sas, pairing: ctx.token });
            true
        }
        Message::Control(Control::InviteReply { accepted }) => {
            let conn = st.conns.get_mut(&ctx.token).expect("current conn");
            let Kind::Stranger(Some(p)) = &mut conn.kind else { return true };
            if matches!(p.stage, PairStage::Failed) {
                return true; // terminal: late replies change nothing
            }
            if !accepted {
                fail(shared, p, id, ctx.token);
                return false;
            }
            if !matches!(p.stage, PairStage::Confirm) || p.remote_ok {
                return false; // confirmation before the SAS exists
            }
            if expired(shared, p, now) {
                fail(shared, p, id, ctx.token);
                drop(st);
                ctx.link.send(&ctl(Control::InviteReply { accepted: false }));
                ctx.link.finish();
                return true;
            }
            p.remote_ok = true;
            let both = p.local_ok;
            shared.emit(NetEvent::InviteAnswered { from: id.to_string(), accepted: true, pairing: ctx.token });
            let to_close = if both { pin(shared, &mut st, ctx.token) } else { Vec::new() };
            drop(st);
            to_close.iter().for_each(|l| l.close());
            true
        }
        Message::Control(Control::Unfriend) => {
            if !matches!(conn.kind, Kind::Friend) {
                return true;
            }
            st.conns.get_mut(&ctx.token).expect("current conn").kind = Kind::Stranger(None);
            st.remove_friend(id);
            let dropped = crate::devices::drop_introduced(shared, &mut st, id);
            // Also drop any other friend link for this id (e.g. one that
            // registered while this message was in flight).
            let other = st.friend_links.remove(id).filter(|t| *t != ctx.token);
            let other = other.and_then(|t| st.conns.get_mut(&t)).map(|c| {
                c.kind = Kind::Stranger(None);
                c.link.clone()
            });
            drop(st);
            if let Some(l) = other {
                l.close();
            }
            crate::devices::close_dropped(dropped);
            false
        }
        // Friend lists between my computers and introductions: authenticated
        // friend links only, budgeted per friend; `devices` checks the rest
        Message::Control(Control::DeviceFriends { friends, want }) => {
            if matches!(conn.kind, Kind::Friend) && conn.dev.proven && st.is_friend(id) && st.budget(id, now).links.take(now) {
                crate::devices::on_device_friends(shared, &mut st, id, friends, want);
            }
            true
        }
        // Linking by code and proving the stored secret: friend links of
        // builds that know them only, budgeted per friend; `devices` checks the rest
        Message::Control(
            c @ (Control::DeviceCode { .. }
            | Control::DeviceCodeNonce { .. }
            | Control::DeviceCodeProof { .. }
            | Control::DeviceCodeOk { .. }
            | Control::DeviceNo { .. }
            | Control::DeviceNonce { .. }
            | Control::DeviceProof { .. }
            | Control::DeviceUnlink
            | Control::DeviceConfirm
            | Control::DeviceActive { .. }
            | Control::DeviceState { .. }),
        ) => {
            // (shared pets have their own bucket: they never starve a claim)
            let state = matches!(c, Control::DeviceState { .. });
            if !(matches!(conn.kind, Kind::Friend) && conn.v >= DEVICES_MIN_VERSION && st.is_friend(id) && st.budget(id, now).device(state, now)) {
                return true;
            }
            let replies = crate::devices::on_link_message(shared, &mut st, ctx.token, id, c);
            drop(st);
            for r in replies {
                ctx.link.send(&ctl(r));
            }
            true
        }
        Message::Control(Control::MyDevices { devices }) => {
            if matches!(conn.kind, Kind::Friend) && st.is_friend(id) && st.budget(id, now).links.take(now) {
                crate::devices::on_my_devices(shared, &mut st, id, devices);
            }
            true
        }
        Message::Visit(v) => {
            if matches!(conn.kind, Kind::Friend) && st.is_friend(id) && st.budget(id, now).visits.take(now) {
                shared.emit(NetEvent::Received { from: id.to_string(), msg: v });
            }
            true
        }
        // Like visits: friend links only (ignored otherwise)
        Message::Ladder(m) => {
            if matches!(conn.kind, Kind::Friend) && st.is_friend(id) && st.budget(id, now).ladder.take(now) {
                shared.emit(NetEvent::Ladder { from: id.to_string(), msg: m });
            }
            true
        }
    }
}

/// 짝짓기 마감과 놀고 있는 낯선 링크를 확인함. `false`면 링크를 끊음
fn timers(shared: &Shared, ctx: &Ctx) -> bool {
    let now = Instant::now();
    let mut st = shared.state();
    let Some(conn) = st.conns.get_mut(&ctx.token) else { return false };
    if conn.link.closing() {
        return true; // graceful close in progress; the session bounds it
    }
    let (since, id) = (conn.since, conn.peer_id.clone());
    match &mut conn.kind {
        Kind::Stranger(Some(p)) if !matches!(p.stage, PairStage::Failed) && (expired(shared, p, now) || reveal_overdue(shared, p, now)) => {
            fail(shared, p, &id, ctx.token);
            drop(st);
            ctx.link.send(&ctl(Control::InviteReply { accepted: false }));
            ctx.link.finish();
            true
        }
        // A stranger link with no invite has no business staying open.
        Kind::Stranger(None) => now.duration_since(since) <= shared.timing.conn_timeout,
        _ => true,
    }
}

/// 세션 정리임. 다이얼 기록은 상대가 주장한 id가 아니라 늘 다이얼한 id를 씀
fn cleanup(shared: &Shared, token: Option<u64>, dialed: Option<&str>) {
    let mut st = shared.state();
    if let Some(d) = dialed {
        st.dialing.remove(d);
        // An invite that never reached a surfaced pairing has no outcome.
        st.invites_out.remove(d);
    }
    let Some(conn) = token.and_then(|t| st.conns.remove(&t)) else { return };
    if conn.dev.confirming.is_some() {
        // 📱 the link went away before both confirmed: cancelled, nothing kept
        shared.emit(NetEvent::DeviceLinkCancelled { id: conn.peer_id.clone() });
    }
    if st.friend_links.get(&conn.peer_id) == token.as_ref() {
        st.friend_links.remove(&conn.peer_id);
    }
    if let Kind::Stranger(Some(p)) = &conn.kind {
        if !p.failure_reported && p.surfaced {
            if let Some(t) = token {
                shared.emit(NetEvent::InviteAnswered { from: conn.peer_id.clone(), accepted: false, pairing: t });
            }
        }
    }
    if conn.announced {
        if st.is_friend(&conn.peer_id) && !st.friend_links.contains_key(&conn.peer_id) {
            // Hold the Disconnected for a grace; see `State::pending_gone`.
            let until = Instant::now() + shared.timing.conn_timeout;
            st.pending_gone.insert(conn.peer_id, until);
        } else {
            st.pending_gone.remove(&conn.peer_id);
            shared.emit(NetEvent::Disconnected { id: conn.peer_id });
        }
    }
}

/// 유예가 끝난 `Disconnected` 이벤트를 내보냄 (틱마다 불림)
pub(crate) fn flush_gone(shared: &Shared) {
    let now = Instant::now();
    let mut st = shared.state();
    if st.pending_gone.is_empty() {
        return;
    }
    let due: Vec<String> = st.pending_gone.iter().filter(|(_, t)| now >= **t).map(|(id, _)| id.clone()).collect();
    for id in due {
        st.pending_gone.remove(&id);
        if !st.friend_links.contains_key(&id) {
            shared.emit(NetEvent::Disconnected { id });
        }
    }
}

/// `id`로의 다이얼이 시작도 연결도 못 함
pub(crate) fn dial_failed(shared: &Shared, id: &str) {
    let mut st = shared.state();
    st.dialing.remove(id);
    st.invites_out.remove(id);
}

/// 지금 `peer_id`와 짝짓기를 시작해도 되는지 봄. 늘 새 나가는 링크에서 함.
/// 이 id를 주장하는 들어온 링크(무단 점유자일 수 있음, 짝짓기 중이어도)는 막지도 재사용되지도 않음
fn can_invite(shared: &Shared, st: &State, peer_id: &str) -> bool {
    peer_id != shared.id
        && !st.invites_out.contains_key(peer_id)
        && (!st.is_friend(peer_id) || st.introduced_only(peer_id))
        && st.friends.len() < MAX_FRIENDS
        && !st.dialing.contains(peer_id)
        && !outbound_pairing(st, peer_id)
}

/// 알려진 후보 하나로 `peer_id`에게 짝짓기 초대를 시작함 (못 하거나 후보가 없으면 아무것도 안 함)
pub(crate) fn invite(shared: &Arc<Shared>, peer_id: &str, pet: &str) {
    let mut st = shared.state();
    if !can_invite(shared, &st, peer_id) {
        return;
    }
    let Some(target) = pick_candidate(&st, peer_id) else {
        return;
    };
    note_dial(&mut st, peer_id, target, Instant::now(), false);
    drop(st);
    invite_at(shared, peer_id, pet, target);
}

/// `target`(LAN 후보, 또는 친구 코드를 쓴 뒤의 릴레이)으로 새 나가는 링크를 열어 `peer_id`와 짝짓기를 시작함.
/// 지금 안 되면(나, 이미 친구, 진행 중) `false`임
pub(crate) fn invite_at(shared: &Arc<Shared>, peer_id: &str, pet: &str, target: transport::Candidate) -> bool {
    let mut st = shared.state();
    if !can_invite(shared, &st, peer_id) {
        return false;
    }
    st.invites_out.insert(peer_id.to_string(), clamp_name(pet));
    st.dialing.insert(peer_id.to_string());
    drop(st);
    transport::dial(shared, peer_id.to_string(), target);
    true
}

/// 사용자가 본 바로 그 짝짓기(`pairing`, 이벤트에서 옴)에 답함
pub(crate) fn answer_invite(shared: &Shared, pairing: u64, accept: bool) {
    let now = Instant::now();
    let mut st = shared.state();
    let token = pairing;
    let Some(conn) = st.conns.get_mut(&token) else { return };
    let link = conn.link.clone();
    let peer_id = conn.peer_id.clone();
    let peer_id = peer_id.as_str();
    let Kind::Stranger(Some(p)) = &mut conn.kind else { return };
    if !matches!(p.stage, PairStage::Confirm) || p.local_ok {
        return;
    }
    if !accept || expired(shared, p, now) {
        if !accept {
            p.failure_reported = true; // our own refusal needs no event
        }
        fail(shared, p, peer_id, token);
        drop(st);
        link.send(&ctl(Control::InviteReply { accepted: false }));
        link.finish();
        return;
    }
    p.local_ok = true;
    let to_close = if p.remote_ok { pin(shared, &mut st, token) } else { Vec::new() };
    drop(st);
    to_close.iter().for_each(|l| l.close());
    link.send(&ctl(Control::InviteReply { accepted: true }));
}
