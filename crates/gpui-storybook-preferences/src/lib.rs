//! Internal local preference persistence and system-detection primitives for
//! GPUI Storybook.
//!
//! This crate owns typed JSON documents, Rust-derived JSON Schema,
//! consumer-scoped repositories, project-local path resolution, invalid-file
//! recovery, `sys-locale` discovery, and injected locale/resolution seams. The
//! public `gpui-storybook` facade owns GPUI appearance events, application
//! startup, and UI orchestration.
//!
//! [`StorybookPreferences`] retains saved intent. [`ResolvedPreferences`]
//! explains effective values and their sources without rewriting that intent.
//! [`ConsumerId`] isolates default persistent paths and row keys, including
//! files and default persistent paths. Persistent, temporary, and disabled
//! modes make filesystem behavior explicit; only persistent mode accepts a JSON
//! path override.

mod detection;
mod repository;
mod resolution;
mod value;

pub use detection::{
    DetectedLocales, FixedLocaleDetector, LocaleDetector, SystemColorScheme, SystemLocaleDetector,
};
pub use repository::{
    OpenRepository, PersistenceMode, PreferenceClock, PreferenceClockError, PreferenceRepository,
    PreferenceStoreError, RecoveryDiagnostic, RecoveryReason, RepositoryOpenError,
    RepositoryOptions, StoreOperation, SystemPreferenceClock, persistent_json_path,
    preference_json_schema, preference_json_schema_pretty,
};
pub use resolution::{
    AvailableThemeResolver, ColorSchemeResolution, ColorSchemeSource, LanguageResolution,
    LanguageSource, ResolutionDiagnostic, ResolutionOverrides, ResolvePreferencesError,
    ResolvedPreferences, SupportedLanguages, SupportedLanguagesError, ThemeResolution, ThemeSource,
    UnsupportedValueSource, resolve_preferences,
};
pub use value::{
    ConsumerId, ConsumerIdError, LanguageTag, LanguageTagError, MAX_CONSUMER_ID_LEN,
    MAX_LANGUAGE_TAG_LEN, MAX_THEME_ID_LEN, PreferenceRecord, PreferredColorScheme,
    PreferredLanguage, PreferredLanguageMode, PreferredScrollbar, StorybookPreferences, ThemeId,
    ThemeIdError,
};

#[cfg(test)]
mod tests;
