use gpui_kit::Action;
use gpui_kit::component::scroll::ScrollbarMode;
use gpui_storybook_preferences::{PreferredColorScheme, SystemColorScheme};
use serde::Deserialize;
use unic_langid::LanguageIdentifier;

#[derive(Action, Clone, Deserialize, Eq, PartialEq)]
#[action(namespace = story, no_json)]
pub struct SelectScrollbarMode(pub ScrollbarMode);

#[derive(Action, Clone, Deserialize, Eq, PartialEq)]
#[action(namespace = story, no_json)]
pub struct SelectLocale(pub LanguageIdentifier);

#[derive(Action, Clone, Eq, PartialEq)]
#[action(namespace = story, no_json)]
pub struct SelectColorScheme(pub PreferredColorScheme);

#[derive(Action, Clone, Eq, PartialEq)]
#[action(namespace = story, no_json)]
pub struct SelectTheme {
    pub scheme: SystemColorScheme,
    pub theme: gpui_kit::SharedString,
}

#[derive(Action, Clone, Debug, Default, Eq, PartialEq)]
#[action(namespace = story)]
pub struct UseSystemLocale;

#[derive(Action, Clone, Debug, Default, Eq, PartialEq)]
#[action(namespace = story)]
pub struct RetryPreferences;

#[derive(Action, Clone, Deserialize, Eq, PartialEq)]
#[action(namespace = story, no_json)]
pub struct SelectFont(pub usize);

#[derive(Action, Clone, Deserialize, Eq, PartialEq)]
#[action(namespace = story, no_json)]
pub struct SelectRadius(pub usize);

#[derive(Action, Clone, Debug, Default, Eq, PartialEq)]
#[action(namespace = story)]
pub struct Quit;

#[derive(Action, Clone, Debug, Default, Eq, PartialEq)]
#[action(namespace = story)]
pub struct Open;

#[derive(Action, Clone, Debug, Default, Eq, PartialEq)]
#[action(namespace = story)]
pub struct CloseWindow;

#[derive(Action, Clone, Debug, Default, Eq, PartialEq)]
#[action(namespace = story)]
pub struct ToggleSearch;
