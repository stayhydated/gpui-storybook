use std::{
    fmt,
    io::{self, Write as _},
    path::{Path, PathBuf},
    sync::Arc,
    time::{SystemTime, UNIX_EPOCH},
};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use strum::{EnumString, IntoStaticStr};

use crate::{ConsumerId, PreferenceRecord, StorybookPreferences};

const JSON_FILE_NAME: &str = "preferences.json";
const JSON_SCHEMA_FILE_NAME: &str = "preferences.schema.json";
const JSON_SCHEMA_ID: &str = "https://stayhydated.github.io/gpui-storybook/preferences.schema.json";
const STORYBOOK_DIR: &str = ".gpui-storybook";
const STORYBOOK_GITIGNORE_FILE_NAME: &str = ".gitignore";
const STORYBOOK_GITIGNORE_CONTENTS: &[u8] = b"*\n";

/// JSON document stored for one Storybook consumer.
#[derive(Debug, Deserialize, JsonSchema, Serialize)]
#[serde(deny_unknown_fields)]
struct PreferenceDocument {
    /// Relative path to the schema that describes this document.
    #[serde(rename = "$schema")]
    schema: String,
    /// Stable identifier for the Storybook binary that owns these preferences.
    consumer_id: ConsumerId,
    #[serde(flatten)]
    record: PreferenceRecord,
}

impl PreferenceDocument {
    fn new(consumer_id: ConsumerId, schema_path: &Path, record: PreferenceRecord) -> Self {
        Self {
            schema: schema_path.file_name().map_or_else(
                || JSON_SCHEMA_FILE_NAME.into(),
                |name| name.to_string_lossy().into(),
            ),
            consumer_id,
            record,
        }
    }

    fn into_record(
        self,
        expected_consumer: &ConsumerId,
    ) -> Result<PreferenceRecord, InvalidJsonDocument> {
        if &self.consumer_id != expected_consumer {
            return Err(InvalidJsonDocument::ConsumerMismatch {
                expected: expected_consumer.clone(),
                actual: self.consumer_id,
            });
        }
        Ok(self.record)
    }
}

/// Returns the JSON Schema generated from the persisted preference document.
pub fn preference_json_schema() -> serde_json::Value {
    let mut schema = serde_json::to_value(schemars::schema_for!(PreferenceDocument))
        .expect("generated preference schema should serialize");
    let object = schema
        .as_object_mut()
        .expect("generated preference schema should be an object");
    object.insert("$id".into(), JSON_SCHEMA_ID.into());
    object.insert("title".into(), "GPUI Storybook Preferences".into());
    object.insert(
        "description".into(),
        "Consumer-scoped developer preferences for a GPUI Storybook binary".into(),
    );
    schema
}

/// Returns the generated preference JSON Schema as formatted JSON.
pub fn preference_json_schema_pretty() -> String {
    let mut schema = serde_json::to_string_pretty(&preference_json_schema())
        .expect("generated preference schema should serialize");
    schema.push('\n');
    schema
}

/// Source of timestamps used for recovery archive names.
pub trait PreferenceClock: Send + Sync {
    /// Returns Unix time in milliseconds.
    ///
    /// # Errors
    ///
    /// Returns [`PreferenceClockError`] when the clock is before the Unix epoch
    /// or cannot fit the persisted timestamp representation.
    fn now_unix_millis(&self) -> Result<i64, PreferenceClockError>;
}

/// Production preference clock backed by [`SystemTime`].
#[derive(Clone, Copy, Debug, Default)]
pub struct SystemPreferenceClock;

impl PreferenceClock for SystemPreferenceClock {
    fn now_unix_millis(&self) -> Result<i64, PreferenceClockError> {
        let duration = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|_| PreferenceClockError::BeforeUnixEpoch)?;
        duration
            .as_millis()
            .try_into()
            .map_err(|_| PreferenceClockError::OutOfRange)
    }
}

/// Failure to produce a local preference timestamp.
#[derive(Clone, Copy, Debug, Eq, thiserror::Error, PartialEq)]
pub enum PreferenceClockError {
    /// The source clock was before the Unix epoch.
    #[error("preference clock is before the Unix epoch")]
    BeforeUnixEpoch,
    /// The timestamp did not fit the persisted representation.
    #[error("preference timestamp is out of range")]
    OutOfRange,
}

/// Local persistence behavior selected by a Storybook consumer.
#[derive(Clone, Copy, Debug, Default, EnumString, Eq, IntoStaticStr, PartialEq)]
#[strum(const_into_str, serialize_all = "snake_case")]
pub enum PersistenceMode {
    /// Store preferences as JSON in the project's hidden Storybook directory.
    #[default]
    Persistent,
    /// Store preferences in a unique temporary JSON file.
    Temporary,
    /// Keep preferences in memory for the repository lifetime.
    Disabled,
}

impl PersistenceMode {
    /// Returns the stable diagnostic token.
    pub const fn token(self) -> &'static str {
        self.into_str()
    }
}

/// Options for opening one consumer-scoped preference repository.
#[derive(Clone)]
pub struct RepositoryOptions {
    /// Stable identifier unique to the consuming Storybook binary.
    pub consumer_id: ConsumerId,
    /// Persistence behavior.
    pub persistence: PersistenceMode,
    /// Explicit persistent JSON path for portable development and tests.
    pub json_path: Option<PathBuf>,
    /// Cargo workspace or standalone package root for the default JSON path.
    ///
    /// `None` uses the process working directory. This value is ignored when
    /// [`Self::json_path`] is set.
    pub project_root: Option<PathBuf>,
    /// Timestamp source used for recovery archive names.
    pub clock: Arc<dyn PreferenceClock>,
}

impl RepositoryOptions {
    /// Creates persistent options using the project-local Storybook data path.
    pub fn persistent(consumer_id: ConsumerId) -> Self {
        Self::new(consumer_id, PersistenceMode::Persistent)
    }

    /// Creates unique temporary file-backed options.
    pub fn temporary(consumer_id: ConsumerId) -> Self {
        Self::new(consumer_id, PersistenceMode::Temporary)
    }

    /// Creates in-memory disabled-persistence options.
    pub fn disabled(consumer_id: ConsumerId) -> Self {
        Self::new(consumer_id, PersistenceMode::Disabled)
    }

    fn new(consumer_id: ConsumerId, persistence: PersistenceMode) -> Self {
        Self {
            consumer_id,
            persistence,
            json_path: None,
            project_root: None,
            clock: Arc::new(SystemPreferenceClock),
        }
    }
}

impl fmt::Debug for RepositoryOptions {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("RepositoryOptions")
            .field("consumer_id", &self.consumer_id)
            .field("persistence", &self.persistence)
            .field("json_path", &self.json_path)
            .field("project_root", &self.project_root)
            .finish_non_exhaustive()
    }
}

/// Structured record of invalid-JSON recovery.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RecoveryDiagnostic {
    /// JSON path reset after recovery.
    pub json_path: PathBuf,
    /// Archived invalid JSON path.
    pub archived_path: PathBuf,
    /// Typed recovery reason.
    pub reason: RecoveryReason,
}

/// Reason a persistent JSON document was archived.
#[derive(Clone, Copy, Debug, Eq, IntoStaticStr, PartialEq)]
#[strum(const_into_str, serialize_all = "snake_case")]
pub enum RecoveryReason {
    /// The file was not a valid preference document for this consumer.
    InvalidJson,
}

impl RecoveryReason {
    /// Returns the stable diagnostic token.
    pub const fn token(self) -> &'static str {
        self.into_str()
    }
}

/// Successful repository open plus any startup recovery diagnostic.
#[derive(Clone, Debug)]
pub struct OpenRepository {
    /// Open consumer-scoped repository.
    pub repository: PreferenceRepository,
    /// Invalid-JSON recovery performed during open.
    pub recovery: Option<RecoveryDiagnostic>,
}

struct RepositoryInner {
    consumer_id: ConsumerId,
    persistence: PersistenceMode,
    record: tokio::sync::Mutex<Option<PreferenceRecord>>,
    path: Option<PathBuf>,
    schema_path: Option<PathBuf>,
    _temporary_directory: Option<tempfile::TempDir>,
}

/// Consumer-scoped typed Storybook preference repository.
#[derive(Clone)]
pub struct PreferenceRepository {
    inner: Arc<RepositoryInner>,
}

impl fmt::Debug for PreferenceRepository {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("PreferenceRepository")
            .field("consumer_id", &self.inner.consumer_id)
            .field("persistence", &self.inner.persistence)
            .field("path", &self.inner.path)
            .field("schema_path", &self.inner.schema_path)
            .finish_non_exhaustive()
    }
}

impl PreferenceRepository {
    /// Opens the selected JSON or in-memory storage backend.
    ///
    /// Persistent and temporary repositories write a JSON Schema generated
    /// from the persisted Rust document type beside the preference file. An
    /// invalid persistent document is archived before the repository continues
    /// with empty saved state.
    ///
    /// # Errors
    ///
    /// Returns [`RepositoryOpenError`] for invalid options, clock failures, or
    /// filesystem failures.
    pub async fn open(options: RepositoryOptions) -> Result<OpenRepository, RepositoryOpenError> {
        if options.persistence != PersistenceMode::Persistent && options.json_path.is_some() {
            return Err(RepositoryOpenError::PathOverrideRequiresPersistent {
                persistence: options.persistence,
            });
        }

        tracing::debug!(
            consumer_id = %options.consumer_id,
            persistence = options.persistence.token(),
            "resolving Storybook preference storage"
        );

        match options.persistence {
            PersistenceMode::Disabled => Ok(OpenRepository {
                repository: Self::from_parts(options, None, None, None, None),
                recovery: None,
            }),
            PersistenceMode::Temporary => {
                let temporary_directory = tokio::task::spawn_blocking(tempfile::tempdir)
                    .await
                    .map_err(|source| RepositoryOpenError::TemporaryDirectoryTask { source })?
                    .map_err(|source| RepositoryOpenError::TemporaryDirectory { source })?;
                let path = temporary_directory.path().join(JSON_FILE_NAME);
                let schema_path = temporary_directory.path().join(JSON_SCHEMA_FILE_NAME);
                prepare_parent(&path).await?;
                write_schema(&schema_path).await?;
                Ok(OpenRepository {
                    repository: Self::from_parts(
                        options,
                        Some(path),
                        Some(schema_path),
                        Some(temporary_directory),
                        None,
                    ),
                    recovery: None,
                })
            },
            PersistenceMode::Persistent => {
                let uses_default_path = options.json_path.is_none();
                let path = match &options.json_path {
                    Some(path) => path.clone(),
                    None => persistent_json_path(
                        options
                            .project_root
                            .as_deref()
                            .unwrap_or_else(|| Path::new("")),
                        &options.consumer_id,
                    ),
                };
                validate_json_path(&path)?;
                let schema_path = schema_path_for(&path);
                validate_distinct_schema_path(&path, &schema_path)?;
                prepare_parent(&path).await?;
                if uses_default_path {
                    ensure_storybook_gitignore(&path).await?;
                }
                write_schema(&schema_path).await?;

                let (record, recovery) = match read_document(&path, &options.consumer_id).await {
                    Ok(record) => (record, None),
                    Err(ReadDocumentError::Io { source })
                        if source.kind() == io::ErrorKind::NotFound =>
                    {
                        (None, None)
                    },
                    Err(ReadDocumentError::Invalid { source }) => {
                        let suffix = options.clock.now_unix_millis()?;
                        let archived_path = archive_invalid_json(&path, suffix).await?;
                        tracing::warn!(
                            path = %path.display(),
                            archived_path = %archived_path.display(),
                            reason = RecoveryReason::InvalidJson.token(),
                            error = %source,
                            "archived invalid Storybook preference JSON"
                        );
                        (
                            None,
                            Some(RecoveryDiagnostic {
                                json_path: path.clone(),
                                archived_path,
                                reason: RecoveryReason::InvalidJson,
                            }),
                        )
                    },
                    Err(ReadDocumentError::Io { source }) => {
                        return Err(RepositoryOpenError::JsonIo {
                            path: path.clone(),
                            source,
                        });
                    },
                };

                Ok(OpenRepository {
                    repository: Self::from_parts(
                        options,
                        Some(path),
                        Some(schema_path),
                        None,
                        record,
                    ),
                    recovery,
                })
            },
        }
    }

    fn from_parts(
        options: RepositoryOptions,
        path: Option<PathBuf>,
        schema_path: Option<PathBuf>,
        temporary_directory: Option<tempfile::TempDir>,
        record: Option<PreferenceRecord>,
    ) -> Self {
        Self {
            inner: Arc::new(RepositoryInner {
                consumer_id: options.consumer_id,
                persistence: options.persistence,
                record: tokio::sync::Mutex::new(record),
                path,
                schema_path,
                _temporary_directory: temporary_directory,
            }),
        }
    }

    /// Returns the stable consumer identifier scoped by this repository.
    pub fn consumer_id(&self) -> &ConsumerId {
        &self.inner.consumer_id
    }

    /// Returns the selected persistence mode.
    pub fn persistence(&self) -> PersistenceMode {
        self.inner.persistence
    }

    /// Returns the preference JSON path for persistent or temporary storage.
    pub fn path(&self) -> Option<&Path> {
        self.inner.path.as_deref()
    }

    /// Returns the generated JSON Schema path for file-backed storage.
    pub fn schema_path(&self) -> Option<&Path> {
        self.inner.schema_path.as_deref()
    }

    /// Loads the consumer's saved record.
    pub async fn load(&self) -> Result<Option<PreferenceRecord>, PreferenceStoreError> {
        Ok(self.inner.record.lock().await.clone())
    }

    /// Creates the consumer record and rejects an existing value.
    pub async fn create(
        &self,
        preferences: StorybookPreferences,
    ) -> Result<PreferenceRecord, PreferenceStoreError> {
        let mut record = self.inner.record.lock().await;
        if record.is_some() {
            return Err(PreferenceStoreError::AlreadyExists {
                consumer_id: self.inner.consumer_id.clone(),
            });
        }
        let created = PreferenceRecord { preferences };
        self.persist(Some(&created), StoreOperation::Create).await?;
        *record = Some(created.clone());
        Ok(created)
    }

    /// Updates the consumer record and rejects a missing value.
    pub async fn update(
        &self,
        preferences: StorybookPreferences,
    ) -> Result<PreferenceRecord, PreferenceStoreError> {
        let mut record = self.inner.record.lock().await;
        if record.is_none() {
            return Err(PreferenceStoreError::NotFound {
                consumer_id: self.inner.consumer_id.clone(),
            });
        }
        let updated = PreferenceRecord { preferences };
        self.persist(Some(&updated), StoreOperation::Update).await?;
        *record = Some(updated.clone());
        Ok(updated)
    }

    /// Creates or updates the consumer record.
    pub async fn upsert(
        &self,
        preferences: StorybookPreferences,
    ) -> Result<PreferenceRecord, PreferenceStoreError> {
        let mut record = self.inner.record.lock().await;
        let updated = PreferenceRecord { preferences };
        self.persist(Some(&updated), StoreOperation::Upsert).await?;
        *record = Some(updated.clone());
        Ok(updated)
    }

    /// Deletes the consumer record and returns whether a value existed.
    pub async fn delete(&self) -> Result<bool, PreferenceStoreError> {
        let mut record = self.inner.record.lock().await;
        if record.is_none() {
            return Ok(false);
        }
        self.persist(None, StoreOperation::Delete).await?;
        *record = None;
        Ok(true)
    }

    async fn persist(
        &self,
        record: Option<&PreferenceRecord>,
        operation: StoreOperation,
    ) -> Result<(), PreferenceStoreError> {
        let Some(path) = &self.inner.path else {
            return Ok(());
        };

        let Some(record) = record else {
            return match tokio::fs::remove_file(path).await {
                Ok(()) => Ok(()),
                Err(source) if source.kind() == io::ErrorKind::NotFound => Ok(()),
                Err(source) => Err(PreferenceStoreError::Io {
                    operation,
                    path: path.clone(),
                    source,
                }),
            };
        };

        let schema_path = self
            .inner
            .schema_path
            .as_deref()
            .expect("file-backed repositories always have a schema path");
        let document =
            PreferenceDocument::new(self.inner.consumer_id.clone(), schema_path, record.clone());
        let mut bytes =
            serde_json::to_vec_pretty(&document).map_err(|source| PreferenceStoreError::Json {
                operation,
                path: path.clone(),
                source,
            })?;
        bytes.push(b'\n');
        write_atomic(path, bytes)
            .await
            .map_err(|source| PreferenceStoreError::Io {
                operation,
                path: path.clone(),
                source,
            })
    }
}

/// Resolves the project-local persistent JSON location for one Storybook consumer.
///
/// The caller supplies the Cargo workspace root or standalone package root.
/// The consumer ID becomes the filename so multiple Storybook binaries remain
/// isolated within the shared `.gpui-storybook` directory.
#[must_use]
pub fn persistent_json_path(project_root: impl AsRef<Path>, consumer_id: &ConsumerId) -> PathBuf {
    project_root
        .as_ref()
        .join(STORYBOOK_DIR)
        .join(format!("{consumer_id}.json"))
}

fn validate_json_path(path: &Path) -> Result<(), RepositoryOpenError> {
    if path.as_os_str().is_empty() || path.file_name().is_none() {
        return Err(RepositoryOpenError::InvalidJsonPath {
            path: path.to_path_buf(),
        });
    }
    Ok(())
}

fn schema_path_for(path: &Path) -> PathBuf {
    path.with_file_name(JSON_SCHEMA_FILE_NAME)
}

fn validate_distinct_schema_path(
    preference_path: &Path,
    schema_path: &Path,
) -> Result<(), RepositoryOpenError> {
    let aliases_schema = preference_path.file_name().is_some_and(|name| {
        name.to_string_lossy()
            .eq_ignore_ascii_case(JSON_SCHEMA_FILE_NAME)
    });
    if aliases_schema {
        return Err(RepositoryOpenError::PreferenceSchemaPathCollision {
            preference_path: preference_path.to_path_buf(),
            schema_path: schema_path.to_path_buf(),
        });
    }
    Ok(())
}

async fn prepare_parent(path: &Path) -> Result<(), RepositoryOpenError> {
    let Some(parent) = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    else {
        return Ok(());
    };
    tokio::fs::create_dir_all(parent)
        .await
        .map_err(|source| RepositoryOpenError::JsonIo {
            path: parent.to_path_buf(),
            source,
        })
}

async fn ensure_storybook_gitignore(json_path: &Path) -> Result<(), RepositoryOpenError> {
    let Some(parent) = json_path.parent() else {
        return Ok(());
    };
    let gitignore_path = parent.join(STORYBOOK_GITIGNORE_FILE_NAME);
    let task_path = gitignore_path.clone();
    let result = tokio::task::spawn_blocking(move || create_gitignore_if_missing(&task_path))
        .await
        .map_err(|source| RepositoryOpenError::JsonIo {
            path: gitignore_path.clone(),
            source: io::Error::other(source),
        })?;
    result.map_err(|source| RepositoryOpenError::JsonIo {
        path: gitignore_path,
        source,
    })
}

fn create_gitignore_if_missing(path: &Path) -> io::Result<()> {
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    let mut temporary = tempfile::NamedTempFile::new_in(parent)?;
    temporary.write_all(STORYBOOK_GITIGNORE_CONTENTS)?;
    temporary.as_file_mut().sync_all()?;
    match temporary.persist_noclobber(path) {
        Ok(_) => Ok(()),
        Err(error) if error.error.kind() == io::ErrorKind::AlreadyExists => Ok(()),
        Err(error) => Err(error.error),
    }
}

async fn write_schema(path: &Path) -> Result<(), RepositoryOpenError> {
    let bytes = preference_json_schema_pretty().into_bytes();
    write_atomic(path, bytes)
        .await
        .map_err(|source| RepositoryOpenError::JsonIo {
            path: path.to_path_buf(),
            source,
        })
}

async fn read_document(
    path: &Path,
    consumer_id: &ConsumerId,
) -> Result<Option<PreferenceRecord>, ReadDocumentError> {
    let bytes = tokio::fs::read(path)
        .await
        .map_err(|source| ReadDocumentError::Io { source })?;
    let document = serde_json::from_slice::<PreferenceDocument>(&bytes).map_err(|source| {
        ReadDocumentError::Invalid {
            source: InvalidJsonDocument::Decode(source),
        }
    })?;
    document
        .into_record(consumer_id)
        .map(Some)
        .map_err(|source| ReadDocumentError::Invalid { source })
}

async fn write_atomic(path: &Path, bytes: Vec<u8>) -> io::Result<()> {
    let path = path.to_path_buf();
    tokio::task::spawn_blocking(move || {
        let parent = path
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
            .unwrap_or_else(|| Path::new("."));
        let mut temporary = tempfile::NamedTempFile::new_in(parent)?;
        temporary.write_all(&bytes)?;
        temporary.as_file_mut().sync_all()?;
        temporary.persist(&path).map_err(|error| error.error)?;
        Ok(())
    })
    .await
    .map_err(io::Error::other)?
}

async fn archive_invalid_json(path: &Path, suffix: i64) -> Result<PathBuf, RepositoryOpenError> {
    let Some(file_name) = path.file_name() else {
        return Err(RepositoryOpenError::InvalidJsonPath {
            path: path.to_path_buf(),
        });
    };
    let mut archived_name = file_name.to_os_string();
    archived_name.push(format!(".corrupt-{suffix}"));
    let archived_path = path.with_file_name(archived_name);
    tokio::fs::rename(path, &archived_path)
        .await
        .map_err(|source| RepositoryOpenError::ArchiveInvalidJson {
            path: path.to_path_buf(),
            archived_path: archived_path.clone(),
            source,
        })?;
    Ok(archived_path)
}

enum ReadDocumentError {
    Io { source: io::Error },
    Invalid { source: InvalidJsonDocument },
}

#[derive(Debug, thiserror::Error)]
enum InvalidJsonDocument {
    #[error("invalid preference JSON: {0}")]
    Decode(serde_json::Error),
    #[error("preference JSON belongs to consumer '{actual}', expected '{expected}'")]
    ConsumerMismatch {
        expected: ConsumerId,
        actual: ConsumerId,
    },
}

/// Failure to open a consumer-scoped preference repository.
#[derive(Debug, thiserror::Error)]
pub enum RepositoryOpenError {
    /// A JSON override was supplied for a nonpersistent mode.
    #[error("JSON path override requires persistent mode, not {persistence:?}")]
    PathOverrideRequiresPersistent { persistence: PersistenceMode },
    /// Tokio could not join temporary-directory creation.
    #[error("failed to join temporary preference directory creation: {source}")]
    TemporaryDirectoryTask {
        #[source]
        source: tokio::task::JoinError,
    },
    /// A unique temporary directory could not be created.
    #[error("failed to create temporary preference directory: {source}")]
    TemporaryDirectory {
        #[source]
        source: io::Error,
    },
    /// The injected timestamp source failed.
    #[error(transparent)]
    Clock(#[from] PreferenceClockError),
    /// The selected JSON path had no filename.
    #[error("preference JSON path '{}' must name a file", path.display())]
    InvalidJsonPath { path: PathBuf },
    /// The preference document aliases its reserved schema sidecar.
    ///
    /// This error is returned before either path is modified.
    #[error(
        "preference JSON path '{}' aliases reserved schema path '{}'",
        preference_path.display(),
        schema_path.display()
    )]
    PreferenceSchemaPathCollision {
        /// Preference document path that would be overwritten by the schema.
        preference_path: PathBuf,
        /// Reserved generated-schema path.
        schema_path: PathBuf,
    },
    /// Archiving invalid preference JSON failed.
    #[error(
        "failed to archive invalid preference JSON '{}' as '{}': {source}",
        path.display(),
        archived_path.display()
    )]
    ArchiveInvalidJson {
        path: PathBuf,
        archived_path: PathBuf,
        #[source]
        source: io::Error,
    },
    /// Preparing, reading, or writing a JSON or schema file failed.
    #[error("preference JSON operation failed at '{}': {source}", path.display())]
    JsonIo {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
}

/// Typed storage operation name used in errors and structured tracing.
#[derive(Clone, Copy, Debug, EnumString, Eq, IntoStaticStr, PartialEq)]
#[strum(const_into_str, serialize_all = "snake_case")]
pub enum StoreOperation {
    /// Read a saved document.
    Load,
    /// Create a document.
    Create,
    /// Update a document.
    Update,
    /// Create or update a document.
    Upsert,
    /// Delete a document.
    Delete,
}

impl StoreOperation {
    /// Returns the stable diagnostic token.
    pub const fn token(self) -> &'static str {
        self.into_str()
    }
}

/// Preference CRUD, JSON, or filesystem failure.
#[derive(Debug, thiserror::Error)]
pub enum PreferenceStoreError {
    /// Create rejected an existing consumer document.
    #[error("preferences already exist for consumer '{consumer_id}'")]
    AlreadyExists { consumer_id: ConsumerId },
    /// Update rejected a missing consumer document.
    #[error("preferences do not exist for consumer '{consumer_id}'")]
    NotFound { consumer_id: ConsumerId },
    /// Serializing the typed JSON document failed.
    #[error("preference {operation:?} JSON encoding failed at '{}': {source}", path.display())]
    Json {
        operation: StoreOperation,
        path: PathBuf,
        #[source]
        source: serde_json::Error,
    },
    /// Writing or deleting the JSON document failed.
    #[error("preference {operation:?} filesystem operation failed at '{}': {source}", path.display())]
    Io {
        operation: StoreOperation,
        path: PathBuf,
        #[source]
        source: io::Error,
    },
}
