//! Public facade for building GPUI storybook binaries.
//!
//! Most applications should depend on this crate rather than the lower-level
//! runtime, macro, or TOML crates. It re-exports the standard runtime shell,
//! story traits, locale helpers, window helpers, and, with the default
//! `macros` feature, the story registration macros.
//!
//! Story registration flows through `inventory`: `#[story]` and
//! `#[derive(ComponentStory)]` submit story entries, `#[derive(Substory)]`
//! derives stable capture keys for styled `section` or custom
//! `StorySectionBase` regions inside a story, and
//! `#[story_init]` submits one-time setup hooks. The hidden `__registry` and
//! `__inventory` re-exports are the stable expansion path used by those
//! macros.
//!
//! [`init`] and `generate_stories` load crate-local `storybook.toml` files for
//! discovered story crates and select a runtime config by matching the running
//! binary name against registered story crate names. Initialization applies
//! launch-only preference overrides; story generation applies `allow` and
//! `disable_story` filtering, then materializes sorted [`StoryContainer`]
//! values. A story crate config's `group` becomes the sidebar's outer group; a
//! story's declared section remains the nested label.
//!
//! Macro-generated stories carry stable [`StoryKey`] values in the form
//! `{crate-package-name}-{registered-story-name}`. These keys are copied into
//! generated [`StoryContainer`] values as typed [`RegisteredStoryMetadata`] for
//! automation and capture routes.
//!
//! Feature boundaries:
//!
//! - `macros`: re-exports proc macros from `gpui-storybook-macros`
//! - `dock`: re-exports the dock workspace helpers from `gpui-storybook-core`
//! - `mcp`: installs a default automation controller during [`init`] and
//!   re-exports MCP automation and capture helpers
//!
//! Applications with embedded locale assets should call
//! `es_fluent_build::track_i18n_assets()` from `build.rs`. Define the embedded
//! i18n module and typed language enum in library-reachable code, then pass
//! typed [`StorybookOptions`] to [`init`] and await readiness before creating a
//! story window.
//!
//! [`PreferenceState::saved`] retains durable user intent, including `System`
//! choices and independent light/dark theme slots. [`PreferenceState::resolved`]
//! reports effective values and their sources after live system detection,
//! registry fallback, and deterministic overrides. [`PersistenceStatus`] is
//! storage-only; locale-adapter failures are reported as diagnostics and are
//! retried on later window activation without falsifying storage state.

#[cfg(feature = "macros")]
pub use gpui_storybook_macros::*;

use std::{
    collections::{BTreeMap, HashMap},
    fmt,
    path::{Path, PathBuf},
};

pub mod preferences;
pub use preferences::{
    ColorSchemeResolution, ColorSchemeSource, ConsumerId, ConsumerIdError, LanguageResolution,
    LanguageSource, LanguageTag, LocaleApplicationError, PersistenceMode, PersistenceStatus,
    PreferenceDiagnostic, PreferenceOverrides, PreferenceState, PreferredColorScheme,
    PreferredLanguage, PreferredLanguageMode, PreferredScrollbar, RecoveryDiagnostic,
    RecoveryReason, ResolutionDiagnostic, ResolvedPreferences, StorybookInitError,
    StorybookOptions, StorybookPreferences, StorybookReady, SystemColorScheme, ThemeId,
    ThemeIdError, ThemeResolution, ThemeSource, UnsupportedValueSource,
};

pub use gpui_es_fluent::try_localize_message as localize_message;
#[cfg(feature = "dock")]
pub use gpui_storybook_core::dock_gallery::{
    StoryWorkspace, create_dock_window, register_story_panels,
};
pub use gpui_storybook_core::registry::{
    RegisteredStoryMetadata, StoryKey, StoryName, StorySectionName,
};
#[cfg(feature = "dock")]
pub use gpui_storybook_core::window_view::DockWindowView;
pub use gpui_storybook_core::{
    assets::Assets,
    capture_region::{
        capture_route_slug, capture_substory, capture_substory_route_id,
        capture_substory_route_id_with_key, capture_substory_with_key,
    },
    gallery::Gallery,
    language::{CurrentLanguage, Language},
    story::{
        Story, StoryContainer, StorySection, StorySectionBase, StorySectionTitle, Substory,
        create_new_window, create_new_window_with_ui, section,
    },
    storybook_window_ui::{StorybookWindow, StorybookWindowUi},
    window_view::SimpleWindowView,
};

#[doc(hidden)]
pub use gpui_storybook_core::registry as __registry;

#[doc(hidden)]
pub use inventory as __inventory;

#[cfg(feature = "mcp")]
pub mod mcp {
    pub use gpui_storybook_mcp::*;
}

#[cfg(feature = "mcp")]
pub mod capture {
    pub use gpui_storybook_mcp::capture::*;
}

struct ResolvedStoryEntry {
    entry: &'static __registry::StoryEntry,
    group: Option<String>,
    section: Option<String>,
}

#[derive(Debug)]
struct DuplicateStoryKeyError {
    key: StoryKey,
    first: StoryRegistrationLocation,
    second: StoryRegistrationLocation,
}

#[derive(Clone, Debug)]
struct StoryRegistrationLocation {
    crate_name: &'static str,
    story_name: StoryName,
    file: &'static str,
    line: u32,
}

impl From<&'static __registry::StoryEntry> for StoryRegistrationLocation {
    fn from(entry: &'static __registry::StoryEntry) -> Self {
        Self {
            crate_name: entry.crate_name,
            story_name: entry.name,
            file: entry.file,
            line: entry.line,
        }
    }
}

impl fmt::Display for DuplicateStoryKeyError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "duplicate story key `{}` registered by {}::{} at {}:{} and {}::{} at {}:{}",
            self.key,
            self.first.crate_name,
            self.first.story_name,
            self.first.file,
            self.first.line,
            self.second.crate_name,
            self.second.story_name,
            self.second.file,
            self.second.line,
        )
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct StoryGroupKey {
    group: Option<String>,
    section: Option<String>,
    title: String,
}

fn group_duplicate_story_titles(
    stories: Vec<::gpui::Entity<StoryContainer>>,
    window: &mut ::gpui::Window,
    cx: &mut ::gpui::App,
) -> Vec<::gpui::Entity<StoryContainer>> {
    let mut grouped: Vec<(StoryGroupKey, Vec<::gpui::Entity<StoryContainer>>)> = Vec::new();

    for story in stories {
        let key = {
            let story_data = story.read(cx);
            StoryGroupKey {
                group: story_data.group.as_ref().map(ToString::to_string),
                section: story_data.section.as_ref().map(ToString::to_string),
                title: story_data.display_title(cx),
            }
        };

        if let Some((_, bucket)) = grouped
            .iter_mut()
            .find(|(existing_key, _)| *existing_key == key)
        {
            bucket.push(story);
        } else {
            grouped.push((key, vec![story]));
        }
    }

    grouped
        .into_iter()
        .map(|(key, bucket)| {
            if bucket.len() == 1 {
                return bucket.into_iter().next().expect("bucket has one story");
            }

            let panel = StoryContainer::list_panel(key.title, bucket, window, cx);
            panel.update(cx, |container, _| {
                container.group = key.group.map(Into::into);
                container.section = key.section.map(Into::into);
            });
            panel
        })
        .collect()
}

fn load_storybook_config(
    entry: &__registry::StoryEntry,
) -> Option<gpui_storybook_toml::StorybookToml> {
    match gpui_storybook_toml::load_from_dir(entry.crate_dir) {
        Ok(config) => config,
        Err(err) => {
            tracing::warn!(
                "Failed to load storybook.toml for crate '{}' ({}): {}",
                entry.crate_name,
                entry.crate_dir,
                err
            );
            None
        },
    }
}

fn current_binary_name() -> Option<String> {
    let argv0 = std::env::args_os().next()?;
    PathBuf::from(argv0)
        .file_stem()
        .map(|stem| stem.to_string_lossy().to_string())
}

fn load_runtime_storybook_config(
    all_entries: &[&'static __registry::StoryEntry],
    crate_configs: &mut HashMap<&'static str, Option<gpui_storybook_toml::StorybookToml>>,
) -> Option<gpui_storybook_toml::StorybookToml> {
    let entry = runtime_story_entry(all_entries)?;

    crate_configs
        .entry(entry.crate_dir)
        .or_insert_with(|| load_storybook_config(entry))
        .clone()
}

fn runtime_story_entry(
    all_entries: &[&'static __registry::StoryEntry],
) -> Option<&'static __registry::StoryEntry> {
    let bin_name = current_binary_name()?;
    all_entries
        .iter()
        .copied()
        .find(|entry| entry.crate_name == bin_name)
}

struct InitContext {
    runtime_config: Option<gpui_storybook_toml::StorybookToml>,
    project_root: PathBuf,
}

fn find_cargo_project_root(start: &Path) -> PathBuf {
    let mut nearest_manifest_dir = None;

    for directory in start.ancestors() {
        let manifest_path = directory.join("Cargo.toml");
        if !manifest_path.is_file() {
            continue;
        }
        nearest_manifest_dir.get_or_insert_with(|| directory.to_path_buf());

        let declares_workspace = std::fs::read_to_string(&manifest_path)
            .ok()
            .and_then(|contents| contents.parse::<toml::Table>().ok())
            .is_some_and(|manifest| manifest.contains_key("workspace"));
        if declares_workspace {
            return directory.to_path_buf();
        }
    }

    nearest_manifest_dir.unwrap_or_else(|| start.to_path_buf())
}

fn load_init_context() -> Result<InitContext, StorybookInitError> {
    let all_entries = inventory::iter::<__registry::StoryEntry>().collect::<Vec<_>>();
    if let Some(entry) = runtime_story_entry(&all_entries) {
        let runtime_config = gpui_storybook_toml::load_from_dir(entry.crate_dir)
            .map_err(|source| StorybookInitError::RuntimeConfig { source })?;
        return Ok(InitContext {
            runtime_config,
            project_root: find_cargo_project_root(Path::new(entry.crate_dir)),
        });
    }

    let working_directory = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    Ok(InitContext {
        runtime_config: None,
        project_root: find_cargo_project_root(&working_directory),
    })
}

fn apply_toml_preference_overrides<L>(
    overrides: &mut PreferenceOverrides<L>,
    config: &gpui_storybook_toml::StorybookToml,
) -> Result<(), StorybookInitError>
where
    L: Language,
{
    let configured = &config.overrides;

    if overrides.color_scheme.is_none() {
        overrides.color_scheme = configured.color_scheme.map(|scheme| match scheme {
            gpui_storybook_toml::StorybookColorScheme::Light => SystemColorScheme::Light,
            gpui_storybook_toml::StorybookColorScheme::Dark => SystemColorScheme::Dark,
        });
    }

    if overrides.theme.is_none()
        && let Some(theme) = configured.theme.as_ref()
    {
        let theme = ThemeId::new(theme).map_err(|_| StorybookInitError::InvalidTomlOverride {
            field: "overrides.theme",
            value: theme.clone(),
        })?;
        overrides.theme = Some(theme);
    }

    if overrides.language.is_none()
        && let Some(language) = configured.language.as_ref()
    {
        let tag = gpui_storybook_preferences::LanguageTag::new(language).map_err(|_| {
            StorybookInitError::InvalidTomlOverride {
                field: "overrides.language",
                value: language.clone(),
            }
        })?;
        let typed = L::try_from(tag.as_identifier().clone()).map_err(|_| {
            StorybookInitError::InvalidTomlOverride {
                field: "overrides.language",
                value: language.clone(),
            }
        })?;
        overrides.language = Some(typed);
    }

    Ok(())
}

fn resolve_story_entry(
    entry: &'static __registry::StoryEntry,
    crate_group: Option<&str>,
    runtime_config: Option<&gpui_storybook_toml::StorybookToml>,
) -> Option<ResolvedStoryEntry> {
    let entry_section = entry.section.map(StorySectionName::as_str);
    let filter_group = crate_group.or(entry_section);

    if let Some(runtime_config) = runtime_config
        && !runtime_config.allows_group(filter_group)
    {
        tracing::debug!(
            "Skipping story '{}' from crate '{}' because group '{:?}' is not listed in runtime allow",
            entry.name,
            entry.crate_name,
            filter_group
        );
        return None;
    }

    if let Some(runtime_config) = runtime_config
        && runtime_config.is_story_disabled(entry.name.as_str())
    {
        tracing::debug!(
            "Skipping story '{}' from crate '{}' because it is listed in runtime disable_story",
            entry.name,
            entry.crate_name
        );
        return None;
    }

    Some(ResolvedStoryEntry {
        entry,
        group: crate_group.map(str::to_string),
        section: entry.section.map(|section| section.as_str().to_string()),
    })
}

fn compare_resolved_story_entries(
    a: &ResolvedStoryEntry,
    b: &ResolvedStoryEntry,
) -> std::cmp::Ordering {
    match (a.entry.section_order, b.entry.section_order) {
        (Some(order_a), Some(order_b)) => order_a
            .cmp(&order_b)
            .then_with(|| a.entry.name.cmp(&b.entry.name)),
        (Some(_), None) => std::cmp::Ordering::Less,
        (None, Some(_)) => std::cmp::Ordering::Greater,
        (None, None) => match (&a.section, &b.section) {
            (Some(sec_a), Some(sec_b)) => sec_a
                .cmp(sec_b)
                .then_with(|| a.entry.name.cmp(&b.entry.name)),
            (Some(_), None) => std::cmp::Ordering::Less,
            (None, Some(_)) => std::cmp::Ordering::Greater,
            (None, None) => a.entry.name.cmp(&b.entry.name),
        },
    }
}

struct StorybookInitialized;

impl ::gpui::Global for StorybookInitialized {}

#[cfg(feature = "mcp")]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum AutomationPreferenceProfile {
    Capture,
    Stdio,
}

#[cfg(feature = "mcp")]
fn apply_automation_preference_profile<L>(
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
/// localization, story registrations, and optional automation hooks. Await the
/// returned task before opening the first window so saved theme and language
/// intent is applied before the first frame.
///
/// The active runtime `storybook.toml` may provide launch-only preference
/// overrides. Values supplied through [`StorybookOptions::with_overrides`] take
/// precedence field by field, and deterministic automation profiles take
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
    cx: &mut ::gpui::App,
    mut options: StorybookOptions<L>,
) -> Result<::gpui::Task<StorybookReady>, StorybookInitError>
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
    if let Some(runtime_config) = init_context.runtime_config.as_ref() {
        apply_toml_preference_overrides(&mut options.overrides, runtime_config)?;
    }

    #[cfg(feature = "mcp")]
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

    gpui_tokio::init(cx);
    gpui_storybook_core::story::init(cx).map_err(|error| {
        tracing::error!(error = %error, error_debug = ?error, "failed to initialize Storybook localization");
        StorybookInitError::CoreInitialization {
            category: "embedded_localization".to_owned(),
        }
    })?;
    #[cfg(feature = "dock")]
    register_story_panels(cx);

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
    cx.set_global(StorybookInitialized);

    Ok(cx.spawn(async move |_cx| {
        let ready = readiness.await;
        #[cfg(feature = "mcp")]
        {
            _cx.update(init_mcp_automation);
        }
        ready
    }))
}

/// Returns the read-only saved and resolved preference snapshot after
/// initialization has begun.
///
/// The snapshot reports `Loading` until the readiness task completes. It stays
/// available in `Error` state when storage fails and fallback presentation is
/// active.
pub fn try_preference_state(cx: &::gpui::App) -> Option<&PreferenceState> {
    gpui_storybook_core::preferences::try_state(cx)
}

#[cfg(feature = "mcp")]
fn init_mcp_automation(cx: &mut ::gpui::App) {
    let automation = gpui_storybook_core::automation::default_storybook_automation(cx)
        .unwrap_or_else(|| {
            gpui_storybook_core::automation::set_default_storybook_automation(
                cx,
                gpui_storybook_mcp::StorybookAutomation::new(),
            )
        });

    if gpui_storybook_mcp::stdio_requested()
        && let Err(error) = gpui_storybook_mcp::start_stdio(automation.clone())
    {
        eprintln!("failed to start gpui-storybook MCP stdio server: {error}");
    }

    if let Err(error) = gpui_storybook_mcp::start_capture_session_from_env(automation) {
        eprintln!("failed to start storybook capture session: {error}");
    }
}

/// Discovers registered stories, applies `storybook.toml` filtering, and
/// returns runtime story containers.
///
/// The active runtime config is selected from the registered story crate whose
/// package name matches the running binary. If no registered story crate
/// matches the binary name, crate-local groups are still attached to stories
/// but runtime `allow` and `disable_story` filters are not applied.
///
/// Stories are sorted by enum-section order when available, then by section and
/// registered story name. Stories with the same title in the same group and
/// section are grouped into one list panel.
pub fn generate_stories(
    window: &mut ::gpui::Window,
    cx: &mut ::gpui::App,
) -> Vec<::gpui::Entity<StoryContainer>> {
    let story_count = inventory::iter::<__registry::StoryEntry>().count();
    let init_count = inventory::iter::<__registry::InitEntry>().count();

    tracing::info!("Discovered {} story(ies)", story_count);
    tracing::info!(
        "Init functions registered: {}",
        if init_count > 0 {
            format!("{} function(s)", init_count)
        } else {
            "none".to_string()
        }
    );

    let all_entries: Vec<_> = inventory::iter::<__registry::StoryEntry>().collect();
    validate_unique_story_keys(&all_entries)
        .unwrap_or_else(|error| panic!("invalid storybook registry: {error}"));
    let mut crate_configs: HashMap<&'static str, Option<gpui_storybook_toml::StorybookToml>> =
        HashMap::new();
    let runtime_config = load_runtime_storybook_config(&all_entries, &mut crate_configs);

    if let Some(runtime_config) = runtime_config.as_ref()
        && let Some(group) = runtime_config.group()
    {
        tracing::info!(
            "Using runtime storybook.toml with group '{}' and allow {:?}",
            group,
            runtime_config.allow.as_ref()
        );
    }

    let mut entries: Vec<_> = all_entries
        .into_iter()
        .filter_map(|entry| {
            let config = crate_configs
                .entry(entry.crate_dir)
                .or_insert_with(|| load_storybook_config(entry));

            resolve_story_entry(
                entry,
                config
                    .as_ref()
                    .and_then(gpui_storybook_toml::StorybookToml::group),
                runtime_config.as_ref(),
            )
        })
        .collect();

    tracing::info!(
        "Collected {} story(ies) after storybook.toml filtering",
        entries.len()
    );

    entries.sort_by(compare_resolved_story_entries);

    let stories = entries
        .into_iter()
        .map(|resolved| {
            let section_info = resolved
                .section
                .as_ref()
                .map(|s| format!(", section: \"{}\"", s))
                .unwrap_or_default();
            let group_info = resolved
                .group
                .as_ref()
                .map(|group| format!(", group: \"{}\"", group))
                .unwrap_or_default();

            tracing::info!(
                "Story: {} (key: {}){}{} ({}:{}) [{}]",
                resolved.entry.name,
                resolved.entry.key(),
                section_info,
                group_info,
                resolved.entry.file,
                resolved.entry.line,
                resolved.entry.crate_name
            );

            let container = (resolved.entry.create_fn)(window, cx);
            container.update(cx, |c, _| {
                c.group = resolved.group.clone().map(Into::into);
                c.section = resolved.section.clone().map(Into::into);
                c.set_registration_metadata(resolved.entry.metadata());
            });
            container
        })
        .collect();

    group_duplicate_story_titles(stories, window, cx)
}

fn validate_unique_story_keys(
    entries: &[&'static __registry::StoryEntry],
) -> Result<(), Box<DuplicateStoryKeyError>> {
    let mut seen = BTreeMap::new();

    for entry in entries {
        if let Some(first) = seen.insert(entry.key(), *entry) {
            return Err(Box::new(DuplicateStoryKeyError {
                key: entry.key(),
                first: StoryRegistrationLocation::from(first),
                second: StoryRegistrationLocation::from(*entry),
            }));
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use es_fluent::{FluentMessage, FluentMessageLookup};
    use gpui::AppContext as _;
    use std::{
        convert::Infallible,
        path::Path,
        time::{SystemTime, UNIX_EPOCH},
    };
    use unic_langid::LanguageIdentifier;

    #[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
    enum TestLanguage {
        #[default]
        English,
    }

    impl strum::IntoEnumIterator for TestLanguage {
        type Iterator = std::array::IntoIter<Self, 1>;

        fn iter() -> Self::Iterator {
            [Self::English].into_iter()
        }
    }

    impl From<TestLanguage> for LanguageIdentifier {
        fn from(_: TestLanguage) -> Self {
            "en".parse().expect("test language tag should be valid")
        }
    }

    impl TryFrom<LanguageIdentifier> for TestLanguage {
        type Error = ();

        fn try_from(identifier: LanguageIdentifier) -> Result<Self, Self::Error> {
            (identifier.language.as_str() == "en")
                .then_some(Self::English)
                .ok_or(())
        }
    }

    impl FluentMessage for TestLanguage {
        fn to_fluent_string_with(&self, _: &mut FluentMessageLookup<'_>) -> String {
            "English".to_owned()
        }
    }

    fn test_options() -> StorybookOptions<TestLanguage> {
        StorybookOptions::new(
            ConsumerId::new("facade-preference-test").expect("test consumer id should be valid"),
            TestLanguage::English,
            |_, _| Ok::<(), Infallible>(()),
        )
    }

    fn unused_create_fn(
        _: &mut ::gpui::Window,
        _: &mut ::gpui::App,
    ) -> ::gpui::Entity<StoryContainer> {
        unreachable!("story creation is not used in these tests");
    }

    static SECTIONED_ENTRY: __registry::StoryEntry = __registry::StoryEntry::new(
        "component-example-SectionedStory",
        "SectionedStory",
        Some("Notes"),
        None,
        unused_create_fn,
        __registry::StoryRegistrationSource::new(
            "component-example",
            "/tmp/component-example",
            "examples/component/src/components/field_notes.rs",
            10,
        ),
    );

    static UNSECTIONED_ENTRY: __registry::StoryEntry = __registry::StoryEntry::new(
        "component-example-UnsectionedStory",
        "UnsectionedStory",
        None,
        None,
        unused_create_fn,
        __registry::StoryRegistrationSource::new(
            "component-example",
            "/tmp/component-example",
            "examples/component/src/components/field_notes.rs",
            42,
        ),
    );

    static ORDERED_FIRST: __registry::StoryEntry = __registry::StoryEntry::new(
        "component-example-ZStory",
        "ZStory",
        Some("Zed"),
        Some(1),
        unused_create_fn,
        __registry::StoryRegistrationSource::new(
            "component-example",
            "/tmp/component-example",
            "src/z.rs",
            1,
        ),
    );

    static ORDERED_SECOND: __registry::StoryEntry = __registry::StoryEntry::new(
        "component-example-AStory",
        "AStory",
        Some("Alpha"),
        Some(2),
        unused_create_fn,
        __registry::StoryRegistrationSource::new(
            "component-example",
            "/tmp/component-example",
            "src/a.rs",
            2,
        ),
    );

    static ORDERED_FIRST_ALPHA: __registry::StoryEntry = __registry::StoryEntry::new(
        "component-example-AStory",
        "AStory",
        Some("Alpha"),
        Some(1),
        unused_create_fn,
        __registry::StoryRegistrationSource::new(
            "component-example",
            "/tmp/component-example",
            "src/a.rs",
            3,
        ),
    );

    fn with_temp_dir(test_fn: impl FnOnce(&Path)) {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time should move forward")
            .as_nanos();
        let path = std::env::temp_dir().join(format!("gpui_storybook_facade_{timestamp}"));
        std::fs::create_dir_all(&path).expect("temp directory should be created");
        test_fn(&path);
        std::fs::remove_dir_all(path).expect("temp directory should be removed");
    }

    fn runtime_config(allow: &[&str]) -> gpui_storybook_toml::StorybookToml {
        gpui_storybook_toml::StorybookToml {
            group: "storybook-app".into(),
            allow: Some(allow.iter().map(|group| (*group).to_string()).collect()),
            disable_story: Vec::new(),
            overrides: gpui_storybook_toml::StorybookPreferenceOverrides::default(),
        }
    }

    #[test]
    fn toml_preference_overrides_map_to_typed_runtime_values() {
        let config = gpui_storybook_toml::StorybookToml {
            group: "storybook-app".to_owned(),
            overrides: gpui_storybook_toml::StorybookPreferenceOverrides {
                color_scheme: Some(gpui_storybook_toml::StorybookColorScheme::Dark),
                theme: Some("Default Dark".to_owned()),
                language: Some("en".to_owned()),
            },
            ..gpui_storybook_toml::StorybookToml::default()
        };
        let mut overrides = PreferenceOverrides::<TestLanguage>::default();

        apply_toml_preference_overrides(&mut overrides, &config)
            .expect("valid TOML overrides should map to typed values");

        assert_eq!(overrides.color_scheme, Some(SystemColorScheme::Dark));
        assert_eq!(
            overrides.theme.as_ref().map(ThemeId::as_str),
            Some("Default Dark")
        );
        assert_eq!(overrides.language, Some(TestLanguage::English));
    }

    #[test]
    fn programmatic_overrides_take_precedence_over_toml() {
        let config = gpui_storybook_toml::StorybookToml {
            group: "storybook-app".to_owned(),
            overrides: gpui_storybook_toml::StorybookPreferenceOverrides {
                color_scheme: Some(gpui_storybook_toml::StorybookColorScheme::Light),
                theme: Some("Default Light".to_owned()),
                language: Some("fr".to_owned()),
            },
            ..gpui_storybook_toml::StorybookToml::default()
        };
        let mut overrides = PreferenceOverrides {
            color_scheme: Some(SystemColorScheme::Dark),
            theme: Some(ThemeId::new("Custom Dark").expect("test theme id should be valid")),
            language: Some(TestLanguage::English),
        };

        apply_toml_preference_overrides(&mut overrides, &config)
            .expect("programmatic values should bypass conflicting TOML values");

        assert_eq!(overrides.color_scheme, Some(SystemColorScheme::Dark));
        assert_eq!(
            overrides.theme.as_ref().map(ThemeId::as_str),
            Some("Custom Dark")
        );
        assert_eq!(overrides.language, Some(TestLanguage::English));
    }

    #[test]
    fn unsupported_toml_language_is_an_initialization_error() {
        let config = gpui_storybook_toml::StorybookToml {
            group: "storybook-app".to_owned(),
            overrides: gpui_storybook_toml::StorybookPreferenceOverrides {
                language: Some("fr".to_owned()),
                ..gpui_storybook_toml::StorybookPreferenceOverrides::default()
            },
            ..gpui_storybook_toml::StorybookToml::default()
        };
        let mut overrides = PreferenceOverrides::<TestLanguage>::default();

        let error = apply_toml_preference_overrides(&mut overrides, &config)
            .expect_err("unsupported typed language should fail initialization");

        assert!(matches!(
            error,
            StorybookInitError::InvalidTomlOverride {
                field: "overrides.language",
                value,
            } if value == "fr"
        ));
    }

    #[cfg(feature = "mcp")]
    #[test]
    fn capture_profile_is_deterministic_and_disables_storage() {
        let mut persistence = PersistenceMode::Persistent;
        let mut json_path = Some(PathBuf::from("portable/preferences.json"));
        let mut overrides = PreferenceOverrides {
            color_scheme: Some(SystemColorScheme::Dark),
            theme: Some(ThemeId::new("Custom Dark").expect("test theme id should be valid")),
            language: Some(1_u8),
        };

        apply_automation_preference_profile(
            AutomationPreferenceProfile::Capture,
            &mut persistence,
            &mut json_path,
            &mut overrides,
            7_u8,
        )
        .expect("the built-in deterministic theme should be valid");

        assert_eq!(persistence, PersistenceMode::Disabled);
        assert_eq!(json_path, None);
        assert_eq!(overrides.color_scheme, Some(SystemColorScheme::Light));
        assert_eq!(
            overrides.theme.as_ref().map(ThemeId::as_str),
            Some("Default Light")
        );
        assert_eq!(overrides.language, Some(7));
    }

    #[cfg(feature = "mcp")]
    #[test]
    fn stdio_profile_is_deterministic_and_uses_temporary_storage() {
        let mut persistence = PersistenceMode::Persistent;
        let mut json_path = Some(PathBuf::from("portable/preferences.json"));
        let mut overrides = PreferenceOverrides {
            color_scheme: Some(SystemColorScheme::Dark),
            theme: Some(ThemeId::new("Custom Dark").expect("test theme id should be valid")),
            language: Some(1_u8),
        };

        apply_automation_preference_profile(
            AutomationPreferenceProfile::Stdio,
            &mut persistence,
            &mut json_path,
            &mut overrides,
            7_u8,
        )
        .expect("the built-in deterministic theme should be valid");

        assert_eq!(persistence, PersistenceMode::Temporary);
        assert_eq!(json_path, None);
        assert_eq!(overrides.color_scheme, Some(SystemColorScheme::Light));
        assert_eq!(
            overrides.theme.as_ref().map(ThemeId::as_str),
            Some("Default Light")
        );
        assert_eq!(overrides.language, Some(7));
    }

    #[gpui::test]
    fn init_rejects_a_path_override_for_non_persistent_storage(cx: &mut ::gpui::App) {
        let options = test_options()
            .with_persistence(PersistenceMode::Temporary)
            .with_json_path("portable/preferences.json");

        let result = init(cx, options);

        assert!(matches!(
            result,
            Err(StorybookInitError::PathOverrideRequiresPersistent)
        ));
        assert!(cx.try_global::<StorybookInitialized>().is_none());
    }

    #[gpui::test]
    async fn init_rejects_a_second_initialization(cx: &mut ::gpui::TestAppContext) {
        cx.executor().allow_parking();
        let first = cx.update(|cx| {
            init(
                cx,
                test_options().with_persistence(PersistenceMode::Disabled),
            )
            .expect("the first initialization should start")
        });

        let second = cx.update(|cx| {
            init(
                cx,
                test_options().with_persistence(PersistenceMode::Disabled),
            )
        });
        assert!(matches!(
            second,
            Err(StorybookInitError::AlreadyInitialized)
        ));

        let _ready = first.await;
    }

    #[gpui::test]
    async fn readiness_completes_before_the_caller_constructs_a_window(
        cx: &mut ::gpui::TestAppContext,
    ) {
        cx.executor().allow_parking();
        assert!(cx.windows().is_empty());

        let readiness = cx.update(|cx| {
            init(
                cx,
                test_options().with_persistence(PersistenceMode::Disabled),
            )
            .expect("valid facade options should initialize")
        });
        assert!(cx.windows().is_empty());

        let ready = readiness.await;
        assert_eq!(ready.persistence_status, PersistenceStatus::Ready);
        assert!(cx.windows().is_empty());

        cx.update(|cx| {
            cx.open_window(Default::default(), |_, cx| cx.new(|_| ::gpui::EmptyView))
                .expect("caller should be able to create a window after readiness")
        });
        assert_eq!(cx.windows().len(), 1);
    }

    #[test]
    fn crate_group_filters_without_overwriting_declared_section() {
        let resolved = resolve_story_entry(
            &SECTIONED_ENTRY,
            Some("gpui-storybook-example-component"),
            Some(&runtime_config(&["gpui-storybook-example-component"])),
        )
        .expect("crate group should satisfy runtime allow");

        assert_eq!(
            resolved.group.as_deref(),
            Some("gpui-storybook-example-component")
        );
        assert_eq!(resolved.section.as_deref(), Some("Notes"));
    }

    #[test]
    fn unsectioned_stories_keep_crate_group_without_faking_a_section() {
        let resolved = resolve_story_entry(
            &UNSECTIONED_ENTRY,
            Some("gpui-storybook-example-component"),
            Some(&runtime_config(&["gpui-storybook-example-component"])),
        )
        .expect("crate group should satisfy runtime allow");

        assert_eq!(
            resolved.group.as_deref(),
            Some("gpui-storybook-example-component")
        );
        assert_eq!(resolved.section.as_deref(), None);
    }

    #[test]
    fn duplicate_story_key_validator_reports_both_registrations() {
        static FIRST_ENTRY: __registry::StoryEntry = __registry::StoryEntry::new(
            "component-example-ButtonStory",
            "ButtonStory",
            None,
            None,
            unused_create_fn,
            __registry::StoryRegistrationSource::new(
                "component-example",
                "/tmp/component-example",
                "src/first.rs",
                10,
            ),
        );
        static SECOND_ENTRY: __registry::StoryEntry = __registry::StoryEntry::new(
            "component-example-ButtonStory",
            "ButtonStory",
            None,
            None,
            unused_create_fn,
            __registry::StoryRegistrationSource::new(
                "component-example",
                "/tmp/component-example",
                "src/second.rs",
                20,
            ),
        );

        let error = validate_unique_story_keys(&[&FIRST_ENTRY, &SECOND_ENTRY])
            .expect_err("duplicate keys should be rejected");

        assert_eq!(error.key.as_str(), "component-example-ButtonStory");
        assert_eq!(
            error.to_string(),
            "duplicate story key `component-example-ButtonStory` registered by component-example::ButtonStory at src/first.rs:10 and component-example::ButtonStory at src/second.rs:20"
        );
    }

    #[test]
    fn unique_story_key_validator_accepts_empty_and_distinct_registrations() {
        assert!(validate_unique_story_keys(&[]).is_ok());
        assert!(validate_unique_story_keys(&[&SECTIONED_ENTRY, &UNSECTIONED_ENTRY]).is_ok());
    }

    #[test]
    fn runtime_filters_reject_unlisted_groups_and_disabled_stories() {
        assert!(
            resolve_story_entry(
                &SECTIONED_ENTRY,
                Some("component-example"),
                Some(&runtime_config(&["other"])),
            )
            .is_none()
        );

        let mut config = runtime_config(&["component-example"]);
        config.disable_story.push("SectionedStory".to_string());
        assert!(
            resolve_story_entry(&SECTIONED_ENTRY, Some("component-example"), Some(&config))
                .is_none()
        );
    }

    #[test]
    fn declared_section_is_the_filter_group_without_crate_config() {
        let resolved =
            resolve_story_entry(&SECTIONED_ENTRY, None, Some(&runtime_config(&["Notes"])))
                .expect("declared section should satisfy the allow list");

        assert_eq!(resolved.group, None);
        assert_eq!(resolved.section.as_deref(), Some("Notes"));
        assert!(resolve_story_entry(&UNSECTIONED_ENTRY, None, None).is_some());
    }

    #[test]
    fn resolved_entries_sort_by_order_section_then_name() {
        let ordered_first = ResolvedStoryEntry {
            entry: &ORDERED_FIRST,
            group: None,
            section: Some("Zed".to_string()),
        };
        let ordered_second = ResolvedStoryEntry {
            entry: &ORDERED_SECOND,
            group: None,
            section: Some("Alpha".to_string()),
        };
        let ordered_first_alpha = ResolvedStoryEntry {
            entry: &ORDERED_FIRST_ALPHA,
            group: None,
            section: Some("Alpha".to_string()),
        };
        let sectioned = ResolvedStoryEntry {
            entry: &SECTIONED_ENTRY,
            group: None,
            section: Some("Notes".to_string()),
        };
        let sectioned_alpha = ResolvedStoryEntry {
            entry: &ORDERED_SECOND,
            group: None,
            section: Some("Alpha".to_string()),
        };
        let unsectioned = ResolvedStoryEntry {
            entry: &UNSECTIONED_ENTRY,
            group: None,
            section: None,
        };

        assert!(compare_resolved_story_entries(&ordered_first, &ordered_second).is_lt());
        assert!(compare_resolved_story_entries(&ordered_second, &ordered_first).is_gt());
        assert!(compare_resolved_story_entries(&ordered_first_alpha, &ordered_first).is_lt());
        assert!(compare_resolved_story_entries(&ordered_first, &sectioned).is_lt());
        assert!(compare_resolved_story_entries(&sectioned, &ordered_first).is_gt());
        assert!(compare_resolved_story_entries(&sectioned_alpha, &sectioned).is_lt());
        assert!(compare_resolved_story_entries(&sectioned, &unsectioned).is_lt());
        assert!(compare_resolved_story_entries(&unsectioned, &sectioned).is_gt());

        let unsectioned_alpha = ResolvedStoryEntry {
            entry: &ORDERED_SECOND,
            group: None,
            section: None,
        };
        assert!(compare_resolved_story_entries(&unsectioned_alpha, &unsectioned).is_lt());
    }

    #[test]
    fn config_loading_handles_valid_missing_and_invalid_files() {
        with_temp_dir(|dir| {
            let crate_dir: &'static str =
                Box::leak(dir.to_string_lossy().into_owned().into_boxed_str());
            let entry = __registry::StoryEntry::new(
                "temp-Story",
                "Story",
                None,
                None,
                unused_create_fn,
                __registry::StoryRegistrationSource::new("temp", crate_dir, "src/lib.rs", 1),
            );

            assert_eq!(load_storybook_config(&entry), None);

            std::fs::write(dir.join("storybook.toml"), "group = \"Temp\"\n")
                .expect("valid config should be written");
            assert_eq!(
                load_storybook_config(&entry).map(|config| config.group),
                Some("Temp".to_string())
            );

            std::fs::write(dir.join("storybook.toml"), "invalid = true\n")
                .expect("invalid config should be written");
            assert_eq!(load_storybook_config(&entry), None);
        });
    }

    #[test]
    fn runtime_config_matches_the_current_test_binary_and_populates_cache() {
        with_temp_dir(|dir| {
            std::fs::write(dir.join("storybook.toml"), "group = \"Test Binary\"\n")
                .expect("runtime config should be written");
            let bin_name = current_binary_name().expect("test binary should have a file stem");
            let crate_name: &'static str = Box::leak(bin_name.into_boxed_str());
            let crate_dir: &'static str =
                Box::leak(dir.to_string_lossy().into_owned().into_boxed_str());
            let entry: &'static __registry::StoryEntry =
                Box::leak(Box::new(__registry::StoryEntry::new(
                    "test-Story",
                    "Story",
                    None,
                    None,
                    unused_create_fn,
                    __registry::StoryRegistrationSource::new(
                        crate_name,
                        crate_dir,
                        "src/lib.rs",
                        1,
                    ),
                )));
            let mut cache = HashMap::new();

            let config = load_runtime_storybook_config(&[entry], &mut cache)
                .expect("matching binary config should load");
            assert_eq!(config.group(), Some("Test Binary"));
            assert!(cache.contains_key(crate_dir));

            let unmatched = load_runtime_storybook_config(&[], &mut cache);
            assert_eq!(unmatched, None);
        });
    }

    #[test]
    fn project_root_prefers_the_workspace_and_supports_standalone_crates() {
        with_temp_dir(|dir| {
            let workspace = dir.join("workspace");
            let member = workspace.join("crates/member");
            let member_source = member.join("src");
            std::fs::create_dir_all(&member_source).expect("workspace member directories create");
            std::fs::write(
                workspace.join("Cargo.toml"),
                "[workspace]\nmembers = [\"crates/member\"]\n",
            )
            .expect("workspace manifest writes");
            std::fs::write(
                member.join("Cargo.toml"),
                "[package]\nname = \"member\"\nversion = \"0.1.0\"\n",
            )
            .expect("member manifest writes");
            assert_eq!(find_cargo_project_root(&member_source), workspace);

            let standalone = dir.join("standalone");
            let standalone_source = standalone.join("src");
            std::fs::create_dir_all(&standalone_source)
                .expect("standalone source directory creates");
            std::fs::write(
                standalone.join("Cargo.toml"),
                "[package]\nname = \"standalone\"\nversion = \"0.1.0\"\n",
            )
            .expect("standalone manifest writes");
            assert_eq!(find_cargo_project_root(&standalone_source), standalone);

            let no_manifest = dir.join("no-manifest");
            assert_eq!(find_cargo_project_root(&no_manifest), no_manifest);
        });
    }
}
