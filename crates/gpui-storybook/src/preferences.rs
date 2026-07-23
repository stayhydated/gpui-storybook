//! Public initialization and preference-state contract for Storybook consumers.
//!
//! Options identify one stable consumer, select a persistence mode, supply the
//! typed locale fallback/adapter, and optionally override resolved values for
//! deterministic automation. The active runtime `storybook.toml` can supply
//! the same launch-only override fields. Saved intent remains distinct from
//! resolved presentation; inspect both through
//! `gpui_storybook::try_preference_state`.

use std::{error::Error, fmt, path::PathBuf, rc::Rc};

use gpui::App;
use gpui_storybook_core::language::Language;

pub use gpui_storybook_core::preferences::{
    PersistenceStatus, PreferenceDiagnostic, PreferenceState, StorybookReady,
};
pub use gpui_storybook_preferences::{
    ColorSchemeResolution, ColorSchemeSource, ConsumerId, ConsumerIdError, LanguageResolution,
    LanguageSource, LanguageTag, PersistenceMode, PreferredColorScheme, PreferredLanguage,
    PreferredLanguageMode, PreferredScrollbar, RecoveryDiagnostic, RecoveryReason,
    ResolutionDiagnostic, ResolvedPreferences, StorybookPreferences, SystemColorScheme, ThemeId,
    ThemeIdError, ThemeResolution, ThemeSource, UnsupportedValueSource, preference_json_schema,
    preference_json_schema_pretty,
};

type ApplyLocale<L> = Rc<dyn Fn(L, &mut App) -> Result<(), LocaleApplicationError>>;

/// Deterministic values that take precedence over saved and detected values.
///
/// Overrides are intended for capture sessions and deterministic tests. They
/// affect resolved runtime state without replacing the user's saved intent.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct PreferenceOverrides<L> {
    /// Effective light or dark appearance override.
    pub color_scheme: Option<SystemColorScheme>,
    /// Effective registered theme override.
    pub theme: Option<ThemeId>,
    /// Effective typed language override.
    pub language: Option<L>,
}

/// Options for initializing one consumer's Storybook runtime.
///
/// Construct this value with [`StorybookOptions::new`], then select storage,
/// path, or deterministic override behavior with the builder methods.
pub struct StorybookOptions<L>
where
    L: Language,
{
    /// Stable identifier that isolates this binary's saved preferences.
    pub consumer_id: ConsumerId,
    /// Typed language used when system negotiation has no supported match.
    pub fallback_language: L,
    /// Local persistence behavior; defaults to persistent project-local storage.
    pub persistence: PersistenceMode,
    /// Explicit persistent JSON path for portable development or tests.
    ///
    /// A path is rejected unless [`Self::persistence`] is
    /// [`PersistenceMode::Persistent`].
    pub json_path: Option<PathBuf>,
    /// Deterministic runtime overrides.
    pub overrides: PreferenceOverrides<L>,
    pub(crate) apply_locale: ApplyLocale<L>,
}

impl<L> StorybookOptions<L>
where
    L: Language,
{
    /// Creates persistent options with a typed fallback and locale adapter.
    ///
    /// The adapter is invoked on the GPUI foreground for the initial resolved
    /// language and every later language change. It should update the
    /// consuming application's own localization manager.
    pub fn new<F, E>(consumer_id: ConsumerId, fallback_language: L, apply_locale: F) -> Self
    where
        F: Fn(L, &mut App) -> Result<(), E> + 'static,
        E: Error + Send + Sync + 'static,
    {
        Self {
            consumer_id,
            fallback_language,
            persistence: PersistenceMode::Persistent,
            json_path: None,
            overrides: PreferenceOverrides::default(),
            apply_locale: Rc::new(move |language, cx| {
                apply_locale(language, cx).map_err(LocaleApplicationError::new)
            }),
        }
    }

    /// Selects local persistence behavior.
    ///
    /// Temporary mode uses a unique JSON file for the repository lifetime.
    /// Disabled mode keeps state in memory. Neither accepts a JSON path
    /// override.
    pub fn with_persistence(mut self, persistence: PersistenceMode) -> Self {
        self.persistence = persistence;
        self
    }

    /// Selects an explicit persistent JSON path.
    ///
    /// Initialization rejects a path override unless persistence is
    /// [`PersistenceMode::Persistent`].
    pub fn with_json_path(mut self, json_path: impl Into<PathBuf>) -> Self {
        self.json_path = Some(json_path.into());
        self
    }

    /// Selects deterministic resolved-value overrides.
    pub fn with_overrides(mut self, overrides: PreferenceOverrides<L>) -> Self {
        self.overrides = overrides;
        self
    }
}

impl<L> fmt::Debug for StorybookOptions<L>
where
    L: Language,
{
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("StorybookOptions")
            .field("consumer_id", &self.consumer_id)
            .field("fallback_language", &self.fallback_language)
            .field("persistence", &self.persistence)
            .field("json_path", &self.json_path)
            .field("overrides", &self.overrides)
            .finish_non_exhaustive()
    }
}

/// Failure reported by a consumer's locale application callback.
#[derive(Debug, thiserror::Error)]
#[error("the consumer locale adapter failed: {source}")]
pub struct LocaleApplicationError {
    #[source]
    source: Box<dyn Error + Send + Sync>,
}

impl LocaleApplicationError {
    fn new(source: impl Error + Send + Sync + 'static) -> Self {
        Self {
            source: Box::new(source),
        }
    }
}

/// Static Storybook configuration error returned before async loading starts.
#[derive(Debug, thiserror::Error)]
pub enum StorybookInitError {
    /// A typed application language could not be converted to a BCP 47 tag.
    #[error("failed to convert Storybook language {language:?} to a BCP 47 tag")]
    InvalidLanguage {
        /// Language value that failed conversion.
        language: String,
    },
    /// The configured fallback is absent from the typed language set.
    #[error("the configured Storybook fallback language is not in the available language set")]
    UnsupportedFallback,
    /// A JSON path was supplied for non-persistent storage.
    #[error("a Storybook JSON path override requires persistent storage")]
    PathOverrideRequiresPersistent,
    /// The active runtime `storybook.toml` could not be loaded or parsed.
    #[error("failed to load the active Storybook runtime config: {source}")]
    RuntimeConfig {
        /// Loader failure with the config path and source error.
        #[source]
        source: gpui_storybook_toml::StorybookTomlError,
    },
    /// A preference override from `storybook.toml` was invalid or unsupported.
    #[error("invalid storybook.toml override `{field} = {value:?}`")]
    InvalidTomlOverride {
        /// Name of the invalid override field.
        field: &'static str,
        /// Configured value retained for an actionable error.
        value: String,
    },
    /// Core localization or preference resolution could not be configured.
    #[error("failed to initialize Storybook runtime ({category})")]
    CoreInitialization {
        /// Stable initialization failure category.
        category: String,
    },
    /// Storybook has already been initialized for this GPUI application.
    #[error("GPUI Storybook is already initialized")]
    AlreadyInitialized,
}
