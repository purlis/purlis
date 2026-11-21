// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **Reading an ask** (ADR 0128 D8): what a request named becomes the exact act and what the
//! person is shown of it, all of it read from the forge and the clone by the app and none of it
//! from the chat.
//!
//! The one function, [`prepare`], is used twice: when the ask is made, and again at each press.
//! That is what makes "the ask shows what will happen" checkable: a press performs the act a read
//! gives **now**, and only while it is the act the person was shown.

use super::ci::{PushCheck, check_push_in};
use super::recognise::valid_rev;
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

/// The most a release's title and notes are, in characters.
const MOST_TITLE: usize = 200;
const MOST_NOTES: usize = 60_000;
/// The most bytes of a title's or notes' raw text, checked before anything is walked: the bound
/// in characters counts only what is left once the unseen are filtered, so it is no bound on
/// what is stored.
const MOST_TITLE_BYTES: usize = 4 * MOST_TITLE;
const MOST_NOTES_BYTES: usize = 4 * MOST_NOTES;

/// Whether `c` draws nothing or changes how the text around it reads: a control character other
/// than a tab or a line break, a zero-width or direction mark, a line or paragraph separator.
pub(super) fn unseen(c: char) -> bool {
    (c.is_control() && !matches!(c, '\n' | '\t'))
        || matches!(
            c,
            '\u{061C}'
                | '\u{200B}'..='\u{200F}'
                | '\u{2028}'..='\u{202E}'
                | '\u{2060}'..='\u{2064}'
                | '\u{2066}'..='\u{206F}'
                | '\u{FEFF}'
                // Fillers and soft hyphens that draw nothing, the tag characters (an invisible
                // copy of ASCII), and the variation selectors.
                | '\u{00AD}'
                | '\u{034F}'
                | '\u{115F}'
                | '\u{1160}'
                | '\u{17B4}'
                | '\u{17B5}'
                | '\u{180B}'..='\u{180F}'
                | '\u{3164}'
                | '\u{FE00}'..='\u{FE0F}'
                | '\u{FFA0}'
                | '\u{E0000}'..='\u{E007F}'
                | '\u{E0100}'..='\u{E01EF}'
        )
}

/// Whether a release's words are within what is shown: checked before an ask is queued, so what
/// waits in memory for a desktop is never longer than what a press could show.
pub(super) fn within_title_and_notes(title: &str, notes: &str) -> Result<(), Refused> {
    title_of(title)?;
    notes_of(notes)?;
    Ok(())
}

/// **A release's title as it is shown and made**: one line, nothing unseen in it. Text a chat
/// wrote is read as the person will read it, so what is shown is what is published.
fn title_of(text: &str) -> Result<String, Refused> {
    if text.len() > MOST_TITLE_BYTES {
        return Err(Refused::Text(
            "a release's title is longer than purlis shows",
        ));
    }
    let one: String = text
        .chars()
        .filter(|c| !unseen(*c))
        .map(|c| if c == '\n' || c == '\t' { ' ' } else { c })
        .collect();
    let one = one.trim().to_owned();
    if one.chars().count() > MOST_TITLE {
        return Err(Refused::Text(
            "a release's title is longer than purlis shows",
        ));
    }
    Ok(one)
}

/// **A release's notes as they are shown and made**: line breaks kept, nothing unseen in them.
fn notes_of(text: &str) -> Result<String, Refused> {
    if text.len() > MOST_NOTES_BYTES {
        return Err(Refused::Text(
            "a release's notes are longer than purlis shows",
        ));
    }
    let notes: String = text
        .replace("\r\n", "\n")
        .replace('\r', "\n")
        .chars()
        .filter(|c| !unseen(*c))
        .collect();
    if notes.chars().count() > MOST_NOTES {
        return Err(Refused::Text(
            "a release's notes are longer than purlis shows",
        ));
    }
    Ok(notes)
}

/// The most characters of one row's value, and of one entry of a list in a row.
const MOST_ROW: usize = 2_000;
const MOST_ENTRY: usize = 300;

/// **Text a chat may have written, as a row shows it**: a character that draws nothing, or breaks
/// the line, is written out (`\u{202e}`), so what is shown can never read as other text, and an
/// entry longer than `most` characters is cut and says so.
fn shown_text(text: &str, most: usize) -> String {
    let mut out = String::new();
    for (at, c) in text.chars().enumerate() {
        if at == most {
            out.push_str(&format!(
                "… ({} more characters)",
                text.chars().count() - most
            ));
            break;
        }
        if unseen(c) || matches!(c, '\n' | '\t') {
            out.push_str(&format!("\\u{{{:04x}}}", c as u32));
        } else {
            out.push(c);
        }
    }
    out
}

/// **A diff as the body shows it**: line breaks and tabs kept, every other character that draws
/// nothing or turns the text around written out (`\u{202e}`), so a changed line cannot read as
/// other text.
fn escaped_lines(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        if unseen(c) {
            out.push_str(&format!("\\u{{{:04x}}}", c as u32));
        } else {
            out.push(c);
        }
    }
    out
}

/// A commit message as one row shows it: its lines joined, nothing unseen. **A message that does
/// not fit the row is refused**, never cut: the message is sent whole, so a tail the row hid
/// would be sent unseen.
fn message_row(message: &str) -> Result<String, Refused> {
    let joined = message
        .lines()
        .map(str::trim_end)
        .collect::<Vec<_>>()
        .join(" ⏎ ");
    if joined.chars().count() > MOST_ROW {
        return Err(Refused::Text(
            "the commit message is longer than purlis shows, so it is not held and not allowed",
        ));
    }
    Ok(shown_text(&joined, MOST_ROW))
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
    // GitLab's releases have no draft and no pre-release: not offered, rather than made as
    // something other than what was asked.
    if world.forge.kind() == crate::forge::Kind::GitLab && (draft || prerelease) {
        return Err(Refused::Text(
            "GitLab has no draft or pre-release releases, so a release is not made as one",
        ));
    }
    let title = title_of(text.title)?;
    let notes = notes_of(text.notes)?;
    let text = Text {
        title: &title,
        notes: &notes,
    };
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
    let forge = world.forge;
    if !valid_rev(branch.as_str()) {
        return Err(Refused::Text("a branch whose name git does not allow"));
    }
    // GitHub's `workflow` scope is the second credential's; GitLab has none, so a pipeline file
    // is pushed with the everyday sign-in and the act is offered without it.
    let second = forge.kind() == crate::forge::Kind::GitHub;
    if second {
        world
            .workflow
            .available(&forge.host())
            .map_err(|why| Refused::NoWorkflowCredential(why.0))?;
    }
    let (against, base) = match forge.branch_tip(repo, branch).map_err(forge_said)? {
        Some(tip) => (Against::BranchTip, tip),
        None => {
            let (default, tip) = forge.default_branch(repo).map_err(forge_said)?;
            (Against::DefaultBranch(default), tip)
        }
    };
    let config = forge.ci_config_path(repo).map_err(forge_said)?;
    let diff: CiDiff = match check_push_in(
        forge.kind(),
        config.as_deref(),
        Some(repo.as_str()),
        world.trees,
        &base,
        tip,
    ) {
        PushCheck::Hold(diff) => diff,
        PushCheck::NoCiFiles => return Err(Refused::NoCiFiles),
        PushCheck::Refuse(why) => return Err(Refused::CiDiff(why)),
    };
    // A path is shown as it is, so one that holds a line break or a character that draws nothing
    // could read as other paths or other words: such a push is not offered.
    if let Some(bad) = diff
        .files
        .iter()
        .find(|path| path.chars().any(|c| c.is_control() || unseen(c)))
    {
        return Err(Refused::CiDiff(format!(
            "a changed file's path holds a line break or a character that draws nothing: {}",
            shown_text(bad, MOST_ENTRY)
        )));
    }
    if diff.files.join(", ").chars().count() > MOST_ROW {
        return Err(Refused::Text(
            "the changed CI files' paths are longer than purlis shows",
        ));
    }
    let compared = match &against {
        Against::BranchTip => format!(
            "the tip of {} on the forge",
            shown_text(branch.as_str(), MOST_ENTRY)
        ),
        Against::DefaultBranch(default) => format!(
            "the tip of {}, the default branch: {} is a new branch",
            shown_text(default.as_str(), MOST_ENTRY),
            shown_text(branch.as_str(), MOST_ENTRY)
        ),
    };
    let mut rows = vec![
        row("Repo", repo.as_str()),
        row("Branch", shown_text(branch.as_str(), MOST_ENTRY)),
        row("New tip", tip.as_str()),
        row("Compared with", format!("{compared} ({})", base.as_str())),
        row("CI files", diff.files.join(", ")),
        row("Commit message", message_row(&diff.message)?),
        row(
            "Other files in this commit",
            match diff.others {
                0 => "None: the commit changes only the CI files above".to_owned(),
                n => format!(
                    "{n}, which are not CI files and are not shown here. They are in the commit \
                     that is pushed"
                ),
            },
        ),
        row(
            "Sign-in used",
            if second {
                "The second sign-in, with the workflow permission. It is used for this push only"
            } else {
                "Your everyday sign-in"
            },
        ),
    ];
    let unseen_in_diff = diff.diff.chars().filter(|c| unseen(*c)).count();
    if unseen_in_diff > 0 {
        rows.push(row(
            "Characters that do not show",
            format!(
                "The changed lines hold {unseen_in_diff} control or direction characters that \
                 draw nothing. They are written out as \\u{{…}} in the diff below"
            ),
        ));
    }
    if !diff.outside.is_empty() {
        rows.push(row(
            "The pipeline also runs, outside this push",
            diff.outside
                .iter()
                .map(|note| shown_text(note, MOST_ENTRY))
                .collect::<Vec<_>>()
                .join("; "),
        ));
    }
    let body = Some(Body {
        label: "Changed lines".to_owned(),
        text: escaped_lines(&diff.diff),
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

#[cfg(test)]
mod tests {
    use super::unseen;

    #[test]
    fn characters_that_draw_nothing_are_unseen() {
        for c in [
            '\u{00AD}',
            '\u{115F}',
            '\u{1160}',
            '\u{3164}',
            '\u{E0041}',
            '\u{E007F}',
            '\u{FE0F}',
            '\u{E0100}',
            '\u{200B}',
        ] {
            assert!(unseen(c), "{:04x}", c as u32);
        }
        assert!(!unseen('a') && !unseen('é') && !unseen('\n'));
    }
}
