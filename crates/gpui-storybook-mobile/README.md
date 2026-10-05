# GPUI Storybook mobile

Bounded loopback transport and operation admission for an application-owned
device host. The app enables automation explicitly, polls requests on its GPUI
owner thread, and carries the supplied mutation lease through native updates,
input execution, rendered-frame waits, and capture observation.

Surface replacement assigns a new session. A disconnected client rediscovers
public state without replaying its submitted mutation.

Session replacement, duplicate receipts, and lease revocation share one atomic
admission boundary. `try_recv` discards revoked queued work. Native queues retain
a non-owning `MutationPermit` and check it immediately before dispatch; the
original lease remains with the operation owner through acknowledgment.
