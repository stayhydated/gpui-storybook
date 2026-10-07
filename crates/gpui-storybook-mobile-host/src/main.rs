use clap::{Parser, Subcommand};
use gpui_storybook_automation::{AutomationBackend as _, StorybookAutomationError};
use gpui_storybook_mobile_host::{
    AdbTransport, AdbTransportOptions, AndroidLaunch, OwnedApplication, ReadinessOptions,
    RemoteBackend, process_id, shared,
};
use std::{net::SocketAddr, path::PathBuf, time::Duration};

type HostError = Box<dyn std::error::Error + Send + Sync>;
#[derive(Parser)]
#[command(about = "Launch, diagnose, and automate an explicitly selected Android application")]
struct Arguments {
    #[command(subcommand)]
    command: Command,
}
#[derive(Subcommand)]
enum Command {
    /// Attach MCP; optional launch/build metadata comes from storybook.toml.
    Serve(ServeArguments),
    /// Inspect device, application, and endpoint readiness without changing them.
    Doctor(DeviceArguments),
    /// Verify public state, captures, and optionally owned-emulator lifecycle.
    Smoke(SmokeArguments),
    /// Export the SDK's matching Activity/SurfaceView adapter for consumer builds.
    AndroidSource {
        #[arg(long)]
        output: PathBuf,
    },
}
#[derive(clap::Args)]
struct DeviceArguments {
    #[arg(long)]
    serial: String,
    #[arg(long, default_value = "127.0.0.1:5037")]
    adb_server: SocketAddr,
    #[arg(long)]
    config: Option<PathBuf>,
    #[arg(long, value_parser = clap::value_parser!(u16).range(1..))]
    device_port: Option<u16>,
}
#[derive(clap::Args)]
struct ServeArguments {
    #[command(flatten)]
    device: DeviceArguments,
    /// Submit one fresh launch using the application's [android] configuration.
    #[arg(long)]
    launch: bool,
    /// Run the application-owned build hook once before installation and launch.
    #[arg(long, requires = "launch")]
    build: bool,
    #[arg(long)]
    install: Option<PathBuf>,
    #[arg(long, requires = "launch")]
    stop_on_eof: bool,
    #[arg(long)]
    allow_interaction: bool,
}

#[derive(clap::Args)]
struct SmokeArguments {
    #[command(flatten)]
    device: DeviceArguments,
    #[arg(long)]
    plan: Option<PathBuf>,
    #[arg(long, default_value = "target/mobile-smoke")]
    output: PathBuf,
    #[arg(long)]
    allow_interaction: bool,
    #[arg(long, requires = "allow_interaction")]
    lifecycle: bool,
}
fn transport(
    arguments: &DeviceArguments,
    timeout: Duration,
) -> Result<AdbTransport, StorybookAutomationError> {
    AdbTransport::new(
        AdbTransportOptions::builder()
            .serial(arguments.serial.clone())
            .server(arguments.adb_server)
            .timeout(timeout)
            .build(),
    )
}
fn configuration(
    arguments: &DeviceArguments,
) -> Result<Option<AndroidLaunch>, StorybookAutomationError> {
    arguments
        .config
        .as_ref()
        .map(AndroidLaunch::load)
        .transpose()
}
fn port(arguments: &DeviceArguments, launch: Option<&AndroidLaunch>) -> u16 {
    arguments
        .device_port
        .or_else(|| launch.map(|launch| launch.application().device_port))
        .unwrap_or(gpui_storybook_mobile::DEVICE_PORT)
}
fn serve(arguments: ServeArguments) -> Result<(), HostError> {
    let config = configuration(&arguments.device)?;
    if arguments.build {
        config
            .as_ref()
            .ok_or("--build requires --config")?
            .build()?;
    }
    let runtime = tokio::runtime::Runtime::new()?;
    let transport = transport(&arguments.device, Duration::from_secs(30))?;
    let mut owned = if arguments.launch {
        Some(OwnedApplication::new(
            config
                .as_ref()
                .ok_or("--launch requires --config with [android]")?
                .application(),
        )?)
    } else {
        None
    };
    let startup = runtime.block_on(async {
        let apk = arguments.install.or_else(|| {
            if arguments.launch {
                config.as_ref().and_then(AndroidLaunch::apk)
            } else {
                None
            }
        });
        if let Some(apk) = apk {
            transport.install(apk).await?;
        }
        if let Some(owned) = &mut owned {
            owned.launch(&transport).await?;
        }
        let backend = RemoteBackend::attach_when_ready(
            transport.clone(),
            port(&arguments.device, config.as_ref()),
            ReadinessOptions::default(),
        )
        .await?;
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
            if let Some(owned) = &owned
                && let Err(cleanup) = runtime.block_on(owned.stop_if_owned(&transport))
            {
                return Err(combine(error, cleanup));
            }
            return Err(error);
        },
    };
    let result = server.serve_stdio_blocking();
    runtime.block_on(backend.shutdown());
    runtime.block_on(transport.shutdown());
    if (arguments.stop_on_eof || result.is_err())
        && let Some(owned) = &owned
        && let Err(cleanup) = runtime.block_on(owned.stop_if_owned(&transport))
    {
        return Err(match result {
            Err(error) => combine(error, cleanup),
            Ok(()) => cleanup.into(),
        });
    }
    result
}
fn combine(operation: HostError, cleanup: StorybookAutomationError) -> HostError {
    match operation.downcast::<StorybookAutomationError>() {
        Ok(operation) => StorybookAutomationError::SettlementFailed {
            operation,
            cleanup: Box::new(cleanup),
        }
        .into(),
        Err(operation) => Box::new(CliSettlementFailure { operation, cleanup }),
    }
}
#[derive(Debug)]
struct CliSettlementFailure {
    operation: HostError,
    cleanup: StorybookAutomationError,
}
impl std::fmt::Display for CliSettlementFailure {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "{}; owned application cleanup also failed: {}",
            self.operation, self.cleanup
        )
    }
}
impl std::error::Error for CliSettlementFailure {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(self.operation.as_ref())
    }
}

fn doctor(arguments: DeviceArguments) -> Result<(), HostError> {
    let config = configuration(&arguments)?;
    let runtime = tokio::runtime::Runtime::new()?;
    let transport = transport(&arguments, Duration::from_secs(5))?;
    runtime.block_on(async {
        let api = transport.shell(&["getprop", "ro.build.version.sdk"]).await;
        let abi = transport.shell(&["getprop", "ro.product.cpu.abi"]).await;
        let package = config.as_ref().map(|config| config.application().package.as_str());
        let pid = match package { Some(package) => Some(process_id(&transport, package).await), None => None };
        let attachment = RemoteBackend::attach_when_ready(transport.clone(), port(&arguments, config.as_ref()), ReadinessOptions::builder().timeout(Duration::from_secs(5)).build()).await;
        let (host, issue) = match attachment {
            Ok(backend) => { let host = backend.host().await; backend.shutdown().await; match host { Ok(host) => (Some(host), None), Err(error) => (None, Some(error)) } },
            Err(error) => (None, Some(error)),
        };
        transport.shutdown().await;
        let output = serde_json::json!({ "ready": host.is_some(), "serial": arguments.serial,
            "device_port": port(&arguments, config.as_ref()), "android_api": api.map(|output| String::from_utf8_lossy(output.stdout()).trim().to_owned()).map_err(|error| error.to_string()),
            "abi": abi.map(|output| String::from_utf8_lossy(output.stdout()).trim().to_owned()).map_err(|error| error.to_string()),
            "package": package, "pid": pid.map(|result| result.map_err(|error| error.to_string())),
            "apk": config.as_ref().and_then(AndroidLaunch::apk), "host": host, "issue": issue,
            "next_action": issue.as_ref().map(ToString::to_string) });
        println!("{}", serde_json::to_string_pretty(&output)?);
        if let Some(issue) = issue { return Err::<_, HostError>(issue.into()); }
        Ok(())
    })
}
fn smoke(arguments: SmokeArguments) -> Result<(), HostError> {
    let config = configuration(&arguments.device)?;
    let plan_path = arguments.plan.or_else(|| {
        arguments.device.config.as_ref().and_then(|path| {
            config
                .as_ref()?
                .application()
                .smoke_plan
                .as_ref()
                .map(|plan| {
                    path.parent()
                        .unwrap_or_else(|| std::path::Path::new("."))
                        .join(plan)
                })
        })
    });
    let plan = plan_path
        .map(gpui_storybook_mobile_host::SmokePlan::load)
        .transpose()?
        .unwrap_or_default();
    let runtime = tokio::runtime::Runtime::new()?;
    let transport = transport(&arguments.device, Duration::from_secs(30))?;
    let report = runtime.block_on(async {
        let runner = gpui_storybook_mobile_host::SmokeRunner::new(
            transport.clone(),
            port(&arguments.device, config.as_ref()),
            config.as_ref().map(|config| config.application().clone()),
        );
        let result = runner
            .run(
                plan,
                gpui_storybook_mobile_host::SmokeOptions::builder()
                    .output(arguments.output.clone())
                    .allow_interaction(arguments.allow_interaction)
                    .lifecycle(arguments.lifecycle)
                    .build(),
            )
            .await;
        runner.shutdown().await;
        result
    })?;
    runtime.block_on(transport.shutdown());
    std::fs::create_dir_all(&arguments.output)?;
    let encoded = serde_json::to_string_pretty(&report)?;
    std::fs::write(arguments.output.join("report.json"), format!("{encoded}\n"))?;
    println!("{encoded}");
    if let Some(error) = report.error() {
        return Err(error.clone().into());
    }
    Ok(())
}

fn main() -> Result<(), HostError> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "warn".into()),
        )
        .with_writer(std::io::stderr)
        .init();
    match Arguments::parse().command {
        Command::Serve(arguments) => serve(arguments),
        Command::Doctor(arguments) => doctor(arguments),
        Command::Smoke(arguments) => smoke(arguments),
        Command::AndroidSource { output } => {
            if let Some(parent) = output
                .parent()
                .filter(|parent| !parent.as_os_str().is_empty())
            {
                std::fs::create_dir_all(parent)?;
            }
            std::fs::write(output, gpui_storybook_mobile::ANDROID_ADAPTER_SOURCE)?;
            Ok(())
        },
    }
}
