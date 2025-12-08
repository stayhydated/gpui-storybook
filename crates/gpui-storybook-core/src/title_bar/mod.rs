mod font_size_selector;
mod locale_selector;

use self::locale_selector::LocaleSelector;
use crate::story::themes::{SwitchTheme, SwitchThemeMode};
use font_size_selector::FontSizeSelector;
use gpui::{
    AnyElement, App, AppContext as _, Context, Entity, InteractiveElement as _, IntoElement,
    MouseButton, ParentElement as _, Render, SharedString, Styled as _, Window, div,
};
use gpui_component::{
    ActiveTheme as _, IconName, Sizable as _, ThemeMode, ThemeRegistry, TitleBar,
    button::{Button, ButtonVariants as _},
    menu::DropdownMenu as _,
};
use std::rc::Rc;

pub struct AppTitleBar {
    title: SharedString,
    font_size_selector: Entity<FontSizeSelector>,
    locale_selector: Entity<LocaleSelector>,
    child: Rc<dyn Fn(&mut Window, &mut App) -> AnyElement>,
}

impl AppTitleBar {
    pub fn new(
        title: impl Into<SharedString>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let font_size_selector = cx.new(|cx| FontSizeSelector::new(window, cx));
        let locale_selector = cx.new(|cx| LocaleSelector::new(window, cx));

        Self {
            title: title.into(),
            font_size_selector,
            locale_selector,
            child: Rc::new(|_, _| div().into_any_element()),
        }
    }

    pub fn child<F, E>(mut self, f: F) -> Self
    where
        E: IntoElement,
        F: Fn(&mut Window, &mut App) -> E + 'static,
    {
        self.child = Rc::new(move |window, cx| f(window, cx).into_any_element());
        self
    }
}

impl Render for AppTitleBar {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let registry = ThemeRegistry::global(cx);
        let mut dropdown_theme_names: Vec<_> = registry.themes().keys().cloned().collect();
        dropdown_theme_names.sort();
        let dropdown_current_theme = cx.theme().theme_name().clone();
        let dropdown_is_dark = cx.theme().mode.is_dark();

        TitleBar::new()
            .child(div().flex().items_center().child(self.title.clone()))
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_end()
                    .px_2()
                    .gap_2()
                    .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                    .child((self.child.clone())(window, cx))
                    .child(
                        Button::new("theme-menu")
                            .small()
                            .ghost()
                            .icon(IconName::Palette)
                            .dropdown_menu(move |mut this, _, _| {
                                this = this.label("Theme");
                                for theme_name in &dropdown_theme_names {
                                    let checked = theme_name == &dropdown_current_theme;
                                    this = this.menu_with_check(
                                        theme_name.clone(),
                                        checked,
                                        Box::new(SwitchTheme(theme_name.clone())),
                                    );
                                }
                                this.separator()
                                    .label("Mode")
                                    .menu_with_check(
                                        "Light",
                                        !dropdown_is_dark,
                                        Box::new(SwitchThemeMode(ThemeMode::Light)),
                                    )
                                    .menu_with_check(
                                        "Dark",
                                        dropdown_is_dark,
                                        Box::new(SwitchThemeMode(ThemeMode::Dark)),
                                    )
                            }),
                    )
                    .child(self.font_size_selector.clone())
                    .child(self.locale_selector.clone()),
            )
    }
}
