//! 서버 전용 명령임: `screenpet-relay admin <command>`가 0700 데이터 폴더의 Unix 소켓 `admin.sock`으로
//! 돌고 있는 릴레이와 말하므로 릴레이 자신의 사용자만 닿음. 연결마다 낱말 JSON 배열 하나를 보내고
//! `{"ok": bool, "text": string}`로 답받음. 명령은 하나씩 돎. 명령 처리는 `Hub`의 `admin_*`가 함.

use crate::hub::Hub;
use serde_json::json;
use std::io::{self, BufRead, BufReader, Read, Write};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

/// 데이터 폴더 안 소켓 이름
const SOCKET: &str = "admin.sock";
/// 소켓 읽기·쓰기 시간 제한
const TIMEOUT: Duration = Duration::from_secs(5);

/// `admin` 도움말 글
pub const USAGE: &str = "\
commands:
  status                        who is online, totals, switches, counters
  kick <id-or-key-prefix>       disconnect (it may come straight back)
  ban <id-or-key-prefix>        disconnect and refuse from now on
  ban-ip <ip>                   the same for an address (IPv6: its /64)
  unban <id-or-key-prefix|ip>   lift a ban
  bans                          list bans
  registration on|off           off: new installs are refused
  friendcode on|off             off: no friend codes issued or redeemed
  relay on|off                  off: everyone disconnected and refused
  purge <id-or-key-prefix>      forget an install (disconnects it if online)
  purge-idle <days> [--yes]     forget offline installs unseen for that long
  purge-islands <min> [--yes]   forget friend groups of at least <min> installs
                                with no one online and no friend outside
  mailbox                       feedback letters (count, unread) and mailbox readers
  mailbox-reader add|remove <id-or-key-prefix>
                                allow / stop an install listing the feedback letters
Without --yes the purge-* commands only list what they would remove.";

/// 데이터 폴더 안 관리 소켓 경로를 돌려줌
pub(crate) fn socket_path(dir: &Path) -> PathBuf {
    dir.join(SOCKET)
}

/// 소켓을 엶. 예전 실행이 남긴 소켓은 바꿈
pub(crate) fn bind(dir: &Path) -> io::Result<UnixListener> {
    use std::os::unix::fs::{FileTypeExt, PermissionsExt};
    let path = socket_path(dir);
    if std::fs::symlink_metadata(&path).is_ok_and(|m| m.file_type().is_socket()) {
        std::fs::remove_file(&path)?;
    }
    let listener = UnixListener::bind(&path)?;
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600))?;
    listener.set_nonblocking(true)?;
    Ok(listener)
}

/// 종료까지 관리 연결을 하나씩 받아 처리함
pub(crate) fn serve(hub: Arc<Hub>, listener: UnixListener) {
    while !hub.stopping() {
        match listener.accept() {
            Ok((stream, _)) => {
                if let Err(e) = answer(&hub, stream) {
                    eprintln!("screenpet-relay: admin connection: {e}");
                }
            }
            Err(e) if e.kind() == io::ErrorKind::WouldBlock => std::thread::sleep(Duration::from_millis(100)),
            Err(_) => std::thread::sleep(Duration::from_millis(500)),
        }
    }
}

/// 연결 하나의 명령을 읽고 답을 씀
fn answer(hub: &Hub, mut stream: UnixStream) -> io::Result<()> {
    stream.set_nonblocking(false)?;
    stream.set_read_timeout(Some(TIMEOUT))?;
    stream.set_write_timeout(Some(TIMEOUT))?;
    let mut line = String::new();
    BufReader::new((&stream).take(4096)).read_line(&mut line)?;
    let reply = match serde_json::from_str::<Vec<String>>(&line) {
        Ok(words) => run(hub, &words),
        Err(_) => Err("bad request".to_string()),
    };
    let (ok, text) = match reply {
        Ok(t) => (true, t),
        Err(t) => (false, t),
    };
    writeln!(stream, "{}", json!({"ok": ok, "text": text}))
}

/// `on`/`off`를 bool로 읽음
fn on_off(word: &str) -> Result<bool, String> {
    match word {
        "on" => Ok(true),
        "off" => Ok(false),
        _ => Err("expected on or off".into()),
    }
}

/// 명령 낱말을 해석해 허브에서 돌림. 성공이면 `Ok(글)`, 실패면 `Err(글)`
fn run(hub: &Hub, words: &[String]) -> Result<String, String> {
    let words: Vec<&str> = words.iter().map(String::as_str).collect();
    let yes = |flag: &[&str]| match flag {
        [] => Ok(false),
        ["--yes"] => Ok(true),
        _ => Err(format!("unexpected arguments\n{USAGE}")),
    };
    let number = |s: &str| s.parse::<u64>().map_err(|_| format!("not a number: {s}"));
    match words.as_slice() {
        ["status"] => Ok(hub.admin_status()),
        ["kick", who] => hub.admin_kick(who),
        ["ban", who] => hub.admin_ban(who),
        ["ban-ip", ip] => hub.admin_ban_ip(ip),
        ["unban", who] => hub.admin_unban(who),
        ["bans"] => Ok(hub.admin_bans()),
        ["mailbox"] => Ok(hub.admin_mailbox()),
        ["mailbox-reader", op, who] => hub.admin_mailbox_reader(op, who),
        ["registration", v] => hub.admin_registration(on_off(v)?),
        ["friendcode", v] => hub.admin_friendcode(on_off(v)?),
        ["relay", v] => hub.admin_relay(on_off(v)?),
        ["purge", who] => hub.admin_purge(who),
        ["purge-idle", days, rest @ ..] => hub.admin_purge_idle(number(days)?, yes(rest)?),
        ["purge-islands", min, rest @ ..] => hub.admin_purge_islands(number(min)?, yes(rest)?),
        _ => Err(format!("unknown command\n{USAGE}")),
    }
}

/// `dir`을 쓰는 릴레이에 명령을 돌림: `(성공 여부, 글)`
pub fn request(dir: &Path, words: &[&str]) -> io::Result<(bool, String)> {
    let mut stream = UnixStream::connect(socket_path(dir))?;
    stream.set_read_timeout(Some(Duration::from_secs(30)))?;
    stream.set_write_timeout(Some(TIMEOUT))?;
    writeln!(stream, "{}", json!(words))?;
    let mut line = String::new();
    BufReader::new(&stream).read_line(&mut line)?;
    let reply: serde_json::Value = serde_json::from_str(&line).map_err(io::Error::other)?;
    Ok((reply["ok"] == true, reply["text"].as_str().unwrap_or_default().to_string()))
}
