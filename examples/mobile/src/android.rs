use gpui_kit::{AppContext as _, Entity, WindowOptions};
use gpui_mobile::android::{host, jni as mobile_jni};
use gpui_storybook_automation::{StoryCanvasBackground, StoryPresentation, StoryViewportPreset};
use gpui_storybook_automation_gpui::{EmbeddedRoot as _, GpuiHostAttachment};
use gpui_storybook_example_embedded::{COUNTER_ROUTE, DemoRoot, NOTES_ROUTE};
use jni::{
    EnvUnowned,
    objects::{JObject, JString},
};
use std::{
    cell::RefCell,
    rc::Rc,
    sync::{Mutex, OnceLock},
    time::Duration,
};

#[derive(Debug, thiserror::Error)]
enum NativeError {
    #[error(transparent)]
    Jni(#[from] jni::errors::Error),
    #[error("{0}")]
    Host(String),
}

#[derive(Clone, Default)]
pub(super) struct ShellState {
    route: String,
    dark: bool,
    revision: u64,
    surface: u64,
    ack: u64,
    applied: bool,
    display_width: i32,
    display_height: i32,
    active: bool,
    x: i32,
    y: i32,
    width: i32,
    height: i32,
    scale: f32,
}
static SHELL: Mutex<ShellState> = Mutex::new(ShellState {
    route: String::new(),
    dark: false,
    revision: 0,
    surface: 0,
    ack: 0,
    applied: true,
    display_width: 0,
    display_height: 0,
    active: false,
    x: 0,
    y: 0,
    width: 0,
    height: 0,
    scale: 1.0,
});
static AUTOMATION: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
static STARTED: OnceLock<()> = OnceLock::new();

// android-activity references this symbol in the pinned runtime's shared
// object. Our manifest chooses the Java SurfaceView host entry point.
#[unsafe(no_mangle)]
fn android_main(_: android_activity::AndroidApp) {
    log::error!("launch Embedded Storybook through MainActivity");
}

struct Owner {
    root: Entity<DemoRoot>,
    attachment: GpuiHostAttachment<DemoRoot>,
    revision: u64,
    surface: u64,
    dark: Option<bool>,
    #[cfg(feature = "automation")]
    automation: Option<super::automation::AutomationHost>,
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
            owner = Some(Rc::new(RefCell::new(Owner {
                attachment: gpui_storybook_example_embedded::attach(&view, window, cx),
                root: view.clone(),
                revision: u64::MAX,
                surface: u64::MAX,
                dark: None,
                #[cfg(feature = "automation")]
                automation: AUTOMATION
                    .load(std::sync::atomic::Ordering::Acquire)
                    .then(super::automation::new),
            })));
            cx.new(|cx| gpui_kit::component::Root::new(view, window, cx))
        })
        .expect("Android platform owns the embedded window");
    let owner = owner.expect("window creates the owner");
    cx.spawn(async move |cx| {
        loop {
            cx.background_executor()
                .timer(Duration::from_millis(16))
                .await;
            let shell = SHELL.lock().expect("shell state").clone();
            let result = window.update(cx, |_, window, cx| {
                let mut owner = owner.borrow_mut();
                if owner.surface != shell.surface {
                    owner.attachment.invalidate(window, cx);
                    owner.attachment =
                        gpui_storybook_example_embedded::attach(&owner.root, window, cx);
                    owner.surface = shell.surface;
                    #[cfg(feature = "automation")]
                    if let Some(automation) = &mut owner.automation {
                        automation.surface_replaced();
                    }
                    owner.revision = u64::MAX;
                }
                if shell.active && owner.revision != shell.revision {
                    let route = if shell.route == NOTES_ROUTE {
                        NOTES_ROUTE
                    } else {
                        COUNTER_ROUTE
                    };
                    if owner.root.read(cx).active_route(cx) != route {
                        owner
                            .attachment
                            .open_story(route, window, cx)
                            .expect("registered shell route");
                    }
                    if owner.dark != Some(shell.dark) {
                        owner
                            .root
                            .update(cx, |root, cx| {
                                root.apply_presentation(
                                    StoryPresentation {
                                        background: if shell.dark {
                                            StoryCanvasBackground::Dark
                                        } else {
                                            StoryCanvasBackground::Light
                                        },
                                        viewport: StoryViewportPreset::Responsive,
                                    },
                                    window,
                                    cx,
                                )
                            })
                            .expect("responsive presentation");
                        owner.dark = Some(shell.dark);
                    }
                    owner.revision = shell.revision;
                    window.refresh();
                }
                #[cfg(feature = "automation")]
                if let Some(mut automation) = owner.automation.take() {
                    automation.poll(&owner.attachment, &shell.snapshot(), window, cx);
                    owner.automation = Some(automation);
                }
            });
            if result.is_err() {
                break;
            }
        }
    })
    .detach();
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
            #[cfg(feature = "automation")]
            super::automation::invalidate_surface();
            host::surface_destroyed();
            let mut shell = SHELL.lock().expect("shell state");
            shell.surface += 1;
            shell.ack = 0;
            shell.active = false;
            Ok(())
        })
        .resolve::<jni::errors::ThrowRuntimeExAndDefault>();
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_storybook_mobile_MainActivity_nativeShell<'a>(
    mut unowned: EnvUnowned<'a>,
    _: JObject<'a>,
    route: JString<'a>,
    dark: u8,
    x: i32,
    y: i32,
    width: i32,
    height: i32,
    scale: f32,
    display_width: i32,
    display_height: i32,
    ack: i64,
) {
    unowned
        .with_env(|_| -> Result<(), NativeError> {
            let mut shell = SHELL.lock().expect("shell state");
            shell.route = route.to_string();
            shell.dark = dark != 0;
            shell.x = x;
            shell.y = y;
            shell.width = width;
            shell.height = height;
            shell.scale = scale;
            shell.display_width = display_width;
            shell.display_height = display_height;
            if ack != 0 {
                shell.ack = ack as u64;
                shell.applied = true;
            }
            shell.revision += 1;
            Ok(())
        })
        .resolve::<jni::errors::ThrowRuntimeExAndDefault>();
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
    #[cfg(feature = "automation")]
    if !super::automation::native_input_allowed() {
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
    #[cfg(feature = "automation")]
    if !super::automation::native_input_allowed() {
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
    #[cfg(feature = "automation")]
    if !super::automation::native_input_allowed() {
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
            SHELL.lock().expect("shell state").active = active != 0;
            if active != 0 {
                host::resumed();
            } else {
                host::paused();
            }
            Ok(())
        })
        .resolve::<jni::errors::ThrowRuntimeExAndDefault>();
}

#[cfg(feature = "automation")]
impl ShellState {
    fn snapshot(&self) -> gpui_storybook_automation_gpui::device::NativeShellSnapshot {
        gpui_storybook_automation_gpui::device::NativeShellSnapshot::builder()
            .route(self.route.clone())
            .dark(self.dark)
            .active(self.active)
            .revision(self.revision)
            .surface(self.surface)
            .ack(self.ack)
            .applied(self.applied)
            .geometry(
                gpui_storybook_automation::wire::SurfaceGeometry::builder()
                    .x(self.x.max(0) as u32)
                    .y(self.y.max(0) as u32)
                    .width(self.width.max(0) as u32)
                    .height(self.height.max(0) as u32)
                    .scale(self.scale)
                    .display_width(self.display_width.max(0) as u32)
                    .display_height(self.display_height.max(0) as u32)
                    .build(),
            )
            .build()
    }
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_storybook_mobile_MainActivity_nativeSelectionCurrent(
    _: EnvUnowned<'_>,
    _: JObject<'_>,
    request: i64,
) -> u8 {
    #[cfg(feature = "automation")]
    {
        let shell = SHELL.lock().expect("shell state");
        u8::from(super::automation::selection_current(
            request as u64,
            shell.surface,
            shell.active,
        ))
    }
    #[cfg(not(feature = "automation"))]
    {
        let _ = request;
        0
    }
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_storybook_mobile_MainActivity_nativeSelectionSettled(
    _: EnvUnowned<'_>,
    _: JObject<'_>,
    request: i64,
    applied: u8,
) {
    #[cfg(feature = "automation")]
    super::automation::selection_settled(request as u64);
    let mut shell = SHELL.lock().expect("shell state");
    shell.ack = request as u64;
    shell.applied = applied != 0;
}
