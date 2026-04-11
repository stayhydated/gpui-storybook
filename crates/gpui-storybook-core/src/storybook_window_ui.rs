use gpui::{AnyElement, App, Entity, IntoElement, MenuItem, Window};
use std::rc::Rc;

pub type AppMenuItemsBuilder = Rc<dyn Fn(&App) -> Vec<MenuItem>>;
pub type TitleBarItemsBuilder = Rc<dyn Fn(&mut Window, &mut App) -> AnyElement>;

#[derive(Clone, Default)]
pub struct StorybookWindowUi {
    pub(crate) app_menu_items: Option<AppMenuItemsBuilder>,
    pub(crate) title_bar_items: Option<TitleBarItemsBuilder>,
}

impl StorybookWindowUi {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_app_menu_items<F>(mut self, build: F) -> Self
    where
        F: Fn(&App) -> Vec<MenuItem> + 'static,
    {
        self.app_menu_items = Some(Rc::new(build));
        self
    }

    pub fn with_title_bar_items<F, E>(mut self, render: F) -> Self
    where
        E: IntoElement,
        F: Fn(&mut Window, &mut App) -> E + 'static,
    {
        self.title_bar_items = Some(Rc::new(move |window, cx| {
            render(window, cx).into_any_element()
        }));
        self
    }
}

pub struct StorybookWindow<V> {
    pub(crate) view: Entity<V>,
    pub(crate) ui: StorybookWindowUi,
}

impl<V> StorybookWindow<V> {
    pub fn new(view: Entity<V>) -> Self {
        Self {
            view,
            ui: StorybookWindowUi::default(),
        }
    }

    pub fn with_ui(mut self, ui: StorybookWindowUi) -> Self {
        self.ui = ui;
        self
    }
}
