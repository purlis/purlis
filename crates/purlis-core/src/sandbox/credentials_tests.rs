// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The person's credential files are never a chat's to read at the full sandbox** (ADR 0067
//! §5 class 6, amended 2026-11-16 and 2026-11-17, rulings Q321 points 17 and 27, and Q325). The
//! SSH folder, all but what holds no secret in it, and each forge and cloud credential file
//! [`credential_paths`] names are denied at the full sandbox, and at the floor only where the
//! project's policy enforces `secrets.credential_files` (`team`, `strict`, or the key itself), in
//! what every harness is compiled to: Claude Code's settings, the Seatbelt profile of a wrapped
//! chat on macOS, and the profile purlis's own wrap reads on Linux. What sits beside them and
//! holds no secret (`known_hosts`, `config`, a public key) stays readable. The person's own SSH
//! agent is no socket a chat reaches until a person allows it.

use std::path::{Path, PathBuf};

use purlis_linux_sandbox::profile::{FileOp, Kind, Op, Profile, Remote};
use serde_json::Value;

use super::*;

const FULL: &str = "[sandbox]\nmode = \"on\"\negress = [\"model-providers\"]\n";
/// The floor under the shipped template, `watch`: the class is not held (Q325).
const FLOOR: &str = "schema = 1\n";
/// The floor where the project's policy enforces the class by its key (Q325).
const FLOOR_ENFORCED: &str = "schema = 1\n[policy]\nsecrets.credential_files = \"enforce\"\n";
/// The floor under `strict`, which enforces every family, this one among them.
const FLOOR_STRICT: &str = "schema = 1\n[policy]\ntemplate = \"strict\"\n";
/// The floor under `team`, whose sandbox level `[sandbox]` still decides (Q316).
const FLOOR_TEAM: &str = "schema = 1\n[policy]\ntemplate = \"team\"\n";

/// Every project whose chats are held from the person's credentials.
const HELD: [&str; 4] = [FULL, FLOOR_ENFORCED, FLOOR_STRICT, FLOOR_TEAM];

/// What `~/.ssh` holds on the machine these tests make: two keys by the usual names, one by a
/// name of the person's own, and what holds no secret.
const SSH_KEYS: [&str; 3] = ["id_ed25519", "id_rsa", "deploy_github"];
const SSH_OPEN_HERE: [&str; 4] = ["id_ed25519.pub", "known_hosts", "known_hosts.old", "config"];

/// What appears in `~/.ssh` after the chat started, by every kind of name: each held.
const SSH_LATE: [&str; 5] = [
    "id_ecdsa",
    "work_key",
    "github.pem",
    "id_ed25519_new",
    "keys/deploy",
];

fn real(path: &Path) -> PathBuf {
    path.canonicalize().expect("a real path")
}

/// A home with an `~/.ssh` as [`SSH_KEYS`] and [`SSH_OPEN_HERE`] say, and every file
/// [`CREDENTIAL_FILES`] names.
fn home() -> (tempfile::TempDir, PathBuf) {
    let dir = tempfile::tempdir().expect("a home");
    let home = real(dir.path());
    let ssh = home.join(".ssh");
    std::fs::create_dir_all(&ssh).expect("~/.ssh");
    for name in SSH_KEYS.iter().chain(SSH_OPEN_HERE.iter()) {
        std::fs::write(ssh.join(name), "x").expect("a file in ~/.ssh");
    }
    for file in CREDENTIAL_FILES {
        let path = home.join(file);
        std::fs::create_dir_all(path.parent().expect("a folder")).expect("its folder");
        std::fs::write(&path, "x").expect("a credential file");
    }
    (dir, home)
}

fn machine(home: &Path, os: Os) -> Machine {
    machine_with(home, os, &[])
}

fn machine_with(home: &Path, os: Os, env: &[(&str, &str)]) -> Machine {
    Machine {
        env: crate::secrets::Env::of(env),
        home: Some(home.to_path_buf()),
        os,
    }
}

/// Every file of `home` a held chat must never read: the keys there at the start, the ones made
/// after it, and each named credential file.
fn secrets_in(home: &Path) -> Vec<PathBuf> {
    SSH_KEYS
        .iter()
        .chain(SSH_LATE.iter())
        .map(|name| home.join(".ssh").join(name))
        .chain(CREDENTIAL_FILES.iter().map(|file| home.join(file)))
        .collect()
}

/// What a chat of `harness` starts under in a project saying `manifest`.
fn started(
    manifest: &str,
    harness: Harness,
    machine: &Machine,
) -> (tempfile::TempDir, PathBuf, Applied) {
    started_granted(manifest, harness, machine, &grant::Grants::default())
}

/// [`started`], for a chat a person allowed `grants`.
fn started_granted(
    manifest: &str,
    harness: Harness,
    machine: &Machine,
    grants: &grant::Grants,
) -> (tempfile::TempDir, PathBuf, Applied) {
    let plane = tempfile::tempdir().expect("a plane");
    std::fs::write(plane.path().join("charter.toml"), manifest).expect("charter.toml");
    let root = real(plane.path());
    let cwd = root.join("w");
    std::fs::create_dir_all(&cwd).expect("a chat's folder");
    let applied = for_start_granted(harness, &root, machine, &|_| true, None, grants)
        .expect("starts")
        .expect("sandboxed");
    (plane, cwd, applied)
}

/// The profile a wrapped chat in `cwd` starts under, as written.
fn written_in(applied: &Applied, cwd: &Path) -> String {
    let tmp = Path::new("/private/var/folders/xx/chat-tmp");
    let hook = Path::new("/private/var/folders/xx/hooks.sock");
    let floor = applied.level() == Level::Floor;
    match applied.form_in(Some(cwd)) {
        Form::Opencode(wrap) if floor => opencode::floor_profile(&wrap, &[4040], Some(hook)),
        Form::Codex(wrap) if floor => codex::floor_profile(&wrap, &[4040], Some(hook)),
        Form::Opencode(wrap) => opencode::profile(&wrap, cwd, tmp, &[4040], Some(hook)),
        Form::Codex(wrap) => codex::profile(&wrap, cwd, tmp, &[4040], Some(hook)),
        Form::ClaudeWrapped(wrap) => claude_wrap::profile(&wrap, cwd, tmp, &[4040], Some(hook)),
        other => panic!("not a wrap: {other:?}"),
    }
    .expect("a profile")
}

/// [`written_in`], read as purlis's Linux wrap reads it.
fn profile_in(applied: &Applied, cwd: &Path) -> Profile {
    Profile::parse(&written_in(applied, cwd)).expect("it reads")
}

fn reads(profile: &Profile, path: &Path) -> bool {
    profile.allows(&Op::File {
        what: FileOp::ReadData,
        path,
        kind: Kind::Regular,
    })
}

fn makes_folder(profile: &Profile, path: &Path) -> bool {
    profile.allows(&Op::File {
        what: FileOp::Create,
        path,
        kind: Kind::Directory,
    })
}

fn reaches(profile: &Profile, socket: &Path) -> bool {
    profile.allows(&Op::Connect(Remote::Unix(Some(socket))))
}

fn strings(value: &Value) -> Vec<String> {
    value
        .as_array()
        .map(|items| {
            items
                .iter()
                .filter_map(|item| item.as_str().map(str::to_owned))
                .collect()
        })
        .unwrap_or_default()
}

/// What Claude Code's settings for `applied` deny its commands reading, and read again.
fn claude_reads(applied: &Applied) -> (Vec<String>, Vec<String>, Vec<String>) {
    let Form::ClaudeCode(settings) = applied.form() else {
        panic!("not Claude Code's settings: {:?}", applied.form());
    };
    (
        strings(&settings.sandbox["filesystem"]["denyRead"]),
        strings(&settings.sandbox["filesystem"]["allowRead"]),
        settings.deny.clone(),
    )
}

#[test]
fn the_ssh_folder_and_every_named_credential_file_are_the_credentials_class() {
    let (_dir, home) = home();
    let plane = tempfile::tempdir().expect("a plane");
    let denied = Denied::of(plane.path(), &machine(&home, Os::Linux));
    let credentials: Vec<&Denial> = denied
        .paths
        .iter()
        .filter(|denial| denial.class == Class::Credentials)
        .collect();
    let held = |path: &Path| {
        credentials.iter().any(|denial| {
            path.starts_with(&denial.path)
                && denial.access == Access::ReadWrite
                && denial.reach == Reach::AndBelow
        })
    };
    for secret in secrets_in(&home) {
        assert!(
            held(&secret),
            "{} is not denied as a credential: {credentials:#?}",
            secret.display()
        );
    }
    // The folder itself, so a key made in it while the chat runs is held by any name.
    assert!(
        credentials
            .iter()
            .any(|denial| denial.path == home.join(".ssh") && is_the_ssh_folder(denial)),
        "{credentials:#?}"
    );
    // The review of #7008: gcloud's token database and application default credentials,
    // git's XDG credential file, and AWS's SSO and CLI token caches.
    for named in [
        ".config/gcloud/application_default_credentials.json",
        ".config/gcloud/access_tokens.db",
        ".config/git/credentials",
        ".aws/sso/cache/0a1b2c.json",
        ".aws/cli/cache/0a1b2c.json",
    ] {
        assert!(held(&home.join(named)), "{named} is not denied");
    }
}

/// The review of #7008: where a tool's own variable moves its credential file, the moved one is
/// denied too, as the same list already follows `OP_CONFIG_DIR`. A variable that is not an
/// absolute path, or that names the home or a folder above it, moves nothing.
#[test]
fn a_credential_file_a_tools_variable_moves_is_denied_where_it_moved() {
    let (_dir, home) = home();
    let elsewhere = tempfile::tempdir().expect("elsewhere");
    let at = real(elsewhere.path());
    let spelled = |name: &str| at.join(name).display().to_string();
    let (gh, xdg, aws, docker, gcloud, netrc) = (
        spelled("gh"),
        spelled("xdg"),
        spelled("aws-credentials"),
        spelled("docker"),
        spelled("gcloud"),
        spelled("netrc"),
    );
    let machine = machine_with(
        &home,
        Os::Linux,
        &[
            ("GH_CONFIG_DIR", &gh),
            ("XDG_CONFIG_HOME", &xdg),
            ("AWS_SHARED_CREDENTIALS_FILE", &aws),
            ("DOCKER_CONFIG", &docker),
            ("CLOUDSDK_CONFIG", &gcloud),
            ("NETRC", &netrc),
        ],
    );
    let plane = tempfile::tempdir().expect("a plane");
    let denied = Denied::of(plane.path(), &machine);
    let held = |path: &Path| {
        denied
            .paths
            .iter()
            .any(|denial| denial.class == Class::Credentials && path.starts_with(&denial.path))
    };
    for moved in [
        at.join("gh/hosts.yml"),
        at.join("xdg/git/credentials"),
        at.join("xdg/glab-cli/config.yml"),
        at.join("aws-credentials"),
        at.join("docker/config.json"),
        at.join("gcloud/credentials.db"),
        at.join("gcloud/access_tokens.db"),
        at.join("gcloud/application_default_credentials.json"),
        at.join("gcloud/legacy_credentials/ada@example.com/adc.json"),
        at.join("netrc"),
    ] {
        assert!(held(&moved), "{} is not denied", moved.display());
        assert!(is_credential(&moved, &machine), "{}", moved.display());
    }
    // gh follows XDG_CONFIG_HOME where GH_CONFIG_DIR is not said.
    let xdg_only = machine_with(&home, Os::Linux, &[("XDG_CONFIG_HOME", &xdg)]);
    assert!(is_credential(&at.join("xdg/gh/hosts.yml"), &xdg_only));
    // And the defaults stay denied whatever moved.
    assert!(held(&home.join(".config/gh/hosts.yml")));
    let home_spelled = home.display().to_string();
    for (name, value) in [
        ("NETRC", "relative/netrc"),
        ("DOCKER_CONFIG", "/"),
        ("AWS_SHARED_CREDENTIALS_FILE", home_spelled.as_str()),
    ] {
        let machine = machine_with(&home, Os::Linux, &[(name, value)]);
        let denied = Denied::of(plane.path(), &machine);
        assert!(
            !denied
                .paths
                .iter()
                .any(|denial| denial.class == Class::Credentials
                    && (denial.path == Path::new("/")
                        || denial.path == home
                        || denial.path.is_relative())),
            "{name}={value} moved a denial onto {:#?}",
            denied.paths
        );
    }
}

#[test]
fn credentials_is_the_sixth_class_and_lifted_with_the_others() {
    assert_eq!(Class::ALL.len(), 6);
    assert_eq!(Class::Credentials.word(), "credentials");
    assert!(Lifted::CLASSES.contains(&Class::Credentials));
}

#[test]
fn a_claude_code_chat_held_from_credentials_never_reads_one() {
    let (_dir, home) = home();
    let machine = machine(&home, Os::MacOs);
    let ssh = home.join(".ssh").display().to_string();
    for manifest in HELD {
        let (_plane, _cwd, applied) = started(manifest, Harness::ClaudeCode, &machine);
        let level = applied.level();
        let (deny_read, allow_read, deny) = claude_reads(&applied);
        let denied = |path: &str| {
            deny_read.iter().any(|rule| {
                let rule = claude::uncased(rule);
                path == rule || path.starts_with(&format!("{rule}/"))
            })
        };
        let read_again = |path: &str| {
            allow_read.iter().any(|rule| {
                rule == path
                    || rule
                        .strip_suffix("*.pub")
                        .and_then(|folder| path.strip_prefix(folder))
                        .is_some_and(|name| !name.contains('/') && name.ends_with(".pub"))
            })
        };
        for secret in secrets_in(&home) {
            let secret = secret.display().to_string();
            assert!(
                denied(&secret) && !read_again(&secret),
                "{manifest}: {secret} is read: {deny_read:?} {allow_read:?}"
            );
            // And its own Read tool, which Claude Code's sandbox does not hold.
            let held_by = if secret.starts_with(&format!("{ssh}/")) {
                format!("{}/**", &ssh[1..])
            } else {
                secret[1..].to_owned()
            };
            assert!(
                deny.iter()
                    .any(|rule| rule.starts_with("Read(") && rule.contains(&held_by)),
                "{level:?}: no Read rule for {secret}"
            );
        }
        for open in SSH_OPEN_HERE.iter().chain(&["work.pub"]) {
            let open = home.join(".ssh").join(open).display().to_string();
            assert!(
                read_again(&open),
                "{manifest}: {open} is not read: {allow_read:?}"
            );
        }
    }
}

/// Q325: at the floor under the shipped template, `watch`, the person's own gh, https git and
/// cloud tools keep working: nothing of the class is denied, and nothing is read again.
#[test]
fn a_claude_code_chat_at_the_floor_under_watch_reads_the_persons_credentials() {
    let (_dir, home) = home();
    let machine = machine(&home, Os::MacOs);
    let (_plane, _cwd, applied) = started(FLOOR, Harness::ClaudeCode, &machine);
    assert_eq!(applied.level(), Level::Floor);
    let (deny_read, allow_read, deny) = claude_reads(&applied);
    let home_spelled = home.display().to_string();
    for rule in deny_read.iter().chain(&deny) {
        assert!(
            !(claude::uncased(rule).contains(&home_spelled)
                && [
                    ".ssh",
                    ".aws",
                    ".netrc",
                    ".git-credentials",
                    ".azure",
                    "gcloud"
                ]
                .iter()
                .any(|name| rule.contains(name))
                && !rule.starts_with("Edit(")),
            "{rule} holds a credential at the floor"
        );
    }
    assert!(allow_read.is_empty(), "{allow_read:?}");
}

#[test]
fn a_wrapped_chat_held_from_credentials_reads_none_and_what_holds_no_secret_still() {
    let (_dir, home) = home();
    for os in [Os::MacOs, Os::Linux] {
        let machine = machine(&home, os);
        for manifest in HELD {
            for harness in [Harness::Codex, Harness::Opencode] {
                let (_plane, cwd, applied) = started(manifest, harness, &machine);
                let profile = profile_in(&applied, &cwd);
                for secret in secrets_in(&home) {
                    assert!(
                        !reads(&profile, &secret),
                        "{os:?} {harness:?} {manifest}: reads {}",
                        secret.display()
                    );
                }
                if os == Os::MacOs {
                    // macOS's disk folds case: another spelling is the same file.
                    for spelled in [".SSH/id_ed25519", ".Docker/config.json", ".AWS/Credentials"] {
                        assert!(
                            !reads(&profile, &home.join(spelled)),
                            "{harness:?} {manifest}: reads {spelled}"
                        );
                    }
                }
                for open in SSH_OPEN_HERE.iter().chain(&["work.pub"]) {
                    assert!(
                        reads(&profile, &home.join(".ssh").join(open)),
                        "{os:?} {harness:?} {manifest}: cannot read {open}"
                    );
                }
                // A public key's name below a folder in `~/.ssh` is no public key of `ssh`'s.
                assert!(!reads(&profile, &home.join(".ssh/keys/deploy.pub")));
            }
        }
    }
}

/// Q325, and the review of #7008: at the floor under `watch`, a wrapped chat reads the person's
/// credentials as before, and no folder above one is held, so `~/.aws` or `~/.config/gcloud` can
/// be made by the person's own tools in the chat.
#[test]
fn a_wrapped_chat_at_the_floor_under_watch_reads_credentials_and_makes_their_folders() {
    let (_dir, home) = home();
    let bare = tempfile::tempdir().expect("a home with no credentials");
    let bare = real(bare.path());
    for os in [Os::MacOs, Os::Linux] {
        for harness in [Harness::Codex, Harness::Opencode] {
            let (_plane, cwd, applied) = started(FLOOR, harness, &machine(&home, os));
            assert_eq!(applied.level(), Level::Floor);
            let profile = profile_in(&applied, &cwd);
            for secret in secrets_in(&home) {
                assert!(
                    reads(&profile, &secret),
                    "{os:?} {harness:?}: cannot read {}",
                    secret.display()
                );
            }
            let (_plane, cwd, applied) = started(FLOOR, harness, &machine(&bare, os));
            let profile = profile_in(&applied, &cwd);
            // `~/.docker` and `~/.ssh` are held at the floor for their own reasons (#2752, and
            // class 5's `authorized_keys` and `config`), whatever this class.
            for folder in [".aws", ".config/gcloud", ".azure", ".aws/sso"] {
                assert!(
                    makes_folder(&profile, &bare.join(folder)),
                    "{os:?} {harness:?}: cannot make ~/{folder}"
                );
            }
        }
    }
}

#[test]
fn a_claude_code_chat_in_purlis_s_wrap_on_linux_never_reads_a_credential() {
    let (_dir, home) = home();
    let machine = machine(&home, Os::Linux);
    for manifest in HELD {
        let (_plane, cwd, applied) = started(manifest, Harness::ClaudeCode, &machine);
        let profile = profile_in(&applied, &cwd);
        for secret in secrets_in(&home) {
            assert!(
                !reads(&profile, &secret),
                "{manifest}: reads {}",
                secret.display()
            );
        }
        assert!(
            reads(&profile, &home.join(".ssh/id_ed25519.pub")),
            "{manifest}: cannot read a public key"
        );
    }
}

/// The review of #7008: the SSH folder is one rule, so a home with thousands of files in
/// `~/.ssh` starts a chat as one with none does: its profile names none of them, and stays
/// within the size a profile may have.
#[test]
fn a_large_ssh_folder_leaves_the_profile_its_size() {
    let (_dir, home) = home();
    for n in 0..3000 {
        std::fs::write(home.join(".ssh").join(format!("host-{n}_key")), "x").expect("a key");
    }
    for os in [Os::MacOs, Os::Linux] {
        let (_plane, cwd, applied) = started(FULL, Harness::Codex, &machine(&home, os));
        let written = written_in(&applied, &cwd);
        assert!(
            !written.contains("host-"),
            "{os:?}: a key is named in the profile"
        );
        assert!(!reads(
            &Profile::parse(&written).expect("it reads"),
            &home.join(".ssh/host-2999_key")
        ));
        let (_plane, _cwd, applied) = started(FULL, Harness::ClaudeCode, &machine(&home, os));
        if let Form::ClaudeCode(settings) = applied.form() {
            assert!(!settings.sandbox.to_string().contains("host-"), "{os:?}");
        }
    }
}

/// The person's own agent, where `SSH_AUTH_SOCK` names it on macOS: launchd's socket under
/// `/private/tmp` (the review of #7008), and the grant a person's Allow keeps for it. Outside
/// every folder a chat may write, so a grant of it holds. Nothing listens there: a kept grant
/// stands while the agent is not running ([`services::still_sockets`]), and what a chat may
/// reach is read from its rules, not from the socket.
fn agent() -> (PathBuf, grant::Grants) {
    static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let path = PathBuf::from(format!(
        "/private/tmp/com.apple.launchd.Purlis{}x{}",
        std::process::id(),
        NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    ))
    .join("Listeners");
    assert!(!path.exists(), "{} is there already", path.display());
    let grants = grant::Grants {
        sockets: vec![path.clone()],
        ..grant::Grants::default()
    };
    (path, grants)
}

/// launchd's agent socket under `/private/tmp` is one a person may allow: no folder a chat
/// writes holds it, so the grant is kept at every start.
#[test]
fn launchds_agent_socket_under_private_tmp_is_one_a_person_may_allow() {
    let (_dir, home) = home();
    let (agent, _) = agent();
    let plane = tempfile::tempdir().expect("a plane");
    std::fs::write(plane.path().join("charter.toml"), FULL).expect("charter.toml");
    let root = real(plane.path());
    for os in [Os::MacOs, Os::Linux] {
        let ground = services::SocketGround::of(&root, &machine(&home, os), &[]);
        assert_eq!(
            services::still_sockets(std::slice::from_ref(&agent), &ground),
            std::slice::from_ref(&agent),
            "{os:?}"
        );
    }
}

/// Q321 point 27: the person's own agent, the socket `SSH_AUTH_SOCK` names, is reached by no
/// chat until a person allows it, and then by that chat.
#[test]
fn the_persons_own_ssh_agent_is_refused_until_a_person_allows_it() {
    let (_dir, home) = home();
    let (agent, allowed) = agent();
    for (os, harness) in [
        (Os::MacOs, Harness::Codex),
        (Os::Linux, Harness::ClaudeCode),
        (Os::Linux, Harness::Opencode),
    ] {
        let machine = machine(&home, os);
        for manifest in [FULL, FLOOR, FLOOR_ENFORCED] {
            let (_plane, cwd, applied) = started(manifest, harness, &machine);
            let level = applied.level();
            assert!(
                !reaches(&profile_in(&applied, &cwd), &agent),
                "{os:?} at {level:?}: the person's agent is reached ungranted"
            );
            let (_plane, cwd, applied) = started_granted(manifest, harness, &machine, &allowed);
            assert!(
                reaches(&profile_in(&applied, &cwd), &agent),
                "{os:?} at {level:?}: the person's agent is not reached once allowed"
            );
        }
    }
}

/// The same for a Claude Code chat on macOS, in Claude Code's own sandbox.
#[test]
fn a_claude_code_chat_on_macos_reaches_the_persons_agent_only_once_allowed() {
    let (_dir, home) = home();
    let (agent, allowed) = agent();
    let machine = machine(&home, Os::MacOs);
    let sockets = |applied: &Applied| {
        let Form::ClaudeCode(settings) = applied.form() else {
            panic!("not Claude Code's settings");
        };
        assert_eq!(
            settings.sandbox["network"]["allowAllUnixSockets"],
            Value::Bool(false)
        );
        strings(&settings.sandbox["network"]["allowUnixSockets"])
    };
    for manifest in [FULL, FLOOR, FLOOR_ENFORCED] {
        let (_plane, _cwd, applied) = started(manifest, Harness::ClaudeCode, &machine);
        let level = applied.level();
        assert!(sockets(&applied).is_empty(), "{level:?}");
        let (_plane, _cwd, applied) =
            started_granted(manifest, Harness::ClaudeCode, &machine, &allowed);
        assert_eq!(
            sockets(&applied),
            [agent.display().to_string()],
            "{level:?}"
        );
    }
}

/// Q321 and Q325, as the person ruled them: a refused credential says plainly that a forge
/// token comes through purlis's vault, that plain gh or https git on those files stops
/// working, and when that holds; and offers the person's SSH agent only where a socket can be
/// allowed (on macOS, D-sock-7).
#[test]
fn a_refused_credential_says_a_forge_token_comes_through_the_vault() {
    for said in [
        grant::CREDENTIALS_ROUTE,
        grant::CREDENTIALS_ROUTE_WITH_AGENT,
    ] {
        for words in [
            "purlis's vault",
            "plain gh",
            "git over https",
            "stops working",
            "full sandbox",
            "policy",
        ] {
            assert!(said.contains(words), "{words:?} not in {said:?}");
        }
    }
    assert!(!grant::CREDENTIALS_ROUTE.contains("SSH agent"));
    assert!(grant::CREDENTIALS_ROUTE_WITH_AGENT.contains("SSH agent"));
    assert_eq!(
        grant::brokered_route(Class::Credentials),
        if cfg!(target_os = "macos") {
            grant::CREDENTIALS_ROUTE_WITH_AGENT
        } else {
            grant::CREDENTIALS_ROUTE
        }
    );
}

/// What a refused read or write names is told as a credential's, never as a vault's: under any
/// spelling a case-folding disk takes, either name across a firmlink, and below a folder.
#[test]
fn a_refused_credential_is_known_as_one() {
    let home = Path::new("/Users/ada");
    let machine = machine(home, Os::MacOs);
    for credential in [
        "/Users/ada/.ssh/id_ed25519",
        "/Users/ada/.ssh/deploy_github",
        "/Users/ada/.ssh/keys/work",
        "/Users/ada/.ssh/keys/work.pub",
        "/Users/ada/.SSH/ID_ED25519",
        "/System/Volumes/Data/Users/ada/.ssh/id_rsa",
        "/Users/ada/.netrc",
        "/Users/ada/.config/gh/hosts.yml",
        "/Users/ada/.Config/GH/hosts.yml",
        "/Users/ada/.azure/msal_token_cache.json",
        "/Users/ada/.config/gcloud/legacy_credentials/ada@example.com/adc.json",
        "/Users/ada/.aws/sso/cache/0a1b.json",
    ] {
        assert!(
            is_credential(Path::new(credential), &machine),
            "{credential}"
        );
    }
    for open in [
        "/Users/ada/.ssh/id_ed25519.pub",
        "/Users/ada/.ssh/known_hosts",
        "/Users/ada/.ssh/known_hosts.old",
        "/Users/ada/.ssh/config",
        "/Users/ada/.ssh",
        "/Users/ada/.config/gh/config.yml",
        "/Users/ada/.aws/config",
        "/Users/ada/project/.netrc",
        "/Users/ada/.sshx/id_rsa",
    ] {
        assert!(!is_credential(Path::new(open), &machine), "{open}");
    }
}

/// A path is read as a program opens it: `.`, `//` and `..` change no name, so no spelling of a
/// credential is open (`liftable` reads them the same way), and no spelling of an open file is a
/// credential.
#[test]
fn a_credential_is_one_under_every_spelling_a_program_opens_it_by() {
    let machine = machine(Path::new("/Users/ada"), Os::MacOs);
    for credential in [
        "/Users/ada/.config/gh/./hosts.yml",
        "/Users/ada/.config//gh//hosts.yml",
        "/Users/ada/.config/gh/x/../hosts.yml",
        "/Users/ada/./.ssh/id_rsa",
        "/Users/ada/.ssh/../.ssh/id_rsa",
        "/Users/ada/.config/gh/hosts.yml/",
    ] {
        assert!(
            is_credential(Path::new(credential), &machine),
            "{credential}"
        );
    }
    for open in [
        "/Users/ada/.config/gh/../config.yml",
        "/Users/ada/.config/gh/./config.yml",
        "/Users/ada/.ssh/./id_ed25519.pub",
    ] {
        assert!(!is_credential(Path::new(open), &machine), "{open}");
    }
}

/// What each entry of [`CREDENTIAL_FILES`] is, said by a row here and never guessed from its name.
enum Entry {
    /// A forge's own login file: the program that reads it, the kind of forge, and the variable the
    /// program takes its token from.
    Forge(&'static str, crate::forge::Kind, &'static str),
    /// Not a forge's file, with the reason it needs no start note or shim.
    Exempt(&'static str),
}

/// **A forge's credential file never ships without its three answers** (#1349): every entry of
/// [`CREDENTIAL_FILES`] is classified here, as a forge with (i) a program the shim refuses to run
/// without a token, (ii) a start-time note for a chat that holds the class with no route, and
/// (iii) the shim's sentence, or as exempt with a written reason. An entry with no row, or a row
/// with no entry, turns this red.
#[cfg(unix)]
#[test]
fn every_forge_credential_file_has_a_shim_a_start_note_and_a_sentence() {
    use crate::forge::Kind;
    use crate::forge::agentidentity::{HostFound, credentials_note};
    use crate::sandbox::tools::{FORGE_PROGRAMS, Handed, Ready, Tool, no_minted_token_said, shim};
    use std::os::unix::fs::PermissionsExt;
    const GIT: &str = "git's own helper, not a forge's API login: no forge program to shim";
    const CLOUD: &str = "a cloud provider's login, not a forge's: its tools are not shimmed";
    const CLASSIFIED: [(&str, Entry); 14] = [
        (".git-credentials", Entry::Exempt(GIT)),
        (".config/git/credentials", Entry::Exempt(GIT)),
        (
            ".netrc",
            Entry::Exempt("any program's login, no one forge's"),
        ),
        (".aws/credentials", Entry::Exempt(CLOUD)),
        (".aws/sso/cache", Entry::Exempt(CLOUD)),
        (".aws/cli/cache", Entry::Exempt(CLOUD)),
        (".config/gcloud/credentials.db", Entry::Exempt(CLOUD)),
        (".config/gcloud/access_tokens.db", Entry::Exempt(CLOUD)),
        (
            ".config/gcloud/application_default_credentials.json",
            Entry::Exempt(CLOUD),
        ),
        (".config/gcloud/legacy_credentials", Entry::Exempt(CLOUD)),
        (".azure", Entry::Exempt(CLOUD)),
        (
            ".docker/config.json",
            Entry::Exempt("a registry's login, not a forge's"),
        ),
        (
            ".config/gh/hosts.yml",
            Entry::Forge("gh", Kind::GitHub, "GH_TOKEN"),
        ),
        (
            ".config/glab-cli/config.yml",
            Entry::Forge("glab", Kind::GitLab, "GITLAB_TOKEN"),
        ),
    ];
    let mut unclassified: Vec<&str> = CREDENTIAL_FILES
        .iter()
        .copied()
        .filter(|file| !CLASSIFIED.iter().any(|row| row.0 == *file))
        .collect();
    unclassified.sort_unstable();
    assert!(
        unclassified.is_empty(),
        "CREDENTIAL_FILES entries with no class here (a forge, or exempt with a reason): {unclassified:?}"
    );
    for (file, entry) in &CLASSIFIED {
        assert!(
            CREDENTIAL_FILES.contains(file),
            "{file} is no longer listed"
        );
        if let Entry::Exempt(reason) = entry {
            assert!(!reason.is_empty(), "{file}: an exemption needs its reason");
        }
    }
    let forges = CLASSIFIED.iter().filter_map(|(_, entry)| match entry {
        Entry::Forge(program, kind, var) => Some((*program, *kind, *var)),
        Entry::Exempt(_) => None,
    });
    for (program, kind, var) in forges {
        assert!(
            FORGE_PROGRAMS.contains(&program),
            "{program} has no shim rule"
        );
        let found = [HostFound {
            host: "forge.test".into(),
            kind: Some(kind),
        }];
        assert!(
            credentials_note(true, false, &found).is_some(),
            "{program}: no start note"
        );
        // The shim, run with no app and no token: the sentence, a failure, and the program unrun.
        let tmp = tempfile::tempdir().unwrap();
        let ran = tmp.path().join("ran");
        let bin = tmp.path().join(program);
        std::fs::write(&bin, format!("#!/bin/sh\ntouch '{}'\n", ran.display())).unwrap();
        std::fs::set_permissions(&bin, std::fs::Permissions::from_mode(0o700)).unwrap();
        let script = tmp.path().join("shim");
        std::fs::write(
            &script,
            shim(
                Path::new("/purlis"),
                "dev",
                &Ready {
                    tool: Tool::one(program, "forge", var, Handed::Env, "token"),
                    program: bin,
                },
            ),
        )
        .unwrap();
        std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o700)).unwrap();
        let out = crate::forklock::output(
            std::process::Command::new(&script)
                .env_remove("PURLIS_HOOK_SOCKET")
                .env_remove("CHARTER_HOOK_SOCKET")
                .env_remove(var),
        )
        .unwrap();
        assert_eq!(out.status.code(), Some(1), "{program}: {out:?}");
        assert!(
            String::from_utf8_lossy(&out.stderr).contains(&no_minted_token_said(program, false)),
            "{program}: {out:?}"
        );
        assert!(!ran.exists(), "{program} ran with no token");
    }
}
