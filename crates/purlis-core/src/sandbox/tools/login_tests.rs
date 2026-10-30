// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! A browser-login tool (D-PE-browser-1): what is read, the page it is bound to, and that run as
//! a command it runs nothing, since purlis performs it in the chat's own browser (#2016).

use std::path::{Path, PathBuf};

use super::super::policy::Locks;
use super::login::{self, Login};
use super::{Asked, Handed, Ready, Tool, Value, declared, keyed, resolve_beside, run_words, shim};

const LOGIN_BY_THE_BROWSER_TOOL: &str = "call the browser_login tool with login = \"shop-login\"";

const HERE: &str = "sandbox.personas.qa";
const PURLIS: &str = "/Applications/Purlis.app/Contents/MacOS/purlis";

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

/// A login's value handed in `var`, from `key`.
fn file(var: &str, key: &str) -> Value {
    Value {
        var: var.to_owned(),
        handed: Handed::File,
        key: key.to_owned(),
    }
}

/// `shop_login` with `user` as its user name's key (none for a page that asks for a password
/// alone) and `key` as its password's.
fn shop_login_with(user: Option<&str>, key: &str) -> Tool {
    Tool {
        name: "shop-login".to_owned(),
        vault: "qa".to_owned(),
        values: std::iter::once(file(login::PASS_VAR, key))
            .chain(user.map(|user| file(login::USER_VAR, user)))
            .collect(),
        login: Some(Login {
            url: "https://shop.example.test/login".to_owned(),
            origin: "https://shop.example.test".to_owned(),
        }),
        args: None,
        mint: None,
    }
}

fn shop_login() -> Tool {
    shop_login_with(Some("username"), "password")
}

fn armed() -> Vec<Ready> {
    vec![Ready {
        tool: shop_login(),
        program: PathBuf::new(),
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
    assert_eq!(
        tools[0].values,
        [file(login::PASS_VAR, "pin")],
        "no user's file"
    );
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
        "shop-login logs in to https://shop.example.test/login with \"username\" and \"password\" \
         from vault qa in a browser"
    );
    let digest = |tool: Tool| super::super::persona::digest_of(&[], &[tool], false);
    let mut elsewhere = shop_login();
    elsewhere.login.as_mut().expect("login").url = "https://shop.example.test/other".to_owned();
    let other_user = shop_login_with(Some("admin"), "password");
    assert_ne!(digest(shop_login()), digest(elsewhere));
    assert_ne!(digest(shop_login()), digest(other_user));
}

/// #1828: a key or a user name may hold " and ", so each is quoted: two logins that differ in
/// which key is the user's and which the password's never share a line, nor an Allow.
#[test]
fn a_login_s_keys_are_quoted_so_no_two_logins_share_a_line() {
    let login = shop_login_with;
    let digest =
        |tool: &Tool| super::super::persona::digest_of(&[], std::slice::from_ref(tool), false);
    let pairs = [
        (login(Some("a and b"), "c"), login(Some("a"), "b and c")),
        (login(Some("a"), "b"), login(None, "a and b")),
        (
            login(Some("a\" and \"b"), "c"),
            login(Some("a"), "b\" and \"c"),
        ),
    ];
    for (one, other) in &pairs {
        assert_ne!(one.whole(), other.whole());
        assert_ne!(
            digest(one),
            digest(other),
            "{} | {}",
            one.whole(),
            other.whole()
        );
    }
    // Without a user, the password's key alone, quoted.
    assert_eq!(
        login(None, "password").whole(),
        "shop-login logs in to https://shop.example.test/login with \"password\" from vault qa \
         in a browser"
    );
}

#[test]
fn a_login_is_armed_with_no_program_of_its_own_whatever_its_name_finds() {
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
            program: PathBuf::new(),
        }]
    );
    let _ = std::fs::remove_dir_all(&base);
}

#[test]
fn a_login_run_as_a_command_runs_nothing_and_names_the_browser_tool() {
    let armed = armed();
    let ready = declared(
        &armed,
        &[shop_login()],
        "qa",
        &asked("shop-login", &files()),
    )
    .expect("declared");
    for said in [
        Vec::new(),
        vec!["-s=owner".to_owned()],
        vec!["https://evil.example/".to_owned()],
    ] {
        let why = run_words(ready, &said).expect_err("runs nothing");
        assert!(why.contains(LOGIN_BY_THE_BROWSER_TOOL), "{said:?}: {why}");
        assert!(why.contains("Nothing ran"), "{why}");
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
fn a_login_s_shim_names_the_browser_tool_and_runs_nothing() {
    let text = shim(
        Path::new(PURLIS),
        "qa",
        &Ready {
            tool: shop_login(),
            program: PathBuf::new(),
        },
    );
    assert!(text.contains(LOGIN_BY_THE_BROWSER_TOOL), "{text}");
    assert!(text.ends_with(">&2\nexit 2\n"), "{text}");
    assert!(!text.contains("secret"), "{text}");
    assert!(!text.contains("exec "), "{text}");
}

#[test]
fn a_login_page_named_in_another_script_is_bound_to_its_ascii_spelling() {
    assert_eq!(
        login::origin_of("https://bücher.example/login").as_deref(),
        Ok("https://xn--bcher-kva.example")
    );
    // Plain http stays this machine's loopback's alone, whatever the host is written in.
    assert!(login::origin_of("http://bücher.example/login").is_err());
    assert!(login::origin_of("http://shop.example.test/login").is_err());
    assert!(login::origin_of("http://127.0.0.1:8123/login").is_ok());
}
