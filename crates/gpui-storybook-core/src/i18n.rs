use gpui::App;
use std::borrow::Borrow;
use unic_langid::LanguageIdentifier;

pub use gpui_es_fluent::I18n;

/// Initializes the embedded localization resources owned by Storybook.
///
/// Application-facing setup normally goes through `gpui_storybook::init`,
/// which calls this function while installing the application's typed locale
/// manager and the rest of the Storybook runtime.
pub fn init(cx: &mut App) -> Result<(), gpui_es_fluent::EmbeddedInitError> {
    gpui_es_fluent::init(cx)
}

/// Changes the active embedded locale.
///
/// # Errors
///
/// Returns an error when the requested locale is unavailable or cannot be
/// installed by the embedded localization manager.
pub fn change_locale<L>(cx: &mut App, locale: L) -> Result<(), gpui_es_fluent::LocalizationError>
where
    L: Into<LanguageIdentifier>,
{
    gpui_es_fluent::change_locale(cx, locale)
}

/// Localizes a Fluent message with the manager stored in the GPUI app context.
///
/// Returns `None` when localization has not been initialized or the message
/// cannot be resolved for the active locale.
pub fn localize_message<T>(cx: &impl Borrow<App>, message: &T) -> Option<String>
where
    T: es_fluent::FluentMessage + ?Sized,
{
    gpui_es_fluent::try_localize_message(cx, message)
}
