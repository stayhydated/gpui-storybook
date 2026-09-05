use super::*;

pub(super) struct StorybookInitialized;

impl ::gpui_kit::Global for StorybookInitialized {}

#[cfg(all(feature = "mcp", any(target_os = "linux", target_os = "macos")))]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum AutomationPreferenceProfile {
    Capture,
    Stdio,
}

#[cfg(all(feature = "mcp", any(target_os = "linux", target_os = "macos")))]
pub(super) fn apply_automation_preference_profile<L>(
    profile: AutomationPreferenceProfile,
    persistence: &mut PersistenceMode,
    json_path: &mut Option<PathBuf>,
    overrides: &mut PreferenceOverrides<L>,
    fallback_language: L,
) -> Result<(), StorybookInitError>
where
    L: Copy,
{
    *persistence = match profile {
        AutomationPreferenceProfile::Capture => PersistenceMode::Disabled,
        AutomationPreferenceProfile::Stdio => PersistenceMode::Temporary,
    };
    *json_path = None;
    overrides.color_scheme = Some(SystemColorScheme::Light);
    overrides.theme = Some(ThemeId::new("Default Light").map_err(|_| {
        StorybookInitError::CoreInitialization {
            category: "deterministic_theme".to_owned(),
        }
    })?);
    overrides.language = Some(fallback_language);
    Ok(())
}

/// Initializes Storybook and starts loading one consumer's local preferences.
///
/// The facade installs the GPUI Tokio runtime, component and Storybook state,
/// localization, story registrations, the live scenario controller, and
/// optional external automation hooks. Await the returned task before opening
/// the first window so saved theme and language intent is applied before the
/// first frame.
///
/// The active runtime `storybook.toml` may provide an initial window mode and
/// launch-only preference overrides. A per-window
/// [`StorybookWindow::with_mode`] value takes precedence over the TOML mode,
/// which takes precedence over the saved window preference. Values supplied
/// through [`StorybookOptions::with_overrides`] take precedence field by field
/// over TOML preference overrides, and deterministic automation profiles take
/// precedence over both.
///
/// Storage failures are represented by [`PersistenceStatus::Error`] in the
/// successful [`StorybookReady`] value; system and configured fallbacks remain
/// usable. Only invalid static configuration returns an error immediately.
///
/// # Errors
///
/// Returns [`StorybookInitError`] when the typed language contract, path/mode
/// combination, runtime `storybook.toml`, preference override, embedded
/// localization setup, or one-time initialization contract is invalid.
pub fn init<L>(
    cx: &mut ::gpui_kit::App,
    mut options: StorybookOptions<L>,
) -> Result<::gpui_kit::Task<StorybookReady>, StorybookInitError>
where
    L: Language,
{
    if cx.try_global::<StorybookInitialized>().is_some() {
        return Err(StorybookInitError::AlreadyInitialized);
    }
    if options.persistence != PersistenceMode::Persistent && options.json_path.is_some() {
        return Err(StorybookInitError::PathOverrideRequiresPersistent);
    }

    let init_context = load_init_context()?;
    let configured_window_mode = init_context
        .runtime_config
        .as_ref()
        .and_then(|config| config.window_mode);
    if let Some(runtime_config) = init_context.runtime_config.as_ref() {
        apply_toml_preference_overrides(&mut options.overrides, runtime_config)?;
    }

    #[cfg(all(feature = "mcp", any(target_os = "linux", target_os = "macos")))]
    {
        let profile = if gpui_storybook_mcp::capture_requested() {
            Some(AutomationPreferenceProfile::Capture)
        } else if gpui_storybook_mcp::stdio_requested() {
            Some(AutomationPreferenceProfile::Stdio)
        } else {
            None
        };
        if let Some(profile) = profile {
            apply_automation_preference_profile(
                profile,
                &mut options.persistence,
                &mut options.json_path,
                &mut options.overrides,
                options.fallback_language,
            )?;
        }
    }

    let mut languages = Vec::new();
    for language in L::iter() {
        let identifier: unic_langid::LanguageIdentifier =
            language
                .try_into()
                .map_err(|_| StorybookInitError::InvalidLanguage {
                    language: format!("{language:?}"),
                })?;
        let tag =
            gpui_storybook_preferences::LanguageTag::new(identifier.to_string()).map_err(|_| {
                StorybookInitError::InvalidLanguage {
                    language: format!("{language:?}"),
                }
            })?;
        languages.push((language, tag));
    }

    let fallback_identifier: unic_langid::LanguageIdentifier = options
        .fallback_language
        .try_into()
        .map_err(|_| StorybookInitError::InvalidLanguage {
            language: format!("{:?}", options.fallback_language),
        })?;
    let fallback_tag = gpui_storybook_preferences::LanguageTag::new(
        fallback_identifier.to_string(),
    )
    .map_err(|_| StorybookInitError::InvalidLanguage {
        language: format!("{:?}", options.fallback_language),
    })?;
    let supported_languages = gpui_storybook_preferences::SupportedLanguages::new(
        languages.iter().map(|(_, tag)| tag.clone()),
        fallback_tag,
    )
    .map_err(|_| StorybookInitError::UnsupportedFallback)?;

    let override_language = options
        .overrides
        .language
        .map(|language| {
            let identifier: unic_langid::LanguageIdentifier =
                language
                    .try_into()
                    .map_err(|_| StorybookInitError::InvalidLanguage {
                        language: format!("{language:?}"),
                    })?;
            gpui_storybook_preferences::LanguageTag::new(identifier.to_string()).map_err(|_| {
                StorybookInitError::InvalidLanguage {
                    language: format!("{language:?}"),
                }
            })
        })
        .transpose()?;

    #[cfg(not(target_family = "wasm"))]
    gpui_tokio::init(cx);
    gpui_storybook_core::story::init(cx).map_err(|error| {
        tracing::error!(error = %error, error_debug = ?error, "failed to initialize Storybook localization");
        StorybookInitError::CoreInitialization {
            category: "embedded_localization".to_owned(),
        }
    })?;
    if let Some(mode) = configured_window_mode {
        gpui_storybook_core::storybook_window_ui::set_configured_storybook_window_mode(mode, cx);
    }
    let global_init_count = inventory::iter::<__registry::InitEntry>().count();
    if global_init_count > 0 {
        tracing::info!("Discovered {} global init function(s)", global_init_count);
        for entry in inventory::iter::<__registry::InitEntry>() {
            tracing::info!("Init fn: {} ({}:{})", entry.fn_name, entry.file, entry.line);
            (entry.init_fn)(cx);
        }
    }

    let apply_locale = options.apply_locale;
    let runtime = gpui_storybook_core::preferences::RuntimeOptions {
        repository: gpui_storybook_core::preferences::repository_options(
            options.consumer_id,
            options.persistence,
            options.json_path,
            init_context.project_root,
        ),
        languages,
        supported_languages,
        locale_detector: std::sync::Arc::new(gpui_storybook_preferences::SystemLocaleDetector),
        initial_scheme: gpui_storybook_core::preferences::color_scheme(cx.window_appearance()),
        overrides: gpui_storybook_preferences::ResolutionOverrides {
            color_scheme: options.overrides.color_scheme,
            theme: options.overrides.theme,
            language: override_language,
        },
        apply_consumer_locale: std::rc::Rc::new(move |language, cx| {
            apply_locale(language, cx).map_err(|error| {
                tracing::error!(
                    error = %error,
                    error_debug = ?error,
                    "consumer Storybook locale adapter failed"
                );
                "consumer_locale".to_owned()
            })
        }),
        localize_consumer_language: std::rc::Rc::new(|language, cx| {
            gpui_es_fluent::try_localize_message(cx, &language)
        }),
    };
    let readiness = gpui_storybook_core::preferences::initialize(runtime, cx).map_err(|error| {
        tracing::error!(error = %error, "failed to resolve initial Storybook preferences");
        StorybookInitError::CoreInitialization {
            category: "preference_resolution".to_owned(),
        }
    })?;
    install_live_automation(cx);
    cx.set_global(StorybookInitialized);

    Ok(cx.spawn(async move |_cx| {
        let ready = readiness.await;
        #[cfg(all(feature = "mcp", any(target_os = "linux", target_os = "macos")))]
        {
            _cx.update(start_mcp_automation);
        }
        ready
    }))
}
