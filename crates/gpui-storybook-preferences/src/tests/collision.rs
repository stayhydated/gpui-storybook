use std::sync::Arc;

use crate::*;

use super::support::*;

#[tokio::test]
async fn explicit_schema_path_collision_preserves_the_preference_file() {
    let directory = tempfile::tempdir().expect("temporary directory creates");
    let path = directory.path().join("preferences.schema.json");
    let original = br#"{"consumer_id":"test.storybook","keep":"these bytes"}"#;
    tokio::fs::write(&path, original)
        .await
        .expect("preference fixture writes");

    let error = PreferenceRepository::open(persistent_options(
        &path,
        TEST_CONSUMER,
        Arc::new(FixedClock(1_234)),
    ))
    .await
    .expect_err("schema path collision is rejected");
    match error {
        RepositoryOpenError::PreferenceSchemaPathCollision {
            preference_path,
            schema_path,
        } => {
            assert_eq!(preference_path, path);
            assert_eq!(schema_path, path);
        },
        other => panic!("expected schema path collision, got {other:?}"),
    }
    assert_eq!(
        tokio::fs::read(&path)
            .await
            .expect("original preference bytes remain readable"),
        original
    );
    let mut entries = tokio::fs::read_dir(directory.path())
        .await
        .expect("directory inventory opens");
    assert_eq!(
        entries
            .next_entry()
            .await
            .expect("directory inventory reads")
            .map(|entry| entry.path()),
        Some(path)
    );
    assert!(
        entries
            .next_entry()
            .await
            .expect("directory inventory completes")
            .is_none()
    );
}

#[tokio::test]
async fn default_schema_path_collision_precedes_schema_write_and_recovery() {
    let directory = tempfile::tempdir().expect("temporary directory creates");
    let storybook_directory = directory.path().join(".gpui-storybook");
    tokio::fs::create_dir(&storybook_directory)
        .await
        .expect("Storybook directory creates");
    let path = storybook_directory.join("preferences.schema.json");
    let original = b"invalid preference bytes";
    tokio::fs::write(&path, original)
        .await
        .expect("preference fixture writes");
    let mut options = RepositoryOptions::persistent(consumer("preferences.schema"));
    options.project_root = Some(directory.path().to_path_buf());
    options.clock = Arc::new(FixedClock(9_876));

    let error = PreferenceRepository::open(options)
        .await
        .expect_err("default schema path collision is rejected");
    assert!(matches!(
        error,
        RepositoryOpenError::PreferenceSchemaPathCollision {
            preference_path,
            schema_path,
        } if preference_path == path && schema_path == path
    ));
    assert_eq!(
        tokio::fs::read(&path)
            .await
            .expect("original preference bytes remain readable"),
        original
    );
    assert!(!storybook_directory.join(".gitignore").exists());
    let mut entries = tokio::fs::read_dir(&storybook_directory)
        .await
        .expect("Storybook directory inventory opens");
    assert_eq!(
        entries
            .next_entry()
            .await
            .expect("Storybook directory inventory reads")
            .map(|entry| entry.path()),
        Some(path)
    );
    assert!(
        entries
            .next_entry()
            .await
            .expect("Storybook directory inventory completes")
            .is_none()
    );
}

#[tokio::test]
async fn schema_path_collision_check_is_case_insensitive() {
    let directory = tempfile::tempdir().expect("temporary directory creates");
    let preference_path = directory.path().join("Preferences.Schema.JSON");
    let schema_path = directory.path().join("preferences.schema.json");

    let error = PreferenceRepository::open(persistent_options(
        &preference_path,
        TEST_CONSUMER,
        Arc::new(FixedClock(5_678)),
    ))
    .await
    .expect_err("case-only schema path collision is rejected");
    assert!(matches!(
        error,
        RepositoryOpenError::PreferenceSchemaPathCollision {
            preference_path: actual_preference_path,
            schema_path: actual_schema_path,
        } if actual_preference_path == preference_path && actual_schema_path == schema_path
    ));
    assert!(!preference_path.exists());
    assert!(!schema_path.exists());
}
