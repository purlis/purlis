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
        "J=$(glab api 'projects/p/pipelines/9/jobs' | jq -r '.[0].id'); glab api \"projects/p/jobs/$J/trace\" | grep -c ok",
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
        "P=purlis%2Fpurlis; J=$(glab api \"projects/$P/pipelines/$PL/jobs\" | jq -r '.[]|select(.name==\"x\")|.id'); glab api \"projects/$P/jobs/$J/trace\" 2>&1 | tail -30",
        "glab api -X GET \"projects/$P/repository/commits/$SHA\" && glab api \"projects/$P/repository/files/a.md/raw?ref=$SHA\" | wc -l",
        "glab api \"projects/$P/repository/commits/$SHA\" --jq .title; node -e 'console.log(process.argv[1])' \"$(glab api \"projects/$P/repository/files/a.md/raw?ref=main\")\"",
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
        "for j in 1 2\ndo\nglab api -X POST projects/p/jobs/$j/play\ndone",
        "for j in 1 2; do\n  glab api -X POST projects/p/jobs/${j}/play;\ndone",
        // An MR's metadata edit, the id bound to digits.
        "M=7; glab api --method PUT projects/p/merge_requests/$M -f title=T",
        "for m in 7 8; do glab api --method PUT projects/p/merge_requests/$m/ -f title=T; done",
    ]);
}

// Round 3: the only lines where a variable may stand in a writing path are the two exact shapes.
// Each line below was run under real bash and wrote the variable some way the reader missed.
#[test]
fn bash_writing_a_variable_some_other_way_is_held() {
    purlis_core::unsteered!();
    held(&[
        // B1: bash writes the variable implicitly.
        "_=1; : 3/merge; gh api -X PUT repos/o/r/pulls/$_",
        "REPLY=1; read <<< 3/merge; gh api -X PUT repos/o/r/pulls/$REPLY",
        "BASH_REMATCH=1; [[ 3/merge =~ .* ]]; glab api -X PUT projects/p/merge_requests/$BASH_REMATCH",
        "OPTARG=1; getopts a: o -a 3/merge; glab api -X PUT projects/p/merge_requests/$OPTARG",
        "MAPFILE=1; mapfile <<< 3/merge; glab api -X PUT projects/p/merge_requests/$MAPFILE",
        "REPLY=1; glab api -X PUT projects/p/merge_requests/$REPLY",
        // B2: the name quoted or escaped.
        "JOB=1; declare J''OB=3/merge; gh api -X PUT repos/o/r/pulls/$JOB",
        "JOB=1; read J\\OB <<< 3/merge; gh api -X PUT repos/o/r/pulls/$JOB",
        "JOB=1; export J\"O\"B=3/merge; gh api -X PUT repos/o/r/pulls/$JOB",
        "JOB=1; printf -v J''OB 3/merge; gh api -X PUT repos/o/r/pulls/$JOB",
        "Q=1; declare $'\\x51=3/merge'; gh api -X PUT repos/o/r/pulls/$Q",
        "'Q'=1; gh api -X PUT repos/o/r/pulls/$Q",
        // B3: IFS.
        "Q=0; IFS=0; gh api -X PATCH repos/o/r/git/refs/heads/main$Q -f sha=abc",
        "Q=10; IFS=0; gh api -X PUT repos/o/r/pulls/$Q",
        // B4: code the reader cannot see.
        "Q=1; source f; gh api -X PUT repos/o/r/pulls/$Q",
        "Q=1; . f; gh api -X PUT repos/o/r/pulls/$Q",
        "Q=1; trap 'source f' DEBUG; gh api -X PUT repos/o/r/pulls/$Q",
        "Q=1; eval \"$Z\"; gh api -X PUT repos/o/r/pulls/$Q",
        // B5: text that looks like a binding and binds nothing.
        ": ';Q=1;'; gh api -X PUT repos/o/r/pulls/$Q",
        "echo do Q=1; gh api -X PUT repos/o/r/pulls/$Q",
        "if false; then :; Q=123; fi; gh api -X PUT repos/o/r/pulls/$Q",
        "cat <<EOF\nQ=1\nEOF\ngh api -X PUT repos/o/r/pulls/$Q",
        "{ :; Q=1; } | cat; gh api -X PUT repos/o/r/pulls/$Q",
        "false &&\nQ=1\ngh api -X PUT repos/o/r/pulls/$Q",
    ]);
}

#[test]
fn a_near_miss_of_the_two_shapes_is_held() {
    purlis_core::unsteered!();
    held(&[
        // An extra statement before or after.
        "echo hi; J=101; glab api -X PUT projects/p/merge_requests/$J",
        "J=101; glab api -X PUT projects/p/merge_requests/$J; echo done",
        "J=101; glab api -X PUT projects/p/merge_requests/$J | tail",
        "J=101; glab api -X PUT projects/p/merge_requests/$J && true",
        "J=101; J=2; glab api -X PUT projects/p/merge_requests/$J",
        "for j in 1 2; do echo $j; glab api -X PUT projects/p/merge_requests/$j; done",
        "for j in 1 2; do glab api -X PUT projects/p/merge_requests/$j; done; echo done",
        "echo; for j in 1 2; do glab api -X PUT projects/p/merge_requests/$j; done",
        // IFS anywhere.
        "J=101; glab api -X PUT projects/p/merge_requests/$J -f x=IFS",
        "IFS=; for j in 1 2; do glab api -X PUT projects/p/merge_requests/$j; done",
        // A bash variable, or a keyword, as the name.
        "PWD=1; glab api -X PUT projects/p/merge_requests/$PWD",
        "RANDOM=1; glab api -X PUT projects/p/merge_requests/$RANDOM",
        "PATH=1; glab api -X PUT projects/p/merge_requests/$PATH",
        "for do in 1; do glab api -X PUT projects/p/merge_requests/$do; done",
        // A quoted name or value, or a name of mixed case.
        "\"J\"=101; glab api -X PUT projects/p/merge_requests/$J",
        "J=\"101\"; glab api -X PUT projects/p/merge_requests/$J",
        "Jb=101; glab api -X PUT projects/p/merge_requests/$Jb",
        "_J=101; glab api -X PUT projects/p/merge_requests/$_J",
        // Two api calls, or another variable, in the call.
        "for j in 1 2; do glab api -X POST projects/p/jobs/$j/play; glab api -X PUT projects/p/merge_requests/$j; done",
        "J=101; glab api -X PUT projects/p/merge_requests/$J -f t=$K",
        "J=101; glab api -X PUT projects/p/merge_requests/$J gh api x",
        // The reference is not a whole segment.
        "J=3; gh api -X PUT repos/o/r/pulls/$J/merge",
        "J=3; gh api -X PUT repos/o/r/pulls/5$J",
        "J=3; gh api -X PUT repos/o/r/pulls$J",
        "J=3; gh api -X PUT repos/o/r/pulls/$J?x=1",
        // A word of the call is the name.
        "J=3; gh api -X PUT repos/o/J/pulls/$J",
    ]);
}

// `$(`, a backtick and `${NAME<op>}` stay refused whatever else the line is.
#[test]
fn a_substitution_or_an_operator_in_the_reference_is_held() {
    purlis_core::unsteered!();
    held(&[
        "J=1; glab api -X PUT projects/p/merge_requests/$(echo 3/merge)",
        "J=1; glab api -X PUT projects/p/merge_requests/`echo 3/merge`",
        "J=1; glab api -X PUT projects/p/merge_requests/${J:-3/merge}",
        "J=1; glab api -X PUT projects/p/merge_requests/${J/1/3\\/merge}",
        "J=1; glab api -X PUT projects/p/merge_requests/${J%%x}",
        "for j in 1; do glab api -X PUT projects/p/merge_requests/${j:-3/merge}; done",
        "for j in 1; do glab api -X PUT projects/p/merge_requests/$(echo $j); done",
    ]);
}

// Shape 5, as the ruling of Q331 reads it: a variable in a writing path is held unless the line
// is one of the two exact shapes. The loop over `$(...)` is the accepted false hold.
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
    assert!(refusal.contains("J=101; glab api"), "{refusal}");
}

// Shape 6.
#[test]
fn editing_a_merge_requests_metadata_passes() {
    purlis_core::unsteered!();
    let dir = tempfile::tempdir().unwrap();
    let body = dir.path().join("body.json");
    std::fs::write(&body, r#"{"title":"New title","description":"Text"}"#).unwrap();
    let body = body.display();
    passes(&[
        &format!("glab api --method PUT projects/p/merge_requests/7/ --input {body}"),
        &format!("glab api --method PUT projects/p/merge_requests/7 --input {body}"),
        "glab api -X PUT projects/p/merge_requests/7 -f title='New title' -f description='Text'",
        "gh api -X PATCH repos/o/r/pulls/7 -f title='New title'",
    ]);
}

/// A request's edit passes when it sets only text (and a pull request's state), and is held when
/// it can retarget or merge it, or when what it sets cannot be read here (#7195).
#[test]
fn editing_a_pull_request_passes_unless_it_retargets_or_cannot_be_read() {
    purlis_core::unsteered!();
    let dir = tempfile::tempdir().unwrap();
    let write = |name: &str, text: &str| {
        let path = dir.path().join(name);
        std::fs::write(&path, text).unwrap();
        path.display().to_string()
    };
    let text = write(
        "text.json",
        r#"{"title":"T","body":"B","state":"closed","maintainer_can_modify":true}"#,
    );
    let retarget = write("retarget.json", r#"{"title":"T","base":"main"}"#);
    let auto = write("auto.json", r#"{"merge_when_pipeline_succeeds":true}"#);
    let array = write("array.json", r#"[{"title":"T"}]"#);
    let broken = write("broken.json", r#"{"title":"#);
    let missing = dir.path().join("missing.json").display().to_string();
    passes(&[
        &format!("gh api -X PATCH repos/o/r/pulls/7 --input {text}"),
        &format!("gh api --method=PATCH repos/o/r/pulls/7 --input {text}"),
        "gh api -X PATCH repos/o/r/pulls/7 -f title=T -f body=B -f state=closed",
        "gh api -X PATCH repos/o/r/pulls/7 -F maintainer_can_modify=true -f body=@notes.md",
    ]);
    held(&[
        &format!("gh api -X PATCH repos/o/r/pulls/7 --input {retarget}"),
        &format!("gh api -X PATCH repos/o/r/pulls/7 --input {auto}"),
        &format!("gh api -X PATCH repos/o/r/pulls/7 --input {array}"),
        &format!("gh api -X PATCH repos/o/r/pulls/7 --input {broken}"),
        &format!("gh api -X PATCH repos/o/r/pulls/7 --input {missing}"),
        &format!("gh api -X PATCH repos/o/r/pulls/7 --input {text} --input {retarget}"),
        &format!("gh api -X PATCH repos/o/r/pulls/7 --input {text} -f base=main"),
        &format!(
            "gh api -X PATCH repos/o/r/pulls/7 --input {dir_path}",
            dir_path = dir.path().display()
        ),
        "gh api -X PATCH repos/o/r/pulls/7 --input -",
        "gh api -X PATCH repos/o/r/pulls/7 --input $BODY",
        "gh api -X PATCH repos/o/r/pulls/7 --input ~/body.json",
        "gh api -X PATCH repos/o/r/pulls/7 --input body-$$.json",
        "gh api -X PATCH repos/o/r/pulls/7 --input <(echo '{\"title\":\"T\"}')",
        "gh api -X PATCH repos/o/r/pulls/7 -f base=main",
        "gh api -X PATCH repos/o/r/pulls/7 -F base=main",
        "gh api -X PATCH repos/o/r/pulls/7 -f title=T -f base=release",
        "gh api -X PATCH repos/o/r/pulls/7 -f 'base[x]=main'",
        "gh api -X PATCH repos/o/r/pulls/7 -f $K=main",
        "gh api -X PATCH repos/o/r/pulls/7 -f auto_merge=true",
        "gh api -X PATCH repos/o/r/pulls/7 --form title=T",
        "gh api -X PATCH repos/o/r/pulls/7/merge -f title=T",
        "gh api -X PUT repos/o/r/pulls/7/merge --input body.json",
    ]);
}

/// The same for GitLab's `PUT merge_requests/<iid>`: its metadata (text, labels, assignees,
/// reviewers, milestone, lock, collaboration, and a `state_event` of `close` or `reopen`) is
/// never held; a target, a merge setting or anything unknown is (#7176, #7195).
#[test]
fn editing_a_merge_requests_metadata_passes_and_its_target_and_merge_settings_are_held() {
    purlis_core::unsteered!();
    let dir = tempfile::tempdir().unwrap();
    let write = |name: &str, text: &str| {
        let path = dir.path().join(name);
        std::fs::write(&path, text).unwrap();
        path.display().to_string()
    };
    let text = write("text.json", r#"{"title":"T","description":"D"}"#);
    let target = write("target.json", r#"{"title":"T","target_branch":"main"}"#);
    let auto = write("auto.json", r#"{"merge_when_pipeline_succeeds":true}"#);
    let squash = write("squash.json", r#"{"squash":true}"#);
    let metadata = write(
        "metadata.json",
        r#"{"labels":"a,b","add_labels":"c","remove_labels":"d","assignee_id":1,"assignee_ids":[1,2],"reviewer_ids":[3],"milestone_id":4,"discussion_locked":true,"allow_collaboration":false,"state_event":"reopen"}"#,
    );
    let unknown = write("unknown.json", r#"{"title":"T","something_new":1}"#);
    let state_merge = write("state-merge.json", r#"{"state_event":"merge"}"#);
    let state_number = write("state-number.json", r#"{"state_event":1}"#);
    let missing = dir.path().join("missing.json").display().to_string();
    passes(&[
        &format!("glab api -X PUT projects/9/merge_requests/3 --input {text}"),
        &format!("glab api -X PUT projects/9/merge_requests/3 --input {metadata}"),
        "glab api -X PUT projects/9/merge_requests/3 -f title=T -f description=D",
        "glab api -X PUT projects/9/merge_requests/3 -f labels=a,b -f add_labels=c -f remove_labels=d",
        "glab api -X PUT projects/9/merge_requests/3 -F assignee_id=1 -F milestone_id=4 -F discussion_locked=true",
        "glab api -X PUT projects/9/merge_requests/3 -f 'assignee_ids[]=1' -f 'reviewer_ids[]=2'",
        "glab api -X PUT projects/9/merge_requests/3 -F allow_collaboration=true -f state_event=close",
        "glab api -X PUT projects/9/merge_requests/3 -f state_event=reopen",
        "glab api --method PUT projects/g%2Fp/merge_requests/3 -F description=@notes.md",
    ]);
    held(&[
        &format!("glab api -X PUT projects/9/merge_requests/3 --input {target}"),
        &format!("glab api -X PUT projects/9/merge_requests/3 --input {auto}"),
        &format!("glab api -X PUT projects/9/merge_requests/3 --input {squash}"),
        &format!("glab api -X PUT projects/9/merge_requests/3 --input {unknown}"),
        &format!("glab api -X PUT projects/9/merge_requests/3 --input {state_merge}"),
        &format!("glab api -X PUT projects/9/merge_requests/3 --input {state_number}"),
        &format!("glab api -X PUT projects/9/merge_requests/3 --input {missing}"),
        "glab api -X PUT projects/9/merge_requests/3 --input -",
        "glab api -X PUT projects/9/merge_requests/3 --input $BODY",
        "glab api -X PUT projects/9/merge_requests/3 -f state_event=$EVENT",
        "glab api -X PUT projects/9/merge_requests/3 -f state_event=merge",
        "glab api -X PUT projects/9/merge_requests/3 -F state_event=@event.txt",
        "glab api -X PUT projects/9/merge_requests/3 -f target_branch=main",
        "glab api -X PUT projects/9/merge_requests/3 -f merge_when_pipeline_succeeds=true",
        "glab api -X PUT projects/9/merge_requests/3 -f auto_merge=true",
        "glab api -X PUT projects/9/merge_requests/3 -F squash=true",
        "glab api -X PUT projects/9/merge_requests/3 -f title=T -f remove_source_branch=true",
        "glab api -X PUT projects/9/merge_requests/3 -f title=T -f should_remove_source_branch=true",
        "glab api -X PUT projects/9/merge_requests/3 -f something_new=1",
        "glab api -X PUT projects/9/merge_requests/3/merge",
    ]);
}

/// A relative `--input` is the file gh opens in the shell's directory, which is the payload's
/// `cwd` unless the line changes it first; where anything in the line can, it is held, because
/// the file it names is not one purlis can read (#7195).
#[test]
fn a_relative_input_is_read_in_the_payloads_directory_and_held_where_the_line_may_change_it() {
    purlis_core::unsteered!();
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("body.json"), r#"{"title":"T"}"#).unwrap();
    let elsewhere = tempfile::tempdir().unwrap();
    std::fs::write(elsewhere.path().join("body.json"), r#"{"base":"main"}"#).unwrap();
    let cwd = dir.path().to_str().unwrap();
    let read = |cmd: &str| purlis_core::floorguard::in_directory(cwd, || mergehold::held(cmd));
    // The file in the payload's directory is read: it sets only a title.
    for cmd in [
        "gh api -X PATCH repos/o/r/pulls/7 --input body.json",
        "gh api -X PATCH repos/o/r/pulls/7 --input ./body.json",
        "glab api -X PUT projects/9/merge_requests/3 --input body.json",
    ] {
        assert_eq!(read(cmd), None, "held: {cmd}");
    }
    // A line that can change directory before the call may read another file than the one
    // beside the payload, so nothing is read.
    let there = elsewhere.path().display();
    for cmd in [
        format!("cd {there} && gh api -X PATCH repos/o/r/pulls/7 --input body.json"),
        format!("cd {there}; gh api -X PATCH repos/o/r/pulls/7 --input body.json"),
        format!("(cd {there} && gh api -X PATCH repos/o/r/pulls/7 --input body.json)"),
        format!("pushd {there} && gh api -X PATCH repos/o/r/pulls/7 --input body.json"),
        "popd && gh api -X PATCH repos/o/r/pulls/7 --input body.json".to_owned(),
        format!("env -C {there} gh api -X PATCH repos/o/r/pulls/7 --input body.json"),
        format!("env --chdir={there} gh api -X PATCH repos/o/r/pulls/7 --input body.json"),
        format!("bash -c 'cd {there} && gh api -X PATCH repos/o/r/pulls/7 --input body.json'"),
        format!("cd {there} && bash -c 'gh api -X PATCH repos/o/r/pulls/7 --input body.json'"),
        format!("\\cd {there} && gh api -X PATCH repos/o/r/pulls/7 --input body.json"),
        format!("c'd' {there}; gh api -X PATCH repos/o/r/pulls/7 --input body.json"),
        format!("\"c\"d {there}; gh api -X PATCH repos/o/r/pulls/7 --input body.json"),
        format!("cd>/dev/null {there}; gh api -X PATCH repos/o/r/pulls/7 --input body.json"),
        "source ./env.sh; gh api -X PATCH repos/o/r/pulls/7 --input body.json".to_owned(),
        "eval \"$GO\"; gh api -X PATCH repos/o/r/pulls/7 --input body.json".to_owned(),
        "git -C /elsewhere status && gh api -X PATCH repos/o/r/pulls/7 --input body.json"
            .to_owned(),
    ] {
        assert!(read(&cmd).is_some(), "not held: {cmd}");
    }
    // With no directory to read it against, a relative path is unread.
    assert!(
        mergehold::held("gh api -X PATCH repos/o/r/pulls/7 --input body.json").is_some(),
        "a relative input with no directory"
    );
    // No body is read from a line that is anything but the call, an absolute path's included.
    let absolute = dir.path().join("body.json");
    let sole = format!(
        "gh api -X PATCH repos/o/r/pulls/7 --input {}",
        absolute.display()
    );
    assert_eq!(read(&sole), None, "held: {sole}");
    let cmd = format!("cd {there} && {sole}");
    assert!(read(&cmd).is_some(), "not held: {cmd}");
}

/// `--input -` with a quoted heredoc on the same line is the body, read for the keys it sets;
/// an unquoted heredoc, a pipe, a substitution or a here-string is stdin purlis does not read
/// (#7195).
#[test]
fn a_quoted_heredoc_on_stdin_is_the_body_and_any_other_stdin_is_held() {
    purlis_core::unsteered!();
    passes(&[
        "gh api -X PATCH repos/o/r/pulls/7 --input - <<'EOF'\n{\"title\":\"T\",\"body\":\"B\"}\nEOF",
        "gh api -X PATCH repos/o/r/pulls/7 --input - <<\"EOF\"\n{\"title\":\"it's $X\"}\nEOF",
        "gh api -X PATCH repos/o/r/pulls/7 --input=- <<'EOF'\n{\"state\":\"closed\"}\nEOF",
        "gh api -X PATCH repos/o/r/pulls/7 --input - <<-'EOF'\n\t{\"title\":\"T\"}\n\tEOF",
        "glab api -X PUT projects/9/merge_requests/3 --input - <<'EOF'\n{\"labels\":\"a\",\"state_event\":\"close\"}\nEOF",
    ]);
    held(&[
        // The body sets what an edit may not.
        "gh api -X PATCH repos/o/r/pulls/7 --input - <<'EOF'\n{\"base\":\"main\"}\nEOF",
        "gh api -X PATCH repos/o/r/pulls/7 --input - <<'EOF'\n{\"title\":\"T\",\"base\":\"main\"}\nEOF",
        "glab api -X PUT projects/9/merge_requests/3 --input - <<'EOF'\n{\"target_branch\":\"main\"}\nEOF",
        // Not one JSON object, or no end to the body.
        "gh api -X PATCH repos/o/r/pulls/7 --input - <<'EOF'\n[{\"title\":\"T\"}]\nEOF",
        "gh api -X PATCH repos/o/r/pulls/7 --input - <<'EOF'\n{\"title\":\nEOF",
        "gh api -X PATCH repos/o/r/pulls/7 --input - <<'EOF'\n{\"title\":\"T\"}",
        // Stdin the shell builds or another program writes.
        "gh api -X PATCH repos/o/r/pulls/7 --input - <<EOF\n{\"title\":\"T\"}\nEOF",
        "gh api -X PATCH repos/o/r/pulls/7 --input - <<EOF\n{\"title\":\"$T\"}\nEOF",
        "echo '{\"title\":\"T\"}' | gh api -X PATCH repos/o/r/pulls/7 --input -",
        "gh api -X PATCH repos/o/r/pulls/7 --input - <<< '{\"title\":\"T\"}'",
        "gh api -X PATCH repos/o/r/pulls/7 --input - < body.json",
        "gh api -X PATCH repos/o/r/pulls/7 --input - <<'EOF' < body.json\n{\"title\":\"T\"}\nEOF",
        "gh api -X PATCH repos/o/r/pulls/7 --input - < <(echo '{\"title\":\"T\"}')",
        "gh api -X PATCH repos/o/r/pulls/7 --input - <<'EOF'\n{\"title\":\"T\"}\nEOF\ngh api -X PATCH repos/o/r/pulls/8 --input - <<'EOF'\n{\"base\":\"main\"}\nEOF",
    ]);
}

/// A body is read only where the call is the line's sole command, so a heredoc beside anything
/// else, on another descriptor, or whose header runs on is held; and a body that names a key twice
/// or a GitHub `state` that is not `open` or `closed` is held (#7195 round 8).
#[test]
fn a_body_is_read_only_where_the_call_is_the_lines_sole_command() {
    purlis_core::unsteered!();
    passes(&[
        "gh api -X PATCH repos/o/r/pulls/7 --input - <<'EOF'\n{\"title\":\"T (x) & y\",\"body\":\"B\"}\nEOF",
        "GH_TOKEN=x gh api -X PATCH repos/o/r/pulls/7 --input - <<'EOF'\n{\"state\":\"open\"}\nEOF",
        "glab api -X PUT projects/9/merge_requests/5 -f labels=x",
        "gh api -X PATCH repos/o/r/pulls/7 -f state=closed",
    ]);
    held(&[
        // A backtick in the first of two heredocs.
        "cat <<'A'\n`x`\nA\ngh api -X PATCH repos/o/r/pulls/7 --input - <<'EOF'\n{\"base\":\"main\"}\nEOF",
        // Another command feeds, or another descriptor takes, the heredoc.
        "echo '{\"base\":\"main\"}' | gh api -X PATCH repos/o/r/pulls/7 --input - 3<<'EOF'\n{\"title\":\"T\"}\nEOF",
        "gh api -X PATCH repos/o/r/pulls/7 --input - 3<<'EOF'\n{\"title\":\"T\"}\nEOF",
        // A quoted argument that runs on past the header, holding a line that reads as the end.
        "gh api -X PATCH repos/o/r/pulls/7 --input - -f title='a <<'EOF'\n{\"title\":\"T\"}\nEOF\nb' <<'EOF'\n{\"base\":\"main\"}\nEOF",
        "gh api -X PATCH repos/o/r/pulls/7 --input - <<'EOF' -f title='x\nEOF\ny'\n{\"title\":\"T\"}\nEOF",
        // A key named twice, whichever a reader keeps.
        "glab api -X PUT projects/9/merge_requests/5 --input - <<'EOF'\n{\"state_event\":\"merge\",\"state_event\":\"close\"}\nEOF",
        "gh api -X PATCH repos/o/r/pulls/7 --input - <<'EOF'\n{\"title\":\"a\",\"title\":\"b\"}\nEOF",
        // A state that is neither `open` nor `closed`.
        "gh api -X PATCH repos/o/r/pulls/7 -f state=merged",
        "gh api -X PATCH repos/o/r/pulls/7 --input - <<'EOF'\n{\"state\":\"merged\"}\nEOF",
        "gh api -X PATCH repos/o/r/pulls/7 --input - <<'EOF'\n{\"state\":3}\nEOF",
        // Anything around the call.
        "true && gh api -X PATCH repos/o/r/pulls/7 --input - <<'EOF'\n{\"title\":\"T\"}\nEOF",
        "(gh api -X PATCH repos/o/r/pulls/7 --input - <<'EOF'\n{\"title\":\"T\"}\nEOF\n)",
        "gh api -X PATCH repos/o/r/pulls/7 --input - <<'EOF' &\n{\"title\":\"T\"}\nEOF",
        "eval gh api -X PATCH repos/o/r/pulls/7 --input - <<'EOF'\n{\"title\":\"T\"}\nEOF",
    ]);
}

/// A body over a mebibyte is held for its size, even where it is one object that sets only a
/// title (#7195).
#[test]
fn a_body_over_a_mebibyte_is_held_for_its_size() {
    purlis_core::unsteered!();
    let dir = tempfile::tempdir().unwrap();
    let pad = |bytes: usize| format!(r#"{{"title":"T","body":"{}"}}"#, "x".repeat(bytes));
    let fits = dir.path().join("fits.json");
    let over = dir.path().join("over.json");
    // The limit is on the file: exactly a mebibyte passes, one byte more is held.
    let overhead = pad(0).len();
    std::fs::write(&fits, pad((1 << 20) - overhead)).unwrap();
    std::fs::write(&over, pad((1 << 20) - overhead + 1)).unwrap();
    assert_eq!(std::fs::metadata(&fits).unwrap().len(), 1 << 20);
    assert_eq!(std::fs::metadata(&over).unwrap().len(), (1 << 20) + 1);
    passes(&[&format!(
        "gh api -X PATCH repos/o/r/pulls/7 --input {}",
        fits.display()
    )]);
    held(&[&format!(
        "gh api -X PATCH repos/o/r/pulls/7 --input {}",
        over.display()
    )]);
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

// #7195 round 4: the shell blanks only space, tab and newline, so a line that opens with any
// other blank is a command word `\vJ=1`, not an assignment, and `$J` is empty.
#[test]
fn a_line_that_only_looks_like_a_digit_binding_is_held() {
    purlis_core::unsteered!();
    held(&[
        "\u{b}J=1; gh api -X PATCH repos/o/r/git/refs/heads/${J}\"main\" -f sha=abc -F force=true",
        "\u{b}M=1; glab api -X PUT projects/p/merge_requests/5/$M/merge",
        "\u{b}M=1; gh api -X PATCH repos/o/r/git/refs/$M/heads/main -f sha=abc",
        "\u{c}M=1; glab api -X PUT projects/p/merge_requests/5/$M/merge",
        "\u{a0}M=1; glab api -X PUT projects/p/merge_requests/5/$M/merge",
        "\u{2028}M=1; glab api -X PUT projects/p/merge_requests/5/$M/merge",
        "\u{85}M=1; glab api -X PUT projects/p/merge_requests/5/$M/merge",
        "\u{3000}M=1; glab api -X PUT projects/p/merge_requests/5/$M/merge",
        "M=1; glab api -X PUT projects/p/merge_requests/5/$M/merge\u{b}",
        "M=1; glab api -X PUT projects/p/merge_requests/5/$M/merge\u{c}",
        "M=1; glab api -X PUT projects/p/merge_requests/5/$M/merge\u{a0}",
        "M=1; glab api -X PUT projects/p/merge_requests/5/$M/merge\u{2028}",
    ]);
}

// A reference stands for a segment of its own: no quote after it, and the path is judged with
// the variable empty as well as a number.
#[test]
fn a_reference_that_can_vanish_or_join_a_quote_is_held() {
    purlis_core::unsteered!();
    held(&[
        "J=1; gh api -X PATCH repos/o/r/git/refs/heads/${J}\"main\" -f sha=abc -F force=true",
        "J=1; gh api -X PATCH repos/o/r/git/refs/heads/$J\"main\" -f sha=abc",
        "J=1; gh api -X PUT repos/o/r/pulls/5/${J}\"%2Fmerge\"",
        "M=1; glab api -X PUT projects/p/$M/merge_requests/5/merge",
        "M=1; gh api -X PUT repos/o/r/pulls/5/$M/merge",
        "M=1; gh api -X POST repos/o/r/$M/merges -f base=main -f head=x",
        "M=1; glab api -X POST projects/p/repository/$M/commits -f branch=main",
        "M=1; glab api -X DELETE projects/p/repository/$M/branches/main",
        "M=1; gh api -X PATCH repos/o/r/git/refs/$M/heads/main -f sha=abc",
        "for m in 1 2; do glab api -X PUT projects/p/$m/merge_requests/5/merge; done",
    ]);
}

// `ApiCall` reads a braced word as one positional, but the shell makes several words of it.
#[test]
fn a_brace_expansion_that_builds_the_method_is_held() {
    purlis_core::unsteered!();
    held(&[
        "gh api {-XPUT,repos/o/r/pulls/5/merge}",
        "gh api {--method=PUT,repos/o/r/pulls/5/merge}",
        "gh api {-XPATCH,repos/o/r/git/refs/heads/main,-fsha=abc}",
        "glab api {-XPUT,projects/p/merge_requests/5/merge}",
        "gh api -X{PUT,GET} repos/o/r/pulls/5/merge",
    ]);
}

#[test]
fn glab_api_has_no_dash_r_so_the_word_after_it_is_a_path() {
    purlis_core::unsteered!();
    held(&["glab api -X PUT -R projects/p/merge_requests/5/merge"]);
    passes(&["gh api -R o/r repos/o/r/pulls/5"]);
}

#[test]
fn the_two_legitimate_shapes_still_pass() {
    purlis_core::unsteered!();
    passes(&[
        "J=101; glab api -X POST projects/p/jobs/$J/play",
        "J=101\nglab api -X POST projects/p/jobs/$J/play\n",
        " J=101;\tglab api -X POST \"projects/p/jobs/${J}/play\" ",
        "for j in 101 102; do glab api -X POST projects/p/jobs/$j/play; done",
    ]);
}

/// Round 9 (#7195): a bash 5.3 function substitution (`${ cmd; }`, `${| cmd; }`) runs in the
/// current shell with its `;` hidden by a quote, so a line holding one is no sole call and its
/// body is unread; a parameter's `${NAME}` is not one.
#[test]
fn a_quoted_funsub_is_no_sole_call_and_a_parameter_expansion_is_() {
    purlis_core::unsteered!();
    let dir = tempfile::tempdir().unwrap();
    let body = dir.path().join("body.json");
    std::fs::write(&body, r#"{"title":"T"}"#).unwrap();
    let b = body.display();
    held(&[
        &format!(
            "gh api -X PATCH repos/o/r/pulls/7 --input {b} -f title=\"${{ cd /elsewhere; }}\""
        ),
        &format!("X=\"${{ cd /elsewhere; }}\" gh api -X PATCH repos/o/r/pulls/7 --input {b}"),
        &format!(
            "gh api -X PATCH repos/o/r/pulls/7 --input {b} -f title=\"${{| cd /elsewhere; }}\""
        ),
        &format!("gh api -X PATCH repos/o/r/pulls/7 --input {b} -f title=${{ cd /elsewhere; }}"),
    ]);
    passes(&[
        &format!("gh api -X PATCH repos/o/r/pulls/7 --input {b} -f title=\"${{TITLE}}\""),
        &format!("gh api -X PATCH repos/o/r/pulls/7 --input {b} -f title=\"${{#}}${{1}}${{?}}\""),
    ]);
}

/// Round 9 (#7195): an output redirection is no other command, so a body is still read beside
/// one; a trailing `&` and an input redirection other than the quoted heredoc still hold.
#[test]
fn an_output_redirection_leaves_the_call_a_sole_call() {
    purlis_core::unsteered!();
    let dir = tempfile::tempdir().unwrap();
    let body = dir.path().join("body.json");
    std::fs::write(&body, r#"{"title":"T"}"#).unwrap();
    let bad = dir.path().join("bad.json");
    std::fs::write(&bad, r#"{"base":"main"}"#).unwrap();
    let (b, x) = (body.display(), bad.display());
    for redirect in [
        ">out.log",
        ">>out.log",
        "2>out.log",
        "2>&1",
        "1>&2",
        "&>out.log",
        "&>>out.log",
    ] {
        passes(&[&format!(
            "gh api -X PATCH repos/o/r/pulls/7 --input {b} {redirect}"
        )]);
        // The heredoc reading takes `&>` for a background `&` and holds it, which fails closed.
        if !redirect.starts_with('&') {
            passes(&[&format!(
                "gh api -X PATCH repos/o/r/pulls/7 --input - {redirect} <<'EOF'\n{{\"title\":\"T\"}}\nEOF"
            )]);
        }
        held(&[
            &format!("gh api -X PATCH repos/o/r/pulls/7 --input {x} {redirect}"),
            &format!(
                "gh api -X PATCH repos/o/r/pulls/7 --input - {redirect} <<'EOF'\n{{\"base\":\"main\"}}\nEOF"
            ),
        ]);
    }
    held(&[
        &format!("gh api -X PATCH repos/o/r/pulls/7 --input {x} 2>&1 &"),
        &format!("gh api -X PATCH repos/o/r/pulls/7 --input {x} >out.log &"),
        "gh api -X PATCH repos/o/r/pulls/7 --input - < body.json 2>&1",
        "gh api -X PATCH repos/o/r/pulls/7 --input - <<< '{\"title\":\"T\"}' 2>&1",
    ]);
}

/// Round 9 (#7195): a backslash and a newline join as bash joins them, and the call is read when
/// the joined line is a sole call; a continuation into anything else, or into a heredoc's body,
/// is held. `{fd}<<'EOF'` is not stdin.
#[test]
fn a_continuation_joins_as_bash_joins_it_and_a_named_descriptor_is_not_stdin() {
    purlis_core::unsteered!();
    let dir = tempfile::tempdir().unwrap();
    let body = dir.path().join("body.json");
    std::fs::write(&body, r#"{"title":"T"}"#).unwrap();
    let bad = dir.path().join("bad.json");
    std::fs::write(&bad, r#"{"base":"main"}"#).unwrap();
    let (b, x) = (body.display(), bad.display());
    passes(&[
        &format!("gh api -X PATCH repos/o/r/pulls/7 \\\n  --input {b}"),
        &format!("gh api -X PATCH repos/o/r/pulls/7 \\\n  --input {b} \\\n  2>&1"),
    ]);
    held(&[
        &format!("gh api -X PATCH repos/o/r/pulls/7 \\\n  --input {x}"),
        &format!("gh api -X PATCH repos/o/r/pulls/7 \\\n  --input {b}; cd /elsewhere"),
        &format!("gh api -X PATCH repos/o/r/pulls/7 --input {b}\ncd /elsewhere"),
        &format!("gh api -X PATCH repos/o/r/pulls/7 \\\n  --input {b} \\\n; cd /elsewhere"),
        &format!("cd /elsewhere \\\n; gh api -X PATCH repos/o/r/pulls/7 --input {b}"),
        "gh api -X PATCH repos/o/r/pulls/7 --input - {fd}<<'EOF'\n{\"title\":\"T\"}\nEOF",
        "gh api -X PATCH repos/o/r/pulls/7 --input - {fd}<<'EOF'\n{\"base\":\"main\"}\nEOF",
    ]);
}

/// Round 9 (#7195): the refusal of a body it cannot read says what to do.
#[test]
fn the_refusal_of_an_unread_body_says_to_run_the_call_alone() {
    purlis_core::unsteered!();
    let cmd = "true && gh api -X PATCH repos/o/r/pulls/7 --input body.json";
    let said = release_floor_reason(cmd, true).expect("refused");
    assert!(said.contains("alone on its line"), "{said}");
    assert!(said.contains("quoted heredoc"), "{said}");
    assert!(said.contains("absolute path"), "{said}");
}

/// What the shell fills in of a `gh api` / `glab api` call is held (#7416); these fill in nothing
/// that adds a word or begins a flag, and pass.
#[test]
fn a_shell_filled_api_call_that_is_one_word_and_no_flag_passes() {
    purlis_core::unsteered!();
    passes(&[
        "gh api repos/o/r/pulls/5",
        "gh api \"repos/o/r/pulls/$N\" --jq .title",
        "glab api \"projects/:id/pipelines/$(glab api projects/:id/pipelines | jq -r '.[0].id')/jobs\"",
        "for id in 1 2; do gh api repos/o/r/pulls/$id; done",
    ]);
}
