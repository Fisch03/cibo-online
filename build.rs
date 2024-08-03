use std::{path::Path, process::Command};

const WEB_CLIENT_DIR: &str = "./web_client";
const STATIC_GAME_DIR: &str = "./static/game";

fn main() {
    println!("cargo:rerun-if-changed=migrations/");

    build_client();
}

fn build_client() {
    let web_client_dir = Path::new(WEB_CLIENT_DIR);

    // build the web client
    if !std::env::var("SKIP_CLIENT_BUILD").is_ok() {
        let mut build_wasm = Command::new("wasm-pack");
        build_wasm.arg("build");
        build_wasm.arg("--target").arg("web");
        build_wasm.current_dir(web_client_dir);
        let status = build_wasm.status().unwrap();

        assert!(status.success(), "Failed to build web client");
    }

    // copy client files to server
    let server_asset_dir = Path::new(STATIC_GAME_DIR);
    std::fs::remove_dir_all(&server_asset_dir).unwrap_or(()); // remove old files
    std::fs::create_dir_all(&server_asset_dir).unwrap();
    copy_dir(&web_client_dir.join("pkg"), &server_asset_dir);
    copy_dir(&web_client_dir.join("static"), &server_asset_dir);

    println!("cargo:rerun-if-changed={}", WEB_CLIENT_DIR);
}

fn copy_dir(src: &Path, dest: &Path) {
    for entry in std::fs::read_dir(src).unwrap() {
        let entry = entry.unwrap();
        let path = entry.path();
        let dest_path = dest.join(path.file_name().unwrap());
        if path.is_dir() {
            std::fs::create_dir(&dest_path).unwrap();
            copy_dir(&path, &dest_path);
        } else {
            std::fs::copy(&path, &dest_path).unwrap();
        }
    }
}
