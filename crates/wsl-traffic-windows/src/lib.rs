//! Windows platform boundary.
//!
//! Exposes Win32 IP Helper wrappers, process queries, and registry checks.

pub mod adapters;
pub mod processes;
pub mod registry;

pub use adapters::get_adapters;
pub use processes::get_running_docker_processes;
pub use registry::{
    get_windows_version, get_wsl_distros_from_registry, is_docker_desktop_installed,
};

/// Returns whether this build target is Windows.
#[must_use]
pub const fn is_windows_target() -> bool {
    cfg!(windows)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_windows_target() {
        assert_eq!(is_windows_target(), cfg!(windows));
    }

    #[test]
    fn test_get_windows_version() {
        let ver = get_windows_version();
        assert!(!ver.is_empty());
    }

    #[test]
    fn test_get_running_docker_processes() {
        let _procs = get_running_docker_processes();
    }
}
