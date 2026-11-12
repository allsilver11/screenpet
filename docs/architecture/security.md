# 보안 정책·기능·코드 위치

> **요약.** 신원은 설치마다 하나인 X25519 정적 키이고, 친구 사이는 Noise XX로 끝에서 끝까지 암호화됨 (릴레이는 암호문만 봄). 처음 만남은 이모지 6개 SAS를 두 사람이 눈으로 확인해야 친구가 됨.
> 릴레이는 믿지 않는 통로로 설계됨: 앱은 릴레이 키를 고정·증명받고, 건의 편지는 개발자 키로 봉인되어 릴레이가 못 읽고, 업데이트는 릴레이 밖 비밀 키의 Ed25519 서명이 맞아야만 믿음.
> 모든 한도는 고칠 수 있는 앱이 아니라 릴레이가 지키고, 친구가 보낸 값은 받는 앱이 허용 목록·범위로 다시 다듬음.
>
> 기준 커밋: `ba6634b`

각 항목: **정책** → **구현** → **코드**.

---

## 1. 신원과 암호화

### 1.1 기기(설치) 키

- 정책: 설치 하나 = 장기 X25519 비밀키 하나. 설정 파일과 따로 두고 사용자만 읽음.
- 구현: config 폴더 `identity.key`(hex). Unix 폴더 0700·파일 0600, Windows는 `%APPDATA%` 사용자 ACL. 깨졌으면 `.bak`으로 남기고 새로 만듦 (그러면 친구와 다시 짝짓기). 다른 읽기 오류면 덮어쓰지 않고 친구 기능을 끔.
- 코드: `crates/screenpet-core/src/config.rs:755` `load_or_create_identity_key`.

### 1.2 친구 사이 Noise

- 정책: LAN이든 릴레이든 같은 암호 세션. 재전송·순서 바꿈·위조 프레임은 링크를 끊음.
- 구현: `Noise_XX_25519_ChaChaPoly_BLAKE2s`, 프롤로그 `screenpet-net/1`, 다이얼한 쪽이 initiator. 핸드셰이크 뒤 첫 메시지 `hello`의 id가 우리가 고정한 키와 맞고 상대도 우리를 고정했을 때만 친구 링크. 키가 다른 id·다른 id 밑의 고정 키는 거절.
- 코드: `crates/screenpet-net/src/noise.rs:27-29`, `peers.rs` 모듈 설명, 복호 실패 `peers.rs:149`.

### 1.3 이모지 SAS 짝짓기

- 정책: 처음 만나는 키는 두 사람이 같은 이모지 6개를 확인해야 서로 고정. 중간자가 키를 갈아 맞출 수 없어야 함.
- 구현: `invite{commit = BLAKE2s(nonce_i)}` → `pair_random{r_r}` → `pair_reveal{nonce_i}` → SAS = `BLAKE2s(handshake_hash‖nonce_i‖r_r)` 앞 36비트 → 64종 이모지 6개. 시도당 성공 확률 2^-36. 초대자가 다이얼한 링크에서만, 20초 안 양쪽 `invite_reply{accepted}`.
- 코드: `noise.rs:32` `SAS_EMOJI`, `:48` `sas`, `protocol.rs:338-345`, `lib.rs` `Timing::invite_timeout`.
- 남용 제한: 동시 짝짓기 요청 4, 주소당 2, 주소당 창마다 시도 3, `Invited` 이벤트 분당 10 (`crates/screenpet-net/src/lib.rs:72-82`).

### 1.4 친구 코드

- 정책: 같은 망 밖 친구를 찾는 짧은 코드. 코드는 *만남*만 허락하고 신원 확인은 여전히 SAS.
- 구현: Crockford base32 8자(40비트), 24시간 수명, 쓰면 60초 동안 채널 1개 허가. 틀린·자기·오프라인 코드는 모두 `unknown` (구분 정보 없음). 추측 제한: 연결당 분당 10, 주소당 분당 30, 전체 600/분(네트워크 공정 몫), 코드당 시간당 서로 다른 요청자 10.
- 코드: `crates/screenpet-relay/src/proto.rs:43` `new_code`, `hub/presence.rs:215` `redeem`, `conn.rs:24`, `lib.rs` `Limits` (`redeems_*`).

### 1.5 릴레이 로그인 증명 (양방향)

- 정책: 남이 내 키로 릴레이에 붙어 우편함을 가로채지 못함. 앱도 진짜 릴레이인지 확인.
- 구현:
  - 앱 → 릴레이: `mac = BLAKE2s("screenpet-relay/1 auth"‖X25519(client, relay)‖nonce‖relay_pub‖client_pub‖id)`. 릴레이는 저차수 키(공유값 0) 거절, 상수 시간 비교.
  - 릴레이 → 앱: `proof = BLAKE2s("screenpet-relay/1 welcome"‖X25519‖cnonce‖nonce‖client_pub)`. 앱은 proof 없거나 틀리면 끊음.
- 코드: `crates/screenpet-crypto/src/lib.rs:26` `auth_mac`, `:31` `welcome_proof`, `crates/screenpet-relay/src/conn.rs:353` `authenticate`, `proto.rs:20` `dh`, `:30` `ct_eq`, `crates/screenpet-net/src/relay.rs:849-856`.

### 1.6 릴레이 키

- 정책: 앱이 릴레이 공개키를 고정함.
- 구현: 기본 `DEFAULT_RELAY_KEY`. challenge의 키가 다르면 접속 거부. 릴레이 개인키는 데이터 폴더 `relay.key`(0600). `relay_url.txt`로 다른 릴레이·키 지정 가능 (키 줄이 없으면 고정 없음).
- 코드: `crates/screenpet-net/src/relay.rs:34`, `:826`, `crates/screenpet-relay/src/store.rs:429` `load_or_create_key`, `src-tauri/src/network.rs:465` `relay_settings`.

### 1.7 📱 연결된 기기

- 정책: 내 두 컴퓨터가 친구 목록을 나누려면 코드 + 양쪽 사용자 확인 + 링크마다 비밀 증명.
- 구현: 8자 코드(10분, 5번 틀리면 취소), 코드는 선에 안 실리고 K = BLAKE2s(label‖code)를 아는지 증명 (쥔 쪽은 먼저 증명하지 않음 → 오프라인 추측 불가). 장기 비밀 S 저장, 이후 링크마다 새 nonce로 S 증명.
- 코드: `crates/screenpet-net/src/devices.rs:42-57`, 모듈 설명.

---

## 2. 봉인된 건의 편지 (💌)

- 정책: 편지는 개발자만 읽음. 릴레이는 저장만 하고 내용을 못 봄. 보낸이에 대해 이름·펫 말고는 안 보냄 (주소·친구 없음).
- 구현:
  - 앱이 `Noise_N_25519_ChaChaPoly_BLAKE2s`(프롤로그 `screenpet-letter/1`)로 `letter::DEV_KEY`에 봉인. 본문 1000자, 봉인 ≤ 8 KiB.
  - 개발자 비밀키는 개발자 컴퓨터 `config.json`의 `dev_key`에만. 다른 사용자의 설정에는 그 필드가 직렬화되지 않음. 저장소에는 공개키만 있음. 새 쌍: `cargo run -p screenpet-net --example dev_key`.
  - 편지 목록·표시·삭제·차단은 릴레이 `control.json`의 `mailbox_readers`에 있는 설치 키만 (`admin mailbox-reader add|remove`). 아니면 조용히 무시.
- 코드: `crates/screenpet-net/src/letter.rs:11` `DEV_KEY`, `:22-24`, `:124` `seal`, `:136` `open`; `src-tauri/src/feedback.rs` `seal`; `crates/screenpet-core/src/config.rs:56` `dev_key`; `crates/screenpet-relay/src/hub/mail.rs:17` `is_reader`, `:79` `reader_edit`; `control.rs:30` `mailbox_readers`.

---

## 3. 공개 건의함 (📮) 개인정보

| 정책 | 구현 | 코드 |
|---|---|---|
| 이름은 첫 글자(그래핌)만 | `김철수` → `김**`, 한 글자 이름 → `*` | 앱 `crates/screenpet-net/src/board.rs:31` `mask_name` |
| 본문은 첫 줄만 | 60자 + `…`. 본문은 개발자 컴퓨터를 안 떠남 | `board.rs:19` `LINE_CHARS`, `:46` `first_line`, `:66` `digest` |
| 릴레이가 다시 검사 | 이름: 첫 그래핌(≤ 8 문자) 뒤 `*`만, ≤ 16자. 줄: 1–80자. 제어·방향·폭 없는 문자 제거. 편지가 있어야 함. 독자만, 한 번에 ≤ 20 | `crates/screenpet-relay/src/board.rs:28` `hidden`, `:41` `check_name`, `:56` `check_line`; `mailbox.rs:286` `board_put`; `hub/mail.rs:114` |
| 🔒 비공개 | 편지는 편지함에 남고 `board_page`의 항목·전체·쪽 수에서 빠짐. 앱도 비공개는 안 올림 | `mailbox.rs:310` `board_page`, `:244` `mark_private`; `src-tauri/src/mailbox.rs` `unpublished` |
| 삭제·차단 연쇄 | 편지 지우기·보낸이 차단 → 요약도 지움 (`prune_board`), 차단 키는 이후 편지 거절 | `mailbox.rs:266` `delete`, `:276` `block` |
| 미리 알림 | 편지 창에 "보낸 편지의 첫 줄과 이름 첫 글자는 📮 건의함에서 다른 사용자에게도 보여요." | `ui/feedback.html:16` |
| 조회 남용 | `board_list`는 연결마다 0.5초에 1번 (더 빠르면 답 없음). 앱은 0.6초 간격 | `Limits::board_gap`, `hub/mail.rs:136`; `crates/screenpet-net/src/relay.rs:98` |

---

## 4. 업데이트 무결성 (🔄)

| 정책 | 구현 | 코드 |
|---|---|---|
| 서명된 목록만 믿음 | `latest.json` 바이트 그대로에 대한 Ed25519 서명(`latest.json.sig`, hex 128자)이 `UPDATE_PUBLIC_KEY`와 맞아야 파싱. 틀리면 3초 뒤 한 번만 다시 (게시 중 교체 대비) | `crates/screenpet-core/src/update.rs:16` `UPDATE_PUBLIC_KEY`, `:211` `verify_latest`; `src-tauri/src/update.rs:171` `fetch_latest` |
| 비밀 키는 릴레이 밖 | `~/personal/screenpet-update-key.txt`, git 제외(`.gitignore` `screenpet-update-key*.txt`), systemd `InaccessiblePaths=-%h/personal`로 릴레이 프로세스가 못 읽음. 서명은 `screenpet-update-sign`만 | `deploy/relay/screenpet-relay.service:36`, `deploy/publish-release.sh:34`, `crates/screenpet-core/src/bin/screenpet-update-sign.rs` |
| 호스트 허용 목록 | `https://pet.danni.day`만. 사용자 릴레이 설정과 무관한 고정 주소 | `update.rs:20` `ALLOWED_HOST`, `:237` `allowed_url` |
| 리다이렉트 | 자동 추적 끔(`max_redirects(0)`), 매번 `redirect_target`이 같은 호스트 https인지 확인, 최대 5번 | `update.rs:24`, `:247`; `src-tauri/src/update.rs:131` `agent`, `:143` `get` |
| 크기·해시 | 크기 1..=300 MB, `size + 1`바이트까지만 읽음, 정확히 `size`여야 함, SHA-256 일치 | `update.rs:22` `MAX_DOWNLOAD`, `:262` `check`; `src-tauri/src/update.rs:212` `download_into` |
| 원자적 파일 | 사용자 임시 폴더 `screenpet-update/`를 비우고 `<name>.part`에 쓰기 → fsync → 해시 확인 → rename. 실패면 폴더 삭제 | `src-tauri/src/update.rs` `download`, `download_into` |
| 이름 검사 | 이 플랫폼 이름 그대로여야 함 (`../`, 절대 URL 거절) | `update.rs` `check`, 시험 `bad_latest_json_is_rejected` |
| Windows 설치 | NSIS `/P /R /UPDATE` 인자 배열로 실행 (셸 없음) | `update.rs:31` `NSIS_ARGS`, `src-tauri/src/update.rs:264` `install_windows` |
| macOS 설치 | `hdiutil attach -nobrowse -readonly`, `/bin/sh -c MAC_SWAP_SCRIPT sh <pid> <mnt> <src> <dst> <tmp>` — 경로는 **인자로만** (문자열에 끼워 넣지 않음). 새 번들을 옆에 복사 후 rename, 실패면 옛 번들 복구 | `update.rs:307` `MAC_SWAP_SCRIPT`, `src-tauri/src/update.rs:277` `install_macos` |
| 롤백 방지 | 서명이 맞아도 지금 버전보다 새 버전이어야 설치 (`same_or_older_version_is_up_to_date`) | `update.rs:262` `check` |
| 사용자 동의 | 조용한 확인(60초 뒤, 2시간마다)은 메뉴 글자만 바꿈. 누르기 전엔 받지도 깔지도 않음 | `update.rs:26-28`, `src-tauri/src/update.rs:80` |
| 서버 쪽 게시 | GitHub digest·size 확인 → 서명 → 앱 키로 검증 → 설치 파일 먼저, `latest.json` 나중 | `deploy/publish-release.sh:101-144` |

- 동결·재생(freeze/replay): `latest.json`에 만료가 없음. 서버를 쥔 사람은 옛 *서명된* `latest.json`을 계속 내줘 새 업데이트를 숨길 수 있음 (동결). 더 옛 버전으로 내리는 재생은 "새 버전만" 규칙으로 막힘.
- macOS 스왑 스크립트가 새 번들의 `com.apple.quarantine` 속성을 지움 (`MAC_SWAP_SCRIPT`). 해시 확인을 마친 파일에만 적용됨.

---

## 5. 릴레이 남용 방지

| 위협 | 대책 | 코드 |
|---|---|---|
| 연결 폭주 | 전체 2048, 주소 16, 네트워크(/24·/48) 128. 인증 전 512/8/32, 차면 가장 오래된 인증 전 연결을 내보냄. 10초 인증 마감, 90초 idle. 스레드 `max_conns + 16` 상한 | `lib.rs:132-183`, `hub/presence.rs:10` `admit_ip`, `hub/mod.rs:280` `evict_unauthed`, `:453` |
| 브라우저 악용 | `Origin` 머리 있는 WS 요청은 403 | `http.rs:146` |
| 주소 위조 | 믿는 프록시(+루프백)에서만 `X-Real-IP`/가장 오른쪽 XFF | `http.rs:96` `client_ip` |
| 등록 폭주 | /24·/64 시간당 30, /24·/48 하루 100, 전체 시간당 2000, 저장 2만 | `hub/presence.rs:124-129` |
| 메시지 폭주 | 연결당 200 msg/s·512 KiB/s, 채널 방향당 20 프레임/s, `friends` 0.5/s(burst 30), `weather_want` 0.1/s(burst 5) | `conn.rs:24-37`, `:203`, `:284` |
| 대역폭 | 주소 2 MiB/s(재연결해도 안 참), 전체 16 MiB/s(네트워크 몫 1/16), 천장 32 MiB/s | `hub/routing.rs:6`, `:102` `bandwidth_allows` |
| 느린 수신자 | 연결 큐 2 MiB 넘으면 채널 닫음, 5초 안 움직이면 `slow`. 전체 큐 256 MiB | `hub/routing.rs:120` `enqueue_frame` |
| 크기 | 텍스트 16 KiB, 프레임 4 + 65535, 요청 머리 8 KiB·5 s | `conn.rs:157-160`, `http.rs:16-18`, `:113` |
| 편지 | 연결당 분당 3, 키당 하루 **5**, 주소당 10, 전체 50, 2일 이상 본 키만, 200통에서 거절(옛 편지는 안 버림), 차단 키 거절 | `conn.rs:26`, `mailbox.rs:14-28`, `:156` `submit` |
| 건의함 조회 | 0.5초 간격 | `Limits::board_gap` |
| 다운로드 | 이름 허용 목록, 심볼릭 링크 거절, 동시 8·주소당 2, 30분·64 KiB/s·10초 정지 컷 | `downloads.rs:35`, `:127`, `:144` |
| 날씨 | 앱은 고정 18곳의 **id만** 보냄. 표 밖 id·좌표 문자열은 무시 → 임의 좌표로 외부 요청을 시킬 수 없음 | `crates/screenpet-proto/src/weather.rs:62` `find_place`, `crates/screenpet-relay/src/weather.rs:116` `want` |
| 차단 | `banned_keys`, `banned_ips`(IPv6 /64), `relay off`, `registration off`, `friendcode off`. `control.json`이 깨지면 시작 거부 | `control.rs:18`, `:47`; `admin.rs:19` |
| 속도 표 메모리 | 표당 키 8192, 네트워크당 16 | `hub/limits.rs:69`, `:90` |

---

## 6. 친구가 보낸 값 검사 (앱)

| 값 | 규칙 | 코드 |
|---|---|---|
| 물건 종류 | `ObjectKind::ALL` 허용 목록만(옛 `dropping` → 똥). 목록에 없는 📌 `app_icon`은 받지 않음 (장난감은 이 컴퓨터를 안 떠남) | `crates/screenpet-core/src/object.rs:95-101`, `:258` `from_id`, `:1177` `accept_toss` |
| 장난감 보내기 | 던지기·들고 가기에서 `is_toy`면 안 실음 (코어 + 앱 이중) | `object.rs:153` `is_toy`; `src-tauri/src/objects.rs:479` |
| 들고 오는 물건 | 거대한 것은 거절 | `object.rs:1187` `accept_carry` |
| 거대한 것 | 화면에 최대 3개(넘치면 가장 오래된 것 제거), 들어올 때 머리 위 남은 자리의 80%까지만 솟음 | `object.rs:23` `MAX_GIANTS`, `:26` `GIANT_ARRIVAL_HEADROOM` |
| 받은 물건 수 | 친구 물건 최대 20, 똥 10 | `object.rs:19-33` |
| 랠리 | 정수만, `MAX_RALLY`(20)로 자름 | `crates/screenpet-net/src/protocol.rs:136` `lenient_rally` |
| 몸 색(tint) | 아는 id만, 아니면 기본 잉크 | `crates/screenpet-core/src/tint.rs:88` `from_wire` |
| 쪽지·답장 | 제어·방향·폭 없는 문자 제거, 80자, 비면 없음. 똥·거대한 것엔 포스트잇 안 받음. 답장은 그 친구의 그 방문의 귀가 `Leave`에서만 | `crates/screenpet-core/src/visit.rs:218` `sanitize_note`, `:226` `accept_reply` |
| 펫 크기 | 손님 0.3–2.0배, 먹이 boost는 1..cap, 똥 크기 범위 | `visit.rs:31` `GUEST_SCALE_RANGE`, `:266` `guest_scale`, `:521` `sanitize_boost`; `poop.rs:69` `sanitize_size` |
| 던지는 속도 | `Fling::sanitized` + `clamp_speed(MAX_FLING_SPEED)`, 높이 0..1 | `object.rs:1179-1180`, `visit.rs:147` |
| 종 | 번들에 없는 species면 손님 거절 | `src-tauri/src/network.rs:1851` (`welcome_guest`) |
| 이름 | 64자로 자름 | `protocol.rs` `clamp_name`, `MAX_NAME` |
| 폭주하는 친구 | 친구별 토큰 버킷: ask 2(2초당 1), steer 30/s, toss 50(1.2초당 1), visit 20(0.5초당 1), ladder 20, links 6(5초당 1), codes 16(2초당 1), states 4(1초당 1). 넘으면 버리고 링크는 유지 | `crates/screenpet-net/src/lib.rs:400-430` |
| 깨진 필드 | `lenient` 역직렬화: 그 필드만 버리고 메시지는 살림. 목록은 최대 256개 | `protocol.rs:53-62`, `:158` `lenient_list` |
| LAN 찾기 | 알림은 힌트일 뿐(Noise가 신원 증명). 본 id 64, 후보 8(출처당 2), 출처 IP당 초당 50 데이터그램, 미검증 다이얼 분당 20 | `crates/screenpet-net/src/discovery.rs:21-37`, `lib.rs:51-82` |

---

## 7. 웹뷰 강화

- **CSP** (`src-tauri/tauri.conf.json` `app.security.csp`): `default-src 'self'`, `script-src 'self'`(인라인·eval 없음), `style-src 'self' 'unsafe-inline'`, `img-src 'self'`, `connect-src 'self' ipc: http://ipc.localhost`, `object-src 'none'`, `base-uri 'none'`. 원격 주소 없음.
  - 시험: `tools/test_csp.py` — 인라인 스크립트·핸들러 없음, eval 불필요, `https?://`·`data:`·`blob:`·`asset:`·`createObjectURL`·`toDataURL` 안 씀, 정책이 검토한 그대로인지.
- **창별 권한** (`src-tauri/capabilities/default.json`): 모든 창은 이벤트 듣기만(`core:event:allow-listen/unlisten`), 그 밖엔 창마다 자기 명령만 (예: `mailbox` 창만 `allow-mailbox-*`, `board` 창은 `allow-board-list/close`). `core:default` 넓은 권한은 어느 창도 없음.
  - 시험: `tools/test_capabilities.py` — 페이지가 부르는 명령 ⊆ 그 창 권한, Tauri API는 명령·듣기만, `core:default` 없음, `build.rs` 명령 목록 = `generate_handler!`.
- **사용자 글은 `textContent`로만**: 편지함·건의함·쪽지 (`ui/mailbox.js:3`, `ui/board.js:3`, `src-tauri/src/objects.rs` 모듈 설명). `innerHTML`은 `ui/main.js:142`의 고정 문자열 하나뿐.
- **`data:`/`blob:` 없음**: 📌 앱 아이콘 그림도 창 초기화 스크립트로 넘겨 canvas에 그림 (CSP 그대로) (`src-tauri/src/objects.rs` 모듈 설명).

---

## 8. 로컬 개인정보

| 항목 | 정책 | 코드 |
|---|---|---|
| ⌨️ 타자 응원 | 개수만 셈. 어떤 키·글자인지 읽지도 남기지도 않음. Windows는 키 256개의 지난 틱 눌림 비트로 올라가는 순간만 셈, macOS는 시스템 키 누름 카운터. 저장·로그·네트워크 없음 | `crates/screenpet-core/src/typing.rs:1-6`, `:21` `KeyEdges`, `platform/macos.rs` `key_presses` |
| macOS 입력 모니터링 | 확인은 `CGPreflightListenEventAccess`(창 없음). 묻기는 켜져 있고·권한 없고·**이 버전에서 아직 안 물었을 때만**, 시작 10초 뒤 펫이 먼저 이유를 말하고 2.5초 뒤 시스템 창. 끌 때는 안 물음. 거절했으면 메뉴가 시스템 설정을 엶 | `typing.rs:104` `ask_at_start`, `:126` `PermissionAsk`; `platform/macos.rs` `typing_permission`; `src-tauri/src/tray.rs:90` |
| 📌 앱 아이콘 장난감 | 그림은 창 스크립트로만, 어디에도 저장·기록 안 함, 친구에게 안 감. 한 번에 1개, 몇 분 뒤 사라짐 | `src-tauri/src/objects.rs:11-13`, `object.rs:95-103` |
| ☀️ 날씨 | 앱은 Open-Meteo를 직접 부르지 않음 → 사용자 IP가 날씨 서비스에 안 감. 릴레이는 도시 좌표만 보냄 | `crates/screenpet-relay/src/weather.rs:1-4`, `:40` |
| `network.log` | config 폴더, 시각(유닉스 초) + `relay connected/disconnected`, `friend <id 앞 8자> connected/disconnected`만. 내용·메시지 없음. 256 KiB 넘으면 새로 씀 | `crates/screenpet-core/src/config.rs:722-730`, `src-tauri/src/network.rs:670` `net_log` |
| 🕐 타임라인 | 메모리에만, 저장·전송 없음 | `src-tauri/src/timeline.rs` 모듈 설명 |
| 릴레이 저장 | 설치 키, 스스로 밝힌 id, 친구 키, 코드, 마지막 본 때. 메시지 내용 없음. `admin status`의 주소는 메모리만 | `crates/screenpet-relay/src/store.rs:1-2`, `hub/mod.rs:100` |

---

## 9. 비밀·저장소 위생

- `.gitignore`: `screenpet-update-key*.txt`(서명 비밀 키), `tools/preview/`, `target/`, `src-tauri/gen/`, `.claude/` 등.
- 저장소에 있는 키는 모두 **공개키**: `UPDATE_PUBLIC_KEY`, `DEFAULT_RELAY_KEY`, `letter::DEV_KEY`.
- 비밀은 저장소 밖: 업데이트 키 `~/personal/`, 릴레이 키·편지 `~/.local/share/screenpet-relay/`(0700/0600, `UMask=0077`), 개발자 편지 키 개발자 `config.json`, 기기 키 `identity.key`.
- 로그는 저장소에 안 둠: 릴레이는 journald(`journalctl --user -u screenpet-relay`), 앱은 config 폴더.
- CI는 `--locked`, Python 의존성 해시 고정, `persist-credentials: false`, 써드파티 액션 커밋 고정 (`.github/workflows/build.yml`).

---

## 10. 알려진 빈틈 / 받아들인 위험

| 항목 | 상태 | 영향 |
|---|---|---|
| 앱 코드 서명 없음 | macOS `signingIdentity: "-"`(ad-hoc), Windows NSIS 서명 단계 없음 (`tauri.conf.json`, `build.yml`에 서명 없음) | 첫 설치 때 Gatekeeper·SmartScreen 경고. ad-hoc이라 업데이트마다 macOS 입력 모니터링 권한이 풀릴 수 있음 (그래서 버전마다 한 번 다시 물음, `typing.rs:102-103`) |
| 업데이트 키가 릴레이와 같은 호스트 | `InaccessiblePaths`로 릴레이 *프로세스*에선 안 보임. 같은 사용자 계정의 다른 침해에는 무방비 | 호스트 계정이 털리면 서명 가능 → 별도 기기 서명이 이상적 |
| `latest.json` 만료 없음 | 롤백은 막지만 동결은 못 막음 | 서버 장악 시 업데이트를 숨길 수 있음 |
| 공개 건의함 자동 게시 | 개발자 앱이 편지를 열면 🔒 표시 없는 편지는 자동으로 요약 게시 (`src-tauri/src/mailbox.rs` `on_list`/`unpublished`). 기본 공개, 편지 창에 미리 알림 | 개발자가 늦게 🔒 하면 그 사이 첫 줄이 보임 |
| `relay_url.txt` 키 생략 | 둘째 줄이 없으면 릴레이 키 고정 없음 | 사용자가 직접 바꾼 경우만. 친구 내용은 여전히 Noise로 보호 |
| 릴레이 메타데이터 | 릴레이는 누가 누구와 언제 채널을 여는지, 프레임 크기를 봄 | 내용은 못 봄 |
| SAS는 사람 확인 | 사용자가 이모지를 안 보고 누르면 중간자 가능 | UX로만 완화 |
