use gpui::{App, Bounds, WindowBounds, WindowKind, WindowOptions, px, size};
use gpui_component::TitleBar;

pub(crate) fn default_storybook_window_options(cx: &App) -> WindowOptions {
    let mut window_size = size(px(1600.0), px(1200.0));
    if let Some(display) = cx.primary_display() {
        let display_size = display.bounds().size;
        window_size.width = window_size.width.min(display_size.width * 0.85);
        window_size.height = window_size.height.min(display_size.height * 0.85);
    }
    let window_bounds = Bounds::centered(None, window_size, cx);

    WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(window_bounds)),
        titlebar: Some(TitleBar::title_bar_options()),
        window_min_size: Some(gpui::Size {
            width: px(640.),
            height: px(480.),
        }),
        kind: WindowKind::Normal,
        #[cfg(target_os = "linux")]
        window_background: gpui::WindowBackgroundAppearance::Transparent,
        #[cfg(target_os = "linux")]
        window_decorations: Some(gpui::WindowDecorations::Client),
        ..Default::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[gpui::test]
    fn default_window_options_set_storybook_size_and_chrome(cx: &mut App) {
        let expected_size = cx
            .primary_display()
            .map_or(size(px(1600.), px(1200.)), |display| {
                let display_size = display.bounds().size;
                size(
                    px(1600.).min(display_size.width * 0.85),
                    px(1200.).min(display_size.height * 0.85),
                )
            });
        let options = default_storybook_window_options(cx);
        let Some(WindowBounds::Windowed(bounds)) = options.window_bounds else {
            panic!("storybook window should use windowed bounds");
        };

        assert_eq!(bounds.size, expected_size);
        assert_eq!(
            options.window_min_size,
            Some(gpui::size(px(640.), px(480.)))
        );
        assert_eq!(options.kind, WindowKind::Normal);
        assert!(options.titlebar.is_some());
        #[cfg(target_os = "linux")]
        {
            assert_eq!(
                options.window_background,
                gpui::WindowBackgroundAppearance::Transparent
            );
            assert_eq!(
                options.window_decorations,
                Some(gpui::WindowDecorations::Client)
            );
        }
    }
}
