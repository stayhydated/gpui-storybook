pub mod components;

use es_fluent::EsFluent;

#[derive(EsFluent)]
pub enum StoryItems {
    Title,
}

/// Section ordering for component registration examples.
#[derive(Clone, Copy, Debug)]
#[repr(usize)]
pub enum StorySection {
    Intro = 1,
    Signals = 2,
    Notes = 3,
}
