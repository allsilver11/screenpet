# 💌 건의하기 / 📬 편지함 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Anyone's pet can carry a sealed letter to the developer through the relay; only the developer's computers can list and open them.

**Architecture:** Letters are sealed in the app with a one-message Noise `N` pattern to a developer public key (`DEV_KEY`). The relay stores the opaque sealed box in a new `mailbox` module (`letters.json`). It enforces every limit server-side, using the authenticated install key and the IP bucket. It serves the list only to install keys in `control.json` `mailbox_readers`. The app has a sender window for everyone; the inbox window and letter-carrying pets exist only when `config.json` has `dev_key`.

**Tech Stack:** Rust (screenpet-relay, screenpet-net with `snow`, screenpet-core, src-tauri), plain JS/HTML webviews (ui/), Python pytest UI tests (tools/).

**Spec:** `docs/superpowers/specs/2026-11-10-feedback-mailbox-design.md`

## Global Constraints

- Letter text: 1–1000 characters after `clean_text`.
- Sealed box: at most 8192 bytes (`MAX_BOX`).
- Relay limits:
  - per install key: 1 letter per 24 h
  - per IP bucket: 3 per 24 h
  - install must be seen on at least 2 days (`seen_days >= 2`)
  - global: 50 per 24 h
  - inbox: 200 letters, then refuse (never evict)
  - blocked keys are refused
- Refusal reasons (wire strings): `daily` `ip` `new` `global` `full` `size` `blocked`.
- Old peers: friend protocol v3 is unchanged. The relay is deployed before the app.
- No IP or email inside letters. The developer secret key never enters the repo.
- All user-visible text is Korean. Code comments follow the file's language.
- Commits end with `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`.

## File structure

| File | Responsibility |
|---|---|
| `crates/screenpet-relay/src/mailbox.rs` (new) | Pure mailbox: limits, letters, blocked list, (de)serialize, file load/save |
| `crates/screenpet-relay/src/proto.rs` | New `ClientMsg`/`ServerMsg` variants |
| `crates/screenpet-relay/src/hub.rs` | Hold `Mailbox` in `State`, letter handlers, push to readers, admin commands |
| `crates/screenpet-relay/src/conn.rs` | Dispatch new client messages |
| `crates/screenpet-relay/src/control.rs` | `mailbox_readers` set |
| `crates/screenpet-relay/src/admin.rs` | `mailbox-reader add/remove/list`, `letters` count |
| `crates/screenpet-net/src/letter.rs` (new) | `Letter` content, `seal`, `open`, sanitize, `DEV_KEY` |
| `crates/screenpet-net/src/relay.rs` | Send `letter`, list/read/delete/block, new server messages → events |
| `crates/screenpet-net/src/lib.rs` | `Net::send_letter`, `Net::mailbox_*`, `NetEvent::{LetterSent, Mailbox, LetterArrived}` |
| `src-tauri/src/feedback.rs` (new) | Sender window + commands, mailbox window + commands, letter pets queue |
| `src-tauri/src/tray.rs`, `network.rs`, `game_loop.rs`, `main.rs` | Menu items, wiring |
| `ui/feedback.html`, `ui/feedback.js`, `ui/mailbox.html`, `ui/mailbox.js` (new) | Windows |
| `crates/screenpet-core/src/config.rs` | `dev_key: Option<String>` |
| `tools/test_feedback_ui.py` (new) | UI tests |

---

### Task 1: Relay mailbox core (`mailbox.rs`)

**Files:**
- Create: `crates/screenpet-relay/src/mailbox.rs`
- Modify: `crates/screenpet-relay/src/lib.rs` (add `pub(crate) mod mailbox;`)

**Interfaces:**
- Produces:
  - `pub const MAX_BOX: usize = 8192; MAX_LETTERS: usize = 200; PER_KEY: usize = 1; PER_IP: usize = 3; GLOBAL: usize = 50; MIN_SEEN_DAYS: u32 = 2; WINDOW_SECS: u64 = 86400;`
  - `#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)] pub struct StoredLetter { pub id: u64, pub at: u64, pub from: String, pub read: bool, pub r#box: String /* base64 */ }`
  - `pub enum Refuse { Daily, Ip, New, Global, Full, Size, Blocked }` with `fn as_str(&self) -> &'static str`
  - `pub struct Mailbox` with `fn submit(&mut self, from: &str, ip: &str, seen_days: u32, sealed: &[u8], now: u64) -> Result<StoredLetter, Refuse>`, `fn list(&self) -> Vec<StoredLetter>` (newest first), `fn mark_read(&mut self, id: u64) -> bool`, `fn delete(&mut self, id: u64) -> bool`, `fn block(&mut self, key: &str) -> usize` (returns removed letters), `fn is_blocked(&self, key: &str) -> bool`, `fn snapshot(&mut self) -> Option<Vec<u8>>` (when dirty), `fn load(dir: &Path) -> Mailbox`, `fn save(dir: &Path, bytes: &[u8]) -> io::Result<()>` (tmp + `.bak` + rename like `write_snapshot`)

- [ ] **Step 1: Write failing tests** (in `mailbox.rs` `#[cfg(test)] mod tests`):

```rust
fn mb() -> Mailbox { Mailbox::default() }
const T: u64 = 1_000_000;

#[test]
fn a_known_install_can_send_one_letter_a_day() {
    let mut m = mb();
    assert!(m.submit("k1", "1.2.3.4", 2, b"x", T).is_ok());
    assert_eq!(m.submit("k1", "1.2.3.4", 2, b"x", T + 10), Err(Refuse::Daily));
    assert!(m.submit("k1", "1.2.3.4", 2, b"x", T + WINDOW_SECS).is_ok());
}

#[test]
fn three_per_address_and_fifty_overall_per_day() {
    let mut m = mb();
    for k in 0..3 { assert!(m.submit(&format!("a{k}"), "9.9.9.9", 5, b"x", T).is_ok()); }
    assert_eq!(m.submit("a3", "9.9.9.9", 5, b"x", T), Err(Refuse::Ip));
    let mut m = mb();
    for k in 0..GLOBAL { assert!(m.submit(&format!("g{k}"), &format!("10.0.{}.{}", k / 200, k % 200), 5, b"x", T).is_ok()); }
    assert_eq!(m.submit("late", "8.8.8.8", 5, b"x", T), Err(Refuse::Global));
}

#[test]
fn brand_new_keys_too_big_boxes_and_blocked_keys_are_refused() {
    let mut m = mb();
    assert_eq!(m.submit("new", "1.1.1.1", 1, b"x", T), Err(Refuse::New));
    assert_eq!(m.submit("k", "1.1.1.1", 2, &vec![0; MAX_BOX + 1], T), Err(Refuse::Size));
    assert_eq!(m.submit("k", "1.1.1.1", 2, b"", T), Err(Refuse::Size));
    let id = m.submit("bad", "2.2.2.2", 3, b"x", T).unwrap().id;
    assert_eq!(m.block("bad"), 1);
    assert!(m.list().iter().all(|l| l.id != id));
    assert_eq!(m.submit("bad", "3.3.3.3", 3, b"x", T + WINDOW_SECS), Err(Refuse::Blocked));
}

#[test]
fn a_full_inbox_refuses_new_letters_instead_of_dropping_old_ones() {
    let mut m = mb();
    for i in 0..MAX_LETTERS as u64 {
        // spread over days so the per-day limits never trigger
        m.submit(&format!("k{i}"), &format!("10.{}.0.1", i), 2, b"x", T + i * WINDOW_SECS).unwrap();
    }
    assert_eq!(m.submit("one-more", "7.7.7.7", 2, b"x", T + 999 * WINDOW_SECS), Err(Refuse::Full));
    assert_eq!(m.list().len(), MAX_LETTERS);
    let oldest = m.list().last().unwrap().id;
    assert!(m.delete(oldest));
    assert!(m.submit("one-more", "7.7.7.7", 2, b"x", T + 999 * WINDOW_SECS).is_ok());
}

#[test]
fn letters_list_newest_first_and_read_flags_stick() {
    let mut m = mb();
    let a = m.submit("a", "1.0.0.1", 2, b"aa", T).unwrap().id;
    let b = m.submit("b", "1.0.0.2", 2, b"bb", T + 5).unwrap().id;
    assert_eq!(m.list().iter().map(|l| l.id).collect::<Vec<_>>(), vec![b, a]);
    assert!(m.mark_read(a) && !m.mark_read(999));
    assert!(m.list().iter().find(|l| l.id == a).unwrap().read);
}

#[test]
fn it_survives_a_restart_and_a_torn_file() {
    let dir = tempfile::tempdir().unwrap();
    let mut m = mb();
    m.submit("a", "1.0.0.1", 2, b"hello", T).unwrap();
    m.block("evil");
    Mailbox::save(dir.path(), &m.snapshot().unwrap()).unwrap();
    m.submit("b", "1.0.0.2", 2, b"x", T + 1).unwrap();
    Mailbox::save(dir.path(), &m.snapshot().unwrap()).unwrap();
    let back = Mailbox::load(dir.path());
    assert_eq!(back.list().len(), 2);
    assert!(back.is_blocked("evil"));
    std::fs::write(dir.path().join("letters.json"), b"{torn").unwrap();
    assert_eq!(Mailbox::load(dir.path()).list().len(), 1, "falls back to .bak");
    // the per-key day limit is remembered too
    let mut back = Mailbox::load(dir.path());
    assert_eq!(back.submit("a", "1.0.0.9", 2, b"x", T + 2), Err(Refuse::Daily));
}
```

- [ ] **Step 2:** `cargo test -p screenpet-relay mailbox` → FAIL (module missing). Check whether `tempfile` is a dev-dependency; if not, use `std::env::temp_dir().join(format!("mb-{}", random))`.

- [ ] **Step 3: Implement `mailbox.rs`:**
  - `struct Data { next_id: u64, letters: Vec<StoredLetter>, sent: Vec<Sent>, blocked: BTreeSet<String> }` with `#[serde(default)]`, where `Sent { key, ip, at }`.
  - `submit` order of checks:
    1. size (`sealed.is_empty() || len > MAX_BOX`)
    2. blocked
    3. `seen_days < MIN_SEEN_DAYS` → New
    4. prune `sent` older than `WINDOW_SECS`
    5. per-key ≥ PER_KEY → Daily
    6. per-ip ≥ PER_IP → Ip
    7. total ≥ GLOBAL → Global
    8. letters ≥ MAX_LETTERS → Full
    9. Push `Sent` and `StoredLetter { id: next_id, at: now, from, read: false, box: base64 }`, set `dirty = true`.
  - Base64: there is no base64 crate; the hex helpers are in screenpet-crypto, so store hex (`proto::to_hex`) under the field name `box` — 8 KiB becomes 16 KiB hex, which is fine. The wire field is `box` (hex). Use `r#box`.

- [ ] **Step 4:** Tests pass.
- [ ] **Step 5:** Commit `feat(relay): mailbox store with server-side letter limits`.

### Task 2: Relay protocol, hub wiring, readers, admin

**Files:**
- Modify: `crates/screenpet-relay/src/proto.rs`, `hub.rs`, `conn.rs`, `control.rs`, `admin.rs`, `lib.rs` (load the mailbox at start next to `Store::open`)
- Test: `crates/screenpet-relay/src/hub.rs` tests (use the existing `hub()`/`peer_at()` helpers), plus `crates/screenpet-relay/tests/` if the end-to-end pattern exists there.

**Interfaces:**
- Consumes: `Mailbox`, `Refuse`, `StoredLetter` (Task 1).
- Produces (wire):
  - `ClientMsg::Letter { r#box: String }`
  - `ClientMsg::Letters`
  - `ClientMsg::LetterRead { id: u64 }`
  - `ClientMsg::LetterDelete { id: u64 }`
  - `ClientMsg::LetterBlock { from: String }`
  - `ServerMsg::LetterOk`
  - `ServerMsg::LetterRefused { reason: String }`
  - `ServerMsg::Letters { items: Vec<StoredLetter> }`
  - `ServerMsg::LetterNew { item: StoredLetter }`
  - `ServerMsg::LettersChanged`
- Control: `Control.mailbox_readers: BTreeSet<String>`.
- Admin: `mailbox-reader add <key>`, `mailbox-reader remove <key>`, `mailbox-readers`, and `letters` (count, unread).

- [ ] **Step 1: Failing hub tests:**

```rust
#[test]
fn a_letter_is_stored_pushed_to_online_readers_and_listed_only_for_them() {
    let hub = hub(Limits::default());
    let (reader, rconn, _rl, rrx) = peer_at(&hub, "5.5.5.5", 1, &[], 8);
    hub.admin_mailbox_reader(&reader, true).unwrap();
    let (sender, sconn, _sl, _srx) = peer_at(&hub, "6.6.6.6", 2, &[], 8);
    hub.set_seen_days_for_test(&sender, 3);
    assert_eq!(hub.letter(&sender, sconn, "6.6.6.6", "abcd"), Some(ServerMsg::LetterOk));
    assert!(matches!(next_text(&rrx), ServerMsg::LetterNew { .. }));
    assert!(matches!(hub.letters(&reader, rconn), Some(ServerMsg::Letters { items }) if items.len() == 1));
    assert_eq!(hub.letters(&sender, sconn), None, "not a reader: ignored");
    assert_eq!(hub.letter(&sender, sconn, "6.6.6.6", "abcd"), Some(ServerMsg::LetterRefused { reason: "daily".into() }));
}

#[test]
fn readers_mark_read_delete_and_block_and_every_reader_hears_it() {
    // two readers online; reader A deletes → reader B gets LettersChanged
}

#[test]
fn bad_hex_is_refused_as_size() {
    // "zz" → LetterRefused { reason: "size" }
}
```

  `next_text`: read `Out::Text` from the receiver and parse `ServerMsg` (add the helper next to the existing test helpers if missing). `set_seen_days_for_test` is `#[cfg(test)]` and sets `seen_days` in the store.

- [ ] **Step 2:** Run the tests and see them fail.
- [ ] **Step 3: Implement:**
  - `State.mailbox: Mailbox` and `data_dir`.
  - `Hub::letter(key, conn, ip, hex)`:
    1. `is_current`; decode hex (failure → `Size`).
    2. `seen_days` from `store.get(key)`.
    3. `submit` with the IP bucket; on success push `LetterNew` to every online peer whose key is in `control.mailbox_readers`.
    4. Save the mailbox outside the lock (snapshot under the lock, then `Mailbox::save`).
  - Readers-only handlers return `None` for non-readers. Read, delete and block push `LettersChanged` to all online readers.
  - `conn.rs` dispatch: a `Letter` is rate-limited per connection to 3 per minute (`refuse` the excess as `LetterRefused { reason: "daily" }`). Other letter messages are plain calls.
  - `admin` commands as listed. `admin_status` gets a `letters: n (unread u)` line.
- [ ] **Step 4:** `cargo test -p screenpet-relay` → all pass.
- [ ] **Step 5:** Update `deploy/relay/README.md`: a mailbox section (`letters.json`, reader admin commands).
- [ ] **Step 6:** Commit `feat(relay): feedback letters — submit, reader list, push, admin readers`.

### Task 3: Sealed letters (`screenpet-net/src/letter.rs`)

**Files:**
- Create: `crates/screenpet-net/src/letter.rs`; export it in `lib.rs`.
- Create a dev key once: `cargo run -p screenpet-net --example dev_key` prints `public` and `secret` hex. Commit the example; put the public key in `DEV_KEY`. Save the secret into the developer's own `config.json` by hand and **never commit it**.

**Interfaces:**
- Produces:
  - `pub const DEV_KEY: &str = "<hex32>";`
  - `pub const MAX_LETTER_CHARS: usize = 1000;`
  - `#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Default)] pub struct Letter { pub text: String, pub name: String, pub species: String, pub stage: String, pub style: String, pub version: String, pub os: String }`
  - `pub fn seal(letter: &Letter, to: &[u8; 32]) -> Option<Vec<u8>>` (None if the text is empty after cleaning or the box is too big)
  - `pub fn open(sealed: &[u8], secret: &[u8; 32]) -> Option<Letter>` (sanitized)
  - `pub fn clean(letter: Letter) -> Option<Letter>`: text via `clean_text` ≤ 1000 chars (None if empty); `name` ≤ 64; species/stage/style/version/os ≤ 32, ASCII-ish.

- [ ] **Step 1: Failing tests:**

```rust
#[test]
fn a_sealed_letter_opens_only_with_the_developer_secret() {
    let secret = crate::noise::random_key(); // or the existing keygen helper
    let public = crate::noise::public_key(&secret);
    let l = Letter { text: "펭귄 귀여워요".into(), name: "단니".into(), species: "penguin".into(), ..Default::default() };
    let sealed = seal(&l, &public).unwrap();
    assert_eq!(open(&sealed, &secret).unwrap().text, "펭귄 귀여워요");
    let other = crate::noise::random_key();
    assert!(open(&sealed, &other).is_none());
    assert!(open(b"junk", &secret).is_none());
}

#[test]
fn letters_are_cleaned_and_limited() {
    let long = Letter { text: "가".repeat(1500), ..Default::default() };
    assert_eq!(clean(long).unwrap().text.chars().count(), MAX_LETTER_CHARS);
    assert!(clean(Letter { text: "   ".into(), ..Default::default() }).is_none());
    let (secret, public) = keypair();
    let sealed = seal(&Letter { text: "가".repeat(1000), name: "x".repeat(200), ..Default::default() }, &public).unwrap();
    assert!(sealed.len() <= 8192);
    assert_eq!(open(&sealed, &secret).unwrap().name.chars().count(), 64);
}

#[test]
fn the_shipped_developer_key_is_a_valid_public_key() {
    assert!(crate::noise::from_hex32(DEV_KEY).is_some());
}
```

- [ ] **Step 2:** Run the tests and see them fail.
- [ ] **Step 3: Implement with `snow`:**
  - Seal: `Builder::new("Noise_N_25519_ChaChaPoly_BLAKE2s".parse()?).remote_public_key(to).build_initiator()?`, then `write_message(json, &mut buf)`.
  - Open: `.local_private_key(secret).build_responder()?`, then `read_message`.
  - Payload is the JSON of the cleaned `Letter`. The Noise max message is 65535, which is fine.
  - Reuse the existing `clean_text` from screenpet-core if net already depends on core. If not, add a small local cleaner: strip control characters, collapse whitespace, take N chars.
- [ ] **Step 4:** Tests pass.
- [ ] **Step 5:** Commit `feat(net): sealed feedback letters (Noise N to the developer key)`.

### Task 4: Net relay client — send and mailbox API

**Files:**
- Modify: `crates/screenpet-net/src/relay.rs` (ClientMsg/ServerMsg variants, handlers, pending letter request with 10 s timeout), `crates/screenpet-net/src/lib.rs` (API + events).
- Test: `relay.rs` tests using the existing fake-relay harness (see `say(ServerMsg::Presence…)` tests around line 990).

**Interfaces:**
- Consumes: `letter::seal`, `letter::open`, `DEV_KEY`.
- Produces:
  - `Net::send_letter(&self, letter: Letter) -> bool` (false if not connected or the seal failed)
  - `NetEvent::LetterSent { result: Result<(), String> }` (reason string from the relay or `"offline"`/`"timeout"`)
  - `Net::mailbox_list(&self)`, `Net::mailbox_read(&self, id: u64)`, `Net::mailbox_delete(&self, id: u64)`, `Net::mailbox_block(&self, from: String)`
  - `NetEvent::Mailbox { letters: Vec<MailItem> }` (after a list or `letters_changed` → the client re-lists automatically)
  - `NetEvent::LetterArrived { item: MailItem }`
  - `pub struct MailItem { pub id: u64, pub at: u64, pub from: String, pub read: bool, pub sealed: Vec<u8> }`. The app opens it with its own `dev_key`, so the net crate never sees the secret.

- [ ] **Step 1: Failing tests:**
  - The fake relay answers `letter` with `letter_ok` → `LetterSent { result: Ok(()) }`.
  - `letter_refused {reason:"daily"}` → `Err("daily")`.
  - No answer within 10 s (use the existing timeout test style or an injectable clock) → `Err("timeout")`.
  - `letter_new` → `LetterArrived`.
  - `letters_changed` → the client sends `letters` → `Mailbox`.
- [ ] **Step 2:** Run the tests and see them fail.
- [ ] **Step 3:** Implement. Only one letter may be in flight at a time.
- [ ] **Step 4:** `cargo test -p screenpet-net` (needs the `CC_*`/`AR_*` env vars from memory) → pass.
- [ ] **Step 5:** Commit `feat(net): send feedback letters and read the developer mailbox through the relay`.

### Task 5: App — 💌 건의하기 (everyone)

**Files:**
- Create: `ui/feedback.html`, `ui/feedback.js`, `src-tauri/src/feedback.rs`.
- Modify:
  - `src-tauri/src/tray.rs`: menu item `FEEDBACK_ID` "💌 개발자에게 건의하기…", above "도움말".
  - `src-tauri/src/main.rs`: register commands `feedback_send(text: String)` and `feedback_state()`; add the window to capabilities.
  - `src-tauri/capabilities/default.json`: the window label `feedback`.
  - `src-tauri/src/network.rs`: build the `Letter` from the own slot and `config` (name, species, stage, style, version, os), call `net.send_letter`, map `LetterSent` → emit `feedback-result`.
- Test: `tools/test_feedback_ui.py` (follow `tools/test_gift_ui.py`/`test_ladder_ui.py` patterns: read the html/js/rs text and run JS snippets with node).

**Interfaces:**
- Produces:
  - JS → Rust `invoke("feedback_send", {text})`.
  - Rust → JS event `feedback-result` with `{ok: bool, reason: string}`.
  - Korean messages per reason exactly as in the spec §4: `daily` `new` `ip` `global` `full` `size` `blocked` `offline` `timeout`. `blocked`/`offline`/`timeout` all show "지금은 보낼 수 없어요".

- [ ] **Step 1: Failing Python tests:**
  - The menu text exists in `tray.rs`.
  - `feedback.html` has a textarea with `maxlength="1000"` and a counter.
  - `feedback.js` maps every reason to the spec message (run the mapping function with node).
  - The window closes 2 s after `ok`.
  - `capabilities/default.json` lists `feedback`.
- [ ] **Step 2:** Run the tests and see them fail.
- [ ] **Step 3:** Implement. On `ok`, the own pet plays the existing wave+poof "send" animation, then returns after 4 s. Reuse the function that `send_guest_home`/own sending uses: `pet.start_wave_poof()`, or whatever the existing name is — find it with `grep -n "poof" crates/screenpet-core/src/brain*.rs`. Do not leave the screen through the network.
- [ ] **Step 4:** Run the Python tests, `cargo check` for Windows, and the core tests.
- [ ] **Step 5:** Commit `feat(app): 💌 개발자에게 건의하기 window`.

### Task 6: App — 📬 편지함 + letter pets (developer only)

**Files:**
- Create: `ui/mailbox.html`, `ui/mailbox.js`.
- Modify:
  - `crates/screenpet-core/src/config.rs`: `dev_key: Option<String>` (hex, `#[serde(default, skip_serializing_if = "Option::is_none")]`). Test that it round-trips and that an invalid key reads as None.
  - `src-tauri/src/feedback.rs`: open letters with `dev_key`; keep `Vec<OpenedLetter>`; commands `mailbox_list`, `mailbox_read(id)`, `mailbox_delete(id)`, `mailbox_block(from)`; event `mailbox`.
  - `tray.rs`: "📬 편지함 (n)" only when `dev_key` is valid.
  - `network.rs`: on connect with `dev_key`, call `mailbox_list`. On `LetterArrived`, open the letter and queue a letter pet. On start-up, queue up to 3 unread letters, 30 s apart.
  - `slots.rs`/`network.rs`: spawn a local guest `Role::Guest(GuestInfo { visit_id: "letter-<id>", owner_id: "", owner_name: "💌 <name>", note: Some(first 80 chars), can_grow: false })`. It enters from a random side, poofs after 60 s or when its letter is read, and never touches the network. Not while focus/house mode refuses arrivals (`pet.refuses_arrivals()`).
- Test: `tools/test_feedback_ui.py` (menu only with dev_key, mailbox rendering escapes text — uses `textContent` only, delete/block buttons invoke the commands), and core config tests.

- [ ] **Step 1:** Write the failing tests (Python + config).
- [ ] **Step 2:** Run them and see them fail.
- [ ] **Step 3:** Implement. Clicking a letter pet → open the mailbox window on that letter.
- [ ] **Step 4:** Run all tests and the Windows check.
- [ ] **Step 5:** Commit `feat(app): 📬 developer mailbox and letter-carrying pets`.

### Task 7: Docs, deploy, release notes

- [ ] `ui/help.html`: a 💌 건의하기 line (one per day, 1000 characters, sealed so only the developer can read it).
- [ ] `docs/releases/v0.6.0.md` (Korean, like the earlier notes).
- [ ] Bump the version to 0.6.0 (the files listed in memory) — only when the user says to release.
- [ ] Deploy the relay:
  1. Build `cargo build --release -p screenpet-relay`.
  2. `install -m 755 target/release/screenpet-relay ~/.local/bin/`.
  3. `systemctl --user restart screenpet-relay`, then check `status`.
  4. Then `screenpet-relay admin mailbox-reader add <dev install key>` for each of the user's computers. The keys come from the app log line or `admin status`.
- [ ] Run the full Python suite under Python 3.12 (scratchpad env) plus the core/net/relay cargo tests before tagging.
