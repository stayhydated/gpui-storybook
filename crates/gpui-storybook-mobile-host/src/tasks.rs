//! Track admitted host tasks across dropped response futures.
use crate::unavailable;
use gpui_storybook_automation::StorybookAutomationError;
use std::sync::{Arc, Mutex};
use tokio::sync::Notify;

pub(crate) struct OwnedTasks {
    runtime: tokio::runtime::Handle,
    state: Mutex<State>,
    settled: Notify,
}
#[derive(Default)]
struct State {
    closing: bool,
    active: usize,
}
pub(crate) struct OwnedTask(Arc<OwnedTasks>);
impl Drop for OwnedTask {
    fn drop(&mut self) {
        self.0.state.lock().expect("owned task state").active -= 1;
        self.0.settled.notify_waiters();
    }
}
impl OwnedTasks {
    pub(crate) fn current() -> Arc<Self> {
        Arc::new(Self {
            runtime: tokio::runtime::Handle::current(),
            state: Mutex::new(State::default()),
            settled: Notify::new(),
        })
    }
    pub(crate) fn admit(self: &Arc<Self>) -> Result<OwnedTask, StorybookAutomationError> {
        let mut state = self.state.lock().expect("owned task state");
        if state.closing {
            return Err(unavailable("host task admission is closed"));
        }
        state.active += 1;
        Ok(OwnedTask(self.clone()))
    }
    pub(crate) fn spawn(&self, operation: impl Future<Output = ()> + Send + 'static) {
        self.runtime.spawn(operation);
    }
    pub(crate) async fn settle(&self) {
        self.state.lock().expect("owned task state").closing = true;
        loop {
            let notified = self.settled.notified();
            tokio::pin!(notified);
            notified.as_mut().enable();
            if self.state.lock().expect("owned task state").active == 0 {
                break;
            }
            notified.await;
        }
    }
}
