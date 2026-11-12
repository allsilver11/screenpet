//! 와이어 형식 고정 시험: 모든 메시지를 글자 그대로의 JSON과 맞춰 봄.
//!
//! 여기 문자열이 바뀌면 배포된 릴레이·옛 앱(v0.6.0–v0.6.4)과 말이 안 통할 수 있음.
//! 고칠 일이 있으면 프로토콜 버전과 옛 버전 호환부터 따질 것.

use screenpet_proto::relay::{BoardIn, BoardItem, BoardStatus, ClientMsg, Letter, ServerMsg};

/// 앱이 보내는 글자가 정확히 `json`이고, 릴레이가 그것을 `m`으로 읽는지 봄
fn client(m: ClientMsg, json: &str) {
    assert_eq!(m.to_json(), json);
    assert_eq!(serde_json::from_str::<ClientMsg>(json).unwrap(), m, "{json}");
}

/// 릴레이가 보내는 글자가 정확히 `json`이고, 앱이 그것을 `m`으로 읽는지 봄
fn server(m: ServerMsg, json: &str) {
    assert_eq!(m.to_json(), json);
    assert_eq!(serde_json::from_str::<ServerMsg>(json).unwrap(), m, "{json}");
}

/// 릴레이가 `json`(옛 앱이 보낼 수 있는 모양)을 `m`으로 읽는지 봄
fn client_in(json: &str, m: ClientMsg) {
    assert_eq!(serde_json::from_str::<ClientMsg>(json).unwrap(), m, "{json}");
}

/// 앱이 `json`(옛 릴레이가 보낼 수 있는 모양)을 `m`으로 읽는지 봄
fn server_in(json: &str, m: ServerMsg) {
    assert_eq!(serde_json::from_str::<ServerMsg>(json).unwrap(), m, "{json}");
}

/// 시험용 편지 하나를 만듦
fn letter(done: bool, rejected: bool, private: bool) -> Letter {
    Letter { id: 7, at: 1_800_000_000, from: "ab".into(), read: true, done, rejected, private, r#box: "00ff".into() }
}

#[test]
fn every_client_message() {
    client(
        ClientMsg::Auth { id: "i".into(), key: "k".into(), mac: "m".into(), friends: vec!["f1".into(), "f2".into()], cnonce: Some("c".into()) },
        r#"{"t":"auth","id":"i","key":"k","mac":"m","friends":["f1","f2"],"cnonce":"c"}"#,
    );
    client(ClientMsg::Friends { keys: vec!["a".into()] }, r#"{"t":"friends","keys":["a"]}"#);
    client(ClientMsg::Redeem { code: "ABCD-1234".into() }, r#"{"t":"redeem","code":"ABCD-1234"}"#);
    client(ClientMsg::Open { req: 3, to: "ab".into() }, r#"{"t":"open","req":3,"to":"ab"}"#);
    client(ClientMsg::Close { ch: 9 }, r#"{"t":"close","ch":9}"#);
    client(ClientMsg::NewCode, r#"{"t":"new_code"}"#);
    client(ClientMsg::Ping, r#"{"t":"ping"}"#);
    client(ClientMsg::Letter { r#box: "00ff".into() }, r#"{"t":"letter","box":"00ff"}"#);
    client(ClientMsg::Letters, r#"{"t":"letters"}"#);
    client(ClientMsg::LetterRead { id: 4 }, r#"{"t":"letter_read","id":4}"#);
    client(ClientMsg::LetterDelete { id: 4 }, r#"{"t":"letter_delete","id":4}"#);
    client(ClientMsg::LetterDone { id: 4, done: true }, r#"{"t":"letter_done","id":4,"done":true}"#);
    client(ClientMsg::LetterBlock { from: "ab".into() }, r#"{"t":"letter_block","from":"ab"}"#);
    client(ClientMsg::WeatherWant { place: "busan".into() }, r#"{"t":"weather_want","place":"busan"}"#);
    client(ClientMsg::LetterReject { id: 4, rejected: true }, r#"{"t":"letter_reject","id":4,"rejected":true}"#);
    client(ClientMsg::LetterPrivate { id: 4, private: true }, r#"{"t":"letter_private","id":4,"private":true}"#);
    client(
        ClientMsg::BoardPut { items: vec![BoardIn { id: 3, name: "김**".into(), line: "펭귄".into() }] },
        r#"{"t":"board_put","items":[{"id":3,"name":"김**","line":"펭귄"}]}"#,
    );
    client(ClientMsg::BoardList { page: 2 }, r#"{"t":"board_list","page":2}"#);
}

#[test]
fn old_and_odd_client_messages_still_parse() {
    // cnonce 전의 앱, 친구 목록을 빼먹은 앱
    client_in(
        r#"{"t":"auth","id":"i","key":"k","mac":"m","friends":[]}"#,
        ClientMsg::Auth { id: "i".into(), key: "k".into(), mac: "m".into(), friends: vec![], cnonce: None },
    );
    client_in(r#"{"t":"auth","id":"i","key":"k","mac":"m"}"#, ClientMsg::Auth { id: "i".into(), key: "k".into(), mac: "m".into(), friends: vec![], cnonce: None });
    // cnonce가 없으면 보내지 않음
    assert_eq!(
        ClientMsg::Auth { id: "i".into(), key: "k".into(), mac: "m".into(), friends: vec![], cnonce: None }.to_json(),
        r#"{"t":"auth","id":"i","key":"k","mac":"m","friends":[]}"#
    );
    client_in(r#"{"t":"board_list"}"#, ClientMsg::BoardList { page: 0 });
    // 모르는 필드는 무시하고, 필드 순서는 상관없음
    client_in(r#"{"to":"ab","extra":1,"t":"open","req":3}"#, ClientMsg::Open { req: 3, to: "ab".into() });
}

#[test]
fn unknown_or_malformed_client_messages_fail() {
    // 릴레이는 이것들을 bad_message로 끊음
    for bad in [r#"{"t":"bogus"}"#, r#"{"t":"open","req":-1,"to":"ab"}"#, r#"{"t":"open","req":1}"#, r#"{"req":1,"to":"ab"}"#, "not json"] {
        assert!(serde_json::from_str::<ClientMsg>(bad).is_err(), "{bad}");
    }
}

#[test]
fn every_server_message() {
    server(ServerMsg::Challenge { relay: "ab".into(), nonce: "cd".into(), v: 3 }, r#"{"t":"challenge","relay":"ab","nonce":"cd","v":3}"#);
    server(
        ServerMsg::Welcome { code: "ABCD-1234".into(), code_ttl: 86400, online: vec!["k".into()], proof: Some("pp".into()) },
        r#"{"t":"welcome","code":"ABCD-1234","code_ttl":86400,"online":["k"],"proof":"pp"}"#,
    );
    server(ServerMsg::Welcome { code: "----".into(), code_ttl: 0, online: vec![], proof: None }, r#"{"t":"welcome","code":"----","code_ttl":0,"online":[]}"#);
    server(ServerMsg::Online { keys: vec!["a".into(), "b".into()] }, r#"{"t":"online","keys":["a","b"]}"#);
    server(ServerMsg::Presence { key: "k".into(), online: true }, r#"{"t":"presence","key":"k","online":true}"#);
    server(ServerMsg::Code { code: "ABCD-1234".into(), code_ttl: 5 }, r#"{"t":"code","code":"ABCD-1234","code_ttl":5}"#);
    server(ServerMsg::Redeemed { code: "ABCD-1234".into(), key: "k".into(), id: "i".into() }, r#"{"t":"redeemed","code":"ABCD-1234","key":"k","id":"i"}"#);
    server(ServerMsg::RedeemFailed { code: "ABCD-1234".into(), reason: "unknown".into() }, r#"{"t":"redeem_failed","code":"ABCD-1234","reason":"unknown"}"#);
    server(ServerMsg::Opened { req: 3, ch: 9 }, r#"{"t":"opened","req":3,"ch":9}"#);
    server(ServerMsg::OpenFailed { req: 3, reason: "offline".into() }, r#"{"t":"open_failed","req":3,"reason":"offline"}"#);
    server(ServerMsg::Incoming { ch: 9, from: "k".into(), id: "i".into() }, r#"{"t":"incoming","ch":9,"from":"k","id":"i"}"#);
    server(ServerMsg::Close { ch: 9 }, r#"{"t":"close","ch":9}"#);
    server(ServerMsg::Pong, r#"{"t":"pong"}"#);
    server(ServerMsg::Error { reason: "rate".into() }, r#"{"t":"error","reason":"rate"}"#);
    server(ServerMsg::LetterOk, r#"{"t":"letter_ok"}"#);
    server(ServerMsg::LetterRefused { reason: "daily".into() }, r#"{"t":"letter_refused","reason":"daily"}"#);
    server(
        ServerMsg::Letters { items: vec![letter(false, true, true)], board: vec![3] },
        r#"{"t":"letters","items":[{"id":7,"at":1800000000,"from":"ab","read":true,"done":false,"rejected":true,"private":true,"box":"00ff"}],"board":[3]}"#,
    );
    server(ServerMsg::Letters { items: vec![], board: vec![3] }, r#"{"t":"letters","items":[],"board":[3]}"#);
    server(
        ServerMsg::LetterNew { item: letter(true, false, false) },
        r#"{"t":"letter_new","item":{"id":7,"at":1800000000,"from":"ab","read":true,"done":true,"rejected":false,"private":false,"box":"00ff"}}"#,
    );
    server(ServerMsg::LettersChanged, r#"{"t":"letters_changed"}"#);
    server(ServerMsg::Weather { place: "yongin".into(), code: 75, at: 1_800_000_000 }, r#"{"t":"weather","place":"yongin","code":75,"at":1800000000}"#);
    server(
        ServerMsg::BoardPage {
            items: vec![BoardItem { id: 3, name: "김**".into(), line: "펭귄".into(), at: 9, status: BoardStatus::Rejected }],
            page: 1,
            pages: 1,
            total: 1,
        },
        r#"{"t":"board_page","items":[{"id":3,"name":"김**","line":"펭귄","at":9,"status":"rejected"}],"page":1,"pages":1,"total":1}"#,
    );
    for (s, json) in [(BoardStatus::None, r#""none""#), (BoardStatus::Done, r#""done""#), (BoardStatus::Rejected, r#""rejected""#)] {
        assert_eq!(serde_json::to_string(&s).unwrap(), json);
    }
}

#[test]
fn old_relays_messages_still_parse() {
    // v1 전 릴레이는 버전을 말하지 않음
    server_in(r#"{"t":"challenge","relay":"ab","nonce":"cd"}"#, ServerMsg::Challenge { relay: "ab".into(), nonce: "cd".into(), v: 0 });
    server_in(r#"{"t":"challenge","relay":"ab","nonce":"cd","v":2}"#, ServerMsg::Challenge { relay: "ab".into(), nonce: "cd".into(), v: 2 });
    // 앱이 안 쓰는 필드는 없어도 됨
    server_in(r#"{"t":"welcome","code":"ABCD-1234"}"#, ServerMsg::Welcome { code: "ABCD-1234".into(), code_ttl: 0, online: vec![], proof: None });
    server_in(r#"{"t":"code","code":"ABCD-1234"}"#, ServerMsg::Code { code: "ABCD-1234".into(), code_ttl: 0 });
    server_in(r#"{"t":"open_failed","req":3}"#, ServerMsg::OpenFailed { req: 3, reason: String::new() });
    server_in(r#"{"t":"incoming","ch":9,"from":"k"}"#, ServerMsg::Incoming { ch: 9, from: "k".into(), id: String::new() });
    // v2 릴레이의 편지 목록에는 board가 없음, 옛 편지에는 done/rejected/private가 없음
    server_in(r#"{"t":"letters","items":[]}"#, ServerMsg::Letters { items: vec![], board: vec![] });
    server_in(
        r#"{"t":"letter_new","item":{"id":7,"at":1800000000,"from":"ab","box":"00ff"}}"#,
        ServerMsg::LetterNew { item: Letter { id: 7, at: 1_800_000_000, from: "ab".into(), read: false, done: false, rejected: false, private: false, r#box: "00ff".into() } },
    );
    server_in(
        r#"{"t":"letter_new","item":{"id":7,"at":1800000000,"from":"ab","read":true,"done":true,"box":"00ff"}}"#,
        ServerMsg::LetterNew { item: letter(true, false, false) },
    );
    // 모르는 상태나 빠진 상태는 없음으로 읽음
    let page = r#"{"t":"board_page","items":[{"id":1,"name":"*","line":"a","at":2,"status":"done"},{"id":2,"name":"*","line":"b","at":1,"status":"someday"},{"id":3,"name":"*","line":"c","at":0}],"page":1,"pages":1,"total":3}"#;
    let ServerMsg::BoardPage { items, .. } = serde_json::from_str::<ServerMsg>(page).unwrap() else { panic!() };
    assert_eq!(items.iter().map(|i| i.status).collect::<Vec<_>>(), vec![BoardStatus::Done, BoardStatus::None, BoardStatus::None]);
    // 앱의 날씨 시각은 부호 있는 정수: 이상한 값이어도 파싱은 됨 (오래된 값으로 버려짐)
    server_in(r#"{"t":"weather","place":"yongin","code":3,"at":-5}"#, ServerMsg::Weather { place: "yongin".into(), code: 3, at: -5 });
}

#[test]
fn unknown_or_malformed_server_messages_fail() {
    // 앱은 이것들을 무시함 (더 새 릴레이의 메시지)
    for bad in [r#"{"t":"aurora"}"#, r#"{"t":"weather","place":"yongin","code":-1,"at":1}"#, r#"{"t":"presence","key":"k"}"#, "[]"] {
        assert!(serde_json::from_str::<ServerMsg>(bad).is_err(), "{bad}");
    }
}

#[test]
fn versions_and_shared_limits() {
    assert_eq!(screenpet_proto::RELAY_PROTOCOL_VERSION, 3);
    assert_eq!((screenpet_proto::WEATHER_MIN_VERSION, screenpet_proto::BOARD_MIN_VERSION), (2, 3));
    assert_eq!((screenpet_proto::MAX_FRAME, screenpet_proto::MAX_ID, screenpet_proto::MAX_FRIENDS, screenpet_proto::MAX_SEALED), (65535, 128, 50, 8192));
    use screenpet_proto::board::*;
    assert_eq!((PER_PAGE, PUT_MAX, MAX_NAME, MAX_LINE, MAX_FIRST), (5, 20, 16, 80, 8));
}
