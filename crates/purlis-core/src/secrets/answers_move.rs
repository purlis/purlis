// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A plane the app moves keeps its answers** (M75 spec §8.10; ticket M75-07a, #7015, part 2).
//!
//! A plane's store is sealed to its clone key and anchored by its path, and a move changes both,
//! so a store left where it was reads as the deny state at the new place ([`Deny::PlaneMoved`]).
//! When **the app itself** moves a plane ([`crate::localproject`]) it calls [`follow_move`]: the
//! old place's store is read and verified against its own anchor, every entry is sealed again for
//! the new place, the counter rises, and the registry follows the plane's id to its new path.
//!
//! # It fails closed
//!
//! - Only the app seals, so any other process leaves the old store where it is
//!   ([`Moved::NotKept`]) and the new place reads as moved: the person's press mends it.
//! - **An old store that is not whole is never carried**: any deny cause at the old place refuses.
//! - **Nothing is carried over something the new place already keeps**, and nothing is carried
//!   while the old place still holds a project (that is a copy, not a move).
//!
//! # What this does not catch
//!
//! A plane moved **without** the app, across filesystems, with no link left: a copy has a new
//! `.git` identity, so the registry's check (which works on one filesystem) does not see it. That
//! is slice 11's (the sandbox holding the plane root's folder entry); doctor declares the gap.

use super::*;

/// **Whether `old` is still a project of its own**: it holds a `.git` that is not the new place
/// seen through the pointer the move leaves at the old path ([`crate::localproject`] calls
/// [`follow_move`] while that link is in place, so `old/.git` exists through it).
fn still_a_project(old: &Path, new: &Path) -> bool {
    old.join(".git").exists() && crate::sandbox::real(old) != crate::sandbox::real(new)
}

/// What [`follow_move`] did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Moved {
    /// Nothing was sealed for the old place (or it is still a project: a copy): nothing to carry.
    NothingKept,
    /// The answers follow the plane: sealed again for the new place.
    Carried,
    /// This process cannot seal (it is not the app, there is no keyring, or the seal key is
    /// locked): the answers stay at the old place, and the new place reads as moved until the
    /// person accepts again in the window.
    NotKept,
}

/// **The app moved a plane from `old` to `new`**: seals its answers again for the new place.
///
/// `Err` where they could not be carried (the old store is not whole, the new place keeps
/// something already, the registry does not read, a write failed): nothing at the old place was
/// changed, and the new place reads as the deny state.
pub fn follow_move(old: &Path, new: &Path) -> io::Result<Moved> {
    if old == new || still_a_project(old, new) {
        return Ok(Moved::NothingKept);
    }
    if seal::key_state() != KeyState::Usable {
        return Ok(Moved::NotKept);
    }
    let new_place = plane_place(new).ok_or_else(no_home)?;
    let _turn = store_lock()?;
    let registry = match read_registry() {
        Read::Value(registry) => registry,
        Read::Missing => return Ok(Moved::NothingKept),
        Read::Broken | Read::Unreadable => {
            return Err(io::Error::other(
                "purlis could not read its registry of planes, so the project's answers did not \
                 follow it: accept again in the window to mend them",
            ));
        }
    };
    let old_real = crate::sandbox::real(old);
    let new_anchor = anchor_key(new);
    // The plane the old place kept, a sealed one before any other: while the pointer is in place
    // `old_real` is the new place, and an unsealed placeholder registered there matches it too.
    let names_old = |p: &&Registered| p.real == old || p.opened == old || p.real == old_real;
    let Some(plane) = registry
        .planes
        .iter()
        .find(|p| names_old(p) && p.sealed)
        .or_else(|| registry.planes.iter().find(names_old))
        .or_else(|| registry.planes.iter().find(|p| p.anchor == new_anchor))
        .cloned()
    else {
        return Ok(Moved::NothingKept);
    };
    if plane.anchor == new_anchor {
        // Followed already, or cut short after the registry did: whole or the deny state.
        return if standing_with(new, false).is_deny() {
            Err(io::Error::other(
                "the project's answers were moving when purlis stopped: accept again in the \
                 window to mend them",
            ))
        } else {
            Ok(Moved::Carried)
        };
    }
    if !plane.sealed {
        return Ok(Moved::NothingKept);
    }
    if registry
        .planes
        .iter()
        .any(|other| other.id != plane.id && other.anchor == new_anchor && other.sealed)
        || store_exists(&new_place)
        || !matches!(
            read_anchor(&new_place.account, &new_place.anchor_bind),
            Read::Missing
        )
    {
        return Err(io::Error::other(
            "purlis already keeps answers for the place this project moved to, so it did not \
             carry the old ones over them: accept again in the window to choose",
        ));
    }
    let old_place = Place {
        bind: plane.clone.clone(),
        folder: answers_folder().ok_or_else(no_home)?.join(&plane.clone),
        account: plane_account(&plane.anchor),
        anchor_bind: plane.anchor.clone(),
    };
    let found = read_place(&old_place, Some(&plane));
    let Some(manifest) = found.manifest.clone().filter(|_| found.denies.is_empty()) else {
        return Err(io::Error::other(
            "the project's answers were not whole where it moved from, so purlis did not carry \
             them to the new place: accept again in the window to start them from what you give",
        ));
    };
    let draft = Draft {
        entries: found.entries,
        imported_nevers: manifest.imported_nevers,
        ..Draft::default()
    };
    write(
        &new_place,
        Some((new, &plane.id)),
        draft,
        found.anchor,
        manifest.counter,
    )?;
    // What the old place kept is not read again: its anchor stays, so a project made there later
    // is the deny state, never a fresh install.
    let _ = std::fs::remove_dir_all(&old_place.folder);
    Ok(Moved::Carried)
}
