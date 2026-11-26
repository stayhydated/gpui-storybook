# GPUI Storybook mobile

Bounded loopback transport and operation admission for an application-owned
device host. Application-owned platform bridges can use the target-neutral
endpoint with `gpui-storybook-automation-gpui` and its device coordinator.
Its Kotlin source is an optional, application-owned native callback adapter,
not a GPUI mobile runtime. The app enables automation explicitly and carries
the supplied lease through native acknowledgment and capture observation.

Surface replacement advances an endpoint-owned session generation. Use `DeviceEndpoint::listen_fresh` for a unique process/session seed, or supply
a process-unique seed of 1–107 bytes to `DeviceEndpoint::listen`; call
`replace_session()` after invalidating the old attachment. A disconnected client rediscovers
public state without replaying its submitted mutation.

Session replacement, duplicate receipts, and lease revocation share one atomic
admission boundary. `try_recv` discards revoked queued work. Native queues retain
a non-owning `MutationPermit` and check it immediately before dispatch; the
original lease remains with the operation owner through acknowledgment.

Each incoming frame has a 30-second overall deadline, including its length
prefix and payload. Responses have a five-second write deadline. Dropping the
endpoint suspends admission, closes active sockets, and joins transport threads.
