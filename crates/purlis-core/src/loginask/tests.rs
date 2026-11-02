// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! What a suggested login may be (#4595): the origin the person confirms is exactly the one
//! filled, a marked key is never offered, and Always writes one line and nothing else.

use std::path::Path;

use super::*;

fn suggested(site: &str) -> Suggested {
    Suggested {
        site: site.to_owned(),
        vault: "qa".to_owned(),
        user_key: Some("username".to_owned()),
        key: "password".to_owned(),
    }
}

/// A project whose file is `text`.
fn project(text: &str) -> tempfile::TempDir {
    let dir = tempfile::tempdir().expect("a folder");
    std::fs::write(crate::names::manifest(dir.path()), text).expect("written");
    dir
}

fn proposed(root: &Path, site: &str) -> Proposal {
    proposal(root, "qa", "shop-login", &suggested(site), &[]).expect("a proposal")
}

#[test]
fn a_site_is_the_origin_and_the_path_of_what_was_suggested() {
    for (site, origin, page) in [
        (
            "https://shop.example.test/login",
            "https://shop.example.test",
            "https://shop.example.test/login",
        ),
        (
            "shop.example.test/login",
            "https://shop.example.test",
            "https://shop.example.test/login",
        ),
        (
            "HTTPS://Shop.Example.test",
            "https://shop.example.test",
            "https://shop.example.test/",
        ),
        (
            "https://shop.example.test:443/a",
            "https://shop.example.test",
            "https://shop.example.test/a",
        ),
        (
            "https://shop.example.test:8443/a",
            "https://shop.example.test:8443",
            "https://shop.example.test:8443/a",
        ),
    ] {
        assert_eq!(
            site_of(site),
            Ok((origin.to_owned(), page.to_owned())),
            "{site}"
        );
    }
}

#[test]
fn a_query_and_a_fragment_never_reach_the_page() {
    let (_, page) = site_of("https://shop.example.test/login?next=https://evil.test#x").unwrap();
    assert_eq!(page, "https://shop.example.test/login");
}

#[test]
fn only_https_is_offered_on_a_named_site() {
    for site in [
        "http://shop.example.test/login",
        "http://localhost/login",
        "http://127.0.0.1/login",
        "ftp://shop.example.test/",
        "javascript://shop.example.test/",
        "data://shop.example.test/",
    ] {
        assert!(site_of(site).is_err(), "{site}");
    }
}

#[test]
fn a_host_that_cant_be_confirmed_by_reading_it_is_refused() {
    for site in [
        "https://localhost/login",
        "https://dev.localhost/login",
        "https://printer.local/login",
        "https://git.corp.internal/login",
        "https://router.lan/login",
        "https://intranet/login",
        "https://127.0.0.1/login",
        "https://2130706433/login",
        "https://0x7f000001/login",
        "https://0x7f.0.0.1/login",
        "https://1.1/login",
        "https://[::1]/login",
        "https://example.test./login",
    ] {
        assert!(site_of(site).is_err(), "{site}");
    }
}

#[test]
fn nothing_a_browser_reads_differently_passes() {
    for site in [
        "https://shop.example.test@evil.test/",
        "https://evil.test\\@shop.example.test/",
        "https://evil.test\\.shop.example.test/",
        "https://shop.example.test%2f@evil.test/",
        "https://evil.test%00.shop.example.test/",
        "https://sho\tp.example.test/",
        "https://shop.example.test/lo\ngin",
        "https://shop.example.test/%2e%2e/login",
        "https://",
        "",
    ] {
        assert!(site_of(site).is_err(), "{site:?}");
    }
}

#[test]
fn a_name_in_another_alphabet_is_filled_as_a_browser_spells_it_and_said() {
    let dir = project("");
    // Cyrillic а, in place of the Latin one.
    let one = proposed(dir.path(), "https://pаypal.example.test/login");
    assert!(one.origin.starts_with("https://xn--"), "{}", one.origin);
    assert!(one.page.starts_with(&one.origin));
    assert!(
        one.warnings
            .iter()
            .any(|warning| matches!(warning, Warning::OtherAlphabet { .. })),
        "{one:?}"
    );
    // Fullwidth dots and letters come to the same ASCII spelling, and are said as any other.
    let (origin, _) = site_of("https://shop。example。test/").expect("its ASCII spelling");
    assert_eq!(origin, "https://shop.example.test");
}

#[test]
fn a_port_of_its_own_is_said() {
    let dir = project("");
    let one = proposed(dir.path(), "https://shop.example.test:8443/");
    assert_eq!(one.warnings, vec![Warning::OtherPort { port: 8443 }]);
}

#[test]
fn a_key_a_mint_line_names_is_never_offered_as_password_or_user() {
    let dir = project(
        "[sandbox.personas.dev.tools.gh]\nvault = \"qa\"\nkey = \"password\"\nmint = \
         \"github-app\"\napp = \"Iv1.abc\"\n",
    );
    let refused = proposal(
        dir.path(),
        "qa",
        "shop-login",
        &suggested("https://a.test.example/"),
        &[],
    )
    .expect_err("refused");
    assert!(refused.contains("mints"), "{refused}");
    let user = Suggested {
        user_key: Some("password".to_owned()),
        key: "other".to_owned(),
        ..suggested("https://a.test.example/")
    };
    assert!(proposal(dir.path(), "qa", "shop-login", &user, &[]).is_err());
}

#[test]
fn no_key_is_offered_while_the_file_cant_be_read_to_tell() {
    let dir = project("not [ toml");
    let refused = proposal(
        dir.path(),
        "qa",
        "shop-login",
        &suggested("https://a.test.example/"),
        &[],
    )
    .expect_err("refused");
    assert!(refused.contains("cannot be read"), "{refused}");
}

#[test]
fn a_password_key_on_another_site_is_said_and_its_own_site_is_not() {
    let dir = project(
        "[sandbox.personas.ops.tools]\nold = { vault = \"qa\", browser = \
         \"https://old.example.test/login\", key = \"password\" }\n",
    );
    let other = proposed(dir.path(), "https://new.example.test/");
    assert_eq!(
        other.warnings,
        vec![Warning::KeyElsewhere {
            origins: vec!["https://old.example.test".to_owned()]
        }]
    );
    assert!(
        other.warnings[0]
            .said()
            .contains("https://old.example.test")
    );
    let same = proposed(dir.path(), "https://old.example.test/signin");
    assert!(same.warnings.is_empty(), "{same:?}");
}

#[test]
fn a_password_named_as_a_user_name_or_in_another_case_is_said_too() {
    let dir = project(
        "[sandbox.personas.ops.tools]\nold = { vault = \"qa\", browser = \
         \"https://old.example.test/login\", key = \"password\" }\n",
    );
    for (user, key) in [("password", "dummy"), ("name", "PASSWORD")] {
        let spelled = Suggested {
            user_key: Some(user.to_owned()),
            key: key.to_owned(),
            ..suggested("https://new.example.test/")
        };
        let one = proposal(dir.path(), "qa", "shop-login", &spelled, &[]).expect("a proposal");
        assert!(
            one.warnings
                .iter()
                .any(|warning| matches!(warning, Warning::KeyElsewhere { .. })),
            "{one:?}"
        );
    }
}

#[test]
fn a_login_allowed_for_now_counts_as_a_site_the_key_is_on() {
    let dir = project("");
    let held = proposed(dir.path(), "https://held.example.test/")
        .tool()
        .expect("a tool");
    let one = proposal(
        dir.path(),
        "qa",
        "other",
        &suggested("https://new.example.test/"),
        std::slice::from_ref(&held),
    )
    .expect("a proposal");
    assert!(matches!(
        one.warnings.as_slice(),
        [Warning::KeyElsewhere { .. }]
    ));
}

#[test]
fn a_marked_key_is_refused_however_a_chat_spells_it() {
    let dir = project(
        "[sandbox.personas.dev.tools.gh]\nvault = \"qa\"\nkey = \"app-key\"\nmint = \
         \"github-app\"\napp = \"Iv1.abc\"\n",
    );
    for key in ["APP-KEY", "App-Key", "section/app-key", "app-key "] {
        let spelled = Suggested {
            key: key.to_owned(),
            ..suggested("https://a.test.example/")
        };
        assert!(
            proposal(dir.path(), "qa", "shop-login", &spelled, &[]).is_err(),
            "{key}"
        );
    }
}

#[test]
fn the_same_key_twice_is_refused() {
    let dir = project("");
    let twice = Suggested {
        user_key: Some("password".to_owned()),
        ..suggested("https://a.test.example/")
    };
    assert!(proposal(dir.path(), "qa", "shop-login", &twice, &[]).is_err());
}

#[test]
fn a_page_no_login_is_usually_on_is_said_and_shown_whole() {
    let dir = project("");
    let one = proposed(dir.path(), "https://wiki.example.test/~mallory/notes.html");
    assert_eq!(
        one.warnings,
        vec![Warning::ChatsPage {
            path: "/~mallory/notes.html".to_owned()
        }]
    );
    assert!(
        proposed(dir.path(), "https://wiki.example.test/login/")
            .warnings
            .is_empty()
    );
}

#[test]
fn a_page_with_marks_that_turn_text_around_is_refused() {
    assert!(site_of("https://shop.example.test/\u{202e}nigol").is_err());
    assert!(site_of("https://shop.example.test/\u{200f}login").is_err());
}

#[test]
fn a_key_a_tool_holds_is_said() {
    let dir = project(
        "[sandbox.personas.ops.tools]\npsql = { vault = \"qa\", as = \"PGPASSWORD env\", key = \
         \"password\" }\n",
    );
    let one = proposed(dir.path(), "https://new.example.test/");
    assert_eq!(
        one.warnings,
        vec![Warning::ToolsKey {
            tool: "psql".to_owned(),
            key: "password".to_owned()
        }]
    );
}

#[test]
fn a_name_that_would_break_the_line_is_refused_before_anyone_is_asked() {
    let dir = project("");
    for name in ["a b", "a=b", "a\"b", "a]\n[b", "", ".hidden", "bash"] {
        assert!(
            proposal(
                dir.path(),
                "qa",
                name,
                &suggested("https://a.test.example/"),
                &[]
            )
            .is_err(),
            "{name:?}"
        );
    }
    let bad = Suggested {
        key: "pass\"\nmint = \"x".to_owned(),
        ..suggested("https://a.test.example/")
    };
    assert!(proposal(dir.path(), "qa", "shop-login", &bad, &[]).is_err());
}

#[test]
fn the_tool_is_what_the_line_says() {
    let dir = project("");
    let one = proposed(dir.path(), "https://shop.example.test/login");
    let tool = one.tool().expect("a tool");
    assert_eq!(tool.name, "shop-login");
    assert_eq!(tool.vault, "qa");
    assert_eq!(tool.login_keys(), Some((Some("username"), "password")));
    let login = tool.login.expect("a login");
    assert_eq!(login.origin, "https://shop.example.test");
    assert_eq!(login.url, "https://shop.example.test/login");
}

#[test]
fn what_the_person_typed_keeps_the_path_and_is_read_as_a_suggestion_is() {
    let dir = project("");
    let one = proposed(dir.path(), "https://evil.example.test/login");
    let moved = one.at(dir.path(), "SHOP.example.test", &[]).expect("moved");
    let page = one
        .at(dir.path(), "shop.example.test/", &[])
        .expect("moved");
    assert_eq!(
        page.page, "https://shop.example.test/",
        "a typed page is the page"
    );
    assert_eq!(moved.origin, "https://shop.example.test");
    assert_eq!(moved.page, "https://shop.example.test/login");
    assert_ne!(moved.digest(), one.digest());
    assert!(one.at(dir.path(), "http://shop.example.test", &[]).is_err());
}

#[test]
fn the_digest_moves_with_everything_the_notice_shows() {
    let dir = project("");
    let one = proposed(dir.path(), "https://shop.example.test/login");
    let each = [
        Proposal {
            persona: "b".into(),
            ..one.clone()
        },
        Proposal {
            login: "b".into(),
            ..one.clone()
        },
        Proposal {
            vault: "b".into(),
            ..one.clone()
        },
        Proposal {
            user_key: None,
            ..one.clone()
        },
        Proposal {
            key: "b".into(),
            ..one.clone()
        },
        Proposal {
            origin: "https://b.test".into(),
            ..one.clone()
        },
        Proposal {
            page: "https://shop.example.test/b".into(),
            ..one.clone()
        },
        Proposal {
            warnings: vec![Warning::OtherPort { port: 1 }],
            ..one.clone()
        },
    ];
    for other in each {
        assert_ne!(other.digest(), one.digest(), "{other:?}");
    }
}

/// `text` with the login added, read back as the project's reader reads it.
fn added(text: &str) -> (String, toml::Table) {
    let dir = project("");
    let one = proposed(dir.path(), "https://shop.example.test/login");
    let after = with_line(text, &one).expect("written");
    let table = after.parse().expect("TOML");
    (after, table)
}

fn login_in(table: &toml::Table) -> &toml::Value {
    &table["sandbox"]["personas"]["qa"]["tools"]["shop-login"]
}

#[test]
fn always_adds_to_a_sandbox_with_no_personas_and_never_makes_one() {
    let (_, table) = added("[project]\nname = \"p\"\n[sandbox]\nmode = \"on\"\n");
    assert_eq!(
        login_in(&table)["browser"].as_str(),
        Some("https://shop.example.test/login")
    );
    assert_eq!(table["project"]["name"].as_str(), Some("p"));
    let dir = project("");
    let one = proposed(dir.path(), "https://shop.example.test/login");
    assert!(with_line("[project]\nname = \"p\"\n", &one).is_err());
}

#[test]
fn always_adds_beside_an_inline_list_and_a_table_and_keeps_comments() {
    let (text, table) = added(
        "# ours\n[sandbox]\nmode = \"on\" # kept\n[sandbox.personas.qa]\nhosts = [\"a.test\"]\ntools \
         = { other = { vault = \"v\", browser = \"https://o.test/\", key = \"k\" } }\n",
    );
    assert!(text.contains("# ours") && text.contains("# kept"), "{text}");
    assert_eq!(login_in(&table)["key"].as_str(), Some("password"));
    assert!(
        table["sandbox"]["personas"]["qa"]["tools"]
            .get("other")
            .is_some()
    );
    let (_, table) = added(
        "[sandbox.personas.qa.tools.other]\nvault = \"v\"\nbrowser = \"https://o.test/\"\nkey = \
         \"k\"\n",
    );
    assert_eq!(login_in(&table)["key"].as_str(), Some("password"));
    assert!(
        table["sandbox"]["personas"]["qa"]["tools"]
            .get("other")
            .is_some()
    );
}

#[test]
fn always_refuses_a_login_of_that_name_there_already_and_writes_nothing() {
    let dir = project("");
    let one = proposed(dir.path(), "https://shop.example.test/login");
    let text = "[sandbox.personas.qa.tools.shop-login]\nvault = \"v\"\nas = \"X env\"\nkey = \
                \"k\"\n";
    assert!(with_line(text, &one).is_err());
    assert!(with_line("sandbox = 3\n", &one).is_err());
    assert!(with_line("not [ toml", &one).is_err());
}

#[test]
fn the_written_login_reads_back_as_the_tool_the_person_allowed() {
    let dir = project("");
    let one = proposed(dir.path(), "https://shop.example.test/login");
    let (text, _) = added("[sandbox]\nmode = \"on\"\n");
    let plane = crate::sandbox::Plane::of(Some(&text));
    let said = plane.said();
    let policy = said.policy.or(said.unset).expect("a policy");
    let tools = &policy.personas["qa"].tools;
    assert_eq!(tools.as_slice(), &[one.tool().expect("a tool")]);
}
