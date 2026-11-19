// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A missing fixture fails in CI and skips loudly on a laptop** (#7188, ruling 4).

use crate::support::real_infra::{Verdict, judge, skip_line};

#[test]
fn a_fixture_that_is_there_is_run_whether_or_not_this_is_ci() {
    purlis_core::unsteered!();
    assert_eq!(judge(true, "a rig", "up.sh", Some(7)), Verdict::Run(7));
    assert_eq!(judge(false, "a rig", "up.sh", Some(7)), Verdict::Run(7));
}

#[test]
fn a_missing_fixture_fails_where_a_job_sets_the_rig_up() {
    purlis_core::unsteered!();
    let Verdict::Fail(why) = judge::<()>(true, "the sshd", "tests/remote-docker/up.sh", None)
    else {
        panic!("a missing fixture in CI must fail");
    };
    assert!(why.contains("the sshd"), "{why}");
    assert!(why.contains("tests/remote-docker/up.sh"), "{why}");
    assert!(why.contains("PURLIS_PARITY"), "{why}");
}

#[test]
fn a_missing_fixture_on_a_laptop_skips_with_a_line_that_says_so() {
    purlis_core::unsteered!();
    let Verdict::Skip(line) = judge::<()>(false, "the sshd", "tests/remote-docker/up.sh", None)
    else {
        panic!("a missing fixture on a laptop must skip");
    };
    assert!(line.starts_with("SKIP real-infra"), "{line}");
    assert_eq!(line, skip_line("the sshd", "tests/remote-docker/up.sh"));
    assert!(line.contains("tests/remote-docker/up.sh"), "{line}");
}
