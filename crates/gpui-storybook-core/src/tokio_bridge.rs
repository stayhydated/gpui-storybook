//! Tokio runtime shared by one Storybook GPUI app context.

use gpui_kit::{App, AppContext, Global, Task};
use std::{future::Future, io};
use tokio::runtime::{Builder, Handle, Runtime};
use tokio::task::JoinError;

struct AbortOnDrop(tokio::task::AbortHandle);

impl Drop for AbortOnDrop {
    fn drop(&mut self) {
        self.0.abort();
    }
}

struct GlobalTokio {
    runtime: Option<Runtime>,
    handle: Handle,
}

impl Global for GlobalTokio {}

impl Drop for GlobalTokio {
    fn drop(&mut self) {
        if let Some(runtime) = self.runtime.take() {
            runtime.shutdown_background();
        }
    }
}

/// Installs a two-worker Tokio runtime in the current GPUI app.
pub fn init(app: &mut App) -> io::Result<()> {
    let runtime = Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()?;
    let handle = runtime.handle().clone();
    app.set_global(GlobalTokio {
        runtime: Some(runtime),
        handle,
    });
    Ok(())
}

/// Uses an existing Tokio runtime in the current GPUI app.
///
/// The caller owns the runtime and must keep it alive for the app's lifetime.
pub fn init_from_handle(app: &mut App, handle: Handle) {
    app.set_global(GlobalTokio {
        runtime: None,
        handle,
    });
}

/// Access to the Tokio runtime installed for a Storybook app.
pub struct Tokio;

impl Tokio {
    /// Spawns work on Tokio and cancels it if the returned GPUI task is dropped.
    pub fn spawn<C, F, R>(app: &C, future: F) -> Task<Result<R, JoinError>>
    where
        C: AppContext,
        F: Future<Output = R> + Send + 'static,
        R: Send + 'static,
    {
        app.read_global(|runtime: &GlobalTokio, app| {
            let task = runtime.handle.spawn(future);
            let cancel = AbortOnDrop(task.abort_handle());
            app.background_spawn(async move {
                let result = task.await;
                drop(cancel);
                result
            })
        })
    }

    /// Returns the runtime handle for work started by a story init hook.
    pub fn handle(app: &App) -> Handle {
        app.global::<GlobalTokio>().handle.clone()
    }
}
