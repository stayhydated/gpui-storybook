//! Runtime shell for GPUI Storybook.

//! `gpui-storybook-core` owns the UI runtime used by the public facade crate:
//! window creation, runtime-selectable gallery and dock layouts, story
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
//!   duplicate-title variant groups whose members remain concrete previews,
//!   section helpers with stable sub-story capture metadata, runtime startup,
//!   and standard window helpers
//! - `gallery`: searchable sidebar plus active-story display
//! - `controls`, `workbench`, and `presentation`: typed live field editing,
//!   per-window selection and preview state, theme editing, story source
//!   details, focus-scoped actions and key bindings, optional GPUI profiler
//!   telemetry and Inspector integration, grouped-story selection, viewport
//!   presets, and canvas settings
//! - `theme_workbench`: deterministic session drafts layered over the selected
//!   base theme, including token rebuilding and external-reload rebasing
//! - `dock_gallery`: dock workspace, sidebar panel, story panel
//!   registry, and layout persistence
//! - `automation`: shared controller and command types for live story
//!   listing, story opening, control reads/edits/resets, serialized capture
//!   controls, screenshot capture, runtime action discovery, route-local
//!   structured value reads, closed in-process interaction batches, and the
//!   optional default automation global consumed by the base gallery and dock
//!   constructors. Gallery and dock use one
//!   frame-aware executor and one exclusive mutation/capture guard.
//! - `capture_region`: story-view and sub-story capture bounds, semantic
//!   interaction targets, and structured values used by automation
//! - `storybook_window_ui`: customization hooks for application menu and
//!   title-bar additions
//! - `language`, `preferences`, and `i18n`: typed locale abstraction,
//!   saved/resolved runtime state, and bridge into
//!   `es-fluent`, `gpui-es-fluent`, and `gpui-component`
//! - `assets`: embedded Storybook assets plus delegated component assets
//! - `registry`: typed `inventory` entry definitions shared with the facade
//!   and macro crates, plus registration metadata copied into runtime story
//!   containers
//! - `catalog`: static story registration autodocs and deterministic JSON export
//!   for tooling that must inspect stories without constructing a GPUI window
//!
//! The preference runtime treats saved intent and resolved presentation as
//! separate state. Standard windows feed appearance and activation events into
//! resolution, selecting a named theme activates its matching appearance while
//! preserving the opposite light/dark slot, and locale changes fan out to
//! Storybook, the consumer adapter, `CurrentLanguage`, and GPUI Component.
//! `PersistenceStatus` reports storage activity only; locale failures remain
//! retryable diagnostics.

extern crate gpui_kit as gpui;

pub mod actions;
pub mod app_menus;
pub mod assets;
pub mod automation;
#[cfg(feature = "capture")]
mod capture_output;
pub mod capture_region;
pub mod catalog;
pub mod controls;
pub mod dock_gallery;
mod dock_layout_store;
mod dock_sidebar_index;
pub mod gallery;
pub mod i18n;
pub mod language;
mod messages;
pub mod preferences;
pub mod presentation;
pub mod registry;
pub mod story;
#[cfg(feature = "inspector")]
pub mod story_inspector;
pub mod storybook_window_ui;
pub mod theme_workbench;
pub mod title_bar;
mod web_fonts;
mod window_options;
pub mod workbench;
