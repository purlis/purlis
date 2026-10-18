// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **Every guard family reads a hostile command in time proportional to its length** (#1355).
//!
//! Reviews found shapes whose cost grew much faster than the command: substitutions in quotes
//! in substitutions, nested arithmetic, chains of wrappers. The caps in front of the guards
//! ([`purlis_core::guardcaps`]) bound the nesting and the size; inside them, each family must
//! grow no faster than the command does. Four times the input may take a little over four times
//! as long; a reading that grew with its square would take sixteen.
//!
//! Each shape here is built at the caps, so the guards themselves are measured, not the caps'
//! refusal. And the whole verdict, every arm in both modes, is held well inside the hook's
//! budget at the largest size and the deepest nesting a command may have, and one past it, as
//! an optimised build reads it. The hook's budget stands behind all of this in `purlis-cli`.

use std::path::Path;
use std::time::{Duration, Instant};

use purlis_core::forge::{Forge, Kind};
use purlis_core::guardcaps::{MAX_LAYERS, MAX_NESTING};
use purlis_core::handoffguard::Caller;
use purlis_core::toolgate::{self, Call, Launched, Plane};
use purlis_core::{
    cloneupdate, commitguard, consentspelling, credguard, floorguard, guardcaps, handoffguard,
    leakguard, planeroot, projectgit, proseguard, rootupdate,
};

/// A hostile shape: its name, and the command at a given size in bytes.
type Shape = (&'static str, Box<dyn Fn(usize) -> String>);

/// A guard family: its name, and a call of it on one command.
type Family<'a> = (&'static str, Box<dyn Fn(&str) + 'a>);

/// `unit` repeated to about `bytes`.
fn fill(unit: &str, bytes: usize) -> String {
    unit.repeat(bytes / unit.len() + 1)
}

/// The hostile shapes, each a function of the command's size in bytes, with substitutions
/// nested `deep` where a shape nests them.
fn shapes(deep: usize) -> Vec<Shape> {
    vec![
        (
            "a chain of wrappers on every line",
            Box::new(|b| {
                fill(
                    &format!("{}ls\n", "eval env nice ".repeat(MAX_LAYERS / 3)),
                    b,
                )
            }),
        ),
        (
            // busybox and `coproc` are read through to what they run (#1325).
            "busybox and coproc in front of every command",
            Box::new(|b| {
                fill(
                    &format!(
                        "{}terraform apply\ncoproc C {{ ls; }}\n",
                        "coproc busybox env ".repeat(MAX_LAYERS / 3)
                    ),
                    b,
                )
            }),
        ),
        (
            // Every shell that runs a string is read, in each spelling of its `-c`, and fish's
            // keywords stand in front of a command (#1325).
            "other shells' strings and fish's keywords in front of every command",
            Box::new(|b| {
                fill(
                    &format!(
                        "{}fish -c 'true; and not begin git checkout feature; end'\n",
                        "mksh -c yash --cmdline tcsh -c ".repeat(MAX_LAYERS / 4)
                    ),
                    b,
                )
            }),
        ),
        (
            // The project-root guard reads a string a shell runs as a line of its own, from
            // where that shell runs, at every level a shape nests (#1325).
            "strings handed to every kind of shell around git, nested",
            Box::new(move |b| {
                let unit =
                    (0..deep).fold("git checkout feature".to_string(), |inner, k| match k % 4 {
                        0 => format!("cd x && sh -c {inner:?}"),
                        1 => format!("fish --command={inner:?}"),
                        2 => format!("eval {inner:?}; git status"),
                        _ => format!("env -C y yash --cmdline {inner:?} <<< {inner:?}"),
                    });
                fill(&format!("{unit}\n"), b)
            }),
        ),
        (
            // What a line exports and assigns is handed to every string a shell runs in it,
            // and joined only where a git may read it (#1325).
            "assignments and exports before many strings handed to shells",
            Box::new(|b| {
                let mut s = String::new();
                let mut i = 0;
                while s.len() < b {
                    s += &format!(
                        "export GIT_V{i}=x; A{i}=y; B=1 C=2 sh -c 'git status; git log'; "
                    );
                    i += 1;
                }
                s
            }),
        ),
        (
            // The project-root guard reads what a shell is fed on its stdin and what a `find`
            // runs as lines of their own too, at every level a shape nests (#1325).
            "strings piped to shells and run by find around git, nested",
            Box::new(move |b| {
                let unit =
                    (0..deep).fold("git checkout feature".to_string(), |inner, k| match k % 3 {
                        0 => format!("echo {inner:?} | sh"),
                        1 => format!("find . -exec sh -c {inner:?} \\;"),
                        _ => format!("printf %s {inner:?} | fish; git status"),
                    });
                fill(&format!("{unit}\n"), b)
            }),
        ),
        (
            // Many commands piped into many shells, a group in front of each: every shell is
            // fed what a bounded run of commands before it prints (#1325).
            "groups piped into shells, one after another",
            Box::new(|b| {
                fill(
                    "{ git status; echo 'git log'; } | sh | bash -s; true; git log | fish; ",
                    b,
                )
            }),
        ),
        (
            // A heredoc fed to fish is read again as fish reads it, from every directory the
            // line's `cd`s may have left the shell in (#1325).
            "heredocs fed to fish after loose cds",
            Box::new(|b| {
                let mut s = String::new();
                let mut i = 0;
                while s.len() < b {
                    s += &format!("cd d{i}; fish <<'E'\ntrue; and git status\nE\n");
                    i += 1;
                }
                s
            }),
        ),
        (
            // What an `eval` exports reaches the rest of the line, and the strings in it (#1325).
            "exports inside evals before many gits",
            Box::new(|b| {
                let mut s = String::new();
                let mut i = 0;
                while s.len() < b {
                    s += &format!("eval export GIT_V{i}=x; git status; sh -c 'git log'; ");
                    i += 1;
                }
                s
            }),
        ),
        (
            // Under `set -a` what an `eval` assigns is exported, and so is what the line assigns
            // after an `eval` turned it on (#1325).
            "assignments inside evals under set -a before many gits",
            Box::new(|b| {
                let mut s = String::from("eval set -a; ");
                let mut i = 0;
                while s.len() < b {
                    s += &format!("eval GIT_V{i}=x; GIT_W{i}=y; git status; ");
                    i += 1;
                }
                s
            }),
        ),
        (
            "a shell's -c inside a shell's -c",
            Box::new(|b| fill(&format!("{}ls\n", "bash -c ".repeat(MAX_LAYERS)), b)),
        ),
        (
            "unclosed substitutions on every line",
            Box::new(move |b| fill(&format!("{}ls\n", "echo \"$(".repeat(deep)), b)),
        ),
        (
            "substitutions nested around a long body",
            Box::new(move |b| {
                format!(
                    "{}{}{}",
                    "echo $(".repeat(deep),
                    fill("a ", b),
                    ")".repeat(deep)
                )
            }),
        ),
        (
            "quotes around substitutions, repeated",
            Box::new(move |b| fill(&format!("{}\n", "'\"$(".repeat(deep)), b)),
        ),
        (
            "nested arithmetic",
            Box::new(move |b| {
                fill(
                    &format!("echo {}1{}\n", "$((".repeat(deep), "))".repeat(deep)),
                    b,
                )
            }),
        ),
        (
            "many escaped values",
            Box::new(|b| format!("git -C x commit -m {}", fill("a\\ \\\"b\\' ", b))),
        ),
        (
            "a long run of exports",
            Box::new(|b| {
                let mut s = String::new();
                let mut i = 0;
                while s.len() < b {
                    s += &format!("export GIT_V{i}=x; ");
                    i += 1;
                }
                s + "git checkout feature"
            }),
        ),
        ("many heredocs", Box::new(|b| fill("cat <<A\nx\nA\n", b))),
        (
            // The project-root update guard reads every git on the line that it does not
            // refuse: restores to the current commit, ended operations, the index (qw346).
            "restores and ended operations in the root, one after another",
            Box::new(|b| {
                fill(
                    "git restore --source=HEAD --worktree -- a b/c d; git checkout HEAD -- x y; \
                     git reset --hard HEAD; git rebase --abort; git stash list; \
                     git restore --staged --source=origin/main q; git checkout -- z\n",
                    b,
                )
            }),
        ),
        (
            // And every path of one restore, each asked whether the sandbox denies it (qw346).
            "a restore of a long run of paths",
            Box::new(|b| {
                format!(
                    "git restore --source=HEAD --worktree -- {}",
                    fill("a/b.md docs/.claude/notes.md ../c ", b)
                )
            }),
        ),
        (
            // An update behind every wrapper and inside strings handed to shells (qw346).
            "updates of the root inside strings handed to shells, nested",
            Box::new(move |b| {
                let unit =
                    (0..deep).fold("git pull --rebase".to_string(), |inner, k| match k % 3 {
                        0 => format!("cd x && sh -c {inner:?}"),
                        1 => format!("env -C y bash -c {inner:?}; git rebase --abort"),
                        _ => format!("echo {inner:?} | sh"),
                    });
                fill(&format!("{unit}\n"), b)
            }),
        ),
        (
            // The clone update guard reads every git that acts in a folder other than the
            // root, and passes over what writes nothing from another commit (qw349).
            "git in a workspace's clone that moves no tree, one after another",
            Box::new(|b| {
                fill(
                    "git -C workspaces/x/repo status; git -C workspaces/x/repo fetch; \
                     cd workspaces/x/repo && git commit -m x; git -C workspaces/x/repo restore \
                     --source=HEAD --worktree -- a b; git -C workspaces/x/repo rebase --abort; \
                     git -C workspaces/x/repo rebase --continue\n",
                    b,
                )
            }),
        ),
        (
            // And asks git about a few moves at most, however many the line holds (qw349).
            "moves in a workspace's clone, one after another",
            Box::new(|b| {
                fill(
                    "git -C workspaces/x/repo checkout a; git -C workspaces/x/repo merge b; \
                     cd workspaces/x/repo && git pull; git -C workspaces/x/repo rebase c; \
                     git -C workspaces/x/repo clone ../other d\n",
                    b,
                )
            }),
        ),
        (
            // A move sent to a long run of folders, each one more place to read (qw349).
            "moves in a clone behind a long run of loose cds",
            Box::new(|b| {
                format!(
                    "cd workspaces/x/repo; {} git reset --hard origin/main",
                    fill("cd a; cd ../b; ", b)
                )
            }),
        ),
        (
            // Each `cd <name> && …;` leaves one more directory the shell may be in, a level
            // deeper, and every git is read against each: asked of the disk once for the
            // line, and spelt up to a cap (qw367).
            "a cd joined by && before every git, each a level deeper",
            Box::new(|b| fill("cd a && git status; git -C b log; git checkout x; ", b)),
        ),
        (
            // Every level of a string a shell runs is read (#1419, #1426): heredocs fed to a
            // shell, each holding a string handed to another, as deep as a shape nests.
            "strings and heredocs handed on to shells, nested",
            Box::new(move |b| {
                let unit = (0..deep).fold("ls".to_string(), |inner, k| {
                    if k % 2 == 0 {
                        format!("bash <<'E{k}'\n{inner}\nE{k}")
                    } else {
                        format!("eval bash -c {inner:?}")
                    }
                });
                fill(&format!("{unit}\n"), b)
            }),
        ),
    ]
}

/// Every family the Bash guard runs, each asked on its own so one that refuses early does not
/// hide a slow one behind it.
fn families(root: &Path) -> Vec<Family<'_>> {
    let forges = vec![
        Forge::default_of(Kind::GitHub),
        Forge::default_of(Kind::GitLab),
    ];
    let caller = Caller {
        agent_id: None,
        harness: Some("claude-code"),
        permission_mode: Some("bypassPermissions"),
    };
    let state = root.join(".charter");
    let r = root.display().to_string();
    vec![
        (
            "the leak guard",
            Box::new(move |c| {
                let _ = leakguard::leak_reason(c, "", &state);
            }),
        ),
        (
            "the one-credential guard",
            Box::new(move |c| {
                let _ = credguard::single_credential_hit(c, &forges);
            }),
        ),
        ("the project-root guards", {
            let r = r.clone();
            Box::new(move |c| {
                let _ = planeroot::plane_root_branch_reason(c, &r, &r);
                let _ = planeroot::plane_root_reset_reason(c, &r, &r);
            })
        }),
        (
            "the release floor",
            Box::new(|c| {
                let _ = floorguard::release_floor_reason(c, true);
            }),
        ),
        (
            "the substitution guards",
            Box::new(|c| {
                let _ = proseguard::forge_substitution_hit(c);
                let _ = proseguard::charter_substitution_hit(c);
            }),
        ),
        (
            "the handoff guard",
            Box::new(move |c| {
                let _ = handoffguard::handoff_refusal(c, caller);
            }),
        ),
        (
            "the consent-spelling guard",
            Box::new(move |c| {
                let _ = consentspelling::refusal(c, root, &[root]);
            }),
        ),
        (
            "the hook-skip guard",
            Box::new(|c| {
                let _ = commitguard::hook_skip_hit(c, "");
            }),
        ),
        ("the project-lock guard", {
            let r = r.clone();
            Box::new(move |c| {
                let _ = projectgit::refusal(c, &r, &r, &root.join("workspaces/x"));
            })
        }),
        ("the clone update guard", {
            // From the root's chat and from a workspace's, so a clone below either is read.
            let r = r.clone();
            Box::new(move |c| {
                let chat = root.join("workspaces/x");
                let _ = cloneupdate::refusal(c, &r, &r, Some(root));
                let _ = cloneupdate::refusal(c, &chat.display().to_string(), &r, Some(&chat));
            })
        }),
        (
            // With a name the sandbox denies said to be in the root, so every git on the line
            // is judged rather than passed over; and with none, as the root here has.
            "the project-root update guard",
            Box::new(move |c| {
                let held = |_: &Path| vec!["purlis.toml".to_string()];
                let _ = rootupdate::refusal_where(c, &r, &r, None, &held);
                let _ = rootupdate::refusal(c, &r, &r, Some(root));
            }),
        ),
    ]
}

#[test]
fn every_guard_family_reads_a_hostile_command_in_time_proportional_to_its_length() {
    purlis_core::unsteered!();
    let dir = tempfile::tempdir().expect("a directory");
    let root = dir.path();
    // A project root with a git directory and a chat folder in it, so the project-lock guard
    // reads every command to its end rather than stopping at a missing folder.
    std::fs::create_dir_all(root.join(".git")).expect("the git directory");
    std::fs::create_dir_all(root.join("workspaces/x")).expect("the chat folder");
    // And a clone of the workspace's own, so git sent there acts on another repository than
    // the project's and the clone update guard reads it (qw349).
    std::fs::create_dir_all(root.join("workspaces/x/repo/.git")).expect("the clone");
    let families = families(root);
    // At the nesting cap: the deepest a command any guard reads may go.
    for (shape, line) in shapes(MAX_NESTING) {
        let (small, large) = (line(4 * 1024), line(16 * 1024));
        // Inside the caps, so it is the guards that are measured.
        assert_eq!(guardcaps::refusal(&large), None, "{shape}");
        for (family, judge) in &families {
            // The faster of two runs each, so a moment of load on the machine is not the measure.
            let time = |cmd: &str| {
                (0..2)
                    .map(|_| {
                        let began = Instant::now();
                        judge(cmd);
                        began.elapsed()
                    })
                    .min()
                    .unwrap_or_default()
            };
            let (t_small, t_large) = (time(&small), time(&large));
            assert!(
                t_large < t_small * 10 + Duration::from_millis(100),
                "{family} on {shape}: 4 KB took {t_small:?}, 16 KB took {t_large:?}"
            );
        }
    }
}

/// The most the whole verdict on one command may take in an optimised build: half the hook's
/// two-second budget. Measured on the slowest shape here (#1355), every arm in an optimised
/// build took about 0.6 s together, the release floor about half of it.
const WHOLE_VERDICT: Duration = Duration::from_secs(1);

/// How much slower the debug build the tests run in reads these shapes than an optimised one:
/// about ten times, measured on the same shapes (#1355), with room for a loaded CI runner. A
/// debug run is held to the optimised bound times this, so a change that made the optimised
/// verdict leave the budget fails here too.
const DEBUG_SLOWDOWN: u32 = 20;

#[test]
fn the_whole_verdict_at_the_largest_and_deepest_command_stays_well_inside_the_budget() {
    purlis_core::unsteered!();
    let dir = tempfile::tempdir().expect("a directory");
    let root = dir.path();
    std::fs::create_dir_all(root.join(".git")).expect("the git directory");
    std::fs::create_dir_all(root.join("workspaces/x")).expect("the chat folder");
    std::fs::create_dir_all(root.join("workspaces/x/repo/.git")).expect("the clone");
    let r = root.display().to_string();
    let state = root.join(".charter");
    let forges = vec![
        Forge::default_of(Kind::GitHub),
        Forge::default_of(Kind::GitLab),
    ];
    let chat_dir = root.join("workspaces/x").display().to_string();
    let plane = Plane {
        root: &r,
        forges: &forges,
        session_dir: &r,
        launched: Launched {
            sandboxed: true,
            chat_dir: Some(&chat_dir),
        },
    };
    // Just under the size cap, so every shape is read rather than refused for its size.
    let size = guardcaps::MAX_COMMAND_BYTES - 4 * 1024;
    let bound = if cfg!(debug_assertions) {
        WHOLE_VERDICT * DEBUG_SLOWDOWN
    } else {
        WHOLE_VERDICT
    };
    let mut slowest = (Duration::ZERO, String::new());
    // A shape that nests nothing is the same command at both depths, and is judged once: judged
    // again it measured the same verdict twice, a third of this test's minutes (#2619).
    let at_the_cap: std::collections::HashMap<&str, String> = shapes(MAX_NESTING)
        .into_iter()
        .map(|(shape, line)| (shape, line(size)))
        .collect();
    assert_eq!(
        at_the_cap.len(),
        shapes(MAX_NESTING).len(),
        "every shape has a name of its own"
    );
    for deep in [MAX_NESTING, MAX_NESTING + 1] {
        for (shape, line) in shapes(deep) {
            let cmd = line(size);
            assert!(cmd.len() <= guardcaps::MAX_COMMAND_BYTES, "{shape}");
            if deep > MAX_NESTING && at_the_cap.get(shape) == Some(&cmd) {
                continue;
            }
            for mode in ["default", "bypassPermissions"] {
                let call = Call {
                    command: &cmd,
                    cwd: &r,
                    state_dir: &state,
                    caller: Caller {
                        agent_id: None,
                        harness: Some("claude-code"),
                        permission_mode: Some(mode),
                    },
                };
                // The faster of two runs, as the family test takes, so a moment of load on the
                // machine is not the measure. The faster is inside the bound exactly when the
                // first is or the second is, so the second runs only when the first is not: the
                // same verdict for half of this test's minutes.
                let run = || {
                    let began = Instant::now();
                    let _ = toolgate::verdict(&call, Some(&plane));
                    began.elapsed()
                };
                let first = run();
                let took = if first < bound {
                    first
                } else {
                    first.min(run())
                };
                if took > slowest.0 {
                    slowest = (took, format!("{shape}, {deep} deep, {mode}"));
                }
            }
        }
    }
    let (took, which) = slowest;
    assert!(
        took < bound,
        "the slowest whole verdict, on {which}, took {took:?}"
    );
}

/// A repository whose every tracked path is one the sandbox denies, `files` of them: what the
/// clone update guard is answered where it asks about one.
struct ManyDenied {
    paths: Vec<String>,
}

impl ManyDenied {
    fn of(files: usize) -> Self {
        Self {
            paths: (0..files)
                .map(|i| format!("pkg{}/.vscode/task{i}.json", i % 97))
                .collect(),
        }
    }
}

impl cloneupdate::Asks for ManyDenied {
    fn top(&self, _: &str) -> cloneupdate::Top {
        cloneupdate::Top::At("/ws/repo".to_string())
    }
    fn head(&self, _: &str) -> cloneupdate::Head {
        cloneupdate::Head::At("h".to_string())
    }
    fn has_index(&self, _: &str) -> bool {
        true
    }
    fn commit(&self, _: &str, name: &str) -> Option<String> {
        Some(name.to_string())
    }
    fn guessed(&self, _: &str, _: &str) -> Option<String> {
        None
    }
    fn differ(&self, _: &str, _: &str, _: &str) -> Option<Vec<String>> {
        Some(self.paths.clone())
    }
    fn replayed(&self, _: &str, _: &str) -> Option<Vec<String>> {
        Some(self.paths.clone())
    }
    fn replayed_from(&self, _: &str, _: &str, _: &str) -> Option<Vec<String>> {
        Some(self.paths.clone())
    }
    fn tracked(&self, _: &str, _: &str) -> Option<Vec<String>> {
        Some(self.paths.clone())
    }
    fn changed(&self, _: &str, paths: &[String]) -> Option<Vec<String>> {
        Some(paths.to_vec())
    }
    fn is_there(&self, _: &Path) -> bool {
        true
    }
    fn left_out(&self, _: &str, _: &[String]) -> Option<Vec<String>> {
        Some(Vec::new())
    }
}

/// The other length a hook reads is the repository's: the clone update guard reads every path
/// a commit tracks and every path two commits differ in, and names those the sandbox denies.
/// Four times the paths may take a little over four times as long, as four times the command
/// may (the review of qw367: each denied path was looked for among those already found).
#[test]
fn the_clone_update_guard_reads_a_repository_in_time_proportional_to_its_paths() {
    purlis_core::unsteered!();
    let place = cloneupdate::Place {
        root: "/project",
        chat_dir: None,
        alone: true,
        sparse: true,
    };
    let words = |line: &str| -> (String, Vec<String>) {
        let mut words = line.split_whitespace().map(str::to_string);
        (words.next().expect("a subcommand"), words.collect())
    };
    // A named move, one that replays commits (each path twice), a forced one, one whose commit
    // cannot be named, and the way back.
    for line in [
        "merge other",
        "rebase other",
        "reset --hard other",
        "pull",
        "restore --source=HEAD --worktree -- src",
    ] {
        let (sub, post) = words(line);
        let time = |files: usize| {
            let repo = ManyDenied::of(files);
            (0..2)
                .map(|_| {
                    let began = Instant::now();
                    let _ = cloneupdate::judged(&sub, &post, "/ws/repo", &place, &repo);
                    began.elapsed()
                })
                .min()
                .unwrap_or_default()
        };
        let (t_small, t_large) = (time(20_000), time(80_000));
        assert!(
            t_large < t_small * 10 + Duration::from_millis(100),
            "git {line}: 20,000 paths took {t_small:?}, 80,000 took {t_large:?}"
        );
    }
}

/// The leak guard follows every `cd` on a line, since what a later command reads is read from
/// where the shell then stands. A long run of them is followed in time proportional to the
/// line: it once spelt the whole way there again for each one, the square of the line, which
/// the family test's two sizes sat too close to its allowance to tell from a slow runner
/// (qw377). Asked here at 16 KB and 64 KB, where the square is unmistakable, of a run that
/// goes back and forth, one that only goes deeper, and one with absolute stops in it.
#[test]
fn the_leak_guard_follows_a_long_run_of_relocations_in_time_proportional_to_its_length() {
    purlis_core::unsteered!();
    let dir = tempfile::tempdir().expect("a directory");
    let state = dir.path().join(".charter");
    for (shape, unit) in [
        ("back and forth", "cd a; cd ../b; "),
        ("deeper and deeper", "cd a; pushd b; "),
        (
            "back to the top now and then",
            "cd a; cd ../b; cd /tmp/x; cd c; ",
        ),
    ] {
        let time = |bytes: usize| {
            let cmd = format!("cd workspaces/x/repo; {} true", fill(unit, bytes));
            assert_eq!(guardcaps::refusal(&cmd), None, "{shape}");
            (0..3)
                .map(|_| {
                    let began = Instant::now();
                    let _ = leakguard::leak_reason(&cmd, "", &state);
                    began.elapsed()
                })
                .min()
                .unwrap_or_default()
        };
        let (t_small, t_large) = (time(16 * 1024), time(64 * 1024));
        assert!(
            t_large < t_small * 10 + Duration::from_millis(100),
            "the leak guard on a run of relocations, {shape}: 16 KB took {t_small:?}, 64 KB \
             took {t_large:?}"
        );
    }
}
