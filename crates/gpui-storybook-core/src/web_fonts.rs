#[cfg(target_family = "wasm")]
use std::borrow::Cow;

use gpui::App;
#[cfg(any(test, target_family = "wasm"))]
use gpui_component::theme::Theme;

#[cfg(any(test, target_family = "wasm"))]
const UI_FONT_FAMILY: &str = "Noto Sans SC";
#[cfg(any(test, target_family = "wasm"))]
const MONO_FONT_FAMILY: &str = "JetBrains Mono";

pub(super) fn init(cx: &mut App) {
    #[cfg(target_family = "wasm")]
    {
        let fonts = vec![
            Cow::Borrowed(
                include_bytes!(concat!(
                    env!("GPUI_COMPONENT_STORY_FONTS_DIR"),
                    "/NotoSansSC-Regular-subset.ttf"
                ))
                .as_slice(),
            ),
            Cow::Borrowed(
                include_bytes!(concat!(
                    env!("GPUI_COMPONENT_STORY_FONTS_DIR"),
                    "/NotoEmoji-Regular.ttf"
                ))
                .as_slice(),
            ),
            Cow::Borrowed(
                include_bytes!(concat!(
                    env!("GPUI_COMPONENT_STORY_FONTS_DIR"),
                    "/JetBrainsMono-Regular.ttf"
                ))
                .as_slice(),
            ),
        ];
        cx.text_system()
            .add_fonts(fonts)
            .expect("gpui-component web fonts should load");
        apply_font_families(cx);
    }

    #[cfg(not(target_family = "wasm"))]
    let _ = cx;
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

    #[gpui::test]
    fn web_font_families_match_the_embedded_gpui_component_fonts(cx: &mut App) {
        gpui_component::init(cx);
        apply_font_families(cx);

        let theme = Theme::global(cx);
        assert_eq!(theme.font_family, UI_FONT_FAMILY);
        assert_eq!(theme.mono_font_family, MONO_FONT_FAMILY);
    }
}
