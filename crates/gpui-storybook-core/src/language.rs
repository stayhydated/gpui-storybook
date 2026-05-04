use es_fluent::FluentMessage;
use gpui::Global;
use strum::IntoEnumIterator;
use unic_langid::LanguageIdentifier;

pub trait Language:
    'static
    + Copy
    + Clone
    + Send
    + Sync
    + IntoEnumIterator
    + TryInto<LanguageIdentifier>
    + TryFrom<LanguageIdentifier>
    + FluentMessage
    + Default
    + std::fmt::Debug
{
}

impl<T> Language for T where
    T: 'static
        + Copy
        + Clone
        + Send
        + Sync
        + IntoEnumIterator
        + TryInto<LanguageIdentifier>
        + TryFrom<LanguageIdentifier>
        + FluentMessage
        + Default
        + std::fmt::Debug
{
}

#[derive(Clone, Copy)]
pub struct CurrentLanguage<L: Language>(pub L);

impl<L: Language> Global for CurrentLanguage<L> {}
