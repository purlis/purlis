// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **Links in a pane's output** (spec #6208, #6215): the window's two commands over the core's
//! resolver ([`purlis_core::panelinks`]).
//!
//! - [`pane_link_resolve`]: what each candidate a line holds is, for a hover. The pane caches
//!   the answers itself, per pane, so hovering a line again asks nothing.
//! - [`pane_link_open`]: the one open command (ADR 0052's list). It is handed the candidate's
//!   text again, never a target, and resolves it again, so a pane can never name what opens. It
//!   answers [`LinkOpened`]: opened, ask the person first, or refused with the window's
//!   sentence. A web address goes out through the opener plugin.
//!
//! Neither command reads a secret's value or hands one back.

use std::path::PathBuf;

use purlis_core::panelinks::{Candidate, Hover, Kind, Opening, Pane, Resolver, Target};
use serde::{Deserialize, Serialize};

use crate::planes::PlaneId;

/// **What the pane's pattern took a candidate for**: a hint, never trusted for more. The wire's
/// half of the core's [`Kind`]; a kind's resolver is the core's `LinkKind`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "lowercase")]
pub enum KindHint {
    Url,
}

impl From<KindHint> for Kind {
    fn from(kind: KindHint) -> Self {
        match kind {
            KindHint::Url => Kind::Url,
        }
    }
}

impl From<Kind> for KindHint {
    fn from(kind: Kind) -> Self {
        match kind {
            Kind::Url => KindHint::Url,
        }
    }
}

/// **Text a pane found that may be a link**: the words as printed, and the pattern's hint.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, specta::Type)]
pub struct LinkCandidate {
    pub kind: KindHint,
    pub text: String,
}

impl From<LinkCandidate> for Candidate {
    fn from(candidate: LinkCandidate) -> Self {
        Candidate {
            kind: candidate.kind.into(),
            text: candidate.text,
        }
    }
}

/// **Where a candidate was found**: the pane's project and the chat it draws, where it has them
/// (a saved chat's read-only terminal has neither).
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize, specta::Type)]
pub struct LinkFrom {
    pub plane: Option<PlaneId>,
    pub session: Option<u32>,
}

impl From<LinkFrom> for Pane {
    fn from(from: LinkFrom) -> Self {
        Pane {
            project: from.plane.map(|plane| PathBuf::from(plane.root())),
            session: from.session,
        }
    }
}

/// **What ⌘-click on a link would do**, said on its hover.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
#[serde(tag = "does", rename_all = "lowercase")]
pub enum LinkDoes {
    /// It opens, without asking.
    Opens,
    /// It asks first, and why.
    Asks { reason: String },
    /// It will not open, and the sentence that says why.
    Refused { reason: String },
}

/// **What a hover over a candidate shows**: its kind, its target in full (its own text, where it
/// is no link) and what opening it does.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
pub struct LinkSeen {
    pub kind: KindHint,
    pub target: String,
    pub does: LinkDoes,
}

/// **The open command's answer.**
#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
#[serde(tag = "answer", rename_all = "lowercase")]
pub enum LinkOpened {
    /// Opened: `target` is what was opened.
    Opened { kind: KindHint, target: String },
    /// Not opened yet: the person is asked, shown its text and where it goes. Accepted, the
    /// same candidate is sent again with `target` as what was confirmed.
    Ask {
        reason: String,
        shown: String,
        target: String,
    },
    /// Not opened, and the sentence the window shows.
    Refused { reason: String },
}

/// **What `candidate` is, for a hover**, resolved once: a link and what opening it does, or
/// text that is no link and why not (`ftp://…` says purlis opens only http and https). `None` is
/// left for a later kind's text that says nothing on a hover.
pub(crate) fn seen(resolver: &Resolver, pane: &Pane, candidate: &Candidate) -> Option<LinkSeen> {
    Some(match resolver.hover(candidate, pane) {
        Hover::Link { target, opening } => LinkSeen {
            kind: target.kind().into(),
            target: target.shown().to_owned(),
            does: match opening {
                Opening::Open(_) => LinkDoes::Opens,
                Opening::Ask { reason, .. } => LinkDoes::Asks { reason },
                Opening::Refused(reason) => LinkDoes::Refused { reason },
            },
        },
        Hover::NotALink(reason) => LinkSeen {
            kind: candidate.kind.into(),
            target: candidate.text.clone(),
            does: LinkDoes::Refused { reason },
        },
    })
}

/// **Opens `candidate`, resolved again from its text**: `open_url` is handed the core's target
/// and nothing the window said. `confirmed` is the target an ask showed and the person accepted.
pub(crate) fn answer(
    resolver: &Resolver,
    pane: &Pane,
    candidate: &Candidate,
    confirmed: Option<&str>,
    open_url: impl FnOnce(&str) -> Result<(), String>,
) -> LinkOpened {
    match resolver.open(candidate, pane, confirmed) {
        Opening::Open(target) => {
            let kind = target.kind().into();
            let opened = match &target {
                Target::Url(url) => open_url(url.as_str()),
            };
            match opened {
                Ok(()) => LinkOpened::Opened {
                    kind,
                    target: target.shown().to_owned(),
                },
                Err(reason) => LinkOpened::Refused { reason },
            }
        }
        Opening::Ask {
            reason,
            shown,
            target,
        } => LinkOpened::Ask {
            reason,
            shown,
            target: target.shown().to_owned(),
        },
        Opening::Refused(reason) => LinkOpened::Refused { reason },
    }
}

/// **What each candidate on a line is**, in order: a link, or none. What a hover shows.
#[tauri::command]
#[specta::specta]
pub async fn pane_link_resolve(
    from: LinkFrom,
    candidates: Vec<LinkCandidate>,
) -> Vec<Option<LinkSeen>> {
    let resolver = Resolver::default();
    let pane = Pane::from(from);
    candidates
        .into_iter()
        .map(|candidate| seen(&resolver, &pane, &candidate.into()))
        .collect()
}

/// **Opens a link a pane found** (⌘-click, Ctrl+click off a Mac): the candidate's text,
/// resolved again here. `confirmed` is the target an ask showed, which the person accepted.
#[tauri::command]
#[specta::specta]
pub async fn pane_link_open(
    app: tauri::AppHandle,
    from: LinkFrom,
    candidate: LinkCandidate,
    confirmed: Option<String>,
) -> LinkOpened {
    answer(
        &Resolver::default(),
        &Pane::from(from),
        &candidate.into(),
        confirmed.as_deref(),
        |url| open_in_browser(&app, url),
    )
}

/// Opens `url` in the person's browser; a scenario test's build opens nothing.
fn open_in_browser<R: tauri::Runtime>(app: &tauri::AppHandle<R>, url: &str) -> Result<(), String> {
    #[cfg(feature = "e2e")]
    {
        let _ = (app, url);
        Ok(())
    }
    #[cfg(not(feature = "e2e"))]
    {
        use tauri_plugin_opener::OpenerExt as _;
        // The opener's own words could repeat the address: the sentence is purlis's.
        app.opener()
            .open_url(url, None::<&str>)
            .map_err(|_| "purlis could not open the address in your browser".to_owned())
    }
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;

    use super::*;

    fn url(text: &str) -> Candidate {
        Candidate {
            kind: Kind::Url,
            text: text.to_owned(),
        }
    }

    /// Opens `text` as the command does, with a browser that records what it was handed.
    fn opened(text: &str) -> (LinkOpened, Vec<String>) {
        let handed = RefCell::new(Vec::new());
        let answered = answer(
            &Resolver::default(),
            &Pane::default(),
            &url(text),
            None,
            |url| {
                handed.borrow_mut().push(url.to_owned());
                Ok(())
            },
        );
        (answered, handed.into_inner())
    }

    #[test]
    fn an_https_address_opens_in_the_browser() {
        let (answered, handed) = opened("https://github.com/purlis/purlis/pull/6148");

        assert_eq!(
            answered,
            LinkOpened::Opened {
                kind: KindHint::Url,
                target: "https://github.com/purlis/purlis/pull/6148".into()
            }
        );
        assert_eq!(handed, ["https://github.com/purlis/purlis/pull/6148"]);
    }

    #[test]
    fn another_scheme_is_refused_and_the_browser_is_handed_nothing() {
        for text in [
            "ftp://mirror.example.org/pub",
            "file:///etc/hosts",
            "javascript:alert(1)",
        ] {
            let (answered, handed) = opened(text);
            assert!(
                matches!(&answered, LinkOpened::Refused { reason } if reason.contains("only http and https")),
                "{text}: {answered:?}"
            );
            assert!(handed.is_empty(), "{text} reached the browser");
        }
    }

    #[test]
    fn what_opens_is_the_core_s_reading_of_the_text_never_a_target_the_pane_named() {
        // The command takes no target: only the text, which the core reads again. A pane that
        // sent words with a hidden byte, or an address the browser would read another way, opens
        // only what the core makes of them.
        let (answered, handed) = opened("https://purlis.dev\u{0}.evil.example/");
        assert!(
            matches!(answered, LinkOpened::Refused { .. }),
            "{answered:?}"
        );
        assert!(handed.is_empty());

        let (_, handed) = opened("HTTPS://PURLIS.dev");
        assert_eq!(handed, ["https://purlis.dev/"]);
    }

    #[test]
    fn a_browser_that_will_not_open_it_is_a_refusal_in_purlis_s_words() {
        let answered = answer(
            &Resolver::default(),
            &Pane::default(),
            &url("https://purlis.dev/"),
            None,
            |_| Err("purlis could not open the address in your browser".into()),
        );

        assert_eq!(
            answered,
            LinkOpened::Refused {
                reason: "purlis could not open the address in your browser".into()
            }
        );
    }

    #[test]
    fn a_hover_says_the_target_and_that_it_opens() {
        let seen = seen(
            &Resolver::default(),
            &Pane::default(),
            &url("https://purlis.dev"),
        );

        assert_eq!(
            seen,
            Some(LinkSeen {
                kind: KindHint::Url,
                target: "https://purlis.dev/".into(),
                does: LinkDoes::Opens,
            })
        );
    }

    #[test]
    fn a_hover_over_another_scheme_says_why_it_won_t_open_and_so_does_one_carrying_a_password() {
        assert_eq!(
            seen(
                &Resolver::default(),
                &Pane::default(),
                &url("ftp://x.example/")
            ),
            Some(LinkSeen {
                kind: KindHint::Url,
                target: "ftp://x.example/".into(),
                does: LinkDoes::Refused {
                    reason: "purlis opens only http and https addresses, so it will not open \
                             this ftp: one"
                        .into()
                },
            })
        );
        assert!(matches!(
            seen(
                &Resolver::default(),
                &Pane::default(),
                // A made-up sign-in, spelled in parts so the commit scan does not take it for one.
                &url(&["https://", "me:", "made-up", "@purlis.dev/"].concat())
            ),
            Some(LinkSeen {
                does: LinkDoes::Refused { .. },
                ..
            })
        ));
    }

    #[test]
    fn the_window_s_words_for_the_answers_are_these() {
        // The shape the window reads (`paneLinks.ts`).
        let json = |answer: &LinkOpened| serde_json::to_value(answer).expect("serialises");
        assert_eq!(
            json(&LinkOpened::Opened {
                kind: KindHint::Url,
                target: "https://a.example/".into()
            }),
            serde_json::json!({"answer": "opened", "kind": "url", "target": "https://a.example/"})
        );
        assert_eq!(
            json(&LinkOpened::Refused {
                reason: "no".into()
            }),
            serde_json::json!({"answer": "refused", "reason": "no"})
        );
        assert_eq!(
            serde_json::to_value(LinkDoes::Asks { reason: "r".into() }).expect("serialises"),
            serde_json::json!({"does": "asks", "reason": "r"})
        );
    }
}
