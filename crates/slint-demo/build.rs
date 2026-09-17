use std::fs;
use std::path::Path;

fn track_ui_files(path: &Path) {
    println!("cargo:rerun-if-changed={}", path.display());
    if path.is_dir() {
        for entry in fs::read_dir(path).expect("UI directory should be readable") {
            track_ui_files(&entry.expect("UI directory entry should be readable").path());
        }
    }
}

fn main() {
    // Track imported Slint files as well as app.slint. Tracking each path
    // avoids relying on Cargo's platform-dependent directory semantics.
    track_ui_files(Path::new("../../ui"));
    slint_build::compile("../../ui/app.slint").expect("Slint UI should compile");
}
