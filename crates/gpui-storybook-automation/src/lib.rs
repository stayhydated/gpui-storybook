//! Target-neutral Storybook automation contracts and backend integration.
//!
//! Hosts own dispatch, readiness, and exclusive operations. Clients never replay
//! submitted mutations after a timeout or disconnect. Platform input, rendering,
//! and application construction belong to backend implementations.

mod backend;
mod controls;
mod interaction;
mod presentation;
mod scenario;
mod semantic;
mod snapshot;
mod validation;
pub mod wire;

pub use backend::*;
pub use controls::*;
pub use interaction::*;
pub use presentation::*;
pub use scenario::*;
pub use semantic::*;
pub use snapshot::*;
pub use validation::*;

pub const DEFAULT_STORY_CAPTURE_WIDTH: u32 = 1280;
pub const DEFAULT_STORY_CAPTURE_HEIGHT: u32 = 720;

#[cfg(test)]
mod tests;
