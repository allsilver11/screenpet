//! 🔄 업데이트 서명 도구 — `deploy/publish-release.sh`가 서버에서 부름 (중계 서버 안에서는 돌지 않음)
//!
//! ```text
//! screenpet-update-sign keygen <secret-file>     새 키: 비밀(hex 씨앗)을 0600으로 쓰고 공개키를 찍음
//! screenpet-update-sign public <secret-file>     비밀의 공개키를 찍음
//! screenpet-update-sign sign <secret-file> <file>  파일 바이트 그대로의 서명(hex 128자)을 찍음
//! screenpet-update-sign verify <file> <sig-file>   앱에 든 키로 확인함 (아니면 exit 1)
//! ```
//! 비밀은 절대 찍지 않음
use screenpet_core::update;
use std::process::exit;

/// 쓰는 법을 찍고 끝냄 (exit 2)
fn usage() -> ! {
    eprintln!("usage: screenpet-update-sign keygen <secret-file> | public <secret-file> | sign <secret-file> <file> | verify <file> <sig-file>");
    exit(2);
}

/// 오류를 찍고 끝냄 (exit 1)
fn fail(msg: &str) -> ! {
    eprintln!("screenpet-update-sign: {msg}");
    exit(1);
}

/// 비밀 키 파일(hex 64자)을 읽음
fn read_secret(path: &str) -> [u8; 32] {
    let text = std::fs::read_to_string(path).unwrap_or_else(|e| fail(&format!("cannot read the secret key {path}: {e}")));
    let hex = text.trim();
    if hex.len() != 64 || !hex.bytes().all(|b| b.is_ascii_hexdigit()) {
        fail(&format!("{path} is not a secret key (64 hex digits)"));
    }
    let mut seed = [0u8; 32];
    for (i, b) in seed.iter_mut().enumerate() {
        *b = u8::from_str_radix(&hex[i * 2..i * 2 + 2], 16).unwrap();
    }
    seed
}

/// 새 비밀을 /dev/urandom에서 만들어 0600 파일로 쓰고 공개키를 찍음 (파일이 이미 있으면 실패)
fn keygen(path: &str) {
    use std::io::{Read, Write};
    let mut seed = [0u8; 32];
    std::fs::File::open("/dev/urandom")
        .and_then(|mut f| f.read_exact(&mut seed))
        .unwrap_or_else(|e| fail(&format!("no randomness: {e}")));
    let mut opts = std::fs::OpenOptions::new();
    opts.write(true).create_new(true);
    #[cfg(unix)]
    std::os::unix::fs::OpenOptionsExt::mode(&mut opts, 0o600);
    let mut f = opts.open(path).unwrap_or_else(|e| fail(&format!("cannot create {path} (it must not exist yet): {e}")));
    writeln!(f, "{}", update::to_hex(&seed)).and_then(|_| f.sync_all()).unwrap_or_else(|e| fail(&format!("cannot write {path}: {e}")));
    println!("{}", update::to_hex(&update::public_key_of(&seed)));
}

/// 하위 명령을 골라 실행함
fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let args: Vec<&str> = args.iter().map(String::as_str).collect();
    match args.as_slice() {
        ["keygen", path] => keygen(path),
        ["public", path] => println!("{}", update::to_hex(&update::public_key_of(&read_secret(path)))),
        ["sign", path, file] => {
            let seed = read_secret(path);
            let data = std::fs::read(file).unwrap_or_else(|e| fail(&format!("cannot read {file}: {e}")));
            print!("{}", update::sign_latest(&seed, &data));
        }
        ["verify", file, sig] => {
            let data = std::fs::read(file).unwrap_or_else(|e| fail(&format!("cannot read {file}: {e}")));
            let sig = std::fs::read(sig).unwrap_or_else(|e| fail(&format!("cannot read {sig}: {e}")));
            if update::verify_latest(&data, &sig).is_err() {
                fail("the signature does not match the key built into the app (wrong secret key file?)");
            }
        }
        _ => usage(),
    }
}
