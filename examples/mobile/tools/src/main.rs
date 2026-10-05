//! Process-isolated probes of published Android libraries. The Python runner
//! owns deadlines and cleanup; input commands are never retried by this driver.
use std::io::{self, BufRead, Write};
use std::path::Path;
use std::sync::{Arc, RwLock};
use std::time::Duration;

use adb_client::{ADBDeviceExt, server_device::ADBServerDevice};
use anyhow::{Context, Result, bail, ensure};
use clap::{Parser, ValueEnum};
use serde_json::{Value, json};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

#[derive(Clone, Copy, Debug, ValueEnum)]
enum Candidate {
    AdbClient,
    Adbutils,
    Uiautomator,
    Uiautomator2,
}

#[derive(Debug, Parser)]
struct Args {
    #[arg(long)]
    candidate: Candidate,
    #[arg(long)]
    serial: String,
    #[arg(long)]
    rpc_url: Option<String>,
    #[arg(long, default_value_t = 9008)]
    port: u16,
    #[arg(long, default_value_t = 1)]
    attempts: u32,
    /// Bypass uiautomator2's server recovery for input operations.
    #[arg(long)]
    single_attempt: bool,
}

enum Probe {
    AdbClient(ADBServerDevice),
    Adbutils(adbutils::AdbDevice),
    Uiautomator(uiautomator::JsonRpcClient),
    Uiautomator2(uiautomator2::Server, bool),
}

/// Bounded stdout/stderr for adb_client's streaming shell API.
struct Limited(Vec<u8>);
impl Write for Limited {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if self.0.len().saturating_add(bytes.len()) > 16 * 1024 * 1024 {
            return Err(io::Error::other("probe output exceeds 16 MiB"));
        }
        self.0.extend_from_slice(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

fn text<'a>(request: &'a Value, key: &str) -> Result<&'a str> {
    request[key]
        .as_str()
        .with_context(|| format!("missing {key}"))
}

impl Probe {
    async fn connect(args: &Args) -> Result<Self> {
        let adb = adbutils::AdbClient::new("127.0.0.1", 5037, Some(Duration::from_secs(10)))
            .device(&args.serial);
        Ok(match args.candidate {
            Candidate::AdbClient => {
                Self::AdbClient(ADBServerDevice::new(args.serial.clone(), None))
            },
            Candidate::Adbutils => Self::Adbutils(adb),
            Candidate::Uiautomator => {
                let settings = Arc::new(RwLock::new(uiautomator::Settings {
                    max_retry: args.attempts,
                    http_timeout: Duration::from_secs(10),
                    ..Default::default()
                }));
                let adb = Arc::new(uiautomator::AdbClient::new().await?);
                let client = if let Some(url) = &args.rpc_url {
                    uiautomator::JsonRpcClient::new_direct_with_rpc_url(
                        args.serial.clone(),
                        adb,
                        settings,
                        url.clone(),
                    )?
                } else {
                    uiautomator::JsonRpcClient::new_direct(args.serial.clone(), adb, settings)
                        .await?
                };
                Self::Uiautomator(client)
            },
            Candidate::Uiautomator2 => Self::Uiautomator2(
                uiautomator2::connect_server(adb, args.port).await?,
                args.single_attempt,
            ),
        })
    }

    async fn command(&mut self, request: &Value) -> Result<Value> {
        match text(request, "operation")? {
            "shell" => {
                let command = text(request, "command")?;
                match self {
                    Self::AdbClient(device) => {
                        let mut stdout = Limited(Vec::new());
                        let mut stderr = Limited(Vec::new());
                        let exit =
                            device.shell_command(&command, Some(&mut stdout), Some(&mut stderr))?;
                        Ok(json!({"stdout":String::from_utf8(stdout.0)?,
                            "stderr":String::from_utf8(stderr.0)?,"exit":exit}))
                    },
                    Self::Adbutils(device) => {
                        let result = device.shell2(command, true).await?;
                        Ok(
                            json!({"stdout":result.stdout,"stderr":result.stderr,"exit":result.returncode}),
                        )
                    },
                    _ => bail!("shell is an ADB probe operation"),
                }
            },
            "install" => {
                let path = text(request, "path")?;
                match self {
                    Self::AdbClient(device) => device.install(Path::new(path), None)?,
                    Self::Adbutils(device) => {
                        device.install(path, true, false, &["-r", "-t"]).await?
                    },
                    _ => bail!("install is an ADB probe operation"),
                }
                Ok(json!(true))
            },
            "forward" => {
                let local = text(request, "local")?;
                let remote = text(request, "remote")?;
                match self {
                    Self::AdbClient(device) => device.forward(remote.into(), local.into())?,
                    Self::Adbutils(device) => device.forward(local, remote, true).await?,
                    _ => bail!("forward is an ADB probe operation"),
                }
                Ok(json!({"allocated_port":null}))
            },
            "remove_forward" => {
                let local = text(request, "local")?;
                match self {
                    Self::AdbClient(device) => device.forward_remove(local.into())?,
                    Self::Adbutils(device) => device.forward_remove(local, true).await?,
                    _ => bail!("remove_forward is an ADB probe operation"),
                }
                Ok(json!(true))
            },
            "wire" => {
                let Self::Adbutils(device) = self else {
                    bail!("raw device socket is an adbutils probe");
                };
                let mut socket = device
                    .create_connection(adbutils::Network::Tcp, "28437")
                    .await?;
                let body = serde_json::to_vec(&request["request"])?;
                ensure!(body.len() <= 1024 * 1024, "request too large");
                socket.write_u32(body.len().try_into()?).await?;
                socket.write_all(&body).await?;
                let size = socket.read_u32().await?;
                ensure!(
                    (1..=1024 * 1024).contains(&size),
                    "invalid wire response length"
                );
                let mut body = vec![0; size as usize];
                socket.read_exact(&mut body).await?;
                Ok(serde_json::from_slice(&body)?)
            },
            "screenshot" => {
                let path = text(request, "path")?;
                let image = match self {
                    Self::AdbClient(device) => {
                        let mut stdout = Limited(Vec::new());
                        let mut stderr = Limited(Vec::new());
                        let exit = device.shell_command(
                            &"screencap -p",
                            Some(&mut stdout),
                            Some(&mut stderr),
                        )?;
                        ensure!(
                            exit.is_none_or(|exit| exit == 0),
                            "screencap failed: {exit:?}"
                        );
                        ensure!(stderr.0.is_empty(), "screencap reported stderr");
                        image::load_from_memory(&stdout.0)?.to_rgb8()
                    },
                    Self::Adbutils(device) => device.screenshot(None, false).await?,
                    _ => bail!("screenshot is an ADB probe operation"),
                };
                image.save(path)?;
                Ok(json!({"width":image.width(),"height":image.height()}))
            },
            "rpc" => {
                let method = text(request, "method")?;
                let params = request["params"].clone();
                match self {
                    Self::Uiautomator(client) => Ok(client.call::<Value>(method, params).await?),
                    Self::Uiautomator2(server, single_attempt) => {
                        if *single_attempt {
                            Ok(uiautomator2::jsonrpc::jsonrpc_call(
                                server.device(),
                                server.port(),
                                method,
                                params,
                                Duration::from_secs(10),
                            )
                            .await?)
                        } else {
                            Ok(server
                                .jsonrpc_call(method, params, Duration::from_secs(10))
                                .await?)
                        }
                    },
                    _ => bail!("JSON-RPC is a UI Automator probe operation"),
                }
            },
            _ => bail!("unknown operation"),
        }
    }

    async fn close(self) -> Result<()> {
        if let Self::Uiautomator2(server, _) = self {
            server.stop().await?;
        }
        Ok(())
    }
}

fn reply(result: Result<Value>) -> Result<()> {
    let value = match result {
        Ok(value) => json!({"ok": value}),
        Err(error) => json!({"error":format!("{error:#}")}),
    };
    println!("{value}");
    io::stdout().flush()?;
    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();
    uiautomator::init_logger();
    let mut probe = Probe::connect(&args).await?;
    reply(Ok(json!({"ready":true})))?;
    for line in io::stdin().lock().lines() {
        let line = line?;
        ensure!(line.len() <= 1024 * 1024, "driver request exceeds 1 MiB");
        let request = serde_json::from_str(&line)?;
        reply(probe.command(&request).await)?;
    }
    probe.close().await
}
