use gpui::{App, Entity, Menu, MenuItem, SharedString};
use gpui_component::{GlobalState, Theme, ThemeMode, ThemeRegistry, menu::AppMenuBar};
use gpui_storybook_preferences::{
    PreferredColorScheme, PreferredLanguage, SystemColorScheme, ThemeId,
};

use crate::{
    actions::{
        Quit, RetryPreferences, SelectColorScheme, SelectLocale, SelectTheme, UseSystemLocale,
    },
    messages::{StorybookMessage, text},
    preferences::{self, PersistenceStatus, StorybookPreferencesGlobal},
    storybook_window_ui::AppMenuItemsBuilder,
};

/// Installs the one-time preference action routing owned by the core runtime.
pub(crate) fn register_actions(cx: &mut App) {
    cx.on_action(|action: &SelectColorScheme, cx| {
        preferences::select_color_scheme(action.0, cx);
    });
    cx.on_action(|action: &SelectTheme, cx| {
        let Ok(theme) = ThemeId::new(action.theme.as_ref()) else {
            tracing::error!("rejected invalid Storybook theme action");
            return;
        };
        preferences::select_theme(action.scheme, theme, cx);
    });
    cx.on_action(|action: &SelectLocale, cx| {
        if let Some(language) = preferences::explicit_language(action.0.clone()) {
            preferences::select_language(language, cx);
        }
    });
    cx.on_action(|_: &UseSystemLocale, cx| {
        preferences::select_language(PreferredLanguage::System, cx);
    });
    cx.on_action(|_: &RetryPreferences, cx| {
        preferences::retry_preferences(cx);
    });
}

pub fn init(
    title: impl Into<SharedString>,
    extra_items: Option<AppMenuItemsBuilder>,
    cx: &mut App,
) -> Entity<AppMenuBar> {
    let app_menu_bar = AppMenuBar::new(cx);
    let title: SharedString = title.into();
    update_app_menu(title.clone(), extra_items.clone(), app_menu_bar.clone(), cx);
    install_reload_handlers(title, extra_items, app_menu_bar.clone(), cx);
    app_menu_bar
}

fn install_reload_handlers(
    title: SharedString,
    extra_items: Option<AppMenuItemsBuilder>,
    app_menu_bar: Entity<AppMenuBar>,
    cx: &mut App,
) {
    cx.observe_global::<StorybookPreferencesGlobal>({
        let title = title.clone();
        let extra_items = extra_items.clone();
        let app_menu_bar = app_menu_bar.clone();
        move |cx| {
            schedule_app_menu_update(title.clone(), extra_items.clone(), app_menu_bar.clone(), cx);
        }
    })
    .detach();
    cx.observe_global::<Theme>(move |cx| {
        schedule_app_menu_update(title.clone(), extra_items.clone(), app_menu_bar.clone(), cx);
    })
    .detach();
}

fn schedule_app_menu_update(
    title: SharedString,
    extra_items: Option<AppMenuItemsBuilder>,
    app_menu_bar: Entity<AppMenuBar>,
    cx: &mut App,
) {
    // A Wayland appearance callback updates the Theme global while its client
    // state is mutably borrowed. Calling `App::set_menus` from that global
    // observer re-enters the platform and panics, so cross the foreground-task
    // boundary before touching native menus.
    cx.spawn(async move |cx| {
        cx.update(|cx| update_app_menu(title, extra_items, app_menu_bar, cx));
    })
    .detach();
}

fn update_app_menu(
    title: impl Into<SharedString>,
    extra_items: Option<AppMenuItemsBuilder>,
    app_menu_bar: Entity<AppMenuBar>,
    cx: &mut App,
) {
    let title: SharedString = title.into();
    cx.set_menus(build_menus(title.clone(), extra_items.clone(), cx));
    let menus = build_menus(title, extra_items, cx)
        .into_iter()
        .map(Menu::owned)
        .collect();
    GlobalState::global_mut(cx).set_app_menus(menus);

    app_menu_bar.update(cx, |menu_bar, cx| {
        menu_bar.reload(cx);
    });
}

fn build_menus(
    title: impl Into<SharedString>,
    extra_items: Option<AppMenuItemsBuilder>,
    cx: &App,
) -> Vec<Menu> {
    let mut items = vec![
        appearance_menu(cx),
        theme_menu(SystemColorScheme::Light, cx),
        theme_menu(SystemColorScheme::Dark, cx),
        language_menu(cx),
        persistence_menu(cx),
    ];

    if let Some(extra_items) = extra_items
        .as_ref()
        .map(|build| build(cx))
        .filter(|items| !items.is_empty())
    {
        items.push(MenuItem::Separator);
        items.extend(extra_items);
    }

    items.push(MenuItem::Separator);
    items.push(MenuItem::action(text(cx, StorybookMessage::Quit), Quit));

    vec![Menu {
        name: title.into(),
        items,
        disabled: false,
    }]
}

fn appearance_menu(cx: &App) -> MenuItem {
    let selected = preferences::try_state(cx).map(|state| state.saved.color_scheme);
    MenuItem::Submenu(Menu {
        name: text(cx, StorybookMessage::Appearance).into(),
        items: [
            (
                StorybookMessage::UseSystemAppearance,
                PreferredColorScheme::System,
            ),
            (StorybookMessage::Light, PreferredColorScheme::Light),
            (StorybookMessage::Dark, PreferredColorScheme::Dark),
        ]
        .into_iter()
        .map(|(label, value)| {
            MenuItem::action(text(cx, label), SelectColorScheme(value))
                .checked(selected == Some(value))
        })
        .collect(),
        disabled: selected.is_none(),
    })
}

fn theme_menu(scheme: SystemColorScheme, cx: &App) -> MenuItem {
    let selected = preferences::try_state(cx).and_then(|state| match scheme {
        SystemColorScheme::Light => state.saved.light_theme.as_ref(),
        SystemColorScheme::Dark => state.saved.dark_theme.as_ref(),
    });
    let mode = match scheme {
        SystemColorScheme::Light => ThemeMode::Light,
        SystemColorScheme::Dark => ThemeMode::Dark,
    };
    let items = ThemeRegistry::global(cx)
        .sorted_themes()
        .into_iter()
        .filter(|theme| theme.mode == mode)
        .map(|theme| {
            let checked = selected.map_or(theme.is_default, |selected| {
                selected.as_str() == theme.name.as_ref()
            });
            MenuItem::action(
                theme.name.clone(),
                SelectTheme {
                    scheme,
                    theme: theme.name.clone(),
                },
            )
            .checked(checked)
        })
        .collect::<Vec<_>>();

    MenuItem::Submenu(Menu {
        name: text(
            cx,
            match scheme {
                SystemColorScheme::Light => StorybookMessage::LightTheme,
                SystemColorScheme::Dark => StorybookMessage::DarkTheme,
            },
        )
        .into(),
        disabled: items.is_empty(),
        items,
    })
}

fn language_menu(cx: &App) -> MenuItem {
    let state = preferences::try_state(cx);
    let mut items = vec![
        MenuItem::action(
            text(cx, StorybookMessage::UseSystemLanguage),
            UseSystemLocale,
        )
        .checked(
            state.is_some_and(|state| matches!(state.saved.language, PreferredLanguage::System)),
        ),
    ];
    items.extend(
        preferences::available_locales(cx)
            .into_iter()
            .map(|(name, tag)| {
                let checked = state.is_some_and(|state| {
                    matches!(
                        &state.saved.language,
                        PreferredLanguage::Explicit(selected) if selected == &tag
                    )
                });
                MenuItem::action(name, SelectLocale(tag.as_identifier().clone())).checked(checked)
            }),
    );

    MenuItem::Submenu(Menu {
        name: text(cx, StorybookMessage::Language).into(),
        items,
        disabled: state.is_none(),
    })
}

fn persistence_menu(cx: &App) -> MenuItem {
    let status = preferences::try_state(cx).map(|state| state.persistence_status);
    persistence_menu_for_status(status, cx)
}

fn persistence_menu_for_status(status: Option<PersistenceStatus>, cx: &App) -> MenuItem {
    let status_label = match status.unwrap_or(PersistenceStatus::Loading) {
        PersistenceStatus::Loading => StorybookMessage::PersistenceLoading,
        PersistenceStatus::Ready => StorybookMessage::PersistenceReady,
        PersistenceStatus::Saving => StorybookMessage::PersistenceSaving,
        PersistenceStatus::Error => StorybookMessage::PersistenceError,
    };
    let mut items = vec![MenuItem::action(text(cx, status_label), RetryPreferences).disabled(true)];
    if status == Some(PersistenceStatus::Error) {
        items.push(MenuItem::action(
            text(cx, StorybookMessage::RetryPreferences),
            RetryPreferences,
        ));
    }

    MenuItem::Submenu(Menu {
        name: text(cx, StorybookMessage::Preferences).into(),
        items,
        disabled: false,
    })
}

#[cfg(test)]
mod tests {
    use std::{cell::Cell, rc::Rc};

    use super::*;

    fn first_item_name(item: MenuItem) -> SharedString {
        let MenuItem::Submenu(menu) = item else {
            panic!("persistence menu should be a submenu");
        };
        let Some(MenuItem::Action { name, .. }) = menu.items.into_iter().next() else {
            panic!("persistence menu should contain a status action");
        };
        name
    }

    #[gpui::test]
    fn menu_status_tracks_saving_ready_and_error_transitions(cx: &mut App) {
        crate::i18n::init(cx).expect("Storybook test localization initializes");

        assert_eq!(
            first_item_name(persistence_menu_for_status(
                Some(PersistenceStatus::Saving),
                cx,
            )),
            text(cx, StorybookMessage::PersistenceSaving)
        );
        assert_eq!(
            first_item_name(persistence_menu_for_status(
                Some(PersistenceStatus::Ready),
                cx,
            )),
            text(cx, StorybookMessage::PersistenceReady)
        );
        let MenuItem::Submenu(error_menu) =
            persistence_menu_for_status(Some(PersistenceStatus::Error), cx)
        else {
            panic!("error status should be a submenu");
        };
        assert_eq!(error_menu.items.len(), 2);
        let MenuItem::Action { name, .. } = &error_menu.items[0] else {
            panic!("first error item should report status");
        };
        assert_eq!(name.as_ref(), text(cx, StorybookMessage::PersistenceError));
    }

    #[gpui::test]
    fn appearance_and_language_menus_expose_system_intent(cx: &mut App) {
        gpui_component::init(cx);
        crate::i18n::init(cx).expect("Storybook test localization initializes");

        let MenuItem::Submenu(appearance) = appearance_menu(cx) else {
            panic!("appearance should be a submenu");
        };
        assert_eq!(appearance.items.len(), 3);
        let MenuItem::Action { name, .. } = &appearance.items[0] else {
            panic!("first appearance item should be an action");
        };
        assert_eq!(
            name.as_ref(),
            text(cx, StorybookMessage::UseSystemAppearance)
        );

        let MenuItem::Submenu(language) = language_menu(cx) else {
            panic!("language should be a submenu");
        };
        let MenuItem::Action { name, .. } = &language.items[0] else {
            panic!("first language item should be an action");
        };
        assert_eq!(name.as_ref(), text(cx, StorybookMessage::UseSystemLanguage));
    }

    #[gpui::test]
    fn menu_reload_crosses_a_foreground_task_boundary(cx: &mut gpui::TestAppContext) {
        let build_count = Rc::new(Cell::new(0));
        let extra_items: AppMenuItemsBuilder = {
            let build_count = build_count.clone();
            Rc::new(move |_| {
                build_count.set(build_count.get() + 1);
                Vec::new()
            })
        };
        cx.update(|cx| {
            gpui_component::init(cx);
            crate::i18n::init(cx).expect("Storybook test localization initializes");
            let app_menu_bar = AppMenuBar::new(cx);
            schedule_app_menu_update("Storybook".into(), Some(extra_items), app_menu_bar, cx);
        });

        assert_eq!(build_count.get(), 0);
        cx.run_until_parked();
        assert_eq!(build_count.get(), 2);
    }
}
