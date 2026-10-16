// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! purlis's cluster operator, "purlis in your cluster" (spec #2041, #2054).
//!
//! A team defines a [`Pool`] once: where its runners come from, today the runner image by
//! digest. A person asks for a runner with a [`Claim`] in a namespace. The operator turns both
//! into objects of the pinned upstream agent-sandbox ([`upstream`], v1.0.6, its definitions
//! vendored in `vendor/agent-sandbox/`), whose controller makes the pod, and tells the person on
//! the Claim whether the runner is ready.
//!
//! It never re-implements what upstream does (adopting a warm runner, making the pod): a Claim
//! becomes one upstream `SandboxClaim` on the pool's `SandboxWarmPool`. Upstream's controller
//! writes that claim's status, which is why purlis's Claim is an object of its own.
//!
//! **What it may do** is `deploy/operator.yaml`'s ClusterRole and nothing else: read Pools and
//! Claims, write a Claim's status, and create and patch the three upstream objects. It holds no
//! right to `exec` or `attach` to a pod and reads no Secret; this crate's Kubernetes client is
//! built without the websocket feature those would need.

mod reconcile;
mod resources;
pub mod upstream;

pub use reconcile::{FIELD_MANAGER, run, settled, status_of, status_of_a_missing_pool};
pub use resources::{Claim, ClaimSpec, ClaimStatus, Pool, PoolSpec};
