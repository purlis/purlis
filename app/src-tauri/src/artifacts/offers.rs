// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A page offers actions; it never performs one** (ADR 0122 §8, #5972).
//!
//! The page posts the whole set it offers now, `{ id, offers: [{ does, subject }] }`, on
//! `purlis.port`; purlis's script carries it as `/.purlis/port/<load>/offers/<id>/<n>?<{offers}>`
//! ([`heard`]), on the same port and load as a read, `<n>` counting the load's sets from 1 so
//! the newest one said is the one drawn however the sets are gated (#6232). It is gated when it is said, as a read is:
//! only on the load whose port is open, and only while the pin and this machine's approval still
//! run the page's script ([`super::still_runs`]); then read in closed types
//! ([`purlis_core::artifact::offers::asked`]). What passes is handed to the page's own window
//! ([`ARTIFACT_OFFERS`]), which draws each offer it can resolve in its catalogue, in purlis's
//! bar, for the person to press. Nothing here, and no command, does what an offer names.
//!
//! A load that begins again withdraws its offers ([`withdrawn`]): the window draws none until
//! the new load offers its own.

use std::path::Path;

use purlis_core::artifact::offers;
use purlis_core::artifact::serve;
use purlis_core::catalogue::{Answer, ErrorKind};
use serde::Serialize;
use tauri::{Emitter, Manager, Runtime};

use super::{Artifacts, Ask, Of, delivery, is_load, page_id, still_runs};

/// What the page's window hears when the page's offers change.
pub(crate) const ARTIFACT_OFFERS: &str = "artifact-offers";

/// One action a page offers: a `Does` id of the window's catalogue and its subject.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
pub struct ArtifactOffer {
    pub does: String,
    pub subject: String,
}

/// What [`ARTIFACT_OFFERS`] carries: the artifact webview's label, and every offer its page
/// makes now (none: it withdrew them, or its page began again).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
pub struct ArtifactOffers {
    pub label: String,
    pub offers: Vec<ArtifactOffer>,
}

/// A set of offers a page said on its port: its load, its id, its number among the load's
/// sets, and the set as it was sent.
#[derive(Debug, PartialEq, Eq)]
struct Said {
    load: String,
    id: u64,
    n: u64,
    query: String,
}

/// **What a request for `path` with `query` says, when it is a page's offers**:
/// `/.purlis/port/<load>/offers/<id>/<n>`.
fn said(path: &str, query: Option<&str>) -> Option<Said> {
    let rest = path.strip_prefix(serve::OWN_PATH)?.strip_prefix("port/")?;
    let (load, said) = rest.split_once('/')?;
    let (id, n) = said.strip_prefix("offers/")?.split_once('/')?;
    is_load(load).then_some(())?;
    Some(Said {
        load: load.to_owned(),
        id: page_id(id)?,
        n: page_id(n)?,
        query: query?.to_owned(),
    })
}

impl Artifacts {
    /// What gating offers from webview `label`'s load `load` needs, while that load's port is
    /// still the open one: the artifact, the bytes served, and its window.
    fn offering(&self, label: &str, load: &str) -> Option<(Of, serve::Snapshot, String)> {
        self.served()
            .get(label)
            .filter(|one| one.load.as_deref() == Some(load))
            .map(|one| (one.of.clone(), one.snapshot.clone(), one.window.clone()))
    }

    /// **Hands set `n` of load `load`'s offers to the window with `draw`**, while that load's
    /// port is still open, only if no newer set of it was taken; `draw` runs under the lock, so
    /// two sets gated on two threads reach the window in the order the page said them (#6232).
    pub(super) fn newest(&self, label: &str, load: &str, n: u64, draw: impl FnOnce()) {
        let mut served = self.served();
        let Some(one) = served
            .get_mut(label)
            .filter(|one| one.load.as_deref() == Some(load))
        else {
            return;
        };
        if n > one.offered {
            one.offered = n;
            draw();
        }
    }

    /// The window artifact webview `label` is in.
    fn window_of(&self, label: &str) -> Option<String> {
        self.served().get(label).map(|one| one.window.clone())
    }
}

/// **The offers a page said, gated again as they are said**: `not-approved` unless the pin is
/// still the one served and this machine's approval still runs its script, `refused` unless the
/// set is offers in closed types.
fn admitted(
    config: Option<&Path>,
    of: &Of,
    snapshot: &serve::Snapshot,
    query: &str,
) -> Result<Vec<ArtifactOffer>, ErrorKind> {
    if !still_runs(config, of, snapshot) {
        return Err(ErrorKind::NotApproved);
    }
    let set = offers::asked(query).ok_or(ErrorKind::Refused)?;
    Ok(set
        .into_iter()
        .map(|one| ArtifactOffer {
            does: one.does,
            subject: one.subject,
        })
        .collect())
}

/// **Webview `label` asked `path` with `query`**: when it is a set of offers on its open port,
/// gated off the scheme's thread, handed to its window, and answered to the page with how many
/// purlis took, or why none.
pub(crate) fn heard<R: Runtime>(
    app: &tauri::AppHandle<R>,
    label: &str,
    path: &str,
    query: Option<&str>,
) {
    let Some(said) = said(path, query) else {
        return;
    };
    let app = app.clone();
    let label = label.to_owned();
    tauri::async_runtime::spawn_blocking(move || {
        let Some(artifacts) = app.try_state::<Artifacts>() else {
            return;
        };
        let Some((of, snapshot, window)) = artifacts.offering(&label, &said.load) else {
            return;
        };
        let config = purlis_core::machine::config_root();
        let now = chrono::Utc::now();
        let answer = match admitted(config.as_deref(), &of, &snapshot, &said.query) {
            Ok(offers) => {
                let taken = offers.len();
                artifacts.newest(&label, &said.load, said.n, || {
                    let _ = app.emit_to(
                        tauri::EventTarget::webview(window.as_str()),
                        ARTIFACT_OFFERS,
                        ArtifactOffers {
                            label: label.clone(),
                            offers,
                        },
                    );
                });
                Answer::data(serde_json::json!({ "offered": taken }), now)
            }
            Err(kind) => Answer::error(kind, now),
        };
        if artifacts.offering(&label, &said.load).is_none() {
            return;
        }
        let Some(webview) = app.get_webview(&label) else {
            return;
        };
        let ask = Ask {
            label: label.clone(),
            load: said.load,
            id: said.id,
            name: String::new(),
            params: None,
            live: None,
        };
        let _ = webview.eval(delivery(&ask, &answer));
    });
}

/// **Artifact webview `label`'s page begins again**: its window draws none of the offers its
/// last load made.
pub(crate) fn withdrawn<R: Runtime>(app: &tauri::AppHandle<R>, label: &str) {
    let Some(window) = app
        .try_state::<Artifacts>()
        .and_then(|artifacts| artifacts.window_of(label))
    else {
        return;
    };
    let _ = app.emit_to(
        tauri::EventTarget::webview(window.as_str()),
        ARTIFACT_OFFERS,
        ArtifactOffers {
            label: label.to_owned(),
            offers: Vec::new(),
        },
    );
}

#[cfg(all(test, not(windows)))]
mod tests {
    use purlis_core::artifact::{self, approval};

    use super::*;

    const LOAD: &str = "0123456789abcdef0123456789abcdef";

    fn sent(text: &str) -> String {
        percent_encoding::utf8_percent_encode(text, percent_encoding::NON_ALPHANUMERIC).to_string()
    }

    #[test]
    fn a_set_of_offers_is_heard_only_on_the_port_path_with_a_load_and_an_id() {
        let set = sent(r#"{"offers":[]}"#);
        assert_eq!(
            said(&format!("/.purlis/port/{LOAD}/offers/3/1"), Some(&set)),
            Some(Said {
                load: LOAD.to_owned(),
                id: 3,
                n: 1,
                query: set.clone(),
            })
        );
        for not in [
            format!("/.purlis/port/{LOAD}/offers/x/1"),
            format!("/.purlis/port/{LOAD}/offers/3"),
            format!("/.purlis/port/{LOAD}/offers/3/"),
            format!("/.purlis/port/{LOAD}/offers/3/x"),
            format!("/.purlis/port/{LOAD}/offers/"),
            format!("/.purlis/port/{LOAD}/perform/3"),
            format!("/.purlis/port/{LOAD}/read/3"),
            "/.purlis/port/short/offers/3".to_owned(),
            format!("/port/{LOAD}/offers/3"),
        ] {
            assert_eq!(said(&not, Some(&set)), None, "{not}");
        }
        assert_eq!(
            said(&format!("/.purlis/port/{LOAD}/offers/3/1"), None),
            None
        );
    }

    /// A plane with workspace `alpha` holding artifact folder `board`, pinned, with script.
    fn board() -> (tempfile::TempDir, Of, serve::Snapshot) {
        let root = tempfile::tempdir().unwrap();
        std::fs::write(root.path().join("charter.toml"), "").unwrap();
        let ws = root.path().join("workspaces/alpha");
        let folder = ws.join("artifacts/board");
        std::fs::create_dir_all(&folder).unwrap();
        std::fs::write(folder.join("index.html"), "<script>offer()</script>").unwrap();
        let request = artifact::Request {
            folder: "board".to_owned(),
            ..artifact::Request::default()
        };
        let pin = match artifact::pin(root.path(), &ws, &request).unwrap() {
            artifact::Pinned::New(pin) => pin.id,
            other => panic!("{other:?}"),
        };
        let snapshot = serve::Snapshot::read(&folder, "board").unwrap();
        let of = Of {
            plane: serde_json::from_str("\"plane\"").unwrap(),
            workspace: "alpha".to_owned(),
            dir: ws,
            folder: "board".to_owned(),
            pin,
            standing: approval::Standing::Approved,
            then: None,
        };
        (root, of, snapshot)
    }

    #[test]
    fn a_page_s_offers_are_taken_only_once_its_script_is_approved_here_and_in_closed_types() {
        let (_root, of, snapshot) = board();
        let config = tempfile::tempdir().unwrap();
        let set = sent(r#"{"offers":[{"does":"todo.done","subject":"fix-login"}]}"#);

        // Not approved on this machine, whatever the window was told when it was served.
        assert_eq!(
            admitted(Some(config.path()), &of, &snapshot, &set),
            Err(ErrorKind::NotApproved)
        );
        assert_eq!(
            admitted(None, &of, &snapshot, &set),
            Err(ErrorKind::NotApproved)
        );

        approval::approve(
            config.path(),
            &of.dir,
            "board",
            &of.pin,
            snapshot.fingerprint(),
        )
        .unwrap();
        assert_eq!(
            admitted(Some(config.path()), &of, &snapshot, &set),
            Ok(vec![ArtifactOffer {
                does: "todo.done".to_owned(),
                subject: "fix-login".to_owned(),
            }])
        );
        // Words of its own for the button are not an offer.
        let worded = sent(
            r#"{"offers":[{"does":"todo.done","subject":"a","title":"Mark everything done"}]}"#,
        );
        assert_eq!(
            admitted(Some(config.path()), &of, &snapshot, &worded),
            Err(ErrorKind::Refused)
        );
    }

    #[test]
    fn offers_are_taken_only_from_the_load_whose_port_is_open() {
        let (_root, of, snapshot) = board();
        let artifacts = Artifacts::default();
        artifacts.served().insert(
            "artifact-1".to_owned(),
            super::super::Served {
                window: "main".to_owned(),
                host: "a1".to_owned(),
                snapshot,
                of,
                theme: String::new(),
                declared: None,
                load: None,
                offered: 0,
            },
        );
        assert!(artifacts.offering("artifact-1", LOAD).is_none());
        artifacts.served().get_mut("artifact-1").unwrap().load = Some(LOAD.to_owned());
        assert_eq!(
            artifacts.offering("artifact-1", LOAD).map(|(_, _, w)| w),
            Some("main".to_owned())
        );
        assert!(
            artifacts
                .offering("artifact-1", "fedcba9876543210fedcba9876543210")
                .is_none()
        );
        assert!(artifacts.offering("artifact-2", LOAD).is_none());
        artifacts.reloaded("artifact-1");
        assert!(artifacts.offering("artifact-1", LOAD).is_none());
        assert_eq!(artifacts.window_of("artifact-1").as_deref(), Some("main"));
    }

    /// **The set a page said last is the one drawn, whatever order its sets are gated in**
    /// (#6232): each is gated on a thread of its own, so a page that offers twice in a row, as
    /// each of two answers comes back, could have its fuller set drawn over by the one before.
    #[test]
    fn only_a_set_newer_than_the_one_taken_is_handed_to_the_window() {
        let (_root, of, snapshot) = board();
        let artifacts = Artifacts::default();
        artifacts.served().insert(
            "artifact-1".to_owned(),
            super::super::Served {
                window: "main".to_owned(),
                host: "a1".to_owned(),
                snapshot,
                of,
                theme: String::new(),
                declared: None,
                load: Some(LOAD.to_owned()),
                offered: 0,
            },
        );
        let mut drawn = Vec::new();
        // The page said set 1 and then set 2; set 2 was gated first.
        for n in [2, 1, 2, 3] {
            artifacts.newest("artifact-1", LOAD, n, || drawn.push(n));
        }
        assert_eq!(drawn, [2, 3]);
        // Nor from another load, or once the port is gone.
        artifacts.newest("artifact-1", "fedcba9876543210fedcba9876543210", 9, || {
            drawn.push(9)
        });
        artifacts.reloaded("artifact-1");
        artifacts.newest("artifact-1", LOAD, 10, || drawn.push(10));
        assert_eq!(drawn, [2, 3]);
        // A new load counts from its own first set.
        artifacts.served().get_mut("artifact-1").unwrap().load = Some(LOAD.to_owned());
        artifacts.served().get_mut("artifact-1").unwrap().offered = 0;
        artifacts.newest("artifact-1", LOAD, 1, || drawn.push(1));
        assert_eq!(drawn, [2, 3, 1]);
    }
}
