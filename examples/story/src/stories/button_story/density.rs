use gpui_kit::component::{
    Disableable as _, IconName, Selectable as _, Sizable as _,
    button::{Button, ButtonVariants as _},
};
use gpui_kit::{IntoElement, ParentElement as _, Styled as _, prelude::FluentBuilder as _, px};
use gpui_storybook::section;

use super::{ButtonControlState, ButtonStory};

pub(super) fn small(state: ButtonControlState) -> impl IntoElement {
    section("Small Size")
        .child(
            action_button("button-6", "Primary Button", state)
                .icon(IconName::Check)
                .primary()
                .small(),
        )
        .child(action_button("button-7", "Secondary Button", state).small())
        .child(
            action_button("button-8", "Danger Button", state)
                .danger()
                .small(),
        )
        .child(
            action_button("button-8-outline", "Outline Button", state)
                .outline()
                .small(),
        )
        .child(
            action_button("button-8-ghost", "Ghost Button", state)
                .ghost()
                .small(),
        )
        .child(
            action_button("button-8-link", "Link Button", state)
                .link()
                .small(),
        )
}

pub(super) fn xsmall(state: ButtonControlState) -> impl IntoElement {
    section("XSmall Size")
        .child(
            action_button("button-xs-1", "Primary Button", state)
                .primary()
                .icon(IconName::Check)
                .xsmall(),
        )
        .child(action_button("button-xs-2", "Secondary Button", state).xsmall())
        .child(
            action_button("button-xs-3", "Danger Button", state)
                .danger()
                .xsmall(),
        )
        .child(
            action_button("button-xs-3-ghost", "Ghost Button", state)
                .ghost()
                .xsmall(),
        )
        .child(
            action_button("button-xs-3-outline", "Outline Button", state)
                .outline()
                .xsmall(),
        )
        .child(
            action_button("button-xs-3-link", "Link Button", state)
                .link()
                .xsmall(),
        )
}

pub(super) fn icon_buttons(state: ButtonControlState) -> impl IntoElement {
    section("Icon Button")
        .child(
            icon_button("icon-button-primary", IconName::Search, state)
                .loading_icon(IconName::LoaderCircle)
                .primary(),
        )
        .child(icon_button("icon-button-secondary", IconName::Info, state))
        .child(icon_button("icon-button-danger", IconName::Close, state).danger())
        .child(
            icon_button("icon-button-small-primary", IconName::Search, state)
                .small()
                .primary(),
        )
        .child(icon_button("icon-button-outline", IconName::Search, state).outline())
        .child(
            icon_button("icon-button-ghost", IconName::ArrowLeft, state)
                .loading_icon(IconName::LoaderCircle)
                .ghost(),
        )
}

pub(super) fn small_icon_buttons(state: ButtonControlState) -> impl IntoElement {
    section("Icon Button")
        .child(icon_button("icon-button-4", IconName::Info, state).small())
        .child(
            icon_button("icon-button-5", IconName::Close, state)
                .small()
                .danger(),
        )
        .child(
            icon_button("icon-button-6", IconName::Search, state)
                .small()
                .primary(),
        )
        .child(icon_button("icon-button-7", IconName::Info, state).xsmall())
        .child(
            icon_button("icon-button-8", IconName::Close, state)
                .xsmall()
                .danger(),
        )
        .child(
            icon_button("icon-button-9", IconName::Heart, state)
                .size(px(24.))
                .ghost(),
        )
}

fn action_button(id: &'static str, label: &'static str, state: ButtonControlState) -> Button {
    Button::new(id)
        .label(label)
        .disabled(state.disabled)
        .selected(state.selected)
        .loading(state.loading)
        .when(state.compact, |this| this.compact())
        .on_click(ButtonStory::on_click)
}

fn icon_button(id: &'static str, icon: IconName, state: ButtonControlState) -> Button {
    Button::new(id)
        .icon(icon)
        .disabled(state.disabled)
        .selected(state.selected)
        .loading(state.loading)
        .when(state.compact, |this| this.compact())
}
