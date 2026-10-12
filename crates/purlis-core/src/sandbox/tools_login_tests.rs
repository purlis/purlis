// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! A browser-login tool (D-PE-browser-1): what is read, the page it is bound to, what a chat's
//! run of it may ask for, and what the app then runs.

use std::path::{Path, PathBuf};

use super::super::policy::Locks;
use super::login::{self, Login};
use super::{Asked, Handed, Ready, Tool, declared, keyed, resolve_beside, run_words, shim};

const HERE: &str = "sandbox.personas.qa";
const PURLIS: &str = "/Applications/Purlis.app/Contents/MacOS/purlis";
const CLI: &str = "/opt/homebrew/bin/playwright-cli";

fn read(text: &str) -> (Vec<Tool>, Vec<String>) {
    let value: toml::Value = toml::from_str(text).expect("TOML");
    let (tools, refused) = keyed(
        value.get("tools"),
        HERE,
        "charter.toml",
        &["sandbox", "personas", "qa"],
    );
    (tools, refused.into_iter().map(|one| one.why).collect())
}

fn shop_login() -> Tool {
    Tool {
        name: "shop-login".to_owned(),
        vault: "qa".to_owned(),
        var: login::PASS_VAR.to_owned(),
        handed: Handed::File,
        key: "password".to_owned(),
        login: Some(Login {
            url: "https://shop.example.test/login".to_owned(),
            origin: "https://shop.example.test".to_owned(),
            user: Some("username".to_owned()),
        }),
    }
}

fn armed() -> Vec<Ready> {
    vec![Ready {
        tool: shop_login(),
        program: PathBuf::from(CLI),
    }]
}

fn files() -> Vec<String> {
    vec![
        "BROWSER_LOGIN_PASS=password".to_owned(),
        "BROWSER_LOGIN_USER=username".to_owned(),
    ]
}

fn asked<'a>(program: &'a str, file: &'a [String]) -> Asked<'a> {
    Asked {
        vault: "qa",
        env: &[],
        file,
        dotenv: &[],
        program,
    }
}

fn args(words: &[&str]) -> Vec<String> {
    words.iter().map(|word| (*word).to_owned()).collect()
}

#[test]
fn a_browser_login_is_read_with_its_page_its_origin_and_both_keys() {
    let (tools, refused) = read(
        r#"tools = { shop-login = { vault = "qa", browser = "https://shop.example.test/login",
                                    user = "username", key = "password" } }"#,
    );
    assert_eq!(refused, Vec::<String>::new());
    assert_eq!(tools, [shop_login()]);
    // A page that asks for a password alone names no user.
    let (tools, refused) = read(
        r#"tools = { pin = { vault = "qa", browser = "https://pin.example.test:8443/", key = "pin" } }"#,
    );
    assert_eq!(refused, Vec::<String>::new());
    let login = tools[0].login.clone().expect("a login");
    assert_eq!(login.origin, "https://pin.example.test:8443");
    assert_eq!(login.user, None);
}

#[test]
fn a_login_page_purlis_cannot_bind_to_one_host_is_refused_whole() {
    for (page, says) in [
        ("http://shop.example.test/login", "plain http"),
        ("ftp://shop.example.test/", "not an https://"),
        ("shop.example.test/login", "not an https://"),
        (
            "https://user:pw@shop.example.test/",
            "carries a user or password",
        ),
        (
            "https://evil.example\\@shop.example.test/",
            "carries a user or password",
        ),
        ("https://Shop.Example.test/", "names no host"),
        ("https:///login", "names no host"),
        ("https://shop.example.test:0/", "no port"),
        ("https://shop.example.test:99999/", "no port"),
        ("https://shop.example.test/a b", "a space"),
        ("https://bücher.example/", "names no host"),
    ] {
        let (tools, refused) = read(&format!(
            "tools = {{ shop = {{ vault = \"qa\", browser = {page:?}, key = \"password\" }} }}"
        ));
        assert_eq!(tools, Vec::<Tool>::new(), "{page}");
        assert_eq!(refused.len(), 1, "{page}: {refused:?}");
        assert!(refused[0].contains(says), "{page}: {}", refused[0]);
        assert!(refused[0].contains("runs nothing"), "{}", refused[0]);
    }
}

#[test]
fn a_login_page_s_origin_is_spelled_as_a_browser_spells_it() {
    for (page, origin) in [
        (
            "https://shop.example.test/login?next=/",
            "https://shop.example.test",
        ),
        (
            "https://shop.example.test:443/login",
            "https://shop.example.test",
        ),
        (
            "https://shop.example.test:8443",
            "https://shop.example.test:8443",
        ),
        ("https://10.0.0.7/login", "https://10.0.0.7"),
        ("https://[fd00::1]:9443/", "https://[fd00::1]:9443"),
        ("http://localhost:3000/login", "http://localhost:3000"),
        ("http://127.0.0.1/login", "http://127.0.0.1"),
    ] {
        assert_eq!(login::origin_of(page).as_deref(), Ok(origin), "{page}");
    }
}

#[test]
fn a_login_takes_no_as_and_user_is_a_login_s_key_alone() {
    let (tools, refused) = read(
        r#"tools = { shop = { vault = "qa", browser = "https://shop.example.test/", as = "X file", key = "p" },
                     kubectl = { vault = "qa", as = "KUBECONFIG file", user = "u", key = "k" } }"#,
    );
    assert_eq!(tools, Vec::<Tool>::new());
    assert_eq!(refused.len(), 2, "{refused:?}");
    assert!(
        refused.iter().any(|why| why.contains("takes no as")),
        "{refused:?}"
    );
    assert!(
        refused
            .iter()
            .any(|why| why.contains("user is a browser login's key")),
        "{refused:?}"
    );
}

#[test]
fn the_allow_is_bound_to_the_login_page_and_both_keys() {
    let whole = shop_login().whole();
    assert_eq!(
        whole,
        "shop-login logs in to https://shop.example.test/login with username and password from \
         vault qa in a browser"
    );
    let digest = |tool: Tool| super::super::persona::digest_of(&[], &[tool], false);
    let mut elsewhere = shop_login();
    elsewhere.login.as_mut().expect("login").url = "https://shop.example.test/other".to_owned();
    let mut other_user = shop_login();
    other_user.login.as_mut().expect("login").user = Some("admin".to_owned());
    assert_ne!(digest(shop_login()), digest(elsewhere));
    assert_ne!(digest(shop_login()), digest(other_user));
}

#[test]
fn a_login_runs_playwright_s_program_found_where_no_chat_writes_never_one_its_name_finds() {
    let base = tempfile::tempdir().expect("a folder").keep();
    let root = base.join("project");
    let system = base.join("system");
    let make = |dir: &Path, name: &str| {
        use std::os::unix::fs::PermissionsExt;
        std::fs::create_dir_all(dir).expect("dir");
        let path = dir.join(name);
        std::fs::write(&path, "#!/bin/sh\n").expect("written");
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).expect("mode");
        path
    };
    // A program named as the tool, even where no chat writes, is not what a login runs.
    make(&system, "shop-login");
    make(&root.join("bin"), login::PROGRAM);
    let real = make(&system, login::PROGRAM);
    let resolved = resolve_beside(
        &root,
        "qa",
        &[shop_login()],
        &Locks::none(),
        &[root.join("bin"), system.clone()],
        &[],
        &[],
    )
    .expect("tools");
    assert_eq!(
        resolved.ready,
        [Ready {
            tool: shop_login(),
            program: real,
        }]
    );
    let _ = std::fs::remove_dir_all(&base);
}

#[test]
fn an_armed_browser_login_logs_in_on_its_declared_page_only() {
    let armed = armed();
    let ready = declared(
        &armed,
        &[shop_login()],
        "qa",
        &asked("shop-login", &files()),
    )
    .expect("declared");
    for (said, session) in [
        (args(&[]), "default"),
        (args(&["-s=owner"]), "owner"),
        (args(&["-s", "viewer"]), "viewer"),
        (args(&["--session=w-1.a_b"]), "w-1.a_b"),
    ] {
        let words = run_words(ready, &said, Some(Path::new(PURLIS))).expect("runs");
        assert_eq!(
            words,
            args(&[
                PURLIS,
                "browser",
                "_login",
                "--cli",
                CLI,
                "--url",
                "https://shop.example.test/login",
                "--session",
                session,
            ]),
            "{said:?}"
        );
    }
    let none = run_words(ready, &[], None).expect_err("no purlis");
    assert!(none.contains("nothing ran"), "{none}");
}

#[test]
fn a_chat_cannot_steer_a_login_to_another_page_host_or_program() {
    let armed = armed();
    let ready = declared(
        &armed,
        &[shop_login()],
        "qa",
        &asked("shop-login", &files()),
    )
    .expect("declared");
    for said in [
        args(&["https://evil.example/"]),
        args(&["--url", "https://evil.example/"]),
        args(&["--cli", "/tmp/evil"]),
        args(&["-s=owner", "--url=https://evil.example/"]),
        args(&["-s=owner", "--", "sh", "-c", "id"]),
        args(&["-s=../owner"]),
        args(&["-s=a b"]),
        args(&["-s="]),
        args(&["-s=-x"]),
    ] {
        let why = run_words(ready, &said, Some(Path::new(PURLIS))).expect_err("refused");
        assert!(
            why.contains("logs in to https://shop.example.test/login"),
            "{said:?}: {why}"
        );
        assert!(why.contains("nothing ran"), "{why}");
    }
    // Playwright's program itself, by its path or its name, is not the tool: a chat that names
    // it would choose the page.
    for program in [CLI, login::PROGRAM] {
        let why = declared(&armed, &[shop_login()], "qa", &asked(program, &files()))
            .expect_err("not the tool");
        assert!(why.contains("is not one"), "{program}: {why}");
    }
}

#[test]
fn a_login_is_asked_for_with_exactly_its_vault_and_files() {
    let armed = armed();
    let swapped = vec![
        "BROWSER_LOGIN_PASS=username".to_owned(),
        "BROWSER_LOGIN_USER=password".to_owned(),
    ];
    let one = vec!["BROWSER_LOGIN_PASS=password".to_owned()];
    let dotenv = vec!["PLAYWRIGHT_MCP_SECRETS_FILE=PASS:password".to_owned()];
    let env = vec!["BROWSER_LOGIN_PASS=password".to_owned()];
    let files = files();
    for asked in [
        asked("shop-login", &swapped),
        asked("shop-login", &one),
        Asked {
            vault: "other",
            ..asked("shop-login", &files)
        },
        Asked {
            dotenv: &dotenv,
            ..asked("shop-login", &files)
        },
        Asked {
            env: &env,
            ..asked("shop-login", &files)
        },
    ] {
        let why = declared(&armed, &[shop_login()], "qa", &asked).expect_err("refused");
        assert!(
            why.contains(
                "purlis secret exec qa --file BROWSER_LOGIN_PASS=password --file \
                 BROWSER_LOGIN_USER=username -- shop-login"
            ),
            "{why}"
        );
    }
}

#[test]
fn an_undeclared_browser_login_is_refused() {
    let files = files();
    let why = declared(&[], &[], "qa", &asked("shop-login", &files)).expect_err("undeclared");
    assert!(
        why.contains(
            "a chat runs only qa's declared tools with a vault, and shop-login is not one"
        ),
        "{why}"
    );
    // Declared and not allowed on this machine, or forbidden by policy: not armed, so refused.
    let why =
        declared(&[], &[shop_login()], "qa", &asked("shop-login", &files)).expect_err("not armed");
    assert!(why.contains("it is not armed in this chat"), "{why}");
}

#[test]
fn a_login_s_shim_asks_the_app_and_runs_nothing_without_one() {
    let text = shim(
        Path::new(PURLIS),
        "qa",
        &Ready {
            tool: shop_login(),
            program: PathBuf::from(CLI),
        },
    );
    assert!(
        text.ends_with(
            "exec '/Applications/Purlis.app/Contents/MacOS/purlis' 'secret' 'exec' 'qa' '--file' \
             'BROWSER_LOGIN_PASS=password' '--file' 'BROWSER_LOGIN_USER=username' '--' \
             'shop-login' \"$@\"\n"
        ),
        "{text}"
    );
    // Outside a chat the app started there is no app to log in through, and Playwright's
    // program is never run in its place.
    assert!(text.contains("exit 1\nfi\n"), "{text}");
    assert!(!text.contains(CLI), "{text}");
}
