use super::*;

#[test]
fn workbench_tabs_have_stable_persisted_indices() {
    for (tab, index) in [
        (WorkbenchTab::Controls, 0),
        (WorkbenchTab::Theme, 1),
        (WorkbenchTab::Inspect, 2),
        (WorkbenchTab::Actions, 3),
        (WorkbenchTab::Scenarios, 4),
        #[cfg(feature = "performance")]
        (WorkbenchTab::Performance, 5),
    ] {
        assert_eq!(tab.index(), index);
        assert_eq!(WorkbenchTab::from_index(index), tab);
    }
}

#[gpui_kit::test]
fn grouped_story_select_targets_one_concrete_variant(cx: &mut TestAppContext) {
    cx.update(gpui_kit::init);
    let window = cx.update(|cx| {
        cx.open_window(Default::default(), |window, cx| {
            let mut variant = |description: &str, klass: &str, cx: &mut App| {
                cx.new(|cx| {
                    let mut story = StoryContainer::new(window, cx);
                    story.name = "Button".into();
                    story.description = description.to_owned().into();
                    story.story_klass = Some(klass.to_owned().into());
                    story
                })
            };
            let primary = variant("Primary variant", "PrimaryButtonStory", cx);
            let danger = variant("Danger variant", "DangerButtonStory", cx);
            let group = StoryContainer::variant_group("Button", vec![primary, danger], window, cx);
            let state = cx.new(|cx| WorkbenchState::new(None, cx));
            state.update(cx, |state, cx| {
                state.set_active_story(Some(group), cx);
            });
            cx.new(|cx| StoryWorkbench::new(state, WorkbenchTab::Controls, window, cx))
        })
        .expect("grouped workbench window should open")
    });
    let mut visual_cx = VisualTestContext::from_window(window.into(), cx);
    let workbench = window
        .root(&mut visual_cx)
        .expect("workbench should be the window root");
    visual_cx.update(|window, cx| {
        _ = window.draw(cx);
    });

    let (select, danger_id) = workbench.read_with(&visual_cx, |workbench, _| {
        (
            workbench.variant_select.clone(),
            workbench.variant_options[1].id,
        )
    });
    assert!(
        visual_cx.debug_bounds("workbench-variant-select").is_some(),
        "grouped stories should render a variant select"
    );
    visual_cx.update(|_, cx| {
        select.update(cx, |_, cx| {
            cx.emit(SelectEvent::Confirm(Some(danger_id)));
        });
    });
    let (active_story, active_group, variant_count) =
        workbench.read_with(&visual_cx, |workbench, cx| {
            let state = workbench.state.read(cx);
            (
                state.active_story().map(|story| story.entity_id()),
                state.active_group().map(|story| story.entity_id()),
                state.variants(cx).len(),
            )
        });

    assert_eq!(active_story, Some(danger_id));
    assert_ne!(active_group, active_story);
    assert_eq!(variant_count, 2);
}

#[test]
fn persisted_panel_state_restores_the_selected_tab() {
    let info = PanelInfo::panel(
        serde_json::to_value(StoryWorkbenchPanelState {
            selected_tab: WorkbenchTab::Theme,
        })
        .expect("panel state serializes"),
    );
    assert_eq!(
        StoryWorkbench::selected_tab_from_panel(&info),
        WorkbenchTab::Theme
    );
}

#[gpui_kit::test]
fn window_scoped_states_keep_preview_independent(cx: &mut App) {
    let first = cx.new(|cx| WorkbenchState::new(None, cx));
    let second = cx.new(|cx| WorkbenchState::new(None, cx));

    first.update(cx, |state, cx| {
        state.set_viewport(StoryViewportPreset::Mobile, cx);
        state.set_background(StoryCanvasBackground::Dark, cx);
    });

    assert_eq!(
        first.read(cx).presentation(),
        StoryPresentation {
            viewport: StoryViewportPreset::Mobile,
            background: StoryCanvasBackground::Dark,
        }
    );
    assert_eq!(first.read(cx).responsive_size(), None);
    assert_eq!(second.read(cx).presentation(), StoryPresentation::default());
}

#[gpui_kit::test]
fn responsive_viewport_inherits_the_previous_fixed_preset(cx: &mut App) {
    let state = cx.new(|cx| WorkbenchState::new(None, cx));

    state.update(cx, |state, cx| {
        state.set_viewport(StoryViewportPreset::Mobile, cx);
        state.set_viewport(StoryViewportPreset::Responsive, cx);
    });
    assert_eq!(
        state.read(cx).responsive_size(),
        Some(size(px(390.), px(844.)))
    );

    state.update(cx, |state, cx| {
        state.set_viewport(StoryViewportPreset::Desktop, cx);
        state.set_viewport(StoryViewportPreset::Responsive, cx);
    });
    assert_eq!(
        state.read(cx).responsive_size(),
        Some(size(px(1440.), px(900.)))
    );
}

#[test]
fn story_source_url_resolves_workspace_relative_files_from_the_crate_directory() {
    let workspace = tempfile::tempdir().expect("temporary workspace");
    let crate_dir = workspace.path().join("examples/story");
    let source_file = workspace.path().join("examples/story/src/lib.rs");
    std::fs::create_dir_all(source_file.parent().expect("source parent"))
        .expect("create source directory");
    std::fs::File::create(&source_file).expect("create source file");

    assert_eq!(
        story_source_url(
            crate_dir.to_str().expect("UTF-8 crate directory"),
            "examples/story/src/lib.rs",
        ),
        Some(
            url::Url::from_file_path(source_file)
                .expect("source file URL")
                .into(),
        )
    );
}
