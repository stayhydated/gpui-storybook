use gpui_kit::component::{button::Button, h_flex, v_flex};
use gpui_kit::{
    AppContext as _, Context, Entity, IntoElement, ParentElement as _, Render, Styled as _, Window,
    div,
};
use gpui_storybook_automation_gpui::GpuiHostAttachment;
use gpui_storybook_example_embedded::DemoRoot;

struct DesktopRoot {
    view: Entity<DemoRoot>,
    // Retained for this window's surface lifecycle; the application owns dispatch.
    attachment: GpuiHostAttachment<DemoRoot>,
}
impl Render for DesktopRoot {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        v_flex()
            .size_full()
            .child(
                h_flex()
                    .p_3()
                    .gap_2()
                    .child(
                        Button::new("counter")
                            .label("Counter")
                            .on_click(cx.listener(|root, _, window, cx| {
                                root.attachment
                                    .open_story("embedded-counter", window, cx)
                                    .expect("registered route");
                            })),
                    )
                    .child(Button::new("notes").label("Notes").on_click(cx.listener(
                        |root, _, window, cx| {
                            root.attachment
                                .open_story("embedded-notes", window, cx)
                                .expect("registered route");
                        },
                    ))),
            )
            .child(div().flex_1().overflow_hidden().child(self.view.clone()))
    }
}

fn main() {
    gpui_kit::application()
        .with_assets(gpui_kit::assets::Assets)
        .run(|cx| {
            gpui_kit::init(cx);
            gpui_kit::open_window(Default::default(), cx, |window, cx| {
                let view = cx.new(|cx| DemoRoot::new(window, cx));
                let attachment = gpui_storybook_example_embedded::attach(&view, window, cx);
                cx.new(|_| DesktopRoot { view, attachment })
            })
            .expect("embedded example window should open");
        });
}
