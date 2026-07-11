use gpui::{App, AppContext as _, Context, FocusHandle, Focusable, IntoElement, Render, Window};

mod first {
    use super::*;

    #[gpui_storybook::story]
    pub struct DuplicateStory {
        focus_handle: FocusHandle,
    }

    impl Focusable for DuplicateStory {
        fn focus_handle(&self, _: &App) -> FocusHandle {
            self.focus_handle.clone()
        }
    }

    impl Render for DuplicateStory {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            gpui::div()
        }
    }

    impl gpui_storybook::Story for DuplicateStory {
        fn title(_: &App) -> String {
            "Duplicate".to_string()
        }

        fn new_view(window: &mut Window, cx: &mut App) -> gpui::Entity<impl Render + Focusable> {
            let _ = window;
            cx.new(|cx| Self {
                focus_handle: cx.focus_handle(),
            })
        }
    }
}

mod second {
    use super::*;

    #[gpui_storybook::story]
    pub struct DuplicateStory {
        focus_handle: FocusHandle,
    }

    impl Focusable for DuplicateStory {
        fn focus_handle(&self, _: &App) -> FocusHandle {
            self.focus_handle.clone()
        }
    }

    impl Render for DuplicateStory {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            gpui::div()
        }
    }

    impl gpui_storybook::Story for DuplicateStory {
        fn title(_: &App) -> String {
            "Duplicate".to_string()
        }

        fn new_view(window: &mut Window, cx: &mut App) -> gpui::Entity<impl Render + Focusable> {
            let _ = window;
            cx.new(|cx| Self {
                focus_handle: cx.focus_handle(),
            })
        }
    }
}

fn main() {}
