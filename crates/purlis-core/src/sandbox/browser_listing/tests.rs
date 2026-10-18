// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

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
fn a_project_that_did_not_turn_the_sandbox_on_is_listed_as_its_settings_say_never_as_the_floor() {
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
