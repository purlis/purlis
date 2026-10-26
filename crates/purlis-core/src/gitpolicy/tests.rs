// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use super::*;

fn repo(origin: Option<&str>) -> (tempfile::TempDir, std::path::PathBuf) {
    let dir = tempfile::tempdir().unwrap();
    let root = std::fs::canonicalize(dir.path()).unwrap();
    std::fs::write(
        root.join("charter.toml"),
        "[[forge]]\nkind = \"gitlab\"\nhost = \"git.internal\"\n",
    )
    .unwrap();
    let clone = root.join("workspaces/alpha/widget");
    std::fs::create_dir_all(&clone).unwrap();
    // Deliberately NOT through `crate::testgit`: its template writes the very
    // `commit.gpgsign = false` these tests assert the policy writes, which would make them
    // pass without it. Nothing here commits, so no signer is ever asked.
    git::run(&clone, &["init", "-q", "."], git::READ).unwrap();
    if let Some(origin) = origin {
        git::run(&clone, &["remote", "add", "origin", origin], git::READ).unwrap();
    }
    (dir, clone)
}

fn config(repo: &Path) -> String {
    std::fs::read_to_string(repo.join(".git/config")).unwrap()
}

#[test]
fn a_github_clone_gets_ghs_helper_no_signing_and_both_ssh_rewrites() {
    let (dir, clone) = repo(Some("https://github.com/acme/widget.git"));
    let root = std::fs::canonicalize(dir.path()).unwrap();

    let changed = apply(&clone, &root);

    assert_eq!(changed.len(), 5, "{changed:?}");
    let written = config(&clone);
    assert!(
        written.contains("helper = !gh auth git-credential"),
        "{written}"
    );
    assert!(written.contains("[commit]\n\tgpgsign = false"), "{written}");
    assert!(written.contains("[tag]\n\tgpgsign = false"), "{written}");
    assert!(
        written.contains(
            "[url \"https://github.com/\"]\n\tinsteadOf = git@github.com:\n\tinsteadOf = ssh://git@github.com/"
        ),
        "{written}"
    );
    assert!(
        apply(&clone, &root).is_empty(),
        "a second apply changes nothing"
    );
}

#[test]
fn a_declared_self_hosted_forge_gets_its_own_host_never_the_default() {
    let (dir, clone) = repo(Some("git@git.internal:team/widget.git"));
    let root = std::fs::canonicalize(dir.path()).unwrap();

    apply(&clone, &root);

    let written = config(&clone);
    assert!(written.contains("!glab auth git-credential"), "{written}");
    assert!(
        written.contains("[url \"https://git.internal/\"]"),
        "{written}"
    );
    assert!(!written.contains("gitlab.com"), "{written}");
}

#[test]
fn a_host_charter_does_not_manage_gets_no_policy_rather_than_another_forges() {
    let (dir, clone) = repo(Some("https://evil.example/acme/widget.git"));
    let root = std::fs::canonicalize(dir.path()).unwrap();

    assert!(apply(&clone, &root).is_empty());
    assert!(!config(&clone).contains("credential"), "{}", config(&clone));
}

/// A clone at `workspaces/<ws>/<name>` under `root`, with `origin` when given.
///
/// Not through `crate::testgit`, for `repo`'s reason above: its template writes the
/// `commit.gpgsign = false` these tests count as drift. Nothing here commits.
fn clone_at(root: &Path, ws: &str, name: &str, origin: Option<&str>) -> PathBuf {
    let clone = root.join("workspaces").join(ws).join(name);
    std::fs::create_dir_all(&clone).unwrap();
    git::run(&clone, &["init", "-q", "."], git::READ).unwrap();
    if let Some(origin) = origin {
        git::run(&clone, &["remote", "add", "origin", origin], git::READ).unwrap();
    }
    clone
}

/// What `charter git-policy` said, in order, and what it exited with.
fn said(root: &Path, apply_it: bool) -> (Vec<Say>, u8) {
    let mut lines = Vec::new();
    let code = policy(root, apply_it, &mut |s| lines.push(s));
    (lines, code)
}

fn info(s: &str) -> Say {
    Say::Info(s.to_string().into())
}

fn warn(s: &str) -> Say {
    Say::Warn(s.to_string().into())
}

fn done(s: &str) -> Say {
    Say::Done(s.to_string().into())
}

#[test]
fn check_names_every_setting_that_is_missing_and_nothing_once_it_is_applied() {
    let (dir, clone) = repo(Some("https://github.com/acme/widget.git"));
    let root = std::fs::canonicalize(dir.path()).unwrap();

    assert_eq!(
        check(&clone, &root),
        [
            "credential.helper != !gh auth git-credential",
            "commit.gpgsign != false",
            "tag.gpgsign != false",
            "url.https://github.com/.insteadOf missing git@github.com:",
            "url.https://github.com/.insteadOf missing ssh://git@github.com/",
        ]
    );
    apply(&clone, &root);
    assert_eq!(check(&clone, &root), Vec::<String>::new());

    // The LAST value of a key is the one git uses, and the one checked.
    git::run(
        &clone,
        &["config", "--local", "--add", "commit.gpgsign", "true"],
        git::READ,
    )
    .unwrap();
    assert_eq!(check(&clone, &root), ["commit.gpgsign != false"]);
}

#[test]
fn check_answers_an_unmanaged_host_with_that_and_only_that() {
    let (dir, clone) = repo(Some("https://evil.example/acme/widget.git"));
    let root = std::fs::canonicalize(dir.path()).unwrap();
    assert_eq!(check(&clone, &root), [UNMANAGED_FORGE]);
}

#[test]
fn the_scan_finds_the_plane_and_every_clone_in_order_and_names_what_it_cannot_look_at() {
    let dir = tempfile::tempdir().unwrap();
    let root = std::fs::canonicalize(dir.path()).unwrap();
    let workspaces = root.join("workspaces");

    // Nothing at all: no plane repo, no `workspaces/`, and nothing that could not be read.
    assert_eq!(scan(&root, &workspaces), (vec![], vec![]));

    git::run(&root, &["init", "-q", "."], git::READ).unwrap();
    let b = clone_at(&root, "alpha", "b", None);
    let a = clone_at(&root, "alpha", "a", None);
    let c = clone_at(&root, "beta", "c", None);
    std::fs::create_dir_all(workspaces.join("alpha/not-a-clone")).unwrap();
    std::fs::write(workspaces.join(".DS_Store"), "").unwrap();
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink("loop", workspaces.join("loop")).unwrap();
        std::os::unix::fs::symlink(".git", workspaces.join("beta/d")).unwrap();
        std::fs::create_dir_all(workspaces.join("beta/looped")).unwrap();
        std::os::unix::fs::symlink(".git", workspaces.join("beta/looped/.git")).unwrap();
    }

    let (found, unseen) = scan(&root, &workspaces);

    assert_eq!(found, [root.clone(), a, b, c]);
    #[cfg(unix)]
    assert_eq!(
        unseen.iter().map(|u| u.path.clone()).collect::<Vec<_>>(),
        [workspaces.join("beta/looped"), workspaces.join("loop")]
    );
}

#[cfg(unix)]
#[test]
fn a_workspaces_directory_that_cannot_be_listed_is_the_one_thing_named() {
    let dir = tempfile::tempdir().unwrap();
    let root = std::fs::canonicalize(dir.path()).unwrap();
    std::os::unix::fs::symlink("workspaces", root.join("workspaces")).unwrap();

    let (found, unseen) = scan(&root, &root.join("workspaces"));
    assert!(found.is_empty());
    assert_eq!(unseen.len(), 1);
    assert_eq!(unseen[0].path, root.join("workspaces"));
    assert!(unseen[0].code.is_some());

    let (lines, code) = said(&root, false);
    assert_eq!(code, 0);
    assert_eq!(
        lines,
        [
            warn(&format!(
                "workspaces/ cannot be checked — fix the symlink loop at {}",
                root.join("workspaces").display()
            )),
            info("No git repos found (control plane + workspace clones)."),
        ]
    );
}

#[test]
fn a_plane_with_no_repos_says_so() {
    let dir = tempfile::tempdir().unwrap();
    assert_eq!(
        said(dir.path(), true),
        (
            vec![info(
                "No git repos found (control plane + workspace clones)."
            )],
            0
        )
    );
}

#[test]
fn the_report_names_each_drifted_and_unmanaged_repo_and_counts_them() {
    let dir = tempfile::tempdir().unwrap();
    let root = std::fs::canonicalize(dir.path()).unwrap();
    clone_at(
        &root,
        "alpha",
        "widget",
        Some("https://github.com/acme/widget.git"),
    );
    clone_at(
        &root,
        "alpha",
        "foreign",
        Some("https://evil.example/acme/foreign.git"),
    );

    let (lines, code) = said(&root, false);

    assert_eq!(code, 0, "drift is a report, not a failure");
    assert_eq!(
        lines,
        [
            warn(&format!("workspaces/alpha/foreign: {UNMANAGED_FORGE}")),
            warn("workspaces/alpha/widget: 5 settings not token-only"),
            info("    credential.helper != !gh auth git-credential"),
            info("    commit.gpgsign != false"),
            info("    tag.gpgsign != false"),
            info("    url.https://github.com/.insteadOf missing git@github.com:"),
            info("1 of 2 repos drifted — fix: purlis git-policy --apply"),
            warn(
                "1 repo has an unrecognised forge — not covered by any policy. Declare \
                 the host in charter.toml's [[forge]] to bring it under management."
            ),
        ]
    );
}

#[test]
fn applying_fixes_every_drifted_repo_and_then_there_is_nothing_to_say_but_that() {
    let dir = tempfile::tempdir().unwrap();
    let root = std::fs::canonicalize(dir.path()).unwrap();
    // The plane itself, with no origin: GitLab's policy, the pre-multi-forge default.
    git::run(&root, &["init", "-q", "."], git::READ).unwrap();
    clone_at(
        &root,
        "alpha",
        "widget",
        Some("https://github.com/acme/widget.git"),
    );
    clone_at(&root, "alpha", "one", Some("https://one.example/x.git"));
    clone_at(&root, "beta", "two", Some("https://two.example/x.git"));

    let (lines, _) = said(&root, true);

    assert_eq!(
        lines,
        [
            done("control plane: applied 5 settings — token-only"),
            warn(&format!("workspaces/alpha/one: {UNMANAGED_FORGE}")),
            done("workspaces/alpha/widget: applied 5 settings — token-only"),
            warn(&format!("workspaces/beta/two: {UNMANAGED_FORGE}")),
            done("Applied the single-credential policy to 2 of 4 repos."),
            warn(
                "2 repos have an unrecognised forge — not covered by any policy. Declare \
                 the host in charter.toml's [[forge]] to bring them under management."
            ),
        ]
    );

    std::fs::remove_dir_all(root.join("workspaces/alpha/one")).unwrap();
    std::fs::remove_dir_all(root.join("workspaces/beta")).unwrap();
    assert_eq!(
        said(&root, false).0,
        [done(
            "All 2 repos are token-only (each forge's own HTTPS token, no SSH, no signing)."
        )]
    );
}

#[test]
fn an_unmanaged_repo_alone_is_not_reported_as_drift() {
    let dir = tempfile::tempdir().unwrap();
    let root = std::fs::canonicalize(dir.path()).unwrap();
    clone_at(&root, "alpha", "one", Some("https://one.example/x.git"));

    assert_eq!(
        said(&root, false).0,
        [
            warn(&format!("workspaces/alpha/one: {UNMANAGED_FORGE}")),
            warn(
                "1 repo has an unrecognised forge — not covered by any policy. Declare \
                 the host in charter.toml's [[forge]] to bring it under management."
            ),
        ]
    );
}

#[test]
fn one_setting_out_of_place_is_drift_and_never_mistaken_for_an_unmanaged_forge() {
    let dir = tempfile::tempdir().unwrap();
    let root = std::fs::canonicalize(dir.path()).unwrap();
    let clone = clone_at(
        &root,
        "alpha",
        "widget",
        Some("https://github.com/acme/widget.git"),
    );
    apply(&clone, &root);
    git::run(
        &clone,
        &["config", "--local", "--unset", "tag.gpgsign"],
        git::READ,
    )
    .unwrap();

    assert_eq!(
        said(&root, false).0,
        [
            warn("workspaces/alpha/widget: 1 setting not token-only"),
            info("    tag.gpgsign != false"),
            info("1 of 1 repo drifted — fix: purlis git-policy --apply"),
        ]
    );
}
