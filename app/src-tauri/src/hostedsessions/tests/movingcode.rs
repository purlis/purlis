// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The window's side of a transfer of code** (1.34, #2046): a refusal is said at once, stream
//! or none, and a bundle past the link's bound is refused at the window. The git is the core's
//! (`purlis_core::codelink`) and the real transfers are `crate::runnercode`'s tests.

use std::path::Path;

use purlis_session_protocol::session::{CodeRef, Refusal};

use super::*;

const PROJECT: &str = "01J9PRJCT00000000000000000";
const REPO: &str = "workspaces/app/web";

fn runtime() -> tokio::runtime::Runtime {
    tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
        .unwrap()
}

#[test]
fn a_take_and_a_give_the_host_refuses_before_it_opens_a_stream_are_said_at_once() {
    // A host that keeps no projects of its own refuses every move of code before any stream.
    let hosted = hosted();
    let into = hosted.root.join("given.bundle");

    let started = Instant::now();
    let took = hosted.link.send_code(PROJECT, REPO, Vec::new(), None, None);
    let gave = hosted.link.fetch_code(PROJECT, REPO, Vec::new(), &into);

    assert!(started.elapsed() < PATIENCE, "{:?}", started.elapsed());
    let took = took.unwrap_err();
    assert!(took.contains("keeps no projects"), "{took}");
    let gave = gave.unwrap_err();
    assert!(gave.contains("keeps no projects"), "{gave}");
}

/// What a runner gives: `bytes` of bundle, whatever the window has.
#[derive(Clone)]
struct Gives(usize);

impl purlis_host::Code for Gives {
    fn take(
        &self,
        _project: &Path,
        _repo: &str,
        _bundle: Option<&Path>,
        refs: &[CodeRef],
        _checkout: Option<&str>,
    ) -> Result<Vec<CodeRef>, Refusal> {
        Ok(refs.to_vec())
    }

    fn give(
        &self,
        _project: &Path,
        _repo: &str,
        _have: &[String],
        into: &Path,
    ) -> Result<(Vec<CodeRef>, bool), Refusal> {
        std::fs::write(into, vec![b'x'; self.0]).unwrap();
        Ok((Vec::new(), true))
    }
}

#[test]
fn a_bundle_from_the_host_past_the_windows_bound_is_refused_there() {
    let runtime = runtime();
    let home = tempfile::tempdir().unwrap();
    let projects = std::fs::canonicalize(home.path()).unwrap().join("projects");
    std::fs::create_dir_all(&projects).unwrap();
    let hosting = Hosting::running_with(
        crate::sessionhost::Composes::as_the_tests_ship(),
        StandIns::default(),
    )
    .keeping_projects_in(projects)
    .moving_code_with(Gives(100));
    let link = runtime
        .block_on(a_link_to(hosting, || {}))
        .expect("the window speaks to the host")
        .holding_bundles_to(8);
    link.call(Command::HoldProject {
        project: PROJECT.into(),
        project_path: String::new(),
    })
    .expect("held where the host keeps it");
    let into = home.path().join("given.bundle");

    let started = Instant::now();
    let refused = link
        .fetch_code(PROJECT, REPO, Vec::new(), &into)
        .unwrap_err();

    assert!(started.elapsed() < PATIENCE, "{:?}", started.elapsed());
    assert!(refused.contains("larger than 8 bytes"), "{refused}");
}

#[test]
fn a_bundle_within_the_windows_bound_is_kept_whole() {
    let runtime = runtime();
    let home = tempfile::tempdir().unwrap();
    let projects = std::fs::canonicalize(home.path()).unwrap().join("projects");
    std::fs::create_dir_all(&projects).unwrap();
    let hosting = Hosting::running_with(
        crate::sessionhost::Composes::as_the_tests_ship(),
        StandIns::default(),
    )
    .keeping_projects_in(projects)
    .moving_code_with(Gives(8));
    let link = runtime
        .block_on(a_link_to(hosting, || {}))
        .expect("the window speaks to the host")
        .holding_bundles_to(8);
    link.call(Command::HoldProject {
        project: PROJECT.into(),
        project_path: String::new(),
    })
    .expect("held where the host keeps it");
    let into = home.path().join("given.bundle");

    let (_, wrote) = link.fetch_code(PROJECT, REPO, Vec::new(), &into).unwrap();

    assert!(wrote);
    assert_eq!(std::fs::read(&into).unwrap(), vec![b'x'; 8]);
}

#[test]
fn a_host_that_does_not_know_a_move_of_code_is_said_to_be_older_in_purliss_words() {
    let said = code_refused(session::CallError::Refused(Refusal {
        code: session::UNKNOWN_COMMAND.into(),
        why: "unknown command `take_code`".into(),
    }));
    assert!(said.contains("before session protocol 1.34"), "{said}");
    assert!(said.contains("Update it"), "{said}");
    let said = code_refused(session::CallError::Refused(Refusal {
        code: session::NOT_ALLOWED.into(),
        why: "not here".into(),
    }));
    assert!(said.contains("not here"), "{said}");
}

/// A link to a host that moves code, holding the project.
fn a_link_moving_code(runtime: &tokio::runtime::Runtime, home: &Path) -> HostLink {
    let projects = std::fs::canonicalize(home).unwrap().join("projects");
    std::fs::create_dir_all(&projects).unwrap();
    let hosting = Hosting::running_with(
        crate::sessionhost::Composes::as_the_tests_ship(),
        StandIns::default(),
    )
    .keeping_projects_in(projects)
    .moving_code_with(Gives(8));
    let link = runtime
        .block_on(a_link_to(hosting, || {}))
        .expect("the window speaks to the host");
    link.call(Command::HoldProject {
        project: PROJECT.into(),
        project_path: String::new(),
    })
    .expect("held where the host keeps it");
    link
}

/// **A send over the limit stops here, measured here** (spec #4275): nothing goes, and what it
/// needs is this machine's own measure of the bundle.
#[test]
fn a_bundle_over_the_limit_says_what_it_needs_and_what_to_raise_it_to() {
    let runtime = runtime();
    let home = tempfile::tempdir().unwrap();
    let link = a_link_moving_code(&runtime, home.path());
    let bundle = home.path().join("large.bundle");
    let file = std::fs::File::create(&bundle).unwrap();
    file.set_len((1024 << 20) + 1).unwrap();

    let started = Instant::now();
    let refused = link
        .send_code(PROJECT, REPO, Vec::new(), None, Some(&bundle))
        .unwrap_err();

    assert!(started.elapsed() < PATIENCE, "{:?}", started.elapsed());
    assert!(refused.contains("it needs 1025 MiB"), "{refused}");
    assert!(refused.contains("Largest code sent"), "{refused}");
    assert!(refused.contains("raise it to 1296 MiB"), "{refused}");
}

/// **A send out of time says its limit, by this machine's clock** (spec #4275).
#[test]
fn a_send_out_of_its_time_says_the_limit_and_double_it() {
    use purlis_core::poollimits::LimitId::CodeSendTime;
    let runtime = runtime();
    let home = tempfile::tempdir().unwrap();
    let link = a_link_moving_code(&runtime, home.path());
    let held = SendHeld {
        most: 1024,
        within: Duration::ZERO,
        given: Some(session::CodeHeld {
            most_bytes: 1024,
            within_secs: 0,
        }),
        late: Some(CodeSendTime.limit().waited(10, None)),
    };
    let given = held.given;

    let refused = link
        .write_on_a_stream(None, held, move |client, transfer| {
            Box::pin(async move {
                client
                    .take_code_held(PROJECT, REPO, transfer, Vec::new(), None, given)
                    .await
            })
        })
        .unwrap_err();

    assert!(refused.contains("Longest code send"), "{refused}");
    assert!(refused.contains("raise it to 20 minutes"), "{refused}");
}
