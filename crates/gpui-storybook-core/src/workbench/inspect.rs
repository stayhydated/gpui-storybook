use super::*;

impl StoryWorkbench {
    pub(super) fn render_inspect(&self, cx: &mut Context<Self>) -> AnyElement {
        let Some(story) = self.active_story(cx) else {
            return v_flex()
                .p_4()
                .text_color(cx.theme().muted_foreground)
                .child("Select a story")
                .into_any_element();
        };
        let story = story.read(cx);
        let key = story.story_key_label().unwrap_or("unregistered").to_owned();
        let source_file = story.source_file_label().unwrap_or("unknown source");
        let source = format!(
            "{}:{}",
            source_file,
            story.source_line().unwrap_or_default()
        );
        let source_url = story
            .registration_metadata()
            .and_then(|metadata| story_source_url(metadata.crate_dir(), source_file));

        let source = source_url
            .map(|url| {
                Link::new("open-story-source")
                    .href(url)
                    .child(source.clone())
                    .into_any_element()
            })
            .unwrap_or_else(|| div().child(source).into_any_element());

        let content = v_flex().p_4().gap_3();
        #[cfg(feature = "inspector")]
        let content = content.child(
            Button::new("open-gpui-inspector")
                .label("Open GPUI Inspector")
                .on_click(|_, window, cx| window.toggle_inspector(cx)),
        );

        content
            .child(
                v_flex()
                    .gap_1()
                    .child(
                        h_flex().justify_between().gap_2().child("Story key").child(
                            Clipboard::new("copy-story-key")
                                .value(key.clone())
                                .tooltip("Copy story key"),
                        ),
                    )
                    .child(key),
            )
            .child(v_flex().gap_1().child("Source").child(source))
            .into_any_element()
    }
}
