//! Android native shell hosting the same production views as the desktop example.

#[cfg(target_os = "android")]
mod android;

#[cfg(all(target_os = "android", feature = "automation"))]
mod automation;
