use gpui_kit::component::{
    ActiveTheme as _, IconName, Side, Sizable as _, Theme,
    button::{Button, ButtonVariants as _},
    menu::DropdownMenu as _,
    scroll::ScrollbarMode,
};
use gpui_kit::{
    Anchor, Context, FocusHandle, InteractiveElement as _, IntoElement, ParentElement as _, Render,
    Window, div, px,
};

use crate::actions::{SelectFont, SelectRadius, SelectScrollbarMode};

pub struct FontSizeSelector {
    focus_handle: FocusHandle,
}

impl FontSizeSelector {
    pub fn new(_: &mut Window, cx: &mut Context<Self>) -> Self {
        Self {
            focus_handle: cx.focus_handle(),
        }
    }

    fn on_select_font(
        &mut self,
        font_size: &SelectFont,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        Theme::global_mut(cx).font_size = px(font_size.0 as f32);
        window.refresh();
    }

    fn on_select_radius(
        &mut self,
        radius: &SelectRadius,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        Theme::global_mut(cx).radius = px(radius.0 as f32);
        window.refresh();
    }

    fn on_select_scrollbar_mode(
        &mut self,
        mode: &SelectScrollbarMode,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let preference = match mode.0 {
            ScrollbarMode::Scrolling => gpui_storybook_preferences::PreferredScrollbar::Scrolling,
            ScrollbarMode::Hover => gpui_storybook_preferences::PreferredScrollbar::Hover,
            ScrollbarMode::Always => gpui_storybook_preferences::PreferredScrollbar::Always,
        };
        crate::preferences::select_scrollbar(preference, cx);
        window.refresh();
    }
}

impl Render for FontSizeSelector {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let focus_handle = self.focus_handle.clone();
        let font_size = cx.theme().font_size.as_f32() as i32;
        let radius = cx.theme().radius.as_f32() as i32;
        let scrollbar_mode = cx.theme().scrollbar_mode;

        div()
            .id("font-size-selector")
            .debug_selector(|| "storybook-settings".to_owned())
            .track_focus(&focus_handle)
            .on_action(cx.listener(Self::on_select_font))
            .on_action(cx.listener(Self::on_select_radius))
            .on_action(cx.listener(Self::on_select_scrollbar_mode))
            .child(
                Button::new("btn")
                    .small()
                    .ghost()
                    .icon(IconName::Settings2)
                    .dropdown_menu(move |this, _, _| {
                        this.scrollable(true)
                            .check_side(Side::Right)
                            .max_h(px(480.))
                            .label("Font Size")
                            .menu_with_check("Large", font_size == 18, Box::new(SelectFont(18)))
                            .menu_with_check(
                                "Medium (default)",
                                font_size == 16,
                                Box::new(SelectFont(16)),
                            )
                            .menu_with_check("Small", font_size == 14, Box::new(SelectFont(14)))
                            .separator()
                            .label("Border Radius")
                            .menu_with_check("8px", radius == 8, Box::new(SelectRadius(8)))
                            .menu_with_check(
                                "6px (default)",
                                radius == 6,
                                Box::new(SelectRadius(6)),
                            )
                            .menu_with_check("4px", radius == 4, Box::new(SelectRadius(4)))
                            .menu_with_check("0px", radius == 0, Box::new(SelectRadius(0)))
                            .separator()
                            .label("Scrollbar")
                            .menu_with_check(
                                "Scrolling to show",
                                scrollbar_mode == ScrollbarMode::Scrolling,
                                Box::new(SelectScrollbarMode(ScrollbarMode::Scrolling)),
                            )
                            .menu_with_check(
                                "Hover to show",
                                scrollbar_mode == ScrollbarMode::Hover,
                                Box::new(SelectScrollbarMode(ScrollbarMode::Hover)),
                            )
                            .menu_with_check(
                                "Always show",
                                scrollbar_mode == ScrollbarMode::Always,
                                Box::new(SelectScrollbarMode(ScrollbarMode::Always)),
                            )
                    })
                    .anchor(Anchor::TopRight),
            )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui_kit::{App, AppContext as _};

    #[gpui_kit::test]
    fn selector_actions_update_theme_settings(cx: &mut App) {
        gpui_kit::init(cx);
        let window: gpui_kit::WindowHandle<FontSizeSelector> = cx
            .open_window(Default::default(), |window, cx| {
                cx.new(|cx| FontSizeSelector::new(window, cx))
            })
            .expect("selector window should open");

        window
            .update(cx, |selector, window, cx| {
                selector.on_select_font(&SelectFont(18), window, cx);
                assert_eq!(cx.theme().font_size, px(18.));

                selector.on_select_radius(&SelectRadius(8), window, cx);
                assert_eq!(cx.theme().radius, px(8.));
            })
            .expect("selector should update");
    }
}
