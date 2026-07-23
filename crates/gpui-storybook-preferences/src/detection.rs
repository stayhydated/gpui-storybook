use std::collections::HashSet;

use strum::IntoStaticStr;

use crate::LanguageTag;

/// Generic light/dark device appearance supplied by the GPUI facade.
#[derive(Clone, Copy, Debug, Default, Eq, IntoStaticStr, PartialEq)]
#[strum(const_into_str, serialize_all = "snake_case")]
pub enum SystemColorScheme {
    /// Light device appearance.
    #[default]
    Light,
    /// Dark device appearance.
    Dark,
}

impl SystemColorScheme {
    /// Returns the stable diagnostic token.
    pub const fn token(self) -> &'static str {
        self.into_str()
    }
}

/// Ordered device locale candidates and validation diagnostics.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct DetectedLocales {
    /// Original platform values in descending preference order.
    pub raw: Vec<String>,
    /// Valid normalized BCP 47 language tags in descending preference order.
    pub candidates: Vec<LanguageTag>,
    /// Count of platform values that were not valid BCP 47 language tags.
    pub rejected_count: usize,
}

impl DetectedLocales {
    /// Validates injected BCP 47 locale strings while preserving order.
    ///
    /// The production detector receives normalized BCP 47 values from
    /// `sys-locale`; this constructor also provides the validation seam used by
    /// deterministic tests.
    pub fn from_raw(raw: Vec<String>) -> Self {
        let mut seen = HashSet::new();
        let mut candidates = Vec::new();
        let mut rejected_count = 0;

        for candidate in &raw {
            match LanguageTag::new(candidate) {
                Ok(candidate) if seen.insert(candidate.clone()) => candidates.push(candidate),
                Ok(_) => {},
                Err(_) => rejected_count += 1,
            }
        }

        Self {
            raw,
            candidates,
            rejected_count,
        }
    }
}

/// Source of ordered device locale candidates.
pub trait LocaleDetector: Send + Sync {
    /// Detects locales in descending platform preference order.
    fn detect(&self) -> DetectedLocales;
}

/// Production locale detector backed by `sys-locale`.
#[derive(Clone, Copy, Debug, Default)]
pub struct SystemLocaleDetector;

impl LocaleDetector for SystemLocaleDetector {
    fn detect(&self) -> DetectedLocales {
        let detected = DetectedLocales::from_raw(sys_locale::get_locales().collect());
        tracing::debug!(
            candidate_count = detected.raw.len(),
            normalized_count = detected.candidates.len(),
            rejected_count = detected.rejected_count,
            "detected ordered system locales"
        );
        detected
    }
}

/// Deterministic ordered locale detector for capture and tests.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct FixedLocaleDetector {
    detected: DetectedLocales,
}

impl FixedLocaleDetector {
    /// Creates a detector from already normalized diagnostics.
    pub fn new(detected: DetectedLocales) -> Self {
        Self { detected }
    }
}

impl LocaleDetector for FixedLocaleDetector {
    fn detect(&self) -> DetectedLocales {
        self.detected.clone()
    }
}
