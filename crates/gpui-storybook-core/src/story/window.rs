use crate::{
    storybook_window_ui::{StorybookWindow, StorybookWindowUi},
    title_bar::AppTitleBar,
    window_options::default_storybook_window_options,
    window_view::SimpleWindowView,
};
use gpui::{
    AnyView, App, AppContext as _, Context, Entity, FocusHandle, Focusable,
    InteractiveElement as _, IntoElement, ParentElement as _, Render, SharedString, Styled as _,
    Subscription, Window, div,
};
use gpui_component::{Root, v_flex};

pub fn create_new_window<F, V>(title: &str, crate_view_fn: F, cx: &mut App)
where
    V: SimpleWindowView,
    F: FnOnce(&mut Window, &mut App) -> Entity<V> + Send + 'static,
{
    create_new_window_with_ui(
        title,
        move |window, cx| StorybookWindow::new(crate_view_fn(window, cx)),
        cx,
    );
}

pub fn create_new_window_with_ui<F, V>(title: &str, create_view_fn: F, cx: &mut App)
where
    V: SimpleWindowView,
    F: FnOnce(&mut Window, &mut App) -> StorybookWindow<V> + Send + 'static,
{
    let options = default_storybook_window_options(cx);
    let title = SharedString::from(title.to_string());

    cx.spawn(async move |cx| {
        let window = cx.open_window(options, |window, cx| {
            let storybook_window = create_view_fn(window, cx);
            let root = cx.new(|cx| {
                StoryRoot::new(
                    title.clone(),
                    storybook_window.view,
                    storybook_window.ui,
                    window,
                    cx,
                )
            });

            let focus_handle = root.focus_handle(cx);
            window.defer(cx, move |window, cx| {
                focus_handle.focus(window, cx);
            });

            cx.new(|cx| Root::new(root, window, cx))
        })?;

        window.update(cx, |_, window, _| {
            window.activate_window();
            window.set_window_title(&title);
        })?;

        Ok::<_, anyhow::Error>(())
    })
    .detach();
}

struct StoryRoot {
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
        let title_bar = cx.new(|cx| AppTitleBar::new(title, ui, window, cx));
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
            view: view.into(),
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

    struct DirectCoreView;

    impl Render for DirectCoreView {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            div().child("direct core view")
        }
    }

    impl SimpleWindowView for DirectCoreView {}

    #[gpui::test]
    fn direct_core_window_does_not_require_the_preference_global(cx: &mut App) {
        gpui_component::init(cx);
        crate::i18n::init(cx).expect("Storybook localization should initialize");
        assert!(
            cx.try_global::<crate::preferences::StorybookPreferencesGlobal>()
                .is_none()
        );

        let window: gpui::WindowHandle<StoryRoot> = cx
            .open_window(Default::default(), |window, cx| {
                let view = cx.new(|_| DirectCoreView);
                cx.new(|cx| {
                    StoryRoot::new(
                        "Direct Core",
                        view,
                        StorybookWindowUi::default(),
                        window,
                        cx,
                    )
                })
            })
            .expect("direct core window should open without facade preferences");

        window
            .update(cx, |_, window, cx| {
                crate::preferences::window_activated(window, cx);
                crate::preferences::select_scrollbar(
                    gpui_storybook_preferences::PreferredScrollbar::Always,
                    cx,
                );
            })
            .expect("optional preference forwarding should remain a no-op");
    }
}
