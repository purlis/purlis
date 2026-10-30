// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! A runner's host on its real link socket (#2044): what the bridge a connector runs opens
//! (ADR 0078 §2). The desktop it is paired with is linked and served the session protocol as
//! `remote-link`; another key, a runner that is not paired, and what the runner keeps that
//! cannot be read are refused; the link socket is this user's alone and goes when the host ends.
//! The paired desktop has the runner keep its profiles (1.43, #2049).
//!
//! These bind a socket, which a sandboxed agent cannot: they first run on CI.

#![cfg(unix)]

use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::sync::{Arc, Mutex};

use purlis_host::{
    Answering, ComposesNothing, Forgetting, HostError, Hosting, Keeping, Listing, Pairing, Place,
    Unpairing, serve_answering,
};
use purlis_session_protocol::hostlink::{self, Handshake, Identity, LinkKey, Liveness, Pinned};
use purlis_session_protocol::link::Link;
use tokio::net::UnixStream;
use tokio::sync::oneshot;

const RUNNER: &str = "01JB0RUNNER0000000000000000";
const DESKTOP: &str = "01JA0DESKT0P00000000000000";

fn place_in(dir: &Path) -> Place {
    Place::at(
        dir.join("purlis-test").join("purlisd.sock"),
        dir.join("config").join("purlisd"),
    )
}

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

/// What the runner answers each link with, changed as a test goes.
type Paired = Arc<Mutex<Result<Option<(Identity, Pinned)>, String>>>;

fn answering(paired: &Paired) -> Answering {
    let paired = Arc::clone(paired);
    Arc::new(move || {
        paired
            .lock()
            .unwrap()
            .as_ref()
            .map(|found| {
                found.as_ref().map(|(ours, desktop)| {
                    (
                        Identity {
                            device: ours.device.clone(),
                            key: ours.key.clone(),
                        },
                        desktop.clone(),
                    )
                })
            })
            .map_err(Clone::clone)
    })
}

/// Takes the desktop out of the runner's peers, as `runnerlink::unpair` does: only the one
/// it is paired with.
fn unpairing(paired: &Paired) -> Unpairing {
    let paired = Arc::clone(paired);
    Arc::new(move |desktop: &Pinned| {
        let mut paired = paired.lock().unwrap();
        match &*paired {
            Ok(Some((_, kept))) if kept == desktop => {
                *paired = Ok(None);
                Ok(())
            }
            _ => Err("not this runner's peer".into()),
        }
    })
}

/// The profiles the runner was asked to keep, in order.
type KeptHere = Arc<Mutex<Vec<purlis_session_protocol::session::ProfileToKeep>>>;

/// Forgets the profile of a name from `kept`, for the desktop `desktop` alone; refused where
/// none of that name is kept.
fn forgetting(kept: &KeptHere, desktop: Option<Pinned>) -> Forgetting {
    let kept = Arc::clone(kept);
    Arc::new(move |asked, name| {
        assert_eq!(
            Some(asked),
            desktop.as_ref(),
            "forgotten for the desktop the link proved"
        );
        let mut kept = kept.lock().unwrap();
        let before = kept.len();
        kept.retain(|profile| profile.name != name);
        if kept.len() == before {
            return Err(format!("no profile named {name} is kept here"));
        }
        Ok(())
    })
}

/// Names each profile in `kept`, for the desktop `desktop` alone.
fn listing(kept: &KeptHere, desktop: Option<Pinned>) -> Listing {
    let kept = Arc::clone(kept);
    Arc::new(move |asked| {
        assert_eq!(
            Some(asked),
            desktop.as_ref(),
            "named for the desktop the link proved"
        );
        Ok(kept
            .lock()
            .unwrap()
            .iter()
            .map(|profile| profile.name.clone())
            .collect())
    })
}

/// Keeps each profile in `kept`, signed out there unless its name is `signed-in`, for the
/// desktop `desktop` alone.
fn keeping(kept: &KeptHere, desktop: Option<Pinned>) -> Keeping {
    use purlis_session_protocol::session::ProfileStanding;
    let kept = Arc::clone(kept);
    Arc::new(move |asked, profile| {
        assert_eq!(
            Some(asked),
            desktop.as_ref(),
            "kept for the desktop the link proved"
        );
        kept.lock().unwrap().push(profile.clone());
        Ok(if profile.name == "signed-in" {
            ProfileStanding::SignedIn
        } else {
            ProfileStanding::SignedOut
        })
    })
}

async fn hosting(
    place: &Place,
    paired: &Paired,
) -> (
    oneshot::Sender<()>,
    tokio::task::JoinHandle<Result<(), HostError>>,
) {
    hosting_keeping(place, paired, &KeptHere::default()).await
}

async fn hosting_keeping(
    place: &Place,
    paired: &Paired,
    kept: &KeptHere,
) -> (
    oneshot::Sender<()>,
    tokio::task::JoinHandle<Result<(), HostError>>,
) {
    let (stop, stopped) = oneshot::channel::<()>();
    let (ready, is_ready) = oneshot::channel::<()>();
    let desktop = paired
        .lock()
        .unwrap()
        .as_ref()
        .ok()
        .and_then(|found| found.as_ref().map(|(_, desktop)| desktop.clone()));
    let (place, pairing) = (
        place.clone(),
        Pairing {
            answering: answering(paired),
            unpairing: unpairing(paired),
            keeping: keeping(kept, desktop.clone()),
            forgetting: forgetting(kept, desktop.clone()),
            listing: listing(kept, desktop),
        },
    );
    let host = tokio::spawn(async move {
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
    if is_ready.await.is_err() {
        panic!("the host did not become ready: {:?}", host.await);
    }
    (stop, host)
}

async fn link(place: &Place, desktop: &Identity, runner: &Pinned) -> Result<Link, String> {
    let stream = UnixStream::connect(place.link())
        .await
        .map_err(|why| why.to_string())?;
    hostlink::connect(
        stream,
        Handshake::THIS_BUILD,
        desktop,
        runner,
        Liveness::default(),
    )
    .await
    .map_err(|why| why.to_string())
}

async fn list(link: &mut Link) -> serde_json::Value {
    called(link, "list").await
}

async fn called(link: &mut Link, command: &str) -> serde_json::Value {
    let call = serde_json::json!({ "call": { "id": 1, "command": command } });
    link.control()
        .send(bytes::Bytes::from(serde_json::to_vec(&call).unwrap()))
        .await
        .expect("the call was sent");
    let reply = link
        .control()
        .next()
        .await
        .expect("a reply came")
        .expect("the reply was read");
    serde_json::from_slice(&reply).unwrap()
}

#[tokio::test]
async fn the_desktop_a_runner_is_paired_with_is_linked_and_served() {
    let dir = tempfile::tempdir().unwrap();
    let place = place_in(dir.path());
    let (runner, desktop) = (identity(RUNNER), identity(DESKTOP));
    let runner_pinned = pinned_of(&runner);
    let paired: Paired = Arc::new(Mutex::new(Ok(Some((runner, pinned_of(&desktop))))));
    let (_stop, _host) = hosting(&place, &paired).await;

    let mut linked = link(&place, &desktop, &runner_pinned)
        .await
        .expect("the paired desktop is linked");
    assert_eq!(
        list(&mut linked).await,
        serde_json::json!({ "reply": { "re": 1, "ok": { "chats": [] } } })
    );
}

#[tokio::test]
async fn a_desktop_the_runner_is_not_paired_with_is_refused() {
    let dir = tempfile::tempdir().unwrap();
    let place = place_in(dir.path());
    let runner = identity(RUNNER);
    let runner_pinned = pinned_of(&runner);
    let paired: Paired = Arc::new(Mutex::new(Ok(Some((
        runner,
        pinned_of(&identity(DESKTOP)),
    )))));
    let (_stop, _host) = hosting(&place, &paired).await;

    let refused = link(&place, &identity(DESKTOP), &runner_pinned)
        .await
        .err()
        .expect("another desktop's key is refused");
    assert!(refused.contains("not the key of the desktop"), "{refused}");
}

#[tokio::test]
async fn a_runner_that_is_not_paired_or_cannot_read_its_pairing_links_nobody() {
    let dir = tempfile::tempdir().unwrap();
    let place = place_in(dir.path());
    let (runner, desktop) = (identity(RUNNER), identity(DESKTOP));
    let runner_pinned = pinned_of(&runner);
    let paired: Paired = Arc::new(Mutex::new(Ok(None)));
    let (_stop, _host) = hosting(&place, &paired).await;
    assert!(link(&place, &desktop, &runner_pinned).await.is_err());

    *paired.lock().unwrap() = Err("the runner's link-key could not be read safely".into());
    assert!(link(&place, &desktop, &runner_pinned).await.is_err());

    // Paired while the host runs: in force at the next link.
    *paired.lock().unwrap() = Ok(Some((runner, pinned_of(&desktop))));
    assert!(link(&place, &desktop, &runner_pinned).await.is_ok());
}

#[tokio::test]
async fn the_link_socket_is_this_users_alone_and_goes_when_the_host_ends() {
    let dir = tempfile::tempdir().unwrap();
    let place = place_in(dir.path());
    let paired: Paired = Arc::new(Mutex::new(Ok(None)));
    let (stop, host) = hosting(&place, &paired).await;

    let mode = std::fs::metadata(place.link())
        .unwrap()
        .permissions()
        .mode();
    assert_eq!(mode & 0o777, 0o600);
    assert_eq!(
        place.link(),
        place.socket.with_file_name("purlisd.link.sock")
    );

    stop.send(()).unwrap();
    host.await.unwrap().unwrap();
    assert!(!place.link().exists());
}

#[tokio::test]
async fn the_paired_desktop_unpairs_the_runner_and_is_linked_no_more() {
    let dir = tempfile::tempdir().unwrap();
    let place = place_in(dir.path());
    let (runner, desktop) = (identity(RUNNER), identity(DESKTOP));
    let runner_pinned = pinned_of(&runner);
    let paired: Paired = Arc::new(Mutex::new(Ok(Some((runner, pinned_of(&desktop))))));
    let (_stop, _host) = hosting(&place, &paired).await;

    let mut linked = link(&place, &desktop, &runner_pinned)
        .await
        .expect("the paired desktop is linked");
    assert_eq!(
        called(&mut linked, "unpair").await,
        serde_json::json!({ "reply": { "re": 1, "ok": "done" } })
    );
    assert!(matches!(*paired.lock().unwrap(), Ok(None)));
    // The runner ends the link it answered on, though the desktop keeps it open.
    let ended = tokio::time::timeout(std::time::Duration::from_secs(10), linked.control().next())
        .await
        .expect("the link ends after the unpairing");
    assert!(!matches!(ended, Some(Ok(_))), "{ended:?}");
    assert!(link(&place, &desktop, &runner_pinned).await.is_err());
}

/// **A profile's settings reach the runner in batches, however large, whole** (1.56, #4081),
/// over a real link: what one control frame could not carry is handed to the runner's keep as
/// the one set of files it checks; a keep naming another count keeps nothing; and a batch refused
/// drops what the link staged.
#[tokio::test]
async fn the_paired_desktop_stages_a_profiles_settings_in_batches_and_a_keep_takes_them_whole() {
    use purlis_session_protocol::link::MOST_CONTROL_FRAME_BYTES;
    use purlis_session_protocol::session::{
        Client, ProfileToKeep, SettingsBatch, SettingsFile, settings_in_batches,
    };
    let dir = tempfile::tempdir().unwrap();
    let place = place_in(dir.path());
    let (runner, desktop) = (identity(RUNNER), identity(DESKTOP));
    let runner_pinned = pinned_of(&runner);
    let paired: Paired = Arc::new(Mutex::new(Ok(Some((runner, pinned_of(&desktop))))));
    let kept = KeptHere::default();
    let (_stop, _host) = hosting_keeping(&place, &paired, &kept).await;
    let client = Client::new(link(&place, &desktop, &runner_pinned).await.unwrap()).0;
    let files = vec![
        SettingsFile {
            path: "settings.json".into(),
            bytes: b"{}".to_vec(),
        },
        SettingsFile {
            path: "skills/large/model.bin".into(),
            bytes: vec![7; 3 * MOST_CONTROL_FRAME_BYTES],
        },
    ];
    let keep = |staged| ProfileToKeep {
        name: "work".into(),
        harness: "claude".into(),
        sandbox_only: false,
        pinned: None,
        settings: None,
        settings_staged: Some(staged),
    };
    let batches = settings_in_batches("work", files.clone());
    assert!(batches.len() > 3, "{} batches", batches.len());

    // A count that is not what arrived keeps nothing, and leaves nothing staged.
    for batch in batches.clone() {
        client.stage_profile_settings(batch).await.unwrap();
    }
    let refused = client.keep_profile(keep(3)).await.unwrap_err();
    assert!(refused.refusal().is_some(), "{refused}");
    assert!(kept.lock().unwrap().is_empty());

    // A batch refused drops what came before it, so its keep has nothing to take.
    client
        .stage_profile_settings(batches[0].clone())
        .await
        .unwrap();
    let another = SettingsBatch {
        profile: "another".into(),
        pieces: Vec::new(),
    };
    let refused = client.stage_profile_settings(another).await.unwrap_err();
    assert!(refused.refusal().is_some(), "{refused}");
    // What follows may be refused too, a piece of a file whose start went with the rest.
    for batch in batches[1..].iter().cloned() {
        let _ = client.stage_profile_settings(batch).await;
    }
    let refused = client.keep_profile(keep(2)).await.unwrap_err();
    assert!(refused.refusal().is_some(), "{refused}");
    assert!(kept.lock().unwrap().is_empty());

    for batch in batches {
        client.stage_profile_settings(batch).await.unwrap();
    }
    client.keep_profile(keep(2)).await.unwrap();
    let handed = kept.lock().unwrap().pop().expect("it was kept");
    assert_eq!(handed.settings, Some(files));
    assert_eq!(handed.settings_staged, None);
}

/// **The paired desktop has the runner keep its profile** (1.43, #2049): the runner is handed the
/// profile's name, harness, sandbox-only mark and pin, and answers whether it is signed in there.
#[tokio::test]
async fn the_paired_desktop_has_the_runner_keep_its_profile_and_hears_where_it_stands() {
    let dir = tempfile::tempdir().unwrap();
    let place = place_in(dir.path());
    let (runner, desktop) = (identity(RUNNER), identity(DESKTOP));
    let runner_pinned = pinned_of(&runner);
    let paired: Paired = Arc::new(Mutex::new(Ok(Some((runner, pinned_of(&desktop))))));
    let kept = KeptHere::default();
    let (_stop, _host) = hosting_keeping(&place, &paired, &kept).await;

    let linked = link(&place, &desktop, &runner_pinned)
        .await
        .expect("the paired desktop is linked");
    let client = purlis_session_protocol::session::Client::new(linked).0;
    let profile = |name: &str| purlis_session_protocol::session::ProfileToKeep {
        name: name.into(),
        harness: "claude".into(),
        sandbox_only: false,
        pinned: Some("2.1.300".into()),
        settings: None,
        settings_staged: None,
    };
    assert_eq!(
        client.keep_profile(profile("signed-in")).await.unwrap(),
        purlis_session_protocol::session::ProfileStanding::SignedIn
    );
    assert_eq!(
        client.keep_profile(profile("work")).await.unwrap(),
        purlis_session_protocol::session::ProfileStanding::SignedOut
    );
    assert_eq!(
        *kept.lock().unwrap(),
        vec![profile("signed-in"), profile("work")]
    );
}

/// **The paired desktop has the runner forget its profile** (1.52, #4047): asked by name, the
/// runner forgets it, and a refusal there is said to the desktop. The desktop hears which
/// profiles the runner keeps for it before and after (1.57, #4060).
#[tokio::test]
async fn the_paired_desktop_has_the_runner_forget_its_profile() {
    let dir = tempfile::tempdir().unwrap();
    let place = place_in(dir.path());
    let (runner, desktop) = (identity(RUNNER), identity(DESKTOP));
    let runner_pinned = pinned_of(&runner);
    let paired: Paired = Arc::new(Mutex::new(Ok(Some((runner, pinned_of(&desktop))))));
    let kept = KeptHere::default();
    let (_stop, _host) = hosting_keeping(&place, &paired, &kept).await;
    let linked = link(&place, &desktop, &runner_pinned)
        .await
        .expect("the paired desktop is linked");
    let client = purlis_session_protocol::session::Client::new(linked).0;
    client
        .keep_profile(purlis_session_protocol::session::ProfileToKeep {
            name: "work".into(),
            harness: "claude".into(),
            sandbox_only: false,
            pinned: None,
            settings: None,
            settings_staged: None,
        })
        .await
        .unwrap();

    assert_eq!(
        client.kept_profiles().await.unwrap(),
        vec!["work".to_owned()]
    );
    client.forget_profile("work").await.expect("forgotten");

    assert!(client.kept_profiles().await.unwrap().is_empty());

    assert!(kept.lock().unwrap().is_empty());
    let refused = client.forget_profile("work").await.unwrap_err();
    assert!(
        refused.to_string().contains("no profile named work"),
        "{refused}"
    );
}
