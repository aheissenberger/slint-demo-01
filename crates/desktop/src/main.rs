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
    desktop::DesktopApp::run();
}
