// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **Signing in to GitLab through the browser** (#730, ADR 0077 §1, §3 and §7), end to end
//! against stand-ins, never the real gitlab.com.
//!
//! The first case runs the whole sign-in over real sockets on this machine's loopback: the
//! real listener on the registered ports, a stand-in browser that comes back to the redirect
//! address, a stand-in token address that grants a token only for the code with its verifier,
//! and a recorded forge that says whose token it is. It asserts what the person ends with (the
//! account, signed in by the browser, its token in its own keyring item and nowhere else) and
//! what the listener is: one that refuses a caller without the sign-in's `state`, answers a
//! fixed page, and is gone afterwards. The second holds what happens with all three ports
//! taken.
//!
//! The third is FI3's, for a token this sign-in made: the access token and the refresh token are
//! in no chat's environment, no answer of a tool purlis's MCP server offers a chat, and no file a
//! chat could be handed.
//!
//! The first two bind sockets and the third writes a project, so all three first run on CI. The
//! flow's logic is covered without either in `forge::pkce` and `forge::loopback`.

use std::io::{Read, Write};
use std::net::{Ipv4Addr, SocketAddrV4, TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use purlis_core::active::Place;
use purlis_core::chatenv::{Starting, compose};
use purlis_core::forge::accounts::{self, ForgeAccount, Method, State};
use purlis_core::forge::http::TokenSource;
use purlis_core::forge::oauth::{self, Endpoints};
use purlis_core::forge::recorded::Recorded;
use purlis_core::forge::registration::{self, REDIRECT_PORTS};
use purlis_core::forge::route::HostScope;
use purlis_core::forge::transport::Transport;
use purlis_core::forge::{Forge, ForgeError, Kind, loopback, pkce};
use secrecy::ExposeSecret;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

/// Not shaped like any forge's token, so only their values can find them.
const ACCESS: &str = "canaryPKCEaccess7f3a91";
const REFRESH: &str = "canaryPKCErefresh2b8d40";
const CODE: &str = "canaryPKCEcode55e1c7";

/// The token address's answer to a good exchange, as GitLab's documentation shows it.
fn grant() -> String {
    serde_json::json!({
        "access_token": ACCESS,
        "token_type": "Bearer",
        "expires_in": 7200,
        "refresh_token": REFRESH,
        "scope": "api",
        "created_at": 1_791_878_400u64,
    })
    .to_string()
}

/// A recorded gitlab.com that says whose token it is.
fn whose() -> Arc<Recorded> {
    let text = std::fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/forge_contract/accounts/gitlab/token_check.json"),
    )
    .unwrap();
    Arc::new(Recorded::parse(&text).unwrap())
}

/// The value of `name` in `url`'s query, percent-decoded.
fn query(url: &str, name: &str) -> String {
    let (_, query) = url.split_once('?').unwrap();
    let raw = query
        .split('&')
        .find_map(|pair| pair.strip_prefix(&format!("{name}=")))
        .unwrap_or_else(|| panic!("{name} is not in the address"));
    percent_encoding::percent_decode_str(raw)
        .decode_utf8()
        .unwrap()
        .into_owned()
}

/// One `GET` of `target` on the listener at `port`, and its whole answer.
fn get(port: u16, target: &str) -> String {
    let mut stream = TcpStream::connect(SocketAddrV4::new(Ipv4Addr::LOCALHOST, port)).unwrap();
    stream
        .set_read_timeout(Some(Duration::from_secs(10)))
        .unwrap();
    write!(
        stream,
        "GET {target} HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nAccept: text/html\r\n\r\n"
    )
    .unwrap();
    let mut answer = String::new();
    stream.read_to_string(&mut answer).unwrap();
    answer
}

/// Every file under `root` but the test build's keyring stub, which stands in for the keychain.
fn files(root: &Path) -> Vec<PathBuf> {
    let mut found = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries {
            let path = entry.unwrap().path();
            if path.is_dir() {
                stack.push(path);
            } else if path.file_name().is_some_and(|n| n != "keyring-stub.json") {
                found.push(path);
            }
        }
    }
    found
}

/// Asserts no file under `roots` holds any of `values`; how many files were searched.
fn in_no_file(roots: &[&Path], values: &[&str]) -> usize {
    let mut searched = 0;
    for path in roots.iter().flat_map(|root| files(root)) {
        let text = String::from_utf8_lossy(&std::fs::read(&path).unwrap()).into_owned();
        for value in values {
            assert!(
                !text.contains(value),
                "{} holds a value of the sign-in",
                path.display()
            );
        }
        searched += 1;
    }
    searched
}

/// The three registered ports are one machine's: the cases that bind them take turns.
static THE_PORTS: Mutex<()> = Mutex::new(());

/// What the stand-in browser saw and was answered.
#[derive(Default)]
struct Browser {
    page: Option<String>,
    stranger: Option<String>,
    back: Option<String>,
}

#[test]
fn a_browser_sign_in_ends_with_the_account_and_its_token_in_the_keyring_alone() {
    purlis_core::unsteered!();
    let _turn = THE_PORTS.lock().unwrap_or_else(|e| e.into_inner());
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    let server = rt.block_on(MockServer::start());
    rt.block_on(
        Mock::given(method("POST"))
            .and(path("/oauth/token"))
            .respond_with(|request: &wiremock::Request| {
                // A token only for this sign-in's code: anything else is refused as GitLab
                // refuses it.
                let body = String::from_utf8_lossy(&request.body).into_owned();
                if body.contains(&format!("code={CODE}")) {
                    ResponseTemplate::new(200).set_body_string(grant())
                } else {
                    ResponseTemplate::new(400).set_body_string(r#"{"error": "invalid_grant"}"#)
                }
            })
            .mount(&server),
    );
    // The stand-in answers on a thread of its own, so the flow, which blocks, can run here.
    let endpoints = Endpoints::at(Kind::GitLab, &server.uri());

    let home = tempfile::tempdir().unwrap();
    let config = home.path();
    let gitlab = registration::on(Kind::GitLab, "gitlab.com").unwrap();
    let seen = Arc::new(Mutex::new(Browser::default()));
    let browsers: Mutex<Vec<std::thread::JoinHandle<()>>> = Mutex::new(Vec::new());
    let bound = Mutex::new(None);

    let bind = |ports: &[u16]| {
        let listening = loopback::bind(ports)?;
        *bound.lock().unwrap() = Some(listening.port());
        Ok(listening)
    };
    let open = |page: &str| {
        let page = page.to_string();
        let seen = seen.clone();
        browsers.lock().unwrap().push(std::thread::spawn(move || {
            let back = query(&page, "redirect_uri");
            let port: u16 = back
                .strip_prefix("http://127.0.0.1:")
                .and_then(|rest| rest.split('/').next())
                .unwrap()
                .parse()
                .unwrap();
            let target = back.split_once(&port.to_string()).unwrap().1;
            // First, a caller that knows the address and not the state.
            let stranger = get(port, &format!("{target}?code=planted&state=guessed"));
            // Then the person's browser, back from the forge.
            let own = get(
                port,
                &format!("{target}?code={CODE}&state={}", query(&page, "state")),
            );
            let mut seen = seen.lock().unwrap();
            seen.page = Some(page);
            seen.stranger = Some(stranger);
            seen.back = Some(own);
        }));
        Ok(())
    };
    let endpoint = oauth::Https::new();
    let granted = pkce::sign_in(
        &HostScope::for_a_test(),
        gitlab,
        &AtomicBool::new(false),
        &pkce::Ways {
            bind: &bind,
            open: &open,
            exchange: &pkce::exchange_at(&endpoint, &endpoints),
            clock: loopback::Clock::system(),
        },
    )
    .expect("the sign-in came back");
    for browser in browsers.into_inner().unwrap() {
        browser.join().unwrap();
    }

    // The listener: on a registered port, refusing the stranger with a fixed page, answering the
    // browser with one, and gone once the redirect was taken.
    let port = bound.into_inner().unwrap().expect("a port was bound");
    assert!(REDIRECT_PORTS.contains(&port));
    let seen = seen.lock().unwrap();
    let page = seen.page.as_deref().unwrap();
    assert!(page.starts_with("https://gitlab.com/oauth/authorize?"));
    assert_eq!(
        query(page, "redirect_uri"),
        registration::redirect_uri(port)
    );
    let stranger = seen.stranger.as_deref().unwrap();
    assert!(
        stranger.starts_with("HTTP/1.1 404 Not Found\r\n"),
        "{stranger}"
    );
    assert!(!stranger.contains("planted") && !stranger.contains("guessed"));
    let back = seen.back.as_deref().unwrap();
    assert!(back.starts_with("HTTP/1.1 200 OK\r\n"), "{back}");
    assert!(back.contains("You can return to purlis"));
    assert!(!back.contains(CODE) && !back.contains(&query(page, "state")));
    let again = TcpListener::bind(SocketAddrV4::new(Ipv4Addr::LOCALHOST, port));
    assert!(
        again.is_ok(),
        "the listener was still bound after the redirect"
    );
    drop(again);

    // The exchange: one post, with the verifier whose challenge the page was shown, and no
    // secret.
    let posts: Vec<String> = rt
        .block_on(server.received_requests())
        .unwrap_or_default()
        .iter()
        .map(|request| String::from_utf8_lossy(&request.body).into_owned())
        .collect();
    let [form] = posts.as_slice() else {
        panic!("{} exchanges", posts.len())
    };
    let decoded = |raw: &str| {
        percent_encoding::percent_decode_str(raw)
            .decode_utf8()
            .unwrap()
            .into_owned()
    };
    // The form's names are percent-encoded too (`grant%5Ftype`).
    let field = |name: &str| {
        form.split('&')
            .filter_map(|pair| pair.split_once('='))
            .find(|(raw, _)| decoded(raw) == name)
            .map(|(_, raw)| decoded(raw))
    };
    assert_eq!(field("grant_type").as_deref(), Some("authorization_code"));
    assert_eq!(field("client_id").as_deref(), Some(gitlab.client_id));
    assert_eq!(
        field("redirect_uri"),
        Some(registration::redirect_uri(port))
    );
    assert_eq!(field("client_secret"), None);
    let verifier = field("code_verifier").expect("the verifier was posted");
    assert_eq!(
        pkce::Verifier::for_a_test(&verifier).challenge(),
        query(page, "code_challenge")
    );

    // The account: listed as signed in by the browser, its token in its own keyring item.
    let recorded = whose();
    let reach = |_: &Forge, _: Arc<dyn TokenSource>| -> Result<Arc<dyn Transport>, ForgeError> {
        Ok(recorded.clone() as Arc<dyn Transport>)
    };
    let keyring = accounts::keyring_at(config);
    let account = accounts::sign_in_with_grant(
        &HostScope::for_a_test(),
        config,
        keyring.as_ref(),
        &reach,
        Kind::GitLab,
        gitlab.host,
        granted,
        Method::Pkce,
    )
    .unwrap();
    assert_eq!(
        (
            account.kind,
            account.host.as_str(),
            account.login.as_str(),
            account.method,
            account.state
        ),
        (
            Kind::GitLab,
            "gitlab.com",
            "jdoe",
            Method::Pkce,
            State::SignedIn
        )
    );
    assert_eq!(account.scopes.as_deref(), Some(&["api".to_string()][..]));
    assert_eq!(
        accounts::list(config, keyring.as_ref()).unwrap(),
        vec![account.clone()]
    );
    assert_eq!(
        accounts::token(keyring.as_ref(), &account)
            .unwrap()
            .unwrap()
            .expose_secret(),
        ACCESS
    );
    // Nowhere else: no file of the config home holds the tokens, the code or the verifier.
    let searched = in_no_file(&[config], &[ACCESS, REFRESH, CODE, verifier.as_str()]);
    assert!(searched >= 1, "the accounts file was searched");
}

#[test]
fn with_all_three_ports_taken_the_sign_in_says_so_and_opens_no_browser() {
    purlis_core::unsteered!();
    let _turn = THE_PORTS.lock().unwrap_or_else(|e| e.into_inner());
    // Whatever is free of the three is taken here, so all three are in use either way.
    let _held: Vec<TcpListener> = REDIRECT_PORTS
        .iter()
        .filter_map(|port| TcpListener::bind(SocketAddrV4::new(Ipv4Addr::LOCALHOST, *port)).ok())
        .collect();
    let opened = Mutex::new(0);
    let open = |_: &str| {
        *opened.lock().unwrap() += 1;
        Ok(())
    };
    let exchange = |_: &[(&str, &str)]| -> Result<(), String> { panic!("exchanged") };
    let why = pkce::sign_in(
        &HostScope::for_a_test(),
        registration::on(Kind::GitLab, "gitlab.com").unwrap(),
        &AtomicBool::new(false),
        &pkce::Ways {
            bind: &loopback::bind,
            open: &open,
            exchange: &exchange,
            clock: loopback::Clock::system(),
        },
    )
    .unwrap_err();
    assert_eq!(
        why,
        "purlis could not take the browser's answer: ports 47823, 47824 and 47825 on this \
         machine are all in use by other software, and the sign-in can come back on no other. \
         Close what is using one of them and sign in again, or sign in with a token instead."
    );
    assert_eq!(*opened.lock().unwrap(), 0);
}

// ---------------------------------------------------------------------------------------
// FI3: a token this sign-in made reaches no chat.

/// Signs in on gitlab.com under `config` with what a browser sign-in is granted.
fn signed_in_by_the_browser(config: &Path) -> ForgeAccount {
    let granted = oauth::Answer::of(200, &grant()).granted().expect("a grant");
    let recorded = whose();
    let reach = |_: &Forge, _: Arc<dyn TokenSource>| -> Result<Arc<dyn Transport>, ForgeError> {
        Ok(recorded.clone() as Arc<dyn Transport>)
    };
    let keyring = accounts::keyring_at(config);
    let account = accounts::sign_in_with_grant(
        &HostScope::for_a_test(),
        config,
        keyring.as_ref(),
        &reach,
        Kind::GitLab,
        "gitlab.com",
        granted,
        Method::Pkce,
    )
    .unwrap();
    // The control: the token IS kept, in its own keyring item.
    assert_eq!(
        accounts::token(keyring.as_ref(), &account)
            .unwrap()
            .unwrap()
            .expose_secret(),
        ACCESS
    );
    account
}

/// A project with one workspace, as the MCP tool tests make one.
fn project() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("charter.toml"), "").unwrap();
    let found = purlis_core::workspaces::Plane::open(dir.path())
        .workspace("alpha")
        .unwrap();
    std::fs::create_dir_all(found.dir()).unwrap();
    found.scaffold_charter().unwrap();
    dir
}

/// A value for each argument a tool's schema requires.
fn arguments(schema: &serde_json::Value) -> serde_json::Map<String, serde_json::Value> {
    let mut args = serde_json::Map::new();
    for name in schema["required"].as_array().into_iter().flatten() {
        if let Some(name) = name.as_str() {
            let value = match schema["properties"][name]["type"]
                .as_str()
                .unwrap_or("string")
            {
                "integer" | "number" => serde_json::json!(1),
                "boolean" => serde_json::json!(false),
                "array" => serde_json::json!(["word"]),
                _ => serde_json::json!("word"),
            };
            args.insert(name.to_string(), value);
        }
    }
    args
}

/// Set on the child the FI3 case runs its body in: the config home the sign-in is kept at.
const CANARY_HOME: &str = "FI3_PKCE_CANARY_CONFIG_HOME";

#[test]
fn a_browser_sign_ins_tokens_are_in_no_chats_environment_tool_answer_or_file() {
    purlis_core::unsteered!();
    // The tools find the account store and the keyring through this process's config home, so
    // the body runs in a child whose config home is this test's own directory.
    let Some(config) = std::env::var_os(CANARY_HOME).map(PathBuf::from) else {
        let home = tempfile::tempdir().unwrap();
        purlis_core::testrun::rerun_steered(
            &[
                "a_gitlab_sign_in_through_the_browser::a_browser_sign_ins_tokens_are_in_no_chats_environment_tool_answer_or_file",
            ],
            &[
                (purlis_core::machine::HOME_VAR, home.path().as_os_str()),
                (CANARY_HOME, home.path().as_os_str()),
            ],
        );
        return;
    };
    assert_eq!(
        purlis_core::machine::config_root().as_deref(),
        Some(config.as_path()),
        "the tools read the config home the sign-in is kept at"
    );
    let plane = project();
    signed_in_by_the_browser(&config);
    let secrets = [ACCESS, REFRESH];

    // 1. The environment a chat starts with.
    let env = compose(
        std::env::vars_os(),
        &Starting {
            harness: None,
            operator: &["*".to_string()],
            strip: &[],
            set: &[],
            git_hooks: None,
            declared: &[],
        },
    );
    for (name, value) in &env {
        for secret in secrets {
            assert!(
                !value.to_string_lossy().contains(secret),
                "{} holds a token",
                name.to_string_lossy()
            );
        }
    }

    // 2. Every tool purlis's MCP server offers a chat.
    let now = chrono::NaiveDate::from_ymd_opt(2026, 10, 13)
        .unwrap()
        .and_hms_opt(9, 0, 0)
        .unwrap();
    let place = Place::Workspace("alpha".into());
    let mut called = 0;
    for tool in &purlis_core::chattools::TOOLS {
        let said = purlis_core::chattools::call(
            plane.path(),
            &place,
            tool.name,
            &arguments(&(tool.schema)()),
            now,
        );
        let said = match said {
            Ok(said) | Err(said) => said,
        };
        for secret in secrets {
            assert!(
                !said.contains(secret),
                "{} answered with a token",
                tool.name
            );
        }
        called += 1;
    }
    assert_eq!(called, purlis_core::chattools::TOOLS.len());

    // 3. Every file a chat could be handed: the project's, and the config home's but the
    // keyring's own.
    let searched = in_no_file(&[plane.path(), &config], &secrets);
    assert!(
        files(&config).iter().any(|p| p == &accounts::path(&config)),
        "the accounts file was searched"
    );
    assert!(searched > 1);
}

/// The browser sign-in is the window's: the `purlis` binary, which a chat can run, names
/// neither the flow nor the listener.
#[test]
fn the_purlis_binary_never_signs_in_through_the_browser() {
    purlis_core::unsteered!();
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("../purlis-cli/src");
    let mut read = 0;
    for path in files(&src) {
        if path.extension().is_some_and(|e| e == "rs") {
            let text = std::fs::read_to_string(&path).unwrap();
            read += 1;
            for word in ["pkce", "loopback", "sign_in_with_grant", "oauth::"] {
                assert!(
                    !text.contains(word),
                    "{} names {word}: the purlis binary signs in nowhere",
                    path.display()
                );
            }
        }
    }
    assert!(read > 0, "no purlis-cli source was read");
}
