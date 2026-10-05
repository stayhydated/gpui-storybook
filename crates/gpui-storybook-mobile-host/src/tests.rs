//! Protocol peers exercise public transport outcomes and the real device lease.
use super::*;
use gpui_storybook_mobile::{DeviceEndpoint, MutationLease};
use std::sync::atomic::{AtomicBool, AtomicUsize};
use tokio::{
    io::{AsyncReadExt as _, AsyncWriteExt as _},
    net::{TcpListener, TcpStream},
    sync::{Semaphore, mpsc},
};

fn host(session: String) -> HostDescriptor {
    HostDescriptor::builder()
        .protocol_version(PROTOCOL_VERSION)
        .session(session)
        .surface_revision(1)
        .route_revision(1)
        .active_route("counter".to_owned())
        .native_route("counter".to_owned())
        .dark(false)
        .orientation(DisplayOrientation::Portrait)
        .geometry(
            SurfaceGeometry::builder()
                .x(0)
                .y(1)
                .width(2)
                .height(2)
                .scale(1.)
                .display_width(2)
                .display_height(3)
                .build(),
        )
        .capabilities(AutomationCapabilities::new([
            AutomationCapability::HostDiscovery,
            AutomationCapability::DisplayCapture,
        ]))
        .build()
}

struct Device {
    port: u16,
    gate: gpui_storybook_mobile::OperationGate,
    mutations: Arc<AtomicUsize>,
    finished: Arc<AtomicUsize>,
    stop: Arc<AtomicBool>,
    owner: Option<std::thread::JoinHandle<()>>,
}
impl Device {
    fn start() -> Self {
        let endpoint = DeviceEndpoint::listen(0, "surface-1".to_owned()).unwrap();
        let gate = endpoint.gate();
        let port = endpoint.port();
        let mutations = Arc::new(AtomicUsize::new(0));
        let finished = Arc::new(AtomicUsize::new(0));
        let stop = Arc::new(AtomicBool::new(false));
        let (count, ends, stopped) = (mutations.clone(), finished.clone(), stop.clone());
        let owner = std::thread::spawn(move || {
            let mut capture: Option<(u64, Option<MutationLease>)> = None;
            while !stopped.load(Ordering::Acquire) {
                if let Some(admitted) = endpoint.try_recv() {
                    let (request, response, lease) = admitted.into_parts();
                    let descriptor = host(endpoint.session());
                    let result = match request.command() {
                        DeviceOperation::PrepareCapture {} => {
                            capture = Some((request.request_id(), lease));
                            DeviceResult::CaptureTicket {
                                ticket: request.request_id(),
                                host: descriptor,
                            }
                        },
                        DeviceOperation::FinishCapture { ticket } => {
                            assert_eq!(capture.as_ref().map(|capture| capture.0), Some(*ticket));
                            capture.take();
                            ends.fetch_add(1, Ordering::AcqRel);
                            DeviceResult::Host(descriptor)
                        },
                        command => {
                            if command.mutates() {
                                count.fetch_add(1, Ordering::AcqRel);
                            }
                            drop(lease);
                            DeviceResult::Host(descriptor)
                        },
                    };
                    let _ = response.send(Ok(result));
                } else {
                    std::thread::yield_now();
                }
            }
        });
        Self {
            port,
            gate,
            mutations,
            finished,
            stop,
            owner: Some(owner),
        }
    }
}
impl Drop for Device {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        self.owner.take().unwrap().join().unwrap();
    }
}

#[derive(Clone, Copy)]
enum Fault {
    None,
    LostReply,
    OversizedReply,
    WrongIdentity,
    HoldReply,
    WrongDescriptor,
    ChangedCapabilities,
}
struct Gateway {
    address: std::net::SocketAddr,
    captures: mpsc::UnboundedReceiver<()>,
    release: Arc<Semaphore>,
    owner: tokio::task::JoinHandle<()>,
}
impl Drop for Gateway {
    fn drop(&mut self) {
        self.owner.abort();
    }
}
async fn adb_command(stream: &mut TcpStream) -> String {
    let mut size = [0; 4];
    stream.read_exact(&mut size).await.unwrap();
    let size = usize::from_str_radix(std::str::from_utf8(&size).unwrap(), 16).unwrap();
    let mut command = vec![0; size];
    stream.read_exact(&mut command).await.unwrap();
    String::from_utf8(command).unwrap()
}
async fn gateway(port: u16, fault: Fault, png: Vec<u8>) -> Gateway {
    let listener = TcpListener::bind(("127.0.0.1", 0)).await.unwrap();
    let address = listener.local_addr().unwrap();
    let (captured, captures) = mpsc::unbounded_channel();
    let release = Arc::new(Semaphore::new(0));
    let permits = release.clone();
    let owner = tokio::spawn(async move {
        loop {
            let (mut stream, _) = listener.accept().await.unwrap();
            let (png, captured, permits) = (png.clone(), captured.clone(), permits.clone());
            tokio::spawn(async move {
                let command = adb_command(&mut stream).await;
                if command == "host:version" {
                    stream.write_all(b"OKAY00040029").await.unwrap();
                    return;
                }
                assert_eq!(command, "host:tport:serial:qualification-device");
                stream.write_all(b"OKAY").await.unwrap();
                stream.write_u64_le(1).await.unwrap();
                let command = adb_command(&mut stream).await;
                stream.write_all(b"OKAY").await.unwrap();
                if command == "exec:screencap -p" {
                    captured.send(()).unwrap();
                    permits.acquire().await.unwrap().forget();
                    let _ = stream.write_all(&png).await;
                } else if command.starts_with("shell,v2,raw:") {
                    for (channel, data) in [
                        (1u8, b"output".as_slice()),
                        (2, b"error".as_slice()),
                        (3, &[7]),
                    ] {
                        stream.write_u8(channel).await.unwrap();
                        stream.write_u32_le(data.len() as u32).await.unwrap();
                        stream.write_all(data).await.unwrap();
                    }
                } else {
                    assert_eq!(command, format!("tcp:{port}"));
                    let mut device = TcpStream::connect(("127.0.0.1", port)).await.unwrap();
                    let size = stream.read_u32().await.unwrap() as usize;
                    let mut body = vec![0; size];
                    stream.read_exact(&mut body).await.unwrap();
                    let request: DeviceRequest = serde_json::from_slice(&body).unwrap();
                    device.write_u32(size as u32).await.unwrap();
                    device.write_all(&body).await.unwrap();
                    let size = device.read_u32().await.unwrap() as usize;
                    let mut body = vec![0; size];
                    device.read_exact(&mut body).await.unwrap();
                    if request.command().mutates() {
                        match fault {
                            Fault::LostReply => return,
                            Fault::OversizedReply => {
                                let _ = stream.write_u32(MAX_WIRE_BYTES as u32 + 1).await;
                                return;
                            },
                            Fault::WrongIdentity => {
                                let response: DeviceResponse =
                                    serde_json::from_slice(&body).unwrap();
                                body = serde_json::to_vec(
                                    &DeviceResponse::builder()
                                        .protocol_version(PROTOCOL_VERSION)
                                        .session("foreign-surface".to_owned())
                                        .request_id(response.request_id() + 1)
                                        .outcome(response.into_outcome())
                                        .build(),
                                )
                                .unwrap();
                            },
                            Fault::HoldReply => {
                                captured.send(()).unwrap();
                                permits.acquire().await.unwrap().forget();
                            },
                            Fault::WrongDescriptor | Fault::ChangedCapabilities => {
                                let mut response: serde_json::Value =
                                    serde_json::from_slice(&body).unwrap();
                                let descriptor = &mut response["outcome"]["Ok"]["value"]["host"];
                                if matches!(fault, Fault::WrongDescriptor) {
                                    descriptor["session"] = serde_json::json!("foreign-descriptor");
                                } else {
                                    descriptor["capabilities"]
                                        .as_array_mut()
                                        .unwrap()
                                        .push(serde_json::json!("pointer"));
                                }
                                body = serde_json::to_vec(&response).unwrap();
                            },
                            Fault::None => (),
                        }
                    }
                    let _ = stream.write_u32(body.len() as u32).await;
                    let _ = stream.write_all(&body).await;
                }
            });
        }
    });
    Gateway {
        address,
        captures,
        release,
        owner,
    }
}
fn transport(gateway: &Gateway, timeout: std::time::Duration) -> AdbTransport {
    AdbTransport::new(
        AdbTransportOptions::builder()
            .serial("qualification-device".to_owned())
            .server(gateway.address)
            .timeout(timeout)
            .build(),
    )
    .unwrap()
}

#[tokio::test]
async fn lost_and_oversized_mutation_replies_report_unknown_without_replay() {
    for fault in [Fault::LostReply, Fault::OversizedReply] {
        let device = Device::start();
        let gateway = gateway(device.port, fault, Vec::new()).await;
        let transport = transport(&gateway, std::time::Duration::from_secs(3));
        let result = transport
            .exchange(
                device.port,
                "surface-1",
                21,
                DeviceOperation::OpenStory {
                    key: "counter".to_owned(),
                },
            )
            .await;
        assert!(matches!(
            result,
            Err(StorybookAutomationError::OutcomeUnknown { request_id: 21, .. })
        ));
        transport
            .exchange(device.port, "surface-1", 22, DeviceOperation::GetHost {})
            .await
            .unwrap();
        assert_eq!(device.mutations.load(Ordering::Acquire), 1);
        assert!(!device.gate.busy());
    }
}
#[tokio::test]
async fn response_identity_and_shell_exit_status_are_preserved() {
    let device = Device::start();
    let gateway = gateway(device.port, Fault::WrongIdentity, Vec::new()).await;
    let transport = transport(&gateway, std::time::Duration::from_secs(3));
    assert!(matches!(
        transport
            .exchange(
                device.port,
                "surface-1",
                31,
                DeviceOperation::OpenStory {
                    key: "counter".to_owned()
                }
            )
            .await,
        Err(StorybookAutomationError::StaleHost { .. })
    ));
    let output = transport
        .shell(&["sh", "-c", "printf output; printf error >&2; exit 7"])
        .await
        .unwrap();
    assert_eq!(
        (output.stdout(), output.stderr(), output.status()),
        (b"output".as_slice(), b"error".as_slice(), 7)
    );
    assert!(output.require_success().is_err());
    assert_eq!(device.mutations.load(Ordering::Acquire), 1);
}
#[tokio::test]
async fn response_deadline_reports_unknown_after_device_execution() {
    let device = Device::start();
    let mut gateway = gateway(device.port, Fault::HoldReply, Vec::new()).await;
    let transport = transport(&gateway, std::time::Duration::from_millis(200));
    let operation = tokio::spawn(async move {
        transport
            .exchange(
                device.port,
                "surface-1",
                41,
                DeviceOperation::OpenStory {
                    key: "counter".to_owned(),
                },
            )
            .await
    });
    gateway.captures.recv().await.unwrap();
    assert!(matches!(
        operation.await.unwrap(),
        Err(StorybookAutomationError::OutcomeUnknown { request_id: 41, .. })
    ));
    gateway.release.add_permits(1);
}
#[tokio::test]
async fn canceled_capture_and_provider_error_both_settle_the_device_ticket() {
    for valid in [true, false] {
        let device = Device::start();
        let mut png = std::io::Cursor::new(Vec::new());
        image::DynamicImage::ImageRgba8(image::RgbaImage::from_pixel(
            2,
            3,
            image::Rgba([20, 40, 60, 255]),
        ))
        .write_to(&mut png, image::ImageFormat::Png)
        .unwrap();
        let mut gateway = gateway(
            device.port,
            Fault::None,
            if valid {
                png.into_inner()
            } else {
                b"invalid PNG".to_vec()
            },
        )
        .await;
        let backend = RemoteBackend::attach_with_transport(
            transport(&gateway, std::time::Duration::from_secs(3)),
            device.port,
        )
        .await
        .unwrap();
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("capture.png");
        let caller_backend = backend.clone();
        let destination = path.clone();
        let caller = tokio::spawn(async move {
            caller_backend
                .capture_host(HostCaptureScope::Gpui, destination)
                .await
        });
        gateway.captures.recv().await.unwrap();
        assert!(device.gate.busy());
        if valid {
            caller.abort();
            assert!(caller.await.unwrap_err().is_cancelled());
            gateway.release.add_permits(1);
        } else {
            gateway.release.add_permits(1);
            assert!(caller.await.unwrap().is_err());
        }
        tokio::time::timeout(std::time::Duration::from_secs(3), backend.shutdown())
            .await
            .unwrap();
        assert_eq!(device.finished.load(Ordering::Acquire), 1);
        assert!(!device.gate.busy());
        if valid {
            let image = image::open(path).unwrap();
            assert_eq!((image.width(), image.height()), (2, 2));
        } else {
            assert!(!path.exists());
        }
    }
}

#[tokio::test]
async fn canceled_install_finishes_owned_file_cleanup_before_shutdown_returns() {
    let listener = TcpListener::bind(("127.0.0.1", 0)).await.unwrap();
    let address = listener.local_addr().unwrap();
    let remote_directory = tempfile::tempdir().unwrap();
    let remote = remote_directory.path().join("uploaded.apk");
    let observed = remote.clone();
    let release = Arc::new(Semaphore::new(0));
    let gate = release.clone();
    let (uploaded, mut uploads) = mpsc::unbounded_channel();
    let owner = tokio::spawn(async move {
        loop {
            let (mut stream, _) = listener.accept().await.unwrap();
            let (observed, uploaded, gate) = (observed.clone(), uploaded.clone(), gate.clone());
            tokio::spawn(async move {
                if adb_command(&mut stream).await == "host:version" {
                    stream.write_all(b"OKAY00040029").await.unwrap();
                    return;
                }
                stream.write_all(b"OKAY").await.unwrap();
                stream.write_u64_le(1).await.unwrap();
                let service = adb_command(&mut stream).await;
                stream.write_all(b"OKAY").await.unwrap();
                if service == "sync:" {
                    let mut tag = [0; 4];
                    stream.read_exact(&mut tag).await.unwrap();
                    assert_eq!(&tag, b"SEND");
                    let size = stream.read_u32_le().await.unwrap() as usize;
                    let mut destination = vec![0; size];
                    stream.read_exact(&mut destination).await.unwrap();
                    assert!(
                        std::str::from_utf8(&destination)
                            .unwrap()
                            .starts_with("/data/local/tmp/storybook-")
                    );
                    let mut bytes = Vec::new();
                    loop {
                        stream.read_exact(&mut tag).await.unwrap();
                        let size = stream.read_u32_le().await.unwrap() as usize;
                        if &tag == b"DONE" {
                            break;
                        }
                        assert_eq!(&tag, b"DATA");
                        assert!(size <= 64 * 1024);
                        let offset = bytes.len();
                        bytes.resize(offset + size, 0);
                        stream.read_exact(&mut bytes[offset..]).await.unwrap();
                    }
                    std::fs::write(&observed, bytes).unwrap();
                    uploaded.send(()).unwrap();
                    gate.acquire().await.unwrap().forget();
                    stream.write_all(b"OKAY\0\0\0\0").await.unwrap();
                } else {
                    assert!(service.starts_with("shell,v2,raw:"));
                    let status = if service.starts_with("shell,v2,raw:rm -f ") {
                        std::fs::remove_file(observed).unwrap();
                        0
                    } else {
                        assert!(service.starts_with("shell,v2,raw:pm install -r -t "));
                        1 // Package-manager failure still requires owned artifact cleanup.
                    };
                    stream.write_all(&[3, 1, 0, 0, 0, status]).await.unwrap();
                }
            });
        }
    });
    let transport = AdbTransport::new(
        AdbTransportOptions::builder()
            .serial("qualification-device".to_owned())
            .server(address)
            .build(),
    )
    .unwrap();
    let local = tempfile::tempdir().unwrap();
    let path = local.path().join("application.apk");
    let contents = vec![0x59; 128 * 1024 + 17];
    std::fs::write(&path, &contents).unwrap();
    let caller_transport = transport.clone();
    let source = path.clone();
    let caller = tokio::spawn(async move { caller_transport.install(source).await });
    uploads.recv().await.unwrap();
    assert_eq!(std::fs::read(&remote).unwrap(), contents);
    caller.abort();
    assert!(caller.await.unwrap_err().is_cancelled());
    assert!(
        remote.exists(),
        "queued completion still owns the uploaded artifact"
    );
    release.add_permits(1);
    tokio::time::timeout(std::time::Duration::from_secs(3), transport.shutdown())
        .await
        .unwrap();
    assert!(!remote.exists());
    assert!(
        transport.install(path).await.is_err(),
        "shutdown closes install admission"
    );
    owner.abort();
}

#[tokio::test]
async fn lost_prepare_reply_releases_the_ticket_without_writing_a_capture() {
    let device = Device::start();
    let gateway = gateway(device.port, Fault::LostReply, Vec::new()).await;
    let backend = RemoteBackend::attach_with_transport(
        transport(&gateway, std::time::Duration::from_secs(3)),
        device.port,
    )
    .await
    .unwrap();
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("unknown.png");
    assert!(matches!(
        backend
            .capture_host(HostCaptureScope::Display, path.clone())
            .await,
        Err(StorybookAutomationError::OutcomeUnknown { .. })
    ));
    backend.shutdown().await;
    assert_eq!(device.finished.load(Ordering::Acquire), 1);
    assert!(!device.gate.busy());
    assert!(!path.exists());
}

#[tokio::test]
async fn inconsistent_capture_metadata_releases_ownership_before_observation() {
    for fault in [Fault::WrongDescriptor, Fault::ChangedCapabilities] {
        let device = Device::start();
        let gateway = gateway(device.port, fault, Vec::new()).await;
        let backend = RemoteBackend::attach_with_transport(
            transport(&gateway, std::time::Duration::from_secs(3)),
            device.port,
        )
        .await
        .unwrap();
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("inconsistent.png");
        assert!(matches!(
            backend
                .capture_host(HostCaptureScope::Display, path.clone())
                .await,
            Err(StorybookAutomationError::StaleHost { .. })
        ));
        backend.shutdown().await;
        assert_eq!(device.finished.load(Ordering::Acquire), 1);
        assert!(!device.gate.busy());
        assert!(!path.exists());
    }
}
