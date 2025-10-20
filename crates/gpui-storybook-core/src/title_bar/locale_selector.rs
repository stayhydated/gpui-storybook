use crate::{
    language::{CurrentLanguage, Language},
    story::SelectLocale,
};
use es_fluent::ToFluentString as _;
use gpui::{
    Context, Corner, FocusHandle, InteractiveElement as _, IntoElement, ParentElement as _, Render,
    Window, div,
};
use gpui_component::{
    IconName, Sizable as _,
    button::{Button, ButtonVariants as _},
    popup_menu::PopupMenuExt as _,
    set_locale,
};
use std::marker::PhantomData;
use strum::IntoEnumIterator as _;
use unic_langid::LanguageIdentifier;

pub struct LocaleSelector<L: Language> {
    focus_handle: FocusHandle,
    _phantom: PhantomData<L>,
}

impl<L: Language> LocaleSelector<L> {
    pub fn new(_: &mut Window, cx: &mut Context<Self>) -> Self {
        Self {
            focus_handle: cx.focus_handle(),
            _phantom: PhantomData,
        }
    }

    fn on_select_locale(
        &mut self,
        locale: &SelectLocale,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        set_locale(&locale.0.to_string());
        crate::i18n::change_locale(locale.0.clone());
        window.refresh();
    }
}

impl<L: Language> Render for LocaleSelector<L> {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let focus_handle = self.focus_handle.clone();
        let current_language = cx.global::<CurrentLanguage<L>>().0;

        div()
            .id("locale-selector")
            .track_focus(&focus_handle)
            .on_action(cx.listener(Self::on_select_locale))
            .child(
                Button::new("btn")
                    .small()
                    .ghost()
                    .icon(IconName::Globe)
                    .popup_menu(move |mut this, _, _| {
                        for lang in L::iter() {
                            let lang_id: LanguageIdentifier = lang.into();
                            println!("{}", &lang_id);
                            let checked = lang_id == current_language.into();
                            println!("{}", &current_language.into());
                            this = this.menu_with_check(
                                lang.to_fluent_string(),
                                checked,
                                Box::new(SelectLocale(lang_id)),
                            );
                        }
                        this
                    })
                    .anchor(Corner::TopRight),
            )
    }
}
