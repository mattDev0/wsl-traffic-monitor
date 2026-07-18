//! Monitoring orchestration boundary.
//!
//! This crate will coordinate discovery, classification, sampling, and state
//! transitions. It currently exposes scaffold metadata only.

use wsl_traffic_core::PRODUCT_NAME;

/// Returns the display name for the monitoring subsystem.
#[must_use]
pub const fn monitor_name() -> &'static str {
    PRODUCT_NAME
}
