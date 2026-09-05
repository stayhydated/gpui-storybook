use gpui_kit::component::{
    Disableable as _, Selectable as _,
    button::{Button, ButtonGroup},
    checkbox::Checkbox,
};
use gpui_kit::{
    Axis, Context, IntoElement, ParentElement as _, Styled as _, prelude::FluentBuilder as _,
};
use gpui_storybook::section;

use super::{ButtonControlState, ButtonStory};

pub(super) fn horizontal(state: ButtonControlState) -> impl IntoElement {
    section("Button Group").child(
        ButtonGroup::new("button-group")
            .outline()
            .disabled(state.disabled)
            .child(group_button("button-one", "One", state))
            .child(group_button("button-two", "Two", state))
            .child(group_button("button-three", "Three", state)),
    )
}

pub(super) fn vertical(state: ButtonControlState) -> impl IntoElement {
    section("Button Group (Vertical)").child(
        ButtonGroup::new("button-group-vertical")
            .outline()
            .layout(Axis::Vertical)
            .disabled(state.disabled)
            .child(group_button("button-one", "One", state))
            .child(group_button("button-two", "Two", state))
            .child(group_button("button-three", "Three", state)),
    )
}

pub(super) fn toggle(state: ButtonControlState, cx: &mut Context<ButtonStory>) -> impl IntoElement {
    section("Toggle Button Group")
        .sub_title(
            Checkbox::new("multiple-button")
                .text_sm()
                .label("Multiple")
                .checked(state.toggle_multiple)
                .on_click(cx.listener(|view, _, _, cx| {
                    view.toggle_multiple = !view.toggle_multiple;
                    cx.notify();
                })),
        )
        .child(
            ButtonGroup::new("toggle-button-group")
                .outline()
                .compact()
                .multiple(state.toggle_multiple)
                .child(
                    Button::new("disabled-toggle-button")
                        .label("Disabled")
                        .selected(state.disabled),
                )
                .child(
                    Button::new("loading-toggle-button")
                        .label("Loading")
                        .selected(state.loading),
                )
                .child(
                    Button::new("selected-toggle-button")
                        .label("Selected")
                        .selected(state.selected),
                )
                .child(
                    Button::new("compact-toggle-button")
                        .label("Compact")
                        .selected(state.compact),
                )
                .on_click(cx.listener(|view, selected: &Vec<usize>, _, cx| {
                    view.disabled = selected.contains(&0);
                    view.loading = selected.contains(&1);
                    view.selected = selected.contains(&2);
                    view.compact = selected.contains(&3);
                    cx.notify();
                })),
        )
}

fn group_button(id: &'static str, label: &'static str, state: ButtonControlState) -> Button {
    Button::new(id)
        .label(label)
        .disabled(state.disabled)
        .selected(state.selected)
        .when(state.compact, |this| this.compact())
        .on_click(ButtonStory::on_click)
}
