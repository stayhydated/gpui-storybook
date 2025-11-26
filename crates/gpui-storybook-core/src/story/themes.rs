use std::path::PathBuf;

use gpui::{Action, App, SharedString};
use gpui_component::{ActiveTheme as _, Theme, ThemeMode, ThemeRegistry, scroll::ScrollbarShow};
use serde::{Deserialize, Serialize};

const STATE_FILE: &str = "target/state.json";
const THEMES_DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/assets/themes");

#[derive(Clone, Debug, Deserialize, Serialize)]
struct State {
    theme: SharedString,
    scrollbar_show: Option<ScrollbarShow>,
}

impl Default for State {
    fn default() -> Self {
        Self {
            theme: SharedString::from("Default Light"),
            scrollbar_show: None,
        }
    }
}

pub fn init(cx: &mut App) {
    let json = std::fs::read_to_string(STATE_FILE).unwrap_or_default();
    let state = serde_json::from_str::<State>(&json).unwrap_or_default();

    if let Err(err) = ThemeRegistry::watch_dir(
        PathBuf::from(THEMES_DIR),
        cx,
        move |cx| {
            if let Some(theme) = ThemeRegistry::global(cx)
                .themes()
                .get(&state.theme)
                .cloned()
            {
                Theme::global_mut(cx).apply_config(&theme);
            }
        },
    ) {
        eprintln!("Failed to watch themes directory: {}", err);
    }

    if let Some(scrollbar_show) = state.scrollbar_show {
        Theme::global_mut(cx).scrollbar_show = scrollbar_show;
    }
    cx.refresh_windows();

    cx.observe_global::<Theme>(|cx| {
        let snapshot = State {
            theme: cx.theme().theme_name().clone(),
            scrollbar_show: Some(cx.theme().scrollbar_show),
        };

        if let Ok(json) = serde_json::to_string_pretty(&snapshot) {
            let _ = std::fs::write(STATE_FILE, json);
        }
    })
    .detach();

    cx.on_action(|switch: &SwitchTheme, cx| {
        if let Some(theme_config) = ThemeRegistry::global(cx).themes().get(&switch.0).cloned() {
            Theme::global_mut(cx).apply_config(&theme_config);
            cx.refresh_windows();
        }
    });

    cx.on_action(|switch: &SwitchThemeMode, cx| {
        Theme::change(switch.0, None, cx);
        cx.refresh_windows();
    });
}

#[derive(Action, Clone, PartialEq)]
#[action(namespace = story_themes, no_json)]
pub(crate) struct SwitchTheme(pub(crate) SharedString);

#[derive(Action, Clone, PartialEq)]
#[action(namespace = story_themes, no_json)]
pub(crate) struct SwitchThemeMode(pub(crate) ThemeMode);
