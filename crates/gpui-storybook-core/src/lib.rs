//! Runtime shell for GPUI Storybook.
//!
//! `gpui-storybook-core` owns the UI runtime used by the public facade crate:
//! window creation, the gallery layout, the optional dock workspace, story
//! container panel behavior, title-bar composition, local preference
//! resolution and persistence, embedded localization/assets, and shared
//! registry entry types.
//!
//! Most applications should start with `gpui-storybook`. Use this crate
//! directly only when you need runtime-level control over the shell.
//!
//! Important module boundaries:
//!
//! - `story`: the [`story::Story`] contract, [`story::StoryContainer`],
//!   section helpers with stable sub-story capture metadata, runtime startup,
//!   and standard window helpers
//! - `gallery`: searchable sidebar plus active-story display
//! - `dock_gallery`: feature-gated dock workspace, sidebar panel, story panel
//!   registry, and layout persistence
//! - `automation`: shared controller and command types for live story
//!   listing, story opening, screenshot capture, and the optional default
//!   automation global consumed by the base gallery and dock constructors
//! - `capture_region`: story-view and sub-story capture bounds used by MCP
//!   screenshot capture
//! - `storybook_window_ui`: customization hooks for application menu and
//!   title-bar additions
//! - `language`, `preferences`, and `i18n`: typed locale abstraction,
//!   saved/resolved runtime state, and bridge into
//!   `es-fluent`, `gpui-es-fluent`, and `gpui-component`
//! - `assets`: embedded Storybook assets plus delegated component assets
//! - `registry`: typed `inventory` entry definitions shared with the facade
//!   and macro crates, plus registration metadata copied into runtime story
//!   containers
//!
//! The preference runtime treats saved intent and resolved presentation as
//! separate state. Standard windows feed appearance and activation events into
//! resolution, independent light/dark theme slots follow the effective scheme,
//! and locale changes fan out to Storybook, the consumer adapter,
//! `CurrentLanguage`, and GPUI Component. `PersistenceStatus` reports storage
//! activity only; locale failures remain retryable diagnostics.

pub mod actions;
pub mod app_menus;
pub mod assets;
pub mod automation;
#[cfg(feature = "capture")]
mod capture_output;
pub mod capture_region;
#[cfg(feature = "dock")]
pub mod dock_gallery;
#[cfg(feature = "dock")]
mod dock_layout_store;
#[cfg(feature = "dock")]
mod dock_sidebar_index;
pub mod gallery;
pub mod i18n;
pub mod language;
mod messages;
pub mod preferences;
pub mod registry;
pub mod story;
pub mod storybook_window_ui;
pub mod title_bar;
mod window_options;
pub mod window_view;
