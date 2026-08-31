use super::*;

#[cfg(not(target_family = "wasm"))]
pub(super) fn load_storybook_config(
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

#[cfg(target_family = "wasm")]
pub(super) fn load_storybook_config(
    _entry: &__registry::StoryEntry,
) -> Option<gpui_storybook_toml::StorybookToml> {
    None
}

pub(super) fn current_binary_name() -> Option<String> {
    let argv0 = std::env::args_os().next()?;
    PathBuf::from(argv0)
        .file_stem()
        .map(|stem| stem.to_string_lossy().to_string())
}

pub(super) fn load_runtime_storybook_config(
    all_entries: &[&'static __registry::StoryEntry],
    crate_configs: &mut HashMap<&'static str, Option<gpui_storybook_toml::StorybookToml>>,
) -> Option<gpui_storybook_toml::StorybookToml> {
    let entry = runtime_story_entry(all_entries)?;

    crate_configs
        .entry(entry.crate_dir)
        .or_insert_with(|| load_storybook_config(entry))
        .clone()
}

pub(super) fn runtime_story_entry(
    all_entries: &[&'static __registry::StoryEntry],
) -> Option<&'static __registry::StoryEntry> {
    let bin_name = current_binary_name()?;
    all_entries
        .iter()
        .copied()
        .find(|entry| entry.crate_name == bin_name)
}

pub(super) struct InitContext {
    pub(super) runtime_config: Option<gpui_storybook_toml::StorybookToml>,
    pub(super) project_root: PathBuf,
}

#[cfg(not(target_family = "wasm"))]
pub(super) fn find_cargo_project_root(start: &Path) -> PathBuf {
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

#[cfg(not(target_family = "wasm"))]
pub(super) fn load_init_context() -> Result<InitContext, StorybookInitError> {
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

#[cfg(target_family = "wasm")]
pub(super) fn load_init_context() -> Result<InitContext, StorybookInitError> {
    Ok(InitContext {
        runtime_config: None,
        project_root: PathBuf::from("."),
    })
}

pub(super) fn apply_toml_preference_overrides<L>(
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

pub(super) fn resolve_story_entry(
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

pub(super) fn compare_resolved_story_entries(
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
