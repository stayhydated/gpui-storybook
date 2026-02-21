use crate::language::Language;
use es_fluent::ToFluentString as _;
use gpui::{App, Global};
use std::marker::PhantomData;
use unic_langid::LanguageIdentifier;

pub trait LocaleStore: Send + Sync {
    fn available_locales(&self) -> Vec<(String, LanguageIdentifier)>;
    fn current_locale(&self, cx: &App) -> LanguageIdentifier;
    fn set_current_locale(&self, locale: LanguageIdentifier, cx: &mut App);
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
    fn available_locales(&self) -> Vec<(String, LanguageIdentifier)> {
        L::iter()
            .map(|l| (l.to_fluent_string(), l.try_into().ok().unwrap()))
            .collect()
    }

    fn current_locale(&self, cx: &App) -> LanguageIdentifier {
        cx.global::<crate::language::CurrentLanguage<L>>()
            .0
            .try_into()
            .ok()
            .unwrap()
    }

    fn set_current_locale(&self, locale: LanguageIdentifier, cx: &mut App) {
        let new_lang = L::try_from(locale.clone()).ok().unwrap();
        cx.set_global(crate::language::CurrentLanguage(new_lang));
        gpui_component::set_locale(&locale.to_string());
        crate::i18n::change_locale(locale);
    }
}
