//! 빌드 스크립트임: `ui/pets`의 `pet.json`을 훑어 `BUNDLED_PETS` 표를 `OUT_DIR`에 만듦.
//! `pets.rs`가 `include!`로 읽음. 시트가 빠진 동물이 있으면 빌드를 멈추고, 끝에 Tauri 빌드를 돌림.
use std::{env, fs, path::Path};

/// 동물 폴더를 검사해 매니페스트 표 코드를 쓰고 `tauri_build`를 부름
fn main() {
    let manifest_dir = env::var("CARGO_MANIFEST_DIR").unwrap();
    let pets_dir = Path::new(&manifest_dir).join("../ui/pets");
    println!("cargo:rerun-if-changed={}", pets_dir.display());

    let mut ids: Vec<String> = fs::read_dir(&pets_dir)
        .expect("ui/pets 폴더가 필요합니다")
        .filter_map(|e| e.ok())
        .filter(|e| e.path().join("pet.json").is_file())
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .collect();
    ids.sort();
    for id in &ids {
        assert!(
            // 배포 빌드는 tools/pack_webp.py가 WebP로 바꿔 둠
            pets_dir.join(id).join("sheet_teenager.png").is_file() || pets_dir.join(id).join("sheet_teenager.webp").is_file(),
            "ui/pets/{id} 폴더에 pet.json은 있지만 sheet_teenager.png(또는 sheet_teenager.webp)가 없습니다"
        );
    }
    assert!(!ids.is_empty(), "ui/pets 안에 pet.json이 있는 동물 폴더가 하나 이상 필요합니다");

    let mut code = String::from("pub const BUNDLED_PETS: &[(&str, &str)] = &[\n");
    for id in &ids {
        let path = pets_dir.join(id).join("pet.json");
        println!("cargo:rerun-if-changed={}", path.display());
        code.push_str(&format!("    ({:?}, include_str!({:?})),\n", id, path.display().to_string()));
    }
    code.push_str("];\n");
    fs::write(Path::new(&env::var("OUT_DIR").unwrap()).join("pets.rs"), code).unwrap();

    // 앱 커맨드도 권한 목록(capabilities/default.json)에 있는 창만 부를 수 있음. main.rs의 generate_handler와 같은 목록 (tools/test_capabilities.py가 맞춰 봄)
    tauri_build::try_build(tauri_build::Attributes::new().app_manifest(tauri_build::AppManifest::new().commands(&[
        "pointer_down",
        "slot_info",
        "feed",
        "show_menu",
        "house_state",
        "house_click",
        "pair_state",
        "pair_answer",
        "house_drag_start",
        "code_state",
        "code_submit",
        "code_renew",
        "code_close",
        "name_state",
        "name_submit",
        "name_close",
        "link_state",
        "link_submit",
        "link_close",
        "link_confirm",
        "size_state",
        "size_set",
        "size_close",
        "letter_state",
        "letter_submit",
        "letter_close",
        "feedback_submit",
        "feedback_close",
        "mailbox_state",
        "mailbox_read",
        "mailbox_done",
        "mailbox_reject",
        "mailbox_private",
        "mailbox_delete",
        "mailbox_block",
        "mailbox_close",
        "board_list",
        "board_close",
        "ladder_state",
        "ladder_host",
        "ladder_begin",
        "ladder_answer",
        "ladder_close",
        "ladder_seen",
        "timeline_state",
        "timeline_fit",
        "dodge_state",
        "dodge_keys",
        "dodge_start",
        "dodge_close",
        "object_down",
        "object_menu",
        "postit_max",
        "postit_submit",
        "postit_close",
        "note_space",
    ])))
    .expect("tauri build failed");
}
