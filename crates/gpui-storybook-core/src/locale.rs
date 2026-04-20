use crate::language::Language;
use anyhow::{Result, anyhow};
use es_fluent::ToFluentString as _;
use gpui::{App, Global};
use std::marker::PhantomData;
use unic_langid::LanguageIdentifier;

pub trait LocaleStore: Send + Sync {
    fn available_locales(&self) -> Result<Vec<(String, LanguageIdentifier)>>;
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
    fn available_locales(&self) -> Result<Vec<(String, LanguageIdentifier)>> {
        L::iter()
            .map(|language| {
                let locale = language.try_into().map_err(|_| {
                    anyhow!("failed to convert language {:?} into a locale", language)
                })?;
                Ok((language.to_fluent_string(), locale))
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

        crate::i18n::change_locale(locale)
            .map_err(|err| anyhow!("failed to change locale to '{locale_name}': {err}"))?;
        cx.set_global(crate::language::CurrentLanguage(new_lang));
        gpui_component::set_locale(&locale_name);

        Ok(())
    }
}
