// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **What counts as outside any plane** (M75-07b, spec §6; review round 4, SEC4-SF1).
//!
//! A process is plane-less only when its working directory, its target's real path (`git -C`'s
//! folder, for example) and every folder of its ancestry are all outside every plane. Otherwise
//! it belongs to a plane, and is never given the local `strict` of a process with no host to ask.
//!
//! **A plane is anchored** when the registry of planes ([`answers::plane_containing`]) holds it.
//! **With no keyring, or no readable registry** (Q320d point 2), the same test runs against the
//! planes found on disk (`plane::find_root`'s upward search for the project file), never the
//! registry alone, and a plane found only there is judged by the shipped default with the seal
//! gap declared: nothing can be sealed for it.

use std::path::{Path, PathBuf};

use crate::secrets::answers;

use super::decide::{Decision, Request, decide_locally};

/// What the registry cannot tell where the keyring works but the plane was never anchored.
pub const NOT_ANCHORED_NOTICE: &str = "This plane is not anchored on this machine, so purlis cannot \
     seal your answers for it and the shipped default applies. Nothing you answer is kept.";

/// **Where a process is**, as the host or the hook reads it.
#[derive(Debug, Clone, Copy)]
pub struct Whereabouts<'a> {
    /// The process's working directory.
    pub cwd: &'a Path,
    /// The folder the command acts on where it names one (`git -C <folder>`).
    pub target: Option<&'a Path>,
    /// The working directory of each process above it, up to the app's chat.
    pub ancestry: &'a [PathBuf],
}

impl Whereabouts<'_> {
    fn paths(&self) -> impl Iterator<Item = &Path> {
        std::iter::once(self.cwd)
            .chain(self.target)
            .chain(self.ancestry.iter().map(PathBuf::as_path))
    }
}

/// Which plane, if any, a process belongs to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Locality {
    /// Nothing of it is in a plane.
    PlaneLess,
    /// Something of it is in the plane at `root`.
    InPlane {
        /// The plane's root, by its real path.
        root: PathBuf,
        /// Whether the registry holds it. Not anchored: found on disk alone.
        anchored: bool,
    },
}

/// **Whether `whereabouts` is outside every plane**, by the registry and by the planes on disk.
/// An anchored plane is preferred where both name one.
pub fn locate(whereabouts: &Whereabouts<'_>) -> Locality {
    if let Some(registered) = whereabouts.paths().find_map(answers::plane_containing) {
        return Locality::InPlane {
            root: registered.real,
            anchored: true,
        };
    }
    // Never the registry alone: where it is empty, unreadable or has no keyring behind it, a
    // plane is still the plane a project file marks above a path.
    let found = whereabouts.paths().find_map(|path| {
        let real = crate::sandbox::real(path);
        crate::plane::find_root(&real).ok()
    });
    match found {
        Some(root) => Locality::InPlane {
            root: crate::sandbox::real(&root),
            anchored: false,
        },
        None => Locality::PlaneLess,
    }
}

/// **What is to be done with an ask from a process** (see [`judge`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Judged {
    /// Outside every plane: the shipped default under `strict`, decided in the asking process.
    Locally(Decision),
    /// In a plane found on disk that nothing is sealed for: the shipped default decides, and
    /// `gap` says why nothing the person loosened or kept applies.
    FoundPlane {
        /// The decision.
        decision: Decision,
        /// The seal gap, in the words the window and doctor use.
        gap: &'static str,
    },
    /// In an anchored plane: its host decides, and no host means a refusal, never a local answer.
    AskTheHost {
        /// The plane's root.
        root: PathBuf,
    },
}

/// **Which decision a process that is no chat's gets** (spec §6): the shipped `strict` outside
/// every plane, the shipped default with the seal gap declared in a plane found only on disk, and
/// the plane's host for one that is anchored.
pub fn judge(whereabouts: &Whereabouts<'_>, request: &Request<'_>) -> Judged {
    match locate(whereabouts) {
        Locality::PlaneLess => Judged::Locally(decide_locally(request)),
        Locality::InPlane {
            anchored: false, ..
        } => Judged::FoundPlane {
            decision: decide_locally(request),
            gap: if answers::registry_reads() {
                NOT_ANCHORED_NOTICE
            } else {
                answers::NO_KEYRING_NOTICE
            },
        },
        Locality::InPlane { root, .. } => Judged::AskTheHost { root },
    }
}

/// **The request a process that is no chat's makes** (spec §6): the action and resource it asks
/// about, and the context a host would make for it where none can: unattended, no sandbox of
/// purlis's, arriving by a hook. Only the fields the action's context declares.
pub fn process_request<'a>(action: &'a str, resource: (&'a str, &'a str)) -> Request<'a> {
    let mut context = serde_json::Map::new();
    for field in super::context_fields(action) {
        let value: serde_json::Value = match field.as_str() {
            "sandbox" => "off".into(),
            "attended" => false.into(),
            "harness_mode" => "default".into(),
            "depth" => 0.into(),
            "via" => "hook".into(),
            _ => continue,
        };
        context.insert(field.clone(), value);
    }
    Request {
        chat: crate::enforcement::UNKNOWN_CHAT,
        of: super::levels::Chat {
            workspace: None,
            persona: None,
        },
        action,
        resource,
        context: serde_json::Value::Object(context),
    }
}

/// **The folder a `git -C <folder>` command acts on**, read from the command's own words, against
/// `cwd` where it is relative: the target of a process whose working directory is elsewhere.
/// `None` for a command with no `-C`. The last `-C` of the `git` words wins, as git reads them.
pub fn target_of(command: &str, cwd: &Path) -> Option<PathBuf> {
    let mut words = command
        .split_whitespace()
        .map(|w| w.trim_matches(['\'', '"']));
    let mut target = None;
    while let Some(word) = words.next() {
        if word == "-C"
            && let Some(folder) = words.next()
        {
            target = Some(cwd.join(folder));
        }
    }
    target
}

#[cfg(test)]
#[path = "locality_tests.rs"]
mod tests;
