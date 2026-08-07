//! Session-scoped theme drafts used by the Storybook workbench.

use gpui::{App, Hsla};
use gpui_component::{Theme, ThemeColor, ThemeTokens};
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, hash::Hasher as _};
use thiserror::Error;

/// One deterministic editable row from [`ThemeColor`].
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ThemeColorRow {
    pub name: String,
    pub color: Hsla,
}

/// Theme draft serialization or field update failure.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
#[error("{0}")]
pub struct ThemeDraftError(String);

/// A session draft layered over the selected base theme.
pub struct ThemeDraft {
    base_theme_name: String,
    base_colors: ThemeColor,
    draft_colors: ThemeColor,
    overrides: BTreeMap<String, Hsla>,
    applied_hash: u64,
}

impl ThemeDraft {
    pub fn new(theme: &Theme) -> Result<Self, ThemeDraftError> {
        let applied_hash = theme_color_hash(&theme.colors)?;
        Ok(Self {
            base_theme_name: theme.theme_name().to_string(),
            base_colors: theme.colors,
            draft_colors: theme.colors,
            overrides: BTreeMap::new(),
            applied_hash,
        })
    }

    pub fn rows(&self) -> Result<Vec<ThemeColorRow>, ThemeDraftError> {
        theme_color_rows(&self.draft_colors)
    }

    pub fn base_theme_name(&self) -> &str {
        &self.base_theme_name
    }

    pub fn set_color(
        &mut self,
        name: &str,
        color: Hsla,
        cx: &mut App,
    ) -> Result<(), ThemeDraftError> {
        let base = theme_color(&self.base_colors, name)?;
        if color == base {
            self.overrides.remove(name);
        } else {
            self.overrides.insert(name.to_owned(), color);
        }
        self.rebuild_and_apply(cx)
    }

    pub fn reset_color(&mut self, name: &str, cx: &mut App) -> Result<(), ThemeDraftError> {
        if theme_color(&self.base_colors, name).is_err() {
            return Err(ThemeDraftError(format!("unknown theme color `{name}`")));
        }
        self.overrides.remove(name);
        self.rebuild_and_apply(cx)
    }

    pub fn reset_all(&mut self, cx: &mut App) -> Result<(), ThemeDraftError> {
        self.overrides.clear();
        self.rebuild_and_apply(cx)
    }

    pub fn export_json(&self) -> Result<String, ThemeDraftError> {
        serde_json::to_string_pretty(&self.draft_colors)
            .map_err(|error| ThemeDraftError(format!("theme export failed: {error}")))
    }

    pub fn import_json(&mut self, json: &str, cx: &mut App) -> Result<(), ThemeDraftError> {
        let imported: ThemeColor = serde_json::from_str(json)
            .map_err(|error| ThemeDraftError(format!("theme import failed: {error}")))?;
        self.overrides = theme_color_rows(&imported)?
            .into_iter()
            .map(|row| (row.name, row.color))
            .collect();
        self.rebuild_and_apply(cx)
    }

    /// Rebase the session draft after a theme selection or external reload.
    ///
    /// Returns `true` when workbench editors should synchronize their values.
    pub fn sync_from_global(&mut self, cx: &mut App) -> Result<bool, ThemeDraftError> {
        let theme = Theme::global(cx);
        let theme_name = theme.theme_name().to_string();
        let colors = theme.colors;
        let global_hash = theme_color_hash(&colors)?;

        if theme_name != self.base_theme_name {
            self.base_theme_name = theme_name;
            self.base_colors = colors;
            self.draft_colors = colors;
            self.overrides.clear();
            self.applied_hash = global_hash;
            return Ok(true);
        }

        if global_hash == self.applied_hash {
            return Ok(false);
        }

        self.base_colors = colors;
        self.rebuild_and_apply(cx)?;
        Ok(true)
    }

    fn rebuild_and_apply(&mut self, cx: &mut App) -> Result<(), ThemeDraftError> {
        self.draft_colors = apply_theme_overrides(self.base_colors, &self.overrides)?;
        let theme = Theme::global_mut(cx);
        theme.colors = self.draft_colors;
        theme.tokens = ThemeTokens::from(&theme.colors);
        self.applied_hash = theme_color_hash(&theme.colors)?;
        cx.refresh_windows();
        Ok(())
    }
}

/// Serialize every upstream color field into deterministic workbench rows.
pub fn theme_color_rows(colors: &ThemeColor) -> Result<Vec<ThemeColorRow>, ThemeDraftError> {
    let value = serde_json::to_value(colors)
        .map_err(|error| ThemeDraftError(format!("theme colors failed to serialize: {error}")))?;
    let serde_json::Value::Object(fields) = value else {
        return Err(ThemeDraftError(
            "theme colors must serialize as an object".to_owned(),
        ));
    };

    fields
        .into_iter()
        .map(|(name, value)| {
            let color = serde_json::from_value(value).map_err(|error| {
                ThemeDraftError(format!(
                    "theme color `{name}` failed to deserialize: {error}"
                ))
            })?;
            Ok(ThemeColorRow { name, color })
        })
        .collect()
}

fn theme_color(colors: &ThemeColor, name: &str) -> Result<Hsla, ThemeDraftError> {
    theme_color_rows(colors)?
        .into_iter()
        .find(|row| row.name == name)
        .map(|row| row.color)
        .ok_or_else(|| ThemeDraftError(format!("unknown theme color `{name}`")))
}

fn apply_theme_overrides(
    base: ThemeColor,
    overrides: &BTreeMap<String, Hsla>,
) -> Result<ThemeColor, ThemeDraftError> {
    let value = serde_json::to_value(base)
        .map_err(|error| ThemeDraftError(format!("base theme failed to serialize: {error}")))?;
    let serde_json::Value::Object(mut fields) = value else {
        return Err(ThemeDraftError(
            "base theme must serialize as an object".to_owned(),
        ));
    };

    for (name, color) in overrides {
        let Some(field) = fields.get_mut(name) else {
            return Err(ThemeDraftError(format!("unknown theme color `{name}`")));
        };
        *field = serde_json::to_value(color).map_err(|error| {
            ThemeDraftError(format!("theme color `{name}` failed to serialize: {error}"))
        })?;
    }

    serde_json::from_value(serde_json::Value::Object(fields))
        .map_err(|error| ThemeDraftError(format!("theme draft failed to deserialize: {error}")))
}

fn theme_color_hash(colors: &ThemeColor) -> Result<u64, ThemeDraftError> {
    let bytes = serde_json::to_vec(colors)
        .map_err(|error| ThemeDraftError(format!("theme colors failed to serialize: {error}")))?;
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    hasher.write(&bytes);
    Ok(hasher.finish())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_serialized_theme_color_has_a_workbench_row() {
        let colors = ThemeColor::default();
        let serialized = serde_json::to_value(colors).expect("theme colors serialize");
        let object = serialized.as_object().expect("theme colors are an object");
        let rows = theme_color_rows(&colors).expect("theme colors become rows");

        assert_eq!(rows.len(), object.len());
        assert_eq!(
            rows.iter().map(|row| row.name.as_str()).collect::<Vec<_>>(),
            object.keys().map(String::as_str).collect::<Vec<_>>()
        );
    }

    #[test]
    fn override_changes_only_the_named_color() {
        let base = ThemeColor::default();
        let rows = theme_color_rows(&base).expect("theme colors become rows");
        let row = rows.first().expect("theme contains colors");
        let replacement = Hsla {
            h: 0.75,
            s: 0.5,
            l: 0.4,
            a: 1.0,
        };
        let overrides = BTreeMap::from([(row.name.clone(), replacement)]);
        let draft = apply_theme_overrides(base, &overrides).expect("override applies");

        let actual = theme_color(&draft, &row.name).expect("overridden color exists");
        assert!((actual.h - replacement.h).abs() < f32::EPSILON);
        assert!((actual.s - replacement.s).abs() < 1e-6);
        assert!((actual.l - replacement.l).abs() < f32::EPSILON);
        assert!((actual.a - replacement.a).abs() < f32::EPSILON);
    }

    #[test]
    fn export_is_deterministic() {
        let theme = Theme::default();
        let draft = ThemeDraft::new(&theme).expect("theme draft initializes");

        assert_eq!(
            draft.export_json().expect("first export succeeds"),
            draft.export_json().expect("second export succeeds")
        );
    }

    #[gpui::test]
    fn same_theme_reload_rebases_and_reapplies_session_overrides(cx: &mut App) {
        gpui_component::init(cx);
        let base_theme = Theme::global(cx).clone();
        let rows = theme_color_rows(&base_theme.colors).expect("base colors serialize");
        let override_row = &rows[0];
        let external_row = &rows[1];
        let override_color = Hsla {
            h: 0.72,
            s: 0.43,
            l: 0.38,
            a: 1.0,
        };
        let external_color = Hsla {
            h: 0.18,
            s: 0.31,
            l: 0.62,
            a: 1.0,
        };
        let mut draft = ThemeDraft::new(&base_theme).expect("draft initializes");
        draft
            .set_color(&override_row.name, override_color, cx)
            .expect("session override applies");

        let external = apply_theme_overrides(
            base_theme.colors,
            &BTreeMap::from([(external_row.name.clone(), external_color)]),
        )
        .expect("external colors build");
        let theme = Theme::global_mut(cx);
        theme.colors = external;
        theme.tokens = ThemeTokens::from(&theme.colors);

        assert!(draft.sync_from_global(cx).expect("reload rebases"));
        let current = draft
            .rows()
            .expect("draft rows remain available")
            .into_iter()
            .map(|row| (row.name, row.color))
            .collect::<BTreeMap<_, _>>();
        assert!((current[&override_row.name].h - override_color.h).abs() < 0.01);
        assert!((current[&external_row.name].h - external_color.h).abs() < 0.01);
    }
}
