use gpui::App;
use std::borrow::Borrow;
use unic_langid::LanguageIdentifier;

pub use gpui_es_fluent::I18n;

pub fn init(cx: &mut App) -> Result<(), gpui_es_fluent::EmbeddedInitError> {
    gpui_es_fluent::init(cx)
}

pub fn change_locale<L>(cx: &mut App, locale: L) -> Result<(), gpui_es_fluent::LocalizationError>
where
    L: Into<LanguageIdentifier>,
{
    gpui_es_fluent::change_locale(cx, locale)
}

pub fn localize_message<T>(cx: &impl Borrow<App>, message: &T) -> Option<String>
where
    T: es_fluent::FluentMessage + ?Sized,
{
    gpui_es_fluent::try_localize_message(cx, message)
}
