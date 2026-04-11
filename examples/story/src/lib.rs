pub mod section;
pub mod stories;

/// Story sections with custom ordering
#[derive(Clone, Copy, Debug)]
#[repr(usize)]
pub enum StorySection {
    Tables = 7,
    Buttons = 6,
}
