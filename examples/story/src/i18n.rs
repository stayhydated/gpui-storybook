use es_fluent::EsFluent;
use es_fluent_lang::es_fluent_language;
use strum::EnumIter;

es_fluent_manager_embedded::define_i18n_module!();

#[es_fluent_language]
#[derive(Clone, Copy, Debug, EnumIter, EsFluent, PartialEq)]
pub enum Languages {}

/// Applies Storybook's resolved locale to this example's GPUI Fluent manager.
pub fn apply_locale(
    language: Languages,
    cx: &mut gpui::App,
) -> Result<(), gpui_es_fluent::EmbeddedInitError> {
    let _linked_module = &GPUI_STORYBOOK_EXAMPLE_STORY_I18N_MODULE;
    gpui_es_fluent::replace_with_language(cx, language)
}
