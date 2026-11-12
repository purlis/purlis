// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use std::path::{Path, PathBuf};

use super::super::{
    Asking, Asks, Candidate, Hover, Kind, LocalFile, Opening, Pane, Resolved, Resolver, Runs,
    Target, WebAddress,
};

/// A link a program marked (OSC 8): the words it printed, and the target it named.
fn marked(text: &str, target: &str) -> Candidate {
    Candidate {
        kind: Kind::Osc8,
        text: text.to_owned(),
        target: Some(target.to_owned()),
    }
}

fn web(text: &str) -> Target {
    Target::Url(url::Url::parse(text).expect("a web address"))
}

/// A project with a workspace `smart-ide` holding `README.md` and `src/main.rs`, and a file
/// outside it.
struct Fixture {
    _held: tempfile::TempDir,
    project: PathBuf,
    outside: PathBuf,
}

fn write(path: &Path) {
    std::fs::create_dir_all(path.parent().expect("a parent")).expect("folders");
    std::fs::write(path, "text\n").expect("written");
}

fn fixture() -> Fixture {
    let held = tempfile::tempdir().expect("a temporary folder");
    let top = std::fs::canonicalize(held.path()).expect("resolved");
    let project = top.join("project");
    write(&project.join("workspaces/smart-ide/README.md"));
    write(&project.join("workspaces/smart-ide/src/main.rs"));
    let outside = top.join("etc/hosts");
    write(&outside);
    Fixture {
        _held: held,
        project,
        outside,
    }
}

impl Fixture {
    fn ws(&self) -> PathBuf {
        self.project.join("workspaces/smart-ide")
    }

    fn chat(&self) -> Pane {
        Pane {
            project: Some(self.project.clone()),
            session: Some(7),
            folder: Some(self.ws()),
            runs: Runs::Here,
        }
    }

    fn url_of(&self, path: &Path) -> String {
        url::Url::from_file_path(path)
            .expect("a file address")
            .to_string()
    }
}

fn links() -> Resolver {
    Resolver::with(vec![
        Box::new(WebAddress),
        Box::new(LocalFile::with(None, Vec::new())),
    ])
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

/// The ask opening `candidate` answers, with its token.
fn ask(links: &Resolver, candidate: &Candidate, pane: &Pane, asks: &Asks) -> Opening {
    match links.open(candidate, pane, asks, None) {
        ask @ Opening::Ask { .. } => ask,
        other => panic!("expected an ask, got {other:?}"),
    }
}

#[test]
fn text_that_is_its_own_address_opens_by_the_web_address_rule() {
    let links = links();
    let pr = marked(
        "https://github.com/purlis/purlis/pull/6218",
        "https://github.com/purlis/purlis/pull/6218",
    );

    assert_eq!(
        links.open(&pr, &here(), &Asks::default(), None),
        Opening::Open(web("https://github.com/purlis/purlis/pull/6218"))
    );
    assert_eq!(
        links.hover(&pr, &here()),
        Hover::Link {
            target: web("https://github.com/purlis/purlis/pull/6218"),
            opening: Box::new(Opening::Open(web(
                "https://github.com/purlis/purlis/pull/6218"
            ))),
        }
    );
}

#[test]
fn plain_words_open_their_target_as_a_found_link_does() {
    let links = links();
    for text in ["Click here", "the docs", "PR 12", "here", "v1.2", "e.g."] {
        assert_eq!(
            links.open(
                &marked(text, "https://purlis.dev/docs"),
                &here(),
                &Asks::default(),
                None
            ),
            Opening::Open(web("https://purlis.dev/docs")),
            "{text}"
        );
    }
}

#[test]
fn an_address_of_the_same_host_is_no_mismatch() {
    let links = links();
    for text in [
        "github.com/purlis",
        "https://github.com/purlis",
        "GitHub.com/purlis/purlis",
        "github.com",
    ] {
        assert_eq!(
            links.open(
                &marked(text, "https://github.com/purlis/purlis/pull/1"),
                &here(),
                &Asks::default(),
                None
            ),
            Opening::Open(web("https://github.com/purlis/purlis/pull/1")),
            "{text}"
        );
    }
}

#[test]
fn text_naming_another_host_asks_showing_both_the_text_and_the_target() {
    let links = links();
    let asks = Asks::default();
    let link = marked(
        "docs.purlis.dev/setup",
        "https://purlis-setup.example.net/s?id=7",
    );

    let Opening::Ask {
        reason,
        shown,
        target,
        token,
        about,
    } = ask(&links, &link, &here(), &asks)
    else {
        unreachable!()
    };
    assert_eq!(reason, "its text says docs.purlis.dev");
    assert_eq!(shown, "docs.purlis.dev/setup");
    assert_eq!(target, web("https://purlis-setup.example.net/s?id=7"));
    assert_eq!(about, Asking::Misleading);
    let token = token.expect("an ask from open carries its token");

    // A hover says it asks, and why, with no token.
    assert!(matches!(
        links.hover(&link, &here()),
        Hover::Link { opening, .. } if matches!(
            &*opening,
            Opening::Ask { about: Asking::Misleading, token: None, .. }
        )
    ));

    // The target is no answer; its token is, once.
    assert!(matches!(
        links.open(
            &link,
            &here(),
            &asks,
            Some("https://purlis-setup.example.net/s?id=7")
        ),
        Opening::Ask { .. }
    ));
    let Opening::Ask {
        token: Some(token_again),
        ..
    } = ask(&links, &link, &here(), &asks)
    else {
        unreachable!()
    };
    assert_ne!(token, token_again);
    assert_eq!(
        links.open(&link, &here(), &asks, Some(&token_again)),
        Opening::Open(web("https://purlis-setup.example.net/s?id=7"))
    );
    assert!(matches!(
        links.open(&link, &here(), &asks, Some(&token_again)),
        Opening::Ask { .. }
    ));
}

#[test]
fn an_address_anywhere_in_the_text_naming_another_host_asks() {
    let links = links();
    for (text, says) in [
        ("see https://github.com/login now", "github.com"),
        ("(github.com/login)", "github.com"),
        ("Open www.purlis.dev.", "www.purlis.dev"),
        ("http://localhost:5173", "localhost"),
    ] {
        let Opening::Ask { reason, about, .. } = ask(
            &links,
            &marked(text, "https://evil.example/login"),
            &here(),
            &Asks::default(),
        ) else {
            unreachable!()
        };
        assert_eq!(reason, format!("its text says {says}"), "{text}");
        assert_eq!(about, Asking::Misleading);
    }
}

#[test]
fn every_other_scheme_is_refused_naming_it_and_says_so_on_hover() {
    let links = links();
    for (target, scheme) in [
        ("javascript:alert(1)", "javascript:"),
        ("vscode://file/etc/hosts", "vscode:"),
        ("ftp://mirror.example.org/pub", "ftp:"),
        ("mailto:someone@example.org", "mailto:"),
        ("ssh://host/x", "ssh:"),
    ] {
        let link = marked("Click here", target);
        let why = refused(links.open(&link, &here(), &Asks::default(), None));
        assert!(why.contains(scheme), "{target}: {why}");
        assert!(
            matches!(links.hover(&link, &here()), Hover::NotALink(said) if said == why),
            "{target}"
        );
    }
}

#[test]
fn a_mark_with_no_target_or_an_unreadable_one_is_refused() {
    let links = links();
    let none = Candidate {
        kind: Kind::Osc8,
        text: "Click here".into(),
        target: None,
    };
    refused(links.open(&none, &here(), &Asks::default(), None));
    for target in ["", "not an address", "https://pur lis.dev/", "https://"] {
        refused(links.open(&marked("x", target), &here(), &Asks::default(), None));
    }
}

#[test]
fn a_mismatch_over_a_target_the_kind_refuses_stays_refused() {
    let links = links();
    let both = ["https://", "me:", "made-up", "@evil.example/"].concat();
    let why = refused(links.open(
        &marked("github.com", &both),
        &here(),
        &Asks::default(),
        None,
    ));
    assert!(why.contains("name or a password"), "{why}");
}

#[test]
fn a_file_address_on_another_host_is_refused() {
    let links = links();
    let link = marked("README.md", "file://build-box/home/me/README.md");

    let why = refused(links.open(&link, &here(), &Asks::default(), None));
    assert!(why.contains("build-box"), "{why}");
}

#[test]
fn a_file_address_on_this_machine_follows_the_file_rules() {
    let here = fixture();
    let links = links();
    let readme = here.ws().join("README.md");
    for address in [
        here.url_of(&readme),
        here.url_of(&readme)
            .replacen("file://", "file://localhost", 1),
    ] {
        // `ls --hyperlink` prints a file's name and marks its address.
        let Opening::Open(Target::File(file)) = links.open(
            &marked("README.md", &address),
            &here.chat(),
            &Asks::default(),
            None,
        ) else {
            panic!("{address}: expected the file to open");
        };
        assert_eq!(file.path, readme);
    }

    // A file outside the workspace asks, as a found one does.
    let Opening::Ask { about, .. } = ask(
        &links,
        &marked("hosts", &here.url_of(&here.outside)),
        &here.chat(),
        &Asks::default(),
    ) else {
        unreachable!()
    };
    assert_eq!(about, Asking::FileOutside);
}

#[test]
fn a_path_naming_the_same_file_is_no_mismatch_and_another_file_asks() {
    let here = fixture();
    let links = links();
    let main = here.url_of(&here.ws().join("src/main.rs"));
    for text in [
        "src/main.rs",
        "./src/main.rs:12",
        "main.rs",
        &here.ws().join("src/main.rs").display().to_string(),
    ] {
        assert!(
            matches!(
                links.open(&marked(text, &main), &here.chat(), &Asks::default(), None),
                Opening::Open(Target::File(_))
            ),
            "{text}"
        );
    }
    for text in [
        "/etc/passwd",
        "src/lib.rs",
        "README.md",
        "https://github.com/x",
    ] {
        let Opening::Ask {
            reason,
            shown,
            about,
            ..
        } = ask(&links, &marked(text, &main), &here.chat(), &Asks::default())
        else {
            unreachable!()
        };
        assert_eq!(about, Asking::Misleading, "{text}");
        assert_eq!(shown, text);
        assert!(reason.starts_with("its text says "), "{text}: {reason}");
    }
}

#[test]
fn a_web_address_named_by_a_file_s_name_is_no_mismatch() {
    let links = links();
    let blob = "https://github.com/purlis/purlis/blob/main/README.md";

    assert_eq!(
        links.open(&marked("README.md", blob), &here(), &Asks::default(), None),
        Opening::Open(web(blob))
    );
}

#[test]
fn a_missing_file_says_so_rather_than_nothing() {
    let here = fixture();
    let links = links();
    let gone = here.url_of(&here.ws().join("gone.rs"));
    let link = marked("gone.rs", &gone);

    assert!(matches!(
        links.hover(&link, &here.chat()),
        Hover::NotALink(_)
    ));
    refused(links.open(&link, &here.chat(), &Asks::default(), None));
    assert!(matches!(
        links.resolve(&link, &here.chat()),
        Resolved::NotALink(_)
    ));
}
