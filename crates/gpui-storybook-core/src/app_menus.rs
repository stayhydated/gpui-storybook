use gpui::{App, BorrowAppContext as _, Entity, Menu, MenuItem, SharedString};
use gpui_component::{ActiveTheme as _, Theme, ThemeMode, ThemeRegistry, menu::AppMenuBar};

use crate::{
    actions::{Quit, SelectLocale},
    locale::LocaleStore,
    story::themes::{SwitchTheme, SwitchThemeMode},
};

pub fn init(title: impl Into<SharedString>, cx: &mut App) -> Entity<AppMenuBar> {
    let app_menu_bar = AppMenuBar::new(cx);
    let title: SharedString = title.into();
    update_app_menu(title.clone(), app_menu_bar.clone(), cx);

    cx.on_action({
        let title = title.clone();
        let app_menu_bar = app_menu_bar.clone();
        move |action: &SelectLocale, cx: &mut App| {
            cx.update_global::<Box<dyn LocaleStore>, _>(|locale_store, cx| {
                locale_store.set_current_locale(action.0.clone(), cx);
            });
            update_app_menu(title.clone(), app_menu_bar.clone(), cx);
            cx.refresh_windows();
        }
    });

    // Observe theme changes to update the menu to refresh the checked state
    cx.observe_global::<Theme>({
        let title = title.clone();
        let app_menu_bar = app_menu_bar.clone();
        move |cx| {
            update_app_menu(title.clone(), app_menu_bar.clone(), cx);
        }
    })
    .detach();

    app_menu_bar
}

fn update_app_menu(title: impl Into<SharedString>, app_menu_bar: Entity<AppMenuBar>, cx: &mut App) {
    let mode = cx.theme().mode;
    cx.set_menus(vec![Menu {
        name: title.into(),
        items: vec![
            MenuItem::Submenu(Menu {
                name: "Appearance".into(),
                items: vec![
                    MenuItem::action("Light", SwitchThemeMode(ThemeMode::Light))
                        .checked(!mode.is_dark()),
                    MenuItem::action("Dark", SwitchThemeMode(ThemeMode::Dark))
                        .checked(mode.is_dark()),
                ],
            }),
            theme_menu(cx),
            language_menu(cx),
            MenuItem::Separator,
            MenuItem::action("Quit", Quit),
        ],
    }]);

    app_menu_bar.update(cx, |menu_bar, cx| {
        menu_bar.reload(cx);
    })
}

fn theme_menu(cx: &App) -> MenuItem {
    let themes = ThemeRegistry::global(cx).sorted_themes();
    let current_name = cx.theme().theme_name();
    MenuItem::Submenu(Menu {
        name: "Theme".into(),
        items: themes
            .iter()
            .map(|theme| {
                let checked = current_name == &theme.name;
                MenuItem::action(theme.name.clone(), SwitchTheme(theme.name.clone()))
                    .checked(checked)
            })
            .collect(),
    })
}

fn language_menu(cx: &App) -> MenuItem {
    let locale_store = cx.global::<Box<dyn LocaleStore>>();
    let available_locales = locale_store.available_locales();
    let current_locale = locale_store.current_locale(cx);

    MenuItem::Submenu(Menu {
        name: "Language".into(),
        items: available_locales
            .iter()
            .map(|(name, lang_id)| {
                let checked = *lang_id == current_locale;
                MenuItem::action(name, SelectLocale(lang_id.clone())).checked(checked)
            })
            .collect(),
    })
}
