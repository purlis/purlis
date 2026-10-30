// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use super::*;
use serde_json::json;

fn plane() -> (tempfile::TempDir, PathBuf) {
    let dir = tempfile::tempdir().unwrap();
    let root = std::fs::canonicalize(dir.path()).unwrap();
    std::fs::write(
        root.join("charter.toml"),
        "[[forge]]\nkind = \"github\"\nowner = \"acme\"\n",
    )
    .unwrap();
    std::fs::create_dir_all(root.join("workspaces/alpha")).unwrap();
    (dir, root)
}

/// #1249 U2: recording a clone waits for the manifest's lock and reads under it, so a
/// row another writer dropped or added while it waited stays as that writer left it.
#[test]
fn recording_a_clone_racing_another_writer_loses_neither_change() {
    let (_dir, root) = plane();
    let ws = Plane::open(&root).workspace("alpha").unwrap();
    ws.write_manifest(&json!({"name": "alpha", "repos": [{"name": "gone"}]}))
        .unwrap();
    let held = ws.manifest_lock().unwrap();
    let recording = {
        let root = root.clone();
        std::thread::spawn(move || {
            let mut said = Vec::new();
            record_membership(
                &Request {
                    root: &root,
                    ws: "alpha",
                    repos: &[],
                    now: chrono::Utc::now(),
                    author: "tester",
                    hosts: None,
                    watch: None,
                },
                &["widget".to_owned()],
                &mut |line| said.push(line),
            );
            said
        })
    };
    std::thread::sleep(std::time::Duration::from_millis(300));
    assert_eq!(
        ws.manifest().0.unwrap()["repos"],
        json!([{"name": "gone"}]),
        "the clone wrote while another writer held the lock"
    );
    // The other writer — a removal — drops a row while it holds the lock.
    ws.write_manifest(&json!({"name": "alpha", "repos": []}))
        .unwrap();
    drop(held);
    let said = recording.join().unwrap();
    assert_eq!(
        ws.manifest().0.unwrap()["repos"],
        json!([{"name": "widget"}]),
        "{said:?}"
    );
}

#[test]
fn a_host_outside_a_sandboxed_chats_egress_is_named_and_one_inside_is_let_through() {
    let url = "https://github.com/acme/widget.git";
    let refused = unreachable_host(&["gitlab.com".to_owned()], url).expect("refused");
    assert!(refused.contains("'github.com'"), "{refused}");
    assert!(refused.contains("[sandbox] egress"), "{refused}");
    assert_eq!(unreachable_host(&["github.com".to_owned()], url), None);
    assert!(
        unreachable_host(&[], url).is_some(),
        "no host listed reaches nothing"
    );
}

#[test]
fn a_clone_for_a_sandboxed_chat_never_runs_git_against_a_host_it_may_not_reach() {
    let (_dir, root) = plane();
    std::fs::create_dir_all(root.join("inventory")).unwrap();
    std::fs::write(
        root.join("inventory/repos.json"),
        json!({"group": "acme", "count": 1, "repos": [{
            "name": "widget",
            "path_with_namespace": "acme/widget",
            "web_url": "https://github.com/acme/widget",
            "forge": "github",
        }]})
        .to_string(),
    )
    .unwrap();
    let mut said = Vec::new();
    let code = clone(
        &Request {
            root: &root,
            ws: "alpha",
            repos: &["widget".to_owned()],
            now: chrono::Utc::now(),
            author: "tester",
            hosts: Some(&["gitlab.com".to_owned()]),
            watch: None,
        },
        &mut |line| said.push(line),
    );
    assert_eq!(code, 1, "{said:?}");
    assert!(
        said.iter()
            .any(|line| matches!(line, Say::Fail(text) if text.contains("'github.com'"))),
        "{said:?}"
    );
    assert!(
        !root.join("workspaces/alpha/widget").exists(),
        "git ran against a host the chat may not reach"
    );
}

/// A plane whose inventory holds one repo, `widget`, at a host the plane does not manage:
/// the clone of it is refused before git runs, so a test sees everything up to the clone
/// and no network.
fn plane_with_an_unreachable_widget() -> (tempfile::TempDir, PathBuf) {
    let (dir, root) = plane();
    std::fs::create_dir_all(root.join("inventory")).unwrap();
    std::fs::write(
        root.join("inventory/repos.json"),
        json!({"group": "acme", "count": 1, "repos": [{
            "name": "widget",
            "path_with_namespace": "acme/widget",
            "web_url": "https://unmanaged.example/acme/widget",
            "forge": "github",
        }]})
        .to_string(),
    )
    .unwrap();
    (dir, root)
}

fn clone_widget(root: &Path, ws: &str, hosts: Option<&[String]>) -> (u8, Vec<Say>) {
    let mut said = Vec::new();
    let code = clone(
        &Request {
            root,
            ws,
            repos: &["widget".to_owned()],
            now: chrono::Utc::now(),
            author: "tester",
            hosts,
            watch: None,
        },
        &mut |line| said.push(line),
    );
    (code, said)
}

#[test]
fn a_clone_into_a_workspace_that_is_not_there_makes_it_first() {
    // #1382: `wscmd::ensure` scaffolds a workspace, so the clone makes one as Python's
    // did, instead of refusing and sending the person to `workspace create`.
    let (_dir, root) = plane_with_an_unreachable_widget();
    let (_, said) = clone_widget(&root, "beta", None);
    let ws = root.join("workspaces/beta");
    assert!(ws.is_dir(), "{said:?}");
    assert!(
        ws.join("memory/MEMORY.md").is_file(),
        "the workspace was made but not scaffolded: {said:?}"
    );
    assert!(
        said.iter()
            .any(|line| matches!(line, Say::Info(text) if text.contains("workspaces/beta/"))),
        "the person is not told a workspace was made: {said:?}"
    );
    assert!(
        !said
            .iter()
            .any(|line| matches!(line, Say::Fail(text) if text.contains("no workspace"))),
        "{said:?}"
    );
}

#[test]
fn a_clone_for_a_sandboxed_chat_does_not_make_a_workspace() {
    // A sandboxed chat's clone is run by the app for it; making a workspace is a step the
    // person or the chat takes on its own (`workspace create`), so it is refused here and
    // nothing is written.
    let (_dir, root) = plane_with_an_unreachable_widget();
    let (code, said) = clone_widget(&root, "beta", Some(&["github.com".to_owned()]));
    assert_eq!(code, 1, "{said:?}");
    assert!(!root.join("workspaces/beta").exists(), "{said:?}");
    assert!(
        said.iter().any(|line| matches!(line, Say::Fail(text)
            if text.contains("purlis workspace create beta") && !text.contains("yet"))),
        "{said:?}"
    );
}

#[test]
fn a_workspace_that_is_a_link_is_not_made_or_scaffolded_through() {
    let (_dir, root) = plane_with_an_unreachable_widget();
    let outside = tempfile::tempdir().unwrap();
    std::os::unix::fs::symlink(outside.path(), root.join("workspaces/evil")).unwrap();
    let (code, said) = clone_widget(&root, "evil", None);
    assert_eq!(code, 1, "{said:?}");
    assert_eq!(
        std::fs::read_dir(outside.path()).unwrap().count(),
        0,
        "something was written through the link: {said:?}"
    );
}

#[test]
fn a_name_that_is_a_path_or_an_option_or_hidden_gets_no_destination() {
    let (_dir, root) = plane();
    let ws = root.join("workspaces/alpha");
    for hostile in [
        "..",
        ".",
        "../escape",
        "a/b",
        "a\\b",
        "/etc",
        "-rf",
        "--upload-pack=x",
        ".github",
        "",
        "a\0b",
    ] {
        let refused = destination(&root, "alpha", &ws, hostile);
        assert!(refused.is_err(), "{hostile:?} was given {refused:?}");
    }
    assert_eq!(
        destination(&root, "alpha", &ws, "widget").unwrap(),
        ws.join("widget")
    );
}

/// SK-9 (#2772): a workspace keeps its skills in `skills/` and `skills.local/`, which a chat is
/// denied writing, so no clone takes either name, in any case.
#[test]
fn a_clone_is_never_named_as_a_workspaces_skills_folder() {
    let (_dir, root) = plane();
    let ws = root.join("workspaces/alpha");
    for taken in ["skills", "skills.local", "Skills"] {
        let refused = destination(&root, "alpha", &ws, taken).unwrap_err();
        assert!(refused.contains("keeps its skills"), "{taken}: {refused}");
    }
    assert!(destination(&root, "alpha", &ws, "skills-repo").is_ok());
}

#[test]
fn a_repo_name_the_next_machine_reads_as_something_else_gets_no_destination() {
    // charter-app#96. `repo_name_ok`'s alphabet admits `nul` and `alpha.`, and this name
    // is written into `workspace.json`, which is committed — so a forge that honestly
    // holds a repo called `nul` would put a directory in this plane that is the null
    // device on the next machine to check it out. `alpha:evil` reaches here at all only
    // because `destination` asks `segment_ok`, which looks at the second character for a
    // drive letter and nowhere else for a colon.
    let (_dir, root) = plane();
    let ws = root.join("workspaces/alpha");
    for hostile in ["nul", "NUL", "con", "aux", "lpt9", "com1.txt", "alpha."] {
        let refused = destination(&root, "alpha", &ws, hostile)
            .expect_err("{hostile} must get no destination");
        assert!(
            refused.contains("purlis will not clone"),
            "{hostile}: {refused}"
        );
    }
    // And the ones the alphabet was already catching keep being caught, by it.
    for hostile in ["alpha:evil", "PROGRA~1"] {
        assert!(
            destination(&root, "alpha", &ws, hostile).is_err(),
            "{hostile}"
        );
    }
    assert_eq!(
        destination(&root, "alpha", &ws, "nul-notes").unwrap(),
        ws.join("nul-notes"),
        "a name that merely starts like a device is an ordinary name"
    );
}

#[test]
fn a_link_at_the_destination_or_at_the_workspace_is_not_cloned_through() {
    let (_dir, root) = plane();
    let outside = tempfile::tempdir().unwrap();
    let ws = root.join("workspaces/alpha");
    // A link at the destination that lands INSIDE the plane — another workspace's clone —
    // is the case only `within_workspace` sees: the data-directory gate is satisfied by
    // where it lands, and a clone would be written into the other workspace.
    std::fs::create_dir_all(root.join("workspaces/beta/stolen")).unwrap();
    std::os::unix::fs::symlink(root.join("workspaces/beta/stolen"), ws.join("widget")).unwrap();
    assert!(destination(&root, "alpha", &ws, "widget").is_err());
    // And one that leaves the plane.
    std::os::unix::fs::symlink(outside.path(), ws.join("gadget")).unwrap();
    assert!(destination(&root, "alpha", &ws, "gadget").is_err());

    std::fs::create_dir_all(root.join("workspaces")).unwrap();
    std::os::unix::fs::symlink(outside.path(), root.join("workspaces/evil")).unwrap();
    let evil = root.join("workspaces/evil");
    assert!(destination(&root, "evil", &evil, "widget").is_err());
}

#[test]
fn the_url_is_https_from_the_record_or_the_forges_own_ssh_rewrite() {
    let (_dir, root) = plane();
    assert_eq!(
        https_url(
            &json!({"web_url": "https://github.com/acme/widget/"}),
            &root
        )
        .unwrap(),
        "https://github.com/acme/widget.git"
    );
    assert_eq!(
        https_url(
            &json!({"ssh_url": "git@github.com:acme/widget.git", "forge": "github"}),
            &root
        )
        .unwrap(),
        "https://github.com/acme/widget.git"
    );
}

#[test]
fn a_url_charter_did_not_build_is_refused() {
    let (_dir, root) = plane();
    for record in [
        json!({"ssh_url": "ext::sh -c 'touch /tmp/pwned'"}),
        json!({"ssh_url": "--upload-pack=touch /tmp/pwned"}),
        json!({"ssh_url": "/etc/passwd"}),
        json!({"web_url": "http://github.com/acme/widget"}),
        json!({"web_url": "https://evil.example/acme/widget"}),
        json!({"web_url": "https://github.com/acme/wid get"}),
    ] {
        assert!(https_url(&record, &root).is_err(), "{record}");
    }
}

#[test]
fn a_credential_in_the_url_is_refused_and_never_repeated() {
    let (_dir, root) = plane();
    // Built from parts so the repository's secret scan does not read the fixture as a token.
    let token = ["ghp_", "SECRET123"].concat();
    let refused = https_url(
        &json!({"web_url": format!("https://x-access-token:{token}@github.com/acme/widget")}),
        &root,
    )
    .unwrap_err();
    assert!(!refused.contains(&token), "{refused}");
    assert!(!refused.contains("x-access-token"), "{refused}");
}

/// A plane that declares a self-hosted GitLab on a port of its own, and a record of a repo there.
fn plane_on_a_port() -> (tempfile::TempDir, PathBuf, Value) {
    let (dir, root) = plane();
    std::fs::write(
        root.join("charter.toml"),
        "[[forge]]\nkind = \"gitlab\"\nhost = \"gitlab.corp.example:8443\"\nowner = \"acme\"\n",
    )
    .unwrap();
    let record = json!({
        "name": "widget",
        "web_url": "https://gitlab.corp.example:8443/acme/widget",
        "forge": "gitlab",
    });
    (dir, root, record)
}

/// A config home with one GitLab account signed in on `host`, through the recorded forge.
fn account_on(host: &str) -> tempfile::TempDir {
    use crate::forge::accounts::{self, Pasted};
    use crate::forge::route::HostScope;
    use crate::forge::transport::Transport as Wire;
    use std::sync::Arc;

    let home = tempfile::tempdir().unwrap();
    let keyring = accounts::keyring_at(home.path());
    let recording = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/forge_contract/accounts/gitlab/token_check.json");
    let recorded = Arc::new(
        crate::forge::recorded::Recorded::parse(&std::fs::read_to_string(recording).unwrap())
            .unwrap(),
    );
    let reach = |_: &Forge,
                 _: Arc<dyn crate::forge::http::TokenSource>|
     -> Result<Arc<dyn Wire>, crate::forge::ForgeError> {
        Ok(recorded.clone() as Arc<dyn Wire>)
    };
    accounts::sign_in_with_token(
        &HostScope::for_a_test(),
        home.path(),
        keyring.as_ref(),
        &reach,
        crate::forge::Kind::GitLab,
        host,
        Pasted::new("kept-for-the-window-git-only".to_string()),
    )
    .unwrap();
    home
}

#[test]
fn a_clone_of_a_self_hosted_repo_is_of_the_server_its_url_names_port_and_all() {
    let (_dir, root, record) = plane_on_a_port();
    let url = https_url(&record, &root).expect("the plane declares this server");
    assert_eq!(url, "https://gitlab.corp.example:8443/acme/widget.git");
    assert_eq!(
        Forge::for_clone(&record, &url),
        Forge {
            kind: crate::forge::Kind::GitLab,
            host: "gitlab.corp.example:8443".to_owned(),
        }
    );
    // A URL on a forge's default port is the host alone, as an account on it is.
    assert_eq!(
        Forge::for_clone(
            &json!({"forge": "github"}),
            "https://github.com/acme/widget.git"
        ),
        Forge::default_of(crate::forge::Kind::GitHub)
    );
}

#[test]
fn the_account_on_that_server_signs_the_clone_and_one_on_another_port_never_does() {
    use crate::forge::gitcredential::{HELPER_ARG, as_the_window};
    use crate::forge::route::HostScope;

    let (_dir, root, record) = plane_on_a_port();
    let url = https_url(&record, &root).unwrap();
    let forge = Forge::for_clone(&record, &url);
    let exe = Path::new("/Applications/purlis.app/Contents/MacOS/charter-app");

    let on_it = account_on("gitlab.corp.example:8443");
    let helper = as_the_window(&HostScope::for_a_test(), exe, on_it.path(), || {
        forge::helper_for(&forge)
    });
    assert!(helper.contains(HELPER_ARG), "{helper}");

    let on_another_port = account_on("gitlab.corp.example");
    let helper = as_the_window(
        &HostScope::for_a_test(),
        exe,
        on_another_port.path(),
        || forge::helper_for(&forge),
    );
    assert!(!helper.contains(HELPER_ARG), "{helper}");
}

mod landing;
