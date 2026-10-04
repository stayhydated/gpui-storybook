use es_fluent::{
    FluentArgs, FluentLocalizer, FluentLocalizerExt as _, registry::StaticFluentMessageKey,
};
use es_fluent_manager_core::{I18nModule as _, Localizer};
use es_fluent_manager_embedded::{EmbeddedInitError, LocalizationError};
use gpui_kit::{App, Global};
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
        key: StaticFluentMessageKey,
        args: Option<&FluentArgs<'a>>,
    ) -> Option<String> {
        self.localizer.localize(key, args.map(FluentArgs::as_raw))
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

/// Attempts to localize a message through Storybook's embedded shell context.
///
/// Consumer messages use the separate application localization context.
///
/// Returns `None` when localization has not been initialized or the message
/// cannot be resolved for the active locale.
pub fn try_localize_message<T>(cx: &impl Borrow<App>, message: &T) -> Option<String>
where
    T: es_fluent::FluentMessage + ?Sized,
{
    cx.borrow()
        .try_global::<StorybookI18n>()?
        .try_localize_message(message)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::messages::StorybookMessage;
    use es_fluent::{FluentMessage, FluentMessageLookup};
    use gpui_kit::TestAppContext;

    struct MissingMessage;

    impl FluentMessage for MissingMessage {
        fn to_fluent_string_with(&self, lookup: &mut FluentMessageLookup<'_>) -> String {
            lookup(
                es_fluent::registry::__macro::static_message_key(
                    "gpui-storybook-core",
                    es_fluent::registry::__macro::static_domain("gpui-storybook-core"),
                    es_fluent::registry::__macro::static_entry_id("missing-test-message"),
                ),
                None,
            )
        }
    }

    #[test]
    fn fallible_shell_lookup_preserves_missing_locale_and_context_contracts() {
        let app = TestAppContext::single();
        app.update(|cx| {
            assert_eq!(
                try_localize_message(cx, &StorybookMessage::Appearance),
                None
            );
            init(cx).expect("embedded shell resources should initialize");
            assert_eq!(
                try_localize_message(cx, &StorybookMessage::Appearance),
                Some("Appearance".to_string())
            );
            assert_eq!(try_localize_message(cx, &MissingMessage), None);
            assert_eq!(
                gpui_es_fluent::try_localize_message(cx, &StorybookMessage::Appearance),
                None,
                "initializing the shell must not install the consumer context"
            );

            change_locale(cx, "zz".parse::<LanguageIdentifier>().unwrap())
                .expect("an unavailable shell locale should use English");
            assert_eq!(
                try_localize_message(cx, &StorybookMessage::Appearance),
                Some("Appearance".to_string())
            );
        });
    }
}
