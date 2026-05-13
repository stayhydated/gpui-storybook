use es_fluent::FluentMessage;
use es_fluent_manager_embedded as i18n_manager;
use gpui::App;
use std::borrow::Borrow;
use unic_langid::LanguageIdentifier;

// reenable later when we i18n this crate
// es_fluent_manager_embedded::define_i18n_module!();

pub struct I18n {
    manager: i18n_manager::EmbeddedI18n,
}

impl I18n {
    fn new() -> Result<Self, i18n_manager::EmbeddedInitError> {
        Ok(Self {
            manager: i18n_manager::EmbeddedI18n::try_new()?,
        })
    }

    fn select_language<L>(&self, locale: L) -> Result<(), i18n_manager::LocalizationError>
    where
        L: Into<LanguageIdentifier>,
    {
        self.manager.select_language(locale)
    }

    fn localize_message<T>(&self, message: &T) -> String
    where
        T: FluentMessage + ?Sized,
    {
        self.manager.localize_message(message)
    }
}

impl gpui::Global for I18n {}

pub fn init(cx: &mut App) -> Result<(), anyhow::Error> {
    if cx.try_global::<I18n>().is_none() {
        cx.set_global(I18n::new()?);
    }
    Ok(())
}

pub fn change_locale<L>(cx: &mut App, locale: L) -> anyhow::Result<()>
where
    L: Into<LanguageIdentifier>,
{
    cx.global::<I18n>()
        .select_language(locale)
        .map_err(Into::into)
}

pub fn localize_message<T>(cx: &impl Borrow<App>, message: &T) -> Option<String>
where
    T: FluentMessage + ?Sized,
{
    Some(cx.borrow().try_global::<I18n>()?.localize_message(message))
}
