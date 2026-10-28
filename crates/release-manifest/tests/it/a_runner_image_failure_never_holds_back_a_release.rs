// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A failed runner image never holds back a desktop release** (#2880). `release.yml` calls
//! `runner-image.yml` for each release it publishes (#2749); when that fails, the desktop is
//! still published: its manifest names no `runnerImage`, and every pool stays on the runner it
//! has. So the jobs after the image's ask for what they need by name, and never for the image.

use crate::workflow;

use workflow::{Job, job, release_jobs, run_lines, step_value};

/// The jobs the desktop's release waits on, but never the runner image's.
const RELEASE: [&str; 3] = ["manifest", "provenance", "publish"];

/// The job's `if:`, unwrapped from `${{ … }}`.
fn condition(job: &Job) -> String {
    let said = job.condition().unwrap_or_default();
    said.strip_prefix("${{")
        .and_then(|said| said.strip_suffix("}}"))
        .unwrap_or(said)
        .trim()
        .to_owned()
}

#[test]
fn the_desktop_is_published_whatever_became_of_the_runner_image() {
    let jobs = release_jobs();
    for name in RELEASE {
        let job = job(&jobs, name);
        let said = condition(job);
        // Run even where a job before it failed, unless the run was cancelled…
        assert!(said.starts_with("!cancelled() && "), "{name}: {said}");
        assert!(
            said.contains("needs.plan.outputs.publish == 'true'"),
            "{name}: {said}"
        );
        // …and only once each job it needs for the desktop has succeeded, the image's aside.
        for need in job.needs() {
            if need.starts_with("runner-image") {
                assert!(
                    !said.contains(&format!("needs.{need}.result")),
                    "{name} waits on {need}: {said}"
                );
            } else {
                assert!(
                    said.contains(&format!("needs.{need}.result == 'success'")),
                    "{name} does not ask for {need} by name: {said}"
                );
            }
        }
    }
    // The image's signature is made only for an image that was built.
    let signature = condition(job(&jobs, "runner-image-signature"));
    assert!(
        !signature.contains("always()") && !signature.contains("cancelled()"),
        "{signature}"
    );
}

#[test]
fn a_manifest_names_the_runner_image_only_where_it_was_signed() {
    let jobs = release_jobs();
    let manifest = job(&jobs, "manifest");
    let steps = manifest.steps();
    let download = steps
        .iter()
        .find(|step| {
            step_value(step, "with", "name")
                == Some("purlis-runner-image-${{ needs.plan.outputs.version }}")
        })
        .expect("the manifest downloads the runner image's line");
    assert_eq!(
        download
            .iter()
            .find_map(|line| line.trim().strip_prefix("if:"))
            .map(str::trim),
        Some("needs.runner-image-signature.result == 'success'")
    );
    let writes = steps
        .iter()
        .find(|step| step[0].contains("name: The manifest, and every check it has to pass first"))
        .expect("the manifest is written");
    assert_eq!(
        step_value(writes, "env", "RUNNER_IMAGE_SIGNED"),
        Some("${{ needs.runner-image-signature.result }}")
    );
    let script = run_lines(writes).join("\n");
    assert!(
        script.contains("if [ \"$RUNNER_IMAGE_SIGNED\" = success ]; then"),
        "{script}"
    );
    assert!(script.contains("\"${runner[@]}\""), "{script}");
}
