use std::env;
use std::fs;
use std::process::Command;

const PAYLOAD: &[u8] = include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/target/release/skills15.exe"));

fn main() {
    let base = env::var("LOCALAPPDATA")
        .or_else(|_| env::var("APPDATA"))
        .unwrap_or_else(|_| ".".to_string());
    let install_dir = std::path::PathBuf::from(base).join("Skills15");
    if let Err(e) = fs::create_dir_all(&install_dir) {
        eprintln!("Failed to create install directory: {e}");
        return;
    }
    let dest = install_dir.join("skills15.exe");
    if let Err(e) = fs::write(&dest, PAYLOAD) {
        eprintln!("Failed to write program: {e}");
        return;
    }
    println!("Installed Skills15 to {}", install_dir.display());
    let _ = Command::new(&dest)
        .current_dir(&install_dir)
        .spawn();
}
