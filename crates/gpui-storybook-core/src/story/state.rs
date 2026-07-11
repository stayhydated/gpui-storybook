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

#[cfg(test)]
mod tests {
    use super::*;

    #[gpui::test]
    fn app_state_initializes_and_exposes_mutable_global_state(cx: &mut App) {
        AppState::init(cx);
        let invisible_panels = AppState::global(cx).invisible_panels.clone();
        assert!(invisible_panels.read(cx).is_empty());

        invisible_panels.update(cx, |panels, _| panels.push("Inspector".into()));
        assert_eq!(invisible_panels.read(cx).as_slice(), &["Inspector"]);

        let replacement = cx.new(|_| vec![SharedString::from("Stories")]);
        AppState::global_mut(cx).invisible_panels = replacement.clone();
        assert_eq!(replacement.read(cx).as_slice(), &["Stories"]);
    }
}
