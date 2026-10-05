# Embedded automation example

Application-owned GPUI roots for Counter and Notes. The same views supply typed
controls, a scoped increment action, rendered semantic state, and fresh inert
scenarios to `GpuiHostAttachment`.

Run the desktop view with `cargo run -p gpui-storybook-example-embedded`.
The application initializes GPUI Kit, owns its window and assets, and supplies
the attachment's route catalog and capture provider. Its owner thread admits
operations with a lease covering the whole batch. Submitted work keeps that
lease through frame callbacks even if the response receiver is dropped.

`cargo test -p gpui-storybook-example-embedded --locked` renders the production
views, clicks the counter, writes through the GPUI text path, repeats fresh
scenarios, and verifies preflight rejection and cancellation ownership.
GPUI text insertion exercises the GPUI input handler; native IME qualification
belongs to the Android harness.
