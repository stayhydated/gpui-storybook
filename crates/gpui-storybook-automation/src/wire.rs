//! Versioned, bounded device transport. Requests are never replayable messages.
//!
//! Frames contain a big-endian u32 length followed by UTF-8 JSON. A connection
//! negotiates with `GetHost` using an empty session; every later request names
//! that exact session. Recreated surfaces require a new connection and catalog.

use crate::*;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::{
    io::{self, Read, Write},
    path::PathBuf,
};

pub const PROTOCOL_VERSION: u32 = 1;
pub const MAX_WIRE_BYTES: usize = 1024 * 1024;

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, bon::Builder)]
#[serde(deny_unknown_fields)]
pub struct SurfaceGeometry {
    x: u32,
    y: u32,
    width: u32,
    height: u32,
    scale: f32,
    display_width: u32,
    display_height: u32,
}
impl SurfaceGeometry {
    pub fn x(&self) -> u32 {
        self.x
    }
    pub fn y(&self) -> u32 {
        self.y
    }
    pub fn width(&self) -> u32 {
        self.width
    }
    pub fn height(&self) -> u32 {
        self.height
    }
    pub fn scale(&self) -> f32 {
        self.scale
    }
    pub fn display_width(&self) -> u32 {
        self.display_width
    }
    pub fn display_height(&self) -> u32 {
        self.display_height
    }
    pub fn validate(&self) -> bool {
        self.width > 0
            && self.height > 0
            && self.scale.is_finite()
            && self.scale > 0.0
            && self
                .x
                .checked_add(self.width)
                .is_some_and(|right| right <= self.display_width)
            && self
                .y
                .checked_add(self.height)
                .is_some_and(|bottom| bottom <= self.display_height)
    }
}

/// Observed display orientation; device tools own orientation changes.
#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DisplayOrientation {
    Portrait,
    Landscape,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, bon::Builder)]
#[serde(deny_unknown_fields)]
pub struct HostDescriptor {
    protocol_version: u32,
    session: String,
    surface_revision: u64,
    route_revision: u64,
    active_route: String,
    native_route: String,
    dark: bool,
    orientation: DisplayOrientation,
    geometry: SurfaceGeometry,
    capabilities: AutomationCapabilities,
}
impl HostDescriptor {
    pub fn protocol_version(&self) -> u32 {
        self.protocol_version
    }
    pub fn session(&self) -> &str {
        &self.session
    }
    pub fn surface_revision(&self) -> u64 {
        self.surface_revision
    }
    pub fn route_revision(&self) -> u64 {
        self.route_revision
    }
    pub fn active_route(&self) -> &str {
        &self.active_route
    }
    pub fn native_route(&self) -> &str {
        &self.native_route
    }
    pub fn dark(&self) -> bool {
        self.dark
    }
    pub fn orientation(&self) -> DisplayOrientation {
        self.orientation
    }
    pub fn geometry(&self) -> &SurfaceGeometry {
        &self.geometry
    }
    pub fn capabilities(&self) -> &AutomationCapabilities {
        &self.capabilities
    }
    pub fn agrees(&self) -> bool {
        let observed = if self.geometry.display_width > self.geometry.display_height {
            DisplayOrientation::Landscape
        } else {
            DisplayOrientation::Portrait
        };
        self.active_route == self.native_route
            && self.geometry.validate()
            && self.orientation == observed
    }
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize)]
#[serde(tag = "operation", rename_all = "snake_case", deny_unknown_fields)]
pub enum DeviceOperation {
    GetHost {},
    ListStories {},
    GetStory {
        key: String,
    },
    CurrentStory {},
    OpenStory {
        key: String,
    },
    ReadControls {},
    SetControl {
        key: String,
        value: ControlValue,
    },
    ResetControl {
        key: Option<String>,
    },
    ListActions {},
    ListTargets {},
    ReadValues {},
    RunSteps {
        request: StoryInteractionRequest,
    },
    RunScenario {
        story_key: Option<String>,
        scenario_key: String,
    },
    ListHostActions {},
    DispatchHostAction {
        action: HostAction,
    },
    /// Retain the device lease through external compositor observation.
    PrepareCapture {},
    /// Validate the observation interval and release its device lease.
    FinishCapture {
        ticket: u64,
    },
}
impl DeviceOperation {
    pub fn mutates(&self) -> bool {
        matches!(
            self,
            Self::OpenStory { .. }
                | Self::SetControl { .. }
                | Self::ResetControl { .. }
                | Self::RunSteps { .. }
                | Self::RunScenario { .. }
                | Self::DispatchHostAction { .. }
                | Self::PrepareCapture {}
        )
    }
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(tag = "action", rename_all = "snake_case", deny_unknown_fields)]
pub enum HostAction {
    SetAppearance { dark: bool },
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, bon::Builder)]
#[serde(deny_unknown_fields)]
pub struct HostActionDescriptor {
    name: String,
    description: String,
    input_schema: serde_json::Value,
}
impl HostActionDescriptor {
    pub fn name(&self) -> &str {
        &self.name
    }
    pub fn description(&self) -> &str {
        &self.description
    }
    pub fn input_schema(&self) -> &serde_json::Value {
        &self.input_schema
    }
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize)]
#[serde(
    tag = "result",
    content = "value",
    rename_all = "snake_case",
    deny_unknown_fields
)]
pub enum DeviceResult {
    Host(HostDescriptor),
    Stories(Vec<StorySnapshot>),
    Story(StorySnapshot),
    Current(StoryCurrentSnapshot),
    Controls(StoryControlsSnapshot),
    Actions(Vec<StoryActionSnapshot>),
    Targets(StoryInteractionTargetsSnapshot),
    Values(StorySemanticValuesSnapshot),
    Interaction(StoryInteractionSnapshot),
    Scenario(StoryScenarioRunSnapshot),
    HostActions(Vec<HostActionDescriptor>),
    CaptureTicket { ticket: u64, host: HostDescriptor },
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, bon::Builder)]
#[serde(deny_unknown_fields)]
pub struct DeviceRequest {
    protocol_version: u32,
    session: String,
    request_id: u64,
    command: DeviceOperation,
}
impl DeviceRequest {
    pub fn protocol_version(&self) -> u32 {
        self.protocol_version
    }
    pub fn session(&self) -> &str {
        &self.session
    }
    pub fn request_id(&self) -> u64 {
        self.request_id
    }
    pub fn command(&self) -> &DeviceOperation {
        &self.command
    }
    pub fn into_command(self) -> DeviceOperation {
        self.command
    }
    pub fn validate_identity(&self, current_session: &str) -> Result<(), StorybookAutomationError> {
        if self.protocol_version != PROTOCOL_VERSION {
            return Err(StorybookAutomationError::ProtocolMismatch {
                expected: PROTOCOL_VERSION,
                actual: self.protocol_version,
            });
        }
        if self.request_id == 0
            || self.session.len() > 128
            || (self.session != current_session
                && !(self.session.is_empty()
                    && matches!(self.command, DeviceOperation::GetHost {})))
        {
            return Err(StorybookAutomationError::StaleHost {
                message: "request names an invalid or replaced session".to_owned(),
            });
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, bon::Builder)]
#[serde(deny_unknown_fields)]
pub struct DeviceResponse {
    protocol_version: u32,
    session: String,
    request_id: u64,
    outcome: Result<DeviceResult, StorybookAutomationError>,
}
impl DeviceResponse {
    pub fn protocol_version(&self) -> u32 {
        self.protocol_version
    }
    pub fn session(&self) -> &str {
        &self.session
    }
    pub fn request_id(&self) -> u64 {
        self.request_id
    }
    pub fn into_outcome(self) -> Result<DeviceResult, StorybookAutomationError> {
        self.outcome
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum HostCaptureScope {
    Display,
    Gpui,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, bon::Builder)]
#[serde(deny_unknown_fields)]
pub struct HostCaptureSnapshot {
    request_id: u64,
    scope: HostCaptureScope,
    path: PathBuf,
    pixel_width: u32,
    pixel_height: u32,
    host: HostDescriptor,
    provider: String,
}
impl HostCaptureSnapshot {
    pub fn scope(&self) -> HostCaptureScope {
        self.scope
    }
    pub fn provider(&self) -> &str {
        &self.provider
    }
    pub fn path(&self) -> &std::path::Path {
        &self.path
    }
    pub fn host(&self) -> &HostDescriptor {
        &self.host
    }
    pub fn request_id(&self) -> u64 {
        self.request_id
    }
    pub fn pixel_width(&self) -> u32 {
        self.pixel_width
    }
    pub fn pixel_height(&self) -> u32 {
        self.pixel_height
    }
}

pub fn read_frame<T: serde::de::DeserializeOwned>(reader: &mut impl Read) -> io::Result<T> {
    let mut prefix = [0u8; 4];
    reader.read_exact(&mut prefix)?;
    let length = u32::from_be_bytes(prefix) as usize;
    if length == 0 || length > MAX_WIRE_BYTES {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "wire frame exceeds bound",
        ));
    }
    let mut buffer = vec![0; length];
    reader.read_exact(&mut buffer)?;
    serde_json::from_slice(&buffer)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))
}
pub fn write_frame<T: Serialize>(writer: &mut impl Write, value: &T) -> io::Result<()> {
    let buffer = serde_json::to_vec(value).map_err(io::Error::other)?;
    if buffer.is_empty() || buffer.len() > MAX_WIRE_BYTES {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "wire frame exceeds bound",
        ));
    }
    writer.write_all(&(buffer.len() as u32).to_be_bytes())?;
    writer.write_all(&buffer)?;
    writer.flush()
}
