use std::{future::Future, pin::Pin, sync::Arc, task::Poll};

use crate::*;

use super::support::*;

/// Occupy the only blocking worker so a real filesystem mutation remains queued.
/// Dropping the gate also releases it when an assertion unwinds.
struct BlockingGate {
    release: Option<std::sync::mpsc::Sender<()>>,
    task: Option<tokio::task::JoinHandle<()>>,
}

impl BlockingGate {
    async fn new() -> Self {
        let (started, ready) = tokio::sync::oneshot::channel();
        let (release, wait) = std::sync::mpsc::channel();
        let task = tokio::task::spawn_blocking(move || {
            let _ = started.send(());
            let _ = wait.recv();
        });
        ready.await.expect("blocking worker starts");
        Self {
            release: Some(release),
            task: Some(task),
        }
    }

    async fn release(mut self) {
        self.release.take().expect("gate is held").send(()).unwrap();
        self.task.take().expect("worker is owned").await.unwrap();
    }
}

impl Drop for BlockingGate {
    fn drop(&mut self) {
        self.release.take();
    }
}

async fn assert_pending<F: Future>(mut future: Pin<&mut F>) {
    std::future::poll_fn(|cx| {
        assert!(future.as_mut().poll(cx).is_pending(), "operation must wait");
        Poll::Ready(())
    })
    .await;
}

fn run_with_one_blocking_worker(future: impl Future<Output = ()>) {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .max_blocking_threads(1)
        .build()
        .expect("test runtime builds")
        .block_on(future);
}

async fn assert_disk_matches_cache(repository: &PreferenceRepository, options: RepositoryOptions) {
    let cached = repository.load().await.expect("cached record loads");
    let reopened = PreferenceRepository::open(options)
        .await
        .expect("saved document reopens")
        .repository
        .load()
        .await
        .expect("saved document loads");
    assert_eq!(
        cached, reopened,
        "disk and cache must describe the same record"
    );
}

#[test]
fn cancelled_admitted_mutations_keep_disk_and_cache_serialized() {
    run_with_one_blocking_worker(async {
        for operation in [
            StoreOperation::Create,
            StoreOperation::Update,
            StoreOperation::Upsert,
            StoreOperation::Delete,
        ] {
            let directory = tempfile::tempdir().expect("test directory creates");
            let path = directory.path().join("preferences.json");
            let options = persistent_options(&path, TEST_CONSUMER, Arc::new(FixedClock(20)));
            let repository = PreferenceRepository::open(options.clone())
                .await
                .expect("repository opens")
                .repository;
            if matches!(operation, StoreOperation::Update | StoreOperation::Delete) {
                repository.create(saved_preferences()).await.unwrap();
            }
            let mut next = saved_preferences();
            next.color_scheme = PreferredColorScheme::Dark;
            let gate = BlockingGate::new().await;
            let mut mutation = Box::pin(async {
                match operation {
                    StoreOperation::Create => repository.create(next.clone()).await.map(|_| ()),
                    StoreOperation::Update => repository.update(next.clone()).await.map(|_| ()),
                    StoreOperation::Upsert => repository.upsert(next.clone()).await.map(|_| ()),
                    StoreOperation::Delete => repository.delete().await.map(|_| ()),
                    StoreOperation::Load => unreachable!("test only mutates records"),
                }
            });
            assert_pending(mutation.as_mut()).await;
            drop(mutation);

            // Cancellation must not let readers or a subsequent write overtake
            // the admitted filesystem mutation while its worker is queued.
            let mut load = Box::pin(repository.load());
            assert_pending(load.as_mut()).await;
            let mut second = Box::pin(repository.upsert(saved_preferences()));
            assert_pending(second.as_mut()).await;
            drop(second);
            gate.release().await;

            let actual = load.await.expect("committed record loads");
            let expected = (operation != StoreOperation::Delete)
                .then_some(PreferenceRecord { preferences: next });
            assert_eq!(actual, expected, "cancelled {operation:?} still commits");
            assert_disk_matches_cache(&repository, options.clone()).await;

            let following = repository.upsert(saved_preferences()).await.unwrap();
            assert_eq!(following.preferences, saved_preferences());
            assert_disk_matches_cache(&repository, options).await;
        }
    });
}

#[test]
fn cancelled_failed_commit_releases_admission_without_changing_cache() {
    run_with_one_blocking_worker(async {
        let directory = tempfile::tempdir().expect("test directory creates");
        let path = directory.path().join("preferences.json");
        let options = persistent_options(&path, TEST_CONSUMER, Arc::new(FixedClock(20)));
        let repository = PreferenceRepository::open(options.clone())
            .await
            .unwrap()
            .repository;
        let original = repository.create(saved_preferences()).await.unwrap();
        let gate = BlockingGate::new().await;
        let mut next = saved_preferences();
        next.color_scheme = PreferredColorScheme::Dark;
        let mut mutation = Box::pin(repository.upsert(next));
        assert_pending(mutation.as_mut()).await;
        drop(mutation);

        // A directory at the destination makes the queued atomic rename fail
        // on every host, without permission/root-user assumptions.
        std::fs::remove_file(&path).unwrap();
        std::fs::create_dir(&path).unwrap();
        let mut load = Box::pin(repository.load());
        assert_pending(load.as_mut()).await;
        gate.release().await;
        assert_eq!(load.await.unwrap(), Some(original));
        assert!(path.is_dir());
        std::fs::remove_dir(&path).unwrap();
        repository.upsert(saved_preferences()).await.unwrap();
        assert_disk_matches_cache(&repository, options).await;
    });
}
