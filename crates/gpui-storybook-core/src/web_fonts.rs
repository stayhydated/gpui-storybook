#[cfg(target_family = "wasm")]
use std::borrow::Cow;

#[cfg(target_family = "wasm")]
use base64::{Engine as _, engine::general_purpose::STANDARD};
use gpui_kit::App;
#[cfg(any(test, target_family = "wasm"))]
use gpui_kit::component::theme::Theme;

#[cfg(any(test, target_family = "wasm"))]
const UI_FONT_FAMILY: &str = "Noto Sans SC";
#[cfg(any(test, target_family = "wasm"))]
const MONO_FONT_FAMILY: &str = "JetBrains Mono";

pub(super) fn init(cx: &mut App) {
    #[cfg(target_family = "wasm")]
    {
        let fonts = vec![
            decode_font(include_str!(
                "../assets/fonts/NotoSansSC-Regular-subset.ttf.base64"
            )),
            decode_font(include_str!("../assets/fonts/NotoEmoji-Regular.ttf.base64")),
            decode_font(include_str!(
                "../assets/fonts/JetBrainsMono-Regular.ttf.base64"
            )),
        ];
        cx.text_system()
            .add_fonts(fonts)
            .expect("GPUI Kit web fonts should load");
        apply_font_families(cx);
    }

    #[cfg(not(target_family = "wasm"))]
    let _ = cx;
}

#[cfg(target_family = "wasm")]
fn decode_font(encoded: &'static str) -> Cow<'static, [u8]> {
    Cow::Owned(
        STANDARD
            .decode(encoded)
            .expect("embedded web font is valid base64"),
    )
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
}
