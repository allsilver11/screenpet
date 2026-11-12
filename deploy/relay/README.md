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
empty store), plus the relay's own key (`relay.key`), and how many days it has
been seen on. No message content. Operator switches and bans are in
`control.json`: its only addresses and keys are the ones you banned. The data
directory is 0700 and the files 0600.

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
mkdir -p ~/.config/systemd/user ~/.local/share/screenpet-relay
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

The unit sets `LimitNOFILE=8192`, `TasksMax=2200`, `MemoryMax=1G` and a
sandbox. **After changing the unit, check it really started**
(`systemctl --user status screenpet-relay`). If it fails with `226/NAMESPACE`
or `218/CAPABILITIES`, this machine's kernel won't allow part of the sandbox:
remove the sandbox block (from `PrivateUsers` down), then `daemon-reload` and
restart. On any start the relay also raises its own soft descriptor limit as
far as the hard limit allows, and admits fewer connections if that's short.

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

## 4. Firewall (once, needs an admin user)

Only the proxy should reach port 9411: a direct connection can hold a thread
for 5 s without saying who it is, and enough of them block everyone, proxy
traffic included. With `ufw` (order matters, and loopback stays allowed):

```sh
sudo ufw allow from 192.168.0.52 to any port 9411 proto tcp
sudo ufw deny 9411/tcp
sudo ufw status numbered      # the allow must be above the deny
```

If `ufw` is not active (`sudo ufw status`), with `iptables` (add a way to
restore them after a reboot, such as `iptables-persistent`):

```sh
sudo iptables  -I INPUT -p tcp --dport 9411 ! -i lo ! -s 192.168.0.52 -j DROP
sudo ip6tables -I INPUT -p tcp --dport 9411 ! -i lo -j DROP
```

Check from another LAN machine: `curl -m 3 http://192.168.0.53:9411/healthz`
must time out, while `https://pet.danni.day/healthz` still answers.

## 5. Check

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
write `off` to disable internet friends. Tray: *친구랑 놀기 ▸ 인터넷 연결: 연결됨/끊김*, *내 친구 코드
보기* (shows the code above the pet; click to copy), *친구 코드 입력…*.

## App updates (🔄 업데이트, v0.6.3)

The app checks `https://pet.danni.day/downloads/latest.json` (fixed address,
not the relay URL setting) and downloads the installer it names from
`/downloads/`, verifying size and SHA-256. The relay serves those files from
`<data dir>/downloads/`: `GET`/`HEAD` only, only `latest.json` and
`ScreenPet_X.Y.Z_x64-setup.exe` / `ScreenPet_X.Y.Z_universal.dmg` (no listing,
no other names, no symlinks), at most 8 installers at once and 2 per client
address (`503 busy` past that), `latest.json` with `Cache-Control: no-cache`.
A download is cut off if it averages under 64 KiB/s after 20 s, stalls for
10 s, or runs past 30 min, so slow clients can't hold the slots.

**Signing.** The app trusts `latest.json` only if `latest.json.sig` (128 hex
digits, Ed25519 over its exact bytes) matches `UPDATE_PUBLIC_KEY` in
`crates/screenpet-core/src/update.rs`, so whoever runs this server or the
relay still can't swap installers. The secret key lives at
`~/personal/screenpet-update-key.txt` (0600, outside the repo and outside the
relay's data folder; the relay never reads it). **Back it up offline** (e.g. a
password manager or an encrypted USB stick): losing it means a new key, which
means a new app build that every user installs by hand. If it leaks, make a new
key the same way and ship a new build. The signer is a separate tool:

```sh
cargo build --release -p screenpet-core --bin screenpet-update-sign
install -m 755 target/release/screenpet-update-sign ~/.local/bin/
# only for a new key (refuses to overwrite); prints the public key for update.rs:
screenpet-update-sign keygen ~/personal/screenpet-update-key.txt
```

After CI has finished a release, publish it on this server:

```sh
deploy/publish-release.sh v0.6.3                  # GITHUB_TOKEN=… if the repo is private
curl -s https://pet.danni.day/downloads/latest.json
```

It downloads both installers through the GitHub API into a temporary folder
in `downloads/`, checks them against GitHub's size and SHA-256, renames them
into place, signs `latest.json` (and checks the signature against the app's
key), then atomically replaces `latest.json` and `latest.json.sig`, then deletes every other
`ScreenPet_*` file: **only the new release is kept**. A failed run leaves the
published files untouched and removes its temporary files (leftovers of a
killed run are removed by the next run). No relay restart is needed.

## Admin commands

Run on the server, as the relay's user. They talk to the running relay through
`admin.sock` in the data directory, so nothing about them is on the network.

```sh
screenpet-relay admin status
screenpet-relay admin --data /some/dir status    # another data directory
```

| Command | What it does |
| --- | --- |
| `status` | Switches, totals, each online install (id, key prefix, connected since, address bucket, friends), counters since start |
| `kick <who>` | Disconnects it. The app comes straight back |
| `ban <who>` | Disconnects it and refuses that key from now on. A whole 64-digit key can be banned before it is ever seen |
| `ban-ip <ip>` | Disconnects and refuses an address (IPv6: its /64). Loopback and trusted proxies are refused: everyone can arrive as those |
| `unban <who or ip>`, `bans` | Lifts a ban, lists them |
| `registration on\|off` | Off: new installs get `registration_closed`; known ones log in. An install forgotten by the retention rules counts as new |
| `friendcode on\|off` | Off: for everyone, nothing is issued (the code reads `----`) and redeeming answers `disabled`, and unused redeems are cancelled. Friends keep connecting. On: online installs get their code back |
| `relay on\|off` | Off: everyone is disconnected and refused, including unauthenticated connections. `/healthz` stays 200 |
| `purge <who>` | Forgets the install, disconnecting it if online. If it comes back it registers afresh |
| `purge-idle <days> [--yes]` | Forgets offline installs unseen for that long |
| `purge-islands <min> [--yes]` | Forgets groups of at least `min` installs joined by mutual friendship, none online. This is what keys befriending only each other look like, but a real group of friends who are all away looks the same, so read the list first |

`<who>` is an id or key prefix of at least 4 characters, or an exact id. If it
matches more than one install, nothing happens and the matches are listed.
Without `--yes` the `purge-*` commands only list what they'd remove. A purged
real install loses nothing lasting: the app sends its friend list at each login.

Switches and bans are saved in `control.json` and survive restarts. If that
file is corrupt the relay **refuses to start** rather than silently lift a ban;
the log says so. Fix it, or delete it to return to everything on and no bans.
Apps of any version keep working: they only log an `error` reason they don't
know, and retry with increasing delay.

## Limits (defaults)

- **Traffic**: 64 KB per frame; 20 frames/s per friend link (burst 40); per
  connection 200 messages/s and 512 KB/s; 64 open links per connection, 2
  between the same two installs; 90 s idle timeout, 10 s to authenticate;
  16 connections per address, 128 per network (IPv4 /24, IPv6 /48), 2048 in
  total. 30 `friends` updates per connection, then 1 per 2 s.
- **Connections not yet authenticated**: at most 8 per address, 32 per network,
  512 in total. A full pool disconnects its oldest member (`busy`), never the
  newcomer.
- **Bandwidth** (friend frames, a link is closed when over): 2 MB/s per address
  (reconnecting doesn't refill it); 16 MB/s for the relay. Each network keeps
  1 MB/s of that however busy the rest are, and everyone stops at 32 MB/s.
- **Queues**: a connection may have 2 MB of frames waiting. Over that the
  frame's link is closed; if the queue also hasn't moved for 5 s the connection
  is dropped (`slow`). 256 MB in all, past which only networks with under 4 MB
  queued may add more, up to 512 MB.
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
  refused rather than evicting live caps; a network may add 64 codes to it per
  hour, an IPv4 /16 or IPv6 /32 256.
- **Installs**: 30 new installs per hour per network (IPv4 /24, IPv6 /64),
  100 per day per wider network (IPv4 /24, IPv6 /48), 2000 per hour in total,
  at most 20 000 stored. Installs with no *mutual* friend that never took part
  in a redeem are forgotten after 24 h unseen; those with one, after 14 days;
  those also seen on 7 different days (and any install from before this was
  counted), after 180 days, and they stay that way if a friend is forgotten.
  Online installs are never dropped. A full store first evicts the least
  recently seen offline install without a friend, then one that hasn't reached
  7 days (it registers again at its next login), and refuses (`full`) only if
  there is neither. Stale installs are looked for every 10 minutes, the first
  time 10 minutes after start, so clients can come back from a restart first.
- **Saving**: changes are written every 30 s and on SIGTERM.
- **Web pages**: a WebSocket request with an `Origin` header is refused.
- **Rate tables** keep at most 8192 keys and at most 16 per network (IPv4
  /24, IPv6 /48; past that a network's newcomers share one key). When full,
  stale keys go first, then the least recently active one.

## Feedback letters (💌 건의하기, v0.6.0)

Any install can leave one sealed letter for the developer. The app seals it to
the developer's public key (Noise `N`), so the relay stores bytes it can't read
in `letters.json` (0600, previous version kept as `letters.json.bak`; both torn
means an empty mailbox, the relay still starts). Limits are enforced here,
never in the app:

- 5 per install key per 24 h (1 before v0.6.3)
- 10 per address (IPv4 / IPv6 /64) per 24 h (3 before v0.6.3)
- the install must have been seen on 2+ days
- 50 per 24 h in total
- 200 letters kept; when full, new ones are refused (old ones are never dropped)
- 8 KiB per sealed letter
- keys a reader blocked are refused

Only installs listed as mailbox readers (the developer's computers, kept in
`control.json`) can list, mark read, delete and block:

```sh
screenpet-relay admin mailbox-reader add <id-or-key-prefix>
screenpet-relay admin mailbox-reader remove <id-or-key-prefix>
screenpet-relay admin mailbox          # letters, unread, readers
```

Deploy the relay before an app that sends letters: older relays answer an
unknown message by disconnecting.

## Public suggestion board (📮 건의함, v0.6.3, protocol v3)

The relay can't read letters, so the developer's app (a mailbox reader)
publishes a digest of each letter it opens: `board_put {items: [{id, name,
line}]}` — the sender's name with every grapheme but the first replaced by
`*` (`김철수` → `김**`, one-grapheme names `*`) and the letter's first line
(≤ 60 characters + "…"). The body never leaves the developer's computer. The
relay takes digests only from mailbox readers, at most 20 a message, and
checks them again: the name must be a real mask (≤ 16 characters, everything
after the first grapheme `*`), the line one line of 1–80 characters; control
and bidi characters are dropped; the letter must exist.

- digests live in `letters.json` with their letter (same `.bak` and save
  lock) and go when it is deleted or its sender blocked; never more than the
  200 letters
- `board_list {page}` from any logged-in app → `board_page {items, page,
  pages, total}`: 5 a page, newest first, each with the letter's date and
  status (`none`, `done` ✅ 반영됨, `rejected` 🙅 반려됨); one request per
  0.5 s per connection, faster ones get no answer
- `letter_reject {id, rejected}` (readers): 🙅 반려. A letter is either done or
  rejected; older developer apps' `letter_done false` clears both. Old
  `letters.json` files load as they are (`done: true` stays done)

Apps send `board_put`, `board_list` and `letter_reject` only to relays whose
challenge says `v >= 3`; older apps never send them. Nothing else to set up:
deploy the new binary, existing letters get published the next time the
developer's app connects.

## Real weather (☀️ 실제 날씨, v0.6.3)

The relay fetches the current WMO weather code from
[Open-Meteo](https://open-meteo.com/) (`api.open-meteo.com`, HTTPS, no key)
and hands it to the apps, so they never call a weather service themselves.
Apps send only a place id from a fixed list of 18 Korean cities
(`weather::PLACES`, the same list as the app's) — `weather_want {place}`;
unknown ids are ignored, apps that never say get Yongin. No coordinates or
anything about the apps go over the wire or to Open-Meteo (only the city's
coordinates).

- each place at most every 30 min (5 min after a failure), only while an app
  on it is connected; a failed fetch keeps the last reading
- places nobody wanted for 2 h are forgotten (at most the 18 built-in ones)
- 10 s timeout, 16 KiB answer cap, one fetch at a time on its own thread;
  errors are logged at most every 10 minutes and never stop the relay
- `SCREENPET_RELAY_WEATHER=off` turns it off (no fetches, no `weather`)

Needs outbound HTTPS (443) and DNS from the server. Apps send `weather_want`
only to relays whose challenge says `v >= 2` (older relays disconnect on
unknown messages); older apps ignore `weather`.
