// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **What a browser with no compiled sandbox lists** (#2176): a browser the person opens from
//! the window has no chat, so no chat's sandbox was compiled for it at a start. It lists what
//! the project's sandbox settings list for a chat on no persona, compiled here as a chat's start
//! compiles them ([`Compiled::of`]): the project's hosts, the hosts and services on this machine
//! the person allowed for the project, and the hosts an administrator's policy pins as never
//! allowed. Read each time a browser starts or acts, so an Allow since reaches it.
//!
//! **Never the floor's reach** (D-2011-10): a browser takes only what is listed, whether or not
//! the project turned the sandbox on, so its settings are read as written, or as the defaults
//! where it names none.

use std::path::Path;

use super::{Compiled, Machine, Plane, Policy, policy, reach::Reach};

/// **What the project at `root` lists for a browser of a chat as `persona`, or as none**
/// (module docs). `None` where the project's file cannot be read or has gone: such a browser
/// lists nothing.
pub fn listed_for(root: &Path, machine: &Machine, persona: Option<&str>) -> Option<Reach> {
    // One spelling of the plane, as a start reads it: the kernel's.
    let real_root = root.canonicalize().unwrap_or_else(|_| root.to_path_buf());
    let root = real_root.as_path();
    let plane = Plane::read(root);
    if plane.unreadable() || plane.missing() {
        return None;
    }
    let locks = policy::Locks::of(root);
    let policy = plane
        .in_force(&locks)
        .or_else(|| plane.said().unset)
        .unwrap_or_else(Policy::defaults);
    Some(Compiled::of(&policy, &plane, root, machine, persona).reach)
}

#[cfg(test)]
mod tests;
