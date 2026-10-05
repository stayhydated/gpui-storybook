//! Track admitted host tasks across dropped response futures.
use crate::unavailable;
use gpui_storybook_automation::StorybookAutomationError;
use std::sync::{Arc, Mutex};
use tokio_util::task::TaskTracker;

pub(crate) struct OwnedTasks {
    runtime: tokio::runtime::Handle,
    closing: Mutex<bool>,
    tracker: TaskTracker,
}
impl OwnedTasks {
    pub(crate) fn current() -> Arc<Self> {
        Arc::new(Self {
            runtime: tokio::runtime::Handle::current(),
            closing: Mutex::new(false),
            tracker: TaskTracker::new(),
        })
    }
    pub(crate) fn spawn(
        &self,
        operation: impl Future<Output = ()> + Send + 'static,
    ) -> Result<(), StorybookAutomationError> {
        let closing = self.closing.lock().expect("owned task admission");
        if *closing {
            return Err(unavailable("host task admission is closed"));
        }
        // TaskTracker::close allows further spawning. Serialize its registration
        // with our admission guard so shutdown cannot observe an empty tracker
        // before a previously admitted task has been registered.
        self.tracker.spawn_on(operation, &self.runtime);
        Ok(())
    }
    pub(crate) async fn settle(&self) {
        {
            let mut closing = self.closing.lock().expect("owned task admission");
            *closing = true;
            self.tracker.close();
        }
        self.tracker.wait().await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn shutdown_drains_work_and_permanently_closes_admission() {
        let tasks = OwnedTasks::current();
        let (release, released) = tokio::sync::oneshot::channel();
        let (completed, completion) = tokio::sync::oneshot::channel();
        tasks
            .spawn(async move {
                released.await.unwrap();
                completed.send(()).unwrap();
            })
            .unwrap();
        let waiting = tasks.clone();
        let shutdown = tokio::spawn(async move { waiting.settle().await });
        // Wait for the observable closed admission rather than a scheduling delay.
        while !tasks.tracker.is_closed() {
            tokio::task::yield_now().await;
        }
        assert!(tasks.spawn(async {}).is_err());
        assert!(!shutdown.is_finished());
        release.send(()).unwrap();
        shutdown.await.unwrap();
        completion.await.unwrap();
        assert!(tasks.spawn(async {}).is_err());
        tasks.settle().await;
    }
}
