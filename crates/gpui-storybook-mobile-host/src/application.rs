//! Consumer launch configuration and cleanup of an explicitly owned process.

use crate::*;
use gpui_storybook_toml::{AndroidApplication, StorybookToml};

pub struct AndroidLaunch {
    application: AndroidApplication,
    directory: PathBuf,
}
impl AndroidLaunch {
    /// Load bounded configuration; artifact paths resolve against its directory.
    pub fn load(path: impl AsRef<Path>) -> Result<Self, StorybookAutomationError> {
        let path = path.as_ref();
        let metadata = std::fs::metadata(path)
            .map_err(|error| unavailable(format!("{}: {error}", path.display())))?;
        if !metadata.is_file() || metadata.len() > 128 * 1024 {
            return Err(unavailable(
                "launch configuration must be a regular file of at most 128 KiB",
            ));
        }
        let source =
            std::fs::read_to_string(path).map_err(|error| unavailable(error.to_string()))?;
        let config: StorybookToml = toml::from_str(&source)
            .map_err(|error| unavailable(format!("{}: {error}", path.display())))?;
        let application = config
            .android
            .ok_or_else(|| unavailable("launch configuration requires an [android] table"))?;
        application.validate().map_err(unavailable)?;
        let directory = path
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
            .unwrap_or_else(|| Path::new("."))
            .to_owned();
        Ok(Self {
            application,
            directory,
        })
    }
    pub fn application(&self) -> &AndroidApplication {
        &self.application
    }
    pub fn apk(&self) -> Option<PathBuf> {
        self.application
            .apk
            .as_ref()
            .map(|path| self.directory.join(path))
    }
    /// Run the application-owned build hook once before starting the host runtime.
    pub fn build(&self) -> Result<(), StorybookAutomationError> {
        let (program, arguments) = self
            .application
            .build
            .split_first()
            .ok_or_else(|| unavailable("[android].build is required for --build"))?;
        let status = std::process::Command::new(program)
            .args(arguments)
            .current_dir(&self.directory)
            .status()
            .map_err(|error| unavailable(format!("application build: {error}")))?;
        if !status.success() {
            return Err(unavailable(format!(
                "application build exited with {status}"
            )));
        }
        Ok(())
    }
}

/// One explicitly launched process. Cleanup preserves a replacement or
/// pre-existing process by comparing its current PID with the observed owner.
pub struct OwnedApplication {
    application: AndroidApplication,
    pid: Option<String>,
    submitted: bool,
}
impl OwnedApplication {
    pub fn new(application: &AndroidApplication) -> Result<Self, StorybookAutomationError> {
        application.validate().map_err(unavailable)?;
        Ok(Self {
            application: application.clone(),
            pid: None,
            submitted: false,
        })
    }
    pub fn pid(&self) -> Option<&str> {
        self.pid.as_deref()
    }
    pub async fn launch(
        &mut self,
        transport: &AdbTransport,
    ) -> Result<(), StorybookAutomationError> {
        if self.submitted {
            return Err(unavailable(
                "application launch was already submitted; observe its outcome without replay",
            ));
        }
        let previous = process_id(transport, &self.application.package).await?;
        let arguments = self.application.launch_arguments();
        let arguments: Vec<_> = arguments.iter().map(String::as_str).collect();
        self.submitted = true;
        let launched = transport
            .shell(&arguments)
            .await
            .and_then(ShellOutput::require_success);
        let observed = process_id(transport, &self.application.package).await;
        match (launched, observed) {
            (Err(operation), Err(cleanup)) => {
                return Err(StorybookAutomationError::SettlementFailed {
                    operation: Box::new(operation),
                    cleanup: Box::new(cleanup),
                });
            },
            (result, Ok(observed)) => {
                if observed != previous {
                    self.pid = observed;
                }
                result?;
            },
            (Ok(_), Err(error)) => return Err(error),
        }
        if self.pid.is_none() {
            return Err(unavailable(
                "fresh application launch has no observed owned PID",
            ));
        }
        Ok(())
    }
    pub async fn stop_if_owned(
        &self,
        transport: &AdbTransport,
    ) -> Result<(), StorybookAutomationError> {
        if let Some(pid) = &self.pid
            && process_id(transport, &self.application.package)
                .await?
                .as_ref()
                == Some(pid)
        {
            transport
                .shell(&["am", "force-stop", &self.application.package])
                .await?
                .require_success()?;
        }
        Ok(())
    }
}

pub async fn process_id(
    transport: &AdbTransport,
    package: &str,
) -> Result<Option<String>, StorybookAutomationError> {
    let output = transport.shell(&["pidof", package]).await?;
    if output.status() == 1 && output.stdout().is_empty() {
        return Ok(None);
    }
    let output = output.require_success()?;
    let pid = std::str::from_utf8(output.stdout())
        .map_err(|error| unavailable(error.to_string()))?
        .trim();
    if pid.is_empty() || !pid.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err(unavailable(
            "PID response is invalid or names multiple processes",
        ));
    }
    Ok(Some(pid.to_owned()))
}
