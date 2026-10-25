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
//! It acts only on Claims admitted under its policy, however it was installed ([`admission`]).
//!
//! **It vouches for each runner's link key** ([`vouch`], #2056): the key a runner's pod made as
//! it started, signed with the operator's own key into a statement on the Claim's status, which
//! the desktop checks against its own pairing over `kubectl exec`.
//!
//! **What it may do** is `deploy/operator.yaml`'s ClusterRole and nothing else: read Pools and
//! Claims, write a Pool's and a Claim's status, read the policy's binding, read, create and
//! patch the three upstream objects, and read a runner's pod by name. It holds no right to
//! `exec` or `attach` to a pod and none to read a Secret: its signing key is mounted in its pod
//! by the kubelet. This crate's Kubernetes client is built without the websocket feature `exec`
//! and `attach` would need.

mod admission;
pub mod doctor;
mod reconcile;
mod resources;
pub mod upstream;
pub mod vouch;

pub use admission::{Admission, POLICY, SETTLE, admission};
pub use reconcile::{
    FIELD_MANAGER, claims_kept_as_owners, claims_that_vouch, claims_woken_by, pool_status_names,
    retry_after_conflict, run, settled, status_of, status_of_a_deleted_pool,
    status_of_a_missing_pool, status_of_an_object_in_the_way,
    status_of_objects_held_by_refused_claims,
};
pub use resources::{Claim, ClaimSpec, ClaimStatus, LinkKey, Pool, PoolSpec, PoolStatus};
