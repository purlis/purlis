// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! A dev build compiles before its commit's `ci` has ended, and signs and uploads only after it
//! went green (R2, #7381; ADR 0042 amended 2026-11-23).
//!
//! Each rule is a function over the workflow's text, so a test can hand it a deliberately broken
//! copy and see it refuse. A rule nobody has seen fail proves nothing.
//!
//! - `plan` takes no environment, holds no secret and does not wait for the gate: it runs the
//!   commit's own tools, and a job that does that holds nothing worth stealing.
//! - Whether a key is set is asked by `keys`, which checks nothing out and runs nothing from
//!   the commit.
//! - In a job that compiles before the gate, a step waits for the commit's `ci` before the first
//!   step that signs or uploads.
//! - Every other job needs the gate, and runs only on its green.
//!
//! "No job that holds a secret restores or saves a cache" is held by
//! `the_workflows_run_only_what_they_pinned`, beside the other cache rules.

use crate::workflow::{
    Job, jobs, read, release_jobs, run_lines, step_uses, step_value, workflows_dir,
};

/// The step that waits, in each job that compiles before the gate.
const WAIT: &str = "Wait for this commit's ci on main, before anything is signed or uploaded";

/// The jobs that start at once: they compile from the commit's own checkout and the lockfiles,
/// and wait for `ci` before they sign or upload.
const COMPILES_BEFORE_THE_GATE: [&str; 2] = ["build", "runner-binary-aarch64"];

/// The jobs that are not behind the gate, and why: `gate` is the gate; `plan` and `keys` hold
/// nothing and publish nothing; the two above wait inside their own steps; `profile-guard` only
/// reports a nightly build's result and the gate passes a nightly at once.
const NOT_BEHIND_THE_GATE: [&str; 6] = [
    "gate",
    "plan",
    "keys",
    "build",
    "runner-binary-aarch64",
    "profile-guard",
];

/// The one secret a step that runs before the wait may name, by this exact name: the Report
/// intake's address, which the compile reads and which the shipped binary carries in the clear.
const COMPILE_SECRET: &str = "PURLIS_REPORT_INTAKE_DSN";

/// The wait step's `shell:`, exactly: the runner's own bash, the file `BASH_ENV` names unset, no
/// profile.
const WAIT_SHELL: &str = "/usr/bin/env -u BASH_ENV /bin/bash --noprofile --norc -eo pipefail {0}";

/// The wait step's environment, exactly. `DEV` is true only for a push to `main`.
const WAIT_ENV: [(&str, &str); 2] = [
    ("GH_TOKEN", "${{ github.token }}"),
    (
        "DEV",
        "${{ github.event_name == 'push' && github.ref == 'refs/heads/main' }}",
    ),
];

/// What the wait step runs, line by line, comments and blank lines left out. It is the step's own
/// text and never a file of the checkout (the commit's code has run in the job by then), and
/// `tools/release-wait.test.mjs` runs it against a fake `gh`. A line added to it, such as a
/// `|| true`, is a change to this constant too.
const WAIT_RUN: [&str; 34] = [
    r##"if [ "$DEV" != true ]; then"##,
    r##"echo "Not a push to main: nothing to wait for.""##,
    r##"exit 0"##,
    r##"fi"##,
    r##"PATH=/usr/bin:/bin:/usr/local/bin:/opt/homebrew/bin"##,
    r##"gh=$(command -v gh) || { echo "::error::no gh on the runner, so nothing is signed or uploaded"; exit 1; }"##,
    r##"case "$gh" in"##,
    r##"/usr/bin/gh | /usr/local/bin/gh | /opt/homebrew/bin/gh) ;;"##,
    r##"*) echo "::error::gh resolves to $gh, which is not the runner's own, so nothing is signed or uploaded"; exit 1 ;;"##,
    r##"esac"##,
    r##"ask() {"##,
    r##"for try in 1 2 3; do"##,
    r##"if out=$("$gh" run view "$1" --repo "$GITHUB_REPOSITORY" --json status,conclusion --jq '.status + " " + (.conclusion // "none")'); then"##,
    r##"echo "$out""##,
    r##"return 0"##,
    r##"fi"##,
    r##"sleep $((try * 15))"##,
    r##"done"##,
    r##"return 1"##,
    r##"}"##,
    r##"id="""##,
    r##"for _ in $(seq 1 30); do"##,
    r##"id=$("$gh" run list --repo "$GITHUB_REPOSITORY" --workflow ci.yml --commit "$GITHUB_SHA" --event push --branch main --limit 1 --json databaseId --jq '.[0].databaseId // empty') || id="""##,
    r##"[ -n "$id" ] && break"##,
    r##"sleep 10"##,
    r##"done"##,
    r##"[ -n "$id" ] || { echo "::error::no ci run of $GITHUB_SHA on its push to main was listed within five minutes, so nothing is signed or uploaded"; exit 1; }"##,
    r##"echo "Waiting for ci run $id of $GITHUB_SHA.""##,
    r##""$gh" run watch "$id" --repo "$GITHUB_REPOSITORY" --interval 30 --exit-status > /dev/null || true"##,
    r##"state=$(ask "$id") || { echo "::error::could not read ci run $id of $GITHUB_SHA, so nothing is signed or uploaded"; exit 1; }"##,
    r##"read -r status conclusion <<< "$state""##,
    r##"[ "$status" = completed ] || { echo "::error::ci run $id of $GITHUB_SHA is $status after the wait, so nothing is signed or uploaded"; exit 1; }"##,
    r##"[ "$conclusion" = success ] || { echo "::error::ci run $id of $GITHUB_SHA ended $conclusion, so nothing is signed or uploaded"; exit 1; }"##,
    r##"echo "ci run $id of $GITHUB_SHA ended success.""##,
];

fn release_text() -> String {
    read(&workflows_dir().join("release.yml"))
}

fn find<'a>(all: &'a [Job], name: &str) -> Result<&'a Job, String> {
    all.iter()
        .find(|j| j.name == name)
        .ok_or_else(|| format!("release.yml has no `{name}` job"))
}

/// The step whose `name:` is `name`, and its place among the job's steps.
fn step_named(job: &Job, name: &str) -> Option<(usize, Vec<String>)> {
    let first = format!("      - name: {name}");
    job.steps()
        .into_iter()
        .enumerate()
        .find(|(_, s)| s.first().is_some_and(|l| l.trim_end() == first))
}

// ----- the rules -----

/// `plan` runs the commit's own tools, so it takes no environment, reads no secret and does not
/// wait for the gate.
fn plan_takes_nothing(text: &str) -> Result<(), String> {
    let all = jobs(text);
    let plan = find(&all, "plan")?;
    if let Some(env) = plan.environment() {
        return Err(format!("plan takes the environment `{env}`"));
    }
    if plan.reads_a_secret() {
        return Err("plan reads a secret".to_owned());
    }
    if plan.needs().iter().any(|n| n == "gate") {
        return Err("plan waits for the gate".to_owned());
    }
    Ok(())
}

/// `keys` is the job that reads the secrets as booleans, so it runs nothing from the commit.
fn keys_runs_nothing_from_the_commit(text: &str) -> Result<(), String> {
    let all = jobs(text);
    let keys = find(&all, "keys")?;
    for step in keys.steps() {
        match step_uses(&step) {
            Some(uses) if uses.starts_with("actions/checkout@") || uses.starts_with("./") => {
                return Err(format!("keys runs the commit's code: {uses}"));
            }
            _ => {}
        }
    }
    if keys.needs().iter().any(|n| n == "gate") {
        return Err(
            "keys waits for the gate, and the build waits for keys: nothing would compile before \
             `ci` ends"
                .to_owned(),
        );
    }
    let writes = keys
        .permissions()
        .unwrap_or_default()
        .into_iter()
        .any(|(_, access)| access == "write");
    if writes {
        return Err("keys holds a write permission".to_owned());
    }
    Ok(())
}

/// Every occurrence of the word `secrets` in `line` that is not `secrets.<name>` followed by
/// something `allowed` accepts. `secrets[`, `toJSON(secrets)` and `secrets: inherit` all fail it.
fn stray_secret(line: &str, allowed: impl Fn(&str, &str) -> bool) -> Option<String> {
    let mut from = 0;
    while let Some(at) = line[from..].find("secrets") {
        let at = from + at;
        let before_is_word = line[..at]
            .chars()
            .next_back()
            .is_some_and(|c| c.is_alphanumeric() || c == '_' || c == '-');
        let rest = &line[at + "secrets".len()..];
        from = at + "secrets".len();
        if before_is_word {
            continue;
        }
        // Only a use of the context counts: `secrets.<name>`, `secrets[..]`, `toJSON(secrets)`,
        // `secrets }}` and `secrets: inherit`. The word in a sentence does not.
        let named = rest.strip_prefix('.').map(|r| {
            let name: String = r
                .chars()
                .take_while(|c| c.is_ascii_alphanumeric() || *c == '_')
                .collect();
            (name.clone(), r[name.len()..].to_owned())
        });
        let used = match &named {
            Some((name, _)) => !name.is_empty(),
            None => rest.starts_with(['[', ')', ':']) || rest.trim_start().starts_with("}}"),
        };
        if !used {
            continue;
        }
        let ok = named.is_some_and(|(name, after)| allowed(&name, &after));
        if !ok {
            return Some(line.trim().to_owned());
        }
    }
    None
}

/// The lines of the job before its steps: its keys, its job-level `env:` among them.
fn job_level(job: &Job) -> Vec<&String> {
    job.body
        .iter()
        .take_while(|l| !l.starts_with("    steps:"))
        .collect()
}

/// A step's own keys (`env`, `if`, `shell`, `run`, ...), the ones indented to the step's level.
fn step_keys(step: &[String]) -> Vec<String> {
    step.iter()
        .skip(1)
        .filter(|l| l.starts_with("        ") && !l.starts_with("         "))
        .filter_map(|l| l.trim().split_once(':').map(|(k, _)| k.to_owned()))
        .collect()
}

/// The wait step is what it must be: no `if:`, no `continue-on-error`, the keys `name`, `env`,
/// `shell` and `run` and no others, its environment, its shell and its run exact.
fn the_wait_step_is_exact(name: &str, wait: &[String]) -> Result<(), String> {
    let keys = step_keys(wait);
    for key in &keys {
        if !["env", "shell", "run"].contains(&key.as_str()) {
            return Err(format!("{name}'s wait has a `{key}` key"));
        }
    }
    for must in ["env", "shell", "run"] {
        if !keys.iter().any(|k| k == must) {
            return Err(format!("{name}'s wait has no `{must}` key"));
        }
    }
    for (key, value) in WAIT_ENV {
        if step_value(wait, "env", key) != Some(value) {
            return Err(format!(
                "{name}'s wait does not set {key} to exactly `{value}`: {:?}",
                step_value(wait, "env", key)
            ));
        }
    }
    let env_keys = wait
        .iter()
        .skip_while(|l| l.trim_end() != "        env:")
        .skip(1)
        .take_while(|l| l.starts_with("          "))
        .filter(|l| !l.starts_with("           ") && !l.trim_start().starts_with('#'))
        .count();
    if env_keys != WAIT_ENV.len() {
        return Err(format!("{name}'s wait sets more than {WAIT_ENV:?} in env"));
    }
    let shell = wait.iter().find_map(|l| l.strip_prefix("        shell:"));
    if shell.map(str::trim) != Some(WAIT_SHELL) {
        return Err(format!(
            "{name}'s wait does not use exactly the shell `{WAIT_SHELL}`: {shell:?}"
        ));
    }
    if run_lines(wait) != WAIT_RUN {
        return Err(format!(
            "{name}'s wait does not run exactly the wait's script: {:#?}",
            run_lines(wait)
        ));
    }
    Ok(())
}

/// In each job that compiles before the gate: a step waits for `ci`, it is exact, no step before
/// it signs, uploads or names a secret but the compile's, nothing at the job's level names one
/// either, and the job may read runs to ask.
fn compiling_jobs_wait_before_they_sign_or_upload(text: &str) -> Result<(), String> {
    let all = jobs(text);
    for name in COMPILES_BEFORE_THE_GATE {
        let job = find(&all, name)?;
        if job.needs().iter().any(|n| n == "gate") {
            return Err(format!(
                "{name} waits for the gate, so it does not start at once"
            ));
        }
        if job_level(job)
            .iter()
            .any(|l| l.starts_with("    continue-on-error"))
        {
            return Err(format!(
                "{name} has a `continue-on-error`, so a failed wait could pass"
            ));
        }
        let permissions = job.permissions().unwrap_or_default();
        if !permissions
            .iter()
            .any(|(k, v)| k == "actions" && v == "read")
        {
            return Err(format!(
                "{name} cannot read runs, so its wait cannot ask: {permissions:?}"
            ));
        }
        let (at, wait) =
            step_named(job, WAIT).ok_or_else(|| format!("{name} has no `{WAIT}` step"))?;
        the_wait_step_is_exact(name, &wait)?;
        let allowed = |secret: &str, rest: &str| {
            secret == COMPILE_SECRET && !rest.starts_with(|c: char| c.is_ascii_alphanumeric())
        };
        for line in job_level(job) {
            if let Some(line) = stray_secret(line, allowed) {
                return Err(format!(
                    "{name} reads a secret at the job's level, before its wait: {line}"
                ));
            }
        }
        let steps = job.steps();
        for before in &steps[..at] {
            let text = before.join("\n");
            let what = text.lines().next().unwrap_or_default().trim().to_owned();
            if let Some(uses) = step_uses(before) {
                let lower = uses.to_ascii_lowercase();
                for upload in ["upload", "attest", "sigstore", "cosign", "release"] {
                    if lower.contains(upload) {
                        return Err(format!(
                            "{name} uploads or signs before it waits ({uses}): {what}"
                        ));
                    }
                }
            }
            for signs in [
                "tauri bundle",
                "gh release",
                "minisign",
                "cosign",
                "curl",
                "docker push",
            ] {
                if text.contains(signs) {
                    return Err(format!(
                        "{name} signs or publishes before it waits ({signs}): {what}"
                    ));
                }
            }
            for line in before {
                if let Some(line) = stray_secret(line, allowed) {
                    return Err(format!("{name} reads a secret before it waits: {line}"));
                }
            }
        }
    }
    Ok(())
}

/// `keys` reads each secret as a boolean, `secrets.X != ''`, and holds `contents: read` and
/// nothing else.
fn keys_reads_booleans_only(text: &str) -> Result<(), String> {
    let all = jobs(text);
    let keys = find(&all, "keys")?;
    for line in &keys.body {
        let boolean = |_: &str, rest: &str| rest.starts_with(" != ''");
        if let Some(line) = stray_secret(line, boolean) {
            return Err(format!(
                "keys reads a secret as more than a boolean: {line}"
            ));
        }
    }
    let permissions = keys.permissions().unwrap_or_default();
    if permissions != [("contents".to_owned(), "read".to_owned())] {
        return Err(format!(
            "keys holds more than `contents: read`: {permissions:?}"
        ));
    }
    Ok(())
}

/// The terms of an `if:` joined by a top-level `&&`, or `None` when a top-level `||` could let
/// the whole condition pass without them. Parentheses and quotes are respected.
fn and_terms(condition: &str) -> Option<Vec<String>> {
    let c = condition.trim();
    let c = c
        .strip_prefix("${{")
        .and_then(|c| c.strip_suffix("}}"))
        .unwrap_or(c)
        .trim();
    let (mut terms, mut depth, mut quote, mut current) = (Vec::new(), 0usize, None, String::new());
    let chars: Vec<char> = c.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        let ch = chars[i];
        match quote {
            Some(q) => {
                if ch == q {
                    quote = None;
                }
            }
            None => match ch {
                '\'' | '"' => quote = Some(ch),
                '(' | '[' => depth += 1,
                ')' | ']' => depth = depth.saturating_sub(1),
                '|' if depth == 0 && chars.get(i + 1) == Some(&'|') => return None,
                '&' if depth == 0 && chars.get(i + 1) == Some(&'&') => {
                    terms.push(current.trim().to_owned());
                    current.clear();
                    i += 2;
                    continue;
                }
                _ => {}
            },
        }
        current.push(ch);
        i += 1;
    }
    terms.push(current.trim().to_owned());
    Some(terms)
}

/// Every job that is not one of the above needs the gate and runs only on its green: that is a
/// term of its `if` joined by a top-level `&&`. A job that runs through `!cancelled()` and needs
/// `keys` runs only when `keys` succeeded.
fn every_other_job_needs_the_gate(text: &str) -> Result<(), String> {
    for job in jobs(text) {
        if NOT_BEHIND_THE_GATE.contains(&job.name.as_str()) {
            continue;
        }
        if !job.needs().iter().any(|n| n == "gate") {
            return Err(format!("job `{}` does not need the gate", job.name));
        }
        let condition = job.condition().unwrap_or_default();
        let terms = and_terms(condition).ok_or_else(|| {
            format!(
                "job `{}` has a top-level `||` in its condition, so its green may not be needed: {condition}",
                job.name
            )
        })?;
        if !terms
            .iter()
            .any(|t| t == "needs.gate.outputs.green == 'true'")
        {
            return Err(format!(
                "job `{}` needs the gate but does not run only on its green: {condition}",
                job.name
            ));
        }
        if terms.iter().any(|t| t == "!cancelled()")
            && job.needs().iter().any(|n| n == "keys")
            && !terms.iter().any(|t| t == "needs.keys.result == 'success'")
        {
            return Err(format!(
                "job `{}` runs through !cancelled() without needs.keys.result == 'success': {condition}",
                job.name
            ));
        }
    }
    Ok(())
}

// ----- the real workflow keeps them -----

#[test]
fn plan_takes_no_environment_holds_no_secret_and_does_not_wait_for_the_gate() {
    plan_takes_nothing(&release_text()).unwrap();
}

#[test]
fn the_job_that_asks_which_secrets_are_set_runs_nothing_from_the_commit() {
    keys_runs_nothing_from_the_commit(&release_text()).unwrap();
}

#[test]
fn the_jobs_that_compile_before_the_gate_wait_for_ci_before_they_sign_or_upload() {
    compiling_jobs_wait_before_they_sign_or_upload(&release_text()).unwrap();
}

#[test]
fn the_job_that_asks_which_secrets_are_set_reads_booleans_and_holds_only_contents_read() {
    keys_reads_booleans_only(&release_text()).unwrap();
}

#[test]
fn every_other_job_needs_the_gate_and_runs_only_on_its_green() {
    every_other_job_needs_the_gate(&release_text()).unwrap();
}

#[test]
fn the_jobs_behind_the_gate_are_all_the_jobs_that_exist() {
    // A name in `NOT_BEHIND_THE_GATE` or `COMPILES_BEFORE_THE_GATE` that no job has is a rule
    // that stopped meaning anything.
    let names: Vec<String> = release_jobs().into_iter().map(|j| j.name).collect();
    for name in NOT_BEHIND_THE_GATE.iter().chain(&COMPILES_BEFORE_THE_GATE) {
        assert!(
            names.iter().any(|n| n == name),
            "no job `{name}` in {names:?}"
        );
    }
    // And the jobs that publish are among those behind the gate.
    for name in [
        "runner-image",
        "runner-image-signature",
        "runner-binaries",
        "sbom",
        "manifest",
        "provenance",
        "publish",
        "symbols",
    ] {
        assert!(
            names.iter().any(|n| n == name),
            "no job `{name}` in {names:?}"
        );
        assert!(
            !NOT_BEHIND_THE_GATE.contains(&name),
            "{name} must be behind the gate"
        );
    }
}

// ----- each rule fails on a workflow broken on purpose -----

/// The text of job `name`'s span: from its `  name:` line to the next job's.
fn span(text: &str, name: &str) -> (usize, usize) {
    let head = format!("\n  {name}:\n");
    let start = text
        .find(&head)
        .unwrap_or_else(|| panic!("no job `{name}`"))
        + 1;
    let mut at = start;
    let mut first = true;
    for line in text[start..].split_inclusive('\n') {
        let is_job = line.starts_with("  ")
            && !line.starts_with("   ")
            && line.trim_end().ends_with(':')
            && !line.trim_start().starts_with('#');
        if is_job && !first {
            return (start, at);
        }
        first = false;
        at += line.len();
    }
    (start, text.len())
}

/// `text` with `edit` applied to job `name`'s lines only.
fn edit_job(text: &str, name: &str, edit: impl FnOnce(&str) -> String) -> String {
    let (start, end) = span(text, name);
    let changed = edit(&text[start..end]);
    assert_ne!(
        &text[start..end],
        changed,
        "the edit changed nothing in `{name}`"
    );
    format!("{}{}{}", &text[..start], changed, &text[end..])
}

/// Job `name` with the line that starts with `prefix` dropped.
fn without_line(text: &str, name: &str, prefix: &str) -> String {
    edit_job(text, name, |job| {
        job.split_inclusive('\n')
            .filter(|l| !l.starts_with(prefix))
            .collect()
    })
}

/// Job `name` with `new` added after the line that starts with `prefix`.
fn with_line_after(text: &str, name: &str, prefix: &str, new: &str) -> String {
    edit_job(text, name, |job| {
        let mut out = String::new();
        for line in job.split_inclusive('\n') {
            out.push_str(line);
            if line.starts_with(prefix) {
                out.push_str(new);
                out.push('\n');
            }
        }
        out
    })
}

/// Job `name` with `new` (whole lines) added just before the step called `step`.
fn with_step_before(text: &str, name: &str, step: &str, new: &str) -> String {
    let first = format!("      - name: {step}");
    edit_job(text, name, |job| {
        let mut out = String::new();
        for line in job.split_inclusive('\n') {
            if line.trim_end() == first {
                out.push_str(new);
                out.push('\n');
            }
            out.push_str(line);
        }
        out
    })
}

/// Job `name` with the step called `step` moved to the end of the job.
fn with_step_last(text: &str, name: &str, step: &str) -> String {
    let first = format!("      - name: {step}");
    edit_job(text, name, |job| {
        let (mut kept, mut moved, mut inside) = (String::new(), String::new(), false);
        for line in job.split_inclusive('\n') {
            if line.trim_end() == first {
                inside = true;
            } else if line.starts_with("      - ") {
                inside = false;
            }
            if inside {
                moved.push_str(line)
            } else {
                kept.push_str(line)
            }
        }
        assert!(!moved.is_empty(), "no step `{step}` in `{name}`");
        format!("{}\n{moved}", kept.trim_end())
    })
}

/// Job `name`'s `needs:` line with `gate` taken out of it.
fn without_the_gate_in_needs(text: &str, name: &str) -> String {
    edit_job(text, name, |job| {
        job.split_inclusive('\n')
            .map(|l| {
                if l.starts_with("    needs:") {
                    l.replace("gate, ", "")
                        .replace(", gate", "")
                        .replace("gate", "plan")
                } else {
                    l.to_owned()
                }
            })
            .collect()
    })
}

fn refuses(result: Result<(), String>, because: &str) {
    let err = result.expect_err("the broken workflow passed");
    assert!(err.contains(because), "refused for another reason: {err}");
}

#[test]
fn a_plan_that_takes_an_environment_is_refused() {
    let broken = with_line_after(
        &release_text(),
        "plan",
        "    name:",
        "    environment: release",
    );
    refuses(plan_takes_nothing(&broken), "takes the environment");
}

#[test]
fn a_plan_that_reads_a_secret_is_refused() {
    let broken = with_line_after(
        &release_text(),
        "plan",
        "    name:",
        "    env:\n      K: ${{ secrets.TAURI_SIGNING_PRIVATE_KEY != '' }}",
    );
    refuses(plan_takes_nothing(&broken), "reads a secret");
}

#[test]
fn a_plan_that_waits_for_the_gate_is_refused() {
    let broken = with_line_after(&release_text(), "plan", "    name:", "    needs: gate");
    refuses(plan_takes_nothing(&broken), "waits for the gate");
}

#[test]
fn a_keys_job_that_checks_the_commit_out_is_refused() {
    let broken = with_line_after(
        &release_text(),
        "keys",
        "    steps:",
        "      - uses: actions/checkout@3d3c42e5aac5ba805825da76410c181273ba90b1 # v7.0.1",
    );
    refuses(
        keys_runs_nothing_from_the_commit(&broken),
        "runs the commit's code",
    );
}

#[test]
fn a_keys_job_behind_the_gate_is_refused() {
    let broken = with_line_after(&release_text(), "keys", "    name:", "    needs: gate");
    let broken = without_line(&broken, "keys", "    needs: plan");
    refuses(
        keys_runs_nothing_from_the_commit(&broken),
        "waits for the gate",
    );
}

#[test]
fn a_build_with_no_wait_step_is_refused() {
    let text = release_text();
    let broken = text.replace(
        &format!("      - name: {WAIT}"),
        "      - name: Think about it",
    );
    assert_ne!(broken, text);
    refuses(
        compiling_jobs_wait_before_they_sign_or_upload(&broken),
        "has no",
    );
}

#[test]
fn a_build_that_waits_after_it_signs_is_refused() {
    let broken = with_step_last(&release_text(), "build", WAIT);
    refuses(
        compiling_jobs_wait_before_they_sign_or_upload(&broken),
        "before it waits",
    );
}

#[test]
fn a_build_that_uploads_before_it_waits_is_refused() {
    let broken = with_step_before(
        &release_text(),
        "build",
        WAIT,
        "      - uses: actions/upload-artifact@cf430e030ddbb5b0abf93d22962f4752f3646cd9 # v7.0.2\n        with:\n          name: early\n          path: x",
    );
    refuses(
        compiling_jobs_wait_before_they_sign_or_upload(&broken),
        "uploads or signs before it waits",
    );
}

#[test]
fn a_build_that_reads_a_signing_secret_before_it_waits_is_refused() {
    let broken = with_step_before(
        &release_text(),
        "build",
        WAIT,
        "      - name: Early key\n        env:\n          TAURI_SIGNING_PRIVATE_KEY: ${{ secrets.TAURI_SIGNING_PRIVATE_KEY }}\n        run: echo",
    );
    refuses(
        compiling_jobs_wait_before_they_sign_or_upload(&broken),
        "before it waits",
    );
}

#[test]
fn a_build_that_waits_for_the_gate_does_not_start_at_once_and_is_refused() {
    let broken = edit_job(&release_text(), "build", |job| {
        job.replacen("    needs: [", "    needs: [gate, ", 1)
    });
    refuses(
        compiling_jobs_wait_before_they_sign_or_upload(&broken),
        "does not start at once",
    );
}

#[test]
fn a_build_that_cannot_read_runs_is_refused() {
    let broken = without_line(&release_text(), "build", "      actions: read");
    refuses(
        compiling_jobs_wait_before_they_sign_or_upload(&broken),
        "cannot read runs",
    );
}

#[test]
fn the_aarch64_runner_binary_is_held_to_the_same_rule() {
    let broken = with_step_last(&release_text(), "runner-binary-aarch64", WAIT);
    refuses(
        compiling_jobs_wait_before_they_sign_or_upload(&broken),
        "runner-binary-aarch64",
    );
}

#[test]
fn a_publishing_job_that_does_not_need_the_gate_is_refused() {
    for name in [
        "runner-image",
        "runner-image-signature",
        "runner-binaries",
        "sbom",
        "manifest",
        "provenance",
        "publish",
        "symbols",
    ] {
        let broken = without_the_gate_in_needs(&release_text(), name);
        let err = every_other_job_needs_the_gate(&broken).expect_err(name);
        assert!(
            err.contains(&format!("`{name}` does not need the gate")),
            "{err}"
        );
    }
}

#[test]
fn a_job_behind_the_gate_that_ignores_its_answer_is_refused() {
    // `gate` answers `green=false` for a red `ci` and still succeeds, so a job that only needs
    // it would run.
    let broken = edit_job(&release_text(), "publish", |job| {
        job.replace(" && needs.gate.outputs.green == 'true'", "")
    });
    refuses(
        every_other_job_needs_the_gate(&broken),
        "does not run only on its green",
    );
}

// ----- each tightened rule fails on a workflow broken as a reviewer broke it -----

/// The text with `from` replaced by `to` once, inside job `name`.
fn swap(text: &str, name: &str, from: &str, to: &str) -> String {
    edit_job(text, name, |job| {
        assert!(job.contains(from), "no `{from}` in `{name}`");
        job.replacen(from, to, 1)
    })
}

#[test]
fn a_wait_with_an_if_is_refused() {
    let broken = swap(
        &release_text(),
        "build",
        &format!("      - name: {WAIT}\n"),
        &format!("      - name: {WAIT}\n        if: needs.keys.outputs.publish == 'false'\n"),
    );
    refuses(
        compiling_jobs_wait_before_they_sign_or_upload(&broken),
        "has a `if` key",
    );
}

#[test]
fn a_wait_that_continues_on_error_is_refused() {
    for job in COMPILES_BEFORE_THE_GATE {
        let broken = swap(
            &release_text(),
            job,
            &format!("      - name: {WAIT}\n"),
            &format!("      - name: {WAIT}\n        continue-on-error: true\n"),
        );
        let err = compiling_jobs_wait_before_they_sign_or_upload(&broken).expect_err(job);
        assert!(err.contains("continue-on-error"), "{err}");
    }
}

#[test]
fn a_wait_whose_script_is_neutered_is_refused() {
    for job in COMPILES_BEFORE_THE_GATE {
        for (from, to) in [
            (
                r##"[ "$conclusion" = success ] || { echo"##,
                r##"[ "$conclusion" = success ] || true || { echo"##,
            ),
            (
                "exit 1; }\n          echo \"ci run $id of $GITHUB_SHA ended success.\"",
                "exit 1; } || true\n          echo \"ci run $id of $GITHUB_SHA ended success.\"",
            ),
            (
                "ended $conclusion, so nothing is signed or uploaded\"; exit 1; }",
                "ended $conclusion, so nothing is signed or uploaded\"; exit 0; }",
            ),
        ] {
            let broken = swap(&release_text(), job, from, to);
            let err = compiling_jobs_wait_before_they_sign_or_upload(&broken).expect_err(job);
            assert!(err.contains("does not run exactly"), "{err}");
        }
    }
}

#[test]
fn a_wait_whose_dev_is_not_exact_is_refused() {
    let broken = swap(
        &release_text(),
        "build",
        "          DEV: ${{ github.event_name == 'push' && github.ref == 'refs/heads/main' }}\n        # The script",
        "          DEV: ${{ false }}\n        # The script",
    );
    refuses(
        compiling_jobs_wait_before_they_sign_or_upload(&broken),
        "does not set DEV",
    );
}

#[test]
fn a_wait_with_another_shell_or_another_env_is_refused() {
    let broken = swap(
        &release_text(),
        "build",
        "shell: /usr/bin/env -u BASH_ENV /bin/bash",
        "shell: /bin/bash",
    );
    refuses(
        compiling_jobs_wait_before_they_sign_or_upload(&broken),
        "does not use exactly the shell",
    );
    let broken = swap(
        &release_text(),
        "runner-binary-aarch64",
        "          GH_TOKEN: ${{ github.token }}\n          DEV:",
        "          GH_TOKEN: ${{ github.token }}\n          BASH_ENV: x\n          DEV:",
    );
    refuses(
        compiling_jobs_wait_before_they_sign_or_upload(&broken),
        "more than",
    );
}

#[test]
fn a_secret_in_a_compiling_jobs_own_env_is_refused() {
    for job in COMPILES_BEFORE_THE_GATE {
        let broken = swap(
            &release_text(),
            job,
            "    env:\n",
            "    env:\n      TAURI_SIGNING_PRIVATE_KEY: ${{ secrets.TAURI_SIGNING_PRIVATE_KEY }}\n",
        );
        let err = compiling_jobs_wait_before_they_sign_or_upload(&broken).expect_err(job);
        assert!(err.contains("at the job's level"), "{err}");
    }
}

#[test]
fn a_step_before_the_wait_that_signs_uploads_or_reaches_for_every_secret_is_refused() {
    for (what, step) in [
        ("cosign", "run: cosign sign-blob x"),
        ("curl", "run: curl -T x https://example.com"),
        ("docker push", "run: docker push example/x"),
        (
            "uploads or signs",
            "uses: github/codeql-action/upload-sarif@0000000000000000000000000000000000000000",
        ),
        (
            "uploads or signs",
            "uses: actions/attest-build-provenance@0000000000000000000000000000000000000000",
        ),
        (
            "reads a secret",
            "env:\n          ALL: ${{ toJSON(secrets) }}\n        run: echo",
        ),
        (
            "reads a secret",
            "env:\n          K: ${{ secrets['TAURI_SIGNING_PRIVATE_KEY'] }}\n        run: echo",
        ),
        (
            "reads a secret",
            "env:\n          D: ${{ secrets.PURLIS_REPORT_INTAKE_DSN_AND_MORE }}\n        run: echo",
        ),
    ] {
        let broken = with_step_before(
            &release_text(),
            "build",
            WAIT,
            &format!("      - name: Early\n        {step}"),
        );
        let err = compiling_jobs_wait_before_they_sign_or_upload(&broken)
            .expect_err(&format!("{what}: {step}"));
        assert!(err.contains(what), "{what}: {step}: {err}");
    }
}

#[test]
fn secrets_inherit_before_the_wait_is_refused() {
    let broken = with_line_after(
        &release_text(),
        "build",
        "    needs:",
        "    secrets: inherit",
    );
    refuses(
        compiling_jobs_wait_before_they_sign_or_upload(&broken),
        "at the job's level",
    );
}

#[test]
fn the_dsn_is_allowed_before_the_wait_and_by_exact_name_only() {
    // The real workflow names it before the wait and passes; see the rule on the real workflow.
    let text = release_text();
    assert!(text.contains("secrets.PURLIS_REPORT_INTAKE_DSN }}"));
    compiling_jobs_wait_before_they_sign_or_upload(&text).unwrap();
}

#[test]
fn a_keys_job_that_reads_a_secret_as_more_than_a_boolean_is_refused() {
    for (from, to) in [
        (
            "${{ secrets.TAURI_SIGNING_PRIVATE_KEY != '' }}",
            "${{ secrets.TAURI_SIGNING_PRIVATE_KEY }}",
        ),
        (
            "${{ secrets.APPLE_CERTIFICATE != '' }}",
            "${{ secrets.APPLE_CERTIFICATE == 'x' }}",
        ),
        (
            "${{ secrets.APPLE_CERTIFICATE_PASSWORD != '' }}",
            "${{ toJSON(secrets) != '' }}",
        ),
    ] {
        let broken = swap(&release_text(), "keys", from, to);
        refuses(keys_reads_booleans_only(&broken), "more than a boolean");
    }
}

#[test]
fn a_keys_job_with_more_than_contents_read_is_refused() {
    let broken = swap(
        &release_text(),
        "keys",
        "    permissions:\n      contents: read\n",
        "    permissions:\n      contents: read\n      actions: read\n",
    );
    refuses(
        keys_reads_booleans_only(&broken),
        "more than `contents: read`",
    );
    let broken = swap(
        &release_text(),
        "keys",
        "    permissions:\n      contents: read\n",
        "    permissions:\n      contents: write\n",
    );
    refuses(
        keys_reads_booleans_only(&broken),
        "more than `contents: read`",
    );
}

#[test]
fn a_publishing_job_whose_green_can_be_skipped_by_an_or_is_refused() {
    for name in [
        "runner-image",
        "runner-binaries",
        "sbom",
        "manifest",
        "provenance",
        "publish",
        "symbols",
    ] {
        let text = release_text();
        let (start, end) = span(&text, name);
        let condition = text[start..end]
            .lines()
            .find(|l| l.starts_with("    if:"))
            .unwrap()
            .to_owned();
        for broken_condition in [
            condition.replacen(" }}", " || always() }}", 1),
            format!(
                "    if: ${{{{ always() || {} }}}}",
                condition
                    .trim_start_matches("    if: ")
                    .trim_start_matches("${{ ")
                    .trim_end_matches(" }}")
            ),
        ] {
            if broken_condition == condition || !broken_condition.contains("||") {
                continue;
            }
            let broken = text.replacen(&condition, &broken_condition, 1);
            let err = every_other_job_needs_the_gate(&broken).expect_err(name);
            assert!(err.contains("top-level `||`"), "{name}: {err}");
        }
    }
}

#[test]
fn a_green_that_is_only_inside_a_parenthesis_or_a_quote_is_refused() {
    // The green must be a top-level term: in parentheses with an `||` beside it, it is optional.
    let text = release_text();
    let (start, end) = span(&text, "publish");
    let condition = text[start..end]
        .lines()
        .find(|l| l.starts_with("    if:"))
        .unwrap()
        .to_owned();
    let broken = text.replacen(
        &condition,
        "    if: ${{ !cancelled() && (needs.gate.outputs.green == 'true' || needs.keys.outputs.publish == 'true') && needs.keys.result == 'success' }}",
        1,
    );
    refuses(
        every_other_job_needs_the_gate(&broken),
        "does not run only on its green",
    );
    let broken = text.replacen(
        &condition,
        "    if: ${{ !cancelled() && contains('needs.gate.outputs.green == true', 'x') && needs.keys.result == 'success' }}",
        1,
    );
    refuses(
        every_other_job_needs_the_gate(&broken),
        "does not run only on its green",
    );
}

#[test]
fn a_symbols_job_that_does_not_need_keys_to_have_succeeded_is_refused() {
    let broken = swap(
        &release_text(),
        "symbols",
        " && needs.keys.result == 'success'",
        "",
    );
    refuses(
        every_other_job_needs_the_gate(&broken),
        "needs.keys.result == 'success'",
    );
}
