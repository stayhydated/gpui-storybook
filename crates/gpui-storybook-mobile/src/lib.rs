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
    collections::BTreeMap,
    io,
    net::{TcpListener, TcpStream},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Receiver, SyncSender},
    },
    time::{Duration, Instant},
};

mod gate;
use gate::GateState;
pub use gate::{MutationLease, MutationPermit, OperationGate};

pub const DEVICE_PORT: u16 = 28437;
const QUEUE_CAPACITY: usize = 32;
const MAX_CONNECTIONS: usize = 4;
const RECEIPT_CAPACITY: usize = 4096;

pub struct AdmittedRequest {
    request: DeviceRequest,
    response: SyncSender<Result<DeviceResult, StorybookAutomationError>>,
    lease: Option<MutationLease>,
}
impl AdmittedRequest {
    pub fn request(&self) -> &DeviceRequest {
        &self.request
    }
    pub fn permit(&self) -> Option<MutationPermit> {
        self.lease.as_ref().map(MutationLease::permit)
    }
    /// Report a response deadline once while retaining admitted work ownership.
    /// The owner still settles queued native work or explicitly invalidates its
    /// host before dropping this request; this report never authorizes replay.
    pub fn report_unknown(&self, message: impl Into<String>) {
        let _ = self
            .response
            .try_send(Err(StorybookAutomationError::OutcomeUnknown {
                request_id: self.request.request_id(),
                message: message.into(),
            }));
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
    gate: OperationGate,
    sender: SyncSender<AdmittedRequest>,
    connections: Mutex<Connections>,
    closed: AtomicBool,
    frame_timeout: Duration,
}

#[derive(Default)]
struct Connections {
    next: u64,
    streams: BTreeMap<u64, TcpStream>,
}

pub struct DeviceEndpoint {
    shared: Arc<SharedEndpoint>,
    receiver: Receiver<AdmittedRequest>,
    port: u16,
    listener: Option<std::thread::JoinHandle<Vec<std::thread::JoinHandle<()>>>>,
}
impl DeviceEndpoint {
    /// Bind only IPv4 loopback with a 30-second overall frame deadline. Supply a
    /// process-unique session seed of 1–107 bytes; this endpoint generates fresh
    /// generations on replacement. Drop closes sockets and joins transport threads.
    /// Call only after application build/runtime opt-in.
    pub fn listen(port: u16, session: String) -> io::Result<Self> {
        Self::listen_with_timeout(port, session, Duration::from_secs(30))
    }
    fn listen_with_timeout(
        port: u16,
        session: String,
        frame_timeout: Duration,
    ) -> io::Result<Self> {
        if session.is_empty() || session.len() > 107 || frame_timeout.is_zero() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "invalid session seed or frame deadline",
            ));
        }
        let listener = TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, port))?;
        let port = listener.local_addr()?.port();
        listener.set_nonblocking(true)?;
        let (sender, receiver) = mpsc::sync_channel(QUEUE_CAPACITY);
        let shared = Arc::new(SharedEndpoint {
            gate: OperationGate(Arc::new(Mutex::new(GateState::new(session)))),
            sender,
            connections: Mutex::new(Connections::default()),
            closed: AtomicBool::new(false),
            frame_timeout,
        });
        let service = shared.clone();
        let listener = std::thread::Builder::new()
            .name("storybook-device-listener".to_owned())
            .spawn(move || {
                let mut threads: Vec<std::thread::JoinHandle<()>> = Vec::new();
                while !service.closed.load(Ordering::Acquire) {
                    let mut ix = 0;
                    while ix < threads.len() {
                        if threads[ix].is_finished() {
                            let _ = threads.swap_remove(ix).join();
                        } else {
                            ix += 1;
                        }
                    }
                    let stream = match listener.accept() {
                        Ok((stream, _)) => stream,
                        Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                            std::thread::sleep(Duration::from_millis(10));
                            continue;
                        },
                        Err(_) => break,
                    };
                    // Registration and closing share a lock so shutdown cannot
                    // miss a socket accepted concurrently with endpoint drop.
                    let connection = {
                        let mut connections =
                            service.connections.lock().expect("device connections");
                        if service.closed.load(Ordering::Acquire)
                            || connections.streams.len() >= MAX_CONNECTIONS
                        {
                            continue;
                        }
                        let Ok(socket) = stream.try_clone() else {
                            continue;
                        };
                        connections.next += 1;
                        let id = connections.next;
                        connections.streams.insert(id, socket);
                        Connection {
                            shared: service.clone(),
                            id,
                        }
                    };
                    let service = service.clone();
                    match std::thread::Builder::new()
                        .name("storybook-device-connection".to_owned())
                        .spawn(move || {
                            let _connection = connection;
                            if let Err(error) = serve_connection(stream, &service) {
                                tracing::debug!(%error, "device connection closed");
                            }
                        }) {
                        Ok(thread) => threads.push(thread),
                        Err(error) => tracing::error!(%error, "device connection spawn failed"),
                    }
                }
                threads
            })?;
        Ok(Self {
            shared,
            receiver,
            port,
            listener: Some(listener),
        })
    }
    pub fn port(&self) -> u16 {
        self.port
    }
    pub fn try_recv(&self) -> Option<AdmittedRequest> {
        while let Ok(admitted) = self.receiver.try_recv() {
            let current = {
                let state = self.shared.gate.0.lock().expect("operation gate");
                !state.suspended
                    && admitted.request.validate_identity(&state.session).is_ok()
                    && admitted
                        .lease
                        .as_ref()
                        .is_none_or(|lease| state.active == Some(lease.token))
            };
            if current {
                return Some(admitted);
            }
            let _ = admitted
                .response
                .try_send(Err(StorybookAutomationError::StaleHost {
                    message: "owning host invalidated queued work".to_owned(),
                }));
        }
        None
    }
    pub fn gate(&self) -> OperationGate {
        self.shared.gate.clone()
    }
    /// Revoke work and atomically reopen admission with a never-reused session.
    /// Receipt clearing is tied to the endpoint-owned generation, independent of
    /// native revision values supplied by the application.
    pub fn replace_session(&self) {
        // Admission, receipts, and lease invalidation share one linearization
        // point. An old-session request cannot acquire a new-session lease.
        let mut state = self.shared.gate.0.lock().expect("operation gate");
        state.rotate();
        tracing::debug!(session = %state.session, "device session replaced");
    }
    pub fn session(&self) -> String {
        self.shared
            .gate
            .0
            .lock()
            .expect("operation gate")
            .session
            .clone()
    }
}

impl Drop for DeviceEndpoint {
    fn drop(&mut self) {
        {
            let connections = self.shared.connections.lock().expect("device connections");
            self.shared.closed.store(true, Ordering::Release);
            for stream in connections.streams.values() {
                let _ = stream.shutdown(std::net::Shutdown::Both);
            }
        }
        self.shared.gate.suspend();
        if let Some(listener) = self.listener.take()
            && let Ok(threads) = listener.join()
        {
            for thread in threads {
                let _ = thread.join();
            }
        }
    }
}

struct Connection {
    shared: Arc<SharedEndpoint>,
    id: u64,
}
impl Drop for Connection {
    fn drop(&mut self) {
        self.shared
            .connections
            .lock()
            .expect("device connections")
            .streams
            .remove(&self.id);
    }
}

struct DeadlineStream<'a> {
    stream: &'a mut TcpStream,
    deadline: Instant,
}
impl DeadlineStream<'_> {
    fn remaining(&self) -> io::Result<Duration> {
        self.deadline
            .checked_duration_since(Instant::now())
            .filter(|duration| !duration.is_zero())
            .ok_or_else(|| {
                io::Error::new(io::ErrorKind::TimedOut, "overall frame deadline exceeded")
            })
    }
}
impl io::Read for DeadlineStream<'_> {
    fn read(&mut self, bytes: &mut [u8]) -> io::Result<usize> {
        self.stream.set_read_timeout(Some(self.remaining()?))?;
        io::Read::read(self.stream, bytes)
    }
}
impl io::Write for DeadlineStream<'_> {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.stream.set_write_timeout(Some(self.remaining()?))?;
        io::Write::write(self.stream, bytes)
    }
    fn flush(&mut self) -> io::Result<()> {
        io::Write::flush(self.stream)
    }
}

fn serve_connection(mut stream: TcpStream, shared: &SharedEndpoint) -> io::Result<()> {
    while !shared.closed.load(Ordering::Acquire) {
        let request: DeviceRequest = read_frame(&mut DeadlineStream {
            stream: &mut stream,
            deadline: Instant::now() + shared.frame_timeout,
        })?;
        let request_id = request.request_id();
        let started = Instant::now();
        let (response, receiver) = mpsc::sync_channel(1);
        let mut rejected_lease = None;
        let (session, admission) = {
            let mut state = shared.gate.0.lock().expect("operation gate");
            let session = state.session.clone();
            let admission = request.validate_identity(&session).and_then(|()| {
                if shared.closed.load(Ordering::Acquire) || state.suspended {
                    return Err(StorybookAutomationError::NoLiveHost);
                }
                if state.receipts.ids.contains(&request_id) {
                    return Err(StorybookAutomationError::StaleHost {
                        message: "request ID was already admitted; rediscover without replay"
                            .to_owned(),
                    });
                }
                let lease = if request.command().mutates() {
                    Some(shared.gate.lease(state.acquire_token()?))
                } else {
                    None
                };
                // Retain a failed enqueue's lease until after the lock is released:
                // its Drop must reacquire this same gate.
                match shared.sender.try_send(AdmittedRequest {
                    request,
                    response,
                    lease,
                }) {
                    Ok(()) => {
                        state.receipts.insert(request_id);
                        Ok(())
                    },
                    Err(mut rejected) => {
                        // Take the rejected lease out before dropping its request;
                        // release it after leaving the admission lock.
                        rejected_lease = match &mut rejected {
                            mpsc::TrySendError::Full(request)
                            | mpsc::TrySendError::Disconnected(request) => request.lease.take(),
                        };
                        if rejected_lease.is_some() {
                            state.active = None;
                        }
                        Err(StorybookAutomationError::AutomationBusy)
                    },
                }
            });
            (session, admission)
        };
        drop(rejected_lease);
        let outcome = match admission {
            Err(error) => Err(error),
            Ok(()) => {
                let deadline = Instant::now() + Duration::from_secs(25);
                loop {
                    if shared.closed.load(Ordering::Acquire) {
                        return Ok(());
                    }
                    let remaining = deadline.saturating_duration_since(Instant::now());
                    match receiver.recv_timeout(remaining.min(Duration::from_millis(50))) {
                        Ok(outcome) => break outcome,
                        Err(mpsc::RecvTimeoutError::Timeout) if !remaining.is_zero() => continue,
                        Err(error) => {
                            break Err(StorybookAutomationError::OutcomeUnknown {
                                request_id,
                                message: error.to_string(),
                            });
                        },
                    }
                }
            },
        };
        tracing::debug!(request_id, %session, elapsed_ms = started.elapsed().as_millis() as u64, success = outcome.is_ok(), "device response settled");
        write_frame(
            &mut DeadlineStream {
                stream: &mut stream,
                deadline: Instant::now() + Duration::from_secs(5),
            },
            &DeviceResponse::builder()
                .protocol_version(PROTOCOL_VERSION)
                .session(session)
                .request_id(request_id)
                .outcome(outcome)
                .build(),
        )?;
    }
    Ok(())
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
        endpoint.replace_session();
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
    fn trickled_frames_expire_without_admission() {
        use std::io::{Read as _, Write as _};
        for partial_prefix in [false, true] {
            let endpoint = DeviceEndpoint::listen_with_timeout(
                0,
                "process".to_owned(),
                Duration::from_millis(150),
            )
            .unwrap();
            let mut stream = connect(&endpoint);
            if !partial_prefix {
                stream.write_all(&1000u32.to_be_bytes()).unwrap();
            }
            let mut writer = stream.try_clone().unwrap();
            let started = Instant::now();
            let sender = std::thread::spawn(move || {
                let prefix = 1000u32.to_be_bytes();
                let bytes = prefix
                    .into_iter()
                    .skip(if partial_prefix { 0 } else { 4 })
                    .chain(std::iter::repeat(b' '));
                for byte in bytes {
                    if writer.write_all(&[byte]).is_err() {
                        break;
                    }
                    if started.elapsed() > Duration::from_secs(3) {
                        break;
                    }
                    std::thread::sleep(Duration::from_millis(10));
                }
            });
            let result = stream.read(&mut [0]);
            assert!(
                matches!(result, Ok(0))
                    || result.is_err_and(|error| error.kind() == io::ErrorKind::ConnectionReset)
            );
            sender.join().unwrap();
            assert!(started.elapsed() < Duration::from_secs(2));
            assert!(endpoint.try_recv().is_none());
            assert!(!endpoint.gate().busy());
        }
    }

    #[test]
    fn drop_closes_incomplete_frames_and_waiting_responses() {
        use std::io::{Read as _, Write as _};
        let endpoint = DeviceEndpoint::listen(0, "process".to_owned()).unwrap();
        let shared = endpoint.shared.clone();
        let mut partial = connect(&endpoint);
        partial.write_all(&100u32.to_be_bytes()).unwrap();
        partial.write_all(b"{").unwrap();
        let mut waiting = connect(&endpoint);
        write_frame(
            &mut waiting,
            &request(
                81,
                PROTOCOL_VERSION,
                "process",
                DeviceOperation::OpenStory {
                    key: "counter".to_owned(),
                },
            ),
        )
        .unwrap();
        let owned = admitted(&endpoint);
        let permit = owned.permit().unwrap();
        drop(endpoint);
        assert!(!permit.is_current());
        assert!(shared.connections.lock().unwrap().streams.is_empty());
        for mut socket in [partial, waiting] {
            let result = socket.read(&mut [0]);
            assert!(
                matches!(result, Ok(0))
                    || result.is_err_and(|error| error.kind() == io::ErrorKind::ConnectionReset)
            );
        }
        drop(owned);
        assert_eq!(
            shared.gate.acquire().err(),
            Some(StorybookAutomationError::NoLiveHost)
        );
    }

    #[test]
    fn invalidated_lease_cannot_release_a_new_operation() {
        let gate = OperationGate::default();
        let old = gate.acquire().unwrap();
        assert!(old.is_current());
        assert!(matches!(
            gate.acquire(),
            Err(StorybookAutomationError::AutomationBusy)
        ));
        gate.invalidate();
        assert!(!old.is_current());
        let current = gate.acquire().unwrap();
        assert!(current.is_current());
        drop(old);
        assert!(gate.busy());
        drop(current);
        assert!(!gate.busy());
    }

    #[test]
    fn replacement_rejects_queued_old_work_and_preserves_the_new_lease() {
        let endpoint = DeviceEndpoint::listen(0, "surface-1".to_owned()).unwrap();
        let mut old = connect(&endpoint);
        write_frame(
            &mut old,
            &request(
                10,
                PROTOCOL_VERSION,
                "surface-1",
                DeviceOperation::OpenStory {
                    key: "counter".to_owned(),
                },
            ),
        )
        .unwrap();
        let deadline = std::time::Instant::now() + Duration::from_secs(3);
        while !endpoint.gate().busy() {
            assert!(std::time::Instant::now() < deadline, "old work admission");
            std::thread::yield_now();
        }
        endpoint.replace_session();
        let mut current = connect(&endpoint);
        // A receipt belongs to its session; the new host can admit the same ID.
        write_frame(
            &mut current,
            &request(
                10,
                PROTOCOL_VERSION,
                "surface-1.1",
                DeviceOperation::OpenStory {
                    key: "notes".to_owned(),
                },
            ),
        )
        .unwrap();
        let (request, response, lease) = admitted(&endpoint).into_parts();
        assert_eq!(request.session(), "surface-1.1");
        assert!(lease.as_ref().unwrap().is_current());
        assert!(endpoint.gate().busy());
        assert!(matches!(
            read_frame::<DeviceResponse>(&mut old)
                .unwrap()
                .into_outcome(),
            Err(StorybookAutomationError::StaleHost { .. })
        ));
        assert!(endpoint.try_recv().is_none());
        assert!(
            endpoint.gate().busy(),
            "discarding old work keeps the current owner"
        );
        response
            .send(Err(StorybookAutomationError::NoLiveHost))
            .unwrap();
        drop(lease);
        assert!(!endpoint.gate().busy());
        assert!(matches!(
            read_frame::<DeviceResponse>(&mut current)
                .unwrap()
                .into_outcome(),
            Err(StorybookAutomationError::NoLiveHost)
        ));
    }
    #[test]
    fn native_surface_release_suspends_admission_until_session_replacement() {
        let endpoint = DeviceEndpoint::listen(0, "surface-1".to_owned()).unwrap();
        let old = endpoint.gate().acquire().unwrap();
        endpoint.gate().suspend();
        assert!(!old.is_current());
        assert!(matches!(
            endpoint.gate().acquire(),
            Err(StorybookAutomationError::NoLiveHost)
        ));
        let mut stream = connect(&endpoint);
        for command in [
            DeviceOperation::GetHost {},
            DeviceOperation::OpenStory {
                key: "counter".to_owned(),
            },
        ] {
            write_frame(
                &mut stream,
                &request(61, PROTOCOL_VERSION, "surface-1", command),
            )
            .unwrap();
            assert!(matches!(
                read_frame::<DeviceResponse>(&mut stream)
                    .unwrap()
                    .into_outcome(),
                Err(StorybookAutomationError::NoLiveHost)
            ));
            assert!(endpoint.try_recv().is_none());
        }
        endpoint.replace_session();
        write_frame(
            &mut stream,
            &request(
                61,
                PROTOCOL_VERSION,
                "surface-1.1",
                DeviceOperation::OpenStory {
                    key: "counter".to_owned(),
                },
            ),
        )
        .unwrap();
        let (_, reply, lease) = admitted(&endpoint).into_parts();
        assert!(lease.as_ref().unwrap().is_current());
        drop(old);
        assert!(endpoint.gate().busy());
        reply
            .send(Err(StorybookAutomationError::NoActiveStory))
            .unwrap();
        drop(lease);
        assert!(!endpoint.gate().busy());
    }
}
