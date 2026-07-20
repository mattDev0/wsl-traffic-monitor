# Testing Strategy

This document describes the intended test layers. The current repository contains only scaffold tests.

## Unit Tests

Each crate should own tests for its pure logic. Monitoring calculations, counter reset handling, configuration parsing, and classifier decisions should be tested without requiring WSL or Windows APIs.

## Integration Tests

Integration tests should use recorded adapter snapshots and synthetic counter deltas before touching live machine state.

Live WSL, Docker Desktop, ETW, or Windows networking tests should be explicitly opt-in because they depend on host configuration and permissions.

## CI

CI runs on Windows and Linux:

- formatting check
- clippy across all targets
- tests across all targets
- docs build without dependencies

Linux CI exists to keep platform-neutral crates honest. Windows CI is required before any platform integration is considered complete.
