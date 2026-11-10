//! 💌 Feedback letters to the developer (spec 2026-11-10-feedback-mailbox).
//!
//! The app seals a [`Letter`] to the developer's public key with a one-message
//! `Noise_N_25519_ChaChaPoly_BLAKE2s` handshake: only the holder of the
//! developer secret can open it. The relay stores the sealed bytes and never
//! sees the text.

use serde::{Deserialize, Serialize};

/// The developer's public key (hex). Its secret lives only in the
/// developer's `config.json` (`dev_key`); make a new pair with
/// `cargo run -p screenpet-net --example dev_key`.
pub const DEV_KEY: &str = "0116e5d331acff1d42f3dbee5be0edf72187fa861f44e7bbcf9182012cfa046a";
/// Longest letter text (characters, after cleaning).
pub const MAX_LETTER_CHARS: usize = 1000;
/// Longest sender name shown in the mailbox.
pub const MAX_NAME: usize = 64;
/// Longest pet / app id fields.
const MAX_ID: usize = 32;
/// Largest sealed letter (the relay refuses bigger ones).
pub const MAX_SEALED: usize = 8192;

const PATTERN: &str = "Noise_N_25519_ChaChaPoly_BLAKE2s";
const PROLOGUE: &[u8] = b"screenpet-letter/1";

/// A letter in the developer's mailbox as the relay lists it (still sealed —
/// the app opens it with its own `dev_key`, the net layer never sees it).
#[derive(Debug, Clone, PartialEq)]
pub struct MailItem {
    pub id: u64,
    /// Unix seconds when the relay took it.
    pub at: u64,
    /// The sender's install key (hex) — for blocking.
    pub from: String,
    pub read: bool,
    pub sealed: Vec<u8>,
}

/// What the developer reads. Nothing else about the sender is sent (no
/// address, no email, no friends).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Letter {
    pub text: String,
    /// The sender's display name ("" if unset).
    pub name: String,
    /// The pet that carries the letter.
    pub species: String,
    pub stage: String,
    pub style: String,
    /// App version and OS (`windows` / `macos`).
    pub version: String,
    pub os: String,
}

fn hidden(c: char) -> bool {
    (c.is_control() && c != '\n')
        || matches!(c, '\u{200B}'..='\u{200F}' | '\u{202A}'..='\u{202E}' | '\u{2060}'..='\u{2064}' | '\u{2066}'..='\u{2069}' | '\u{FEFF}' | '\u{061C}' | '\u{180E}' | '\u{00AD}' | '\u{3164}' | '\u{115F}' | '\u{1160}' | '\u{FFA0}' | '\u{2800}' | '\u{E0000}'..='\u{E007F}' | '\u{2028}' | '\u{2029}')
}

/// Letter text: CRLF → LF, invisible and control characters dropped (newlines
/// kept), at most one blank line in a row, trimmed, `MAX_LETTER_CHARS` long.
fn clean_body(raw: &str) -> String {
    let raw = raw.replace("\r\n", "\n").replace('\r', "\n");
    let mut out = String::new();
    let mut newlines = 0;
    for c in raw.chars().filter(|&c| !hidden(c)) {
        if c == '\n' {
            newlines += 1;
            if newlines > 2 {
                continue;
            }
        } else {
            newlines = 0;
        }
        out.push(c);
    }
    let t: String = out.trim().chars().take(MAX_LETTER_CHARS).collect();
    t.trim_end().to_string()
}

fn clean_line(raw: &str, max: usize) -> String {
    let t: String = raw.chars().filter(|&c| !hidden(c) && c != '\n').collect();
    t.trim().chars().take(max).collect::<String>().trim_end().to_string()
}

fn clean_id(raw: &str) -> String {
    raw.chars().filter(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | '.')).take(MAX_ID).collect::<String>().to_ascii_lowercase()
}

/// Every field tidied; `None` if there is no text left.
pub fn clean(l: Letter) -> Option<Letter> {
    let text = clean_body(&l.text);
    if text.is_empty() {
        return None;
    }
    Some(Letter {
        text,
        name: clean_line(&l.name, MAX_NAME),
        species: clean_id(&l.species),
        stage: clean_id(&l.stage),
        style: clean_id(&l.style),
        version: clean_id(&l.version),
        os: clean_id(&l.os),
    })
}

/// Seal `letter` to the public key `to`. `None` if there's nothing to say or
/// it would be too big.
pub fn seal(letter: &Letter, to: &[u8; 32]) -> Option<Vec<u8>> {
    let letter = clean(letter.clone())?;
    let payload = serde_json::to_vec(&letter).ok()?;
    let mut hs = snow::Builder::new(PATTERN.parse().ok()?).prologue(PROLOGUE).ok()?.remote_public_key(to).ok()?.build_initiator().ok()?;
    let mut buf = vec![0u8; payload.len() + 128];
    let n = hs.write_message(&payload, &mut buf).ok()?;
    buf.truncate(n);
    (buf.len() <= MAX_SEALED).then_some(buf)
}

/// Open a sealed letter with the developer secret (`None` if it isn't ours,
/// was changed, or doesn't read as a letter). The result is cleaned again.
pub fn open(sealed: &[u8], secret: &[u8; 32]) -> Option<Letter> {
    if sealed.is_empty() || sealed.len() > MAX_SEALED {
        return None;
    }
    let mut hs = snow::Builder::new(PATTERN.parse().ok()?).prologue(PROLOGUE).ok()?.local_private_key(secret).ok()?.build_responder().ok()?;
    let mut buf = vec![0u8; sealed.len()];
    let n = hs.read_message(sealed, &mut buf).ok()?;
    clean(serde_json::from_slice(&buf[..n]).ok()?)
}

/// The shipped developer public key.
pub fn dev_key() -> Option<[u8; 32]> {
    crate::noise::hex32(DEV_KEY)
}

/// A developer secret typed into `config.json` (hex, spaces around ignored).
pub fn secret_from_hex(s: &str) -> Option<[u8; 32]> {
    crate::noise::hex32(s.trim())
}

/// A fresh (secret, public) pair, both hex.
pub fn new_dev_keypair() -> (String, String) {
    let secret = crate::noise::random32();
    (crate::noise::to_hex(&secret), crate::noise::to_hex(&crate::noise::public_key(&secret)))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn keypair() -> ([u8; 32], [u8; 32]) {
        let secret = crate::noise::random32();
        (secret, crate::noise::public_key(&secret))
    }

    #[test]
    fn a_sealed_letter_opens_only_with_the_developer_secret() {
        let (secret, public) = keypair();
        let l = Letter { text: "펭귄 귀여워요\n고마워요".into(), name: "단니".into(), species: "penguin".into(), ..Default::default() };
        let sealed = seal(&l, &public).unwrap();
        let opened = open(&sealed, &secret).unwrap();
        assert_eq!((opened.text.as_str(), opened.name.as_str(), opened.species.as_str()), ("펭귄 귀여워요\n고마워요", "단니", "penguin"));
        let (other, _) = keypair();
        assert!(open(&sealed, &other).is_none());
        assert!(open(b"junk", &secret).is_none());
        let mut tampered = sealed.clone();
        *tampered.last_mut().unwrap() ^= 1;
        assert!(open(&tampered, &secret).is_none());
    }

    #[test]
    fn letters_are_cleaned_and_limited() {
        let long = Letter { text: "가".repeat(1500), ..Default::default() };
        assert_eq!(clean(long).unwrap().text.chars().count(), MAX_LETTER_CHARS);
        assert!(clean(Letter { text: " \n \u{200B} ".into(), ..Default::default() }).is_none());
        let messy = clean(Letter { text: "a\r\nb\u{0007}\n\n\n\n\nc".into(), species: "<b>cat</b>".into(), ..Default::default() }).unwrap();
        assert_eq!(messy.text, "a\nb\n\nc", "CRLF, control chars and runs of blank lines");
        assert_eq!(messy.species, "bcatb", "ids keep only [a-z0-9_.-]");
        let (secret, public) = keypair();
        let sealed = seal(&Letter { text: "가".repeat(1000), name: "x".repeat(200), ..Default::default() }, &public).unwrap();
        assert!(sealed.len() <= MAX_SEALED, "{}", sealed.len());
        assert_eq!(open(&sealed, &secret).unwrap().name.chars().count(), MAX_NAME);
        assert!(seal(&Letter::default(), &public).is_none(), "nothing to say");
    }

    #[test]
    fn the_shipped_developer_key_is_a_valid_public_key() {
        assert!(dev_key().is_some());
        let (s, p) = new_dev_keypair();
        assert_eq!(crate::noise::to_hex(&crate::noise::public_key(&crate::noise::hex32(&s).unwrap())), p);
        assert_eq!(secret_from_hex(&format!("  {s}\n")), crate::noise::hex32(&s));
        assert_eq!(secret_from_hex("nope"), None);
    }
}
