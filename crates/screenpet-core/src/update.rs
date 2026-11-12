//! 🔄 업데이트: 우리 서버(pet.danni.day/downloads)의 latest.json을 읽어 이 컴퓨터용 설치 파일을 고르고 확인하는 순수한 부분
//! (내려받기·설치는 앱의 update.rs — 여기는 네트워크·파일 없이 시험한다)
use std::fmt;
use std::path::{Path, PathBuf};
use std::time::Duration;

/// 업데이트 파일이 있는 곳 (릴레이 서버가 내준다 — deploy/publish-release.sh가 올린다).
/// 사용자가 바꿀 수 있는 릴레이 주소와는 상관없이 늘 여기다
pub const DOWNLOAD_BASE: &str = "https://pet.danni.day/downloads/";
/// 최신 버전 정보 ({"version":"X.Y.Z","files":{"windows":{name,sha256,size},"macos":{…}}})
pub const LATEST_URL: &str = "https://pet.danni.day/downloads/latest.json";
/// latest.json의 Ed25519 서명 (16진수 128글자). deploy/publish-release.sh가 서버 밖의 비밀 키로 만든다
pub const LATEST_SIG_URL: &str = "https://pet.danni.day/downloads/latest.json.sig";
/// 업데이트 서명 공개 키. 비밀 키는 릴레이 서버(프로세스)에 두지 않는다 — 서버를 가진 사람도 이 키 없이는 설치 파일을 바꿔 끼울 수 없다
/// (87b6329f…ce8c — 비밀 키는 개발자 서버의 ~/personal/screenpet-update-key.txt, deploy/relay/README.md)
pub const UPDATE_PUBLIC_KEY: [u8; 32] = [
    0x87, 0xb6, 0x32, 0x9f, 0x09, 0xa4, 0x39, 0x60, 0xa3, 0xc7, 0xe9, 0xb7, 0xe6, 0xde, 0x30, 0x81, 0xfc, 0xe7, 0x46, 0xbc, 0x1a, 0xa9, 0x54, 0x37, 0x38, 0x32, 0x14, 0x89, 0x13, 0x40, 0xce, 0x8c,
];
/// 받을 수 있는 곳은 이 호스트 하나 (https만). 넘겨주기도 이 호스트 안에서만 따라간다
pub const ALLOWED_HOST: &str = "pet.danni.day";
/// 설치 파일 크기 상한 (지금 dmg가 100MB쯤)
pub const MAX_DOWNLOAD: u64 = 300 * 1024 * 1024;
/// 넘겨주기(3xx) 횟수 상한
pub const MAX_REDIRECTS: usize = 5;
/// 앱이 켜진 뒤 첫 조용한 확인, 그 뒤 간격 (새 버전이 있으면 메뉴 글자만 바꾼다 — 누르기 전엔 설치하지 않는다)
pub const FIRST_CHECK_AFTER: Duration = Duration::from_secs(60);
pub const CHECK_EVERY: Duration = Duration::from_secs(6 * 60 * 60);
/// Windows NSIS 설치 파일 (Tauri v2 installer.nsi): /P 진행 막대만 (묻지 않고, 켜져 있는 앱은 닫는다),
/// /R 다 깔면 앱을 다시 켠다 (/P·/S에서만 본다), /UPDATE 지우고 다시 깔지 않고 덮어쓴다 (바로가기·WebView2 설치 건너뜀)
pub const NSIS_ARGS: [&str; 3] = ["/P", "/R", "/UPDATE"];

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Version {
    pub major: u32,
    pub minor: u32,
    pub patch: u32,
}

impl Version {
    /// "v0.6.3" 또는 "0.6.3"만 (꼬리표 붙은 시험판은 None)
    pub fn parse(s: &str) -> Option<Version> {
        let s = s.strip_prefix('v').unwrap_or(s);
        let mut parts = s.split('.');
        let mut next = || -> Option<u32> {
            let p = parts.next()?;
            if p.is_empty() || p.len() > 9 || !p.bytes().all(|b| b.is_ascii_digit()) {
                return None;
            }
            p.parse().ok()
        };
        let v = Version { major: next()?, minor: next()?, patch: next()? };
        parts.next().is_none().then_some(v)
    }
}

impl fmt::Display for Version {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{}.{}", self.major, self.minor, self.patch)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Platform {
    Windows,
    MacOs,
}

impl Platform {
    pub fn current() -> Option<Platform> {
        if cfg!(windows) {
            Some(Platform::Windows)
        } else if cfg!(target_os = "macos") {
            Some(Platform::MacOs)
        } else {
            None
        }
    }

    /// latest.json "files" 안의 열쇠
    pub fn key(self) -> &'static str {
        match self {
            Platform::Windows => "windows",
            Platform::MacOs => "macos",
        }
    }

    /// .github/workflows/build.yml이 올리는 이름 그대로 (서버에도 이 이름으로 둔다)
    pub fn asset_name(self, v: Version) -> String {
        match self {
            Platform::Windows => format!("ScreenPet_{v}_x64-setup.exe"),
            Platform::MacOs => format!("ScreenPet_{v}_universal.dmg"),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Asset {
    pub name: String,
    pub url: String,
    pub size: u64,
    pub sha256: [u8; 32],
}

#[derive(Debug, Clone, PartialEq)]
pub struct Release {
    pub version: Version,
    pub asset: Asset,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Check {
    UpToDate(Version),
    Newer(Release),
}

#[derive(Debug, Clone, PartialEq)]
pub enum UpdateError {
    /// 이 운영체제는 업데이트를 받지 않는다
    Unsupported,
    /// 인터넷 (자세한 건 로그에만)
    Network,
    BadResponse,
    NoAsset(Version),
    /// latest.json의 서명이 없거나 맞지 않다 — 서버가 바뀌었을 수 있으니 아무것도 믿지 않는다
    BadSignature,
    /// latest.json에 올바른 SHA-256이 없다 — 확인할 수 없는 파일은 깔지 않는다
    NoDigest,
    TooBig,
    BadUrl,
    SizeMismatch,
    DigestMismatch,
    Disk,
    Install(&'static str),
}

impl fmt::Display for UpdateError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            UpdateError::Unsupported => write!(f, "이 컴퓨터에서는 쓸 수 없어요"),
            UpdateError::Network => write!(f, "인터넷 연결을 확인해 주세요"),
            UpdateError::BadResponse => write!(f, "업데이트 정보를 읽지 못했어요"),
            UpdateError::NoAsset(v) => write!(f, "v{v}에 이 컴퓨터용 파일이 없어요"),
            UpdateError::NoDigest => write!(f, "확인용 SHA-256이 없어서 설치하지 않았어요"),
            UpdateError::BadSignature => write!(f, "업데이트 정보의 서명이 맞지 않아 설치하지 않았어요"),
            UpdateError::TooBig => write!(f, "파일이 너무 커요"),
            UpdateError::BadUrl => write!(f, "ScreenPet 서버가 아닌 주소라 받지 않았어요"),
            UpdateError::SizeMismatch => write!(f, "파일을 끝까지 받지 못했어요"),
            UpdateError::DigestMismatch => write!(f, "받은 파일이 손상됐어요 (SHA-256이 달라요)"),
            UpdateError::Disk => write!(f, "파일을 저장하지 못했어요"),
            UpdateError::Install(why) => write!(f, "{why}"),
        }
    }
}

/// latest.json의 "sha256": 16진수 64글자 (대소문자 상관없이)
pub fn parse_sha256(hex: &str) -> Option<[u8; 32]> {
    if hex.len() != 64 || !hex.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    let mut out = [0u8; 32];
    for (i, byte) in out.iter_mut().enumerate() {
        *byte = u8::from_str_radix(&hex[i * 2..i * 2 + 2], 16).ok()?;
    }
    Some(out)
}

/// latest.json.sig: 16진수 128글자 그대로 (빈칸·줄바꿈·접두어 없이)
pub fn parse_signature(text: &[u8]) -> Option<[u8; 64]> {
    if text.len() != 128 || !text.iter().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    let text = std::str::from_utf8(text).ok()?;
    let mut out = [0u8; 64];
    for (i, byte) in out.iter_mut().enumerate() {
        *byte = u8::from_str_radix(&text[i * 2..i * 2 + 2], 16).ok()?;
    }
    Some(out)
}

/// latest.json 바이트 그대로에 대한 서명을 `key`로 확인한다 (strict: 약한 키·변형 서명도 거절)
pub fn verify_latest_with(key: &[u8; 32], json: &[u8], sig: &[u8]) -> Result<(), UpdateError> {
    use ed25519_dalek::{Signature, VerifyingKey};
    let sig = parse_signature(sig).ok_or(UpdateError::BadSignature)?;
    let key = VerifyingKey::from_bytes(key).map_err(|_| UpdateError::BadSignature)?;
    key.verify_strict(json, &Signature::from_bytes(&sig)).map_err(|_| UpdateError::BadSignature)
}

/// 앱이 쓰는 것: UPDATE_PUBLIC_KEY로 확인. 맞아야만 latest.json을 읽는다 (check는 그다음)
pub fn verify_latest(json: &[u8], sig: &[u8]) -> Result<(), UpdateError> {
    verify_latest_with(&UPDATE_PUBLIC_KEY, json, sig)
}

/// 비밀 키(32바이트 시드)로 서명 — 16진수 128글자 (bin/screenpet-update-sign이 쓴다, 앱은 쓰지 않는다)
pub fn sign_latest(seed: &[u8; 32], json: &[u8]) -> String {
    use ed25519_dalek::{Signer, SigningKey};
    to_hex(&SigningKey::from_bytes(seed).sign(json).to_bytes())
}

/// 비밀 키(시드)의 공개 키
pub fn public_key_of(seed: &[u8; 32]) -> [u8; 32] {
    ed25519_dalek::SigningKey::from_bytes(seed).verifying_key().to_bytes()
}

pub fn to_hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// 설치 파일을 받을 주소 (이름은 check가 이미 확인한 것)
pub fn download_url(name: &str) -> String {
    format!("{DOWNLOAD_BASE}{name}")
}

/// https이고 호스트가 ALLOWED_HOST (사용자·포트·이상한 글자 없이)
pub fn allowed_url(url: &str) -> bool {
    let Some(rest) = url.strip_prefix("https://") else { return false };
    if url.chars().any(|c| c.is_whitespace() || c.is_control() || c == '\\') {
        return false;
    }
    let host = rest.split(['/', '?', '#']).next().unwrap_or("");
    host.eq_ignore_ascii_case(ALLOWED_HOST)
}

/// 넘겨주기(Location)를 따라갈 주소: 같은 호스트의 https 절대 주소, 또는 "/"로 시작하면 지금 주소의 호스트에 붙인다. 다른 곳은 None
pub fn redirect_target(from: &str, location: &str) -> Option<String> {
    let next = if location.starts_with("https://") {
        location.to_string()
    } else if location.starts_with('/') && !location.starts_with("//") {
        let rest = from.strip_prefix("https://")?;
        let host = rest.split(['/', '?', '#']).next()?;
        format!("https://{host}{location}")
    } else {
        return None;
    };
    allowed_url(&next).then_some(next)
}

/// latest.json을 읽어 지금 버전과 견준다. 새 버전이면 이 컴퓨터용 파일의 이름·SHA-256·크기를 꼼꼼히 확인한다:
/// version은 "X.Y.Z" (v 없이), 이름은 그 버전의 이 컴퓨터용 이름 그대로, sha256은 16진수 64글자, 크기는 1..=MAX_DOWNLOAD
pub fn check(json: &str, current: Version, platform: Platform) -> Result<Check, UpdateError> {
    let v: serde_json::Value = serde_json::from_str(json).map_err(|_| UpdateError::BadResponse)?;
    let version = v["version"].as_str().filter(|s| !s.starts_with('v')).and_then(Version::parse).ok_or(UpdateError::BadResponse)?;
    if version <= current {
        return Ok(Check::UpToDate(version));
    }
    let files = v["files"].as_object().ok_or(UpdateError::BadResponse)?;
    let f = files.get(platform.key()).filter(|f| f.is_object()).ok_or(UpdateError::NoAsset(version))?;
    let name = platform.asset_name(version);
    if f["name"].as_str() != Some(name.as_str()) {
        return Err(UpdateError::NoAsset(version));
    }
    let sha256 = f["sha256"].as_str().and_then(parse_sha256).ok_or(UpdateError::NoDigest)?;
    let size = f["size"].as_u64().filter(|&s| s > 0).ok_or(UpdateError::BadResponse)?;
    if size > MAX_DOWNLOAD {
        return Err(UpdateError::TooBig);
    }
    let url = download_url(&name);
    if !allowed_url(&url) {
        return Err(UpdateError::BadUrl);
    }
    Ok(Check::Newer(Release { version, asset: Asset { name, url, size, sha256 } }))
}

/// macOS: 실행 파일(…/X.app/Contents/MacOS/screenpet)에서 앱 번들(…/X.app). 번들 밖이면 None
pub fn app_bundle_of(exe: &Path) -> Option<PathBuf> {
    let macos = exe.parent()?;
    let contents = macos.parent()?;
    let bundle = contents.parent()?;
    let is_app = bundle.extension().is_some_and(|e| e == "app");
    (macos.file_name()? == "MacOS" && contents.file_name()? == "Contents" && is_app).then(|| bundle.to_path_buf())
}

/// 디스크 이미지 맨 위의 이름들에서 앱 하나: ScreenPet.app, 없으면 .app이 딱 하나일 때 그것
pub fn pick_app(names: &[String]) -> Option<String> {
    if names.iter().any(|n| n == "ScreenPet.app") {
        return Some("ScreenPet.app".into());
    }
    let apps: Vec<&String> = names.iter().filter(|n| n.ends_with(".app") && !n.starts_with('.')).collect();
    (apps.len() == 1).then(|| apps[0].clone())
}

/// macOS: 앱이 끝나기를 기다렸다가 번들을 새것으로 바꾸고 다시 켜는 도우미.
/// `/bin/sh -c MAC_SWAP_SCRIPT sh <pid> <마운트 지점> <새 .app> <지금 .app> <임시 폴더>` — 경로는 인자로만 받는다 (글자에 끼워 넣지 않는다).
/// 새 번들을 옆에 다 복사한 뒤 이름만 바꿔 끼운다 (실패하면 옛 번들을 되돌린다). 그다음 이미지를 떼고 임시 폴더를 지우고 연다
pub const MAC_SWAP_SCRIPT: &str = r#"pid="$1"; mnt="$2"; src="$3"; dst="$4"; tmp="$5"
n=0
while kill -0 "$pid" 2>/dev/null; do
  n=$((n+1))
  if [ "$n" -gt 300 ]; then break; fi
  sleep 0.2
done
if ! kill -0 "$pid" 2>/dev/null; then
  new="$dst.update-new"; old="$dst.update-old"
  rm -rf "$new" "$old"
  if /usr/bin/ditto "$src" "$new"; then
    /usr/bin/xattr -dr com.apple.quarantine "$new" 2>/dev/null
    if mv "$dst" "$old"; then
      if mv "$new" "$dst"; then rm -rf "$old"; else mv "$old" "$dst"; fi
    fi
  fi
  rm -rf "$new"
fi
/usr/bin/hdiutil detach "$mnt" -quiet || /usr/bin/hdiutil detach "$mnt" -force -quiet
rm -rf "$tmp"
/usr/bin/open "$dst"
"#;

/// 트레이 "🔄 업데이트" 항목의 상태
#[derive(Debug, Clone, PartialEq)]
pub enum MenuState {
    Idle,
    Checking,
    Downloading(Version),
    Installing(Version),
    /// 조용한 확인에서 찾았다 (아직 받지 않았다)
    Available(Version),
    UpToDate(Version),
    Failed(UpdateError),
}

impl MenuState {
    pub fn label(&self) -> String {
        match self {
            MenuState::Idle => "🔄 업데이트".into(),
            MenuState::Checking => "🔄 업데이트 확인 중…".into(),
            MenuState::Downloading(v) => format!("🔄 업데이트 받는 중… (v{v})"),
            MenuState::Installing(v) => format!("🔄 v{v} 설치하는 중…"),
            MenuState::Available(v) => format!("🔄 업데이트 (v{v} 나왔어요!)"),
            MenuState::UpToDate(v) => format!("🔄 업데이트 — 최신 버전이에요 (v{v})"),
            MenuState::Failed(e) => format!("🔄 업데이트를 받지 못했어요: {e}"),
        }
    }

    /// 확인·받기·설치 중에는 누를 수 없다
    pub fn enabled(&self) -> bool {
        !matches!(self, MenuState::Checking | MenuState::Downloading(_) | MenuState::Installing(_))
    }

    /// 눌러서 시작한 업데이트의 말풍선 (128px 창에 한 줄로 짧게 — 자세한 건 메뉴 글자)
    pub fn bubble(&self) -> Option<&'static str> {
        match self {
            MenuState::Downloading(_) => Some("업데이트 받는 중…"),
            MenuState::Installing(_) => Some("새 버전으로 바꿀게요!"),
            MenuState::UpToDate(_) => Some("최신 버전이에요!"),
            MenuState::Failed(_) => Some("업데이트 실패 😢"),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const HEX: &str = "07bc8daa58a56cad67885795ab01f57cd5d12b5839c2ef7abd5cfbae823c1e95";

    fn v(s: &str) -> Version {
        Version::parse(s).unwrap()
    }

    fn latest_json(version: &str, sha: &str) -> String {
        format!(
            r#"{{"version":"{version}","files":{{
            "windows":{{"name":"ScreenPet_{version}_x64-setup.exe","sha256":"{sha}","size":51426064}},
            "macos":{{"name":"ScreenPet_{version}_universal.dmg","sha256":"{sha}","size":106147349}}}}}}"#
        )
    }

    #[test]
    fn versions_parse_strictly_and_compare_numerically() {
        assert_eq!(v("v0.6.3"), Version { major: 0, minor: 6, patch: 3 });
        assert_eq!(v("0.6.3"), v("v0.6.3"));
        for bad in ["", "v", "v0.6", "v0.6.3.1", "v0.6.3-beta", "v0..3", "v0.6.x", "vv0.6.3", " 0.6.3", "v0.6.+3"] {
            assert_eq!(Version::parse(bad), None, "{bad}");
        }
        assert!(v("0.6.10") > v("0.6.9"));
        assert!(v("0.10.0") > v("0.9.99"));
        assert!(v("1.0.0") > v("0.99.99"));
        assert_eq!(v("v1.2.3").to_string(), "1.2.3");
    }

    #[test]
    fn asset_names_match_the_release_workflow() {
        assert_eq!(Platform::Windows.asset_name(v("0.6.3")), "ScreenPet_0.6.3_x64-setup.exe");
        assert_eq!(Platform::MacOs.asset_name(v("0.6.3")), "ScreenPet_0.6.3_universal.dmg");
    }

    #[test]
    fn sha256_needs_exactly_64_hex_digits() {
        let d = parse_sha256(HEX).unwrap();
        assert_eq!(d[0], 0x07);
        assert_eq!(d[31], 0x95);
        assert_eq!(parse_sha256(&HEX.to_uppercase()), Some(d));
        assert_eq!(parse_sha256(&format!("sha256:{HEX}")), None);
        assert_eq!(parse_sha256(&HEX[..62]), None);
        assert_eq!(parse_sha256(&format!("{}zz", &HEX[..62])), None);
        assert_eq!(parse_sha256(&format!("{HEX}00")), None);
        assert_eq!(parse_sha256("+7bc8daa58a56cad67885795ab01f57cd5d12b5839c2ef7abd5cfbae823c1e9"), None);
        assert_eq!(parse_sha256(" 7bc8daa58a56cad67885795ab01f57cd5d12b5839c2ef7abd5cfbae823c1e9"), None);
        assert_eq!(parse_sha256(""), None);
    }

    fn test_seed(n: u8) -> [u8; 32] {
        [n; 32]
    }

    #[test]
    fn a_valid_signature_over_the_exact_bytes_passes() {
        let seed = test_seed(7);
        let key = public_key_of(&seed);
        let json = latest_json("0.6.3", HEX);
        let sig = sign_latest(&seed, json.as_bytes());
        assert_eq!(sig.len(), 128);
        assert_eq!(verify_latest_with(&key, json.as_bytes(), sig.as_bytes()), Ok(()));
        assert_eq!(verify_latest_with(&key, json.as_bytes(), sig.to_uppercase().as_bytes()), Ok(()));
    }

    #[test]
    fn tampered_json_wrong_key_and_bad_signatures_fail() {
        let seed = test_seed(7);
        let key = public_key_of(&seed);
        let json = latest_json("0.6.3", HEX);
        let sig = sign_latest(&seed, json.as_bytes());
        let bad = Err(UpdateError::BadSignature);
        // 한 글자라도 바뀐 json (빈칸 하나 포함)
        assert_eq!(verify_latest_with(&key, json.replace("51426064", "51426065").as_bytes(), sig.as_bytes()), bad);
        assert_eq!(verify_latest_with(&key, format!("{json} ").as_bytes(), sig.as_bytes()), bad);
        // 다른 키
        assert_eq!(verify_latest_with(&public_key_of(&test_seed(8)), json.as_bytes(), sig.as_bytes()), bad);
        assert_eq!(verify_latest(json.as_bytes(), sig.as_bytes()), bad, "a test key is not the app's key");
        // 없거나 망가진 서명
        let mut flipped = sig.clone().into_bytes();
        flipped[10] = if flipped[10] == b'0' { b'1' } else { b'0' };
        for s in [&b""[..], b"garbage", &flipped, format!("{sig}\n").as_bytes(), format!(" {sig}").as_bytes(), &sig.as_bytes()[..126], "zz".repeat(64).as_bytes(), "00".repeat(64).as_bytes()] {
            assert_eq!(verify_latest_with(&key, json.as_bytes(), s), bad, "{:?}", String::from_utf8_lossy(s));
        }
        assert!(UpdateError::BadSignature.to_string().contains("서명"));
    }

    #[test]
    fn the_app_key_is_a_real_ed25519_key() {
        assert!(ed25519_dalek::VerifyingKey::from_bytes(&UPDATE_PUBLIC_KEY).is_ok());
        assert_ne!(UPDATE_PUBLIC_KEY, [0u8; 32]);
        assert_eq!(LATEST_SIG_URL, format!("{LATEST_URL}.sig"));
    }

    #[test]
    fn updates_come_only_from_our_server() {
        assert_eq!(LATEST_URL, format!("{DOWNLOAD_BASE}latest.json"));
        assert!(allowed_url(LATEST_URL));
        assert_eq!(download_url("ScreenPet_0.6.3_universal.dmg"), "https://pet.danni.day/downloads/ScreenPet_0.6.3_universal.dmg");
        assert!(allowed_url("https://PET.danni.day/downloads/x"));
        for bad in [
            "http://pet.danni.day/downloads/latest.json",
            "https://pet.danni.day.evil.com/a",
            "https://evil.com/pet.danni.day/a",
            "https://user@pet.danni.day/a",
            "https://pet.danni.day:8443/a",
            "https://xpet.danni.day/a",
            "https://danni.day/a",
            "https://pet.danni.day\\@evil.com/a",
            "https://pet.danni.day/a b",
            "https://github.com/allsilver11/screenpet/releases/download/v0.6.3/x.exe",
            "https://objects.githubusercontent.com/a",
            "https://api.github.com/repos/allsilver11/screenpet/releases/latest",
            "ftp://pet.danni.day/a",
            "",
        ] {
            assert!(!allowed_url(bad), "{bad}");
        }
    }

    #[test]
    fn redirects_follow_only_within_our_server() {
        let from = "https://pet.danni.day/downloads/latest.json";
        assert_eq!(redirect_target(from, "/downloads/other.json").as_deref(), Some("https://pet.danni.day/downloads/other.json"));
        assert_eq!(redirect_target(from, "https://pet.danni.day/x").as_deref(), Some("https://pet.danni.day/x"));
        assert_eq!(redirect_target(from, "https://github.com/x"), None);
        assert_eq!(redirect_target(from, "https://evil.com/x"), None);
        assert_eq!(redirect_target(from, "http://pet.danni.day/x"), None);
        assert_eq!(redirect_target(from, "//evil.com/x"), None);
        assert_eq!(redirect_target(from, "relative/x"), None);
    }

    #[test]
    fn a_newer_version_picks_this_platforms_file_with_its_sha256() {
        let json = latest_json("0.6.3", HEX);
        let Check::Newer(r) = check(&json, v("0.6.2"), Platform::MacOs).unwrap() else { panic!() };
        assert_eq!(r.version, v("0.6.3"));
        assert_eq!(r.asset.name, "ScreenPet_0.6.3_universal.dmg");
        assert_eq!(r.asset.size, 106147349);
        assert_eq!(r.asset.sha256, parse_sha256(HEX).unwrap());
        assert_eq!(r.asset.url, "https://pet.danni.day/downloads/ScreenPet_0.6.3_universal.dmg");
        let Check::Newer(r) = check(&json, v("0.6.2"), Platform::Windows).unwrap() else { panic!() };
        assert_eq!(r.asset.name, "ScreenPet_0.6.3_x64-setup.exe");
        assert_eq!(r.asset.size, 51426064);
        assert_eq!(r.asset.url, "https://pet.danni.day/downloads/ScreenPet_0.6.3_x64-setup.exe");
        assert_eq!(Platform::Windows.key(), "windows");
        assert_eq!(Platform::MacOs.key(), "macos");
    }

    #[test]
    fn same_or_older_version_is_up_to_date() {
        let json = latest_json("0.6.3", HEX);
        assert_eq!(check(&json, v("0.6.3"), Platform::Windows), Ok(Check::UpToDate(v("0.6.3"))));
        assert_eq!(check(&json, v("0.7.0"), Platform::Windows), Ok(Check::UpToDate(v("0.6.3"))));
    }

    #[test]
    fn missing_or_bad_sha256_refuses_to_install() {
        let ok = latest_json("0.6.3", HEX);
        let no_sha = ok.replace(&format!(r#","sha256":"{HEX}""#), "");
        assert_eq!(check(&no_sha, v("0.6.2"), Platform::Windows), Err(UpdateError::NoDigest));
        for bad in [&HEX[..63], "abcd", &format!("sha256:{HEX}")] {
            assert_eq!(check(&latest_json("0.6.3", bad), v("0.6.2"), Platform::Windows), Err(UpdateError::NoDigest), "{bad}");
        }
        assert!(UpdateError::NoDigest.to_string().contains("SHA-256"));
    }

    #[test]
    fn bad_latest_json_is_rejected() {
        let ok = latest_json("0.6.3", HEX);
        let w = Platform::Windows;
        assert_eq!(check("not json", v("0.6.2"), w), Err(UpdateError::BadResponse));
        assert_eq!(check(r#"{"version":"latest"}"#, v("0.6.2"), w), Err(UpdateError::BadResponse));
        for bad in ["v0.6.3", "0.6", "0.6.3-beta", "0.6.3.1"] {
            assert_eq!(check(&ok.replace(r#""version":"0.6.3""#, &format!(r#""version":"{bad}""#)), v("0.6.2"), w), Err(UpdateError::BadResponse), "{bad}");
        }
        assert_eq!(check(r#"{"version":"0.6.3"}"#, v("0.6.2"), w), Err(UpdateError::BadResponse));
        // 버전과 파일 이름의 버전이 다르거나, 이름이 이 컴퓨터용이 아니거나, 경로가 섞이면 이 컴퓨터용 파일이 없는 것
        assert_eq!(check(&ok.replace(r#""version":"0.6.3""#, r#""version":"0.6.4""#), v("0.6.2"), w), Err(UpdateError::NoAsset(v("0.6.4"))));
        let swapped = ok.replace("ScreenPet_0.6.3_x64-setup.exe", "ScreenPet_0.6.3_universal.dmg");
        assert_eq!(check(&swapped, v("0.6.2"), w), Err(UpdateError::NoAsset(v("0.6.3"))));
        let traversal = ok.replace("ScreenPet_0.6.3_x64-setup.exe", "../ScreenPet_0.6.3_x64-setup.exe");
        assert_eq!(check(&traversal, v("0.6.2"), w), Err(UpdateError::NoAsset(v("0.6.3"))));
        let url = ok.replace("ScreenPet_0.6.3_x64-setup.exe", "https://evil.example/ScreenPet_0.6.3_x64-setup.exe");
        assert_eq!(check(&url, v("0.6.2"), w), Err(UpdateError::NoAsset(v("0.6.3"))));
        let only_mac = format!(r#"{{"version":"0.6.3","files":{{"macos":{{"name":"ScreenPet_0.6.3_universal.dmg","sha256":"{HEX}","size":1}}}}}}"#);
        assert_eq!(check(&only_mac, v("0.6.2"), w), Err(UpdateError::NoAsset(v("0.6.3"))));
        // 크기: 300MB까지, 0·음수·글자는 안 된다
        assert_eq!(check(&ok.replace("51426064", "314572801"), v("0.6.2"), w), Err(UpdateError::TooBig));
        assert!(matches!(check(&ok.replace("51426064", "314572800"), v("0.6.2"), w), Ok(Check::Newer(_))));
        for bad in ["0", "-1", "\"12\"", "1.5"] {
            assert_eq!(check(&ok.replace("51426064", bad), v("0.6.2"), w), Err(UpdateError::BadResponse), "{bad}");
        }
    }

    #[test]
    fn bundle_path_comes_from_the_executable_inside_an_app() {
        assert_eq!(app_bundle_of(Path::new("/Applications/ScreenPet.app/Contents/MacOS/screenpet")), Some(PathBuf::from("/Applications/ScreenPet.app")));
        assert_eq!(app_bundle_of(Path::new("/Users/a/My Apps/Screen Pet 2.app/Contents/MacOS/x")), Some(PathBuf::from("/Users/a/My Apps/Screen Pet 2.app")));
        assert_eq!(app_bundle_of(Path::new("/usr/local/bin/screenpet")), None);
        assert_eq!(app_bundle_of(Path::new("/Applications/ScreenPet/Contents/MacOS/screenpet")), None);
        assert_eq!(app_bundle_of(Path::new("/x.app/Contents/Resources/screenpet")), None);
        assert_eq!(app_bundle_of(Path::new("screenpet")), None);
    }

    #[test]
    fn the_dmg_app_is_screenpet_or_the_only_app() {
        let names = |n: &[&str]| n.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        assert_eq!(pick_app(&names(&["Applications", "ScreenPet.app", ".background"])).as_deref(), Some("ScreenPet.app"));
        assert_eq!(pick_app(&names(&["Applications", "Other.app"])).as_deref(), Some("Other.app"));
        assert_eq!(pick_app(&names(&["A.app", "B.app"])), None);
        assert_eq!(pick_app(&names(&["Applications", ".hidden.app"])), None);
    }

    #[test]
    fn menu_labels_say_what_is_happening() {
        assert_eq!(MenuState::Idle.label(), "🔄 업데이트");
        assert_eq!(MenuState::Downloading(v("0.6.3")).label(), "🔄 업데이트 받는 중… (v0.6.3)");
        assert_eq!(MenuState::Available(v("0.6.3")).label(), "🔄 업데이트 (v0.6.3 나왔어요!)");
        assert_eq!(MenuState::UpToDate(v("0.6.2")).label(), "🔄 업데이트 — 최신 버전이에요 (v0.6.2)");
        assert!(MenuState::Failed(UpdateError::Network).label().starts_with("🔄 업데이트를 받지 못했어요: "));
        assert!(!MenuState::Checking.enabled() && !MenuState::Downloading(v("1.0.0")).enabled() && !MenuState::Installing(v("1.0.0")).enabled());
        assert!(MenuState::Idle.enabled() && MenuState::Available(v("1.0.0")).enabled() && MenuState::Failed(UpdateError::Disk).enabled());
        assert_eq!(MenuState::Available(v("1.0.0")).bubble(), None);
        for s in [MenuState::Downloading(v("1.0.0")), MenuState::UpToDate(v("1.0.0")), MenuState::Failed(UpdateError::Disk)] {
            assert!(s.bubble().unwrap().chars().count() <= 12, "{s:?}");
        }
    }

    #[test]
    fn nsis_flags_are_passive_relaunch_update() {
        assert_eq!(NSIS_ARGS, ["/P", "/R", "/UPDATE"]);
    }

    /// 도우미를 /usr/bin 대신 가짜 도구로 돌려 본다: 앱이 끝나면 번들을 바꾸고, 이미지를 떼고, 임시 폴더를 지우고, 연다
    #[cfg(unix)]
    #[test]
    fn mac_swap_script_replaces_the_bundle_after_the_app_exits() {
        use std::fs;
        use std::process::Command;
        let root = std::env::temp_dir().join(format!("screenpet-swap-test-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        let tools = root.join("tools");
        fs::create_dir_all(&tools).unwrap();
        let log = root.join("log");
        let write_tool = |name: &str, body: &str| {
            let p = tools.join(name);
            fs::write(&p, format!("#!/bin/sh\n{body}\n")).unwrap();
            Command::new("chmod").arg("+x").arg(&p).status().unwrap();
        };
        write_tool("ditto", r#"cp -R "$1" "$2""#);
        write_tool("xattr", "exit 0");
        write_tool("hdiutil", &format!(r#"echo "hdiutil $*" >> "{}""#, log.display()));
        write_tool("open", &format!(r#"echo "open $1" >> "{}""#, log.display()));
        let script = MAC_SWAP_SCRIPT.replace("/usr/bin/", &format!("{}/", tools.display()));

        let mnt = root.join("mnt");
        let src = mnt.join("ScreenPet.app");
        fs::create_dir_all(src.join("Contents")).unwrap();
        fs::write(src.join("Contents/version"), "new").unwrap();
        let dst = root.join("Apps dir/ScreenPet.app");
        fs::create_dir_all(dst.join("Contents")).unwrap();
        fs::write(dst.join("Contents/version"), "old").unwrap();
        fs::write(dst.join("Contents/stale"), "x").unwrap();
        let tmp = root.join("tmp");
        fs::create_dir_all(&tmp).unwrap();
        fs::write(tmp.join("ScreenPet.dmg"), "dmg").unwrap();

        // 이미 끝난 프로세스
        let mut done = Command::new("true").spawn().unwrap();
        let pid = done.id();
        done.wait().unwrap();

        let status = Command::new("/bin/sh").arg("-c").arg(&script).arg("sh").arg(pid.to_string()).arg(&mnt).arg(&src).arg(&dst).arg(&tmp).status().unwrap();
        assert!(status.success());
        assert_eq!(fs::read_to_string(dst.join("Contents/version")).unwrap(), "new");
        assert!(!dst.join("Contents/stale").exists());
        assert!(!root.join("Apps dir/ScreenPet.app.update-old").exists() && !root.join("Apps dir/ScreenPet.app.update-new").exists());
        assert!(!tmp.exists());
        let log = fs::read_to_string(&log).unwrap();
        assert!(log.contains(&format!("hdiutil detach {} -quiet", mnt.display())), "{log}");
        assert!(log.trim_end().ends_with(&format!("open {}", dst.display())), "{log}");
        let _ = fs::remove_dir_all(&root);
    }

    #[cfg(unix)]
    #[test]
    fn mac_swap_script_keeps_the_old_bundle_if_copying_fails() {
        use std::fs;
        use std::process::Command;
        let root = std::env::temp_dir().join(format!("screenpet-swap-fail-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        let tools = root.join("tools");
        fs::create_dir_all(&tools).unwrap();
        for (name, body) in [("ditto", "exit 1"), ("xattr", "exit 0"), ("hdiutil", "exit 0"), ("open", "exit 0")] {
            let p = tools.join(name);
            fs::write(&p, format!("#!/bin/sh\n{body}\n")).unwrap();
            Command::new("chmod").arg("+x").arg(&p).status().unwrap();
        }
        let script = MAC_SWAP_SCRIPT.replace("/usr/bin/", &format!("{}/", tools.display()));
        let dst = root.join("ScreenPet.app");
        fs::create_dir_all(&dst).unwrap();
        fs::write(dst.join("v"), "old").unwrap();
        let mut done = Command::new("true").spawn().unwrap();
        let pid = done.id();
        done.wait().unwrap();
        let status = Command::new("/bin/sh").arg("-c").arg(&script).arg("sh").arg(pid.to_string()).arg(root.join("mnt")).arg(root.join("mnt/ScreenPet.app")).arg(&dst).arg(root.join("tmp")).status().unwrap();
        assert!(status.success());
        assert_eq!(fs::read_to_string(dst.join("v")).unwrap(), "old");
        let _ = fs::remove_dir_all(&root);
    }
}
