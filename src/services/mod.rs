// Network Manager - Background Services
// Copyright (C) 2026 Christos A. Daggas
// SPDX-License-Identifier: MIT

//! Background services for automated operations.
//!
//! This module contains services that run in the background:
//! - Scheduler: Activates profiles at scheduled times
//! - Watchdog: Monitors connectivity and takes action on failure
//! - Sandbox: Provides script execution isolation
//! - Encryption: Profile data encryption/decryption

pub mod watchdog;
pub mod sandbox;
pub mod encryption;
pub mod autoswitch;

pub use watchdog::WatchdogService;

#[allow(unused_imports)]
pub use sandbox::SandboxRunner;
#[allow(unused_imports)]
pub use encryption::ProfileEncryption;
pub use autoswitch::AutoSwitchService;
