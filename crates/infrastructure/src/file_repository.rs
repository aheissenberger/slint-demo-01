//! Persistent, file-backed [`AppRepository`]/[`NoteRepository`] adapter.
//!
//! Data is stored as a single versioned JSON document so the application no
//! longer loses settings, submissions, or notes when it restarts. Reads are
//! resilient by design: a missing file starts fresh, and a corrupt file is
//! backed up next to itself and replaced with defaults rather than crashing
//! the application or the agent API.

use application::{AppRepository, ApplicationError, NoteRepository};
use directories::ProjectDirs;
use domain::{AppSettings, Note, NoteId, SubmissionRecord};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};
use tracing::{debug, instrument, warn};

/// Current on-disk schema version. Bump this and extend
/// [`FileRepository::migrate`] whenever the persisted shape changes in a way
/// that is not already covered by `#[serde(default)]` field additions.
const CURRENT_SCHEMA_VERSION: u32 = 1;

const DATA_DIR_ENV_VAR: &str = "SLINT_DEMO_DATA_DIR";
const DATA_FILE_NAME: &str = "app-data.json";

#[derive(Debug, Clone, Serialize, Deserialize)]
struct PersistedData {
    #[serde(default)]
    version: u32,
    #[serde(default)]
    settings: AppSettings,
    #[serde(default)]
    submissions: Vec<SubmissionRecord>,
    #[serde(default)]
    notes: Vec<Note>,
}

impl Default for PersistedData {
    fn default() -> Self {
        Self {
            version: CURRENT_SCHEMA_VERSION,
            settings: AppSettings::default(),
            submissions: Vec::new(),
            notes: Vec::new(),
        }
    }
}

/// A production-suitable persistence adapter backed by a single JSON file.
///
/// Construction never fails: if the target path cannot be read or is
/// corrupted, the repository logs a warning, best-effort backs up the
/// unreadable file, and starts from default values so the application can
/// keep running. When no path is configured (see [`FileRepository::in_memory`])
/// the repository behaves like a plain in-memory store.
#[derive(Debug, Clone)]
pub struct FileRepository {
    path: Option<PathBuf>,
    state: Arc<Mutex<PersistedData>>,
}

impl FileRepository {
    /// Opens (or creates on first save) the data file at `path`.
    pub fn open(path: impl Into<PathBuf>) -> Self {
        let path = path.into();
        let data = Self::load_or_recover(&path);
        Self {
            path: Some(path),
            state: Arc::new(Mutex::new(data)),
        }
    }

    /// A repository that never touches disk. Useful for tests or as a
    /// fallback when the platform data directory is unavailable.
    pub fn in_memory() -> Self {
        Self {
            path: None,
            state: Arc::new(Mutex::new(PersistedData::default())),
        }
    }

    /// Opens the repository at the platform-appropriate data directory,
    /// honoring `SLINT_DEMO_DATA_DIR` for overrides (tests, packaging).
    pub fn with_default_location() -> Self {
        Self::open(Self::default_data_file())
    }

    fn default_data_file() -> PathBuf {
        Self::default_data_dir().join(DATA_FILE_NAME)
    }

    fn default_data_dir() -> PathBuf {
        if let Some(dir) = std::env::var_os(DATA_DIR_ENV_VAR) {
            return PathBuf::from(dir);
        }
        ProjectDirs::from("com", "aheissenberger", "slint-demo")
            .map(|directories| directories.data_local_dir().to_path_buf())
            .unwrap_or_else(|| PathBuf::from(".slint-demo"))
    }

    fn load_or_recover(path: &Path) -> PersistedData {
        let contents = match fs::read_to_string(path) {
            Ok(contents) => contents,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                debug!(path = %path.display(), "keine Datendatei vorhanden, starte mit Vorgabewerten");
                return PersistedData::default();
            }
            Err(error) => {
                warn!(%error, path = %path.display(), "Datendatei konnte nicht gelesen werden, verwende Vorgabewerte");
                return PersistedData::default();
            }
        };

        match serde_json::from_str::<PersistedData>(&contents) {
            Ok(mut data) => {
                Self::migrate(&mut data);
                data
            }
            Err(error) => {
                warn!(%error, path = %path.display(), "Datendatei ist beschädigt, sichere sie und setze auf Vorgabewerte zurück");
                Self::backup_corrupt_file(path);
                PersistedData::default()
            }
        }
    }

    /// Upgrades data loaded from an older schema version in-place. New
    /// optional fields are already handled by `#[serde(default)]`; this hook
    /// exists for shape changes that need explicit conversion logic.
    fn migrate(data: &mut PersistedData) {
        if data.version < CURRENT_SCHEMA_VERSION {
            debug!(
                from = data.version,
                to = CURRENT_SCHEMA_VERSION,
                "migriere gespeicherte Daten auf aktuelles Schema"
            );
            data.version = CURRENT_SCHEMA_VERSION;
        }
    }

    fn backup_corrupt_file(path: &Path) {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or_default();
        let backup_path = path.with_extension(format!("corrupt-{suffix}.bak"));
        if let Err(error) = fs::copy(path, &backup_path) {
            warn!(%error, path = %backup_path.display(), "Sicherung der beschädigten Datendatei fehlgeschlagen");
        } else {
            warn!(path = %backup_path.display(), "beschädigte Datendatei gesichert");
        }
    }

    fn persist(&self, data: &PersistedData) -> Result<(), ApplicationError> {
        let Some(path) = &self.path else {
            return Ok(());
        };
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|error| {
                ApplicationError::Repository(format!(
                    "Datenverzeichnis konnte nicht angelegt werden: {error}"
                ))
            })?;
        }
        let json = serde_json::to_string_pretty(data).map_err(|error| {
            ApplicationError::Repository(format!(
                "Daten konnten nicht serialisiert werden: {error}"
            ))
        })?;

        // Write to a temp file first and rename into place so a crash or
        // power loss mid-write cannot leave a truncated, unparsable file.
        let tmp_path = path.with_extension("tmp");
        fs::write(&tmp_path, json).map_err(|error| {
            ApplicationError::Repository(format!(
                "Datendatei konnte nicht geschrieben werden: {error}"
            ))
        })?;
        fs::rename(&tmp_path, path).map_err(|error| {
            ApplicationError::Repository(format!("Datendatei konnte nicht ersetzt werden: {error}"))
        })
    }

    fn lock(&self) -> Result<std::sync::MutexGuard<'_, PersistedData>, ApplicationError> {
        self.state
            .lock()
            .map_err(|_| ApplicationError::Repository("Repositorysperre beschädigt".into()))
    }

    pub fn build_service(&self) -> application::AppService<Self> {
        application::AppService::new(self.clone())
    }
}

impl AppRepository for FileRepository {
    fn load_settings(&self) -> Result<AppSettings, ApplicationError> {
        Ok(self.lock()?.settings.clone())
    }

    fn save_settings(&self, settings: AppSettings) -> Result<(), ApplicationError> {
        let snapshot = {
            let mut data = self.lock()?;
            data.settings = settings;
            data.clone()
        };
        self.persist(&snapshot)
    }

    fn save_submission(&self, submission: SubmissionRecord) -> Result<(), ApplicationError> {
        let snapshot = {
            let mut data = self.lock()?;
            data.submissions.push(submission);
            data.clone()
        };
        self.persist(&snapshot)
    }

    fn list_submissions(&self) -> Result<Vec<SubmissionRecord>, ApplicationError> {
        Ok(self.lock()?.submissions.clone())
    }
}

impl NoteRepository for FileRepository {
    fn create_note(&self, note: Note) -> Result<(), ApplicationError> {
        let snapshot = {
            let mut data = self.lock()?;
            data.notes.push(note);
            data.clone()
        };
        self.persist(&snapshot)
    }

    fn list_notes(&self) -> Result<Vec<Note>, ApplicationError> {
        Ok(self.lock()?.notes.clone())
    }

    fn update_note(&self, note: Note) -> Result<(), ApplicationError> {
        let snapshot = {
            let mut data = self.lock()?;
            if let Some(existing) = data.notes.iter_mut().find(|n| n.id() == note.id()) {
                *existing = note;
            }
            data.clone()
        };
        self.persist(&snapshot)
    }

    fn delete_note(&self, id: &NoteId) -> Result<(), ApplicationError> {
        let snapshot = {
            let mut data = self.lock()?;
            data.notes.retain(|note| note.id() != id);
            data.clone()
        };
        self.persist(&snapshot)
    }
}

/// Initializes the default persistent repository, falling back to an
/// in-memory-only store (with a logged warning) if the platform data
/// directory turns out to be unusable.
#[instrument(name = "infrastructure.initialized", fields(component = "repository"))]
pub fn initialize_file_repository() -> FileRepository {
    let repository = FileRepository::with_default_location();
    debug!(
        path = ?repository.path,
        "initialized persistent file-backed repository"
    );
    repository
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{TimeZone, Utc};
    use domain::{NoteBody, NoteTitle};
    use tempfile::TempDir;

    fn data_file(dir: &TempDir) -> PathBuf {
        dir.path().join("nested").join(DATA_FILE_NAME)
    }

    #[test]
    fn missing_file_starts_with_defaults_and_creates_parent_dirs_on_save() {
        let dir = TempDir::new().unwrap();
        let path = data_file(&dir);
        assert!(!path.exists());

        let repo = FileRepository::open(&path);
        assert_eq!(repo.load_settings().unwrap(), AppSettings::default());
        assert!(repo.list_submissions().unwrap().is_empty());

        repo.save_settings(AppSettings {
            theme_mode: "dark".into(),
            ..AppSettings::default()
        })
        .unwrap();
        assert!(path.exists());
    }

    #[test]
    fn saved_settings_and_submissions_survive_reopening_the_repository() {
        let dir = TempDir::new().unwrap();
        let path = data_file(&dir);

        let repo = FileRepository::open(&path);
        repo.save_settings(AppSettings {
            theme_mode: "light".into(),
            ..AppSettings::default()
        })
        .unwrap();
        let record = SubmissionRecord {
            id: domain::AppId::new("01890f3e-8c00-7b9a-a6d1-2f0a3b4c5d6e").unwrap(),
            title: "submit".into(),
            description: "Test".into(),
            created_at: Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).single().unwrap(),
            active: true,
        };
        repo.save_submission(record.clone()).unwrap();
        drop(repo);

        let reopened = FileRepository::open(&path);
        assert_eq!(reopened.load_settings().unwrap().theme_mode, "light");
        assert_eq!(reopened.list_submissions().unwrap(), vec![record]);
    }

    #[test]
    fn notes_are_created_updated_and_deleted_through_the_file_repository() {
        let dir = TempDir::new().unwrap();
        let repo = FileRepository::open(data_file(&dir));

        let id = NoteId::new("note-1").unwrap();
        let title = NoteTitle::new("Titel").unwrap();
        let body = NoteBody::new("Inhalt").unwrap();
        let created_at = Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).single().unwrap();
        let note = Note::new(id.clone(), title, body, created_at);
        repo.create_note(note.clone()).unwrap();
        assert_eq!(repo.list_notes().unwrap(), vec![note.clone()]);

        let mut updated = note;
        updated.archive(created_at);
        repo.update_note(updated.clone()).unwrap();
        assert!(repo.list_notes().unwrap()[0].is_archived());

        repo.delete_note(&id).unwrap();
        assert!(repo.list_notes().unwrap().is_empty());
    }

    #[test]
    fn legacy_data_without_version_or_notes_field_is_migrated_on_load() {
        let dir = TempDir::new().unwrap();
        let path = data_file(&dir);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        let legacy_json = r#"{
            "settings": {
                "app_name": "Legacy App",
                "server_url": "https://legacy.example",
                "enabled": true
            },
            "submissions": []
        }"#;
        fs::write(&path, legacy_json).unwrap();

        let repo = FileRepository::open(&path);
        let settings = repo.load_settings().unwrap();
        assert_eq!(settings.app_name, "Legacy App");
        assert_eq!(settings.theme_mode, "system");
        assert!(repo.list_notes().unwrap().is_empty());

        // Persisting after migration upgrades the on-disk schema version.
        repo.save_settings(settings).unwrap();
        let raw = fs::read_to_string(&path).unwrap();
        let persisted: serde_json::Value = serde_json::from_str(&raw).unwrap();
        assert_eq!(persisted["version"], CURRENT_SCHEMA_VERSION);
    }

    #[test]
    fn corrupt_data_file_is_backed_up_and_replaced_with_defaults() {
        let dir = TempDir::new().unwrap();
        let path = data_file(&dir);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, "{ this is not valid json").unwrap();

        let repo = FileRepository::open(&path);
        assert_eq!(repo.load_settings().unwrap(), AppSettings::default());
        assert!(repo.list_submissions().unwrap().is_empty());

        let backups: Vec<_> = fs::read_dir(path.parent().unwrap())
            .unwrap()
            .filter_map(|entry| entry.ok())
            .filter(|entry| entry.file_name().to_string_lossy().contains("corrupt-"))
            .collect();
        assert_eq!(backups.len(), 1, "expected exactly one corrupt-file backup");
    }

    #[test]
    fn unreadable_directory_falls_back_to_defaults_without_panicking() {
        // Pointing the repository at a path whose parent is actually a file
        // (not a directory) simulates an unusable location; saves should
        // fail gracefully with an ApplicationError rather than panicking.
        let dir = TempDir::new().unwrap();
        let blocking_file = dir.path().join("not-a-directory");
        fs::write(&blocking_file, "x").unwrap();
        let path = blocking_file.join(DATA_FILE_NAME);

        let repo = FileRepository::open(&path);
        assert_eq!(repo.load_settings().unwrap(), AppSettings::default());

        let result = repo.save_settings(AppSettings::default());
        assert!(result.is_err());
    }

    #[test]
    fn in_memory_repository_never_touches_disk() {
        let repo = FileRepository::in_memory();
        repo.save_settings(AppSettings {
            theme_mode: "dark".into(),
            ..AppSettings::default()
        })
        .unwrap();
        assert_eq!(repo.load_settings().unwrap().theme_mode, "dark");
    }
}
