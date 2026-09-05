use std::sync::Arc;

use crate::*;

use super::support::*;

#[tokio::test]
async fn concurrent_writes_leave_one_complete_typed_document() {
    let directory = tempfile::tempdir().expect("temporary directory creates");
    let path = directory.path().join("preferences.json");
    let repository = PreferenceRepository::open(persistent_options(
        &path,
        TEST_CONSUMER,
        Arc::new(FixedClock(20)),
    ))
    .await
    .expect("JSON repository opens")
    .repository;

    let first = {
        let repository = repository.clone();
        let mut preferences = saved_preferences();
        preferences.color_scheme = PreferredColorScheme::Light;
        tokio::spawn(async move { repository.upsert(preferences).await })
    };
    let second = {
        let repository = repository.clone();
        let mut preferences = saved_preferences();
        preferences.color_scheme = PreferredColorScheme::Dark;
        tokio::spawn(async move { repository.upsert(preferences).await })
    };
    first
        .await
        .expect("first task joins")
        .expect("first write succeeds");
    second
        .await
        .expect("second task joins")
        .expect("second write succeeds");

    let bytes = tokio::fs::read(&path)
        .await
        .expect("final JSON document reads");
    let value: serde_json::Value =
        serde_json::from_slice(&bytes).expect("final JSON document is complete");
    assert_eq!(value["consumer_id"], TEST_CONSUMER);
    let final_preferences = PreferenceRepository::open(persistent_options(
        &path,
        TEST_CONSUMER,
        Arc::new(FixedClock(30)),
    ))
    .await
    .expect("final JSON document reopens")
    .repository
    .load()
    .await
    .expect("final document loads")
    .expect("final record exists")
    .preferences;
    assert!(matches!(
        final_preferences.color_scheme,
        PreferredColorScheme::Light | PreferredColorScheme::Dark
    ));
}

#[gpui_kit::test]
async fn gpui_tokio_runs_json_repository_work_without_blocking_the_foreground(
    cx: &mut gpui_kit::TestAppContext,
) {
    cx.executor().allow_parking();

    let directory = tempfile::tempdir().expect("temporary directory creates");
    let path = directory.path().join("gpui-tokio-preferences.json");
    let options = persistent_options(&path, TEST_CONSUMER, Arc::new(FixedClock(8_000)));
    let expected = saved_preferences();
    let expected_for_task = expected.clone();
    let (start_sender, start_receiver) = tokio::sync::oneshot::channel();
    cx.update(gpui_tokio::init);

    let storage_task = cx.update(|cx| {
        gpui_tokio::Tokio::spawn(cx, async move {
            start_receiver
                .await
                .expect("GPUI foreground releases repository task");
            let repository = PreferenceRepository::open(options)
                .await
                .expect("repository opens on GPUI's Tokio runtime")
                .repository;
            repository
                .upsert(expected_for_task)
                .await
                .expect("repository saves on GPUI's Tokio runtime");
            repository
                .load()
                .await
                .expect("repository loads on GPUI's Tokio runtime")
                .expect("saved document exists")
        })
    });

    start_sender
        .send(())
        .expect("Tokio spawn returned control to the GPUI foreground");
    let record = storage_task.await.expect("Tokio repository task joins");
    assert_eq!(record.preferences, expected);
}
