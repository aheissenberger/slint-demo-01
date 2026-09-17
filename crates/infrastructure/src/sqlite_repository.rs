use application::{AppRepository, ApplicationError, NoteRepository};
use chrono::{DateTime, Utc};
use directories::ProjectDirs;
use domain::{AppId, AppSettings, Note, NoteBody, NoteId, NoteTitle, SubmissionRecord};
use rusqlite::{params, Connection};
use rusqlite_migration::{Migrations, M};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use tracing::{debug, instrument};

const DATA_DIR_ENV_VAR: &str = "SLINT_DEMO_DATA_DIR";
const DATABASE_FILE_NAME: &str = "app-data.sqlite3";

#[derive(Debug, Clone)]
pub struct SqliteRepository {
    connection: Arc<Mutex<Connection>>,
}

impl SqliteRepository {
    pub fn open(path: impl Into<PathBuf>) -> Result<Self, ApplicationError> {
        let path = path.into();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|error| {
                ApplicationError::Repository(format!(
                    "Datenverzeichnis konnte nicht angelegt werden: {error}"
                ))
            })?;
        }
        let mut connection = Connection::open(&path).map_err(sqlite_error)?;
        connection
            .pragma_update(None, "foreign_keys", true)
            .map_err(sqlite_error)?;
        Self::migrations()
            .to_latest(&mut connection)
            .map_err(|error| {
                ApplicationError::DataRecoveryRequired(format!(
                    "Datenbankschema konnte nicht migriert werden: {error}"
                ))
            })?;
        debug!(path = %path.display(), "initialized SQLite repository");
        Ok(Self {
            connection: Arc::new(Mutex::new(connection)),
        })
    }

    pub fn in_memory() -> Result<Self, ApplicationError> {
        let mut connection = Connection::open_in_memory().map_err(sqlite_error)?;
        connection
            .pragma_update(None, "foreign_keys", true)
            .map_err(sqlite_error)?;
        Self::migrations()
            .to_latest(&mut connection)
            .map_err(|error| {
                ApplicationError::DataRecoveryRequired(format!(
                    "Datenbankschema konnte nicht migriert werden: {error}"
                ))
            })?;
        Ok(Self {
            connection: Arc::new(Mutex::new(connection)),
        })
    }

    pub fn with_default_location() -> Result<Self, ApplicationError> {
        Self::open(Self::default_data_file())
    }

    fn default_data_file() -> PathBuf {
        Self::default_data_dir().join(DATABASE_FILE_NAME)
    }

    fn default_data_dir() -> PathBuf {
        if let Some(dir) = std::env::var_os(DATA_DIR_ENV_VAR) {
            return PathBuf::from(dir);
        }
        ProjectDirs::from("com", "aheissenberger", "slint-demo")
            .map(|directories| directories.data_local_dir().to_path_buf())
            .unwrap_or_else(|| PathBuf::from(".slint-demo"))
    }

    fn migrations() -> Migrations<'static> {
        Migrations::new(vec![M::up(
            r#"
            CREATE TABLE app_settings (
                id INTEGER PRIMARY KEY CHECK (id = 1),
                app_name TEXT NOT NULL,
                server_url TEXT NOT NULL,
                enabled INTEGER NOT NULL CHECK (enabled IN (0, 1)),
                theme_mode TEXT NOT NULL CHECK (theme_mode IN ('system', 'light', 'dark'))
            );
            INSERT INTO app_settings (id, app_name, server_url, enabled, theme_mode)
            VALUES (1, 'Slint Agent Demo', 'https://example.internal', 1, 'system');

            CREATE TABLE submissions (
                id TEXT PRIMARY KEY,
                title TEXT NOT NULL,
                description TEXT NOT NULL,
                created_at TEXT NOT NULL,
                active INTEGER NOT NULL CHECK (active IN (0, 1))
            );

            CREATE TABLE notes (
                id TEXT PRIMARY KEY,
                title TEXT NOT NULL,
                body TEXT NOT NULL,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL,
                archived INTEGER NOT NULL CHECK (archived IN (0, 1))
            );
            CREATE INDEX notes_active_updated_at_idx ON notes (archived, updated_at DESC);
            "#,
        )])
    }

    fn lock(&self) -> Result<std::sync::MutexGuard<'_, Connection>, ApplicationError> {
        self.connection
            .lock()
            .map_err(|_| ApplicationError::Repository("Datenbanksperre beschädigt".into()))
    }

    pub fn build_service(&self) -> application::AppService<Self> {
        application::AppService::new(self.clone())
    }
}

impl AppRepository for SqliteRepository {
    fn load_settings(&self) -> Result<AppSettings, ApplicationError> {
        self.lock()?
            .query_row(
                "SELECT app_name, server_url, enabled, theme_mode FROM app_settings WHERE id = 1",
                [],
                |row| {
                    Ok(AppSettings {
                        app_name: row.get(0)?,
                        server_url: row.get(1)?,
                        enabled: row.get::<_, i64>(2)? != 0,
                        theme_mode: row.get(3)?,
                    })
                },
            )
            .map_err(sqlite_error)
    }

    fn save_settings(&self, settings: AppSettings) -> Result<(), ApplicationError> {
        self.lock()?
            .execute(
                r#"
                UPDATE app_settings
                SET app_name = ?1, server_url = ?2, enabled = ?3, theme_mode = ?4
                WHERE id = 1
                "#,
                params![
                    settings.app_name,
                    settings.server_url,
                    bool_to_i64(settings.enabled),
                    settings.theme_mode
                ],
            )
            .map_err(sqlite_error)?;
        Ok(())
    }

    fn save_submission(&self, submission: SubmissionRecord) -> Result<(), ApplicationError> {
        self.lock()?
            .execute(
                r#"
                INSERT INTO submissions (id, title, description, created_at, active)
                VALUES (?1, ?2, ?3, ?4, ?5)
                "#,
                params![
                    submission.id.as_str(),
                    submission.title,
                    submission.description,
                    submission.created_at.to_rfc3339(),
                    bool_to_i64(submission.active)
                ],
            )
            .map_err(sqlite_error)?;
        Ok(())
    }

    fn list_submissions(&self) -> Result<Vec<SubmissionRecord>, ApplicationError> {
        let connection = self.lock()?;
        let mut statement = connection
            .prepare(
                "SELECT id, title, description, created_at, active FROM submissions ORDER BY created_at DESC",
            )
            .map_err(sqlite_error)?;
        let records = statement
            .query_map([], |row| {
                let id: String = row.get(0)?;
                let created_at: String = row.get(3)?;
                Ok(SubmissionRecord {
                    id: AppId::new(id).map_err(domain_error_to_sqlite)?,
                    title: row.get(1)?,
                    description: row.get(2)?,
                    created_at: parse_datetime(&created_at).map_err(domain_error_to_sqlite)?,
                    active: row.get::<_, i64>(4)? != 0,
                })
            })
            .map_err(sqlite_error)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(sqlite_error)?;
        Ok(records)
    }
}

impl NoteRepository for SqliteRepository {
    fn create_note(&self, note: Note) -> Result<(), ApplicationError> {
        self.lock()?
            .execute(
                r#"
                INSERT INTO notes (id, title, body, created_at, updated_at, archived)
                VALUES (?1, ?2, ?3, ?4, ?5, ?6)
                "#,
                params![
                    note.id().as_str(),
                    note.title().as_str(),
                    note.body().as_str(),
                    note.created_at().to_rfc3339(),
                    note.updated_at().to_rfc3339(),
                    bool_to_i64(note.is_archived())
                ],
            )
            .map_err(sqlite_error)?;
        Ok(())
    }

    fn list_notes(&self) -> Result<Vec<Note>, ApplicationError> {
        let connection = self.lock()?;
        let mut statement = connection
            .prepare(
                r#"
                SELECT id, title, body, created_at, updated_at, archived
                FROM notes
                ORDER BY archived ASC, updated_at DESC, title COLLATE NOCASE ASC
                "#,
            )
            .map_err(sqlite_error)?;
        let notes = statement
            .query_map([], row_to_note)
            .map_err(sqlite_error)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(sqlite_error)?;
        Ok(notes)
    }

    fn update_note(&self, note: Note) -> Result<(), ApplicationError> {
        self.lock()?
            .execute(
                r#"
                UPDATE notes
                SET title = ?2, body = ?3, updated_at = ?4, archived = ?5
                WHERE id = ?1
                "#,
                params![
                    note.id().as_str(),
                    note.title().as_str(),
                    note.body().as_str(),
                    note.updated_at().to_rfc3339(),
                    bool_to_i64(note.is_archived())
                ],
            )
            .map_err(sqlite_error)?;
        Ok(())
    }

    fn delete_note(&self, id: &NoteId) -> Result<(), ApplicationError> {
        self.lock()?
            .execute("DELETE FROM notes WHERE id = ?1", params![id.as_str()])
            .map_err(sqlite_error)?;
        Ok(())
    }
}

fn row_to_note(row: &rusqlite::Row<'_>) -> rusqlite::Result<Note> {
    let id: String = row.get(0)?;
    let title: String = row.get(1)?;
    let body: String = row.get(2)?;
    let created_at: String = row.get(3)?;
    let updated_at: String = row.get(4)?;
    Ok(Note::from_persisted(
        NoteId::new(id).map_err(domain_error_to_sqlite)?,
        NoteTitle::new(title).map_err(domain_error_to_sqlite)?,
        NoteBody::new(body).map_err(domain_error_to_sqlite)?,
        parse_datetime(&created_at).map_err(domain_error_to_sqlite)?,
        parse_datetime(&updated_at).map_err(domain_error_to_sqlite)?,
        row.get::<_, i64>(5)? != 0,
    ))
}

fn parse_datetime(value: &str) -> Result<DateTime<Utc>, String> {
    DateTime::parse_from_rfc3339(value)
        .map(|datetime| datetime.with_timezone(&Utc))
        .map_err(|error| format!("Zeitstempel ist ungültig: {error}"))
}

fn domain_error_to_sqlite(error: impl std::fmt::Display) -> rusqlite::Error {
    rusqlite::Error::ToSqlConversionFailure(Box::new(std::io::Error::new(
        std::io::ErrorKind::InvalidData,
        error.to_string(),
    )))
}

fn sqlite_error(error: rusqlite::Error) -> ApplicationError {
    match error {
        rusqlite::Error::SqliteFailure(
            rusqlite::ffi::Error {
                code:
                    rusqlite::ErrorCode::DatabaseCorrupt
                    | rusqlite::ErrorCode::NotADatabase
                    | rusqlite::ErrorCode::SchemaChanged,
                ..
            },
            message,
        ) => ApplicationError::DataRecoveryRequired(format!(
            "SQLite-Datenbank ist beschädigt oder inkompatibel: {}",
            message.unwrap_or_else(|| "keine Detailmeldung".into())
        )),
        error => ApplicationError::Repository(format!("SQLite-Fehler: {error}")),
    }
}

fn bool_to_i64(value: bool) -> i64 {
    if value {
        1
    } else {
        0
    }
}

/// Initializes the default persistent repository using SQLite and applies all
/// pending migrations before the application state is loaded.
#[instrument(
    name = "infrastructure.sqlite_initialized",
    fields(component = "repository")
)]
pub fn initialize_sqlite_repository() -> Result<SqliteRepository, ApplicationError> {
    SqliteRepository::with_default_location()
}

#[cfg(test)]
mod tests {
    use super::*;
    use application::{AppRepository, NoteRepository};
    use chrono::TimeZone;
    use tempfile::TempDir;

    fn data_file(dir: &TempDir) -> PathBuf {
        dir.path().join("nested").join(DATABASE_FILE_NAME)
    }

    #[test]
    fn migrations_create_the_schema_and_default_settings() {
        let repo = SqliteRepository::in_memory().unwrap();
        assert_eq!(repo.load_settings().unwrap(), AppSettings::default());
    }

    #[test]
    fn settings_submissions_and_notes_survive_reopening() {
        let dir = TempDir::new().unwrap();
        let path = data_file(&dir);
        let repo = SqliteRepository::open(&path).unwrap();
        repo.save_settings(AppSettings {
            theme_mode: "dark".into(),
            ..AppSettings::default()
        })
        .unwrap();

        let submitted_at = Utc.with_ymd_and_hms(2026, 1, 2, 3, 4, 5).single().unwrap();
        let record = SubmissionRecord {
            id: AppId::new("submission-1").unwrap(),
            title: "submit".into(),
            description: "Wert".into(),
            created_at: submitted_at,
            active: true,
        };
        repo.save_submission(record.clone()).unwrap();

        let note = Note::new(
            NoteId::new("note-1").unwrap(),
            NoteTitle::new("Titel").unwrap(),
            NoteBody::new("Inhalt").unwrap(),
            submitted_at,
        );
        repo.create_note(note.clone()).unwrap();
        drop(repo);

        let reopened = SqliteRepository::open(path).unwrap();
        assert_eq!(reopened.load_settings().unwrap().theme_mode, "dark");
        assert_eq!(reopened.list_submissions().unwrap(), vec![record]);
        assert_eq!(reopened.list_notes().unwrap(), vec![note]);
    }

    #[test]
    fn notes_can_be_updated_archived_and_deleted() {
        let repo = SqliteRepository::in_memory().unwrap();
        let created_at = Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).single().unwrap();
        let id = NoteId::new("note-1").unwrap();
        let mut note = Note::new(
            id.clone(),
            NoteTitle::new("Titel").unwrap(),
            NoteBody::new("Inhalt").unwrap(),
            created_at,
        );
        repo.create_note(note.clone()).unwrap();

        note.rename(NoteTitle::new("Neu").unwrap(), created_at);
        note.archive(created_at);
        repo.update_note(note).unwrap();
        let notes = repo.list_notes().unwrap();
        assert_eq!(notes[0].title().as_str(), "Neu");
        assert!(notes[0].is_archived());

        repo.delete_note(&id).unwrap();
        assert!(repo.list_notes().unwrap().is_empty());
    }

    #[test]
    fn schema_version_is_tracked_by_migration_crate() {
        let repo = SqliteRepository::in_memory().unwrap();
        let connection = repo.lock().unwrap();
        assert_eq!(
            SqliteRepository::migrations()
                .current_version(&connection)
                .unwrap(),
            rusqlite_migration::SchemaVersion::Inside(std::num::NonZeroUsize::new(1).unwrap())
        );
    }
}
