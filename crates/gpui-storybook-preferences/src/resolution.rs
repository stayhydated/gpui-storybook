use std::collections::HashSet;

use fluent_langneg::{NegotiationStrategy, negotiate_languages};
use strum::IntoStaticStr;

use crate::{
    DetectedLocales, LanguageTag, PreferredColorScheme, PreferredLanguage, PreferredScrollbar,
    StorybookPreferences, SystemColorScheme, ThemeId,
};

/// Validated embedded languages and configured application fallback.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SupportedLanguages {
    available: Vec<LanguageTag>,
    fallback: LanguageTag,
}

impl SupportedLanguages {
    /// Validates an ordered embedded language set and fallback.
    ///
    /// Duplicate language tags are removed while preserving the first entry.
    ///
    /// # Errors
    ///
    /// Returns [`SupportedLanguagesError`] when the set is empty or does not
    /// contain the configured fallback.
    pub fn new(
        available: impl IntoIterator<Item = LanguageTag>,
        fallback: LanguageTag,
    ) -> Result<Self, SupportedLanguagesError> {
        let mut seen = HashSet::new();
        let available = available
            .into_iter()
            .filter(|language| seen.insert(language.clone()))
            .collect::<Vec<_>>();
        if available.is_empty() {
            return Err(SupportedLanguagesError::Empty);
        }
        if !available.contains(&fallback) {
            return Err(SupportedLanguagesError::UnsupportedFallback { fallback });
        }
        Ok(Self {
            available,
            fallback,
        })
    }

    /// Returns embedded languages in application order.
    pub fn available(&self) -> &[LanguageTag] {
        &self.available
    }

    /// Returns the configured application fallback.
    pub fn fallback(&self) -> &LanguageTag {
        &self.fallback
    }

    fn exact(&self, requested: &LanguageTag) -> Option<LanguageTag> {
        self.available
            .iter()
            .find(|available| *available == requested)
            .cloned()
    }
}

/// Failure to construct the application's supported-language contract.
#[derive(Debug, thiserror::Error)]
pub enum SupportedLanguagesError {
    /// No embedded languages were supplied.
    #[error("Storybook supported language set must not be empty")]
    Empty,
    /// The fallback was absent from the embedded set.
    #[error("Storybook fallback language '{fallback}' is not supported")]
    UnsupportedFallback {
        /// Configured fallback language.
        fallback: LanguageTag,
    },
}

/// Injected theme-registry availability and fallback seam.
pub trait AvailableThemeResolver {
    /// Returns whether `theme` is registered for `scheme`.
    fn is_available(&self, scheme: SystemColorScheme, theme: &ThemeId) -> bool;

    /// Returns the registered fallback for `scheme`.
    fn fallback(&self, scheme: SystemColorScheme) -> Option<ThemeId>;
}

/// Deterministic values used by tests and capture sessions.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ResolutionOverrides {
    /// Effective light/dark override.
    pub color_scheme: Option<SystemColorScheme>,
    /// Effective named-theme override.
    pub theme: Option<ThemeId>,
    /// Effective language override.
    pub language: Option<LanguageTag>,
}

/// Explanation for the effective light/dark scheme.
#[derive(Clone, Copy, Debug, Eq, IntoStaticStr, PartialEq)]
#[strum(const_into_str, serialize_all = "snake_case")]
pub enum ColorSchemeSource {
    /// Deterministic capture or test override.
    Override,
    /// Explicit saved light or dark intent.
    Explicit,
    /// Current device appearance used by saved system intent.
    System,
}

impl ColorSchemeSource {
    /// Returns the stable diagnostic token.
    pub const fn token(self) -> &'static str {
        self.into_str()
    }
}

/// Effective light/dark scheme and source.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ColorSchemeResolution {
    /// Effective light/dark scheme.
    pub scheme: SystemColorScheme,
    /// Source that selected the effective scheme.
    pub source: ColorSchemeSource,
}

/// Explanation for the effective named theme.
#[derive(Clone, Copy, Debug, Eq, IntoStaticStr, PartialEq)]
#[strum(const_into_str, serialize_all = "snake_case")]
pub enum ThemeSource {
    /// Deterministic capture or test override.
    Override,
    /// Saved theme for the effective light/dark slot.
    Saved,
    /// Registered fallback for the effective slot.
    Fallback,
}

impl ThemeSource {
    /// Returns the stable diagnostic token.
    pub const fn token(self) -> &'static str {
        self.into_str()
    }
}

/// Effective named theme and source.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ThemeResolution {
    /// Registered theme identifier.
    pub theme: ThemeId,
    /// Source that selected the effective theme.
    pub source: ThemeSource,
}

/// Explanation for the effective language.
#[derive(Clone, Copy, Debug, Eq, IntoStaticStr, PartialEq)]
#[strum(const_into_str, serialize_all = "snake_case")]
pub enum LanguageSource {
    /// Deterministic capture or test override.
    Override,
    /// Supported explicit saved language.
    Explicit,
    /// Negotiated ordered device locale.
    System,
    /// Configured application fallback.
    Fallback,
}

impl LanguageSource {
    /// Returns the stable diagnostic token.
    pub const fn token(self) -> &'static str {
        self.into_str()
    }
}

/// Effective supported language and source.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LanguageResolution {
    /// Effective supported language.
    pub language: LanguageTag,
    /// Source that selected the effective language.
    pub source: LanguageSource,
}

/// Origin of an unsupported named value retained for diagnostics.
#[derive(Clone, Copy, Debug, Eq, IntoStaticStr, PartialEq)]
#[strum(const_into_str, serialize_all = "snake_case")]
pub enum UnsupportedValueSource {
    /// Value came from saved intent.
    Saved,
    /// Value came from a deterministic override.
    Override,
}

impl UnsupportedValueSource {
    /// Returns the stable diagnostic token.
    pub const fn token(self) -> &'static str {
        self.into_str()
    }
}

/// Non-fatal resolution diagnostic retained for the later public facade.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ResolutionDiagnostic {
    /// A named theme was missing and the registered slot fallback was used.
    MissingTheme {
        /// Effective light/dark slot.
        scheme: SystemColorScheme,
        /// Missing registry identifier.
        requested: ThemeId,
        /// Registered fallback that was used.
        fallback: ThemeId,
        /// Whether the missing value was saved or overridden.
        source: UnsupportedValueSource,
    },
    /// An explicit or overridden language was unsupported and fallback was used.
    UnsupportedLanguage {
        /// Retained unsupported tag.
        requested: LanguageTag,
        /// Configured fallback that was used.
        fallback: LanguageTag,
        /// Whether the missing value was saved or overridden.
        source: UnsupportedValueSource,
    },
    /// No valid ordered device locale matched an embedded language.
    NoSupportedSystemLocale {
        /// Configured fallback that was used.
        fallback: LanguageTag,
        /// Number of platform locale strings rejected during normalization.
        rejected_count: usize,
    },
}

/// Fully resolved Storybook preferences and structured diagnostics.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedPreferences {
    /// Original saved intent, including unsupported named values.
    pub saved: StorybookPreferences,
    /// Effective light/dark scheme.
    pub color_scheme: ColorSchemeResolution,
    /// Effective registered theme.
    pub theme: ThemeResolution,
    /// Effective supported language.
    pub language: LanguageResolution,
    /// Effective saved scrollbar policy.
    pub scrollbar: PreferredScrollbar,
    /// Non-fatal fallback diagnostics.
    pub diagnostics: Vec<ResolutionDiagnostic>,
}

/// Failure to resolve preferences against the injected application registry.
#[derive(Clone, Debug, Eq, thiserror::Error, PartialEq)]
pub enum ResolvePreferencesError {
    /// The theme registry did not provide a usable fallback for a slot.
    #[error("no registered fallback theme is available for {scheme:?} mode")]
    MissingFallbackTheme {
        /// Light/dark slot without a fallback.
        scheme: SystemColorScheme,
    },
}

/// Resolves saved intent against one detection snapshot and application
/// registries.
///
/// # Errors
///
/// Returns [`ResolvePreferencesError::MissingFallbackTheme`] when the injected
/// theme resolver cannot supply an available fallback for the effective slot.
pub fn resolve_preferences(
    saved: &StorybookPreferences,
    detected_scheme: SystemColorScheme,
    detected_locales: &DetectedLocales,
    supported_languages: &SupportedLanguages,
    themes: &dyn AvailableThemeResolver,
    overrides: &ResolutionOverrides,
) -> Result<ResolvedPreferences, ResolvePreferencesError> {
    let color_scheme = resolve_color_scheme(saved.color_scheme, detected_scheme, overrides);
    let mut diagnostics = Vec::new();
    let theme = resolve_theme(
        saved,
        color_scheme.scheme,
        themes,
        overrides,
        &mut diagnostics,
    )?;
    let language = resolve_language(
        &saved.language,
        detected_locales,
        supported_languages,
        overrides,
        &mut diagnostics,
    );

    Ok(ResolvedPreferences {
        saved: saved.clone(),
        color_scheme,
        theme,
        language,
        scrollbar: saved.scrollbar,
        diagnostics,
    })
}

fn resolve_color_scheme(
    saved: PreferredColorScheme,
    detected: SystemColorScheme,
    overrides: &ResolutionOverrides,
) -> ColorSchemeResolution {
    if let Some(scheme) = overrides.color_scheme {
        return ColorSchemeResolution {
            scheme,
            source: ColorSchemeSource::Override,
        };
    }
    match saved {
        PreferredColorScheme::System => ColorSchemeResolution {
            scheme: detected,
            source: ColorSchemeSource::System,
        },
        PreferredColorScheme::Light => ColorSchemeResolution {
            scheme: SystemColorScheme::Light,
            source: ColorSchemeSource::Explicit,
        },
        PreferredColorScheme::Dark => ColorSchemeResolution {
            scheme: SystemColorScheme::Dark,
            source: ColorSchemeSource::Explicit,
        },
    }
}

fn resolve_theme(
    saved: &StorybookPreferences,
    scheme: SystemColorScheme,
    themes: &dyn AvailableThemeResolver,
    overrides: &ResolutionOverrides,
    diagnostics: &mut Vec<ResolutionDiagnostic>,
) -> Result<ThemeResolution, ResolvePreferencesError> {
    let requested = overrides
        .theme
        .as_ref()
        .map(|theme| {
            (
                theme,
                ThemeSource::Override,
                UnsupportedValueSource::Override,
            )
        })
        .or_else(|| {
            let saved_theme = match scheme {
                SystemColorScheme::Light => saved.light_theme.as_ref(),
                SystemColorScheme::Dark => saved.dark_theme.as_ref(),
            };
            saved_theme.map(|theme| (theme, ThemeSource::Saved, UnsupportedValueSource::Saved))
        });

    if let Some((requested, source, _)) = requested
        && themes.is_available(scheme, requested)
    {
        return Ok(ThemeResolution {
            theme: requested.clone(),
            source,
        });
    }

    let Some(fallback) = themes
        .fallback(scheme)
        .filter(|fallback| themes.is_available(scheme, fallback))
    else {
        return Err(ResolvePreferencesError::MissingFallbackTheme { scheme });
    };

    if let Some((requested, _, source)) = requested {
        tracing::warn!(
            scheme = scheme.token(),
            source = source.token(),
            "named Storybook theme is unavailable; using registered fallback"
        );
        diagnostics.push(ResolutionDiagnostic::MissingTheme {
            scheme,
            requested: requested.clone(),
            fallback: fallback.clone(),
            source,
        });
    }

    Ok(ThemeResolution {
        theme: fallback,
        source: ThemeSource::Fallback,
    })
}

fn resolve_language(
    saved: &PreferredLanguage,
    detected: &DetectedLocales,
    supported: &SupportedLanguages,
    overrides: &ResolutionOverrides,
    diagnostics: &mut Vec<ResolutionDiagnostic>,
) -> LanguageResolution {
    if let Some(overridden) = &overrides.language {
        return supported.exact(overridden).map_or_else(
            || {
                diagnostics.push(ResolutionDiagnostic::UnsupportedLanguage {
                    requested: overridden.clone(),
                    fallback: supported.fallback.clone(),
                    source: UnsupportedValueSource::Override,
                });
                LanguageResolution {
                    language: supported.fallback.clone(),
                    source: LanguageSource::Fallback,
                }
            },
            |language| LanguageResolution {
                language,
                source: LanguageSource::Override,
            },
        );
    }

    if let PreferredLanguage::Explicit(explicit) = saved {
        return supported.exact(explicit).map_or_else(
            || {
                diagnostics.push(ResolutionDiagnostic::UnsupportedLanguage {
                    requested: explicit.clone(),
                    fallback: supported.fallback.clone(),
                    source: UnsupportedValueSource::Saved,
                });
                LanguageResolution {
                    language: supported.fallback.clone(),
                    source: LanguageSource::Fallback,
                }
            },
            |language| LanguageResolution {
                language,
                source: LanguageSource::Explicit,
            },
        );
    }

    let negotiated = negotiate_languages(
        &detected.candidates,
        &supported.available,
        None,
        NegotiationStrategy::Lookup,
    );
    if let Some(language) = negotiated.first() {
        return LanguageResolution {
            language: (*language).clone(),
            source: LanguageSource::System,
        };
    }

    tracing::warn!(
        locale_count = detected.candidates.len(),
        rejected_count = detected.rejected_count,
        "no system locale matched an embedded Storybook language"
    );
    diagnostics.push(ResolutionDiagnostic::NoSupportedSystemLocale {
        fallback: supported.fallback.clone(),
        rejected_count: detected.rejected_count,
    });
    LanguageResolution {
        language: supported.fallback.clone(),
        source: LanguageSource::Fallback,
    }
}
