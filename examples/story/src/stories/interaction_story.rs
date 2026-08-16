use gpui::{
    Action, App, AppContext as _, Context, Entity, Focusable, InteractiveElement as _, IntoElement,
    ParentElement as _, Render, StatefulInteractiveElement as _, Styled as _, Subscription, Window,
    div, px,
};
use gpui_component::{
    ActiveTheme as _, IndexPath, h_flex,
    input::{Input, InputState},
    select::{Select, SelectEvent, SelectState},
    v_flex,
};
use schemars::JsonSchema;
use serde::Deserialize;

/// Sets the inert status text displayed by the interaction automation fixture.
#[derive(Action, Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq)]
#[action(namespace = interaction_story)]
pub struct SetAutomationStatus {
    /// New local fixture status.
    pub value: String,
}

/// Deterministic controls for exercising in-process MCP interaction steps.
#[derive(gpui_storybook::StoryControls)]
#[gpui_storybook::story(crate::StorySection::Automation)]
pub struct InteractionStory {
    input: Entity<InputState>,
    select: Entity<SelectState<Vec<&'static str>>>,
    #[storybook(control(category = "Fixture"))]
    prefix: String,
    status: String,
    hovered: bool,
    clicks: usize,
    _subscriptions: Vec<Subscription>,
}

impl gpui_storybook::Story for InteractionStory {
    fn title(_: &App) -> String {
        "Interaction automation".to_owned()
    }

    fn description(_: &App) -> String {
        "Inert focus, text, select, pointer, action, viewport, and next-frame capture targets."
            .to_owned()
    }

    fn new_view(window: &mut Window, cx: &mut App) -> Entity<Self> {
        let input = cx.new(|cx| InputState::new(window, cx).placeholder("Type Unicode text"));
        let select = cx.new(|cx| {
            SelectState::new(
                vec!["Alpha", "Beta", "Gamma"],
                Some(IndexPath::default()),
                window,
                cx,
            )
        });

        cx.new(|cx| {
            let selection_subscription = cx.subscribe(
                &select,
                |this: &mut Self, _, event: &SelectEvent<Vec<&'static str>>, cx| {
                    if let SelectEvent::Confirm(Some(value)) = event {
                        this.status = format!("selected:{value}");
                        cx.notify();
                    }
                },
            );
            Self {
                input,
                select,
                prefix: "fixture".to_owned(),
                status: "idle".to_owned(),
                hovered: false,
                clicks: 0,
                _subscriptions: vec![selection_subscription],
            }
        })
    }
}

impl Focusable for InteractionStory {
    fn focus_handle(&self, cx: &App) -> gpui::FocusHandle {
        self.input.focus_handle(cx)
    }
}

impl Render for InteractionStory {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let selected = self
            .select
            .read(cx)
            .selected_value()
            .copied()
            .unwrap_or("none");
        let input_value = self.input.read(cx).value();
        let viewport = window.viewport_size();

        v_flex()
            .id("interaction-automation-fixture")
            .w_full()
            .max_w(px(720.0))
            .gap_4()
            .on_action(cx.listener(|this, action: &SetAutomationStatus, _, cx| {
                this.status = action.value.clone();
                cx.notify();
            }))
            .child("Deterministic in-process interaction target")
            .child(Input::new(&self.input))
            .child(Select::new(&self.select).placeholder("Choose a fixture value"))
            .child(gpui_storybook::interaction_target(
                "pointer-target",
                "Pointer target",
                div()
                    .id("interaction-pointer-target")
                    .w_full()
                    .h(px(96.0))
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded(cx.theme().radius)
                    .border_1()
                    .border_color(cx.theme().border)
                    .bg(if self.hovered {
                        cx.theme().accent
                    } else {
                        cx.theme().muted
                    })
                    .on_hover(cx.listener(|this, hovered, _, cx| {
                        this.hovered = *hovered;
                        cx.notify();
                    }))
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.clicks += 1;
                        this.status = "pressed".to_owned();
                        let this = cx.entity().downgrade();
                        window.on_next_frame(move |_, cx| {
                            let _ = this.update(cx, |this, cx| {
                                this.status = "idle".to_owned();
                                cx.notify();
                            });
                        });
                        cx.notify();
                    }))
                    .child("Pointer target"),
            ))
            .child(
                h_flex()
                    .gap_4()
                    .child(format!("{} status:{}", self.prefix, self.status))
                    .child(format!("hovered:{}", self.hovered))
                    .child(format!("clicks:{}", self.clicks)),
            )
            .child(format!("input:{input_value}"))
            .child(format!("selected:{selected}"))
            .child(format!(
                "viewport:{}x{}",
                f32::from(viewport.width),
                f32::from(viewport.height)
            ))
    }
}
