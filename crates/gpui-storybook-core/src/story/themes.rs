#[cfg(all(debug_assertions, not(target_family = "wasm")))]
use std::path::PathBuf;

use gpui::App;
#[cfg(all(debug_assertions, not(target_family = "wasm")))]
use gpui_component::ThemeRegistry;

#[cfg(all(debug_assertions, not(target_family = "wasm")))]
const THEMES_DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/assets/themes");

/// Native debug environment variable selecting the complete consumer theme
/// directory.
pub const STORYBOOK_THEME_DIR_ENV: &str = "STORYBOOK_THEME_DIR";

pub fn init(cx: &mut App) {
    #[cfg(all(debug_assertions, not(target_family = "wasm")))]
    {
        // ThemeRegistry owns one watched directory. A consumer override therefore
        // becomes the complete custom-theme source for this Storybook process.
        let themes_dir = std::env::var_os(STORYBOOK_THEME_DIR_ENV)
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from(THEMES_DIR));
        if !themes_dir.exists() {
            tracing::warn!(path = %themes_dir.display(), "Storybook theme directory does not exist");
            return;
        }
        if let Err(err) = ThemeRegistry::watch_dir(themes_dir.clone(), cx, |cx| {
            crate::preferences::theme_registry_changed(cx);
            cx.refresh_windows();
        }) {
            tracing::error!(
                error = %err,
                path = %themes_dir.display(),
                "failed to watch Storybook themes directory"
            );
        }
    }

    #[cfg(any(not(debug_assertions), target_family = "wasm"))]
    let _ = cx;
}
