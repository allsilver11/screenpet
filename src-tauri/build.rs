use std::{env, fs, path::Path};

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
            // 배포 빌드는 tools/pack_webp.py가 WebP로 바꿔 둔다
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

    // 픽셀 그림 (ui/pets_pixel/<id>/pet.json + 다섯 단계 시트). 아직 없는 동물은 빠지고, 그 동물은 픽셀 스타일에서도 동글로 보인다
    let pixel_dir = Path::new(&manifest_dir).join("../ui/pets_pixel");
    println!("cargo:rerun-if-changed={}", pixel_dir.display());
    let mut pixel_ids: Vec<String> = fs::read_dir(&pixel_dir)
        .map(|dir| dir.filter_map(|e| e.ok()).filter(|e| e.path().join("pet.json").is_file()).map(|e| e.file_name().to_string_lossy().into_owned()).collect())
        .unwrap_or_default();
    pixel_ids.sort();
    let mut code = String::from("pub const BUNDLED_PIXEL_PETS: &[(&str, &str)] = &[\n");
    for id in &pixel_ids {
        let missing = ["baby", "kinder", "child", "teenager", "adult"].into_iter().find(|st| !pixel_dir.join(id).join(format!("sheet_{st}.png")).is_file());
        if let Some(st) = missing {
            println!("cargo:warning=ui/pets_pixel/{id}: sheet_{st}.png 없음 — 이 동물은 동글로 보인다");
            continue;
        }
        let path = pixel_dir.join(id).join("pet.json");
        println!("cargo:rerun-if-changed={}", path.display());
        code.push_str(&format!("    ({:?}, include_str!({:?})),\n", id, path.display().to_string()));
    }
    code.push_str("];\n");
    fs::write(Path::new(&env::var("OUT_DIR").unwrap()).join("pets_pixel.rs"), code).unwrap();

    // 앱 커맨드도 권한 목록(capabilities/default.json)에 있는 창만 부를 수 있다. main.rs의 generate_handler와 같은 목록 (tools/test_capabilities.py가 맞춰 본다)
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
        "ladder_state",
        "ladder_host",
        "ladder_begin",
        "ladder_answer",
        "ladder_close",
        "ladder_seen",
        "gift_collect",
        "book_state",
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
