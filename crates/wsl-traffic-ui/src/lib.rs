//! Native UI boundary.
//!
//! Tray and taskbar integration will live here once measurement behavior is
//! validated.

/// UI surface reserved by the scaffold.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum UiSurface {
    /// Windows notification area surface.
    Tray,
    /// Windows taskbar embedded display surface.
    Taskbar,
}
