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
            // And its own Read tool, which Claude Code's sandbox does not hold: a rule on it or
            // on a folder above it, as Claude Code reads one (#6961).
            assert!(
                claude::tool_holds(&deny, "Read", &format!("/{secret}")),
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

/// #7205: a chat that holds the person's credentials claims the forge lock, and its line hands it
/// no path to their forge login; one at the floor under `watch`, which holds none and has no mint,
/// keeps its own login on purpose and is handed nothing of the lock.
#[cfg(not(feature = "forge-wiring"))]
#[test]
fn a_chat_that_holds_the_credentials_class_is_started_with_the_forge_lock() {
    let (_home, home) = home();
    let machine = machine(&home, Os::MacOs);
    for harness in Harness::ALL {
        for manifest in HELD {
            let (_plane, cwd, applied) = started(manifest, harness, &machine);
            assert!(applied.forge_locked(), "{harness:?} {manifest}");
            // A wrapped harness is not started without its proxy, which a test has none of.
            if harness != Harness::ClaudeCode {
                continue;
            }
            let line = applied
                .line(
                    Words {
                        program: "chat".to_owned(),
                        command: Vec::new(),
                        armed: Vec::new(),
                        charters: Vec::new(),
                    },
                    &At {
                        cwd: Some(&cwd),
                        ..At::default()
                    },
                )
                .expect("starts");
            let set = |name: &str| {
                line.env
                    .iter()
                    .rev()
                    .find(|(key, _)| key == name)
                    .map(|(_, value)| value.as_str())
            };
            // No folder of the chat's own here to write the lock's files in, so each reads as
            // nothing at all.
            assert_eq!(set("GH_CONFIG_DIR"), Some("/dev/null"), "{harness:?}");
            assert_eq!(set("GIT_CONFIG_GLOBAL"), Some("/dev/null"), "{harness:?}");
            assert_eq!(set("GIT_CONFIG_SYSTEM"), Some("/dev/null"), "{harness:?}");
            assert_eq!(set("GIT_TERMINAL_PROMPT"), Some("0"), "{harness:?}");
            assert_eq!(set("SSH_AUTH_SOCK"), Some(""), "{harness:?}");
            // Claude Code's own sandbox sets git's ssh for each command, so its line names none.
            assert_eq!(set("GIT_SSH_COMMAND"), None, "{harness:?}");
        }
        let (_plane, cwd, applied) = started(FLOOR, harness, &machine);
        assert!(
            !applied.forge_locked(),
            "{harness:?} at the floor under watch"
        );
        if harness != Harness::ClaudeCode {
            continue;
        }
        let line = applied
            .line(
                Words {
                    program: "chat".to_owned(),
                    command: Vec::new(),
                    armed: Vec::new(),
                    charters: Vec::new(),
                },
                &At {
                    cwd: Some(&cwd),
                    ..At::default()
                },
            )
            .expect("starts");
        assert!(
            !line.env.iter().any(|(key, _)| key == "GH_CONFIG_DIR"),
            "{harness:?}"
        );
    }
}

/// #7205: the config purlis compiles for a chat that claims the forge lock has the harness's
/// forge connector off, and one that does not claim it is compiled as it was.
#[cfg(not(feature = "forge-wiring"))]
#[test]
fn a_chat_that_claims_the_forge_lock_is_compiled_with_its_forge_connectors_off() {
    use crate::harness::{Kit, StateHooks};
    let (_home, home) = home();
    let machine = machine(&home, Os::MacOs);
    let plugin = tempfile::tempdir().expect("a plugin folder");
    let shim = crate::opencode::shim_in(plugin.path());
    std::fs::create_dir_all(shim.parent().expect("a folder")).expect("its folder");
    std::fs::write(&shim, "// shim").expect("the shim");
    let kit = Kit {
        skills: None,
        laid_out: None,
        binary: Path::new("/bin/charter"),
        plugin: Some(plugin.path()),
        persona: None,
        graded: false,
        judging: false,
        writes_its_piece: false,
        runs_a_shell: false,
        own: &[],
    };
    let compiled = |harness: Harness, manifest: &str| {
        let (_plane, cwd, applied) = started(manifest, harness, &machine);
        let StateHooks::ThisSessionOnly { args, env, .. } = harness.state_hooks(
            kit,
            Some(&cwd),
            &crate::harness_plugin::Chosen::new(),
            Some(&applied),
        ) else {
            panic!("{harness:?} is armed per session");
        };
        (args, env)
    };
    // Claude Code keeps every server it had, purlis's own and the person's among them: only a
    // forge connector's tools are denied, in the settings it is handed.
    let (args, _) = compiled(Harness::ClaudeCode, FULL);
    assert!(
        !args.iter().any(|arg| arg == "--strict-mcp-config"),
        "{args:?}"
    );
    let at = args
        .iter()
        .position(|arg| arg == "--mcp-config")
        .expect("--mcp-config");
    let servers: Value = serde_json::from_str(&args[at + 1]).expect("JSON");
    assert!(
        servers["mcpServers"][crate::chattools::SERVER].is_object(),
        "purlis's own server stays: {servers}"
    );
    let at = args
        .iter()
        .position(|arg| arg == "--settings")
        .expect("--settings");
    let settings: Value = serde_json::from_str(&args[at + 1]).expect("JSON");
    let deny: Vec<&str> = settings["permissions"]["deny"]
        .as_array()
        .expect("deny rules")
        .iter()
        .filter_map(Value::as_str)
        .collect();
    for name in crate::forgelock::CONNECTORS {
        assert!(deny.contains(&format!("mcp__{name}").as_str()), "{name}");
    }
    assert!(
        !deny.contains(&format!("mcp__{}", crate::chattools::SERVER).as_str()),
        "purlis's own is not denied"
    );
    let (args, _) = compiled(Harness::ClaudeCode, FLOOR);
    assert!(
        !args.iter().any(|arg| arg.contains("mcp__github")),
        "{args:?}"
    );
    let config = |env: &[(String, String)]| -> Value {
        let (_, text) = env
            .iter()
            .find(|(name, _)| name == crate::opencode::CONFIG_ENV)
            .expect("its session config");
        serde_json::from_str(text).expect("JSON")
    };
    let (_, env) = compiled(Harness::Opencode, FULL);
    assert_eq!(config(&env)["mcp"]["github"]["enabled"], false);
    assert!(
        config(&env)["mcp"]["purlis"].is_object(),
        "purlis's own stays"
    );
    let (_, env) = compiled(Harness::Opencode, FLOOR);
    assert!(config(&env)["mcp"]["github"].is_null());
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

#[cfg(not(feature = "forge-wiring"))]
/// What each entry of [`CREDENTIAL_FILES`] is, said by a row here and never guessed from its name.
enum Entry {
    /// A forge's own login file: the program that reads it, the kind of forge, the variable the
    /// program takes its token from, the words of the setup flow every answer names, and whether
    /// an undeclared program is guarded ([`GUARDED_FORGES`]).
    Forge(
        &'static str,
        crate::forge::Kind,
        &'static str,
        &'static str,
        bool,
    ),
    /// Not a forge's file, with the reason it needs no start note or shim.
    Exempt(&'static str),
}

/// **A forge's credential file never ships without its answers** (#1349): every entry of
/// [`CREDENTIAL_FILES`] is classified here, as exempt with a written reason or as a forge that has
///
/// 1. a **route**: a declared tool that gives the chat a token and reaches the forge's host
///    ([`forge_routes`]), the one thing a chat under the class takes in place of the person's login;
/// 2. a **start check**: the note a chat that holds the class with no route is told, naming the
///    forge's own flow, and said no more where a route to that very host stands;
/// 3. a **shim answer** where the tool is declared but there is no token, naming the way, exit 1;
/// 4. an **undeclared answer** where no tool is, for a forge whose program a chat under the class
///    only fails on ([`GUARDED_FORGES`]): its flow, exit 1, the program never run;
/// 5. a **setup flow** the answers name, that the person can run.
///
/// An entry with no row, a row with no entry, or a row with an answer taken away, turns this red.
#[cfg(unix)]
#[cfg(not(feature = "forge-wiring"))]
#[test]
fn every_forge_credential_file_has_a_route_a_start_check_a_shim_answer_and_a_setup_flow() {
    use crate::forge::Kind;
    use crate::forge::agentidentity::{HostFound, credentials_note};
    use crate::sandbox::tools::{
        FORGE_PROGRAMS, GUARDED_FORGES, Handed, Ready, Tool, forge_routes, gitlab_flow,
        guard_forges, no_minted_token_said, shim,
    };
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
            Entry::Forge("gh", Kind::GitHub, "GH_TOKEN", "Agent identity", false),
        ),
        (
            ".config/glab-cli/config.yml",
            Entry::Forge(
                "glab",
                Kind::GitLab,
                "GITLAB_TOKEN",
                "purlis gitlab access",
                true,
            ),
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
        Entry::Forge(program, kind, var, flow, guarded) => {
            Some((*program, *kind, *var, *flow, *guarded))
        }
        Entry::Exempt(_) => None,
    });
    let run = |script: &Path, var: &str| {
        crate::forklock::output(
            std::process::Command::new(script)
                .env_remove("PURLIS_HOOK_SOCKET")
                .env_remove("CHARTER_HOOK_SOCKET")
                .env_remove(var),
        )
        .unwrap()
    };
    let executable = |path: &Path, text: String| {
        std::fs::write(path, text).unwrap();
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o700)).unwrap();
    };
    for (program, kind, var, flow, guarded) in forges {
        // 5. the setup flow, which the answers below name.
        assert!(
            FORGE_PROGRAMS.contains(&program),
            "{program} has no shim rule"
        );
        assert_eq!(GUARDED_FORGES.contains(&program), guarded, "{program}");
        let tmp = tempfile::tempdir().unwrap();
        let ran = tmp.path().join("ran");
        let bin = tmp.path().join(program);
        executable(&bin, format!("#!/bin/sh\ntouch '{}'\n", ran.display()));
        // 1. the route: the tool declared, as the forge's own host.
        let mut tool = Tool::one(program, "forge", var, Handed::Env, "token");
        tool.host = Some("forge.test".to_owned());
        let ready = Ready {
            tool,
            program: bin.clone(),
        };
        let glab = kind == Kind::GitLab;
        assert_eq!(
            forge_routes(std::slice::from_ref(&ready)),
            [(glab, "forge.test".to_owned())],
            "{program}: no route"
        );
        // 2. the start check, said with no route and silent with one to that very host.
        let found = [HostFound {
            host: "forge.test".into(),
            kind: Some(kind),
        }];
        let note = credentials_note(true, &[], &found, Some("dev")).expect("a start note");
        assert!(note.contains(flow), "{program}: {note}");
        assert_eq!(
            credentials_note(
                true,
                &[(glab, "forge.test".to_owned())],
                &found,
                Some("dev")
            ),
            None,
            "{program}: a route to the host still gets the note"
        );
        assert!(
            credentials_note(
                true,
                &[(glab, "other.test".to_owned())],
                &found,
                Some("dev")
            )
            .is_some(),
            "{program}: a route to another host serves this one"
        );
        // 3. the shim of the declared tool, run with no app and no token: the sentence, a
        // failure, and the program unrun.
        let script = tmp.path().join("shim");
        executable(&script, shim(Path::new("/purlis"), "dev", &ready));
        let out = run(&script, var);
        assert_eq!(out.status.code(), Some(1), "{program}: {out:?}");
        let said = String::from_utf8_lossy(&out.stderr).into_owned();
        assert!(
            said.contains(&no_minted_token_said(program, false)),
            "{program}: {said}"
        );
        assert!(!ran.exists(), "{program} ran with no token");
        // 4. the undeclared program's shim, where it is guarded.
        if guarded {
            let resolved = guard_forges(None, tmp.path(), "dev", &["forge.test".to_owned()], &[])
                .expect("a guard");
            let [(name, sentence)] = resolved.guards.as_slice() else {
                panic!("{:?}", resolved.guards)
            };
            assert_eq!(name, program);
            assert!(sentence.contains(flow), "{program}: {sentence}");
            assert!(
                sentence.contains(&gitlab_flow("dev", Some("forge.test"))),
                "{program}: {sentence}"
            );
            let guard = tmp.path().join("guard");
            executable(
                &guard,
                crate::sandbox::tools::guard_shim("dev", name, sentence),
            );
            let out = run(&guard, var);
            assert_eq!(out.status.code(), Some(1), "{program}: {out:?}");
            assert!(
                String::from_utf8_lossy(&out.stderr).contains(sentence.as_str()),
                "{program}: {out:?}"
            );
            assert!(!ran.exists(), "{program} ran undeclared");
        }
    }
}

/// #7205 beside #1349: the forge lock empties `gh`'s config folder, so `gh` may never read the
/// `hosts.yml` whose refusal raises the credentials Notice. A locked chat with no mint is told
/// from its start instead (the tab's note, which reads the class and the mint, not a failed read),
/// and the shim of a chat with a mint answers with its sentence and runs nothing, whatever its
/// config folder holds.
#[cfg(not(feature = "forge-wiring"))]
#[test]
fn a_locked_chat_with_no_token_is_told_so_without_gh_reading_its_config() {
    use crate::forge::Kind;
    use crate::forge::agentidentity::{HostFound, credentials_note};
    use crate::sandbox::tools::{Handed, Ready, Tool, no_minted_token_said, shim};
    use std::os::unix::fs::PermissionsExt;

    let (_home, home) = home();
    let machine = machine(&home, Os::MacOs);
    let found = [HostFound {
        host: "github.com".into(),
        kind: Some(Kind::GitHub),
    }];
    for manifest in HELD {
        let (_plane, _cwd, applied) = started(manifest, Harness::ClaudeCode, &machine);
        assert!(applied.forge_locked(), "{manifest}");
        let confines = applied.confines();
        let holds = confines
            .denied
            .iter()
            .any(|denial| denial.class == Class::Credentials);
        let routes = crate::sandbox::tools::forge_routes(&confines.tools);
        let note = credentials_note(holds, &routes, &found, Some("dev"))
            .unwrap_or_else(|| panic!("a locked chat with no mint says so: {manifest}"));
        assert!(note.contains("Agent identity"), "{note}");
    }

    let tmp = tempfile::tempdir().unwrap();
    let ran = tmp.path().join("ran");
    let bin = tmp.path().join("gh");
    std::fs::write(&bin, format!("#!/bin/sh\ntouch '{}'\n", ran.display())).unwrap();
    std::fs::set_permissions(&bin, std::fs::Permissions::from_mode(0o700)).unwrap();
    let script = tmp.path().join("shim");
    std::fs::write(
        &script,
        shim(
            Path::new("/purlis"),
            "dev",
            &Ready {
                tool: Tool::one("gh", "forge", "GH_TOKEN", Handed::Env, "token"),
                program: bin,
            },
        ),
    )
    .unwrap();
    std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o700)).unwrap();
    let empty = tempfile::tempdir().unwrap();
    let out = crate::forklock::output(
        std::process::Command::new(&script)
            .env("GH_CONFIG_DIR", empty.path())
            .env_remove("PURLIS_HOOK_SOCKET")
            .env_remove("CHARTER_HOOK_SOCKET")
            .env_remove("GH_TOKEN"),
    )
    .unwrap();
    assert_eq!(out.status.code(), Some(1), "{out:?}");
    assert!(
        String::from_utf8_lossy(&out.stderr).contains(&no_minted_token_said("gh", false)),
        "{out:?}"
    );
    assert!(!ran.exists(), "gh ran with no token");
}
