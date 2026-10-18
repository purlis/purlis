// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! A local socket or a system service as a grant names it (ADR 0093): what it may name, what
//! allowing it is said to mean, that a committed declaration grants nothing, and how a start
//! compiles what was granted.

use std::path::{Path, PathBuf};

use super::*;
use crate::sandbox::grant::{Grants, Refused, What};
use crate::sandbox::{
    Access, Class, Compiled, Denial, Machine, Os, Plane, claude, local, seatbelt,
};

// ---- what allowing one means (D-sock-6) ---------------------------------------------------

#[test]
fn each_known_kind_of_socket_says_what_allowing_it_means() {
    let table: [(&str, Known); 14] = [
        ("/var/run/docker.sock", Known::Containers),
        ("/Users/op/.docker/run/docker.sock", Known::Containers),
        ("/Users/op/.colima/default/docker.sock", Known::Containers),
        ("/Users/op/.orbstack/run/docker.sock", Known::Containers),
        ("/Users/op/.rd/docker.sock", Known::Containers),
        ("/run/user/501/podman/podman.sock", Known::Containers),
        (
            "/private/tmp/com.apple.launchd.AbC/Listeners",
            Known::SshAgent,
        ),
        ("/tmp/ssh-XXXX/agent.123", Known::SshAgent),
        ("/Users/op/.gnupg/S.gpg-agent.ssh", Known::SshAgent),
        ("/tmp/.s.PGSQL.5432", Known::Database),
        ("/tmp/mysql.sock", Known::Database),
        ("/opt/homebrew/var/run/redis.sock", Known::Database),
        ("/tmp/mongodb-27017.sock", Known::Database),
        ("/Users/op/Library/some-app/ipc.sock", Known::Unknown),
    ];
    for (path, kind) in table {
        assert_eq!(known_socket(Path::new(path), None), kind, "{path}");
    }
    // The person's own agent, wherever it lives, is an agent.
    assert_eq!(
        known_socket(
            Path::new("/somewhere/odd.sock"),
            Some(std::ffi::OsStr::new("/somewhere/odd.sock"))
        ),
        Known::SshAgent
    );
}

#[test]
fn the_risk_text_is_the_operators_words_per_kind() {
    assert_eq!(
        Known::Containers.risk(),
        "lets this chat control containers, which can reach your whole machine: about the same \
         as no sandbox"
    );
    assert_eq!(Known::SshAgent.risk(), "lets this chat use your SSH keys");
    assert_eq!(
        Known::Database.risk(),
        "lets this chat use that database as you"
    );
    assert_eq!(
        Known::CertificateCheck.risk(),
        "lets Go and similar tools check certificates; that service can make its own network \
         requests"
    );
    assert_eq!(
        Known::Unknown.risk(),
        "a service on this machine; purlis can't tell what it does"
    );
    assert_eq!(
        known(&What::Service(TRUSTD.to_owned())),
        Known::CertificateCheck
    );
    assert_eq!(
        known(&What::Service("com.example.helper".to_owned())),
        Known::Unknown
    );
}

#[test]
fn an_ask_says_what_allowing_it_means_and_whether_the_project_expects_it() {
    let docker = What::Socket(PathBuf::from("/var/run/docker.sock"));
    let none = Declared::default();
    assert_eq!(
        said_of(&docker, &none),
        format!("Allowing it {RISK_CONTAINERS}.")
    );
    let expects = Declared {
        sockets: vec![PathBuf::from("/var/run/docker.sock")],
        services: Vec::new(),
    };
    assert_eq!(
        said_of(&docker, &expects),
        format!("This project expects it. Allowing it {RISK_CONTAINERS}.")
    );
    assert_eq!(
        said_of(&What::Service("com.example.helper".to_owned()), &none),
        format!("It is {RISK_UNKNOWN}.")
    );
}

// ---- what a grant may name -----------------------------------------------------------------

#[test]
fn a_socket_is_named_whole_and_must_be_a_socket_now() {
    let dir = tempfile::tempdir().expect("a folder");
    let file = dir.path().join("not-a-socket");
    std::fs::write(&file, "x").expect("written");
    assert!(matches!(
        socket("docker.sock", &SocketGround::default()),
        Err(Refused::Not(_))
    ));
    assert!(matches!(
        socket("~/.docker/run/docker.sock", &SocketGround::default()),
        Err(Refused::Not(_))
    ));
    assert!(matches!(
        socket(&file.display().to_string(), &SocketGround::default()),
        Err(Refused::Not(_))
    ));
    assert!(matches!(
        socket(
            &dir.path().join("gone.sock").display().to_string(),
            &SocketGround::default()
        ),
        Err(Refused::Not(_))
    ));
}

#[test]
fn a_service_is_named_whole_and_never_a_keychains() {
    assert_eq!(service(TRUSTD), Ok(TRUSTD.to_owned()));
    assert_eq!(
        service(" com.example.helper-2_x "),
        Ok("com.example.helper-2_x".to_owned())
    );
    for not in [
        "",
        "trustd",
        "com.apple.*",
        ".com.apple",
        "com.apple.",
        "a b.c",
        "com/apple",
    ] {
        assert!(matches!(service(not), Err(Refused::Not(_))), "{not:?}");
    }
    for never in [
        "com.apple.SecurityServer",
        "com.apple.securityd.xpc",
        "com.apple.secd",
        "com.example.keychain-helper",
    ] {
        assert!(matches!(service(never), Err(Refused::Never(_))), "{never}");
    }
}

#[test]
fn a_kept_socket_swapped_for_a_link_reshaped_or_where_a_chat_writes_is_not_compiled() {
    let dir = tempfile::tempdir().expect("a folder");
    let at = dir.path().canonicalize().expect("real");
    for folder in ["real", "now-a-folder.sock", "project", "vaults"] {
        std::fs::create_dir_all(at.join(folder)).expect("made");
    }
    std::fs::write(at.join("real/now-a-file.sock"), "x").expect("written");
    std::os::unix::fs::symlink(at.join("real"), at.join("link")).expect("linked");
    let ground = SocketGround {
        denied: vec![Denial {
            class: Class::Vaults,
            path: at.join("vaults"),
            access: Access::ReadWrite,
            named: None,
        }],
        writable: vec![at.join("project")],
        // This fixture may itself sit in a harness's temp root: that rule has its own test.
        harness_temp: Vec::new(),
    };
    let kept = [
        at.join("real/a.sock"),
        at.join("link/b.sock"),
        at.join("vaults/c.sock"),
        at.join("project/d.sock"),
        at.join("now-a-folder.sock"),
        at.join("real/now-a-file.sock"),
        PathBuf::from("relative.sock"),
        at.join("real/a.sock"),
    ];
    // Not there now is kept: an engine that is not running has no socket until it starts, and
    // makes it anew each time.
    assert_eq!(still_sockets(&kept, &ground), vec![at.join("real/a.sock")]);
    assert_eq!(
        still_services(&[TRUSTD.to_owned(), "nope".to_owned(), TRUSTD.to_owned()]),
        vec![TRUSTD.to_owned()]
    );
}

/// MF-3 of #1808's review: no socket where a chat may write is ever offered, so a chat cannot
/// put a socket of its choosing, or a link to one it may not reach, under a harmless name.
#[test]
fn a_socket_where_a_chat_may_write_is_refused_with_why() {
    let ground = SocketGround {
        denied: Vec::new(),
        writable: vec![PathBuf::from("/Users/op/plane")],
        harness_temp: vec!["claude-".to_owned()],
    };
    for path in [
        "/Users/op/plane/workspaces/alpha/app.sock",
        "/private/tmp/claude-501/x/app.sock",
        "/private/var/folders/ab/cd/T/claude-501/app.sock",
    ] {
        assert!(
            matches!(ground.refuses(Path::new(path)), Some(Refused::Outside(why)) if why.contains("a chat may write")),
            "{path}"
        );
    }
    for path in [
        "/var/run/docker.sock",
        "/private/tmp/.s.PGSQL.5432",
        "/Users/op/.docker/run/docker.sock",
    ] {
        assert_eq!(ground.refuses(Path::new(path)), None, "{path}");
    }
    // The ground a start uses holds the project tree and this user's temp folder.
    let (_dir, root) = project();
    let of = SocketGround::of(&root, &machine_at(&root), &[]);
    assert!(of.refuses(&root.join("x.sock")).is_some());
    assert!(
        of.refuses(&crate::sandbox::real(&std::env::temp_dir()).join("x.sock"))
            .is_some()
    );
}

#[test]
fn lima_is_a_container_engine_only_as_a_folder_of_its_own() {
    assert_eq!(
        known_socket(Path::new("/Users/op/.lima/default/sock/docker.sock"), None),
        Known::Containers
    );
    assert_eq!(
        known_socket(Path::new("/Users/op/.lima/default/sock/x.sock"), None),
        Known::Containers
    );
    assert_eq!(
        known_socket(Path::new("/Users/op/climate/run.sock"), None),
        Known::Unknown
    );
}

// ---- a committed declaration grants nothing (D-sock-4) ------------------------------------

const DECLARES: &str = "[sandbox]\nmode = \"on\"\negress = [\"model-providers\"]\n\
                        expects = [\"/var/run/docker.sock\", \"com.apple.trustd.agent\", \
                        \"not a thing\"]\n";

#[test]
fn a_committed_declaration_is_read_as_what_the_project_expects() {
    let top: toml::Table = DECLARES.parse().expect("toml");
    assert_eq!(
        declared_in(Some(&top)),
        Declared {
            sockets: vec![PathBuf::from("/var/run/docker.sock")],
            // The certificate check is the project's own setting since R1, never a grant, so
            // a declaration of it is left out: there is nothing to ask a person for.
            services: vec![],
        }
    );
    let other: toml::Table = "[sandbox]\nexpects = [\"com.example.helper\"]\n"
        .parse()
        .expect("toml");
    assert_eq!(
        declared_in(Some(&other)).services,
        vec!["com.example.helper".to_owned()]
    );
    // And the key is one the file may hold: no refusal for it.
    assert!(Plane::of(Some(DECLARES)).said().refused.is_empty());
}

/// The operator's ruling R1 (2026-10-13, amending rulings 2 and 3 of ADR 0093 for this one
/// service): a sandboxed chat has the certificate check with nothing allowed by anyone, and the
/// committed `certificate-checks` only ever narrows. `true` says what an absent key says.
/// What a repository declares is drawn to a person and named in a question, so a path that
/// cannot be said plainly declares nothing: one that carries a character that draws as
/// nothing or moves the cursor, or one longer than any socket's path.
#[test]
fn a_declared_socket_that_cannot_be_said_plainly_declares_nothing() {
    let long = format!("/{}", "a".repeat(2000));
    let top: toml::Table = format!(
        "[sandbox]\nexpects = [\"/var/run/docker.sock\", \"/run/a\\nb.sock\", \
         \"/run/a\\u001b[2Jb.sock\", \"/run/a\\u202eb.sock\", \"{long}\"]\n"
    )
    .parse()
    .expect("toml");
    assert_eq!(
        declared_in(Some(&top)).sockets,
        [PathBuf::from("/var/run/docker.sock")]
    );
}

/// A declaration is drawn whole in one question, and each item is looked at on this machine
/// every time it is read, so it is bounded in number too: what is past the most a project
/// declares ([`MOST_DECLARED`]) declares nothing. A name written twice counts once.
#[test]
fn a_project_declares_no_more_than_a_question_can_show() {
    let many: Vec<String> = (0..500)
        .flat_map(|n| {
            [
                format!("\"/run/s{n}.sock\""),
                format!("\"/run/s{n}.sock\""),
                format!("\"com.example.s{n}\""),
            ]
        })
        .collect();
    let top: toml::Table = format!("[sandbox]\nexpects = [{}]\n", many.join(", "))
        .parse()
        .expect("toml");
    let declared = declared_in(Some(&top));
    assert_eq!(
        declared.sockets.len() + declared.services.len(),
        MOST_DECLARED
    );
    assert_eq!(declared.sockets[0], PathBuf::from("/run/s0.sock"));
    assert_eq!(declared.services[0], "com.example.s0");
    assert_eq!(declared.sockets[1], PathBuf::from("/run/s1.sock"));
}

#[test]
fn a_chat_has_the_certificate_check_by_default_and_the_committed_key_only_turns_it_off() {
    let (_dir, root) = project();
    let home = tempfile::tempdir().expect("a config home");
    local::TEST_CONFIG_HOME.with(|at| *at.borrow_mut() = Some(home.path().to_path_buf()));
    let sandbox = |root: &Path| {
        let compiled = compiled_at(root, &Grants::default());
        let claude =
            claude::settings(&compiled).expect("compiles").sandbox["enableWeakerNetworkIsolation"]
                .clone();
        let mut own = seatbelt::Own::default();
        seatbelt::widen(&mut own, &compiled.widened).expect("widened");
        (
            compiled.widened.trust,
            claude,
            own.after.contains(&seatbelt::TRUST.to_owned()),
        )
    };
    // The default project: on, with nothing declared and nothing allowed.
    assert_eq!(sandbox(&root), (true, serde_json::json!(true), true));
    assert!(declared(&root).services.is_empty());
    assert_eq!(
        crate::sandbox::CertificateChecks::at(&root),
        Some(crate::sandbox::CertificateChecks::On)
    );
    for (value, on) in [("true", true), ("false", false)] {
        std::fs::write(
            crate::names::manifest(&root),
            format!(
                "[sandbox]\nmode = \"on\"\negress = [\"model-providers\"]\n\
                 certificate-checks = {value}\n"
            ),
        )
        .expect("written");
        assert_eq!(
            sandbox(&root),
            (on, serde_json::json!(on), on),
            "certificate-checks = {value}"
        );
        // Declared by neither value: it is no grant.
        assert!(declared(&root).services.is_empty());
    }
    assert_eq!(
        crate::sandbox::CertificateChecks::at(&root),
        Some(crate::sandbox::CertificateChecks::OffByProject)
    );
    // **`false` only narrows, and nothing a person allowed widens past it**: neither a grant
    // an older build kept for this machine, nor one held for a chat.
    local::grant_service(&root, TRUSTD).expect("kept");
    let allowed = compiled_at(
        &root,
        &Grants {
            services: vec![TRUSTD.to_owned()],
            ..Grants::default()
        },
    );
    assert!(!allowed.widened.trust);
    assert!(allowed.widened.services.is_empty());
    local::TEST_CONFIG_HOME.with(|at| *at.borrow_mut() = None);
}

/// R1: an administrator's `socket-grants` lock turns the certificate check off whatever the
/// project's file says, and says who set it.
#[test]
fn policy_turns_the_certificate_check_off_and_says_who() {
    use crate::sandbox::{CertificateChecks, Policy, policy::Locks};
    let locks = Locks::parse(
        r#"{"owner": "IT", "sandbox": {"socket-grants": false}}"#,
        Path::new("/etc/purlis/policy.json"),
    );
    let said = CertificateChecks::of(&Policy::defaults(), &locks);
    let CertificateChecks::OffByPolicy(why) = &said else {
        panic!("{said:?}");
    };
    assert!(why.contains("Policy turns certificate checks off"), "{why}");
    assert!(why.contains("IT"), "{why}");
    assert!(!said.is_on());
    // Policy first: it is the one that holds, where the project turned it off too.
    let off = Policy {
        certificate_checks: false,
        ..Policy::defaults()
    };
    assert!(matches!(
        CertificateChecks::of(&off, &locks),
        CertificateChecks::OffByPolicy(_)
    ));
    assert_eq!(
        CertificateChecks::of(&off, &Locks::none()),
        CertificateChecks::OffByProject
    );
    assert_eq!(
        CertificateChecks::of(&Policy::defaults(), &Locks::none()),
        CertificateChecks::On
    );
    // And a project whose chats are not sandboxed says nothing of it.
    let dir = tempfile::tempdir().expect("a folder");
    assert_eq!(CertificateChecks::at(dir.path()), None);
}

#[test]
fn a_planted_committed_declaration_grants_nothing() {
    let dir = tempfile::tempdir().expect("a folder");
    let root = dir.path().canonicalize().expect("real").join("plane");
    std::fs::create_dir_all(&root).expect("made");
    std::fs::write(crate::names::manifest(&root), DECLARES).expect("planted");
    let compiled = compiled_at(&root, &Grants::default());
    assert!(compiled.widened.sockets.is_empty(), "{compiled:?}");
    assert!(compiled.widened.services.is_empty());
    let settings = claude::settings(&compiled).expect("compiles");
    assert!(settings.sandbox["network"]["allowUnixSockets"].is_null());
}

/// MF-1 of #1808's review: socket and service grants are kept in the person's own config home,
/// never in the project, so a state file a repository carries names none that is read.
#[test]
fn a_state_file_in_the_project_carrying_sockets_and_services_grants_none() {
    let (_dir, root) = project();
    let home = tempfile::tempdir().expect("a config home");
    local::TEST_CONFIG_HOME.with(|at| *at.borrow_mut() = Some(home.path().to_path_buf()));
    let state = local::legacy_path(&root);
    std::fs::create_dir_all(state.parent().expect("a folder")).expect("made");
    std::fs::write(
        &state,
        r#"{"sockets_mine": ["/var/run/docker.sock"], "services_mine": ["com.apple.trustd.agent"]}"#,
    )
    .expect("planted");
    let compiled = compiled_at(&root, &Grants::default());
    assert!(compiled.widened.sockets.is_empty());
    assert!(compiled.widened.services.is_empty());
    local::TEST_CONFIG_HOME.with(|at| *at.borrow_mut() = None);
}

// ---- what a start compiles in (D-sock-1, D-sock-2) ------------------------------------------

fn machine_at(root: &Path) -> Machine {
    Machine {
        env: crate::secrets::Env::of(&[]),
        home: Some(root.join("home")),
        os: Os::MacOs,
    }
}

fn compiled_at(root: &Path, chat: &Grants) -> Compiled {
    super::super::grant::TEST_HARNESS_TEMP.with(|roots| roots.set(&[]));
    let plane = Plane::read(root);
    let policy = plane.said().policy.expect("on");
    Compiled::granted(&policy, &plane, root, &machine_at(root), None, chat)
}

fn project() -> (tempfile::TempDir, PathBuf) {
    let dir = tempfile::tempdir().expect("a folder");
    let root = dir.path().canonicalize().expect("real").join("plane");
    std::fs::create_dir_all(&root).expect("made");
    std::fs::write(
        crate::names::manifest(&root),
        "[sandbox]\nmode = \"on\"\negress = [\"model-providers\"]\n",
    )
    .expect("written");
    (dir, root)
}

#[test]
fn a_chats_socket_and_yours_reach_its_compiled_sandbox_and_trustd_is_never_a_service_by_name() {
    let (_dir, root) = project();
    let home = tempfile::tempdir().expect("a config home");
    local::TEST_CONFIG_HOME.with(|at| *at.borrow_mut() = Some(home.path().to_path_buf()));
    // Outside every folder a chat may write, the temp folder among them; not there now, which
    // keeps a grant (an engine that is not running has no socket).
    let mine = PathBuf::from("/purlis-test-nowhere/mine.sock");
    local::grant_socket(&root, &mine).expect("kept");
    // In this machine's config home, keyed by the clone, never in the project.
    let kept = local::path(&root);
    assert!(kept.starts_with(home.path()), "{}", kept.display());
    assert!(!local::legacy_path(&root).exists());
    local::grant_service(&root, "com.example.helper").expect("kept");
    // Kept as the kernel names it: a grant is judged to its resolved path before it is kept.
    let docker = PathBuf::from("/purlis-test-nowhere/docker.sock");
    let chat = Grants {
        sockets: vec![docker.clone()],
        services: vec![TRUSTD.to_owned()],
        ..Grants::default()
    };
    let compiled = compiled_at(&root, &chat);
    assert_eq!(compiled.widened.sockets, vec![docker.clone(), mine.clone()]);
    assert_eq!(
        compiled.widened.services,
        vec!["com.example.helper".to_owned()]
    );
    assert!(compiled.widened.trust);

    // Claude Code: each socket by path, the hook socket first once it reports.
    let settings = claude::settings(&compiled).expect("compiles");
    assert_eq!(
        settings.sandbox["network"]["allowUnixSockets"],
        serde_json::json!([docker.display().to_string(), mine.display().to_string()])
    );
    assert_eq!(settings.sandbox["enableWeakerNetworkIsolation"], true);
    let hook = root.join("hooks.sock");
    let reporting = settings.reporting_on(Some(&hook));
    let listed = reporting.sandbox["network"]["allowUnixSockets"]
        .as_array()
        .expect("a list")
        .clone();
    assert_eq!(listed.len(), 3);
    assert_eq!(listed[1], serde_json::json!(docker.display().to_string()));

    // purlis's own wrap: connect only, by path; a service by its whole name.
    let mut own = seatbelt::Own::default();
    seatbelt::widen(&mut own, &compiled.widened).expect("widened");
    assert!(own.after.contains(&format!(
        "(allow network-outbound (remote unix-socket (path-literal \"{}\")))",
        docker.display()
    )));
    assert!(
        own.after
            .contains(&"(allow mach-lookup (global-name \"com.example.helper\"))".to_owned())
    );
    assert!(own.after.contains(&seatbelt::TRUST.to_owned()));

    // Revoked, it is gone from the next start.
    local::revoke_socket(&root, &mine).expect("revoked");
    local::revoke_service(&root, "com.example.helper").expect("revoked");
    let after = compiled_at(&root, &Grants::default());
    assert!(after.widened.sockets.is_empty());
    assert!(after.widened.services.is_empty());
    local::TEST_CONFIG_HOME.with(|at| *at.borrow_mut() = None);
}

/// R1: a grant of the certificate check an older build kept in this machine's file (qw351's
/// "Allow on this machine", a Notice's Allow) is harmless: with it or without it a chat has the
/// check where the project and policy leave it on, by its own switch and one rule, never as a
/// service by name beside it.
#[test]
fn a_grant_of_the_certificate_check_an_older_build_kept_changes_nothing() {
    let (_dir, root) = project();
    let home = tempfile::tempdir().expect("a config home");
    local::TEST_CONFIG_HOME.with(|at| *at.borrow_mut() = Some(home.path().to_path_buf()));
    let before = compiled_at(&root, &Grants::default());
    local::grant_service(&root, TRUSTD).expect("kept");
    let compiled = compiled_at(&root, &Grants::default());
    assert_eq!(compiled.widened, before.widened);
    assert!(compiled.widened.trust);
    assert!(compiled.widened.services.is_empty());
    let mut own = seatbelt::Own::default();
    seatbelt::widen(&mut own, &compiled.widened).expect("widened");
    // The certificate check's own service and no other: nothing wider is compiled for it.
    assert_eq!(
        own.after
            .iter()
            .filter(|rule| rule.contains("mach-lookup"))
            .count(),
        1,
        "{:?}",
        own.after
    );
    local::revoke_service(&root, TRUSTD).expect("revoked");
    assert!(compiled_at(&root, &Grants::default()).widened.trust);
    local::TEST_CONFIG_HOME.with(|at| *at.borrow_mut() = None);
}

// ---- what the project expects is asked about up front (ADR 0093, ruling 2) -----------------

fn expecting(root: &Path, expects: &str) {
    std::fs::write(
        crate::names::manifest(root),
        format!(
            "[sandbox]\nmode = \"on\"\negress = [\"model-providers\"]\nexpects = [{expects}]\n"
        ),
    )
    .expect("written");
}

/// Each state of the up-front question: declared and not allowed (asked, each item with what
/// allowing it means); allowed (nothing awaits); answered "Not now" (not asked again, still
/// awaited for Settings); the declaration changed (asked again); and locked by policy (said
/// with who set it).
#[test]
fn what_a_project_expects_is_asked_once_until_it_is_allowed_declined_or_changes() {
    let home = tempfile::tempdir().expect("a config home");
    local::TEST_CONFIG_HOME.with(|at| *at.borrow_mut() = Some(home.path().to_path_buf()));
    let (_dir, root) = project();
    let machine = machine_at(&root);
    // Nothing declared: nothing to ask. Nor the certificate check, which is no grant (R1).
    assert_eq!(awaiting(&root, &machine), None);
    expecting(&root, "\"com.apple.trustd.agent\"");
    assert_eq!(awaiting(&root, &machine), None);

    // Declared and not allowed: asked, each with its own words.
    expecting(
        &root,
        "\"com.example.helper\", \"/purlis-test-nowhere/docker.sock\"",
    );
    let asked = awaiting(&root, &machine).expect("awaited");
    assert!(asked.asked);
    assert_eq!(asked.locked, None);
    assert_eq!(
        asked.shown(),
        ["/purlis-test-nowhere/docker.sock", "com.example.helper"]
    );
    let socket = &asked.items[0];
    assert!(
        socket.said.contains("about the same as no sandbox"),
        "{socket:?}"
    );
    // A socket that is not there now cannot be allowed, and the item says so.
    assert!(socket.refused.is_some(), "{socket:?}");
    let service = &asked.items[1];
    assert_eq!(service.what, What::Service("com.example.helper".to_owned()));
    assert!(service.said.contains(RISK_UNKNOWN), "{service:?}");
    assert_eq!(service.refused, None);
    // A declaration grants nothing: the start compiles neither.
    let compiled = compiled_at(&root, &Grants::default());
    assert!(compiled.widened.services.is_empty() && compiled.widened.sockets.is_empty());
    // And a chat's pane says it from the start.
    // In purlis's own words alone: what a repository wrote is named on the project's Notice,
    // drawn as data, and never inside this sentence.
    let note = start_note(&root, &machine, &compiled.widened).expect("said");
    assert!(
        note.contains("2 local sockets or system services"),
        "{note}"
    );
    assert!(note.contains("has not allowed"), "{note}");
    assert!(
        !note.contains("com.example.helper") && !note.contains("docker.sock"),
        "{note}"
    );

    // Answered "Not now": remembered on this machine, not asked again, still awaited.
    local::acknowledge_expects(&root, &asked.answered_as_named()).expect("kept");
    let declined = awaiting(&root, &machine).expect("still awaited, for Settings");
    assert!(!declined.asked);
    assert_eq!(declined.items, asked.items);
    assert_eq!(start_note(&root, &machine, &compiled.widened), None);
    // The answer is this machine's, never the project's.
    assert!(!local::legacy_path(&root).exists());

    // The declaration changes, a new item: asked again, whole.
    expecting(
        &root,
        "\"com.example.helper\", \"/purlis-test-nowhere/docker.sock\", \"com.example.other\"",
    );
    let again = awaiting(&root, &machine).expect("awaited");
    assert!(again.asked);
    assert_eq!(again.items.len(), 3);

    // Allowed for this project on this machine: it awaits no more, and the rest still does.
    local::grant_service(&root, "com.example.helper").expect("kept");
    let rest = awaiting(&root, &machine).expect("awaited");
    assert_eq!(
        rest.shown(),
        ["/purlis-test-nowhere/docker.sock", "com.example.other"]
    );
    // Allowed for every project on this machine counts too (R2).
    local::grant_service_everywhere("com.example.other", None).expect("kept");
    local::grant_socket_everywhere(Path::new("/purlis-test-nowhere/docker.sock"), None)
        .expect("kept");
    assert_eq!(awaiting(&root, &machine), None);
    // A chat that holds a thing itself is not told it lacks it.
    local::revoke_service_everywhere("com.example.other").expect("revoked");
    let held = crate::sandbox::Widened {
        services: vec!["com.example.other".to_owned()],
        ..crate::sandbox::Widened::default()
    };
    assert_eq!(start_note(&root, &machine, &held), None);

    // Locked by policy: said with who set it; the items are still named.
    crate::sandbox::policy::set_for_this_test(crate::sandbox::policy::Locks::parse(
        r#"{"owner": "IT", "sandbox": {"socket-grants": false}}"#,
        Path::new("/etc/purlis/policy.json"),
    ));
    let locked = awaiting(&root, &machine).expect("awaited");
    crate::sandbox::policy::set_for_this_test(crate::sandbox::policy::Locks::none());
    let why = locked.locked.clone().expect("locked");
    assert!(why.contains("Policy forbids"), "{why}");
    assert!(why.contains("IT"), "{why}");
    assert_eq!(locked.shown(), ["com.example.other"]);
    // A policy that leaves "for me on this machine" out locks it as well: the Allow would be
    // refused, so it is not offered.
    crate::sandbox::policy::set_for_this_test(crate::sandbox::policy::Locks::parse(
        r#"{"owner": "IT", "sandbox": {"allow-scopes": ["chat"]}}"#,
        Path::new("/etc/purlis/policy.json"),
    ));
    let scoped = awaiting(&root, &machine).expect("awaited");
    crate::sandbox::policy::set_for_this_test(crate::sandbox::policy::Locks::none());
    let why = scoped.locked.expect("locked");
    assert!(
        why.contains("Policy removed") && why.contains("IT"),
        "{why}"
    );

    // A project whose chats are not sandboxed asks nothing.
    std::fs::write(
        crate::names::manifest(&root),
        "[sandbox]\nexpects = [\"com.example.other\"]\n",
    )
    .expect("written");
    assert_eq!(awaiting(&root, &machine), None);
    local::TEST_CONFIG_HOME.with(|at| *at.borrow_mut() = None);
}

/// R2 (2026-10-13): a socket or a service allowed "on this machine, for every project" is kept
/// in this machine's one file, never in a project or under a clone's key, and reaches a chat's
/// start in every project, one made after the Allow too. Each is judged again for the project
/// a chat starts in, and Remove takes it from all of them.
#[test]
fn a_socket_or_a_service_allowed_for_every_project_reaches_each_projects_next_start() {
    let home = tempfile::tempdir().expect("a config home");
    local::TEST_CONFIG_HOME.with(|at| *at.borrow_mut() = Some(home.path().to_path_buf()));
    let (_first_dir, first) = project();
    let docker = PathBuf::from("/purlis-test-nowhere/docker.sock");
    let made = |what: &str, target: &str| local::Made {
        what: what.to_owned(),
        target: target.to_owned(),
        level: "machine".to_owned(),
        at: 7,
        chat: Some("steward 1".to_owned()),
    };
    local::grant_socket_everywhere(
        &docker,
        Some(made("socket", "/purlis-test-nowhere/docker.sock")),
    )
    .expect("kept");
    local::grant_service_everywhere("com.example.helper", None).expect("kept");
    // One file for every project, in the config home, and nothing under either project.
    let kept = local::everywhere_path();
    assert!(kept.starts_with(home.path()), "{}", kept.display());
    assert_eq!(
        kept.file_name().and_then(|name| name.to_str()),
        Some(local::EVERY_PROJECT)
    );
    assert_ne!(kept, local::path(&first));
    assert!(
        !local::path(&first).exists(),
        "no clone's own file is written"
    );
    assert!(local::granted_sockets(&first).is_empty());
    // A project made after the Allow has it too.
    let (_second_dir, second) = project();
    for root in [&first, &second] {
        let compiled = compiled_at(root, &Grants::default());
        assert_eq!(compiled.widened.sockets, vec![docker.clone()], "{root:?}");
        assert_eq!(
            compiled.widened.services,
            vec!["com.example.helper".to_owned()]
        );
    }
    assert_eq!(local::made_everywhere().len(), 1);
    // Allowed twice is kept once.
    local::grant_socket_everywhere(&docker, None).expect("kept");
    assert_eq!(local::sockets_everywhere(), vec![docker.clone()]);
    // Policy's lock takes them away at a start, as it takes a project's.
    crate::sandbox::policy::set_for_this_test(crate::sandbox::policy::Locks::parse(
        r#"{"owner": "IT", "sandbox": {"socket-grants": false}}"#,
        Path::new("/etc/purlis/policy.json"),
    ));
    let locked = compiled_at(&first, &Grants::default());
    crate::sandbox::policy::set_for_this_test(crate::sandbox::policy::Locks::none());
    assert!(locked.widened.sockets.is_empty());
    assert!(locked.widened.services.is_empty());
    // Removed, it is gone from every project's next start, with its record.
    local::revoke_socket_everywhere(&docker).expect("revoked");
    local::revoke_service_everywhere("com.example.helper").expect("revoked");
    for root in [&first, &second] {
        let after = compiled_at(root, &Grants::default());
        assert!(after.widened.sockets.is_empty());
        assert!(after.widened.services.is_empty());
    }
    assert!(local::made_everywhere().is_empty());
    local::TEST_CONFIG_HOME.with(|at| *at.borrow_mut() = None);
}

/// R2: a socket allowed for every project is still judged for the project a chat starts in.
/// One inside that project's own tree (a folder its chats write) reaches no chat there.
#[test]
fn a_socket_allowed_for_every_project_is_judged_again_for_each_project() {
    let home = tempfile::tempdir().expect("a config home");
    local::TEST_CONFIG_HOME.with(|at| *at.borrow_mut() = Some(home.path().to_path_buf()));
    let (_dir, root) = project();
    let (_other_dir, other) = project();
    let inside = root.join("run").join("svc.sock");
    local::grant_socket_everywhere(&inside, None).expect("kept");
    assert!(
        compiled_at(&root, &Grants::default())
            .widened
            .sockets
            .is_empty(),
        "a chat of that project could put any socket there"
    );
    // It is in a temp folder, which every chat writes, so no other project takes it either.
    assert!(
        compiled_at(&other, &Grants::default())
            .widened
            .sockets
            .is_empty()
    );
    local::TEST_CONFIG_HOME.with(|at| *at.borrow_mut() = None);
}

/// R2: "every project on this machine" is a socket's or a service's alone, no word of a policy
/// names it, and a policy that fixes the scopes leaves it out.
#[test]
fn every_project_is_offered_for_a_socket_or_a_service_only_and_policy_scopes_leave_it_out() {
    use crate::sandbox::grant::Level;
    use crate::sandbox::policy::Locks;
    assert_eq!(Level::of_word("machine"), Some(Level::Machine));
    assert_eq!(Level::Machine.said(), "on this machine, for every project");
    let socket = What::Socket(PathBuf::from("/var/run/docker.sock"));
    let service = What::Service("com.example.helper".to_owned());
    let none = Locks::none();
    assert_eq!(none.refuses_grant(&socket, Level::Machine), None);
    assert_eq!(none.refuses_grant(&service, Level::Machine), None);
    let host = What::Host(crate::sandbox::grant::host("api.example.com").expect("a host"));
    assert!(none.refuses_grant(&host, Level::Machine).is_some());
    assert!(
        none.refuses_grant(&What::Write(PathBuf::from("/opt/cache")), Level::Machine)
            .is_some()
    );
    // A policy that fixes the scopes: "you" does not stand in for every project.
    let scoped = Locks::parse(
        r#"{"owner": "IT", "sandbox": {"allow-scopes": ["chat", "you"]}}"#,
        Path::new("/etc/purlis/policy.json"),
    );
    assert_eq!(scoped.refuses_grant(&socket, Level::You), None);
    let why = scoped
        .refuses_grant(&socket, Level::Machine)
        .expect("left out");
    assert!(why.contains("Policy removed Allow"), "{why}");
    // "machine" is the policy's word for it (ruling C3): named, it is kept.
    let worded = Locks::parse(
        r#"{"sandbox": {"allow-scopes": ["machine"]}}"#,
        Path::new("/etc/purlis/policy.json"),
    );
    assert_eq!(worded.refused_because(), None);
    assert_eq!(worded.refuses_grant(&socket, Level::Machine), None);
    // The lock on sockets and services holds at this level as at any.
    let locked = Locks::parse(
        r#"{"sandbox": {"socket-grants": false}}"#,
        Path::new("/etc/purlis/policy.json"),
    );
    assert!(locked.refuses_grant(&socket, Level::Machine).is_some());
}

/// #1927: what a person allowed before a policy fixed `allow-scopes` is judged against the
/// policy at every start, whichever level kept it. A level the policy leaves out contributes
/// nothing, and what is kept stays on disk, said as held, for the day the policy lets it back.
#[test]
fn what_was_kept_at_a_scope_policy_leaves_out_reaches_no_start_and_stays_kept() {
    use crate::sandbox::grant::Level;
    use crate::sandbox::policy::{Locks, set_for_this_test};
    let home = tempfile::tempdir().expect("a config home");
    local::TEST_CONFIG_HOME.with(|at| *at.borrow_mut() = Some(home.path().to_path_buf()));
    let (_dir, root) = project();
    let everywhere = PathBuf::from("/purlis-test-nowhere/everywhere.sock");
    let mine = PathBuf::from("/purlis-test-nowhere/mine.sock");
    let held = PathBuf::from("/purlis-test-nowhere/chat.sock");
    let once = PathBuf::from("/purlis-test-nowhere/once.sock");
    local::grant_socket_everywhere(&everywhere, None).expect("kept");
    local::grant_service_everywhere("com.example.everywhere", None).expect("kept");
    local::grant_socket(&root, &mine).expect("kept");
    local::grant_service(&root, "com.example.mine").expect("kept");
    let mut chat = Grants::default();
    chat.add(&What::Socket(held.clone()));
    chat.add(&What::Service("com.example.chat".to_owned()));
    chat.add_once(&What::Socket(once.clone()));
    let started = |policy: Option<&str>| {
        set_for_this_test(policy.map_or_else(Locks::none, |json| {
            Locks::parse(json, Path::new("/etc/purlis/policy.json"))
        }));
        let widened = compiled_at(&root, &chat).widened;
        set_for_this_test(Locks::none());
        (widened.sockets, widened.services)
    };
    let names = |names: &[&str]| {
        names
            .iter()
            .map(|one| (*one).to_owned())
            .collect::<Vec<_>>()
    };
    // No policy, and one that fixes no scopes: every level's.
    for open in [None, Some(r#"{"sandbox": {}}"#)] {
        assert_eq!(
            started(open),
            (
                vec![held.clone(), once.clone(), mine.clone(), everywhere.clone()],
                names(&[
                    "com.example.chat",
                    "com.example.mine",
                    "com.example.everywhere"
                ])
            ),
            "{open:?}"
        );
    }
    // Every project left out: this chat's and this project's alone.
    assert_eq!(
        started(Some(
            r#"{"sandbox": {"allow-scopes": ["once", "chat", "you"]}}"#
        )),
        (
            vec![held.clone(), once.clone(), mine.clone()],
            names(&["com.example.chat", "com.example.mine"])
        )
    );
    // Only this chat: what was kept for the project and for every project reaches nothing,
    // and neither does what was allowed once, which is a scope of its own.
    assert_eq!(
        started(Some(r#"{"sandbox": {"allow-scopes": ["chat"]}}"#)),
        (vec![held.clone()], names(&["com.example.chat"]))
    );
    // Every project named and this project left out: neither word stands in for the other.
    assert_eq!(
        started(Some(
            r#"{"sandbox": {"allow-scopes": ["once", "machine"]}}"#
        )),
        (
            vec![once.clone(), everywhere.clone()],
            names(&["com.example.everywhere"])
        )
    );
    // A file purlis refuses keeps no scope: nothing.
    assert_eq!(
        started(Some(r#"{"sandbox": {"allow-scopes": ["everywhere"]}}"#)),
        (Vec::new(), Vec::new())
    );
    // Nothing was taken off this machine's record by any of that.
    assert_eq!(local::sockets_everywhere(), vec![everywhere.clone()]);
    assert_eq!(local::granted_sockets(&root), vec![mine.clone()]);
    assert_eq!(local::granted_services(&root), names(&["com.example.mine"]));
    // And each held one is said as held, with who set the policy.
    let scoped = Locks::parse(
        r#"{"owner": "IT", "sandbox": {"allow-scopes": ["once", "chat", "you"]}}"#,
        Path::new("/etc/purlis/policy.json"),
    );
    let why = scoped
        .holds_kept(&What::Socket(everywhere.clone()), Level::Machine)
        .expect("held");
    assert!(why.starts_with("Held by policy"), "{why}");
    assert!(
        why.contains("on this machine, for every project") && why.contains("set by IT"),
        "{why}"
    );
    assert_eq!(
        scoped.holds_kept(&What::Socket(mine.clone()), Level::You),
        None
    );
    assert_eq!(
        Locks::none().holds_kept(&What::Socket(mine), Level::You),
        None
    );
    set_for_this_test(scoped);
    let held_now = crate::sandbox::held_by_policy(&root);
    set_for_this_test(Locks::none());
    assert_eq!(
        held_now
            .iter()
            .map(|one| (one.what.target(), one.level))
            .collect::<Vec<_>>(),
        [
            (everywhere.display().to_string(), Level::Machine),
            ("com.example.everywhere".to_owned(), Level::Machine),
        ]
    );
    local::TEST_CONFIG_HOME.with(|at| *at.borrow_mut() = None);
}

/// #1927: a declared socket that was allowed at a scope policy now leaves out is not allowed
/// any more, so it waits again, and is asked about at the scope that is left.
#[test]
fn what_a_project_expects_waits_again_once_policy_holds_the_scope_it_was_allowed_at() {
    use crate::sandbox::policy::{Locks, set_for_this_test};
    let home = tempfile::tempdir().expect("a config home");
    local::TEST_CONFIG_HOME.with(|at| *at.borrow_mut() = Some(home.path().to_path_buf()));
    let (_dir, root) = project();
    std::fs::write(
        crate::names::manifest(&root),
        "[sandbox]\nmode = \"on\"\negress = [\"model-providers\"]\nexpects = \
         [\"com.example.helper\"]\n",
    )
    .expect("written");
    local::grant_service_everywhere("com.example.helper", None).expect("kept");
    let machine = machine_at(&root);
    assert_eq!(awaiting(&root, &machine), None, "allowed for every project");
    set_for_this_test(Locks::parse(
        r#"{"sandbox": {"allow-scopes": ["chat", "you"]}}"#,
        Path::new("/etc/purlis/policy.json"),
    ));
    let waits = awaiting(&root, &machine);
    set_for_this_test(Locks::none());
    let waits = waits.expect("held by policy, so it waits");
    assert_eq!(waits.shown(), ["com.example.helper"]);
    assert_eq!(waits.locked, None, "for me on this machine is still open");
    local::TEST_CONFIG_HOME.with(|at| *at.borrow_mut() = None);
}

/// #1927: an answer is to what was shown. After "Not now", a declared path that comes to name
/// another socket is asked about again; one that names what was answered before is not, so a
/// link moving between two answered sockets asks once for each and no more.
#[test]
fn a_declared_path_that_names_another_socket_since_the_answer_is_asked_about_again() {
    let item = |written: &str, named: &str, refused: bool| Awaited {
        what: What::Socket(PathBuf::from(named)),
        written: written.to_owned(),
        said: String::new(),
        refused: refused.then(|| "Not there now.".to_owned()),
    };
    let answered = |written: &str, named: &[&str]| local::Answered {
        written: written.to_owned(),
        named: named.iter().map(|one| (*one).to_owned()).collect(),
    };
    const WRITTEN: &str = "/opt/project/run/dev.sock";
    let harmless = item(WRITTEN, "/Users/dev/.tool/dev.sock", false);
    let engine = item(WRITTEN, "/Users/dev/.docker/run/docker.sock", false);
    // Never answered: asked.
    assert!(still_asked(std::slice::from_ref(&harmless), &[]));
    // Answered as it was shown: not asked again.
    let seen = [answered(WRITTEN, &["/Users/dev/.tool/dev.sock"])];
    assert!(!still_asked(std::slice::from_ref(&harmless), &seen));
    // The same declared path names another socket now: asked again.
    assert!(still_asked(std::slice::from_ref(&engine), &seen));
    // Both answered: neither is asked about again, whichever the path names today.
    let both = [answered(
        WRITTEN,
        &[
            "/Users/dev/.tool/dev.sock",
            "/Users/dev/.docker/run/docker.sock",
        ],
    )];
    assert!(!still_asked(std::slice::from_ref(&engine), &both));
    assert!(!still_asked(std::slice::from_ref(&harmless), &both));
    // An answer to another declared path answers nothing here.
    let other = [answered("/opt/other.sock", &["/Users/dev/.tool/dev.sock"])];
    assert!(still_asked(std::slice::from_ref(&harmless), &other));
    // A service is named as it is written.
    let helper = Awaited {
        what: What::Service("com.example.helper".to_owned()),
        written: "com.example.helper".to_owned(),
        said: String::new(),
        refused: None,
    };
    assert!(!still_asked(
        std::slice::from_ref(&helper),
        &[answered("com.example.helper", &["com.example.helper"])]
    ));
}

/// MF-2 fold-in of #1808's review: an administrator's `socket-grants` lock drops every socket
/// and service at a start, this chat's and yours, and the certificate check with them.
#[test]
fn the_socket_grants_lock_drops_every_socket_and_service_at_a_start() {
    let locks = crate::sandbox::policy::Locks::parse(
        r#"{"owner": "IT", "sandbox": {"socket-grants": false}}"#,
        Path::new("/etc/purlis/policy.json"),
    );
    let widened = crate::sandbox::Widened {
        trust: true,
        ..crate::sandbox::Widened::default()
    }
    .and_granted(
        &[PathBuf::from("/purlis-test-nowhere/docker.sock")],
        &[TRUSTD.to_owned(), "com.example.helper".to_owned()],
        &SocketGround::default(),
        &locks,
    );
    assert!(widened.sockets.is_empty());
    assert!(widened.services.is_empty());
    assert!(!widened.trust);
    // And with no lock, each is taken.
    let open = crate::sandbox::Widened::default().and_granted(
        &[PathBuf::from("/purlis-test-nowhere/docker.sock")],
        &[TRUSTD.to_owned()],
        &SocketGround::default(),
        &crate::sandbox::policy::Locks::none(),
    );
    assert_eq!(open.sockets.len(), 1);
    // Never from a grant (R1): the project's setting alone turns it on.
    assert!(!open.trust);
    assert!(open.services.is_empty());
}

/// #1808's review, MF-1a: the file an always grant is kept in is one every chat is denied
/// writing: purlis's own folder in the config home.
#[test]
fn the_machine_grants_file_is_denied_to_every_chats_writes() {
    let (_dir, root) = project();
    let home = tempfile::tempdir().expect("a config home");
    let home_path = home.path().canonicalize().expect("real");
    local::TEST_CONFIG_HOME.with(|at| *at.borrow_mut() = Some(home_path.clone()));
    let file = local::path(&root);
    let machine = Machine {
        env: crate::secrets::Env::of(&[(
            crate::machine::HOME_VAR,
            home_path.to_str().expect("utf-8"),
        )]),
        home: Some(root.join("home")),
        os: Os::MacOs,
    };
    let denied = crate::sandbox::Denied::of(&root, &machine);
    assert!(
        denied
            .paths
            .iter()
            .any(|denial| file.starts_with(&denial.path)),
        "{} is not denied: {:?}",
        file.display(),
        denied.paths
    );
    // And as compiled: a Claude Code chat's profile denies writing it.
    super::super::grant::TEST_HARNESS_TEMP.with(|roots| roots.set(&[]));
    let plane = Plane::read(&root);
    let policy = plane.said().policy.expect("on");
    let compiled = Compiled::granted(&policy, &plane, &root, &machine, None, &Grants::default());
    let settings = claude::settings(&compiled).expect("compiles");
    let deny = settings.sandbox["filesystem"]["denyWrite"]
        .as_array()
        .expect("a denyWrite list");
    assert!(
        deny.iter()
            .filter_map(serde_json::Value::as_str)
            .any(|denied| file.starts_with(denied)),
        "{} is not in denyWrite: {deny:?}",
        file.display()
    );
    local::TEST_CONFIG_HOME.with(|at| *at.borrow_mut() = None);
}

/// #1808's second review: a socket in a folder others may write and this user does not own
/// (`/tmp`, where Postgres keeps `.s.PGSQL.5432` by default) is never granted, nor compiled:
/// anyone could have put it there, or put another in its place.
#[test]
fn a_socket_in_a_folder_others_may_write_is_never_granted_nor_compiled() {
    let tmp = Path::new("/private/tmp");
    if !tmp.is_dir() {
        return;
    }
    let postgres = tmp.join(".s.PGSQL.5432");
    assert!(exposed(&postgres));
    assert!(still_sockets(&[postgres], &SocketGround::default()).is_empty());
    // A folder of this user's, others' bits off, is not.
    let dir = tempfile::tempdir().expect("a folder");
    std::fs::set_permissions(
        dir.path(),
        std::os::unix::fs::PermissionsExt::from_mode(0o700),
    )
    .expect("this user's alone");
    assert!(!exposed(&dir.path().join("x.sock")));
}

/// #1808's third review: the shape a socket is in, rule by rule, with the owner faked. A socket
/// another user owns is refused; one the system owns, or this user, is not; a folder anyone may
/// write is exposed whoever owns it; a system folder others may write only as a system group
/// (`/private/var/run`, root:daemon 0775) is not.
#[test]
fn the_shape_of_a_socket_is_judged_rule_by_rule() {
    let me = 501;
    let seen = |owner: u32, folder: Option<(u32, u32)>| Seen {
        socket: true,
        links: 1,
        owner,
        folder,
    };
    let mine_0700 = Some((0o40700, me));
    assert_eq!(shape_of(seen(me, mine_0700), me), Shape::Socket);
    assert_eq!(shape_of(seen(0, Some((0o40775, 0))), me), Shape::Socket);
    assert_eq!(shape_of(seen(502, mine_0700), me), Shape::Foreign);
    assert_eq!(shape_of(seen(0, Some((0o41777, 0))), me), Shape::Exposed);
    assert_eq!(shape_of(seen(me, Some((0o40775, 502))), me), Shape::Exposed);
    assert_eq!(shape_of(seen(me, Some((0o40777, me))), me), Shape::Socket);
    assert_eq!(
        shape_of(
            Seen {
                links: 2,
                ..seen(me, mine_0700)
            },
            me
        ),
        Shape::Linked
    );
    assert_eq!(
        shape_of(
            Seen {
                socket: false,
                ..seen(me, mine_0700)
            },
            me
        ),
        Shape::Other
    );
}

/// #1808's third review: every folder each harness's compiled sandbox lets a chat write is one
/// the socket ground refuses, so no socket there is ever offered.
#[test]
fn every_folder_a_compiled_sandbox_lets_a_chat_write_is_refused_for_a_socket() {
    super::super::grant::TEST_HARNESS_TEMP.with(|roots| roots.set(&[]));
    let dir = tempfile::tempdir().expect("a folder");
    let base = dir.path().canonicalize().expect("real");
    let root = base.join("plane");
    std::fs::create_dir_all(&root).expect("made");
    std::fs::write(crate::names::manifest(&root), "[sandbox]\nmode = \"on\"\n").expect("written");
    let machine = Machine {
        env: crate::secrets::Env::of(&[]),
        home: Some(base.join("home")),
        os: Os::MacOs,
    };
    let ground = SocketGround::of(&root, &machine, &[]);
    let mut missed = Vec::new();
    for harness in [
        crate::harness::Harness::ClaudeCode,
        crate::harness::Harness::Codex,
        crate::harness::Harness::Opencode,
    ] {
        let applied = crate::sandbox::for_start(harness, &root, &machine, &|_| true)
            .unwrap_or_else(|why| panic!("{harness:?}: {why}"))
            .expect("sandboxed");
        let mut writable: Vec<PathBuf> = applied.writable();
        if let crate::sandbox::Form::ClaudeCode(settings) = applied.form()
            && let Some(list) = settings.sandbox["filesystem"]["allowWrite"].as_array()
        {
            for one in list.iter().filter_map(serde_json::Value::as_str) {
                writable.push(PathBuf::from(one.trim_end_matches("/*/**")));
            }
        }
        for path in writable {
            let probe = path.join("x.sock");
            if ground.refuses(&probe).is_none()
                && ground.refuses(&crate::sandbox::real(&probe)).is_none()
            {
                missed.push(format!("{harness:?} {}", path.display()));
            }
        }
    }
    assert!(missed.is_empty(), "not refused: {missed:#?}");
}

// ---- what a person chooses from (ADR 0093, amended 2026-10-13) --------------------------------

fn env_of(
    pairs: &'static [(&'static str, &'static str)],
) -> impl Fn(&str) -> Option<std::ffi::OsString> {
    move |key| {
        pairs
            .iter()
            .find(|(name, _)| *name == key)
            .map(|(_, value)| std::ffi::OsString::from(value))
    }
}

#[test]
fn the_places_looked_in_are_a_fixed_table_and_the_apps_own_environment() {
    let home = Path::new("/Users/op");
    let found = candidates(
        Some(home),
        &env_of(&[
            ("DOCKER_HOST", "unix:///Users/op/.colima/work/docker.sock"),
            (
                "SSH_AUTH_SOCK",
                "/private/tmp/com.apple.launchd.AbC/Listeners",
            ),
            ("PGHOST", "/Users/op/pg"),
            ("PGPORT", "5433"),
            ("MYSQL_UNIX_PORT", "/Users/op/mysql/mysql.sock"),
        ]),
    );
    for path in [
        "/Users/op/.colima/work/docker.sock",
        "/Users/op/.docker/run/docker.sock",
        "/var/run/docker.sock",
        "/Users/op/.orbstack/run/docker.sock",
        "/Users/op/.local/share/containers/podman/machine/podman.sock",
        "/private/tmp/com.apple.launchd.AbC/Listeners",
        "/Users/op/.gnupg/S.gpg-agent.ssh",
        "/Users/op/pg/.s.PGSQL.5433",
        "/Users/op/mysql/mysql.sock",
        "/var/run/postgresql/.s.PGSQL.5432",
    ] {
        assert!(found.contains(&PathBuf::from(path)), "{path}: {found:?}");
    }
    // The environment's engine first, then the table's, then agents, then databases.
    let at = |path: &str| {
        found
            .iter()
            .position(|one| one == Path::new(path))
            .expect(path)
    };
    assert_eq!(at("/Users/op/.colima/work/docker.sock"), 0);
    assert!(at("/var/run/docker.sock") < at("/private/tmp/com.apple.launchd.AbC/Listeners"));
    assert!(at("/private/tmp/com.apple.launchd.AbC/Listeners") < at("/Users/op/pg/.s.PGSQL.5433"));
    // No place in a folder anyone may write: no grant names a socket there.
    assert!(
        found.iter().all(|path| !path.starts_with("/tmp")),
        "{found:?}"
    );
}

#[test]
fn an_endpoint_that_is_no_local_socket_is_no_place_to_look() {
    let found = candidates(
        Some(Path::new("/Users/op")),
        &env_of(&[
            ("DOCKER_HOST", "tcp://10.0.0.5:2376"),
            (
                "CONTAINER_HOST",
                "ssh://core@localhost:2222/run/podman.sock",
            ),
            ("SSH_AUTH_SOCK", ""),
            ("PGHOST", "db.example.com"),
            ("PGPORT", "5432; rm"),
        ]),
    );
    assert!(
        found
            .iter()
            .all(|path| path.is_absolute() && !path.to_string_lossy().contains("://")),
        "{found:?}"
    );
    assert!(
        !found
            .iter()
            .any(|path| path.to_string_lossy().contains("10.0.0.5")
                || path.to_string_lossy().contains("example.com")),
        "{found:?}"
    );
    // With no home folder, only the places from `/`.
    let rootless = candidates(None, &env_of(&[]));
    assert!(rootless.contains(&PathBuf::from("/var/run/docker.sock")));
    assert!(rootless.iter().all(|path| path.is_absolute()));
}

#[test]
fn only_what_a_grant_may_name_is_listed_once_each_in_the_order_looked() {
    let places = [
        PathBuf::from("/var/run/docker.sock"),
        PathBuf::from("/Users/op/.docker/run/docker.sock"),
        PathBuf::from("/Users/op/plane/planted.sock"),
        PathBuf::from("/Users/op/gone.sock"),
        PathBuf::from("/Users/op/agent/Listeners"),
        PathBuf::from("/var/run/postgresql/.s.PGSQL.5432"),
    ];
    // As `socket` would answer: the system's docker.sock is a link to the home folder's, one is
    // where a chat may write, and one is not there.
    let judge = |typed: &str| match typed {
        "/var/run/docker.sock" | "/Users/op/.docker/run/docker.sock" => {
            Ok(PathBuf::from("/Users/op/.docker/run/docker.sock"))
        }
        "/Users/op/plane/planted.sock" => Err(Refused::Outside("a chat may write".to_owned())),
        "/Users/op/gone.sock" => Err(Refused::Not("not a socket".to_owned())),
        other => Ok(PathBuf::from(other)),
    };
    let found = discovered_by(
        &places,
        Some(std::ffi::OsStr::new("/Users/op/agent/Listeners")),
        &judge,
    );
    assert_eq!(
        found,
        vec![
            Found {
                path: PathBuf::from("/Users/op/.docker/run/docker.sock"),
                known: Known::Containers,
            },
            Found {
                path: PathBuf::from("/Users/op/agent/Listeners"),
                known: Known::SshAgent,
            },
            Found {
                path: PathBuf::from("/var/run/postgresql/.s.PGSQL.5432"),
                known: Known::Database,
            },
        ]
    );
}

#[test]
fn a_candidate_is_judged_exactly_as_a_named_socket_is() {
    // Nothing in a fresh folder is a socket, and a file is not one: the same `socket` refuses
    // each, so nothing is listed. (A real socket is bound on CI by the app's tests.)
    let dir = tempfile::tempdir().expect("a folder");
    let file = dir.path().join("docker.sock");
    std::fs::write(&file, "x").expect("written");
    let places = [file, dir.path().join("gone.sock")];
    assert_eq!(discovered(&places, &SocketGround::default()), Vec::new());
}

#[test]
fn the_services_offered_to_choose_are_the_ones_a_grant_may_name() {
    // The certificate check is no grant (R1), so it is never one to choose.
    assert!(!GRANTABLE_SERVICES.contains(&TRUSTD));
    for name in GRANTABLE_SERVICES {
        assert_eq!(service(name), Ok((*name).to_owned()));
    }
    assert_eq!(Known::CertificateCheck.named(), "Certificate check");
    assert_eq!(Known::Containers.named(), "Container engine");
}

// ---- the system's name lookup (#2657) -------------------------------------------------------

/// #2657: **a socket grant never names the system's name-lookup sockets**: each path of
/// [`crate::sandboxblock::RESOLVER_SOCKETS`], written whole or as a `unix://` address, is
/// refused in one sentence, whether or not a socket is there, and one kept for it is never
/// compiled in.
#[test]
fn a_socket_grant_never_names_the_systems_name_lookup() {
    let (_dir, root) = project();
    let ground = SocketGround::of(&root, &machine_at(&root), &[]);
    for path in crate::sandboxblock::RESOLVER_SOCKETS {
        for typed in [path.to_owned(), format!("unix://{path}")] {
            assert_eq!(
                socket(&typed, &ground),
                Err(Refused::Outside(NAME_LOOKUP.to_owned())),
                "{typed}"
            );
        }
        assert!(is_name_lookup(Path::new(path)), "{path}");
    }
    assert!(!is_name_lookup(Path::new("/var/run/docker.sock")));
    assert_eq!(NAME_LOOKUP.matches(". ").count(), 0, "one sentence");
    let kept: Vec<PathBuf> = crate::sandboxblock::RESOLVER_SOCKETS
        .iter()
        .map(PathBuf::from)
        .collect();
    assert!(still_sockets(&kept, &ground).is_empty());
}
