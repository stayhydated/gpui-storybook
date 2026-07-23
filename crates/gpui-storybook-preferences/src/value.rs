use std::{borrow::Cow, fmt, str::FromStr};

use schemars::{JsonSchema, Schema, SchemaGenerator, json_schema};
use serde::{Deserialize, Deserializer, Serialize, Serializer, de::Error as _};
use strum::{EnumString, IntoStaticStr};
use unic_langid::{LanguageIdentifier, LanguageIdentifierError};

/// Maximum byte length accepted for a stable Storybook consumer identifier.
pub const MAX_CONSUMER_ID_LEN: usize = 128;

/// Maximum byte length accepted for a theme registry identifier.
pub const MAX_THEME_ID_LEN: usize = 256;

/// Maximum byte length accepted for a BCP 47 language tag.
pub const MAX_LANGUAGE_TAG_LEN: usize = 128;

/// Stable identifier that isolates one Storybook consumer's preference document.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ConsumerId(String);

impl ConsumerId {
    /// Validates a stable consumer identifier.
    ///
    /// Identifiers use lowercase ASCII letters, digits, `.`, `-`, and `_`.
    /// They must begin and end with an ASCII letter or digit so the same value
    /// can safely be used as a JSON value and path component.
    ///
    /// # Errors
    ///
    /// Returns [`ConsumerIdError`] when the identifier is empty, too long, or
    /// contains an unsupported character or boundary.
    pub fn new(value: impl Into<String>) -> Result<Self, ConsumerIdError> {
        let value = value.into();
        if value.is_empty() {
            return Err(ConsumerIdError::Empty);
        }
        if value.len() > MAX_CONSUMER_ID_LEN {
            return Err(ConsumerIdError::TooLong {
                max: MAX_CONSUMER_ID_LEN,
            });
        }

        let is_alphanumeric =
            |character: u8| character.is_ascii_lowercase() || character.is_ascii_digit();
        let bytes = value.as_bytes();
        if !is_alphanumeric(bytes[0]) {
            return Err(ConsumerIdError::InvalidStart);
        }
        if !is_alphanumeric(bytes[bytes.len() - 1]) {
            return Err(ConsumerIdError::InvalidEnd);
        }
        if let Some((index, _)) = bytes.iter().enumerate().find(|(_, character)| {
            !is_alphanumeric(**character) && !matches!(**character, b'.' | b'-' | b'_')
        }) {
            return Err(ConsumerIdError::InvalidCharacter { index });
        }

        Ok(Self(value))
    }

    /// Returns the stable JSON and path token.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl AsRef<str> for ConsumerId {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl fmt::Display for ConsumerId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl FromStr for ConsumerId {
    type Err = ConsumerIdError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::new(value)
    }
}

impl Serialize for ConsumerId {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(self.as_str())
    }
}

impl<'de> Deserialize<'de> for ConsumerId {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        Self::new(String::deserialize(deserializer)?).map_err(D::Error::custom)
    }
}

impl JsonSchema for ConsumerId {
    fn schema_name() -> Cow<'static, str> {
        "ConsumerId".into()
    }

    fn schema_id() -> Cow<'static, str> {
        concat!(module_path!(), "::ConsumerId").into()
    }

    fn json_schema(_: &mut SchemaGenerator) -> Schema {
        json_schema!({
            "description": "Stable identifier for the consuming Storybook binary. Uses lowercase ASCII letters, digits, '.', '-', and '_', and begins and ends with a letter or digit.",
            "type": "string",
            "minLength": 1,
            "maxLength": MAX_CONSUMER_ID_LEN,
            "pattern": "^[a-z0-9](?:[a-z0-9._-]*[a-z0-9])?$",
            "examples": ["my-app.storybook"]
        })
    }
}

/// Failure to validate a stable Storybook consumer identifier.
#[derive(Clone, Copy, Debug, Eq, thiserror::Error, PartialEq)]
pub enum ConsumerIdError {
    /// The identifier was empty.
    #[error("Storybook consumer id must not be empty")]
    Empty,
    /// The identifier exceeded the stable storage bound.
    #[error("Storybook consumer id exceeds {max} bytes")]
    TooLong {
        /// Maximum supported byte length.
        max: usize,
    },
    /// The identifier did not start with a lowercase ASCII letter or digit.
    #[error("Storybook consumer id must start with a lowercase ASCII letter or digit")]
    InvalidStart,
    /// The identifier did not end with a lowercase ASCII letter or digit.
    #[error("Storybook consumer id must end with a lowercase ASCII letter or digit")]
    InvalidEnd,
    /// The identifier contained an unsupported character.
    #[error("Storybook consumer id contains an unsupported character at byte {index}")]
    InvalidCharacter {
        /// Byte index of the unsupported character.
        index: usize,
    },
}

/// Normalized theme registry identifier.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ThemeId(String);

impl ThemeId {
    /// Trims and validates a theme registry identifier.
    ///
    /// # Errors
    ///
    /// Returns [`ThemeIdError`] for an empty, overlong, or control-character
    /// value.
    pub fn new(value: impl AsRef<str>) -> Result<Self, ThemeIdError> {
        let normalized = value.as_ref().trim();
        if normalized.is_empty() {
            return Err(ThemeIdError::Empty);
        }
        if normalized.len() > MAX_THEME_ID_LEN {
            return Err(ThemeIdError::TooLong {
                max: MAX_THEME_ID_LEN,
            });
        }
        if normalized.chars().any(char::is_control) {
            return Err(ThemeIdError::ControlCharacter);
        }
        Ok(Self(normalized.to_owned()))
    }

    /// Returns the normalized registry identifier.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl AsRef<str> for ThemeId {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl fmt::Display for ThemeId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl FromStr for ThemeId {
    type Err = ThemeIdError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::new(value)
    }
}

impl Serialize for ThemeId {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(self.as_str())
    }
}

impl<'de> Deserialize<'de> for ThemeId {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        Self::new(String::deserialize(deserializer)?).map_err(D::Error::custom)
    }
}

impl JsonSchema for ThemeId {
    fn schema_name() -> Cow<'static, str> {
        "ThemeId".into()
    }

    fn schema_id() -> Cow<'static, str> {
        concat!(module_path!(), "::ThemeId").into()
    }

    fn json_schema(_: &mut SchemaGenerator) -> Schema {
        json_schema!({
            "description": "Identifier of a theme registered by the consuming application. Leading and trailing whitespace is removed, and control characters are invalid.",
            "type": "string",
            "minLength": 1,
            "maxLength": MAX_THEME_ID_LEN,
            "pattern": r"^[^\u0000-\u001F\u007F-\u009F]+$",
            "examples": ["Default Light"]
        })
    }
}

/// Failure to validate a theme registry identifier.
#[derive(Clone, Copy, Debug, Eq, thiserror::Error, PartialEq)]
pub enum ThemeIdError {
    /// The normalized identifier was empty.
    #[error("theme id must not be empty")]
    Empty,
    /// The identifier exceeded the storage bound.
    #[error("theme id exceeds {max} bytes")]
    TooLong {
        /// Maximum supported byte length.
        max: usize,
    },
    /// The identifier contained a control character.
    #[error("theme id must not contain control characters")]
    ControlCharacter,
}

/// Validated and canonically formatted BCP 47 language tag.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct LanguageTag(LanguageIdentifier);

impl LanguageTag {
    /// Parses and normalizes a BCP 47 language tag.
    ///
    /// # Errors
    ///
    /// Returns [`LanguageTagError`] for an empty, overlong, or invalid tag.
    pub fn new(value: impl AsRef<str>) -> Result<Self, LanguageTagError> {
        let normalized = value.as_ref().trim();
        if normalized.is_empty() {
            return Err(LanguageTagError::Empty);
        }
        if normalized.len() > MAX_LANGUAGE_TAG_LEN {
            return Err(LanguageTagError::TooLong {
                max: MAX_LANGUAGE_TAG_LEN,
            });
        }
        let identifier = normalized
            .parse::<LanguageIdentifier>()
            .map_err(|source| LanguageTagError::Invalid { source })?;
        Ok(Self(identifier))
    }

    /// Returns the normalized Fluent language identifier.
    pub fn as_identifier(&self) -> &LanguageIdentifier {
        &self.0
    }
}

impl AsRef<LanguageIdentifier> for LanguageTag {
    fn as_ref(&self) -> &LanguageIdentifier {
        self.as_identifier()
    }
}

impl fmt::Display for LanguageTag {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(formatter)
    }
}

impl FromStr for LanguageTag {
    type Err = LanguageTagError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::new(value)
    }
}

impl Serialize for LanguageTag {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

impl<'de> Deserialize<'de> for LanguageTag {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        Self::new(String::deserialize(deserializer)?).map_err(D::Error::custom)
    }
}

impl JsonSchema for LanguageTag {
    fn schema_name() -> Cow<'static, str> {
        "LanguageTag".into()
    }

    fn schema_id() -> Cow<'static, str> {
        concat!(module_path!(), "::LanguageTag").into()
    }

    fn json_schema(_: &mut SchemaGenerator) -> Schema {
        json_schema!({
            "description": "Syntactically valid BCP 47 language tag normalized to canonical casing.",
            "type": "string",
            "minLength": 1,
            "maxLength": MAX_LANGUAGE_TAG_LEN,
            "format": "language-tag",
            "examples": ["en", "en-US", "zh-Hant"]
        })
    }
}

/// Failure to parse a normalized BCP 47 language tag.
#[derive(Debug, thiserror::Error)]
pub enum LanguageTagError {
    /// The normalized tag was empty.
    #[error("language tag must not be empty")]
    Empty,
    /// The tag exceeded the storage bound.
    #[error("language tag exceeds {max} bytes")]
    TooLong {
        /// Maximum supported byte length.
        max: usize,
    },
    /// The tag did not parse as a language identifier.
    #[error("invalid BCP 47 language tag")]
    Invalid {
        /// Parser failure retained for diagnostics.
        #[source]
        source: LanguageIdentifierError,
    },
}

/// Saved appearance intent.
#[derive(
    Clone,
    Copy,
    Debug,
    Default,
    Deserialize,
    EnumString,
    Eq,
    IntoStaticStr,
    JsonSchema,
    PartialEq,
    Serialize,
)]
#[serde(rename_all = "snake_case")]
#[strum(const_into_str, serialize_all = "snake_case")]
pub enum PreferredColorScheme {
    /// Follow the detected device appearance.
    #[default]
    System,
    /// Always render a light theme.
    Light,
    /// Always render a dark theme.
    Dark,
}

impl PreferredColorScheme {
    /// Returns the stable persisted token.
    pub const fn token(self) -> &'static str {
        self.into_str()
    }
}

/// Stable persisted language-mode token.
#[derive(
    Clone,
    Copy,
    Debug,
    Default,
    Deserialize,
    EnumString,
    Eq,
    IntoStaticStr,
    JsonSchema,
    PartialEq,
    Serialize,
)]
#[serde(rename_all = "snake_case")]
#[strum(const_into_str, serialize_all = "snake_case")]
pub enum PreferredLanguageMode {
    /// Negotiate from ordered device locales.
    #[default]
    System,
    /// Resolve one explicit saved language tag.
    Explicit,
}

impl PreferredLanguageMode {
    /// Returns the stable persisted token.
    pub const fn token(self) -> &'static str {
        self.into_str()
    }
}

/// Saved language intent.
#[derive(Clone, Debug, Default, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(tag = "mode", content = "tag", rename_all = "snake_case")]
pub enum PreferredLanguage {
    /// Negotiate from ordered device locales.
    #[default]
    System,
    /// Use a validated explicit language tag when it remains supported.
    Explicit(
        /// BCP 47 language tag to use instead of device locale negotiation.
        LanguageTag,
    ),
}

impl PreferredLanguage {
    /// Returns the persisted mode token.
    pub const fn mode(&self) -> PreferredLanguageMode {
        match self {
            Self::System => PreferredLanguageMode::System,
            Self::Explicit(_) => PreferredLanguageMode::Explicit,
        }
    }

    /// Returns the explicit tag when this intent is explicit.
    pub fn explicit_tag(&self) -> Option<&LanguageTag> {
        match self {
            Self::System => None,
            Self::Explicit(tag) => Some(tag),
        }
    }
}

/// Saved scrollbar visibility policy.
#[derive(
    Clone,
    Copy,
    Debug,
    Default,
    Deserialize,
    EnumString,
    Eq,
    IntoStaticStr,
    JsonSchema,
    PartialEq,
    Serialize,
)]
#[serde(rename_all = "snake_case")]
#[strum(const_into_str, serialize_all = "snake_case")]
pub enum PreferredScrollbar {
    /// Show while scrolling and fade after inactivity.
    #[default]
    Scrolling,
    /// Show while the pointer hovers the scroll area.
    Hover,
    /// Always show scrollbars.
    Always,
}

impl PreferredScrollbar {
    /// Returns the stable persisted token.
    pub const fn token(self) -> &'static str {
        self.into_str()
    }
}

/// One typed Storybook preference aggregate.
#[derive(Clone, Debug, Default, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct StorybookPreferences {
    /// Saved appearance intent.
    pub color_scheme: PreferredColorScheme,
    /// Saved theme registry identifier for light appearance.
    pub light_theme: Option<ThemeId>,
    /// Saved theme registry identifier for dark appearance.
    pub dark_theme: Option<ThemeId>,
    /// Saved device-negotiated or explicit language intent.
    pub language: PreferredLanguage,
    /// Saved scrollbar policy.
    pub scrollbar: PreferredScrollbar,
}

/// Stored preference aggregate.
#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PreferenceRecord {
    /// Typed saved intent.
    pub preferences: StorybookPreferences,
}
