//! A paired phone's side of this desktop (iOS design §3): the app's wiring of
//! `purlis-session-protocol`'s gateway over `purlis_core::remote`'s state.
//!
//! - [`board`]: the `session::Host` a phone talks to, over the app's own seam.
//! - [`timeline`]: a chat's level-2 story, from the event log and the hooks' asks.
//! - [`adapters`]: the gateway's `Pins`, `PairingDesk` and revocation over core's peers and
//!   desk, and its `Heard` into the audit.
//! - [`gateway`]: the listener's lifecycle, keep-awake, and the commands the window calls.
//! - [`awake`]: the keep-awake child.

pub mod adapters;
pub mod awake;
pub mod board;
pub mod gateway;
pub mod timeline;

#[cfg(test)]
mod awake_tests;
#[cfg(test)]
mod board_tests;
#[cfg(test)]
mod gateway_tests;
#[cfg(test)]
mod timeline_tests;
