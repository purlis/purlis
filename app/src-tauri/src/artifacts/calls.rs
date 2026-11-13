// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A page calls the catalogue queries it declared, on its one port** (ADR 0122 §7, #5969).
//!
//! The page posts `{ id, query: "<id>@<N>", params }` on `purlis.port`; purlis's script carries
//! it as `/.purlis/port/<load>/query/<id>?<{query, params}>` ([`asked`]), on the same port and
//! load as a data file's read. Each call is gated when it is asked, as a read is: the pin and
//! this machine's approval read again, then admitted only as the page declared it
//! ([`calls::admit`]), a plane-scoped one only with its own grant
//! ([`artifact_grant_plane`]); then answered as data in the catalogue's envelope
//! ([`purlis_core::catalogue::answer`]), of the workspace the artifact is pinned to.

use std::path::Path;

use purlis_core::artifact::{approval, calls, data, serve};
use purlis_core::catalogue::{Answer, ErrorKind};
use serde::Serialize;
use serde_json::Value;
use tauri::Manager;

use super::{APPROVALS_CHANGED, ArtifactApprovalChanged, Artifacts, Of, blocking, still_runs};
use crate::planes::Planes;

/// The most a call's `{query, params}` may be as the page sends it, percent-encoded.
const MOST_ASKED: usize = 8 << 10;

/// **What a page's query ask carries**, the query string after `?`: the declared id it calls
/// and its parameters; nothing when it is not that shape.
pub(super) fn asked(query: &str) -> Option<(String, Value)> {
    if query.len() > MOST_ASKED {
        return None;
    }
    let text = percent_encoding::percent_decode_str(query)
        .decode_utf8()
        .ok()?;
    let Value::Object(mut said) = serde_json::from_str::<Value>(&text).ok()? else {
        return None;
    };
    let Some(Value::String(id)) = said.remove("query") else {
        return None;
    };
    let params = said.remove("params").unwrap_or(Value::Null);
    said.is_empty().then_some((id, params))
}

/// One query a page declared, as its approval lists it in the bar.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
pub struct ArtifactQuery {
    /// `<id>@<N>`.
    pub id: String,
    /// `workspace` or `plane`.
    pub scope: String,
    /// Each declared parameter that widens what it reads, as `<name>: <what it then reads>`.
    pub widens: Vec<String>,
    /// That the id is deprecated or replaced, which the bar warns of.
    pub warning: Option<String>,
}

/// **What the approval lists of the queries `declared` names.**
pub(super) fn listed(declared: Option<&data::Declared>) -> Vec<ArtifactQuery> {
    let Some(declared) = declared else {
        return Vec::new();
    };
    calls::listed_with(&declared.queries, super::queried::declared_here)
        .into_iter()
        .map(|one| ArtifactQuery {
            id: one.id,
            scope: one.scope.as_str().to_owned(),
            widens: one.widens,
            warning: one.warning,
        })
        .collect()
}

impl Artifacts {
    /// What webview `label`'s bar lists of its page's queries as it is served now, and whether
    /// it may call its plane-scoped ones.
    pub(super) fn queries_of(
        &self,
        label: &str,
        config: Option<&Path>,
    ) -> (Vec<ArtifactQuery>, bool) {
        let served = self.served();
        let Some(one) = served.get(label) else {
            return (Vec::new(), false);
        };
        let plane = approval::plane_granted(
            config,
            &one.of.dir,
            &one.of.folder,
            &one.of.pin,
            &one.snapshot,
        );
        (listed(one.declared.as_ref()), plane)
    }
}

/// **The answer to a page calling `asked` with `params`**, gated again at the ask. An
/// extension's query is its viewer's own extension's to answer (#5971).
pub(super) fn answer<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
    config: Option<&Path>,
    of: &Of,
    snapshot: &serve::Snapshot,
    declared: Option<&data::Declared>,
    asked: &str,
    params: &Value,
) -> Answer {
    if super::queried::is_extension(asked) {
        return super::queried::answer(app, config, of, snapshot, declared, asked, params);
    }
    // The chats are the app's own record, not the plane's (#6235): admitted as any query is,
    // and answered from the project as it is held.
    if is_chats(asked) {
        return match admitted(config, of, snapshot, declared, asked, params) {
            Ok(_) => chats_answer(app, &of.plane, &of.workspace, params),
            Err(kind) => Answer::error(kind, chrono::Utc::now()),
        };
    }
    let root = app
        .try_state::<Planes>()
        .and_then(|planes| planes.held(&of.plane).ok())
        .map(|held| held.root().to_path_buf());
    answer_in(
        root.as_deref(),
        config,
        of,
        snapshot,
        declared,
        asked,
        params,
    )
}

/// Whether the query a page names, as declared (`<id>@<N>`) or bare, is `purlis.chats`.
pub(super) fn is_chats(asked: &str) -> bool {
    let id = asked.rsplit_once('@').map_or(asked, |(id, _)| id);
    purlis_core::catalogue::find(id).is_some_and(|(q, _)| q.name == purlis_core::chattools::CHATS)
}

/// **`purlis.chats` asked with `params`** of workspace `workspace` of project `plane` (#6235):
/// its parameters checked against its closed types, its data the app's own record, under the
/// cap every answer is held to.
pub(super) fn chats_answer<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
    plane: &crate::planes::PlaneId,
    workspace: &str,
    params: &Value,
) -> Answer {
    use purlis_core::catalogue::{answer, chats};
    let at = chrono::Utc::now();
    let asked = answer::checked(&chats::parameters(), params)
        .and_then(|args| chats::asked(&args).map_err(|_| ErrorKind::Refused));
    let asked = match asked {
        Ok(asked) => asked,
        Err(kind) => return Answer::error(kind, at),
    };
    let Some(held) = app
        .try_state::<Planes>()
        .and_then(|planes| planes.held(plane).ok())
    else {
        return Answer::error(ErrorKind::Failed, at);
    };
    capped(
        crate::chatlisting::answer(&held, plane, workspace, asked),
        at,
    )
}

/// `data` as an answer read `at`, or `too-large` where it is past the cap every answer is
/// held to.
fn capped(data: Value, at: chrono::DateTime<chrono::Utc>) -> Answer {
    use purlis_core::catalogue::answer::MOST_BYTES;
    if serde_json::to_vec(&data).map_or(usize::MAX, |b| b.len()) > MOST_BYTES {
        return Answer::error(ErrorKind::TooLarge, at);
    }
    Answer::data(data, at)
}

/// [`answer`], of the plane at `root` (none: the plane is no longer open).
fn answer_in(
    root: Option<&Path>,
    config: Option<&Path>,
    of: &Of,
    snapshot: &serve::Snapshot,
    declared: Option<&data::Declared>,
    asked: &str,
    params: &Value,
) -> Answer {
    let at = chrono::Utc::now();
    let query = match admitted(config, of, snapshot, declared, asked, params) {
        Ok(query) => query,
        Err(kind) => return Answer::error(kind, at),
    };
    let Some(root) = root else {
        return Answer::error(ErrorKind::Failed, at);
    };
    purlis_core::catalogue::answer::ask(
        root,
        &of.workspace,
        query,
        params,
        &as_the_person(),
        at,
        chrono::Local::now().naive_local(),
    )
}

/// **How a page's query asks the forge**: as the person in the window, in the background
/// (the operator's ruling of 2026-11-11, ADR 0122 as amended that day). ADR 0070's route sends
/// it on their sign-in, else their `gh` login, through the account's ETag store, and the
/// request budget holds it back once the account's hour is spent. The token stays in the
/// transport: the page is handed the answer's data, never a credential.
pub(super) fn as_the_person() -> purlis_core::catalogue::milestones::Reach<'static> {
    purlis_core::catalogue::milestones::Reach {
        caller: purlis_core::forge::Caller::window().background(),
        backend_of: &backend_of,
    }
}

/// A repo's backend, over the transport its forge resolves each caller to.
fn backend_of(repo: &purlis_core::forge::pr::Repo) -> Box<dyn purlis_core::forge::ForgeBackend> {
    repo.backend()
}

/// **The query a page calling `asked` with `params` may be answered by**, gated again at the
/// ask: the pin and this machine's approval still running its script, admitted only as declared
/// ([`calls::admit`]), and a plane-scoped one only with its grant; otherwise why not. A call and
/// a subscription (#5970) are admitted alike.
pub(super) fn admitted(
    config: Option<&Path>,
    of: &Of,
    snapshot: &serve::Snapshot,
    declared: Option<&data::Declared>,
    asked: &str,
    params: &Value,
) -> Result<&'static purlis_core::catalogue::Query, ErrorKind> {
    if !still_runs(config, of, snapshot) {
        return Err(ErrorKind::NotApproved);
    }
    let plane = approval::plane_granted(config, &of.dir, &of.folder, &of.pin, snapshot);
    let declared = declared.map_or(&[][..], |d| d.queries.as_slice());
    calls::admit(declared, plane, asked, params)
}

/// **Grants an approved artifact its plane-scoped queries** (ADR 0122 §7, #5969): a grant of its
/// own, from the person's press in purlis's bar, of exactly the pin and template their window
/// was served. A new approval of the folder drops it.
#[tauri::command]
#[specta::specta]
pub async fn artifact_grant_plane(
    app: tauri::AppHandle,
    window: tauri::Window,
    artifacts: tauri::State<'_, Artifacts>,
    label: String,
) -> Result<(), String> {
    let Some((of, fingerprint)) = artifacts.served_to(&label, window.label()) else {
        return Err("that artifact is not open in this window any more".into());
    };
    if !of.standing.runs() {
        return Err("approve the artifact's script first".into());
    }
    let config = crate::extensions::config_root()
        .map_err(|_| "this machine has no config home, so purlis keeps no approvals".to_owned())?;
    let (dir, folder, pin) = (of.dir.clone(), of.folder.clone(), of.pin.clone());
    blocking(move || approval::grant_plane(&config, &dir, &folder, &pin, &fingerprint)).await?;
    crate::windows::emit_for_plane(
        &app,
        &of.plane,
        APPROVALS_CHANGED,
        ArtifactApprovalChanged {
            workspace: of.workspace,
            folder: of.folder,
        },
    );
    Ok(())
}

#[cfg(all(test, not(windows)))]
mod tests {
    use purlis_core::artifact;
    use serde_json::json;

    use super::*;

    #[test]
    fn a_chats_answer_past_the_cap_is_too_large_and_one_at_it_is_given() {
        use purlis_core::catalogue::answer::MOST_BYTES;
        let at = chrono::Utc::now();
        // `{"chats":["…"]}` is 14 bytes around the text.
        let of = |text: usize| json!({ "chats": ["x".repeat(text)] });
        assert!(capped(of(MOST_BYTES - 14), at).outcome.is_ok());
        assert_eq!(
            capped(of(MOST_BYTES - 13), at).outcome,
            Err(ErrorKind::TooLarge)
        );
    }

    #[test]
    fn a_query_ask_carries_the_declared_id_and_its_params_and_nothing_else() {
        let encode = |v: &Value| {
            percent_encoding::utf8_percent_encode(
                &v.to_string(),
                percent_encoding::NON_ALPHANUMERIC,
            )
            .to_string()
        };
        assert_eq!(
            asked(&encode(
                &json!({ "query": "purlis.todo_list@1", "params": { "done": true } })
            )),
            Some(("purlis.todo_list@1".to_owned(), json!({ "done": true })))
        );
        assert_eq!(
            asked(&encode(&json!({ "query": "purlis.todo_list@1" }))),
            Some(("purlis.todo_list@1".to_owned(), Value::Null))
        );
        for not in [
            json!({ "query": 1 }),
            json!({ "params": {} }),
            json!({ "query": "purlis.todo_list@1", "read": "data.json" }),
            json!(["purlis.todo_list@1"]),
        ] {
            assert_eq!(asked(&encode(&not)), None, "{not}");
        }
        assert_eq!(asked("%ff"), None);
        assert_eq!(asked(&"a".repeat(MOST_ASKED + 1)), None);
    }

    /// A plane with workspace `alpha` holding artifact folder `board` whose page is `page`,
    /// pinned, and the snapshot it is served as.
    fn board(page: &str) -> (tempfile::TempDir, Of, serve::Snapshot) {
        let root = tempfile::tempdir().unwrap();
        std::fs::write(root.path().join("charter.toml"), "").unwrap();
        let ws = root.path().join("workspaces/alpha");
        let folder = ws.join("artifacts/board");
        std::fs::create_dir_all(&folder).unwrap();
        std::fs::write(folder.join("index.html"), page).unwrap();
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

    const PAGE: &str = r#"<script type="application/json" id="purlis">{"api":1,"queries":{"purlis.todo_list@1":[],"purlis.memory_search@1":["query"]}}</script><script>go()</script>"#;

    #[test]
    fn a_declared_query_is_answered_as_data_once_approved_and_an_undeclared_one_never() {
        let (root, of, snapshot) = board(PAGE);
        let config = tempfile::tempdir().unwrap();
        let declared = super::super::declared_by(&snapshot);
        let ask = |asked: &str, params: Value| {
            answer_in(
                Some(root.path()),
                Some(config.path()),
                &of,
                &snapshot,
                declared.as_ref(),
                asked,
                &params,
            )
            .outcome
            .map_err(|k| k.as_str())
        };

        // Not approved here: nothing is called.
        assert_eq!(ask("purlis.todo_list@1", json!({})), Err("not-approved"));

        approval::approve(
            config.path(),
            &of.dir,
            "board",
            &of.pin,
            snapshot.fingerprint(),
        )
        .unwrap();
        assert_eq!(
            ask("purlis.todo_list@1", json!({})),
            Ok(json!({ "done": false, "todos": [] }))
        );
        assert_eq!(
            ask("purlis.memory_search@1", json!({ "query": "x" })),
            Ok(json!({ "memories": [] }))
        );
        // Undeclared: another query, another version, a parameter not declared.
        assert_eq!(
            ask("purlis.change_status@1", json!({})),
            Err("not-approved")
        );
        assert_eq!(ask("purlis.todo_list@2", json!({})), Err("not-approved"));
        assert_eq!(
            ask("purlis.todo_list@1", json!({ "done": true })),
            Err("not-approved")
        );
        // Declared, and out of its type.
        assert_eq!(
            ask("purlis.memory_search@1", json!({ "query": 3 })),
            Err("refused")
        );
        // The plane is not open any more.
        let gone = answer_in(
            None,
            Some(config.path()),
            &of,
            &snapshot,
            declared.as_ref(),
            "purlis.todo_list@1",
            &json!({}),
        );
        assert_eq!(gone.outcome, Err(ErrorKind::Failed));
    }

    #[test]
    fn the_approval_lists_the_declared_queries_with_their_scope() {
        let (_root, _of, snapshot) = board(PAGE);
        let declared = super::super::declared_by(&snapshot);
        let listed = listed(declared.as_ref());
        assert_eq!(
            listed.iter().map(|q| q.id.as_str()).collect::<Vec<_>>(),
            ["purlis.todo_list@1", "purlis.memory_search@1"]
        );
        assert!(
            listed
                .iter()
                .all(|q| q.scope == "workspace" && q.warning.is_none())
        );
        assert!(super::listed(None).is_empty());
    }
}
