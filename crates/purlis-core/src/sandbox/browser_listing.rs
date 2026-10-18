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
mod tests {
    use super::*;
    use crate::sandbox::{Os, reach::By};

    fn machine() -> Machine {
        Machine {
            env: crate::secrets::Env::of(&[]),
            home: Some(std::path::PathBuf::from("/home/op")),
            os: Os::MacOs,
        }
    }

    fn project(text: &str) -> tempfile::TempDir {
        let root = tempfile::tempdir().expect("a project");
        std::fs::write(crate::names::manifest(root.path()), text).expect("written");
        root
    }

    fn listed(root: &Path, persona: Option<&str>) -> Vec<(String, By)> {
        listed_for(root, &machine(), persona)
            .expect("a listing")
            .written()
            .listed
    }

    const ON: &str = "[sandbox]\nmode = \"on\"\negress = []\nhosts = [\"api.example\"]\n\
                      [sandbox.personas.devops]\nhosts = [\"ops.example\"]\n";

    #[test]
    fn a_browser_on_no_persona_lists_the_projects_hosts_and_no_personas() {
        let root = project(ON);
        crate::sandbox::persona::allow_every_as_listed(
            root.path(),
            &Plane::of(Some(ON)).said().policy.expect("on"),
        );
        assert_eq!(
            listed(root.path(), None),
            [("api.example".into(), By::Open)]
        );
    }

    #[test]
    fn a_persona_lists_its_own_hosts_only_as_the_person_here_allowed_them() {
        let root = project(ON);
        assert_eq!(
            listed(root.path(), Some("devops")),
            [("api.example".into(), By::Open)]
        );
        crate::sandbox::persona::allow_every_as_listed(
            root.path(),
            &Plane::of(Some(ON)).said().policy.expect("on"),
        );
        assert_eq!(
            listed(root.path(), Some("devops")),
            [
                ("api.example".into(), By::Open),
                ("ops.example".into(), By::Persona)
            ]
        );
    }

    #[test]
    fn a_project_that_did_not_turn_the_sandbox_on_is_listed_as_its_settings_say_never_as_the_floor()
    {
        let root = project("[sandbox]\negress = []\nhosts = [\"api.example\"]\n");
        let reach = listed_for(root.path(), &machine(), None).expect("a listing");
        assert!(!reach.written().anything, "never the floor's reach");
        assert_eq!(
            reach.written().listed,
            [("api.example".to_owned(), By::Open)]
        );
    }

    #[test]
    fn a_service_on_this_machine_the_person_allowed_for_the_project_is_listed_as_theirs() {
        let root = project("[sandbox]\nmode = \"on\"\negress = []\n");
        crate::sandbox::local::grant_port(root.path(), 5173).expect("granted");
        let listed = listed(root.path(), None);
        assert!(
            listed
                .iter()
                .any(|(host, by)| host.contains("5173") && *by == By::You),
            "{listed:?}"
        );
    }

    #[test]
    fn a_project_whose_file_cannot_be_read_or_has_gone_lists_nothing() {
        let unreadable = project("[sandbox\nmode = ");
        assert!(listed_for(unreadable.path(), &machine(), None).is_none());
        let gone = tempfile::tempdir().expect("a project");
        assert!(listed_for(gone.path(), &machine(), None).is_none());
    }
}
