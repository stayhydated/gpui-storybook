use std::path::PathBuf;

use gpui::App;
use gpui_component::ThemeRegistry;

const THEMES_DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/assets/themes");

pub fn init(cx: &mut App) {
    #[cfg(debug_assertions)]
    {
        let themes_dir = PathBuf::from(THEMES_DIR);
        if themes_dir.exists()
            && let Err(err) = ThemeRegistry::watch_dir(themes_dir, cx, |cx| {
                crate::preferences::theme_registry_changed(cx);
                cx.refresh_windows();
            })
        {
            tracing::error!(error = %err, "failed to watch Storybook themes directory");
        }
    }
}
