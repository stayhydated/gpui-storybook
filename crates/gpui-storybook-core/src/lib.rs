//! Runtime shell for GPUI Storybook.
//!
//! `gpui-storybook-core` owns the UI runtime used by the public facade crate:
//! window creation, the gallery layout, the optional dock workspace, story
//! container panel behavior, title-bar composition, theme persistence, locale
//! switching, embedded assets, and shared registry entry types.
//!
//! Most applications should start with `gpui-storybook`. Use this crate
//! directly only when you need runtime-level control over the shell.
//!
//! Important module boundaries:
//!
//! - `story`: the [`story::Story`] contract, [`story::StoryContainer`],
//!   section helpers, runtime startup, and standard window helpers
//! - `gallery`: searchable sidebar plus active-story display
//! - `dock_gallery`: feature-gated dock workspace, sidebar panel, story panel
//!   registry, and layout persistence
//! - `automation`: shared controller and command types for live story
//!   listing, story opening, screenshot capture, and the optional default
//!   automation global consumed by the base gallery and dock constructors
//! - `storybook_window_ui`: customization hooks for application menu and
//!   title-bar additions
//! - `language`, `locale`, and `i18n`: locale abstraction and bridge into
//!   `es-fluent`, `gpui-es-fluent`, and `gpui-component`
//! - `assets`: embedded Storybook assets plus delegated component assets
//! - `registry`: typed `inventory` entry definitions shared with the facade
//!   and macro crates

pub mod actions;
pub mod app_menus;
pub mod assets;
pub mod automation;
#[cfg(feature = "dock")]
pub mod dock_gallery;
pub mod gallery;
pub mod i18n;
pub mod language;
pub mod locale;
pub mod registry;
pub mod story;
pub mod storybook_window_ui;
pub mod title_bar;
mod window_options;
pub mod window_view;
