// Suppress the console window on Windows release builds so the app launches
// as a GUI-only process; debug builds keep the console for log output.
#![cfg_attr(
    all(target_os = "windows", not(debug_assertions)),
    windows_subsystem = "windows"
)]

fn main() {
    tracing_subscriber::fmt()
        .json()
        .with_max_level(tracing::Level::INFO)
        .init();
    let verify_installation = std::env::args_os().nth(1).as_deref()
        == Some(std::ffi::OsStr::new("--verify-installation"));
    let result = if verify_installation {
        slint_demo::DesktopApp::verify_installation()
    } else {
        slint_demo::DesktopApp::run()
    };
    if let Err(error) = result {
        if !verify_installation && error.is_already_running() {
            tracing::info!(%error);
            return;
        }
        tracing::error!(%error);
        std::process::exit(1);
    }
}
