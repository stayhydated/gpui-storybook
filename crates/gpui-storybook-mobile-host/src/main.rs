use clap::Parser;
use gpui_storybook_automation::StorybookAutomationError;
use gpui_storybook_mobile_host::{AdbTransport, AdbTransportOptions, RemoteBackend, shared};
use std::{net::SocketAddr, path::PathBuf, time::Duration};

type HostError = Box<dyn std::error::Error + Send + Sync>;
const EXAMPLE_PACKAGE: &str = "dev.storybook.mobile";

#[derive(Parser)]
#[command(about = "Attach MCP to an explicitly selected, opted-in Android application")]
struct Arguments {
    #[arg(long)]
    serial: String,
    #[arg(long, default_value = "127.0.0.1:5037")]
    adb_server: SocketAddr,
    #[arg(long, default_value_t = gpui_storybook_mobile::DEVICE_PORT, value_parser = clap::value_parser!(u16).range(1..))]
    device_port: u16,
    /// Install this APK once before attachment, preserving the installed package on failure.
    #[arg(long)]
    install: Option<PathBuf>,
    /// Launch a fresh repository example process with runtime automation opt-in.
    #[arg(long)]
    launch_example: bool,
    /// Stop the owned example process on MCP EOF (requires --launch-example).
    #[arg(long, requires = "launch_example")]
    stop_on_eof: bool,
    #[arg(long)]
    allow_interaction: bool,
}

async fn process_id(transport: &AdbTransport) -> Result<Option<String>, HostError> {
    let output = transport.shell(&["pidof", EXAMPLE_PACKAGE]).await?;
    if output.status() == 1 && output.stdout().is_empty() {
        return Ok(None);
    }
    let output = output.require_success()?;
    let pid = std::str::from_utf8(output.stdout())?.trim();
    if pid.is_empty() || !pid.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err("example PID response is invalid or names multiple processes".into());
    }
    Ok(Some(pid.to_owned()))
}

async fn stop_owned(transport: &AdbTransport, owned: Option<&str>) -> Result<(), HostError> {
    if let Some(owned) = owned
        && process_id(transport).await?.as_deref() == Some(owned)
    {
        transport
            .shell(&["am", "force-stop", EXAMPLE_PACKAGE])
            .await?
            .require_success()?;
    }
    Ok(())
}

async fn attach(transport: AdbTransport, port: u16) -> Result<RemoteBackend, HostError> {
    // Only readiness observations are repeated; commands are submitted once.
    let deadline = tokio::time::Instant::now() + Duration::from_secs(30);
    loop {
        match tokio::time::timeout_at(
            deadline,
            RemoteBackend::attach_with_transport(transport.clone(), port),
        )
        .await
        .map_err(|_| "device startup deadline exceeded")?
        {
            Ok(backend) => return Ok(backend),
            Err(error)
                if matches!(
                    error,
                    StorybookAutomationError::NoLiveHost
                        | StorybookAutomationError::CaptureUnavailable { .. }
                        | StorybookAutomationError::OutcomeUnknown { .. }
                ) && tokio::time::Instant::now() < deadline =>
            {
                eprintln!("waiting for device readiness: {error}");
                tokio::time::sleep(Duration::from_millis(250)).await;
            },
            Err(error) => return Err(error.into()),
        }
    }
}

fn main() -> Result<(), HostError> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "warn".into()),
        )
        .with_writer(std::io::stderr)
        .with_span_events(tracing_subscriber::fmt::format::FmtSpan::CLOSE)
        .init();
    let arguments = Arguments::parse();
    let runtime = tokio::runtime::Runtime::new()?;
    let transport = AdbTransport::new(
        AdbTransportOptions::builder()
            .serial(arguments.serial)
            .server(arguments.adb_server)
            .build(),
    )?;
    let mut owned_pid = None;
    let startup = runtime.block_on(async {
        if let Some(apk) = arguments.install {
            transport.install(apk).await?;
        }
        if arguments.launch_example {
            let previous = process_id(&transport).await?;
            let launched = transport
                .shell(&[
                    "am",
                    "start",
                    "-W",
                    "-S",
                    "-n",
                    "dev.storybook.mobile/.MainActivity",
                    "--ez",
                    "storybook_automation",
                    "true",
                ])
                .await
                .and_then(|output| output.require_success());
            // Even a lost launch reply can leave a newly owned process alive.
            let observed = process_id(&transport).await?;
            if observed != previous {
                owned_pid = observed;
            }
            launched?;
            if owned_pid.is_none() {
                return Err::<_, HostError>("fresh example launch has no owned PID".into());
            }
        }
        let backend = attach(transport.clone(), arguments.device_port).await?;
        let server = gpui_storybook_mcp::server_with_options(
            shared(backend.clone()),
            gpui_storybook_mcp::StorybookMcpServerOptions::default()
                .with_interaction(arguments.allow_interaction),
        )?;
        Ok::<_, HostError>((backend, server))
    });
    let (backend, server) = match startup {
        Ok(started) => started,
        Err(error) => {
            runtime.block_on(transport.shutdown());
            if let Err(cleanup) = runtime.block_on(stop_owned(&transport, owned_pid.as_deref())) {
                eprintln!("owned startup cleanup failed: {cleanup}");
            }
            return Err(error);
        },
    };
    let result = server.serve_stdio_blocking();
    runtime.block_on(backend.shutdown());
    runtime.block_on(transport.shutdown());
    if arguments.stop_on_eof || result.is_err() {
        let cleanup = runtime.block_on(stop_owned(&transport, owned_pid.as_deref()));
        if let Err(error) = cleanup {
            if result.is_ok() {
                return Err(error);
            }
            eprintln!("owned app cleanup failed: {error}");
        }
    }
    result
}
