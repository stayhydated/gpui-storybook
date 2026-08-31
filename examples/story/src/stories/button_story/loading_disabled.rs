use gpui::{Context, IntoElement, ParentElement as _, Styled as _};
use gpui_component::{
    ActiveTheme as _, Sizable as _,
    button::{Button, ButtonVariants as _},
    checkbox::Checkbox,
    h_flex,
    progress::ProgressCircle,
};
use gpui_storybook::section;

use super::{ButtonControlState, ButtonStory, ButtonSubstory};

pub(super) fn controls(
    state: ButtonControlState,
    cx: &mut Context<ButtonStory>,
) -> impl IntoElement {
    h_flex()
        .gap_3()
        .child(
            Checkbox::new("disabled-button")
                .label("Disabled")
                .checked(state.disabled)
                .on_click(cx.listener(|view, _, _, cx| {
                    view.disabled = !view.disabled;
                    cx.notify();
                })),
        )
        .child(
            Checkbox::new("loading-button")
                .label("Loading")
                .checked(state.loading)
                .on_click(cx.listener(|view, _, _, cx| {
                    view.loading = !view.loading;
                    cx.notify();
                })),
        )
        .child(
            Checkbox::new("selected-button")
                .label("Selected")
                .checked(state.selected)
                .on_click(cx.listener(|view, _, _, cx| {
                    view.selected = !view.selected;
                    cx.notify();
                })),
        )
        .child(
            Checkbox::new("compact-button")
                .label("Compact")
                .checked(state.compact_control)
                .on_click(cx.listener(|view, _, _, cx| {
                    view.compact = !view.compact;
                    cx.notify();
                })),
        )
        .child(
            Checkbox::new("shadow-button")
                .label("Shadow")
                .checked(cx.theme().shadow)
                .on_click(cx.listener(|_, _, window, cx| {
                    let mut theme = cx.theme().clone();
                    theme.shadow = !theme.shadow;
                    cx.set_global::<gpui_component::Theme>(theme);
                    window.refresh();
                })),
        )
}

pub(super) fn with_progress(cx: &mut Context<ButtonStory>) -> impl IntoElement {
    section(ButtonSubstory::WithProgress).child(
        h_flex()
            .gap_4()
            .child(
                Button::new("progress-button-1")
                    .primary()
                    .large()
                    .icon(
                        ProgressCircle::new("circle-progress-1")
                            .color(cx.theme().primary_foreground)
                            .value(25.),
                    )
                    .label("Installing..."),
            )
            .child(
                Button::new("progress-button-2")
                    .icon(ProgressCircle::new("circle-progress-2").value(35.))
                    .label("Installing..."),
            )
            .child(
                Button::new("progress-button-3")
                    .small()
                    .icon(ProgressCircle::new("circle-progress-3").value(68.))
                    .label("Installing..."),
            )
            .child(
                Button::new("progress-button-4")
                    .xsmall()
                    .icon(ProgressCircle::new("circle-progress-4").value(85.))
                    .label("Installing..."),
            ),
    )
}
