use gpui::{App, AppContext as _, Entity, Global, SharedString};

pub struct AppState {
    pub invisible_panels: Entity<Vec<SharedString>>,
}

impl AppState {
    pub(crate) fn init(cx: &mut App) {
        let state = Self {
            invisible_panels: cx.new(|_| Vec::new()),
        };
        cx.set_global::<AppState>(state);
    }

    pub fn global(cx: &App) -> &Self {
        cx.global::<Self>()
    }

    pub fn global_mut(cx: &mut App) -> &mut Self {
        cx.global_mut::<Self>()
    }
}

impl Global for AppState {}
