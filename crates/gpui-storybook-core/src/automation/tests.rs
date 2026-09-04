use super::capture::expanded_window_size;
#[cfg(feature = "capture")]
use super::capture::image_crop_rect;
use super::*;

fn sample_story(key: &str, title: &str) -> StorySnapshot {
    StorySnapshot {
        key: key.to_string(),
        crate_name: "crate".to_string(),
        story_name: format!("{title}Story"),
        title: title.to_string(),
        description: format!("{title} description"),
        group: Some("Examples".to_string()),
        section: Some("Components".to_string()),
        source_file: format!("src/{}.rs", title.to_lowercase()),
        source_line: 7,
        capture_route_id: key.to_string(),
        default_size: StoryDefaultSize::default(),
        scenarios: Vec::new(),
    }
}

#[test]
fn story_routes_resolve_substory_capture_ids() {
    let automation =
        StorybookAutomation::with_stories(vec![sample_story("crate-ButtonStory", "Button")]);

    let story = automation
        .get_story("crate-ButtonStory/with-progress")
        .expect("substory route should resolve through its base story");

    assert_eq!(story.key, "crate-ButtonStory");
    assert_eq!(story.capture_route_id, "crate-ButtonStory/with-progress");
    assert_eq!(story.title, "Button / With Progress");
}

#[test]
fn automation_state_tracks_stories_current_route_and_revision() {
    let button = sample_story("crate-ButtonStory", "Button");
    let table = sample_story("crate-TableStory", "Table");
    let automation = StorybookAutomation::with_stories(vec![button.clone(), table.clone()]);

    assert_eq!(automation.stories(), vec![button.clone(), table.clone()]);
    assert_eq!(automation.current_story().story, Some(button.clone()));
    assert_eq!(automation.current_story().revision, 0);

    let current = automation
        .confirm_current_story("crate-TableStory")
        .expect("registered story should become current");
    assert_eq!(current.story, Some(table.clone()));
    assert_eq!(current.revision, 1);

    let unchanged = automation
        .confirm_current_story("crate-TableStory")
        .expect("current story should remain valid");
    assert_eq!(unchanged.revision, 1);

    automation.set_stories(vec![table.clone()]);
    assert_eq!(automation.current_story().story, Some(table));
    assert_eq!(automation.current_story().revision, 1);

    automation.set_stories(vec![button.clone()]);
    assert_eq!(automation.current_story().story, Some(button));
    assert_eq!(automation.current_story().revision, 2);

    automation.set_stories(Vec::new());
    assert_eq!(automation.current_story().story, None);
    assert_eq!(automation.current_story().revision, 3);
}

#[test]
fn missing_story_routes_return_typed_errors() {
    let automation = StorybookAutomation::new();

    assert_eq!(
        automation.get_story("missing"),
        Err(StorybookAutomationError::StoryNotFound {
            key: "missing".to_string(),
        })
    );
    assert_eq!(
        automation.confirm_current_story("missing"),
        Err(StorybookAutomationError::StoryNotFound {
            key: "missing".to_string(),
        })
    );
}

#[test]
fn scenario_catalog_lists_stable_descriptors_and_reports_missing_keys() {
    let mut story = sample_story("crate-ButtonStory", "Button");
    story.scenarios = vec![
        StoryScenario::new("press", "Press button")
            .description("Presses the primary button.")
            .step(StoryScenarioStep::new(
                "focus button",
                StoryInteractionStep::FocusNext,
            )),
    ];
    let automation = StorybookAutomation::with_stories(vec![story.clone()]);

    let listed = automation
        .list_scenarios()
        .expect("current story scenarios should be listed");
    assert_eq!(listed.story, story);
    assert_eq!(listed.scenarios.len(), 1);
    assert_eq!(listed.scenarios[0].key, "press");
    let route_listing = automation
        .list_scenarios_for("crate-ButtonStory/section")
        .expect("substory scenarios should be listed");
    assert_eq!(
        route_listing.story.capture_route_id,
        "crate-ButtonStory/section"
    );
    assert_eq!(route_listing.story.title, "Button / Section");
    assert_eq!(route_listing.scenarios, listed.scenarios);

    assert_eq!(
        automation.list_scenarios_for("missing"),
        Err(StorybookAutomationError::StoryNotFound {
            key: "missing".to_owned(),
        })
    );
}

#[test]
fn run_scenario_resolves_descriptor_before_requiring_live_host() {
    let mut story = sample_story("crate-ButtonStory", "Button");
    story.scenarios =
        vec![
            StoryScenario::new("press", "Press button").step(StoryScenarioStep::new(
                "focus button",
                StoryInteractionStep::FocusNext,
            )),
        ];
    let automation = StorybookAutomation::with_stories(vec![story]);
    let runtime = tokio::runtime::Builder::new_current_thread()
        .build()
        .expect("test runtime should build");

    runtime.block_on(async {
        assert_eq!(
            automation
                .run_scenario(Some("crate-ButtonStory".to_owned()), "missing")
                .await,
            Err(StorybookAutomationError::ScenarioNotFound {
                story_key: "crate-ButtonStory".to_owned(),
                scenario_key: "missing".to_owned(),
            })
        );
        assert_eq!(
            automation
                .run_scenario(Some("crate-ButtonStory".to_owned()), "press")
                .await,
            Err(StorybookAutomationError::NoLiveHost)
        );
    });
}

#[test]
fn open_and_capture_require_a_live_host() {
    let story = sample_story("crate-ButtonStory", "Button");
    let automation = StorybookAutomation::with_stories(vec![story]);
    let runtime = tokio::runtime::Builder::new_current_thread()
        .build()
        .expect("test runtime should build");

    runtime.block_on(async {
        assert_eq!(
            automation.open_story("crate-ButtonStory").await,
            Err(StorybookAutomationError::NoLiveHost)
        );
        assert_eq!(
            automation
                .capture_current_story(StoryScreenshotRequest::default())
                .await,
            Err(StorybookAutomationError::NoLiveHost)
        );
        assert_eq!(
            automation.open_story("missing").await,
            Err(StorybookAutomationError::StoryNotFound {
                key: "missing".to_string(),
            })
        );
    });
}

#[test]
fn command_receiver_attaches_once() {
    let automation = StorybookAutomation::new();

    assert!(automation.take_command_receiver().is_some());
    assert!(automation.take_command_receiver().is_none());
}

#[test]
fn facade_automation_waits_for_catalog_and_live_host() {
    let automation = StorybookAutomation::new();
    let runtime = tokio::runtime::Builder::new_current_thread()
        .build()
        .expect("test runtime should build");

    runtime.block_on(async {
        let waiting = automation.clone();
        let ready = tokio::spawn(async move { waiting.wait_until_ready().await });
        tokio::task::yield_now().await;
        assert!(!ready.is_finished());

        automation.set_stories(vec![sample_story("crate-ButtonStory", "Button")]);
        tokio::task::yield_now().await;
        assert!(!ready.is_finished());

        let _receiver = automation
            .take_command_receiver()
            .expect("live host should attach once");
        ready.await.expect("readiness task should complete");
    });
}

#[test]
fn explicitly_seeded_automation_skips_facade_startup_wait() {
    let automation =
        StorybookAutomation::with_stories(vec![sample_story("crate-ButtonStory", "Button")]);
    let runtime = tokio::runtime::Builder::new_current_thread()
        .build()
        .expect("test runtime should build");

    runtime.block_on(automation.wait_until_ready());
}

#[test]
fn operation_guard_rejects_mutations_until_its_owner_drops() {
    let automation =
        StorybookAutomation::with_stories(vec![sample_story("crate-ButtonStory", "Button")]);
    let operation = automation
        .begin_operation()
        .expect("first mutation should acquire the operation guard");

    assert!(matches!(
        automation.begin_operation(),
        Err(StorybookAutomationError::AutomationBusy)
    ));
    assert_eq!(automation.stories().len(), 1, "reads remain available");

    drop(operation);
    assert!(automation.begin_operation().is_ok());
}

#[gpui::test]
fn default_automation_round_trips_through_app_global(cx: &mut App) {
    assert!(default_storybook_automation(cx).is_none());
    let automation = StorybookAutomation::new();
    let installed = set_default_storybook_automation(cx, automation.clone());
    let restored = default_storybook_automation(cx).expect("automation should be installed");

    assert!(Arc::ptr_eq(&automation, &installed));
    assert!(Arc::ptr_eq(&automation, &restored));
    let wrapper = DefaultStorybookAutomation::new(automation.clone());
    assert!(Arc::ptr_eq(&wrapper.automation(), &automation));
}

#[test]
fn default_sizes_paths_and_capture_validation_are_stable() {
    let story = sample_story("crate-ButtonStory/with-icon", "Button");
    assert_eq!(
        StoryDefaultSize::default(),
        StoryDefaultSize {
            width: DEFAULT_STORY_CAPTURE_WIDTH,
            height: DEFAULT_STORY_CAPTURE_HEIGHT,
        }
    );
    assert_eq!(
        default_capture_output_path(&story),
        PathBuf::from("target/storybook-captures/crate-ButtonStory/with-icon.png")
    );

    assert_eq!(
        validate_capture_target_size(&StoryScreenshotRequest::default()),
        Ok(None)
    );
    assert_eq!(
        validate_capture_target_size(&StoryScreenshotRequest {
            width: Some(800),
            height: Some(600),
            ..StoryScreenshotRequest::default()
        }),
        Ok(Some((800, 600)))
    );
    assert_eq!(
        validate_capture_target_size(&StoryScreenshotRequest {
            viewport: Some(StoryViewportPreset::Mobile),
            ..StoryScreenshotRequest::default()
        }),
        Ok(Some((390, 844)))
    );
    assert!(matches!(
        validate_capture_target_size(&StoryScreenshotRequest {
            width: Some(0),
            height: Some(600),
            ..StoryScreenshotRequest::default()
        }),
        Err(StorybookAutomationError::InvalidCaptureRequest { message })
            if message.contains("greater than zero")
    ));
    assert!(matches!(
        validate_capture_target_size(&StoryScreenshotRequest {
            width: Some(800),
            ..StoryScreenshotRequest::default()
        }),
        Err(StorybookAutomationError::InvalidCaptureRequest { message })
            if message.contains("provided together")
    ));
}

#[test]
fn capture_target_size_only_expands_a_clipped_story_region() {
    let window = gpui::size(px(800.), px(600.));
    assert_eq!(
        expanded_window_size(
            window,
            gpui::Bounds {
                origin: gpui::point(px(200.), px(100.)),
                size: gpui::size(px(400.), px(300.)),
            },
        ),
        None
    );
    assert_eq!(
        expanded_window_size(
            window,
            gpui::Bounds {
                origin: gpui::point(px(300.), px(150.)),
                size: gpui::size(px(600.), px(500.)),
            },
        ),
        Some(gpui::size(px(900.), px(650.)))
    );
}

#[test]
fn automation_errors_have_actionable_messages_and_exit_codes() {
    let errors = [
        (
            StorybookAutomationError::NoLiveHost,
            "no live GPUI storybook host is attached",
        ),
        (
            StorybookAutomationError::HostDisconnected {
                message: "closed".to_string(),
                steps_dispatched: 2,
            },
            "live GPUI storybook host disconnected after 2 dispatched step(s): closed",
        ),
        (
            StorybookAutomationError::StoryNotFound {
                key: "missing".to_string(),
            },
            "story route `missing` was not found",
        ),
        (
            StorybookAutomationError::AutomationBusy,
            "another storybook automation mutation is already active",
        ),
        (
            StorybookAutomationError::CaptureUnavailable {
                message: "unavailable".to_string(),
            },
            "unavailable",
        ),
        (
            StorybookAutomationError::InvalidCaptureRequest {
                message: "invalid".to_string(),
            },
            "invalid",
        ),
        (
            StorybookAutomationError::InteractionTargetsUnavailable {
                route: "story/section".to_string(),
            },
            "interaction targets are unavailable because route `story/section` is not rendered",
        ),
        (
            StorybookAutomationError::InteractionTargetNotFound {
                route: "story".to_string(),
                key: "execute".to_string(),
            },
            "interaction target `execute` was not found in route `story`",
        ),
        (
            StorybookAutomationError::DuplicateInteractionTarget {
                route: "story".to_string(),
                key: "execute".to_string(),
            },
            "interaction target `execute` is duplicated in route `story`",
        ),
        (
            StorybookAutomationError::SemanticValuesUnavailable {
                route: "story/section".to_string(),
            },
            "semantic values are unavailable because route `story/section` is not rendered",
        ),
        (
            StorybookAutomationError::DuplicateSemanticValue {
                route: "story".to_string(),
                key: "response".to_string(),
            },
            "semantic value `response` is duplicated in route `story`",
        ),
    ];

    for (error, expected) in errors {
        assert_eq!(error.to_string(), expected);
    }

    let successful = Ok(StoryCaptureSnapshot {
        request_id: 1,
        path: PathBuf::from("capture.png"),
        pixel_width: 1,
        pixel_height: 1,
        story: sample_story("crate-ButtonStory", "Button"),
    });
    assert_eq!(capture_exit_code(&successful), 0);
    assert_eq!(
        capture_exit_code(&Err(StorybookAutomationError::CaptureUnavailable {
            message: "unavailable".to_string(),
        })),
        1
    );
}

#[cfg(feature = "capture")]
#[test]
fn image_crop_rect_scales_clamps_and_rejects_empty_regions() {
    let image = image::RgbaImage::new(200, 100);
    let window_size = gpui::size(px(100.), px(50.));

    assert_eq!(
        image_crop_rect(
            Bounds {
                origin: point(px(10.), px(5.)),
                size: gpui::size(px(40.), px(20.)),
            },
            window_size,
            &image,
        ),
        Some((20, 10, 80, 40))
    );
    assert_eq!(
        image_crop_rect(
            Bounds {
                origin: point(px(-10.), px(-5.)),
                size: gpui::size(px(200.), px(100.)),
            },
            window_size,
            &image,
        ),
        Some((0, 0, 200, 100))
    );
    assert_eq!(
        image_crop_rect(Bounds::default(), window_size, &image),
        None
    );
    assert_eq!(
        image_crop_rect(Bounds::default(), gpui::size(px(0.), px(50.)), &image,),
        None
    );
    assert_eq!(
        image_crop_rect(Bounds::default(), window_size, &image::RgbaImage::new(0, 0)),
        None
    );
}
