// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! Every workflow runs third-party actions by commit, and a release job that holds a signing
//! secret or publishes restores no cache (ADR 0042, amendment of 2026-09-29).
//!
//! A tag or a branch is a name its owner can move. A commit SHA names one tree, so what a job
//! runs is what was reviewed when the pin was written. The version the SHA was resolved from
//! stays beside it as a comment, which is what Dependabot reads to propose the next pin.
//!
//! A cache is written by whichever job saved it last. The release jobs that hold the keys, or
//! can write the release, build from their own checkout and nothing else, which is slower and
//! is the point. `ci`, `stress` and `mutants` keep their caches.
//!
//! Both rules follow a step into the repository's own composite actions (`uses:
//! ./.github/actions/<name>`, #1625): the action is pinned by the checkout, but the actions it
//! runs and the caches it restores are as much the job's as its own steps.

use crate::workflow;

use workflow::{
    Job, action_steps, local_action, local_action_files, release_jobs, step_uses, uses_value,
    workflow_files,
};

/// Why `value`, written after `uses:`, is not pinned by commit, or `None` if it is. `comment`
/// is the text after the value's `#`, if any.
fn unpinned(value: &str, comment: Option<&str>) -> Option<String> {
    // A local action is this repository's own code, pinned by the checkout itself.
    if value.starts_with("./") {
        return None;
    }
    if let Some(image) = value.strip_prefix("docker://") {
        return (!image.contains("@sha256:")).then(|| "a docker image not pinned by digest".into());
    }
    let Some((action, reference)) = value.rsplit_once('@') else {
        return Some("no `@ref` at all".into());
    };
    if action.split('/').count() < 2 {
        return Some(format!("`{action}` is not `owner/repo`"));
    }
    let is_sha = reference.len() == 40
        && reference
            .chars()
            .all(|c| c.is_ascii_digit() || ('a'..='f').contains(&c));
    if !is_sha {
        return Some(format!(
            "`@{reference}` is a tag or branch, not a full commit SHA"
        ));
    }
    match comment.map(str::trim) {
        Some(version) if !version.is_empty() => None,
        _ => Some("no `# <version>` comment naming what the SHA was resolved from".into()),
    }
}

/// Every `uses:` line in `text` that is not pinned, as `line: value — why`.
fn unpinned_lines(text: &str) -> Vec<String> {
    text.lines()
        .enumerate()
        .filter(|(_, line)| !line.trim_start().starts_with('#'))
        .filter_map(|(i, line)| {
            let value = uses_value(line)?;
            let comment = line.split_once(" #").map(|(_, c)| c);
            unpinned(value, comment).map(|why| format!("{}: {value} — {why}", i + 1))
        })
        .collect()
}

#[test]
fn the_pin_rule_refuses_a_tag_a_branch_and_a_bare_sha_and_takes_a_commented_sha() {
    let sha = "0123456789abcdef0123456789abcdef01234567";
    assert!(unpinned("actions/checkout@v4", Some("v4")).is_some());
    assert!(unpinned("dtolnay/rust-toolchain@stable", None).is_some());
    assert!(
        unpinned("actions/checkout@0123456", Some("v4")).is_some(),
        "a short SHA"
    );
    assert!(
        unpinned(
            &format!("actions/checkout@{}", sha.to_uppercase()),
            Some("v4")
        )
        .is_some(),
        "GitHub's SHAs are lowercase; anything else is a ref name that looks like one"
    );
    assert!(
        unpinned(&format!("actions/checkout@{sha}"), None).is_some(),
        "no version comment"
    );
    assert!(unpinned(&format!("actions/checkout@{sha}"), Some(" ")).is_some());
    assert!(unpinned("docker://alpine:3", None).is_some());

    assert_eq!(
        unpinned(&format!("actions/checkout@{sha}"), Some(" v4.2.2")),
        None
    );
    assert_eq!(
        unpinned(&format!("github/codeql-action/init@{sha}"), Some("v3.1.0")),
        None
    );
    assert_eq!(unpinned("./.github/actions/local", None), None);
    assert_eq!(
        unpinned(&format!("docker://alpine@sha256:{sha}"), None),
        None
    );
}

#[test]
fn the_line_reader_finds_uses_in_every_position_a_workflow_writes_it() {
    let sha = "0123456789abcdef0123456789abcdef01234567";
    let text = format!(
        "jobs:\n  a:\n    uses: org/repo/.github/workflows/w.yml@main\n    steps:\n      - uses: actions/checkout@v4\n      - name: x\n        uses: \"actions/setup-node@v4\"\n      # - uses: commented/out@v1\n      - uses: actions/cache@{sha} # v4.0.0\n"
    );
    let found = unpinned_lines(&text);
    assert_eq!(found.len(), 3, "{found:#?}");
    assert!(found[0].starts_with("3: org/repo/.github/workflows/w.yml@main"));
    assert!(found[1].starts_with("5: actions/checkout@v4"));
    assert!(found[2].starts_with("7: actions/setup-node@v4"));
}

#[test]
fn every_action_every_workflow_runs_is_pinned_by_commit() {
    let files = workflow_files();
    assert!(files.len() >= 4, "found only {files:?}");
    let mut problems = Vec::new();
    for path in &files {
        let name = path.file_name().unwrap().to_string_lossy().into_owned();
        for problem in unpinned_lines(&workflow::read(path)) {
            problems.push(format!("{name}:{problem}"));
        }
    }
    // The composite actions the workflows call run third-party actions of their own (#1625).
    let actions = local_action_files();
    assert!(actions.len() >= 2, "found only {actions:?}");
    for path in &actions {
        let name = path
            .strip_prefix(workflow::workflows_dir().join(".."))
            .unwrap_or(path)
            .display()
            .to_string();
        for problem in unpinned_lines(&workflow::read(path)) {
            problems.push(format!("{name}:{problem}"));
        }
    }
    assert!(
        problems.is_empty(),
        "pin each of these to the commit its tag resolves to, with the tag as a comment \
         (`gh api repos/<owner>/<repo>/git/ref/tags/<tag>`, and for an annotated tag \
         `gh api repos/<owner>/<repo>/git/tags/<sha>` once more):\n{}",
        problems.join("\n")
    );
}

/// A job that holds what a release is made with: a secret, the `release` environment, the
/// power to publish, or the OIDC token that signs its provenance (#583).
fn is_privileged(job: &Job) -> bool {
    job.reads_a_secret()
        || job.publishes()
        || job.environment().is_some()
        || job.can_mint_an_oidc_token()
}

/// Why this step restores or saves a cache, or `None`.
fn caches(step: &[String]) -> Option<&'static str> {
    let action = step_uses(step)?.split('@').next().unwrap_or_default();
    let with = |key: &str| {
        step.iter()
            .find_map(|l| l.trim().strip_prefix(key).map(str::trim))
    };
    match action {
        "Swatinem/rust-cache" => Some("rust-cache restores a cache another job saved"),
        "actions/cache" | "actions/cache/restore" | "actions/cache/save" => {
            Some("actions/cache restores or saves a cache")
        }
        "actions/setup-node" if with("cache:").is_some() => {
            Some("setup-node with `cache:` restores the npm cache, and saves one on a miss")
        }
        // setup-node turns its cache on by itself when package.json names npm as its package
        // manager, so off has to be said, not just not asked for.
        "actions/setup-node" if with("package-manager-cache:") != Some("false") => {
            Some("setup-node without `package-manager-cache: false` may cache on its own")
        }
        _ => None,
    }
}

/// Why this step caches, followed into the composite action it calls when it calls one of the
/// repository's own (#1625). `action_text` reads `./.github/actions/<name>`'s `action.yml`.
fn caches_through(step: &[String], action_text: &dyn Fn(&str) -> Option<String>) -> Option<String> {
    if let Some(why) = caches(step) {
        return Some(why.to_owned());
    }
    let called = step_uses(step)?;
    if !called.starts_with("./") {
        return None;
    }
    let Some(text) = action_text(called) else {
        return Some(format!("{called} is not an action this repository has"));
    };
    action_steps(&text).iter().find_map(|inner| {
        caches_through(inner, action_text)
            .map(|why| format!("{called} runs a step that caches: {why}"))
    })
}

/// The text of the repository's own action `value` (`./.github/actions/<name>`), if it has one.
fn repository_action(value: &str) -> Option<String> {
    local_action(value).map(|path| workflow::read(&path))
}

#[test]
fn the_cache_rule_follows_a_step_into_the_composite_action_it_calls() {
    let sha = "0123456789abcdef0123456789abcdef01234567";
    let caching = format!(
        "name: x\nruns:\n  using: composite\n  steps:\n    - shell: bash\n      run: echo\n    - uses: actions/cache/restore@{sha} # v6\n      with:\n        path: ~/x\n"
    );
    let quiet =
        "name: y\nruns:\n  using: composite\n  steps:\n    - shell: bash\n      run: echo\n";
    let nesting =
        "name: z\nruns:\n  using: composite\n  steps:\n    - uses: ./.github/actions/caching\n";
    let text = |value: &str| match value {
        "./.github/actions/caching" => Some(caching.clone()),
        "./.github/actions/quiet" => Some(quiet.to_owned()),
        "./.github/actions/nesting" => Some(nesting.to_owned()),
        _ => None,
    };
    let step = |value: &str| vec![format!("      - uses: {value}")];
    let why = caches_through(&step("./.github/actions/caching"), &text).expect("it caches");
    assert!(
        why.contains("actions/cache restores or saves a cache"),
        "{why}"
    );
    assert!(
        caches_through(&step("./.github/actions/nesting"), &text).is_some(),
        "two deep"
    );
    assert_eq!(
        caches_through(&step("./.github/actions/quiet"), &text),
        None
    );
    assert!(
        caches_through(&step("./.github/actions/gone"), &text).is_some(),
        "an action that cannot be read cannot be shown to cache nothing"
    );
}

#[test]
fn the_repositorys_own_actions_are_read_and_the_package_cache_is_seen_in_one() {
    // The Linux packages action restores and saves a cache, so a privileged job calling it
    // would be refused; the toolchain action caches nothing.
    let packages = vec!["      - uses: ./.github/actions/linux-packages".to_owned()];
    assert!(caches_through(&packages, &repository_action).is_some());
    let toolchain = vec!["      - uses: ./.github/actions/rust-toolchain".to_owned()];
    assert_eq!(caches_through(&toolchain, &repository_action), None);
}

#[test]
fn the_cache_rule_sees_rust_cache_actions_cache_and_both_setup_node_caches() {
    let step = |lines: &[&str]| lines.iter().map(|l| (*l).to_owned()).collect::<Vec<_>>();
    let sha = "0123456789abcdef0123456789abcdef01234567";
    assert!(
        caches(&step(&[&format!(
            "      - uses: Swatinem/rust-cache@{sha} # v2"
        )]))
        .is_some()
    );
    assert!(caches(&step(&["      - uses: actions/cache/restore@v4"])).is_some());
    assert!(
        caches(&step(&[
            "      - uses: actions/setup-node@v4",
            "        with:",
            "          cache: npm",
            "          package-manager-cache: false",
        ]))
        .is_some()
    );
    assert!(
        caches(&step(&[
            "      - uses: actions/setup-node@v4",
            "        with:",
            "          node-version-file: .nvmrc",
        ]))
        .is_some()
    );
    assert_eq!(
        caches(&step(&[
            "      - uses: actions/setup-node@v4",
            "        with:",
            "          node-version-file: .nvmrc",
            "          package-manager-cache: false",
        ])),
        None
    );
    assert_eq!(caches(&step(&["      - run: npm ci"])), None);
}

#[test]
fn no_release_job_that_holds_a_key_or_publishes_restores_a_cache() {
    let jobs = release_jobs();
    let privileged: Vec<&Job> = jobs.iter().filter(|job| is_privileged(job)).collect();
    // Found by what they hold, so a new job with a secret is held to this without being named.
    let names: Vec<&str> = privileged.iter().map(|job| job.name.as_str()).collect();
    assert_eq!(
        names,
        [
            "plan",
            "build",
            "runner-image",
            "runner-image-signature",
            "provenance",
            "publish"
        ],
        "{names:?}"
    );

    let mut problems = Vec::new();
    for job in privileged {
        for step in job.steps() {
            if let Some(why) = caches_through(&step, &repository_action) {
                problems.push(format!("job `{}`: {} ({why})", job.name, step[0].trim()));
            }
        }
    }
    assert!(problems.is_empty(), "{}", problems.join("\n"));
}

#[test]
fn no_job_that_builds_the_releases_commit_caches_through_an_action_it_calls() {
    // The rule above follows composite actions only for the jobs that hold a key or publish.
    // `manifest` holds nothing but writes what the updater reads, and `runner-image.yml`'s
    // `binary` builds the purlis the signed image carries; each calls the toolchain action
    // on the commit `plan` chose. So every job of both workflows is held to caching nothing,
    // through every action it calls, and an action that starts caching fails here.
    let dir = workflow::workflows_dir();
    let mut problems = Vec::new();
    for file in ["release.yml", "runner-image.yml"] {
        for job in workflow::jobs(&workflow::read(&dir.join(file))) {
            for step in job.steps() {
                if let Some(why) = caches_through(&step, &repository_action) {
                    problems.push(format!(
                        "{file}: job `{}`: {} ({why})",
                        job.name,
                        step[0].trim()
                    ));
                }
            }
        }
    }
    assert!(problems.is_empty(), "{}", problems.join("\n"));
}

#[test]
fn the_other_workflows_keep_their_caches() {
    // The rule above is for the release's privileged jobs, not a ban on caching: `ci` still
    // restores and saves one, which is what makes a PR's checks quick.
    let ci = workflow::jobs(&workflow::read(&workflow::workflows_dir().join("ci.yml")));
    let cached = ci
        .iter()
        .flat_map(Job::steps)
        .filter(|step| caches(step).is_some())
        .count();
    assert!(cached > 0, "ci.yml restores no cache at all");
}

#[test]
fn dependabot_keeps_the_action_pins_current() {
    let path = workflow::workflows_dir().join("../dependabot.yml");
    let text = workflow::read(&path);
    assert!(
        text.lines()
            .any(|l| l.trim() == "- package-ecosystem: github-actions"),
        "a SHA pin nobody moves is a pin to an old release; dependabot.yml has no \
         github-actions entry"
    );
}
