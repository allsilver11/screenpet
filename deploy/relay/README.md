# ScreenPet relay (`screenpet-relay`)

Lets pets visit friends who are **not on the same network** (spec phase 3 §1b).
The relay is an untrusted pipe: each install authenticates with its Noise
static key, the relay only joins two installs that list each other as friends
(or once, right after one of them typed the other's friend code), and the
friends run their end-to-end Noise session *through* it — the relay sees only
ciphertext. Pairing is still confirmed by the 6-emoji check on both screens.

It stores only, per install: its public key, id label, the friend keys it
wants to reach, and its current friend code (`relay.json`, previous version
kept as `relay.json.bak`; a torn file falls back to the backup, then to an
empty store), plus the relay's own key (`relay.key`). No message content.
The data directory is 0700 and the files 0600.

**The app pins the relay's public key** (`DEFAULT_RELAY_KEY` in
`crates/screenpet-net/src/relay.rs`, printed in the relay's start-up log).
Keep `relay.key` backed up: a new key means a new app build (or every user
putting the URL + key in `relay_url.txt`).

## 1. Build

```sh
cargo build --release -p screenpet-relay
install -m 755 target/release/screenpet-relay ~/.local/bin/
```

CI also uploads a Linux binary as the `screenpet-relay-linux` artifact.

## 2. Run as a systemd *user* service

```sh
mkdir -p ~/.config/systemd/user
cp deploy/relay/screenpet-relay.service ~/.config/systemd/user/
systemctl --user daemon-reload
systemctl --user enable --now screenpet-relay
journalctl --user -u screenpet-relay -f      # logs
```

It listens on `0.0.0.0:9411` (plain HTTP/WebSocket; TLS is done by the proxy)
with data in `~/.local/share/screenpet-relay/`. Options: `--bind ADDR:PORT`,
`--data DIR`, `--trusted-proxy IP` (repeatable) (or `SCREENPET_RELAY_BIND`,
`SCREENPET_RELAY_DATA`, `SCREENPET_RELAY_TRUSTED_PROXIES=ip,ip`).

User services stop when you log out unless lingering is on (one-time, needs
admin rights):

```sh
sudo loginctl enable-linger "$USER"
```

Without systemd: `nohup ~/.local/bin/screenpet-relay >> ~/.local/share/screenpet-relay/log 2>&1 &`.

## 3. Nginx Proxy Manager

*Hosts → Proxy Hosts → Add Proxy Host*

- **Domain Names**: `pet.danni.day`
- **Scheme** `http`, **Forward Hostname / IP**: the relay server's LAN IP
  (here `192.168.0.53`), **Forward Port** `9411`
- **Websockets Support**: **on** (required)
- *SSL* tab: request a Let's Encrypt certificate, **Force SSL** on

For its per-address limits the relay believes `X-Real-IP` (else the
rightmost `X-Forwarded-For` entry) only when the TCP peer is loopback or a
configured trusted proxy (default `192.168.0.52`, the NPM host; it logs the
list at start). Any other peer is limited by its own address. IPv6 clients
are grouped by /64.

## 4. Check

```sh
curl -i http://127.0.0.1:9411/healthz          # 200 ok (on the server)
curl -i https://pet.danni.day/healthz          # 200 ok (through the proxy)
# Two throwaway installs pair through the public relay and exchange a visit:
SCREENPET_LIVE_RELAY=wss://pet.danni.day/ws \
  cargo test -p screenpet-net --test relay -- --ignored live_relay_smoke
```

A WebSocket client connecting to `wss://pet.danni.day/ws` first receives
`{"t":"challenge",...}`.

## App side

The app uses `wss://pet.danni.day/ws` with the pinned relay key by default.
To use another relay, put its URL on the first line of `relay_url.txt` in
the app's config folder (`%APPDATA%\com.allsilver11.screenpet\` on Windows)
and, optionally, its public key (64 hex digits) on the second line to pin it;
write `off` to disable internet friends. Tray: *친구 화면 ▸ 인터넷 연결: 연결됨/끊김*, *내 친구 코드
보기* (shows the code above the pet; click to copy), *친구 코드 입력…*.

## Limits (defaults)

- **Traffic**: 64 KB per frame; 20 frames/s per friend link (burst 40); per
  connection 200 messages/s and 512 KB/s; 64 open links per connection, 2
  between the same two installs; 90 s idle timeout, 10 s to authenticate;
  16 connections per address, 2048 in total.
- **Friends and codes**: 50 friend keys per install. Friend codes rotate
  every 24 h, or on request (`new_code`, 5 per connection). A redeemed code
  allows one link within 60 s. Wrong, own and offline codes all answer
  `unknown`.
- **Code guessing**: 10 attempts per connection and 30 per address per
  minute. The global budget is 600 well-formed codes per minute (malformed
  input isn't counted). Each network (IPv4 /24, IPv6 /48) has a fair share
  of 60/min (never more than the per-address cap) that the budget never
  refuses, with a hard ceiling of 1200/min for everyone. Each code accepts
  at most 10 distinct requesters per hour (a retry counts once, a new code
  starts fresh). When that table is full of live entries, new codes are
  refused rather than evicting live caps.
- **Installs**: 30 new installs per hour per network (IPv4 /24, IPv6 /64),
  2000 per hour in total, at most 20 000 stored. Installs with no *mutual*
  friend that never took part in a redeem are forgotten after 24 h unseen,
  others after 180 days; online installs are never dropped. A full store
  first evicts the least recently seen offline install without a mutual
  friend, and refuses (`full`) only if there is none.
- **Rate tables** keep at most 8192 keys and at most 16 per network (IPv4
  /24, IPv6 /48; past that a network's newcomers share one key). When full,
  stale keys go first, then the least recently active one.
