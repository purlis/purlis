// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! A browser's first start downloads its engine (#2207): the host pushes how far the download
//! has come to each link of 1.37 or later that may start a browser, and never to an older one;
//! and the window's `cancel_engine_download` stops it, the start refused in words and nothing of
//! it kept. The browser is a stand-in, so none of this needs an engine.

#![cfg(unix)]

use std::sync::Arc;
use std::time::Duration;

use purlis_host::standin::{StandInBrowsers, StandIns};
use purlis_host::{ComposesNothing, Hosting};
use purlis_session_protocol::auth::{Credentials, Scope};
use purlis_session_protocol::link;
use purlis_session_protocol::session::{
    self, Client, Events, ProgramKind, Pushed, StartProgram, Told,
};
use purlis_session_protocol::version::{Speaks, Version};
use tokio::io::duplex;
use tokio::time::timeout;

const PROJECT: &str = "01J9PRJCT00000000000000000";
const PROGRAM: &str = "01J9BROWSER000000000000000";
const PATIENCE: Duration = Duration::from_secs(5);

static HELD: std::sync::LazyLock<Credentials> =
    std::sync::LazyLock::new(|| Credentials::mint().unwrap());

/// A link of the window's to `hosting`, speaking `speaks`, holding the project and subscribed.
async fn a_window(hosting: &Hosting, speaks: Speaks) -> (Arc<Client>, Events) {
    let (a, b) = duplex(4 << 20);
    let (client, served) = tokio::join!(
        link::connect(a, speaks, Scope::LocalUi, HELD.of(Scope::LocalUi)),
        link::serve_any(b, session::speaks(), &HELD)
    );
    tokio::spawn(session::serve(served.unwrap(), hosting.clone(), None));
    let (client, _views, events) = Client::new(client.unwrap());
    client
        .subscribe(session::Since::new())
        .await
        .expect("subscribed");
    (Arc::new(client), events)
}

/// What the host pushed of the engine's download, until nothing more comes for a moment.
async fn downloads_told(events: &mut Events) -> Vec<(String, u64, Option<u64>)> {
    let mut told = Vec::new();
    while let Ok(Some(next)) = timeout(Duration::from_millis(300), events.recv()).await {
        if let Told::Pushed(Pushed::EngineDownload {
            program,
            done,
            total,
        }) = next
        {
            told.push((program, done, total));
        }
    }
    told
}

fn hosting() -> (
    Hosting,
    StandInBrowsers,
    tempfile::TempDir,
    tempfile::TempDir,
) {
    let store = tempfile::tempdir().unwrap();
    let browsers = StandInBrowsers::in_folder(store.path().join("browsers")).holding_starts();
    let hosting =
        Hosting::running_with(ComposesNothing, StandIns::default()).with_browsers(browsers.clone());
    let project = tempfile::tempdir().unwrap();
    (hosting, browsers, store, project)
}

fn start() -> StartProgram {
    StartProgram {
        program: Some(PROGRAM.into()),
        ..StartProgram::of(PROJECT, ProgramKind::Browser)
    }
}

#[tokio::test]
async fn a_download_is_pushed_to_a_window_that_knows_of_downloads_and_never_to_an_older_one() {
    let (hosting, browsers, _store, project) = hosting();
    let (window, mut events) = a_window(&hosting, session::speaks()).await;
    let older = Speaks::new([Version {
        major: 1,
        minor: session::KNOWS_ENGINE_DOWNLOADS.minor - 1,
    }]);
    let (older_window, mut older_events) = a_window(&hosting, older).await;
    for client in [&window, &older_window] {
        client
            .hold_project(PROJECT, project.path().to_str().unwrap())
            .await
            .expect("held");
    }

    let starting = tokio::spawn({
        let window = Arc::clone(&window);
        async move { window.start_program(start()).await }
    });
    timeout(PATIENCE, async {
        while browsers.waiting() == 0 {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("the start waits on its download");
    let download = browsers
        .download()
        .expect("the start was given its download");
    download.tell(1_048_576, Some(158_334_976));
    download.tell(158_334_976, Some(158_334_976));

    let told = downloads_told(&mut events).await;
    let told_older = downloads_told(&mut older_events).await;
    // Let go before anything is asserted, so a failing check never leaves the start waiting.
    browsers.let_starts_go();
    timeout(PATIENCE, starting)
        .await
        .unwrap()
        .unwrap()
        .expect("the browser starts");

    assert_eq!(
        told,
        [
            (PROGRAM.to_owned(), 1_048_576, Some(158_334_976)),
            (PROGRAM.to_owned(), 158_334_976, Some(158_334_976)),
        ]
    );
    assert_eq!(told_older, [], "a link before 1.37 is pushed nothing of it");
}

#[tokio::test]
async fn a_cancelled_download_refuses_the_start_in_words_and_keeps_nothing_of_it() {
    let (hosting, browsers, store, project) = hosting();
    let (window, _events) = a_window(&hosting, session::speaks()).await;
    window
        .hold_project(PROJECT, project.path().to_str().unwrap())
        .await
        .expect("held");

    let starting = tokio::spawn({
        let window = Arc::clone(&window);
        async move { window.start_program(start()).await }
    });
    timeout(PATIENCE, async {
        while browsers.waiting() == 0 {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("the start waits on its download");

    window
        .cancel_engine_download(PROGRAM)
        .await
        .expect("cancelled");
    let cancelled = browsers.download().expect("its download").is_cancelled();
    browsers.let_starts_go();
    assert!(cancelled, "the download is told it is cancelled");

    let refused = timeout(PATIENCE, starting)
        .await
        .unwrap()
        .unwrap()
        .unwrap_err();
    let refusal = refused.refusal().expect("a refusal");
    assert!(
        refusal.why.contains(purlis_host::ENGINE_DOWNLOAD_CANCELLED),
        "{}",
        refusal.why
    );
    assert!(
        !store.path().join("browsers").join(PROGRAM).exists(),
        "nothing of the browser is kept"
    );
    // Cancelling once it is over is answered all the same.
    window
        .cancel_engine_download(PROGRAM)
        .await
        .expect("nothing to cancel is no refusal");
}

#[tokio::test]
async fn a_start_that_names_one_still_downloading_is_refused() {
    let (hosting, browsers, _store, project) = hosting();
    let (window, _events) = a_window(&hosting, session::speaks()).await;
    window
        .hold_project(PROJECT, project.path().to_str().unwrap())
        .await
        .expect("held");
    let starting = tokio::spawn({
        let window = Arc::clone(&window);
        async move { window.start_program(start()).await }
    });
    timeout(PATIENCE, async {
        while browsers.waiting() == 0 {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("the start waits on its download");

    let again = timeout(PATIENCE, window.start_program(start())).await;

    // Let go before anything is asserted, so a failing check never leaves the start waiting.
    browsers.let_starts_go();
    let again = again.expect("the second start is answered without waiting");
    timeout(PATIENCE, starting)
        .await
        .unwrap()
        .unwrap()
        .expect("the first starts");
    match again {
        Err(purlis_session_protocol::session::CallError::Refused(refusal)) => {
            assert_eq!(refusal.code, session::MALFORMED, "{refusal:?}");
            assert!(refusal.why.contains(PROGRAM), "{refusal:?}");
        }
        other => panic!("the second start was not refused: {other:?}"),
    }
}
