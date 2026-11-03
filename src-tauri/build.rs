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
            pets_dir.join(id).join("sheet.png").is_file(),
            "ui/pets/{id} 폴더에 pet.json은 있지만 sheet.png가 없습니다"
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

    // 하우스 그림의 문 위치 (ui/house/house.json). 아직 없으면 None — 앱은 기본 문 위치를 쓴다
    let house_json = Path::new(&manifest_dir).join("../ui/house/house.json");
    println!("cargo:rerun-if-changed={}", house_json.display());
    let house = if house_json.is_file() {
        format!("pub const HOUSE_JSON: Option<&str> = Some(include_str!({:?}));\n", house_json.display().to_string())
    } else {
        "pub const HOUSE_JSON: Option<&str> = None;\n".to_string()
    };
    fs::write(Path::new(&env::var("OUT_DIR").unwrap()).join("house.rs"), house).unwrap();

    tauri_build::build();
}
