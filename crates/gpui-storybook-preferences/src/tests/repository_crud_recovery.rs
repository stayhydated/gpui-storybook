use std::{path::Path, sync::Arc};

use crate::*;

use super::support::*;

#[tokio::test]
async fn json_repository_supports_typed_crud_reopen_and_generated_schema() {
    let directory = tempfile::tempdir().expect("temporary directory creates");
    let path = directory.path().join("test.storybook.json");
    let clock: Arc<dyn PreferenceClock> = Arc::new(FixedClock(1_000));
    let options = persistent_options(&path, TEST_CONSUMER, clock);

    let opened = PreferenceRepository::open(options.clone())
        .await
        .expect("JSON repository opens");
    assert!(opened.recovery.is_none());
    let repository = opened.repository;
    assert_eq!(repository.persistence(), PersistenceMode::Persistent);
    assert_eq!(repository.path(), Some(path.as_path()));
    let schema_path = directory.path().join("preferences.schema.json");
    assert_eq!(repository.schema_path(), Some(schema_path.as_path()));
    assert_eq!(
        tokio::fs::read_to_string(&schema_path)
            .await
            .expect("generated schema reads"),
        crate::preference_json_schema_pretty()
    );
    assert_eq!(
        repository.load().await.expect("empty repository loads"),
        None
    );

    let created = repository
        .create(saved_preferences())
        .await
        .expect("typed preferences create");
    assert_eq!(created.preferences, saved_preferences());
    assert!(matches!(
        repository.create(saved_preferences()).await,
        Err(PreferenceStoreError::AlreadyExists { .. })
    ));

    let document: serde_json::Value =
        serde_json::from_slice(&tokio::fs::read(&path).await.expect("preference JSON reads"))
            .expect("preference JSON parses");
    assert_eq!(document["$schema"], "preferences.schema.json");
    assert_eq!(document["consumer_id"], TEST_CONSUMER);
    assert_eq!(document.get("format_version"), None);
    assert_eq!(document.get("created_at_millis"), None);
    assert_eq!(document.get("updated_at_millis"), None);
    assert_eq!(document["preferences"]["window_mode"], "dock");
    assert_eq!(document["preferences"]["color_scheme"], "system");
    assert_eq!(document["preferences"]["language"]["mode"], "explicit");
    assert_eq!(document["preferences"]["language"]["tag"], "fr");

    let mut changed = saved_preferences();
    changed.color_scheme = PreferredColorScheme::Dark;
    let updated = repository
        .update(changed.clone())
        .await
        .expect("typed preferences update");
    assert_eq!(updated.preferences, changed);

    let reopened = PreferenceRepository::open(options)
        .await
        .expect("JSON repository reopens")
        .repository;
    assert_eq!(
        reopened
            .load()
            .await
            .expect("reopened JSON loads")
            .expect("saved document exists")
            .preferences,
        changed
    );
    assert!(reopened.delete().await.expect("saved JSON deletes"));
    assert!(!path.exists());
    assert!(schema_path.exists());
    assert!(!reopened.delete().await.expect("missing JSON stays deleted"));
}

#[tokio::test]
async fn invalid_json_is_archived_byte_for_byte_and_defaults_remain_available() {
    let directory = tempfile::tempdir().expect("temporary directory creates");
    let path = directory.path().join("preferences.json");
    let invalid = br#"{"consumer_id":"test.storybook","unexpected":true}"#;
    tokio::fs::write(&path, invalid)
        .await
        .expect("invalid JSON fixture writes");

    let opened = PreferenceRepository::open(persistent_options(
        &path,
        TEST_CONSUMER,
        Arc::new(FixedClock(7_654_321)),
    ))
    .await
    .expect("invalid JSON recovers");
    let diagnostic = opened.recovery.expect("recovery is reported");
    let archived_path = directory.path().join("preferences.json.corrupt-7654321");
    assert_eq!(diagnostic.json_path, path);
    assert_eq!(diagnostic.archived_path, archived_path);
    assert_eq!(diagnostic.reason, RecoveryReason::InvalidJson);
    assert_eq!(diagnostic.reason.token(), "invalid_json");
    assert_eq!(
        tokio::fs::read(&diagnostic.archived_path)
            .await
            .expect("archived bytes read"),
        invalid
    );
    assert_eq!(
        opened
            .repository
            .load()
            .await
            .expect("recovered repository loads"),
        None
    );
    assert!(opened.repository.schema_path().is_some_and(Path::exists));
}

#[tokio::test]
async fn a_document_for_another_consumer_is_recovered_as_invalid_json() {
    let directory = tempfile::tempdir().expect("temporary directory creates");
    let path = directory.path().join("preferences.json");
    let first = PreferenceRepository::open(persistent_options(
        &path,
        "first.storybook",
        Arc::new(FixedClock(10)),
    ))
    .await
    .expect("first repository opens")
    .repository;
    first
        .upsert(saved_preferences())
        .await
        .expect("first consumer saves");
    drop(first);

    let second = PreferenceRepository::open(persistent_options(
        &path,
        "second.storybook",
        Arc::new(FixedClock(11)),
    ))
    .await
    .expect("consumer mismatch recovers");
    assert_eq!(
        second.recovery.as_ref().map(|value| value.reason),
        Some(RecoveryReason::InvalidJson)
    );
    assert_eq!(
        second
            .repository
            .load()
            .await
            .expect("second repository loads"),
        None
    );
}
