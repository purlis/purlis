// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! The engine a browser runs on (#2009, spec #2008).
//!
//! A crate of its own, outside `purlis-core`, so a session host links the engine without the
//! rest of the core (the direction of #1995). In the order a browser's start meets them:
//!
//! - [`install`]: which engine runs, and the proof that it is the one meant: the pinned Chrome
//!   for Testing, downloaded on first use and checked against a trusted manifest before it
//!   runs, its age warned about and refused; or the machine's own Chrome, by the person's
//!   choice.
//! - [`launch`]: what the engine is started with: its own sandbox on, the reach proxy its only
//!   route, no direct UDP, password saving, autofill and crash reports off, no window.
//! - [`process`]: one process group per browser, led by its [`keeper`], with DevTools on an
//!   inherited pipe and no port; the group ends with its host, however the host ends.
//! - [`cdp`]: DevTools over that pipe, one NUL-ended JSON message at a time.
//! - [`WebAddress`]: the only addresses purlis sends a browser to, http and https; every call
//!   that opens a page takes one, and nothing outside this crate speaks DevTools to the engine.
//!
//! Driving pages (snapshots, actions), the reach proxy itself and the broker that owns
//! browsers are other parts of the session host (#2010, #2011, #2012).

mod address;
pub mod cdp;
pub mod drive;
pub mod install;
pub mod keeper;
pub mod launch;
pub mod machine;
pub mod process;

pub use address::{NotWebAddress, WebAddress};
