// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use std::path::{Path, PathBuf};

use super::super::{
    Asking, Asks, Candidate, Hover, Kind, LocalFile, Opening, Pane, Resolved, Resolver, Runs,
    Target, WebAddress,
};
use super::{MOST_TARGET, MOST_TEXT};

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
    for text in [
        "Click here",
        "the docs",
        "PR 12",
        "here.",
        "(see this!)",
        "Go -> docs",
        "Ｃｌｉｃｋ",
    ] {
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
        "github.com/purlis/purlis/pull/1",
        "https://github.com/purlis/purlis/pull/1",
        "GitHub.com/purlis/purlis/pull/1/",
        "github.com",
        "github.com.",
        "See github.com/purlis/purlis/pull/1 now",
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
        ("Open https:purlis.dev", "purlis.dev"),
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
        ("mailto:someone", "mailto:"),
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

/// The reason the ask for `text` marked as a link to `target` gives, or a panic saying it opened.
fn asks_saying(text: &str, target: &str) -> String {
    match links().open(&marked(text, target), &here(), &Asks::default(), None) {
        Opening::Ask {
            reason,
            about: Asking::Misleading,
            ..
        } => reason,
        other => panic!("{text:?} → {target}: expected the mismatch ask, got {other:?}"),
    }
}

/// Whether `text` marked as a link to `target` opens without asking.
fn opens(text: &str, target: &str) -> bool {
    matches!(
        links().open(&marked(text, target), &here(), &Asks::default(), None),
        Opening::Open(_)
    )
}

#[test]
fn a_name_names_a_web_address_by_its_host_alone_never_its_path() {
    assert_eq!(
        asks_saying("paypal.com", "https://evil.example/paypal.com"),
        "its text says paypal.com"
    );
    // A file's name a browser reads as a host asks: one click.
    assert_eq!(
        asks_saying(
            "README.md",
            "https://github.com/purlis/purlis/blob/main/README.md"
        ),
        "its text says readme.md"
    );
    assert!(opens("purlis.dev", "https://purlis.dev/docs/setup"));
}

#[test]
fn characters_that_draw_nothing_are_read_as_the_reader_sees_the_words() {
    for hidden in [
        '\u{200B}', '\u{200C}', '\u{200D}', '\u{00AD}', '\u{2060}', '\u{2066}', '\u{2068}',
        '\u{2069}', '\u{202E}', '\u{FEFF}',
    ] {
        let text = format!("pay{hidden}pal.com");
        assert_eq!(
            asks_saying(&text, "https://evil.example/"),
            "its text says paypal.com",
            "U+{:04X}",
            u32::from(hidden)
        );
        let around = format!("{hidden}paypal.com{hidden}");
        assert_eq!(
            asks_saying(&around, "https://evil.example/"),
            "its text says paypal.com",
            "U+{:04X} around",
            u32::from(hidden)
        );
    }
}

#[test]
fn look_alike_dots_and_wide_letters_are_read_as_the_dots_and_letters_they_look_like() {
    for dot in ['\u{3002}', '\u{FF0E}', '\u{FF61}', '\u{FE52}', '\u{2024}'] {
        let text = format!("paypal{dot}com");
        assert_eq!(
            asks_saying(&text, "https://evil.example/"),
            "its text says paypal.com",
            "U+{:04X}",
            u32::from(dot)
        );
    }
    assert_eq!(
        asks_saying("ｐａｙｐａｌ．ｃｏｍ", "https://evil.example/"),
        "its text says paypal.com"
    );
    // Folded, the same host is no mismatch.
    assert!(opens("ｐｕｒｌｉｓ．ｄｅｖ", "https://purlis.dev/"));
}

#[test]
fn every_wrapper_a_sentence_puts_around_a_word_is_taken_off() {
    for text in [
        "*paypal.com*",
        "**paypal.com**",
        "_paypal.com_",
        "—paypal.com—",
        "paypal.com…",
        "…paypal.com",
        "「paypal.com」",
        "『paypal.com』",
        "paypal.com's login",
        "paypal.com’s login",
        "“paypal.com”",
        "‘paypal.com’",
        "«paypal.com»",
        "(paypal.com)",
        "[paypal.com]",
        "<paypal.com>",
        "\"paypal.com\"",
        "`paypal.com`",
        "paypal.com.",
        ".paypal.com",
        "¡paypal.com!",
        "paypal.com?",
        "paypal.com—log in",
    ] {
        assert!(
            asks_about_its_words(text, "https://evil.example/"),
            "{text}"
        );
    }
    // ASCII wrappers come off, so the host itself is no mismatch.
    for text in [
        "(paypal.com)",
        "*paypal.com*",
        "\"paypal.com\"",
        "paypal.com.",
        "<paypal.com>",
    ] {
        assert!(opens(text, "https://paypal.com/"), "{text}");
    }
}

#[test]
fn another_scheme_asks_and_none_shown_is_the_targets() {
    assert_eq!(
        asks_saying("https://purlis.dev/login", "http://purlis.dev/login"),
        "its text says https://purlis.dev/login"
    );
    assert_eq!(
        asks_saying("http://purlis.dev/", "https://purlis.dev/"),
        "its text says http://purlis.dev/"
    );
    assert!(opens("purlis.dev/login", "http://purlis.dev/login"));
    assert!(opens("localhost:3000", "http://localhost:3000/"));
}

#[test]
fn another_port_asks() {
    assert_eq!(
        asks_saying("localhost.test:5173/x", "http://localhost.test:3000/x"),
        "its text says localhost.test:5173/x"
    );
    assert_eq!(
        asks_saying("https://purlis.dev/", "https://purlis.dev:8443/"),
        "its text says https://purlis.dev/"
    );
    assert_eq!(
        asks_saying("purlis.dev", "https://purlis.dev:8443/"),
        "its text says purlis.dev"
    );
    assert!(opens("https://purlis.dev:443/", "https://purlis.dev/"));
    assert!(opens("purlis.dev:8443/a", "https://purlis.dev:8443/a"));
}

#[test]
fn a_name_before_an_at_asks_for_the_reader_takes_it_for_the_host() {
    for text in [
        "https://paypal.com@evil.example/",
        "paypal.com@evil.example",
        "paypal.com@evil.example/login",
    ] {
        assert_eq!(
            asks_saying(text, "https://evil.example/login"),
            format!("its text says {text}")
        );
    }
}

#[test]
fn an_email_address_names_its_domain() {
    assert_eq!(
        // Made-up addresses, spelled in parts so the commit scan does not take them for real ones.
        asks_saying(
            &["Write to help", "@paypal.com"].concat(),
            "https://evil.example/"
        ),
        "its text says paypal.com"
    );
    // A name before the host is shown, so it must be the target's too.
    assert!(asks_about_its_words(
        &["help", "@purlis.dev"].concat(),
        "https://purlis.dev/contact"
    ));
}

#[test]
fn a_target_past_the_most_is_refused_unread_and_long_words_ask() {
    let links = links();
    let long = format!("https://purlis.dev/{}", "a".repeat(MOST_TARGET));
    let why = refused(links.open(
        &marked("Click here", &long),
        &here(),
        &Asks::default(),
        None,
    ));
    assert!(why.contains("longer than purlis reads"), "{why}");
    let just = format!(
        "https://purlis.dev/{}",
        "a".repeat(MOST_TARGET - "https://purlis.dev/".len())
    );
    assert!(opens("Click here", &just));

    let text = "word ".repeat(MOST_TEXT);
    assert!(matches!(
        links.open(
            &marked(&text, "https://purlis.dev/"),
            &here(),
            &Asks::default(),
            None
        ),
        Opening::Ask {
            about: Asking::Misleading,
            ..
        }
    ));
}

#[test]
fn a_file_address_in_capitals_on_another_host_is_refused() {
    let why = refused(links().open(
        &marked("README.md", "FILE://build-box/home/me/README.md"),
        &here(),
        &Asks::default(),
        None,
    ));
    assert!(why.contains("build-box"), "{why}");
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

/// Whether opening `text` marked as a link to `target` asks first about its words.
fn asks_about_its_words(text: &str, target: &str) -> bool {
    matches!(
        links().open(&marked(text, target), &here(), &Asks::default(), None),
        Opening::Ask {
            about: Asking::Misleading,
            ..
        }
    )
}

#[test]
fn a_character_that_may_stand_for_a_dot_inside_a_host_asks() {
    for dot in [
        '\u{A4F8}',
        '\u{0702}',
        '\u{10101}',
        '\u{2E33}',
        '\u{0589}',
        '\u{06D4}',
        '\u{00B7}',
        '\u{2027}',
        '\u{22C5}',
        '\u{1D16D}',
        '\u{30FB}',
        '\u{2219}',
    ] {
        let text = format!("paypal{dot}com");
        assert!(
            asks_about_its_words(&text, "https://evil.example/"),
            "U+{:04X}",
            u32::from(dot)
        );
        // And so it does where the target is the host it seems to name: its reading is uncertain.
        assert!(
            asks_about_its_words(&text, "https://paypal.com/"),
            "U+{:04X} to the same host",
            u32::from(dot)
        );
    }
}

#[test]
fn a_mark_drawn_over_a_host_asks() {
    for over in [
        '\u{0332}', '\u{0335}', '\u{0336}', '\u{0338}', '\u{0347}', '\u{0359}', '\u{20D2}',
        '\u{20E8}', '\u{0489}', '\u{1AB0}',
    ] {
        for text in [
            format!("paypal.com{over}"),
            format!("pay{over}pal.com"),
            format!("paypal.{over}com"),
        ] {
            assert!(
                asks_about_its_words(&text, "https://evil.example/"),
                "U+{:04X} in {text:?}",
                u32::from(over)
            );
            assert!(
                asks_about_its_words(&text, "https://paypal.com/"),
                "U+{:04X} in {text:?} to the same host",
                u32::from(over)
            );
        }
    }
}

#[test]
fn a_single_name_with_a_port_and_localhost_are_hosts() {
    assert_eq!(
        asks_saying("localhost:3000", "http://localhost:9999/"),
        "its text says localhost:3000"
    );
    assert_eq!(
        asks_saying("localhost", "https://evil.example/"),
        "its text says localhost"
    );
    assert_eq!(
        asks_saying("dev:3000", "http://evil.example:3000/"),
        "its text says dev"
    );
    assert!(opens("localhost:3000", "http://localhost:3000/"));
    assert!(opens("localhost", "http://localhost/"));
    assert!(opens("(localhost)", "http://localhost/"));
    assert!(asks_about_its_words("*localhost*", "https://evil.example/"));
    // A time reads as a host and port to a browser, so it asks: one click.
    assert!(asks_about_its_words("12:30", "https://purlis.dev/"));
}

#[test]
fn an_ip_address_is_a_host_and_port() {
    for text in ["127.0.0.1", "127.0.0.1:8080", "[::1]", "[::1]:8080"] {
        assert!(
            asks_about_its_words(text, "https://evil.example/"),
            "{text}"
        );
    }
    assert_eq!(
        asks_saying("127.0.0.1:8080", "http://127.0.0.1:9090/"),
        "its text says 127.0.0.1:8080"
    );
    assert!(opens("127.0.0.1:8080", "http://127.0.0.1:8080/"));
    assert!(opens("[::1]:8080", "http://[::1]:8080/"));
}

#[test]
fn an_international_last_label_is_a_host() {
    assert_eq!(
        asks_saying("paypal.xn--p1ai", "https://evil.example/"),
        "its text says paypal.xn--p1ai"
    );
    assert!(opens("purlis.xn--p1ai", "https://purlis.xn--p1ai/"));
}

#[test]
fn a_word_outside_ascii_asks_unless_it_names_the_target() {
    // Plain means ASCII: any other letter may be one a browser reads as a host's.
    for text in ["Нажмите здесь", "naïve café", "日本語のドキュメント"] {
        assert!(
            asks_about_its_words(text, "https://purlis.dev/docs"),
            "{text}"
        );
    }
    assert!(opens("Cliquez ici", "https://purlis.dev/docs"));
}

#[test]
fn a_word_is_read_as_a_browser_reads_an_address() {
    // Every one of these is the loopback address, or no address purlis.dev is.
    for text in [
        "127.1",
        "0x7f.1",
        "0177.0.0.1",
        "%70aypal.com",
        "paypal%2ecom",
        "paypal.com\\@purlis.dev",
        "https:paypal.com",
        "https:/paypal.com",
        "https:\\\\paypal.com",
        "HTTPS://PAYPAL.COM",
        "paypal.com#purlis.dev",
        "paypal.com?purlis.dev",
        "paypal.com;purlis.dev",
        "paypal.com\\purlis.dev",
    ] {
        assert!(asks_about_its_words(text, "https://purlis.dev/"), "{text}");
    }
    assert!(opens("127.1", "http://127.0.0.1/"));
    assert!(opens("0x7f.1:8080", "http://127.0.0.1:8080/"));
    assert!(opens("https:purlis.dev", "https://purlis.dev/"));
}

#[test]
fn another_path_on_the_same_host_asks() {
    assert_eq!(
        asks_saying("github.com/purlis", "https://github.com/attacker"),
        "its text says github.com/purlis"
    );
    for (text, target) in [
        ("github.com/purlis", "https://github.com/purlis/purlis"),
        ("github.com/purlis?tab=repos", "https://github.com/purlis"),
        ("github.com/purlis#top", "https://github.com/purlis#bottom"),
        ("github.com/%70urlis", "https://github.com/purlis"),
    ] {
        assert!(asks_about_its_words(text, target), "{text} → {target}");
    }
    assert!(opens("github.com/purlis/", "https://github.com/purlis"));
    assert!(opens(
        "github.com/purlis?tab=repos",
        "https://github.com/purlis?tab=repos"
    ));
    // A host alone names no path, so any path on it opens.
    assert!(opens("github.com", "https://github.com/purlis/purlis"));
}

#[test]
fn an_international_name_and_www_are_the_host_they_name() {
    assert!(opens("bücher.example", "https://xn--bcher-kva.example/"));
    assert!(opens("xn--bcher-kva.example", "https://bücher.example/"));
    assert!(opens("www.purlis.dev", "https://purlis.dev/"));
    assert!(opens("purlis.dev", "https://www.purlis.dev/"));
    assert!(asks_about_its_words(
        "bucher.example",
        "https://bücher.example/"
    ));
    assert!(asks_about_its_words(
        "www2.purlis.dev",
        "https://purlis.dev/"
    ));
}
