use std::{collections::HashSet, path::PathBuf, sync::Arc};

use crate::*;

pub(super) const TEST_CONSUMER: &str = "test.storybook";

#[derive(Clone, Copy, Debug)]
pub(super) struct FixedClock(pub(super) i64);

impl PreferenceClock for FixedClock {
    fn now_unix_millis(&self) -> Result<i64, PreferenceClockError> {
        Ok(self.0)
    }
}

#[derive(Clone, Copy, Debug)]
pub(super) struct FailingClock;

impl PreferenceClock for FailingClock {
    fn now_unix_millis(&self) -> Result<i64, PreferenceClockError> {
        Err(PreferenceClockError::BeforeUnixEpoch)
    }
}

#[derive(Debug)]
pub(super) struct TestThemes {
    light: HashSet<ThemeId>,
    dark: HashSet<ThemeId>,
    light_fallback: Option<ThemeId>,
    dark_fallback: Option<ThemeId>,
}

impl TestThemes {
    pub(super) fn standard() -> Self {
        Self {
            light: [theme("light-default"), theme("light-paper")]
                .into_iter()
                .collect(),
            dark: [theme("dark-default"), theme("dark-ocean")]
                .into_iter()
                .collect(),
            light_fallback: Some(theme("light-default")),
            dark_fallback: Some(theme("dark-default")),
        }
    }

    pub(super) fn unavailable_fallbacks() -> Self {
        Self {
            light: HashSet::new(),
            dark: HashSet::new(),
            light_fallback: Some(theme("not-registered")),
            dark_fallback: None,
        }
    }
}

impl AvailableThemeResolver for TestThemes {
    fn is_available(&self, scheme: SystemColorScheme, theme: &ThemeId) -> bool {
        match scheme {
            SystemColorScheme::Light => self.light.contains(theme),
            SystemColorScheme::Dark => self.dark.contains(theme),
        }
    }

    fn fallback(&self, scheme: SystemColorScheme) -> Option<ThemeId> {
        match scheme {
            SystemColorScheme::Light => self.light_fallback.clone(),
            SystemColorScheme::Dark => self.dark_fallback.clone(),
        }
    }
}

pub(super) fn consumer(value: &str) -> ConsumerId {
    value.parse().expect("test consumer id is valid")
}

pub(super) fn theme(value: &str) -> ThemeId {
    value.parse().expect("test theme id is valid")
}

pub(super) fn language(value: &str) -> LanguageTag {
    value.parse().expect("test language tag is valid")
}

pub(super) fn supported_languages() -> SupportedLanguages {
    SupportedLanguages::new(
        [language("en-US"), language("fr"), language("zh-Hant")],
        language("en-US"),
    )
    .expect("test language set is valid")
}

pub(super) fn saved_preferences() -> StorybookPreferences {
    StorybookPreferences {
        window_mode: StorybookWindowMode::Dock,
        color_scheme: PreferredColorScheme::System,
        light_theme: Some(theme("light-paper")),
        dark_theme: Some(theme("dark-ocean")),
        language: PreferredLanguage::Explicit(language("fr")),
        scrollbar: PreferredScrollbar::Always,
    }
}

pub(super) fn persistent_options(
    path: impl Into<PathBuf>,
    consumer_id: &str,
    clock: Arc<dyn PreferenceClock>,
) -> RepositoryOptions {
    let mut options = RepositoryOptions::persistent(consumer(consumer_id));
    options.json_path = Some(path.into());
    options.clock = clock;
    options
}
