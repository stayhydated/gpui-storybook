use gpui::{Action, actions};
use gpui_component::scroll::ScrollbarShow;
use serde::Deserialize;
use unic_langid::LanguageIdentifier;

#[derive(Action, Clone, Deserialize, Eq, PartialEq)]
#[action(namespace = story, no_json)]
pub struct SelectScrollbarShow(pub ScrollbarShow);

#[derive(Action, Clone, Deserialize, Eq, PartialEq)]
#[action(namespace = story, no_json)]
pub struct SelectLocale(pub LanguageIdentifier);

#[derive(Action, Clone, Deserialize, Eq, PartialEq)]
#[action(namespace = story, no_json)]
pub struct SelectFont(pub usize);

#[derive(Action, Clone, Deserialize, Eq, PartialEq)]
#[action(namespace = story, no_json)]
pub struct SelectRadius(pub usize);

actions!(story, [Quit, Open, CloseWindow, ToggleSearch]);
