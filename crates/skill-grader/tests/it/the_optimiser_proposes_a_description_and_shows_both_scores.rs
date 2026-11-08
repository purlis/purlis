// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! The description optimiser (#5740 of spec #5714): for one built-in with failing trigger tests,
//! a rewritten description proposed from those failures, graded by the CI grader, and both
//! scores side by side. It writes nothing unless asked, and no key is ever in what it says.
//!
//! The stand-in `claude` is the grading harness and the proposer at once: started with
//! purlis's plugin it grades, starting purlis-secrets on a query about a vault, and on one
//! about a token only where the plugin's `SKILL.md` says "token"; started without a plugin it
//! is the proposer, keeps what it was asked, and answers with the reply the test wrote.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use purlis_skills::grade::ci::Harness;
use skill_grader::optimise::{Optimise, Optimised, optimise};

use crate::fixture::{RECORDED_PLUGIN, fixture, recorded};

const KEY: &str = "the-test-key-0123456789";

const PROPOSAL: &str = "Use when a command or a deploy needs a password or an API token held in a \
                        purlis vault. Not for a website login — that is purlis-browser.";

struct Optimiser {
    fixture: crate::fixture::Fixture,
    /// What the proposer was asked.
    asked: PathBuf,
}

/// The grade fixture, its `claude` replaced by one that grades by the description and proposes
/// `reply`.
fn optimiser(reply: &str) -> Optimiser {
    let mut fixture = fixture(true, true);
    let dir = fixture.dir.path();
    let answer = dir.join("answer.jsonl");
    let line = serde_json::json!({
        "type": "result",
        "subtype": "success",
        "is_error": false,
        "result": reply,
    });
    std::fs::write(&answer, format!("{line}\n")).unwrap();
    let asked = dir.join("asked");
    let pick = |started: bool| recorded(Harness::ClaudeCode, started).display().to_string();
    let script = format!(
        r#"#!/bin/sh
printf '%s\n' "$@" >> '{argv}'
[ -n "$ANTHROPIC_API_KEY" ] || {{ echo 'no key' >&2; exit 3; }}
plugin=''
prev=''
for word in "$@"; do
  [ "$prev" = --plugin-dir ] && [ -z "$plugin" ] && plugin="$word"
  prev="$word"
  query="$word"
done
if [ -z "$plugin" ]; then
  printf '%s' "$query" > '{asked}'
  cat '{answer}'
  exit 0
fi
case "$query" in
  *vault*) out='{started}' ;;
  *token*)
    if grep -q token "$plugin/skills/purlis-secrets/SKILL.md"; then out='{started}'; else out='{not}'; fi ;;
  *) out='{not}' ;;
esac
sed "s#{recorded_plugin}#$plugin#g" "$out"
"#,
        argv = fixture.argv.display(),
        asked = asked.display(),
        answer = answer.display(),
        started = pick(true),
        not = pick(false),
        recorded_plugin = RECORDED_PLUGIN,
    );
    fixture.program = stand_in::program(dir, "claude-optimiser", &script);
    Optimiser { fixture, asked }
}

impl Optimiser {
    fn optimise(&self, write: bool) -> Result<Optimised, String> {
        optimise(&Optimise {
            ask: self.fixture.ask(Some(KEY)),
            skill: "purlis-secrets",
            proposer: &self.fixture.program,
            proposer_key: Some(KEY),
            write,
            // Off CI, as a developer runs it: Claude Code needs no trust bypassed.
            in_ci: false,
        })
    }

    fn skill_md(&self) -> PathBuf {
        self.fixture.plugin.join("skills/purlis-secrets/SKILL.md")
    }
}

/// Every file under `dir`, by its path, with its bytes.
fn files(dir: &Path) -> BTreeMap<PathBuf, Vec<u8>> {
    let mut all = BTreeMap::new();
    let mut folders = vec![dir.to_path_buf()];
    while let Some(folder) = folders.pop() {
        for entry in std::fs::read_dir(&folder).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                folders.push(path);
            } else {
                all.insert(path.clone(), std::fs::read(&path).unwrap());
            }
        }
    }
    all
}

fn said(optimised: Optimised) -> String {
    match optimised {
        Optimised::Proposed { said, .. } => said,
        other => panic!("nothing was proposed: {other:?}"),
    }
}

#[test]
fn a_skill_with_failing_tests_gets_a_proposal_from_them_and_both_scores_and_no_file_changes() {
    let optimiser = optimiser(PROPOSAL);
    let before = files(&optimiser.fixture.plugin);
    let said = said(optimiser.optimise(false).unwrap());
    for line in [
        "purlis-secrets",
        "Claude Code",
        "> Use a credential held in a purlis vault.",
        &format!("> {PROPOSAL}"),
        "| crowded | 50% (1 of 2) | 100% (2 of 2) | 0% (0 of 2) | 0% (0 of 2) |",
        "| bare | 50% (1 of 2) | 100% (2 of 2) | 0% (0 of 2) | 0% (0 of 2) |",
        "below its strict bar",
        "meets its strict bar",
        "Nothing was written",
    ] {
        assert!(said.contains(line), "no {line:?} in:\n{said}");
    }
    // Proposed from the failing test, with the current description and the house style.
    let asked = std::fs::read_to_string(&optimiser.asked).unwrap();
    for line in [
        "store this API token so the deploy can use it",
        "Use a credential held in a purlis vault.",
        "purlis-browser",
        "1,024",
    ] {
        assert!(
            asked.contains(line),
            "the proposer was not told {line:?}:\n{asked}"
        );
    }
    assert_eq!(
        files(&optimiser.fixture.plugin),
        before,
        "a file of the plugin changed"
    );
}

#[test]
fn asked_to_write_it_puts_the_proposal_in_skill_md_and_changes_nothing_else() {
    let optimiser = optimiser(PROPOSAL);
    let skill_md = optimiser.skill_md();
    std::fs::write(
        &skill_md,
        "---\nname: purlis-secrets\ndescription: Use a credential held in a purlis vault.\n\
         license: Apache-2.0\n---\n\n# Secrets\n\nThe body, as written.\n",
    )
    .unwrap();
    let Optimised::Proposed { said, written } = optimiser.optimise(true).unwrap() else {
        panic!("nothing was proposed");
    };
    assert!(written, "{said}");
    assert!(said.contains("Written to"), "{said}");
    assert_eq!(
        std::fs::read_to_string(&skill_md).unwrap(),
        format!(
            "---\nname: purlis-secrets\ndescription: {PROPOSAL}\nlicense: Apache-2.0\n---\n\n\
             # Secrets\n\nThe body, as written.\n"
        )
    );
}

#[test]
fn no_key_is_in_what_it_says_or_writes_even_where_the_proposer_repeats_it() {
    let optimiser = optimiser(&format!("{PROPOSAL} Your key is {KEY}."));
    let Optimised::Proposed { said, .. } = optimiser.optimise(true).unwrap() else {
        panic!("nothing was proposed");
    };
    assert!(!said.contains(KEY), "the key is in:\n{said}");
    let written = std::fs::read_to_string(optimiser.skill_md()).unwrap();
    assert!(!written.contains(KEY), "the key was written:\n{written}");
    let argv = std::fs::read_to_string(&optimiser.fixture.argv).unwrap();
    assert!(!argv.contains(KEY), "the key reached a command line");
}

#[test]
fn a_skill_whose_every_test_passes_gets_no_proposal_and_no_proposer_is_started() {
    let optimiser = optimiser(PROPOSAL);
    std::fs::write(
        optimiser.skill_md(),
        "---\nname: purlis-secrets\ndescription: A purlis vault's password or token.\n---\n",
    )
    .unwrap();
    let Optimised::Nothing(said) = optimiser.optimise(false).unwrap() else {
        panic!("a proposal was made for a skill whose every test passes");
    };
    assert!(said.contains("every trigger test passes"), "{said}");
    assert!(!optimiser.asked.exists(), "the proposer was started");
}

#[test]
fn without_a_key_nothing_is_started_and_it_says_why() {
    let optimiser = optimiser(PROPOSAL);
    let optimised = optimise(&Optimise {
        ask: optimiser.fixture.ask(None),
        skill: "purlis-secrets",
        proposer: &optimiser.fixture.program,
        proposer_key: None,
        write: false,
        in_ci: false,
    })
    .unwrap();
    let Optimised::Skipped(said) = optimised else {
        panic!("it ran without a key: {optimised:?}");
    };
    assert!(said.contains("ANTHROPIC_API_KEY"), "{said}");
    assert!(said.contains("purlis secret exec"), "{said}");
    assert!(!optimiser.fixture.argv.exists(), "a harness was started");
}

#[test]
fn off_ci_it_refuses_to_grade_codex_since_that_bypasses_the_trust_codex_asks_for_its_hooks() {
    let fixture = crate::fixture::fixture_on(Harness::Codex, true, true);
    let refused = optimise(&Optimise {
        ask: fixture.ask(Some(KEY)),
        skill: "purlis-secrets",
        proposer: &fixture.program,
        proposer_key: Some(KEY),
        write: false,
        in_ci: false,
    })
    .expect_err("it graded Codex off CI");
    for said in ["--dangerously-bypass-hook-trust", "CI=true", "Codex"] {
        assert!(refused.contains(said), "no {said:?} in: {refused}");
    }
    assert!(!fixture.argv.exists(), "a harness was started");
}
