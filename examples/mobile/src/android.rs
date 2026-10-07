use gpui_kit::{AppContext as _, WindowOptions};
use gpui_mobile::android::{host, jni as mobile_jni};
use gpui_storybook_automation_gpui::device::{DeviceHost, DeviceHostOptions, NativeShellHandle};
use gpui_storybook_example_embedded::DemoRoot;
use jni::{
    EnvUnowned,
    objects::{JObject, JString},
};
use std::sync::OnceLock;

#[derive(Debug, thiserror::Error)]
enum NativeError {
    #[error(transparent)]
    Jni(#[from] jni::errors::Error),
    #[error("{0}")]
    Host(String),
}

static NATIVE: OnceLock<NativeShellHandle> = OnceLock::new();
static AUTOMATION: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
static STARTED: OnceLock<()> = OnceLock::new();

// android-activity references this symbol in the pinned runtime's shared
// object. Our manifest chooses the Java SurfaceView host entry point.
#[unsafe(no_mangle)]
fn android_main(_: android_activity::AndroidApp) {
    log::error!("launch Embedded Storybook through MainActivity");
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_storybook_mobile_MainActivity_nativeStart<'a>(
    mut unowned: EnvUnowned<'a>,
    activity: JObject<'a>,
    automation: u8,
) {
    unowned
        .with_env(|env| -> Result<(), NativeError> {
            android_logger::init_once(
                android_logger::Config::default()
                    .with_max_level(log::LevelFilter::Info)
                    .with_tag("storybook-mobile"),
            );
            mobile_jni::install_panic_hook();
            mobile_jni::set_host_activity(env, &activity).map_err(NativeError::Host)?;
            AUTOMATION.store(automation != 0, std::sync::atomic::Ordering::Release);
            STARTED.get_or_init(|| host::start_with_assets(gpui_kit::assets::Assets, launch));
            Ok(())
        })
        .resolve::<jni::errors::ThrowRuntimeExAndDefault>();
}

fn launch(cx: &mut gpui_kit::App) {
    gpui_kit::init(cx);
    let mut owner = None;
    let window = cx
        .open_window(WindowOptions::default(), |window, cx| {
            let view = cx.new(|cx| DemoRoot::new(window, cx));
            let options = DeviceHostOptions::builder()
                .automation(
                    cfg!(feature = "automation")
                        && AUTOMATION.load(std::sync::atomic::Ordering::Acquire),
                )
                .build();
            let host = DeviceHost::attach_android(
                &view,
                gpui_storybook_example_embedded::catalog(),
                gpui_storybook_example_embedded::capabilities(),
                options,
                window,
                cx,
            )
            .expect("attach the existing Android root");
            let _ = NATIVE.set(host.native_shell());
            owner = Some(host);
            cx.new(|cx| gpui_kit::component::Root::new(view, window, cx))
        })
        .expect("Android platform owns the embedded window");
    owner.expect("window creates the owner").run(window, cx);
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_storybook_mobile_MainActivity_nativeSurface<'a>(
    mut unowned: EnvUnowned<'a>,
    _: JObject<'a>,
    surface: JObject<'a>,
    scale: f32,
) {
    unowned
        .with_env(|env| -> Result<(), NativeError> {
            // SAFETY: the JVM supplies a live Surface for this callback. The NDK
            // wrapper acquires a reference retained by the mobile render thread.
            let window = unsafe {
                ndk::native_window::NativeWindow::from_surface(
                    env.get_raw().cast(),
                    surface.as_raw().cast(),
                )
            }
            .ok_or_else(|| NativeError::Host("Surface has no native window".to_owned()))?;
            host::surface_created(window, scale);
            Ok(())
        })
        .resolve::<jni::errors::ThrowRuntimeExAndDefault>();
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_storybook_mobile_MainActivity_nativeRelease<'a>(
    mut unowned: EnvUnowned<'a>,
    _: JObject<'a>,
) {
    unowned
        .with_env(|_| -> Result<(), NativeError> {
            host::surface_destroyed();
            Ok(())
        })
        .resolve::<jni::errors::ThrowRuntimeExAndDefault>();
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_storybook_mobile_MainActivity_nativeAutomationEvent<'a>(
    mut unowned: EnvUnowned<'a>,
    _: JObject<'a>,
    event: JString<'a>,
) -> u8 {
    unowned
        .with_env(|_| -> Result<u8, NativeError> {
            let accepted = NATIVE.get().is_some_and(|shell| {
                shell
                    .publish_json(&event.to_string())
                    .inspect_err(|error| log::warn!("native automation event: {error}"))
                    .is_ok()
            });
            Ok(u8::from(accepted))
        })
        .resolve::<jni::errors::ThrowRuntimeExAndDefault>()
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_storybook_mobile_MainActivity_nativeTouch<'a>(
    mut unowned: EnvUnowned<'a>,
    _: JObject<'a>,
    action: i32,
    id: i32,
    x: f32,
    y: f32,
) {
    if !NATIVE.get().is_none_or(NativeShellHandle::is_input_allowed) {
        return;
    }

    unowned
        .with_env(|_| -> Result<(), NativeError> {
            host::motion_event(action as u32, 0, &[host::Pointer { id, x, y }]);
            Ok(())
        })
        .resolve::<jni::errors::ThrowRuntimeExAndDefault>();
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_storybook_mobile_MainActivity_nativeKey<'a>(
    mut unowned: EnvUnowned<'a>,
    _: JObject<'a>,
    code: i32,
    action: i32,
    meta: i32,
) {
    if !NATIVE.get().is_none_or(NativeShellHandle::is_input_allowed) {
        return;
    }

    unowned
        .with_env(|_| -> Result<(), NativeError> {
            host::key(code, action, meta);
            Ok(())
        })
        .resolve::<jni::errors::ThrowRuntimeExAndDefault>();
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_storybook_mobile_MainActivity_nativeIme<'a>(
    mut unowned: EnvUnowned<'a>,
    _: JObject<'a>,
    session: i64,
    kind: i32,
    text: JString<'a>,
    start: i32,
    end: i32,
) {
    if !NATIVE.get().is_none_or(NativeShellHandle::is_input_allowed) {
        return;
    }

    unowned
        .with_env(|_| -> Result<(), NativeError> {
            host::ime_event(
                session as u64,
                kind,
                text.to_string(),
                start.max(0) as usize,
                end.max(0) as usize,
            );
            Ok(())
        })
        .resolve::<jni::errors::ThrowRuntimeExAndDefault>();
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_storybook_mobile_MainActivity_nativeActive<'a>(
    mut unowned: EnvUnowned<'a>,
    _: JObject<'a>,
    active: u8,
) {
    unowned
        .with_env(|_| -> Result<(), NativeError> {
            if active != 0 {
                host::resumed();
            } else {
                host::paused();
            }
            Ok(())
        })
        .resolve::<jni::errors::ThrowRuntimeExAndDefault>();
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_storybook_mobile_MainActivity_nativeSelectionCurrent(
    _: EnvUnowned<'_>,
    _: JObject<'_>,
    request: i64,
) -> u8 {
    u8::from(
        NATIVE
            .get()
            .is_some_and(|shell| shell.is_current(request as u64)),
    )
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_storybook_mobile_MainActivity_nativeInputAllowed(
    _: EnvUnowned<'_>,
    _: JObject<'_>,
) -> u8 {
    u8::from(NATIVE.get().is_none_or(NativeShellHandle::is_input_allowed))
}
