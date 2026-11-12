//! QA용 헤드리스 사다리 참가자임 (spec 2026-11-04 §7). 앱은 한 PC에 하나만 뜨므로(single-instance) 이 봇 여러 개로
//! 3~10명 판을 시험함.
//!
//! ```sh
//! cargo run -p screenpet-net --example ladder_bot -- --dir /tmp/bot1 --name 봇1
//! cargo run -p screenpet-net --example ladder_bot -- --dir /tmp/bot2 --name 봇2 --relay   # 친구 코드로
//! ```
//!
//! - 상태 폴더(`--dir`)에 id·키·친구를 저장함: 다시 켜도 같은 친구.
//! - 발견 포트는 아무 포트(0)를 열고 공지만 47321로 보냄: 같은 PC의 앱이 봇을 보고 친구 신청을 걸 수 있음
//!   (앱이 47321을 쥐고 있어 봇이 그 포트를 열 수 없음). 다른 PC의 앱도 같은 네트워크면 봄.
//! - 친구 신청은 SAS 이모지를 찍고 자동으로 확인함. 사다리 초대는 자동으로 참가함.
//! - `--decline` 거절, `--busy` 바쁨 거절, `--drop-after-lock` 잠그기를 받으면 공개하지 않고 바로 꺼짐 (끊김 흉내).
//! - 끝나면 시드·지문 이모지·결과 표를 찍음 (앱 화면과 맞춰 봄).
//! - **봇이 방장**: 콘솔에 `i` + Enter → 연결된 친구 모두에게 초대 (당첨 1 + 꽝), 모두 답하면 저절로 시작.
//!   `i 2` = 당첨 2개, `s` = 지금 온 사람만으로 시작, `q` = 그만두기, `f` = 친구 목록, `c` = 내 친구 코드.
//! - **봇끼리 친구**: 둘 다 `--relay`로 켜고 한쪽 콘솔에 `a <다른 봇의 친구 코드>` (예: `a Z6ES-676D`): 양쪽 다 자동 확인.

use std::collections::{HashMap, HashSet};
use std::io::BufRead;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use screenpet_core::ladder::{build, disambiguate, permutation, print_indices, trace};
use screenpet_net::ladder::{DeclineReason, GuestGame, HostGame, LockedResult, Phase, Seat};
use screenpet_net::{hex32, new_install_id, normalize_code, parse_relay_key, to_hex, Friend, LadderMsg, Net, NetConfig, NetEvent, DEFAULT_RELAY_KEY, DEFAULT_RELAY_URL, SAS_EMOJI};
use serde::{Deserialize, Serialize};

/// 명령줄 옵션임
#[derive(Default)]
struct Args {
    dir: Option<PathBuf>,
    name: String,
    pet: String,
    relay: bool,
    decline: bool,
    busy: bool,
    drop_after_lock: bool,
}

/// 명령줄을 읽음 (모르는 옵션은 무시)
fn args() -> Args {
    let mut a = Args { name: "봇".into(), pet: "cat".into(), ..Default::default() };
    let mut it = std::env::args().skip(1);
    while let Some(x) = it.next() {
        match x.as_str() {
            "--dir" => a.dir = it.next().map(PathBuf::from),
            "--name" => a.name = it.next().unwrap_or(a.name),
            "--pet" => a.pet = it.next().unwrap_or(a.pet),
            "--relay" => a.relay = true,
            "--decline" => a.decline = true,
            "--busy" => a.busy = true,
            "--drop-after-lock" => a.drop_after_lock = true,
            other => {
                eprintln!("모르는 옵션 {other}\n쓰는 법: ladder_bot --dir <폴더> [--name 이름] [--pet cat] [--relay] [--decline|--busy|--drop-after-lock]");
                std::process::exit(2);
            }
        }
    }
    if a.dir.is_none() {
        eprintln!("--dir <상태 폴더>가 필요해요");
        std::process::exit(2);
    }
    a
}

/// 상태 폴더의 `bot.json` 내용임: id, 비밀키(hex), 친구 (id, 공개키 hex)
#[derive(Serialize, Deserialize)]
struct State {
    id: String,
    key: String,
    friends: Vec<(String, String)>,
}

/// 저장된 비밀키를 읽음. 깨졌으면 None임
fn key_of(st: &State) -> Option<[u8; 32]> {
    hex32(&st.key)
}

/// `bot.json`에는 비밀키가 들어 있음: 남이 못 읽게 잠금 (Windows는 폴더의 ACL을 따름)
#[cfg(unix)]
fn lock_state(path: &Path) -> std::io::Result<()> {
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))
}

/// Unix 밖에서는 아무것도 안 함 (폴더 ACL을 따름)
#[cfg(not(unix))]
fn lock_state(_path: &Path) -> std::io::Result<()> {
    Ok(())
}

/// 주인만 읽을 수 있게 상태 파일을 만듦
#[cfg(unix)]
fn open_state(path: &Path) -> std::io::Result<std::fs::File> {
    use std::os::unix::fs::OpenOptionsExt;
    std::fs::OpenOptions::new().write(true).create(true).truncate(true).mode(0o600).open(path)
}

/// 상태 파일을 만듦
#[cfg(not(unix))]
fn open_state(path: &Path) -> std::io::Result<std::fs::File> {
    std::fs::File::create(path)
}

/// 상태를 읽음. 없으면 새 id·키로 만들어 저장함
fn load(dir: &Path) -> State {
    let path = dir.join("bot.json");
    if let Some(st) = std::fs::read(&path).ok().and_then(|b| serde_json::from_slice(&b).ok()) {
        lock_state(&path).expect("lock state");
        return st;
    }
    std::fs::create_dir_all(dir).expect("state dir");
    let st = State { id: new_install_id(), key: to_hex(&Net::generate_static_key()), friends: Vec::new() };
    save(dir, &st);
    st
}

/// 상태를 저장함
fn save(dir: &Path, st: &State) {
    use std::io::Write;
    let path = dir.join("bot.json");
    // 이미 있는 파일은 mode가 듣지 않으므로 쓰기 전에 잠금
    if path.exists() {
        lock_state(&path).expect("lock state");
    }
    let mut f = open_state(&path).expect("open state");
    f.write_all(&serde_json::to_vec_pretty(st).expect("json")).expect("write state");
}

/// 방장 메시지들을 각 친구에게 보냄
fn send_all(net: &Net, out: Vec<(String, LadderMsg)>) {
    for (to, m) in out {
        net.send_ladder(&to, m);
    }
}

/// 지문 비트를 이모지 넷으로 바꿈
fn emoji(print: u32) -> String {
    print_indices(print).iter().map(|&i| SAS_EMOJI[i]).collect::<Vec<_>>().join("")
}

/// 봇 본체임: 네트워크를 띄우고 친구 신청·사다리 메시지에 자동으로 답하며 콘솔 명령을 받음
fn main() {
    let a = args();
    let dir = a.dir.clone().expect("checked");
    let mut st = load(&dir);
    let Some(key) = key_of(&st) else {
        eprintln!("{}의 key가 hex 64자가 아니에요 — 파일을 지우면 새 봇으로 시작해요", dir.join("bot.json").display());
        std::process::exit(2);
    };
    let friends = st.friends.iter().filter_map(|(id, k)| Some(Friend { id: id.clone(), public_key: hex32(k)? })).collect();
    let mut cfg = NetConfig::new(st.id.clone(), a.name.clone(), key, friends);
    cfg.discovery_port = 0;
    cfg.broadcast_to = SocketAddr::from(([255, 255, 255, 255], 47321));
    if a.relay {
        cfg.relay_url = Some(DEFAULT_RELAY_URL.to_string());
        cfg.relay_key = parse_relay_key(DEFAULT_RELAY_KEY);
    }
    let (net, rx) = Net::start(cfg).expect("start net");
    println!("🤖 {} 시작 (id {}, 친구 {}명). 앱 트레이 `친구랑 놀기 ▸`에서 친구 신청하세요.", a.name, st.id, st.friends.len());

    println!("   방장이 되려면 친구가 연결된 뒤 `i` + Enter (`i 2` = 당첨 2개, `s` 시작, `q` 그만두기, `f` 친구, `c` 친구 코드, `a <코드>` 친구 추가)");

    // 콘솔 명령 (방장 모드)
    let (cmd_tx, cmd_rx) = std::sync::mpsc::channel::<String>();
    std::thread::spawn(move || {
        for line in std::io::stdin().lock().lines().map_while(Result::ok) {
            if cmd_tx.send(line.trim().to_string()).is_err() {
                break;
            }
        }
    });

    let mut names: HashMap<String, String> = HashMap::new();
    let mut online: HashSet<String> = HashSet::new();
    let mut code: Option<String> = None;
    let mut game: Option<GuestGame> = None;
    let mut host: Option<(HostGame, usize)> = None;
    let mut started = false;
    let mut shown: Option<Phase> = None;
    let me = Seat { name: a.name.clone(), pet: a.pet.clone(), style: String::new() };
    loop {
        while let Ok(cmd) = cmd_rx.try_recv() {
            let mut words = cmd.split_whitespace();
            match words.next() {
                Some("i") => {
                    if host.as_ref().is_some_and(|(h, _)| !h.phase().is_over()) || game.as_ref().is_some_and(|g| !g.phase().is_over()) {
                        println!("이미 판이 있어요");
                        continue;
                    }
                    let winners: usize = words.next().and_then(|w| w.parse().ok()).unwrap_or(1).max(1);
                    let friends: Vec<String> = online.iter().filter(|id| net.peer_version(id).is_some_and(|v| v >= screenpet_net::LADDER_MIN_VERSION)).cloned().collect();
                    let Some((h, out)) = HostGame::new(me.clone(), &preset(friends.len() + 1, winners), &friends, Instant::now()) else {
                        println!("초대할 친구가 없어요 (연결된 새 판 친구 {}명)", friends.len());
                        continue;
                    };
                    send_all(&net, out);
                    println!("📨 {}명에게 초대를 보냈어요 (당첨 {winners}개)", friends.len());
                    host = Some((h, winners));
                    started = false;
                    shown = None;
                }
                Some("s") => {
                    if let Some((h, w)) = host.as_mut() {
                        match h.start(&preset(h.joined() + 1, *w), Instant::now()) {
                            Some(out) => {
                                send_all(&net, out);
                                started = true;
                            }
                            None => println!("아직 시작할 수 없어요 (참가 {}명)", h.joined()),
                        }
                    }
                }
                Some("q") => {
                    if let Some((h, _)) = host.as_mut() {
                        send_all(&net, h.abort());
                    }
                }
                Some("f") => {
                    for id in &online {
                        println!("   ● {} (판 v{})", names.get(id).map_or(id.as_str(), String::as_str), net.peer_version(id).unwrap_or(0));
                    }
                    if online.is_empty() {
                        println!("   연결된 친구가 없어요");
                    }
                }
                Some("c") => println!("🔑 내 친구 코드: {}", code.as_deref().unwrap_or("(아직 없음 — --relay로 켰는지 확인)")),
                Some("a") => match words.next().and_then(normalize_code) {
                    Some(c) if a.relay => {
                        println!("💞 친구 코드 {c}로 친구 신청…");
                        net.redeem_code(&c, &a.pet);
                    }
                    Some(_) => println!("친구 코드는 중계 서버로 맺어요 — --relay로 다시 켜 주세요"),
                    None => println!("쓰는 법: a ABCD-1234"),
                },
                Some(other) => println!("모르는 명령 {other} (i / s / q / f / c / a <코드>)"),
                None => {}
            }
        }
        let ev = rx.recv_timeout(Duration::from_millis(200));
        let now = Instant::now();
        match ev {
            Ok(NetEvent::Invited { name, sas, pairing, .. }) => {
                println!("💞 {name}의 친구 신청 — 이모지 {sas} (자동으로 확인)");
                net.answer_invite(pairing, true);
            }
            // 내가 건 친구 신청 (`a <코드>`) — 이쪽도 자동으로 확인
            Ok(NetEvent::InviteSent { sas, pairing, .. }) => {
                println!("💞 친구 신청 보냄 — 이모지 {sas} (자동으로 확인)");
                net.answer_invite(pairing, true);
            }
            Ok(NetEvent::InviteAnswered { accepted: false, .. }) => println!("💔 친구 맺기가 취소됐어요"),
            Ok(NetEvent::CodeFailed { code, reason }) => println!("💔 친구 코드 {code}를 쓸 수 없어요 ({reason})"),
            Ok(NetEvent::Connected { id, name }) => {
                println!("● {name} 연결됨 (판 v{})", net.peer_version(&id).unwrap_or(0));
                online.insert(id.clone());
                names.insert(id, name);
                st.friends = net.friends().iter().map(|f| (f.id.clone(), to_hex(&f.public_key))).collect();
                save(&dir, &st);
            }
            Ok(NetEvent::Disconnected { id }) => {
                println!("○ {} 끊김", names.get(&id).map_or(id.as_str(), String::as_str));
                online.remove(&id);
                if let Some((h, _)) = host.as_mut() {
                    send_all(&net, h.on_disconnected(&id, Instant::now()));
                }
                if let Some(g) = game.as_mut().filter(|g| g.host_id() == id) {
                    g.on_host_disconnected();
                }
            }
            Ok(NetEvent::FriendCode { code: c }) => {
                println!("🔑 내 친구 코드: {c}  (앱 트레이 `친구랑 놀기 ▸ 친구 코드 입력…`에 넣으세요)");
                code = Some(c);
            }
            Ok(NetEvent::Ladder { from, msg: msg @ LadderMsg::LadderInvite { .. } }) => {
                let Some(mut g) = GuestGame::on_invite(&from, msg, now) else {
                    println!("이상한 초대는 무시");
                    continue;
                };
                if game.as_ref().is_some_and(|g| !g.phase().is_over()) || host.as_ref().is_some_and(|(h, _)| !h.phase().is_over()) {
                    if let Some(m) = g.decline(DeclineReason::Playing) {
                        net.send_ladder(&from, m);
                    }
                    continue;
                }
                println!("🎲 {}의 사다리타기 초대 (초대 {}명)", g.host_name(), g.invited());
                if a.busy || a.decline {
                    let reason = if a.busy { DeclineReason::Busy } else { DeclineReason::No };
                    if let Some(m) = g.decline(reason) {
                        net.send_ladder(&from, m);
                    }
                    println!("🙅 거절했어요 ({})", reason.wire());
                    continue;
                }
                if let Some(m) = g.join(me.clone(), now) {
                    net.send_ladder(&from, m);
                }
                game = Some(g);
            }
            Ok(NetEvent::Ladder { from, msg }) => {
                if let Some((h, _)) = host.as_mut() {
                    let answer = matches!(&msg, LadderMsg::LadderJoin { .. } | LadderMsg::LadderDecline { .. });
                    send_all(&net, h.on_message(&from, msg, now));
                    if answer {
                        for (id, ans) in h.answers() {
                            println!("   {} → {ans:?}", names.get(&id).map_or(id.as_str(), String::as_str));
                        }
                    }
                    continue;
                }
                let Some(g) = game.as_mut().filter(|g| g.host_id() == from) else { continue };
                if a.drop_after_lock && matches!(msg, LadderMsg::LadderLock { .. }) {
                    println!("💥 잠그기를 받았어요 — 공개하지 않고 꺼집니다 (--drop-after-lock)");
                    net.shutdown();
                    return;
                }
                if let LadderMsg::LadderStart { mode, .. } = &msg {
                    println!("▶ 방장이 진행 시작 ({mode})");
                }
                if let Some(reply) = g.on_message(msg, now) {
                    net.send_ladder(&from, reply);
                }
            }
            Ok(_) => {}
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {}
            Err(_) => return,
        }
        if let Some((h, w)) = host.as_mut() {
            send_all(&net, h.tick(now));
            // 모두 답하면 저절로 시작 (미리 채운 칸이라 인원에 맞춰 다시 만듦)
            if !started && h.all_answered() && h.joined() > 0 && *h.phase() == Phase::Inviting {
                if let Some(out) = h.start(&preset(h.joined() + 1, *w), now) {
                    send_all(&net, out);
                    started = true;
                }
            }
            if shown.as_ref() != Some(h.phase()) {
                report(h.phase());
                // 끝나면 참가자에게 진행 시작 신호 (앱과 같음)
                if let Phase::Done(o) = h.phase() {
                    let msg = LadderMsg::LadderStart { game_id: h.game_id(), mode: "one_by_one".into() };
                    for p in &o.players {
                        if let Some(id) = h.friend_of(&p.token) {
                            net.send_ladder(&id, msg.clone());
                        }
                    }
                }
                shown = Some(h.phase().clone());
            }
            if h.phase().is_over() {
                host = None;
                shown = None;
            }
        } else if let Some(g) = game.as_mut() {
            if let Some(m) = g.tick(now) {
                net.send_ladder(&g.host_id().to_string(), m);
            }
            if shown.as_ref() != Some(g.phase()) {
                report(g.phase());
                shown = Some(g.phase().clone());
            }
            if g.phase().is_over() {
                game = None;
                shown = None;
            }
        }
    }
}

/// 당첨 w개 + 꽝 나머지 결과 칸을 만듦 (당첨은 인원을 넘지 않음)
fn preset(count: usize, winners: usize) -> Vec<LockedResult> {
    (0..count)
        .map(|i| if i < winners.min(count) { LockedResult { text: "당첨".into(), mood: "good".into() } } else { LockedResult { text: "꽝".into(), mood: "bad".into() } })
        .collect()
}

/// 게임 단계를 콘솔에 찍음
fn report(phase: &Phase) {
    match phase {
        Phase::Done(o) => {
            let mut s = o.stream();
            let mut next = || s.next_u32();
            let perm = permutation(o.players.len(), &mut next);
            let ladder = build(&perm, &mut next);
            let names = disambiguate(&o.players.iter().map(|p| p.name.clone()).collect::<Vec<_>>());
            println!("✅ 결과 — 지문 {} · 시드 {}", emoji(o.print), to_hex(&o.seed));
            for (i, name) in names.iter().enumerate() {
                let end = *trace(&ladder, i).last().expect("path");
                let mark = if i == o.me { " (나)" } else if i == o.host { " (방장)" } else { "" };
                println!("   {name}{mark} → {} [{}]", o.results[end].text, o.results[end].mood);
            }
        }
        Phase::Cancelled { reason, who } => println!("✖ 취소됨 ({reason:?}{})", who.as_ref().map(|w| format!(", {w}")).unwrap_or_default()),
        Phase::Locked => println!("🔒 잠겼어요 — 공개 기다리는 중"),
        Phase::Joined => println!("🙋 참가함"),
        Phase::Inviting => println!("⏳ 답 기다리는 중 (모두 답하면 시작, `s` = 지금 시작)"),
        _ => {}
    }
}

/// 상태 파일 권한과 깨진 키 처리 시험임
#[cfg(test)]
mod tests {
    use super::*;

    /// 시험용 빈 임시 폴더를 만듦
    fn temp_dir(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("screenpet-bot-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[cfg(unix)]
    #[test]
    fn the_state_file_with_the_key_is_private() {
        use std::os::unix::fs::PermissionsExt;
        let d = temp_dir("perms");
        let mode = |d: &Path| std::fs::metadata(d.join("bot.json")).unwrap().permissions().mode() & 0o777;
        let st = load(&d);
        assert_eq!(mode(&d), 0o600, "created private");
        // 예전 봇이 남이 읽을 수 있게 만든 파일도 다시 저장하거나 읽을 때 잠금
        std::fs::set_permissions(d.join("bot.json"), std::fs::Permissions::from_mode(0o644)).unwrap();
        assert_eq!(load(&d).key, st.key);
        assert_eq!(mode(&d), 0o600, "locked on load");
        std::fs::set_permissions(d.join("bot.json"), std::fs::Permissions::from_mode(0o644)).unwrap();
        save(&d, &st);
        assert_eq!(mode(&d), 0o600, "locked on save");
        std::fs::remove_dir_all(&d).unwrap();
    }

    #[test]
    fn a_broken_key_is_an_error_not_a_panic_or_a_zero_key() {
        let st = |key: &str| State { id: "x".into(), key: key.into(), friends: Vec::new() };
        assert_eq!(key_of(&st(&"ab".repeat(32))), Some([0xab; 32]));
        assert_eq!(key_of(&st(&format!("{}x", "가".repeat(21)))), None, "64 bytes, not on char boundaries");
        assert_eq!(key_of(&st(&"zz".repeat(32))), None, "not hex: used to become an all-zero key");
        assert_eq!(key_of(&st("ab")), None);
    }
}
