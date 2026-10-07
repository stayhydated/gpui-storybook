//! Bounded discovery and reattachment repeat observations only.

use crate::*;
use std::time::Duration;

/// Conditions for a fresh device attachment. No submitted operation is retried.
#[derive(Clone, Debug, bon::Builder)]
pub struct ReadinessOptions {
    #[builder(default = Duration::from_secs(30))]
    timeout: Duration,
    #[builder(default = Duration::from_millis(100))]
    poll_interval: Duration,
    orientation: Option<DisplayOrientation>,
    previous_session: Option<String>,
}
impl Default for ReadinessOptions {
    fn default() -> Self {
        Self::builder().build()
    }
}

impl RemoteBackend {
    /// Discover readiness or explicitly replace a stale attachment. Each attempt
    /// is a fresh read-only `GetHost` handshake; mutations and captures stay single
    /// submissions. Keep the returned backend's runtime alive through shutdown.
    pub async fn attach_when_ready(
        transport: AdbTransport,
        device_port: u16,
        options: ReadinessOptions,
    ) -> Result<Self, StorybookAutomationError> {
        if device_port == 0 || options.timeout.is_zero() || options.poll_interval.is_zero() {
            return Err(unavailable(
                "readiness requires a nonzero port, timeout, and poll interval",
            ));
        }
        let deadline = tokio::time::Instant::now() + options.timeout;
        let mut last = StorybookAutomationError::NoLiveHost;
        loop {
            match tokio::time::timeout_at(
                deadline,
                Self::attach_with_transport(transport.clone(), device_port),
            )
            .await
            {
                Ok(Ok(backend)) => {
                    let host = &backend.descriptor;
                    let issue = if options
                        .orientation
                        .is_some_and(|orientation| host.orientation() != orientation)
                    {
                        Some(HostReadinessIssue::OrientationPending {
                            expected: options.orientation.unwrap(),
                            observed: host.orientation(),
                        })
                    } else if options.previous_session.as_deref() == Some(host.session()) {
                        Some(HostReadinessIssue::SessionPending {
                            session: host.session().to_owned(),
                        })
                    } else {
                        None
                    };
                    if let Some(issue) = issue {
                        last = StorybookAutomationError::HostNotReady { issue };
                    } else {
                        return Ok(backend);
                    }
                },
                Ok(Err(
                    error @ (StorybookAutomationError::HostNotReady { .. }
                    | StorybookAutomationError::NoLiveHost
                    | StorybookAutomationError::CaptureUnavailable { .. }
                    | StorybookAutomationError::OutcomeUnknown { .. }
                    | StorybookAutomationError::StaleHost { .. }),
                )) => last = error,
                Ok(Err(error)) => return Err(error),
                Err(_) => break,
            }
            tracing::debug!(%last, "waiting for device readiness");
            if tokio::time::Instant::now() >= deadline {
                break;
            }
            tokio::time::sleep_until(
                (tokio::time::Instant::now() + options.poll_interval).min(deadline),
            )
            .await;
        }
        Err(StorybookAutomationError::DeviceReadinessTimedOut {
            seconds: options.timeout.as_secs(),
            last_observation: Box::new(last),
        })
    }
}
