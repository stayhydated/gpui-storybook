use std::rc::Rc;

use gpui::{
    AnyView, App, AppContext as _, Context, Entity, FocusHandle, Focusable,
    InteractiveElement as _, IntoElement, ParentElement as _, Render, SharedString, Styled as _,
    Subscription, Window, div,
};
use gpui_component::{
    IndexPath, Root, Sizable as _,
    dock::{ClosePanel, ToggleZoom},
    h_flex,
    searchable_list::SearchableListItem,
    select::{Select, SelectEvent, SelectState},
    v_flex,
};
use gpui_storybook_preferences::StorybookWindowMode;

use crate::{
    automation::{
        SharedStorybookAutomation, StorybookAutomationCommand, default_storybook_automation,
    },
    dock_gallery::StoryWorkspace,
    gallery::Gallery,
    messages::{StorybookMessage, text},
    preferences::{self, StorybookPreferencesGlobal},
    storybook_window_ui::{StorybookWindow, StorybookWindowUi, configured_storybook_window_mode},
    title_bar::AppTitleBar,
    window_options::default_storybook_window_options,
};

use super::StoryContainer;

/// Opens the standard Storybook window with a runtime-selectable layout.
///
/// The initial mode is selected in this order: a
/// [`StorybookWindow::with_mode`] value, the active `storybook.toml`
/// `window_mode`, then the saved consumer preference. Users can switch layouts
/// from the title bar, and the choice is saved for later windows.
pub fn create_storybook_window<F>(title: &str, create_window: F, cx: &mut App)
where
    F: FnOnce(&mut Window, &mut App) -> StorybookWindow + Send + 'static,
{
    let options = default_storybook_window_options(cx);
    let title = SharedString::from(title.to_owned());

    cx.bind_keys(vec![
        gpui::KeyBinding::new("shift-escape", ToggleZoom, None),
        gpui::KeyBinding::new("ctrl-w", ClosePanel, None),
    ]);

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

#[derive(Clone)]
struct WindowModeOption {
    value: StorybookWindowMode,
    label: SharedString,
}

impl SearchableListItem for WindowModeOption {
    type Value = StorybookWindowMode;

    fn title(&self) -> SharedString {
        self.label.clone()
    }

    fn value(&self) -> &Self::Value {
        &self.value
    }
}

enum ActiveStorybookView {
    Gallery {
        root: Entity<StoryRoot>,
        gallery: Entity<Gallery>,
    },
    Dock(Entity<StoryWorkspace>),
}

impl ActiveStorybookView {
    fn view(&self) -> AnyView {
        match self {
            Self::Gallery { root, .. } => root.clone().into(),
            Self::Dock(workspace) => workspace.clone().into(),
        }
    }

    fn handle_automation_command(
        &self,
        command: StorybookAutomationCommand,
        window: &mut Window,
        cx: &mut App,
    ) {
        match self {
            Self::Gallery { gallery, .. } => {
                gallery.update(cx, |gallery, cx| {
                    gallery.handle_automation_command(command, window, cx);
                });
            },
            Self::Dock(workspace) => {
                workspace.update(cx, |workspace, cx| {
                    workspace.handle_automation_command(command, window, cx);
                });
            },
        }
    }
}

struct StorybookShell {
    focus_handle: FocusHandle,
    title: SharedString,
    stories: Vec<Entity<StoryContainer>>,
    ui: StorybookWindowUi,
    mode: StorybookWindowMode,
    follows_saved_mode: bool,
    mode_select: Entity<SelectState<Vec<WindowModeOption>>>,
    active: ActiveStorybookView,
    automation: Option<SharedStorybookAutomation>,
    _subscriptions: Vec<Subscription>,
}

impl StorybookShell {
    fn new(
        title: SharedString,
        spec: StorybookWindow,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let saved_mode = preferences::try_state(cx)
            .map(|state| state.saved.window_mode)
            .unwrap_or_default();
        let (mode, follows_saved_mode) =
            initial_window_mode(spec.mode, configured_storybook_window_mode(cx), saved_mode);
        let selected_index = StorybookWindowMode::ALL
            .iter()
            .position(|candidate| *candidate == mode)
            .map(IndexPath::new);
        let mode_select =
            cx.new(|cx| SelectState::new(Self::mode_options(cx), selected_index, window, cx));
        let automation = default_storybook_automation(cx);
        let active = Self::build_active(
            mode,
            title.clone(),
            spec.stories.clone(),
            Self::ui_with_mode_select(spec.ui.clone(), mode_select.clone()),
            automation.clone(),
            window,
            cx,
        );

        let mut subscriptions = vec![cx.subscribe_in(
            &mode_select,
            window,
            |this, _, event: &SelectEvent<Vec<WindowModeOption>>, window, cx| {
                let SelectEvent::Confirm(Some(mode)) = event else {
                    return;
                };
                this.follows_saved_mode = true;
                this.set_mode(*mode, true, window, cx);
            },
        )];

        if cx.try_global::<StorybookPreferencesGlobal>().is_some() {
            subscriptions.push(cx.observe_global_in::<StorybookPreferencesGlobal>(
                window,
                |this, window, cx| {
                    this.refresh_mode_options(window, cx);
                    if this.follows_saved_mode
                        && let Some(mode) =
                            preferences::try_state(cx).map(|state| state.saved.window_mode)
                    {
                        this.set_mode(mode, false, window, cx);
                    }
                },
            ));
        }

        let this = Self {
            focus_handle: cx.focus_handle(),
            title,
            stories: spec.stories,
            ui: spec.ui,
            mode,
            follows_saved_mode,
            mode_select,
            active,
            automation,
            _subscriptions: subscriptions,
        };
        this.attach_automation_host(window, cx);
        this
    }

    fn mode_options(cx: &App) -> Vec<WindowModeOption> {
        vec![
            WindowModeOption {
                value: StorybookWindowMode::Gallery,
                label: text(cx, StorybookMessage::Gallery).into(),
            },
            WindowModeOption {
                value: StorybookWindowMode::Dock,
                label: text(cx, StorybookMessage::DockWorkspace).into(),
            },
        ]
    }

    fn ui_with_mode_select(
        ui: StorybookWindowUi,
        mode_select: Entity<SelectState<Vec<WindowModeOption>>>,
    ) -> StorybookWindowUi {
        let custom_title_bar = ui.title_bar_items.clone();
        StorybookWindowUi {
            app_menu_items: ui.app_menu_items,
            title_bar_items: Some(Rc::new(move |window, cx| {
                h_flex()
                    .gap_2()
                    .children(custom_title_bar.as_ref().map(|render| render(window, cx)))
                    .child(
                        Select::new(&mode_select)
                            .title_prefix(format!("{}: ", text(cx, StorybookMessage::Layout)))
                            .xsmall(),
                    )
                    .into_any_element()
            })),
        }
    }

    fn build_active(
        mode: StorybookWindowMode,
        title: SharedString,
        stories: Vec<Entity<StoryContainer>>,
        ui: StorybookWindowUi,
        automation: Option<SharedStorybookAutomation>,
        window: &mut Window,
        cx: &mut App,
    ) -> ActiveStorybookView {
        match mode {
            StorybookWindowMode::Gallery => {
                let gallery = cx.new(|cx| {
                    Gallery::new_without_automation_host(stories, None, automation, window, cx)
                });
                let root = cx.new(|cx| StoryRoot::new(title, gallery.clone(), ui, window, cx));
                ActiveStorybookView::Gallery { root, gallery }
            },
            StorybookWindowMode::Dock => ActiveStorybookView::Dock(cx.new(|cx| {
                StoryWorkspace::new_without_automation_host(
                    stories, title, ui, automation, window, cx,
                )
            })),
        }
    }

    fn set_mode(
        &mut self,
        mode: StorybookWindowMode,
        persist: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.mode != mode {
            self.mode = mode;
            self.active = Self::build_active(
                mode,
                self.title.clone(),
                self.stories.clone(),
                Self::ui_with_mode_select(self.ui.clone(), self.mode_select.clone()),
                self.automation.clone(),
                window,
                cx,
            );
            self.active_focus_handle(cx).focus(window, cx);
            cx.notify();
        }

        if self.mode_select.read(cx).selected_value() != Some(&mode) {
            self.mode_select.update(cx, |select, cx| {
                select.set_selected_value(&mode, window, cx);
            });
        }
        if persist {
            preferences::select_window_mode(mode, cx);
        }
    }

    fn refresh_mode_options(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.mode_select.update(cx, |select, cx| {
            select.set_items(Self::mode_options(cx), window, cx);
            select.set_selected_value(&self.mode, window, cx);
        });
    }

    fn attach_automation_host(&self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(automation) = self.automation.clone() else {
            return;
        };
        let Some(mut receiver) = automation.take_command_receiver() else {
            return;
        };

        cx.spawn_in(window, async move |this, cx| {
            while let Some(command) = receiver.recv().await {
                let _ = this.update_in(cx, |shell, window, cx| {
                    shell.active.handle_automation_command(command, window, cx);
                });
            }
        })
        .detach();
    }

    fn active_focus_handle(&self, cx: &App) -> FocusHandle {
        match &self.active {
            ActiveStorybookView::Gallery { root, .. } => root.focus_handle(cx),
            ActiveStorybookView::Dock(_) => self.focus_handle.clone(),
        }
    }
}

fn initial_window_mode(
    explicit: Option<StorybookWindowMode>,
    configured: Option<StorybookWindowMode>,
    saved: StorybookWindowMode,
) -> (StorybookWindowMode, bool) {
    match (explicit, configured) {
        (Some(mode), _) | (None, Some(mode)) => (mode, false),
        (None, None) => (saved, true),
    }
}

impl Focusable for StorybookShell {
    fn focus_handle(&self, cx: &App) -> FocusHandle {
        self.active_focus_handle(cx)
    }
}

impl Render for StorybookShell {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
            .id("storybook-shell")
            .track_focus(&self.focus_handle)
            .size_full()
            .child(self.active.view())
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

    #[test]
    fn initial_window_mode_prefers_explicit_then_toml_then_saved() {
        assert_eq!(
            initial_window_mode(
                Some(StorybookWindowMode::Gallery),
                Some(StorybookWindowMode::Dock),
                StorybookWindowMode::Dock,
            ),
            (StorybookWindowMode::Gallery, false)
        );
        assert_eq!(
            initial_window_mode(
                None,
                Some(StorybookWindowMode::Dock),
                StorybookWindowMode::Gallery,
            ),
            (StorybookWindowMode::Dock, false)
        );
        assert_eq!(
            initial_window_mode(None, None, StorybookWindowMode::Dock),
            (StorybookWindowMode::Dock, true)
        );
    }

    #[gpui::test]
    fn direct_core_window_does_not_require_the_preference_global(cx: &mut App) {
        gpui_component::init(cx);
        crate::i18n::init(cx).expect("Storybook localization should initialize");
        assert!(
            cx.try_global::<crate::preferences::StorybookPreferencesGlobal>()
                .is_none()
        );

        let window: gpui::WindowHandle<StorybookShell> = cx
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
            .update(cx, |shell, window, cx| {
                assert_eq!(shell.mode, StorybookWindowMode::Gallery);
                shell.set_mode(StorybookWindowMode::Dock, true, window, cx);
                assert_eq!(shell.mode, StorybookWindowMode::Dock);
                assert_eq!(
                    shell.mode_select.read(cx).selected_value(),
                    Some(&StorybookWindowMode::Dock)
                );
                crate::preferences::select_scrollbar(
                    gpui_storybook_preferences::PreferredScrollbar::Always,
                    cx,
                );
            })
            .expect("optional preference forwarding should remain a no-op");
    }
}
