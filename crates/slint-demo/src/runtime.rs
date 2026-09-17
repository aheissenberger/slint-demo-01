use directories::ProjectDirs;
use fs2::FileExt;
use serde::{Deserialize, Serialize};
use std::fs::{self, File, OpenOptions};
use std::io;
use std::path::PathBuf;

const QUALIFIER: &str = "com";
const ORGANIZATION: &str = "aheissenberger";
const APPLICATION: &str = "slint-demo";
const RUNTIME_DIRECTORY: &str = "runtime";
const INSTANCE_LOCK_FILE: &str = "instance.lock";
const WINDOW_STATE_FILE: &str = "window-state.json";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct WindowState {
    pub width: u32,
    pub height: u32,
    pub x: i32,
    pub y: i32,
}

#[derive(Debug)]
pub struct RuntimeStorage {
    directory: PathBuf,
}

impl RuntimeStorage {
    pub fn open() -> io::Result<Self> {
        Self::at(Self::default_directory()?)
    }

    fn default_directory() -> io::Result<PathBuf> {
        ProjectDirs::from(QUALIFIER, ORGANIZATION, APPLICATION)
            .ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::NotFound,
                    "Betriebssystemdatenverzeichnis konnte nicht bestimmt werden",
                )
            })
            .map(|directories| directories.data_local_dir().join(RUNTIME_DIRECTORY))
    }

    fn at(directory: PathBuf) -> io::Result<Self> {
        fs::create_dir_all(&directory)?;
        Ok(Self { directory })
    }

    pub fn acquire_single_instance(&self) -> io::Result<SingleInstanceGuard> {
        let path = self.directory.join(INSTANCE_LOCK_FILE);
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(&path)?;
        file.try_lock_exclusive()?;
        Ok(SingleInstanceGuard { file })
    }

    pub fn load_window_state(&self) -> io::Result<Option<WindowState>> {
        let path = self.directory.join(WINDOW_STATE_FILE);
        let contents = match fs::read_to_string(&path) {
            Ok(contents) => contents,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(error),
        };
        serde_json::from_str(&contents).map(Some).map_err(|error| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("Gespeicherter Fensterzustand ist ungültig: {error}"),
            )
        })
    }

    pub fn save_window_state(&self, state: WindowState) -> io::Result<()> {
        let path = self.directory.join(WINDOW_STATE_FILE);
        let temporary_path = path.with_extension("tmp");
        let contents = serde_json::to_vec(&state).map_err(io::Error::other)?;
        fs::write(&temporary_path, contents)?;
        fs::rename(temporary_path, path)
    }
}

#[derive(Debug)]
pub struct SingleInstanceGuard {
    file: File,
}

impl Drop for SingleInstanceGuard {
    fn drop(&mut self) {
        if let Err(error) = FileExt::unlock(&self.file) {
            tracing::warn!(%error, "Prozesssperre konnte nicht freigegeben werden");
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeNotification {
    pub title: String,
    pub body: String,
}

pub fn show_notification(notification: &NativeNotification) {
    #[cfg(target_os = "macos")]
    {
        if let Err(error) = notify_rust::set_application("com.aheissenberger.slint-demo") {
            tracing::warn!(%error, "macOS-Bundle-ID für Benachrichtigung konnte nicht gesetzt werden");
        }
    }

    if let Err(error) = notify_rust::Notification::new()
        .appname("Slint Agent Demo")
        .summary(&notification.title)
        .body(&notification.body)
        .show()
    {
        tracing::warn!(%error, "Native Benachrichtigung konnte nicht angezeigt werden");
    }
}

pub fn is_single_instance_error(error: &io::Error) -> bool {
    matches!(error.kind(), io::ErrorKind::WouldBlock)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temporary_directory(test_name: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "slint-demo-runtime-{test_name}-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("system clock")
                .as_nanos()
        ))
    }

    #[test]
    fn window_state_round_trips_through_runtime_storage() {
        let directory = temporary_directory("window-state");
        let storage = RuntimeStorage::at(directory.clone()).expect("runtime storage");
        let state = WindowState {
            width: 1024,
            height: 768,
            x: 45,
            y: 90,
        };

        assert_eq!(
            storage.load_window_state().expect("load absent state"),
            None
        );
        storage.save_window_state(state).expect("save state");
        assert_eq!(
            storage.load_window_state().expect("load state"),
            Some(state)
        );

        fs::remove_dir_all(directory).expect("remove temporary runtime directory");
    }

    #[test]
    fn second_instance_cannot_acquire_the_process_lock() {
        let directory = temporary_directory("instance-lock");
        let storage = RuntimeStorage::at(directory.clone()).expect("runtime storage");
        let first_instance = storage.acquire_single_instance().expect("first instance");

        let error = storage
            .acquire_single_instance()
            .expect_err("second instance must be rejected");
        assert!(is_single_instance_error(&error));

        drop(first_instance);
        fs::remove_dir_all(directory).expect("remove temporary runtime directory");
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn default_runtime_directory_uses_windows_local_app_data() {
        let local_app_data =
            PathBuf::from(std::env::var_os("LOCALAPPDATA").expect("LOCALAPPDATA must be set"));
        let expected = local_app_data
            .join(ORGANIZATION)
            .join(APPLICATION)
            .join("data")
            .join(RUNTIME_DIRECTORY);

        assert_eq!(
            RuntimeStorage::default_directory().expect("runtime directory"),
            expected
        );
    }
}
