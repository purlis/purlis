// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! #3489: **a closed repo row's count is read once, and again only when the repo moves.** The
//! explorer's closed repo rows each say how many files their repo changed, which was a full
//! `branch_status` per repo on every refresh. The count is kept per clone in the store the save
//! standings share (`purlis_core::standings::Shared`, FD-11): read again when the repo's git files
//! move or the one watch per repo touches it, and otherwise answered from what was kept.

use std::cell::Cell;

use crate::support;

use purlis_core::files::{self, Branch, Mark};

fn reader() -> files::Reader {
    files::Reader::new(
        std::env::current_exe().expect("the test binary"),
        [
            "a_closed_repo_reads_what_it_changed_once_until_it_moves::reader_child",
            "--exact",
            "--nocapture",
            "--test-threads=1",
            files::READ_ARG,
        ]
        .map(std::ffi::OsString::from),
    )
}

/// The reader's child: in a run of this binary that [`reader`] started, it answers the one
/// question it was asked and exits; in any other run it does nothing.
#[test]
fn reader_child() {
    purlis_core::unsteered!();
    if let Some(code) = files::serve_if_asked() {
        std::process::exit(code);
    }
}

#[test]
fn a_repos_count_is_read_once_and_again_only_when_its_git_files_move_or_its_watch_touches_it() {
    purlis_core::unsteered!();
    let f = support::plane_with_clone("thing");
    let reader = reader();
    let reads = Cell::new(0);
    let count = || {
        files::repo_changes(&f.plane, &f.ws, &f.repo, |branch: Branch<'_>| {
            reads.set(reads.get() + 1);
            files::status(&reader, &f.plane, branch)
        })
        .map(|rolled| rolled.map(|one| (one.mark, one.count)))
    };
    std::fs::write(f.clone.join("new.md"), "new\n").unwrap();

    assert_eq!(count(), Ok(Some((Mark::Added, 1))));
    assert_eq!(count(), Ok(Some((Mark::Added, 1))));
    assert_eq!(
        reads.get(),
        1,
        "a second ask with nothing moved reads nothing"
    );

    // A write in the working tree: the watch touches the clone, and the count is read again.
    std::fs::write(f.clone.join("other.md"), "other\n").unwrap();
    purlis_core::planegit::touch(&f.clone);
    assert_eq!(count(), Ok(Some((Mark::Added, 2))));
    assert_eq!(reads.get(), 2);

    // A commit moves git's own files, which the kept answer is stamped with: read again, with no
    // touch, and a clean tree counts nothing.
    support::git(&f.clone, &["add", "-A"]);
    support::git(&f.clone, &["commit", "-q", "-m", "both"]);
    assert_eq!(count(), Ok(None));
    assert_eq!(reads.get(), 3);
    assert_eq!(count(), Ok(None));
    assert_eq!(reads.get(), 3);
}

#[test]
fn a_repo_the_plane_does_not_hold_is_refused_in_a_sentence_and_read_by_nothing() {
    purlis_core::unsteered!();
    let f = support::plane_with_clone("thing");
    let reads = Cell::new(0);
    let said = files::repo_changes(&f.plane, &f.ws, "../outside", |_| {
        reads.set(reads.get() + 1);
        Err(files::Refused::Read("never".into()))
    });
    assert!(said.is_err(), "{said:?}");
    assert_eq!(reads.get(), 0);
}
