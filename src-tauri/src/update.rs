//! 🔄 업데이트 (트레이 메뉴): 우리 서버(https://pet.danni.day/downloads/)의 latest.json을 보고 이 컴퓨터용 설치 파일을 받아
//! 크기·SHA-256을 확인한 뒤 설치하고 앱을 끝냄. 주소는 고정 (사용자가 바꾸는 릴레이 주소와 상관없음), pet.danni.day https만.
//! Tauri 업데이터 플러그인은 쓰지 않음 — 대신 latest.json에 우리 Ed25519 서명(latest.json.sig, 공개 키는 screenpet_core::update)이 맞아야
//! 그 안을 믿고, 받은 파일은 latest.json의 sha256·size와 맞아야만 설치함. 서버(릴레이)를 가진 사람도 비밀 키 없이는 설치 파일을 바꿔 끼울 수 없음.
//! 버전 견주기·파일 고르기·주소 확인은 screenpet_core::update (Linux에서 시험), 여기는 네트워크·파일·프로세스만.
//! 켜진 뒤 1분, 그 뒤 2시간마다 조용히 확인해서 새 버전이 있으면 메뉴 글자만 바꿈 (누르기 전엔 받지도 깔지도 않음)
use std::fs::{self, File};
use std::io::{ErrorKind, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use std::thread;
use std::time::{Duration, Instant};

use screenpet_log::log;
use screenpet_core::update::{self, Check, MenuState, Platform, Release, UpdateError, Version};
use sha2::{Digest, Sha256};
use tauri::{AppHandle, Emitter, Manager};

use crate::game_loop::Input;
use crate::AppState;

/// 눌러서 시작한 확인·받기·설치가 도는 중 (두 번 누르기와 조용한 확인이 메뉴 글자를 덮는 것을 막음)
static BUSY: AtomicBool = AtomicBool::new(false);
/// 메뉴 글자를 바꾸는 쪽이 둘(눌러서·조용히)이라 한 번에 하나씩
static MENU_LOCK: Mutex<()> = Mutex::new(());

/// 연결 시간 제한
const CONNECT_TIMEOUT: Duration = Duration::from_secs(15);
/// latest.json 받기 시간 제한
const LATEST_TIMEOUT: Duration = Duration::from_secs(30);
/// 설치 파일 받기 시간 제한
const DOWNLOAD_TIMEOUT: Duration = Duration::from_secs(30 * 60);
/// latest.json 상한 (보통 수백 바이트)
const LATEST_BODY_LIMIT: u64 = 64 * 1024;
/// latest.json.sig는 128글자
const SIG_BODY_LIMIT: u64 = 1024;
/// 서명 받기가 실패하면 이만큼 뒤 한 번 더 해 봄
const SIGNATURE_RETRY_AFTER: Duration = Duration::from_secs(3);
/// 받은 파일을 두는 곳 (사용자별 임시 폴더). 받을 때마다 비우고 시작함
const WORK_DIR: &str = "screenpet-update";

/// 이 빌드의 버전
fn current_version() -> Version {
    Version::parse(env!("CARGO_PKG_VERSION")).expect("package version is X.Y.Z")
}

/// 트레이 "🔄 업데이트"를 눌렀음: 뒤에서 확인 → (새 버전이면) 받기 → 확인 → 설치 → 앱 끝내기
pub fn on_click(app: &AppHandle) {
    if BUSY.swap(true, Ordering::SeqCst) {
        return;
    }
    let app = app.clone();
    thread::spawn(move || {
        if let Err(e) = run(&app) {
            log!(Error, "update", "update_fail", "err" => format!("{e:?}"));
            show(&app, MenuState::Failed(e), true);
        }
        BUSY.store(false, Ordering::SeqCst);
    });
}

/// 업데이트 한 번을 돌림: 확인 → 받기 → 확인 → 설치. 단계마다 메뉴 글자를 바꿈
fn run(app: &AppHandle) -> Result<(), UpdateError> {
    let platform = Platform::current().ok_or(UpdateError::Unsupported)?;
    show(app, MenuState::Checking, false);
    // 서버에 올린 릴리즈가 없으면 "최신 버전이에요" (실패로 보이지 않게)
    let release = match update::missing_is_up_to_date(fetch_latest(platform), current_version())? {
        Check::UpToDate(_) => {
            show(app, MenuState::UpToDate(current_version()), true);
            return Ok(());
        }
        Check::Newer(release) => release,
    };
    announce(app, release.version);
    show(app, MenuState::Downloading(release.version), true);
    let file = download(&release)?;
    show(app, MenuState::Installing(release.version), true);
    log!(Info, "update", "install_start", "version" => release.version);
    match platform {
        Platform::Windows => install_windows(app, &file),
        Platform::MacOs => install_macos(app, &file),
    }
}

/// 켜진 뒤 한 번, 그 뒤 2시간마다 조용히 확인함. 새 버전이면 메뉴 글자만 "🔄 업데이트 (vX 나왔어요!)" — 받거나 깔지 않음
pub fn start_quiet_checks(app: &AppHandle) {
    let Some(platform) = Platform::current() else { return };
    let app = app.clone();
    thread::spawn(move || {
        thread::sleep(update::FIRST_CHECK_AFTER);
        loop {
            match fetch_latest(platform) {
                Ok(Check::Newer(r)) => {
                    log!(Info, "update", "available", "version" => r.version);
                    show_quietly(&app, MenuState::Available(r.version));
                    announce(&app, r.version);
                }
                Ok(Check::UpToDate(_)) => {}
                Err(e) => log!(Warn, "update", "check_fail", "err" => format!("{e:?}")),
            }
            thread::sleep(update::CHECK_EVERY);
        }
    });
}

/// 새 버전을 찾았다고 게임 루프에 알림: 내 동물들이 "new version!!" 하고 뜀 (버전마다 한 번은 게임 루프가 지킴)
fn announce(app: &AppHandle, v: Version) {
    let _ = app.state::<AppState>().tx.lock().unwrap().send(Input::NewVersion(v));
}

/// 메뉴 글자 (+ 눌러서 시작한 것이면 펫 말풍선 한 줄)
fn show(app: &AppHandle, state: MenuState, say: bool) {
    let _guard = MENU_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    set_menu(app, &state);
    if say {
        if let Some(line) = state.bubble() {
            let target = if app.state::<AppState>().housed.load(Ordering::Relaxed) { crate::house::LABEL } else { crate::slots::OWN_LABEL };
            let _ = app.emit_to(target, "pet-say-text", line);
        }
    }
}

/// 조용한 확인: 눌러서 하는 업데이트가 도는 동안은 글자를 건드리지 않음
fn show_quietly(app: &AppHandle, state: MenuState) {
    let _guard = MENU_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    if !BUSY.load(Ordering::SeqCst) {
        set_menu(app, &state);
    }
}

/// 트레이 업데이트 메뉴 글자를 바꿈
fn set_menu(app: &AppHandle, state: &MenuState) {
    let item = &app.state::<AppState>().menu.update;
    let _ = item.set_text(state.label());
    let _ = item.set_enabled(state.enabled());
}

/// 넘겨주기는 따라가지 않게 해 두고 (max_redirects(0)) 한 번마다 허용된 곳인지 보며 직접 따라감
fn agent(timeout: Duration) -> ureq::Agent {
    ureq::Agent::config_builder()
        .https_only(true)
        .max_redirects(0)
        .http_status_as_error(false)
        .user_agent(format!("ScreenPet/{}", env!("CARGO_PKG_VERSION")))
        .timeout_connect(Some(CONNECT_TIMEOUT))
        .timeout_global(Some(timeout))
        .build()
        .into()
}

/// 허락된 주소(pet.danni.day https)만 GET함. 넘겨주기도 같은 호스트만 따라감
fn get(agent: &ureq::Agent, url: &str, accept: &str) -> Result<ureq::http::Response<ureq::Body>, UpdateError> {
    // pet.danni.day https만 (넘겨주기도 redirect_target이 같은 호스트만 줌)
    if !update::allowed_url(url) {
        return Err(UpdateError::BadUrl);
    }
    let mut url = url.to_string();
    for _ in 0..=update::MAX_REDIRECTS {
        let resp = agent.get(&url).header("Accept", accept).call().map_err(|e| {
            log!(Warn, "update", "get_fail", "err" => e);
            UpdateError::Network
        })?;
        let status = resp.status();
        if status.is_redirection() {
            let location = resp.headers().get("location").and_then(|v| v.to_str().ok()).ok_or(UpdateError::BadResponse)?;
            url = update::redirect_target(&url, location).ok_or(UpdateError::BadUrl)?;
            continue;
        }
        if !status.is_success() {
            log!(Warn, "update", "http_status", "status" => status.as_u16());
            return Err(update::status_error(status.as_u16()));
        }
        return Ok(resp);
    }
    Err(UpdateError::BadUrl)
}

/// latest.json과 latest.json.sig를 받아 서명(update::UPDATE_PUBLIC_KEY)부터 확인하고, 맞을 때만 읽음.
/// 서버가 둘을 막 바꾸는 순간에 걸렸을 수 있으니 서명이 틀리면 한 번만 잠깐 뒤에 다시 받음 — 그래도 틀리면 아무것도 믿지 않음
fn fetch_latest(platform: Platform) -> Result<Check, UpdateError> {
    let agent = agent(LATEST_TIMEOUT);
    let json = match fetch_signed(&agent) {
        Err(UpdateError::BadSignature) => {
            thread::sleep(SIGNATURE_RETRY_AFTER);
            fetch_signed(&agent)?
        }
        other => other?,
    };
    let json = String::from_utf8(json).map_err(|_| UpdateError::BadResponse)?;
    update::check(&json, current_version(), platform)
}

/// 서명이 맞는 latest.json 바이트 그대로 (확인 전엔 읽지 않음)
fn fetch_signed(agent: &ureq::Agent) -> Result<Vec<u8>, UpdateError> {
    let json = fetch_bytes(agent, update::LATEST_URL, "application/json", LATEST_BODY_LIMIT)?;
    let sig = fetch_bytes(agent, update::LATEST_SIG_URL, "text/plain", SIG_BODY_LIMIT)?;
    update::verify_latest(&json, &sig).inspect_err(|_| log!(Error, "update", "bad_signature"))?;
    Ok(json)
}

/// 주소에서 최대 `limit`바이트를 받음
fn fetch_bytes(agent: &ureq::Agent, url: &str, accept: &str, limit: u64) -> Result<Vec<u8>, UpdateError> {
    let mut resp = get(agent, url, accept)?;
    resp.body_mut().with_config().limit(limit).read_to_vec().map_err(|e| {
        let file = url.rsplit('/').next().unwrap_or("");
        log!(Warn, "update", "read_fail", "file" => file, "err" => e);
        UpdateError::Network
    })
}

/// 설치 파일을 임시 폴더에 받음: 크기는 latest.json이 말한 만큼 (상한 update::MAX_DOWNLOAD), SHA-256이 맞아야 .part를 떼어 줌
fn download(release: &Release) -> Result<PathBuf, UpdateError> {
    let dir = std::env::temp_dir().join(WORK_DIR);
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).map_err(|_| UpdateError::Disk)?;
    let started = Instant::now();
    log!(Info, "update", "download_start", "version" => release.version, "bytes" => release.asset.size);
    let result = download_into(&dir, release);
    let took_ms = started.elapsed().as_millis();
    match &result {
        Ok(_) => log!(Info, "update", "download_ok", "version" => release.version, "bytes" => release.asset.size, "took_ms" => took_ms),
        Err(e) => {
            log!(Error, "update", "download_fail", "version" => release.version, "took_ms" => took_ms, "err" => format!("{e:?}"));
            let _ = fs::remove_dir_all(&dir);
        }
    }
    result
}

/// 설치 파일을 `dir`에 받아 크기와 SHA-256을 확인함
fn download_into(dir: &Path, release: &Release) -> Result<PathBuf, UpdateError> {
    let size = release.asset.size;
    if size > update::MAX_DOWNLOAD {
        return Err(UpdateError::TooBig);
    }
    let resp = get(&agent(DOWNLOAD_TIMEOUT), &release.asset.url, "application/octet-stream")?;
    // 말한 크기보다 1바이트 더까지만 읽음 (넘치면 SizeMismatch)
    let mut body = resp.into_body().into_reader().take(size + 1);
    let part = dir.join(format!("{}.part", release.asset.name));
    let mut file = File::create(&part).map_err(|_| UpdateError::Disk)?;
    let mut hasher = Sha256::new();
    let mut total: u64 = 0;
    let mut buf = vec![0u8; 64 * 1024];
    loop {
        let n = match body.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => n,
            Err(e) if e.kind() == ErrorKind::Interrupted => continue,
            Err(e) => {
                log!(Warn, "update", "read_fail", "file" => release.asset.name.as_str(), "err" => e);
                return Err(UpdateError::Network);
            }
        };
        total += n as u64;
        if total > size {
            return Err(UpdateError::SizeMismatch);
        }
        hasher.update(&buf[..n]);
        file.write_all(&buf[..n]).map_err(|_| UpdateError::Disk)?;
    }
    if total != size {
        return Err(UpdateError::SizeMismatch);
    }
    file.sync_all().map_err(|_| UpdateError::Disk)?;
    drop(file);
    if hasher.finalize().as_slice() != release.asset.sha256.as_slice() {
        return Err(UpdateError::DigestMismatch);
    }
    let path = dir.join(&release.asset.name);
    fs::rename(&part, &path).map_err(|_| UpdateError::Disk)?;
    Ok(path)
}

/// 설정을 저장하고 앱을 끝냄 (트레이 "종료"와 같은 길)
fn quit_app(app: &AppHandle) {
    if app.state::<AppState>().tx.lock().unwrap().send(Input::Quit).is_err() {
        app.exit(0);
    }
}

/// Windows: NSIS 설치 파일을 /P /R /UPDATE로 띄우고 (진행 막대만, 다 깔면 다시 켬) 앱을 끝내 파일이 잠기지 않게 함.
/// 셸 없이 인자로만 넘김. 설치 모드가 currentUser라 관리자 권한을 묻지 않음
fn install_windows(app: &AppHandle, installer: &Path) -> Result<(), UpdateError> {
    let installer = installer.to_path_buf();
    Command::new(&installer).args(update::NSIS_ARGS).stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null()).spawn().map_err(|e| {
        log!(Error, "update", "install_fail", "err" => e);
        UpdateError::Install("설치 프로그램을 열지 못했어요")
    })?;
    quit_app(app);
    Ok(())
}

/// macOS: dmg를 붙이고 (Finder에 띄우지 않고 읽기 전용), 안의 ScreenPet.app을 찾아,
/// 앱이 끝나기를 기다렸다가 지금 번들을 바꾸고 다시 여는 도우미(update::MAC_SWAP_SCRIPT)를 띄운 뒤 앱을 끝냄
#[cfg(unix)]
fn install_macos(app: &AppHandle, dmg: &Path) -> Result<(), UpdateError> {
    use std::os::unix::process::CommandExt;
    let exe = std::env::current_exe().and_then(|p| p.canonicalize()).map_err(|_| UpdateError::Install("앱 위치를 찾지 못했어요"))?;
    let bundle = update::app_bundle_of(&exe).ok_or(UpdateError::Install("ScreenPet.app 안에서 실행하고 있지 않아요"))?;
    let folder = bundle.parent().ok_or(UpdateError::Install("앱 위치를 찾지 못했어요"))?;
    if !can_write(folder) {
        return Err(UpdateError::Install("앱 폴더에 쓸 수 없어요 — ScreenPet을 응용 프로그램 폴더로 옮긴 뒤 다시 해 주세요"));
    }
    let dir = dmg.parent().ok_or(UpdateError::Disk)?.to_path_buf();
    let mnt = dir.join("mnt");
    fs::create_dir_all(&mnt).map_err(|_| UpdateError::Disk)?;
    let attached = Command::new("/usr/bin/hdiutil")
        .args(["attach", "-nobrowse", "-readonly", "-noautoopen", "-mountpoint"])
        .arg(&mnt)
        .arg(dmg)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
    if !matches!(attached, Ok(s) if s.success()) {
        let _ = fs::remove_dir_all(&dir);
        return Err(UpdateError::Install("디스크 이미지를 열지 못했어요"));
    }
    let names: Vec<String> = fs::read_dir(&mnt).map(|d| d.filter_map(|e| e.ok()).map(|e| e.file_name().to_string_lossy().into_owned()).collect()).unwrap_or_default();
    let src = update::pick_app(&names).map(|name| mnt.join(name)).filter(|src| src.join("Contents").join("Info.plist").is_file());
    let Some(src) = src else {
        detach(&mnt, &dir);
        return Err(UpdateError::Install("디스크 이미지 안에 앱이 없어요"));
    };
    let helper = Command::new("/bin/sh")
        .arg("-c")
        .arg(update::MAC_SWAP_SCRIPT)
        .arg("sh")
        .arg(std::process::id().to_string())
        .arg(&mnt)
        .arg(&src)
        .arg(&bundle)
        .arg(&dir)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .process_group(0)
        .spawn();
    if let Err(e) = helper {
        log!(Error, "update", "install_fail", "err" => e);
        detach(&mnt, &dir);
        return Err(UpdateError::Install("새 버전으로 바꾸지 못했어요"));
    }
    quit_app(app);
    Ok(())
}

/// unix 밖에서는 지원 안 함
#[cfg(not(unix))]
fn install_macos(_app: &AppHandle, _dmg: &Path) -> Result<(), UpdateError> {
    Err(UpdateError::Unsupported)
}

/// dmg를 떼고 작업 폴더를 지움
#[cfg(unix)]
fn detach(mnt: &Path, dir: &Path) {
    let _ = Command::new("/usr/bin/hdiutil").arg("detach").arg(mnt).arg("-force").stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null()).status();
    let _ = fs::remove_dir_all(dir);
}

/// 번들을 바꿔 끼울 폴더에 쓸 수 있는지 (옮기지 않고 dmg·다운로드 폴더에서 바로 켠 앱은 읽기 전용 자리에서 돎)
#[cfg(unix)]
fn can_write(folder: &Path) -> bool {
    let probe = folder.join(format!(".screenpet-update-check-{}", std::process::id()));
    let ok = fs::create_dir(&probe).is_ok();
    let _ = fs::remove_dir(&probe);
    ok
}
