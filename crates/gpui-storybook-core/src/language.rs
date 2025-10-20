use es_fluent::FluentDisplay;
use gpui::Global;
use strum::IntoEnumIterator;
use unic_langid::LanguageIdentifier;

pub trait Language:
    'static + Copy + Clone + Send + Sync + IntoEnumIterator + Into<LanguageIdentifier> + FluentDisplay
{
}

impl<T> Language for T where
    T: 'static
        + Copy
        + Clone
        + Send
        + Sync
        + IntoEnumIterator
        + Into<LanguageIdentifier>
        + FluentDisplay
{
}

#[derive(Clone, Copy)]
pub struct CurrentLanguage<L: Language>(pub L);

impl<L: Language> Global for CurrentLanguage<L> {}
