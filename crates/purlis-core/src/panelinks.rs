// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **Links in a pane's output** (spec #6208, #6215): the core's half. A pane finds text that
//! looks like a link and sends only that text, a kind hint and where it was found
//! ([`Candidate`], [`Pane`]). The core decides what it is and what opening it does.
//!
//! - **The kind is a seam** ([`LinkKind`]): each kind reads a candidate's text into a
//!   [`Target`] or says it is not a link, and decides what opening that target does. The web
//!   address ([`Kind::Url`]) is the first; files and chats follow (#6216, #6217).
//! - **Opening resolves again** ([`Resolver::open`]): what the window asks to open is the
//!   candidate's text, never a target it resolved, so a pane can never name what opens.
//! - **The answer is one of three** ([`Opening`]): open it, ask the person first (showing what
//!   it is and where it goes), or refuse it with the sentence the window shows. An ask the
//!   person accepted comes back with the target they were shown, and opens only that target.

use std::path::PathBuf;

/// **What the pane thinks a candidate is**: a hint that picks the kind that reads it, never
/// trusted for more.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Kind {
    /// An `http:` or `https:` address.
    Url,
}

/// **Text a pane found that may be a link**: the words as the program printed them, and the
/// kind the pane's pattern took them for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Candidate {
    pub kind: Kind,
    pub text: String,
}

/// **Where a candidate was found**: the project the pane is in, and the chat it draws, where it
/// draws one. What a later kind resolves against (a chat's working folder, its project's chat
/// numbers); a web address needs neither.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Pane {
    pub project: Option<PathBuf>,
    pub session: Option<u32>,
}

/// **Where a link goes**, as the core resolved it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Target {
    /// A web address, as the browser will be handed it.
    Url(String),
}

impl Target {
    /// The kind of link it is.
    pub fn kind(&self) -> Kind {
        match self {
            Target::Url(_) => Kind::Url,
        }
    }

    /// The target in full, as the person is shown it: a hover's first line, and what an ask
    /// the person accepted sends back.
    pub fn shown(&self) -> &str {
        match self {
            Target::Url(url) => url,
        }
    }
}

/// **What a candidate is**: a link to a target, or not a link, and why not.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Resolved {
    Link(Target),
    NotALink(String),
}

/// **What opening a link does**, the open command's answer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Opening {
    /// Open it now.
    Open(Target),
    /// Ask the person first: why, what it shows (its text), and where it goes.
    Ask {
        reason: String,
        shown: String,
        target: Target,
    },
    /// Not opened, and the sentence the window shows.
    Refused(String),
}

/// **One kind of link** (#6208's pluggable kind): how its text is read into a target, and what
/// opening that target does.
pub trait LinkKind: Send + Sync {
    /// The kind hint it answers.
    fn kind(&self) -> Kind;
    /// `text`, found in `pane`, read into a target, or not a link.
    fn resolve(&self, text: &str, pane: &Pane) -> Resolved;
    /// What opening `target`, resolved from `text` in `pane`, does.
    fn opening(&self, target: Target, text: &str, pane: &Pane) -> Opening;
}

/// **The core's link resolver**: the kinds it knows, each reading the candidates of its hint.
pub struct Resolver {
    kinds: Vec<Box<dyn LinkKind>>,
}

impl Default for Resolver {
    /// The kinds purlis links: web addresses.
    fn default() -> Self {
        Self::with(vec![Box::new(WebAddress)])
    }
}

impl Resolver {
    /// A resolver of these kinds. A hint no kind answers is never a link.
    pub fn with(kinds: Vec<Box<dyn LinkKind>>) -> Self {
        Self { kinds }
    }

    fn kind_of(&self, kind: Kind) -> Option<&dyn LinkKind> {
        self.kinds
            .iter()
            .find(|one| one.kind() == kind)
            .map(|one| &**one)
    }

    /// **What `candidate`, found in `pane`, is**: what a hover shows.
    pub fn resolve(&self, candidate: &Candidate, pane: &Pane) -> Resolved {
        match self.kind_of(candidate.kind) {
            Some(kind) => kind.resolve(&candidate.text, pane),
            None => Resolved::NotALink("purlis does not link that kind of text".into()),
        }
    }

    /// **What opening `candidate` does**, resolved again from its text. `confirmed` is the
    /// target an ask showed, which the person accepted: it opens that target and no other.
    pub fn open(&self, candidate: &Candidate, pane: &Pane, confirmed: Option<&str>) -> Opening {
        let Some(kind) = self.kind_of(candidate.kind) else {
            return Opening::Refused("purlis does not link that kind of text".into());
        };
        let target = match kind.resolve(&candidate.text, pane) {
            Resolved::Link(target) => target,
            Resolved::NotALink(why) => return Opening::Refused(why),
        };
        match kind.opening(target, &candidate.text, pane) {
            Opening::Ask { target, .. } if confirmed == Some(target.shown()) => {
                Opening::Open(target)
            }
            opening => opening,
        }
    }
}

/// **A web address** (`http:` and `https:` only, #6208 decision 5): opened in the person's
/// browser without asking. Every other scheme is not a link, and opening one is refused.
pub struct WebAddress;

impl LinkKind for WebAddress {
    fn kind(&self) -> Kind {
        Kind::Url
    }

    fn resolve(&self, text: &str, _pane: &Pane) -> Resolved {
        // The words exactly, with nothing the parser would quietly drop around or inside them:
        // what opens is what the person was shown.
        if text.is_empty() || text.chars().any(|c| c.is_whitespace() || c.is_control()) {
            return Resolved::NotALink("that is not a web address".into());
        }
        let Ok(url) = url::Url::parse(text) else {
            return Resolved::NotALink("purlis could not read that address".into());
        };
        match url.scheme() {
            "http" | "https" => {}
            other => {
                return Resolved::NotALink(format!(
                    "purlis opens only http and https addresses, so it will not open this {other}: one"
                ));
            }
        }
        if url.host_str().is_none_or(str::is_empty) {
            return Resolved::NotALink("that address names no host".into());
        }
        Resolved::Link(Target::Url(url.to_string()))
    }

    fn opening(&self, target: Target, _text: &str, _pane: &Pane) -> Opening {
        let Target::Url(url) = &target;
        let carries = url::Url::parse(url)
            .map(|url| !url.username().is_empty() || url.password().is_some())
            .unwrap_or(true);
        if carries {
            return Opening::Refused(
                "purlis will not open an address that carries a name or a password".into(),
            );
        }
        Opening::Open(target)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn url(text: &str) -> Candidate {
        Candidate {
            kind: Kind::Url,
            text: text.to_owned(),
        }
    }

    fn here() -> Pane {
        Pane {
            project: Some(PathBuf::from("/planes/one")),
            session: Some(7),
        }
    }

    fn refused(opening: Opening) -> String {
        match opening {
            Opening::Refused(why) => why,
            other => panic!("expected a refusal, got {other:?}"),
        }
    }

    #[test]
    fn an_https_address_is_a_link_to_itself_and_opens() {
        let links = Resolver::default();
        let pr = url("https://github.com/purlis/purlis/pull/6148");

        assert_eq!(
            links.resolve(&pr, &here()),
            Resolved::Link(Target::Url(
                "https://github.com/purlis/purlis/pull/6148".into()
            ))
        );
        assert_eq!(
            links.open(&pr, &here(), None),
            Opening::Open(Target::Url(
                "https://github.com/purlis/purlis/pull/6148".into()
            ))
        );
    }

    #[test]
    fn an_http_address_opens_too() {
        let opening =
            Resolver::default().open(&url("http://localhost:5173/a?b=c#d"), &here(), None);

        assert_eq!(
            opening,
            Opening::Open(Target::Url("http://localhost:5173/a?b=c#d".into()))
        );
    }

    #[test]
    fn a_bare_host_is_shown_as_the_browser_will_be_handed_it() {
        let resolved = Resolver::default().resolve(&url("https://purlis.dev"), &Pane::default());

        assert_eq!(
            resolved,
            Resolved::Link(Target::Url("https://purlis.dev/".into()))
        );
    }

    #[test]
    fn every_other_scheme_is_not_a_link_and_opening_one_is_refused_naming_it() {
        let links = Resolver::default();
        for (text, scheme) in [
            ("ftp://mirror.example.org/pub", "ftp:"),
            ("file:///etc/passwd", "file:"),
            ("javascript:alert(1)", "javascript:"),
            ("vscode://file/etc/hosts", "vscode:"),
            ("smb://share/x", "smb:"),
        ] {
            assert!(
                matches!(links.resolve(&url(text), &here()), Resolved::NotALink(_)),
                "{text} linked"
            );
            let why = refused(links.open(&url(text), &here(), None));
            assert!(why.contains("only http and https"), "{text}: {why}");
            assert!(why.contains(scheme), "{text}: {why}");
        }
    }

    #[test]
    fn text_with_a_space_or_a_control_character_in_it_is_not_a_link() {
        let links = Resolver::default();
        for text in [
            "",
            " https://purlis.dev/",
            "https://purlis.dev/ ",
            "https://pur lis.dev/",
            "https://purlis.dev/\u{7}",
            "https://purlis.dev/\n",
        ] {
            assert!(
                matches!(links.resolve(&url(text), &here()), Resolved::NotALink(_)),
                "{text:?} linked"
            );
            refused(links.open(&url(text), &here(), None));
        }
    }

    #[test]
    fn an_address_carrying_a_name_or_a_password_is_refused_on_open() {
        let links = Resolver::default();
        // Made-up sign-ins, spelled in parts so the commit scan does not take them for real ones.
        let name = ["https://", "me", "@purlis.dev/"].concat();
        let both = ["https://", "me:", "made-up", "@purlis.dev/"].concat();
        for text in [name.as_str(), both.as_str()] {
            let why = refused(links.open(&url(text), &here(), None));
            assert!(why.contains("name or a password"), "{text}: {why}");
        }
    }

    /// A kind that always asks first: the seam's other answers, which later kinds give.
    struct Asks;
    impl LinkKind for Asks {
        fn kind(&self) -> Kind {
            Kind::Url
        }
        fn resolve(&self, text: &str, _pane: &Pane) -> Resolved {
            Resolved::Link(Target::Url(format!("https://real.example/{text}")))
        }
        fn opening(&self, target: Target, text: &str, _pane: &Pane) -> Opening {
            Opening::Ask {
                reason: "its text names another address".into(),
                shown: text.to_owned(),
                target,
            }
        }
    }

    #[test]
    fn a_kind_is_a_seam_another_kind_can_take() {
        let links = Resolver::with(vec![Box::new(Asks)]);

        assert_eq!(
            links.open(&url("setup"), &here(), None),
            Opening::Ask {
                reason: "its text names another address".into(),
                shown: "setup".into(),
                target: Target::Url("https://real.example/setup".into()),
            }
        );
    }

    #[test]
    fn an_ask_accepted_opens_the_target_it_showed_and_no_other() {
        let links = Resolver::with(vec![Box::new(Asks)]);

        assert_eq!(
            links.open(&url("setup"), &here(), Some("https://real.example/setup")),
            Opening::Open(Target::Url("https://real.example/setup".into()))
        );
        // Accepted for another target than the one the text resolves to now: asked again.
        assert!(matches!(
            links.open(&url("setup"), &here(), Some("https://elsewhere.example/")),
            Opening::Ask { .. }
        ));
    }

    #[test]
    fn a_hint_no_kind_answers_is_never_a_link() {
        let links = Resolver::with(vec![]);

        assert!(matches!(
            links.resolve(&url("https://purlis.dev/"), &here()),
            Resolved::NotALink(_)
        ));
        refused(links.open(&url("https://purlis.dev/"), &here(), None));
    }
}
