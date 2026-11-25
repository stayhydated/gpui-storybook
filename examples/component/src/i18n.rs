use iso_types::iso_locale;

es_fluent_manager_embedded::define_i18n_module!();

// Preserve the native language names used by the Storybook language picker.
#[iso_locale(custom)]
pub enum Languages {}

/// Applies Storybook's resolved locale to this example's GPUI Fluent manager.
pub fn apply_locale(
    language: Languages,
    cx: &mut gpui_kit::App,
) -> Result<(), gpui_es_fluent::EmbeddedInitError> {
    let _linked_module = &GPUI_STORYBOOK_EXAMPLE_COMPONENT_I18N_MODULE;
    gpui_es_fluent::replace_with_language(cx, language)
}

#[cfg(test)]
mod tests {
    use super::*;
    use es_fluent::FluentLocalizerExt as _;
    use gpui_es_fluent::I18n;
    use iso_types::VariantArray as _;

    #[test]
    fn embedded_language_picker_preserves_autonyms() {
        assert_eq!(Languages::default(), Languages::En);
        assert_eq!(Languages::VARIANTS.len(), 3);
        for (current, title) in [
            (Languages::En, "Title"),
            (Languages::Fr, "Titre"),
            (Languages::Zh, "标题"),
        ] {
            // Use the same selection policy as apply_locale: the independent
            // shell catalog need not contain every locale the example supports.
            let i18n = I18n::new_with_language(current)
                .expect("example ships resources for all three languages");
            assert_eq!(
                i18n.try_localize_message(&crate::StoryItems::Title)
                    .as_deref(),
                Some(title)
            );
            assert_eq!(i18n.localize_message(&Languages::En), "English");
            assert_eq!(i18n.localize_message(&Languages::Fr), "français");
            assert_eq!(i18n.localize_message(&Languages::Zh), "中文");
        }
    }
}
