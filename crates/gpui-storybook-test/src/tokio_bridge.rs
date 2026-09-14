//! Tokio runtime shared by the stories in one headless app context.

use gpui_kit::{App, Global};
use std::io;
use tokio::runtime::{Builder, Handle, Runtime};

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

/// Access to the Tokio runtime installed for a headless story app.
pub struct Tokio;

impl Tokio {
    /// Returns the runtime handle for work started by a story init hook.
    pub fn handle(app: &App) -> Handle {
        app.global::<GlobalTokio>().handle.clone()
    }
}
