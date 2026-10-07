//! Production example views shared by desktop and Android embedding.

use gpui_kit::component::{
    ActiveTheme as _, Disableable as _, Sizable as _, Theme, ThemeMode,
    button::Button,
    input::{Input, InputEvent, InputState},
    v_flex,
};
use gpui_kit::{
    App, AppContext as _, Context, Entity, Focusable as _, InteractiveElement as _, IntoElement,
    ParentElement as _, Render, Styled as _, Subscription, Window, div,
};
use gpui_storybook_automation::*;
use gpui_storybook_automation_gpui::{
    EmbeddedRoot, EmbeddedRoute, GpuiHostAttachment,
    interaction::NoCaptureProvider,
    regions::{StorybookElementExt as _, capture_story_view_with_scroll},
};
use std::{collections::BTreeSet, rc::Rc};

#[derive(
    gpui_kit::Action, Clone, Debug, Default, serde::Deserialize, Eq, schemars::JsonSchema, PartialEq,
)]
#[action(namespace = embedded_demo)]
pub struct Increment;

pub const COUNTER_ROUTE: &str = "embedded-counter";
pub const NOTES_ROUTE: &str = "embedded-notes";

pub struct DemoRoot {
    route: &'static str,
    revision: u64,
    count: usize,
    enabled: bool,
    canvas_background: StoryCanvasBackground,
    note: Entity<InputState>,
    action_focus: gpui_kit::FocusHandle,
    subscriptions: Vec<Subscription>,
}

impl DemoRoot {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let note = cx.new(|cx| InputState::new(window, cx).placeholder("Write a note"));
        let subscription = cx.subscribe(&note, |root, _, event, cx| {
            if matches!(event, InputEvent::Change) {
                root.revision += 1;
                cx.notify();
            }
        });
        Self {
            route: COUNTER_ROUTE,
            revision: 0,
            count: 0,
            enabled: true,
            canvas_background: StoryCanvasBackground::Theme,
            note,
            action_focus: cx.focus_handle().tab_stop(true),
            subscriptions: vec![subscription],
        }
    }
    pub fn count(&self) -> usize {
        self.count
    }
    pub fn route(&self) -> &str {
        self.route
    }
    pub fn note(&self, cx: &App) -> String {
        self.note.read(cx).value().to_string()
    }
    fn increment(&mut self, cx: &mut Context<Self>) {
        if self.enabled {
            self.revision += 1;
            self.count = self.count.saturating_add(1);
            cx.notify();
        }
    }
}

pub fn capabilities() -> AutomationCapabilities {
    use AutomationCapability::*;
    AutomationCapabilities::new([
        Navigation,
        Controls,
        ControlMutation,
        SemanticValues,
        Focus,
        Keystrokes,
        TextInsertion,
        Actions,
        Pointer,
        Scroll,
        FrameWaits,
        SemanticTargets,
        FreshScenarios,
        Presentation,
    ])
}

pub fn catalog() -> Vec<StorySnapshot> {
    let counter = StoryScenario::new("increment-twice", "Increment twice")
        .step(("first increment".to_owned(), click("increment")))
        .step(("second increment".to_owned(), click("increment")))
        .postcondition(
            StoryInteractionPostcondition::new("public-state", serde_json::json!(2))
                .json_pointer("/count"),
        );
    let notes = StoryScenario::new("write-note", "Write a note")
        .step(("focus note".to_owned(), click("note-input")))
        .step((
            "write note".to_owned(),
            StoryInteractionStep::Text {
                value: "hello".to_owned(),
            },
        ))
        .postcondition(
            StoryInteractionPostcondition::new("public-state", serde_json::json!("hello"))
                .json_pointer("/note"),
        );
    [
        (COUNTER_ROUTE, "Counter", "DemoRoot", counter),
        (NOTES_ROUTE, "Notes", "DemoRoot", notes),
    ]
    .into_iter()
    .map(|(key, title, story_name, scenario)| {
        EmbeddedRoute::builder()
            .key(key.to_owned())
            .title(title.to_owned())
            .crate_name(env!("CARGO_PKG_NAME").to_owned())
            .story_name(story_name.to_owned())
            .source_file(file!().to_owned())
            .source_line(line!())
            .scenarios(vec![scenario])
            .build()
            .into()
    })
    .collect()
}

pub fn attach(
    root: &Entity<DemoRoot>,
    window: &Window,
    cx: &mut App,
) -> GpuiHostAttachment<DemoRoot> {
    GpuiHostAttachment::attach(
        root,
        catalog(),
        capabilities(),
        Rc::new(NoCaptureProvider),
        window,
        cx,
    )
    .expect("example routes have unique checked keys")
}

fn click(target: &str) -> StoryInteractionStep {
    StoryInteractionStep::ClickTarget {
        target_key: target.to_owned(),
        button: StoryMouseButton::Left,
        click_count: 1,
        modifiers: StoryModifiers::default(),
    }
}

impl EmbeddedRoot for DemoRoot {
    fn revision(&self, _: &App) -> u64 {
        self.revision
    }
    fn active_route(&self, _: &App) -> String {
        self.route.to_owned()
    }
    fn select_route(
        &mut self,
        route: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Result<(), StorybookAutomationError> {
        self.revision += 1;
        self.route = match route {
            COUNTER_ROUTE => COUNTER_ROUTE,
            NOTES_ROUTE => NOTES_ROUTE,
            _ => {
                return Err(StorybookAutomationError::StoryNotFound {
                    key: route.to_owned(),
                });
            },
        };
        if self.route == NOTES_ROUTE {
            self.note.focus_handle(cx).focus(window, cx);
        } else {
            self.action_focus.focus(window, cx);
        }
        cx.notify();
        Ok(())
    }
    fn action_scope(&self, route: &str, _: &App) -> BTreeSet<String> {
        if route == COUNTER_ROUTE {
            BTreeSet::from(["embedded_demo::Increment".to_owned()])
        } else {
            BTreeSet::new()
        }
    }
    fn control_catalog(&self, route: &str, _: &App) -> Vec<ControlSpec> {
        let (key, label, kind, default) = if route == COUNTER_ROUTE {
            (
                "enabled",
                "Enabled",
                ControlKind::Checkbox,
                ControlValue::Boolean(true),
            )
        } else {
            (
                "note",
                "Note",
                ControlKind::Text,
                ControlValue::Text(String::new()),
            )
        };
        vec![ControlSpec {
            key: key.to_owned(),
            label: label.to_owned(),
            description: String::new(),
            category: "Content".to_owned(),
            kind,
            default,
            bounds: ControlBounds::default(),
            options: Vec::new(),
        }]
    }
    fn read_control(&self, key: &str, cx: &App) -> Result<ControlValue, ControlError> {
        match key {
            "enabled" if self.route == COUNTER_ROUTE => Ok(ControlValue::Boolean(self.enabled)),
            "note" if self.route == NOTES_ROUTE => Ok(ControlValue::Text(self.note(cx))),
            _ => Err(ControlError::UnknownControl {
                key: key.to_owned(),
            }),
        }
    }
    fn set_control(
        &mut self,
        key: &str,
        value: ControlValue,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Result<(), ControlError> {
        match (key, value) {
            ("enabled", ControlValue::Boolean(value)) if self.route == COUNTER_ROUTE => {
                self.enabled = value
            },
            ("note", ControlValue::Text(value)) if self.route == NOTES_ROUTE => self
                .note
                .update(cx, |note, cx| note.set_value(value, window, cx)),
            (_, value) => {
                return Err(ControlError::InvalidValue {
                    key: key.to_owned(),
                    expected: if self.route == COUNTER_ROUTE {
                        "boolean"
                    } else {
                        "text"
                    },
                    actual: value.kind_name(),
                });
            },
        }
        self.revision += 1;
        cx.notify();
        Ok(())
    }
    fn reset_control(
        &mut self,
        key: Option<&str>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Result<(), ControlError> {
        let spec = self.control_catalog(self.route, cx).remove(0);
        if key.is_some_and(|key| key != spec.key) {
            return Err(ControlError::UnknownControl {
                key: key.unwrap().to_owned(),
            });
        }
        self.set_control(&spec.key, spec.default, window, cx)
    }
    fn apply_presentation(
        &mut self,
        presentation: StoryPresentation,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Result<(), StorybookAutomationError> {
        self.revision += 1;
        self.canvas_background = presentation.background;
        match presentation.background {
            StoryCanvasBackground::Light => Theme::change(ThemeMode::Light, Some(window), cx),
            StoryCanvasBackground::Dark => Theme::change(ThemeMode::Dark, Some(window), cx),
            StoryCanvasBackground::Theme | StoryCanvasBackground::Transparent => {},
        }
        cx.notify();
        Ok(())
    }
    fn recreate_fixture(
        &mut self,
        route: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Result<(), StorybookAutomationError> {
        self.revision += 1;
        if route == COUNTER_ROUTE {
            self.count = 0;
            self.enabled = true;
        } else if route == NOTES_ROUTE {
            self.note = cx.new(|cx| InputState::new(window, cx).placeholder("Write a note"));
            self.subscriptions = vec![cx.subscribe(&self.note, |root, _, event, cx| {
                if matches!(event, InputEvent::Change) {
                    root.revision += 1;
                    cx.notify();
                }
            })];
        } else {
            return Err(StorybookAutomationError::StoryNotFound {
                key: route.to_owned(),
            });
        }
        cx.notify();
        Ok(())
    }
}

impl Render for DemoRoot {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let public_state = serde_json::json!({ "route": self.route, "count": self.count, "enabled": self.enabled, "note": self.note(cx) });
        let content = v_flex()
            .id("embedded-demo")
            .key_context("EmbeddedDemo")
            .track_focus(&self.action_focus)
            .size_full()
            .p_4()
            .gap_4()
            .bg(
                if self.canvas_background == StoryCanvasBackground::Transparent {
                    cx.theme().background.opacity(0.)
                } else {
                    cx.theme().background
                },
            )
            .text_color(cx.theme().foreground)
            .on_action(cx.listener(|this, _: &Increment, _, cx| this.increment(cx)));
        let content = if self.route == COUNTER_ROUTE {
            content
                .child(div().text_xl().child("Counter"))
                .child(div().text_3xl().child(self.count.to_string()))
                .child(
                    Button::new("increment")
                        .label("Increment")
                        .large()
                        .disabled(!self.enabled)
                        .on_click(cx.listener(|this, _, _, cx| this.increment(cx)))
                        .storybook_target(),
                )
        } else {
            content
                .child(div().text_xl().child("Notes"))
                .child("Note")
                .child(
                    Input::new(&self.note)
                        .large()
                        .storybook_target_as("note-input", "Note"),
                )
        };
        capture_story_view_with_scroll(
            self.route,
            None,
            content.storybook_value_as("public-state", "Public state", public_state),
        )
    }
}

#[cfg(test)]
mod tests;
