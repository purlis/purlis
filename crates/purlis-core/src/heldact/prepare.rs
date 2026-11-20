// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **Reading an ask** (ADR 0128 D8): what a request named becomes the exact act and what the
//! person is shown of it, all of it read from the forge and the clone by the app and none of it
//! from the chat.
//!
//! The one function, [`prepare`], is used twice: when the ask is made, and again at each press.
//! That is what makes "the ask shows what will happen" checkable: a press performs the act a read
//! gives **now**, and only while it is the act the person was shown.

use super::ci::{PushCheck, check_push};
use super::{Act, Against, Body, CiDiff, Prepared, Refused, Requested, Row, Shown, TagName, World};
use crate::mergehold::{Base, HeadSha, Repo};

/// **Reads `requested`** into the act and what is shown of it, or says why it is not offered.
///
/// A merge is the merge hold's ([`crate::mergehold`]) and is never read here.
pub fn prepare(world: &World<'_>, requested: &Requested) -> Result<Prepared, Refused> {
    let repo = match requested {
        Requested::Merge(_) => {
            return Err(Refused::Forge(
                "a merge is held by the merge hold, not here".to_owned(),
            ));
        }
        Requested::Tag { repo, .. }
        | Requested::Release { repo, .. }
        | Requested::CiPush { repo, .. }
        | Requested::Fork { source: repo } => repo,
    };
    if !world.recorded.holds(repo) {
        return Err(Refused::NotRecorded(repo.clone()));
    }
    match requested {
        Requested::Tag { repo, name, rev } => tag(world, repo, name, rev),
        Requested::Release {
            repo,
            tag,
            rev,
            title,
            notes,
            draft,
            prerelease,
        } => release(
            world,
            repo,
            tag,
            rev.as_deref(),
            Text { title, notes },
            (*draft, *prerelease),
        ),
        Requested::CiPush { repo, branch, tip } => ci_push(world, repo, branch, tip),
        Requested::Fork { source } => fork(world, source),
        Requested::Merge(_) => unreachable!("returned above"),
    }
}

/// **The request, pinned to what the ask read**: a name that moves (a branch a tag or release was
/// asked at) becomes the commit it was, so the press's read resolves the same commit and a branch
/// that moves afterwards changes nothing.
pub fn pinned(requested: &Requested, prepared: &Prepared) -> Requested {
    match (requested, &prepared.act) {
        (Requested::Tag { repo, name, .. }, Act::Tag { target, .. }) => Requested::Tag {
            repo: repo.clone(),
            name: name.clone(),
            rev: target.as_str().to_owned(),
        },
        (
            Requested::Release {
                repo,
                tag,
                title,
                notes,
                draft,
                prerelease,
                ..
            },
            Act::Release { target, .. },
        ) => Requested::Release {
            repo: repo.clone(),
            tag: tag.clone(),
            rev: Some(target.as_str().to_owned()),
            title: title.clone(),
            notes: notes.clone(),
            draft: *draft,
            prerelease: *prerelease,
        },
        _ => requested.clone(),
    }
}

fn row(label: &str, value: impl Into<String>) -> Row {
    Row {
        label: label.to_owned(),
        value: value.into(),
    }
}

fn forge_said(why: String) -> Refused {
    Refused::Forge(why)
}

/// The commit `rev` names, as the forge resolves it.
fn commit(world: &World<'_>, repo: &Repo, rev: &str) -> Result<HeadSha, Refused> {
    world
        .forge
        .rev_commit(repo, rev)
        .map_err(|why| Refused::NoSuchCommit {
            rev: rev.to_owned(),
            why,
        })
}

/// Whether `target` is on the default branch, in the words a row uses.
fn on_default(world: &World<'_>, repo: &Repo, target: &HeadSha) -> Result<Row, Refused> {
    let (default, _) = world.forge.default_branch(repo).map_err(forge_said)?;
    let on = world
        .forge
        .on_branch(repo, &default, target)
        .map_err(forge_said)?;
    Ok(row(
        "On the default branch",
        if on {
            format!("Yes, it is on {}", default.as_str())
        } else {
            format!(
                "No: this commit is not on {}. Nobody merged it there",
                default.as_str()
            )
        },
    ))
}

fn tag(world: &World<'_>, repo: &Repo, name: &TagName, rev: &str) -> Result<Prepared, Refused> {
    let target = commit(world, repo, rev)?;
    if let Some(at) = world.forge.tag_at(repo, name).map_err(forge_said)? {
        return Err(Refused::TagExists {
            name: name.clone(),
            at,
        });
    }
    let rows = vec![
        row("Repo", repo.as_str()),
        row("Tag", name.as_str()),
        row("Commit", target.as_str()),
        on_default(world, repo, &target)?,
    ];
    Ok(Prepared {
        act: Act::Tag {
            repo: repo.clone(),
            name: name.clone(),
            target,
        },
        shown: Shown {
            kind: "tag",
            repo: repo.as_str().to_owned(),
            rows,
            body: None,
        },
    })
}

/// The words a release carries.
struct Text<'a> {
    title: &'a str,
    notes: &'a str,
}

fn release(
    world: &World<'_>,
    repo: &Repo,
    tag: &TagName,
    rev: Option<&str>,
    text: Text<'_>,
    (draft, prerelease): (bool, bool),
) -> Result<Prepared, Refused> {
    let exists = world.forge.tag_at(repo, tag).map_err(forge_said)?;
    let asked = match rev {
        Some(rev) => Some(commit(world, repo, rev)?),
        None => None,
    };
    let (target, creates_tag) = match (exists, asked) {
        // The tag is there: the release goes against it, as it is. A request that names another
        // commit would move it, which no ask does.
        (Some(at), Some(asked)) if at != asked => {
            return Err(Refused::TagExists {
                name: tag.clone(),
                at,
            });
        }
        (Some(at), _) => (at, false),
        (None, Some(asked)) => (asked, true),
        // No target named: GitHub's default is the default branch's head, read now and pinned.
        (None, None) => {
            let (_, tip) = world.forge.default_branch(repo).map_err(forge_said)?;
            (tip, true)
        }
    };
    let mut rows = vec![
        row("Repo", repo.as_str()),
        row("Tag", tag.as_str()),
        row("Commit", target.as_str()),
        on_default(world, repo, &target)?,
        row(
            "Makes the tag",
            if creates_tag {
                "Yes, at this commit"
            } else {
                "No, it exists at this commit"
            },
        ),
        row(
            "Title",
            if text.title.is_empty() {
                "(none)"
            } else {
                text.title
            },
        ),
        row("Pre-release", if prerelease { "Yes" } else { "No" }),
    ];
    if draft {
        rows.push(row("Draft", "Yes: it is not published"));
    }
    Ok(Prepared {
        act: Act::Release {
            repo: repo.clone(),
            tag: tag.clone(),
            target,
            creates_tag,
            title: text.title.to_owned(),
            notes: text.notes.to_owned(),
            draft,
            prerelease,
        },
        shown: Shown {
            kind: "release",
            repo: repo.as_str().to_owned(),
            rows,
            body: Some(Body {
                label: "Notes".to_owned(),
                text: text.notes.to_owned(),
            }),
        },
    })
}

fn ci_push(
    world: &World<'_>,
    repo: &Repo,
    branch: &Base,
    tip: &HeadSha,
) -> Result<Prepared, Refused> {
    world
        .workflow
        .available(&world.forge.host())
        .map_err(|why| Refused::NoWorkflowCredential(why.0))?;
    let forge = world.forge;
    let (against, base) = match forge.branch_tip(repo, branch).map_err(forge_said)? {
        Some(tip) => (Against::BranchTip, tip),
        None => {
            let (default, tip) = forge.default_branch(repo).map_err(forge_said)?;
            (Against::DefaultBranch(default), tip)
        }
    };
    let config = forge.ci_config_path(repo).map_err(forge_said)?;
    let diff: CiDiff = match check_push(forge.kind(), config.as_deref(), world.trees, &base, tip) {
        PushCheck::Hold(diff) => diff,
        PushCheck::NoCiFiles => return Err(Refused::NoCiFiles),
        PushCheck::Refuse(why) => return Err(Refused::CiDiff(why)),
    };
    let compared = match &against {
        Against::BranchTip => format!("the tip of {} on the forge", branch.as_str()),
        Against::DefaultBranch(default) => format!(
            "the tip of {}, the default branch: {} is a new branch",
            default.as_str(),
            branch.as_str()
        ),
    };
    let mut rows = vec![
        row("Repo", repo.as_str()),
        row("Branch", branch.as_str()),
        row("New tip", tip.as_str()),
        row("Compared with", format!("{compared} ({})", base.as_str())),
        row("CI files", diff.files.join(", ")),
        row(
            "Sign-in used",
            "The second sign-in, with the workflow permission. It is used for this push only",
        ),
    ];
    if !diff.outside.is_empty() {
        rows.push(row(
            "The pipeline also runs, outside this push",
            diff.outside.join("; "),
        ));
    }
    let body = Some(Body {
        label: "Changed lines".to_owned(),
        text: diff.diff.clone(),
    });
    Ok(Prepared {
        act: Act::CiPush {
            repo: repo.clone(),
            branch: branch.clone(),
            tip: tip.clone(),
            against,
            base,
            diff,
        },
        shown: Shown {
            kind: "CI-file push",
            repo: repo.as_str().to_owned(),
            rows,
            body,
        },
    })
}

fn fork(world: &World<'_>, source: &Repo) -> Result<Prepared, Refused> {
    if !world.forge.may_fork(source).map_err(forge_said)? {
        return Err(Refused::ForkNotAllowed(source.clone()));
    }
    let source_id = world.forge.source_id(source).map_err(forge_said)?;
    let into = world.forge.account().map_err(forge_said)?;
    let rows = vec![
        row("Source", source.as_str()),
        row("Goes to", format!("your own account, {into}")),
        row("The host allows it", "Yes"),
    ];
    Ok(Prepared {
        act: Act::Fork {
            source: source.clone(),
            source_id,
            into,
        },
        shown: Shown {
            kind: "fork",
            repo: source.as_str().to_owned(),
            rows,
            body: None,
        },
    })
}
