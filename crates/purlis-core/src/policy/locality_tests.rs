// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use super::*;
use crate::sandbox::local::OwnConfigHome;
use crate::secrets::answers;
use crate::secrets::seal;
use std::path::{Path, PathBuf};

/// A machine of the test's own: its config home, a keyring with a key, and a folder for planes.
fn machine() -> (OwnConfigHome, tempfile::TempDir) {
    let home = OwnConfigHome::new();
    seal::forget_kept_for_this_test();
    seal::set_for_this_test(Some(vec![7; 32]));
    seal::set_foreign_for_this_test(false);
    seal::set_locked_for_this_test(false);
    seal::set_no_keyring_for_this_test(false);
    (home, tempfile::tempdir().expect("a folder for planes"))
}

/// A plane at `name` under `folder`: a git folder and the project file that marks it.
fn plane(folder: &Path, name: &str) -> PathBuf {
    let root = std::fs::canonicalize(folder).unwrap().join(name);
    std::fs::create_dir_all(root.join(".git")).unwrap();
    std::fs::write(root.join("purlis.toml"), "").unwrap();
    root
}

/// A folder that is no plane's, beside the planes.
fn outside(folder: &Path) -> PathBuf {
    let dir = std::fs::canonicalize(folder).unwrap().join("elsewhere");
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn at(cwd: &Path) -> Whereabouts<'_> {
    Whereabouts {
        cwd,
        target: None,
        ancestry: &[],
    }
}

/// The five plane-less scenarios stay so: nothing of the process is in a plane, and it runs the
/// shipped default under `strict` in its own process.
#[test]
fn a_process_outside_every_anchored_plane_runs_the_shipped_strict_locally() {
    let (_home, planes) = machine();
    let root = plane(planes.path(), "p");
    answers::register(&root).expect("registered");
    let elsewhere = outside(planes.path());
    assert_eq!(locate(&at(&elsewhere)), Locality::PlaneLess);
    let said = judge(&at(&elsewhere), &said_request());
    assert_eq!(
        said,
        Judged::Locally(crate::policy::decide::decide_locally(&said_request()))
    );
}

fn said_request() -> crate::policy::decide::Request<'static> {
    crate::policy::decide::Request {
        chat: "1",
        of: crate::policy::levels::Chat {
            workspace: None,
            persona: None,
        },
        action: "exec",
        resource: ("Command", "ls"),
        context: serde_json::json!({}),
    }
}

/// `git -C <plane>` run from a folder outside every plane: the working directory is outside, the
/// target is not, and so the process is never judged as plane-less.
#[test]
fn git_c_into_an_anchored_plane_is_never_judged_as_plane_less() {
    let (_home, planes) = machine();
    let root = plane(planes.path(), "p");
    answers::register(&root).expect("registered");
    let elsewhere = outside(planes.path());
    let whereabouts = Whereabouts {
        cwd: &elsewhere,
        target: Some(&root),
        ancestry: &[],
    };
    assert_eq!(
        locate(&whereabouts),
        Locality::InPlane {
            root: root.clone(),
            anchored: true
        }
    );
    // A target reached through a link is the plane's by its real path.
    #[cfg(unix)]
    {
        let link = elsewhere.join("link");
        std::os::unix::fs::symlink(&root, &link).unwrap();
        let through = Whereabouts {
            cwd: &elsewhere,
            target: Some(&link),
            ancestry: &[],
        };
        assert!(matches!(locate(&through), Locality::InPlane { .. }));
    }
}

/// A process that sits outside, started by one that works in an anchored plane: its ancestry says
/// it is the plane's.
#[test]
fn a_process_whose_ancestry_is_in_an_anchored_plane_is_never_judged_as_plane_less() {
    let (_home, planes) = machine();
    let root = plane(planes.path(), "p");
    answers::register(&root).expect("registered");
    let elsewhere = outside(planes.path());
    let ancestry = [elsewhere.clone(), root.join("sub")];
    let whereabouts = Whereabouts {
        cwd: &elsewhere,
        target: None,
        ancestry: &ancestry,
    };
    assert!(matches!(
        locate(&whereabouts),
        Locality::InPlane { anchored: true, .. }
    ));
}

/// No keyring: the registry cannot be read, so the planes found on disk are what is asked, and a
/// process inside one is never plane-less.
#[test]
fn without_a_keyring_a_process_in_a_plane_found_on_disk_is_never_plane_less() {
    let (_home, planes) = machine();
    let root = plane(planes.path(), "p");
    answers::register(&root).expect("registered while a keyring was there");
    seal::set_no_keyring_for_this_test(true);
    let inside = root.join("sub");
    std::fs::create_dir_all(&inside).unwrap();
    assert_eq!(
        locate(&at(&inside)),
        Locality::InPlane {
            root: root.clone(),
            anchored: false
        }
    );
    // The same through `git -C`, and through the ancestry.
    let elsewhere = outside(planes.path());
    let target = Whereabouts {
        cwd: &elsewhere,
        target: Some(&inside),
        ancestry: &[],
    };
    assert!(matches!(locate(&target), Locality::InPlane { .. }));
    let ancestry = [inside];
    let parent = Whereabouts {
        cwd: &elsewhere,
        target: None,
        ancestry: &ancestry,
    };
    assert!(matches!(locate(&parent), Locality::InPlane { .. }));
    // And a folder outside every plane stays plane-less.
    assert_eq!(locate(&at(&elsewhere)), Locality::PlaneLess);
}

/// Inside a plane found on disk with no keyring, the shipped default decides and the seal gap is
/// declared in what the decision says.
#[test]
fn without_a_keyring_inside_a_found_plane_the_shipped_default_applies_with_the_gap_declared() {
    let (_home, planes) = machine();
    let root = plane(planes.path(), "p");
    seal::set_no_keyring_for_this_test(true);
    let judged = judge(&at(&root), &said_request());
    let Judged::FoundPlane { decision, gap } = judged else {
        panic!("a found plane is judged by the shipped default: {judged:?}");
    };
    assert_eq!(
        decision,
        crate::policy::decide::decide_locally(&said_request())
    );
    assert_eq!(gap, answers::NO_KEYRING_NOTICE);
}

/// A plane anchored on a machine with a keyring is the host's to judge: the process must ask it,
/// and is refused where it cannot.
#[test]
fn a_process_in_an_anchored_plane_is_the_hosts_to_ask() {
    let (_home, planes) = machine();
    let root = plane(planes.path(), "p");
    answers::register(&root).expect("registered");
    let judged = judge(&at(&root), &said_request());
    assert_eq!(judged, Judged::AskTheHost { root });
}

/// `git -C <folder>` names the folder the command acts on, relative to the working directory.
#[test]
fn the_target_of_a_git_c_command_is_its_folder() {
    let cwd = Path::new("/work/elsewhere");
    assert_eq!(
        target_of("git -C /plane merge feature", cwd),
        Some(PathBuf::from("/plane"))
    );
    assert_eq!(
        target_of("git -C ../plane merge feature", cwd),
        Some(Path::new("/work/elsewhere").join("../plane"))
    );
    assert_eq!(target_of("git merge feature", cwd), None);
    assert_eq!(
        target_of("git -C a status && git -C \"b\" merge x", cwd),
        Some(cwd.join("b"))
    );
}

/// A merge asked by a process the app did not start is decided by the shipped default under
/// `strict`, in unattended terms: there is no window to ask, and the policy's answer is its own.
#[test]
fn a_process_that_is_no_chats_asks_with_the_context_a_host_would_make_for_it() {
    let request = process_request("merge", ("Branch", "acme/web#1"));
    assert_eq!(request.context["attended"], false);
    assert_eq!(request.context["via"], "hook");
    assert_eq!(request.context["sandbox"], "off");
    // It fits the schema: a decision names a rule and is not a schema error.
    let said = crate::policy::decide::decide_locally(&request);
    assert!(!said.rules_said().is_empty(), "{said:?}");
}
