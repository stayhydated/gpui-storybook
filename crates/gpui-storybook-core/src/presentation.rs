//! Window-local preview presentation settings and action records.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Named viewport sizes available in the workbench and capture automation.
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum StoryViewportPreset {
    /// Fill the available story canvas.
    #[default]
    Responsive,
    Mobile,
    Tablet,
    Desktop,
}

impl StoryViewportPreset {
    pub const ALL: [Self; 4] = [Self::Responsive, Self::Mobile, Self::Tablet, Self::Desktop];

    pub const fn label(self) -> &'static str {
        match self {
            Self::Responsive => "Responsive",
            Self::Mobile => "Mobile",
            Self::Tablet => "Tablet",
            Self::Desktop => "Desktop",
        }
    }

    /// Pixel dimensions for fixed presets. Responsive mode returns `None`.
    pub const fn dimensions(self) -> Option<(u32, u32)> {
        match self {
            Self::Responsive => None,
            Self::Mobile => Some((390, 844)),
            Self::Tablet => Some((768, 1024)),
            Self::Desktop => Some((1440, 900)),
        }
    }
}

/// Background applied behind the active story preview.
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum StoryCanvasBackground {
    #[default]
    Theme,
    Light,
    Dark,
    Transparent,
}

impl StoryCanvasBackground {
    pub const ALL: [Self; 4] = [Self::Theme, Self::Light, Self::Dark, Self::Transparent];

    pub const fn label(self) -> &'static str {
        match self {
            Self::Theme => "Theme",
            Self::Light => "Light",
            Self::Dark => "Dark",
            Self::Transparent => "Transparent",
        }
    }
}

/// Preview configuration applied to the selected story entity.
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct StoryPresentation {
    pub viewport: StoryViewportPreset,
    pub background: StoryCanvasBackground,
    pub grid: bool,
}

/// One deterministic entry in the window-local workbench action log.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct StoryActionEntry {
    pub sequence: u64,
    pub name: String,
    pub detail: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn viewport_presets_have_stable_dimensions() {
        assert_eq!(StoryViewportPreset::Responsive.dimensions(), None);
        assert_eq!(StoryViewportPreset::Mobile.dimensions(), Some((390, 844)));
        assert_eq!(StoryViewportPreset::Tablet.dimensions(), Some((768, 1024)));
        assert_eq!(StoryViewportPreset::Desktop.dimensions(), Some((1440, 900)));
    }
}
