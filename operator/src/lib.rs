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
//! **Each person's runners run in a namespace of their own** ([`person`], #2055): the operator
//! makes it, gives the person alone a role there and caps their runners with a quota. A Claim
//! stays where it is made, and its finalizer removes its runner when it is deleted.
//!
//! **A Claim is kept or fresh** (#2057): a kept one's runner starts cold with a volume of its
//! own for its home, which keeps its work and its pairing through the loss of its pod; a fresh
//! one's takes a warm runner where one waits, and keeps nothing. While a runner starts, its
//! Claim says its stage ([`stage`]).
//!
//! **It vouches for each runner's link key** ([`vouch`], #2056): the key a runner's pod made as
//! it started, signed with the operator's own key into a statement on the Claim's status, which
//! the desktop checks against its own pairing over `kubectl exec`.
//!
//! **What it may do** is `deploy/operator.yaml`'s ClusterRole and nothing else: read Pools and
//! Claims, write a Pool's and a Claim's status and a Claim's finalizer, read the policies'
//! bindings, read, create, patch and delete the three upstream objects, read a runner's pod by
//! name, and make each person's runner namespace, its role binding and its quota. It holds no
//! `exec` or `attach` right to a pod itself and none to read a Secret: its signing keys are mounted
//! in its pod by the kubelet, and read again as a team admin rolls them (#3305). It binds one role, `purlis-runner-owner`, only to one person, only
//! in runner namespaces, as the policy on runner namespaces holds at admission
//! ([`RUNNER_NAMESPACES_POLICY`]). This crate's Kubernetes client is built without the websocket feature `exec`
//! and `attach` would need.

mod admission;
pub mod doctor;
pub mod person;
mod reconcile;
mod resources;
pub mod stage;
pub mod upstream;
pub mod vouch;

pub use admission::{
    Admission, POLICY, RUNNER_NAMESPACES_POLICY, SETTLE, admission, in_force_since,
};
pub use reconcile::{
    CLUSTER_NAMESPACE, FIELD_MANAGER, LinkKeyIs, OVER_QUOTA_AGAIN, UNCOUNTED_FOR,
    claims_of_upstream, claims_woken_by, cluster_of, pool_objects_go_with, pool_status_patch,
    retry_after_conflict, run, settled, status_of, status_of_a_deleted_pool,
    status_of_a_missing_pool, status_of_an_object_in_the_way, status_patch, status_withdrawn,
};
pub use resources::{
    Claim, ClaimSpec, ClaimStatus, KeptVolume, LinkKey, NextSigningKeyVouch, Pool, PoolSpec,
    PoolStatus,
};
