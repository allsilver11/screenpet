# 구조화 로그

릴레이 서버와 앱이 같은 한 줄 형식으로 로그를 씀. 어디서 무엇이 잘못됐는지 `grep` 한 번으로 좁히려는 것임.

- 형식·가리기: `crates/screenpet-log/src/lib.rs` (`log!` 매크로, `format_line`, `mask_ip`, `short`, `scrub_err`)
- 릴레이: stderr → journald. 창·고리는 `crates/screenpet-relay/src/monitor.rs`
- 앱: config 폴더의 `network.log` (+ stderr). 싱크는 `src-tauri/src/network.rs` `install_log_sink`
- 검사: `tools/test_logging.py` (목록·개인정보), `tools/test_no_logs_in_git.py` (로그 파일이 git에 안 들어감)

## 1. 형식

```
2026-11-13T15:04:05.123+09:00 INFO  hub      login        conn=c42 peer=f7da89ae ip=14.33.162.x v=3 new=0 friends=4 online=2
```

| 칸 | 내용 |
|---|---|
| 시각 | KST(+09:00 고정) ISO-8601, 밀리초까지. UTC 안 씀. tz 데이터베이스 안 씀 |
| 수준 | 5칸 맞춤: `INFO `, `WARN `, `ERROR` |
| 영역 | 8칸 맞춤, 고정 낱말 (아래 표) |
| 사건 | 12칸 맞춤, 고정 snake_case 이름 (아래 목록) |
| 필드 | `key=value`를 부른 순서대로. 값에 빈칸·따옴표·`=`가 있거나 비었으면 `"…"`로 감싸고 `\"`·`\\`·`\n` 이스케이프 |

표준 키:

| 키 | 뜻 |
|---|---|
| conn | 릴레이 연결 번호 (`c42`). 릴레이 실행마다 accept에서 하나씩 늘어남. accept부터 close까지 같은 번호 |
| peer | 설치 공개키 hex 앞 8자. 앱은 자기 키 앞 8자를 같은 이름으로 적음 → 양쪽 로그를 맞추는 열쇠 |
| ip | 가린 주소: IPv4 마지막 옥텟 → `x`, IPv6 앞 네 묶음 + `::x` |
| v | 릴레이 프로토콜 버전 |
| took_ms | 걸린 밀리초 (느린 길: 저장, 날씨, 편지 저장, 다운로드, 앱 업데이트) |
| err | 짧은 오류 글. 경로 낱말은 `<path>`로 바뀌고 160자에서 잘림 |
| n, bytes | 개수, 바이트 |
| friend, to, from, owner, via | 친구 설치 id(앱)·키(릴레이) 앞 8자 |
| reason | 고정 낱말 까닭 (`idle`, `rate`, `not_friends` …) |

## 2. 수준과 영역

| 수준 | 언제 |
|---|---|
| INFO | 보통 사건 (접속, 로그인, 채널, 편지 받음) |
| WARN | 수상하거나 되살릴 수 있는 일: 한도, 나쁜 메시지, 거절, 다시 해 보기, 느린 저장 |
| ERROR | 고장: 디스크 I/O 실패, 다시 해 봐도 실패(날씨 3번 잇달아), 서명 불일치 |

| 쪽 | 영역 | 다루는 것 |
|---|---|---|
| 릴레이 | conn | TCP·WebSocket 연결: accept, 거절, 인증, 나쁜 메시지, 한도, close |
| 릴레이 | hub | 로그인·로그아웃, 시작·멈춤, 프록시·fd 경고 |
| 릴레이 | presence | 친구 목록, 친구 코드 |
| 릴레이 | route | 채널 열기·실패·끊기 |
| 릴레이 | mail | 📮 건의 편지, 독자 편집, `letters.json` |
| 릴레이 | board | 📮 공개 건의함 |
| 릴레이 | weather | ☀️ 날씨 받기 (Open-Meteo, 예비 MET Norway) |
| 릴레이 | dl | 🔄 `/downloads/` |
| 릴레이 | admin | 관리 명령·소켓, 내보내기 |
| 릴레이 | store | `relay.json` 읽기·쓰기 |
| 앱 | net | 친구 연결, 릴레이 붙음/끊김, 기기 연결, 네트워크 꺼짐 |
| 앱 | relay | 릴레이 클라이언트 (screenpet-net): 로그인, 거절, 닫힘 |
| 앱 | update | 🔄 업데이트 확인·받기·설치 |
| 앱 | visit | 놀러 가기·손님 |
| 앱 | toss | 물건 던지기·받기 |

## 3. 사건 목록

### 릴레이

| 영역 | 사건 | 수준 | 필드 | 뜻 |
|---|---|---|---|---|
| conn | `accept` | INFO | conn ip | WebSocket 연결을 받아들임 (한도 안) |
| conn | `refused` | WARN | conn ip reason | 받기 전 거절: `busy`(한도), `banned`, `relay_off`, `origin`(웹 페이지) |
| conn | `auth_fail` | WARN | conn ip | 인증 MAC 틀림 (대는 키는 증명 안 됐으므로 안 적음) |
| conn | `bad_message` | WARN | conn peer reason | `bad_json`, `too_large`, `unexpected`, `bad_frame` → 끊음 |
| conn | `rate_limited` | WARN | conn peer what | 연결 한도: `messages`·`friends`면 끊음, `redeem`·`letter`·`new_code`·`weather_want`는 그 요청만 거절 |
| conn | `close` | INFO | conn peer reason up_s | 연결 끝. reason: `client_close`, `idle`, `timeout`(인증 안 함), `gone`(소켓 끊김), `write_fail`, `rate`, `slow`, `replaced`, `kicked`, `banned`, `busy`, `shutdown`, `relay_off`, `auth`, 거절 까닭 등 |
| conn | `thread_refused` | WARN | conn | 스레드가 너무 많아 연결을 버림 |
| conn | `accept_fail` | ERROR | err | accept 실패 (보통 fd 바닥, 1분에 한 번만) |
| hub | `login` | INFO | conn peer ip v new friends online | 인증·로그인 됨. new=1 새 설치, friends=알린 친구 수, online=서로 친구이면서 접속 중 |
| hub | `login_refused` | WARN | conn peer ip reason | `banned`, `registration_closed`, `busy`(새 설치 한도), `full`, `relay_off` |
| hub | `login_replaced` | WARN | conn peer ip old_conn old_ip | 같은 키가 다시 로그인해 옛 연결을 밀어냄 (자주면 신원 복사 의심) |
| hub | `logout` | INFO | conn peer channels | 허브에서 나감 (밀려난 옛 연결은 안 적음) |
| hub | `unauthed_evicted` | WARN | conn ip | 인증 전 연결 풀이 차서 가장 오래된 것을 내보냄 |
| hub | `proxy_no_address` | WARN | | 믿는 프록시가 X-Real-IP를 안 줌 (실행마다 한 번) |
| hub | `fd_limit` | WARN | limit n | fd 한도 때문에 연결 상한을 n으로 낮춤 |
| hub | `start` | INFO | bind v relay_pub proxies weather | 시작 (relay_pub = 릴레이 공개키 앞 16자) |
| hub | `stop` | INFO | | SIGTERM·SIGINT로 멈춤 |
| hub | `start_fail` | ERROR | err | 시작 못 함 |
| presence | `friends` | INFO | conn peer n online | 친구 목록 갱신 |
| presence | `redeem_ok` | INFO | conn peer to | 친구 코드 사용 성공 (코드는 안 적음) |
| presence | `redeem_fail` | WARN | conn peer reason | `unknown`, `rate`, `disabled` |
| presence | `new_code` | INFO | conn peer | 새 친구 코드 요청 |
| route | `channel_open` | INFO | conn peer to ch | 채널 열림 |
| route | `channel_fail` | WARN | conn peer to reason | `not_friends`, `offline`, `busy`, `self`, `bad_key` |
| route | `channel_cut` | WARN | conn peer ch reason | 넘기다 닫음: `rate`(속도·대역폭), `queue`(받는 쪽 큐 참) |
| mail | `letter_in` | INFO | conn peer id bytes took_ms | 건의 편지 받고 저장 (내용 없음, took_ms = 저장) |
| mail | `letter_refused` | WARN | conn peer reason | `daily`, `ip`, `new`, `global`, `full`, `size`, `blocked` |
| mail | `letters_list` | INFO | conn peer n | 독자가 편지함을 받음 |
| mail | `mail_edit` | INFO | conn peer op id took_ms | 독자 편집: `read`, `done`, `reject`, `private`, `delete`, `block`(id 0) |
| mail | `save_fail` | ERROR | took_ms err | `letters.json` 쓰기 실패 (다음에 다시 씀) |
| mail | `load_fail` | ERROR | file | `letters.json`(또는 `.bak`)을 못 읽음 |
| board | `board_put` | INFO | conn peer n changed | 독자가 공개 요약을 올림 (이름·글 없음) |
| board | `board_list` | INFO | conn peer page n | 공개 건의함 한 쪽을 줌 |
| weather | `fetch_ok` | INFO | src place wmo took_ms | 지역 날씨 받음. src = `open_meteo` 또는 `met`, wmo = 날씨 코드 (MET은 기호를 WMO로 바꾼 값) |
| weather | `fetch_fail` | WARN, ERROR | src place n took_ms err | 서비스 하나 실패 (곧바로 다른 서비스를 물음). n = 그 서비스·지역의 잇단 실패 수. 두 서비스 다 실패했고 n ≥ 3이면 ERROR, 그때는 마지막 좋은 값을 두고 5분 뒤 다시 함. `err="backing off after HTTP 429/403"`(took_ms 0) = 그 서비스는 1시간 쉬는 중이라 안 물음 |
| dl | `meta` | INFO | conn ip file head ok | `latest.json`·`.sig` 요청 (앱 업데이트 확인) |
| dl | `download_start` | INFO | conn ip file bytes | 설치 파일 보내기 시작 |
| dl | `download_done` | INFO | conn file bytes took_ms | 다 보냄 |
| dl | `download_cut` | WARN | conn file reason bytes took_ms | 끊음: `slow`, `deadline`, `stopping`, `write_fail`(받는 쪽 끊김), `read_fail`, `short_file` |
| dl | `download_busy` | WARN | conn ip file | 동시 다운로드 한도 → 503 |
| dl | `not_found` | WARN | conn file | 허락된 이름인데 파일 없음 (릴리스 덜 올림) |
| admin | `command` | INFO | cmd ok | 관리 명령 (명령 낱말만, 인자 없음) |
| admin | `dropped` | INFO | conn peer reason | 관리 명령으로 내보냄: `kicked`, `banned`, `purged` |
| admin | `conn_fail` | WARN | err | 관리 소켓 연결 하나가 깨짐 |
| admin | `socket_fail` | ERROR | err | `admin.sock`을 못 만듦 (관리 명령 못 씀) |
| store | `save_slow` | WARN | took_ms | `relay.json` 저장이 500ms 넘음 |
| store | `save_fail` | ERROR | took_ms err | `relay.json` 쓰기 실패 (다음 주기에 다시) |
| store | `load_fail` | ERROR | file err | 데이터 파일을 못 읽음 |
| store | `load_backup` | WARN | file | `.bak`으로 시작함 |
| store | `load_empty` | ERROR | | 읽을 데이터 파일이 없어 빈 저장소로 시작 |

### 앱 (`network.log`)

| 영역 | 사건 | 수준 | 필드 | 뜻 |
|---|---|---|---|---|
| relay | `login` | INFO | peer v friends online took_ms | 릴레이 로그인 성공. peer = 내 키 앞 8자, v = 릴레이 버전 |
| relay | `connect_fail` | WARN | err | 릴레이에 못 붙음 (다시 해 보는 동안 첫 번만) |
| relay | `refused` | WARN | peer reason | 로그인 거절 (`banned`, `busy`, `registration_closed` …) |
| relay | `closed` | WARN | peer reason | 릴레이가 `error{reason}`으로 끊음 (`idle`, `replaced`, `rate`, `shutdown` …) |
| relay | `disconnected` | INFO | peer | 붙어 있던 연결이 끝남 |
| relay | `key_mismatch` | ERROR | | 고정한 릴레이 키와 다름 → 거부 |
| relay | `no_proof` | ERROR | | 릴레이가 키 증명을 안 함 → 거부 |
| net | `relay_up` | INFO | peer | 릴레이 연결됨 (메뉴 쪽) |
| net | `relay_down` | WARN | peer | 릴레이 끊김 |
| net | `friend_up` | INFO | friend | 친구 연결됨 (LAN이나 릴레이) |
| net | `friend_down` | INFO | friend | 친구 끊김 |
| net | `friend_removed` | INFO | friend reason | `forgot_us`(친구가 나를 지움), `dropped`(소개자 지움·미확인) |
| net | `device_unlinked` | INFO | friend | 📱 내 다른 컴퓨터 연결이 풀림 |
| net | `device_unreachable` | WARN | | 📱 내 다른 컴퓨터에 못 닿음, 여기서 놂 |
| net | `standby_on` | INFO | friend | 📱 이 컴퓨터가 대기로 감 |
| net | `standby_off` | INFO | reason | 📱 다시 여기서 놂: `in_use`, `pet_changed` |
| net | `code_unavailable` | WARN | reason | 새 친구 코드를 못 받음 (`relay_offline`) |
| net | `link_code_fail` | WARN | friend err | 📱 연결 코드를 못 만듦 |
| net | `disabled` | ERROR | reason err | 네트워크 꺼짐: `config_unreadable`, `identity_key`, `start` |
| net | `discovery_off` | WARN | port err | LAN 찾기 포트를 못 잡음, 릴레이만 씀 |
| update | `available` | INFO | version | 조용한 확인에서 새 버전 찾음 |
| update | `check_fail` | WARN | err | 조용한 확인 실패 |
| update | `get_fail` | WARN | err | HTTP 요청 실패 |
| update | `http_status` | WARN | status | 성공 아닌 HTTP 상태 |
| update | `read_fail` | WARN | file err | 본문 읽기 실패 |
| update | `bad_signature` | ERROR | | `latest.json` 서명이 안 맞음 |
| update | `download_start` | INFO | version bytes | 설치 파일 받기 시작 |
| update | `download_ok` | INFO | version bytes took_ms | 받고 크기·SHA-256 맞음 |
| update | `download_fail` | ERROR | version took_ms err | 받기 실패 (`Network`, `SizeMismatch`, `DigestMismatch`, `Disk` …) |
| update | `install_start` | INFO | version | 설치 시작 |
| update | `install_fail` | ERROR | err | 설치 프로그램·도우미를 못 띄움 |
| update | `update_fail` | ERROR | err | 눌러서 한 업데이트가 실패로 끝남 |
| visit | `send_own` | INFO | friend side with_note | 내 펫을 친구에게 보냄 |
| visit | `visit_out` | INFO | friend visit carried | 내 펫이 친구 화면으로 넘어감 |
| visit | `own_walk_on` | INFO | visit from to | 내 펫이 다음 친구로 걸어감 |
| visit | `came_home` | INFO | friend with_reply | 내 펫이 답장을 들고 돌아옴 |
| visit | `call_home` | WARN | friend reason | 너무 오래 소식 없어 부름 (`silent`) |
| visit | `come_home_timeout` | WARN | | 부름에 답이 없어 그냥 돌아옴 |
| visit | `invited` | INFO | friend | 친구가 초대해서 감 |
| visit | `guest_in` | INFO | visit friend with_note via | 손님이 들어옴 |
| visit | `sent_back` | INFO | visit friend reason | 손님을 바로 돌려보냄: `busy`(집중·하우스), `too_soon` |
| visit | `declined` | WARN | visit friend err | 손님을 받을 수 없음 |
| visit | `guest_home` | INFO | visit with_reply | 손님을 집으로 보냄 |
| visit | `guest_on` | INFO | visit owner to | 손님을 다음 친구에게 보냄 |
| visit | `guest_walk_on` | INFO | visit owner to | 손님이 다음 친구에게 걸어감 |
| visit | `guest_left` | INFO | visit owner | 손님 창이 화면을 떠남 |
| visit | `carried_off` | INFO | visit to | 손님이 내 펫을 데려감 |
| visit | `carry_on` | INFO | visit from to | 📱 내 다른 컴퓨터의 펫을 데려다줌 |
| visit | `carry_home` | INFO | visit from | 📱 그 펫이 집으로 감 |
| visit | `unknown_object` | WARN | | 손님이 든 물건 종류를 모름 |
| toss | `toss_out` | INFO | friend kind with_note | 물건을 친구에게 던짐 |
| toss | `toss_volley` | INFO | friend n | 물건 여러 개 던지기 |
| toss | `toss_in` | INFO | friend kind with_note | 친구가 던진 물건을 받음 |
| toss | `toss_unknown` | WARN | friend | 모르는 물건 종류라 버림 |

## 4. 개인정보 규칙

- 안 적음: 편지 내용, 건의함 이름·글, 펫 이름, 사람 이름, 친구 코드, 키·비밀(앞 8자 말고), 누른 키, 아이콘 픽셀, 포스트잇 글, 전체 IP, 집 폴더 아래 경로.
- 키·id는 앞 8자만 (`short`). 주소는 가림 (`mask_ip`). `err` 값은 경로 낱말을 `<path>`로 바꿈 (`scrub_err`). 데이터 폴더 경로는 시작 줄에도 안 적음.
- 관리 명령은 명령 낱말만 (`cmd=kick`), 인자(키·주소)는 안 적음.
- 포스트잇·답장은 있었는지만 (`with_note=1`).
- `tools/test_logging.py`가 모든 `log!` 호출을 읽어 키 목록, `peer`·`friend`류는 `short`, `ip`는 `mask_ip`를 거치는지, 값 식에 `name`·`code`·`key`·`secret`·`note`·`line`·`.display()` 같은 것이 없는지 보고, 사건이 이 목록과 맞는지 봄.
- 로그 파일은 git에 안 넣음: `.gitignore`의 `*.log`, `logs/`, 그리고 `tools/test_no_logs_in_git.py`.

## 5. 지켜보기

릴레이 (서버에서, 릴레이 사용자로):

```sh
journalctl --user -u screenpet-relay -f                          # 실시간
journalctl --user -u screenpet-relay -f -o cat                   # journald 머리 없이 우리 줄만
journalctl --user -u screenpet-relay --since "10 min ago"        # 최근 10분
journalctl --user -u screenpet-relay --since "10 min ago" -o cat | grep 'ERROR'
journalctl --user -u screenpet-relay --since today -o cat | grep -E ' (WARN|ERROR) '
journalctl --user -u screenpet-relay --since "1 hour ago" -o cat | grep ' weather '    # 영역으로
journalctl --user -u screenpet-relay --since today -o cat | grep 'conn=c42'           # 연결 하나의 처음부터 끝
journalctl --user -u screenpet-relay --since today -o cat | grep 'peer=f7da89ae'      # 설치 하나
journalctl --user -u screenpet-relay --since today -o cat | grep ' close ' | grep -o 'reason=[a-z_]*' | sort | uniq -c | sort -rn
```

journald도 자기 시각을 붙이지만 우리 줄의 KST 밀리초 시각이 기준임.

관리 명령:

```sh
screenpet-relay admin status      # 스위치·접속 중·누계 카운터 + 최근 5분 / 1시간 창
screenpet-relay admin recent      # 최근 WARN/ERROR 20줄 (메모리 고리 200줄)
screenpet-relay admin recent 100
```

`admin status` 끝의 창 (`5분 / 1시간`):

```
rates (last 5m / 1h):
  conns_accepted 3 / 41
  conns_closed 2 / 39
  logins 3 / 40
  bad_message 0 / 1
  rate_limited 0 / 0
  letters 0 / 1
  letters_refused 0 / 0
  board_lists 1 / 6
  weather_ok 1 / 12
  weather_met_ok 0 / 0
  weather_fail 0 / 0
  downloads_started 0 / 2
  downloads_cut 0 / 0
errors by area (last 5m / 1h):
  conn 0 / 0
  …
```

창은 1분 칸이라 "5분" = 지금 칸 포함 다섯 칸. 재시작하면 창·고리는 비고 누계도 0부터.

## 6. 증상별 볼 곳

| 증상 | 볼 곳 |
|---|---|
| 친구가 모두 오프라인 | 앱: `relay login`이 있나, `relay connect_fail`·`relay refused`·`relay closed`·`net relay_down`. 릴레이: `grep 'peer=<내 키 8자>'` → `hub login`의 `online=`(0이면 친구가 접속 안 했거나 서로 친구가 아님), `presence friends`의 n. `conn close reason=idle`이 잦으면 네트워크가 끊김. `login_replaced`가 잦으면 같은 신원을 두 컴퓨터가 씀 |
| 던진 물건이 안 감 | 보낸 앱: `toss toss_out`이 있나 (없으면 그 끝에 연결된 친구가 없음). 받는 앱: `toss toss_in`, `toss_unknown`. 릴레이: `route channel_open`/`channel_fail reason=…`, `channel_cut reason=rate|queue` |
| 건의가 안 감 | 릴레이: `mail letter_refused reason=…`(`daily`·`new`·`ip`·`global`·`full`), `conn rate_limited what=letter`, `mail save_fail`. 앱: 그때 `relay login`이 있었나 (`v`가 3 미만이면 옛 릴레이) |
| 날씨가 안 보임 | 릴레이: `weather fetch_fail`(src, err, n), `admin status`의 `weather_ok / weather_met_ok / weather_fail`(`weather_met_ok`가 늘면 Open-Meteo가 안 되고 MET이 대신함), 시작 줄의 `weather=off`(SCREENPET_RELAY_WEATHER=off). 앱: `relay login`의 `v`가 2 이상인지 |
| 업데이트 실패 | 앱: `update check_fail`·`get_fail`·`http_status`·`bad_signature`·`download_fail`·`install_fail`. 릴레이: `dl meta`(확인이 닿았나), `dl not_found`(파일 덜 올림), `dl download_cut reason=slow|deadline`, `download_busy` |
| 서버가 느림 | 릴레이: `store save_slow`·`save_fail`, `mail letter_in`의 took_ms, `weather fetch_ok` took_ms, `route channel_cut reason=queue`, `conn close reason=slow`, `hub unauthed_evicted`·`conn refused reason=busy`(연결 폭주), `fd_limit`, `accept_fail`. `admin status`의 `conns_accepted` 창으로 폭주인지 봄 |

## 7. 친구의 `network.log`를 릴레이 줄과 맞추기

1. 친구에게 config 폴더의 `network.log`를 받음 (Windows `%APPDATA%\com.screenpet.app\`, macOS `~/Library/Application Support/com.screenpet.app/`). 256 KiB를 넘으면 처음부터 다시 쓰므로 일이 난 뒤 곧 받을 것. 이름·글·코드·전체 주소는 없음.
2. `relay login` 줄의 `peer=`가 그 사람의 키 앞 8자임:
   ```
   2026-11-13T15:04:05.101+09:00 INFO  relay    login        peer=f7da89ae v=3 friends=4 online=2 took_ms=84
   ```
3. 서버에서 같은 시각 근처를 그 peer로 찾음 (시각이 둘 다 KST라 그대로 견줌):
   ```sh
   journalctl --user -u screenpet-relay --since "2026-11-13 15:00" --until "2026-11-13 15:10" -o cat | grep 'peer=f7da89ae'
   ```
4. 거기서 `conn=`을 얻고 그 연결 전체를 봄 (accept·인증 실패처럼 peer가 아직 없는 줄도 나옴):
   ```sh
   journalctl --user -u screenpet-relay --since today -o cat | grep 'conn=c42 '
   ```
5. 친구끼리 문제면 두 사람의 `friend=`(설치 id 앞 8자, 앱 로그)와 릴레이의 `to=`(키 앞 8자)는 다른 값임: 릴레이 쪽은 각자의 `relay login` peer로 맞춤.
