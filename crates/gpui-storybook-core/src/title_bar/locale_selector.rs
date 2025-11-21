use crate::{locale::LocaleStore, story::SelectLocale};
use gpui::{
    BorrowAppContext as _, Context, Corner, FocusHandle, InteractiveElement as _, IntoElement,
    ParentElement as _, Render, Window, div,
};
use gpui_component::{
    IconName, Sizable as _,
    button::{Button, ButtonVariants as _},
    menu::DropdownMenu as _,
};

pub struct LocaleSelector {
    focus_handle: FocusHandle,
}

impl LocaleSelector {
    pub fn new(_: &mut Window, cx: &mut Context<Self>) -> Self {
        Self {
            focus_handle: cx.focus_handle(),
        }
    }

    fn on_select_locale(
        &mut self,
        locale: &SelectLocale,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        cx.update_global::<Box<dyn LocaleStore>, _>(|locale_store, cx| {
            locale_store.set_current_locale(locale.0.clone(), cx);
        });
        window.refresh();
    }
}

impl Render for LocaleSelector {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let focus_handle = self.focus_handle.clone();
        let locale_store = cx.global::<Box<dyn LocaleStore>>();
        let available_locales = locale_store.available_locales();
        let current_language = locale_store.current_locale(cx);

        div()
            .id("locale-selector")
            .track_focus(&focus_handle)
            .on_action(cx.listener(Self::on_select_locale))
            .child(
                Button::new("btn")
                    .small()
                    .ghost()
                    .icon(IconName::Globe)
                    .dropdown_menu(move |mut this, _, _| {
                        for (name, lang_id) in &available_locales {
                            let checked = *lang_id == current_language;
                            this = this.menu_with_check(
                                name,
                                checked,
                                Box::new(SelectLocale(lang_id.to_owned())),
                            );
                        }
                        this
                    })
                    .anchor(Corner::TopRight),
            )
    }
}
