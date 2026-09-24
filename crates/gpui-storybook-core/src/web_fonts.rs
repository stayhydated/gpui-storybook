#[cfg(target_family = "wasm")]
use std::borrow::Cow;

#[cfg(any(test, target_family = "wasm"))]
use base64::{Engine as _, engine::general_purpose::STANDARD};
use gpui_kit::App;
#[cfg(any(test, target_family = "wasm"))]
use gpui_kit::component::theme::Theme;

#[cfg(any(test, target_family = "wasm"))]
const UI_FONT_FAMILY: &str = "Noto Sans SC Thin";
#[cfg(any(test, target_family = "wasm"))]
const MONO_FONT_FAMILY: &str = "JetBrains Mono";
#[cfg(any(test, target_family = "wasm"))]
const SYSTEM_UI_FONT_FAMILY: &str = "IBM Plex Sans";

#[cfg(any(test, target_family = "wasm"))]
fn embedded_web_fonts() -> [&'static str; 4] {
    [
        include_str!("../assets/fonts/NotoSansSC-Regular-subset.ttf.base64"),
        include_str!("../assets/fonts/NotoEmoji-Regular.ttf.base64"),
        include_str!("../assets/fonts/JetBrainsMono-Regular.ttf.base64"),
        include_str!("../assets/fonts/IBMPlexSans-Regular.ttf.base64"),
    ]
}

pub(super) fn init(cx: &mut App) {
    #[cfg(target_family = "wasm")]
    {
        let fonts = embedded_web_fonts()
            .into_iter()
            .map(|encoded| Cow::Owned(decode_font(encoded)))
            .collect();
        cx.text_system()
            .add_fonts(fonts)
            .expect("GPUI Kit web fonts should load");
        apply_font_families(cx);
    }

    #[cfg(not(target_family = "wasm"))]
    let _ = cx;
}

#[cfg(any(test, target_family = "wasm"))]
fn decode_font(encoded: &str) -> Vec<u8> {
    let compact = encoded
        .bytes()
        .filter(|byte| !byte.is_ascii_whitespace())
        .collect::<Vec<_>>();
    STANDARD
        .decode(compact)
        .expect("embedded web font is valid base64")
}

#[cfg(any(test, target_family = "wasm"))]
fn apply_font_families(cx: &mut App) {
    let theme = Theme::global_mut(cx);
    theme.font_family = UI_FONT_FAMILY.into();
    theme.mono_font_family = MONO_FONT_FAMILY.into();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[gpui_kit::test]
    fn web_font_families_match_the_embedded_gpui_kit_fonts(cx: &mut App) {
        gpui_kit::init(cx);
        apply_font_families(cx);

        let theme = Theme::global(cx);
        assert_eq!(theme.font_family, UI_FONT_FAMILY);
        assert_eq!(theme.mono_font_family, MONO_FONT_FAMILY);
    }

    fn family_name(face: &ttf_parser::Face<'_>) -> Option<String> {
        let names = face.names();
        [
            ttf_parser::name_id::TYPOGRAPHIC_FAMILY,
            ttf_parser::name_id::FAMILY,
        ]
        .into_iter()
        .find_map(|wanted| {
            names
                .into_iter()
                .filter(|name| name.name_id == wanted)
                .find_map(|name| name.to_string())
        })
    }

    #[test]
    fn embedded_web_fonts_decode_and_expose_the_theme_families() {
        let decoded = embedded_web_fonts().map(decode_font);
        let faces = decoded
            .iter()
            .map(|font| ttf_parser::Face::parse(font, 0).expect("embedded web font should parse"))
            .collect::<Vec<_>>();

        for face in &faces {
            assert!(
                face.tables().glyf.is_some(),
                "embedded web fonts should carry TrueType outlines"
            );
        }
        assert_eq!(family_name(&faces[0]).as_deref(), Some(UI_FONT_FAMILY));
        assert_eq!(family_name(&faces[1]).as_deref(), Some("Noto Emoji"));
        assert_eq!(family_name(&faces[2]).as_deref(), Some(MONO_FONT_FAMILY));
        assert_eq!(
            family_name(&faces[3]).as_deref(),
            Some(SYSTEM_UI_FONT_FAMILY)
        );
    }
}
