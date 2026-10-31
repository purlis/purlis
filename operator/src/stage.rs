// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **Where a cold claim's runner is in starting** (spec #2041 story 48, #2057): while upstream
//! says the runner is not ready yet, a Claim's `Ready` is `False` with the reason its pod gives,
//! `Scheduling` until a node takes it, `Starting` while the node attaches its volumes and makes
//! its sandbox, `Pulling` until its image is on that node, `Starting` after, so a wait is never a
//! mystery. A reason upstream gives for a runner that will not be
//! ready on its own (an error, a claim expired, a sandbox suspended) is left as upstream says it.

use crate::resources::{ClaimStatus, READY};
use crate::upstream::KEY_STEP;
use k8s_openapi::api::core::v1::{ContainerStatus, Pod};

/// The pod waits for a node.
pub const SCHEDULING: &str = "Scheduling";
/// The pod has a node, which is pulling the runner's image.
pub const PULLING: &str = "Pulling";
/// The image is there, and the runner is starting.
pub const STARTING: &str = "Starting";

/// The reasons `Ready` is `False` with while a runner is on its way (agent-sandbox v1.0.6's, and
/// the operator's own [`STARTING`] before upstream says anything): only these are told as a
/// stage. Any other is upstream's word on a runner that will not be ready on its own.
pub const ON_ITS_WAY: [&str; 5] = [
    STARTING,
    "SandboxMissing",
    "SandboxNotReady",
    "SandboxCreatePending",
    "DependenciesNotReady",
];

/// What a container waiting for its image says, as the kubelet words it.
const PULL_FAILING: [&str; 3] = ["ErrImagePull", "ImagePullBackOff", "ErrImageNeverPull"];

/// The stage of a runner whose pod is `pod` (none made yet), and its words.
pub fn stage_of(pod: Option<&Pod>) -> (&'static str, String) {
    let Some(pod) = pod else {
        return (
            SCHEDULING,
            "The runner's pod is being made, and waits for a node.".to_owned(),
        );
    };
    let status = pod.status.as_ref();
    let scheduled = status
        .and_then(|status| status.conditions.as_ref())
        .and_then(|conditions| conditions.iter().find(|c| c.type_ == "PodScheduled"));
    match scheduled {
        Some(scheduled) if scheduled.status == "True" => {}
        Some(scheduled) => {
            let mut words = "The runner's pod waits for a node.".to_owned();
            if let Some(why) = scheduled.message.as_deref().filter(|why| !why.is_empty()) {
                words.push_str(&format!(" The scheduler: {why}"));
            }
            return (SCHEDULING, words);
        }
        None => {
            return (SCHEDULING, "The runner's pod waits for a node.".to_owned());
        }
    }
    let statuses: Vec<&ContainerStatus> = status
        .into_iter()
        .flat_map(|status| {
            status
                .init_container_statuses
                .iter()
                .flatten()
                .chain(status.container_statuses.iter().flatten())
        })
        .collect();
    if let Some((reason, why)) = statuses.iter().find_map(|status| {
        let waiting = status.state.as_ref()?.waiting.as_ref()?;
        let reason = waiting.reason.as_deref()?;
        PULL_FAILING.contains(&reason).then(|| {
            (
                reason.to_owned(),
                waiting.message.clone().unwrap_or_default(),
            )
        })
    }) {
        return (
            PULLING,
            format!("The runner's image is not pulled yet: {reason} {why}")
                .trim_end()
                .to_owned(),
        );
    }
    // Before the pod is ready to start containers, its node is still attaching and mounting its
    // volumes (a kept claim's own among them) and making its sandbox: no image is pulled yet, so
    // it is starting, not pulling, however long an attach takes.
    let ready_to_start = status
        .and_then(|status| status.conditions.as_ref())
        .and_then(|conditions| {
            conditions
                .iter()
                .find(|c| c.type_ == "PodReadyToStartContainers")
        });
    if let Some(ready_to_start) = ready_to_start
        && ready_to_start.status != "True"
    {
        return (
            STARTING,
            "The runner's node is attaching its volume and making its sandbox.".to_owned(),
        );
    }
    // Both containers run the pool's image: once the first step's is on the node, it is.
    let pulled = statuses
        .iter()
        .find(|status| status.name == KEY_STEP)
        .is_some_and(|status| {
            !status.image_id.is_empty()
                || status
                    .state
                    .as_ref()
                    .is_some_and(|state| state.running.is_some() || state.terminated.is_some())
        });
    if !pulled {
        return (
            PULLING,
            "The runner's node is pulling its image.".to_owned(),
        );
    }
    (STARTING, "The runner is starting.".to_owned())
}

/// `status`, with its `Ready` told as a stage of `pod` ([`stage_of`]) where it is `False` with a
/// reason of a runner on its way ([`ON_ITS_WAY`]).
pub fn staged(mut status: ClaimStatus, pod: Option<&Pod>) -> ClaimStatus {
    if let Some(ready) = status.conditions.iter_mut().find(|c| {
        c.type_ == READY && c.status == "False" && ON_ITS_WAY.contains(&c.reason.as_str())
    }) {
        let (reason, message) = stage_of(pod);
        ready.reason = reason.to_owned();
        ready.message = message;
    }
    status
}
