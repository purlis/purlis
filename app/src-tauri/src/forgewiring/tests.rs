// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! The held acts' context and the push gate (one login K3): which repo a push is of, who may be
//! acted for, and how many pushes are read at once.

use std::path::PathBuf;
use std::sync::Arc;

use purlis_core::forge::Kind;
use purlis_core::forge::confirmed::ConfirmedRepo;
use purlis_core::forge::proxy::listener::{PushGate, PushRead, PushUpdate, PushVerdict};
use purlis_core::forge::proxy::{
    ChatId, Credential, CredentialError, CredentialRequest, CredentialSource, ForgeProxy, NoObjects,
};
use purlis_core::heldact::HeldContext;
use purlis_core::mergehold::Repo;

use super::*;

fn entry(host: &str, path: &str) -> (ConfirmedRepo, PathBuf) {
    (
        ConfirmedRepo {
            clone_key: "clone".to_owned(),
            kind: if host.contains("gitlab") {
                Kind::GitLab
            } else {
                Kind::GitHub
            },
            host: host.to_owned(),
            path: path.to_owned(),
            repo_id: 7,
            account: "a1".to_owned(),
            client_id: String::new(),
        },
        PathBuf::from("/clone"),
    )
}

#[test]
fn a_path_confirmed_on_one_host_is_the_repo_there_and_not_at_another_hosts_address() {
    let (repo, _) = pick(vec![entry("github.com", "acme/app")], "acme/app").expect("one repo");

    assert!(is_at(&repo, "github.com", 443));
    assert!(
        !is_at(&repo, "gitlab.com", 443),
        "a push to GitLab is not the push of the GitHub repo of the same path"
    );
    assert!(
        !is_at(&repo, "github.com", 8443),
        "a port is part of the address"
    );
}

#[test]
fn a_path_confirmed_on_two_forges_is_refused_for_a_path_cannot_tell_them_apart() {
    let both = vec![
        entry("github.com", "acme/app"),
        entry("gitlab.com", "acme/app"),
    ];

    let refused = pick(both, "acme/app").expect_err("refused");

    assert!(refused.contains("more than one forge"), "{refused}");
}

#[test]
fn a_path_confirmed_for_two_clones_of_one_host_is_one_repo() {
    let mut second = entry("github.com", "Acme/App");
    second.1 = PathBuf::from("/other");

    let picked = pick(vec![entry("github.com", "acme/app"), second], "acme/app");

    assert!(picked.is_ok());
}

#[test]
fn a_path_nothing_confirms_is_refused() {
    let refused = pick(vec![entry("github.com", "acme/app")], "acme/other").expect_err("refused");

    assert!(refused.contains("not a repo confirmed"), "{refused}");
}

#[test]
fn no_more_than_four_quarantine_fetches_run_at_once_and_a_finished_one_gives_its_place_back() {
    let quarantines = Quarantines::default();

    let held: Vec<_> = (0..MOST_FETCHING)
        .map(|_| quarantines.begin_fetching().expect("a place"))
        .collect();

    assert!(quarantines.begin_fetching().is_none(), "the fifth waits");
    drop(held);
    assert!(
        quarantines.begin_fetching().is_some(),
        "places are given back"
    );
}

/// A set of sessions: the chat named `chat` is session 7, and it is open where `open` says.
struct Sessions {
    open: bool,
}

impl SessionOfRun for Sessions {
    fn session(&self, run: &str) -> Option<u32> {
        (self.open && run == "chat").then_some(7)
    }
    fn is_open(&self, session: u32) -> bool {
        self.open && session == 7
    }
}

struct NoCredentials;

impl CredentialSource for NoCredentials {
    fn credential_for(&self, _: &CredentialRequest) -> Result<Credential, CredentialError> {
        Err(CredentialError::Unavailable)
    }
}

fn context(open: bool, config_root: &std::path::Path) -> Arc<ProjectContext> {
    Arc::new(ProjectContext {
        project: PathBuf::from("/no/such/project"),
        config_root: config_root.to_path_buf(),
        sessions: Arc::new(Sessions { open }),
        quarantines: Arc::new(Quarantines::default()),
        held: std::sync::Weak::new(),
    })
}

fn gate(context: Arc<ProjectContext>) -> CiPushGate {
    CiPushGate {
        context,
        proxy: Arc::new(ForgeProxy::new(
            Box::new(NoCredentials),
            Box::new(NoObjects),
        )),
        quarantines: Arc::new(Quarantines::default()),
        parent: PathBuf::from("/no/such/quarantine"),
        cache_parent: None,
    }
}

fn push_of(host: &str, repo: &str) -> PushRead {
    PushRead {
        host: host.to_owned(),
        port: 443,
        repo: repo.to_owned(),
        updates: vec![PushUpdate {
            old: "0".repeat(40),
            new: "1".repeat(40),
            name: "refs/heads/work".to_owned(),
        }],
        pack: Vec::new(),
    }
}

fn said(verdict: PushVerdict) -> String {
    match verdict {
        PushVerdict::Refuse(why) => why,
        other => panic!("not refused: {other:?}"),
    }
}

#[test]
fn a_press_acts_for_a_chat_only_while_its_session_is_open() {
    let home = tempfile::tempdir().unwrap();
    let repo = Repo::try_from("acme/app").unwrap();

    let closed = context(false, home.path()).world(7, &repo).err();

    assert_eq!(closed.as_deref(), Some("this chat is not open"));
}

#[test]
fn a_push_of_a_chat_that_is_not_open_is_not_sent() {
    let home = tempfile::tempdir().unwrap();
    let gate = gate(context(false, home.path()));

    let why = said(gate.check(&ChatId::new("chat"), &push_of("github.com", "acme/app")));

    assert!(why.contains("not open"), "{why}");
}

#[test]
fn a_push_to_a_repo_nothing_confirms_is_not_sent_whatever_host_it_names() {
    let home = tempfile::tempdir().unwrap();
    let gate = gate(context(true, home.path()));

    for host in ["github.com", "gitlab.com"] {
        let why = said(gate.check(&ChatId::new("chat"), &push_of(host, "acme/app")));

        assert!(why.contains("not a repo confirmed"), "{host}: {why}");
    }
}

// ---- a push to a branch the forge has already (a rebase), read through the gate ----

use purlis_core::heldact::{BaseObjects, Forked, HeldForge, TagName};

/// A forge that answers two tips: the default branch's, and the one branch's.
struct TwoTips {
    default_tip: HeadSha,
    branch_tip: HeadSha,
}

impl HeldForge for TwoTips {
    fn kind(&self) -> Kind {
        Kind::GitHub
    }
    fn host(&self) -> String {
        "github.com".to_owned()
    }
    fn default_branch(&self, _: &Repo) -> Result<(Base, HeadSha), String> {
        Ok((Base::try_from("main").unwrap(), self.default_tip.clone()))
    }
    fn branch_tip(&self, _: &Repo, _: &Base) -> Result<Option<HeadSha>, String> {
        Ok(Some(self.branch_tip.clone()))
    }
    fn ci_config_path(&self, _: &Repo) -> Result<Option<String>, String> {
        Ok(None)
    }
    fn rev_commit(&self, _: &Repo, _: &str) -> Result<HeadSha, String> {
        unreachable!()
    }
    fn on_branch(&self, _: &Repo, _: &Base, _: &HeadSha) -> Result<bool, String> {
        unreachable!()
    }
    fn tag_at(&self, _: &Repo, _: &TagName) -> Result<Option<HeadSha>, String> {
        unreachable!()
    }
    fn source_id(&self, _: &Repo) -> Result<String, String> {
        unreachable!()
    }
    fn account(&self) -> Result<String, String> {
        unreachable!()
    }
    fn may_fork(&self, _: &Repo) -> Result<bool, String> {
        unreachable!()
    }
    fn create_tag(&self, _: &Repo, _: &TagName, _: &HeadSha) -> Result<(), String> {
        unreachable!()
    }
    fn create_release(
        &self,
        _: &Repo,
        _: &TagName,
        _: &str,
        _: &str,
        _: bool,
        _: bool,
    ) -> Result<(), String> {
        unreachable!()
    }
    fn make_fork(&self, _: &Repo, _: &str) -> Result<Forked, String> {
        unreachable!()
    }
    fn push_tip(
        &self,
        _: &Repo,
        _: &Base,
        _: &HeadSha,
        _: &HeadSha,
        _: bool,
        _: Option<&purlis_core::secrets::keyring::Secret>,
    ) -> Result<(), purlis_core::heldact::PushError> {
        unreachable!()
    }
}

/// The forge's objects, from a local repository (the real source is https only).
struct LocalForge(PathBuf);

impl BaseObjects for LocalForge {
    fn fetch(&self, into: &std::path::Path, base: &HeadSha) -> Result<(), String> {
        let out = git_in(
            into,
            &[
                "-c",
                "protocol.file.allow=always",
                "fetch",
                "--quiet",
                "--no-tags",
                &self.0.to_string_lossy(),
                base.as_str(),
            ],
        );
        out.map(|_| ())
    }

    fn advertised(&self, _: &std::path::Path) -> Option<Vec<String>> {
        let out = git_in(
            &self.0,
            &["for-each-ref", "--format=%(objectname)", "refs/heads"],
        )
        .ok()?;
        Some(
            String::from_utf8(out)
                .ok()?
                .lines()
                .map(str::to_owned)
                .collect(),
        )
    }
}

fn git_in(dir: &std::path::Path, args: &[&str]) -> Result<Vec<u8>, String> {
    let out = purlis_core::forklock::output(
        std::process::Command::new("git")
            .current_dir(dir)
            .args(["-c", "user.name=t", "-c", "user.email=t@example.invalid"])
            .args([
                "-c",
                "commit.gpgsign=false",
                "-c",
                "core.hooksPath=/dev/null",
            ])
            .args(args)
            .env("GIT_CONFIG_GLOBAL", "/dev/null")
            .env("GIT_CONFIG_SYSTEM", "/dev/null"),
    )
    .map_err(|e| e.to_string())?;
    if out.status.success() {
        Ok(out.stdout)
    } else {
        Err(String::from_utf8_lossy(&out.stderr).into_owned())
    }
}

fn commit_with(dir: &std::path::Path, files: &[(&str, &[u8])], message: &str) -> HeadSha {
    for (name, bytes) in files {
        let path = dir.join(name);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, bytes).unwrap();
    }
    git_in(dir, &["add", "-A"]).unwrap();
    git_in(dir, &["commit", "-q", "-m", message]).unwrap();
    let head = git_in(dir, &["rev-parse", "HEAD"]).unwrap();
    HeadSha::try_from(String::from_utf8(head).unwrap().trim().to_owned()).unwrap()
}

/// The thin pack a client sends for `tip` when the forge advertises `have`.
fn thin_pack(dir: &std::path::Path, tip: &HeadSha, have: &[&HeadSha]) -> Vec<u8> {
    let mut input = format!("{}\n", tip.as_str());
    for h in have {
        input.push_str(&format!("^{}\n", h.as_str()));
    }
    let mut child = purlis_core::forklock::spawn(
        std::process::Command::new("git")
            .current_dir(dir)
            .args(["pack-objects", "--stdout", "--revs", "--thin", "-q"])
            .env("GIT_CONFIG_GLOBAL", "/dev/null")
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped()),
    )
    .unwrap();
    std::io::Write::write_all(&mut child.stdin.take().unwrap(), input.as_bytes()).unwrap();
    child.wait_with_output().unwrap().stdout
}

const CI: &str = ".github/workflows/ci.yml";

/// A forge whose branch `task` is at an old tip built on an old default branch, and whose default
/// branch has moved on, and the chat's rebase of `task` onto it: a push that is not a
/// fast-forward, over a branch that is there.
struct Rebase {
    _dir: tempfile::TempDir,
    forge: PathBuf,
    default_tip: HeadSha,
    old_tip: HeadSha,
}

fn rebase_scene() -> Rebase {
    let dir = tempfile::tempdir().unwrap();
    let forge = std::fs::canonicalize(dir.path()).unwrap();
    git_in(&forge, &["init", "-q", "-b", "main", "."]).unwrap();
    let m0 = commit_with(&forge, &[(CI, b"v1\n"), ("README", b"one\n")], "m0");
    // The branch's old tip on the forge: its own CI file is binary, so a diff against it cannot be
    // shown, while a diff against the default branch's can.
    git_in(&forge, &["checkout", "-q", "-b", "task"]).unwrap();
    let old_tip = commit_with(&forge, &[(CI, b"bin\0\n"), ("a.txt", b"a\n")], "t0");
    git_in(&forge, &["checkout", "-q", "main"]).unwrap();
    let default_tip = commit_with(&forge, &[(CI, b"v2\n")], "m1");
    assert_ne!(m0, default_tip);
    Rebase {
        _dir: dir,
        forge,
        default_tip,
        old_tip,
    }
}

impl Rebase {
    /// The chat's rebase: a commit on the default tip, with `ci` as the CI file.
    fn rebased(&self, ci: &[u8]) -> (HeadSha, Vec<u8>) {
        git_in(&self.forge, &["checkout", "-q", "-B", "rebased", "main"]).unwrap();
        let tip = commit_with(&self.forge, &[(CI, ci), ("a.txt", b"a\n")], "t1");
        git_in(&self.forge, &["checkout", "-q", "main"]).unwrap();
        let pack = thin_pack(&self.forge, &tip, &[&self.old_tip, &self.default_tip]);
        (tip, pack)
    }

    fn read(&self, tip: &HeadSha, pack: &[u8]) -> Result<(), PushVerdict> {
        self.read_keeping(tip, pack).0
    }

    /// [`Self::read`], and whether a quarantine of the push is kept after it.
    fn read_keeping(&self, tip: &HeadSha, pack: &[u8]) -> (Result<(), PushVerdict>, bool) {
        let home = tempfile::tempdir().unwrap();
        let parent = tempfile::tempdir().unwrap();
        let gate = CiPushGate {
            parent: parent.path().to_path_buf(),
            ..gate(context(true, home.path()))
        };
        let forge = TwoTips {
            default_tip: self.default_tip.clone(),
            branch_tip: self.old_tip.clone(),
        };
        let read = gate.read_update(
            &ChatId::new("chat"),
            7,
            &forge,
            &LocalForge(self.forge.clone()),
            &Repo::try_from("acme/app").unwrap(),
            &Update {
                branch: Base::try_from("task").unwrap(),
                tip: tip.clone(),
                old: self.old_tip.as_str(),
                created: false,
                pack,
            },
        );
        let kept = gate.quarantines.of(7, "acme/app").is_some();
        (read, kept)
    }
}

#[test]
fn a_rebase_over_a_branch_that_is_there_changes_no_ci_file_when_its_files_are_the_default_branchs()
{
    let scene = rebase_scene();
    let (tip, pack) = scene.rebased(b"v2\n");

    let read = scene.read(&tip, &pack);

    assert!(read.is_ok(), "{read:?}");
}

#[test]
fn a_rebase_that_changes_a_ci_file_is_read_against_the_default_branchs_tip_not_the_branchs_old_one()
{
    let scene = rebase_scene();
    let (tip, pack) = scene.rebased(b"v3\n");

    let why = match scene.read(&tip, &pack) {
        Err(PushVerdict::Refuse(why)) => why,
        other => panic!("not refused: {other:?}"),
    };

    // The change is held for the person (no sink is on here, so it is said so), and it is shown
    // as a text diff against the default branch's tip. Against the branch's old tip, whose file is
    // binary, it could not be shown at all, and would be refused as binary.
    assert!(why.contains("held acts are not on here"), "{why}");
    assert!(!why.contains("binary"), "{why}");
}

#[test]
fn a_held_ci_push_nobody_waits_on_keeps_no_quarantine() {
    // #7685 review (F6): no sink is on here, so no ask waits on the push, and the repository it
    // was read in is not kept until the chat closes.
    let scene = rebase_scene();
    let (tip, pack) = scene.rebased(b"v3\n");

    let (read, kept) = scene.read_keeping(&tip, &pack);

    assert!(matches!(read, Err(PushVerdict::Refuse(_))), "{read:?}");
    assert!(!kept, "the quarantine of a push nobody waits on is let go");
}

// ---- a push that sends no new objects: its tip must be in a branch of this repo ----

impl Rebase {
    /// A commit the forge resolves by id and no branch holds: it was on a branch since deleted.
    fn dangling(&self) -> HeadSha {
        git_in(&self.forge, &["checkout", "-q", "-b", "gone", "main"]).unwrap();
        let tip = commit_with(
            &self.forge,
            &[("secret.txt", b"a secret\n")],
            "force-pushed away",
        );
        git_in(&self.forge, &["checkout", "-q", "main"]).unwrap();
        git_in(&self.forge, &["branch", "-q", "-D", "gone"]).unwrap();
        tip
    }

    /// The empty pack git sends when the forge has every object of `tip`.
    fn empty(&self) -> Vec<u8> {
        thin_pack(&self.forge, &self.default_tip, &[&self.default_tip])
    }
}

#[test]
fn an_empty_pack_naming_a_commit_a_branch_of_the_repo_holds_is_read_through_the_gate() {
    let scene = rebase_scene();

    let read = scene.read(&scene.default_tip, &scene.empty());

    assert!(read.is_ok(), "{read:?}");
}

#[test]
fn an_empty_pack_naming_a_commit_the_forge_resolves_but_no_branch_holds_is_refused_through_the_gate()
 {
    let scene = rebase_scene();
    let dangling = scene.dangling();

    let why = match scene.read(&dangling, &scene.empty()) {
        Err(PushVerdict::Refuse(why)) => why,
        other => panic!("not refused: {other:?}"),
    };

    assert!(
        why.contains("neither in the push nor in a branch of this repo"),
        "{why}"
    );
}

#[test]
fn a_refused_second_sign_in_is_deleted_from_the_keychain_so_no_ask_offers_it_again() {
    use purlis_core::forge::accounts::workflow::{Availability, availability};
    use purlis_core::forge::accounts::{Method, State};
    let home = tempfile::tempdir().unwrap();
    let account = ForgeAccount {
        id: "01J9WORKFLOWOVER".into(),
        kind: Kind::GitHub,
        host: "github.com".into(),
        login: "octocat".into(),
        method: Method::Device,
        state: State::SignedIn,
        scopes: Some(vec!["repo".into(), "read:org".into()]),
        client_id: None,
    };
    let keyring = accounts::keyring_at(home.path());
    keyring
        .set(&accounts::service(&account), "workflow", "a-second-token")
        .unwrap();
    keyring
        .set(&accounts::service(&account), "token", "the-everyday-token")
        .unwrap();
    assert_eq!(
        availability(keyring.as_ref(), &account),
        Availability::Ready
    );
    let over = WorkflowOver {
        config_root: home.path().to_path_buf(),
        account: account.clone(),
    };

    over.drop_refused(&HostScope::for_a_test());

    assert_eq!(
        availability(keyring.as_ref(), &account),
        Availability::NeedsPress
    );
    // The everyday sign-in is untouched.
    assert!(
        keyring
            .get(&accounts::service(&account), "token")
            .unwrap()
            .is_some()
    );
}
