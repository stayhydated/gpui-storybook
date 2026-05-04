use es_fluent::FluentMessage;
use es_fluent_manager_embedded as i18n_manager;
use std::sync::OnceLock;
use unic_langid::LanguageIdentifier;

es_fluent_manager_embedded::define_i18n_module!();

static I18N: OnceLock<i18n_manager::EmbeddedI18n> = OnceLock::new();

pub fn init() -> Result<(), anyhow::Error> {
    if I18N.get().is_some() {
        return Ok(());
    }

    let i18n = i18n_manager::EmbeddedI18n::try_new()?;
    let _ = I18N.set(i18n);
    Ok(())
}

pub fn change_locale<L>(locale: L) -> anyhow::Result<()>
where
    L: Into<LanguageIdentifier>,
{
    let locale = locale.into();
    let i18n = I18N
        .get()
        .ok_or_else(|| anyhow::anyhow!("embedded i18n has not been initialized"))?;

    i18n.select_language(locale).map_err(Into::into)
}

pub fn localize_message<T>(message: &T) -> Option<String>
where
    T: FluentMessage + ?Sized,
{
    let i18n = I18N.get()?;
    Some(i18n.localize_message(message))
}
