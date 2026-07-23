#[cfg(not(feature = "dock"))]
use gpui_storybook::Gallery;
#[cfg(feature = "dock")]
use gpui_storybook::StoryWorkspace;
use gpui_storybook::{Assets, ConsumerId, StorybookOptions};
use gpui_storybook_example_component::i18n::{self, Languages};

#[allow(unused_imports)]
use gpui_storybook_example_component::*;

const CONSUMER_ID: &str = "gpui-storybook-example-component";

fn main() {
    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    let app = gpui_platform::application().with_assets(Assets);

    app.run(move |app_cx| {
        let consumer_id = match ConsumerId::new(CONSUMER_ID) {
            Ok(consumer_id) => consumer_id,
            Err(error) => {
                tracing::error!(error = %error, "invalid component example consumer id");
                app_cx.quit();
                return;
            },
        };
        let options = StorybookOptions::new(consumer_id, Languages::default(), i18n::apply_locale);
        let readiness = match gpui_storybook::init(app_cx, options) {
            Ok(readiness) => readiness,
            Err(error) => {
                tracing::error!(
                    error = %error,
                    "failed to initialize component example Storybook"
                );
                app_cx.quit();
                return;
            },
        };

        let http_client = std::sync::Arc::new(reqwest_client::ReqwestClient::new());
        app_cx.set_http_client(http_client);

        app_cx
            .spawn(async move |cx| {
                let ready = readiness.await;
                tracing::info!(
                    persistence_status = ?ready.persistence_status,
                    diagnostics = ?ready.diagnostics,
                    "component example preferences are ready"
                );
                if !ready.diagnostics.is_empty() {
                    tracing::warn!(
                        persistence_status = ?ready.persistence_status,
                        diagnostics = ?ready.diagnostics,
                        "component example initialized with preference diagnostics"
                    );
                }

                cx.update(|app_cx| {
                    if let Some(state) = gpui_storybook::try_preference_state(app_cx) {
                        tracing::info!(
                            color_scheme_source = ?state.resolved.color_scheme.source,
                            theme_source = ?state.resolved.theme.source,
                            language_source = ?state.resolved.language.source,
                            resolution_diagnostic_count = state.resolution_diagnostics.len(),
                            "component example preference state applied"
                        );
                    }
                    app_cx.activate(true);

                    #[cfg(not(feature = "dock"))]
                    gpui_storybook::create_new_window(
                        &format!("{} - Stories", env!("CARGO_PKG_NAME")),
                        move |window, cx| {
                            let all_stories = gpui_storybook::generate_stories(window, cx);
                            Gallery::view(all_stories, None, window, cx)
                        },
                        app_cx,
                    );

                    #[cfg(feature = "dock")]
                    gpui_storybook::create_dock_window(
                        &format!("{} - Stories", env!("CARGO_PKG_NAME")),
                        move |window, cx| {
                            let all_stories = gpui_storybook::generate_stories(window, cx);
                            StoryWorkspace::view(all_stories, window, cx)
                        },
                        app_cx,
                    );
                });
            })
            .detach();
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn startup_contract_uses_a_stable_consumer_and_typed_adapter() {
        let consumer = ConsumerId::new(CONSUMER_ID).expect("checked consumer id");
        let options =
            StorybookOptions::new(consumer.clone(), Languages::default(), i18n::apply_locale);
        assert_eq!(options.consumer_id, consumer);
        assert_eq!(options.fallback_language, Languages::default());
    }

    #[gpui::test]
    async fn facade_startup_localizes_the_linked_consumer_module(cx: &mut gpui::TestAppContext) {
        cx.executor().allow_parking();
        let readiness = cx.update(|cx| {
            let consumer = ConsumerId::new("gpui-storybook-component-i18n-test")
                .expect("checked test consumer id");
            gpui_storybook::init(
                cx,
                StorybookOptions::new(consumer, Languages::default(), i18n::apply_locale)
                    .with_persistence(gpui_storybook::PersistenceMode::Disabled)
                    .with_overrides(gpui_storybook::PreferenceOverrides {
                        language: Some(Languages::Fr),
                        ..Default::default()
                    }),
            )
            .expect("facade startup should initialize")
        });
        let ready = readiness.await;
        assert!(ready.diagnostics.is_empty());

        let title = cx.update(|cx| {
            gpui_storybook::localize_message(
                cx,
                &gpui_storybook_example_component::StoryItems::Title,
            )
        });
        assert_eq!(title.as_deref(), Some("Titre"));
    }
}
