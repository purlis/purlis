// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use super::*;

// ---- the policy watch (#1431) ------------------------------------------------------------

#[test]
fn the_policy_watch_tells_a_change_once_and_a_rewrite_to_the_same_locks_never() {
    use sandbox::policy::Locks;
    let file = std::path::Path::new("/etc/purlis/policy.json");
    let required = || Locks::parse(r#"{"owner": "IT", "sandbox": {"opt-out": false}}"#, file);
    let mut watch = PolicyWatch::from(Locks::none());
    assert!(!watch.moved(Locks::none()), "no policy, then none");
    assert!(watch.moved(required()), "a policy landed");
    assert!(!watch.moved(required()), "the same locks, read again");
    assert!(
        watch.moved(Locks::parse(
            r#"{"owner": "Ops", "sandbox": {"opt-out": false}}"#,
            file
        )),
        "who set it is part of what the Notice says"
    );
    assert!(watch.moved(Locks::none()), "the policy went");
}

// ---- the live ask (#1666) ----------------------------------------------------------------

#[test]
fn an_allow_says_whether_the_command_carries_on_or_the_chat_restarts() {
    use sandbox::grant::Level;
    let live = Kept {
        live: vec![4, 6],
        retry: vec![6],
        already: false,
    };
    assert_eq!(
        allowed_said(Some(4), Level::You, &live),
        "Allowed for me on this machine. The command that asked carries on now; nothing \
             restarts."
    );
    // Taken live by other chats only: this one still restarts.
    assert!(allowed_said(Some(5), Level::You, &live).contains("restarts on the same conversation"));
    // Allowed already, and this chat is not on the proxy: said so, and it restarts to take
    // it, where pressing Allow used to be refused and the Notice stayed up.
    // A held connection that had given up: the chat is told to run it again.
    assert!(
        allowed_said(Some(6), Level::You, &live).contains("told to run it again"),
        "{}",
        allowed_said(Some(6), Level::You, &live)
    );
    // Allowed already, and taken live: said so, and the command carries on.
    let both = Kept {
        live: vec![4],
        retry: Vec::new(),
        already: true,
    };
    assert!(allowed_said(Some(4), Level::You, &both).starts_with("It was allowed already"));
    let already = Kept {
        live: Vec::new(),
        retry: Vec::new(),
        already: true,
    };
    let said = allowed_said(Some(4), Level::Project, &already);
    assert!(
        said.starts_with("It was allowed already, for everyone in this project."),
        "{said}"
    );
    assert!(said.contains("restarts on the same conversation"), "{said}");
}

#[test]
fn keep_blocked_answers_the_block_it_showed_and_tells_the_chat_in_fixed_words() {
    let chats = crate::chats::Chats::new();
    let told: std::sync::Arc<std::sync::Mutex<Vec<(u32, purlis_core::askoutcome::Outcome)>>> =
        std::sync::Arc::default();
    let keep = std::sync::Arc::clone(&told);
    chats.tell_network_words_to(std::sync::Arc::new(move |session, word| {
        keep.lock().unwrap().push((session, word));
    }));
    let shown = crate::taskblocks::BlockShown {
        operation: "connect".to_owned(),
        kind: "host".to_owned(),
        what: GrantWhat::Host,
        target: "paste.example:8443".to_owned(),
    };
    // Not held on it: answered already, so nothing is done and the chat is told nothing.
    assert!(keep_block(&chats, 4, &shown, None, &|_, _| Ok(())).is_none());
    assert!(told.lock().unwrap().is_empty());
    chats.blocks().heard(
        4,
        crate::taskblocks::HeldBlock {
            operation: "connect".to_owned(),
            kind: "host".to_owned(),
            what: GrantWhat::Host,
            target: "paste.example:8443".to_owned(),
        },
    );
    keep_block(&chats, 4, &shown, None, &|_, _| Ok(())).expect("kept blocked");
    assert_eq!(
        *told.lock().unwrap(),
        vec![(
            4,
            purlis_core::askoutcome::Outcome {
                about: purlis_core::askoutcome::About::Host {
                    host: "paste.example:8443".to_owned(),
                },
                answer: purlis_core::askoutcome::Answer::KeptBlocked { reason: None },
            },
        )]
    );
    // Answered: a second press finds nothing held, and tells the chat nothing more.
    assert!(keep_block(&chats, 4, &shown, None, &|_, _| Ok(())).is_none());
    assert_eq!(told.lock().unwrap().len(), 1);
}

// ---- an administrator's policy (#1343), through the test build's seam ----

#[test]
fn a_persona_s_hosts_locked_by_policy_say_who_locked_them_and_no_policy_reads_none() {
    use sandbox::policy::{Locks, set_for_this_test};
    let root = std::path::Path::new("/home/dev/plane");
    // A test build never reads this machine's file: no policy until a test sets one.
    assert_eq!(persona_hosts_locked(root), None);
    assert_eq!(state_on_policy(root), None);
    set_for_this_test(Locks::parse(
        r#"{"owner": "IT", "sandbox": {"persona-hosts": false}}"#,
        std::path::Path::new("/etc/purlis/policy.json"),
    ));
    let said = persona_hosts_locked(root).expect("locked");
    set_for_this_test(Locks::none());
    assert_eq!(
        said,
        "Policy forbids a persona's own hosts. Locked by policy, set by IT in \
             /etc/purlis/policy.json."
    );
}

/// This thread's policy until it is dropped, a panic included: no later test on the thread
/// is left under it.
struct Under;

impl Under {
    fn policy(json: &str) -> Self {
        sandbox::policy::set_for_this_test(sandbox::policy::Locks::parse(
            json,
            std::path::Path::new("/etc/purlis/policy.json"),
        ));
        Self
    }
}

impl Drop for Under {
    fn drop(&mut self) {
        sandbox::policy::set_for_this_test(sandbox::policy::Locks::none());
    }
}

/// D-1423-1: a policy that forbids the opt-out requires the sandbox, so a project that has
/// not turned it on reads as on, is offered nothing, and says who requires it.
#[test]
fn a_project_with_no_sandbox_reads_as_on_and_required_where_policy_forbids_the_opt_out() {
    let project = tempfile::tempdir().expect("a project");
    std::fs::write(project.path().join("charter.toml"), "schema = 1\n").expect("toml");
    let free = state_of(project.path());
    assert!(!free.on && free.offer, "{free:?}");
    {
        let _under = Under::policy(r#"{"owner": "IT", "sandbox": {"opt-out": false}}"#);
        let state = state_of(project.path());
        assert!(state.on, "{state:?}");
        assert!(!state.offer, "what is on already is not offered");
        assert_eq!(
            state.policy.expect("a policy").required.as_deref(),
            Some("On, required by policy, set by IT in /etc/purlis/policy.json.")
        );
    }
    // A policy that locks something else requires nothing.
    let _under = Under::policy(r#"{"owner": "IT", "sandbox": {"write-grants": false}}"#);
    let state = state_of(project.path());
    assert!(!state.on, "{state:?}");
    assert_eq!(state.policy.expect("a policy").required, None);
}

/// #1423: what is kept on this machine and policy now drops is marked, never listed as
/// granted: a folder under a write-grants lock.
#[test]
fn a_kept_grant_policy_now_drops_is_marked_locked_on_the_granted_list() {
    let project = tempfile::tempdir().expect("a project");
    let folder = project.path().join("out");
    sandbox::local::grant_write(project.path(), &folder).expect("kept");
    let chats = crate::chats::Chats::new();
    let free = grants_of(
        project.path(),
        &chats,
        &crate::chatbrowsers::ChatBrowsers::default(),
    );
    assert_eq!(free.len(), 1, "{free:?}");
    assert_eq!(free[0].locked, None);
    let _under = Under::policy(r#"{"owner": "IT", "sandbox": {"write-grants": false}}"#);
    let held = grants_of(
        project.path(),
        &chats,
        &crate::chatbrowsers::ChatBrowsers::default(),
    );
    assert_eq!(held[0].what, GrantWhat::Write);
    assert_eq!(
        held[0].locked.as_deref(),
        Some(
            "Policy forbids allowing a chat to write a folder. Locked by policy, set by IT \
                 in /etc/purlis/policy.json."
        )
    );
}

/// What Settings is told of the policy for the project at `root`.
fn state_on_policy(root: &std::path::Path) -> Option<SandboxPolicy> {
    SandboxPolicy::of(&sandbox::policy::Locks::of(root))
}

// ---- a block's Allow, and the Granted list (#1342, #1348) ----

fn no_audit() -> impl Fn(Option<u32>, &sandbox::grant::Audited<'_>) -> Result<(), String> {
    |_, _| Err("the event log is not open".to_owned())
}

fn machine() -> sandbox::Machine {
    sandbox::Machine {
        env: purlis_core::secrets::Env::of(&[]),
        home: Some(std::path::PathBuf::from("/nohome/dev")),
        os: sandbox::Os::MacOs,
    }
}

/// #1681: a chat taking its own persona's grants is audited, as the chat's persona's hosts
/// for that chat, before anything is taken; an audit that fails takes nothing, and a chat
/// holding no other's is neither audited nor taken.
#[test]
fn a_chat_taking_its_own_persona_s_grants_is_audited_first() {
    let seen = std::sync::Mutex::new(Vec::new());
    let audit = |chat: Option<u32>, audited: &sandbox::grant::Audited<'_>| {
        seen.lock().unwrap().push((
            chat,
            audited.granted,
            audited.what.to_owned(),
            audited.target.to_owned(),
            audited.level.word(),
        ));
        Ok(())
    };
    let taken = std::cell::Cell::new(0);
    let take = || {
        taken.set(taken.get() + 1);
        true
    };
    assert_eq!(
        own_grants(8, Some(Some("devops".to_owned())), &audit, take),
        Ok(true)
    );
    assert_eq!(
        *seen.lock().unwrap(),
        [(
            Some(8),
            true,
            "persona-hosts".to_owned(),
            "devops".to_owned(),
            "chat"
        )]
    );
    assert_eq!(own_grants(9, None, &audit, take), Ok(false));
    assert!(own_grants(8, Some(None), &no_audit(), take).is_err());
    assert_eq!(taken.get(), 1, "taken only once audited");
    assert_eq!(seen.lock().unwrap().len(), 1);
}

#[test]
fn nothing_is_allowed_that_was_not_audited_or_that_a_grant_cannot_name() {
    let project = tempfile::tempdir().expect("a project");
    let chats = crate::chats::Chats::new();
    let refused = |what, target: &str, level| {
        allow(
            project.path(),
            &machine(),
            &chats,
            3,
            (what, target, level),
            &no_audit(),
            100,
        )
        .expect_err("refused")
        .to_string()
    };
    assert!(
        refused(GrantWhat::Write, "/tmp/x", GrantLevel::Project)
            .contains("everyone in the project")
    );
    assert!(refused(GrantWhat::Host, "169.254.169.254", GrantLevel::You).contains("link-local"));
    // A write is judged against the chat's own folder, so a chat that is not open gets none.
    assert!(refused(GrantWhat::Write, "/tmp/x", GrantLevel::Chat).contains("not open"));
    // An unaudited grant is no grant: nothing is kept at any level.
    assert!(refused(GrantWhat::Host, "api.example.com", GrantLevel::You).contains("event log"));
    assert!(sandbox::hosts::personal(project.path()).is_empty());
    assert!(
        grants_of(
            project.path(),
            &chats,
            &crate::chatbrowsers::ChatBrowsers::default()
        )
        .is_empty()
    );
}

/// ADR 0093 (D-sock-2, D-sock-3): a socket or a service is this machine's, so never allowed
/// for everyone in the project; once is a socket's or a service's alone; and each is judged
/// again, never taken on the window's word.
#[test]
fn a_socket_or_a_service_is_never_the_projects_and_once_is_theirs_alone() {
    let project = tempfile::tempdir().expect("a project");
    let chats = crate::chats::Chats::new();
    let refused = |what, target: &str, level| {
        allow(
            project.path(),
            &machine(),
            &chats,
            3,
            (what, target, level),
            &no_audit(),
            100,
        )
        .expect_err("refused")
        .to_string()
    };
    for what in [GrantWhat::Socket, GrantWhat::Service] {
        assert!(
            refused(what, "/var/run/docker.sock", GrantLevel::Project)
                .contains("committed file that granted one"),
            "{what:?}"
        );
    }
    assert!(refused(GrantWhat::Host, "api.example.com", GrantLevel::Once).contains("not once"));
    assert!(refused(GrantWhat::Write, "/tmp/x", GrantLevel::Once).contains("not once"));
    // Not a socket on this machine now: nothing to allow.
    assert!(
        refused(GrantWhat::Socket, "/nowhere/docker.sock", GrantLevel::You)
            .contains("not a local socket")
    );
    assert!(
        refused(
            GrantWhat::Service,
            "com.apple.SecurityServer",
            GrantLevel::You
        )
        .contains("vault")
    );
    // An unaudited grant is no grant.
    assert!(
        refused(GrantWhat::Service, "com.example.helper", GrantLevel::You).contains("event log")
    );
    assert!(sandbox::local::granted_services(project.path()).is_empty());
}

/// R1 (2026-10-13): Settings says what a chat's next start has. On by default with
/// nothing allowed by anyone; off where the project's file says `false`; and nothing is
/// said where the sandbox is off.
#[test]
fn settings_says_certificate_checks_are_on_unless_the_project_turns_them_off() {
    let said = |text: &str| {
        let project = tempfile::tempdir().expect("a project");
        std::fs::write(project.path().join("charter.toml"), text).expect("toml");
        state_on(project.path(), sandbox::Os::MacOs).certificate_check
    };
    let on = Some(CertificateCheck {
        on: true,
        locked: None,
        widens: true,
    });
    assert_eq!(said("[sandbox]\nmode = \"on\"\n"), on);
    assert_eq!(
        said("[sandbox]\nmode = \"on\"\ncertificate-checks = true\n"),
        on
    );
    assert_eq!(
        said("[sandbox]\nmode = \"on\"\ncertificate-checks = false\n"),
        Some(CertificateCheck {
            on: false,
            locked: None,
            widens: true,
        })
    );
    assert_eq!(said(""), None);
    // Off a Mac the setting widens nothing, and Settings can say so.
    let project = tempfile::tempdir().expect("a project");
    std::fs::write(
        project.path().join("charter.toml"),
        "[sandbox]\nmode = \"on\"\n",
    )
    .expect("toml");
    assert_eq!(
        state_on(project.path(), sandbox::Os::Linux).certificate_check,
        Some(CertificateCheck {
            on: true,
            locked: None,
            widens: false,
        })
    );
}

/// R1: an administrator's `socket-grants` lock turns certificate checks off whatever the
/// project says, and Settings says who set it.
#[test]
fn settings_says_policy_turned_certificate_checks_off_and_who() {
    let locks = sandbox::policy::Locks::parse(
        r#"{"owner": "IT", "sandbox": {"socket-grants": false}}"#,
        std::path::Path::new("/etc/purlis/policy.json"),
    );
    let said = CertificateCheck::of(
        sandbox::CertificateChecks::of(&sandbox::Policy::defaults(), &locks),
        sandbox::Os::MacOs,
    );
    assert!(!said.on);
    let why = said.locked.expect("said");
    assert!(why.contains("Policy turns certificate checks off"), "{why}");
    assert!(why.contains("IT"), "{why}");
}

/// R1: the certificate check is no grant, so nothing allows it by name: not a block's
/// Allow at any level, and one an older build kept is not listed as allowed.
#[test]
fn the_certificate_check_is_never_granted_by_name_and_an_old_grant_is_not_listed() {
    use sandbox::services::TRUSTD;
    let project = tempfile::tempdir().expect("a project");
    let root = project.path();
    std::fs::write(root.join("charter.toml"), "[sandbox]\nmode = \"on\"\n").expect("toml");
    for level in [GrantLevel::Once, GrantLevel::Chat, GrantLevel::You] {
        let refused = judged(
            root,
            &sandbox::Machine::this(),
            Some(root),
            1,
            (GrantWhat::Service, TRUSTD, level),
        )
        .expect_err("refused")
        .to_string();
        assert!(refused.contains("Settings"), "{refused}");
    }
    sandbox::local::grant_service(root, TRUSTD).expect("an older build's");
    let chats = crate::chats::Chats::new();
    assert!(
        !grants_of(root, &chats, &crate::chatbrowsers::ChatBrowsers::default())
            .iter()
            .any(|one| one.target == TRUSTD),
        "it grants nothing, so it is not listed"
    );
}

/// A path a repository declared may be a link to another socket. The window is told the
/// socket an Allow would keep, as the kernel names it, beside the path as written.
#[test]
fn a_declared_socket_is_named_as_the_grant_would_name_it() {
    use sandbox::grant::What;
    use sandbox::services::{Awaited, Awaiting};
    let said: ProjectExpects = Awaiting {
        items: vec![
            Awaited {
                what: What::Socket("/Users/op/.docker/run/docker.sock".into()),
                written: "/opt/tools/harmless.sock".to_owned(),
                said: "said".to_owned(),
                refused: None,
            },
            Awaited {
                what: What::Service("com.example.helper".to_owned()),
                written: "com.example.helper".to_owned(),
                said: "said".to_owned(),
                refused: None,
            },
        ],
        locked: None,
        asked: true,
    }
    .into();
    assert_eq!(
        said.items
            .iter()
            .map(|item| (item.named.as_str(), item.target.as_str()))
            .collect::<Vec<_>>(),
        [
            (
                "/Users/op/.docker/run/docker.sock",
                "/opt/tools/harmless.sock"
            ),
            ("com.example.helper", "com.example.helper"),
        ]
    );
}

/// **An Allow keeps what the person was shown, as they were shown it.** The question is
/// drawn once and may be pressed much later. A declared path that leads to another socket
/// by then, and an item drawn as one that cannot be allowed which now can, are not what
/// the person answered: neither is kept, audited or recorded as answered, so each is
/// asked about again as it stands. What still stands as drawn beside them is kept.
#[test]
fn an_allow_keeps_only_what_stands_as_the_question_drew_it() {
    use sandbox::grant::What;
    use sandbox::services::{Awaited, Awaiting};
    let project = tempfile::tempdir().expect("a project");
    let root = project.path();
    std::fs::write(root.join("charter.toml"), "[sandbox]\nmode = \"on\"\n").expect("toml");
    let chats = crate::chats::Chats::new();
    let seen = std::sync::Mutex::new(Vec::new());
    let audit = |_: Option<u32>, audited: &sandbox::grant::Audited<'_>| {
        seen.lock().unwrap().push(audited.target.to_owned());
        Ok(())
    };
    let service = |name: &str| Awaited {
        what: What::Service(name.to_owned()),
        written: name.to_owned(),
        said: "said".to_owned(),
        refused: None,
    };
    // What waits at the press: a link that leads to a container engine's socket now, a
    // socket that came to be there since, and a service that stands as it was drawn.
    let now = || {
        Some(Awaiting {
            items: vec![
                Awaited {
                    what: What::Socket("/Users/op/.docker/run/docker.sock".into()),
                    written: "/opt/project/run/dev.sock".to_owned(),
                    said: "said".to_owned(),
                    refused: None,
                },
                Awaited {
                    what: What::Socket("/opt/project/run/later.sock".into()),
                    written: "/opt/project/run/later.sock".to_owned(),
                    said: "said".to_owned(),
                    refused: None,
                },
                service("com.example.helper"),
            ],
            locked: None,
            asked: true,
        })
    };
    // What the window drew: the link leading to a harmless socket, the second as one that
    // cannot be allowed here now, and the service.
    let drew = [
        ExpectedShown {
            target: "/opt/project/run/dev.sock".to_owned(),
            named: Some("/Users/op/.local/share/harmless.sock".to_owned()),
        },
        ExpectedShown {
            target: "/opt/project/run/later.sock".to_owned(),
            named: None,
        },
        ExpectedShown {
            target: "com.example.helper".to_owned(),
            named: Some("com.example.helper".to_owned()),
        },
    ];
    allow_awaited(root, &chats, now(), &drew, &audit, 100).expect("the service is allowed");
    assert_eq!(*seen.lock().unwrap(), ["com.example.helper"]);
    assert!(sandbox::local::granted_sockets(root).is_empty());
    assert_eq!(
        sandbox::local::expects_seen(root),
        ["com.example.helper"],
        "what changed under the question is not answered"
    );
    // With nothing left that stands as drawn, the Allow is refused and says why.
    let refused = allow_awaited(root, &chats, now(), &drew[..2], &audit, 101)
        .expect_err("nothing stands as drawn");
    assert!(refused.contains("changed"), "{refused}");
    assert_eq!(seen.lock().unwrap().len(), 1);
    assert!(sandbox::local::granted_sockets(root).is_empty());
}

/// **A folder of yours that no chat may write, for a real socket** (#2626): under your home,
/// where `SocketGround` refuses neither, and short, as a socket's whole path must fit the
/// kernel's 104 bytes (`sun_path` on macOS). The temp folder is a chat's to write, so a socket
/// there is refused by design, and a checkout's path is often too long. On CI
/// (`GITHUB_ACTIONS=true`) a home that cannot hold it fails the test; anywhere else, a sandboxed
/// agent's home among them, the test says why it did not run and passes.
#[cfg(unix)]
fn a_folder_of_yours_no_chat_writes() -> Option<tempfile::TempDir> {
    let on_ci = std::env::var("GITHUB_ACTIONS").as_deref() == Ok("true");
    let made = std::env::var_os("HOME")
        .filter(|home| !home.is_empty())
        .ok_or_else(|| std::io::Error::other("HOME is not set"))
        .and_then(|home| {
            tempfile::Builder::new()
                .prefix(".socket-check-")
                .tempdir_in(home)
        });
    match made {
        Ok(folder) if folder.path().join("b.sock").as_os_str().len() < 100 => Some(folder),
        Ok(folder) => {
            assert!(
                !on_ci,
                "{} is too long for a socket",
                folder.path().display()
            );
            println!(
                "not run: {} is too long for a socket",
                folder.path().display()
            );
            None
        }
        Err(err) => {
            assert!(!on_ci, "CI runs this in the runner's home: {err}");
            println!(
                "not run: no folder of yours can be made in your home here ({err}); CI runs it"
            );
            None
        }
    }
}

/// **#2626, against real sockets** (ADR 0093, #1923, #1936): a project declares a path that is
/// a link to socket A. The up-front Notice names A, the socket an Allow would keep, beside the
/// path as written. The link moves to socket B: an Allow pressed for what was drawn keeps
/// nothing, audits nothing and says it changed; the question is asked again, after a "Not
/// now", naming B; and an Allow for B as drawn then keeps B.
#[cfg(unix)]
#[test]
fn a_declared_link_to_a_real_socket_is_named_and_kept_only_as_the_notice_drew_it() {
    use std::os::unix::net::UnixListener;
    let Some(mine) = a_folder_of_yours_no_chat_writes() else {
        return;
    };
    let (a, b) = (mine.path().join("a.sock"), mine.path().join("b.sock"));
    let _a = UnixListener::bind(&a).expect("socket A");
    let _b = UnixListener::bind(&b).expect("socket B");
    let a = a.canonicalize().expect("A as the kernel names it");
    let b = b.canonicalize().expect("B as the kernel names it");
    let project = tempfile::tempdir().expect("a project");
    let root = project.path();
    // The link is in the project, a folder a chat may write: what it leads to can change.
    let link = root.join("dev.sock");
    std::os::unix::fs::symlink(&a, &link).expect("the link to A");
    let written = link.display().to_string();
    std::fs::write(
        root.join("charter.toml"),
        format!("[sandbox]\nmode = \"on\"\nexpects = [\"{written}\"]\n"),
    )
    .expect("toml");
    let chats = crate::chats::Chats::new();
    let seen = std::sync::Mutex::new(Vec::new());
    let audit = |_: Option<u32>, audited: &sandbox::grant::Audited<'_>| {
        seen.lock().unwrap().push(audited.target.to_owned());
        Ok(())
    };
    let named = |expects: &ProjectExpects| -> Vec<(String, String, Option<String>)> {
        expects
            .items
            .iter()
            .map(|item| {
                (
                    item.target.clone(),
                    item.named.clone(),
                    item.refused.clone(),
                )
            })
            .collect()
    };
    let path = |socket: &std::path::Path| socket.display().to_string();

    // The Notice, up front: socket A, the one an Allow would keep, beside the path as written.
    let drawn = state_of(root).expects.expect("asked up front");
    assert!(drawn.asked);
    assert_eq!(named(&drawn), [(written.clone(), path(&a), None)]);
    let as_drawn = |expects: &ProjectExpects| -> Vec<ExpectedShown> {
        expects
            .items
            .iter()
            .map(|item| ExpectedShown {
                target: item.target.clone(),
                named: Some(item.named.clone()),
            })
            .collect()
    };
    let shown = as_drawn(&drawn);
    let declined = decline_expects(root, &shown)
        .expect("Not now")
        .expects
        .expect("still awaited");
    assert!(!declined.asked, "answered for A");

    // The link moves to socket B.
    std::fs::remove_file(&link).expect("the link goes");
    std::os::unix::fs::symlink(&b, &link).expect("the link to B");

    // An Allow pressed for what was drawn (A) keeps nothing, audits nothing, and says so.
    let refused = allow_expects(root, &chats, &shown, &audit, 100)
        .expect_err("the path no longer names what was drawn");
    assert_eq!(refused, EXPECTS_CHANGED);
    assert!(seen.lock().unwrap().is_empty());
    assert!(sandbox::local::granted_sockets(root).is_empty());

    // The question is asked again, naming B.
    let again = state_of(root).expects.expect("asked again");
    assert!(again.asked, "B was never answered");
    assert_eq!(named(&again), [(written.clone(), path(&b), None)]);

    // An Allow for B as drawn keeps B, and nothing waits after it.
    allow_expects(root, &chats, &as_drawn(&again), &audit, 101).expect("B is kept");
    assert_eq!(*seen.lock().unwrap(), [path(&b)]);
    assert_eq!(sandbox::local::granted_sockets(root), [b]);
    assert_eq!(state_of(root).expects, None);
}

/// ADR 0093, ruling 2: what a project declares it expects is asked about up front and
/// grants nothing by itself. Each state: declared and not allowed (asked, with each item's
/// own words); "Not now" (remembered here, still offered in Settings); a new declared item
/// (asked again); Allow (audited, kept in this machine's file for this project, and what
/// cannot be allowed here is left out and said); and policy's lock (nothing is allowed,
/// and who set it is said).
#[test]
fn what_a_project_expects_is_asked_once_allowed_by_the_person_and_never_by_the_file() {
    let project = tempfile::tempdir().expect("a project");
    let root = project.path();
    let expecting = |expects: &str| {
        std::fs::write(
            root.join("charter.toml"),
            format!("[sandbox]\nmode = \"on\"\nexpects = [{expects}]\n"),
        )
        .expect("toml");
    };
    let chats = crate::chats::Chats::new();
    let seen = std::sync::Mutex::new(Vec::new());
    let audit = |chat: Option<u32>, audited: &sandbox::grant::Audited<'_>| {
        seen.lock().unwrap().push((
            chat,
            audited.granted,
            audited.what.to_owned(),
            audited.target.to_owned(),
            audited.level.word(),
        ));
        Ok(())
    };
    std::fs::write(root.join("charter.toml"), "[sandbox]\nmode = \"on\"\n").expect("toml");
    assert_eq!(state_of(root).expects, None, "nothing declared");

    expecting("\"com.example.helper\", \"/purlis-test-nowhere/docker.sock\"");
    let said = state_of(root).expects.expect("asked");
    assert!(said.asked);
    assert_eq!(said.locked, None);
    assert_eq!(
        said.items
            .iter()
            .map(|item| (item.what, item.target.as_str(), item.refused.is_some()))
            .collect::<Vec<_>>(),
        [
            (GrantWhat::Socket, "/purlis-test-nowhere/docker.sock", true),
            (GrantWhat::Service, "com.example.helper", false),
        ]
    );
    assert!(said.items[0].says.contains("about the same as no sandbox"));
    // The declaration allowed nothing.
    assert!(sandbox::local::granted_services(root).is_empty());

    // Not now: remembered, still awaited, and nothing is allowed or audited.
    let as_drawn = |named: &dyn Fn(&ExpectedItem) -> String| -> Vec<ExpectedShown> {
        said.items
            .iter()
            .map(|item| ExpectedShown {
                target: item.target.clone(),
                named: Some(named(item)),
            })
            .collect()
    };
    // #1927: an answer is to what was shown. A "Not now" sent for a socket drawn under
    // another name than the item has now answers nothing: it is asked about again.
    let stale = decline_expects(root, &as_drawn(&|_| "/Users/dev/.other/x.sock".to_owned()))
        .expect("kept")
        .expects
        .expect("awaited");
    assert!(stale.asked, "what was shown is not what stands");
    // And something the window names that the file does not declare is not recorded.
    decline_expects(
        root,
        &[ExpectedShown {
            target: "com.example.undeclared".to_owned(),
            named: None,
        }],
    )
    .expect("kept");
    assert!(sandbox::local::expects_seen(root).is_empty());
    let declined = decline_expects(root, &as_drawn(&|item| item.named.clone()))
        .expect("kept")
        .expects
        .expect("awaited");
    assert!(!declined.asked);
    assert_eq!(declined.items, said.items);
    assert!(seen.lock().unwrap().is_empty());
    assert!(sandbox::local::granted_services(root).is_empty());

    // The declaration changes: asked again.
    expecting(
        "\"com.example.helper\", \"/purlis-test-nowhere/docker.sock\", \"com.example.other\"",
    );
    assert!(state_of(root).expects.expect("asked").asked);

    // As the window draws each: a service by its name, the socket that is not there as
    // one that cannot be allowed.
    let drawn = |targets: &[&str]| -> Vec<ExpectedShown> {
        targets
            .iter()
            .map(|target| ExpectedShown {
                target: (*target).to_owned(),
                named: (!target.starts_with('/')).then(|| (*target).to_owned()),
            })
            .collect()
    };
    // An unaudited Allow is no Allow.
    let all = drawn(&[
        "com.example.helper",
        "/purlis-test-nowhere/docker.sock",
        "com.example.other",
    ]);
    assert!(
        allow_expects(root, &chats, &all, &no_audit(), 100)
            .expect_err("refused")
            .contains("event log")
    );
    assert!(sandbox::local::granted_services(root).is_empty());
    // Allow, for what was shown: only that, only what the file declares, and never what
    // cannot be allowed here. Something the window names that the file does not is nothing.
    let shown = drawn(&[
        "com.example.helper",
        "/purlis-test-nowhere/docker.sock",
        "com.example.never-declared",
    ]);
    allow_expects(root, &chats, &shown, &audit, 100).expect("allowed");
    assert_eq!(
        sandbox::local::granted_services(root),
        ["com.example.helper"]
    );
    assert!(sandbox::local::granted_sockets(root).is_empty());
    assert_eq!(
        *seen.lock().unwrap(),
        [(
            None,
            true,
            "service".to_owned(),
            "com.example.helper".to_owned(),
            "you"
        )]
    );
    // What is left still awaits: the socket that is not there, answered, and the item
    // that was not shown, which is still to be asked about. An answer covers what the
    // person saw and nothing declared since.
    let left = state_of(root).expects.expect("awaited");
    assert_eq!(left.items.len(), 2);
    assert!(left.asked, "an item nobody was shown is not answered");
    assert_eq!(
        sandbox::local::expects_seen(root),
        ["/purlis-test-nowhere/docker.sock", "com.example.helper"]
    );
    // Nothing shown that can be allowed: said, and nothing kept.
    assert!(
        allow_expects(
            root,
            &chats,
            &drawn(&["/purlis-test-nowhere/docker.sock"]),
            &audit,
            101
        )
        .expect_err("refused")
        .contains("nothing was allowed")
    );

    // Policy's lock: nothing is allowed, and the state says who set it.
    sandbox::policy::set_for_this_test(sandbox::policy::Locks::parse(
        r#"{"owner": "IT", "sandbox": {"socket-grants": false}}"#,
        std::path::Path::new("/etc/purlis/policy.json"),
    ));
    let locked = state_of(root).expects.expect("awaited");
    let refused = allow_expects(root, &chats, &drawn(&["com.example.other"]), &audit, 102);
    sandbox::policy::set_for_this_test(sandbox::policy::Locks::none());
    let why = locked.locked.expect("locked");
    assert!(
        why.contains("Policy forbids") && why.contains("IT"),
        "{why}"
    );
    assert_eq!(refused.expect_err("refused"), why);
    assert_eq!(seen.lock().unwrap().len(), 1);
}

/// R2 (2026-10-13): a socket or a service allowed "on this machine, for every project" is
/// judged as any grant is, audited once, kept in this machine's one file (never the
/// project's, nor under its clone's key), listed in every project's Allowed list with
/// Remove, and taken out of them all by one Remove. A host or a folder is never kept there.
#[test]
fn a_socket_or_a_service_is_allowed_for_every_project_listed_in_each_and_removed_once() {
    use sandbox::grant::{Level, What};
    let everywhere = tempfile::tempdir().expect("a config home");
    sandbox::local::everywhere_for_this_test(Some(
        everywhere.path().join(sandbox::local::EVERY_PROJECT),
    ));
    let project = tempfile::tempdir().expect("a project");
    let other = tempfile::tempdir().expect("another project");
    for root in [project.path(), other.path()] {
        std::fs::write(root.join("charter.toml"), "[sandbox]\nmode = \"on\"\n").expect("toml");
    }
    let root = project.path();
    let chats = crate::chats::Chats::new();
    let seen = std::sync::Mutex::new(Vec::new());
    let audit = |chat: Option<u32>, audited: &sandbox::grant::Audited<'_>| {
        seen.lock().unwrap().push((
            chat,
            audited.granted,
            audited.what.to_owned(),
            audited.target.to_owned(),
            audited.level.word(),
        ));
        Ok(())
    };
    // Judged by the core at this level: a service by its whole name, never the
    // certificate check's, and never a host or a folder.
    let judge = |what, target: &str| {
        judged(
            root,
            &sandbox::Machine::this(),
            Some(root),
            1,
            (what, target, GrantLevel::Machine),
        )
    };
    let (service, level) = judge(GrantWhat::Service, "com.example.helper").expect("judged");
    assert_eq!(level, Level::Machine);
    assert!(
        judge(GrantWhat::Host, "api.example.com")
            .expect_err("refused")
            .to_string()
            .contains("only a local socket or a system service")
    );
    assert!(judge(GrantWhat::Service, sandbox::services::TRUSTD).is_err());
    // An unaudited Allow is no Allow.
    assert!(
        kept_for(root, &chats, None, (&service, level), &no_audit(), 100)
            .expect_err("refused")
            .contains("event log")
    );
    assert!(sandbox::local::services_everywhere().is_empty());

    let socket = What::Socket("/purlis-test-nowhere/docker.sock".into());
    for what in [&service, &socket] {
        let kept = kept_for(root, &chats, None, (what, level), &audit, 100).expect("kept");
        assert!(!kept.already);
    }
    // Allowed twice is kept once, and audited once.
    assert!(
        kept_for(root, &chats, None, (&socket, level), &audit, 101)
            .expect("allowed already")
            .already
    );
    assert_eq!(
        *seen.lock().unwrap(),
        [
            (
                None,
                true,
                "service".to_owned(),
                "com.example.helper".to_owned(),
                "machine"
            ),
            (
                None,
                true,
                "socket".to_owned(),
                "/purlis-test-nowhere/docker.sock".to_owned(),
                "machine"
            ),
        ]
    );
    // This machine's one file: nothing in the project, nothing under its clone's key.
    assert!(sandbox::local::granted_sockets(root).is_empty());
    assert!(sandbox::local::granted_services(root).is_empty());
    assert!(sandbox::local::everywhere_path().starts_with(everywhere.path()));
    // A host or a folder is refused there whoever asks.
    let host = What::Host(sandbox::grant::host("api.example.com").expect("a host"));
    assert!(kept_for(root, &chats, None, (&host, level), &audit, 102).is_err());

    // Listed in each project, at that level, with when it was allowed.
    let listed_in = |root: &std::path::Path| -> Vec<(GrantWhat, String, GrantLevel, Option<u32>)> {
        grants_of(root, &chats, &crate::chatbrowsers::ChatBrowsers::default())
            .into_iter()
            .filter(|one| one.level == GrantLevel::Machine)
            .map(|one| (one.what, one.target, one.level, one.at))
            .collect()
    };
    for root in [project.path(), other.path()] {
        assert_eq!(
            listed_in(root),
            [
                (
                    GrantWhat::Socket,
                    "/purlis-test-nowhere/docker.sock".to_owned(),
                    GrantLevel::Machine,
                    Some(100)
                ),
                (
                    GrantWhat::Service,
                    "com.example.helper".to_owned(),
                    GrantLevel::Machine,
                    Some(100)
                ),
            ]
        );
    }
    // Remove, from the other project's list: gone from both, audited, with its record.
    let id = grants_of(
        other.path(),
        &chats,
        &crate::chatbrowsers::ChatBrowsers::default(),
    )
    .into_iter()
    .find(|one| one.level == GrantLevel::Machine && one.what == GrantWhat::Socket)
    .expect("listed")
    .id;
    revoke(other.path(), &chats, &id, &audit).expect("removed");
    assert!(sandbox::local::sockets_everywhere().is_empty());
    assert_eq!(listed_in(root).len(), 1);
    assert_eq!(sandbox::local::made_everywhere().len(), 1);
    let last = seen.lock().unwrap().last().cloned().expect("audited");
    assert_eq!((last.1, last.4), (false, "machine"));
    // Removed twice, it is no longer there.
    assert!(revoke(other.path(), &chats, &id, &audit).is_err());
    sandbox::local::everywhere_for_this_test(None);
}

#[test]
fn a_vault_you_allowed_a_persona_is_listed_and_revoke_takes_it_out_audited_once() {
    let project = tempfile::tempdir().expect("a project");
    let root = project.path();
    let chats = crate::chats::Chats::new();
    sandbox::local::grant_vault(root, "devops", "steward").expect("granted");
    sandbox::local::record_made(
        root,
        sandbox::local::Made {
            what: "vault".to_owned(),
            target: "devops for steward".to_owned(),
            level: "you".to_owned(),
            at: 42,
            chat: Some("steward 1".to_owned()),
        },
    )
    .expect("recorded");
    let listed = grants_of(root, &chats, &crate::chatbrowsers::ChatBrowsers::default());
    assert_eq!(
        listed,
        [SandboxGrant {
            id: "you\u{1f}vault\u{1f}devops\u{1f}steward".to_owned(),
            what: GrantWhat::Vault,
            target: "devops".to_owned(),
            persona: Some("steward".to_owned()),
            level: GrantLevel::You,
            by: None,
            at: Some(42),
            chat: Some("steward 1".to_owned()),
            locked: None,
            waiting: None,
            for_no_persona: false,
            expected: false,
            cautions: None,
        }]
    );
    // A block's Allow never grants one: only the refused vault's own Notice does.
    let from_a_block = allow(
        root,
        &machine(),
        &chats,
        3,
        (GrantWhat::Vault, "devops", GrantLevel::You),
        &no_audit(),
        100,
    )
    .expect_err("refused")
    .to_string();
    assert!(
        from_a_block.contains("notice its refusal raises"),
        "{from_a_block}"
    );
    // Not audited, not revoked.
    assert!(revoke(root, &chats, &listed[0].id, &no_audit()).is_err());
    assert_eq!(
        grants_of(root, &chats, &crate::chatbrowsers::ChatBrowsers::default()).len(),
        1
    );

    let heard = std::sync::Mutex::new(Vec::new());
    let audit = |number: Option<u32>, audited: &sandbox::grant::Audited<'_>| {
        heard.lock().unwrap().push((
            number,
            audited.kind(),
            audited.what.to_owned(),
            audited.target.to_owned(),
        ));
        Ok(())
    };
    revoke(root, &chats, &listed[0].id, &audit).expect("revoked");
    assert!(grants_of(root, &chats, &crate::chatbrowsers::ChatBrowsers::default()).is_empty());
    assert!(sandbox::local::granted_vaults(root).is_empty());
    assert!(sandbox::local::made(root).is_empty());
    let again = revoke(root, &chats, &listed[0].id, &audit).expect_err("not there");
    assert!(again.contains("no longer there"), "{again}");
    assert_eq!(
        *heard.lock().unwrap(),
        [(
            None,
            "trust.sandbox.revoke",
            "vault".to_owned(),
            "devops for steward".to_owned()
        )],
        "one audit, for the one revoke that happened"
    );
}

/// #1362, D-1362-7: a persona's committed hosts reach nothing here until the person allows
/// them as shown; the Allow is audited, listed credited to the persona, and revocable.
#[test]
fn a_persona_s_hosts_are_allowed_as_shown_listed_for_the_persona_and_revoked() {
    let project = tempfile::tempdir().expect("a project");
    let root = project.path();
    std::fs::write(
        root.join("charter.toml"),
        "schema = 1\n\n[sandbox]\nmode = \"on\"\n\n[sandbox.personas.devops]\n\
             hosts = [\"10.0.0.5:6443\", \"*.internal.example\"]\n",
    )
    .expect("toml");
    let chats = crate::chats::Chats::new();
    let state = state_of(root);
    assert_eq!(state.persona_hosts.len(), 1);
    let shown = state.persona_hosts[0].clone();
    assert!(!shown.allowed);
    assert!(grants_of(root, &chats, &crate::chatbrowsers::ChatBrowsers::default()).is_empty());

    // A digest that is not the list's as it stands allows nothing.
    let stale = allow_persona(root, "devops", "not-what-stands", &no_audit(), 7)
        .expect_err("changed since");
    assert!(stale.contains("changed after they were shown"), "{stale}");
    // An Allow that is not audited is no Allow.
    assert!(allow_persona(root, "devops", &shown.digest, &no_audit(), 7).is_err());
    assert!(!state_of(root).persona_hosts[0].allowed);

    let heard = std::sync::Mutex::new(Vec::new());
    let audit = |number: Option<u32>, audited: &sandbox::grant::Audited<'_>| {
        heard.lock().unwrap().push((
            number,
            audited.kind(),
            audited.what.to_owned(),
            audited.target.to_owned(),
        ));
        Ok(())
    };
    allow_persona(root, "devops", &shown.digest, &audit, 42).expect("allowed");
    assert!(state_of(root).persona_hosts[0].allowed);
    let listed = grants_of(root, &chats, &crate::chatbrowsers::ChatBrowsers::default());
    assert_eq!(
        listed,
        [SandboxGrant {
            id: "you\u{1f}persona-hosts\u{1f}devops".to_owned(),
            what: GrantWhat::PersonaHosts,
            target: "10.0.0.5:6443, *.internal.example".to_owned(),
            persona: Some("devops".to_owned()),
            level: GrantLevel::You,
            by: None,
            at: Some(42),
            chat: None,
            locked: None,
            waiting: None,
            for_no_persona: false,
            expected: false,
            cautions: None,
        }]
    );
    // A teammate changes the list: the Allow grants nothing, and is listed as waiting.
    std::fs::write(
        root.join("charter.toml"),
        "schema = 1\n\n[sandbox]\nmode = \"on\"\n\n[sandbox.personas.devops]\n\
             hosts = [\"10.0.0.5:6443\"]\n",
    )
    .expect("toml");
    let now = state_of(root).persona_hosts[0].clone();
    assert!(!now.allowed);
    assert!(now.waiting);
    let waiting = grants_of(root, &chats, &crate::chatbrowsers::ChatBrowsers::default());
    assert_eq!(waiting.len(), 1);
    assert_eq!(waiting[0].id, listed[0].id);
    assert!(
        waiting[0]
            .waiting
            .as_deref()
            .is_some_and(|why| why.starts_with("waiting: the list changed")),
        "{waiting:?}"
    );
    // Put back as it was allowed: it stays waiting until it is allowed anew (D-1362-13).
    std::fs::write(
        root.join("charter.toml"),
        "schema = 1\n\n[sandbox]\nmode = \"on\"\n\n[sandbox.personas.devops]\n\
             hosts = [\"10.0.0.5:6443\", \"*.internal.example\"]\n",
    )
    .expect("toml");
    assert!(state_of(root).persona_hosts[0].waiting);
    assert!(
        grants_of(root, &chats, &crate::chatbrowsers::ChatBrowsers::default())[0]
            .waiting
            .is_some()
    );
    revoke(root, &chats, &listed[0].id, &audit).expect("revoked");
    assert!(grants_of(root, &chats, &crate::chatbrowsers::ChatBrowsers::default()).is_empty());
    assert!(!state_of(root).persona_hosts[0].allowed);
    assert!(!state_of(root).persona_hosts[0].waiting);
    assert_eq!(
        *heard.lock().unwrap(),
        [
            (
                None,
                "trust.sandbox.grant",
                "persona-hosts".to_owned(),
                "devops: 10.0.0.5:6443, *.internal.example".to_owned()
            ),
            (
                None,
                "trust.sandbox.revoke",
                "persona-hosts".to_owned(),
                "devops".to_owned()
            ),
        ]
    );
}

#[test]
fn a_vault_grant_policy_forbids_is_listed_locked() {
    use sandbox::policy::{Locks, set_for_this_test};
    let project = tempfile::tempdir().expect("a project");
    let root = project.path();
    sandbox::local::grant_vault(root, "devops", "steward").expect("granted");
    set_for_this_test(Locks::parse(
        r#"{"owner": "IT", "sandbox": {"vault-grants": false}}"#,
        std::path::Path::new("/etc/purlis/policy.json"),
    ));
    let listed = grants_of(
        root,
        &crate::chats::Chats::new(),
        &crate::chatbrowsers::ChatBrowsers::default(),
    );
    set_for_this_test(Locks::none());
    assert_eq!(
        listed[0].locked.as_deref(),
        Some(
            "Policy forbids allowing a persona a vault it is not tagged for. Locked by \
                 policy, set by IT in /etc/purlis/policy.json."
        )
    );
}

/// #1431: an entry policy locks out is kept, and a person may still take it away: Revoke
/// works on it while the policy stands, audited as any other.
#[test]
fn a_grant_policy_locks_out_is_still_revoked() {
    let project = tempfile::tempdir().expect("a project");
    let root = project.path();
    let chats = crate::chats::Chats::new();
    sandbox::local::grant_write(root, std::path::Path::new("/tmp/tool-cache")).expect("kept");
    sandbox::local::grant_vault(root, "devops", "steward").expect("kept");
    let _under = Under::policy(
        r#"{"owner": "IT", "sandbox": {"write-grants": false, "vault-grants": false}}"#,
    );
    let listed = grants_of(root, &chats, &crate::chatbrowsers::ChatBrowsers::default());
    assert_eq!(listed.len(), 2, "{listed:?}");
    assert!(listed.iter().all(|one| one.locked.is_some()), "{listed:?}");
    let heard = std::sync::Mutex::new(0);
    let audit = |_: Option<u32>, _: &sandbox::grant::Audited<'_>| {
        *heard.lock().unwrap() += 1;
        Ok(())
    };
    for one in &listed {
        revoke(root, &chats, &one.id, &audit).expect("revoked");
    }
    assert!(grants_of(root, &chats, &crate::chatbrowsers::ChatBrowsers::default()).is_empty());
    assert_eq!(*heard.lock().unwrap(), 2);
}

/// #1927: what was kept at a scope a policy's `allow-scopes` now leaves out is listed as
/// held by policy, whichever level kept it, with who set the policy. It is neither taken
/// off the record nor hidden, and Remove still takes it away.
#[test]
fn what_was_kept_at_a_scope_policy_leaves_out_is_listed_as_held_and_still_removed() {
    let everywhere = tempfile::tempdir().expect("a config home");
    sandbox::local::everywhere_for_this_test(Some(
        everywhere.path().join(sandbox::local::EVERY_PROJECT),
    ));
    let project = tempfile::tempdir().expect("a project");
    let root = project.path();
    let chats = crate::chats::Chats::new();
    let path = std::path::Path::new;
    sandbox::local::grant_write(root, path("/tmp/tool-cache")).expect("kept");
    sandbox::local::grant_socket(root, path("/purlis-test-nowhere/mine.sock")).expect("kept");
    sandbox::local::grant_socket_everywhere(path("/purlis-test-nowhere/docker.sock"), None)
        .expect("kept");
    sandbox::local::grant_service_everywhere("com.example.helper", None).expect("kept");
    let held = |root: &std::path::Path| -> Vec<(GrantLevel, GrantWhat, bool)> {
        grants_of(root, &chats, &crate::chatbrowsers::ChatBrowsers::default())
            .into_iter()
            .map(|one| (one.level, one.what, one.locked.is_some()))
            .collect()
    };
    let rows = |you: bool, machine: bool| {
        vec![
            (GrantLevel::You, GrantWhat::Write, you),
            (GrantLevel::You, GrantWhat::Socket, you),
            (GrantLevel::Machine, GrantWhat::Socket, machine),
            (GrantLevel::Machine, GrantWhat::Service, machine),
        ]
    };
    assert_eq!(held(root), rows(false, false), "no policy holds nothing");
    {
        // A policy that fixes no scopes holds nothing either.
        let _under = Under::policy(r#"{"owner": "IT", "sandbox": {}}"#);
        assert_eq!(held(root), rows(false, false));
    }
    {
        let _under =
            Under::policy(r#"{"owner": "IT", "sandbox": {"allow-scopes": ["chat", "you"]}}"#);
        assert_eq!(held(root), rows(false, true));
        let why = grants_of(root, &chats, &crate::chatbrowsers::ChatBrowsers::default())
            .into_iter()
            .find(|one| one.level == GrantLevel::Machine)
            .and_then(|one| one.locked)
            .expect("held");
        assert_eq!(
            why,
            "Held by policy, which leaves out what was allowed on this machine, for every \
                 project. Locked by policy, set by IT in /etc/purlis/policy.json."
        );
    }
    {
        let _under =
            Under::policy(r#"{"owner": "IT", "sandbox": {"allow-scopes": ["chat", "machine"]}}"#);
        assert_eq!(held(root), rows(true, false));
    }
    let _under = Under::policy(r#"{"owner": "IT", "sandbox": {"allow-scopes": ["chat"]}}"#);
    assert_eq!(held(root), rows(true, true));
    // Nothing left the record, and each is still the person's to take away.
    assert_eq!(sandbox::local::sockets_everywhere().len(), 1);
    assert_eq!(sandbox::local::granted_sockets(root).len(), 1);
    let heard = std::sync::Mutex::new(0);
    let audit = |_: Option<u32>, _: &sandbox::grant::Audited<'_>| {
        *heard.lock().unwrap() += 1;
        Ok(())
    };
    for one in grants_of(root, &chats, &crate::chatbrowsers::ChatBrowsers::default()) {
        revoke(root, &chats, &one.id, &audit).expect("removed");
    }
    assert!(grants_of(root, &chats, &crate::chatbrowsers::ChatBrowsers::default()).is_empty());
    assert_eq!(*heard.lock().unwrap(), 4);
    assert!(sandbox::local::sockets_everywhere().is_empty());
    sandbox::local::everywhere_for_this_test(None);
}

/// #1927: a block's own Allow of a local socket keeps what its Notice drew, the socket by
/// the name the kernel gave it then. Where its path leads somewhere else by the press,
/// nothing is kept: the Allow is refused in fixed words, as the up-front Allow is.
#[test]
fn a_blocks_socket_allow_keeps_only_the_socket_its_notice_drew() {
    let drawn = "/Users/dev/.docker/run/docker.sock";
    // Still the socket the Notice named: kept as drawn.
    assert_eq!(
        socket_as_drawn(drawn, drawn.into()),
        Ok(std::path::PathBuf::from(drawn))
    );
    // Its path leads to another socket now (a link in it moved since): never kept,
    // whatever the other socket is, and said as a change, not as a refusal's words.
    assert_eq!(
        socket_as_drawn(drawn, "/Users/dev/.other/engine.sock".into()),
        Err(NotJudged::SocketChanged)
    );
    assert_eq!(NotJudged::SocketChanged.to_string(), SOCKET_CHANGED);
    // The sentence is purlis's own, whole: it names no path a chat could have chosen.
    assert!(!SOCKET_CHANGED.contains('/'), "{SOCKET_CHANGED}");
}

/// #1927: when an Allow finds its socket changed, the Notice is told the block again as
/// it stands now, and the chat is held on that, never on what was drawn before.
#[test]
fn a_block_whose_socket_changed_is_told_again_as_it_stands() {
    let project = tempfile::tempdir().expect("a project");
    let root = project.path().canonicalize().expect("real");
    let plane: PlaneId =
        serde_json::from_value(serde_json::json!(root.display().to_string())).expect("a plane id");
    let chats = crate::chats::Chats::new();
    let was = crate::taskblocks::HeldBlock {
        operation: "connect".to_owned(),
        kind: "local-socket".to_owned(),
        what: GrantWhat::Socket,
        target: "/purlis-test-nowhere/docker.sock".to_owned(),
    };
    chats.blocks().heard(4, was.clone());
    let told = shown_again(&plane, &chats, 4, &was).expect("told again");
    assert_eq!(told.session, 4);
    assert_eq!(
        (told.operation.as_str(), told.kind.as_str()),
        ("connect", "local-socket")
    );
    // Nothing is there now, so no Allow is drawn for it, and it says why.
    assert_ne!(told.offer, crate::hooks::BlockOffer::Socket);
    assert!(told.levels.is_empty());
    assert!(told.route.is_some());
    assert!(
        !chats.blocks().holds(4, &was),
        "the block as it was drawn is answered by nothing more"
    );
}

#[test]
fn a_folder_you_were_granted_is_listed_and_revoke_takes_it_out_audited_once() {
    let project = tempfile::tempdir().expect("a project");
    let root = project.path();
    let chats = crate::chats::Chats::new();
    let folder = std::path::PathBuf::from("/tmp/tool-cache");
    sandbox::local::grant_write(root, &folder).expect("granted");
    sandbox::local::record_made(
        root,
        sandbox::local::Made {
            what: "write".to_owned(),
            target: "/tmp/tool-cache".to_owned(),
            level: "you".to_owned(),
            at: 42,
            chat: None,
        },
    )
    .expect("recorded");
    let listed = grants_of(root, &chats, &crate::chatbrowsers::ChatBrowsers::default());
    assert_eq!(
        listed,
        [SandboxGrant {
            id: "you\u{1f}write\u{1f}/tmp/tool-cache".to_owned(),
            what: GrantWhat::Write,
            target: "/tmp/tool-cache".to_owned(),
            persona: None,
            level: GrantLevel::You,
            by: None,
            at: Some(42),
            chat: None,
            locked: None,
            waiting: None,
            for_no_persona: false,
            expected: false,
            cautions: None,
        }]
    );
    // Not audited, not revoked.
    assert!(revoke(root, &chats, &listed[0].id, &no_audit()).is_err());
    assert_eq!(
        grants_of(root, &chats, &crate::chatbrowsers::ChatBrowsers::default()).len(),
        1
    );

    let heard = std::sync::Mutex::new(Vec::new());
    let audit = |number: Option<u32>, audited: &sandbox::grant::Audited<'_>| {
        heard
            .lock()
            .unwrap()
            .push((number, audited.kind(), audited.target.to_owned()));
        Ok(())
    };
    revoke(root, &chats, &listed[0].id, &audit).expect("revoked");
    assert!(grants_of(root, &chats, &crate::chatbrowsers::ChatBrowsers::default()).is_empty());
    assert!(sandbox::local::granted_writes(root).is_empty());
    // Revoked twice: the second is refused before anything is audited.
    let again = revoke(root, &chats, &listed[0].id, &audit).expect_err("not there");
    assert!(again.contains("no longer there"), "{again}");
    assert!(revoke(root, &chats, "nonsense", &audit).is_err());
    assert_eq!(
        *heard.lock().unwrap(),
        [(None, "trust.sandbox.revoke", "/tmp/tool-cache".to_owned())],
        "one audit, for the one revoke that happened"
    );
}

#[test]
fn a_folder_you_list_widens_the_allowlist_and_nothing_a_class_or_path_holds() {
    let project = tempfile::tempdir().expect("a project");
    let root = project.path();
    assert_eq!(
        list_folder(root, &machine(), "/noopt/tools"),
        Ok(GrantableListed {
            folders: vec!["/noopt/tools".to_owned()],
            holds: Vec::new(),
        })
    );
    for refused in ["/", "/nohome/dev", "relative"] {
        assert!(list_folder(root, &machine(), refused).is_err(), "{refused}");
    }
    // Listed, and Settings is told what it holds that later code loads (review R3): an
    // editor's user settings, under it.
    let support = "/nohome/dev/Library/Application Support";
    let held = list_folder(root, &machine(), support).expect("listed");
    assert!(
        held.holds
            .iter()
            .any(|one| one.ends_with("Application Support")),
        "{held:?}"
    );
    // What macOS starts at login is later code itself since #2071: never listed.
    assert!(list_folder(root, &machine(), "/nohome/dev/Library/LaunchAgents").is_err());
    // One that holds a harness's own is refused outright.
    assert!(list_folder(root, &machine(), "/nohome/dev/Library").is_err());
    assert_eq!(
        listed_folders(root),
        ["/noopt/tools", "/nohome/dev/Library/Application Support"]
    );
}

#[cfg(unix)]
#[test]
fn a_listed_folder_that_now_resolves_elsewhere_is_dropped_and_said() {
    // Review R8: listed, then swapped for a link by whatever could write where it was.
    let project = tempfile::tempdir().expect("a project");
    let root = project.path().canonicalize().expect("real");
    let listed = root.join("tools");
    let out = root.join("elsewhere");
    std::fs::create_dir_all(&listed).expect("made");
    std::fs::create_dir_all(&out).expect("made");
    sandbox::local::list_grantable(&root, &listed).expect("listed");
    assert_eq!(listed_and_pruned(&root).dropped, Vec::<String>::new());
    std::fs::remove_dir(&listed).expect("removed");
    std::os::unix::fs::symlink(&out, &listed).expect("linked");
    let now = listed_and_pruned(&root);
    assert_eq!(now.dropped, [listed.display().to_string()]);
    assert!(now.folders.is_empty());
}

#[test]
fn a_block_report_is_drafted_from_the_fixed_words_alone() {
    let report = sandbox_block_report(
        "write".to_owned(),
        "project-files".to_owned(),
        Some("claude".to_owned()),
    )
    .unwrap();
    assert_eq!(report.repository, purlis_core::report::UPSTREAM);
    assert_eq!(
        report.title,
        "Sandbox blocked purlis's own write (project-files)"
    );
    assert!(
        report.body.contains("- **harness:** claude"),
        "{}",
        report.body
    );
    // A harness word purlis does not start is not one, and says nothing of itself.
    let other = sandbox_block_report(
        "write".to_owned(),
        "project-files".to_owned(),
        Some("/home/dev/my-harness".to_owned()),
    )
    .unwrap();
    assert!(!other.body.contains("/home/dev"), "{}", other.body);
    assert!(
        other.body.contains("- **harness:** not known"),
        "{}",
        other.body
    );
}

#[test]
fn a_block_report_refuses_words_the_sandbox_does_not_report() {
    for (operation, kind) in [
        ("write", "/Users/dev/plane/workspaces/a/sessions"),
        ("rm -rf /", "project-files"),
        ("", ""),
    ] {
        assert!(
            sandbox_block_report(operation.to_owned(), kind.to_owned(), None).is_err(),
            "{operation} {kind}"
        );
    }
}

#[test]
fn only_the_draft_that_was_shown_is_filed() {
    let shown = sandbox_block_report("connect".to_owned(), "host".to_owned(), None).unwrap();
    assert!(shown_draft("connect", "host", None, &shown.digest).is_ok());
    let refused = shown_draft("connect", "host", Some("claude"), &shown.digest).unwrap_err();
    assert!(refused.contains("nothing was sent"), "{refused}");
    assert!(shown_draft("write", "host", None, &shown.digest).is_err());
}

#[test]
fn a_project_without_the_sandbox_shows_the_picker_nothing() {
    assert_eq!(SandboxAhead::of(Ahead::Off), None);
}

#[test]
fn the_picker_is_told_a_refusal_with_its_install_command() {
    assert_eq!(
        SandboxAhead::of(Ahead::Refused {
            why: "socat is not installed".to_owned(),
            install: Some("sudo apt install socat".to_owned()),
        }),
        Some(SandboxAhead {
            state: "refused".to_owned(),
            said: "socat is not installed".to_owned(),
            install: Some("sudo apt install socat".to_owned()),
            locked: None,
        })
    );
}

#[test]
fn on_windows_the_picker_is_told_the_chat_starts_without_it_and_why() {
    let lifted = sandbox::Lifted {
        by: sandbox::By::NoBackend(sandbox::Os::Windows),
        reason: None,
    };
    let row = SandboxAhead::of(Ahead::Unsandboxed(lifted.clone())).expect("a row");
    assert_eq!(row.state, "unsandboxed");
    assert_eq!(row.said, lifted.notice());
}

/// Ruling V78 c: the line is the compiled-in command for what is missing, typed and never
/// run, so it carries no newline.
#[test]
fn the_install_line_is_charters_own_command_with_no_return_at_the_end() {
    let missing = backend::missing(sandbox::Os::Linux, &|_| false);
    assert_eq!(
        install_line(missing.as_ref(), "ID=fedora\n").as_deref(),
        Ok("sudo dnf install bubblewrap socat")
    );
    assert!(
        install_line(None, "ID=fedora\n").is_err(),
        "nothing missing"
    );
    assert!(
        install_line(missing.as_ref(), "ID=nixos\n").is_err(),
        "a distribution purlis does not know"
    );
}

/// #1340: Settings › Sandbox draws the core's presets, each with the hosts the core lets a
/// chat in this project reach, the project's forges among code hosting's.
#[test]
fn settings_is_handed_every_preset_with_the_hosts_the_core_lists() {
    let plane = sandbox::Plane::of(Some(
        "[sandbox]\nmode = \"on\"\n\n[[forge]]\nkind = \"gitlab\"\nhost = \"git.example.org\"\n",
    ));
    let shown = presets_of(&plane, &sandbox::policy::Locks::none());
    assert_eq!(
        shown
            .iter()
            .map(|one| (one.word.as_str(), one.title.as_str()))
            .collect::<Vec<_>>(),
        [
            ("model-providers", "AI providers"),
            ("forge", "Code hosting"),
            ("toolchains", "Package registries"),
        ]
    );
    for (one, preset) in shown.iter().zip(sandbox::Preset::ALL) {
        assert_eq!(
            one.hosts,
            sandbox::hosts(&[preset], &plane, &sandbox::policy::Locks::none())
        );
    }
    assert!(shown[1].hosts.contains(&"git.example.org".to_owned()));
    assert!(!shown[0].hosts.contains(&"git.example.org".to_owned()));
    // #1422: the core says which preset widens the package caches.
    assert_eq!(
        shown
            .iter()
            .map(|one| one.widens_caches)
            .collect::<Vec<_>>(),
        [false, false, true]
    );
}

/// #1340, #1362: a persona's own hosts, as the core read them, and whether the person
/// allowed them here (D-1362-7).
#[test]
fn settings_is_handed_each_personas_own_hosts() {
    let root = tempfile::tempdir().expect("a project");
    let plane = sandbox::Plane::of(Some(
        "[sandbox]\nmode = \"on\"\n\n[sandbox.personas.devops]\nhosts = [\"10.0.0.5:6443\"]\n",
    ));
    let none = sandbox::policy::Locks::none();
    let digest = sandbox::persona::digest(
        &[sandbox::hosts::Host::parse("10.0.0.5:6443").expect("a host")],
        false,
    );
    let devops = |allowed: bool| PersonaHosts {
        persona: "devops".to_owned(),
        hosts: vec!["10.0.0.5:6443".to_owned()],
        reached: vec!["10.0.0.5:6443".to_owned()],
        tools: Vec::new(),
        digest: digest.clone(),
        allowed,
        default: false,
        waiting: false,
        cautions: None,
        refused: None,
    };
    assert_eq!(
        persona_hosts_of(root.path(), &plane, &none),
        [devops(false)]
    );
    sandbox::local::allow_persona_hosts(root.path(), "devops", &digest).expect("kept");
    assert_eq!(persona_hosts_of(root.path(), &plane, &none), [devops(true)]);
    assert!(persona_hosts_of(root.path(), &sandbox::Plane::of(None), &none).is_empty());
}

/// #1411 C3 (D-1410e): a project whose own manifest has gone, or cannot be read as TOML,
/// refuses every chat, so Settings says so as its own state, never as "off", and offers
/// nothing that would write a manifest in its place.
#[test]
fn a_project_whose_manifest_is_gone_or_unreadable_says_chats_are_refused_never_off() {
    let gone = tempfile::tempdir().expect("a project with no manifest");
    let state = state_of(gone.path());
    let said = state.manifest.expect("the gone manifest is said");
    assert!(
        said.contains("is missing") && said.contains("refused until"),
        "{said}"
    );
    assert!(
        !state.offer,
        "no offer writes a manifest in place of the gone one"
    );

    let broken = tempfile::tempdir().expect("a project");
    std::fs::write(broken.path().join("charter.toml"), "[sandbox\nmode = ").expect("toml");
    let state = state_of(broken.path());
    let said = state.manifest.expect("the unreadable manifest is said");
    assert!(
        said.contains("cannot be read") && said.contains("refused until"),
        "{said}"
    );
    assert!(!state.offer);

    let fine = tempfile::tempdir().expect("a project");
    std::fs::write(fine.path().join("charter.toml"), "schema = 1\n").expect("toml");
    assert_eq!(state_of(fine.path()).manifest, None);
}

/// D-PE-ruling-1: a persona's tools are shown with its hosts and wait for the same Allow; a
/// persona with tools alone is asked about too.
#[test]
fn a_personas_tools_are_shown_and_allowed_with_its_hosts() {
    let root = tempfile::tempdir().expect("a project");
    let plane = sandbox::Plane::of(Some(
        "[sandbox]\nmode = \"on\"\n\n[sandbox.personas.devops]\ntools = { kubectl = { vault \
             = \"devops\", as = \"KUBECONFIG file\", key = \"kubeconfig\" } }\n",
    ));
    let none = sandbox::policy::Locks::none();
    let shown = persona_hosts_of(root.path(), &plane, &none);
    assert_eq!(shown.len(), 1);
    assert_eq!(
        shown[0].tools,
        ["kubectl with kubeconfig from vault devops as KUBECONFIG file"]
    );
    assert!(shown[0].reached.is_empty());
    assert!(!shown[0].allowed);
    sandbox::local::allow_persona_hosts(root.path(), "devops", &shown[0].digest).expect("kept");
    assert!(persona_hosts_of(root.path(), &plane, &none)[0].allowed);
}

/// T-1: a tool handed several values is shown with every value on its line, its Allow is of
/// all of them, and a change to any one asks again.
#[test]
fn a_tool_handed_several_values_is_shown_and_allowed_whole() {
    let root = tempfile::tempdir().expect("a project");
    let declared = |key: &str| {
        sandbox::Plane::of(Some(&format!(
            "[sandbox]\nmode = \"on\"\n\n[sandbox.personas.logs]\ntools = {{ curl = {{ vault = \
             \"elastic-logs-master\", values = {{ KIBANA_USER = {{ key = \"kibana-username\", as = \
             \"env\" }}, KIBANA_PASS = {{ key = \"{key}\", as = \"file\" }} }} }} }}\n"
        )))
    };
    let none = sandbox::policy::Locks::none();
    let plane = declared("kibana-password");
    let shown = persona_hosts_of(root.path(), &plane, &none);
    let line = "curl with \"kibana-password\" as KIBANA_PASS file and \"kibana-username\" as \
                KIBANA_USER env from vault elastic-logs-master";
    assert_eq!(shown[0].tools, [line]);
    sandbox::local::allow_persona_hosts(root.path(), "logs", &shown[0].digest).expect("kept");
    assert!(persona_hosts_of(root.path(), &plane, &none)[0].allowed);
    let changed = persona_hosts_of(root.path(), &declared("admin-password"), &none);
    assert!(!changed[0].allowed, "one value's key changed: asked again");
    // On the Granted row and in the audit, hosts by commas, and two tools never run together.
    let host = sandbox::hosts::Host::parse("10.0.0.5:6443").expect("a host");
    let kubectl = sandbox::tools::Tool::one(
        "kubectl",
        "devops",
        "KUBECONFIG",
        sandbox::tools::Handed::File,
        "kubeconfig",
    );
    let tools = sandbox::persona::shown_in(root.path(), &plane, &none)[0]
        .tools
        .iter()
        .cloned()
        .chain([kubectl])
        .collect::<Vec<_>>();
    assert_eq!(
        hosts_and_tools(&[host.clone(), host], &tools),
        format!(
            "10.0.0.5:6443, 10.0.0.5:6443; {line}; kubectl with kubeconfig from vault devops as \
             KUBECONFIG file"
        )
    );
    assert_eq!(hosts_and_tools(&[], &[]), "");
}

/// T-4: a tool whose program is a general-purpose one carries one sentence beside its Allow, a
/// tool off the list none, and the sentence is never part of what the Allow is bound to.
#[test]
fn a_general_purpose_tool_says_so_beside_its_allow_and_an_unlisted_one_does_not() {
    let project = tempfile::tempdir().expect("a project");
    let root = project.path();
    std::fs::write(
        root.join("charter.toml"),
        "schema = 1\n\n[sandbox]\nmode = \"on\"\n\n[sandbox.personas.logs]\ntools = { curl = { \
         vault = \"api\", as = \"API_TOKEN env\", key = \"api-token\" } }\n\n\
         [sandbox.personas.cloud]\ntools = { aws = { vault = \"cloud\", as = \"AWS_TOKEN env\", \
         key = \"aws-token\" } }\n",
    )
    .expect("toml");
    let said = "curl can show the values it is handed in the chat's output; allow it only for a \
                persona you trust with them.";
    let shown = state_of(root).persona_hosts;
    let of = |persona: &str| {
        shown
            .iter()
            .find(|one| one.persona == persona)
            .cloned()
            .expect("shown")
    };
    let logs = of("logs");
    assert_eq!(logs.cautions, Some(vec![said.to_owned()]));
    assert_eq!(of("cloud").cautions, None);
    // The line and its digest are the tool's alone, as before the sentence.
    assert_eq!(
        logs.tools,
        ["curl with api-token from vault api as API_TOKEN env"]
    );
    assert_eq!(
        logs.digest,
        sandbox::persona::digest_of(
            &[],
            &[sandbox::tools::Tool::one(
                "curl",
                "api",
                "API_TOKEN",
                sandbox::tools::Handed::Env,
                "api-token"
            )],
            false
        )
    );
    // On the Granted row too, once allowed.
    let chats = crate::chats::Chats::new();
    sandbox::local::allow_persona_hosts(root, "logs", &logs.digest).expect("kept");
    sandbox::local::allow_persona_hosts(root, "cloud", &of("cloud").digest).expect("kept");
    let rows = grants_of(root, &chats, &crate::chatbrowsers::ChatBrowsers::default());
    let row = |persona: &str| {
        rows.iter()
            .find(|one| one.persona.as_deref() == Some(persona))
            .expect("listed")
    };
    assert_eq!(row("logs").cautions, Some(vec![said.to_owned()]));
    assert_eq!(
        row("logs").target,
        "curl with api-token from vault api as API_TOKEN env"
    );
    assert_eq!(row("cloud").cautions, None);
}

#[test]
fn taking_the_offer_turns_the_project_on_and_it_is_not_offered_again() {
    let project = tempfile::tempdir().expect("a project");
    std::fs::write(project.path().join("charter.toml"), "schema = 1\n").expect("toml");
    assert_eq!(
        state_of(project.path()),
        SandboxState {
            on: false,
            offer: true,
            said: None,
            never: never_here(sandbox::Os::this()),
            hosts_changed: None,
            browser_sites_changed: None,
            presets_changed: None,
            presets: presets_of(
                &sandbox::Plane::read(project.path()),
                &sandbox::policy::Locks::none(),
            ),
            persona_hosts: Vec::new(),
            besides: SandboxBesides::default(),
            policy: None,
            manifest: None,
            certificate_check: None,
            expects: None,
            team_off: None,
            level_held: None,
            yours: None,
        }
    );

    let after = answer(project.path(), true).expect("answered");

    assert_eq!(
        after,
        SandboxState {
            on: true,
            offer: false,
            said: Some(
                "no chat has started under this project's sandbox on this machine yet".to_owned()
            ),
            never: never_here(sandbox::Os::this()),
            hosts_changed: None,
            browser_sites_changed: None,
            presets_changed: None,
            presets: presets_of(
                &sandbox::Plane::read(project.path()),
                &sandbox::policy::Locks::none(),
            ),
            persona_hosts: Vec::new(),
            besides: SandboxBesides::default(),
            policy: None,
            manifest: None,
            certificate_check: Some(CertificateCheck {
                on: true,
                locked: None,
                widens: sandbox::Os::this() == sandbox::Os::MacOs,
            }),
            expects: None,
            team_off: None,
            level_held: None,
            yours: None,
        }
    );
}

#[test]
fn keeping_it_off_leaves_the_project_as_it_was_and_asks_no_more() {
    let project = tempfile::tempdir().expect("a project");
    std::fs::write(project.path().join("charter.toml"), "schema = 1\n").expect("toml");

    let after = answer(project.path(), false).expect("answered");

    assert_eq!(
        after,
        SandboxState {
            on: false,
            offer: false,
            said: None,
            never: never_here(sandbox::Os::this()),
            hosts_changed: None,
            browser_sites_changed: None,
            presets_changed: None,
            presets: presets_of(
                &sandbox::Plane::read(project.path()),
                &sandbox::policy::Locks::none(),
            ),
            persona_hosts: Vec::new(),
            besides: SandboxBesides::default(),
            policy: None,
            manifest: None,
            certificate_check: None,
            expects: None,
            team_off: None,
            level_held: None,
            yours: None,
        }
    );
    assert_eq!(
        std::fs::read_to_string(project.path().join("charter.toml")).expect("toml"),
        "schema = 1\n"
    );
}

/// The picker's row for `profile` alone, on this machine.
fn ahead_here(
    profile: &purlis_core::profiles::Profile,
    root: &std::path::Path,
) -> Option<SandboxAhead> {
    ahead_of_each(
        std::slice::from_ref(profile),
        root,
        &sandbox::Machine::this(),
        &backend::installed,
    )
    .pop()
    .flatten()
}

/// #2179: **one picker compiles each harness's sandbox once**, however many profiles it
/// lists of that harness. Every New tab asks for the picker, and a compile per row made
/// each open pay one per profile (about 200 ms a compile on the macOS runner). A compile
/// asks whether the sandbox's backend is installed, which is what is counted here.
#[test]
fn one_picker_compiles_each_harness_once_however_many_profiles_share_it() {
    let compiles_with = |more_claude_profiles: usize| {
        let plane = tempfile::tempdir().expect("a plane");
        std::fs::write(
            plane.path().join(purlis_core::plane::MANIFEST),
            "[sandbox]\nmode = \"on\"\n",
        )
        .expect("written");
        // Declared and approved by nobody, so the picker runs none of their programs.
        let declared: String = (0..more_claude_profiles)
            .map(|n| format!("[harness.work{n}]\nkind = \"claude\"\ncommand = [\"claude{n}\"]\n"))
            .collect();
        std::fs::write(
            plane.path().join(purlis_core::profiles::LOCAL_FILE),
            declared,
        )
        .expect("written");
        let asked = std::sync::atomic::AtomicUsize::new(0);
        let machine = purlis_core::sandbox::Machine {
            env: purlis_core::secrets::Env::of(&[]),
            home: None,
            // Where every harness has a sandbox charter compiles.
            os: purlis_core::sandbox::Os::MacOs,
        };
        let options = crate::start_options_on(plane.path(), &machine, &|_| {
            asked.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            true
        })
        .expect("read");
        assert_eq!(
            options
                .profiles
                .iter()
                .filter(|row| row.kind == "claude")
                .count(),
            1 + more_claude_profiles,
            "{:?}",
            options.refused
        );
        // Each row is still told its sandbox. Not which: the built-in row checks this
        // machine's own Claude Code, whatever it answers here.
        assert!(
            options.profiles.iter().all(|row| row.sandbox.is_some()),
            "{:?}",
            options
                .profiles
                .iter()
                .map(|row| (&row.name, &row.sandbox))
                .collect::<Vec<_>>()
        );
        asked.into_inner()
    };
    let one = compiles_with(0);
    assert!(one > 0, "the picker compiles the sandbox it shows");
    assert_eq!(
        compiles_with(4),
        one,
        "five Claude Code profiles cost what one does"
    );
}

/// Must-fix (verifier r7): the picker never runs a profile's program before the start's
/// gate lets it start (approved, among the rest). Past the gate, its program is checked,
/// which runs it. The program answers as something other than Claude Code, so whether it
/// ran shows in the row: the probe runs inside charter's wrap (D-88k), where it can leave
/// no other trace.
#[cfg(unix)]
#[test]
fn the_picker_runs_no_program_nobody_approved() {
    let project = tempfile::tempdir().expect("a project");
    // A folder no chat writes: the start refuses a program, or any file its command
    // names, in a temp folder (ruling V87g, D-88g).
    let outside = stand_in::NoChatWrites::new();
    std::fs::write(
        project.path().join("charter.toml"),
        "[sandbox]\nmode = \"on\"\n",
    )
    .expect("charter.toml");
    // Run as `/bin/sh <script>`.
    let script = outside.path().join("claude");
    std::fs::write(&script, "echo 'not claude'\n").expect("the script");
    std::fs::write(
        project.path().join(purlis_core::profiles::LOCAL_FILE),
        format!(
            "[harness.work]\nkind = \"claude\"\ncommand = [\"/bin/sh\", {:?}]\n",
            script.display().to_string()
        ),
    )
    .expect("a profile");
    let set = purlis_core::profiles::current(project.path());
    let profile = set.get("work").expect("declared");

    let row = ahead_here(profile, project.path()).expect("a row");

    // Where this machine can apply the sandbox at all (not a CI runner without bubblewrap):
    // the row says the check waits for the approval, so the program was not run; once
    // approved, it is checked, and its answer refuses it.
    if row.state == "sandboxed" {
        assert_eq!(
            row.said, CHECKED_ONCE_APPROVED,
            "an unapproved program was run"
        );
        purlis_core::profiletrust::record_launched(
            project.path(),
            "work",
            &purlis_core::profiletrust::fingerprint(profile),
        )
        .expect("approved");
        let approved = ahead_here(profile, project.path()).expect("a row");
        assert_eq!(approved.state, "refused", "an approved program is checked");
        assert!(
            approved.said.contains("does not answer as Claude Code"),
            "{approved:?}"
        );
    }
}

/// #1341: a teammate is told once of the project's hosts, and not again once it was read.
#[test]
fn the_project_s_hosts_are_told_once_and_not_again_once_read() {
    let project = tempfile::tempdir().expect("a project");
    std::fs::write(
        project.path().join("charter.toml"),
        "[sandbox]\nmode = \"on\"\nhosts = [\"10.100.39.145:6443\"]\n",
    )
    .expect("toml");
    let told = state_of(project.path()).hosts_changed.expect("told");
    assert_eq!(told.added, ["10.100.39.145:6443"]);
    assert_eq!(told.removed, Vec::<String>::new());

    let after = acknowledge(project.path(), &told.now).expect("read");

    assert_eq!(after.hosts_changed, None);
}

/// #2443: a teammate's browser site is told once, site by site, and reaches a browser here once
/// it was shown; the Granted list has it apart, waiting until then.
#[test]
fn the_project_s_browser_sites_are_told_once_and_wait_in_granted_until_shown() {
    let project = tempfile::tempdir().expect("a project");
    std::fs::write(
        project.path().join("charter.toml"),
        "[sandbox]\nmode = \"on\"\nbrowser-sites = [\"news.example.org:443\"]\n",
    )
    .expect("toml");
    let told = state_of(project.path())
        .browser_sites_changed
        .expect("told");
    assert_eq!(told.added, ["news.example.org:443"]);
    let row = |root: &std::path::Path| {
        grants_of(
            root,
            &crate::chats::Chats::new(),
            &crate::chatbrowsers::ChatBrowsers::default(),
        )
        .into_iter()
        .find(|one| one.what == GrantWhat::BrowserSite)
        .expect("listed")
    };
    let waiting = row(project.path());
    assert_eq!(waiting.level, GrantLevel::Project);
    assert_eq!(waiting.target, "news.example.org:443");
    assert!(waiting.waiting.is_some(), "{waiting:?}");

    let after = acknowledge_browser_sites(project.path(), &told.added).expect("read");

    assert_eq!(after.browser_sites_changed, None);
    assert_eq!(row(project.path()).waiting, None);
    assert_eq!(
        browser_site_of(&row(project.path()).id),
        Some(SiteGranted::Project(
            sandbox::hosts::Host::parse("news.example.org:443").unwrap()
        ))
    );
}

/// #1385: a teammate is told once of a preset that widens, and not again once it was read.
#[test]
fn a_widening_preset_is_told_once_and_not_again_once_read() {
    let project = tempfile::tempdir().expect("a project");
    std::fs::write(
        project.path().join("charter.toml"),
        "[sandbox]\nmode = \"on\"\n",
    )
    .expect("toml");
    // A default project tells nothing: certificate checks are among the defaults (R1).
    assert_eq!(state_of(project.path()).presets_changed, None);
    // This machine last saw the project before they were: told once, with the widening.
    sandbox::local::acknowledge_presets(
        project.path(),
        &["model-providers", "forge", "toolchains"].map(str::to_owned),
    )
    .expect("seen");
    let told = state_of(project.path()).presets_changed.expect("told");
    assert_eq!(told.added, ["Certificate checks"]);
    assert_eq!(told.widens.len(), 1);

    let after = acknowledge_presets(project.path(), &told.now).expect("read");

    assert_eq!(after.presets_changed, None);
}

/// Fold-in (round 11): the offer never reads as covering a harness that is never sandboxed
/// on this system, whatever the project.
#[test]
fn the_offer_names_each_harness_never_sandboxed_on_this_system() {
    // #1123: charter wraps Codex as it wraps opencode, on macOS and, since #1040, on Linux.
    assert!(never_here(sandbox::Os::MacOs).is_empty());
    assert!(never_here(sandbox::Os::Linux).is_empty());
    assert_eq!(
        never_here(sandbox::Os::Other),
        [
            "Codex: purlis can wrap it on macOS and Linux only, so far",
            "opencode: purlis can wrap it on macOS and Linux only, so far",
        ]
    );
}

/// qw384: a service on this Mac is one port, for this chat or every chat of this project on
/// this machine, never once, every project or the project's; and it is judged again, never
/// taken on the window's word.
#[test]
fn a_port_on_this_mac_is_one_port_for_this_chat_or_this_project_here() {
    let project = tempfile::tempdir().expect("a project");
    let chats = crate::chats::Chats::new();
    let refused = |target: &str, level| {
        allow(
            project.path(),
            &machine(),
            &chats,
            3,
            (GrantWhat::Port, target, level),
            &no_audit(),
            100,
        )
        .expect_err("refused")
        .to_string()
    };
    for level in [GrantLevel::Once, GrantLevel::Machine, GrantLevel::Project] {
        assert!(
            refused("50795", level).contains("for this chat, or for every chat of this project"),
            "{level:?}"
        );
    }
    for never in ["0", "1-65535", "*", "localhost"] {
        assert!(
            refused(never, GrantLevel::You).contains("one port"),
            "{never}"
        );
    }
    // An unaudited grant is no grant.
    assert!(refused("50795", GrantLevel::You).contains("event log"));
    // For a chat that is not open, refused before any audit is tried (#2454).
    assert!(refused("50795", GrantLevel::Chat).contains("not open"));
    assert!(sandbox::local::granted_ports(project.path()).is_empty());
    assert!(!chats.holds_for(3, &sandbox::grant::What::Port(50795)));
}

/// #2071 PR C, #2156: the team's off as the window asks it, naming a commit and an author only
/// where purlis verified them on the upstream it fetched.
#[test]
fn the_team_s_off_is_asked_with_its_commit_and_author_and_answered_once() {
    use purlis_core::sandbox::team::{Arrival, Arrived, TeamOff};
    let arrived = Arrived {
        commit: "0123456789abcdef0123456789abcdef01234567".to_owned(),
        author: "Ada Lovelace".to_owned(),
    };
    let upstream = Arrival::Upstream(arrived.clone());
    let asks = TeamOffState::of(TeamOff::Asks(upstream.clone())).expect("asked");
    assert_eq!(asks.commit.as_deref(), Some(arrived.commit.as_str()));
    assert_eq!(asks.author.as_deref(), Some("Ada Lovelace"));
    assert_eq!(asks.answer, None);
    assert_eq!(
        asks.said,
        "Your project turned the sandbox off (commit 0123456 by Ada Lovelace)."
    );
    assert_eq!(
        TeamOffState::of(TeamOff::Accepted(upstream.clone())).map(|one| one.answer),
        Some(Some(true))
    );
    assert_eq!(
        TeamOffState::of(TeamOff::KeptOn(upstream)).map(|one| one.answer),
        Some(Some(false))
    );
    assert_eq!(TeamOffState::of(TeamOff::No), None);
    assert_eq!(TeamOffState::of(TeamOff::NotCommitted), None);

    // Not verified on the upstream: local, with no commit and no author.
    let local = TeamOffState::of(TeamOff::Asks(Arrival::Local)).expect("asked");
    assert_eq!((local.commit, local.author), (None, None));
    assert!(local.said.contains("local"), "{}", local.said);
}

#[test]
fn under_a_never_policy_the_state_says_why_no_level_goes_lower() {
    let project = tempfile::tempdir().unwrap();
    std::fs::write(
        project.path().join("charter.toml"),
        "schema = 1\n[sandbox]\nmode = \"on\"\npolicy = \"never\"\n",
    )
    .unwrap();
    let state = state_of(project.path());
    let held = state.level_held.expect("held");
    assert!(held.contains("policy = \"never\""), "{held}");
    assert_eq!(state.team_off, None);
}

/// #2071: the Sandbox page says your own level, as a chat you start here would take it, and
/// whether you confirmed it; a level the file alone says, lower than the project's, is not one.
#[test]
fn settings_says_your_level_as_it_starts_your_chats_here() {
    use purlis_core::sandbox::personal::{self, Choice};
    let project = tempfile::tempdir().expect("a project");
    std::fs::write(
        project.path().join("charter.toml"),
        "[sandbox]\nmode = \"on\"\n",
    )
    .unwrap();
    let local = project.path().join(purlis_core::profiles::LOCAL_FILE);
    assert_eq!(state_on(project.path(), sandbox::Os::MacOs).yours, None);
    std::fs::write(&local, "[sandbox]\nlevel = \"off\"\n").unwrap();
    assert_eq!(state_on(project.path(), sandbox::Os::MacOs).yours, None);
    personal::set_by_you(project.path(), None, Some(Choice::Off)).unwrap();
    assert_eq!(
        state_on(project.path(), sandbox::Os::MacOs).yours,
        Some(YourLevel {
            level: "off".to_owned(),
            confirmed: true
        })
    );
}

/// #2071: the page says your level only as a start takes it: a floor a policy that forbids the
/// opt-out keeps at the full sandbox is not said as yours.
#[test]
fn settings_says_no_floor_of_yours_that_policy_keeps_at_the_full_sandbox() {
    use purlis_core::sandbox::personal::{self, Choice};
    use purlis_core::sandbox::policy::{Locks, set_for_this_test};
    let project = tempfile::tempdir().expect("a project");
    std::fs::write(
        project.path().join("charter.toml"),
        "[sandbox]\nmode = \"on\"\n",
    )
    .unwrap();
    std::fs::write(
        project.path().join(purlis_core::profiles::LOCAL_FILE),
        "[sandbox]\nlevel = \"floor\"\n",
    )
    .unwrap();
    personal::set_by_you(project.path(), None, Some(Choice::Floor)).unwrap();
    assert_eq!(
        state_on(project.path(), sandbox::Os::MacOs)
            .yours
            .map(|it| it.level),
        Some("floor".to_owned())
    );
    set_for_this_test(Locks::parse(
        r#"{"owner": "Platform team", "sandbox": {"opt-out": false}}"#,
        std::path::Path::new("policy.json"),
    ));
    let yours = state_on(project.path(), sandbox::Os::MacOs).yours;
    set_for_this_test(Locks::none());
    assert_eq!(yours, None);
}

/// #1970: a port added by hand in Settings › Sandbox is judged by the one function a block's
/// Allow is judged by, so every value and scope the Notice refuses, the form refuses in the
/// same words, and nothing is kept or audited for either.
#[test]
fn a_port_added_in_settings_is_refused_exactly_where_the_notice_s_allow_is() {
    use purlis_core::sandbox::policy::{Locks, set_for_this_test};
    let project = tempfile::tempdir().expect("a project");
    let root = project.path();
    let chats = crate::chats::Chats::new();
    let seen = std::sync::Mutex::new(Vec::<String>::new());
    let audit = |_: Option<u32>, audited: &sandbox::grant::Audited<'_>| {
        seen.lock().unwrap().push(audited.target.to_owned());
        Ok(())
    };
    let notice = |target: &str, level| {
        allow(
            root,
            &machine(),
            &chats,
            3,
            (GrantWhat::Port, target, level),
            &audit,
            100,
        )
        .expect_err("the Notice refuses it")
        .to_string()
    };
    let form = |target: &str, chat, level| {
        allow_port(root, &machine(), &chats, chat, (target, level), &audit, 100)
            .expect_err("the form refuses it")
            .to_string()
    };
    let own = purlis_core::forge::registration::REDIRECT_PORTS[0].to_string();
    let never = [
        "",
        " ",
        "0",
        "65536",
        "-1",
        "1-65535",
        "50795-50800",
        "50795,50796",
        "50795 50796",
        "*",
        "any",
        "localhost",
        "localhost:50795",
        "127.0.0.1:50795",
        "0x1F",
        own.as_str(),
    ];
    // For this project on this machine no chat is read, so a chat named or not is the same
    // Allow. For one open chat the same comparison runs where a chat can be started
    // (`chats::tests::a_port_added_in_settings_for_one_chat_is_held_for_it_alone`).
    for target in never {
        let said = notice(target, GrantLevel::You);
        assert_eq!(form(target, None, GrantLevel::You), said, "{target:?}");
        assert_eq!(form(target, Some(3), GrantLevel::You), said, "{target:?}");
    }
    assert_eq!(
        form(&own, None, GrantLevel::You),
        purlis_core::sandbox::localport::OWN_SAID
    );
    // Never once, every project on this machine, or the committed file.
    for level in [GrantLevel::Once, GrantLevel::Machine, GrantLevel::Project] {
        let said = notice("50795", level);
        assert!(said.contains("for this chat, or for every chat"), "{said}");
        assert_eq!(form("50795", Some(3), level), said, "{level:?}");
        assert_eq!(form("50795", None, level), said, "{level:?}");
    }
    // Past a policy that forbids it, at either scope.
    set_for_this_test(Locks::parse(
        r#"{"owner": "IT", "sandbox": {"socket-grants": false}}"#,
        std::path::Path::new("/etc/purlis/policy.json"),
    ));
    let said = notice("50795", GrantLevel::You);
    assert!(said.contains("Policy forbids"), "{said}");
    assert_eq!(form("50795", None, GrantLevel::You), said);
    set_for_this_test(Locks::none());
    // For this chat names a chat open in this project, which the core also checks before
    // anything is judged or audited: never no chat, nor one that is not open here.
    assert_eq!(
        form("50795", None, GrantLevel::Chat),
        "purlis did not allow port 50795: no open chat was named."
    );
    assert_eq!(
        form("50795", Some(3), GrantLevel::Chat),
        "purlis did not allow port 50795: chat 3 is not a chat open in this project."
    );
    assert!(
        seen.lock().unwrap().is_empty(),
        "nothing refused is audited"
    );
    assert!(sandbox::local::granted_ports(root).is_empty());
    assert!(!chats.holds_for(3, &sandbox::grant::What::Port(50795)));
}

/// #1970: a port added in Settings for this project on this machine is audited, kept where a
/// Notice's Allow keeps it, and listed on the Granted list; added again, it is neither kept
/// nor audited twice.
#[test]
fn a_port_added_in_settings_is_audited_kept_and_listed_as_allowed() {
    let project = tempfile::tempdir().expect("a project");
    let root = project.path();
    let chats = crate::chats::Chats::new();
    let seen = std::sync::Mutex::new(Vec::new());
    let audit = |chat: Option<u32>, audited: &sandbox::grant::Audited<'_>| {
        seen.lock().unwrap().push((
            chat,
            audited.granted,
            audited.what.to_owned(),
            audited.target.to_owned(),
            audited.level.word(),
        ));
        Ok(())
    };
    let added = allow_port(
        root,
        &machine(),
        &chats,
        Some(3),
        (" 50795 ", GrantLevel::You),
        &audit,
        100,
    )
    .expect("added");
    assert!(!added.live);
    assert_eq!(
        added.said,
        "Allowed for me on this machine. A chat that is running reaches it from its next start."
    );
    assert_eq!(sandbox::local::granted_ports(root), [50795]);
    let listed: Vec<_> = grants_of(root, &chats, &crate::chatbrowsers::ChatBrowsers::default())
        .into_iter()
        .filter(|one| one.what == GrantWhat::Port)
        .map(|one| (one.target, one.level, one.at))
        .collect();
    assert_eq!(listed, [("50795".to_owned(), GrantLevel::You, Some(100))]);

    let again = allow_port(
        root,
        &machine(),
        &chats,
        None,
        ("50795", GrantLevel::You),
        &audit,
        101,
    )
    .expect("allowed already");
    assert_eq!(
        again.said,
        "It was allowed already, for me on this machine."
    );
    assert_eq!(sandbox::local::granted_ports(root), [50795]);
    assert_eq!(
        *seen.lock().unwrap(),
        [(None, true, "port".to_owned(), "50795".to_owned(), "you")]
    );
}

// ---- an Allow for one chat is audited only once it is kept (#2454) ----

/// Chats on a host that runs nothing, and one chat open on it.
fn one_open_chat() -> (crate::chats::Chats, u32) {
    let chats = crate::chats::Chats::on_host(
        Box::new(|_| {}),
        Box::new(crate::host::pretend::Pretend::default()),
        crate::chats::tests::no_project(),
    );
    let session = chats
        .start(
            &purlis_core::reopen::Chat {
                program: "/nowhere/a-program-nothing-runs".to_owned(),
                name: "ide.7".to_owned(),
                ..Default::default()
            },
            SIZE,
        )
        .expect("the host opens it");
    (chats, session)
}

/// What an audit was asked to write: the chat, the target and the level.
type Seen = std::sync::Mutex<Vec<(Option<u32>, String, &'static str)>>;

fn recording(
    seen: &Seen,
) -> impl Fn(Option<u32>, &sandbox::grant::Audited<'_>) -> Result<(), String> {
    |chat, audited| {
        seen.lock()
            .unwrap()
            .push((chat, audited.target.to_owned(), audited.level.word()));
        Ok(())
    }
}

/// The level a chat's grant of `what` `target` is listed at, if one holds it.
fn held_at(
    root: &std::path::Path,
    chats: &crate::chats::Chats,
    (what, target): (GrantWhat, &str),
) -> Option<GrantLevel> {
    grants_of(root, chats, &crate::chatbrowsers::ChatBrowsers::default())
        .into_iter()
        .find(|one| one.what == what && one.target == target && one.chat.is_some())
        .map(|one| one.level)
}

const HOST: (GrantWhat, &str) = (GrantWhat::Host, "api.example.com");
const SERVICE: (GrantWhat, &str) = (GrantWhat::Service, "com.example.helper");

/// #2454: an Allow for one chat that is not open is refused before anything is audited, where
/// the audit used to record an Allow that `hold_grant` then refused to keep.
#[test]
fn an_allow_for_a_chat_that_is_not_open_is_neither_audited_nor_kept() {
    let project = tempfile::tempdir().expect("a project");
    let chats = crate::chats::Chats::new();
    let seen = Seen::default();
    for ((what, target), level) in [
        (HOST, GrantLevel::Chat),
        (SERVICE, GrantLevel::Chat),
        (SERVICE, GrantLevel::Once),
    ] {
        let refused = judged_and_kept(
            project.path(),
            &machine(),
            &chats,
            Some(3),
            (what, target, level),
            &recording(&seen),
            100,
        )
        .expect_err("chat 3 is not open")
        .to_string();
        assert!(
            refused.contains("not open"),
            "{target} {level:?}: {refused}"
        );
    }
    assert!(
        seen.lock().unwrap().is_empty(),
        "nothing refused is audited"
    );
    assert!(chats.chat_grants().is_empty());
}

/// #2454: an Allow for one open chat whose audit cannot be written keeps nothing, and a grant
/// once that the audit could not make lasting stays once. (The first loop held on the old order
/// too, which audited before it held; it fails where a grant is kept after a failed audit.)
#[test]
fn an_allow_for_one_chat_that_cannot_be_audited_is_not_kept() {
    let project = tempfile::tempdir().expect("a project");
    let root = project.path();
    let (chats, session) = one_open_chat();
    let allow = |(what, target): (GrantWhat, &str), level, audit: Audit<'_>| {
        judged_and_kept(
            root,
            &machine(),
            &chats,
            Some(session),
            (what, target, level),
            audit,
            100,
        )
    };
    for (asked, level) in [
        (HOST, GrantLevel::Chat),
        (SERVICE, GrantLevel::Chat),
        (SERVICE, GrantLevel::Once),
    ] {
        let refused = allow(asked, level, &no_audit())
            .expect_err("unaudited")
            .to_string();
        assert!(
            refused.contains("event log"),
            "{asked:?} {level:?}: {refused}"
        );
        assert_eq!(held_at(root, &chats, asked), None, "{asked:?} {level:?}");
    }
    assert!(chats.chat_grants().is_empty());
    let seen = Seen::default();
    allow(SERVICE, GrantLevel::Once, &recording(&seen)).expect("allowed once");
    allow(SERVICE, GrantLevel::Chat, &no_audit()).expect_err("unaudited");
    assert_eq!(
        held_at(root, &chats, SERVICE),
        Some(GrantLevel::Once),
        "an unaudited Allow for the chat leaves the grant once"
    );
}

/// #2454: an Allow for one open chat is audited once and kept; allowed again it is neither kept
/// nor audited twice, and a grant once made lasting for the chat is audited as such.
#[test]
fn an_allow_for_one_open_chat_is_audited_once_and_kept() {
    let project = tempfile::tempdir().expect("a project");
    let root = project.path();
    let (chats, session) = one_open_chat();
    let seen = Seen::default();
    let allow = |(what, target): (GrantWhat, &str), level| {
        judged_and_kept(
            root,
            &machine(),
            &chats,
            Some(session),
            (what, target, level),
            &recording(&seen),
            100,
        )
        .expect("allowed")
        .2
    };
    assert!(!allow(HOST, GrantLevel::Chat).already);
    assert!(allow(HOST, GrantLevel::Chat).already);
    assert_eq!(held_at(root, &chats, HOST), Some(GrantLevel::Chat));
    assert!(!allow(SERVICE, GrantLevel::Once).already);
    assert!(allow(SERVICE, GrantLevel::Once).already);
    assert!(
        !allow(SERVICE, GrantLevel::Chat).already,
        "once, made lasting"
    );
    assert!(
        allow(SERVICE, GrantLevel::Once).already,
        "lasting covers once"
    );
    assert_eq!(held_at(root, &chats, SERVICE), Some(GrantLevel::Chat));
    assert_eq!(
        *seen.lock().unwrap(),
        [
            (Some(session), "api.example.com".to_owned(), "chat"),
            (Some(session), "com.example.helper".to_owned(), "once"),
            (Some(session), "com.example.helper".to_owned(), "chat"),
        ]
    );
}

/// #2454: a grant for one chat is seen by nothing until its audit is written — not a start's
/// sandbox, not the Granted list, and not a second Allow of the same, which is no "allowed
/// already" and writes its own audit; kept, the two are one grant.
#[test]
fn a_grant_for_one_chat_is_seen_by_nothing_until_its_audit_is_written() {
    let project = tempfile::tempdir().expect("a project");
    let root = project.path();
    let (chats, session) = one_open_chat();
    let seen = Seen::default();
    let inner = recording(&seen);
    let second = std::cell::RefCell::new(None);
    let outer = |chat: Option<u32>, audited: &sandbox::grant::Audited<'_>| {
        assert!(
            chats.grants_a_start_of(session).is_empty(),
            "a start has it"
        );
        assert_eq!(held_at(root, &chats, HOST), None, "the Granted list has it");
        if second.borrow().is_none() {
            let again = judged_and_kept(
                root,
                &machine(),
                &chats,
                Some(session),
                (HOST.0, HOST.1, GrantLevel::Chat),
                &inner,
                101,
            );
            *second.borrow_mut() = Some(again.map(|(_, _, kept)| kept.already));
        }
        recording(&seen)(chat, audited)
    };
    let kept = judged_and_kept(
        root,
        &machine(),
        &chats,
        Some(session),
        (HOST.0, HOST.1, GrantLevel::Chat),
        &outer,
        100,
    )
    .expect("allowed");
    assert!(!kept.2.already);
    assert_eq!(
        second.into_inner().expect("a second Allow ran"),
        Ok(false),
        "not allowed already while the first was not kept"
    );
    assert_eq!(
        seen.lock().unwrap().len(),
        2,
        "each Allow wrote its own audit"
    );
    assert_eq!(chats.chat_grants().len(), 1, "one grant kept");
    assert_eq!(
        chats.grants_a_start_of(session).hosts.len(),
        1,
        "a start has it once audited"
    );
}

/// #2454: an Allow for one chat whose audit fails while the chat restarts leaves the grant in
/// neither run's sandbox.
#[test]
fn an_audit_that_fails_while_the_chat_restarts_leaves_no_start_the_grant() {
    let project = tempfile::tempdir().expect("a project");
    let root = project.path();
    let (chats, session) = one_open_chat();
    let restarted = std::cell::Cell::new(None);
    let failing = |_: Option<u32>, _: &sandbox::grant::Audited<'_>| {
        let again = chats.start_fresh(session, SIZE).expect("it restarts");
        assert!(
            chats.grants_a_start_of(again).is_empty(),
            "the restart has it"
        );
        restarted.set(Some(again));
        Err("the event log is not open".to_owned())
    };
    judged_and_kept(
        root,
        &machine(),
        &chats,
        Some(session),
        (SERVICE.0, SERVICE.1, GrantLevel::Chat),
        &failing,
        100,
    )
    .expect_err("unaudited");
    let again = restarted.get().expect("restarted during the audit");
    assert!(chats.grants_a_start_of(session).is_empty());
    assert!(chats.grants_a_start_of(again).is_empty());
    assert!(chats.chat_grants().is_empty());
}

/// #2454: an Allow whose audit fails lets go of nothing another Allow kept while it was being
/// written — a grant revoked and allowed again stays, and one made lasting stays lasting.
#[test]
fn an_audit_that_fails_takes_nothing_another_allow_kept_meanwhile() {
    let project = tempfile::tempdir().expect("a project");
    let root = project.path();
    let (chats, session) = one_open_chat();
    let seen = Seen::default();
    let allow = |asked: (GrantWhat, &str), level, audit: Audit<'_>| {
        judged_and_kept(
            root,
            &machine(),
            &chats,
            Some(session),
            (asked.0, asked.1, level),
            audit,
            100,
        )
    };
    allow(HOST, GrantLevel::Chat, &recording(&seen)).expect("allowed");
    let (id, kept) = chats.chat_grants().remove(0);
    let host = kept.what;
    assert!(chats.revoke(&id, &host));
    // While an Allow of it again fails its audit, another Allow keeps it (audited), it is
    // revoked, and allowed again.
    let failing = |_: Option<u32>, _: &sandbox::grant::Audited<'_>| {
        allow(HOST, GrantLevel::Chat, &recording(&seen)).expect("allowed meanwhile");
        assert!(chats.revoke(&id, &host));
        allow(HOST, GrantLevel::Chat, &recording(&seen)).expect("allowed again");
        Err("the event log is not open".to_owned())
    };
    allow(HOST, GrantLevel::Chat, &failing).expect_err("unaudited");
    assert_eq!(held_at(root, &chats, HOST), Some(GrantLevel::Chat));
    // Once, then made lasting (audited) while an Allow of it for the chat fails its audit.
    allow(SERVICE, GrantLevel::Once, &recording(&seen)).expect("once");
    let failing = |_: Option<u32>, _: &sandbox::grant::Audited<'_>| {
        allow(SERVICE, GrantLevel::Chat, &recording(&seen)).expect("made lasting");
        Err("the event log is not open".to_owned())
    };
    allow(SERVICE, GrantLevel::Chat, &failing).expect_err("unaudited");
    assert_eq!(held_at(root, &chats, SERVICE), Some(GrantLevel::Chat));
}

/// #2454: a chat that closes while its Allow's audit is written keeps nothing, as its grants
/// end with it, and the audit records that the grant it wrote ended.
#[test]
fn a_chat_that_closes_while_its_allow_is_audited_keeps_nothing_and_the_audit_says_so() {
    let project = tempfile::tempdir().expect("a project");
    let root = project.path();
    let (chats, session) = one_open_chat();
    let written = std::sync::Mutex::new(Vec::new());
    let closing = |_: Option<u32>, audited: &sandbox::grant::Audited<'_>| {
        if audited.granted {
            chats.close(session).expect("it closes");
        }
        written.lock().unwrap().push(audited.granted);
        Ok(())
    };
    let refused = judged_and_kept(
        root,
        &machine(),
        &chats,
        Some(session),
        (HOST.0, HOST.1, GrantLevel::Chat),
        &closing,
        100,
    )
    .expect_err("closed before it was kept")
    .to_string();
    assert!(refused.contains("closed"), "{refused}");
    assert_eq!(*written.lock().unwrap(), [true, false]);
    assert!(chats.chat_grants().is_empty());
    assert!(
        chats.owed_restarts().is_empty(),
        "a refusal owes no restart"
    );
}

const SIZE: purlis_core::engine::Size = purlis_core::engine::Size {
    columns: 80,
    rows: 24,
};

/// #2454: a Revoke made while an Allow's audit is written stands. Two Allows of the same are
/// reserved; the first is kept, the chat starts again to take it, the person revokes it, and
/// the second is refused: nothing is kept, no restart is owed, and the audit records it ended.
#[test]
fn a_revoke_made_while_an_allow_is_audited_stands() {
    let project = tempfile::tempdir().expect("a project");
    let root = project.path();
    let (chats, session) = one_open_chat();
    let written = std::sync::Mutex::new(Vec::new());
    let restarted = std::cell::Cell::new(None);
    let first = |_: Option<u32>, audited: &sandbox::grant::Audited<'_>| {
        written.lock().unwrap().push(("first", audited.granted));
        Ok(())
    };
    let second = |_: Option<u32>, audited: &sandbox::grant::Audited<'_>| {
        written.lock().unwrap().push(("second", audited.granted));
        if audited.granted {
            judged_and_kept(
                root,
                &machine(),
                &chats,
                Some(session),
                (HOST.0, HOST.1, GrantLevel::Chat),
                &first,
                101,
            )
            .expect("the first is kept");
            // The chat starts again to take it, and its old run ends.
            let again = chats.start_fresh(session, SIZE).expect("it starts again");
            chats.close(session).expect("the old run ends");
            restarted.set(Some(again));
            let (id, kept) = chats.chat_grants().remove(0);
            assert!(chats.revoke(&id, &kept.what), "the person revokes it");
        }
        Ok(())
    };
    let refused = judged_and_kept(
        root,
        &machine(),
        &chats,
        Some(session),
        (HOST.0, HOST.1, GrantLevel::Chat),
        &second,
        100,
    )
    .expect_err("revoked meanwhile")
    .to_string();
    assert!(refused.contains("taken back"), "{refused}");
    let again = restarted.get().expect("restarted");
    assert!(chats.chat_grants().is_empty(), "the Revoke stands");
    assert!(chats.grants_a_start_of(again).is_empty());
    assert!(
        chats.owed_restarts().is_empty(),
        "a refusal owes no restart"
    );
    assert_eq!(
        *written.lock().unwrap(),
        [("second", true), ("first", true), ("second", false)]
    );
}

/// #2454: an Allow for the chat is kept for the run a restart on its conversation began while
/// its audit was written; one allowed once was for the run it was allowed in alone.
#[test]
fn an_allow_for_the_chat_is_kept_by_the_run_a_restart_began_while_it_was_audited() {
    let project = tempfile::tempdir().expect("a project");
    let root = project.path();
    let (chats, session) = one_open_chat();
    let ran = std::cell::Cell::new(session);
    let restarting = |_: Option<u32>, audited: &sandbox::grant::Audited<'_>| {
        if audited.granted {
            let again = chats.start_fresh(ran.get(), SIZE).expect("it restarts");
            chats.close(ran.get()).expect("the old run ends");
            ran.set(again);
        }
        Ok(())
    };
    let allow = |asked: (GrantWhat, &str), level| {
        judged_and_kept(
            root,
            &machine(),
            &chats,
            Some(ran.get()),
            (asked.0, asked.1, level),
            &restarting,
            100,
        )
    };
    allow(HOST, GrantLevel::Chat).expect("kept for the new run");
    assert_eq!(chats.grants_a_start_of(ran.get()).hosts.len(), 1);
    let refused = allow(SERVICE, GrantLevel::Once)
        .expect_err("once was for the run that ended")
        .to_string();
    assert!(refused.contains("closed"), "{refused}");
    assert!(chats.grants_a_start_of(ran.get()).services.is_empty());
}

/// #2510, ruled 2026-10-18: a service of the runner a chat runs on is allowed once or for that
/// chat alone, and only where its runner asks about it; nothing is audited or kept on this
/// machine for one refused, so the chat's next start plans it as before.
#[test]
fn a_runners_service_is_never_allowed_past_its_chat_nor_kept_on_this_machine() {
    let project = tempfile::tempdir().expect("a project");
    let chats = crate::chats::Chats::new();
    let audited = std::cell::Cell::new(0);
    let audit = |_: Option<u32>, _: &sandbox::grant::Audited<'_>| {
        audited.set(audited.get() + 1);
        Ok(())
    };
    for level in [GrantLevel::You, GrantLevel::Project, GrantLevel::Machine] {
        assert_eq!(
            allow_on_its_runner(&chats, 3, ("5432", level), &audit),
            Err(RUNNER_SERVICE_SCOPES.to_owned()),
            "{level:?}"
        );
    }
    let not_asked = allow_on_its_runner(&chats, 3, ("5432", GrantLevel::Chat), &audit)
        .expect_err("its runner does not ask about it");
    assert!(not_asked.contains("5432"), "{not_asked}");
    assert_eq!(audited.get(), 0, "nothing audited");
    assert!(sandbox::local::granted_ports(project.path()).is_empty());
}

/// #2510: an Allow the runner did not take is audited as taken back and refused in words, so
/// the audit never holds an Allow that did not reach the runner.
#[test]
fn a_runners_service_its_runner_did_not_take_is_audited_back_and_refused() {
    let audited = std::cell::RefCell::new(Vec::new());
    let audit = |_: Option<u32>, one: &sandbox::grant::Audited<'_>| {
        audited
            .borrow_mut()
            .push((one.granted, one.target.to_owned()));
        Ok(())
    };
    let refused = allowed_on_its_runner(4, ("5432", GrantLevel::Chat), &audit, &|_| true, &|_| {
        Err("the runner refused it".to_owned())
    })
    .expect_err("refused");
    assert!(refused.contains("the runner refused it"), "{refused}");
    assert_eq!(
        *audited.borrow(),
        [(true, "5432".to_owned()), (false, "5432".to_owned())]
    );

    audited.borrow_mut().clear();
    let allowed = allowed_on_its_runner(4, ("5432", GrantLevel::Once), &audit, &|_| true, &|_| {
        Ok(crate::chats::Live {
            reached: vec![4],
            told_to_retry: Vec::new(),
        })
    })
    .expect("allowed");
    assert!(allowed.live);
    assert_eq!(*audited.borrow(), [(true, "5432".to_owned())]);
}

/// #2510: a block on a runner's service, answered with Allow for you in a project, grants
/// nothing in that project on this machine; the same Allow of a service on this Mac does, so
/// the project's granted ports are where such a grant would land.
#[test]
fn allow_for_you_on_a_runners_service_grants_nothing_in_the_project_here() {
    use crate::taskblocks::HeldBlock;
    let project = tempfile::tempdir().expect("a project");
    let chats = crate::chats::Chats::new();
    let audit = |_: Option<u32>, _: &sandbox::grant::Audited<'_>| Ok(());
    let block = |kind: &str| HeldBlock {
        operation: "connect".to_owned(),
        kind: kind.to_owned(),
        what: GrantWhat::Port,
        target: "5432".to_owned(),
    };

    let refused = allow_held(
        project.path(),
        &machine(),
        &chats,
        (3, &block("runner-service")),
        GrantLevel::You,
        &audit,
        100,
    )
    .expect_err("refused");
    assert_eq!(refused.to_string(), RUNNER_SERVICE_SCOPES);
    assert!(sandbox::local::granted_ports(project.path()).is_empty());

    allow_held(
        project.path(),
        &machine(),
        &chats,
        (3, &block("local-service")),
        GrantLevel::You,
        &audit,
        100,
    )
    .expect("a service on this Mac is allowed for you");
    assert_eq!(sandbox::local::granted_ports(project.path()), [5432]);
}

/// #2653: **a host allowed for me is removed from my hosts**: Remove on its row takes it out of
/// the personal hosts file, audited at "for me". A chat is named only for a one-chat grant,
/// so a for-me revoke that names a chat, or a one-chat revoke that names none, removes nothing
/// and audits nothing.
#[test]
fn a_host_allowed_for_me_is_removed_from_my_hosts_and_a_chat_named_with_it_removes_nothing() {
    use sandbox::grant::Level;
    let project = tempfile::tempdir().expect("a project");
    let root = project.path();
    let chats = crate::chats::Chats::new();
    let host = sandbox::hosts::Host::parse("api.example.com").expect("a host");
    purlis_core::settings::hosts::grant(root, hosts_file(Level::You), &host).expect("granted");
    let id = grants_of(root, &chats, &crate::chatbrowsers::ChatBrowsers::default())
        .into_iter()
        .find(|one| one.what == GrantWhat::Host && one.level == GrantLevel::You)
        .expect("listed")
        .id;
    assert_eq!(id, format!("you{SEP}host{SEP}api.example.com"));

    let heard = std::sync::Mutex::new(Vec::new());
    let audit = |number: Option<u32>, audited: &sandbox::grant::Audited<'_>| {
        heard
            .lock()
            .unwrap()
            .push((number, audited.granted, audited.level.word()));
        Ok(())
    };
    for named in [
        format!("you{SEP}chat-1{SEP}host{SEP}api.example.com"),
        format!("machine{SEP}chat-1{SEP}host{SEP}api.example.com"),
        format!("project{SEP}chat-1{SEP}host{SEP}api.example.com"),
        format!("chat{SEP}host{SEP}api.example.com"),
        format!("once{SEP}host{SEP}api.example.com"),
    ] {
        let refused = revoke(root, &chats, &named, &audit).expect_err("refused");
        assert!(refused.contains("no longer there"), "{named:?}: {refused}");
    }
    assert_eq!(sandbox::hosts::personal(root), std::slice::from_ref(&host));
    assert!(heard.lock().unwrap().is_empty(), "nothing audited");

    revoke(root, &chats, &id, &audit).expect("removed");
    assert!(sandbox::hosts::personal(root).is_empty());
    assert_eq!(*heard.lock().unwrap(), [(None, false, "you")]);
}

/// Every level an Allow may name, as the window names it.
const EVERY_LEVEL: [GrantLevel; 5] = [
    GrantLevel::Once,
    GrantLevel::Chat,
    GrantLevel::You,
    GrantLevel::Machine,
    GrantLevel::Project,
];

/// Every socket and service of the system's name lookup (#2657), as a block or a file names it.
fn name_lookups() -> Vec<(GrantWhat, &'static str)> {
    sandbox::services::NAME_LOOKUP_SOCKETS
        .iter()
        .map(|path| (GrantWhat::Socket, *path))
        .chain(
            sandbox::services::NAME_LOOKUP_SERVICES
                .iter()
                .map(|name| (GrantWhat::Service, *name)),
        )
        .collect()
}

/// How many times an audit was told.
type Count = std::sync::Arc<std::sync::Mutex<usize>>;

/// An audit that counts what it was told, and the count.
fn counted() -> (
    Count,
    impl Fn(Option<u32>, &sandbox::grant::Audited<'_>) -> Result<(), String>,
) {
    let count = std::sync::Arc::new(std::sync::Mutex::new(0));
    let audit = {
        let count = std::sync::Arc::clone(&count);
        move |_: Option<u32>, _: &sandbox::grant::Audited<'_>| {
            *count.lock().unwrap() += 1;
            Ok(())
        }
    };
    (count, audit)
}

/// #2657: **a grant never names the system's name lookup**, from a block's Notice, at any
/// level: each of its sockets and services is refused before anything is audited or kept, in
/// purlis's own words (a grant for everyone in the project is refused for every socket and
/// service, as it always is).
#[test]
fn a_notice_never_allows_the_systems_name_lookup_at_any_level() {
    let project = tempfile::tempdir().expect("a project");
    let root = project.path();
    let chats = crate::chats::Chats::new();
    let (audited, audit) = counted();
    for (what, target) in name_lookups() {
        for level in EVERY_LEVEL {
            let refused = allow(
                root,
                &machine(),
                &chats,
                3,
                (what, target, level),
                &audit,
                100,
            )
            .expect_err("refused")
            .to_string();
            if level == GrantLevel::Project {
                assert!(refused.contains("everyone in the project"), "{refused}");
            } else {
                assert_eq!(
                    refused,
                    sandbox::services::NAME_LOOKUP_REFUSED,
                    "{target} {level:?}"
                );
            }
        }
    }
    assert_eq!(*audited.lock().unwrap(), 0, "nothing audited");
    assert!(sandbox::local::granted_sockets(root).is_empty());
    assert!(sandbox::local::granted_services(root).is_empty());
}

/// #2657: what a project expects is never allowed for the system's name lookup from Settings
/// or its Notice: each of its sockets is drawn as one that cannot be allowed, saying why, and
/// its service is not taken as declared; an Allow keeps nothing however the window drew it,
/// and the one keep refuses each at every level it keeps at.
#[test]
fn settings_never_allow_the_systems_name_lookup_a_project_expects() {
    let project = tempfile::tempdir().expect("a project");
    let root = project.path();
    let lookups = name_lookups();
    let expects: Vec<String> = lookups
        .iter()
        .map(|(_, target)| format!("\"{target}\""))
        .collect();
    std::fs::write(
        root.join(purlis_core::plane::MANIFEST),
        format!(
            "[sandbox]\nmode = \"on\"\nexpects = [{}]\n",
            expects.join(", ")
        ),
    )
    .expect("charter.toml");
    let awaiting = sandbox::services::awaiting(root, &sandbox::Machine::this()).expect("it waits");
    // A service no grant names is never taken as declared, so it is never asked about.
    assert_eq!(
        awaiting
            .items
            .iter()
            .map(|item| item.written.as_str())
            .collect::<Vec<_>>(),
        sandbox::services::NAME_LOOKUP_SOCKETS
    );
    for item in &awaiting.items {
        assert_eq!(
            item.refused.as_deref(),
            Some(sandbox::services::NAME_LOOKUP_REFUSED),
            "{}",
            item.written
        );
    }
    let chats = crate::chats::Chats::new();
    let (audited, audit) = counted();
    for (_, target) in &lookups {
        for named in [None, Some((*target).to_owned())] {
            let shown = [ExpectedShown {
                target: (*target).to_owned(),
                named,
            }];
            assert!(
                allow_expects(root, &chats, &shown, &audit, 100).is_err(),
                "{target}"
            );
        }
    }
    let every: Vec<ExpectedShown> = lookups
        .iter()
        .map(|(_, target)| ExpectedShown {
            target: (*target).to_owned(),
            named: Some((*target).to_owned()),
        })
        .collect();
    assert!(allow_expects(root, &chats, &every, &audit, 100).is_err());
    assert!(sandbox::local::granted_sockets(root).is_empty());
    assert!(sandbox::local::granted_services(root).is_empty());
    for (what, target) in &lookups {
        let what = match what {
            GrantWhat::Socket => sandbox::grant::What::Socket((*target).into()),
            _ => sandbox::grant::What::Service((*target).to_owned()),
        };
        for level in [
            sandbox::grant::Level::Once,
            sandbox::grant::Level::Chat,
            sandbox::grant::Level::You,
            sandbox::grant::Level::Machine,
        ] {
            let refused =
                kept_for(root, &chats, Some(3), (&what, level), &audit, 100).expect_err("refused");
            assert_eq!(
                refused,
                sandbox::services::NAME_LOOKUP_REFUSED,
                "{target} {level:?}"
            );
        }
    }
    assert_eq!(*audited.lock().unwrap(), 0, "nothing audited");
}

/// #2657: the Granted list says each socket and service of the system's name lookup kept on
/// this machine, for this project or for every project, reaches no chat, and why, and still
/// removes it.
#[test]
fn the_granted_list_says_a_kept_name_lookup_reaches_no_chat() {
    let everywhere = tempfile::tempdir().expect("a config home");
    sandbox::local::everywhere_for_this_test(Some(
        everywhere.path().join(sandbox::local::EVERY_PROJECT),
    ));
    let project = tempfile::tempdir().expect("a project");
    let root = project.path();
    let lookups = name_lookups();
    for (what, target) in &lookups {
        match what {
            GrantWhat::Socket => {
                sandbox::local::grant_socket(root, std::path::Path::new(target)).expect("written");
                sandbox::local::grant_socket_everywhere(std::path::Path::new(target), None)
                    .expect("written");
            }
            _ => {
                sandbox::local::grant_service(root, target).expect("written");
                sandbox::local::grant_service_everywhere(target, None).expect("written");
            }
        }
    }
    let chats = crate::chats::Chats::new();
    let rows: Vec<SandboxGrant> =
        grants_of(root, &chats, &crate::chatbrowsers::ChatBrowsers::default())
            .into_iter()
            .filter(|one| matches!(one.what, GrantWhat::Socket | GrantWhat::Service))
            .collect();
    for level in [GrantLevel::You, GrantLevel::Machine] {
        for (what, target) in &lookups {
            let row = rows
                .iter()
                .find(|one| one.what == *what && one.target == *target && one.level == level)
                .unwrap_or_else(|| panic!("{target} {level:?} listed"));
            assert_eq!(
                row.locked.as_deref(),
                Some(sandbox::services::NAME_LOOKUP_REFUSED),
                "{target} {level:?}"
            );
        }
    }
    for row in &rows {
        revoke(root, &chats, &row.id, &|_, _| Ok(())).expect("removed");
    }
    assert!(sandbox::local::granted_sockets(root).is_empty());
    assert!(sandbox::local::granted_services(root).is_empty());
    assert!(sandbox::local::sockets_everywhere().is_empty());
    assert!(sandbox::local::services_everywhere().is_empty());
    sandbox::local::everywhere_for_this_test(None);
}

/// #2657: every name of the Mac's lookup socket is refused: one kept under its name on the
/// data volume is listed as reaching no chat, and a Notice refuses it.
#[cfg(target_os = "macos")]
#[test]
fn every_name_of_the_macs_lookup_socket_is_refused_and_listed_as_reaching_no_chat() {
    let project = tempfile::tempdir().expect("a project");
    let root = project.path();
    let chats = crate::chats::Chats::new();
    for spelling in [
        "/System/Volumes/Data/private/var/run/mDNSResponder",
        "/private/var/run/mDNSResponder",
    ] {
        let refused = allow(
            root,
            &machine(),
            &chats,
            3,
            (GrantWhat::Socket, spelling, GrantLevel::You),
            &|_, _| Ok(()),
            100,
        )
        .expect_err("refused")
        .to_string();
        assert_eq!(
            refused,
            sandbox::services::NAME_LOOKUP_REFUSED,
            "{spelling}"
        );
        sandbox::local::grant_socket(root, std::path::Path::new(spelling)).expect("written");
    }
    let rows: Vec<SandboxGrant> =
        grants_of(root, &chats, &crate::chatbrowsers::ChatBrowsers::default())
            .into_iter()
            .filter(|one| one.what == GrantWhat::Socket)
            .collect();
    assert_eq!(rows.len(), 2);
    for row in rows {
        assert_eq!(
            row.locked.as_deref(),
            Some(sandbox::services::NAME_LOOKUP_REFUSED),
            "{}",
            row.target
        );
    }
}
