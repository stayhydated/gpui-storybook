use std::{path::PathBuf, sync::Arc};

use crate::*;

use super::support::*;

#[tokio::test]
async fn persistence_modes_and_json_path_contracts_are_explicit_and_host_safe() {
    let directory = tempfile::tempdir().expect("temporary directory creates");
    let project_root = directory.path().join("workspace");
    tokio::fs::create_dir(&project_root)
        .await
        .expect("project root creates");
    let standard_path = persistent_json_path(&project_root, &consumer(TEST_CONSUMER));
    assert_eq!(
        standard_path,
        project_root
            .join(".gpui-storybook")
            .join("test.storybook.json")
    );
    assert_eq!(
        standard_path.file_name().and_then(|value| value.to_str()),
        Some("test.storybook.json")
    );
    assert!(
        standard_path
            .parent()
            .expect("persistent file has a parent")
            .ends_with(".gpui-storybook")
    );

    let mut default_path_options = RepositoryOptions::persistent(consumer(TEST_CONSUMER));
    default_path_options.project_root = Some(project_root.clone());
    let persistent = PreferenceRepository::open(default_path_options)
        .await
        .expect("default persistent repository opens")
        .repository;
    assert_eq!(persistent.path(), Some(standard_path.as_path()));
    assert_eq!(
        persistent.schema_path(),
        Some(
            project_root
                .join(".gpui-storybook/preferences.schema.json")
                .as_path()
        )
    );
    let gitignore_path = project_root.join(".gpui-storybook/.gitignore");
    assert_eq!(
        tokio::fs::read_to_string(&gitignore_path)
            .await
            .expect("generated gitignore reads"),
        "*\n"
    );
    tokio::fs::write(&gitignore_path, "# consumer-owned\n")
        .await
        .expect("custom gitignore writes");
    let mut second_options = RepositoryOptions::persistent(consumer("second.storybook"));
    second_options.project_root = Some(project_root);
    let second = PreferenceRepository::open(second_options)
        .await
        .expect("second persistent repository opens")
        .repository;
    assert_eq!(second.schema_path(), persistent.schema_path());
    assert_eq!(
        tokio::fs::read_to_string(gitignore_path)
            .await
            .expect("custom gitignore remains readable"),
        "# consumer-owned\n"
    );

    let temporary =
        PreferenceRepository::open(RepositoryOptions::temporary(consumer(TEST_CONSUMER)))
            .await
            .expect("temporary JSON repository opens")
            .repository;
    let temporary_path = temporary.path().expect("temporary JSON path").to_path_buf();
    let temporary_schema = temporary
        .schema_path()
        .expect("temporary schema path")
        .to_path_buf();
    assert!(temporary_schema.exists());
    temporary
        .upsert(saved_preferences())
        .await
        .expect("temporary JSON saves");
    assert!(temporary_path.exists());
    drop(temporary);
    assert!(!temporary_path.exists());
    assert!(!temporary_schema.exists());

    let disabled = PreferenceRepository::open(RepositoryOptions::disabled(consumer(TEST_CONSUMER)))
        .await
        .expect("disabled repository opens")
        .repository;
    assert_eq!(disabled.path(), None);
    assert_eq!(disabled.schema_path(), None);
    disabled
        .upsert(saved_preferences())
        .await
        .expect("disabled mode keeps typed memory state");
    assert_eq!(
        disabled
            .load()
            .await
            .expect("disabled mode loads")
            .expect("memory state exists")
            .preferences,
        saved_preferences()
    );

    let mut invalid_options = RepositoryOptions::temporary(consumer(TEST_CONSUMER));
    invalid_options.json_path = Some(PathBuf::from("portable/preferences.json"));
    assert!(matches!(
        PreferenceRepository::open(invalid_options).await,
        Err(RepositoryOpenError::PathOverrideRequiresPersistent {
            persistence: PersistenceMode::Temporary,
        })
    ));

    let failing_directory = tempfile::tempdir().expect("temporary directory creates");
    let failing_path = failing_directory.path().join("preferences.json");
    tokio::fs::write(&failing_path, b"{}")
        .await
        .expect("invalid JSON fixture writes");
    assert!(matches!(
        PreferenceRepository::open(persistent_options(
            &failing_path,
            TEST_CONSUMER,
            Arc::new(FailingClock),
        ))
        .await,
        Err(RepositoryOpenError::Clock(
            PreferenceClockError::BeforeUnixEpoch
        ))
    ));
}

#[tokio::test]
async fn ordinary_filesystem_failures_do_not_archive_unrelated_input() {
    let directory = tempfile::tempdir().expect("temporary directory creates");
    let blocking_parent = directory.path().join("not-a-directory");
    tokio::fs::write(&blocking_parent, b"keep me")
        .await
        .expect("blocking file writes");
    let nested_json = blocking_parent.join("preferences.json");

    let error = PreferenceRepository::open(persistent_options(
        &nested_json,
        TEST_CONSUMER,
        Arc::new(FixedClock(4_242)),
    ))
    .await
    .expect_err("directory preparation fails");
    assert!(matches!(error, RepositoryOpenError::JsonIo { .. }));
    assert_eq!(
        tokio::fs::read(&blocking_parent)
            .await
            .expect("blocking file remains"),
        b"keep me"
    );
    let mut entries = tokio::fs::read_dir(directory.path())
        .await
        .expect("directory inventory opens");
    let entry = entries
        .next_entry()
        .await
        .expect("directory inventory reads")
        .expect("blocking file remains");
    assert_eq!(entry.path(), blocking_parent);
    assert!(
        entries
            .next_entry()
            .await
            .expect("directory inventory completes")
            .is_none()
    );
}
