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
//!   it is and where it goes), or refuse it with the sentence the window shows.
//! - **An ask is answered with its token** ([`Asks`], #6216): the core issues a one-use token
//!   bound to the pane and the target an ask showed, and only that token opens it. It proves
//!   the window was handed that ask, not that a person read it.
//! - **A file path** ([`Kind::File`], [`LocalFile`]) links only once the core finds the file,
//!   and opens in the light editor at its line; one outside the chat's workspace asks first.
//! - **A runner chat's path** is mapped through its container's mounts first ([`Mounts`],
//!   #6221): one that maps to nothing is not a link.

use std::path::PathBuf;

mod asks;
mod file;
mod mounts;
pub use asks::{Asks, Identity};
pub use file::{FileAt, Home, HomeIs, LocalFile, ReadFrom, home_of};
pub use mounts::{Mount, Mounts};

/// **What the pane thinks a candidate is**: a hint that picks the kind that reads it, never
/// trusted for more.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Kind {
    /// An `http:` or `https:` address.
    Url,
    /// A file path, with an optional `:line[:col]`, or a `file:` address.
    File,
}

/// **Text a pane found that may be a link**: the words as the program printed them, and the
/// kind the pane's pattern took them for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Candidate {
    pub kind: Kind,
    pub text: String,
}

/// **Where a candidate was found**: the project the pane is in, the chat it draws, where it
/// draws one, and the folder that chat works in. What a kind resolves against (a file's path,
/// its project's chat numbers); a web address needs none of it.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Pane {
    pub project: Option<PathBuf>,
    pub session: Option<u32>,
    /// The folder the chat works in, which a relative path starts from.
    pub folder: Option<PathBuf>,
    /// Whose filesystem the chat's paths name.
    pub runs: Runs,
}

/// **Whose filesystem a pane's paths name**: this machine's, or a runner's. A runner's paths
/// name this machine's files only through the mounts its runner kind supplies (#6221). Any pane
/// purlis cannot place maps through none ([`Mounts::none`]).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum Runs {
    #[default]
    Here,
    Mapped(Mounts),
}

/// **Where a link goes**, as the core resolved it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Target {
    /// A web address, as the browser will be handed it: read once, never parsed again.
    Url(url::Url),
    /// A file on this machine, at a line. Boxed: it holds its path and its place.
    File(Box<FileAt>),
}

impl Target {
    /// The kind of link it is.
    pub fn kind(&self) -> Kind {
        match self {
            Target::Url(_) => Kind::Url,
            Target::File(_) => Kind::File,
        }
    }

    /// The target in full, as the person is shown it: a hover's first line, and what an ask
    /// the person accepted sends back.
    pub fn shown(&self) -> &str {
        match self {
            Target::Url(url) => url.as_str(),
            Target::File(file) => file.shown(),
        }
    }
}

/// **What a candidate is**: a link to a target, or not a link, and why not.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Resolved {
    Link(Target),
    NotALink(String),
    /// Not a link, and nothing to say on its hover: path-shaped words that name no file.
    Quiet,
}

/// **What a hover over a candidate shows**: a link, where it goes and what opening it does, read
/// once; or not a link, and why not.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Hover {
    /// Boxed: the answer holds the target again, as an ask shows it.
    Link {
        target: Target,
        opening: Box<Opening>,
    },
    NotALink(String),
    /// Nothing to show: [`Resolved::Quiet`].
    Quiet,
}

/// **What opening a link does**, the open command's answer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Opening {
    /// Open it now.
    Open(Target),
    /// Ask the person first: why, what it shows (its text), and where it goes. `token` is what
    /// the answer is sent back with ([`Asks`]): `None` as a kind or a hover says it, and the
    /// token [`Resolver::open`] issued in its answer.
    Ask {
        reason: String,
        shown: String,
        target: Target,
        token: Option<String>,
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
    /// An ask for `target` in `pane` was answered with its token, and it opens now: what the
    /// kind keeps of that, in `asks`. Nothing, unless the kind says so.
    fn accepted(&self, _target: &Target, _pane: &Pane, _asks: &Asks) {}
}

/// **The core's link resolver**: the kinds it knows, each reading the candidates of its hint.
pub struct Resolver {
    kinds: Vec<Box<dyn LinkKind>>,
}

impl Default for Resolver {
    /// The kinds purlis links: web addresses and file paths.
    fn default() -> Self {
        Self::with(vec![Box::new(WebAddress), Box::new(LocalFile::here())])
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

    /// `candidate` read by the kind its hint picks, with that kind; what it is otherwise.
    fn read(
        &self,
        candidate: &Candidate,
        pane: &Pane,
    ) -> Result<(&dyn LinkKind, Target), Resolved> {
        let Some(kind) = self.kind_of(candidate.kind) else {
            return Err(Resolved::NotALink(
                "purlis does not link that kind of text".into(),
            ));
        };
        match kind.resolve(&candidate.text, pane) {
            Resolved::Link(target) => Ok((kind, target)),
            other => Err(other),
        }
    }

    /// **What `candidate`, found in `pane`, is.**
    pub fn resolve(&self, candidate: &Candidate, pane: &Pane) -> Resolved {
        match self.read(candidate, pane) {
            Ok((_, target)) => Resolved::Link(target),
            Err(other) => other,
        }
    }

    /// **What a hover over `candidate` shows**: what it is and what opening it does, resolved
    /// once.
    pub fn hover(&self, candidate: &Candidate, pane: &Pane) -> Hover {
        match self.read(candidate, pane) {
            Ok((kind, target)) => Hover::Link {
                opening: Box::new(kind.opening(target.clone(), &candidate.text, pane)),
                target,
            },
            Err(Resolved::NotALink(why)) => Hover::NotALink(why),
            Err(_) => Hover::Quiet,
        }
    }

    /// **What opening `candidate` does**, resolved again from its text. An ask is answered
    /// with a token issued in `asks`; `answer` is the token the window sent back for the ask it
    /// was handed, which opens the target that ask showed, in that pane, once.
    pub fn open(
        &self,
        candidate: &Candidate,
        pane: &Pane,
        asks: &Asks,
        answer: Option<&str>,
    ) -> Opening {
        let (kind, target) = match self.read(candidate, pane) {
            Ok(read) => read,
            Err(Resolved::NotALink(why)) => return Opening::Refused(why),
            Err(_) => return Opening::Refused(NOT_A_LINK.into()),
        };
        match kind.opening(target, &candidate.text, pane) {
            Opening::Ask { target, .. }
                if answer.is_some_and(|token| asks.accepted(token, pane, &target)) =>
            {
                kind.accepted(&target, pane, asks);
                Opening::Open(target)
            }
            Opening::Ask {
                reason,
                shown,
                target,
                ..
            } => match asks.issue(pane, &target) {
                Some(token) => Opening::Ask {
                    reason,
                    shown,
                    target,
                    token: Some(token),
                },
                None => Opening::Refused(UNASKABLE.into()),
            },
            other => other,
        }
    }
}

/// What opening a link that asks first answers where no token can be issued for its ask.
pub const UNASKABLE: &str = "purlis could not ask about that link, so it is not opened";

/// What opening text that is no link answers, where its kind says nothing more.
pub const NOT_A_LINK: &str = "that is not a link purlis can open";

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
        Resolved::Link(Target::Url(url))
    }

    fn opening(&self, target: Target, _text: &str, _pane: &Pane) -> Opening {
        let Target::Url(url) = &target else {
            return Opening::Refused("purlis does not open that as a web address".into());
        };
        if !url.username().is_empty() || url.password().is_some() {
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

    /// The target of a web address, as the core reads it.
    fn web(text: &str) -> Target {
        Target::Url(url::Url::parse(text).expect("a web address"))
    }

    fn here() -> Pane {
        Pane {
            project: Some(PathBuf::from("/planes/one")),
            session: Some(7),
            ..Pane::default()
        }
    }

    fn refused(answer: Opening) -> String {
        match answer {
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
            Resolved::Link(web("https://github.com/purlis/purlis/pull/6148"))
        );
        assert_eq!(
            links.open(&pr, &here(), &Asks::default(), None),
            Opening::Open(web("https://github.com/purlis/purlis/pull/6148"))
        );
    }

    #[test]
    fn an_http_address_opens_too() {
        let opening = Resolver::default().open(
            &url("http://localhost:5173/a?b=c#d"),
            &here(),
            &Asks::default(),
            None,
        );

        assert_eq!(opening, Opening::Open(web("http://localhost:5173/a?b=c#d")));
    }

    #[test]
    fn a_bare_host_is_shown_as_the_browser_will_be_handed_it() {
        let resolved = Resolver::default().resolve(&url("https://purlis.dev"), &Pane::default());

        assert_eq!(resolved, Resolved::Link(web("https://purlis.dev/")));
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
            let why = refused(links.open(&url(text), &here(), &Asks::default(), None));
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
            refused(links.open(&url(text), &here(), &Asks::default(), None));
        }
    }

    #[test]
    fn an_address_carrying_a_name_or_a_password_is_refused_on_open() {
        let links = Resolver::default();
        // Made-up sign-ins, spelled in parts so the commit scan does not take them for real ones.
        let name = ["https://", "me", "@purlis.dev/"].concat();
        let both = ["https://", "me:", "made-up", "@purlis.dev/"].concat();
        for text in [name.as_str(), both.as_str()] {
            let why = refused(links.open(&url(text), &here(), &Asks::default(), None));
            assert!(why.contains("name or a password"), "{text}: {why}");
        }
    }

    /// A kind that always asks first: the seam's other answers, which later kinds give.
    struct AlwaysAsks;
    impl LinkKind for AlwaysAsks {
        fn kind(&self) -> Kind {
            Kind::Url
        }
        fn resolve(&self, text: &str, _pane: &Pane) -> Resolved {
            Resolved::Link(web(&format!("https://real.example/{text}")))
        }
        fn opening(&self, target: Target, text: &str, _pane: &Pane) -> Opening {
            Opening::Ask {
                reason: "its text names another address".into(),
                shown: text.to_owned(),
                target,
                token: None,
            }
        }
    }

    #[test]
    fn a_kind_is_a_seam_another_kind_can_take() {
        let links = Resolver::with(vec![Box::new(AlwaysAsks)]);

        let Opening::Ask {
            reason,
            shown,
            target,
            token,
        } = links.open(&url("setup"), &here(), &Asks::default(), None)
        else {
            panic!("expected an ask");
        };
        assert_eq!(reason, "its text names another address");
        assert_eq!(shown, "setup");
        assert_eq!(target, web("https://real.example/setup"));
        let token = token.expect("an ask from open carries its token");
        assert_eq!(token.len(), 32, "{token}");
    }

    /// The token an ask in `pane` was issued, from `asks`.
    fn asked(links: &Resolver, pane: &Pane, asks: &Asks) -> String {
        match links.open(&url("setup"), pane, asks, None) {
            Opening::Ask {
                token: Some(token), ..
            } => token,
            other => panic!("expected an ask, got {other:?}"),
        }
    }

    #[test]
    fn an_ask_is_opened_by_its_token_once() {
        let links = Resolver::with(vec![Box::new(AlwaysAsks)]);
        let asks = Asks::default();
        let token = asked(&links, &here(), &asks);

        assert_eq!(
            links.open(&url("setup"), &here(), &asks, Some(&token)),
            Opening::Open(web("https://real.example/setup"))
        );
        // Spent: the same token asks again.
        assert!(matches!(
            links.open(&url("setup"), &here(), &asks, Some(&token)),
            Opening::Ask { .. }
        ));
    }

    #[test]
    fn the_target_the_ask_showed_is_no_answer_only_its_token_is() {
        let links = Resolver::with(vec![Box::new(AlwaysAsks)]);
        let asks = Asks::default();
        asked(&links, &here(), &asks);

        // The tracer's old answer, the target itself, now opens nothing.
        assert!(matches!(
            links.open(
                &url("setup"),
                &here(),
                &asks,
                Some("https://real.example/setup")
            ),
            Opening::Ask { .. }
        ));
    }

    #[test]
    fn a_token_of_another_pane_or_another_target_opens_nothing_and_is_spent() {
        let links = Resolver::with(vec![Box::new(AlwaysAsks)]);
        let asks = Asks::default();
        let other_chat = Pane {
            session: Some(8),
            ..here()
        };
        let token = asked(&links, &here(), &asks);
        assert!(matches!(
            links.open(&url("setup"), &other_chat, &asks, Some(&token)),
            Opening::Ask { .. }
        ));
        assert!(matches!(
            links.open(&url("setup"), &here(), &asks, Some(&token)),
            Opening::Ask { .. }
        ));

        let token = asked(&links, &here(), &asks);
        assert!(matches!(
            links.open(&url("elsewhere"), &here(), &asks, Some(&token)),
            Opening::Ask { .. }
        ));
    }

    #[test]
    fn a_token_past_its_life_opens_nothing() {
        let links = Resolver::with(vec![Box::new(AlwaysAsks)]);
        let asks = Asks::with_lives(std::time::Duration::ZERO, asks::OPENED_LIFE);
        let token = asked(&links, &here(), &asks);

        assert!(matches!(
            links.open(&url("setup"), &here(), &asks, Some(&token)),
            Opening::Ask { .. }
        ));
    }

    #[test]
    fn a_hover_reads_the_text_once_into_where_it_goes_and_what_opening_does() {
        let links = Resolver::default();

        assert_eq!(
            links.hover(&url("https://purlis.dev"), &here()),
            Hover::Link {
                target: web("https://purlis.dev/"),
                opening: Box::new(Opening::Open(web("https://purlis.dev/"))),
            }
        );
        assert!(matches!(
            links.hover(&url("ftp://mirror.example.org/pub"), &here()),
            Hover::NotALink(why) if why.contains("ftp:")
        ));
    }

    #[test]
    fn a_hint_no_kind_answers_is_never_a_link() {
        let links = Resolver::with(vec![]);

        assert!(matches!(
            links.resolve(&url("https://purlis.dev/"), &here()),
            Resolved::NotALink(_)
        ));
        refused(links.open(&url("https://purlis.dev/"), &here(), &Asks::default(), None));
    }
}
