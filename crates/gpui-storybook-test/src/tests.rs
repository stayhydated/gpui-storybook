use super::*;
#[cfg(not(target_family = "wasm"))]
use std::sync::atomic::{AtomicBool, Ordering};

#[cfg(not(target_family = "wasm"))]
static TOKIO_STORY_INIT_RAN: AtomicBool = AtomicBool::new(false);

#[cfg(not(target_family = "wasm"))]
fn tokio_story_init(app: &mut App) {
    let _handle = gpui_tokio::Tokio::handle(app);
    TOKIO_STORY_INIT_RAN.store(true, Ordering::SeqCst);
}

#[cfg(not(target_family = "wasm"))]
inventory::submit! {
    InitEntry {
        init_fn: tokio_story_init,
        fn_name: "tokio_story_init",
        file: file!(),
        line: line!(),
    }
}

#[cfg(not(target_family = "wasm"))]
#[gpui_kit::test]
fn portable_runtime_installs_tokio_before_story_init_hooks(cx: &mut App) {
    TOKIO_STORY_INIT_RAN.store(false, Ordering::SeqCst);

    initialize_portable_story_app(cx).expect("portable runtime should initialize");

    assert!(TOKIO_STORY_INIT_RAN.load(Ordering::SeqCst));
}

#[gpui_kit::test]
fn story_without_control_target_has_an_empty_snapshot(cx: &mut App) {
    let snapshots = read_control_snapshots(None, cx).expect("missing controls should be empty");

    assert!(snapshots.is_empty());
}

#[test]
fn custom_substory_capture_owns_route_verification() {
    let route = RouteCase::Substory {
        key: "application-surface".to_owned(),
    };

    assert!(!uses_core_route_registry(&route, true));
    assert!(uses_core_route_registry(&route, false));
    assert!(!uses_core_route_registry(&RouteCase::Root, false));
}

#[test]
fn case_file_name_is_stable_and_safe() {
    assert_eq!(
        case_file_name("crate/Button root"),
        "id-crate%2F%42utton%20root"
    );
    assert_eq!(case_file_name(""), "id-");
    assert_ne!(case_file_name("a b"), case_file_name("a?b"));
    assert_ne!(
        case_file_name("A").to_ascii_lowercase(),
        case_file_name("a").to_ascii_lowercase()
    );
}

#[test]
fn explicit_settle_frames_override_the_runner_default() {
    let runner = HeadlessStoryRunner::new(RunnerConfig::default().settle_frames(5));
    let mut request = CaptureRequest::new("crate-Button");
    request.settle_frames = 2;
    let case = runner.request_case(request).unwrap();

    assert_eq!(effective_settle_frames(case.settle_frames, 5, None), 2);
    assert_eq!(effective_settle_frames(0, 5, None), 5);
}

#[test]
fn performance_frames_remain_a_minimum_after_settle_resolution() {
    let performance = PerformanceOptions::new().measured_frames(4);

    assert_eq!(effective_settle_frames(2, 5, Some(&performance)), 4);
    assert_eq!(effective_settle_frames(6, 5, Some(&performance)), 6);
}

#[test]
fn named_theme_and_language_require_a_configurator() {
    let request = CaptureRequest::new("crate-Button");
    let mut case = HeadlessStoryRunner::default()
        .request_case(request)
        .unwrap();
    case.theme = ThemeCase::named("Consumer Theme");
    let error = validate_case_configuration(&case, &RunnerConfig::default()).unwrap_err();
    assert!(matches!(
        error,
        StorybookTestError::CaseConfigurationRequired { axis } if axis == "theme"
    ));
}

#[test]
fn default_matrix_axes_do_not_require_callbacks() {
    let case = HeadlessStoryRunner::default()
        .request_case(CaptureRequest::new("crate-Button"))
        .unwrap();
    validate_case_configuration(&case, &RunnerConfig::default()).unwrap();
}

#[test]
fn built_in_theme_modes_do_not_require_callbacks() {
    let mut case = HeadlessStoryRunner::default()
        .request_case(CaptureRequest::new("crate-Button"))
        .unwrap();
    for theme in ["light", "Default Dark"] {
        case.theme = ThemeCase::named(theme);
        validate_case_configuration(&case, &RunnerConfig::default()).unwrap();
    }
}
