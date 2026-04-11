use es_fluent::EsFluent;
use es_fluent_lang::es_fluent_language;
use gpui_storybook::Assets;
#[cfg(not(feature = "dock"))]
use gpui_storybook::Gallery;
#[cfg(feature = "dock")]
use gpui_storybook::StoryWorkspace;
use strum::EnumIter;

es_fluent_manager_embedded::define_i18n_module!();

#[allow(unused_imports)]
use gpui_storybook_example_story::*;

#[es_fluent_language]
#[derive(Clone, Copy, Debug, EnumIter, EsFluent, PartialEq)]
pub enum Languages {}

fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    let app = gpui_platform::application().with_assets(Assets);

    app.run(move |app_cx| {
        gpui_storybook::init(Languages::default(), app_cx);
        gpui_storybook::change_locale(Languages::default());

        let http_client = std::sync::Arc::new(reqwest_client::ReqwestClient::new());
        app_cx.set_http_client(http_client);

        app_cx.activate(true);

        #[cfg(not(feature = "dock"))]
        gpui_storybook::create_new_window(
            &format!("{} - Stories", env!("CARGO_PKG_NAME")),
            move |window, cx| {
                // Stories are filtered/grouped by examples/story/storybook.toml.
                let all_stories = gpui_storybook::generate_stories(window, cx);
                Gallery::view(all_stories, None, window, cx)
            },
            app_cx,
        );

        #[cfg(feature = "dock")]
        gpui_storybook::create_dock_window(
            &format!("{} - Stories", env!("CARGO_PKG_NAME")),
            move |window, cx| {
                // Stories are filtered/grouped by examples/story/storybook.toml.
                let all_stories = gpui_storybook::generate_stories(window, cx);

                StoryWorkspace::view(all_stories, window, cx)
            },
            app_cx,
        );
    });
}
