//! Process launcher for GPUI Storybook automation sessions.
//!
//! Linux sessions receive a private Sway compositor configured with wlroots'
//! headless backend and the Pixman software renderer. Other targets execute the
//! child directly.

use std::{
    ffi::{OsStr, OsString},
    io,
    process::{Command, ExitStatus},
};

/// Environment variable that overrides the Sway executable.
pub const SWAY_ENV_VAR: &str = "GPUI_STORYBOOK_SWAY";

/// One child command to execute through the platform launcher.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LaunchCommand {
    program: OsString,
    args: Vec<OsString>,
}

impl LaunchCommand {
    /// Construct a command and its arguments.
    pub fn new(
        program: impl Into<OsString>,
        args: impl IntoIterator<Item = impl Into<OsString>>,
    ) -> Self {
        Self {
            program: program.into(),
            args: args.into_iter().map(Into::into).collect(),
        }
    }

    /// Child executable.
    pub fn program(&self) -> &OsStr {
        &self.program
    }

    /// Child arguments.
    pub fn args(&self) -> &[OsString] {
        &self.args
    }
}

/// Platform launcher options.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct LaunchOptions {
    /// Explicit Sway executable. The environment override and then `sway` are
    /// used when omitted.
    pub sway: Option<OsString>,
}

/// Run one child command and return its exit status.
pub fn run(command: &LaunchCommand, options: &LaunchOptions) -> io::Result<ExitStatus> {
    platform::run(command, options)
}

#[cfg(not(target_os = "linux"))]
mod platform {
    use super::*;

    pub(super) fn run(command: &LaunchCommand, _options: &LaunchOptions) -> io::Result<ExitStatus> {
        Command::new(command.program())
            .args(command.args())
            .status()
    }
}

#[cfg(target_os = "linux")]
mod platform {
    use super::*;
    use std::{
        fs::{self, File},
        os::unix::fs::{FileTypeExt as _, PermissionsExt as _},
        path::{Path, PathBuf},
        process::{Child, Stdio},
        thread,
        time::{Duration, Instant},
    };
    use tempfile::TempDir;

    const READY_TIMEOUT: Duration = Duration::from_secs(5);
    const READY_POLL_INTERVAL: Duration = Duration::from_millis(50);
    const SWAY_CONFIG: &str = "output * mode 1920x1200\nseat seat0 fallback true\nfor_window [app_id=\".*\"] floating enable\n";

    struct HeadlessSession {
        sway: Child,
        runtime: TempDir,
    }

    impl Drop for HeadlessSession {
        fn drop(&mut self) {
            let _ = self.sway.kill();
            let _ = self.sway.wait();
        }
    }

    pub(super) fn run(command: &LaunchCommand, options: &LaunchOptions) -> io::Result<ExitStatus> {
        let mut session = start_sway(options)?;
        let display = wait_for_wayland_socket(&mut session)?;
        let status = Command::new(command.program())
            .args(command.args())
            .env("XDG_RUNTIME_DIR", session.runtime.path())
            .env("WAYLAND_DISPLAY", display)
            .env("LIBGL_ALWAYS_SOFTWARE", "1")
            .env_remove("DISPLAY")
            .env_remove("I3SOCK")
            .env_remove("SWAYSOCK")
            .env_remove("WAYLAND_SOCKET")
            .env_remove("ZED_HEADLESS")
            .status();
        drop(session);
        status
    }

    fn start_sway(options: &LaunchOptions) -> io::Result<HeadlessSession> {
        let runtime = tempfile::Builder::new()
            .prefix("gpui-storybook-")
            .tempdir()?;
        fs::set_permissions(runtime.path(), fs::Permissions::from_mode(0o700))?;
        let config_path = runtime.path().join("sway.conf");
        fs::write(&config_path, SWAY_CONFIG)?;
        let log_path = runtime.path().join("sway.log");
        let log = File::create(&log_path)?;
        let sway = options
            .sway
            .clone()
            .or_else(|| std::env::var_os(SWAY_ENV_VAR))
            .unwrap_or_else(|| OsString::from("sway"));
        let child = Command::new(sway)
            .args([OsStr::new("--unsupported-gpu"), OsStr::new("--config")])
            .arg(&config_path)
            .env("XDG_RUNTIME_DIR", runtime.path())
            .env("WLR_BACKENDS", "headless")
            .env("WLR_HEADLESS_OUTPUTS", "1")
            .env("WLR_LIBINPUT_NO_DEVICES", "1")
            .env("WLR_RENDERER", "pixman")
            .env("WLR_RENDERER_ALLOW_SOFTWARE", "1")
            .env("LIBGL_ALWAYS_SOFTWARE", "1")
            .env_remove("DISPLAY")
            .env_remove("I3SOCK")
            .env_remove("SWAYSOCK")
            .env_remove("WAYLAND_DISPLAY")
            .env_remove("WAYLAND_SOCKET")
            .env_remove("ZED_HEADLESS")
            .stdin(Stdio::null())
            .stdout(Stdio::from(log.try_clone()?))
            .stderr(Stdio::from(log))
            .spawn()?;
        Ok(HeadlessSession {
            sway: child,
            runtime,
        })
    }

    fn wait_for_wayland_socket(session: &mut HeadlessSession) -> io::Result<OsString> {
        let deadline = Instant::now() + READY_TIMEOUT;
        loop {
            if let Some(socket) = find_wayland_socket(session.runtime.path())? {
                return socket
                    .file_name()
                    .map(OsStr::to_os_string)
                    .ok_or_else(|| io::Error::other("Wayland socket has no file name"));
            }
            if let Some(status) = session.sway.try_wait()? {
                return Err(sway_start_error(
                    session.runtime.path(),
                    format!("Sway exited before its Wayland socket was ready ({status})"),
                ));
            }
            if Instant::now() >= deadline {
                return Err(sway_start_error(
                    session.runtime.path(),
                    "Sway did not create a Wayland socket within 5 seconds".to_owned(),
                ));
            }
            thread::sleep(READY_POLL_INTERVAL);
        }
    }

    fn find_wayland_socket(runtime: &Path) -> io::Result<Option<PathBuf>> {
        for entry in fs::read_dir(runtime)? {
            let entry = entry?;
            if entry
                .file_name()
                .to_str()
                .is_some_and(|name| name.starts_with("wayland-"))
                && entry.file_type()?.is_socket()
            {
                return Ok(Some(entry.path()));
            }
        }
        Ok(None)
    }

    fn sway_start_error(runtime: &Path, message: String) -> io::Error {
        let log = fs::read_to_string(runtime.join("sway.log")).unwrap_or_default();
        if log.trim().is_empty() {
            io::Error::other(message)
        } else {
            io::Error::other(format!("{message}\n{log}"))
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use std::{os::unix::net::UnixListener, process::Command};

        #[test]
        fn finds_only_wayland_unix_sockets() {
            let runtime = tempfile::tempdir().expect("runtime should be created");
            fs::write(runtime.path().join("wayland-not-a-socket"), "not a socket")
                .expect("fixture should be written");
            assert_eq!(
                find_wayland_socket(runtime.path()).expect("runtime should scan"),
                None
            );
            let socket = runtime.path().join("wayland-7");
            let _listener = UnixListener::bind(&socket).expect("socket should bind");
            assert_eq!(
                find_wayland_socket(runtime.path()).expect("runtime should scan"),
                Some(socket)
            );
        }

        #[test]
        fn launcher_propagates_child_status_and_cleans_up_sway() {
            let fixture = tempfile::tempdir().expect("fixture should be created");
            let source_path = fixture.path().join("fake_sway.rs");
            let sway_path = fixture.path().join("fake-sway");
            let state_path = fixture.path().join("fake-sway-state");
            fs::write(
                &source_path,
                format!(
                    r#"use std::{{env, fs, os::unix::net::UnixListener, thread, time::Duration}};
fn main() {{
    let runtime = env::var("XDG_RUNTIME_DIR").expect("runtime");
    let _listener = UnixListener::bind(format!("{{runtime}}/wayland-9")).expect("socket");
    fs::write({state_path:?}, format!("{{}}\n{{runtime}}\n", std::process::id())).expect("state");
    loop {{ thread::sleep(Duration::from_secs(1)); }}
}}
"#,
                    state_path = state_path,
                ),
            )
            .expect("fake Sway source should be written");
            let compile = Command::new("rustc")
                .args(["--edition=2024", "-o"])
                .arg(&sway_path)
                .arg(&source_path)
                .status()
                .expect("rustc should run");
            assert!(compile.success(), "fake Sway should compile");

            let status = run(
                &LaunchCommand::new(
                    "/bin/sh",
                    [
                        "-c",
                        "test -n \"$XDG_RUNTIME_DIR\" && test \"$WAYLAND_DISPLAY\" = wayland-9 && test \"$LIBGL_ALWAYS_SOFTWARE\" = 1 && test -S \"$XDG_RUNTIME_DIR/$WAYLAND_DISPLAY\"; exit 23",
                    ],
                ),
                &LaunchOptions {
                    sway: Some(sway_path.into_os_string()),
                },
            )
            .expect("launcher should run the child");
            assert_eq!(status.code(), Some(23));

            let state = fs::read_to_string(&state_path).expect("fake Sway should record state");
            let mut lines = state.lines();
            let pid = lines.next().expect("state should contain pid");
            let runtime = PathBuf::from(lines.next().expect("state should contain runtime"));
            assert!(!runtime.exists(), "private runtime should be removed");
            let alive = Command::new("/bin/kill")
                .args(["-0", pid])
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status()
                .expect("kill probe should run");
            assert!(!alive.success(), "fake Sway should be stopped");
        }
    }
}
