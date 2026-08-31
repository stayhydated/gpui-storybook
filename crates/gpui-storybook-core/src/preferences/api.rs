use super::*;

/// Installs a loading runtime and returns a task that completes after local
/// state has been resolved and applied on the GPUI foreground.
#[doc(hidden)]
pub fn initialize<L>(
    options: RuntimeOptions<L>,
    cx: &mut App,
) -> Result<Task<StorybookReady>, ResolvePreferencesError>
where
    L: Language,
{
    let repository_options = options.repository.clone();
    let runtime = Runtime::new(options, cx)?;
    cx.set_global(StorybookPreferencesGlobal(Box::new(runtime)));

    let storage_task = spawn_storage(cx, load_preferences(repository_options, None));

    Ok(cx.spawn(async move |cx: &mut AsyncApp| {
        let loaded = storage_task.await.unwrap_or_else(|_| StartupLoad::Failed {
            repository: None,
            path: None,
            category: "tokio_join".to_owned(),
        });
        cx.update(|cx| {
            cx.update_global::<StorybookPreferencesGlobal, _>(|global, cx| {
                global.0.finish_loading(loaded, cx)
            })
        })
    }))
}

/// Returns the current Storybook preference snapshot when initialized.
pub fn try_state(cx: &App) -> Option<&PreferenceState> {
    cx.try_global::<StorybookPreferencesGlobal>()
        .map(|runtime| runtime.0.state())
}

/// Returns localized available languages in typed application order.
pub(crate) fn available_locales(cx: &App) -> Vec<(String, LanguageTag)> {
    cx.try_global::<StorybookPreferencesGlobal>()
        .map_or_else(Vec::new, |runtime| runtime.0.available_locales(cx))
}

/// Applies a user-selected appearance intent and queues persistence.
pub fn select_color_scheme(value: PreferredColorScheme, cx: &mut App) {
    if cx.try_global::<StorybookPreferencesGlobal>().is_some() {
        cx.update_global::<StorybookPreferencesGlobal, _>(|runtime, cx| {
            runtime.0.select_color_scheme(value, cx);
        });
    }
}

/// Applies a named theme and activates its matching light or dark appearance.
///
/// The opposite theme slot remains saved for the next appearance transition.
pub fn select_theme(scheme: SystemColorScheme, theme: ThemeId, cx: &mut App) {
    if cx.try_global::<StorybookPreferencesGlobal>().is_some() {
        cx.update_global::<StorybookPreferencesGlobal, _>(|runtime, cx| {
            runtime.0.select_theme(scheme, theme, cx);
        });
    }
}

/// Applies a system or explicit language intent and queues persistence.
pub fn select_language(value: PreferredLanguage, cx: &mut App) {
    if cx.try_global::<StorybookPreferencesGlobal>().is_some() {
        cx.update_global::<StorybookPreferencesGlobal, _>(|runtime, cx| {
            runtime.0.select_language(value, cx);
        });
    }
}

/// Applies a scrollbar policy and queues persistence.
///
/// This is a no-op when the facade preference runtime is not installed.
pub fn select_scrollbar(value: PreferredScrollbar, cx: &mut App) {
    if cx.try_global::<StorybookPreferencesGlobal>().is_some() {
        cx.update_global::<StorybookPreferencesGlobal, _>(|runtime, cx| {
            runtime.0.select_scrollbar(value, cx);
        });
    }
}

/// Applies a Storybook window mode and queues persistence.
///
/// This is a no-op when the facade preference runtime is not installed.
pub fn select_window_mode(value: StorybookWindowMode, cx: &mut App) {
    if cx.try_global::<StorybookPreferencesGlobal>().is_some() {
        cx.update_global::<StorybookPreferencesGlobal, _>(|runtime, cx| {
            runtime.0.select_window_mode(value, cx);
        });
    }
}

/// Retries loading preferences after startup failure or saving dirty intent.
///
/// This is a no-op when the facade preference runtime is not installed.
pub fn retry_preferences(cx: &mut App) {
    if cx.try_global::<StorybookPreferencesGlobal>().is_some() {
        cx.update_global::<StorybookPreferencesGlobal, _>(|runtime, cx| {
            runtime.0.retry_preferences(cx);
        });
    }
}

/// Feeds a live window appearance event into preference resolution.
///
/// This is a no-op when the facade preference runtime is not installed.
pub fn window_appearance_changed(window: &mut Window, cx: &mut App) {
    if cx.try_global::<StorybookPreferencesGlobal>().is_some() {
        cx.update_global::<StorybookPreferencesGlobal, _>(|runtime, cx| {
            runtime.0.window_appearance_changed(window, cx);
        });
    }
}

/// Re-detects system locale and appearance when a window becomes active.
///
/// This is a no-op when the facade preference runtime is not installed.
pub fn window_activated(window: &mut Window, cx: &mut App) {
    if cx.try_global::<StorybookPreferencesGlobal>().is_some() {
        cx.update_global::<StorybookPreferencesGlobal, _>(|runtime, cx| {
            runtime.0.window_activated(window, cx);
        });
    }
}

/// Re-resolves the effective slot after the development theme registry reloads.
#[cfg(not(target_family = "wasm"))]
pub(crate) fn theme_registry_changed(cx: &mut App) {
    if cx.try_global::<StorybookPreferencesGlobal>().is_some() {
        cx.update_global::<StorybookPreferencesGlobal, _>(|runtime, cx| {
            runtime.0.theme_registry_changed(cx);
        });
    }
}
