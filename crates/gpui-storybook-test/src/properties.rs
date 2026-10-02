use super::*;
use proptest::prelude::*;
use std::{collections::BTreeSet, path::Component};

fn text() -> impl Strategy<Value = String> {
    prop::collection::vec(any::<char>(), 0..33)
        .prop_map(|characters| characters.into_iter().collect())
}

// Read percent escapes rather than reproducing the production encoder's byte
// whitelist. Malformed escapes must fail instead of being silently normalized.
fn decode_fragment(encoded: &str) -> Vec<u8> {
    let mut bytes = encoded.as_bytes().iter().copied();
    let mut decoded = Vec::new();
    while let Some(byte) = bytes.next() {
        if byte == b'%' {
            let digits = [bytes.next().unwrap(), bytes.next().unwrap()];
            let digits = std::str::from_utf8(&digits).unwrap();
            decoded.push(u8::from_str_radix(digits, 16).unwrap());
        } else {
            decoded.push(byte);
        }
    }
    decoded
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(128))]

    #[test]
    fn filename_encoding_is_reversible_safe_and_case_distinct(value in text()) {
        let encoded = encode_id_fragment(&value);
        prop_assert_eq!(decode_fragment(&encoded), value.as_bytes());
        let filename = case_file_name(&value);
        prop_assert_eq!(decode_fragment(filename.strip_prefix("id-").unwrap()), value.as_bytes());
        let components = Path::new(&filename).components().collect::<Vec<_>>();
        prop_assert_eq!(components.len(), 1);
        prop_assert!(matches!(components[0], Component::Normal(_)));

        // Appending NUL constructs distinct inputs throughout shrinking.
        // This checks encoded values only; no filesystem accepts a literal NUL.
        let extended = format!("{value}\0");
        prop_assert_ne!(filename.to_ascii_lowercase(), case_file_name(&extended).to_ascii_lowercase());
        prop_assert_ne!(case_file_name(&format!("{value}A")).to_ascii_lowercase(),
            case_file_name(&format!("{value}a")).to_ascii_lowercase());
    }

    #[test]
    fn small_matrices_keep_axis_values_cardinality_and_order(
        story_count in 1usize..4,
        viewport_inputs in prop::collection::vec((text(), 1u32..2049, 1u32..2049), 0..3),
        theme_inputs in prop::collection::vec(text(), 0..3),
        language_inputs in prop::collection::vec(text(), 0..3),
        control_inputs in prop::collection::vec(text(), 0..3),
        selected in any::<bool>(),
    ) {
        let discovered = (0..story_count)
            .map(|index| StoryDescriptor::for_test(&format!("Story-{index}"), "Story"))
            .collect::<Vec<_>>();
        // Index prefixes preserve unique, nonempty axis labels while payloads
        // and lengths shrink. At most 96 cases expand; no stories are rendered.
        let mut matrix = CaptureMatrix::new()
            .route(RouteCase::root())
            .route(RouteCase::substory("State"))
            .output_dir("captures");
        if selected {
            matrix.story_keys = discovered.iter().rev().map(|story| story.key().to_owned()).collect();
        }
        matrix.viewports = viewport_inputs.into_iter().enumerate()
            .map(|(index, (label, width, height))| ViewportCase::new(format!("viewport-{index}/{label}"), width, height))
            .collect();
        matrix.themes = theme_inputs.into_iter().enumerate()
            .map(|(index, label)| ThemeCase::named(format!("theme-{index}/{label}"))).collect();
        matrix.languages = language_inputs.into_iter().enumerate()
            .map(|(index, label)| LanguageCase::named(format!("language-{index}/{label}"))).collect();
        matrix.control_cases = control_inputs.into_iter().enumerate()
            .map(|(index, label)| ControlCase::new(format!("controls-{index}/{label}"), BTreeMap::new())).collect();
        let per_story = 2 * matrix.viewports.len().max(1) * matrix.themes.len().max(1)
            * matrix.languages.len().max(1) * matrix.control_cases.len().max(1);
        let cases = matrix.expand(&discovered).unwrap();
        prop_assert_eq!(cases.len(), story_count * per_story);
        prop_assert_eq!(&cases, &matrix.expand(&discovered).unwrap());
        prop_assert!(cases.windows(2).all(|pair| pair[0].id < pair[1].id));
        let mut reordered = matrix.clone();
        reordered.story_keys.reverse();
        reordered.viewports.reverse();
        reordered.themes.reverse();
        reordered.languages.reverse();
        reordered.control_cases.reverse();
        let mut reversed_discovery = discovered.clone();
        reversed_discovery.reverse();
        prop_assert_eq!(&cases, &reordered.expand(&reversed_discovery).unwrap());
        let ids = cases.iter().map(|case| &case.id).collect::<BTreeSet<_>>();
        prop_assert_eq!(ids.len(), cases.len());
        let tuples = cases.iter().map(|case| (
            case.story_key.as_str(), case.route.label(), case.viewport.name.as_str(),
            case.theme.name.as_str(), case.language.name.as_str(), case.controls.name.as_str(),
        )).collect::<BTreeSet<_>>();
        prop_assert_eq!(tuples.len(), cases.len());
        for story in &discovered {
            prop_assert_eq!(cases.iter().filter(|case| case.story_key == story.key()).count(), per_story);
        }
        let viewports = if matrix.viewports.is_empty() { vec![ViewportCase::default()] } else { matrix.viewports };
        let themes = if matrix.themes.is_empty() { vec![ThemeCase::default()] } else { matrix.themes };
        let languages = if matrix.languages.is_empty() { vec![LanguageCase::default()] } else { matrix.languages };
        let controls = if matrix.control_cases.is_empty() { vec![ControlCase::defaults()] } else { matrix.control_cases };
        for case in &cases {
            let expected_route = match &case.route {
                RouteCase::Root => case.story_key.clone(),
                RouteCase::Substory { key } => {
                    prop_assert_eq!(key, "State");
                    format!("{}/state", case.story_key)
                }
            };
            prop_assert_eq!(&case.route_id, &expected_route);
            prop_assert!(viewports.contains(&case.viewport));
            prop_assert!(themes.contains(&case.theme));
            prop_assert!(languages.contains(&case.language));
            prop_assert!(controls.contains(&case.controls));
            prop_assert_eq!(case.id.split('/').count(), 9);
            let output = case.output_path.as_ref().unwrap();
            prop_assert_eq!(output.parent(), Some(Path::new("captures")));
            let stem = output.file_stem().unwrap().to_str().unwrap();
            prop_assert_eq!(decode_fragment(stem.strip_prefix("id-").unwrap()), case.id.as_bytes());
        }
    }
}
