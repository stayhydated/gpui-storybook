use gpui_kit::component::{Root, v_flex};
use gpui_kit::{
    AnyView, App, AppContext as _, Context, Entity, FocusHandle, Focusable,
    InteractiveElement as _, IntoElement, ParentElement as _, Render, SharedString, Styled as _,
    Subscription, Window, div,
};

use crate::{
    automation::{
        SharedStorybookAutomation, StorybookAutomationCommand, StorybookAutomationCommandReceiver,
        default_storybook_automation,
    },
    gallery::Gallery,
    storybook_window_ui::{StorybookWindow, StorybookWindowUi},
    title_bar::AppTitleBar,
    window_options::default_storybook_window_options,
};

use super::StoryContainer;

/// Opens the standard Storybook window.
pub fn create_storybook_window<F>(title: &str, create_window: F, cx: &mut App)
where
    F: FnOnce(&mut Window, &mut App) -> StorybookWindow + Send + 'static,
{
    let options = default_storybook_window_options(cx);
    let title = SharedString::from(title.to_owned());

    cx.spawn(async move |cx| {
        let window = cx.open_window(options, |window, cx| {
            let spec = create_window(window, cx);
            let shell = cx.new(|cx| StorybookShell::new(title.clone(), spec, window, cx));
            let focus_handle = shell.focus_handle(cx);
            window.defer(cx, move |window, cx| {
                focus_handle.focus(window, cx);
            });
            cx.new(|cx| Root::new(shell, window, cx))
        })?;

        window.update(cx, |_, window, _| {
            window.activate_window();
            window.set_window_title(&title);
        })?;

        Ok::<_, anyhow::Error>(())
    })
    .detach();
}

struct StorybookShell {
    focus_handle: FocusHandle,
    root: Entity<StoryRoot>,
    gallery: Entity<Gallery>,
    automation: Option<SharedStorybookAutomation>,
}

impl StorybookShell {
    fn new(
        title: SharedString,
        spec: StorybookWindow,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let automation = default_storybook_automation(cx);
        let command_receiver = automation
            .as_ref()
            .and_then(|automation| automation.take_command_receiver());
        let automation = if command_receiver.is_some() {
            automation
        } else {
            None
        };
        let (root, gallery) =
            Self::build_gallery(title, spec.stories, spec.ui, automation.clone(), window, cx);

        let this = Self {
            focus_handle: cx.focus_handle(),
            root,
            gallery,
            automation,
        };
        if let Some(command_receiver) = command_receiver {
            this.attach_automation_host(command_receiver, window, cx);
        }
        this
    }

    fn build_gallery(
        title: SharedString,
        stories: Vec<Entity<StoryContainer>>,
        ui: StorybookWindowUi,
        automation: Option<SharedStorybookAutomation>,
        window: &mut Window,
        cx: &mut App,
    ) -> (Entity<StoryRoot>, Entity<Gallery>) {
        let gallery = cx
            .new(|cx| Gallery::new_without_automation_host(stories, None, automation, window, cx));
        let root = cx.new(|cx| StoryRoot::new(title, gallery.clone(), ui, window, cx));
        (root, gallery)
    }

    fn handle_automation_command(
        &self,
        command: StorybookAutomationCommand,
        window: &mut Window,
        cx: &mut App,
    ) {
        if self.automation.is_none() {
            return;
        }
        self.gallery.update(cx, |gallery, cx| {
            gallery.handle_automation_command(command, window, cx);
        });
    }

    fn attach_automation_host(
        &self,
        mut receiver: StorybookAutomationCommandReceiver,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        cx.spawn_in(window, async move |this, cx| {
            while let Some(command) = receiver.recv().await {
                let _ = this.update_in(cx, |shell, window, cx| {
                    shell.handle_automation_command(command, window, cx);
                });
            }
        })
        .detach();
    }
}

impl Focusable for StorybookShell {
    fn focus_handle(&self, cx: &App) -> FocusHandle {
        self.root.focus_handle(cx)
    }
}

impl Render for StorybookShell {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
            .id("storybook-shell")
            .track_focus(&self.focus_handle)
            .size_full()
            .child(self.root.clone())
    }
}

pub(crate) struct StoryRoot {
    focus_handle: FocusHandle,
    title_bar: Entity<AppTitleBar>,
    view: AnyView,
    _preference_subscriptions: Vec<Subscription>,
}

impl StoryRoot {
    pub fn new(
        title: impl Into<SharedString>,
        view: impl Into<AnyView>,
        ui: StorybookWindowUi,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let view = view.into();
        let gallery = view.clone().downcast::<Gallery>().ok();
        let title_bar = cx.new(|cx| {
            let title_bar = AppTitleBar::new(title, ui, window, cx);
            if let Some(gallery) = gallery {
                title_bar.sidebar_child(move |_, cx| {
                    Gallery::title_bar_sidebar_controls(gallery.clone(), cx)
                })
            } else {
                title_bar
            }
        });
        let preference_subscriptions = vec![
            cx.observe_window_appearance(window, |_, window, cx| {
                crate::preferences::window_appearance_changed(window, cx);
            }),
            cx.observe_window_activation(window, |_, window, cx| {
                crate::preferences::window_activated(window, cx);
            }),
        ];
        crate::preferences::window_appearance_changed(window, cx);
        Self {
            focus_handle: cx.focus_handle(),
            title_bar,
            view,
            _preference_subscriptions: preference_subscriptions,
        }
    }
}

impl Focusable for StoryRoot {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for StoryRoot {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let sheet_layer = Root::render_sheet_layer(window, cx);
        let dialog_layer = Root::render_dialog_layer(window, cx);
        let notification_layer = Root::render_notification_layer(window, cx);

        div().id("story-root").size_full().child(
            v_flex()
                .size_full()
                .child(self.title_bar.clone())
                .child(
                    div()
                        .track_focus(&self.focus_handle)
                        .flex_1()
                        .overflow_hidden()
                        .child(self.view.clone()),
                )
                .children(sheet_layer)
                .children(dialog_layer)
                .children(notification_layer),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::registry::{RegisteredStoryMetadata, StoryKey, StoryName};
    use tokio::sync::oneshot;

    fn story(
        key: &'static str,
        name: &'static str,
        window: &mut Window,
        cx: &mut App,
    ) -> Entity<StoryContainer> {
        cx.new(|cx| {
            let mut story = StoryContainer::new(window, cx);
            story.name = name.into();
            story.story_klass = Some(name.into());
            story.set_registration_metadata(RegisteredStoryMetadata::new(
                StoryKey::new(key),
                StoryName::new(name),
                None,
                "crate",
                "/tmp/crate",
                "src/stories.rs",
                1,
            ));
            story
        })
    }

    #[gpui_kit::test]
    fn direct_core_window_does_not_require_the_preference_global(cx: &mut App) {
        gpui_kit::init(cx);
        crate::i18n::init(cx).expect("Storybook localization should initialize");
        assert!(
            cx.try_global::<crate::preferences::StorybookPreferencesGlobal>()
                .is_none()
        );

        let window: gpui_kit::WindowHandle<StorybookShell> = cx
            .open_window(Default::default(), |window, cx| {
                cx.new(|cx| {
                    StorybookShell::new(
                        "Direct Core".into(),
                        StorybookWindow::new(Vec::new()),
                        window,
                        cx,
                    )
                })
            })
            .expect("direct core window should open without facade preferences");

        window
            .update(cx, |_, _, cx| {
                crate::preferences::select_scrollbar(
                    gpui_storybook_preferences::PreferredScrollbar::Always,
                    cx,
                );
            })
            .expect("optional preference forwarding should remain a no-op");
    }

    #[gpui_kit::test]
    fn only_the_first_storybook_shell_claims_the_default_controller(cx: &mut App) {
        gpui_kit::init(cx);
        crate::i18n::init(cx).expect("Storybook localization should initialize");
        let automation = crate::automation::StorybookAutomation::new();
        crate::automation::set_default_storybook_automation(cx, automation);
        let open = |cx: &mut App| {
            cx.open_window(Default::default(), |window, cx| {
                cx.new(|cx| {
                    StorybookShell::new(
                        "Storybook".into(),
                        StorybookWindow::new(Vec::new()),
                        window,
                        cx,
                    )
                })
            })
            .expect("Storybook window should open")
        };
        let first: gpui_kit::WindowHandle<StorybookShell> = open(cx);
        let second: gpui_kit::WindowHandle<StorybookShell> = open(cx);

        first
            .update(cx, |shell, _, _| assert!(shell.automation.is_some()))
            .expect("first shell should own the controller host");
        second
            .update(cx, |shell, _, _| assert!(shell.automation.is_none()))
            .expect("second shell should reject the claimed controller");
    }

    #[gpui_kit::test]
    fn gallery_keeps_the_live_view_on_the_automation_route(cx: &mut App) {
        gpui_kit::init(cx);
        crate::i18n::init(cx).expect("Storybook localization should initialize");
        let automation = crate::automation::StorybookAutomation::new();
        crate::automation::set_default_storybook_automation(cx, automation.clone());
        let window: gpui_kit::WindowHandle<StorybookShell> = cx
            .open_window(Default::default(), |window, cx| {
                let button = story("crate-ButtonStory", "ButtonStory", window, cx);
                let table = story("crate-TableStory", "TableStory", window, cx);
                cx.new(|cx| {
                    StorybookShell::new(
                        "Storybook".into(),
                        StorybookWindow::new(vec![button, table]),
                        window,
                        cx,
                    )
                })
            })
            .expect("Storybook shell should open");

        window
            .update(cx, |shell, window, cx| {
                let (response, mut result) = oneshot::channel();
                shell.handle_automation_command(
                    StorybookAutomationCommand::OpenStory {
                        key: "crate-TableStory/expanded".to_owned(),
                        response,
                        _operation: automation
                            .begin_operation()
                            .expect("story-open operation should start"),
                    },
                    window,
                    cx,
                );
                result
                    .try_recv()
                    .expect("story-open response should be sent")
                    .expect("table story should open");
                assert_eq!(
                    shell
                        .gallery
                        .read(cx)
                        .active_story_snapshot(cx)
                        .map(|story| story.key)
                        .as_deref(),
                    Some("crate-TableStory")
                );
                assert_eq!(
                    automation
                        .current_story()
                        .story
                        .expect("automation should keep the selected story")
                        .capture_route_id,
                    "crate-TableStory/expanded"
                );
            })
            .expect("Storybook shell should open the automation route");
    }
}
