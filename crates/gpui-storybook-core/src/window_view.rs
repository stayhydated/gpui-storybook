/// Marker trait for views that can only be rendered inside [`create_new_window`](crate::story::create_new_window).
///
/// Implement this on your `Entity<T>` to allow it as a return type for the
/// closure passed to `create_new_window`.
pub trait SimpleWindowView: Into<gpui::AnyView> {}

/// Marker trait for views that can only be rendered inside [`create_dock_window`](crate::dock_gallery::create_dock_window).
///
/// Implement this on your `Entity<T>` to allow it as a return type for the
/// closure passed to `create_dock_window`.
#[cfg(feature = "dock")]
pub trait DockWindowView: Into<gpui::AnyView> {}
