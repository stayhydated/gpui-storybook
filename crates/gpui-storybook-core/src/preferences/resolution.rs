use super::*;

struct RegistryThemes<'a> {
    registry: &'a ThemeRegistry,
}

impl AvailableThemeResolver for RegistryThemes<'_> {
    fn is_available(&self, scheme: SystemColorScheme, theme: &ThemeId) -> bool {
        self.registry
            .themes()
            .get(theme.as_str())
            .is_some_and(|config| config.mode == theme_mode(scheme))
    }

    fn fallback(&self, scheme: SystemColorScheme) -> Option<ThemeId> {
        let config = match scheme {
            SystemColorScheme::Light => self.registry.default_light_theme(),
            SystemColorScheme::Dark => self.registry.default_dark_theme(),
        };
        ThemeId::new(config.name.as_ref()).ok()
    }
}
/// Converts a GPUI window appearance into the platform-independent scheme.
pub fn color_scheme(appearance: WindowAppearance) -> SystemColorScheme {
    match appearance {
        WindowAppearance::Light | WindowAppearance::VibrantLight => SystemColorScheme::Light,
        WindowAppearance::Dark | WindowAppearance::VibrantDark => SystemColorScheme::Dark,
    }
}

pub(super) fn resolve(
    saved: &StorybookPreferences,
    detected_scheme: SystemColorScheme,
    detected_locales: &DetectedLocales,
    supported_languages: &SupportedLanguages,
    overrides: &ResolutionOverrides,
    cx: &App,
) -> Result<ResolvedPreferences, ResolvePreferencesError> {
    resolve_preferences(
        saved,
        detected_scheme,
        detected_locales,
        supported_languages,
        &RegistryThemes {
            registry: ThemeRegistry::global(cx),
        },
        overrides,
    )
}

pub(super) fn theme_mode(scheme: SystemColorScheme) -> ThemeMode {
    match scheme {
        SystemColorScheme::Light => ThemeMode::Light,
        SystemColorScheme::Dark => ThemeMode::Dark,
    }
}

pub(super) fn scrollbar_mode(scrollbar: PreferredScrollbar) -> ScrollbarMode {
    match scrollbar {
        PreferredScrollbar::Scrolling => ScrollbarMode::Scrolling,
        PreferredScrollbar::Hover => ScrollbarMode::Hover,
        PreferredScrollbar::Always => ScrollbarMode::Always,
    }
}

pub(super) fn repository_open_category(error: &RepositoryOpenError) -> &'static str {
    match error {
        RepositoryOpenError::PathOverrideRequiresPersistent { .. } => "path_override",
        RepositoryOpenError::UnsupportedPersistence { .. } => "unsupported_persistence",
        RepositoryOpenError::TemporaryDirectoryTask { .. } => "temporary_task",
        RepositoryOpenError::TemporaryDirectory { .. } => "temporary_directory",
        RepositoryOpenError::Clock(_) => "clock",
        RepositoryOpenError::InvalidJsonPath { .. } => "invalid_json_path",
        RepositoryOpenError::PreferenceSchemaPathCollision { .. } => "schema_path_collision",
        RepositoryOpenError::ArchiveInvalidJson { .. } => "archive_invalid_json",
        RepositoryOpenError::JsonIo { .. } => "json_io",
    }
}

pub(super) fn repository_open_path(error: &RepositoryOpenError) -> Option<PathBuf> {
    match error {
        RepositoryOpenError::InvalidJsonPath { path }
        | RepositoryOpenError::ArchiveInvalidJson { path, .. }
        | RepositoryOpenError::JsonIo { path, .. } => Some(path.clone()),
        RepositoryOpenError::PreferenceSchemaPathCollision {
            preference_path, ..
        } => Some(preference_path.clone()),
        _ => None,
    }
}

pub(super) fn store_error_category(
    error: &gpui_storybook_preferences::PreferenceStoreError,
) -> &'static str {
    use gpui_storybook_preferences::PreferenceStoreError;
    match error {
        PreferenceStoreError::AlreadyExists { .. } => "already_exists",
        PreferenceStoreError::NotFound { .. } => "not_found",
        PreferenceStoreError::Json { .. } => "json",
        PreferenceStoreError::Io { .. } => "io",
    }
}

/// Converts a public persistence selection into repository options.
#[doc(hidden)]
pub fn repository_options(
    consumer_id: gpui_storybook_preferences::ConsumerId,
    persistence: PersistenceMode,
    json_path: Option<PathBuf>,
    project_root: PathBuf,
) -> RepositoryOptions {
    let mut options = match persistence {
        PersistenceMode::Persistent => RepositoryOptions::persistent(consumer_id),
        PersistenceMode::Temporary => RepositoryOptions::temporary(consumer_id),
        PersistenceMode::Disabled => RepositoryOptions::disabled(consumer_id),
    };
    options.json_path = json_path;
    options.project_root = Some(project_root);
    options
}

/// Converts an explicit locale action payload into validated saved intent.
pub(crate) fn explicit_language(identifier: LanguageIdentifier) -> Option<PreferredLanguage> {
    LanguageTag::new(identifier.to_string())
        .ok()
        .map(PreferredLanguage::Explicit)
}
