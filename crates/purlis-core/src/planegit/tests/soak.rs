// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The soak** (#5824 F10, acceptance): two machines saving one plane in a request mode for
//! many rounds, against real git and a bare remote standing in for the forge.
//!
//! Each round both machines write what chats write — a session record with the index and the
//! `## Sessions` line generated from it, a todo with its index, and words of their own in files
//! of their own — and save. One lands first: the forge merges its request by merge, squash or
//! rebase in turn, and it settles with a save written since ([`When::MergedAt`]). The other's
//! request is open meanwhile, so it brings the moved target in ([`When::Moved`]), lands, and
//! settles in turn. Every step is asserted green, nothing either side wrote is lost, and no
//! stop is ever left at either root.
//!
//! Memory indexes and `[sandbox] hosts` (#5824 F4, F5) are lane B's, and soak in a test of their
//! own that is ignored until lane B lands.

use std::collections::BTreeMap;
use std::path::Path;

use crate::planegit::bringin::{BroughtIn, When, bring_in};
use super::twoclones::{Two, commit, read, run, write};

const WS: &str = "workspaces/alpha";
const CHARTER: &str = "# alpha\n\n## Vision\n\nShip it\n\n## Sessions\n\nNo session records yet.\n";
const ROUNDS: usize = 30;
/// The soak stops early rather than run past this, and says how far it got.
const AT_MOST: std::time::Duration = std::time::Duration::from_secs(240);

/// The forge's three ways to merge a request, in turn.
const METHODS: [&str; 3] = ["merge", "squash", "rebase"];

struct Soak {
    two: Two,
    /// A working clone of the bare remote, where the forge merges.
    forge: std::path::PathBuf,
    /// Every file either machine wrote that must survive, with its text.
    written: BTreeMap<String, String>,
    /// Files either machine deleted, which must stay deleted.
    deleted: Vec<String>,
    lane_b: bool,
}

impl Soak {
    fn new(lane_b: bool) -> Soak {
        let two = Two::new(&[
            (&format!("{WS}/workspace.md"), CHARTER),
            (&format!("{WS}/sessions/index.md"), "# Sessions\n\n"),
            (&format!("{WS}/todos/MEMORY.md"), "# Todos\n\n"),
            ("personas/_shared/memory/MEMORY.md", "# Memory\n\n"),
        ]);
        let forge = two.bare.parent().unwrap().join("forge");
        run(
            two.bare.parent().unwrap(),
            &[
                "clone",
                "-q",
                two.bare.to_str().unwrap(),
                forge.to_str().unwrap(),
            ],
        );
        run(&forge, &["config", "user.name", "Forge"]);
        run(&forge, &["config", "user.email", "forge@example.invalid"]);
        Soak {
            two,
            forge,
            written: BTreeMap::new(),
            deleted: Vec::new(),
            lane_b,
        }
    }

    fn clone(&self, machine: &str) -> &Path {
        if machine == "mine" {
            &self.two.mine
        } else {
            &self.two.theirs
        }
    }

    fn keep(&mut self, root: &Path, file: &str, text: &str) {
        write(root, file, text);
        self.written.insert(file.to_owned(), text.to_owned());
    }

    /// What a round of chats on `machine` writes, as purlis writes it, then a save.
    fn chats_write(&mut self, machine: &str, round: usize) {
        let root = self.clone(machine).to_path_buf();
        let at = format!(
            "2026{:02}{:02}-0000{:02}",
            1 + round / 28,
            1 + round % 28,
            u8::from(machine == "theirs")
        );
        // A session record, and what purlis generates from this machine's records.
        let record = format!("{WS}/sessions/{at}-{machine}.md");
        self.keep(
            &root,
            &record,
            &format!("---\ntitle: round {round} on {machine}\n---\n\nWhat happened.\n"),
        );
        let records = listed(&root, &format!("{WS}/sessions"), "index.md");
        let index: String = records.iter().map(|name| format!("- {name}\n")).collect();
        write(
            &root,
            &format!("{WS}/sessions/index.md"),
            &format!("# Sessions\n\n{index}"),
        );
        let charter = read(&root, &format!("{WS}/workspace.md"));
        write(
            &root,
            &format!("{WS}/workspace.md"),
            &crate::mdsection::replace(
                &charter,
                "Sessions",
                &format!(
                    "{} session records — latest round {round} on {machine}",
                    records.len()
                ),
            ),
        );
        // A todo, and its index as this machine has it.
        self.keep(
            &root,
            &format!("{WS}/todos/{at}-{machine}.md"),
            &format!("---\nname: todo {round} {machine}\n---\n\nDo it.\n"),
        );
        let todos = listed(&root, &format!("{WS}/todos"), "MEMORY.md");
        let index: String = todos
            .iter()
            .map(|name| format!("- [{name}]({name})\n"))
            .collect();
        write(
            &root,
            &format!("{WS}/todos/MEMORY.md"),
            &format!("# Todos\n\n{index}"),
        );
        // Words of its own, in a file of its own; and, now and then, one of its own deleted.
        self.keep(
            &root,
            &format!("notes/{machine}/{round}.md"),
            &format!("{machine} wrote this in round {round}.\n"),
        );
        if round >= 2 && round.is_multiple_of(5) {
            let gone = format!("notes/{machine}/{}.md", round - 2);
            std::fs::remove_file(root.join(&gone)).unwrap();
            self.written.remove(&gone);
            self.deleted.push(gone);
        }
        if self.lane_b {
            // A memory file and its line in the index; an Always-allowed host.
            let memory = format!("personas/_shared/memory/{machine}-{round}.md");
            self.keep(
                &root,
                &memory,
                &format!("---\nname: {machine} {round}\n---\n\nA fact.\n"),
            );
            let index = read(&root, "personas/_shared/memory/MEMORY.md");
            write(
                &root,
                "personas/_shared/memory/MEMORY.md",
                &format!("{index}- [{machine} {round}]({machine}-{round}.md)\n"),
            );
            let toml = read(&root, "charter.toml");
            let host = format!("{machine}-{round}.example");
            let toml = match toml.split_once("hosts = [") {
                Some((head, rest)) => format!("{head}hosts = [\"{host}\", {rest}"),
                None => format!("{toml}\n[sandbox]\nhosts = [\"{host}\"]\n"),
            };
            write(&root, "charter.toml", &toml);
        }
        commit(&root, &format!("round {round} on {machine}"));
    }

    /// `machine`'s save branch merged into `release` by `method`, as the forge does it. Answers
    /// the commit the save branch was pushed at.
    fn lands(&self, machine: &str, method: &str) -> String {
        let root = self.clone(machine);
        let head = run(root, &["rev-parse", "HEAD"]);
        let save = format!("save/{machine}");
        run(
            root,
            &[
                "push",
                "-q",
                "-f",
                "origin",
                &format!("HEAD:refs/heads/{save}"),
            ],
        );
        let forge = &self.forge;
        run(forge, &["fetch", "-q", "origin"]);
        run(
            forge,
            &["checkout", "-q", "-B", "release", "origin/release"],
        );
        let theirs = format!("origin/{save}");
        match method {
            "merge" => {
                run(
                    forge,
                    &["merge", "-q", "--no-ff", "-m", "Merge request", &theirs],
                );
            }
            "squash" => {
                run(forge, &["merge", "-q", "--squash", &theirs]);
                run(forge, &["commit", "-q", "-m", "Squashed request"]);
            }
            "rebase" => {
                // A request the forge cannot replay commit by commit is refused for this method
                // (GitHub's `canBeRebased`), and purlis's merge takes the next one the
                // repository allows: a merge commit.
                run(forge, &["checkout", "-q", "-B", "rebased", &theirs]);
                if crate::testgit::run(forge, &["rebase", "-q", "release"]).ok() {
                    run(forge, &["checkout", "-q", "release"]);
                    run(forge, &["merge", "-q", "--ff-only", "rebased"]);
                } else {
                    run(forge, &["rebase", "--abort"]);
                    run(forge, &["checkout", "-q", "release"]);
                    run(
                        forge,
                        &["merge", "-q", "--no-ff", "-m", "Merge request", &theirs],
                    );
                }
            }
            other => panic!("no merge called {other}"),
        }
        run(forge, &["push", "-q", "origin", "release"]);
        head
    }

    /// `machine` fetches the remote and brings the target in.
    fn brings_in(&self, machine: &str, when: When, round: usize) {
        let root = self.clone(machine);
        run(root, &["fetch", "-q", "origin"]);
        let got = bring_in(root, "release", false, when);
        assert!(
            matches!(got, BroughtIn::Moved(_) | BroughtIn::Nothing),
            "round {round}, {machine} ({when:?}): {got:?}"
        );
        self.no_stop_is_left(round);
    }

    fn no_stop_is_left(&self, round: usize) {
        for root in [&self.two.mine, &self.two.theirs] {
            for stop in ["rebase-merge", "rebase-apply", "MERGE_HEAD"] {
                assert!(
                    !root.join(".git").join(stop).exists(),
                    "round {round}: {stop} at {}",
                    root.display()
                );
            }
            assert_eq!(
                run(root, &["symbolic-ref", "HEAD"]),
                "refs/heads/release",
                "round {round}: {} is on its branch",
                root.display()
            );
        }
    }

    /// Nothing either machine wrote is lost, on the remote's `release` and in each clone.
    fn nothing_is_lost(&self, round: usize) {
        let tip = run(&self.two.bare, &["rev-parse", "release"]);
        for (file, text) in &self.written {
            let done = crate::testgit::run(&self.two.bare, &["show", &format!("{tip}:{file}")]);
            assert!(done.ok(), "round {round}: {file} is not on the remote");
            assert_eq!(&done.out, text, "round {round}: {file} on the remote");
        }
        for gone in &self.deleted {
            let done = crate::testgit::run(
                &self.two.bare,
                &["cat-file", "-e", &format!("{tip}:{gone}")],
            );
            assert!(!done.ok(), "round {round}: {gone} came back");
        }
        let records = self
            .written
            .keys()
            .filter(|f| f.contains("/sessions/"))
            .count();
        let index = crate::testgit::run(
            &self.two.bare,
            &["show", &format!("{tip}:{WS}/sessions/index.md")],
        )
        .out;
        assert!(!index.contains("<<<<<<<"), "round {round}: {index}");
        let todos = crate::testgit::run(
            &self.two.bare,
            &["show", &format!("{tip}:{WS}/todos/MEMORY.md")],
        )
        .out;
        for todo in self.written.keys().filter(|f| f.contains("/todos/")) {
            let name = todo.rsplit('/').next().unwrap();
            assert!(
                todos.contains(name),
                "round {round}: {name} not in the todos index:\n{todos}"
            );
        }
        let charter = crate::testgit::run(
            &self.two.bare,
            &["show", &format!("{tip}:{WS}/workspace.md")],
        )
        .out;
        assert!(!charter.contains("<<<<<<<"), "round {round}: {charter}");
        assert!(records > 0);
        if self.lane_b {
            let memory = crate::testgit::run(
                &self.two.bare,
                &["show", &format!("{tip}:personas/_shared/memory/MEMORY.md")],
            )
            .out;
            for file in self
                .written
                .keys()
                .filter(|f| f.starts_with("personas/_shared/memory/") && !f.ends_with("MEMORY.md"))
            {
                let name = file.rsplit('/').next().unwrap();
                assert_eq!(
                    memory.matches(&format!("({name})")).count(),
                    1,
                    "round {round}: {name} once in:\n{memory}"
                );
            }
            let toml =
                crate::testgit::run(&self.two.bare, &["show", &format!("{tip}:charter.toml")]).out;
            for round_ in 0..=round {
                for machine in ["mine", "theirs"] {
                    let host = format!("{machine}-{round_}.example");
                    assert!(
                        toml.contains(&host),
                        "round {round}: {host} not allowed:\n{toml}"
                    );
                }
            }
        }
    }

    /// One round: both machines write and save, one lands and settles, then the other.
    fn round(&mut self, round: usize) {
        let (first, second) = if round.is_multiple_of(2) {
            ("mine", "theirs")
        } else {
            ("theirs", "mine")
        };
        // Each machine brings in what the remote has since its last round.
        self.brings_in(first, When::Moved, round);
        self.brings_in(second, When::Moved, round);
        self.chats_write(first, round);
        self.chats_write(second, round);
        // The first lands, and a chat saves on it again before it settles.
        let pushed = self.lands(first, METHODS[round % METHODS.len()]);
        let root = self.clone(first).to_path_buf();
        self.keep(
            &root,
            &format!("notes/{first}/{round}-after.md"),
            "written while it merged\n",
        );
        commit(&root, "written while it merged");
        self.brings_in(first, When::MergedAt(&pushed), round);
        // The second's request is open while the target moved: it brings it in, then lands.
        self.brings_in(second, When::Moved, round);
        let pushed = self.lands(second, METHODS[(round + 1) % METHODS.len()]);
        self.brings_in(second, When::MergedAt(&pushed), round);
        // The first lands what it wrote while its request merged.
        self.brings_in(first, When::Moved, round);
        let pushed = self.lands(first, METHODS[(round + 2) % METHODS.len()]);
        self.brings_in(first, When::MergedAt(&pushed), round);
        self.nothing_is_lost(round);
    }
}

/// The files directly in `dir` of `root`, but `index`, sorted.
fn listed(root: &Path, dir: &str, index: &str) -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(root.join(dir))
        .unwrap()
        .flatten()
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .filter(|name| name != index)
        .collect();
    names.sort();
    names
}

fn soak(lane_b: bool) {
    let mut soak = Soak::new(lane_b);
    let started = std::time::Instant::now();
    let mut rounds = 0;
    while rounds < ROUNDS && started.elapsed() < AT_MOST {
        soak.round(rounds);
        rounds += 1;
    }
    use std::io::Write;
    let _ = writeln!(
        std::io::stderr(),
        "the soak ran {rounds} rounds in {:?}",
        started.elapsed()
    );
    assert!(rounds >= 10, "only {rounds} rounds ran within {AT_MOST:?}");
}

#[test]
fn two_machines_saving_for_many_rounds_stay_green_and_lose_nothing() {
    soak(false);
}

#[test]
#[ignore = "lane B, #5824 F4/F5"]
fn two_machines_saving_memory_and_host_allows_for_many_rounds_stay_green_and_lose_nothing() {
    soak(true);
}
