//! GPUI-owned Tokio runtime bridge used by the Storybook runtime.

use std::future::Future;

use gpui_kit::{App, AppContext, Global, ReadGlobal as _, Task};
use tokio::{
    runtime::{Builder, Handle, Runtime},
    task::{AbortHandle, JoinError},
};

struct TokioRuntime {
    runtime: Option<Runtime>,
    handle: Handle,
}

impl Global for TokioRuntime {}

impl Drop for TokioRuntime {
    fn drop(&mut self) {
        if let Some(runtime) = self.runtime.take() {
            runtime.shutdown_background();
        }
    }
}

struct AbortOnDrop(Option<AbortHandle>);

impl AbortOnDrop {
    fn disarm(&mut self) {
        self.0.take();
    }
}

impl Drop for AbortOnDrop {
    fn drop(&mut self) {
        if let Some(handle) = self.0.take() {
            handle.abort();
        }
    }
}

/// Installs the Tokio runtime used for Storybook storage work.
pub fn init(cx: &mut App) {
    let runtime = Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
        .expect("failed to initialize the Storybook Tokio runtime");
    let handle = runtime.handle().clone();
    cx.set_global(TokioRuntime {
        runtime: Some(runtime),
        handle,
    });
}

/// Spawns work on Tokio and returns its result through a GPUI task.
///
/// Dropping the returned GPUI task cancels the Tokio task.
pub fn spawn<C, F, R>(cx: &C, future: F) -> Task<Result<R, JoinError>>
where
    C: AppContext,
    F: Future<Output = R> + Send + 'static,
    R: Send + 'static,
{
    cx.read_global(|runtime: &TokioRuntime, cx| {
        let join = runtime.handle.spawn(future);
        let mut abort = AbortOnDrop(Some(join.abort_handle()));
        cx.background_spawn(async move {
            let result = join.await;
            abort.disarm();
            result
        })
    })
}

/// Returns a handle to the Storybook Tokio runtime.
pub fn handle(cx: &App) -> Handle {
    TokioRuntime::global(cx).handle.clone()
}
