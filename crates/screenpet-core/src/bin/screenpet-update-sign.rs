//! 🔄 Update signing for `deploy/publish-release.sh` (runs on the server, never inside the relay).
//!
//! ```text
//! screenpet-update-sign keygen <secret-file>     new key: secret (hex seed) written 0600, public key printed
//! screenpet-update-sign public <secret-file>     print the public key of a secret
//! screenpet-update-sign sign <secret-file> <file>  print the signature (128 hex) of the file's exact bytes
//! screenpet-update-sign verify <file> <sig-file>   check against the key built into the app (exit 1 if not)
//! ```
//! The secret is never printed.
use screenpet_core::update;
use std::process::exit;

fn usage() -> ! {
    eprintln!("usage: screenpet-update-sign keygen <secret-file> | public <secret-file> | sign <secret-file> <file> | verify <file> <sig-file>");
    exit(2);
}

fn fail(msg: &str) -> ! {
    eprintln!("screenpet-update-sign: {msg}");
    exit(1);
}

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
