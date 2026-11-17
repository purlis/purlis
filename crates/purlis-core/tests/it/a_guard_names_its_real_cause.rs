// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A guard's refusal names its real cause** (#6962, T1.1-72).
//!
//! - Creating a branch and committing a file through the API is no merge. A new ref whose name
//!   cannot be read is refused in its own words, a tag is held as a release, and a file committed
//!   to the default branch is refused as the push to it that it is, naming `purlis worktree
//!   push`. None of them is the merge hold's, so none points at `gh pr merge`.
//! - A `curl --variable` placeholder is a name, not a credential, to every guard that reads a
//!   line for one ([`secretshape::kind_as_read`]: a dispatch's and a handoff's brief, a memory
//!   write) and to a save's scan of a staged file ([`secretshape::found_as_read`], the save's
//!   own question). No arm of the Bash gate refuses the bare line on its own today.
//!
//! The time-limit refusal is the hook's, in
//! `crates/purlis-cli/tests/it/a_guard_decides_within_its_budget.rs`.

use purlis_core::floorguard::release_floor_reason;
use purlis_core::mergehold;
use purlis_core::secretshape::{self, Structured};

/// What the merge hold says when it cannot name a request: the wrong words for every line here.
const POINTS_AT_A_MERGE: &str = "gh pr merge";

/// The release floor's refusal of `cmd` in a chat nobody is at.
fn refused(cmd: &str) -> String {
    release_floor_reason(cmd, true).unwrap_or_else(|| panic!("not refused unattended: {cmd}"))
}

/// Neither the floor nor the merge hold refuses `cmd`.
fn allowed(cmd: &str) {
    assert_eq!(release_floor_reason(cmd, true), None, "the floor: {cmd}");
    assert_eq!(mergehold::held(cmd), None, "the merge hold: {cmd}");
}

#[test]
fn creating_a_literal_branch_through_git_refs_is_allowed() {
    purlis_core::unsteered!();
    for cmd in [
        "gh api -X POST repos/o/r/git/refs -f ref=refs/heads/fix -f sha=abc",
        "gh api -X POST repos/o/r/git/refs -f ref=refs/heads/fix -f sha=\"$(git rev-parse HEAD)\"",
        "gh api repos/o/r/git/refs -F ref=refs/heads/t1.1-72-fix -f sha=abc",
    ] {
        allowed(cmd);
    }
}

#[test]
fn an_opaque_or_shell_filled_git_refs_ref_is_refused_with_write_it_out_literally() {
    purlis_core::unsteered!();
    for cmd in [
        "gh api -X POST repos/o/r/git/refs -f ref=\"refs/heads/$BRANCH\" -f sha=abc",
        "gh api -X POST repos/o/r/git/refs -f ref=\"$REF\" -f sha=abc",
        "gh api -X POST repos/o/r/git/refs --input ref.json",
        "gh api -X POST repos/o/r/git/refs -f sha=abc",
    ] {
        let said = refused(cmd);
        assert!(
            said.contains("can't tell what this ref writes; write it out literally"),
            "{cmd}: {said}"
        );
        assert!(!said.contains("merge"), "{cmd}: {said}");
        assert_eq!(mergehold::held(cmd), None, "no merge to hold: {cmd}");
        assert_eq!(release_floor_reason(cmd, false), None, "attended: {cmd}");
    }
}

#[test]
fn a_tag_through_git_refs_is_held_as_a_release() {
    purlis_core::unsteered!();
    for cmd in [
        "gh api -X POST repos/o/r/git/refs -f ref=refs/tags/v1.2.3 -f sha=abc",
        "gh api -X PATCH repos/o/r/git/refs/tags/v1.2.3 -f sha=abc",
    ] {
        let said = refused(cmd);
        assert!(
            said.contains("A release or a tag made through the API publishes"),
            "{cmd}: {said}"
        );
        assert!(!said.contains(POINTS_AT_A_MERGE), "{cmd}: {said}");
        assert_eq!(mergehold::held(cmd), None, "a tag is no merge: {cmd}");
    }
}

#[test]
fn putting_contents_on_a_literal_non_default_branch_is_allowed() {
    purlis_core::unsteered!();
    for cmd in [
        "gh api -X PUT repos/o/r/contents/docs/a.md -f branch=fix -f message=m -f content=eA==",
        "gh api -X PUT repos/o/r/contents/a.md -f message=m -f content=\"$(base64 < a.md)\" -f branch=t1.1-72-fix",
        "gh api --method PUT repos/{owner}/{repo}/contents/a.md -f branch=fix -f message=m -f content=eA==",
        "gh api -X DELETE repos/o/r/contents/a.md -f branch=fix -f message=m -f sha=abc",
    ] {
        allowed(cmd);
    }
}

#[test]
fn putting_contents_with_no_branch_or_on_the_default_branch_is_refused_as_a_push_naming_purlis_worktree_push()
 {
    purlis_core::unsteered!();
    for cmd in [
        "gh api -X PUT repos/o/r/contents/a.md -f message=m -f content=eA==",
        "gh api -X PUT repos/o/r/contents/a.md -f branch=main -f message=m -f content=eA==",
        "gh api -X PUT repos/o/r/contents/a.md -f branch=master -f message=m -f content=eA==",
        "gh api -X PUT repos/o/r/contents/a.md -f branch=\"$B\" -f message=m -f content=eA==",
        "gh api -X PUT repos/o/r/contents/a.md --input body.json",
        "gh api -X DELETE repos/o/r/contents/a.md -f message=m -f sha=abc",
    ] {
        let said = refused(cmd);
        assert!(said.contains("push to"), "{cmd}: {said}");
        assert!(said.contains("default branch"), "{cmd}: {said}");
        assert!(said.contains("`purlis worktree push`"), "{cmd}: {said}");
        assert!(!said.contains(POINTS_AT_A_MERGE), "{cmd}: {said}");
        assert!(!said.contains("merge"), "{cmd}: {said}");
        assert_eq!(mergehold::held(cmd), None, "no merge to hold: {cmd}");
        assert_eq!(release_floor_reason(cmd, false), None, "attended: {cmd}");
    }
}

/// The password grant the report describes, in the shapes the secrets guide teaches.
const PLACEHOLDER_LINES: [&str; 4] = [
    "curl --variable %AUTH_PASSWORD --expand-data 'grant_type=password&username=me&password={{AUTH_PASSWORD}}' https://id.example.com/token",
    "purlis secret exec idp --env AUTH_PASSWORD=idp-password -- curl -s --variable %AUTH_PASSWORD --expand-data \"password={{AUTH_PASSWORD}}&grant_type=password\" https://id.example.com/token",
    "curl --variable 'AUTH_TOKEN@/run/token' --expand-header 'X-Token: x' --expand-data token={{AUTH_TOKEN}} https://x",
    "curl --variable %API_SECRET --expand-data secret={{API_SECRET}} https://x",
];

/// Whether `text` is read as holding a credential by a guard that reads a line, or by a save's
/// scan of a staged file of `path`.
fn a_credential(text: &str, path: &str) -> (bool, bool) {
    (
        secretshape::kind_as_read(None, text).is_some(),
        secretshape::found_as_read(Structured::of(path), text).is_some(),
    )
}

#[test]
fn a_curl_variable_placeholder_is_not_a_credential_to_the_command_guard() {
    purlis_core::unsteered!();
    for line in PLACEHOLDER_LINES {
        assert_eq!(secretshape::secret_kind(line), None, "{line}");
        assert_eq!(a_credential(line, "notes.md"), (false, false), "{line}");
    }
}

#[test]
fn a_literal_password_in_the_same_field_is_still_a_credential() {
    purlis_core::unsteered!();
    for line in [
        "curl --variable %AUTH_PASSWORD --expand-data 'grant_type=password&password=hunter2is' https://x",
        "curl --variable %AUTH_PASSWORD --expand-data 'password={{AUTH_PASSWORD}}x9' https://x",
        "curl --variable %AUTH_PASSWORD --expand-data 'password={{AUTH_PASSWORD}}&secret=hunter2is' https://x",
    ] {
        assert_eq!(a_credential(line, "notes.md"), (true, true), "{line}");
    }
}

#[test]
fn a_placeholder_no_variable_on_the_line_names_is_still_a_credential() {
    purlis_core::unsteered!();
    for line in [
        "curl --expand-data 'password={{AUTH_PASSWORD}}' https://x",
        "curl --variable %OTHER_PASSWORD --expand-data 'password={{AUTH_PASSWORD}}' https://x",
        "curl --variable %AUTH_PASSWORD_2 --expand-data 'password={{AUTH_PASSWORD}}' https://x",
        "curl --variable %AUTH_PASSWORD\ncurl --expand-data 'password={{AUTH_PASSWORD}}' https://x",
        "echo --variable-AUTH_PASSWORD 'password={{AUTH_PASSWORD}}'",
    ] {
        assert_eq!(a_credential(line, "notes.md"), (true, true), "{line:?}");
    }
}

/// S2: a `-F` field gh reads from a file (`@name`) or fills in from the checkout (`{branch}`)
/// holds no value on the line, so purlis cannot read the branch or ref it names.
#[test]
fn a_typed_branch_read_from_a_file_or_filled_in_by_gh_is_refused_as_a_push() {
    purlis_core::unsteered!();
    for cmd in [
        "gh api -X PUT repos/o/r/contents/a.md -F branch=@branch.txt -f message=m -f content=eA==",
        "gh api -X PUT repos/o/r/contents/a.md -F branch={branch} -f message=m -f content=eA==",
        "gh api -X PUT repos/o/r/contents/a.md -F 'branch=x-{branch}' -f message=m -f content=eA==",
    ] {
        let said = refused(cmd);
        assert!(said.contains("default branch"), "{cmd}: {said}");
        assert!(said.contains("`purlis worktree push`"), "{cmd}: {said}");
    }
}

#[test]
fn a_typed_ref_read_from_a_file_or_filled_in_by_gh_is_refused_with_write_it_out_literally() {
    purlis_core::unsteered!();
    for cmd in [
        "gh api -X POST repos/o/r/git/refs -F ref=@ref.txt -f sha=abc",
        "gh api -X POST repos/o/r/git/refs -F ref=refs/heads/{branch} -f sha=abc",
    ] {
        let said = refused(cmd);
        assert!(
            said.contains("can't tell what this ref writes; write it out literally"),
            "{cmd}: {said}"
        );
    }
}

/// S3: a contents call on a repository named by id, or through a GHES full URL, commits too.
#[test]
fn putting_contents_by_repository_id_or_a_ghes_url_is_refused_as_a_push() {
    purlis_core::unsteered!();
    for cmd in [
        "gh api -X PUT repositories/123/contents/a.md -f message=m -f content=eA==",
        "gh api -X PUT https://ghe.example.com/api/v3/repos/o/r/contents/a.md -f branch=main -f message=m -f content=eA==",
    ] {
        let said = refused(cmd);
        assert!(said.contains("default branch"), "{cmd}: {said}");
        assert!(said.contains("`purlis worktree push`"), "{cmd}: {said}");
    }
}

/// S5: a `--variable` that defines the name from a value on the same line puts that value on
/// the line, so its placeholder does not make the line clean. The value is built from parts.
#[test]
fn a_placeholder_whose_variable_is_defined_from_a_literal_on_the_line_is_still_a_credential() {
    purlis_core::unsteered!();
    let value = ["not", "a", "real", "value"].join("-");
    for line in [
        format!(
            "curl --variable AUTH_PASSWORD={value} --expand-data 'password={{{{AUTH_PASSWORD}}}}' https://x"
        ),
        format!(
            "curl --variable '%AUTH_PASSWORD={value}' --expand-data 'password={{{{AUTH_PASSWORD}}}}' https://x"
        ),
    ] {
        assert_eq!(a_credential(&line, "notes.md"), (true, true), "{line}");
    }
}

/// The nit: `--variable` counts only as an argument of curl, not in a comment or another
/// program's words.
#[test]
fn a_variable_flag_outside_curls_arguments_does_not_name_the_placeholder() {
    purlis_core::unsteered!();
    for line in [
        "curl --expand-data 'password={{AUTH_PASSWORD}}' https://x # --variable %AUTH_PASSWORD",
        "echo --variable %AUTH_PASSWORD 'password={{AUTH_PASSWORD}}'",
    ] {
        assert_eq!(a_credential(line, "notes.md"), (true, true), "{line}");
    }
}
