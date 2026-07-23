use super::{state::AppState, themes};
use crate::{
    actions::{Quit, ToggleSearch},
    app_menus, i18n,
    messages::{StorybookMessage, text},
};
use gpui::{App, KeyBinding, Menu, MenuItem, OsAction};
use gpui_component::input::{Copy, Cut, Paste, Redo, Undo};

pub fn init(cx: &mut App) -> Result<(), gpui_es_fluent::EmbeddedInitError> {
    gpui_component::init(cx);
    i18n::init(cx)?;
    AppState::init(cx);
    themes::init(cx);
    app_menus::register_actions(cx);

    cx.bind_keys([
        KeyBinding::new("/", ToggleSearch, None),
        KeyBinding::new("cmd-q", Quit, None),
    ]);

    cx.on_action(|_: &Quit, cx: &mut App| {
        cx.quit();
    });

    cx.set_menus(vec![
        Menu {
            name: text(cx, StorybookMessage::Storybook).into(),
            items: vec![MenuItem::action(text(cx, StorybookMessage::Quit), Quit)],
            disabled: false,
        },
        Menu {
            name: text(cx, StorybookMessage::Edit).into(),
            items: vec![
                MenuItem::os_action(text(cx, StorybookMessage::Undo), Undo, OsAction::Undo),
                MenuItem::os_action(text(cx, StorybookMessage::Redo), Redo, OsAction::Redo),
                MenuItem::separator(),
                MenuItem::os_action(text(cx, StorybookMessage::Cut), Cut, OsAction::Cut),
                MenuItem::os_action(text(cx, StorybookMessage::Copy), Copy, OsAction::Copy),
                MenuItem::os_action(text(cx, StorybookMessage::Paste), Paste, OsAction::Paste),
            ],
            disabled: false,
        },
        Menu {
            name: text(cx, StorybookMessage::Window).into(),
            items: vec![],
            disabled: false,
        },
    ]);
    cx.activate(true);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[gpui::test]
    fn runtime_init_installs_storybook_state(cx: &mut App) {
        init(cx).expect("Storybook localization should initialize");

        assert!(cx.try_global::<AppState>().is_some());
        assert!(cx.try_global::<gpui_component::Theme>().is_some());
        assert!(cx.try_global::<gpui_component::ThemeRegistry>().is_some());
        assert_eq!(
            crate::messages::text(cx, crate::messages::StorybookMessage::Storybook),
            "GPUI Storybook"
        );
    }

    #[gpui::test]
    fn shell_messages_fall_back_when_requested_locale_is_consumer_only(cx: &mut App) {
        init(cx).expect("Storybook localization should initialize");
        crate::i18n::change_locale(
            cx,
            "fr".parse::<unic_langid::LanguageIdentifier>()
                .expect("valid consumer-only locale"),
        )
        .expect("Storybook should fall back to its embedded English locale");

        assert_eq!(
            crate::messages::text(cx, crate::messages::StorybookMessage::Storybook),
            "GPUI Storybook"
        );
    }
}
