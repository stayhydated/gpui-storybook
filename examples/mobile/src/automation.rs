//! Android queue adapter for the reusable device-to-GPUI coordinator.
use gpui_storybook_automation::wire::{HostAction, HostActionDescriptor};
use gpui_storybook_automation_gpui::device::{
    DeviceCoordinator, NativeSelection, NativeSubmissionError,
};
use gpui_storybook_example_embedded::DemoRoot;
use gpui_storybook_mobile::{DEVICE_PORT, DeviceEndpoint, OperationGate};
use std::{
    sync::{Mutex, OnceLock},
    time::{SystemTime, UNIX_EPOCH},
};

pub(super) type AutomationHost = DeviceCoordinator<DemoRoot>;
static NATIVE_GATE: OnceLock<OperationGate> = OnceLock::new();
static SELECTION: Mutex<Option<NativeSelection>> = Mutex::new(None);

pub(super) fn new() -> AutomationHost {
    let session = format!(
        "{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos()
    );
    let endpoint =
        DeviceEndpoint::listen(DEVICE_PORT, session).expect("opted-in loopback endpoint");
    let _ = NATIVE_GATE.set(endpoint.gate());
    DeviceCoordinator::new(endpoint, select_native)
}

pub(super) fn native_input_allowed() -> bool {
    NATIVE_GATE.get().is_none_or(|gate| !gate.busy())
}
pub(super) fn invalidate_surface() {
    if let Some(gate) = NATIVE_GATE.get() {
        gate.suspend();
    }
}
/// Called on Android's UI thread immediately before its selection callback.
/// Surface release and route dispatch are serialized on this same thread.
pub(super) fn selection_current(request: u64, surface: u64, active: bool) -> bool {
    SELECTION
        .lock()
        .expect("native selection")
        .as_ref()
        .is_some_and(|selection| {
            selection.request_id() == request
                && selection.surface() == surface
                && active
                && selection.permit().is_current()
        })
}
pub(super) fn selection_settled(request: u64) {
    let mut queued = SELECTION.lock().expect("native selection");
    if queued
        .as_ref()
        .is_some_and(|selection| selection.request_id() == request)
    {
        queued.take();
    }
}
fn select_native(selection: NativeSelection) -> Result<(), NativeSubmissionError> {
    let action = match selection.action() {
        Some(HostAction::Invoke { name, arguments })
            if matches!(name.as_str(), "compose.increment" | "compose.reset")
                && arguments
                    .as_object()
                    .is_some_and(|fields| fields.is_empty()) =>
        {
            name.clone()
        },
        None => String::new(),
        Some(_) => {
            return Err(NativeSubmissionError::Rejected(
                "invalid Compose action arguments".to_owned(),
            ));
        },
    };
    let route = selection.route().to_owned();
    let dark = selection.dark();
    let request = selection.request_id();
    *SELECTION.lock().expect("native selection") = Some(selection);
    let mut submitted = false;
    let result = gpui_mobile::android::jni::with_env(|env| {
        let activity = gpui_mobile::android::jni::activity(env)?;
        let route = env.new_string(route).map_err(|error| error.to_string())?;
        let action = env.new_string(action).map_err(|error| error.to_string())?;
        submitted = true;
        env.call_method(
            &activity,
            jni::jni_str!("gpuiSelect"),
            jni::jni_sig!("(Ljava/lang/String;ZJLjava/lang/String;)V"),
            &[
                jni::objects::JValue::Object(&route),
                jni::objects::JValue::Bool(dark),
                jni::objects::JValue::Long(request as i64),
                jni::objects::JValue::Object(&action),
            ],
        )
        .map_err(|error| error.to_string())?;
        Ok(())
    });
    match result {
        Ok(()) => Ok(()),
        Err(message) if submitted => Err(NativeSubmissionError::OutcomeUnknown(message)),
        Err(message) => {
            selection_settled(request);
            Err(NativeSubmissionError::Rejected(message))
        },
    }
}

pub(super) fn native_actions() -> Vec<HostActionDescriptor> {
    [("compose.increment", "Increment the Compose counter"), ("compose.reset", "Reset the Compose counter")]
        .into_iter().map(|(name, description)| HostActionDescriptor::builder()
            .name(name.to_owned()).description(description.to_owned())
            .input_schema(serde_json::json!({"type":"object", "required":["action","name","arguments"],
                "properties":{"action":{"const":"invoke"}, "name":{"const":name},
                "arguments":{"type":"object","additionalProperties":false}}, "additionalProperties":false}))
            .build()).collect()
}
