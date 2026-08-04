use anyhow::anyhow;

use gpui::AssetSource;
use rust_embed::RustEmbed;

pub use gpui_component_assets::Assets as ComponentAssets;

#[cfg(target_family = "wasm")]
thread_local! {
    static COMPONENT_ASSETS: ComponentAssets = ComponentAssets::new(
        "https://longbridge.github.io/gpui-component/gallery"
    );
}

#[cfg(not(target_family = "wasm"))]
fn load_component_asset(path: &str) -> gpui::Result<Option<std::borrow::Cow<'static, [u8]>>> {
    ComponentAssets.load(path)
}

#[cfg(target_family = "wasm")]
fn load_component_asset(path: &str) -> gpui::Result<Option<std::borrow::Cow<'static, [u8]>>> {
    COMPONENT_ASSETS.with(|assets| assets.load(path))
}

#[cfg(not(target_family = "wasm"))]
fn list_component_assets(path: &str) -> gpui::Result<Vec<gpui::SharedString>> {
    ComponentAssets.list(path)
}

#[cfg(target_family = "wasm")]
fn list_component_assets(path: &str) -> gpui::Result<Vec<gpui::SharedString>> {
    COMPONENT_ASSETS.with(|assets| assets.list(path))
}

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
            return load_component_asset(path);
        }

        LocalAssets::get(path)
            .map(|f| Some(f.data))
            .ok_or_else(|| anyhow!("could not find asset at path \"{path}\""))
    }

    fn list(&self, path: &str) -> gpui::Result<Vec<gpui::SharedString>> {
        let mut results = Vec::new();

        if path.is_empty() || path.starts_with("icons") || "icons".starts_with(path) {
            results.extend(list_component_assets(path)?);
        }

        results.extend(
            LocalAssets::iter()
                .filter(|asset_path| asset_path.starts_with(path))
                .map(Into::into),
        );

        Ok(results)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn asset_source_loads_local_and_component_assets() {
        let assets = Assets;

        assert!(
            assets
                .load("")
                .expect("blank path should be valid")
                .is_none()
        );
        assert!(
            assets
                .load("themes/adventure.json")
                .expect("embedded theme should load")
                .is_some()
        );
        assert!(
            assets
                .load("icons/arrow-down.svg")
                .expect("component icon should load")
                .is_some()
        );
        assert!(assets.load("missing.asset").is_err());
    }

    #[test]
    fn asset_lists_support_root_local_and_partial_icon_prefixes() {
        let assets = Assets;
        let root = assets.list("").expect("root assets should list");
        let themes = assets.list("themes/").expect("themes should list");
        let icons = assets
            .list("icon")
            .expect("partial icon prefix should list");

        assert!(root.iter().any(|path| path == "themes/adventure.json"));
        assert!(themes.iter().all(|path| path.starts_with("themes/")));
        assert!(icons.iter().any(|path| path.starts_with("icons/")));
    }
}
