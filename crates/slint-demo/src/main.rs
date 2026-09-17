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
    if let Err(error) = slint_demo::DesktopApp::run() {
        if error.is_already_running() {
            tracing::info!(%error);
            return;
        }
        tracing::error!(%error);
        std::process::exit(1);
    }
}
