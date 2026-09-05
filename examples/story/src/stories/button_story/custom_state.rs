use gpui_kit::component::{
    ActiveTheme as _, Disableable as _, IconName, Selectable as _,
    button::{Button, ButtonCustomVariant, ButtonVariants as _},
};
use gpui_kit::{Context, IntoElement, ParentElement as _, prelude::FluentBuilder as _};
use gpui_storybook::section;

use super::{ButtonControlState, ButtonStory};

pub(super) fn custom(state: ButtonControlState, cx: &mut Context<ButtonStory>) -> impl IntoElement {
    let custom_variant = ButtonCustomVariant::new(cx)
        .color(cx.theme().magenta)
        .foreground(cx.theme().magenta)
        .hover(cx.theme().magenta.opacity(0.1))
        .active(cx.theme().magenta);

    section("Custom Button")
        .child(button("button-6-custom", "Custom Button", state).custom(custom_variant))
        .child(
            button("button-outline-6-custom", "Outline Button", state)
                .outline()
                .custom(custom_variant),
        )
        .child(
            button("button-outline-6-custom-1", "Icon Button", state)
                .outline()
                .icon(IconName::Bell)
                .custom(custom_variant),
        )
}

fn button(id: &'static str, label: &'static str, state: ButtonControlState) -> Button {
    Button::new(id)
        .label(label)
        .disabled(state.disabled)
        .selected(state.selected)
        .loading(state.loading)
        .when(state.compact, |this| this.compact())
        .on_click(ButtonStory::on_click)
}
