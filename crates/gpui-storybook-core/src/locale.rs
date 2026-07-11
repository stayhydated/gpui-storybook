use crate::language::Language;
use anyhow::{Result, anyhow};
use gpui::{App, Global};
use std::marker::PhantomData;
use unic_langid::LanguageIdentifier;

pub trait LocaleStore: Send + Sync {
    fn available_locales(&self, cx: &App) -> Result<Vec<(String, LanguageIdentifier)>>;
    fn current_locale(&self, cx: &App) -> Result<LanguageIdentifier>;
    fn set_current_locale(&self, locale: LanguageIdentifier, cx: &mut App) -> Result<()>;
}

impl Global for Box<dyn LocaleStore> {}

#[derive(Default)]
pub struct LocaleManager<L: Language> {
    _phantom: PhantomData<L>,
}

impl<L: Language> LocaleManager<L> {
    pub fn new() -> Self {
        Self::default()
    }
}

impl<L: Language> LocaleStore for LocaleManager<L> {
    fn available_locales(&self, cx: &App) -> Result<Vec<(String, LanguageIdentifier)>> {
        L::iter()
            .map(|language| {
                let locale = language.try_into().map_err(|_| {
                    anyhow!("failed to convert language {:?} into a locale", language)
                })?;
                let label = crate::i18n::localize_message(cx, &language)
                    .unwrap_or_else(|| locale.to_string());
                Ok((label, locale))
            })
            .collect()
    }

    fn current_locale(&self, cx: &App) -> Result<LanguageIdentifier> {
        let current_language = cx.global::<crate::language::CurrentLanguage<L>>().0;
        current_language.try_into().map_err(|_| {
            anyhow!(
                "failed to convert current language {:?} into a locale",
                current_language
            )
        })
    }

    fn set_current_locale(&self, locale: LanguageIdentifier, cx: &mut App) -> Result<()> {
        let new_lang = L::try_from(locale.clone()).map_err(|_| {
            anyhow!("failed to convert locale '{locale}' into an application language")
        })?;
        let locale_name = locale.to_string();

        crate::i18n::change_locale(cx, locale)
            .map_err(|err| anyhow!("failed to change locale to '{locale_name}': {err}"))?;
        cx.set_global(crate::language::CurrentLanguage(new_lang));
        gpui_component::set_locale(&locale_name);

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use es_fluent::{FluentMessage, FluentMessageLookup};

    #[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
    enum TestLanguage {
        #[default]
        En,
        Fr,
    }

    impl strum::IntoEnumIterator for TestLanguage {
        type Iterator = std::array::IntoIter<Self, 2>;

        fn iter() -> Self::Iterator {
            [Self::En, Self::Fr].into_iter()
        }
    }

    impl TryFrom<TestLanguage> for LanguageIdentifier {
        type Error = ();

        fn try_from(language: TestLanguage) -> Result<Self, Self::Error> {
            match language {
                TestLanguage::En => "en".parse().map_err(|_| ()),
                TestLanguage::Fr => "fr".parse().map_err(|_| ()),
            }
        }
    }

    impl TryFrom<LanguageIdentifier> for TestLanguage {
        type Error = ();

        fn try_from(locale: LanguageIdentifier) -> Result<Self, Self::Error> {
            match locale.to_string().as_str() {
                "en" => Ok(Self::En),
                "fr" => Ok(Self::Fr),
                _ => Err(()),
            }
        }
    }

    impl FluentMessage for TestLanguage {
        fn to_fluent_string_with(&self, _: &mut FluentMessageLookup<'_>) -> String {
            match self {
                Self::En => "English".to_string(),
                Self::Fr => "French".to_string(),
            }
        }
    }

    #[derive(Clone, Copy, Debug, Default)]
    struct InvalidLanguage;

    impl strum::IntoEnumIterator for InvalidLanguage {
        type Iterator = std::iter::Once<Self>;

        fn iter() -> Self::Iterator {
            std::iter::once(Self)
        }
    }

    impl TryFrom<InvalidLanguage> for LanguageIdentifier {
        type Error = ();

        fn try_from(_: InvalidLanguage) -> Result<Self, Self::Error> {
            Err(())
        }
    }

    impl TryFrom<LanguageIdentifier> for InvalidLanguage {
        type Error = ();

        fn try_from(_: LanguageIdentifier) -> Result<Self, Self::Error> {
            Err(())
        }
    }

    impl FluentMessage for InvalidLanguage {
        fn to_fluent_string_with(&self, _: &mut FluentMessageLookup<'_>) -> String {
            "Invalid".to_string()
        }
    }

    #[gpui::test]
    fn locale_manager_lists_reads_and_sets_typed_languages(cx: &mut App) {
        crate::i18n::init(cx).expect("embedded i18n should initialize");
        cx.set_global(crate::language::CurrentLanguage(TestLanguage::En));
        let manager = LocaleManager::<TestLanguage>::new();

        assert_eq!(
            manager.available_locales(cx).expect("locales should list"),
            vec![
                ("English".to_string(), "en".parse().expect("valid locale")),
                ("French".to_string(), "fr".parse().expect("valid locale")),
            ]
        );
        assert_eq!(
            manager
                .current_locale(cx)
                .expect("current locale should convert"),
            "en".parse::<LanguageIdentifier>().expect("valid locale")
        );
        let error = manager
            .set_current_locale("en".parse().expect("valid locale"), cx)
            .expect_err("empty test resources should reject locale selection");
        assert!(error.to_string().contains("failed to change locale"));
        assert_eq!(
            cx.global::<crate::language::CurrentLanguage<TestLanguage>>()
                .0,
            TestLanguage::En
        );

        let error = manager
            .set_current_locale("de".parse().expect("valid locale"), cx)
            .expect_err("unsupported typed locale should fail");
        assert!(error.to_string().contains("application language"));
    }

    #[gpui::test]
    fn locale_manager_reports_language_conversion_errors(cx: &mut App) {
        cx.set_global(crate::language::CurrentLanguage(InvalidLanguage));
        let manager = LocaleManager::<InvalidLanguage>::new();

        assert!(
            manager
                .available_locales(cx)
                .expect_err("invalid language should not list")
                .to_string()
                .contains("failed to convert language")
        );
        assert!(
            manager
                .current_locale(cx)
                .expect_err("invalid current language should fail")
                .to_string()
                .contains("failed to convert current language")
        );
    }
}
