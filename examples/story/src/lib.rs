pub mod i18n;
pub mod stories;

/// Story sections with custom ordering
#[derive(Clone, Copy, Debug)]
#[repr(usize)]
pub enum StorySection {
    CustomSections = 8,
    Tables = 7,
    Buttons = 6,
    Grouped = 5,
}
