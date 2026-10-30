// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! The two Landlock layers: the supervisor's, beneath what it performs, and the chat's, inside
//! it. Each is required whole ([`CompatLevel::HardRequirement`]): a kernel that cannot apply
//! one starts nothing.

use std::path::Path;

use landlock::{
    ABI, Access, AccessFs, AccessNet, CompatLevel, Compatible, NetPort, PathBeneath, PathFd,
    Ruleset, RulesetAttr, RulesetCreatedAttr, RulesetStatus, Scope,
};

use crate::profile::Reach;

const AT: ABI = ABI::V6;

/// The kernel's Landlock ABI, read by which of the first [`crate::MIN_ABI`] it can create a
/// ruleset for: `0` with none, and [`crate::MIN_ABI`] for that one or any later.
pub fn abi() -> i32 {
    let tries: [(i32, ABI); 6] = [
        (6, ABI::V6),
        (5, ABI::V5),
        (4, ABI::V4),
        (3, ABI::V3),
        (2, ABI::V2),
        (1, ABI::V1),
    ];
    tries
        .into_iter()
        .find(|(n, abi)| creates(*n, *abi).is_ok())
        .map_or(0, |(n, _)| n)
}

/// Whether this kernel creates a ruleset of everything ABI `abi` (number `n`) handles.
fn creates(n: i32, abi: ABI) -> Result<(), landlock::RulesetError> {
    let mut ruleset = Ruleset::default()
        .set_compatibility(CompatLevel::HardRequirement)
        .handle_access(AccessFs::from_all(abi))?;
    if n >= 4 {
        ruleset = ruleset.handle_access(AccessNet::from_all(abi))?;
    }
    if n >= 6 {
        ruleset = ruleset.scope(Scope::from_all(abi))?;
    }
    ruleset.create().map(drop)
}

fn fully(status: RulesetStatus) -> Result<(), String> {
    match status {
        RulesetStatus::FullyEnforced => Ok(()),
        other => Err(format!("Landlock was not applied whole ({other:?})")),
    }
}

/// The supervisor's own layer: it reads everything, writes only what `reach` names, makes no
/// device, and connects over TCP only to `reach`'s ports (anywhere, at the floor), and binds
/// and connects to the ports picked for the chat's test servers (`picked`, #3396).
pub(super) fn supervisor(reach: &Reach, picked: &[u16]) -> Result<(), String> {
    let all = AccessFs::from_all(AT);
    let read = AccessFs::from_read(AT);
    let write = AccessFs::from_write(AT) & !(AccessFs::MakeChar | AccessFs::MakeBlock);
    let mut ruleset = Ruleset::default()
        .set_compatibility(CompatLevel::HardRequirement)
        .handle_access(all)
        .map_err(|why| why.to_string())?
        .scope(Scope::from_all(AT))
        .map_err(|why| why.to_string())?;
    if !reach.any_host {
        ruleset = ruleset
            .handle_access(AccessNet::from_all(AT))
            .map_err(|why| why.to_string())?;
    }
    let mut created = ruleset.create().map_err(|why| why.to_string())?;
    created = created
        .add_rule(PathBeneath::new(open("/")?, read))
        .map_err(|why| why.to_string())?;
    // The devices a terminal program writes, and this machine's process files, which the
    // supervisor writes only for a process inside the sandbox (`super::judge`).
    for (path, access) in [
        (
            "/dev",
            AccessFs::WriteFile | AccessFs::IoctlDev | AccessFs::Truncate,
        ),
        ("/proc", AccessFs::WriteFile | AccessFs::Truncate),
        ("/dev/shm", write),
    ] {
        if let Ok(fd) = open(path) {
            created = created
                .add_rule(PathBeneath::new(fd, access))
                .map_err(|why| why.to_string())?;
        }
    }
    for path in &reach.written {
        // A path not there yet is made by no rule here: what the profile lets a chat write
        // beneath it is reached through a folder above it that is.
        let Some(at) = path.ancestors().find(|it| it.exists()) else {
            continue;
        };
        let fd = open(at)?;
        let access = if at.is_dir() {
            write
        } else {
            write & AccessFs::from_file(AT)
        };
        created = created
            .add_rule(PathBeneath::new(fd, access))
            .map_err(|why| why.to_string())?;
    }
    if !reach.any_host {
        for port in &reach.ports {
            created = created
                .add_rule(NetPort::new(*port, AccessNet::ConnectTcp))
                .map_err(|why| why.to_string())?;
        }
        for port in picked {
            created = created
                .add_rule(NetPort::new(
                    *port,
                    AccessNet::BindTcp | AccessNet::ConnectTcp,
                ))
                .map_err(|why| why.to_string())?;
        }
    }
    fully(
        created
            .restrict_self()
            .map_err(|why| why.to_string())?
            .ruleset,
    )
}

/// The chat's layer: it starts programs and nothing else by itself. Every file it opens and
/// every connection it makes is the supervisor's; it binds no port, signals no process
/// outside the sandbox and reaches no abstract socket made outside it.
pub(super) fn chat() -> Result<(), String> {
    let created = Ruleset::default()
        .set_compatibility(CompatLevel::HardRequirement)
        .handle_access(AccessFs::from_all(AT))
        .map_err(|why| why.to_string())?
        .handle_access(AccessNet::from_all(AT))
        .map_err(|why| why.to_string())?
        .scope(Scope::from_all(AT))
        .map_err(|why| why.to_string())?
        .create()
        .map_err(|why| why.to_string())?
        // What starting a program needs.
        .add_rule(PathBeneath::new(
            open("/")?,
            AccessFs::Execute | AccessFs::ReadFile,
        ))
        .map_err(|why| why.to_string())?;
    fully(
        created
            .restrict_self()
            .map_err(|why| why.to_string())?
            .ruleset,
    )
}

fn open(path: impl AsRef<Path>) -> Result<PathFd, String> {
    let path = path.as_ref();
    PathFd::new(path).map_err(|why| format!("{}: {why}", path.display()))
}
