//! Device admission and bounded loopback transport for an opted-in application.
//!
//! The application polls [`DeviceEndpoint::try_recv`] on its existing owner
//! thread. Each admitted mutation carries a device-owned lease until completion.
//! The transport waits on a bounded response channel; disconnection never cancels
//! or replays an admitted operation. Surface replacement invalidates its session.
//! Callers assign unique IDs per session. The last 4096 admitted IDs are rejected
//! on resubmission; eviction permits bounded memory, never automatic replay.

use gpui_storybook_automation::{StorybookAutomationError, wire::*};
use std::{
    collections::{BTreeSet, VecDeque},
    io,
    net::{TcpListener, TcpStream},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicUsize, Ordering},
        mpsc::{self, Receiver, SyncSender},
    },
    time::Duration,
};

pub const DEVICE_PORT: u16 = 28437;
const QUEUE_CAPACITY: usize = 32;
const MAX_CONNECTIONS: usize = 4;
const RECEIPT_CAPACITY: usize = 4096;

#[derive(Default)]
struct Receipts {
    ids: BTreeSet<u64>,
    order: VecDeque<u64>,
}
impl Receipts {
    fn insert(&mut self, id: u64) {
        self.ids.insert(id);
        self.order.push_back(id);
        if self.order.len() > RECEIPT_CAPACITY {
            self.ids
                .remove(&self.order.pop_front().expect("full receipt window"));
        }
    }
}

#[derive(Default)]
struct GateState {
    active: Option<u64>,
    next: u64,
}

#[derive(Clone, Default)]
pub struct OperationGate(Arc<Mutex<GateState>>);
impl OperationGate {
    pub fn acquire(&self) -> Result<MutationLease, StorybookAutomationError> {
        let mut state = self.0.lock().expect("operation gate");
        if state.active.is_some() {
            return Err(StorybookAutomationError::AutomationBusy);
        }
        state.next = state.next.checked_add(1).expect("operation IDs exhausted");
        let token = state.next;
        state.active = Some(token);
        Ok(MutationLease {
            gate: self.clone(),
            token,
        })
    }
    pub fn invalidate(&self) {
        self.0.lock().expect("operation gate").active = None;
    }
    pub fn busy(&self) -> bool {
        self.0.lock().expect("operation gate").active.is_some()
    }
}

pub struct MutationLease {
    gate: OperationGate,
    token: u64,
}
impl Drop for MutationLease {
    fn drop(&mut self) {
        let mut state = self.gate.0.lock().expect("operation gate");
        if state.active == Some(self.token) {
            state.active = None;
        }
    }
}

pub struct AdmittedRequest {
    request: DeviceRequest,
    response: SyncSender<Result<DeviceResult, StorybookAutomationError>>,
    lease: Option<MutationLease>,
}
impl AdmittedRequest {
    pub fn request(&self) -> &DeviceRequest {
        &self.request
    }
    pub fn into_parts(
        self,
    ) -> (
        DeviceRequest,
        SyncSender<Result<DeviceResult, StorybookAutomationError>>,
        Option<MutationLease>,
    ) {
        (self.request, self.response, self.lease)
    }
}

struct SharedEndpoint {
    session: Mutex<String>,
    gate: OperationGate,
    sender: SyncSender<AdmittedRequest>,
    connections: AtomicUsize,
    receipts: Mutex<Receipts>,
    closed: AtomicBool,
}

pub struct DeviceEndpoint {
    shared: Arc<SharedEndpoint>,
    receiver: Receiver<AdmittedRequest>,
    port: u16,
}
impl DeviceEndpoint {
    /// Bind only IPv4 loopback. Call only after application build/runtime opt-in.
    pub fn listen(port: u16, session: String) -> io::Result<Self> {
        let listener = TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, port))?;
        let port = listener.local_addr()?.port();
        listener.set_nonblocking(true)?;
        let (sender, receiver) = mpsc::sync_channel(QUEUE_CAPACITY);
        let shared = Arc::new(SharedEndpoint {
            session: Mutex::new(session),
            gate: OperationGate::default(),
            sender,
            connections: AtomicUsize::new(0),
            receipts: Mutex::default(),
            closed: AtomicBool::new(false),
        });
        let service = shared.clone();
        std::thread::Builder::new()
            .name("storybook-device-listener".to_owned())
            .spawn(move || {
                while !service.closed.load(Ordering::Acquire) {
                    let stream = match listener.accept() {
                        Ok((stream, _)) => stream,
                        Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                            std::thread::sleep(Duration::from_millis(10));
                            continue;
                        },
                        Err(_) => break,
                    };
                    if service
                        .connections
                        .try_update(Ordering::AcqRel, Ordering::Acquire, |value| {
                            (value < MAX_CONNECTIONS).then_some(value + 1)
                        })
                        .is_err()
                    {
                        continue;
                    }
                    let service = service.clone();
                    let connection = ConnectionCount(service.clone());
                    if let Err(error) = std::thread::Builder::new()
                        .name("storybook-device-connection".to_owned())
                        .spawn(move || {
                            let _connection = connection;
                            let _ = serve_connection(stream, &service);
                        })
                    {
                        eprintln!("device connection spawn failed: {error}");
                    }
                }
            })?;
        Ok(Self {
            shared,
            receiver,
            port,
        })
    }
    pub fn port(&self) -> u16 {
        self.port
    }
    pub fn try_recv(&self) -> Option<AdmittedRequest> {
        self.receiver.try_recv().ok()
    }
    pub fn gate(&self) -> OperationGate {
        self.shared.gate.clone()
    }
    pub fn replace_session(&self, session: String) {
        *self.shared.session.lock().expect("device session") = session;
        *self.shared.receipts.lock().expect("request receipts") = Receipts::default();
        self.shared.gate.invalidate();
    }
    pub fn session(&self) -> String {
        self.shared.session.lock().expect("device session").clone()
    }
}

impl Drop for DeviceEndpoint {
    fn drop(&mut self) {
        self.shared.closed.store(true, Ordering::Release);
    }
}

struct ConnectionCount(Arc<SharedEndpoint>);
impl Drop for ConnectionCount {
    fn drop(&mut self) {
        self.0.connections.fetch_sub(1, Ordering::AcqRel);
    }
}

fn serve_connection(mut stream: TcpStream, shared: &SharedEndpoint) -> io::Result<()> {
    stream.set_read_timeout(Some(Duration::from_secs(30)))?;
    stream.set_write_timeout(Some(Duration::from_secs(5)))?;
    loop {
        let request: DeviceRequest = read_frame(&mut stream)?;
        let session = shared.session.lock().expect("device session").clone();
        let request_id = request.request_id();
        let (response, receiver) = mpsc::sync_channel(1);
        let admission = request.validate_identity(&session).and_then(|()| {
            let mut receipts = shared.receipts.lock().expect("request receipts");
            if receipts.ids.contains(&request_id) {
                return Err(StorybookAutomationError::StaleHost {
                    message: "request ID was already admitted; rediscover without replay"
                        .to_owned(),
                });
            }
            let lease = if request.command().mutates() {
                Some(shared.gate.acquire()?)
            } else {
                None
            };
            shared
                .sender
                .try_send(AdmittedRequest {
                    request,
                    response,
                    lease,
                })
                .map_err(|_| StorybookAutomationError::AutomationBusy)?;
            receipts.insert(request_id);
            Ok(())
        });
        let outcome = match admission {
            Err(error) => Err(error),
            Ok(()) => receiver
                .recv_timeout(Duration::from_secs(25))
                .unwrap_or_else(|error| {
                    Err(StorybookAutomationError::OutcomeUnknown {
                        request_id,
                        message: error.to_string(),
                    })
                }),
        };
        write_frame(
            &mut stream,
            &DeviceResponse::builder()
                .protocol_version(PROTOCOL_VERSION)
                .session(session)
                .request_id(request_id)
                .outcome(outcome)
                .build(),
        )?;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn request(id: u64, version: u32, session: &str, command: DeviceOperation) -> DeviceRequest {
        DeviceRequest::builder()
            .protocol_version(version)
            .session(session.to_owned())
            .request_id(id)
            .command(command)
            .build()
    }
    fn connect(endpoint: &DeviceEndpoint) -> TcpStream {
        let stream = TcpStream::connect(("127.0.0.1", endpoint.port())).unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(3)))
            .unwrap();
        stream
    }
    fn admitted(endpoint: &DeviceEndpoint) -> AdmittedRequest {
        let deadline = std::time::Instant::now() + Duration::from_secs(3);
        loop {
            if let Some(request) = endpoint.try_recv() {
                return request;
            }
            assert!(
                std::time::Instant::now() < deadline,
                "request was not admitted"
            );
            std::thread::sleep(Duration::from_millis(1));
        }
    }
    #[test]
    fn real_endpoint_rejects_protocol_and_retains_disconnected_mutation_ownership() {
        let endpoint = DeviceEndpoint::listen(0, "surface-1".to_owned()).unwrap();
        let mut invalid = connect(&endpoint);
        write_frame(
            &mut invalid,
            &request(
                1,
                PROTOCOL_VERSION + 1,
                "surface-1",
                DeviceOperation::OpenStory {
                    key: "counter".to_owned(),
                },
            ),
        )
        .unwrap();
        let response: DeviceResponse = read_frame(&mut invalid).unwrap();
        assert!(matches!(
            response.into_outcome(),
            Err(StorybookAutomationError::ProtocolMismatch { .. })
        ));
        assert!(!endpoint.gate().busy());
        assert!(endpoint.try_recv().is_none());
        drop(invalid);

        let mut first = connect(&endpoint);
        write_frame(
            &mut first,
            &request(
                2,
                PROTOCOL_VERSION,
                "surface-1",
                DeviceOperation::OpenStory {
                    key: "counter".to_owned(),
                },
            ),
        )
        .unwrap();
        let (_, response, lease) = admitted(&endpoint).into_parts();
        first.shutdown(std::net::Shutdown::Both).unwrap();
        drop(first);
        assert!(endpoint.gate().busy());
        let mut second = connect(&endpoint);
        write_frame(
            &mut second,
            &request(
                3,
                PROTOCOL_VERSION,
                "surface-1",
                DeviceOperation::OpenStory {
                    key: "notes".to_owned(),
                },
            ),
        )
        .unwrap();
        assert!(matches!(
            read_frame::<DeviceResponse>(&mut second)
                .unwrap()
                .into_outcome(),
            Err(StorybookAutomationError::AutomationBusy)
        ));
        write_frame(
            &mut second,
            &request(
                2,
                PROTOCOL_VERSION,
                "surface-1",
                DeviceOperation::OpenStory {
                    key: "counter".to_owned(),
                },
            ),
        )
        .unwrap();
        assert!(matches!(
            read_frame::<DeviceResponse>(&mut second)
                .unwrap()
                .into_outcome(),
            Err(StorybookAutomationError::StaleHost { .. })
        ));
        assert!(endpoint.try_recv().is_none());
        response
            .send(Err(StorybookAutomationError::InteractionFailed {
                request_id: 2,
                steps_dispatched: 1,
                message: "owner settled".to_owned(),
            }))
            .unwrap();
        drop(lease);
        assert!(!endpoint.gate().busy());
        endpoint.replace_session("surface-2".to_owned());
        write_frame(
            &mut second,
            &request(
                4,
                PROTOCOL_VERSION,
                "surface-1",
                DeviceOperation::OpenStory {
                    key: "counter".to_owned(),
                },
            ),
        )
        .unwrap();
        assert!(matches!(
            read_frame::<DeviceResponse>(&mut second)
                .unwrap()
                .into_outcome(),
            Err(StorybookAutomationError::StaleHost { .. })
        ));
        assert!(endpoint.try_recv().is_none());
    }
    #[test]
    fn malformed_and_oversized_frames_close_before_admission() {
        use std::io::{Read as _, Write as _};
        let endpoint = DeviceEndpoint::listen(0, "surface".to_owned()).unwrap();
        for body in [br#"{"protocol_version":1,"session":"surface","request_id":1,"command":{"operation":"get_host","unexpected":true}}"#.to_vec(), vec![]] {
            let mut stream = connect(&endpoint);
            let length = if body.is_empty() { MAX_WIRE_BYTES + 1 } else { body.len() };
            stream.write_all(&(length as u32).to_be_bytes()).unwrap(); stream.write_all(&body).unwrap();
            let mut byte = [0];
            assert_eq!(stream.read(&mut byte).unwrap(), 0);
            assert!(endpoint.try_recv().is_none()); assert!(!endpoint.gate().busy());
        }
    }

    #[test]
    fn invalidated_lease_cannot_release_a_new_operation() {
        let gate = OperationGate::default();
        let old = gate.acquire().unwrap();
        assert!(matches!(
            gate.acquire(),
            Err(StorybookAutomationError::AutomationBusy)
        ));
        gate.invalidate();
        let current = gate.acquire().unwrap();
        drop(old);
        assert!(gate.busy());
        drop(current);
        assert!(!gate.busy());
    }
}
