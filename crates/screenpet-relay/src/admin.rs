//! Server-only commands: `screenpet-relay admin <command>` talks to the running
//! relay over the Unix socket `admin.sock` in the 0700 data directory, so only
//! the relay's own user can reach it. One JSON array of words per connection,
//! answered with `{"ok": bool, "text": string}`. Commands run one at a time.

use crate::hub::Hub;
use serde_json::json;
use std::io::{self, BufRead, BufReader, Read, Write};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

const SOCKET: &str = "admin.sock";
const TIMEOUT: Duration = Duration::from_secs(5);

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

pub(crate) fn socket_path(dir: &Path) -> PathBuf {
    dir.join(SOCKET)
}

/// Listen on the socket, replacing one left by an earlier run.
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

fn on_off(word: &str) -> Result<bool, String> {
    match word {
        "on" => Ok(true),
        "off" => Ok(false),
        _ => Err("expected on or off".into()),
    }
}

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

/// Run a command on the relay using `dir`: `(succeeded, text)`.
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
