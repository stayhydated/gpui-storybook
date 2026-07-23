use es_fluent::{
    FluentArgs, FluentLocalizer, FluentLocalizerExt as _,
    registry::{StaticFluentDomain, StaticFluentEntryId},
};
use es_fluent_manager_core::{I18nModule as _, Localizer};
use es_fluent_manager_embedded::{EmbeddedInitError, LocalizationError};
use gpui::{App, Global};
use std::borrow::Borrow;
use unic_langid::LanguageIdentifier;

es_fluent_manager_embedded::define_i18n_module!();

struct StorybookI18n {
    localizer: Box<dyn Localizer>,
}

impl Global for StorybookI18n {}

impl StorybookI18n {
    fn select_language(&self, language: &LanguageIdentifier) -> Result<(), LocalizationError> {
        match self.localizer.select_language(language) {
            Ok(()) => Ok(()),
            Err(LocalizationError::LanguageNotSupported(_)) => {
                tracing::debug!(
                    "requested Storybook shell locale is unavailable; using embedded English fallback"
                );
                self.localizer.select_language(&fallback_language())
            },
            Err(error) => Err(error),
        }
    }
}

impl FluentLocalizer for StorybookI18n {
    fn localize<'a>(
        &self,
        id: StaticFluentEntryId,
        args: Option<&FluentArgs<'a>>,
    ) -> Option<String> {
        self.localizer.localize(id, args.map(FluentArgs::as_raw))
    }

    fn localize_in_domain<'a>(
        &self,
        domain: StaticFluentDomain,
        id: StaticFluentEntryId,
        args: Option<&FluentArgs<'a>>,
    ) -> Option<String> {
        (domain == StaticFluentDomain::from_package_name(env!("CARGO_PKG_NAME")))
            .then(|| self.localize(id, args))
            .flatten()
    }
}

fn fallback_language() -> LanguageIdentifier {
    "en".parse()
        .expect("Storybook's embedded fallback language should be valid")
}

/// Initializes the embedded localization resources owned by Storybook.
///
/// Application-facing setup normally goes through `gpui_storybook::init`,
/// which calls this function while installing the application's typed locale
/// manager and the rest of the Storybook runtime.
pub fn init(cx: &mut App) -> Result<(), EmbeddedInitError> {
    let _linked_module = &GPUI_STORYBOOK_CORE_I18N_MODULE;
    if cx.try_global::<StorybookI18n>().is_some() {
        return Ok(());
    }
    let localizer = GPUI_STORYBOOK_CORE_I18N_MODULE.create_localizer();
    localizer
        .select_language(&fallback_language())
        .map_err(EmbeddedInitError::LanguageSelection)?;
    cx.set_global(StorybookI18n { localizer });
    Ok(())
}

/// Changes the active embedded Storybook shell locale.
///
/// When the requested locale is not embedded by Storybook, the shell falls
/// back to its embedded English resources. Consumer messages remain owned by
/// the application's separate localization manager.
///
/// # Errors
///
/// Returns an error when the requested locale or the embedded English fallback
/// cannot be installed by the Storybook localization manager.
pub fn change_locale<L>(cx: &mut App, locale: L) -> Result<(), LocalizationError>
where
    L: Into<LanguageIdentifier>,
{
    cx.global::<StorybookI18n>().select_language(&locale.into())
}

/// Localizes a Fluent message with the manager stored in the GPUI app context.
///
/// Returns `None` when localization has not been initialized or the message
/// cannot be resolved for the active locale.
pub fn localize_message<T>(cx: &impl Borrow<App>, message: &T) -> Option<String>
where
    T: es_fluent::FluentMessage + ?Sized,
{
    cx.borrow()
        .try_global::<StorybookI18n>()?
        .try_localize_message(message)
}
