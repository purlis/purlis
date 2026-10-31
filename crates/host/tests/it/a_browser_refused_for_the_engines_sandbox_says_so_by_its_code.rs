// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! A browser whose engine cannot make its sandbox on the host's machine is refused with a code
//! of its own (1.36, #2899), `no_engine_sandbox`, in the binary's words; any other failure to
//! start is refused `not_started` as before. The browsers are stand-ins, so none of this needs
//! an engine.

#![cfg(unix)]

use purlis_host::standin::{StandInBrowsers, StandIns};
use purlis_host::{ComposesNothing, Hosting};
use purlis_session_protocol::auth::{Credentials, Scope};
use purlis_session_protocol::link;
use purlis_session_protocol::session::{self, CallError, Client, ProgramKind, StartProgram};
use tokio::io::duplex;

const PROJECT: &str = "01J9PRJCT00000000000000000";

static HELD: std::sync::LazyLock<Credentials> =
    std::sync::LazyLock::new(|| Credentials::mint().unwrap());

async fn client_of(hosting: &Hosting, project: &std::path::Path) -> Client {
    let (a, b) = duplex(4 << 20);
    let (client, served) = tokio::join!(
        link::connect(
            a,
            session::speaks(),
            Scope::LocalUi,
            HELD.of(Scope::LocalUi)
        ),
        link::serve_any(b, session::speaks(), &HELD)
    );
    tokio::spawn(session::serve(served.unwrap(), hosting.clone(), None));
    let (client, _views, _events) = Client::new(client.unwrap());
    client
        .hold_project(PROJECT, project.to_str().unwrap())
        .await
        .expect("the project is held");
    client
}

fn refusal(started: Result<String, CallError>) -> session::Refusal {
    match started {
        Err(CallError::Refused(refusal)) => refusal,
        other => panic!("refused, not {other:?}"),
    }
}

#[tokio::test]
async fn a_start_the_engines_sandbox_stops_is_refused_with_its_own_code_and_words() {
    let store = tempfile::tempdir().unwrap();
    let project = tempfile::tempdir().unwrap();
    let browsers = StandInBrowsers::in_folder(store.path().join("browsers"));
    let hosting =
        Hosting::running_with(ComposesNothing, StandIns::default()).with_browsers(browsers.clone());
    let window = client_of(&hosting, project.path()).await;

    browsers.fails_next_for_the_engines_sandbox();
    let refused = refusal(
        window
            .start_program(StartProgram::of(PROJECT, ProgramKind::Browser))
            .await,
    );
    assert_eq!(refused.code, session::NO_ENGINE_SANDBOX);
    assert_eq!(refused.why, purlis_host::standin::NO_SANDBOX_SAID);

    browsers.fails_next();
    let refused = refusal(
        window
            .start_program(StartProgram::of(PROJECT, ProgramKind::Browser))
            .await,
    );
    assert_eq!(refused.code, session::NOT_STARTED);
}
