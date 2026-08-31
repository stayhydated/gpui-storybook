//! Exports linked story registration metadata without opening a GPUI window.

use gpui_storybook_example_story as _;

fn main() -> Result<(), gpui_storybook::StoryCatalogExportError> {
    println!("{}", gpui_storybook::static_story_catalog_json_pretty()?);
    Ok(())
}
