// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **Generated policies, nightly** (M75 slice 42, #7051; Q317 points 6 and 12; spec §14.6,
//! §14.11): policies written from a seed, inside the subset, asked the fixed cells and shuffled,
//! and the workflow that runs them nightly with fresh seeds and on a cedar bump's pull request.
//!
//! The generator and the cells are `policy_proof::generator`; what judges them is slice 09's
//! oracle against the engine (`policy_proof::equality`), and the sandbox's differential against
//! the authorizer (slice 22) takes the same seeds from the same variable.

use std::path::{Path, PathBuf};
use std::process::Command;

use crate::policy_proof::PolicyInput;
use crate::policy_proof::generator::{
    self, FIXED_SEEDS, POLICIES_PER_SEED, SEEDS_VARIABLE, cells, generate, seeds, seeds_from,
    shuffled,
};
use crate::policy_proof::{Answer, equality_of_one_policy, oracle};
use purlis_core::enforcement::Template;
use purlis_core::forklock;
use purlis_core::policy::decide::Outcome;
use purlis_core::policy::{File, Level, load};

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn workflow() -> String {
    std::fs::read_to_string(repo_root().join(".github/workflows/policy-nightly.yml"))
        .expect("the nightly workflow exists")
}

#[test]
fn the_generator_emits_only_subset_policies() {
    purlis_core::unsteered!();
    // A hundred and more seeds' worth: the subset is a property of what the generator can say,
    // not of the three seeds a pull request runs.
    let mut written = 0;
    for seed in 0..=40u64 {
        for policy in generate(seed) {
            let file = policy.file_text();
            if let Err(problems) = load(&[
                File::shipped_default(),
                File::at("policy.cedar", &file, Level::Project),
            ]) {
                panic!(
                    "seed {seed} policy {} is outside the subset: {problems:?}\n{file}",
                    policy.index
                );
            }
            written += 1;
        }
    }
    assert_eq!(written, 41 * POLICIES_PER_SEED);
}

#[test]
fn the_subset_check_refuses_what_the_generator_must_never_say() {
    purlis_core::unsteered!();
    // The proof that the test above can fail: a sandbox rule on a literal not spelt normally is
    // outside the subset and the loader names it.
    let outside = "// purlis-policy 1\n@id(\"sandbox-gen-0\") @level(\"project\")\nforbid (principal, action == Action::\"read\", resource) when { resource in Path::\"/plane/other/../other/z\" };\n";
    let Err(problems) = load(&[
        File::shipped_default(),
        File::at("policy.cedar", outside, Level::Project),
    ]) else {
        panic!("a path not spelt normally was admitted");
    };
    assert!(
        problems
            .iter()
            .any(|p| p.rule.as_deref() == Some("sandbox-gen-0")),
        "{problems:?}"
    );
}

/// Whether a generated rule (its `@id` carries `-gen-`) is among the rules that decided.
fn decided_by_a_generated_rule(answer: &Answer) -> bool {
    answer.deciding.iter().any(|id| id.contains("-gen-"))
}

#[test]
fn generated_policies_agree_with_enforcement_on_every_cell() {
    purlis_core::unsteered!();
    let baseline: Vec<Answer> = {
        let none = PolicyInput::template(Template::Strict);
        cells().iter().map(|c| oracle(&none, c)).collect()
    };
    let mut asked = 0;
    for seed in seeds() {
        let mut decided = 0;
        let mut moved = 0;
        for policy in generate(seed) {
            // Each seed asks its own order: an answer that depends on what came before it differs
            // between seeds.
            let ordered = shuffled(seed, cells().into_iter().enumerate().collect::<Vec<_>>());
            let (positions, asked_cells): (Vec<usize>, Vec<_>) = ordered.into_iter().unzip();
            let answers = match equality_of_one_policy(&policy.input(), &asked_cells) {
                Ok(answers) => answers,
                Err((at, mismatch)) => panic!(
                    "{mismatch:?} on {:?}\nseed {} ({SEEDS_VARIABLE}={} replays it), policy {}:\n{}",
                    asked_cells[at],
                    policy.seed,
                    policy.seed,
                    policy.index,
                    policy.file_text()
                ),
            };
            for (answer, at) in answers.iter().zip(&positions) {
                decided += usize::from(decided_by_a_generated_rule(answer));
                moved += usize::from(answer != &baseline[*at]);
            }
            asked += answers.len();
        }
        // The rules must decide, not only parse and agree: a seed whose rules never fire on any
        // cell, or never move an answer off the bare `strict` one, proved nothing.
        assert!(
            decided > 0,
            "seed {seed}: no generated rule decided any cell ({SEEDS_VARIABLE}={seed} replays it)"
        );
        assert!(
            moved > 0,
            "seed {seed}: no generated rule changed an answer ({SEEDS_VARIABLE}={seed} replays it)"
        );
    }
    assert_eq!(asked, seeds().len() * POLICIES_PER_SEED * cells().len());
}

#[test]
fn a_rule_that_never_fires_is_not_one_that_decided() {
    purlis_core::unsteered!();
    let by = |ids: &[&str]| Answer {
        outcome: Outcome::Deny,
        deciding: ids.iter().map(|s| (*s).to_owned()).collect(),
        after_mode: Outcome::Deny,
    };
    assert!(decided_by_a_generated_rule(&by(&["guards-git-gen-2"])));
    assert!(!decided_by_a_generated_rule(&by(&[
        "policy-everything-else"
    ])));
    assert!(!decided_by_a_generated_rule(&by(&[])));
}

#[test]
fn seeds_are_u64_end_to_end_and_the_high_bits_change_the_policies() {
    purlis_core::unsteered!();
    let big = u64::MAX - 3;
    assert_eq!(seeds_from(Some(&big.to_string())), vec![big]);
    assert_eq!(seeds_from(Some("18446744073709551615")), vec![u64::MAX]);
    assert!(std::panic::catch_unwind(|| seeds_from(Some("18446744073709551616"))).is_err());
    let text = |seed| -> Vec<String> { generate(seed).into_iter().map(|g| g.rules).collect() };
    // Seeds that differ only above the low byte write different policies.
    assert_ne!(text(1), text(1 + (1 << 8)));
    assert_ne!(text(1), text(1 + (1 << 40)));
}

#[test]
fn a_seed_writes_the_same_policies_and_the_same_order_every_time() {
    purlis_core::unsteered!();
    let text = |seed| -> Vec<String> { generate(seed).into_iter().map(|g| g.rules).collect() };
    assert_eq!(text(7), text(7));
    assert_ne!(text(7), text(8));
    let order = |seed| -> Vec<String> {
        shuffled(seed, cells())
            .iter()
            .map(|c| format!("{} {}", c.action, c.context))
            .collect()
    };
    assert_eq!(order(7), order(7));
    assert_ne!(order(7), order(8));
    let mut sorted_a = order(7);
    let mut sorted_b = order(8);
    sorted_a.sort();
    sorted_b.sort();
    assert_eq!(sorted_a, sorted_b, "a shuffle keeps every cell");
}

#[test]
fn the_nightly_seeds_come_from_the_environment_and_a_bad_one_is_refused() {
    purlis_core::unsteered!();
    assert_eq!(seeds_from(None), FIXED_SEEDS);
    assert_eq!(seeds_from(Some("")), FIXED_SEEDS);
    assert_eq!(seeds_from(Some("11, 23,200")), vec![11, 23, 200]);
    for bad in ["-1", "x", "1,,2"] {
        let caught = std::panic::catch_unwind(|| seeds_from(Some(bad)));
        assert!(caught.is_err(), "`{bad}` was taken for seeds");
    }
    // The nightly sets it three times over, so a night is three seeds, not the fixed ones.
    let yml = workflow();
    assert!(
        yml.contains(SEEDS_VARIABLE),
        "the workflow never sets {SEEDS_VARIABLE}"
    );
    let _ = generator::FIXED_SEEDS;
}

/// The `uses:` lines of a workflow: each pinned to a commit.
fn pinned(yml: &str) {
    for line in yml
        .lines()
        .filter(|l| l.trim_start().starts_with("- uses:") || l.trim_start().starts_with("uses:"))
    {
        let target = line.split("uses:").nth(1).unwrap().trim();
        if target.starts_with("./") {
            continue;
        }
        let sha = target.split('@').nth(1).unwrap_or_default();
        let sha = sha.split_whitespace().next().unwrap_or_default();
        assert!(
            sha.len() == 40 && sha.chars().all(|c| c.is_ascii_hexdigit()),
            "`{line}` is not pinned by a full commit SHA"
        );
    }
}

#[test]
fn a_cedar_version_bump_runs_the_nightly_set() {
    purlis_core::unsteered!();
    let yml = workflow();
    // Its triggers: nightly, by hand, and a pull request that touches the lockfile or the
    // dependency that names cedar. Never `pull_request_target`.
    assert!(
        yml.contains("schedule:") && yml.contains("cron:"),
        "no nightly schedule"
    );
    assert!(yml.contains("workflow_dispatch:"));
    assert!(
        yml.contains("pull_request:"),
        "a bump's pull request does not start it"
    );
    assert!(!yml.contains("pull_request_target"));
    for path in ["Cargo.lock", "crates/purlis-core/Cargo.toml"] {
        assert!(
            yml.contains(&format!("- {path}")),
            "a change to {path} does not start it"
        );
    }
    // The pull request runs the set only when a cedar crate moved, by the script this test also
    // runs below; the nightly and a dispatch run it always.
    assert!(
        yml.contains("tools/cedar-bump.sh"),
        "the job does not ask the script"
    );
    assert!(
        // Fail closed: the set is skipped only on an explicit `bumped=false`.
        yml.contains("needs.bump.outputs.bumped != 'false'")
            && yml.contains("set -euo pipefail")
            && yml.contains("github.event_name == 'schedule'")
            && yml.contains("github.event_name == 'workflow_dispatch'"),
        "the set's condition names no bump, no night and no dispatch"
    );
    // The same set three times, each on its own seed, through the test binary a pull request runs.
    assert!(yml.contains("cargo test") && yml.contains("--test it"));
    assert!(yml.contains("generated_policies_agree_with_enforcement_on_every_cell"));
    assert!(yml.contains("the_compiled_sandbox_equals_the_authorizer"));
    // The repository's rules for a workflow: no new secrets, nothing by default, pinned actions.
    assert!(yml.contains("permissions: {}"));
    assert!(yml.contains("contents: read"));
    assert!(
        !yml.contains("secrets."),
        "a nightly with no secrets reads none"
    );
    pinned(&yml);
}

/// A repository with a lockfile at two commits: `(base, head)`.
fn lock_history(base: &str, head: &str) -> (tempfile::TempDir, String, String) {
    let dir = tempfile::tempdir().expect("a temp dir");
    let git = |args: &[&str]| {
        let out = forklock::output(
            Command::new("git")
                .args(args)
                .current_dir(dir.path())
                .env("GIT_CONFIG_GLOBAL", "/dev/null")
                .env("GIT_CONFIG_SYSTEM", "/dev/null"),
        )
        .expect("git runs");
        assert!(
            out.status.success(),
            "git {args:?}: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        String::from_utf8_lossy(&out.stdout).trim().to_owned()
    };
    git(&["init", "-q", "-b", "main"]);
    git(&["config", "user.email", "t@example.com"]);
    git(&["config", "user.name", "t"]);
    git(&["config", "commit.gpgsign", "false"]);
    std::fs::write(dir.path().join("Cargo.lock"), base).unwrap();
    git(&["add", "Cargo.lock"]);
    git(&["commit", "-q", "-m", "base"]);
    let first = git(&["rev-parse", "HEAD"]);
    std::fs::write(dir.path().join("Cargo.lock"), head).unwrap();
    git(&["commit", "-q", "--allow-empty", "-am", "head"]);
    let second = git(&["rev-parse", "HEAD"]);
    (dir, first, second)
}

fn lock(cedar: &str, other: &str) -> String {
    format!(
        "version = 4\n\n[[package]]\nname = \"cedar-policy\"\nversion = \"{cedar}\"\n\n\
         [[package]]\nname = \"cedar-policy-core\"\nversion = \"{cedar}\"\n\n\
         [[package]]\nname = \"serde\"\nversion = \"{other}\"\n"
    )
}

fn bumped(base: &str, head: &str) -> String {
    let (dir, first, second) = lock_history(base, head);
    let out = forklock::output(
        Command::new("sh")
            .arg(repo_root().join("tools/cedar-bump.sh"))
            .args([&first, &second])
            .current_dir(dir.path()),
    )
    .expect("sh runs");
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8_lossy(&out.stdout).trim().to_owned()
}

#[test]
fn the_bump_script_says_true_only_when_a_cedar_crate_moved() {
    purlis_core::unsteered!();
    assert_eq!(
        bumped(&lock("4.14.1", "1.0.0"), &lock("4.15.0", "1.0.0")),
        "bumped=true"
    );
    // A move of another crate is not a cedar bump.
    assert_eq!(
        bumped(&lock("4.14.1", "1.0.0"), &lock("4.14.1", "1.0.1")),
        "bumped=false"
    );
    assert_eq!(
        bumped(&lock("4.14.1", "1.0.0"), &lock("4.14.1", "1.0.0")),
        "bumped=false"
    );
    // A cedar crate taken from another source at the same version is a move too.
    let git_sourced = lock("4.14.1", "1.0.0").replacen(
        "version = \"4.14.1\"\n",
        "version = \"4.14.1\"\nsource = \"git+https://example.invalid/cedar#abc\"\n",
        1,
    );
    assert_eq!(
        bumped(&lock("4.14.1", "1.0.0"), &git_sourced),
        "bumped=true"
    );
    // Fail closed: a base that has no lockfile, or a commit that does not exist, runs the set.
    let (dir, _, second) = lock_history(&lock("4.14.1", "1.0.0"), &lock("4.14.1", "1.0.0"));
    // An empty or missing commit would read the index: it runs the set.
    for args in [vec!["", second.as_str()], vec![second.as_str()]] {
        let out = forklock::output(
            Command::new("sh")
                .arg(repo_root().join("tools/cedar-bump.sh"))
                .args(&args)
                .current_dir(dir.path()),
        )
        .expect("sh runs");
        assert_eq!(
            String::from_utf8_lossy(&out.stdout).trim(),
            "bumped=true",
            "{args:?}"
        );
    }
    let out = forklock::output(
        Command::new("sh")
            .arg(repo_root().join("tools/cedar-bump.sh"))
            .args(["0000000000000000000000000000000000000000", &second])
            .current_dir(dir.path()),
    )
    .expect("sh runs");
    assert_eq!(String::from_utf8_lossy(&out.stdout).trim(), "bumped=true");
}
