use es_fluent::EsFluent;
use gpui::App;

/// User-facing text owned by the Storybook shell.
#[derive(Clone, Copy, Debug, EsFluent)]
pub(crate) enum StorybookMessage {
    Storybook,
    Appearance,
    UseSystemAppearance,
    Light,
    Dark,
    LightTheme,
    DarkTheme,
    Language,
    UseSystemLanguage,
    Preferences,
    PersistenceLoading,
    PersistenceReady,
    PersistenceSaving,
    PersistenceError,
    PersistenceSaveFailed,
    RetryPreferences,
    RetrySave,
    Quit,
    Edit,
    Undo,
    Redo,
    Cut,
    Copy,
    Paste,
    Window,
}

pub(crate) fn text(cx: &App, message: StorybookMessage) -> String {
    crate::i18n::localize_message(cx, &message).unwrap_or_else(|| {
        tracing::error!(message = ?message, "missing embedded Storybook message");
        String::new()
    })
}
