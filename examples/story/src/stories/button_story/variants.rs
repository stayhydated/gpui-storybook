use gpui_kit::component::{
    Disableable as _, Selectable as _, Sizable as _,
    button::{Button, ButtonVariants as _},
};
use gpui_kit::{Context, IntoElement, ParentElement as _, prelude::FluentBuilder as _};
use gpui_storybook::section;

use super::{ButtonControlState, ButtonStory, ButtonSubstory};

pub(super) fn normal(state: ButtonControlState, _: &mut Context<ButtonStory>) -> impl IntoElement {
    section(ButtonSubstory::NormalButton)
        .max_w_lg()
        .child(normal_button("button-0", "Default", state))
        .child(normal_button("button-1", "Primary", state).primary())
        .child(normal_button("button-2", "Secondary", state).secondary())
        .child(normal_button("button-4", "Danger", state).danger())
        .child(normal_button("button-4-warning", "Warning", state).warning())
        .child(normal_button("button-4-success", "Success", state).success())
        .child(normal_button("button-5-info", "Info", state).info())
        .child(normal_button("button-5-ghost", "Ghost", state).ghost())
        .child(normal_button("button-5-link", "Link", state).link())
        .child(normal_button("button-5-text", "Text", state).text())
}

pub(super) fn outline(state: ButtonControlState) -> impl IntoElement {
    section("Outline Button")
        .max_w_lg()
        .child(
            button("button-outline-1", "Primary Button", state)
                .primary()
                .outline(),
        )
        .child(button("button-outline-2", "Normal Button", state).outline())
        .child(
            button("button-outline-4-danger", "Danger Button", state)
                .danger()
                .outline(),
        )
        .child(
            button("button-outline-4-warning", "Warning Button", state)
                .warning()
                .outline(),
        )
        .child(
            button("button-outline-4-success", "Success Button", state)
                .success()
                .outline(),
        )
        .child(
            button("button-outline-5-info", "Info Button", state)
                .info()
                .outline(),
        )
        .child(
            button("button-outline-5-ghost", "Ghost Button", state)
                .ghost()
                .outline(),
        )
        .child(
            button("button-outline-5-link", "Link Button", state)
                .link()
                .outline(),
        )
        .child(
            button("button-outline-5-text", "Text Button", state)
                .text()
                .outline(),
        )
}

pub(super) fn dropdown_caret(state: ButtonControlState) -> impl IntoElement {
    section("With Dropdown Caret")
        .max_w_lg()
        .child(
            button("button-outline-1", "Primary Button", state)
                .primary()
                .dropdown_caret(true),
        )
        .child(button("button-outline-2", "Default Button", state).dropdown_caret(true))
        .child(
            button("button-outline-2", "Secondary Button", state)
                .secondary()
                .dropdown_caret(true),
        )
        .child(
            button("button-outline-5-ghost", "Ghost Button", state)
                .ghost()
                .dropdown_caret(true),
        )
        .child(
            button("button-outline-5-link", "Link Button", state)
                .link()
                .dropdown_caret(true),
        )
        .child(
            button("button-outline-5-text", "Small Button", state)
                .outline()
                .small()
                .dropdown_caret(true),
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

fn normal_button(id: &'static str, label: &'static str, state: ButtonControlState) -> Button {
    button(id, label, state).on_hover(ButtonStory::on_hover)
}
