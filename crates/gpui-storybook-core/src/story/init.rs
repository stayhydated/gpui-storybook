use super::{state::AppState, themes};
use crate::{
    actions::{Quit, ToggleSearch},
    i18n,
};
use gpui::{App, KeyBinding, Menu, MenuItem, OsAction};
use gpui_component::input::{Copy, Cut, Paste, Redo, Undo};

pub fn init(cx: &mut App) {
    i18n::init();
    gpui_component::init(cx);
    AppState::init(cx);
    themes::init(cx);

    cx.bind_keys([
        KeyBinding::new("/", ToggleSearch, None),
        KeyBinding::new("cmd-q", Quit, None),
    ]);

    cx.on_action(|_: &Quit, cx: &mut App| {
        cx.quit();
    });

    cx.set_menus(vec![
        Menu {
            name: "GPUI App".into(),
            items: vec![MenuItem::action("Quit", Quit)],
        },
        Menu {
            name: "Edit".into(),
            items: vec![
                MenuItem::os_action("Undo", Undo, OsAction::Undo),
                MenuItem::os_action("Redo", Redo, OsAction::Redo),
                MenuItem::separator(),
                MenuItem::os_action("Cut", Cut, OsAction::Cut),
                MenuItem::os_action("Copy", Copy, OsAction::Copy),
                MenuItem::os_action("Paste", Paste, OsAction::Paste),
            ],
        },
        Menu {
            name: "Window".into(),
            items: vec![],
        },
    ]);
    cx.activate(true);
}
