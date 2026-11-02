// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use super::*;

#[test]
fn a_profile_within_the_limit_is_over_nothing() {
    let one = sized("work", Ok((42 * MIB, 900)), &Chosen::default(), true);
    assert_eq!(one.mib, Some(42));
    assert_eq!(one.files, Some(900));
    assert!(one.hits.is_empty(), "{:?}", one.hits);
    assert!(one.in_use);
}

#[test]
fn a_profile_over_the_size_says_what_to_raise_it_to() {
    let limits = Chosen::default().with(LimitId::ProfileSize, Some(128));
    let one = sized("work", Ok((212 * MIB, 900)), &limits, false);
    let [hit] = one.hits.as_slice() else {
        panic!("{:?}", one.hits);
    };
    assert_eq!(hit.limit, LimitId::ProfileSize);
    assert_eq!((hit.in_force, hit.needed), (128, 212));
    assert!(hit.suggested.is_some_and(|to| to >= 212), "{hit:?}");
}

#[test]
fn a_part_of_a_mib_counts_as_a_whole_one_as_a_send_does() {
    let limits = Chosen::default().with(LimitId::ProfileSize, Some(2));
    let one = sized("work", Ok((2 * MIB + 1, 1)), &limits, false);
    assert_eq!(one.mib, Some(3));
    assert_eq!(one.hits.len(), 1);
}

#[test]
fn too_many_files_is_its_own_limit() {
    let limits = Chosen::default().with(LimitId::ProfileFiles, Some(100));
    let one = sized("work", Ok((MIB, 101)), &limits, false);
    assert_eq!(one.hits[0].limit, LimitId::ProfileFiles);
}

#[test]
fn a_profile_that_couldnt_be_measured_says_why_and_is_over_nothing() {
    let one = sized("work", Err("no folder".into()), &Chosen::default(), false);
    assert_eq!(one.mib, None);
    assert_eq!(one.unread.as_deref(), Some("no folder"));
    assert!(one.hits.is_empty());
}

#[test]
fn a_project_with_nothing_open_uses_nothing_but_its_default() {
    let plane = tempfile::tempdir().unwrap();
    assert!(in_use(plane.path()).is_empty());
}
