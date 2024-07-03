use std::{path::Path, process::Command};

const WEB_CLIENT_DIR: &str = "./web_client";
const SERVER_DIR: &str = "./server";

fn main() {
    let web_client_dir = Path::new(WEB_CLIENT_DIR);

    // build the web client
    let mut build_wasm = Command::new("wasm-pack");
    build_wasm.arg("build");
    build_wasm.arg("--target").arg("web");
    build_wasm.current_dir(web_client_dir);
    build_wasm.status().unwrap();

    // copy client files to server
    let server_asset_dir = Path::new(SERVER_DIR).join("static");
    std::fs::remove_dir_all(&server_asset_dir).unwrap_or(()); // remove old files
    std::fs::create_dir_all(&server_asset_dir).unwrap();
    copy_dir(&web_client_dir.join("pkg"), &server_asset_dir);
    copy_dir(&web_client_dir.join("static"), &server_asset_dir);

    // run the server
    let server_dir = Path::new(SERVER_DIR);
    let mut run_server = Command::new("cargo");
    run_server.arg("run");
    run_server.current_dir(server_dir);
    run_server.status().unwrap();
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
