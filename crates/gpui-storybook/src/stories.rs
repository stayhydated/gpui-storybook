use super::*;

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
/// section share one navigation entry whose concrete variants open separately.
pub fn generate_stories(
    window: &mut ::gpui_kit::Window,
    cx: &mut ::gpui_kit::App,
) -> Vec<::gpui_kit::Entity<StoryContainer>> {
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

pub(super) fn validate_unique_story_keys(
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
