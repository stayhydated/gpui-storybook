use clap::Parser;
use gpui_storybook_mobile_host::{RemoteBackend, adb, shared};
use std::path::PathBuf;

#[derive(Parser)]
#[command(about = "Attach MCP to an explicitly selected, opted-in Android application")]
struct Arguments {
    #[arg(long)]
    serial: String,
    #[arg(long, default_value_t = gpui_storybook_mobile::DEVICE_PORT)]
    device_port: u16,
    /// Install this APK before attachment.
    #[arg(long)]
    install: Option<PathBuf>,
    /// Launch a fresh repository example process with runtime automation opt-in.
    #[arg(long)]
    launch_example: bool,
    /// Stop the example process on MCP EOF (requires --launch-example).
    #[arg(long, requires = "launch_example")]
    stop_on_eof: bool,
    #[arg(long)]
    allow_interaction: bool,
}
fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let arguments = Arguments::parse();
    if let Some(apk) = &arguments.install {
        adb(
            &arguments.serial,
            &[
                "install",
                "-r",
                apk.to_str().ok_or("APK path is not UTF-8")?,
            ],
        )?;
    }
    if arguments.launch_example {
        adb(
            &arguments.serial,
            &[
                "shell",
                "am",
                "start",
                "-S",
                "-n",
                "dev.storybook.mobile/.MainActivity",
                "--ez",
                "storybook_automation",
                "true",
            ],
        )?;
    }
    // Read-only handshakes may wait for startup; admitted mutations are never retried.
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(30);
    let backend = loop {
        match RemoteBackend::attach(arguments.serial.clone(), arguments.device_port) {
            Ok(backend) => break backend,
            Err(error)
                if matches!(
                    error,
                    gpui_storybook_automation::StorybookAutomationError::NoLiveHost
                        | gpui_storybook_automation::StorybookAutomationError::CaptureUnavailable { .. }
                        | gpui_storybook_automation::StorybookAutomationError::OutcomeUnknown { .. }
                ) && std::time::Instant::now() < deadline =>
            {
                eprintln!("waiting for device readiness: {error}");
                std::thread::sleep(std::time::Duration::from_millis(250));
            },
            Err(error) => return Err(error.into()),
        }
    };
    let server = gpui_storybook_mcp::server_with_options(
        shared(backend),
        gpui_storybook_mcp::StorybookMcpServerOptions::default()
            .with_interaction(arguments.allow_interaction),
    )?;
    let result = server.serve_stdio_blocking();
    if arguments.stop_on_eof {
        adb(
            &arguments.serial,
            &["shell", "am", "force-stop", "dev.storybook.mobile"],
        )?;
    }
    result
}
