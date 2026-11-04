// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A login whose form is on a sign-in provider's site, end to end on a real engine** (#5152,
//! the person's ruling of 2026-11-04): a fixture app that signs in on a fixture provider on
//! another port, and sends the browser back to the provider straight after the code comes back,
//! as an OpenID Connect app does. The logged-in browser keeps reaching both, and the chat may
//! act on the provider's pages only while a sign-in runs.
//!
//! It runs where `end_to_end`'s tests run: on Linux, on CI or with `PURLIS_TEST_PINNED_ENGINE`.

use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::{Arc, Mutex, PoisonError};

use serde_json::json;

use super::end_to_end::{
    Host, LOGIN, Window, a_real_engine_runs_here, in_homes_of_its_own, red_pixels, ref_on,
};

/// What the app's page says once the browser is signed in and back from the provider.
const SIGNED_IN: &str = "Signed in to the fixture app";

/// What the provider's frame inside the app's page shows, drawn in the fixture's red.
const PROVIDER_FRAME: &str = "Provider frame text";

/// What the provider's own account page shows.
const PROVIDER_ACCOUNT: &str = "Your provider account";

/// The fixture app and the fixture provider, each on a loopback port of its own, and the paths
/// each was sent.
struct Sites {
    app: u16,
    provider: u16,
    heard: Arc<Mutex<Vec<String>>>,
}

impl Sites {
    fn serve() -> Sites {
        let app = TcpListener::bind("127.0.0.1:0").expect("the app's port");
        let provider = TcpListener::bind("127.0.0.1:0").expect("the provider's port");
        let ports = (
            app.local_addr().expect("its address").port(),
            provider.local_addr().expect("its address").port(),
        );
        let heard: Arc<Mutex<Vec<String>>> = Arc::default();
        for (listener, role) in [(app, Role::App), (provider, Role::Provider)] {
            let heard = Arc::clone(&heard);
            std::thread::spawn(move || {
                for stream in listener.incoming().flatten() {
                    let heard = Arc::clone(&heard);
                    std::thread::spawn(move || answer(stream, role, ports, &heard));
                }
            });
        }
        Sites {
            app: ports.0,
            provider: ports.1,
            heard,
        }
    }

    fn app_at(&self, path: &str) -> String {
        format!("http://127.0.0.1:{}/{path}", self.app)
    }

    fn provider_at(&self, path: &str) -> String {
        format!("http://127.0.0.1:{}/{path}", self.provider)
    }

    fn heard(&self) -> Vec<String> {
        self.heard
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }
}

#[derive(Clone, Copy)]
enum Role {
    App,
    Provider,
}

/// One request to the app or the provider, answered as the fixture's routes say.
fn answer(stream: TcpStream, role: Role, (app, provider): (u16, u16), heard: &Mutex<Vec<String>>) {
    let mut reader = BufReader::new(stream.try_clone().expect("the stream twice"));
    let mut first = String::new();
    if reader.read_line(&mut first).is_err() {
        return;
    }
    let mut length = 0;
    loop {
        let mut header = String::new();
        match reader.read_line(&mut header) {
            Ok(0) | Err(_) => break,
            Ok(_) if header == "\r\n" || header == "\n" => break,
            Ok(_) => {
                if let Some((name, value)) = header.split_once(':')
                    && name.eq_ignore_ascii_case("content-length")
                {
                    length = value.trim().parse().unwrap_or(0);
                }
            }
        }
    }
    let mut body = vec![0; length];
    let _ = reader.read_exact(&mut body);
    let target = first.split_whitespace().nth(1).unwrap_or("/");
    // A request through a proxy names the whole address.
    let path = target
        .strip_prefix("http://")
        .and_then(|rest| rest.split_once('/').map(|(_, path)| path))
        .unwrap_or_else(|| target.trim_start_matches('/'));
    let path = path.split('?').next().unwrap_or_default();
    let provider_at = |path: &str| format!("http://127.0.0.1:{provider}/{path}");
    let app_at = |path: &str| format!("http://127.0.0.1:{app}/{path}");
    let (who, found) = match role {
        Role::App => ("app", app_page(path, &provider_at)),
        Role::Provider => ("provider", provider_page(path, &app_at)),
    };
    heard
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .push(format!("{who} /{path}"));
    let head = match &found {
        Page::Html(body) => format!(
            "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\n\
             Cache-Control: no-store\r\nConnection: close\r\n\r\n",
            body.len()
        ),
        Page::Moved(to) => format!(
            "HTTP/1.1 302 Found\r\nLocation: {to}\r\nContent-Length: 0\r\nCache-Control: \
             no-store\r\nConnection: close\r\n\r\n"
        ),
        Page::Missing => {
            "HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n".to_owned()
        }
    };
    let mut out = stream;
    let _ = out.write_all(head.as_bytes());
    if let Page::Html(body) = found {
        let _ = out.write_all(body.as_bytes());
    }
}

enum Page {
    Html(String),
    Moved(String),
    Missing,
}

fn html(title: &str, body: &str) -> Page {
    Page::Html(format!(
        "<!doctype html><html lang=\"en\"><head><meta charset=\"utf-8\"><title>{title}</title>\
         <style>body {{ margin: 0; font: 24px sans-serif; }}</style></head><body>{body}</body>\
         </html>"
    ))
}

/// The app: its page sends the browser to the provider to sign in; the code comes back to
/// `cb`, whose script sends the browser straight back to the provider, which sends it home.
fn app_page(path: &str, provider_at: &dyn Fn(&str) -> String) -> Page {
    match path {
        "" => html("Fixture app", "<h1>Fixture app</h1>"),
        "start" | "relogin" => Page::Moved(provider_at("login")),
        "cb" => html(
            "Signing in",
            &format!(
                "<p>Signing in</p><script>location.replace({:?})</script>",
                provider_at("again")
            ),
        ),
        "home" => html(
            "Fixture app home",
            &format!(
                "<h1>{SIGNED_IN}</h1><p><a href={:?}>Manage your account</a></p>\
                 <iframe title=\"Provider frame\" src={:?} width=\"400\" height=\"120\"></iframe>",
                provider_at("account"),
                provider_at("frame"),
            ),
        ),
        _ => Page::Missing,
    }
}

/// The provider: its login form, its answer to a sign-in, its silent check, its account page,
/// and the frame the app shows.
fn provider_page(path: &str, app_at: &dyn Fn(&str) -> String) -> Page {
    match path {
        "login" => html(
            "Provider sign in",
            "<h1>Sign in to the provider</h1><form action=\"signin\" method=\"post\">\
             <label>User name <input name=\"username\" autocomplete=\"username\"></label>\
             <label>Password <input type=\"password\" name=\"password\"></label>\
             <button type=\"submit\">Sign in</button></form>",
        ),
        "signin" => Page::Moved(app_at("cb?code=fixture")),
        "again" => Page::Moved(app_at("home")),
        "account" => html(
            "Provider account",
            &format!(
                "<h1>{PROVIDER_ACCOUNT}</h1><form action=\"consent\" method=\"post\">\
                 <button type=\"submit\" autofocus>Allow access</button></form>"
            ),
        ),
        "consent" => html("Consented", "<h1>Access allowed</h1>"),
        "frame" => html(
            "Provider frame",
            &format!(
                "<div style=\"background: rgb(255, 0, 0); color: rgb(255, 0, 0); height: \
                 100px\">{PROVIDER_FRAME}</div>"
            ),
        ),
        _ => Page::Missing,
    }
}

/// #5152: through every hop, a login on the fixture app whose form is on the fixture provider.
#[test]
fn a_logged_in_browser_reaches_its_sign_in_provider_and_the_chat_is_kept_off_its_pages() {
    if !a_real_engine_runs_here() {
        return;
    }
    in_homes_of_its_own(
        "browsers::tests::sign_in_provider::the_sign_in_provider_through_every_hop_in_homes_of_their_own",
    );
}

/// [`a_logged_in_browser_reaches_its_sign_in_provider_and_the_chat_is_kept_off_its_pages`], in
/// homes of its own.
#[test]
#[ignore = "run by a_logged_in_browser_reaches_its_sign_in_provider_and_the_chat_is_kept_off_its_pages in homes of its own"]
fn the_sign_in_provider_through_every_hop_in_homes_of_their_own() {
    if !a_real_engine_runs_here() {
        return;
    }
    let sites = Sites::serve();
    let app = purlis_core::sandbox::browser_reach::Site::of("127.0.0.1", sites.app);
    let provider = purlis_core::sandbox::browser_reach::Site::of("127.0.0.1", sites.provider);
    // A port of this machine is reached only where the person allowed it, as a public sign-in
    // host is by the login: both are allowed, and once logged in the browser is held to the
    // login's sites whatever else is allowed.
    let window = Window::declaring(
        Host::serving,
        &format!(
            "browser = \"{}\", form = \"http://127.0.0.1:{}\"",
            sites.app_at("start"),
            sites.provider
        ),
        &[app, provider],
    );
    let (chat, mut tools) = window.a_chat();
    let first = tools.call("browser_snapshot", json!({}));
    assert!(first.error, "{first:?}");
    crate::chatbrowsers::allowed(&window.held, chat, false).expect("the person allows it");
    window.opens(&mut tools, &sites.app_at(""));

    // The login fills on the provider, and the app sends the browser back to the provider
    // straight after the code comes back: the browser gets home.
    let logged_in = tools.done("browser_login", json!({ "login": LOGIN }));
    let home = tools.call(
        "browser_wait_for_text",
        json!({ "text": SIGNED_IN, "seconds": 20 }),
    );
    assert!(
        home.text.contains("The text was seen."),
        "the app's own hop back to the provider is carried: {}\nheard: {:?}",
        home.text,
        sites.heard()
    );
    assert!(
        sites.heard().iter().any(|one| one == "provider /again"),
        "{:?}",
        sites.heard()
    );

    // The login's answer lists the hosts the browser reaches, and what the chat may do on each.
    let (app_host, provider_host) = (
        format!("127.0.0.1:{}", sites.app),
        format!("127.0.0.1:{}", sites.provider),
    );
    assert!(
        logged_in.text.contains(&app_host)
            && logged_in.text.contains(&provider_host)
            && logged_in.text.contains("browser_login"),
        "{}",
        logged_in.text
    );

    // The provider's frame inside the app's page loads, and nothing of it reaches the chat.
    let snapshot = tools.done("browser_snapshot", json!({}));
    assert!(snapshot.text.contains(SIGNED_IN), "{}", snapshot.text);
    assert!(
        sites.heard().iter().any(|one| one == "provider /frame"),
        "{:?}",
        sites.heard()
    );
    assert!(!snapshot.text.contains(PROVIDER_FRAME), "{}", snapshot.text);
    let found = tools.call("browser_find", json!({ "text": PROVIDER_FRAME }));
    assert!(!found.text.contains("[e"), "{}", found.text);
    let shot = tools.done("browser_screenshot", json!({}));
    assert_eq!(
        red_pixels(&shot.images[0]),
        0,
        "the provider's frame is painted over"
    );

    // An address on the provider is refused, with the reason.
    let went = tools.call(
        "browser_navigate",
        json!({ "address": sites.provider_at("account") }),
    );
    assert!(went.error, "{}", went.text);
    assert!(went.text.contains(&provider_host), "{}", went.text);
    let opened = tools.call(
        "browser_open",
        json!({ "address": sites.provider_at("account") }),
    );
    assert!(opened.error, "{}", opened.text);

    // The app's own link to the provider is followed; on the provider's page every read and act
    // of the chat's is refused, with the reason.
    let link = ref_on(&snapshot.text, "Manage your account");
    let clicked = tools.done("browser_click", json!({ "target": link }));
    assert!(clicked.text.contains(&provider_host), "{}", clicked.text);
    for (tool, arguments) in [
        ("browser_snapshot", json!({})),
        ("browser_find", json!({ "text": "Allow" })),
        ("browser_screenshot", json!({})),
        ("browser_press", json!({ "keys": ["Enter"] })),
        ("browser_type", json!({ "fields": [{ "text": "x" }] })),
        ("browser_wait_for_text", json!({ "text": PROVIDER_ACCOUNT })),
    ] {
        let refused = tools.call(tool, arguments);
        assert!(refused.error, "{tool}: {}", refused.text);
        assert!(
            refused.text.contains(&provider_host),
            "{tool}: {}",
            refused.text
        );
        assert!(
            !refused.text.contains(PROVIDER_ACCOUNT),
            "{tool}: {}",
            refused.text
        );
    }
    assert!(
        !sites.heard().iter().any(|one| one == "provider /consent"),
        "nothing the chat did reached the provider's form: {:?}",
        sites.heard()
    );
    tools.done("browser_back", json!({}));

    // The provider shows its form again: the chat is told to call browser_login.
    tools.done(
        "browser_navigate",
        json!({ "address": sites.app_at("relogin") }),
    );
    let asks = tools.call("browser_snapshot", json!({}));
    assert!(asks.error, "{}", asks.text);
    assert!(asks.text.contains("browser_login"), "{}", asks.text);
    // And a login signs in anew.
    tools.done("browser_login", json!({ "login": LOGIN }));
    let again = tools.call(
        "browser_wait_for_text",
        json!({ "text": SIGNED_IN, "seconds": 20 }),
    );
    assert!(again.text.contains("The text was seen."), "{}", again.text);
}
