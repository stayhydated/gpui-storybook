//! Android JNI dispatch for the SDK's Activity-owned Kotlin adapter.

use crate::EmbeddedRoot;
use crate::device::{DeviceHost, DeviceHostOptions, NativeDispatch, NativeSubmissionError};
use gpui::{App, Entity, Window};
use gpui_storybook_automation::{AutomationCapabilities, StorySnapshot, StorybookAutomationError};

impl<Root: EmbeddedRoot> DeviceHost<Root> {
    /// Attach the existing root to `StorybookAutomation.dispatch` on the Activity.
    /// The Activity exposes `storybookDispatch(String): Boolean`, forwards three
    /// native bridge callbacks to [`Self::native_shell`], and supplies application
    /// route, appearance, and action callbacks to the Kotlin adapter.
    pub fn attach_android(
        root: &Entity<Root>,
        catalog: Vec<StorySnapshot>,
        capabilities: AutomationCapabilities,
        options: DeviceHostOptions,
        window: &Window,
        cx: &mut App,
    ) -> Result<Self, StorybookAutomationError> {
        Self::attach(root, catalog, capabilities, options, dispatch, window, cx)
    }
}

fn dispatch(payload: NativeDispatch) -> Result<(), NativeSubmissionError> {
    let encoded = serde_json::to_string(&payload)
        .map_err(|error| NativeSubmissionError::Rejected(error.to_string()))?;
    let mut submitted = false;
    let result = gpui_mobile::android::jni::with_env(|env| {
        let activity = gpui_mobile::android::jni::activity(env)?;
        let encoded = env.new_string(encoded).map_err(|error| error.to_string())?;
        submitted = true;
        env.call_method(
            &activity,
            jni::jni_str!("storybookDispatch"),
            jni::jni_sig!("(Ljava/lang/String;)Z"),
            &[jni::objects::JValue::Object(&encoded)],
        )
        .and_then(jni::JValueOwned::z)
        .map_err(|error| error.to_string())
    });
    match result {
        Ok(true) => Ok(()),
        Ok(false) => Err(NativeSubmissionError::Rejected(
            "native owner rejected enqueue".to_owned(),
        )),
        Err(message) if submitted => Err(NativeSubmissionError::OutcomeUnknown(message)),
        Err(message) => Err(NativeSubmissionError::Rejected(message)),
    }
}
