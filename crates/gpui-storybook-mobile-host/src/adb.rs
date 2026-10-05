//! Selected adbutils primitives under host-owned deadlines and byte budgets.
use adbutils::{AdbClient, AdbDevice, Network};
use gpui_storybook_automation::{StorybookAutomationError, wire::*};
use std::{
    net::SocketAddr,
    sync::{Arc, OnceLock},
    time::Duration,
};
use tokio::{
    io::{AsyncReadExt as _, AsyncWriteExt as _},
    net::TcpStream,
};

use crate::{request_id, tasks::OwnedTasks, unavailable};

pub(crate) const MAX_CAPTURE_BYTES: usize = 64 * 1024 * 1024;
const MAX_SHELL_BYTES: usize = 1024 * 1024;

/// Explicit device selection and overall I/O deadlines. The local ADB server
/// owns transport selection; Storybook owns framing, limits, and input outcomes.
#[derive(Clone, Debug, bon::Builder)]
pub struct AdbTransportOptions {
    serial: String,
    #[builder(default = SocketAddr::from(([127, 0, 0, 1], 5037)))]
    server: SocketAddr,
    #[builder(default = Duration::from_secs(30))]
    timeout: Duration,
    #[builder(default = Duration::from_secs(10))]
    capture_timeout: Duration,
    #[builder(default = Duration::from_secs(120))]
    install_timeout: Duration,
}

/// An explicitly selected device with a fresh connection per operation.
/// Failed submissions are never replayed. Cancellation closes the host socket;
/// the device keeps ownership of work it already admitted.
#[derive(Clone)]
pub struct AdbTransport {
    device: AdbDevice,
    options: AdbTransportOptions,
    installs: Arc<OnceLock<Arc<OwnedTasks>>>,
}

impl AdbTransport {
    pub fn new(options: AdbTransportOptions) -> Result<Self, StorybookAutomationError> {
        if options.serial.is_empty()
            || options.serial.len() > 256
            || options.serial.contains(['\0', '\r', '\n'])
            || !options.server.ip().is_loopback()
            || options.server.port() == 0
            || options.timeout.is_zero()
            || options.capture_timeout.is_zero()
            || options.install_timeout.is_zero()
        {
            return Err(unavailable("invalid explicit ADB transport options"));
        }
        let device = AdbClient::new(
            options.server.ip().to_string(),
            options.server.port(),
            Some(options.timeout),
        )
        .device(&options.serial);
        Ok(Self {
            device,
            options,
            installs: Arc::new(OnceLock::new()),
        })
    }

    pub fn serial(&self) -> &str {
        &self.options.serial
    }
    pub fn timeout(&self) -> Duration {
        self.options.timeout
    }

    /// Stream one bounded APK to a unique owned device path, run one install,
    /// then remove that path on success or failure. Cancellation of the caller
    /// retains the task on its initial runtime; keep it alive and call
    /// [`Self::shutdown`] before stopping it.
    /// Package-manager failures never trigger uninstall or mutation replay.
    pub async fn install(&self, path: std::path::PathBuf) -> Result<(), StorybookAutomationError> {
        let owner = self.installs.get_or_init(OwnedTasks::current);
        let job = owner.admit()?;
        let transport = self.clone();
        let (reply, receiver) = tokio::sync::oneshot::channel();
        owner.spawn(async move {
            let _job = job;
            let result = transport.install_owned(path).await;
            let _ = reply.send(result);
        });
        receiver
            .await
            .map_err(|error| unavailable(error.to_string()))?
    }

    /// Close install admission and await admitted installs and owned-file cleanup,
    /// including tasks whose caller canceled. Keep the first install runtime alive.
    pub async fn shutdown(&self) {
        self.installs
            .get_or_init(OwnedTasks::current)
            .settle()
            .await;
    }

    async fn install_owned(
        &self,
        path: std::path::PathBuf,
    ) -> Result<(), StorybookAutomationError> {
        let mut file = tokio::fs::File::open(path)
            .await
            .map_err(|error| unavailable(error.to_string()))?;
        let metadata = file
            .metadata()
            .await
            .map_err(|error| unavailable(error.to_string()))?;
        const MAX_APK_BYTES: u64 = 512 * 1024 * 1024;
        if !metadata.is_file() || metadata.len() == 0 || metadata.len() > MAX_APK_BYTES {
            return Err(unavailable("APK must be a regular file of at most 512 MiB"));
        }
        let id = request_id();
        let remote = format!("/data/local/tmp/storybook-{}-{id}.apk", std::process::id());
        let operation = async {
            let mut stream = self.service("sync:").await?;
            let upload = async {
                let destination = format!("{remote},33188");
                stream.write_all(b"SEND").await?;
                stream.write_u32_le(destination.len() as u32).await?;
                stream.write_all(destination.as_bytes()).await?;
                let mut buffer = [0u8; 64 * 1024];
                let mut sent = 0;
                loop {
                    let size = file.read(&mut buffer).await?;
                    if size == 0 {
                        break;
                    }
                    sent += size as u64;
                    if sent > metadata.len() {
                        return Err(std::io::Error::other("APK changed while streaming"));
                    }
                    stream.write_all(b"DATA").await?;
                    stream.write_u32_le(size as u32).await?;
                    stream.write_all(&buffer[..size]).await?;
                }
                if sent != metadata.len() {
                    return Err(std::io::Error::other("APK changed while streaming"));
                }
                stream.write_all(b"DONE").await?;
                stream.write_u32_le(0).await?;
                let mut status = [0u8; 4];
                stream.read_exact(&mut status).await?;
                let size = stream.read_u32_le().await? as usize;
                match &status {
                    b"OKAY" if size == 0 => (),
                    b"FAIL" if size <= 64 * 1024 => {
                        let mut message = vec![0; size];
                        stream.read_exact(&mut message).await?;
                        return Err(std::io::Error::other(
                            String::from_utf8_lossy(&message).into_owned(),
                        ));
                    },
                    _ => return Err(std::io::Error::other("invalid ADB sync completion")),
                }
                Ok::<_, std::io::Error>(())
            };
            upload
                .await
                .map_err(|error| unavailable(error.to_string()))?;
            drop(stream);
            let output = self
                .shell(&["pm", "install", "-r", "-t", &remote])
                .await?
                .require_success()?;
            if !String::from_utf8_lossy(output.stdout())
                .lines()
                .any(|line| line.trim() == "Success")
            {
                return Err(unavailable(
                    "package manager did not report successful installation",
                ));
            }
            Ok(())
        };
        let result = tokio::time::timeout(self.options.install_timeout, operation)
            .await
            .map_err(|_| unknown(id, "APK install deadline exceeded"))
            .and_then(|result| result);
        let removed = self
            .shell(&["rm", "-f", &remote])
            .await
            .and_then(ShellOutput::require_success);
        result?;
        removed.map(|_| ())
    }

    pub(crate) async fn exchange(
        &self,
        port: u16,
        session: &str,
        id: u64,
        command: DeviceOperation,
    ) -> Result<DeviceResult, StorybookAutomationError> {
        let request = DeviceRequest::builder()
            .protocol_version(PROTOCOL_VERSION)
            .session(session.to_owned())
            .request_id(id)
            .command(command)
            .build();
        let body = serde_json::to_vec(&request).map_err(|error| unavailable(error.to_string()))?;
        if body.is_empty() || body.len() > MAX_WIRE_BYTES {
            return Err(unavailable("device request exceeds wire bound"));
        }
        let mut submitted = false;
        let operation = async {
            let mut stream = self
                .device
                .create_connection(Network::Tcp, &port.to_string())
                .await
                .map_err(|error| unavailable(error.to_string()))?;
            submitted = true;
            let result = async {
                stream.write_u32(body.len() as u32).await?;
                stream.write_all(&body).await?;
                let size = stream.read_u32().await? as usize;
                if size == 0 || size > MAX_WIRE_BYTES {
                    return Err(std::io::Error::other("device response exceeds wire bound"));
                }
                let mut body = vec![0; size];
                stream.read_exact(&mut body).await?;
                serde_json::from_slice::<DeviceResponse>(&body).map_err(std::io::Error::other)
            }
            .await
            .map_err(|error| unknown(id, error.to_string()))?;
            if result.protocol_version() != PROTOCOL_VERSION {
                return Err(StorybookAutomationError::ProtocolMismatch {
                    expected: PROTOCOL_VERSION,
                    actual: result.protocol_version(),
                });
            }
            if result.request_id() != id || (!session.is_empty() && result.session() != session) {
                return Err(StorybookAutomationError::StaleHost {
                    message: "device response identity does not match submitted request".to_owned(),
                });
            }
            let response_session = result.session().to_owned();
            let outcome = result.into_outcome()?;
            let host = match &outcome {
                DeviceResult::Host(host) | DeviceResult::CaptureTicket { host, .. } => Some(host),
                _ => None,
            };
            if let Some(host) = host {
                if host.protocol_version() != PROTOCOL_VERSION {
                    return Err(StorybookAutomationError::ProtocolMismatch {
                        expected: PROTOCOL_VERSION,
                        actual: host.protocol_version(),
                    });
                }
                if host.session().is_empty()
                    || host.session().len() > 128
                    || host.session() != response_session
                {
                    return Err(StorybookAutomationError::StaleHost {
                        message: "host descriptor does not match its response session".to_owned(),
                    });
                }
                if !host.agrees() {
                    return Err(unavailable(
                        "host descriptor geometry and native route disagree",
                    ));
                }
            }
            Ok(outcome)
        };
        match tokio::time::timeout(self.options.timeout, operation).await {
            Ok(result) => result,
            Err(_) if submitted => Err(unknown(id, "device response deadline exceeded")),
            Err(_) => Err(unavailable("ADB device connection deadline exceeded")),
        }
    }

    async fn service(&self, name: &str) -> Result<TcpStream, StorybookAutomationError> {
        let mut connection = self
            .device
            .open_transport(None, Some(self.options.timeout))
            .await
            .map_err(|error| unavailable(error.to_string()))?;
        connection
            .send_command(name)
            .await
            .map_err(|error| unavailable(error.to_string()))?;
        connection
            .check_okay()
            .await
            .map_err(|error| unavailable(error.to_string()))?;
        connection
            .into_stream()
            .map_err(|error| unavailable(error.to_string()))
    }

    pub(crate) async fn screencap(&self) -> Result<Vec<u8>, StorybookAutomationError> {
        let capture = async {
            let stream = self.service("exec:screencap -p").await?;
            let mut body = Vec::new();
            stream
                .take(MAX_CAPTURE_BYTES as u64 + 1)
                .read_to_end(&mut body)
                .await
                .map_err(|error| unavailable(error.to_string()))?;
            if body.len() > MAX_CAPTURE_BYTES {
                return Err(unavailable("ADB PNG exceeds 64 MiB"));
            }
            Ok(body)
        };
        tokio::time::timeout(self.options.capture_timeout, capture)
            .await
            .map_err(|_| unavailable("ADB capture deadline exceeded"))?
    }

    /// Run one argument-quoted shell-v2 command, preserving both streams and its
    /// terminal exit status. The combined stream budget is 1 MiB. An interrupted
    /// command returns an unknown outcome and is never resubmitted.
    pub async fn shell(&self, arguments: &[&str]) -> Result<ShellOutput, StorybookAutomationError> {
        let id = request_id();
        let command = adbutils::utils::list2cmdline(arguments);
        if arguments.is_empty() || command.contains('\0') || command.len() > 16 * 1024 {
            return Err(unavailable(
                "ADB shell command is empty, contains NUL, or exceeds bound",
            ));
        }
        let mut submitted = false;
        let operation = async {
            // The service command can start execution before its OKAY reply.
            let mut connection = self
                .device
                .open_transport(None, Some(self.options.timeout))
                .await
                .map_err(|error| unavailable(error.to_string()))?;
            submitted = true;
            connection
                .send_command(&format!("shell,v2,raw:{command}"))
                .await
                .map_err(|error| unknown(id, error.to_string()))?;
            connection
                .check_okay()
                .await
                .map_err(|error| unknown(id, error.to_string()))?;
            let mut stream = connection
                .into_stream()
                .map_err(|error| unknown(id, error.to_string()))?;
            read_shell(&mut stream)
                .await
                .map_err(|error| unknown(id, error.to_string()))
        };
        match tokio::time::timeout(self.options.timeout, operation).await {
            Ok(result) => result,
            Err(_) if submitted => Err(unknown(id, "ADB shell deadline exceeded")),
            Err(_) => Err(unavailable("ADB shell connection deadline exceeded")),
        }
    }
}

/// Observed shell-v2 output; a terminal status is required for success.
#[derive(Debug)]
pub struct ShellOutput {
    stdout: Vec<u8>,
    stderr: Vec<u8>,
    status: u8,
}
impl ShellOutput {
    pub fn stdout(&self) -> &[u8] {
        &self.stdout
    }
    pub fn stderr(&self) -> &[u8] {
        &self.stderr
    }
    pub fn status(&self) -> u8 {
        self.status
    }
    pub fn require_success(self) -> Result<Self, StorybookAutomationError> {
        if self.status != 0 {
            return Err(unavailable(format!(
                "ADB shell exited {}: {}",
                self.status,
                String::from_utf8_lossy(&self.stderr)
            )));
        }
        Ok(self)
    }
}

async fn read_shell(stream: &mut TcpStream) -> std::io::Result<ShellOutput> {
    let mut result = ShellOutput {
        stdout: Vec::new(),
        stderr: Vec::new(),
        status: 0,
    };
    loop {
        let channel = stream.read_u8().await?;
        let size = stream.read_u32_le().await? as usize;
        if channel == 3 {
            if size != 1 {
                return Err(std::io::Error::other("invalid shell exit packet"));
            }
            result.status = stream.read_u8().await?;
            return Ok(result);
        }
        if !matches!(channel, 1 | 2)
            || size > MAX_SHELL_BYTES
            || result.stdout.len() + result.stderr.len() + size > MAX_SHELL_BYTES
        {
            return Err(std::io::Error::other(
                "ADB shell output exceeds bound or has an invalid channel",
            ));
        }
        let output = if channel == 1 {
            &mut result.stdout
        } else {
            &mut result.stderr
        };
        let offset = output.len();
        output.resize(offset + size, 0);
        stream.read_exact(&mut output[offset..]).await?;
    }
}

fn unknown(id: u64, message: impl Into<String>) -> StorybookAutomationError {
    StorybookAutomationError::OutcomeUnknown {
        request_id: id,
        message: message.into(),
    }
}

impl std::fmt::Debug for AdbTransport {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("AdbTransport")
            .field("options", &self.options)
            .finish_non_exhaustive()
    }
}
