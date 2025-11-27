use anyhow::anyhow;

use gpui::AssetSource;
use rust_embed::RustEmbed;

pub use gpui_component_assets::Assets as ComponentAssets;

#[derive(RustEmbed)]
#[folder = "assets"]
#[include = "i18n/**/*"]
#[include = "themes/**/*"]
#[exclude = "*.DS_Store"]
pub struct LocalAssets;

pub struct Assets;

impl AssetSource for Assets {
    fn load(&self, path: &str) -> gpui::Result<Option<std::borrow::Cow<'static, [u8]>>> {
        if path.is_empty() {
            return Ok(None);
        }

        if path.starts_with("icons/") {
            return ComponentAssets.load(path);
        }

        LocalAssets::get(path)
            .map(|f| Some(f.data))
            .ok_or_else(|| anyhow!("could not find asset at path \"{path}\""))
    }

    fn list(&self, path: &str) -> gpui::Result<Vec<gpui::SharedString>> {
        let mut results = Vec::new();

        if path.is_empty() || path.starts_with("icons") || "icons".starts_with(path) {
            results.extend(ComponentAssets.list(path)?);
        }

        results.extend(
            LocalAssets::iter()
                .filter_map(|p| p.starts_with(path).then(|| p.into()))
                .collect::<Vec<_>>(),
        );

        Ok(results)
    }
}
