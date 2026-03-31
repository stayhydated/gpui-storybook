/// Marker trait for view types that can be rendered inside
/// [`create_new_window`](crate::story::create_new_window).
///
/// Implement this on your view struct to allow `Entity<YourView>` as the
/// return type of the closure passed to `create_new_window`.
///
/// ```ignore
/// impl gpui_storybook::SimpleWindowView for MyShell {}
/// ```
pub trait SimpleWindowView: gpui::Render + 'static {}

/// Marker trait for view types that can be rendered inside
/// [`create_dock_window`](crate::dock_gallery::create_dock_window).
///
/// Implement this on your view struct to allow `Entity<YourView>` as the
/// return type of the closure passed to `create_dock_window`.
///
/// ```ignore
/// impl gpui_storybook::DockWindowView for MyWorkspace {}
/// ```
#[cfg(feature = "dock")]
pub trait DockWindowView: gpui::Render + 'static {}
