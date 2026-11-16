// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The person's credential files are never a sandboxed chat's to read** (ADR 0067 §5 class 6,
//! amended 2026-11-16, ruling Q321 points 17 and 27). An SSH private key in `~/.ssh` and each
//! forge and cloud credential file [`CREDENTIAL_FILES`] names are denied at the full sandbox and
//! at the floor, in what every harness is compiled to: Claude Code's settings, the Seatbelt
//! profile of a wrapped chat on macOS, and the profile purlis's own wrap reads on Linux. What
//! sits beside them and holds no secret (`known_hosts`, `config`, a public key) stays readable.
//! The person's own SSH agent is no socket a chat reaches until a person allows it.

use std::path::{Path, PathBuf};

use purlis_linux_sandbox::profile::{FileOp, Kind, Op, Profile, Remote};
use serde_json::Value;

use super::*;

const FULL: &str = "[sandbox]\nmode = \"on\"\negress = [\"model-providers\"]\n";
const FLOOR: &str = "schema = 1\n";

/// What `~/.ssh` holds on the machine these tests make: two keys by the usual names, one by a
/// name of the person's own, and what holds no secret.
const SSH_KEYS: [&str; 3] = ["id_ed25519", "id_rsa", "deploy_github"];
const SSH_OPEN: [&str; 6] = [
    "id_ed25519.pub",
    "known_hosts",
    "known_hosts.old",
    "config",
    "authorized_keys",
    "allowed_signers",
];

fn real(path: &Path) -> PathBuf {
    path.canonicalize().expect("a real path")
}

/// A home with an `~/.ssh` as [`SSH_KEYS`] and [`SSH_OPEN`] say, and every file
/// [`CREDENTIAL_FILES`] names.
fn home() -> (tempfile::TempDir, PathBuf) {
    let dir = tempfile::tempdir().expect("a home");
    let home = real(dir.path());
    let ssh = home.join(".ssh");
    std::fs::create_dir_all(&ssh).expect("~/.ssh");
    for name in SSH_KEYS.iter().chain(SSH_OPEN.iter()) {
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
    Machine {
        env: crate::secrets::Env::of(&[]),
        home: Some(home.to_path_buf()),
        os,
    }
}

/// Every file of `home` a chat must never read.
fn secrets_in(home: &Path) -> Vec<PathBuf> {
    SSH_KEYS
        .iter()
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

/// The profile a wrapped chat in `cwd` starts under, read as purlis's Linux wrap reads it.
fn profile_in(applied: &Applied, cwd: &Path) -> Profile {
    let tmp = Path::new("/private/var/folders/xx/chat-tmp");
    let hook = Path::new("/private/var/folders/xx/hooks.sock");
    let floor = applied.level() == Level::Floor;
    let written = match applied.form_in(Some(cwd)) {
        Form::Opencode(wrap) if floor => opencode::floor_profile(&wrap, &[4040], Some(hook)),
        Form::Codex(wrap) if floor => codex::floor_profile(&wrap, &[4040], Some(hook)),
        Form::Opencode(wrap) => opencode::profile(&wrap, cwd, tmp, &[4040], Some(hook)),
        Form::Codex(wrap) => codex::profile(&wrap, cwd, tmp, &[4040], Some(hook)),
        Form::ClaudeWrapped(wrap) => claude_wrap::profile(&wrap, cwd, tmp, &[4040], Some(hook)),
        other => panic!("not a wrap: {other:?}"),
    }
    .expect("a profile");
    Profile::parse(&written).expect("it reads")
}

fn reads(profile: &Profile, path: &Path) -> bool {
    profile.allows(&Op::File {
        what: FileOp::ReadData,
        path,
        kind: Kind::Regular,
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

#[test]
fn ssh_private_keys_and_the_named_credential_files_are_the_credentials_class() {
    let (_dir, home) = home();
    let plane = tempfile::tempdir().expect("a plane");
    let denied = Denied::of(plane.path(), &machine(&home, Os::Linux));
    let credentials: Vec<&Denial> = denied
        .paths
        .iter()
        .filter(|denial| denial.class == Class::Credentials)
        .collect();
    for secret in secrets_in(&home) {
        assert!(
            credentials
                .iter()
                .any(|denial| denial.path == secret && denial.access == Access::ReadWrite),
            "{} is not denied as a credential: {credentials:#?}",
            secret.display()
        );
    }
    // The usual names are denied whether there or not: a key made while the chat runs is held.
    for usual in ["id_ecdsa", "id_ed25519_sk", "id_dsa", "identity"] {
        let path = home.join(".ssh").join(usual);
        assert!(
            credentials.iter().any(|denial| denial.path == path),
            "{} is not denied",
            path.display()
        );
    }
    for open in SSH_OPEN {
        let path = home.join(".ssh").join(open);
        assert!(
            !denied
                .paths
                .iter()
                .any(|denial| path.starts_with(&denial.path) && denial.access != Access::Write),
            "{} holds no secret and is denied to reads",
            path.display()
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
fn a_claude_code_chat_at_the_full_sandbox_and_the_floor_never_reads_a_credential() {
    let (_dir, home) = home();
    let machine = machine(&home, Os::MacOs);
    for manifest in [FULL, FLOOR] {
        let (_plane, _cwd, applied) = started(manifest, Harness::ClaudeCode, &machine);
        let Form::ClaudeCode(settings) = applied.form() else {
            panic!("not Claude Code's settings: {:?}", applied.form());
        };
        let level = applied.level();
        let deny_read = strings(&settings.sandbox["filesystem"]["denyRead"]);
        for secret in secrets_in(&home) {
            let secret = secret.display().to_string();
            assert!(
                deny_read.iter().any(|rule| claude::uncased(rule) == secret),
                "{level:?}: {secret} not in {deny_read:?}"
            );
            // And its own Read tool, which Claude Code's sandbox does not hold.
            assert!(
                settings
                    .deny
                    .iter()
                    .any(|rule| rule.starts_with("Read(") && rule.contains(&secret[1..])),
                "{level:?}: no Read rule for {secret}"
            );
        }
        for open in SSH_OPEN {
            let open = home.join(".ssh").join(open).display().to_string();
            assert!(
                !deny_read.iter().any(|rule| claude::uncased(rule) == open),
                "{level:?}: {open} is denied"
            );
        }
    }
}

#[test]
fn a_wrapped_chat_on_macos_at_the_full_sandbox_and_the_floor_never_reads_a_credential() {
    let (_dir, home) = home();
    let machine = machine(&home, Os::MacOs);
    for manifest in [FULL, FLOOR] {
        for harness in [Harness::Codex, Harness::Opencode] {
            let (_plane, cwd, applied) = started(manifest, harness, &machine);
            let level = applied.level();
            let profile = profile_in(&applied, &cwd);
            for secret in secrets_in(&home) {
                assert!(
                    !reads(&profile, &secret),
                    "{harness:?} at {level:?} reads {}",
                    secret.display()
                );
            }
            // macOS's disk folds case: another spelling is the same file.
            for spelled in [".SSH/id_ed25519", ".Docker/config.json", ".AWS/Credentials"] {
                assert!(
                    !reads(&profile, &home.join(spelled)),
                    "{harness:?} at {level:?} reads {spelled}"
                );
            }
            assert!(
                reads(&profile, &home.join(".ssh/known_hosts")),
                "{harness:?} at {level:?} cannot read known_hosts"
            );
        }
    }
}

#[test]
fn a_claude_code_chat_in_purlis_s_wrap_on_linux_never_reads_a_credential() {
    let (_dir, home) = home();
    let machine = machine(&home, Os::Linux);
    for manifest in [FULL, FLOOR] {
        let (_plane, cwd, applied) = started(manifest, Harness::ClaudeCode, &machine);
        let level = applied.level();
        let profile = profile_in(&applied, &cwd);
        for secret in secrets_in(&home) {
            assert!(
                !reads(&profile, &secret),
                "{level:?} reads {}",
                secret.display()
            );
        }
        assert!(
            reads(&profile, &home.join(".ssh/id_ed25519.pub")),
            "{level:?} cannot read a public key"
        );
    }
}

/// The person's own agent, where `SSH_AUTH_SOCK` would name it, and the grant a person's Allow
/// keeps for it. Outside every folder a chat may write, the temp folder among them, or the grant
/// is refused (a chat could put any socket there): so at the root, not in a temp folder. Nothing
/// listens there: a kept grant stands while the agent is not running ([`services::still_sockets`]),
/// and what a chat may reach is read from its rules, not from the socket.
fn agent() -> (PathBuf, grant::Grants) {
    static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let path = PathBuf::from(format!(
        "/purlis-test-agent-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    ))
    .join("agent.sock");
    assert!(!path.exists(), "{} is there already", path.display());
    let grants = grant::Grants {
        sockets: vec![path.clone()],
        ..grant::Grants::default()
    };
    (path, grants)
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
    ] {
        let machine = machine(&home, os);
        for manifest in [FULL, FLOOR] {
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
    for manifest in [FULL, FLOOR] {
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

/// Q321, as the person ruled it: a refused credential says plainly that a forge token comes
/// only through purlis's vault, and that plain gh or https git on those files stops working.
#[test]
fn a_refused_credential_says_a_forge_token_comes_through_the_vault() {
    let said = grant::brokered_route(Class::Credentials);
    for words in [
        "purlis's vault",
        "plain gh",
        "git over https",
        "stops working",
    ] {
        assert!(said.contains(words), "{words:?} not in {said:?}");
    }
}

/// What a refused read names is told as a credential's, never as a vault's.
#[test]
fn a_refused_read_of_a_credential_is_known_as_one() {
    let home = Path::new("/Users/ada");
    for credential in [
        ".ssh/id_ed25519",
        ".ssh/deploy_github",
        ".netrc",
        ".config/gh/hosts.yml",
        ".azure/msal_token_cache.json",
        ".config/gcloud/legacy_credentials/ada@example.com/adc.json",
    ] {
        assert!(is_credential(&home.join(credential), home), "{credential}");
    }
    for open in [
        ".ssh/id_ed25519.pub",
        ".ssh/known_hosts",
        ".ssh/config",
        ".config/gh/config.yml",
        ".aws/config",
        "project/.netrc",
    ] {
        assert!(!is_credential(&home.join(open), home), "{open}");
    }
}
