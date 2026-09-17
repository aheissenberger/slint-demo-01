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
    println!("cargo:rerun-if-changed=translations");

    // Static UI text in ui/**/*.slint is marked with the `@tr(...)` macro and
    // resolved through Slint's own translation infrastructure rather than
    // being piped through Rust. Translations are bundled directly into the
    // binary (no runtime `gettext` dependency); the `.po` files live under
    // `translations/<locale>/LC_MESSAGES/<domain>.po`, where `<domain>` is
    // this crate's package name ("slint-demo"). German is the source
    // language the `@tr("...")` literals are written in, so `de` is an
    // identity translation and doubles as the documented fallback if no
    // bundled locale matches at runtime.
    let config =
        slint_build::CompilerConfiguration::new().with_bundled_translations("translations");
    slint_build::compile_with_config("../../ui/app.slint", config)
        .expect("Slint UI should compile");
}
