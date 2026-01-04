use crate::story::StoryContainer;

/// Entry type for story registration
pub struct StoryEntry {
    pub name: &'static str,
    pub section: Option<&'static str>,
    pub section_order: Option<usize>,
    pub create_fn: fn(&mut ::gpui::Window, &mut ::gpui::App) -> ::gpui::Entity<StoryContainer>,
    pub file: &'static str,
    pub line: u32,
}

inventory::collect!(StoryEntry);

/// Entry type for init function registration
pub struct InitEntry {
    pub init_fn: fn(&mut ::gpui::App),
    pub fn_name: &'static str,
    pub file: &'static str,
    pub line: u32,
}

inventory::collect!(InitEntry);
