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
            services: vec![TRUSTD.to_owned()],
        }
    );
    // And the key is one the file may hold: no refusal for it.
    assert!(Plane::of(Some(DECLARES)).said().refused.is_empty());
}

/// The operator's rulings 2 and 3 (2026-10-12): the committed `certificate-checks` declares the
/// certificate check and grants nothing; the person allows it once on their own machine.
#[test]
fn a_committed_certificate_checks_declares_the_check_and_grants_nothing() {
    let (_dir, root) = project();
    std::fs::write(
        crate::names::manifest(&root),
        "[sandbox]\nmode = \"on\"\negress = [\"model-providers\"]\ncertificate-checks = true\n",
    )
    .expect("written");
    let compiled = compiled_at(&root, &Grants::default());
    assert!(!compiled.widened.trust, "a committed file grants nothing");
    let settings = claude::settings(&compiled).expect("compiles");
    assert_eq!(settings.sandbox["enableWeakerNetworkIsolation"], false);
    let declared = declared(&root);
    assert_eq!(declared.services, vec![TRUSTD.to_owned()]);
    assert_eq!(
        said_of(&What::Service(TRUSTD.to_owned()), &declared),
        format!("This project expects certificate checks. Allowing it {RISK_TRUSTD}.")
    );
    // Allowed by the person, for the chat: on.
    let allowed = compiled_at(
        &root,
        &Grants {
            services: vec![TRUSTD.to_owned()],
            ..Grants::default()
        },
    );
    assert!(allowed.widened.trust);
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
    assert!(!compiled.widened.trust);
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
    let state = local::path(&root);
    std::fs::create_dir_all(state.parent().expect("a folder")).expect("made");
    std::fs::write(
        &state,
        r#"{"sockets_mine": ["/var/run/docker.sock"], "services_mine": ["com.apple.trustd.agent"]}"#,
    )
    .expect("planted");
    let compiled = compiled_at(&root, &Grants::default());
    assert!(compiled.widened.sockets.is_empty());
    assert!(compiled.widened.services.is_empty());
    assert!(!compiled.widened.trust);
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
fn a_chats_socket_and_yours_reach_its_compiled_sandbox_and_trustd_is_the_certificate_check() {
    let (_dir, root) = project();
    let home = tempfile::tempdir().expect("a config home");
    local::TEST_CONFIG_HOME.with(|at| *at.borrow_mut() = Some(home.path().to_path_buf()));
    // Outside every folder a chat may write, the temp folder among them; not there now, which
    // keeps a grant (an engine that is not running has no socket).
    let mine = PathBuf::from("/purlis-test-nowhere/mine.sock");
    local::grant_socket(&root, &mine).expect("kept");
    // In this machine's config home, keyed by the clone, never in the project.
    let kept = local::machine_grants_file(&root).expect("a file");
    assert!(kept.starts_with(home.path()), "{}", kept.display());
    assert!(!local::path(&root).exists());
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
    assert!(open.trust);
}
