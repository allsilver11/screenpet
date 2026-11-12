//! 🔄 bin/screenpet-update-sign (deploy/publish-release.sh가 부름)이 만든 서명을 앱의 확인이 받아들이는지 봄
use screenpet_core::update;
use std::path::PathBuf;
use std::process::Command;

const BIN: &str = env!("CARGO_BIN_EXE_screenpet-update-sign");

/// 테스트: 비어 있는 임시 폴더
fn tmp(tag: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("screenpet-sign-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

/// 테스트: hex 64자를 32바이트로
fn hex32(s: &str) -> [u8; 32] {
    let s = s.trim();
    assert_eq!(s.len(), 64, "{s}");
    let mut out = [0u8; 32];
    for (i, b) in out.iter_mut().enumerate() {
        *b = u8::from_str_radix(&s[i * 2..i * 2 + 2], 16).unwrap();
    }
    out
}

/// 테스트: 서명 도구를 args로 실행함
fn run(args: &[&std::ffi::OsStr]) -> std::process::Output {
    Command::new(BIN).args(args).output().unwrap()
}

#[test]
fn the_helper_signs_what_the_app_verifies() {
    let d = tmp("ok");
    let key = d.join("key.txt");
    let out = run(&["keygen".as_ref(), key.as_os_str()]);
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    let public = hex32(&String::from_utf8(out.stdout).unwrap());
    let secret = std::fs::read_to_string(&key).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(std::fs::metadata(&key).unwrap().permissions().mode() & 0o777, 0o600);
    }
    let shown = run(&["public".as_ref(), key.as_os_str()]);
    assert_eq!(hex32(&String::from_utf8(shown.stdout.clone()).unwrap()), public);
    assert!(!String::from_utf8_lossy(&shown.stdout).contains(secret.trim()), "the secret is never printed");

    let json = br#"{"version":"0.6.3","files":{}}"#;
    let file = d.join("latest.json");
    std::fs::write(&file, json).unwrap();
    let signed = run(&["sign".as_ref(), key.as_os_str(), file.as_os_str()]);
    assert!(signed.status.success());
    let sig = signed.stdout;
    assert_eq!(sig.len(), 128, "no newline, exactly 128 hex digits");
    assert_eq!(update::verify_latest_with(&public, json, &sig), Ok(()));
    assert!(update::verify_latest_with(&public, br#"{"version":"0.6.4","files":{}}"#, &sig).is_err());

    // `verify` checks against the key built into the app: a test key is refused.
    let sig_file = d.join("latest.json.sig");
    std::fs::write(&sig_file, &sig).unwrap();
    let checked = run(&["verify".as_ref(), file.as_os_str(), sig_file.as_os_str()]);
    assert_eq!(checked.status.code(), Some(1));
    // keygen never overwrites a key.
    let again = run(&["keygen".as_ref(), key.as_os_str()]);
    assert!(!again.status.success());
    assert_eq!(std::fs::read_to_string(&key).unwrap(), secret);
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn bad_secret_files_and_arguments_are_refused() {
    let d = tmp("bad");
    let key = d.join("key.txt");
    std::fs::write(&key, "not a key").unwrap();
    let file = d.join("f");
    std::fs::write(&file, "x").unwrap();
    assert_eq!(run(&["sign".as_ref(), key.as_os_str(), file.as_os_str()]).status.code(), Some(1));
    assert_eq!(run(&["sign".as_ref(), d.join("missing").as_os_str(), file.as_os_str()]).status.code(), Some(1));
    assert_eq!(run(&[]).status.code(), Some(2));
    assert_eq!(run(&["frobnicate".as_ref()]).status.code(), Some(2));
    let _ = std::fs::remove_dir_all(&d);
}
