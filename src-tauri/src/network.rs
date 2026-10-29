//! 친구 네트워크(screenpet-net)와 앱 사이 (spec phase 3 §1–§5).
//! - 시작: 설치 id·비밀키·친구 목록으로 `Net::start` (실패해도 앱은 동작 — 로그만)
//! - 네트워크 이벤트 → 게임 루프 입력 (`Input::Net`)으로 넘겨, 설정·슬롯은 게임 루프 스레드에서만 고친다
//! - 짝맞추기 창(`pair`): 양쪽에 이모지 6개를 보이고 두 사람이 모두 확인해야 친구가 된다
//! - 놀러 가기: 내 동물이 연결된 끝으로 나가면 `Visit`, 손님이 떠나면 `Leave`, 끊기면 바로 귀환

use std::collections::{BTreeMap, HashSet};
use std::sync::mpsc::Sender;
use std::thread;
use std::time::{Duration, Instant};

use screenpet_core::behavior::Personality;
use screenpet_core::brain::State;
use screenpet_core::care::Stage;
use screenpet_core::config::Config;
use screenpet_core::visit::{self, Side};
use screenpet_net::{Net, NetConfig, NetEvent, Visit};
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, WebviewUrl, WebviewWindow, WebviewWindowBuilder, WindowEvent};

use crate::game_loop::Input;
use crate::slots::{self, GuestInfo, Role, Slot};
use crate::tray::{FriendItem, FriendsMenu};
use crate::AppState;

/// 짝맞추기 창 라벨
pub const PAIR_LABEL: &str = "pair";
const PAIR_SIZE: (f64, f64) = (320.0, 168.0);
/// 초대를 보냈는데 이만큼 동안 InviteSent가 없으면 "연결하지 못했어요" (net은 그 전 실패를 알려 주지 않는다)
const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
/// 이모지를 보인 뒤 두 사람이 모두 확인해야 하는 시간 (net의 invite_timeout과 같다)
const CONFIRM_TIMEOUT: Duration = Duration::from_secs(20);
/// 결과("친구가 되었어요" / 실패)를 보여 준 뒤 창을 닫기까지
const RESULT_SECS: Duration = Duration::from_millis(2500);
/// "집에 와"를 보냈는데 Leave가 이만큼 안 오면 그냥 돌아온다
const COME_HOME_FALLBACK: Duration = Duration::from_secs(15);
/// 동시에 받는 손님 수 (친구마다 한 마리)
const MAX_GUESTS: usize = 8;
/// 표시 이름이 없을 때
const DEFAULT_NAME: &str = "친구";

/// 게임 루프로 들어오는 네트워크 관련 입력
pub enum NetInput {
    Event(NetEvent),
    /// 트레이: 발견된 사람에게 친구 신청
    Invite(String),
    /// 짝맞추기 창 버튼 (true = 똑같아요, 친구 맺기 / false = 달라요·취소·창 닫기)
    PairAnswer(bool),
    SetSide { id: String, side: Side },
    Unfriend(String),
    /// "다시 친구 맺기": 친구에서 지우고 새로 신청
    Repair(String),
    /// 트레이 "집에 와"
    ComeHome,
}

/// 짝맞추기 창에 보이는 상태 (`pair-state` 이벤트, `pair_state` 커맨드)
#[derive(Debug, Clone, Default, Serialize)]
pub struct PairView {
    /// "connecting" | "confirm" | "waiting" | "done" | "failed"
    pub phase: &'static str,
    /// 상대가 먼저 신청했음
    pub incoming: bool,
    /// 상대 표시 이름
    pub name: String,
    /// 상대 동물 이름 (받은 신청일 때)
    pub pet: String,
    /// 이모지 6개
    pub sas: Vec<String>,
    /// 상대가 이미 "똑같아요"를 눌렀음
    pub other_ok: bool,
    /// 실패 이유: "unreachable" | "declined" | "timeout" | "busy"
    pub reason: &'static str,
}

struct Pairing {
    peer: String,
    /// net의 짝맞추기 토큰 (InviteSent/Invited 뒤에 생긴다)
    token: Option<u64>,
    view: PairView,
    /// 지금 단계가 끝나는 때 (연결 대기 10초, 확인 20초, 결과 표시 2.5초)
    deadline: Instant,
}

/// 내 동물이 친구 화면에 가 있음
struct AwayVisit {
    friend: String,
    visit_id: u64,
    /// 나갔던 끝 (돌아올 때 여기서 들어온다)
    side: Side,
    /// "집에 와"를 보낸 뒤 그냥 돌아올 때
    come_home_by: Option<Instant>,
}

pub struct Network {
    net: Option<Net>,
    /// 내 표시 이름
    name: String,
    /// 발견된 사람 id → 이름 (친구 포함)
    seen: BTreeMap<String, String>,
    connected: HashSet<String>,
    /// 나를 더 이상 친구로 알아보지 못하는 친구
    forgot: HashSet<String>,
    pairing: Option<Pairing>,
    away: Option<AwayVisit>,
    last_menu: Option<FriendsMenu>,
}

fn to_net_side(s: Side) -> screenpet_net::Side {
    match s {
        Side::Left => screenpet_net::Side::Left,
        Side::Right => screenpet_net::Side::Right,
    }
}

fn from_net_side(s: screenpet_net::Side) -> Side {
    match s {
        screenpet_net::Side::Left => Side::Left,
        screenpet_net::Side::Right => Side::Right,
    }
}

fn opposite(s: Side) -> Side {
    match s {
        Side::Left => Side::Right,
        Side::Right => Side::Left,
    }
}

fn personality_name(p: Personality) -> &'static str {
    match p {
        Personality::Friendly => "friendly",
        Personality::Aloof => "aloof",
    }
}

fn clamp(s: &str) -> String {
    s.chars().take(64).collect()
}

/// Windows 사용자 이름 (spec §1). 없으면 "친구"
fn display_name() -> String {
    let name = std::env::var("USERNAME").or_else(|_| std::env::var("USER")).unwrap_or_default();
    let name = clamp(name.trim());
    if name.is_empty() {
        DEFAULT_NAME.to_string()
    } else {
        name
    }
}

/// 내 동물이 화면 끝으로 걸어 나가는 중
fn is_exiting(own: &Slot) -> bool {
    matches!(own.pet.state, State::Exit { .. })
}

impl Network {
    /// 네트워크를 시작한다. 설치 id가 새로 생기면 config를 고치므로 true를 돌려준다 (저장 필요).
    /// 비밀키는 config.json이 아니라 config 폴더의 `identity.key`에 있다.
    pub fn start(app: &AppHandle, config: &mut Config) -> (Network, bool) {
        let dirty = config.ensure_install_id(screenpet_net::new_install_id);
        let name = display_name();
        let mut network = Network {
            net: None,
            name: name.clone(),
            seen: BTreeMap::new(),
            connected: HashSet::new(),
            forgot: HashSet::new(),
            pairing: None,
            away: None,
            last_menu: None,
        };
        let state = app.state::<AppState>();
        let key = match screenpet_core::config::load_or_create_identity_key(&state.config_dir, Net::generate_static_key) {
            Ok(k) => k,
            Err(e) => {
                eprintln!("network disabled: cannot read or create the identity key ({e})");
                return (network, dirty);
            }
        };
        let friends = config.friends.iter().map(|f| screenpet_net::Friend { id: f.id.clone(), public_key: f.public_key }).collect();
        let id = config.install_id.clone().unwrap_or_default();
        match Net::start(NetConfig::new(id, name, key, friends)) {
            Ok((net, rx)) => {
                // 네트워크 이벤트를 게임 루프 입력으로 옮긴다 (Net이 없어지면 rx가 끝나 스레드도 끝난다)
                let tx: Sender<Input> = state.tx.lock().unwrap().clone();
                thread::spawn(move || {
                    for ev in rx {
                        if tx.send(Input::Net(NetInput::Event(ev))).is_err() {
                            break;
                        }
                    }
                });
                network.net = Some(net);
            }
            Err(e) => eprintln!("network disabled: {e}"),
        }
        (network, dirty)
    }

    fn is_connected(&self, id: &str) -> bool {
        self.connected.contains(id)
    }

    /// 내 동물이 친구 화면에 가 있거나 가는 중 — 하우스·동물 바꾸기를 막는다
    pub fn own_busy(&self, own: &Slot) -> bool {
        self.away.is_some() || own.pet.is_away() || is_exiting(own)
    }

    /// 매 틱 update 전에: 연결된 친구가 있는 끝으로만 나갈 수 있다. 하우스 안·가는 중·하우스 토글 중엔 나가지 않는다
    pub fn set_exit_edges(&self, config: &Config, own: &mut Slot) {
        let blocked = self.net.is_none() || config.housed || own.pet.is_housed() || own.pet.is_going_home();
        let edges = if blocked { Default::default() } else { config.exit_edges(|id| self.is_connected(id)) };
        own.pet.set_exit_edges(edges.left, edges.right);
    }

    /// 내 동물이 side 끝으로 다 나갔다: 그 끝의 연결된 친구 하나에게 Visit. 아무도 없으면(그새 끊김) 바로 다시 들어온다
    pub fn own_exited(&mut self, config: &Config, own: &mut Slot, side: Side) {
        let friends = config.friends_at(side, |id| self.is_connected(id));
        let (Some(net), false) = (&self.net, friends.is_empty()) else {
            own.pet.enter(&own.world, side);
            return;
        };
        let friend = friends[fastrand::usize(..friends.len())].id.clone();
        let visit_id = fastrand::u64(..);
        net.send(
            &friend,
            Visit::Visit {
                visit_id,
                species: own.entry.id.clone(),
                stage: own.shown.stage.to_string(),
                scale: own.shown.scale,
                personality: personality_name(own.entry.manifest.personality).to_string(),
                owner: self.name.clone(),
                enter_side: to_net_side(opposite(side)),
            },
        );
        eprintln!("own pet went to visit {friend} (visit {visit_id})");
        self.away = Some(AwayVisit { friend, visit_id, side, come_home_by: None });
    }

    /// 손님이 떠났다 (집 쪽 끝으로 걸어 나감 / 손 흔들고 사라짐): 주인에게 Leave
    pub fn guest_left(&self, guest: &GuestInfo) {
        let (Some(net), Ok(visit_id)) = (&self.net, guest.visit_id.parse::<u64>()) else { return };
        net.send(&guest.owner_id, Visit::Leave { visit_id });
    }

    /// 내 동물이 돌아온다 (나갔던 끝에서 걸어 들어온다)
    fn come_back(&mut self, own: &mut Slot) {
        if let Some(a) = self.away.take() {
            own.pet.enter(&own.world, a.side);
        }
    }

    /// 네트워크 입력 처리. config가 바뀌었으면 true (저장 필요)
    pub fn handle(&mut self, app: &AppHandle, input: NetInput, config: &mut Config, slots: &mut Vec<Slot>) -> bool {
        match input {
            NetInput::Event(ev) => self.on_event(app, ev, config, slots),
            NetInput::Invite(id) => {
                self.start_invite(app, &id, &slots[0].entry.id);
                false
            }
            NetInput::PairAnswer(accept) => {
                self.on_pair_answer(app, accept);
                false
            }
            NetInput::SetSide { id, side } => {
                // 이미 체크된 항목을 누르면 체크가 풀리므로 메뉴를 항상 다시 그린다
                self.last_menu = None;
                config.set_friend_side(&id, side)
            }
            NetInput::Unfriend(id) => self.unfriend(&id, config),
            NetInput::Repair(id) => {
                let dirty = self.unfriend(&id, config);
                self.start_invite(app, &id, &slots[0].entry.id);
                dirty
            }
            NetInput::ComeHome => {
                if let (Some(net), Some(a)) = (&self.net, self.away.as_mut()) {
                    net.send(&a.friend, Visit::ComeHome { visit_id: a.visit_id });
                    a.come_home_by.get_or_insert(Instant::now() + COME_HOME_FALLBACK);
                }
                false
            }
        }
    }

    fn unfriend(&mut self, id: &str, config: &mut Config) -> bool {
        if let Some(net) = &self.net {
            // 연결돼 있었다면 Disconnected가 뒤따라 손님 보내기·귀환을 처리한다
            net.unfriend(id);
        }
        self.forgot.remove(id);
        config.remove_friend(id)
    }

    fn on_event(&mut self, app: &AppHandle, ev: NetEvent, config: &mut Config, slots: &mut Vec<Slot>) -> bool {
        match ev {
            NetEvent::PeerSeen { id, name } => {
                self.seen.insert(id, clamp(&name));
            }
            NetEvent::PeerGone { id } => {
                self.seen.remove(&id);
            }
            NetEvent::Invited { from, name, pet, sas, pairing } => self.on_invited(app, from, name, pet, sas, pairing),
            NetEvent::InviteSent { to, sas, pairing } => self.on_invite_sent(app, &to, &sas, pairing),
            NetEvent::InviteAnswered { from: _, accepted, pairing } => self.on_invite_answered(app, accepted, pairing),
            NetEvent::Connected { id, name } => {
                self.connected.insert(id.clone());
                self.forgot.remove(&id);
                let name = clamp(&name);
                if let Some(p) = self.pairing.as_mut().filter(|p| p.peer == id && matches!(p.view.phase, "confirm" | "waiting")) {
                    p.view.phase = "done";
                    p.deadline = Instant::now() + RESULT_SECS;
                    self.show_pair(app);
                }
                return self.sync_friends(config, &id, &name);
            }
            NetEvent::Disconnected { id } => {
                self.connected.remove(&id);
                // 그 친구의 손님은 손 흔들고 사라진다
                for s in slots.iter_mut() {
                    if matches!(&s.role, Role::Guest(g) if g.owner_id == id) {
                        s.pet.go_home_now();
                    }
                }
                // 내 동물이 그 친구에게 가 있었으면 바로 돌아온다
                if self.away.as_ref().is_some_and(|a| a.friend == id) {
                    self.come_back(&mut slots[0]);
                }
                // 상대가 친구를 끊었으면(unfriend) net은 조용히 지운다 — 설정에서도 지운다
                if let Some(net) = &self.net {
                    if !net.friends().iter().any(|f| f.id == id) {
                        self.forgot.remove(&id);
                        return config.remove_friend(&id);
                    }
                }
            }
            NetEvent::Received { from, msg } => self.on_visit_message(app, config, slots, &from, msg),
            NetEvent::FriendForgotUs { id } => {
                self.forgot.insert(id);
            }
        }
        false
    }

    /// 짝맞추기가 끝나 net이 친구를 고정했으면 config에도 (공개키·이름) 적는다. 연결될 때마다 이름도 고친다
    fn sync_friends(&mut self, config: &mut Config, id: &str, name: &str) -> bool {
        let Some(net) = &self.net else { return false };
        let Some(f) = net.friends().into_iter().find(|f| f.id == id) else { return false };
        let name = if name.is_empty() { self.seen.get(id).cloned().unwrap_or_else(|| DEFAULT_NAME.to_string()) } else { name.to_string() };
        config.upsert_friend(id, &name, f.public_key)
    }

    fn on_visit_message(&mut self, app: &AppHandle, config: &Config, slots: &mut Vec<Slot>, from: &str, msg: Visit) {
        match msg {
            Visit::Visit { visit_id, species, stage, scale, owner, enter_side, .. } => {
                if let Err(e) = self.welcome_guest(app, config, slots, from, visit_id, &species, &stage, scale, &owner, from_net_side(enter_side)) {
                    // 받을 수 없는 손님은 바로 돌려보낸다 (주인 쪽에서 원래 동물이 돌아온다)
                    eprintln!("declined visit {visit_id} from {from}: {e}");
                    if let Some(net) = &self.net {
                        net.send(from, Visit::Leave { visit_id });
                    }
                }
            }
            Visit::Leave { visit_id } => {
                if self.away.as_ref().is_some_and(|a| a.friend == from && a.visit_id == visit_id) {
                    self.come_back(&mut slots[0]);
                }
            }
            Visit::ComeHome { visit_id } => {
                let id = visit_id.to_string();
                for s in slots.iter_mut() {
                    if matches!(&s.role, Role::Guest(g) if g.owner_id == from && g.visit_id == id) {
                        s.pet.go_home_now();
                    }
                }
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn welcome_guest(
        &self,
        app: &AppHandle,
        config: &Config,
        slots: &mut Vec<Slot>,
        from: &str,
        visit_id: u64,
        species: &str,
        stage: &str,
        scale: f64,
        owner: &str,
        side: Side,
    ) -> Result<(), String> {
        let state = app.state::<AppState>();
        let entry = state.pets.iter().find(|p| p.id == species).cloned().ok_or_else(|| format!("unknown species {species:?}"))?;
        let stage: Stage = serde_json::from_value(serde_json::Value::String(stage.to_string())).map_err(|_| format!("unknown stage {stage:?}"))?;
        let guests = slots.iter().filter(|s| !s.is_own());
        if guests.clone().count() >= MAX_GUESTS {
            return Err("too many guests".into());
        }
        if guests.into_iter().any(|s| matches!(&s.role, Role::Guest(g) if g.owner_id == from)) {
            return Err("already visiting".into());
        }
        let scale = visit::guest_scale(scale, stage.scale() * entry.manifest.size_mult(stage));
        let owner_name = config.friend(from).map(|f| f.name.clone()).unwrap_or_else(|| clamp(owner));
        let guest = GuestInfo { visit_id: visit_id.to_string(), owner_id: from.to_string(), owner_name };
        let seed = fastrand::u64(..);
        let slot = slots::spawn_guest(app, guest, entry, stage, scale, side, &slots[0].world, seed)?;
        eprintln!("guest {visit_id} from {from} arrived");
        slots.push(slot);
        Ok(())
    }

    // ---- 짝맞추기 ----

    fn pet_name(app: &AppHandle, species: &str) -> String {
        let state = app.state::<AppState>();
        state.pets.iter().find(|p| p.id == species).map(|p| p.manifest.name.clone()).unwrap_or_else(|| "동물".to_string())
    }

    fn start_invite(&mut self, app: &AppHandle, peer: &str, own_species: &str) {
        let Some(net) = &self.net else { return };
        if self.pairing.as_ref().is_some_and(|p| !matches!(p.view.phase, "done" | "failed")) {
            // 한 번에 하나 — 진행 중인 창을 다시 보인다
            self.show_pair(app);
            return;
        }
        net.invite(peer, own_species);
        let name = self.seen.get(peer).cloned().unwrap_or_else(|| DEFAULT_NAME.to_string());
        self.pairing = Some(Pairing {
            peer: peer.to_string(),
            token: None,
            view: PairView { phase: "connecting", name, ..PairView::default() },
            deadline: Instant::now() + CONNECT_TIMEOUT,
        });
        self.show_pair(app);
    }

    fn on_invite_sent(&mut self, app: &AppHandle, to: &str, sas: &str, token: u64) {
        match self.pairing.as_mut() {
            Some(p) if p.peer == to && p.token.is_none() && p.view.phase == "connecting" => {
                p.token = Some(token);
                p.view.phase = "confirm";
                p.view.sas = sas.split(' ').map(str::to_string).collect();
                p.deadline = Instant::now() + CONFIRM_TIMEOUT;
                self.show_pair(app);
            }
            // 취소했거나 다른 신청 중 — 이 짝맞추기는 거절
            _ => {
                if let Some(net) = &self.net {
                    net.answer_invite(token, false);
                }
            }
        }
    }

    fn on_invited(&mut self, app: &AppHandle, from: String, name: String, pet: String, sas: String, token: u64) {
        let Some(net) = &self.net else { return };
        if self.pairing.as_ref().is_some_and(|p| !matches!(p.view.phase, "done" | "failed")) {
            // 한 번에 하나만 — 나머지는 거절
            net.answer_invite(token, false);
            return;
        }
        let name = clamp(&name);
        let name = if name.is_empty() { DEFAULT_NAME.to_string() } else { name };
        self.pairing = Some(Pairing {
            peer: from,
            token: Some(token),
            view: PairView {
                phase: "confirm",
                incoming: true,
                name,
                pet: Self::pet_name(app, &pet),
                sas: sas.split(' ').map(str::to_string).collect(),
                ..PairView::default()
            },
            deadline: Instant::now() + CONFIRM_TIMEOUT,
        });
        self.show_pair(app);
    }

    fn on_invite_answered(&mut self, app: &AppHandle, accepted: bool, token: u64) {
        let Some(p) = self.pairing.as_mut().filter(|p| p.token == Some(token)) else { return };
        if !matches!(p.view.phase, "confirm" | "waiting") {
            return;
        }
        if accepted {
            // 상대가 확인했다 — 나도 확인했으면 곧 Connected가 온다
            p.view.other_ok = true;
        } else {
            p.view.phase = "failed";
            p.view.reason = "declined";
            p.deadline = Instant::now() + RESULT_SECS;
        }
        self.show_pair(app);
    }

    fn on_pair_answer(&mut self, app: &AppHandle, accept: bool) {
        let Some(p) = self.pairing.as_mut() else {
            Self::hide_pair(app);
            return;
        };
        if accept {
            if p.view.phase == "confirm" {
                if let (Some(net), Some(t)) = (&self.net, p.token) {
                    net.answer_invite(t, true);
                }
                p.view.phase = "waiting";
                self.show_pair(app);
            }
            return;
        }
        // 달라요·취소·창 닫기: 확인 단계였다면 거절을 보낸다 (연결 중이었다면 뒤늦은 InviteSent를 거절한다)
        if matches!(p.view.phase, "confirm" | "waiting") {
            if let (Some(net), Some(t)) = (&self.net, p.token) {
                net.answer_invite(t, false);
            }
        }
        self.pairing = None;
        Self::hide_pair(app);
    }

    /// 짝맞추기 시간 재기
    fn tick_pairing(&mut self, app: &AppHandle, now: Instant) {
        let Some(p) = self.pairing.as_mut() else { return };
        if now < p.deadline {
            return;
        }
        match p.view.phase {
            "connecting" => {
                p.view.phase = "failed";
                p.view.reason = "unreachable";
                p.deadline = now + RESULT_SECS;
                self.show_pair(app);
            }
            "confirm" | "waiting" => {
                if let (Some(net), Some(t)) = (&self.net, p.token) {
                    net.answer_invite(t, false);
                }
                p.view.phase = "failed";
                p.view.reason = "timeout";
                p.deadline = now + RESULT_SECS;
                self.show_pair(app);
            }
            _ => {
                self.pairing = None;
                Self::hide_pair(app);
            }
        }
    }

    /// 짝맞추기 창에 지금 상태를 보이고 앞으로 가져온다 (처음이면 만든다 — 게임 루프 스레드에서만)
    fn show_pair(&self, app: &AppHandle) {
        let Some(p) = &self.pairing else { return };
        *app.state::<AppState>().pair.lock().unwrap() = p.view.clone();
        let window = match app.get_webview_window(PAIR_LABEL) {
            Some(w) => w,
            None => match build_pair_window(app) {
                Ok(w) => w,
                Err(e) => {
                    eprintln!("failed to create pairing window: {e}");
                    return;
                }
            },
        };
        let _ = app.emit_to(PAIR_LABEL, "pair-state", p.view.clone());
        if !window.is_visible().unwrap_or(false) {
            let _ = window.center();
            let _ = window.show();
            let _ = window.set_focus();
        }
    }

    fn hide_pair(app: &AppHandle) {
        if let Some(w) = app.get_webview_window(PAIR_LABEL) {
            let _ = w.hide();
        }
    }

    /// 매 틱: 짝맞추기 시간, "집에 와" 대비, 트레이 `친구 화면` 메뉴
    pub fn tick(&mut self, app: &AppHandle, now: Instant, config: &Config, own: &mut Slot) {
        self.tick_pairing(app, now);
        if self.away.as_ref().and_then(|a| a.come_home_by).is_some_and(|t| now >= t) {
            eprintln!("no answer to come-home; coming back anyway");
            self.come_back(own);
        }
        let menu = self.menu_model(config, own);
        if self.last_menu.as_ref() != Some(&menu) {
            crate::tray::fill_friends(app, &menu);
            self.last_menu = Some(menu);
        }
    }

    fn menu_model(&self, config: &Config, own: &Slot) -> FriendsMenu {
        let strangers = self.seen.iter().filter(|(id, _)| config.friend(id).is_none()).map(|(id, name)| (id.clone(), name.clone())).collect();
        let friends = config
            .friends
            .iter()
            .map(|f| FriendItem {
                id: f.id.clone(),
                name: f.name.clone(),
                side: f.side,
                connected: self.is_connected(&f.id),
                forgot: self.forgot.contains(&f.id),
            })
            .collect();
        FriendsMenu { available: self.net.is_some(), strangers, friends, away: self.away.is_some() && own.pet.is_away() }
    }
}

/// 짝맞추기 창: 작고 테두리 있는 창, 항상 위. 닫기(X)는 숨기기 + 취소로 바꾼다 (창은 다시 쓴다)
fn build_pair_window(app: &AppHandle) -> tauri::Result<WebviewWindow> {
    let window = WebviewWindowBuilder::new(app, PAIR_LABEL, WebviewUrl::App("pair.html".into()))
        .title("ScreenPet 친구 맺기")
        .inner_size(PAIR_SIZE.0, PAIR_SIZE.1)
        .resizable(false)
        .maximizable(false)
        .minimizable(false)
        .always_on_top(true)
        .skip_taskbar(false)
        .center()
        .visible(false)
        .build()?;
    let handle = app.clone();
    window.on_window_event(move |ev| {
        if let WindowEvent::CloseRequested { api, .. } = ev {
            api.prevent_close();
            let _ = handle.state::<AppState>().tx.lock().unwrap().send(Input::Net(NetInput::PairAnswer(false)));
        }
    });
    Ok(window)
}
