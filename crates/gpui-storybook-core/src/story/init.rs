use super::{state::AppState, themes};
use crate::{
    actions::{Quit, ToggleSearch},
    app_menus, i18n,
    messages::{StorybookMessage, text},
};
use gpui_kit::component::input::{Copy, Cut, Paste, Redo, Undo};
use gpui_kit::{App, KeyBinding, Menu, MenuItem, OsAction};

pub fn init(cx: &mut App) -> Result<(), gpui_es_fluent::EmbeddedInitError> {
    gpui_kit::init(cx);
    #[cfg(not(feature = "inspector"))]
    disable_inspector_shortcut(cx);
    crate::web_fonts::init(cx);
    i18n::init(cx)?;
    AppState::init(cx);
    #[cfg(feature = "inspector")]
    crate::story_inspector::init(cx);
    themes::init(cx);
    app_menus::register_actions(cx);
    crate::dock_gallery::register_story_panels(cx);

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

#[cfg(not(feature = "inspector"))]
fn disable_inspector_shortcut(cx: &mut App) {
    let shortcut = if cfg!(target_os = "macos") {
        "cmd-alt-i"
    } else {
        "ctrl-shift-i"
    };

    // gpui-component installs this binding automatically in debug builds. Remove Storybook's
    // default keyboard entry point unless its own inspector feature is selected.
    cx.bind_keys([KeyBinding::new(
        shortcut,
        gpui_kit::Unbind("inspector::ToggleInspector".into()),
        None,
    )]);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn inspector_shortcut() -> &'static str {
        if cfg!(target_os = "macos") {
            "cmd-alt-i"
        } else {
            "ctrl-shift-i"
        }
    }

    #[gpui_kit::test]
    fn runtime_init_installs_storybook_state(cx: &mut App) {
        init(cx).expect("Storybook localization should initialize");

        assert!(cx.try_global::<AppState>().is_some());
        assert!(cx.try_global::<gpui_kit::component::Theme>().is_some());
        assert!(
            cx.try_global::<gpui_kit::component::ThemeRegistry>()
                .is_some()
        );
        assert_eq!(
            crate::messages::text(cx, crate::messages::StorybookMessage::Storybook),
            "GPUI Storybook"
        );
        assert_eq!(
            crate::messages::text(cx, crate::messages::StorybookMessage::Layout),
            "Layout"
        );
        assert_eq!(
            crate::messages::text(cx, crate::messages::StorybookMessage::Gallery),
            "Gallery"
        );
        assert_eq!(
            crate::messages::text(cx, crate::messages::StorybookMessage::DockWorkspace),
            "Dock workspace"
        );
    }

    #[cfg(not(feature = "inspector"))]
    #[gpui_kit::test]
    fn runtime_init_disables_the_component_inspector_shortcut(cx: &mut App) {
        init(cx).expect("Storybook localization should initialize");

        let keystroke = gpui_kit::Keystroke::parse(inspector_shortcut()).expect("valid shortcut");
        let keymap = cx.key_bindings();
        let (bindings, pending) = keymap.borrow().bindings_for_input(&[keystroke], &[]);

        assert!(!pending);
        assert!(bindings.is_empty());
    }

    #[cfg(feature = "inspector")]
    #[gpui_kit::test]
    fn runtime_init_keeps_the_component_inspector_shortcut(cx: &mut App) {
        init(cx).expect("Storybook localization should initialize");

        let keystroke = gpui_kit::Keystroke::parse(inspector_shortcut()).expect("valid shortcut");
        let keymap = cx.key_bindings();
        let (bindings, pending) = keymap.borrow().bindings_for_input(&[keystroke], &[]);

        assert!(!pending);
        assert_eq!(bindings.len(), 1);
        assert_eq!(bindings[0].action().name(), "inspector::ToggleInspector");
    }

    #[gpui_kit::test]
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
