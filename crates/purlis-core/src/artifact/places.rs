// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **Where a `purlis://` link in an artifact's page goes** (ADR 0122 §5 as amended, #5967;
//! `docs/artifact-links.md`).
//!
//! A page is a hub: a link written in it may open a place in purlis. It never acts. The places
//! are a closed list, each one shape of link:
//!
//! | Link                                   | Opens                                         |
//! |----------------------------------------|-----------------------------------------------|
//! | `purlis://workspace/<workspace>`       | the workspace, focused                        |
//! | `purlis://todo/<workspace>/<todo>`     | the todo's view (`<todo>` its file's stem)    |
//! | `purlis://chat/<chat id>`              | the chat, shown (`<chat id>` its ULID)         |
//! | `purlis://issue/<tracker key>`         | the chat that works on the work item          |
//!
//! A tracker key's `#` is written `%23`, so it is not read as the link's fragment:
//! `purlis://issue/github:github.com/purlis/purlis%235967`.
//!
//! **Anything else is refused** ([`place`]), with a sentence the window shows: another first
//! word (`purlis://run/…`, `purlis://dispatch/…`), a place with more after it
//! (`purlis://todo/alpha/fix-login/done`, `purlis://chat/<id>/end`), a query or a fragment, a
//! name or a port, a name that is not one. So no link can name an action, and no link carries a
//! parameter one could hide in.
//!
//! This module only reads the link. Whether the place is there, and going to it, is the app's
//! (`artifact_links.rs`) and the window's (`app/src/artifactPlaces.ts`).

use crate::work::TrackerKey;

/// The scheme of a link into purlis.
pub const SCHEME: &str = "purlis";

/// The longest link this reads.
const MOST: usize = 2048;

/// **A place in purlis a link opens**: one of a closed list, and nothing that does anything.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Place {
    /// A workspace of the artifact's project, by its name.
    Workspace { workspace: String },
    /// A todo, by its workspace and its file's stem.
    Todo { workspace: String, todo: String },
    /// A chat, by its id (its ULID, ADR 0066), in its canonical spelling.
    Chat { chat: String },
    /// A work item, by its tracker key (ADR 0088), which the chat working on it stands for.
    Issue { key: TrackerKey },
}

/// **Where `href`, a `purlis:` link as the page holds it, goes**, or the sentence that refuses
/// it.
pub fn place(href: &str) -> Result<Place, String> {
    if href.len() > MOST {
        return Err("that purlis link is too long, so purlis did not follow it".into());
    }
    let prefix = "purlis://";
    let rest = match href.get(..prefix.len()) {
        Some(scheme) if scheme.eq_ignore_ascii_case(prefix) => &href[prefix.len()..],
        _ => {
            return Err(
                "a purlis link starts `purlis://`, and this one does not, so purlis did not \
                 follow it"
                    .into(),
            );
        }
    };
    if rest.contains(['?', '#']) {
        return Err(
            "a purlis link carries nothing after `?` or `#`, so purlis did not follow this one"
                .into(),
        );
    }
    let (word, after) = rest.split_once('/').unwrap_or((rest, ""));
    let unknown = || {
        format!(
            "a purlis link opens a workspace, a todo, a chat or an issue, and `{word}` is none of \
             them, so purlis did not follow it"
        )
    };
    match word {
        "workspace" => {
            let [workspace] = names(after)?;
            Ok(Place::Workspace { workspace })
        }
        "todo" => {
            let [workspace, todo] = names(after)?;
            TrackerKey::todo(&workspace, &todo).map_err(|_| not_a_name(&todo))?;
            Ok(Place::Todo { workspace, todo })
        }
        "chat" => {
            let [chat] = names(after)?;
            match crate::reopen::a_ulid(&chat) {
                Some(chat) => Ok(Place::Chat { chat }),
                None => Err(format!(
                    "`{chat}` is not a chat's id, so purlis did not follow the link"
                )),
            }
        }
        "issue" => {
            let key = decoded(after)?;
            TrackerKey::parse(&key)
                .map(|key| Place::Issue { key })
                .map_err(|why| format!("{why}, so purlis did not follow the link"))
        }
        _ => Err(unknown()),
    }
}

/// **The `N` names of `path`**, `/`-separated, each percent-decoded once and one plain name,
/// or the sentence that refuses them: too few, a name that is not one, or more after them.
fn names<const N: usize>(path: &str) -> Result<[String; N], String> {
    let mut parts = path.split('/');
    let mut out: [String; N] = std::array::from_fn(|_| String::new());
    for slot in &mut out {
        let name = decoded(parts.next().unwrap_or_default())?;
        if !crate::contain::segment_ok(&name) || name.contains(['/', '\\', ':']) {
            return Err(not_a_name(&name));
        }
        *slot = name;
    }
    if let Some(more) = parts.next() {
        return Err(format!(
            "a purlis link only opens a place, and this one names more after it (`{more}`), so \
             purlis did not follow it"
        ));
    }
    Ok(out)
}

/// `text`, percent-decoded once, or the sentence that refuses it when that is not UTF-8.
fn decoded(text: &str) -> Result<String, String> {
    percent_encoding::percent_decode_str(text)
        .decode_utf8()
        .map(|text| text.into_owned())
        .map_err(|_| "that purlis link is not readable text, so purlis did not follow it".into())
}

fn not_a_name(name: &str) -> String {
    if name.is_empty() {
        "that purlis link leaves a name out, so purlis did not follow it".into()
    } else {
        format!("`{name}` is not a name purlis gives anything, so purlis did not follow the link")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ULID: &str = "01M70V414APWHKNRPG7YHK6J20";

    fn refused(href: &str) -> String {
        match place(href) {
            Err(why) => why,
            Ok(went) => panic!("{href} was followed to {went:?}"),
        }
    }

    #[test]
    fn a_workspace_link_opens_the_workspace() {
        assert_eq!(
            place("purlis://workspace/alpha"),
            Ok(Place::Workspace {
                workspace: "alpha".into()
            })
        );
        assert_eq!(
            place("PURLIS://workspace/smart-ide"),
            Ok(Place::Workspace {
                workspace: "smart-ide".into()
            })
        );
    }

    #[test]
    fn a_todo_link_opens_the_todo_of_its_workspace() {
        assert_eq!(
            place("purlis://todo/alpha/fix-login"),
            Ok(Place::Todo {
                workspace: "alpha".into(),
                todo: "fix-login".into()
            })
        );
    }

    #[test]
    fn a_chat_link_opens_the_chat_by_its_id_in_its_canonical_spelling() {
        assert_eq!(
            place(&format!("purlis://chat/{ULID}")),
            Ok(Place::Chat { chat: ULID.into() })
        );
        assert_eq!(
            place(&format!("purlis://chat/{}", ULID.to_lowercase())),
            Ok(Place::Chat { chat: ULID.into() })
        );
        assert!(refused("purlis://chat/7").contains("is not a chat's id"));
    }

    #[test]
    fn an_issue_link_names_its_work_item_by_its_tracker_key_with_its_hash_escaped() {
        assert_eq!(
            place("purlis://issue/github:github.com/purlis/purlis%235967"),
            Ok(Place::Issue {
                key: TrackerKey::parse("github:github.com/purlis/purlis#5967").unwrap()
            })
        );
        assert_eq!(
            place("purlis://issue/todo:alpha/fix-login"),
            Ok(Place::Issue {
                key: TrackerKey::parse("todo:alpha/fix-login").unwrap()
            })
        );
        assert!(refused("purlis://issue/nothing-here").contains("is not a tracker key"));
    }

    #[test]
    fn a_link_naming_an_action_is_refused() {
        for (href, word) in [
            ("purlis://run/regenerate", "run"),
            ("purlis://dispatch/steward", "dispatch"),
            ("purlis://todo-done/alpha/fix-login", "todo-done"),
            ("purlis://settings", "settings"),
        ] {
            assert_eq!(
                refused(href),
                format!(
                    "a purlis link opens a workspace, a todo, a chat or an issue, and `{word}` is \
                     none of them, so purlis did not follow it"
                ),
                "{href}"
            );
        }
        for (href, more) in [
            ("purlis://todo/alpha/fix-login/done", "done"),
            ("purlis://workspace/alpha/remove", "remove"),
            (&format!("purlis://chat/{ULID}/end") as &str, "end"),
        ] {
            assert_eq!(
                refused(href),
                format!(
                    "a purlis link only opens a place, and this one names more after it \
                     (`{more}`), so purlis did not follow it"
                ),
                "{href}"
            );
        }
        assert!(
            refused("purlis://issue/github:github.com/purlis/purlis%235967/close")
                .contains("is not a tracker key")
        );
    }

    #[test]
    fn a_link_carrying_a_query_or_a_fragment_is_refused() {
        for href in [
            "purlis://workspace/alpha?do=remove",
            "purlis://todo/alpha/fix-login?done=1",
            "purlis://workspace/alpha#remove",
        ] {
            assert!(
                refused(href).contains("carries nothing after `?` or `#`"),
                "{href}"
            );
        }
    }

    #[test]
    fn a_link_whose_names_are_not_names_is_refused() {
        for href in [
            "purlis://workspace/..",
            "purlis://workspace/a%2Fb",
            "purlis://workspace/",
            "purlis://workspace",
            "purlis://todo/alpha",
            "purlis://todo/alpha/",
            "purlis://todo/../fix-login",
            "purlis://workspace/%ff",
            "purlis://user@workspace/alpha",
            "purlis://workspace:80/alpha",
            "purlis:workspace/alpha",
            "purlis:///workspace/alpha",
        ] {
            assert!(place(href).is_err(), "{href} was followed");
        }
    }

    #[test]
    fn a_link_that_is_not_a_purlis_link_or_too_long_is_refused() {
        assert!(place("https://example.com/").is_err());
        assert!(refused(&format!("purlis://workspace/{}", "a".repeat(MOST))).contains("too long"));
    }
}
