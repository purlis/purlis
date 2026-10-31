// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A browser's answer reaches the login on its runner, and nothing comes back** (#4044, spec
//! #4023 seam 5): a browser's request of this machine's loopback, carried over a paired link to a
//! runner's host, which hands it to the login listening on the runner's own loopback. The port on
//! this machine is closed once it is carried.

#![cfg(unix)]

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use purlis_core::profilestore::sign_in::callback::{self, Carry, Forward};
use purlis_core::profilestore::sign_in::waiting::Waiting;
use purlis_host::{ComposesNothing, Hosting, Pairing, Place, serve_answering};
use purlis_session_protocol::hostlink::{self, Handshake, Identity, LinkKey, Liveness, Pinned};
use purlis_session_protocol::session::{Client, ProfileStanding};
use tokio::net::UnixStream;
use tokio::sync::oneshot;

const STATE: &str = "Zm9yLXRoaXMtc2lnbi1pbi1hbG9uZQ";

fn identity(device: &str) -> Identity {
    Identity {
        device: device.into(),
        key: LinkKey::generate().unwrap(),
    }
}

fn pinned_of(identity: &Identity) -> Pinned {
    Pinned {
        device: identity.device.clone(),
        key: identity.key.public(),
    }
}

fn place_in(dir: &Path) -> Place {
    Place::at(
        dir.join("purlis-test").join("purlisd.sock"),
        dir.join("config").join("purlisd"),
    )
}

/// A runner's host paired with `desktop`, handing redirects on as `purlis serve`'s does.
async fn a_runner(
    place: &Place,
    runner: Identity,
    desktop: Pinned,
    waiting_in: PathBuf,
) -> oneshot::Sender<()> {
    let (stop, stopped) = oneshot::channel::<()>();
    let (ready, is_ready) = oneshot::channel::<()>();
    let pairing = Pairing {
        answering: Arc::new(move || {
            Ok(Some((
                Identity {
                    device: runner.device.clone(),
                    key: runner.key.clone(),
                },
                desktop.clone(),
            )))
        }),
        unpairing: Arc::new(|_| Ok(())),
        keeping: Arc::new(|_, _| Ok(ProfileStanding::SignedOut)),
        forgetting: Arc::new(|_, _| Ok(())),
        deleting_own_folders: Arc::new(|_, _| Ok(None)),
        listing: Arc::new(|_| Ok(Vec::new())),
        holding: Arc::new(|_, _, _| Ok((Vec::new(), false))),
        delivering: Arc::new(move |target| callback::deliver_waiting_in(&waiting_in, target)),
    };
    let place = place.clone();
    tokio::spawn(async move {
        serve_answering(
            &place,
            None,
            Hosting::new(ComposesNothing),
            Some(pairing),
            move || {
                let _ = ready.send(());
            },
            async move {
                let _ = stopped.await;
            },
        )
        .await
    });
    is_ready.await.expect("the host became ready");
    stop
}

/// The login on the runner: takes one request on its own port, answers it, and hands it back.
fn a_login_listening() -> std::thread::JoinHandle<String> {
    let listener = TcpListener::bind(("127.0.0.1", callback::PORT)).expect("the login's port");
    std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        let mut request = Vec::new();
        let mut piece = [0u8; 1024];
        while !request.windows(4).any(|end| end == b"\r\n\r\n") {
            let read = stream.read(&mut piece).unwrap();
            request.extend_from_slice(&piece[..read]);
        }
        stream
            .write_all(b"HTTP/1.1 200 OK\r\n\r\nthe login's own page")
            .unwrap();
        String::from_utf8(request).unwrap()
    })
}

#[test]
fn a_browsers_answer_reaches_the_login_on_its_runner_and_nothing_comes_back() {
    purlis_core::unsteered!();
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
        .unwrap();
    runtime.block_on(async {
        let dir = tempfile::tempdir().unwrap();
        let place = place_in(dir.path());
        let (runner, desktop) = (
            identity("01JB0RUNNER0000000000000000"),
            identity("01JA0DESKT0P00000000000000"),
        );
        let runner_pinned = pinned_of(&runner);
        let waiting_in = dir.path().join("sign-ins-waiting");
        let _waiting = Waiting::record(&waiting_in, STATE).expect("the login's record");
        let _stop = a_runner(&place, runner, pinned_of(&desktop), waiting_in).await;
        let stream = UnixStream::connect(place.link()).await.unwrap();
        let link = hostlink::connect(
            stream,
            Handshake::THIS_BUILD,
            &desktop,
            &runner_pinned,
            Liveness::default(),
        )
        .await
        .expect("the paired desktop is linked");
        let client = Client::new(link).0;
        let login = a_login_listening();
        let here = tokio::runtime::Handle::current();
        let carry: Carry = Arc::new(move |target: &str| {
            here.block_on(client.sign_in_redirect(target))
                .map_err(|why| why.to_string())
        });

        let forward = Forward::open(0, STATE, carry, |_| ()).expect("this machine listens");
        let port = forward.port();
        let answered = tokio::task::spawn_blocking(move || {
            let mut browser = TcpStream::connect(("127.0.0.1", port)).unwrap();
            write!(
                browser,
                "GET /auth/callback?code=c&state={STATE} HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\n\r\n"
            )
            .unwrap();
            browser
                .set_read_timeout(Some(Duration::from_secs(30)))
                .unwrap();
            let mut answered = String::new();
            let _ = browser.read_to_string(&mut answered);
            answered
        })
        .await
        .unwrap();

        assert!(answered.starts_with("HTTP/1.1 200"), "{answered}");
        assert!(!answered.contains("the login's own page"), "{answered}");
        let heard = login.join().unwrap();
        assert!(
            heard.starts_with(&format!(
                "GET /auth/callback?code=c&state={STATE} HTTP/1.1\r\n"
            )),
            "{heard}"
        );
        assert!(
            TcpStream::connect(("127.0.0.1", port)).is_err(),
            "this machine's port is closed once it is carried"
        );
    });
}
