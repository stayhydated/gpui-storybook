//! Public facade for building GPUI storybook binaries.
//!
//! Most applications should depend on this crate rather than the lower-level
//! runtime, macro, or TOML crates. It re-exports the standard runtime shell,
//! typed controls and workbench state, story traits, locale helpers, the
//! runtime-selectable Gallery/Dock window, and, with the default `macros`
//! feature, the story registration macros.
//! The Inspect workbench tab always shows the active story key and source.
//! Enable the opt-in `inspector` feature for its GPUI Component Inspector
//! button and `StoryInspectorState` story-root metadata.
//!
//! Story registration flows through `inventory`: `#[story]` and
//! `#[derive(ComponentStory)]` submit story entries, `#[derive(Substory)]`
//! derives stable capture keys for styled `section` or custom
//! `StorySectionBase` regions inside a story, and
//! `#[story_init]` submits one-time setup hooks. The hidden `__registry` and
//! `__inventory` re-exports are the stable expansion path used by those
//! macros.
//!
//! `#[derive(StoryControls)]` and field-level `#[storybook(control...)]`
//! metadata connect an exact `Entity<S>` to the Controls tab and MCP
//! automation. Only marked fields are registered. `#[derive(ComponentStory)]`
//! accepts the same field metadata and derives reset defaults from its
//! configured example.
//! Explicit stories declare reusable workflows with [`Story::scenarios`]; a
//! component derive accepts `scenarios = ...`. Workbench and automation runs
//! recreate the concrete story before applying scenario controls,
//! presentation, named steps, exact postconditions, and optional capture.
//! [`StorybookElementExt`] associates route-local automation targets and JSON
//! state with rendered children. MCP clients can read semantic values after a
//! fresh frame without requiring a screenshot; capture remains the
//! visual-presentation assertion surface.
//!
//! [`init`] and `generate_stories` load crate-local `storybook.toml` files for
//! discovered story crates and select a runtime config by matching the running
//! binary name against registered story crate names. Initialization applies
//! its initial `window_mode` and launch-only preference overrides; story
//! generation applies `allow` and `disable_story` filtering, then materializes
//! sorted [`StoryContainer`] values. A story crate config's `group` becomes the
//! sidebar's outer group; a story's declared section remains the nested label.
//!
//! Macro-generated stories carry stable [`StoryKey`] values in the form
//! `{crate-package-name}-{registered-story-name}`. These keys are copied into
//! generated [`StoryContainer`] values as typed [`RegisteredStoryMetadata`] for
//! automation and capture routes.
//!
//! [`static_story_catalog`] and [`static_story_catalog_json`] expose the
//! registration catalog without opening a window or constructing live stories.
//! The static records include Rustdoc and control shape metadata; localized
//! titles, descriptions, and control defaults remain runtime values.
//!
//! Feature boundaries:
//!
//! - `macros`: re-exports proc macros from `gpui-storybook-macros`
//! - `inspector`: adds GPUI Inspector activation and story-root metadata; the
//!   Inspect tab's story key and source remain part of the base workbench
//! - `mcp`: serves the live controller installed by [`init`] over MCP and
//!   re-exports automation and capture helpers. Generic remote input tools are
//!   advertised only when
//!   `GPUI_STORYBOOK_MCP_ALLOW_INTERACTION=1`; typed controls remain available
//!   without that capability. This feature is supported on Linux and macOS;
//!   enabling it on Windows or another target produces a compile-time error.
//! - `performance`: enables GPUI window profiler histograms and the debug frame
//!   overlay in the Perf workbench tab
//!
//! Applications with embedded locale assets should call
//! `es_fluent_build::track_i18n_assets()` from `build.rs`. Define the embedded
//! i18n module and typed language enum in library-reachable code, then pass
//! typed [`StorybookOptions`] to [`init`] and await readiness before creating a
//! story window.
//!
//! [`PreferenceState::saved`] retains durable user intent, including the
//! Gallery/Dock window mode, `System` choices, and independent light/dark theme
//! slots. Choosing a named theme also activates its matching appearance while
//! preserving the opposite slot.
//! [`PreferenceState::resolved`] reports effective values and their sources
//! after live system detection, registry fallback, and deterministic overrides.
//! [`PersistenceStatus`] is storage-only; locale-adapter failures are reported
//! as diagnostics and are retried on later window activation without falsifying
//! storage state.

#[cfg(all(feature = "mcp", not(any(target_os = "linux", target_os = "macos"))))]
compile_error!(
    "the `gpui-storybook/mcp` feature supports Linux and macOS; Windows and other targets are unsupported"
);

#[cfg(feature = "macros")]
pub use gpui_storybook_macros::*;

use std::{
    collections::{BTreeMap, HashMap},
    fmt,
    path::PathBuf,
};

#[cfg(not(target_family = "wasm"))]
use std::path::Path;

pub mod preferences;
pub use preferences::{
    ColorSchemeResolution, ColorSchemeSource, ConsumerId, ConsumerIdError, LanguageResolution,
    LanguageSource, LanguageTag, LocaleApplicationError, PersistenceMode, PersistenceStatus,
    PreferenceDiagnostic, PreferenceOverrides, PreferenceState, PreferredColorScheme,
    PreferredLanguage, PreferredLanguageMode, PreferredScrollbar, RecoveryDiagnostic,
    RecoveryReason, ResolutionDiagnostic, ResolvedPreferences, StorybookInitError,
    StorybookOptions, StorybookPreferences, StorybookReady, StorybookWindowMode, SystemColorScheme,
    ThemeId, ThemeIdError, ThemeResolution, ThemeSource, UnsupportedValueSource,
};

pub use gpui_es_fluent::try_localize_message as localize_message;
pub use gpui_storybook_core::catalog::{
    StaticControlKind, StaticControlSpec, StoryCatalog, StoryCatalogEntry, StoryCatalogExportError,
    StoryCatalogSource, export_static_catalog_json, export_static_catalog_json_pretty,
    static_story_catalog, static_story_catalog_json, static_story_catalog_json_pretty,
    write_static_catalog_json, write_static_catalog_json_pretty,
};
pub use gpui_storybook_core::dock_gallery::{StoryWorkspace, register_story_panels};
pub use gpui_storybook_core::registry::{
    RegisteredStoryMetadata, StoryAutodoc, StoryKey, StoryName, StorySectionName,
};
#[cfg(feature = "inspector")]
pub use gpui_storybook_core::story_inspector::StoryInspectorState;
pub use gpui_storybook_core::{
    assets::Assets,
    automation::{
        StoryActionSnapshot, StoryCaptureSnapshot, StoryInteractionCaptureRequest,
        StoryInteractionDispatch, StoryInteractionObservation, StoryInteractionPostcondition,
        StoryInteractionPostconditionSnapshot, StoryInteractionRequest, StoryInteractionSnapshot,
        StoryInteractionStep, StoryInteractionTargetBounds, StoryInteractionTargetSnapshot,
        StoryInteractionTargetsSnapshot, StoryModifier, StoryModifiers, StoryMouseButton,
        StoryPoint, StoryPointSpace, StoryScenarioRunSnapshot, StoryScenariosSnapshot,
        StorySemanticValueSnapshot, StorySemanticValuesSnapshot, StorybookAutomationError,
    },
    capture_region::{
        StorybookElementExt, capture_route_slug, capture_substory, capture_substory_route_id,
        capture_substory_route_id_with_key, capture_substory_with_key,
    },
    controls::{
        ControlBounds, ControlColor, ControlError, ControlKind, ControlSnapshot, ControlSpec,
        ControlTarget, ControlValue, ControlValueField, StoryControls, choice_control_value,
        parse_choice_control_value,
    },
    gallery::Gallery,
    language::{CurrentLanguage, Language},
    presentation::{StoryCanvasBackground, StoryPresentation, StoryViewportPreset},
    story::themes::STORYBOOK_THEME_DIR_ENV,
    story::{
        Story, StoryContainer, StoryScenario, StoryScenarioSnapshot, StoryScenarioStep,
        StorySection, StorySectionBase, StorySectionTitle, Substory, create_storybook_window,
        section,
    },
    storybook_window_ui::{StorybookWindow, StorybookWindowUi},
    theme_workbench::{ThemeColorRow, ThemeDraft, ThemeDraftError, theme_color_rows},
    workbench::{StoryWorkbench, WorkbenchState, WorkbenchTab},
};

#[doc(hidden)]
pub use gpui_storybook_core::registry as __registry;

#[doc(hidden)]
pub use inventory as __inventory;

#[cfg(all(feature = "mcp", any(target_os = "linux", target_os = "macos")))]
pub mod mcp {
    pub use gpui_storybook_mcp::*;
}

#[cfg(all(feature = "mcp", any(target_os = "linux", target_os = "macos")))]
pub mod capture {
    pub use gpui_storybook_mcp::capture::*;
}

struct ResolvedStoryEntry {
    entry: &'static __registry::StoryEntry,
    group: Option<String>,
    section: Option<String>,
}

#[derive(Debug)]
struct DuplicateStoryKeyError {
    key: StoryKey,
    first: StoryRegistrationLocation,
    second: StoryRegistrationLocation,
}

#[derive(Clone, Debug)]
struct StoryRegistrationLocation {
    crate_name: &'static str,
    story_name: StoryName,
    file: &'static str,
    line: u32,
}

impl From<&'static __registry::StoryEntry> for StoryRegistrationLocation {
    fn from(entry: &'static __registry::StoryEntry) -> Self {
        Self {
            crate_name: entry.crate_name,
            story_name: entry.name,
            file: entry.file,
            line: entry.line,
        }
    }
}

impl fmt::Display for DuplicateStoryKeyError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "duplicate story key `{}` registered by {}::{} at {}:{} and {}::{} at {}:{}",
            self.key,
            self.first.crate_name,
            self.first.story_name,
            self.first.file,
            self.first.line,
            self.second.crate_name,
            self.second.story_name,
            self.second.file,
            self.second.line,
        )
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct StoryGroupKey {
    group: Option<String>,
    section: Option<String>,
    title: String,
}

mod automation;
mod config;
mod entries;
mod init;
mod stories;

use automation::install_live_automation;
#[cfg(all(feature = "mcp", any(target_os = "linux", target_os = "macos")))]
use automation::start_mcp_automation;
pub use automation::try_preference_state;
use config::{
    apply_toml_preference_overrides, compare_resolved_story_entries, load_init_context,
    load_runtime_storybook_config, load_storybook_config, resolve_story_entry,
};
#[cfg(test)]
use config::{current_binary_name, find_cargo_project_root};
use entries::group_duplicate_story_titles;
#[cfg(test)]
use init::StorybookInitialized;
pub use init::init;
#[cfg(all(test, feature = "mcp", any(target_os = "linux", target_os = "macos")))]
use init::{AutomationPreferenceProfile, apply_automation_preference_profile};
pub use stories::generate_stories;
#[cfg(test)]
use stories::validate_unique_story_keys;

#[cfg(test)]
mod tests;
