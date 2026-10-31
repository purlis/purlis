// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! A repository whose own config names programs for git to run: no git purlis runs there
//! starts any of them (#810, #1805, ADR 0084's rule for every git call).
//!
//! A chat can write the repository it works in, its config included where no sandbox holds it,
//! and purlis runs git there from outside any sandbox. So the fixture plants, in the
//! repository's own config, a program for every way config makes git start one on the verbs
//! purlis runs: a hooks folder (`core.hooksPath`) holding every hook those verbs fire, an
//! fsmonitor (`core.fsmonitor`), a pager (`core.pager`, `pager.<verb>`), aliases under the
//! verbs' own names, every driver a committed `.gitattributes` can turn on (a filter's clean,
//! smudge and long-running process, a diff textconv, a diff command and the external diff, a
//! merge driver), a signer for each signature format and the command that picks an ssh signing
//! key, the editors, a trailer's command under both its spellings, what a remote by name starts
//! at the other end, a hook named in config (git 2.54 and later), the programs only a network
//! call or a tool verb reaches (the credential helper, the askpass program, the ssh command and
//! the diff and merge tools), and the programs `git-lfs` itself starts (#1805). Each program only
//! appends its name to a mark file. Then every verb
//! is run through the runner (`purlis_core::worktree::git`), each entry point of it, and the
//! mark file must not exist.
//!
//! **The control** runs plain git on the same fixture and sees the hooks, the fsmonitor and
//! every driver run, so a plant that had stopped working cannot pass for a runner that holds. A
//! pager and a shadowing alias have no control: git starts a pager only for a terminal, which
//! the runner never gives it, and never takes an alias under a verb of its own. They are planted
//! so that a runner that one day did either is caught.
//!
//! **Real LFS keeps working**: a repository set up by `git lfs install --local` is checked out
//! with its content and reads as clean through the runner.
//!
//! Two layouts, because the runner reads them differently: a clone, whose `.git` is a folder,
//! and a work tree whose git directory is kept beside it (`--separate-git-dir`), which the
//! runner names to git itself (#1055). The linked worktree the run adds is a third, read the
//! second way.
//!
//! Unix only: the planted programs are shell scripts.
#![cfg(unix)]

use crate::support;

use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

use purlis_core::worktree::git;

/// Every hook a verb the runner is asked here fires: `worktree add` (post-checkout), `merge`
/// (post-merge), any ref it moves (reference-transaction), an index it writes
/// (post-index-change), and the rest a writer's verb would reach, so that a verb added to the
/// run below is covered without touching this list.
const HOOKS: [&str; 12] = [
    "post-checkout",
    "post-merge",
    "reference-transaction",
    "post-index-change",
    "pre-commit",
    "prepare-commit-msg",
    "commit-msg",
    "post-commit",
    "pre-merge-commit",
    "pre-rebase",
    "post-rewrite",
    "pre-push",
];

/// The verbs the runner is asked below, each planted as an alias of the same name.
const VERBS: [&str; 7] = [
    "status",
    "log",
    "diff",
    "worktree",
    "merge",
    "fetch",
    "check-attr",
];

/// Every driver key the fixture's `.gitattributes` turns on, and the name its program marks.
/// `filter.p.process` is a filter of its own, so that a long-running filter is planted whole
/// and not only beside a clean and a smudge.
const DRIVERS: [(&str, &str); 7] = [
    ("filter.x.clean", "clean"),
    ("filter.x.smudge", "smudge"),
    ("filter.p.process", "process"),
    ("diff.t.textconv", "textconv"),
    ("diff.x.command", "diff-command"),
    ("diff.external", "diff-external"),
    ("merge.x.driver", "merge-driver"),
];

/// Every other key that names a program a verb purlis runs can start, and the name its program
/// marks: a signer for each signature format (OpenPGP, ssh, X.509) and the command that picks an
/// ssh signing key, the editors, a trailer's command under both its spellings. The control
/// reaches each of the first eight; the last two are planted so that a run that one day reaches
/// them is caught (an alternate's refs, and a proxy for a transport every network call refuses).
/// What a remote by name starts at the other end has a test of its own, as purlis refuses the
/// call there.
const PROGRAMS: [(&str, &str); 10] = [
    ("gpg.program", "signer"),
    ("gpg.ssh.program", "ssh-signer"),
    ("gpg.x509.program", "x509-signer"),
    ("gpg.ssh.defaultKeyCommand", "ssh-key-command"),
    ("core.editor", "editor"),
    ("sequence.editor", "sequence-editor"),
    ("trailer.foo.cmd", "trailer"),
    ("trailer.bar.command", "trailer-command"),
    ("core.alternateRefsCommand", "alternate-refs"),
    ("core.gitProxy", "git-proxy"),
];

/// How many of [`PROGRAMS`] the control reaches.
const REACHED: usize = 8;

/// The commits carrying a signature, each in one format: the branch it is on and the armour its
/// signature starts with. A `log` asked to show signatures, and a `verify-commit`, hand each to
/// the signer of its format ([`PROGRAMS`]).
const SIGNED: [(&str, &str); 3] = [
    ("signed", "PGP SIGNATURE"),
    ("signed-ssh", "SSH SIGNATURE"),
    ("signed-x509", "SIGNED MESSAGE"),
];

/// The trailers a commit is given, one for each trailer command planted in [`PROGRAMS`].
const TRAILERS: [&str; 4] = ["--trailer", "foo=x", "--trailer", "bar=y"];

/// The keys that name a program a call reaches only by crossing a network or by a verb purlis
/// never runs, and the name each program marks (#1805): the credential helper, the askpass
/// program and the ssh command, which every network call resets or refuses
/// (`git::run_network`), and the diff and merge tools, which only `difftool` and `mergetool`
/// start. The control reaches the first four.
const CANNOT_RUN: [(&str, &str); 4] = [
    ("credential.helper", "credential-helper"),
    ("core.askPass", "askpass"),
    ("core.sshCommand", "ssh-command"),
    ("difftool.x.cmd", "difftool"),
];

/// The merge tool's key and mark, planted with no control: `mergetool` needs a conflict the
/// control's merge, whose driver runs, never leaves.
const MERGETOOL: (&str, &str) = ("mergetool.x.cmd", "mergetool");

/// A hook named in config, with its command's mark (#1805): git 2.54 and later run it on each
/// event it is given, whatever `core.hooksPath` says. It is given every event in [`HOOKS`].
const CONFIG_HOOK: (&str, &str) = ("planted", "config-hook");

/// A remote no network answers: an ssh one starts the ssh command before it connects.
const SSH_REMOTE: &str = "ssh://purlis-planted.invalid/r";

/// A shell alias that asks git's credential machinery for a credential, as a fetch over HTTPS
/// that was asked for one would: the helpers, then the askpass program.
const FILL: &str =
    "alias.fill=!printf 'protocol=https\\nhost=purlis-planted.invalid\\n\\n' | git credential fill";

/// A command a gc runs to name the objects it must keep (`gc.recentObjectsHook`, git 2.42 and
/// later), and its mark. The key is a list, which no `-c` empties, so a verb that collects
/// garbage is refused and the gc any other verb would start on its own is turned off.
const RECENT_OBJECTS_HOOK: (&str, &str) = ("gc.recentObjectsHook", "recent-objects");

/// The verbs that collect garbage themselves, each of which would run [`RECENT_OBJECTS_HOOK`].
const COLLECTS_GARBAGE: [&[&str]; 4] = [
    &["gc", "-q"],
    &["repack", "-q", "-d", "--cruft"],
    &["prune"],
    &["maintenance", "run", "--task=gc"],
];

/// Whether this git is at least `want` (major, minor). Said when it is not, since the plant
/// named by `what` then checks nothing.
fn git_at_least(want: [u32; 2], what: &str) -> bool {
    let mut cmd = support::unsigned();
    cmd.arg("version");
    let out = purlis_core::forklock::output(&mut cmd).expect("git runs");
    let said = String::from_utf8_lossy(&out.stdout).into_owned();
    let version: Vec<u32> = said
        .split_whitespace()
        .nth(2)
        .unwrap_or_default()
        .split('.')
        .take(2)
        .map(|part| part.parse().unwrap_or(0))
        .collect();
    let here = version >= want.to_vec();
    if !here {
        println!("{}: no {what}, so that plant checks nothing", said.trim());
    }
    here
}

/// Whether this git runs hooks named in config: 2.54 and later.
fn config_hooks_here() -> bool {
    git_at_least([2, 54], "hooks named in config")
}

/// Whether this git runs `gc.recentObjectsHook`: 2.42 and later.
fn recent_objects_hooks_here() -> bool {
    git_at_least([2, 42], "gc.recentObjectsHook")
}

/// An identity on the command line, for a commit in a fixture whose machine has none.
const WHO: [&str; 4] = [
    "-c",
    "user.name=Planted",
    "-c",
    "user.email=planted@example.invalid",
];

/// A repository with config that names programs, and where they say they ran.
struct Planted {
    _dir: tempfile::TempDir,
    case: PathBuf,
    /// The work tree the runner is asked in.
    work: PathBuf,
    /// A bare repository with `main` in it, to fetch from.
    upstream: PathBuf,
    /// The file each planted program appends its name to.
    ran: PathBuf,
}

#[derive(Clone, Copy)]
enum Layout {
    /// `.git` is a folder in the work tree.
    Clone,
    /// `.git` is a file naming a git directory kept beside the work tree.
    KeptBeside,
}

/// A program at `case/<name>` that appends `name` to the mark file and exits 0.
fn mark(case: &Path, ran: &Path, name: &str) -> PathBuf {
    let program = case.join(name);
    let mark = format!("echo '{name}' >> '{}'\n", ran.display());
    let body = match name {
        // A clean or a smudge filter that changes nothing: it hands the file back as it was
        // given, so git reads the same content with the filter as without it.
        "clean" | "smudge" | "lfs-extension-clean" | "lfs-extension-smudge" => {
            format!("{mark}cat\n")
        }
        // A long-running filter that answers git's handshake and offers nothing, so git goes on
        // without it: its welcome and version, a flush, an empty capability list. Then it waits
        // for git to close its input.
        "process" => format!(
            "{mark}printf '0016git-filter-server\\n000eversion=2\\n00000000'\ncat >/dev/null\n"
        ),
        _ => mark,
    };
    std::fs::write(&program, format!("#!/bin/sh\n{body}exit 0\n")).unwrap();
    std::fs::set_permissions(&program, std::fs::Permissions::from_mode(0o755)).unwrap();
    program
}

fn planted(layout: Layout) -> Planted {
    let dir = tempfile::tempdir().unwrap();
    let case = std::fs::canonicalize(dir.path()).unwrap();
    let work = case.join("work");
    match layout {
        Layout::Clone => {
            std::fs::create_dir_all(&work).unwrap();
            support::git(&work, &["init", "-q", "-b", "main", "."]);
        }
        Layout::KeptBeside => {
            let store = case.join("store");
            support::git(
                &case,
                &[
                    "init",
                    "-q",
                    "-b",
                    "main",
                    &format!("--separate-git-dir={}", store.display()),
                    "work",
                ],
            );
        }
    }
    // Which driver each file turns on. Committed, as a repository a chat cloned would have it.
    std::fs::write(
        work.join(".gitattributes"),
        "a.txt filter=x diff=x merge=x\ns.txt filter=x\nt.txt diff=t\np.txt filter=p\n",
    )
    .unwrap();
    std::fs::write(work.join("a.txt"), "one\n").unwrap();
    for name in ["s.txt", "t.txt", "p.txt", "b.txt"] {
        std::fs::write(work.join(name), format!("{name}\n")).unwrap();
    }
    support::git(&work, &["add", "."]);
    support::git(&work, &["commit", "-q", "-m", "one"]);
    support::git(&work, &["branch", "other"]);
    support::git(&work, &["checkout", "-q", "-b", "ahead"]);
    std::fs::write(work.join("a.txt"), "two\n").unwrap();
    support::git(&work, &["commit", "-q", "-am", "two"]);
    support::git(&work, &["checkout", "-q", "other"]);
    std::fs::write(work.join("a.txt"), "three\n").unwrap();
    std::fs::write(work.join("t.txt"), "three\n").unwrap();
    support::git(&work, &["commit", "-q", "-am", "three"]);
    support::git(&work, &["checkout", "-q", "main"]);
    let upstream = case.join("upstream");
    support::git(&case, &["init", "-q", "--bare", "upstream"]);
    support::git(
        &work,
        &["push", "-q", &upstream.display().to_string(), "ahead"],
    );
    for name in ["a.txt", "t.txt", "b.txt"] {
        std::fs::write(
            work.join(name),
            "changed, so status and diff have work to do\n",
        )
        .unwrap();
    }
    // The same bytes with another time: git can only tell these from the index by reading them
    // through their clean filter, which is what a status then runs.
    let past = std::time::SystemTime::now() - std::time::Duration::from_secs(600);
    for name in ["s.txt", "p.txt"] {
        let path = work.join(name);
        std::fs::write(&path, format!("{name}\n")).unwrap();
        std::fs::File::options()
            .write(true)
            .open(&path)
            .unwrap()
            .set_modified(past)
            .unwrap();
    }

    // A commit for each signature format, which a `log` asked to show signatures hands to that
    // format's signer to check. Written as objects: no signer is needed to make them.
    let tree = support::git(&work, &["rev-parse", "main^{tree}"]);
    let head = support::git(&work, &["rev-parse", "main"]);
    for (branch, armour) in SIGNED {
        let object = case.join(format!("{branch}-commit"));
        std::fs::write(
            &object,
            format!(
                "tree {}\nparent {}\nauthor A <a@example.invalid> 1700000000 +0000\n\
                 committer A <a@example.invalid> 1700000000 +0000\n\
                 gpgsig -----BEGIN {armour}-----\n \n iQEzBAABCAAdFiEE\n -----END {armour}-----\n\
                 \nsigned\n",
                String::from_utf8_lossy(&tree.stdout).trim(),
                String::from_utf8_lossy(&head.stdout).trim(),
            ),
        )
        .unwrap();
        let signed = support::git(
            &work,
            &[
                "hash-object",
                "-t",
                "commit",
                "-w",
                &object.display().to_string(),
            ],
        );
        support::git(
            &work,
            &[
                "update-ref",
                &format!("refs/heads/{branch}"),
                String::from_utf8_lossy(&signed.stdout).trim(),
            ],
        );
    }
    // git checks an ssh signature only against a file of allowed signers, so the fixture names
    // one (empty): without it the ssh signer would never be reached, by the control either.
    let allowed = case.join("allowed-signers");
    std::fs::write(&allowed, "").unwrap();

    // Planted last, so the fixture's own git above ran none of it.
    let ran = case.join("ran");
    let hooks = case.join("hooks");
    std::fs::create_dir_all(&hooks).unwrap();
    for hook in HOOKS {
        mark(&hooks, &ran, hook);
    }
    let fsmonitor = mark(&case, &ran, "fsmonitor");
    let pager = mark(&case, &ran, "pager");
    let alias = mark(&case, &ran, "alias");
    let mut config = vec![
        ("core.hooksPath".to_owned(), hooks.display().to_string()),
        ("core.fsmonitor".to_owned(), fsmonitor.display().to_string()),
        ("core.pager".to_owned(), pager.display().to_string()),
    ];
    for verb in VERBS {
        config.push((format!("pager.{verb}"), pager.display().to_string()));
        config.push((format!("alias.{verb}"), format!("!{}", alias.display())));
    }
    for (key, name) in DRIVERS {
        config.push((
            key.to_owned(),
            mark(&case, &ran, name).display().to_string(),
        ));
    }
    // A filter git must not skip: a runner that only emptied it would fail every checkout.
    config.push(("filter.x.required".to_owned(), "true".to_owned()));
    for (key, name) in PROGRAMS {
        config.push((
            key.to_owned(),
            mark(&case, &ran, name).display().to_string(),
        ));
    }
    config.push(("log.showSignature".to_owned(), "true".to_owned()));
    config.push((
        "gpg.ssh.allowedSignersFile".to_owned(),
        allowed.display().to_string(),
    ));
    for (key, name) in CANNOT_RUN.into_iter().chain([MERGETOOL]) {
        let program = mark(&case, &ran, name).display().to_string();
        // A helper is a command line of git's own unless it starts with `!`.
        let value = if key == "credential.helper" {
            format!("!{program}")
        } else {
            program
        };
        config.push((key.to_owned(), value));
    }
    config.push(("diff.tool".to_owned(), "x".to_owned()));
    config.push(("merge.tool".to_owned(), "x".to_owned()));
    let (hook, hook_mark) = CONFIG_HOOK;
    config.push((
        format!("hook.{hook}.command"),
        mark(&case, &ran, hook_mark).display().to_string(),
    ));
    let (recent, recent_mark) = RECENT_OBJECTS_HOOK;
    config.push((
        recent.to_owned(),
        mark(&case, &ran, recent_mark).display().to_string(),
    ));
    for (key, value) in &config {
        support::git(&work, &["config", key, value]);
    }
    for event in HOOKS {
        support::git(
            &work,
            &["config", "--add", &format!("hook.{hook}.event"), event],
        );
    }
    assert!(!ran.exists(), "planting ran something");
    Planted {
        _dir: dir,
        case,
        work,
        upstream,
        ran,
    }
}

/// What the planted programs said they ran, in order.
fn ran(planted: &Planted) -> Vec<String> {
    std::fs::read_to_string(&planted.ran)
        .map(|text| text.lines().map(str::to_owned).collect())
        .unwrap_or_default()
}

/// Every verb, through every entry point of the runner a test can reach: what purlis does in
/// a repository, from a read to a linked worktree it adds and a merge it makes.
fn run_everything(planted: &Planted) {
    let work = planted.work.as_path();
    let piece = planted.case.join("piece");
    let piece_arg = piece.display().to_string();
    let upstream = planted.upstream.display().to_string();
    let answered = |what: &str, run: Result<git::Run, git::GitUnavailable>| {
        let run = run.unwrap_or_else(|e| panic!("{what}: git did not start: {e}"));
        assert!(run.ok(), "{what}: {run:?}");
    };

    answered(
        "status",
        git::run(work, &["status", "--porcelain"], git::READ),
    );
    answered(
        "log",
        git::run(work, &["log", "-1", "--oneline"], git::READ),
    );
    answered("diff", git::run(work, &["diff"], git::READ));
    answered(
        "worktree add",
        git::run_untimed(work, &["worktree", "add", "-q", "-b", "piece", &piece_arg]),
    );
    // An interactive rebase with nothing to do still hands its list to the sequence editor.
    with_who(&piece, &["rebase", "-q", "-i", "HEAD"]);
    git::run_untimed(&piece, &["rebase", "--abort"]).expect("git starts");
    answered(
        "status in the added worktree",
        git::run(&piece, &["status", "--porcelain"], git::READ),
    );
    answered(
        "merge",
        git::run_untimed(&piece, &["merge", "-q", "--ff-only", "ahead"]),
    );
    // Not a fast-forward, and both sides changed `a.txt`: the merge driver's turn. With the
    // driver held, git cannot merge the file, and the merge stops on the conflict.
    // With an identity: a merge commit needs one, and a runner's machine may have none.
    let merged = git::run_untimed(
        &piece,
        &[&WHO[..], &["merge", "-q", "--no-edit", "other"]].concat(),
    )
    .unwrap_or_else(|e| panic!("merge other: git did not start: {e}"));
    if !merged.ok() {
        let aborted = git::run_untimed(&piece, &["merge", "--abort"])
            .unwrap_or_else(|e| panic!("merge --abort: git did not start: {e}"));
        assert!(
            aborted.ok(),
            "merge --abort: {aborted:?}, after the merge of other said: {merged:?}"
        );
    }
    answered(
        "diff of a branch",
        git::run(work, &["diff", "main", "ahead"], git::READ),
    );
    answered("show", git::run(work, &["show", "ahead"], git::READ));
    answered(
        "log -p",
        git::run(work, &["log", "-p", "-1", "other"], git::READ),
    );
    // A blame reads the file through its textconv, which no option on the call turns off. A
    // held textconv is no program at all, and blame fails rather than read the file without
    // it: what is checked here is that it started nothing.
    git::run(work, &["blame", "t.txt"], git::READ)
        .unwrap_or_else(|e| panic!("blame: git did not start: {e}"));
    answered(
        "fetch",
        git::run_network(
            work,
            None,
            &[
                "fetch",
                "-q",
                &upstream,
                "ahead:refs/remotes/upstream/ahead",
            ],
        ),
    );
    let attrs = git::run_with_input(
        work,
        &["check-attr", "-z", "--stdin", "--all"],
        b"a.txt\0".to_vec(),
        git::READ,
    )
    .expect("git starts");
    assert_eq!(
        attrs.code,
        Some(0),
        "check-attr: {}",
        String::from_utf8_lossy(&attrs.err)
    );
    for (branch, _) in SIGNED {
        answered(
            &format!("log of the commit on {branch}"),
            git::run(work, &["log", "-1", branch], git::READ),
        );
        git::run(work, &["verify-commit", branch], git::READ).expect("git starts");
    }
    // A commit given no message starts the editor; one given trailers, their commands.
    with_who(work, &["commit", "-q", "--allow-empty"]);
    with_who(
        work,
        &[&["commit", "-q", "--allow-empty", "-m", "t"][..], &TRAILERS].concat(),
    );
    // A signed commit, where the repository names its own signer: refused before git runs,
    // saying where the signer belongs instead (D-1805-ruling). In the ssh format too, where a
    // commit given no signing key would first ask the repository's key command for one.
    for format in [&[][..], &["-c", "gpg.format=ssh"]] {
        let mut signing: Vec<&str> = WHO.to_vec();
        signing.extend_from_slice(format);
        signing.extend(["commit", "-q", "--allow-empty", "-S", "-m", "signed here"]);
        let signed = git::run_untimed(work, &signing).expect("git starts");
        assert!(
            !signed.ok()
                && signed.err.contains("names a signing program")
                && signed
                    .err
                    .contains("Set the signer in your global git config"),
            "a signed commit {format:?}: {signed:?}"
        );
    }
    // A ref written outside a checkout or a merge fires the reference-transaction hook.
    answered(
        "update-ref",
        git::run(work, &["update-ref", "refs/heads/moved", "main"], git::READ),
    );
    answered(
        "branch",
        git::run(work, &["branch", "made", "main"], git::READ),
    );
    // A verb that collects garbage would run the gc's command, which no `-c` turns off: refused
    // before git runs (`maintenance` already off the network entry point, as it can fetch).
    for args in COLLECTS_GARBAGE {
        let refused = git::run_untimed(work, args)
            .unwrap_or_else(|e| panic!("{args:?}: git did not start: {e}"));
        assert!(
            !refused.ok() && refused.err.contains("so git was not run"),
            "{args:?}: {refused:?}"
        );
    }
    // An ssh remote, by URL and as `host:path`: every network call refuses the transport before
    // the ssh command starts.
    for remote in [SSH_REMOTE, "purlis-planted.invalid:r"] {
        let refused = git::run_network(work, None, &["fetch", "-q", remote])
            .unwrap_or_else(|e| panic!("fetch {remote}: git did not start: {e}"));
        assert!(!refused.ok(), "fetch {remote}: {refused:?}");
    }
    // Asked for a credential, a network call has no helper but the one it is given, and no
    // askpass program. The call must reach git's credential machinery and end there, at the
    // prompt git may not show: a runner that refused the alias, or a fill that never asked,
    // would leave the planted helper and askpass program unchecked.
    let filled = git::run_network(work, None, &["-c", FILL, "fill"])
        .unwrap_or_else(|e| panic!("credential fill: git did not start: {e}"));
    assert!(
        !filled.ok()
            && filled.err.contains(
                "could not read Username for 'https://purlis-planted.invalid': \
                 terminal prompts disabled"
            ),
        "credential fill did not end at git's own prompt: {filled:?}"
    );
    for (what, raw) in [
        (
            "status as the session",
            git::run_as_session(work, &["status", "--porcelain"], git::READ),
        ),
        (
            "status from a hook",
            git::run_in_hook(work, &["status", "--porcelain"], git::READ),
        ),
    ] {
        let raw = raw.unwrap_or_else(|e| panic!("{what}: git did not start: {e}"));
        assert_eq!(
            raw.code,
            Some(0),
            "{what}: {}",
            String::from_utf8_lossy(&raw.err)
        );
    }
}

/// `args` through the runner with [`WHO`] before them, for a call whose answer does not
/// matter here: only what it started does.
fn with_who(dir: &Path, args: &[&str]) {
    let mut all: Vec<&str> = WHO.to_vec();
    all.extend_from_slice(args);
    git::run_untimed(dir, &all).unwrap_or_else(|e| panic!("{args:?}: git did not start: {e}"));
}

fn nothing_runs_in(layout: Layout) {
    let planted = planted(layout);

    run_everything(&planted);

    assert_eq!(
        ran(&planted),
        Vec::<String>::new(),
        "a program the repository's config names was run by purlis's git"
    );
}

#[test]
fn nothing_a_clones_config_names_runs_on_any_git_purlis_runs_there() {
    purlis_core::unsteered!();
    nothing_runs_in(Layout::Clone);
}

#[test]
fn nothing_the_config_of_a_git_directory_kept_beside_its_work_tree_names_runs() {
    purlis_core::unsteered!();
    nothing_runs_in(Layout::KeptBeside);
}

/// The control: git that is not the runner, on the same fixture, runs the fsmonitor, the
/// hooks, every driver and every other program it reaches. Without it a plant git had stopped reading would pass for a runner that holds.
fn the_plant_is_live_in(layout: Layout) {
    let planted = planted(layout);
    let plain = |dir: &Path, args: &[&str]| {
        let mut cmd = support::unsigned();
        cmd.arg("-C").arg(dir).args(args);
        cmd.env("GIT_CONFIG_GLOBAL", "/dev/null")
            .env("GIT_CONFIG_SYSTEM", "/dev/null")
            // A harness's own editor, askpass or ssh would answer in the planted one's place.
            .env_remove("GIT_EDITOR")
            .env_remove("GIT_SEQUENCE_EDITOR")
            .env_remove("GIT_ASKPASS")
            .env_remove("SSH_ASKPASS")
            .env_remove("GIT_SSH")
            .env_remove("GIT_SSH_COMMAND");
        purlis_core::forklock::output(&mut cmd).expect("git runs")
    };
    let piece = planted.case.join("piece").display().to_string();

    plain(&planted.work, &["status", "--porcelain"]);
    plain(
        &planted.work,
        &["worktree", "add", "-q", "-b", "piece", &piece],
    );
    let piece_dir = planted.case.join("piece");
    plain(
        &piece_dir,
        &[&WHO[..], &["rebase", "-q", "-i", "HEAD"]].concat(),
    );
    for (branch, _) in SIGNED {
        plain(&planted.work, &["log", "-1", branch]);
    }
    plain(&planted.work, &["fetch", "-q", "up"]);
    plain(
        &planted.work,
        &["push", "-q", "up", "ahead:refs/heads/pushed"],
    );
    plain(
        &planted.work,
        &[&WHO[..], &["commit", "-q", "--allow-empty"]].concat(),
    );
    plain(
        &planted.work,
        &[
            &WHO[..],
            &["commit", "-q", "--allow-empty", "-m", "t"],
            &TRAILERS,
        ]
        .concat(),
    );
    // An ssh signature with no signing key named: git asks the key command for one first.
    plain(
        &planted.work,
        &[
            &WHO[..],
            &[
                "-c",
                "gpg.format=ssh",
                "commit",
                "-q",
                "--allow-empty",
                "-S",
                "-m",
                "s",
            ],
        ]
        .concat(),
    );
    plain(&planted.work, &["diff"]);
    // The external diff answers for every file it covers; a textconv is reached without it.
    plain(&planted.work, &["diff", "--no-ext-diff"]);
    plain(&planted.work, &["blame", "t.txt"]);
    let piece = planted.case.join("piece");
    plain(&piece, &["merge", "-q", "--ff-only", "ahead"]);
    let merged = plain(
        &piece,
        &[&WHO[..], &["merge", "-q", "--no-edit", "other"]].concat(),
    );
    let merge_said = String::from_utf8_lossy(&merged.stderr).into_owned();
    plain(&planted.work, &["fetch", "-q", SSH_REMOTE]);
    plain(&planted.work, &["-c", FILL, "fill"]);
    plain(&planted.work, &["difftool", "--no-prompt"]);
    // An object nothing reaches, older than the gc keeps such an object for: git asks the gc's
    // command only about an object it would otherwise remove.
    let loose = planted.case.join("loose");
    std::fs::write(&loose, "reached by nothing\n").unwrap();
    let written = plain(
        &planted.work,
        &["hash-object", "-w", &loose.display().to_string()],
    );
    let id = String::from_utf8_lossy(&written.stdout).trim().to_owned();
    let at = plain(
        &planted.work,
        &[
            "rev-parse",
            "--git-path",
            &format!("objects/{}/{}", &id[..2], &id[2..]),
        ],
    );
    let object = planted
        .work
        .join(String::from_utf8_lossy(&at.stdout).trim());
    std::fs::File::open(&object)
        .unwrap_or_else(|e| panic!("the loose object {}: {e}", object.display()))
        .set_modified(std::time::SystemTime::now() - std::time::Duration::from_secs(60 * 86_400))
        .unwrap();
    plain(&planted.work, &["gc", "-q"]);

    let said = ran(&planted);
    let drivers = DRIVERS.map(|(_, name)| name);
    let programs = PROGRAMS.map(|(_, name)| name);
    let cannot_run = CANNOT_RUN.map(|(_, name)| name);
    let hook = config_hooks_here().then_some(CONFIG_HOOK.1);
    let recent = recent_objects_hooks_here().then_some(RECENT_OBJECTS_HOOK.1);
    for expected in ["fsmonitor", "post-checkout"]
        .iter()
        .chain(&drivers)
        .chain(&programs[..REACHED])
        .chain(&cannot_run)
        .chain(hook.as_ref())
        .chain(recent.as_ref())
    {
        assert!(
            said.iter().any(|name| name == expected),
            "plain git did not run the planted {expected}: {said:?} (the merge of other said: {merge_said})"
        );
    }
}

#[test]
fn the_control_plain_git_runs_what_a_clones_config_names() {
    purlis_core::unsteered!();
    the_plant_is_live_in(Layout::Clone);
}

#[test]
fn the_control_plain_git_runs_what_a_kept_beside_git_directorys_config_names() {
    purlis_core::unsteered!();
    the_plant_is_live_in(Layout::KeptBeside);
}

/// Whether `git lfs` runs here. GitHub's runners ship it; where it is missing, the test says
/// so and checks nothing rather than failing.
fn lfs_is_here() -> bool {
    let mut cmd = support::unsigned();
    cmd.args(["lfs", "version"]);
    let here = purlis_core::forklock::output(&mut cmd).is_ok_and(|out| out.status.success());
    if !here {
        println!("skipped: git-lfs is not installed here, so real LFS was not checked");
    }
    here
}

/// What the runner holds is the repository's own drivers, not LFS: a repository set up the
/// standard way (`git lfs install --local`) is checked out with its content, and a file whose
/// time moved still reads as unchanged, which only the clean filter can tell.
#[test]
fn a_repository_with_real_lfs_is_checked_out_with_its_content_and_reads_clean() {
    purlis_core::unsteered!();
    // A machine without git-lfs checks nothing here, and says so.
    if !lfs_is_here() {
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    let case = std::fs::canonicalize(dir.path()).unwrap();
    let work = case.join("work");
    std::fs::create_dir_all(&work).unwrap();
    support::git(&work, &["init", "-q", "-b", "main", "."]);
    support::git(&work, &["lfs", "install", "--local"]);
    support::git(&work, &["lfs", "track", "*.bin"]);
    let content = "the content LFS keeps out of git\n";
    std::fs::write(work.join("big.bin"), content).unwrap();
    support::git(&work, &["add", ".gitattributes", "big.bin"]);
    support::git(&work, &["commit", "-q", "-m", "one"]);
    let stored = support::git(&work, &["cat-file", "-p", "HEAD:big.bin"]);
    assert!(
        String::from_utf8_lossy(&stored.stdout).starts_with("version https://git-lfs"),
        "the fixture did not store a pointer: {stored:?}"
    );

    let piece = case.join("piece");
    let added = git::run_untimed(
        &work,
        &[
            "worktree",
            "add",
            "-q",
            "-b",
            "piece",
            &piece.display().to_string(),
        ],
    )
    .expect("git starts");
    assert!(added.ok(), "worktree add: {added:?}");
    assert_eq!(
        std::fs::read_to_string(piece.join("big.bin")).unwrap(),
        content,
        "the checkout holds the pointer, not the content"
    );

    let past = std::time::SystemTime::now() - std::time::Duration::from_secs(600);
    for tree in [&work, &piece] {
        let path = tree.join("big.bin");
        std::fs::write(&path, content).unwrap();
        std::fs::File::options()
            .write(true)
            .open(&path)
            .unwrap()
            .set_modified(past)
            .unwrap();
        let status = git::run(tree, &["status", "--porcelain"], git::READ).expect("git starts");
        assert!(status.ok(), "status: {status:?}");
        assert_eq!(status.out, "", "an unchanged LFS file reads as changed");
    }
}

/// A file's time moved back, so that git can tell it from the index only by reading it through
/// its clean filter.
fn touched_in_the_past(path: &Path) {
    let past = std::time::SystemTime::now() - std::time::Duration::from_secs(600);
    std::fs::File::options()
        .write(true)
        .open(path)
        .unwrap()
        .set_modified(past)
        .unwrap();
}

/// The programs `git-lfs` itself starts, named in a repository's own config (#1805): an
/// extension's clean and smudge, which LFS's filter runs on every file it tracks, and a transfer
/// agent, which only a transfer to or from an LFS server starts. purlis turns the whole LFS filter
/// off there, so `git-lfs` never starts at all. The control: plain git's status runs the
/// extension's clean. A transfer agent has no control, as no LFS server answers here; the runner's
/// unit test pins that it holds the filter whole for it too.
#[test]
fn a_program_lfs_would_start_is_never_reached() {
    purlis_core::unsteered!();
    // A machine without git-lfs checks nothing here, and says so.
    if !lfs_is_here() {
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    let case = std::fs::canonicalize(dir.path()).unwrap();
    let work = case.join("work");
    std::fs::create_dir_all(&work).unwrap();
    support::git(&work, &["init", "-q", "-b", "main", "."]);
    support::git(&work, &["lfs", "install", "--local"]);
    support::git(&work, &["lfs", "track", "*.bin"]);
    std::fs::write(work.join("big.bin"), "the content LFS keeps out of git\n").unwrap();
    support::git(&work, &["add", ".gitattributes", "big.bin"]);
    support::git(&work, &["commit", "-q", "-m", "one"]);

    // Planted last, so the fixture's own git above ran none of it.
    let ran = case.join("ran");
    let at = |name: &str| mark(&case, &ran, name).display().to_string();
    for (key, value) in [
        (
            "lfs.extension.x.clean",
            format!("{} %f", at("lfs-extension-clean")),
        ),
        (
            "lfs.extension.x.smudge",
            format!("{} %f", at("lfs-extension-smudge")),
        ),
        ("lfs.extension.x.priority", "0".to_owned()),
        ("lfs.customtransfer.x.path", at("lfs-transfer-agent")),
        ("lfs.standalonetransferagent", "x".to_owned()),
    ] {
        support::git(&work, &["config", key, &value]);
    }
    let said = || {
        std::fs::read_to_string(&ran)
            .map(|text| text.lines().map(str::to_owned).collect())
            .unwrap_or_default()
    };
    assert_eq!(said(), Vec::<String>::new(), "planting ran something");

    let big = work.join("big.bin");
    touched_in_the_past(&big);
    let status = git::run(&work, &["status", "--porcelain"], git::READ).expect("git starts");
    assert!(status.ok(), "status: {status:?}");
    let piece = case.join("piece");
    let added = git::run_untimed(
        &work,
        &[
            "worktree",
            "add",
            "-q",
            "-b",
            "piece",
            &piece.display().to_string(),
        ],
    )
    .expect("git starts");
    assert!(added.ok(), "worktree add: {added:?}");
    assert_eq!(
        said(),
        Vec::<String>::new(),
        "a program the repository's config names for git-lfs was run by purlis's git"
    );

    touched_in_the_past(&big);
    let mut cmd = support::unsigned();
    cmd.arg("-C").arg(&work).args(["status", "--porcelain"]);
    cmd.env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_SYSTEM", "/dev/null");
    purlis_core::forklock::output(&mut cmd).expect("git runs");
    let control: Vec<String> = said();
    assert!(
        control.iter().any(|name| name == "lfs-extension-clean"),
        "plain git did not run the planted LFS extension: {control:?}"
    );
}

/// What a remote by name starts at the other end (`remote.<name>.uploadpack`, `.receivepack`):
/// git keeps the first of these it reads, so no command line can turn one off, and purlis
/// refuses every call that can reach a remote in a repository whose own config names one. The
/// control: plain git runs both.
fn a_remote_s_programs_are_never_reached_in(layout: Layout) {
    let planted = planted(layout);
    let work = planted.work.as_path();
    support::git(
        work,
        &[
            "config",
            "remote.up.url",
            &planted.upstream.display().to_string(),
        ],
    );
    for (key, name) in [
        ("remote.up.uploadpack", "upload-pack"),
        ("remote.up.receivepack", "receive-pack"),
    ] {
        support::git(
            work,
            &[
                "config",
                key,
                &mark(&planted.case, &planted.ran, name)
                    .display()
                    .to_string(),
            ],
        );
    }
    let upstream = planted.upstream.display().to_string();

    for args in [
        &["fetch", "-q", "up"][..],
        &["push", "-q", "up", "ahead:refs/heads/pushed"],
        &["ls-remote", "up"],
        // By its address, too: purlis does not tell which remote a call will reach.
        &[
            "fetch",
            "-q",
            &upstream,
            "ahead:refs/remotes/upstream/ahead",
        ],
    ] {
        let refused = git::run_network(work, None, args)
            .unwrap_or_else(|e| panic!("{args:?}: git did not start: {e}"));
        assert!(
            !refused.ok() && refused.err.contains("purlis cannot turn off"),
            "{args:?}: {refused:?}"
        );
    }
    assert_eq!(
        ran(&planted),
        Vec::<String>::new(),
        "a remote's program ran"
    );

    for args in [
        &["fetch", "-q", "up"][..],
        &["push", "-q", "up", "ahead:refs/heads/pushed"],
    ] {
        let mut cmd = support::unsigned();
        cmd.arg("-C").arg(work).args(args);
        cmd.env("GIT_CONFIG_GLOBAL", "/dev/null")
            .env("GIT_CONFIG_SYSTEM", "/dev/null");
        purlis_core::forklock::output(&mut cmd).expect("git runs");
    }
    let said = ran(&planted);
    for expected in ["upload-pack", "receive-pack"] {
        assert!(
            said.iter().any(|name| name == expected),
            "plain git did not run the planted {expected}: {said:?}"
        );
    }
}

#[test]
fn a_clones_remote_programs_are_never_reached() {
    purlis_core::unsteered!();
    a_remote_s_programs_are_never_reached_in(Layout::Clone);
}

#[test]
fn a_kept_beside_git_directorys_remote_programs_are_never_reached() {
    purlis_core::unsteered!();
    a_remote_s_programs_are_never_reached_in(Layout::KeptBeside);
}

/// The remote helper a remote's `vcs` names (`remote.<name>.vcs`, #1853): git starts
/// `git-remote-<vcs>` for a call that reaches that remote, an empty `vcs` names a helper too, and
/// no command line turns one off, so purlis refuses every call that can reach a remote in a
/// repository whose own config names one. The control: plain git, finding the helper on its
/// `PATH`, runs it. The runner's refusal is said before git runs, so where the helper is on
/// no `PATH` the refusal still names the setting rather than git's failure to find it.
fn a_remote_helper_is_never_reached_in(layout: Layout) {
    let planted = planted(layout);
    let work = planted.work.as_path();
    let helpers = planted.case.join("helpers");
    std::fs::create_dir_all(&helpers).unwrap();
    mark(&helpers, &planted.ran, "git-remote-planted");
    for (key, value) in [
        ("remote.vh.url", "planted-address"),
        ("remote.vh.vcs", "planted"),
    ] {
        support::git(work, &["config", key, value]);
    }
    let upstream = planted.upstream.display().to_string();

    for args in [
        &["fetch", "-q", "vh"][..],
        &["push", "-q", "vh", "ahead:refs/heads/pushed"],
        &["ls-remote", "vh"],
        // By its address, too: purlis does not tell which remote a call will reach.
        &[
            "fetch",
            "-q",
            &upstream,
            "ahead:refs/remotes/upstream/ahead",
        ],
    ] {
        let refused = git::run_network(work, None, args)
            .unwrap_or_else(|e| panic!("{args:?}: git did not start: {e}"));
        assert!(
            !refused.ok() && refused.err.contains("purlis cannot turn off"),
            "{args:?}: {refused:?}"
        );
    }
    assert_eq!(ran(&planted), Vec::<String>::new(), "a remote's helper ran");

    let path = std::env::join_paths(
        std::iter::once(helpers.clone()).chain(
            std::env::var_os("PATH")
                .map(|path| std::env::split_paths(&path).collect::<Vec<_>>())
                .unwrap_or_default(),
        ),
    )
    .unwrap();
    let mut cmd = support::unsigned();
    cmd.arg("-C").arg(work).args(["fetch", "-q", "vh"]);
    cmd.env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_SYSTEM", "/dev/null")
        .env("PATH", path);
    purlis_core::forklock::output(&mut cmd).expect("git runs");
    let said = ran(&planted);
    assert!(
        said.iter().any(|name| name == "git-remote-planted"),
        "plain git did not run the planted remote helper: {said:?}"
    );
}

#[test]
fn a_clones_remote_helper_is_never_reached() {
    purlis_core::unsteered!();
    a_remote_helper_is_never_reached_in(Layout::Clone);
}

#[test]
fn a_kept_beside_git_directorys_remote_helper_is_never_reached() {
    purlis_core::unsteered!();
    a_remote_helper_is_never_reached_in(Layout::KeptBeside);
}
