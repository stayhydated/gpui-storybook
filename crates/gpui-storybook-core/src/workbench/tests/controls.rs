use super::*;

#[gpui_kit::test]
fn external_story_recreation_rebinds_control_editor_subscriptions(cx: &mut TestAppContext) {
    let window = cx.update(|cx| {
        gpui_kit::init(cx);
        cx.open_window(Default::default(), |window, cx| {
            cx.new(|cx| RecreatedControlWorkbenchFixture::new(window, cx))
        })
        .expect("recreated control fixture window")
    });
    let mut visual_cx = VisualTestContext::from_window(window.into(), cx);
    let fixture = window
        .root(&mut visual_cx)
        .expect("recreated control fixture should be the window root");
    let (story, workbench) = fixture.read_with(&visual_cx, |fixture, _| {
        (fixture.story.clone(), fixture.workbench.clone())
    });
    visual_cx.update(|window, cx| {
        _ = window.draw(cx);
    });
    let original_editor_ids = workbench.read_with(&visual_cx, |workbench, _| {
        workbench
            .editors
            .iter()
            .map(|(key, editor)| {
                let id = match editor {
                    ControlEditor::Text(state) | ControlEditor::Number { state, .. } => {
                        state.entity_id()
                    },
                    ControlEditor::Range { state, .. } => state.entity_id(),
                    ControlEditor::Color(state) => state.entity_id(),
                };
                (key.clone(), id)
            })
            .collect::<BTreeMap<_, _>>()
    });

    visual_cx.update(|window, cx| {
        story.update(cx, |story, cx| {
            assert!(story.recreate_for_scenario(window, cx));
        });
    });
    visual_cx.run_until_parked();
    visual_cx.update(|window, cx| {
        _ = window.draw(cx);
    });

    let (recreated_editor_ids, text, number, range, color) =
        workbench.read_with(&visual_cx, |workbench, _| {
            let ids = workbench
                .editors
                .iter()
                .map(|(key, editor)| {
                    let id = match editor {
                        ControlEditor::Text(state) | ControlEditor::Number { state, .. } => {
                            state.entity_id()
                        },
                        ControlEditor::Range { state, .. } => state.entity_id(),
                        ControlEditor::Color(state) => state.entity_id(),
                    };
                    (key.clone(), id)
                })
                .collect::<BTreeMap<_, _>>();
            let Some(ControlEditor::Text(text)) = workbench.editors.get("label") else {
                panic!("recreated text control editor should exist");
            };
            let Some(ControlEditor::Number { state: number, .. }) = workbench.editors.get("count")
            else {
                panic!("recreated number control editor should exist");
            };
            let Some(ControlEditor::Range { state: range, .. }) = workbench.editors.get("ratio")
            else {
                panic!("recreated range control editor should exist");
            };
            let Some(ControlEditor::Color(color)) = workbench.editors.get("tint") else {
                panic!("recreated color control editor should exist");
            };
            (
                ids,
                text.clone(),
                number.clone(),
                range.clone(),
                color.clone(),
            )
        });
    assert_eq!(original_editor_ids.len(), 4);
    assert_eq!(recreated_editor_ids.len(), 4);
    for (key, original_id) in original_editor_ids {
        assert_ne!(recreated_editor_ids.get(&key), Some(&original_id), "{key}");
    }
    assert_eq!(
        story.read_with(&visual_cx, |story, _| story.recreation_generation()),
        1
    );

    let tint = gpui_kit::Hsla {
        h: 0.5,
        s: 0.6,
        l: 0.4,
        a: 0.8,
    };
    visual_cx.update(|window, cx| {
        text.update(cx, |editor, cx| {
            editor.set_value("rebound", window, cx);
            cx.emit(InputEvent::Change);
        });
        number.update(cx, |editor, cx| {
            editor.set_value("9", window, cx);
            cx.emit(InputEvent::Change);
        });
        range.update(cx, |_, cx| {
            cx.emit(SliderEvent::Change(0.75.into()));
        });
        color.update(cx, |_, cx| {
            cx.emit(ColorPickerEvent::Change(Some(tint)));
        });
    });
    visual_cx.run_until_parked();

    let values = visual_cx.update(|_, cx| {
        let target = story
            .read(cx)
            .control_target()
            .expect("recreated story should expose controls");
        ["label", "count", "ratio", "tint"].map(|key| {
            target
                .value(key, cx)
                .expect("control should remain readable")
        })
    });
    assert_eq!(
        values,
        [
            ControlValue::Text("rebound".to_owned()),
            ControlValue::Integer(9),
            ControlValue::Float(0.75),
            ControlValue::Color(tint.into()),
        ]
    );
}
