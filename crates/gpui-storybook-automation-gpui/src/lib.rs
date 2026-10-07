//! Automation instrumentation for application-owned GPUI windows.
//! Each app and window owns its rendered registry. Surface replacement must call
//! [`regions::invalidate_window_regions`] before attaching its replacement.
pub mod regions;

pub mod interaction;
pub mod snapshot;

pub mod attachment;
pub use attachment::{AttachedInteraction, AttachmentError, EmbeddedRoot, GpuiHostAttachment};
mod route;
pub use route::EmbeddedRoute;

#[cfg(feature = "device")]
pub mod device;

#[cfg(all(feature = "android", target_os = "android"))]
pub mod android;
