// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! The merge hold holds what merges or moves a protected ref, and nothing else (#7176; Q330,
//! Q331). A read of the forge, CI job control, an edit of a request's metadata, text that only
//! mentions a forge CLI, and a write to a branch that is not protected each pass; every merge
//! stays held, standing alone, in a substitution, behind a pipe or chained.

use purlis_core::floorguard::{merge_reason, release_floor_reason};
use purlis_core::mergehold;

/// Each passes the hold and the floor's merge half, unattended.
fn passes(cmds: &[&str]) {
    for cmd in cmds {
        assert_eq!(mergehold::held(cmd), None, "held: {cmd}");
        assert_eq!(merge_reason(cmd), None, "a merge to the floor: {cmd}");
    }
}

/// Each is held, and refused unattended.
fn held(cmds: &[&str]) {
    for cmd in cmds {
        assert!(mergehold::held(cmd).is_some(), "not held: {cmd}");
        assert!(
            release_floor_reason(cmd, true).is_some(),
            "not floored: {cmd}"
        );
    }
}

// Shape 1.
#[test]
fn a_read_only_glab_get_of_a_job_trace_in_a_pipeline_passes() {
    purlis_core::unsteered!();
    passes(&[
        "glab api projects/purlis%2Fpurlis/jobs/123/trace | sed 's/\\x1b\\[[0-9;]*m//g' | grep -E 'FAIL|error' | head -20",
        "J=$(glab api 'projects/p/pipelines/9/jobs' | jq -r '.[0].id'); glab api projects/p/jobs/$J/trace | grep -c ok",
        "glab api \"projects/p/jobs/$(glab api projects/p/pipelines/9/jobs | jq -r '.[0].id')/trace\" | tail -50",
        "glab api projects/p/jobs/123/trace 2>&1 | sed -n '1,40p' > $TMPDIR/trace.log; grep -n panicked $TMPDIR/trace.log",
    ]);
}

#[test]
fn read_only_glab_gets_with_variables_and_substitutions_pass() {
    purlis_core::unsteered!();
    passes(&[
        "glab api \"projects/$P/jobs/$J/trace\" | sed 's/\\x1b\\[[0-9;]*m//g' | grep -E 'FAIL' | head",
        "glab api \"projects/$(echo $P | sed 's|/|%2F|g')/jobs/$J/trace\" | tail -n 40",
        "P=purlis%2Fpurlis; J=$(glab api projects/$P/pipelines/$PL/jobs | jq -r '.[]|select(.name==\"x\")|.id'); glab api projects/$P/jobs/$J/trace 2>&1 | tail -30",
        "glab api -X GET projects/$P/repository/commits/$SHA && glab api \"projects/$P/repository/files/a.md/raw?ref=$SHA\" | wc -l",
        "glab api projects/$P/repository/commits/$SHA --jq .title; node -e 'console.log(process.argv[1])' \"$(glab api projects/$P/repository/files/a.md/raw?ref=main)\"",
    ]);
}

// Shape 2.
#[test]
fn a_branch_poll_until_loop_passes() {
    purlis_core::unsteered!();
    passes(&[
        "until [ \"$(glab api projects/p/repository/branches/main | jq -r .commit.id)\" = \"$SHA\" ]; do sleep 20; done",
        "(until [ \"$(glab api projects/p/repository/branches/main | jq -r .commit.id)\" = \"$SHA\" ]; do sleep 20; done; echo done) &",
        "until glab api projects/p/repository/branches/main | grep -q \"$SHA\"; do sleep 20; done",
    ]);
}

// Shape 3.
#[test]
fn a_heredoc_that_only_mentions_glab_passes() {
    purlis_core::unsteered!();
    passes(&[
        "python3 -I - <<'PY'\nopen('notes.md','w').write('glab mr merge 5\\nglab api -X PUT projects/p/merge_requests/5/merge\\n')\nPY\ndiff notes.md old.md; bash -n run.sh",
        "python3 -I - <<'PY'\nlines = ['glab api -X PUT projects/1/merge_requests/2/merge', 'gh pr merge 3']\nopen('f.txt','w').write('\\n'.join(lines))\nPY\ndiff f.txt g.txt\nbash -n h.sh",
    ]);
}

// Shape 4.
#[test]
fn chained_read_only_glab_gets_with_a_node_script_pass() {
    purlis_core::unsteered!();
    passes(&[
        "glab api projects/p/repository/commits/abc123 && glab api 'projects/p/repository/files/docs%2Fa.md/raw?ref=main' && node tools/check.mjs",
        "glab api projects/p/repository/commits/abc123 | jq .id; glab api \"projects/p/repository/files/a.md/raw?ref=$SHA\" > $TMPDIR/a.md; node -e 'console.log(1)'",
    ]);
}

// Shape 5.
#[test]
fn playing_a_manual_ci_job_passes() {
    purlis_core::unsteered!();
    passes(&[
        "glab api -X POST projects/p/jobs/123/play",
        "for j in 1 2 3; do glab api -X POST projects/p/jobs/$j/play; done",
        "for j in 101 102; do glab api -X POST projects/p/jobs/$j/play; done",
        "J=101; glab api -X POST projects/p/jobs/$J/play",
        "J=101\nglab api -X POST \"projects/p/jobs/${J}/play\"",
        "glab api --method POST projects/p/jobs/123/retry",
    ]);
}

// Shape 5, as the ruling of Q331 reads it: a variable in a writing path is held unless the line
// binds it to digits. The loop over `$(...)` is the accepted false hold.
#[test]
fn variables_in_writing_paths_hold() {
    purlis_core::unsteered!();
    held(&[
        "for j in $(glab api projects/p/pipelines/9/jobs | jq -r '.[] | .id'); do glab api -X POST \"projects/p/jobs/$j/play\"; done",
        // B1: a variable in an id's place carries a path.
        "n='3/merge'; glab api -X PUT \"projects/p/merge_requests/$n\"",
        "for n in 3/merge; do gh api -X PUT \"repos/o/r/pulls/$n\"; done",
        "n='3/merge'; gh api -X PUT repos/o/r/pulls/$n -f sha=abc",
        "n=3/merge glab api -X PUT projects/p/merge_requests/$n",
        "export n=3/merge; glab api -X PUT projects/p/merge_requests/$n",
        "read n < f; glab api -X PUT projects/p/merge_requests/$n",
        "n=$(cat f); glab api -X PUT projects/p/merge_requests/$n",
        "n='3/merge'; glab api -X PUT projects/p/merge_requests/${n}",
        "n='3/merge'; glab api -X PUT projects/p/merge_requests/$n/",
        "n='3/merge'; gh api -X PUT repos/{owner}/{repo}/pulls/$n",
        "n='3/merge'; glab api -X PUT https://gitlab.com/api/v4/projects/p/merge_requests/$n",
        // B2: a variable in the project, repository or collection re-routes the call.
        "P='1/merge_requests/3/merge?x='; glab api -X PUT \"projects/$P/jobs/$J/play\"",
        "ID='1/git/refs/heads/main?x='; gh api -X PATCH repositories/$ID/issues/5 -f sha=abc",
        "P='1/repository/branches/main?x='; glab api -X DELETE projects/$P/issues/5",
        "P='1/protected_branches/main?x='; glab api -X DELETE projects/$P/labels/5",
        "ID='1/pulls/3/merge?x='; gh api -X PUT repositories/$ID/issues/5",
        "j='1/play -X PUT projects/p/merge_requests/3/merge'; glab api -X POST projects/p/jobs/$j",
        // A digit binding that does not hold on the line.
        "J=101 glab api -X POST projects/p/jobs/$J/play",
        "glab api -X POST projects/p/jobs/$J/play; J=101",
        "J=101; J=$(cat f); glab api -X POST projects/p/jobs/$J/play",
        "J=101; read J; glab api -X POST projects/p/jobs/$J/play",
        "J=101 && glab api -X POST projects/p/jobs/$J/play",
        "(J=101); glab api -X POST projects/p/jobs/$J/play",
        "J=1/merge; glab api -X POST projects/p/jobs/$J/play",
        "for j in 1 2/merge; do glab api -X POST projects/p/merge_requests/$j; done",
    ]);
}

#[test]
fn a_held_loop_over_a_substitution_says_to_loop_over_digits() {
    purlis_core::unsteered!();
    let cmd = "for j in $(glab api projects/p/pipelines/9/jobs | jq -r '.[].id'); do glab api -X POST \"projects/p/jobs/$j/play\"; done";
    let said = release_floor_reason(cmd, true).expect("held");
    assert!(said.contains("for j in 101 102"), "{said}");
    let held = mergehold::held(cmd).expect("held");
    let refusal = mergehold::refusal_for(cmd, &held, &mergehold::AppSaid::Unheard);
    assert!(refusal.contains("projects/p/jobs/$j/play"), "{refusal}");
    assert!(refusal.contains("for j in 101 102"), "{refusal}");
}

// Shape 6.
#[test]
fn editing_a_merge_requests_metadata_passes() {
    purlis_core::unsteered!();
    passes(&[
        "glab api --method PUT projects/p/merge_requests/7/ --input $TMPDIR/body.json",
        "glab api --method PUT projects/p/merge_requests/7 --input $TMPDIR/body.json",
        "glab api -X PUT projects/p/merge_requests/7 -f title='New title' -f description='Text'",
        "gh api -X PATCH repos/o/r/pulls/7 -f title='New title'",
    ]);
}

// Shape 7.
#[test]
fn a_new_branch_and_a_commit_to_it_through_the_api_pass_the_merge_hold() {
    purlis_core::unsteered!();
    passes(&[
        "gh api -X POST repos/o/r/git/refs -f ref=refs/heads/feature-x -f sha=abc",
        "gh api -X PUT repos/o/r/contents/docs/a.md -f message=m -f content=Zm9v -f branch=feature-x",
    ]);
}

#[test]
fn every_merge_form_stays_held_alone_in_a_substitution_behind_a_pipe_and_chained() {
    purlis_core::unsteered!();
    let forms = [
        "gh pr merge 12",
        "glab mr merge 12",
        "gh api -X PUT repos/o/r/pulls/12/merge",
        "glab api -X PUT projects/p/merge_requests/12/merge",
        "glab api --method PUT projects/p/merge_requests/12/merge --input $TMPDIR/b.json",
        "gh api graphql -f query='mutation { mergePullRequest(input: {pullRequestId: \"x\"}) { clientMutationId } }'",
        "gh api -X PATCH repos/o/r/git/refs/heads/main -f sha=abc",
        "gh api -X POST repos/o/r/git/refs -f ref=refs/heads/main -f sha=abc",
        "glab api -X POST projects/p/repository/commits -f branch=main",
        // A variable in an id's place does not hide a merge: the verb is written out.
        "glab api -X PUT projects/$P/merge_requests/$N/merge",
        "gh api -X PUT repos/$R/pulls/$N/merge",
        "gh api -X PUT repos/o/r/pulls/$N/merge -f merge_method=squash",
        // A variable where it could be a verb, a repository or a branch is read as a merge.
        "glab api -X PUT projects/p/merge_requests/7/$V",
        "gh api -X PATCH repos/o/r/git/refs/heads/$B -f sha=abc",
        "gh api -X POST repos/o/r/git/refs -f ref=\"refs/heads/$B\" -f sha=abc",
        "glab api -X POST \"projects/p/$(echo merge_requests/5/merge)\"",
    ];
    for form in forms {
        held(&[
            form,
            &format!("x=$({form})"),
            &format!("{form} | head -1"),
            &format!("echo start && {form}; echo end"),
            &format!("glab api projects/p/jobs/1/trace | grep ok; {form}"),
            &format!("for i in 1 2; do {form}; done"),
            &format!("until {form}; do sleep 1; done"),
        ]);
    }
}

/// A file committed through the API to the default branch is a push, which the floor refuses in
/// its own words and the merge hold leaves alone (#6962): it names the route, `purlis worktree
/// push`, and does not call the call a merge.
#[test]
fn a_commit_to_the_default_branch_through_the_api_names_the_push_route_not_a_merge() {
    purlis_core::unsteered!();
    let cmd = "gh api -X PUT repos/o/r/contents/a.md -f message=m -f content=Zm9v -f branch=main";
    let said = release_floor_reason(cmd, true).expect("refused unattended");
    assert!(said.contains("purlis worktree push"), "{said}");
    assert!(!said.contains("gh pr merge"), "{said}");
    assert_eq!(mergehold::held(cmd), None);
}

/// A refusal of a line the hold took for a merge says what it took for one.
#[test]
fn an_unreadable_refusal_names_what_it_took_for_a_merge() {
    purlis_core::unsteered!();
    for (cmd, took) in [
        (
            "gh api -X POST repos/o/r/git/refs -f ref=\"refs/heads/$B\" -f sha=abc",
            "default branch",
        ),
        (
            "gh api -X PUT repos/$R/pulls/12/merge",
            "filled in by the shell",
        ),
        ("gh pr merge feature-branch", "`gh pr merge`"),
    ] {
        let held = mergehold::held(cmd).expect(cmd);
        let said = mergehold::refusal_for(cmd, &held, &mergehold::AppSaid::Unheard);
        assert!(said.contains(took), "{cmd}: {said}");
        assert!(said.contains("nobody was asked"), "{cmd}: {said}");
        assert!(
            !said.contains("cannot tell which request this merges"),
            "{said}"
        );
    }
}
