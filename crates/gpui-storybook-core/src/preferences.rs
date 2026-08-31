//! GPUI-owned resolved preference state and runtime orchestration.

use std::{future::Future, mem, path::PathBuf, rc::Rc, sync::Arc};

use gpui::{
    App, AsyncApp, BorrowAppContext as _, Global, InteractiveElement as _, SharedString, Task,
    Window, WindowAppearance,
};
use gpui_component::{
    Theme, ThemeMode, ThemeRegistry, WindowExt as _,
    button::{Button, ButtonVariants as _},
    notification::Notification,
    scroll::ScrollbarMode,
};
use gpui_storybook_preferences::{
    AvailableThemeResolver, DetectedLocales, LanguageTag, LocaleDetector, PersistenceMode,
    PreferenceRepository, PreferredColorScheme, PreferredLanguage, PreferredScrollbar,
    RecoveryDiagnostic, RepositoryOpenError, RepositoryOptions, ResolutionDiagnostic,
    ResolutionOverrides, ResolvePreferencesError, ResolvedPreferences, StorybookPreferences,
    StorybookWindowMode, SupportedLanguages, SystemColorScheme, ThemeId, resolve_preferences,
};
use unic_langid::LanguageIdentifier;

use crate::{i18n, language::Language};

fn spawn_storage<T, F>(cx: &mut App, future: F) -> Task<Result<T, ()>>
where
    T: Send + 'static,
    F: Future<Output = T> + Send + 'static,
{
    #[cfg(not(target_family = "wasm"))]
    {
        let task = gpui_tokio::Tokio::spawn(cx, future);
        cx.spawn(async move |_cx| task.await.map_err(|_| ()))
    }

    #[cfg(target_family = "wasm")]
    {
        cx.spawn(async move |_cx| Ok(future.await))
    }
}

/// Current state of local preference storage.
///
/// Locale application and resolution diagnostics do not change this status.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum PersistenceStatus {
    /// The local repository is opening and the saved document is loading.
    #[default]
    Loading,
    /// Saved intent is loaded and no write is outstanding.
    Ready,
    /// An optimistic session change is being written.
    Saving,
    /// Loading or saving failed; resolved session state remains usable.
    Error,
}

/// Structured, observable startup or preference-application diagnostic.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PreferenceDiagnostic {
    /// Invalid local JSON was archived before defaults were applied.
    Recovered(RecoveryDiagnostic),
    /// Repository startup failed and defaults remain active for this session.
    LoadFailed {
        /// Optional path involved in the failure.
        path: Option<PathBuf>,
        /// Stable error category.
        category: String,
    },
    /// A save failed; the optimistic in-memory selection remains active.
    SaveFailed {
        /// Stable error category.
        category: String,
    },
    /// Applying the resolved locale failed.
    LocaleApplicationFailed {
        /// Resolved BCP 47 language tag.
        language: String,
        /// Stable error category.
        category: String,
    },
}

/// Readiness result returned after initial load and foreground resolution.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StorybookReady {
    /// Storage state after startup completes.
    pub persistence_status: PersistenceStatus,
    /// Startup diagnostics, including storage recovery/failure and locale
    /// application failure.
    pub diagnostics: Vec<PreferenceDiagnostic>,
}

/// Saved intent and effective values used by Storybook menus and windows.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PreferenceState {
    /// User intent retained independently of system resolution.
    pub saved: StorybookPreferences,
    /// Effective theme, appearance, language, source, and fallback diagnostics.
    pub resolved: ResolvedPreferences,
    /// Current local storage activity. Locale failures do not change it.
    pub persistence_status: PersistenceStatus,
    /// Storage and locale-application diagnostics.
    pub diagnostics: Vec<PreferenceDiagnostic>,
    /// Resolution fallback diagnostics for direct UI inspection.
    pub resolution_diagnostics: Vec<ResolutionDiagnostic>,
}

/// Prevalidated runtime configuration assembled by the public facade.
#[doc(hidden)]
pub struct RuntimeOptions<L>
where
    L: Language,
{
    pub repository: RepositoryOptions,
    pub languages: Vec<(L, LanguageTag)>,
    pub supported_languages: SupportedLanguages,
    pub locale_detector: Arc<dyn LocaleDetector>,
    pub initial_scheme: SystemColorScheme,
    pub overrides: ResolutionOverrides,
    pub apply_consumer_locale: Rc<dyn Fn(L, &mut App) -> Result<(), String>>,
    pub localize_consumer_language: Rc<dyn Fn(L, &App) -> Option<String>>,
}

pub(crate) trait PreferenceRuntime: 'static {
    fn state(&self) -> &PreferenceState;
    fn available_locales(&self, cx: &App) -> Vec<(String, LanguageTag)>;
    fn select_color_scheme(&mut self, value: PreferredColorScheme, cx: &mut App);
    fn select_theme(&mut self, scheme: SystemColorScheme, theme: ThemeId, cx: &mut App);
    fn select_language(&mut self, value: PreferredLanguage, cx: &mut App);
    fn select_scrollbar(&mut self, value: PreferredScrollbar, cx: &mut App);
    fn select_window_mode(&mut self, value: StorybookWindowMode, cx: &mut App);
    fn window_appearance_changed(&mut self, window: &mut Window, cx: &mut App);
    fn window_activated(&mut self, window: &mut Window, cx: &mut App);
    #[cfg(not(target_family = "wasm"))]
    fn theme_registry_changed(&mut self, cx: &mut App);
    fn retry_preferences(&mut self, cx: &mut App);
    fn finish_loading(&mut self, loaded: StartupLoad, cx: &mut App) -> StorybookReady;
    fn finish_save(&mut self, result: Result<(), String>, cx: &mut App);
    fn finish_reload(&mut self, loaded: StartupLoad, cx: &mut App);
    fn finish_reopen(&mut self, result: RetryOpen, cx: &mut App);
}

pub(crate) struct StorybookPreferencesGlobal(pub(crate) Box<dyn PreferenceRuntime>);

impl Global for StorybookPreferencesGlobal {}

mod api;
mod resolution;
mod runtime;

pub(crate) use api::available_locales;
#[cfg(not(target_family = "wasm"))]
pub(crate) use api::theme_registry_changed;
pub use api::{
    initialize, retry_preferences, select_color_scheme, select_language, select_scrollbar,
    select_theme, select_window_mode, try_state, window_activated, window_appearance_changed,
};
pub(crate) use resolution::explicit_language;
pub use resolution::{color_scheme, repository_options};
use resolution::{
    repository_open_category, repository_open_path, resolve, scrollbar_mode, store_error_category,
    theme_mode,
};
pub(crate) use runtime::{RetryOpen, StartupLoad};
use runtime::{Runtime, load_preferences};
