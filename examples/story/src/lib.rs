use gpui_kit::Application;
use gpui_storybook::{ConsumerId, StorybookOptions, StorybookWindow};

extern crate gpui_kit as gpui;

pub mod i18n;
pub mod stories;

use i18n::Languages;

/// Story sections with custom ordering.
#[derive(Clone, Copy, Debug)]
#[repr(usize)]
pub enum StorySection {
    CustomSections = 8,
    Tables = 7,
    Buttons = 6,
    Grouped = 5,
    Automation = 4,
}

const CONSUMER_ID: &str = "gpui-storybook-example-story";

fn storybook_options() -> Result<StorybookOptions<Languages>, gpui_storybook::ConsumerIdError> {
    let options = StorybookOptions::new(
        ConsumerId::new(CONSUMER_ID)?,
        Languages::default(),
        i18n::apply_locale,
    );

    #[cfg(target_family = "wasm")]
    let options = options.with_persistence(gpui_storybook::PersistenceMode::Disabled);

    Ok(options)
}

pub fn run_storybook(app: Application) {
    app.run(move |app_cx| {
        let options = match storybook_options() {
            Ok(options) => options,
            Err(error) => {
                tracing::error!(error = %error, "invalid story example consumer id");
                app_cx.quit();
                return;
            },
        };
        let readiness = match gpui_storybook::init(app_cx, options) {
            Ok(readiness) => readiness,
            Err(error) => {
                tracing::error!(error = %error, "failed to initialize story example Storybook");
                app_cx.quit();
                return;
            },
        };

        #[cfg(not(target_family = "wasm"))]
        {
            let http_client = std::sync::Arc::new(reqwest_client::ReqwestClient::new());
            app_cx.set_http_client(http_client);
        }

        app_cx
            .spawn(async move |cx| {
                let ready = readiness.await;
                tracing::info!(
                    persistence_status = ?ready.persistence_status,
                    diagnostics = ?ready.diagnostics,
                    "story example preferences are ready"
                );
                if !ready.diagnostics.is_empty() {
                    tracing::warn!(
                        persistence_status = ?ready.persistence_status,
                        diagnostics = ?ready.diagnostics,
                        "story example initialized with preference diagnostics"
                    );
                }

                cx.update(|app_cx| {
                    if let Some(state) = gpui_storybook::try_preference_state(app_cx) {
                        tracing::info!(
                            color_scheme_source = ?state.resolved.color_scheme.source,
                            theme_source = ?state.resolved.theme.source,
                            language_source = ?state.resolved.language.source,
                            resolution_diagnostic_count = state.resolution_diagnostics.len(),
                            "story example preference state applied"
                        );
                    }
                    app_cx.activate(true);

                    gpui_storybook::create_storybook_window(
                        &format!("{} - Stories", env!("CARGO_PKG_NAME")),
                        move |window, cx| {
                            let stories = gpui_storybook::generate_stories(window, cx);
                            assert!(
                                !stories.is_empty(),
                                "story example Storybook requires linked stories"
                            );
                            StorybookWindow::new(stories)
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

    #[test]
    fn binary_links_expected_story_registrations() {
        let mut story_keys =
            gpui_storybook::__inventory::iter::<gpui_storybook::__registry::StoryEntry>()
                .filter(|entry| entry.crate_name == env!("CARGO_PKG_NAME"))
                .map(|entry| entry.key.as_str())
                .collect::<Vec<_>>();
        story_keys.sort_unstable();

        assert_eq!(
            story_keys,
            [
                "gpui-storybook-example-story-ActionsAndScenariosStory",
                "gpui-storybook-example-story-ButtonStory",
                "gpui-storybook-example-story-CustomSectionStory",
                "gpui-storybook-example-story-GroupedDetailsStory",
                "gpui-storybook-example-story-GroupedSummaryStory",
                "gpui-storybook-example-story-HelloWorld",
                "gpui-storybook-example-story-InteractionStory",
                "gpui-storybook-example-story-TableStory",
            ]
        );
    }
}
