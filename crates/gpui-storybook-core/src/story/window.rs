use crate::{
    storybook_window_ui::{StorybookWindow, StorybookWindowUi},
    title_bar::AppTitleBar,
    window_options::default_storybook_window_options,
    window_view::SimpleWindowView,
};
use gpui::{
    AnyView, App, AppContext as _, Context, Entity, FocusHandle, Focusable,
    InteractiveElement as _, IntoElement, ParentElement as _, Render, SharedString, Styled as _,
    Window, div,
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
        let window = cx
            .open_window(options, |window, cx| {
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
            })
            .expect("failed to open window");

        window
            .update(cx, |_, window, _| {
                window.activate_window();
                window.set_window_title(&title);
            })
            .expect("failed to update window");

        Ok::<_, anyhow::Error>(())
    })
    .detach();
}

struct StoryRoot {
    focus_handle: FocusHandle,
    title_bar: Entity<AppTitleBar>,
    view: AnyView,
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
        Self {
            focus_handle: cx.focus_handle(),
            title_bar,
            view: view.into(),
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
