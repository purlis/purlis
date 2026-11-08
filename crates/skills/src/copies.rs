// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **Whether two copies of a skill are the same** (#5737, spec #5714 decision Q6): every name in
//! one folder is in the other, of the same kind, and every file holds the same text, all the way
//! down. Read through held folders and never through a link, so a copy is identical only where
//! purlis read the whole of both: a link, a file it does not read, or more than it reads makes
//! the two differ, and both load.

use std::path::Path;

use crate::held::{Folder, Follow, Kind, Opened};
use crate::tree::{MOST_FILE_BYTES, MOST_FILES, MOST_SKILL_DEPTH};

/// **Whether the skill folders `mine` and `theirs` hold the same files**, `theirs` opened through
/// a link at its last step only where `follow` says (the person's own folder may be one an
/// installer left), `mine` never.
pub(crate) fn same(mine: &Path, (theirs, follow): (&Path, Follow)) -> bool {
    let (Some(mine), Some(theirs)) = (
        Folder::open(mine, Follow::NoLink),
        Folder::open(theirs, follow),
    ) else {
        return false;
    };
    let mut files = 0;
    same_below(&mine, &theirs, 0, &mut files)
}

/// [`same`] for two held folders `depth` below the skill's own, `files` read so far.
fn same_below(mine: &Folder, theirs: &Folder, depth: usize, files: &mut usize) -> bool {
    if depth > MOST_SKILL_DEPTH {
        return false;
    }
    let (Ok(names), Ok(other)) = (mine.names(), theirs.names()) else {
        return false;
    };
    if names != other {
        return false;
    }
    names
        .iter()
        .all(|name| match (mine.kind(name), theirs.kind(name)) {
            (Some(Kind::File), Some(Kind::File)) => {
                *files += 1;
                *files <= MOST_FILES
                    && matches!(
                        (mine.read(name, MOST_FILE_BYTES), theirs.read(name, MOST_FILE_BYTES)),
                        (Ok(a), Ok(b)) if a == b
                    )
            }
            (Some(Kind::Folder), Some(Kind::Folder)) => matches!(
                (mine.child(name, Follow::NoLink), theirs.child(name, Follow::NoLink)),
                (Opened::Folder(a), Opened::Folder(b)) if same_below(&a, &b, depth + 1, files)
            ),
            _ => false,
        })
}
