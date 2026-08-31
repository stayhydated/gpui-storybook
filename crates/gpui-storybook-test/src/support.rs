use super::*;

pub(super) fn effective_settle_frames(
    requested: u32,
    configured: u32,
    performance: Option<&PerformanceOptions>,
) -> u32 {
    let settled = if requested == 0 {
        configured
    } else {
        requested
    };
    settled.max(performance.map_or(0, |options| options.measured_frames))
}

pub(super) fn headless_error(error: anyhow::Error) -> StorybookTestError {
    StorybookTestError::Headless {
        message: error.to_string(),
    }
}

pub(super) fn write_png(path: &Path, image: &RgbaImage) -> Result<(), StorybookTestError> {
    if let Some(parent) = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        std::fs::create_dir_all(parent).map_err(|error| StorybookTestError::Output {
            path: parent.to_path_buf(),
            message: error.to_string(),
        })?;
    }
    image
        .save_with_format(path, image::ImageFormat::Png)
        .map_err(|error| StorybookTestError::Output {
            path: path.to_path_buf(),
            message: error.to_string(),
        })
}

pub(super) fn validate_case_configuration(
    case: &CaptureCase,
    config: &RunnerConfig,
) -> Result<(), StorybookTestError> {
    if case
        .theme
        .theme
        .as_deref()
        .is_some_and(|theme| builtin_theme_mode(theme).is_none())
        && config.case_configurator.is_none()
    {
        return Err(StorybookTestError::CaseConfigurationRequired {
            axis: "theme".to_owned(),
        });
    }
    if case.language.language.is_some() && config.case_configurator.is_none() {
        return Err(StorybookTestError::CaseConfigurationRequired {
            axis: "language".to_owned(),
        });
    }
    Ok(())
}

pub(super) fn configuration_axis(case: &CaptureCase) -> String {
    if case.theme.theme.is_some() {
        "theme".to_owned()
    } else if case.language.language.is_some() {
        "language".to_owned()
    } else {
        "presentation".to_owned()
    }
}

pub(super) fn builtin_theme_mode(theme: &str) -> Option<ThemeMode> {
    match theme.trim().to_ascii_lowercase().as_str() {
        "light" | "default light" => Some(ThemeMode::Light),
        "dark" | "default dark" => Some(ThemeMode::Dark),
        _ => None,
    }
}

pub(super) fn apply_builtin_theme(theme: &ThemeCase, window: &mut Window, app: &mut App) {
    if let Some(theme) = theme.theme.as_deref()
        && let Some(mode) = builtin_theme_mode(theme)
    {
        Theme::change(mode, Some(window), app);
    }
}

pub(super) fn initialize_portable_story_app(app: &mut App) -> Result<(), StorybookTestError> {
    #[cfg(not(target_family = "wasm"))]
    gpui_tokio::init(app);
    init_story_runtime(app).map_err(|error| StorybookTestError::RuntimeInitialization {
        message: error.to_string(),
    })?;
    for init in inventory::iter::<InitEntry>() {
        (init.init_fn)(app);
    }
    Ok(())
}

pub(super) fn apply_controls_to_story(
    story: &Entity<StoryContainer>,
    controls: &BTreeMap<String, ControlValue>,
    app: &mut App,
) -> Result<(), StorybookTestError> {
    if controls.is_empty() {
        return Ok(());
    }
    let target = {
        let story = story.read(app);
        story.control_target()
    }
    .ok_or_else(|| StorybookTestError::ControlsUnavailable {
        key: "capture".to_owned(),
    })?;
    for (key, value) in controls {
        target.set(key, value.clone(), app)?;
    }
    Ok(())
}

pub(super) fn read_control_snapshots(
    target: Option<Rc<dyn ControlTarget>>,
    app: &mut App,
) -> Result<Vec<ControlSnapshot>, StorybookTestError> {
    match target {
        Some(target) => target.snapshots(app).map_err(StorybookTestError::from),
        None => Ok(Vec::new()),
    }
}

pub(super) fn uses_core_route_registry(route: &RouteCase, has_custom_route_capture: bool) -> bool {
    matches!(route, RouteCase::Substory { .. }) && !has_custom_route_capture
}

pub(super) fn viewport_preset(viewport: &ViewportCase) -> StoryViewportPreset {
    match (viewport.width, viewport.height) {
        (390, 844) => StoryViewportPreset::Mobile,
        (768, 1024) => StoryViewportPreset::Tablet,
        (1440, 900) => StoryViewportPreset::Desktop,
        _ => StoryViewportPreset::Responsive,
    }
}

pub(crate) fn encode_id_fragment(value: &str) -> String {
    const HEX: &[u8; 16] = b"0123456789ABCDEF";

    let mut encoded = String::with_capacity(value.len());
    for byte in value.bytes() {
        if byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'-' | b'_' | b'.')
        {
            encoded.push(char::from(byte));
        } else {
            encoded.push('%');
            encoded.push(char::from(HEX[usize::from(byte >> 4)]));
            encoded.push(char::from(HEX[usize::from(byte & 0x0f)]));
        }
    }
    encoded
}

/// Encodes a case ID as one deterministic PNG filename component.
pub(crate) fn case_file_name(id: &str) -> String {
    format!("id-{}", encode_id_fragment(id))
}
