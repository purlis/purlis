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

/// Where a page's own form shows: on the page.
fn on_its_page(page: &str) -> Result<String, String> {
    Ok(page.to_owned())
}

/// [`super::proposal`] for a site whose form is on its own page, as every suggestion was before
/// #4995.
fn proposal(
    root: &Path,
    persona: &str,
    login: &str,
    suggested: &Suggested,
    granted: &[Tool],
) -> Result<Proposal, String> {
    super::proposal(root, persona, login, suggested, &on_its_page, granted)
}

/// [`Proposal::at`] where the typed page's form is on its own page.
fn at(one: &Proposal, root: &Path, typed: &str) -> Result<Proposal, String> {
    one.at(root, typed, &on_its_page, &[])
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
    let same = proposed(dir.path(), "https://old.example.test/");
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

/// The person's ruling of 2026-11-05: any page but the site's front one is said, a usual login
/// address too, as the site decides what its page does as it opens.
#[test]
fn a_page_but_the_front_one_is_said_and_shown_whole() {
    let dir = project("");
    for path in ["/~mallory/notes.html", "/login/", "/signin"] {
        let one = proposed(dir.path(), &format!("https://wiki.example.test{path}"));
        assert_eq!(
            one.warnings,
            vec![Warning::ChatsPage {
                path: path.to_owned()
            }]
        );
        let said = one.warnings[0].said();
        assert!(
            said.contains(&format!("The chat chose the page {path}, not")),
            "{said}"
        );
        assert!(
            said.contains("sign the browser in to someone else's account"),
            "{said}"
        );
    }
    assert!(
        proposed(dir.path(), "https://wiki.example.test/")
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
    let moved = at(&one, dir.path(), "SHOP.example.test").expect("moved");
    let page = at(&one, dir.path(), "shop.example.test/").expect("moved");
    assert_eq!(
        page.page, "https://shop.example.test/",
        "a typed page is the page"
    );
    assert_eq!(moved.origin, "https://shop.example.test");
    assert_eq!(moved.page, "https://shop.example.test/login");
    assert_ne!(moved.digest(), one.digest());
    assert!(at(&one, dir.path(), "http://shop.example.test").is_err());
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
            form: "https://auth.example.test".into(),
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

/// Where the SSO page of #4995 sent the browser: a form on another host, with what the site
/// handed the browser in its address.
const FOUND_ELSEWHERE: &str =
    "https://auth.example.test/realms/x/protocol/openid-connect/auth?client_id=app&state=s3cr3t";

fn signs_in_elsewhere(root: &Path) -> Proposal {
    super::proposal(
        root,
        "qa",
        "shop-login",
        &suggested("https://app.example.test/"),
        &|page| {
            assert_eq!(page, "https://app.example.test/");
            Ok(FOUND_ELSEWHERE.to_owned())
        },
        &[],
    )
    .expect("a proposal")
}

/// #4995: the page opens, and its form's origin is the one place the password goes. The person
/// is told plainly that the two differ, with both hosts.
#[test]
fn a_form_on_another_host_is_where_the_password_goes_and_is_said() {
    let dir = project("");
    let one = signs_in_elsewhere(dir.path());
    assert_eq!(one.origin, "https://app.example.test");
    assert_eq!(one.page, "https://app.example.test/");
    assert_eq!(one.form, "https://auth.example.test");
    let warning = Warning::FormElsewhere {
        page_host: "app.example.test".into(),
        form_host: "auth.example.test".into(),
    };
    assert_eq!(one.warnings, vec![warning.clone()]);
    let said = warning.said();
    assert!(
        said.contains("The password goes to auth.example.test, not app.example.test"),
        "{said}"
    );
    let login = one.tool().expect("a tool").login.expect("a login");
    assert_eq!(login.url, "https://app.example.test/");
    assert_eq!(login.fills_on(), "https://auth.example.test");
    assert!(!one.line().contains("s3cr3t"), "{}", one.line());
}

/// A form on the page's own origin is a proposal as before #4995: no warning, and no `form` in
/// the line.
#[test]
fn a_form_on_the_page_s_own_origin_is_said_as_before_and_writes_no_form() {
    let dir = project("");
    let one = super::proposal(
        dir.path(),
        "qa",
        "shop-login",
        &suggested("https://shop.example.test/"),
        &|_| Ok("https://shop.example.test/login?next=%2F".to_owned()),
        &[],
    )
    .expect("a proposal");
    assert_eq!(one.form, one.origin);
    assert!(one.warnings.is_empty(), "{:?}", one.warnings);
    assert!(!one.line().contains("form"), "{}", one.line());
}

#[test]
fn a_form_that_isnt_https_or_cant_be_confirmed_is_refused_naming_its_host_alone() {
    let dir = project("");
    for (found, said) in [
        ("http://auth.example.test/login?token=s3cr3t", "not https"),
        ("https://10.0.0.7/login?token=s3cr3t", "an address"),
        ("https://intranet/login?token=s3cr3t", "a bare name"),
        (
            "https://sso.corp/login?token=s3cr3t",
            "a name on this machine",
        ),
        ("about:blank", "not https"),
        ("", "not https"),
    ] {
        let refused = super::proposal(
            dir.path(),
            "qa",
            "shop-login",
            &suggested("https://app.example.test/"),
            &|_| Ok(found.to_owned()),
            &[],
        )
        .expect_err(found);
        assert!(refused.contains(said), "{found}: {refused}");
        assert!(refused.contains("records nothing"), "{found}: {refused}");
        assert!(!refused.contains("s3cr3t"), "{found}: {refused}");
    }
}

#[test]
fn a_form_s_name_in_another_alphabet_and_its_port_are_said() {
    let dir = project("");
    let one = super::proposal(
        dir.path(),
        "qa",
        "shop-login",
        &suggested("https://app.example.test/"),
        &|_| Ok("https://xn--80ak6aa92e.example.test:8443/login".to_owned()),
        &[],
    )
    .expect("a proposal");
    assert!(
        one.warnings
            .iter()
            .any(|warning| matches!(warning, Warning::OtherAlphabet { .. })),
        "{:?}",
        one.warnings
    );
    assert!(
        one.warnings.contains(&Warning::OtherPort { port: 8443 }),
        "{:?}",
        one.warnings
    );
}

/// The site a key is on is where its password is typed, and a sign-in form it already serves for
/// another site's page is said as that site: either way the person hears where else it goes.
#[test]
fn a_key_is_on_the_site_its_password_is_typed_on_and_the_sites_its_form_serves() {
    let dir = project(
        "[sandbox.personas.ops.tools]\nold = { vault = \"qa\", browser = \
         \"https://portal.example.test/\", form = \"https://auth.example.test\", key = \
         \"password\" }\n",
    );
    let same_form = signs_in_elsewhere(dir.path());
    assert!(
        same_form.warnings.contains(&Warning::KeyElsewhere {
            origins: vec!["https://portal.example.test".to_owned()]
        }),
        "{:?}",
        same_form.warnings
    );
    let on_the_portal = proposed(dir.path(), "https://portal.example.test/");
    assert_eq!(
        on_the_portal.warnings,
        vec![Warning::KeyElsewhere {
            origins: vec!["https://auth.example.test".to_owned()]
        }]
    );
}

#[test]
fn always_writes_the_form_beside_the_page_and_it_reads_back_as_allowed() {
    let dir = project("");
    let one = signs_in_elsewhere(dir.path());
    let text = with_line("[sandbox]\nmode = \"on\"\n", &one).expect("written");
    let table: toml::Table = text.parse().expect("TOML");
    assert_eq!(
        login_in(&table)["form"].as_str(),
        Some("https://auth.example.test")
    );
    let plane = crate::sandbox::Plane::of(Some(&text));
    let said = plane.said();
    let policy = said.policy.or(said.unset).expect("a policy");
    assert_eq!(
        policy.personas["qa"].tools.as_slice(),
        &[one.tool().expect("a tool")]
    );
}

/// A typed site is followed to its form again: the form found for the old page never stands for
/// a new one.
#[test]
fn a_typed_site_takes_the_form_found_for_it() {
    let dir = project("");
    let one = signs_in_elsewhere(dir.path());
    let moved = one
        .at(
            dir.path(),
            "shop.example.test",
            &|page| {
                assert_eq!(
                    page, "https://shop.example.test/",
                    "the typed page is followed"
                );
                Ok(page.to_owned())
            },
            &[],
        )
        .expect("moved");
    assert_eq!(moved.form, "https://shop.example.test");
    assert!(
        moved
            .warnings
            .iter()
            .all(|warning| !matches!(warning, Warning::FormElsewhere { .. }))
    );
}

/// Nobody is asked about a login purlis couldn't follow to its form, and a suggestion that's
/// refused for what it names is refused before the page is opened at all.
#[test]
fn a_page_that_cant_be_followed_to_its_form_is_refused_and_a_bad_name_never_opens_it() {
    let dir = project("");
    let refused = super::proposal(
        dir.path(),
        "qa",
        "shop-login",
        &suggested("https://app.example.test/"),
        &|_| Err("no login form appeared".to_owned()),
        &[],
    )
    .expect_err("refused");
    assert!(refused.contains("no login form appeared"), "{refused}");
    let opened = std::cell::Cell::new(false);
    let bad = Suggested {
        vault: "a vault".to_owned(),
        ..suggested("https://app.example.test/")
    };
    for one in [bad, suggested("http://app.example.test/")] {
        assert!(
            super::proposal(
                dir.path(),
                "qa",
                "shop-login",
                &one,
                &|page| {
                    opened.set(true);
                    Ok(page.to_owned())
                },
                &[],
            )
            .is_err()
        );
    }
    assert!(
        !opened.get(),
        "nothing is opened for a suggestion that's refused"
    );
}

// ---- A call naming another site for a declared login (#4996) ----

/// The project file of #4996: login `shop-login` recorded on the app's address.
const RECORDED: &str = "# ours\n[sandbox]\nmode = \"on\"\n[sandbox.personas.qa]\ntools = { \
                        shop-login = { vault = \"qa\", browser = \"https://app.example.test/\", \
                        user = \"username\", key = \"password\" }, other = { vault = \"v\", \
                        browser = \"https://o.test/\", key = \"k\" } }\n";

fn declared(text: &str) -> Tool {
    super::declared_in(text, "qa", "shop-login").expect("declared")
}

fn replacing(root: &Path, text: &str) -> Proposal {
    signs_in_elsewhere(root)
        .replacing(root, &declared(text), &[])
        .expect("a replacement")
}

#[test]
fn a_replacement_names_the_site_it_replaces_and_moves_the_digest() {
    let dir = project(RECORDED);
    let one = replacing(dir.path(), RECORDED);
    assert_eq!(
        one.replaces,
        Some(Replaced {
            page: "https://app.example.test/".into(),
            form: "https://app.example.test".into(),
        })
    );
    assert_ne!(one.digest(), signs_in_elsewhere(dir.path()).digest());
    // The site it replaces is not another site the password is on.
    assert!(
        one.warnings
            .iter()
            .all(|warning| !matches!(warning, Warning::KeyElsewhere { .. })),
        "{:?}",
        one.warnings
    );
    assert!(!signs_in_elsewhere(dir.path()).is_where(&declared(RECORDED)));
    let same = proposed(dir.path(), "https://app.example.test/");
    assert!(same.is_where(&declared(RECORDED)));
}

#[test]
fn a_call_never_changes_a_declared_login_s_vault_or_keys() {
    let dir = project(RECORDED);
    for other in [
        Suggested {
            vault: "prod".into(),
            ..suggested("https://app.example.test/")
        },
        Suggested {
            key: "other".into(),
            ..suggested("https://app.example.test/")
        },
        Suggested {
            user_key: None,
            ..suggested("https://app.example.test/")
        },
    ] {
        let one = proposal(dir.path(), "qa", "shop-login", &other, &[]).expect("a proposal");
        let refused = one
            .replacing(dir.path(), &declared(RECORDED), &[])
            .expect_err("refused");
        assert!(refused.contains("change its site alone"), "{refused}");
    }
}

#[test]
fn always_replaces_that_one_line_and_keeps_the_rest_of_the_file() {
    let dir = project(RECORDED);
    let one = replacing(dir.path(), RECORDED);
    let after = with_line(RECORDED, &one).expect("written");
    assert!(after.contains("# ours"), "{after}");
    let table: toml::Table = after.parse().expect("TOML");
    let tools = &table["sandbox"]["personas"]["qa"]["tools"];
    assert_eq!(
        tools["shop-login"]["form"].as_str(),
        Some("https://auth.example.test")
    );
    assert_eq!(tools["other"]["browser"].as_str(), Some("https://o.test/"));
    let plane = crate::sandbox::Plane::of(Some(&after));
    let said = plane.said();
    let policy = said.policy.or(said.unset).expect("a policy");
    let read = &policy.personas["qa"].tools;
    assert!(read.contains(&one.tool().expect("a tool")), "{read:?}");
    assert_eq!(read.len(), 2);
    // As a table of its own, too.
    let table_form = "[sandbox.personas.qa.tools.shop-login]\nvault = \"qa\"\nbrowser = \
                      \"https://app.example.test/\"\nuser = \"username\"\nkey = \"password\"\n";
    let one = replacing(dir.path(), table_form);
    let after = with_line(table_form, &one).expect("written");
    assert!(after.contains("auth.example.test"), "{after}");
}

/// A line that moved since the notice was drawn is not the one the person was shown: nothing is
/// written over it.
#[test]
fn always_writes_nothing_over_a_line_that_changed_since_it_was_shown() {
    let dir = project(RECORDED);
    let one = replacing(dir.path(), RECORDED);
    for changed in [
        RECORDED.replace(
            "https://app.example.test/",
            "https://elsewhere.example.test/",
        ),
        RECORDED.replace("key = \"password\"", "key = \"other\""),
        RECORDED.replace("shop-login = ", "renamed = "),
    ] {
        assert!(with_line(&changed, &one).is_err(), "{changed}");
    }
}

// ---- A key the vault doesn't hold ----

/// A project whose vault `qa` is a plain file holding `held`, and `op`, a 1Password vault.
fn with_vaults(held: &str) -> tempfile::TempDir {
    let dir = project("");
    std::fs::write(
        dir.path().join("vaults.json"),
        r#"{"vaults":{
            "qa":{"provider":"plain-file","config":{"file":"q.json"}},
            "op":{"provider":"1password","config":{"vault":"Team"}}
        }}"#,
    )
    .expect("a registry");
    std::fs::write(dir.path().join("q.json"), held).expect("a vault file");
    dir
}

fn ctx(root: &Path) -> crate::secrets::Ctx {
    crate::secrets::Ctx::new(root, crate::secrets::Env::of(&[]))
}

#[test]
fn a_key_its_vault_has_nothing_under_is_said_by_name() {
    let dir = with_vaults(r#"{"password":"p"}"#);
    let one = proposed(dir.path(), "https://shop.example.test/");
    assert_eq!(
        not_in_vault(&ctx(dir.path()), &one),
        Some(Warning::NotInVault {
            vault: "qa".to_owned(),
            keys: vec!["username".to_owned()],
        })
    );
    let said = not_in_vault(&ctx(dir.path()), &one)
        .expect("a warning")
        .said();
    assert_eq!(
        said,
        "Vault qa has nothing under username yet. If you allow this, purlis asks you for its \
         value when it fills this login. Check the name if you expected it there."
    );
}

#[test]
fn keys_its_vault_holds_are_not_said() {
    let dir = with_vaults(r#"{"password":"p","username":"u"}"#);
    let one = proposed(dir.path(), "https://shop.example.test/");
    assert_eq!(not_in_vault(&ctx(dir.path()), &one), None);
}

#[test]
fn a_vault_that_cant_be_listed_quietly_is_not_listed() {
    let dir = with_vaults("{}");
    let one = Proposal {
        vault: "op".to_owned(),
        ..proposed(dir.path(), "https://shop.example.test/")
    };
    assert_eq!(not_in_vault(&ctx(dir.path()), &one), None);
}

#[test]
fn two_keys_its_vault_has_nothing_under_are_said_as_two() {
    let dir = with_vaults("{}");
    let one = proposed(dir.path(), "https://shop.example.test/");
    let said = not_in_vault(&ctx(dir.path()), &one)
        .expect("a warning")
        .said();
    assert_eq!(
        said,
        "Vault qa has nothing under username or password yet. If you allow this, purlis asks \
         you for their values when it fills this login. Check the names if you expected them \
         there."
    );
}
