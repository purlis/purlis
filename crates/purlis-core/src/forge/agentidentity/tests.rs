// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use std::io::{Read as _, Write as _};
use std::sync::Mutex;

use super::*;
use crate::forge::oauth::Answer;
use crate::forge::transport::{Endpoint as Path_, NoAnswer};

/// Not a key: the shape [`mint::a_private_key`] looks for, written so nothing takes it for one.
fn pem() -> String {
    let kind = "RSA PRIVATE";
    format!("-----BEGIN {kind} KEY-----
abc
-----END {kind} KEY-----
")
}

fn acme() -> Wanted {
    Wanted {
        owner: Owner::Organisation("acme".to_owned()),
        workflows: false,
    }
}

fn app_answer(owner: &str, kind: &str) -> serde_json::Value {
    serde_json::json!({
        "client_id": "Iv23liExample",
        "slug": "acme-purlis-agent",
        "client_secret": "not-kept",
        "webhook_secret": "not-kept",
        "pem": pem(),
        "owner": { "login": owner, "type": kind },
        "permissions": {
            "contents": "write", "issues": "write", "metadata": "read", "pull_requests": "write"
        },
        "events": [],
    })
}

/// GitHub's conversion endpoint, as a test sets it, recording what was asked.
struct Conversions {
    answer: (u16, String),
    asked: Mutex<Vec<String>>,
}

impl Conversions {
    fn say(status: u16, body: &serde_json::Value) -> Conversions {
        Conversions {
            answer: (status, body.to_string()),
            asked: Mutex::new(Vec::new()),
        }
    }
}

impl Endpoint for Conversions {
    fn post(&self, url: &str, form: &[(&str, &str)]) -> Result<Answer, String> {
        assert!(form.is_empty(), "a conversion sends nothing but its code");
        self.asked.lock().unwrap().push(url.to_owned());
        Ok(Answer::of(self.answer.0, &self.answer.1))
    }
}

/// A request head to `port`, answered with status line and body.
fn get(port: u16, target: &str) -> String {
    let mut stream = std::net::TcpStream::connect(("127.0.0.1", port)).expect("connects");
    write!(
        stream,
        "GET {target} HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\n\r\n"
    )
    .expect("sends");
    let mut answer = String::new();
    stream.read_to_string(&mut answer).expect("reads");
    answer
}

/// The value of `name=` in a page, up to its closing quote.
fn after<'a>(text: &'a str, name: &str) -> &'a str {
    let from = text.find(name).expect("in the page") + name.len();
    let rest = &text[from..];
    &rest[..rest.find('"').expect("closes")]
}

#[test]
fn the_manifest_asks_q5s_permissions_with_its_webhook_off() {
    let sent: serde_json::Value =
        serde_json::from_str(&manifest(&acme(), "http://127.0.0.1:1/back")).unwrap();
    assert_eq!(
        sent["default_permissions"],
        serde_json::json!({
            "contents": "write", "issues": "write", "metadata": "read", "pull_requests": "write"
        })
    );
    assert_eq!(sent["public"], false);
    assert_eq!(sent["hook_attributes"]["active"], false);
    assert_eq!(sent["default_events"], serde_json::json!([]));
    assert_eq!(sent["redirect_url"], "http://127.0.0.1:1/back");
    assert_eq!(sent["name"], "acme-purlis-agent");
}

#[test]
fn the_workflows_box_adds_workflows_and_nothing_else() {
    let wanted = Wanted {
        workflows: true,
        ..acme()
    };
    let sent: serde_json::Value = serde_json::from_str(&manifest(&wanted, "x")).unwrap();
    assert_eq!(sent["default_permissions"]["workflows"], "write");
    assert_eq!(sent["default_permissions"].as_object().unwrap().len(), 5);
}

#[test]
fn an_app_name_fits_what_github_takes() {
    let wanted = Wanted {
        owner: Owner::Organisation("a".repeat(39)),
        workflows: false,
    };
    assert!(wanted.app_name().len() <= 34, "{}", wanted.app_name());
}

#[test]
fn the_page_goes_to_the_owners_own_address_and_refuses_a_name_that_cant_be_one() {
    assert_eq!(
        new_app_page(&Owner::Organisation("acme".into()), "s").unwrap(),
        "https://github.com/organizations/acme/settings/apps/new?state=s"
    );
    assert_eq!(
        new_app_page(&Owner::Personal(None), "s").unwrap(),
        "https://github.com/settings/apps/new?state=s"
    );
    assert!(new_app_page(&Owner::Organisation("a/../b".into()), "s").is_err());
}

#[test]
fn the_page_escapes_the_manifest_and_may_post_only_to_github() {
    let (page, policy) = post_page("https://github.com/x?a=1&b=2", r#"{"a":"<b>&"}"#);
    assert!(page.contains("action=\"https://github.com/x?a=1&amp;b=2\""));
    assert!(page.contains("value=\"{&quot;a&quot;:&quot;&lt;b&gt;&amp;&quot;}\""));
    assert!(
        policy.contains("form-action https://github.com;"),
        "{policy}"
    );
    assert!(policy.starts_with("default-src 'none'"), "{policy}");
    assert!(!policy.contains("unsafe-inline"), "{policy}");
}

#[test]
fn a_redirect_with_the_right_state_gets_the_app_and_any_other_request_gets_nothing() {
    let conversions = Conversions::say(201, &app_answer("acme", "Organization"));
    let cancelled = AtomicBool::new(false);
    let wanted = acme();
    std::thread::scope(|scope| {
        let (port_tx, port_rx) = std::sync::mpsc::channel();
        let open = |address: &str| {
            port_tx.send(address.to_owned()).unwrap();
            Ok(())
        };
        let browser = scope.spawn(move || {
            let address: String = port_rx.recv().expect("the browser is opened");
            let rest = address.strip_prefix("http://127.0.0.1:").unwrap();
            let (port, path) = rest.split_once('/').unwrap();
            let port: u16 = port.parse().unwrap();
            // Without the one-off address there is no page, so no state to find.
            assert!(get(port, "/purlis/agent-identity/start/guess").contains("404"));
            let page = get(port, &format!("/{path}"));
            assert!(page.contains("200 OK"), "{page}");
            assert!(page.contains("form-action https://github.com"), "{page}");
            let action = after(&page, "action=\"");
            let state = action.split("state=").nth(1).expect("a state").to_owned();
            // A forged redirect is refused, and the flow goes on waiting.
            assert!(get(port, "/purlis/agent-identity/back?code=evil&state=wrong").contains("404"));
            assert!(get(port, "/purlis/agent-identity/back?code=evil").contains("404"));
            let back = get(
                port,
                &format!("/purlis/agent-identity/back?code=abc123&state={state}"),
            );
            assert!(back.contains("GitHub made the App"), "{back}");
            assert!(
                !back.contains("abc123"),
                "a page holds nothing of the request"
            );
            // One redirect: the listener is closed now.
            let closed = (0..100).any(|_| {
                std::thread::sleep(std::time::Duration::from_millis(20));
                std::net::TcpStream::connect(("127.0.0.1", port)).is_err()
            });
            assert!(closed, "the listener stays open after its one redirect");
        });
        let created = create_app(
            &wanted,
            &cancelled,
            &Ways {
                bind: &loopback::bind_any,
                open: &open,
                exchange: &conversions,
                clock: loopback::Clock::system(),
            },
        )
        .expect("made");
        browser.join().expect("the browser's part held");
        assert_eq!(created.app, "Iv23liExample");
        assert_eq!(
            created.install_page(),
            "https://github.com/apps/acme-purlis-agent/installations/new"
        );
    });
    assert_eq!(
        *conversions.asked.lock().unwrap(),
        ["https://api.github.com/app-manifests/abc123/conversions"]
    );
}

fn code(value: &str) -> Code {
    let state = State::for_a_test("s");
    match pkce::read_redirect_at("/b", &format!("/b?code={value}&state=s"), &state) {
        Redirected::Code(code) => code,
        _ => panic!("a code"),
    }
}

#[test]
fn a_code_that_could_leave_its_path_is_never_sent() {
    let conversions = Conversions::say(201, &app_answer("acme", "Organization"));
    let refused = convert(&conversions, &code("..%2f..%2fuser"), &acme());
    assert!(refused.is_err());
    assert!(conversions.asked.lock().unwrap().is_empty());
}

#[test]
fn an_app_of_another_owner_is_dropped_with_its_key() {
    for (login, kind) in [("evil", "Organization"), ("acme", "User")] {
        let refused = convert(
            &Conversions::say(201, &app_answer(login, kind)),
            &code("c"),
            &acme(),
        );
        assert!(refused.unwrap_err().contains("another account"));
    }
}

#[test]
fn an_app_that_can_do_more_than_was_asked_is_dropped() {
    let mut wider = app_answer("acme", "Organization");
    wider["permissions"]["administration"] = "write".into();
    let refused = convert(&Conversions::say(201, &wider), &code("c"), &acme());
    assert!(refused.unwrap_err().contains("more than purlis asked"));

    let mut raised = app_answer("acme", "Organization");
    raised["permissions"]["metadata"] = "write".into();
    assert!(convert(&Conversions::say(201, &raised), &code("c"), &acme()).is_err());

    let mut listens = app_answer("acme", "Organization");
    listens["events"] = serde_json::json!(["push"]);
    assert!(convert(&Conversions::say(201, &listens), &code("c"), &acme()).is_err());
}

#[test]
fn what_github_refuses_and_what_is_not_a_key_keep_nothing() {
    let refused = convert(
        &Conversions::say(404, &serde_json::json!({})),
        &code("c"),
        &acme(),
    );
    assert!(refused.unwrap_err().contains("HTTP 404"));
    let mut no_key = app_answer("acme", "Organization");
    no_key["pem"] = "hello".into();
    assert!(convert(&Conversions::say(201, &no_key), &code("c"), &acme()).is_err());
}

#[test]
fn a_created_app_never_prints_its_key() {
    let created = convert(
        &Conversions::say(201, &app_answer("acme", "Organization")),
        &code("c"),
        &acme(),
    )
    .unwrap();
    assert!(!format!("{created:?}").contains("PRIVATE KEY"));
}

#[test]
fn a_reserved_key_names_its_app_and_nothing_else_does() {
    assert_eq!(app_of(&key_of("Iv23li")), Some("Iv23li"));
    assert_eq!(app_of("agent-identity-other"), None);
    assert_eq!(app_of("github-Iv23li"), None);
}

#[test]
fn finish_writes_the_lines_it_shows_and_keeps_the_rest_of_the_file() {
    let before = "# mine\nschema = 1\n\n[sandbox.personas.dev]\nhosts = [\"a.example\"]\n";
    let after = with_lines(before, "Iv23li", &["dev".to_owned(), "qa".to_owned()]).unwrap();
    assert!(after.starts_with("# mine\nschema = 1\n"), "{after}");
    assert!(after.contains("hosts = [\"a.example\"]"), "{after}");
    let cfg: toml::Table = after.parse().unwrap();
    for persona in ["dev", "qa"] {
        let gh = &cfg["sandbox"]["personas"][persona]["tools"]["gh"];
        assert_eq!(gh["vault"].as_str(), Some("agent-identity"));
        assert_eq!(gh["mint"].as_str(), Some("github-app"));
        assert_eq!(gh["app"].as_str(), Some("Iv23li"));
        assert_eq!(gh["key"].as_str(), Some("agent-identity-github-Iv23li"));
    }
    let shown = lines_shown("Iv23li", &["dev".to_owned()]);
    assert_eq!(
        shown,
        "[sandbox.personas.dev.tools]\ngh = { vault = \"agent-identity\", mint = \"github-app\", \
         app = \"Iv23li\", key = \"agent-identity-github-Iv23li\" }"
    );
    assert_eq!(
        with_lines(&after, "Iv23li", &["dev".to_owned()]).unwrap(),
        after
    );
}

#[test]
fn finish_adds_to_a_tools_table_written_inline() {
    let before = "[sandbox.personas.dev]\ntools = { aws = { vault = \"v\", as = \"AWS env\", key = \"k\" } }\n";
    let after = with_lines(before, "Iv23li", &["dev".to_owned()]).unwrap();
    let cfg: toml::Table = after.parse().unwrap();
    assert_eq!(
        cfg["sandbox"]["personas"]["dev"]["tools"]["gh"]["app"].as_str(),
        Some("Iv23li")
    );
    assert!(
        cfg["sandbox"]["personas"]["dev"]["tools"]
            .get("aws")
            .is_some()
    );
}

#[test]
fn finish_replaces_no_gh_line_the_person_did_not_see() {
    let before = "[sandbox.personas.dev.tools]\ngh = { vault = \"v\", as = \"GH_TOKEN env\", key = \"k\" }\n";
    let refused = with_lines(before, "Iv23li", &["dev".to_owned()]).unwrap_err();
    assert!(refused.contains("dev"), "{refused}");
}

#[test]
fn removing_takes_out_only_that_apps_lines() {
    let text = with_lines("", "Iv23li", &["dev".to_owned(), "qa".to_owned()]).unwrap();
    let text = format!(
        "{text}\n[sandbox.personas.other.tools]\ngh = {{ vault = \"v\", mint = \"github-app\", \
         app = \"Other\", key = \"k\" }}\n"
    );
    let cfg: toml::Table = without_lines(&text, "Iv23li").unwrap().parse().unwrap();
    assert!(cfg["sandbox"]["personas"]["dev"].get("tools").is_none());
    assert!(cfg["sandbox"]["personas"]["qa"].get("tools").is_none());
    assert_eq!(
        cfg["sandbox"]["personas"]["other"]["tools"]["gh"]["app"].as_str(),
        Some("Other")
    );
}

#[test]
fn held_reads_each_app_with_its_personas() {
    let root = tempfile::tempdir().unwrap();
    add_lines(root.path(), "Iv23li", &["dev".to_owned(), "qa".to_owned()]).unwrap();
    assert_eq!(
        held(root.path()).unwrap(),
        [Held {
            app: "Iv23li".to_owned(),
            vault: VAULT.to_owned(),
            key: key_of("Iv23li"),
            personas: vec!["dev".to_owned(), "qa".to_owned()],
        }]
    );
    // What Finish wrote is what the mint mark and the tool reader take.
    assert_eq!(
        mint::marked(root.path()).unwrap(),
        [(VAULT.to_owned(), key_of("Iv23li"))]
    );
}

/// GitHub's API as a test sets it, by path, recording each request.
struct Api {
    answers: Vec<(&'static str, serde_json::Value)>,
    asked: Mutex<Vec<String>>,
}

impl Api {
    fn say(answers: Vec<(&'static str, serde_json::Value)>) -> Api {
        Api {
            answers,
            asked: Mutex::new(Vec::new()),
        }
    }
}

impl Transport for Api {
    fn send(&self, _: &Forge, call: &Call) -> Result<Reply, NoAnswer> {
        let Path_::Rest { path, .. } = &call.endpoint else {
            panic!("REST only");
        };
        self.asked.lock().unwrap().push(path.clone());
        let found = self.answers.iter().find(|(at, _)| at == path);
        Ok(Reply {
            code: i32::from(found.is_none()),
            out: found.map_or(String::new(), |(_, body)| body.to_string()),
            err: found.map_or("not found (HTTP 404)".to_owned(), |_| String::new()),
            status: Some(if found.is_some() { 200 } else { 404 }),
            headers: Vec::new(),
        })
    }

    fn check_auth(&self, _: &Forge) -> Result<(), crate::forge::ForgeError> {
        Ok(())
    }
}

fn the_app() -> (&'static str, serde_json::Value) {
    (
        "app",
        serde_json::json!({
            "slug": "acme-purlis-agent", "owner": { "login": "acme", "type": "Organization" }
        }),
    )
}

#[test]
fn an_app_not_installed_yet_is_said_so_and_mints_nothing() {
    let jwt = Api::say(vec![
        the_app(),
        ("app/installations?per_page=100", serde_json::json!([])),
    ]);
    let stood = stand_over(&jwt).unwrap();
    assert!(stood.installations.is_empty());
    let refused = prove_over(&jwt, &|_| panic!("no token")).unwrap_err();
    assert!(refused.contains("isn't installed"), "{refused}");
}

#[test]
fn finish_proves_a_real_token_reads_with_it_and_revokes_it() {
    let jwt = Api::say(vec![
        the_app(),
        (
            "app/installations?per_page=100",
            serde_json::json!([{ "id": 7 }]),
        ),
        (
            "app/installations/7/access_tokens",
            serde_json::json!({ "token": "ghs_real" }),
        ),
    ]);
    let token = std::sync::Arc::new(Api::say(vec![
        (
            "installation/repositories?per_page=1",
            serde_json::json!({ "total_count": 12, "repositories": [{ "full_name": "acme/web" }] }),
        ),
        ("repos/acme/web", serde_json::json!({})),
        ("installation/token", serde_json::json!({})),
    ]));
    struct Shared(std::sync::Arc<Api>);
    impl Transport for Shared {
        fn send(&self, forge: &Forge, call: &Call) -> Result<Reply, NoAnswer> {
            self.0.send(forge, call)
        }
        fn check_auth(&self, forge: &Forge) -> Result<(), crate::forge::ForgeError> {
            self.0.check_auth(forge)
        }
    }
    let shared = token.clone();
    let proof = prove_over(&jwt, &move |given| {
        assert_eq!(given, "ghs_real");
        Ok(Box::new(Shared(shared.clone())) as Box<dyn Transport>)
    })
    .unwrap();
    assert_eq!(proof.repos, 12);
    assert_eq!(proof.read.as_deref(), Some("acme/web"));
    assert_eq!(proof.stand.acts_as(), "acme-purlis-agent");
    assert_eq!(
        proof.stand.settings_page(),
        "https://github.com/organizations/acme/settings/apps/acme-purlis-agent"
    );
    assert_eq!(
        token.asked.lock().unwrap().last().map(String::as_str),
        Some("installation/token"),
        "the proof's token is revoked last"
    );
}
