use gpui::{IntoElement, ParentElement as _, Styled as _, prelude::FluentBuilder as _};
use gpui_component::{
    Disableable as _, Icon, IconName, Selectable as _,
    button::{Button, ButtonVariants as _},
    h_flex,
};
use gpui_storybook::section;

use super::{ButtonControlState, ButtonStory, ButtonSubstory};

pub(super) fn with_icon(state: ButtonControlState) -> impl IntoElement {
    section(ButtonSubstory::ButtonWithIcon)
        .child(
            button("button-icon-1", "Confirm", state)
                .outline()
                .icon(IconName::Check),
        )
        .child(
            button("button-icon-2", "Abort", state)
                .outline()
                .icon(IconName::Close),
        )
        .child(
            button("button-icon-3", "Maximize", state)
                .outline()
                .icon(Icon::new(IconName::Maximize)),
        )
        .child(
            Button::new("button-icon-4")
                .child(
                    h_flex()
                        .items_center()
                        .gap_2()
                        .child("Custom Child")
                        .child(IconName::ChevronDown)
                        .child(IconName::Eye),
                )
                .disabled(state.disabled)
                .selected(state.selected)
                .loading(state.loading)
                .when(state.compact, |this| this.compact())
                .on_click(ButtonStory::on_click),
        )
        .child(
            button("button-icon-5-ghost", "Confirm", state)
                .ghost()
                .icon(IconName::Check),
        )
        .child(
            button("button-icon-6-link", "Link", state)
                .link()
                .icon(IconName::Check),
        )
        .child(
            button("button-icon-6-text", "Text Button", state)
                .text()
                .icon(IconName::Check),
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
