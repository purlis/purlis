// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! The Developer ID key is imported on a GitHub-hosted macOS runner and on no other, and a stable
//! build is notarized (ADR 0042 amended 2026-11-25).
//!
//! `build` may be given a self-hosted Mac, so it seals its bundle ad-hoc and names no Developer ID
//! or notarization secret; the `sign-macos` job, behind the gate and in the `release` environment,
//! signs what `build` made. Each rule is a function over the workflow's text, so a test can hand it
//! a deliberately broken copy and see it refuse. A rule nobody has seen fail proves nothing.
//!
//! The shape that `a_dev_build_compiles_before_the_gate_and_signs_after_it` holds (compile before
//! the gate, sign after it, every other job behind the gate) is held there, and `sign-macos` is one
//! of the jobs it holds behind the gate; this file holds what is new.

use crate::workflow::{Job, jobs, read, run_lines, step_uses, step_value, workflows_dir};

const SIGN: &str = "sign-macos";

/// The job that compiles the updater archive's signer, which `sign-macos` runs with the
/// updater's key, so it is held as if it held the key: hosted, and nothing in reach.
const SIGNER: &str = "build-signer";

/// The one name the signer is handed on under, and the path it is run from once checked.
const SIGNER_ARTIFACT: &str = "purlis-updater-signer-${{ needs.plan.outputs.version }}";
const SIGNER_PATH: &str = "signer/sign-updater-archive";
const SIGNER_CHECK: &str = "The signer is the one build-signer built";

fn release_text() -> String {
    read(&workflows_dir().join("release.yml"))
}

fn find<'a>(all: &'a [Job], name: &str) -> Result<&'a Job, String> {
    all.iter()
        .find(|j| j.name == name)
        .ok_or_else(|| format!("no job `{name}`"))
}

fn step_named(job: &Job, name: &str) -> Option<(usize, Vec<String>)> {
    job.steps()
        .into_iter()
        .enumerate()
        .find(|(_, s)| s[0].trim() == format!("- name: {name}"))
}

/// Every `secrets.<NAME>` a line reads, and `None` for a use of the whole context (`secrets[..]`,
/// `toJSON(secrets)`, `secrets: inherit`) that could read any.
fn secrets_read(line: &str) -> Result<Vec<String>, String> {
    let mut names = Vec::new();
    let mut from = 0;
    while let Some(at) = line[from..].find("secrets") {
        let at = from + at;
        let rest = &line[at + "secrets".len()..];
        from = at + "secrets".len();
        let word = line[..at]
            .chars()
            .next_back()
            .is_some_and(|c| c.is_alphanumeric() || c == '_' || c == '-');
        if word {
            continue;
        }
        if let Some(r) = rest.strip_prefix('.') {
            let name: String = r
                .chars()
                .take_while(|c| c.is_ascii_alphanumeric() || *c == '_')
                .collect();
            if !name.is_empty() {
                names.push(name);
            }
        } else if rest.starts_with(['[', ')', ':']) || rest.trim_start().starts_with("}}") {
            return Err(line.trim().to_owned());
        }
    }
    Ok(names)
}

fn is_comment(line: &str) -> bool {
    line.trim_start().starts_with('#')
}

/// The secrets each job other than `sign-macos` may read: `keys` reads booleans (held by its own
/// rule), `build` the compile's address and the updater's key for Linux, and so on. What matters
/// here is the Apple ones, which only `sign-macos` reads, and the `APPLE_*` names a job may write.
fn only_the_signing_job_names_an_apple_secret(text: &str) -> Result<(), String> {
    for job in jobs(text) {
        if job.name == SIGN {
            continue;
        }
        for line in job.body.iter().filter(|l| !is_comment(l)) {
            // `keys` counts which are set, as booleans, and nothing else.
            let boolean = job.name == "keys" && line.contains(" != ''");
            for name in secrets_read(line)
                .map_err(|l| format!("job `{}` reads the whole secrets context: {l}", job.name))?
            {
                if name.starts_with("APPLE_") && !boolean {
                    return Err(format!(
                        "job `{}` reads secrets.{name}, and only {SIGN} may: {}",
                        job.name,
                        line.trim()
                    ));
                }
            }
            // `build` seals ad-hoc, so the one Apple name it may write is the ad-hoc identity.
            if job.name != "keys" {
                for apple in [
                    "APPLE_CERTIFICATE",
                    "APPLE_API_",
                    "APPLE_ID",
                    "APPLE_PASSWORD",
                ] {
                    if line.contains(apple) {
                        return Err(format!(
                            "job `{}` names {apple}, and only {SIGN} may: {}",
                            job.name,
                            line.trim()
                        ));
                    }
                }
                if line.contains("APPLE_SIGNING_IDENTITY")
                    && !line.contains("APPLE_SIGNING_IDENTITY=-")
                {
                    return Err(format!(
                        "job `{}` names APPLE_SIGNING_IDENTITY other than as the ad-hoc `-`: {}",
                        job.name,
                        line.trim()
                    ));
                }
            }
        }
    }
    Ok(())
}

/// `sign-macos` runs on a runner GitHub hosts: a literal `macos-` label, no expression, no
/// `self-hosted`, no variable, no matrix, no container.
fn the_signing_job_runs_on_a_hosted_runner(text: &str) -> Result<(), String> {
    runs_on_a_hosted_runner(text, SIGN)
}

/// Job `name` runs on a runner GitHub hosts, as `sign-macos` must.
fn runs_on_a_hosted_runner(text: &str, name: &str) -> Result<(), String> {
    let all = jobs(text);
    let job = find(&all, name)?;
    let runs_on: Vec<&String> = job
        .body
        .iter()
        .filter(|l| l.starts_with("    runs-on:"))
        .collect();
    let [line] = runs_on.as_slice() else {
        return Err(format!("{name} has {} `runs-on:` lines", runs_on.len()));
    };
    let label = line.trim_start_matches("    runs-on:").trim();
    let literal = label.starts_with("macos-")
        && label
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '.');
    if !literal {
        return Err(format!(
            "{name} runs on `{label}`, and it must be a literal GitHub-hosted `macos-` label"
        ));
    }
    for key in ["    strategy:", "    container:", "    services:"] {
        if job.body.iter().any(|l| l.starts_with(key)) {
            return Err(format!("{name} has `{}`", key.trim()));
        }
    }
    for line in job.body.iter().filter(|l| !is_comment(l)) {
        for word in ["self-hosted", "vars.", "inputs."] {
            if line.contains(word) {
                return Err(format!(
                    "{name} mentions `{word}`, which could move it off a hosted runner: {}",
                    line.trim()
                ));
            }
        }
    }
    Ok(())
}

/// The condition's terms joined by a top-level `&&`.
fn and_terms(condition: &str) -> Vec<String> {
    let c = condition
        .trim()
        .trim_start_matches("${{")
        .trim_end_matches("}}")
        .trim();
    let (mut terms, mut depth, mut current) = (Vec::new(), 0usize, String::new());
    let chars: Vec<char> = c.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        match chars[i] {
            '(' | '[' => depth += 1,
            ')' | ']' => depth = depth.saturating_sub(1),
            '&' if depth == 0 && chars.get(i + 1) == Some(&'&') => {
                terms.push(current.trim().to_owned());
                current.clear();
                i += 2;
                continue;
            }
            _ => {}
        }
        current.push(chars[i]);
        i += 1;
    }
    terms.push(current.trim().to_owned());
    terms
}

/// `sign-macos` is in the `release` environment, behind the gate, runs only when the run publishes
/// and the build succeeded, and holds `contents: read` and nothing else.
fn the_signing_job_is_behind_the_gate_in_the_release_environment(text: &str) -> Result<(), String> {
    let all = jobs(text);
    let job = find(&all, SIGN)?;
    if job.environment() != Some("release") {
        return Err(format!("{SIGN} is in {:?}", job.environment()));
    }
    for need in ["gate", "plan", "keys", "build"] {
        if !job.needs().iter().any(|n| n == need) {
            return Err(format!("{SIGN} does not need `{need}`"));
        }
    }
    let condition = job.condition().unwrap_or_default();
    if condition.contains("||") {
        return Err(format!(
            "{SIGN} has an `||` in its condition, so a term may not be needed: {condition}"
        ));
    }
    let terms = and_terms(condition);
    for want in [
        "needs.gate.outputs.green == 'true'",
        "needs.keys.outputs.publish == 'true'",
        "needs.keys.result == 'success'",
        "needs.build.result == 'success'",
    ] {
        if !terms.iter().any(|t| t == want) {
            return Err(format!(
                "{SIGN} does not run only when `{want}`: {condition}"
            ));
        }
    }
    let permissions = job.permissions().unwrap_or_default();
    if permissions != [("contents".to_owned(), "read".to_owned())] {
        return Err(format!(
            "{SIGN} holds more than `contents: read`: {permissions:?}"
        ));
    }
    if job
        .body
        .iter()
        .any(|l| l.starts_with("    continue-on-error"))
    {
        return Err(format!("{SIGN} has a `continue-on-error`"));
    }
    Ok(())
}

/// `build` makes no Developer ID signature: its `codesign` runs only read, its macOS updater
/// archive is never made and the step that reads the updater's key is Linux's.
fn the_build_job_does_no_developer_id_signing(text: &str) -> Result<(), String> {
    let all = jobs(text);
    let build = find(&all, "build")?;
    for line in build.body.iter().filter(|l| !is_comment(l)) {
        if line.contains("codesign")
            && !(line.contains("--verify")
                || line.contains("codesign -dv")
                || line.contains("codesign -d "))
        {
            return Err(format!(
                "build runs a codesign that is not a check: {}",
                line.trim()
            ));
        }
        if line.contains("notarytool")
            || line.contains("stapler")
            || line.contains("security import")
        {
            return Err(format!("build notarizes or imports a key: {}", line.trim()));
        }
    }
    let (_, material) = step_named(build, "Signing material").ok_or("no Signing material step")?;
    if !material
        .iter()
        .any(|l| l.contains("runner.os == 'Linux'") && l.trim_start().starts_with("if:"))
    {
        return Err("build's Signing material step is not Linux's alone".to_owned());
    }
    let (_, config) = step_named(build, "This build's config").ok_or("no config step")?;
    let lines = run_lines(&config);
    let at = |want: &str| lines.iter().position(|l| l.contains(want));
    match (
        at(r#"if [ "$RUNNER_OS" = "macOS" ]; then"#),
        at("createUpdaterArtifacts = false"),
    ) {
        (Some(mac), Some(off)) if mac < off => {}
        other => {
            return Err(format!(
                "build's macOS leg does not turn off the updater archive first: {other:?}"
            ));
        }
    }
    Ok(())
}

/// What `sign-macos` signs, and how.
fn the_signing_job_signs_inside_out_with_the_runtime_and_no_entitlements(
    text: &str,
) -> Result<(), String> {
    let all = jobs(text);
    let job = find(&all, SIGN)?;
    let (_, sign) = step_named(job, "Sign the app, every nested executable first")
        .ok_or("no step that signs the app")?;
    let lines = run_lines(&sign);
    // Every set of flags it signs with has the runtime, and the Developer ID's has a secure
    // timestamp from Apple's server.
    let sets: Vec<&String> = lines.iter().filter(|l| l.contains("flags=(")).collect();
    if sets.len() != 2 {
        return Err(format!(
            "the signing step has {} sets of flags, not 2",
            sets.len()
        ));
    }
    for set in &sets {
        if !set.contains("--options runtime") {
            return Err(format!(
                "a set of signing flags lacks `--options runtime`: {set}"
            ));
        }
    }
    if !sets.iter().any(|l| l.contains("--timestamp --keychain")) {
        return Err("the Developer ID signing has no secure timestamp".to_owned());
    }
    for line in job.body.iter().filter(|l| !is_comment(l)) {
        if line.contains("--entitlements") && !line.contains("codesign -d") {
            return Err(format!("{SIGN} signs with entitlements: {}", line.trim()));
        }
        // `--deep` signs a bundle's insides with one identity and none of the flags above in order.
        if line.contains("codesign") && line.contains("--deep") && !line.contains("--verify") {
            return Err(format!("{SIGN} signs with --deep: {}", line.trim()));
        }
    }
    let at = |want: &str| lines.iter().position(|l| l.contains(want));
    let (nested, bundles, app) = (
        at("done < \"$RUNNER_TEMP/machos.txt\""),
        at("done < \"$RUNNER_TEMP/bundles.txt\""),
        at("codesign \"${flags[@]}\" --sign \"$identity\" \"$app\""),
    );
    match (nested, bundles, app) {
        (Some(n), Some(b), Some(a)) if n < b && b < a => Ok(()),
        other => Err(format!(
            "nested executables, then nested bundles, then the app, in that order: {other:?}"
        )),
    }
}

/// The final artifacts are verified after they are made and before they are uploaded, and the
/// hardened runtime and entitlements are read off the one that ships.
fn the_final_artifacts_are_verified_before_they_are_uploaded(text: &str) -> Result<(), String> {
    let all = jobs(text);
    let job = find(&all, SIGN)?;
    let (signed, _) = step_named(job, "Sign the app, every nested executable first")
        .ok_or("no step that signs the app")?;
    let (made, _) = step_named(
        job,
        "The app.zip and the updater archive, from the signed app",
    )
    .ok_or("no step that makes the archives")?;
    let (sig, _) = step_named(job, "The updater archive's signature").ok_or("no signature step")?;
    let (verified, verify) = step_named(
        job,
        "The final artifacts carry the signature this run asked for",
    )
    .ok_or("the final artifacts are not verified")?;
    let (runtime, runtime_step) = step_named(
        job,
        "The app keeps the hardened runtime and carries no entitlement",
    )
    .ok_or("the hardened runtime is not read off the final app")?;
    let (uploaded, _) = step_named(job, "Upload").ok_or("no upload")?;
    if !(signed < made && made < sig && sig < verified && verified < uploaded && runtime < uploaded)
    {
        return Err("sign, make, sign the archive, verify, then upload, in that order".to_owned());
    }
    let text = verify.join("\n");
    for want in [
        "codesign --verify --deep --strict",
        "Authority=Developer ID Application:",
        "Signature=adhoc",
        "CDHash",
        "dist/purlis-macos-arm64.app.zip",
        "dist/purlis-darwin-aarch64.app.tar.gz",
        "dist/purlis-macos-arm64.dmg",
    ] {
        if !text.contains(want) {
            return Err(format!("the final verification does not read `{want}`"));
        }
    }
    let runtime_text = runtime_step.join("\n");
    for want in [
        "*runtime*)",
        "codesign -d --entitlements - --xml",
        "dist/purlis-macos-arm64.app.zip",
    ] {
        if !runtime_text.contains(want) {
            return Err(format!("the runtime check does not read `{want}`"));
        }
    }
    // The only exemption is the hardened-runtime read's, report-only on a dev publish.
    let exempt: Vec<&String> = runtime_step
        .iter()
        .filter(|l| l.trim().starts_with("continue-on-error:"))
        .collect();
    if exempt.len() != 1
        || exempt[0].trim() != "continue-on-error: ${{ needs.plan.outputs.channel == 'dev' }}"
    {
        return Err(format!(
            "the runtime read's one exemption is for a dev build: {exempt:?}"
        ));
    }
    for (name, step) in job.steps().into_iter().map(|s| (s[0].clone(), s)) {
        if name.contains("hardened runtime") {
            continue;
        }
        if step
            .iter()
            .any(|l| l.trim().starts_with("continue-on-error"))
        {
            return Err(format!("{SIGN} has an exempt step: {name}"));
        }
    }
    Ok(())
}

/// Notarization is a stable build's alone, with the whole key, and it is stapled and checked.
fn notarization_is_stable_only_and_all_or_none(text: &str) -> Result<(), String> {
    let all = jobs(text);
    let keys = find(&all, "keys")?;
    let keys_text = keys.body.join("\n");
    let rule = r#"if [ "$CHANNEL" = stable ] && [ "$macos_signing" = developer-id ] && [ "$notary" = all ]; then"#;
    if !keys_text.contains(rule) {
        return Err(
            "keys does not notarize only a stable, Developer ID signed build with all three"
                .to_owned(),
        );
    }
    for want in [
        "the notarization secrets are half set",
        "a stable build is notarized, and the Developer ID is set without",
        "the notarization secrets are set without a Developer ID",
        "the Developer ID secrets are half set",
    ] {
        if !keys_text.contains(want) {
            return Err(format!("keys no longer refuses: {want}"));
        }
    }
    let job = find(&all, SIGN)?;
    for name in [
        "The notarization key, in a file of this job",
        "Notarize and staple the app",
    ] {
        let (_, step) = step_named(job, name).ok_or_else(|| format!("no step `{name}`"))?;
        if !step
            .iter()
            .any(|l| l.trim() == "if: needs.keys.outputs.notarize == 'true'")
        {
            return Err(format!(
                "`{name}` does not run only when keys said to notarize"
            ));
        }
    }
    let (_, key) =
        step_named(job, "The notarization key, in a file of this job").ok_or("no key step")?;
    let lines = run_lines(&key);
    if !lines.iter().any(|l| l == "umask 077") {
        return Err("the notarization key is written without `umask 077`".to_owned());
    }
    let (_, notarize) = step_named(job, "Notarize and staple the app").ok_or("no notarize step")?;
    let text = run_lines(&notarize).join("\n");
    for want in [
        "notarytool submit",
        "--wait",
        "!= Accepted",
        "stapler staple",
        "stapler validate",
    ] {
        if !text.contains(want) {
            return Err(format!("the notarization step lacks `{want}`"));
        }
    }
    // The key file is removed however the job ended.
    let (_, gone) = step_named(job, "The keychain and the notarization key are removed")
        .ok_or("nothing removes the notarization key")?;
    if !gone.iter().any(|l| l.trim() == "if: always()")
        || !gone
            .iter()
            .any(|l| l.contains("rm -f \"$NOTARY_KEY_FILE\""))
    {
        return Err("the key file is not removed by an `if: always()` step".to_owned());
    }
    // No other job notarizes.
    for other in &all {
        if other.name != SIGN
            && other
                .body
                .iter()
                .filter(|l| !is_comment(l))
                .any(|l| l.contains("notarytool") || l.contains("stapler"))
        {
            return Err(format!("job `{}` notarizes", other.name));
        }
    }
    Ok(())
}

/// Only `sign-macos` downloads the macOS bundle `build` hands on unsigned.
fn only_the_signing_job_takes_the_unsigned_bundle(text: &str) -> Result<(), String> {
    for job in jobs(text) {
        if job.name == SIGN {
            continue;
        }
        for line in job.body.iter().filter(|l| !is_comment(l)) {
            if line.contains("purlis-macos-arm64-unsigned") {
                return Err(format!(
                    "job `{}` takes the unsigned bundle: {}",
                    job.name,
                    line.trim()
                ));
            }
        }
    }
    let all = jobs(text);
    let job = find(&all, SIGN)?;
    let takes = job.steps().iter().any(|s| {
        step_uses(s).is_some_and(|u| u.starts_with("actions/download-artifact@"))
            && s.iter()
                .any(|l| l.contains("name: purlis-macos-arm64-unsigned-"))
    });
    if !takes {
        return Err(format!("{SIGN} does not download the unsigned bundle"));
    }
    Ok(())
}

/// `build-signer` compiles the updater archive's signer with nothing in reach: a hosted runner,
/// `plan` its only need (so it compiles while `build` does), no environment, no secret, no token
/// but `contents: read`, and it hands on the one binary under one name with its sha256 as an
/// output. No cache is held by `the_workflows_run_only_what_they_pinned`, for every job of
/// `release.yml`.
fn the_signer_is_built_on_a_hosted_runner_with_nothing_in_reach(text: &str) -> Result<(), String> {
    runs_on_a_hosted_runner(text, SIGNER)?;
    let all = jobs(text);
    let job = find(&all, SIGNER)?;
    if job.needs() != ["plan", "keys"] {
        return Err(format!(
            "{SIGNER} needs {:?}, and it needs `plan` and `keys` alone",
            job.needs()
        ));
    }
    if let Some(environment) = job.environment() {
        return Err(format!("{SIGNER} is in the environment `{environment}`"));
    }
    // `secrets.X`, `secrets[..]`, `toJSON(secrets)` and `secrets: inherit` all say `secrets`.
    if let Some(line) = job.body.iter().find(|l| l.contains("secrets")) {
        return Err(format!("{SIGNER} names a secret: {}", line.trim()));
    }
    for key in ["github.token", "GITHUB_TOKEN", "KEY"] {
        if let Some(line) = job.body.iter().find(|l| l.contains(key)) {
            return Err(format!("{SIGNER} names `{key}`: {}", line.trim()));
        }
    }
    let permissions = job.permissions().unwrap_or_default();
    if permissions != [("contents".to_owned(), "read".to_owned())] {
        return Err(format!(
            "{SIGNER} holds more than `contents: read`: {permissions:?}"
        ));
    }
    if job
        .body
        .iter()
        .any(|l| l.trim_start().starts_with("continue-on-error"))
    {
        return Err(format!("{SIGNER} has a `continue-on-error`"));
    }
    if !job
        .body
        .iter()
        .any(|l| l.trim() == "sha256: ${{ steps.signer.outputs.sha256 }}")
    {
        return Err(format!(
            "{SIGNER} does not give the signer's sha256 as an output"
        ));
    }
    let steps = job.steps();
    let built = steps
        .iter()
        .position(|s| s.iter().any(|l| l.trim() == "id: signer"))
        .ok_or_else(|| format!("{SIGNER} has no step `signer`"))?;
    let run = run_lines(&steps[built]);
    for want in [
        "cargo build --locked --release -p release-manifest --bin sign-updater-archive",
        "sha=$(shasum -a 256 target/release/sign-updater-archive | cut -d' ' -f1)",
        r#"echo "sha256=$sha" >> "$GITHUB_OUTPUT""#,
    ] {
        if !run.iter().any(|l| l == want) {
            return Err(format!("{SIGNER}'s step `signer` does not run `{want}`"));
        }
    }
    let uploads: Vec<usize> = steps
        .iter()
        .enumerate()
        .filter(|(_, s)| step_uses(s).is_some_and(|u| u.contains("upload-artifact")))
        .map(|(i, _)| i)
        .collect();
    let [upload] = uploads.as_slice() else {
        return Err(format!(
            "{SIGNER} has {} uploads, and it has one",
            uploads.len()
        ));
    };
    let up = &steps[*upload];
    let with = |key: &str| step_value(up, "with", key);
    if with("name") != Some(SIGNER_ARTIFACT)
        || with("path") != Some("target/release/sign-updater-archive")
        || *upload < built
    {
        return Err(format!(
            "{SIGNER} uploads something other than the signer it hashed, under `{SIGNER_ARTIFACT}`"
        ));
    }
    Ok(())
}

/// `sign-macos` compiles nothing: it downloads the signer `build-signer` built, holds its bytes
/// to the sha256 that job gave before anything runs it, and runs it from there. No other job
/// takes the signer.
fn the_signing_job_runs_only_the_signer_it_checked(text: &str) -> Result<(), String> {
    let all = jobs(text);
    for other in &all {
        if other.name == SIGN || other.name == SIGNER {
            continue;
        }
        // A download by `pattern:`, or of every artifact (no `name:`), could take the signer
        // without naming it; only one that names its artifact passes.
        for step in other.steps() {
            if step_uses(&step).is_some_and(|u| u.starts_with("actions/download-artifact@"))
                && step_value(&step, "with", "name").is_none()
            {
                let pattern = step_value(&step, "with", "pattern").unwrap_or("every artifact");
                if !pattern.contains("-symbols-") {
                    return Err(format!(
                        "job `{}` takes the updater archive's signer, or may, by `{pattern}`",
                        other.name
                    ));
                }
            }
        }
        if let Some(line) = other
            .body
            .iter()
            .find(|l| l.contains("purlis-updater-signer"))
        {
            return Err(format!(
                "job `{}` takes the updater archive's signer: {}",
                other.name,
                line.trim()
            ));
        }
    }
    let job = find(&all, SIGN)?;
    if !job.needs().iter().any(|n| n == SIGNER) {
        return Err(format!("{SIGN} does not need `{SIGNER}`"));
    }
    if !and_terms(job.condition().unwrap_or_default())
        .iter()
        .any(|t| t == "needs.build-signer.result == 'success'")
    {
        return Err(format!("{SIGN} does not ask {SIGNER} for its success"));
    }
    for line in &job.body {
        let words = line.split(|c: char| !(c.is_ascii_alphanumeric() || c == '-' || c == '_'));
        if let Some(word) = words
            .into_iter()
            .find(|w| ["cargo", "rustc", "rust-toolchain"].contains(w))
        {
            return Err(format!("{SIGN} compiles (`{word}`): {}", line.trim()));
        }
    }
    let steps = job.steps();
    let downloaded = steps
        .iter()
        .position(|s| {
            step_uses(s).is_some_and(|u| u.starts_with("actions/download-artifact@"))
                && step_value(s, "with", "name") == Some(SIGNER_ARTIFACT)
                && step_value(s, "with", "path") == Some("signer")
        })
        .ok_or_else(|| format!("{SIGN} does not download `{SIGNER_ARTIFACT}` into `signer`"))?;
    let (checked, check) =
        step_named(job, SIGNER_CHECK).ok_or_else(|| format!("{SIGN} has no `{SIGNER_CHECK}`"))?;
    if check.iter().any(|l| l.trim_start().starts_with("if:")) {
        return Err(format!(
            "`{SIGNER_CHECK}` has a condition, so it may not run"
        ));
    }
    if step_value(&check, "env", "SIGNER_SHA256")
        != Some("${{ needs.build-signer.outputs.sha256 }}")
    {
        return Err(format!("`{SIGNER_CHECK}` does not read {SIGNER}'s sha256"));
    }
    let run = run_lines(&check).join("\n");
    for want in [
        "set -euo pipefail",
        r#"[[ "$SIGNER_SHA256" =~ ^[0-9a-f]{64}$ ]]"#,
        &format!("got=$(shasum -a 256 {SIGNER_PATH} | cut -d' ' -f1)"),
        r#"[ "$got" = "$SIGNER_SHA256" ]"#,
    ] {
        if !run.contains(want) {
            return Err(format!("`{SIGNER_CHECK}` does not compare `{want}`"));
        }
    }
    let runs: Vec<(usize, Vec<String>)> = steps
        .iter()
        .enumerate()
        .filter(|(i, _)| *i != checked)
        .map(|(i, s)| (i, run_lines(s)))
        .filter(|(_, lines)| lines.iter().any(|l| l.contains("sign-updater-archive")))
        .collect();
    let [(signs, lines)] = runs.as_slice() else {
        return Err(format!(
            "{SIGN} runs the signer in {} steps, and it runs it in one",
            runs.len()
        ));
    };
    let from_the_checked_path = lines
        .iter()
        .filter(|l| l.contains("sign-updater-archive"))
        .all(|l| l.starts_with(&format!("{SIGNER_PATH} ")));
    if !from_the_checked_path {
        return Err(format!("{SIGN} runs a signer other than `{SIGNER_PATH}`"));
    }
    if !(downloaded < checked && checked < *signs) {
        return Err(format!(
            "{SIGN} downloads, checks and only then runs the signer, in that order"
        ));
    }
    Ok(())
}

// ----- the real workflow keeps them -----

#[test]
fn only_sign_macos_reads_or_names_a_developer_id_or_notarization_secret() {
    only_the_signing_job_names_an_apple_secret(&release_text()).unwrap();
}

#[test]
fn sign_macos_runs_on_a_hosted_macos_runner_and_never_a_self_hosted_one() {
    the_signing_job_runs_on_a_hosted_runner(&release_text()).unwrap();
}

#[test]
fn sign_macos_is_behind_the_gate_in_the_release_environment_and_only_when_the_run_publishes() {
    the_signing_job_is_behind_the_gate_in_the_release_environment(&release_text()).unwrap();
}

#[test]
fn the_build_job_does_no_developer_id_signing_on_any_runner() {
    the_build_job_does_no_developer_id_signing(&release_text()).unwrap();
}

#[test]
fn sign_macos_signs_inside_out_with_the_hardened_runtime_and_no_entitlement() {
    the_signing_job_signs_inside_out_with_the_runtime_and_no_entitlements(&release_text()).unwrap();
}

#[test]
fn the_final_artifacts_are_read_after_they_are_made_and_before_they_are_uploaded() {
    the_final_artifacts_are_verified_before_they_are_uploaded(&release_text()).unwrap();
}

#[test]
fn only_a_stable_build_is_notarized_and_the_key_is_all_or_none() {
    notarization_is_stable_only_and_all_or_none(&release_text()).unwrap();
}

#[test]
fn only_sign_macos_takes_the_unsigned_bundle() {
    only_the_signing_job_takes_the_unsigned_bundle(&release_text()).unwrap();
}

#[test]
fn the_macos_bundle_every_later_job_takes_is_the_one_sign_macos_uploads() {
    // `sbom`, `manifest`, `provenance` and `publish` download `purlis-macos-arm64-<version>`:
    // `build` uploads it only on a run that publishes nothing, and `sign-macos` on one that does.
    let text = release_text();
    let all = jobs(&text);
    let sign = find(&all, SIGN).unwrap();
    assert!(
        sign.body
            .iter()
            .any(|l| l.trim() == "name: purlis-macos-arm64-${{ needs.plan.outputs.version }}"),
        "{SIGN} uploads under the name the later jobs download"
    );
    for name in ["sbom", "manifest", "provenance", "publish"] {
        let job = find(&all, name).unwrap();
        assert!(
            job.needs().iter().any(|n| n == SIGN),
            "{name} does not wait for {SIGN}"
        );
    }
    for name in ["manifest", "provenance", "publish"] {
        let job = find(&all, name).unwrap();
        assert!(
            job.condition()
                .unwrap_or_default()
                .contains("needs.sign-macos.result == 'success'"),
            "{name} does not ask {SIGN} for its success"
        );
    }
}

#[test]
fn the_updater_archives_signer_is_built_on_a_hosted_runner_with_no_secret_environment_or_key() {
    the_signer_is_built_on_a_hosted_runner_with_nothing_in_reach(&release_text()).unwrap();
}

#[test]
fn sign_macos_compiles_nothing_and_runs_the_signer_only_once_its_sha256_is_build_signers() {
    the_signing_job_runs_only_the_signer_it_checked(&release_text()).unwrap();
}

// ----- each rule fails on a workflow broken on purpose -----

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

/// `text` with the first `from` in job `name` replaced by `to`.
fn swap(text: &str, name: &str, from: &str, to: &str) -> String {
    let (start, end) = span(text, name);
    let job = &text[start..end];
    assert!(job.contains(from), "`{from}` is not in `{name}`");
    format!(
        "{}{}{}",
        &text[..start],
        job.replacen(from, to, 1),
        &text[end..]
    )
}

/// Every `from` in job `name` replaced by `to`.
fn swap_all(text: &str, name: &str, from: &str, to: &str) -> String {
    let (start, end) = span(text, name);
    let job = &text[start..end];
    assert!(job.contains(from), "`{from}` is not in `{name}`");
    format!(
        "{}{}{}",
        &text[..start],
        job.replace(from, to),
        &text[end..]
    )
}

fn refuses(result: Result<(), String>, because: &str) {
    let err = result.expect_err("the broken workflow passed");
    assert!(err.contains(because), "refused for another reason: {err}");
}

#[test]
fn a_build_that_reads_the_developer_id_certificate_is_refused() {
    let broken = swap(
        &release_text(),
        "build",
        "    env:\n      VERSION:",
        "    env:\n      APPLE_CERTIFICATE: ${{ secrets.APPLE_CERTIFICATE }}\n      VERSION:",
    );
    refuses(
        only_the_signing_job_names_an_apple_secret(&broken),
        "only sign-macos may",
    );
}

#[test]
fn a_build_that_names_the_identity_other_than_as_the_adhoc_dash_is_refused() {
    let broken = swap(
        &release_text(),
        "build",
        "echo \"APPLE_SIGNING_IDENTITY=-\" >> \"$GITHUB_ENV\"",
        "echo \"APPLE_SIGNING_IDENTITY=Developer ID Application: Purlis, Inc\" >> \"$GITHUB_ENV\"",
    );
    refuses(
        only_the_signing_job_names_an_apple_secret(&broken),
        "other than as the ad-hoc",
    );
}

#[test]
fn a_build_or_runner_job_that_reads_the_whole_secrets_context_is_refused() {
    for job in ["build", "runner-binaries", "publish"] {
        let broken = swap(
            &release_text(),
            job,
            "    steps:\n",
            "    env:\n      ALL: ${{ toJSON(secrets) }}\n    steps:\n",
        );
        refuses(
            only_the_signing_job_names_an_apple_secret(&broken),
            "reads the whole secrets context",
        );
    }
}

#[test]
fn a_job_that_reads_the_notarization_key_beside_the_signer_is_refused() {
    let broken = swap(
        &release_text(),
        "runner-binaries",
        "    env:\n      VERSION:",
        "    env:\n      P8: ${{ secrets.APPLE_API_KEY_P8 }}\n      VERSION:",
    );
    refuses(
        only_the_signing_job_names_an_apple_secret(&broken),
        "APPLE_API_KEY_P8",
    );
}

#[test]
fn a_signing_job_on_a_self_hosted_or_variable_runner_is_refused() {
    for runs_on in [
        "self-hosted",
        "${{ vars.MACOS_RELEASE_RUNNER || 'macos-latest' }}",
        "[self-hosted, macOS]",
        "ubuntu-24.04",
    ] {
        let broken = swap(
            &release_text(),
            SIGN,
            "runs-on: macos-latest",
            &format!("runs-on: {runs_on}"),
        );
        refuses(
            the_signing_job_runs_on_a_hosted_runner(&broken),
            "literal GitHub-hosted",
        );
    }
}

#[test]
fn a_signing_job_with_a_matrix_or_a_container_is_refused() {
    for (key, because) in [
        (
            "    strategy:\n      matrix:\n        os: [macos-latest]\n",
            "strategy",
        ),
        ("    container: some/image\n", "container"),
    ] {
        let broken = swap(
            &release_text(),
            SIGN,
            "    permissions:\n",
            &format!("{key}    permissions:\n"),
        );
        refuses(the_signing_job_runs_on_a_hosted_runner(&broken), because);
    }
}

#[test]
fn a_signing_job_outside_the_release_environment_or_ahead_of_the_gate_is_refused() {
    let text = release_text();
    refuses(
        the_signing_job_is_behind_the_gate_in_the_release_environment(&swap(
            &text,
            SIGN,
            "    environment: release\n",
            "",
        )),
        "is in",
    );
    refuses(
        the_signing_job_is_behind_the_gate_in_the_release_environment(&swap(
            &text,
            SIGN,
            "needs: [gate, plan, keys, build, build-signer]",
            "needs: [plan, keys, build, build-signer]",
        )),
        "does not need `gate`",
    );
    refuses(
        the_signing_job_is_behind_the_gate_in_the_release_environment(&swap(
            &text,
            SIGN,
            "needs.gate.outputs.green == 'true' &&",
            "needs.gate.outputs.green == 'true' ||",
        )),
        "`||`",
    );
    refuses(
        the_signing_job_is_behind_the_gate_in_the_release_environment(&swap(
            &text,
            SIGN,
            " && needs.keys.outputs.publish == 'true'",
            "",
        )),
        "needs.keys.outputs.publish == 'true'",
    );
    refuses(
        the_signing_job_is_behind_the_gate_in_the_release_environment(&swap(
            &text,
            SIGN,
            "    permissions:\n      contents: read\n",
            "    permissions:\n      contents: read\n      id-token: write\n",
        )),
        "more than `contents: read`",
    );
}

#[test]
fn a_build_that_signs_with_a_named_identity_or_notarizes_is_refused() {
    let text = release_text();
    let broken = swap(
        &text,
        "build",
        "          echo \"the identity is '-'",
        "          codesign --force --sign \"Developer ID Application: Purlis, Inc\" target/x.app\n          echo \"the identity is '-'",
    );
    refuses(
        the_build_job_does_no_developer_id_signing(&broken),
        "not a check",
    );
    let broken = swap(
        &text,
        "build",
        "          echo \"the identity is '-'",
        "          xcrun notarytool submit x.zip\n          echo \"the identity is '-'",
    );
    refuses(
        the_build_job_does_no_developer_id_signing(&broken),
        "notarizes or imports",
    );
}

#[test]
fn a_build_whose_signing_material_reaches_macos_is_refused() {
    let broken = swap(
        &release_text(),
        "build",
        "        if: steps.ci.outputs.green == 'true' && runner.os == 'Linux'\n        env:\n          TAURI_SIGNING_PRIVATE_KEY:",
        "        if: steps.ci.outputs.green == 'true'\n        env:\n          TAURI_SIGNING_PRIVATE_KEY:",
    );
    refuses(
        the_build_job_does_no_developer_id_signing(&broken),
        "Linux's alone",
    );
}

#[test]
fn a_build_that_makes_the_macos_updater_archive_is_refused() {
    let broken = swap(
        &release_text(),
        "build",
        "          if [ \"$RUNNER_OS\" = \"macOS\" ]; then\n            echo \"the macOS bundle is sealed ad-hoc here",
        "          if [ \"$RUNNER_OS\" = \"macOS-never\" ]; then\n            echo \"the macOS bundle is sealed ad-hoc here",
    );
    // The condition changed but is still there, so the order still reads; take the switch away.
    let broken = broken.replace(
        "createUpdaterArtifacts = false",
        "createUpdaterArtifacts = true",
    );
    refuses(
        the_build_job_does_no_developer_id_signing(&broken),
        "updater archive",
    );
}

#[test]
fn a_signing_step_with_entitlements_deep_or_no_runtime_is_refused() {
    let text = release_text();
    let broken = swap(
        &text,
        SIGN,
        "--force --options runtime --timestamp --keychain",
        "--force --timestamp --keychain",
    );
    refuses(
        the_signing_job_signs_inside_out_with_the_runtime_and_no_entitlements(&broken),
        "--options runtime",
    );
    let broken = swap(
        &text,
        SIGN,
        "--force --options runtime --timestamp --keychain",
        "--force --options runtime --keychain",
    );
    refuses(
        the_signing_job_signs_inside_out_with_the_runtime_and_no_entitlements(&broken),
        "secure timestamp",
    );
    let broken = swap(
        &text,
        SIGN,
        "          codesign \"${flags[@]}\" --sign \"$identity\" \"$app\"\n",
        "          codesign \"${flags[@]}\" --sign \"$identity\" --entitlements app/src-tauri/e.plist \"$app\"\n          codesign \"${flags[@]}\" --sign \"$identity\" \"$app\"\n",
    );
    refuses(
        the_signing_job_signs_inside_out_with_the_runtime_and_no_entitlements(&broken),
        "entitlements",
    );
    let broken = swap(
        &text,
        SIGN,
        "          codesign \"${flags[@]}\" --sign \"$identity\" \"$app\"\n",
        "          codesign \"${flags[@]}\" --deep --sign \"$identity\" \"$app\"\n",
    );
    refuses(
        the_signing_job_signs_inside_out_with_the_runtime_and_no_entitlements(&broken),
        "--deep",
    );
}

#[test]
fn a_signing_step_that_signs_the_app_before_what_is_inside_it_is_refused() {
    let text = release_text();
    let app = "          codesign \"${flags[@]}\" --sign \"$identity\" \"$app\"\n";
    let broken = swap(&text, SIGN, app, "");
    let broken = swap(
        &broken,
        SIGN,
        "          while IFS= read -r f; do\n",
        &format!("{app}          while IFS= read -r f; do\n"),
    );
    refuses(
        the_signing_job_signs_inside_out_with_the_runtime_and_no_entitlements(&broken),
        "in that order",
    );
}

#[test]
fn a_signing_job_that_uploads_before_it_verifies_or_never_reads_an_artifact_is_refused() {
    let text = release_text();
    // Verification dropped.
    let broken = swap(
        &text,
        SIGN,
        "- name: The final artifacts carry the signature this run asked for",
        "- name: Something else",
    );
    refuses(
        the_final_artifacts_are_verified_before_they_are_uploaded(&broken),
        "not verified",
    );
    // One artifact no longer read.
    let broken = swap_all(&text, SIGN, "dist/purlis-macos-arm64.dmg", "dist/other.dmg");
    refuses(
        the_final_artifacts_are_verified_before_they_are_uploaded(&broken),
        "does not read",
    );
    // The authority no longer asked.
    let broken = swap(
        &text,
        SIGN,
        "'^Authority=Developer ID Application:'",
        "'^Authority='",
    );
    refuses(
        the_final_artifacts_are_verified_before_they_are_uploaded(&broken),
        "Authority=Developer ID Application:",
    );
}

#[test]
fn a_signing_job_with_another_exempt_step_is_refused() {
    let text = release_text();
    let broken = swap(
        &text,
        SIGN,
        "      - name: The final artifacts carry the signature this run asked for\n",
        "      - name: The final artifacts carry the signature this run asked for\n        continue-on-error: true\n",
    );
    refuses(
        the_final_artifacts_are_verified_before_they_are_uploaded(&broken),
        "exempt step",
    );
}

#[test]
fn a_notarization_that_is_not_stable_only_or_not_all_or_none_is_refused() {
    let text = release_text();
    let broken = swap(
        &text,
        "keys",
        r#"if [ "$CHANNEL" = stable ] && [ "$macos_signing" = developer-id ] && [ "$notary" = all ]; then"#,
        r#"if [ "$macos_signing" = developer-id ] && [ "$notary" = all ]; then"#,
    );
    refuses(
        notarization_is_stable_only_and_all_or_none(&broken),
        "only a stable",
    );
    let broken = swap(
        &text,
        "keys",
        "the notarization secrets are half set",
        "the notarization secrets are fine",
    );
    refuses(
        notarization_is_stable_only_and_all_or_none(&broken),
        "no longer refuses",
    );
}

#[test]
fn a_notarization_whose_key_stays_behind_or_whose_step_ignores_apple_is_refused() {
    let text = release_text();
    let broken = swap(&text, SIGN, "          umask 077\n", "");
    refuses(
        notarization_is_stable_only_and_all_or_none(&broken),
        "umask 077",
    );
    let broken = swap(
        &text,
        SIGN,
        "      - name: The keychain and the notarization key are removed\n        if: always()\n",
        "      - name: The keychain and the notarization key are removed\n",
    );
    refuses(
        notarization_is_stable_only_and_all_or_none(&broken),
        "`if: always()`",
    );
    let broken = swap(&text, SIGN, "!= Accepted", "!= Whatever");
    refuses(
        notarization_is_stable_only_and_all_or_none(&broken),
        "!= Accepted",
    );
    let broken = swap(
        &text,
        SIGN,
        "        if: needs.keys.outputs.notarize == 'true'\n        env:\n          APPLE_API_ISSUER:",
        "        env:\n          APPLE_API_ISSUER:",
    );
    refuses(
        notarization_is_stable_only_and_all_or_none(&broken),
        "does not run only when keys said to notarize",
    );
}

#[test]
fn a_job_that_notarizes_beside_sign_macos_is_refused() {
    let broken = swap(
        &release_text(),
        "publish",
        "    steps:\n",
        "    steps:\n      - name: Notarize\n        run: xcrun notarytool submit x.zip\n",
    );
    refuses(
        notarization_is_stable_only_and_all_or_none(&broken),
        "notarizes",
    );
}

#[test]
fn a_job_that_downloads_the_unsigned_bundle_beside_sign_macos_is_refused() {
    let broken = swap(
        &release_text(),
        "publish",
        "    steps:\n",
        "    steps:\n      - uses: actions/download-artifact@v8\n        with:\n          name: purlis-macos-arm64-unsigned-${{ needs.plan.outputs.version }}\n",
    );
    refuses(
        only_the_signing_job_takes_the_unsigned_bundle(&broken),
        "takes the unsigned bundle",
    );
}

#[test]
fn a_signer_built_on_a_self_hosted_or_variable_runner_is_refused() {
    for runs_on in [
        "self-hosted",
        "${{ fromJSON(vars.MACOS_RELEASE_RUNNER || '\"macos-latest\"') }}",
        "ubuntu-24.04",
    ] {
        let broken = swap(
            &release_text(),
            SIGNER,
            "runs-on: macos-latest",
            &format!("runs-on: {runs_on}"),
        );
        refuses(
            the_signer_is_built_on_a_hosted_runner_with_nothing_in_reach(&broken),
            "literal GitHub-hosted",
        );
    }
}

#[test]
fn a_signer_built_after_the_build_or_the_gate_is_refused() {
    for needs in ["[plan, keys, build]", "[gate, plan, keys]"] {
        let broken = swap(
            &release_text(),
            SIGNER,
            "    needs: [plan, keys]\n",
            &format!("    needs: {needs}\n"),
        );
        refuses(
            the_signer_is_built_on_a_hosted_runner_with_nothing_in_reach(&broken),
            "it needs `plan` and `keys` alone",
        );
    }
}

#[test]
fn a_signer_built_with_an_environment_a_secret_or_a_key_in_reach_is_refused() {
    let text = release_text();
    refuses(
        the_signer_is_built_on_a_hosted_runner_with_nothing_in_reach(&swap(
            &text,
            SIGNER,
            "    permissions:\n",
            "    environment: release\n    permissions:\n",
        )),
        "is in the environment",
    );
    for env in [
        "TAURI_SIGNING_PRIVATE_KEY: ${{ secrets.TAURI_SIGNING_PRIVATE_KEY }}",
        "ALL: ${{ toJSON(secrets) }}",
    ] {
        refuses(
            the_signer_is_built_on_a_hosted_runner_with_nothing_in_reach(&swap(
                &text,
                SIGNER,
                "        id: signer\n",
                &format!("        id: signer\n        env:\n          {env}\n"),
            )),
            "names a secret",
        );
    }
    refuses(
        the_signer_is_built_on_a_hosted_runner_with_nothing_in_reach(&swap(
            &text,
            SIGNER,
            "        id: signer\n",
            "        id: signer\n        env:\n          GH_TOKEN: ${{ github.token }}\n",
        )),
        "github.token",
    );
    refuses(
        the_signer_is_built_on_a_hosted_runner_with_nothing_in_reach(&swap(
            &text,
            SIGNER,
            "    permissions:\n      contents: read\n",
            "    permissions:\n      contents: read\n      actions: write\n",
        )),
        "more than `contents: read`",
    );
}

#[test]
fn a_signer_handed_on_without_its_sha256_or_under_another_name_is_refused() {
    let text = release_text();
    refuses(
        the_signer_is_built_on_a_hosted_runner_with_nothing_in_reach(&swap(
            &text,
            SIGNER,
            "      sha256: ${{ steps.signer.outputs.sha256 }}\n",
            "",
        )),
        "sha256 as an output",
    );
    refuses(
        the_signer_is_built_on_a_hosted_runner_with_nothing_in_reach(&swap(
            &text,
            SIGNER,
            r#"          echo "sha256=$sha" >> "$GITHUB_OUTPUT""#,
            r#"          echo "sha256=0000" >> "$GITHUB_OUTPUT""#,
        )),
        "does not run",
    );
    refuses(
        the_signer_is_built_on_a_hosted_runner_with_nothing_in_reach(&swap(
            &text,
            SIGNER,
            "          path: target/release/sign-updater-archive\n",
            "          path: target/release/\n",
        )),
        "uploads something other",
    );
}

#[test]
fn a_signing_job_that_compiles_the_signer_again_is_refused() {
    let broken = swap(
        &release_text(),
        SIGN,
        "      - name: The bundle `build` made\n",
        "      - run: cargo build --locked --release -p release-manifest --bin sign-updater-archive\n      - name: The bundle `build` made\n",
    );
    refuses(
        the_signing_job_runs_only_the_signer_it_checked(&broken),
        "compiles (`cargo`)",
    );
}

#[test]
fn a_signing_job_that_runs_the_signer_unchecked_is_refused() {
    let text = release_text();
    // No check at all.
    refuses(
        the_signing_job_runs_only_the_signer_it_checked(&swap(
            &text,
            SIGN,
            &format!("      - name: {SIGNER_CHECK}\n"),
            "      - name: The signer\n",
        )),
        "has no `The signer is the one build-signer built`",
    );
    // A check that compares the file with itself.
    refuses(
        the_signing_job_runs_only_the_signer_it_checked(&swap(
            &text,
            SIGN,
            r#"[ "$got" = "$SIGNER_SHA256" ]"#,
            r#"[ "$got" = "$got" ]"#,
        )),
        "does not compare",
    );
    // A check against a hash of the job's own making.
    refuses(
        the_signing_job_runs_only_the_signer_it_checked(&swap(
            &text,
            SIGN,
            "SIGNER_SHA256: ${{ needs.build-signer.outputs.sha256 }}",
            "SIGNER_SHA256: ${{ env.ANYTHING }}",
        )),
        "does not read build-signer's sha256",
    );
    // A check that may be skipped.
    refuses(
        the_signing_job_runs_only_the_signer_it_checked(&swap(
            &text,
            SIGN,
            &format!("      - name: {SIGNER_CHECK}\n"),
            &format!("      - name: {SIGNER_CHECK}\n        if: false\n"),
        )),
        "has a condition",
    );
    // A signer run from somewhere the check did not read.
    refuses(
        the_signing_job_runs_only_the_signer_it_checked(&swap(
            &text,
            SIGN,
            "          signer/sign-updater-archive dist/",
            "          ./sign-updater-archive dist/",
        )),
        "a signer other than",
    );
    // A signing job that no longer waits for the signer's job.
    refuses(
        the_signing_job_runs_only_the_signer_it_checked(&swap(
            &text,
            SIGN,
            " && needs.build-signer.result == 'success'",
            "",
        )),
        "for its success",
    );
}

#[test]
fn a_signing_job_that_runs_the_signer_before_it_checks_it_is_refused() {
    let text = release_text();
    let (start, end) = span(&text, SIGN);
    let job = &text[start..end];
    let head = format!("      - name: {SIGNER_CHECK}\n");
    let at = job.find(&head).expect("the check step");
    let len = job[at + head.len()..]
        .find("\n      - ")
        .map(|n| head.len() + n + 1)
        .expect("a step after the check");
    let check = &job[at..at + len];
    let moved = job.replacen(check, "", 1).replacen(
        "      # The `.dmg` is allowed to fail",
        &format!("{check}      # The `.dmg` is allowed to fail"),
        1,
    );
    assert_ne!(moved, job, "the check step did not move");
    let broken = format!("{}{}{}", &text[..start], moved, &text[end..]);
    refuses(
        the_signing_job_runs_only_the_signer_it_checked(&broken),
        "in that order",
    );
}

#[test]
fn a_job_that_takes_the_signer_beside_sign_macos_is_refused() {
    let broken = swap(
        &release_text(),
        "publish",
        "    steps:\n",
        "    steps:\n      - uses: actions/download-artifact@v8\n        with:\n          name: purlis-updater-signer-${{ needs.plan.outputs.version }}\n",
    );
    refuses(
        the_signing_job_runs_only_the_signer_it_checked(&broken),
        "takes the updater archive's signer",
    );
}

#[test]
fn a_job_that_downloads_by_a_pattern_that_could_take_the_signer_is_refused() {
    for with in [
        "          pattern: purlis-*-${{ needs.plan.outputs.version }}\n",
        "          path: all\n",
    ] {
        let broken = swap(
            &release_text(),
            "publish",
            "    steps:\n",
            &format!(
                "    steps:\n      - uses: actions/download-artifact@v8\n        with:\n{with}"
            ),
        );
        refuses(
            the_signing_job_runs_only_the_signer_it_checked(&broken),
            "or may, by",
        );
    }
}
