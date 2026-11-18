// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The scenario suite's network, all of it on this machine** (T1.1-02, #6849): scenarios never
//! touch the internet, and a site's own refusal is never taken for purlis's.
//!
//! - [`TestCa`]: a test certificate authority, made new for each run and constrained to `.test`
//!   names, and the bundle of the system's roots plus it that the suite's clients trust
//!   ([`TestCa::bundle`], [`ca_env`]).
//! - [`Origin`]: a local HTTP or HTTPS origin in one of the named [`Variant`]s, recording every
//!   request it gets ([`Seen`]).
//! - [`FakeForge`]: a GitHub-shaped and GitLab-shaped API for a merge, recording every request and
//!   the credential it presented (T1.1-50).
//! - [`DirectTarget`]: TCP and UDP listeners on this machine's **non-loopback** interface
//!   address, so an escape cell that bypasses the proxy has something it could have reached; the
//!   UDP one speaks enough DNS to record a direct lookup.
//!
//! A proxy reaches an origin only through purlis-core's resolver seam
//! (`purlis_core::sandbox::names`, a test build's alone), which maps an exact name and port to an
//! origin's [`Origin::addr`]. Nothing here edits `/etc/hosts` or any system setting.

mod ca;
mod direct;
mod forge;
mod origin;

pub use ca::{CA_VARIABLES, TestCa, ca_env};
pub use direct::{DirectTarget, interface_address};
pub use forge::{Credential, FORGE_HEADER, FakeForge, Forge, MERGE_SHA, Refusal, Request};
pub use origin::{FORBIDDEN_BODY, ORIGIN_HEADER, Origin, Scheme, Seen, Variant};
