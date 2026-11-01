// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **Agent identity: a GitHub App from a manifest, from the window** (#4485, rulings Q1 to Q23 on
//! #4464; ADR 0077 as amended 2026-10-29). What a chat acts as on a forge, and never a person's
//! forge account.
//!
//! **The flow, in the window's process and nowhere else:**
//!
//! 1. A listener binds a port the system picks on `127.0.0.1` ([`loopback::bind_any`]).
//! 2. The browser is sent to a one-off address on it, which answers a page holding the manifest
//!    and a `state`, and posts them to GitHub. GitHub takes a manifest only by a post, so a link
//!    alone can't do this. Only the holder of that address gets the page.
//! 3. GitHub sends the browser back with a one-time code and that `state`. Any other request is
//!    refused by the listener's own rules, and the first that matches closes it.
//! 4. The code is exchanged at once ([`convert`]). The answer is held to what was asked: the
//!    owner, the permissions, no events. The client secret and webhook secret are wiped unread.
//! 5. The key goes straight to a vault under [`mint::RESERVED_PREFIX`] ([`keep_key`]), so it is
//!    marked from the moment it is stored: no read but the mint's ever gets it, before or after
//!    the tool line exists. Nothing of it crosses to a page, a chat or a log.
//!
//! **Everything else is read fresh.** The tool lines in the project file say which Apps are set
//! up ([`held`]), the vault says which wait for Finish ([`pending`]), and GitHub says who an App
//! acts as and on how many repos ([`stand`], [`prove`]). There is no new store.

use std::path::Path;
use std::sync::atomic::AtomicBool;
use std::time::Duration;

use base64::Engine as _;
use sha2::{Digest as _, Sha256};
use zeroize::Zeroizing;

use super::apptoken;
use super::loopback::{self, Heard, Listening, Stopped};
use super::oauth::Endpoint;
use super::pkce::{self, Code, Redirected, State};
use super::transport::{Call, Field, Method, Reply, Transport};
use super::{Forge, Kind};
use crate::sandbox::tools::mint;
use crate::secrets::{Ctx, cmd, registry};

/// The vault Agent identity keeps an App's key in, made in the keyring when there is none.
pub const VAULT: &str = "agent-identity";

/// The one forge host this flow makes an App on. Others are #3882's.
pub const HOST: &str = apptoken::HOST;

/// How long the person has on GitHub's page before the listener closes.
pub const ALLOWED: Duration = Duration::from_secs(15 * 60);

/// Where the browser is first sent, under a word of its own.
const START_PATH: &str = "/purlis/agent-identity/start/";

/// Where GitHub sends the browser back.
const BACK_PATH: &str = "/purlis/agent-identity/back";

/// The longest code taken. GitHub's are far shorter.
const MOST_CODE: usize = 256;

/// The longest name GitHub takes for an App.
const MOST_APP_NAME: usize = 34;

/// How long a call to GitHub's API waits.
const CALL_WAITS: Duration = Duration::from_secs(20);

/// The page the browser shows once the code is taken. Fixed text; nothing of the request.
const CAME_BACK_PAGE: &str = "<!doctype html>\n<html lang=\"en\"><head><meta charset=\"utf-8\">\
<title>purlis</title></head><body><h1>GitHub made the App</h1>\
<p>purlis opens the page for choosing its repos next. You can close this tab.</p></body></html>\n";

/// The page for a redirect that said no.
const DECLINED_PAGE: &str = "<!doctype html>\n<html lang=\"en\"><head><meta charset=\"utf-8\">\
<title>purlis</title></head><body><h1>GitHub did not make the App</h1>\
<p>Return to purlis to try again. You can close this tab.</p></body></html>\n";

/// The one script of the page that posts the manifest: its hash is the only script its content
/// policy allows.
const POST_THE_FORM: &str = "document.getElementById(\"f\").submit()";

/// **Who holds the App** (Q3): the organisation or personal account that holds the repos.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Owner {
    Organisation(String),
    /// The person's own account, under the name purlis knows it by, where it knows one.
    Personal(Option<String>),
}

/// What the person chose.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Wanted {
    pub owner: Owner,
    /// Q5's box: also let it change CI workflows. Off by default.
    pub workflows: bool,
}

/// Whether `name` can be a GitHub account's.
fn account_ok(name: &str) -> bool {
    (1..=39).contains(&name.len())
        && name.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-')
        && !name.starts_with('-')
}

/// Whether `slug` can be a GitHub App's.
fn slug_ok(slug: &str) -> bool {
    (1..=64).contains(&slug.len())
        && slug
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_'))
}

/// A client id as a tool line takes it.
fn client_id_ok(app: &str) -> bool {
    (1..=64).contains(&app.len())
        && app
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'-'))
}

impl Wanted {
    /// The permissions the App asks for (Q5).
    fn permissions(&self) -> Vec<(&'static str, &'static str)> {
        let mut asked = vec![
            ("contents", "write"),
            ("issues", "write"),
            ("metadata", "read"),
            ("pull_requests", "write"),
        ];
        if self.workflows {
            asked.push(("workflows", "write"));
        }
        asked
    }

    /// The name the App is first offered under. GitHub's page lets the person change a taken one.
    fn app_name(&self) -> String {
        const AGENT: &str = "purlis-agent";
        match &self.owner {
            Owner::Organisation(name) | Owner::Personal(Some(name)) => {
                let room = MOST_APP_NAME - AGENT.len() - 1;
                let short: String = name.chars().take(room).collect();
                format!("{}-{AGENT}", short.trim_end_matches('-'))
            }
            Owner::Personal(None) => AGENT.to_owned(),
        }
    }
}

/// **The manifest GitHub is sent** (Q5), its webhook off, and private to the owner.
#[must_use]
pub fn manifest(wanted: &Wanted, redirect: &str) -> String {
    let permissions: serde_json::Map<String, serde_json::Value> = wanted
        .permissions()
        .into_iter()
        .map(|(name, level)| (name.to_owned(), serde_json::Value::from(level)))
        .collect();
    serde_json::json!({
        "name": wanted.app_name(),
        "url": "https://purlis.com",
        "description": "What chats act as on GitHub, made from purlis.",
        "public": false,
        "redirect_url": redirect,
        "hook_attributes": { "url": "https://purlis.com", "active": false },
        "default_events": [],
        "default_permissions": permissions,
    })
    .to_string()
}

/// **The GitHub page a manifest is posted to**, or why the owner can't be named in one.
pub fn new_app_page(owner: &Owner, state: &str) -> Result<String, String> {
    match owner {
        Owner::Organisation(name) if account_ok(name) => Ok(format!(
            "https://{HOST}/organizations/{name}/settings/apps/new?state={state}"
        )),
        Owner::Organisation(name) => Err(format!(
            "{} is not a GitHub organisation's name.",
            crate::shown::short(name)
        )),
        Owner::Personal(_) => Ok(format!("https://{HOST}/settings/apps/new?state={state}")),
    }
}

/// `text` safe inside a double-quoted HTML attribute.
fn attribute(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('"', "&quot;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

/// **The page that posts the manifest**, with no other way out: answers its body and its content
/// policy, which lets it post only to GitHub and run only its one script. A browser that runs no
/// script still has the button.
fn post_page(action: &str, manifest: &str) -> (String, String) {
    let hash = base64::engine::general_purpose::STANDARD
        .encode(Sha256::digest(POST_THE_FORM.as_bytes()).as_slice());
    let policy = format!(
        "default-src 'none'; form-action https://{HOST}; base-uri 'none'; script-src 'sha256-{hash}'"
    );
    let body = format!(
        "<!doctype html>\n<html lang=\"en\"><head><meta charset=\"utf-8\"><title>purlis</title>\
         </head><body><h1>Going on to GitHub</h1><form id=\"f\" method=\"post\" action=\"{}\">\
         <input type=\"hidden\" name=\"manifest\" value=\"{}\">\
         <button type=\"submit\">Continue to GitHub</button></form>\
         <script>{POST_THE_FORM}</script></body></html>\n",
        attribute(action),
        attribute(manifest),
    );
    (body, policy)
}

/// **An App GitHub just made.** Its key is held only until [`keep_key`] stores it.
pub struct Created {
    pub app: String,
    pub slug: String,
    pub owner: String,
    pub personal: bool,
    pem: Zeroizing<String>,
}

impl std::fmt::Debug for Created {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Created")
            .field("app", &self.app)
            .field("slug", &self.slug)
            .field("owner", &self.owner)
            .finish_non_exhaustive()
    }
}

impl Created {
    /// The page where the person installs the App on their repos.
    #[must_use]
    pub fn install_page(&self) -> String {
        format!("https://{HOST}/apps/{}/installations/new", self.slug)
    }
}

/// **Exchanges GitHub's one-time code for the App** and holds the answer to what was asked.
/// Refused with a sentence that holds nothing GitHub sent but its status: a code that isn't a
/// code is never sent, and an answer for another owner, wider permissions or any event is
/// dropped with its key.
pub fn convert(endpoint: &dyn Endpoint, code: &Code, wanted: &Wanted) -> Result<Created, String> {
    let code = code.expose();
    let shaped = (1..=MOST_CODE).contains(&code.len())
        && code
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_'));
    if !shaped {
        return Err("GitHub's answer wasn't a code purlis can use. Start over.".to_owned());
    }
    let url = format!("https://api.{HOST}/app-manifests/{code}/conversions");
    let mut answer = endpoint
        .post(&url, &[])
        .map_err(|why| format!("GitHub could not be asked for the App: {why}"))?;
    // Neither is ever used: an App's webhook is off, and it signs in with no secret.
    drop(answer.secret("client_secret"));
    drop(answer.secret("webhook_secret"));
    if !(200..300).contains(&answer.status) {
        return Err(format!(
            "GitHub refused to hand over the App (HTTP {}). Its code works once and for an hour. \
             Start over.",
            answer.status
        ));
    }
    let owner = answer.value("owner");
    let owner_text = |key: &str| {
        owner
            .and_then(|owner| owner.get(key))
            .and_then(serde_json::Value::as_str)
            .unwrap_or("")
            .to_owned()
    };
    let login = owner_text("login");
    let kind = owner_text("type");
    let right_owner = match &wanted.owner {
        Owner::Organisation(name) => kind == "Organization" && login.eq_ignore_ascii_case(name),
        Owner::Personal(Some(name)) => kind == "User" && login.eq_ignore_ascii_case(name),
        Owner::Personal(None) => kind == "User",
    };
    if !right_owner || !account_ok(&login) {
        return Err(
            "GitHub made the App for another account than you chose, so purlis kept \
                    nothing. Delete it on GitHub and start over."
                .to_owned(),
        );
    }
    let asked = wanted.permissions();
    let wider = answer
        .value("permissions")
        .and_then(serde_json::Value::as_object)
        .is_some_and(|given| {
            given.iter().any(|(name, level)| {
                let wanted = asked
                    .iter()
                    .find(|(asked_name, _)| *asked_name == name.as_str())
                    .map(|(_, level)| *level);
                !matches!(
                    (wanted, level.as_str()),
                    (Some("write"), Some("read" | "write")) | (Some("read"), Some("read"))
                )
            })
        });
    let listens = answer
        .value("events")
        .and_then(serde_json::Value::as_array)
        .is_some_and(|events| !events.is_empty());
    if wider || listens {
        return Err(
            "The App GitHub made can do more than purlis asked for, so purlis kept \
                    nothing. Delete it on GitHub and start over."
                .to_owned(),
        );
    }
    let app = answer.text("client_id").unwrap_or("").to_owned();
    let slug = answer.text("slug").unwrap_or("").to_owned();
    let pem = answer.secret("pem");
    let Some(pem) = pem.filter(|_| client_id_ok(&app) && slug_ok(&slug)) else {
        return Err("GitHub's answer didn't hold what an App needs. Start over.".to_owned());
    };
    let pem = Zeroizing::new(secrecy::ExposeSecret::expose_secret(&pem).to_owned());
    if !mint::a_private_key(&pem) {
        return Err("GitHub's answer didn't hold what an App needs. Start over.".to_owned());
    }
    Ok(Created {
        app,
        slug,
        personal: kind == "User",
        owner: login,
        pem,
    })
}

/// What the flow needs from where it runs.
pub struct Ways<'a> {
    pub bind: &'a dyn Fn() -> Result<Listening, String>,
    /// Opens an address in the person's browser.
    pub open: &'a dyn Fn(&str) -> Result<(), String>,
    pub exchange: &'a dyn Endpoint,
    pub clock: loopback::Clock<'a>,
}

/// What a listener's stop means to the person.
fn stopped_said(why: Stopped) -> String {
    match why {
        Stopped::TimedOut => "GitHub's page wasn't finished in time. Start over.",
        Stopped::Cancelled => "Stopped. No App was kept.",
        Stopped::Declined(_) => "GitHub didn't make the App. Start over.",
        Stopped::Strangers => {
            "Too many requests that weren't GitHub's reached purlis's listener, so it stopped. \
             Start over."
        }
        Stopped::Failed => "purlis's listener failed. Start over.",
    }
    .to_owned()
}

/// **The whole of the browser's part**: the page opened, its one redirect taken, the code
/// exchanged. The listener is gone by the time the code is sent to GitHub.
pub fn create_app(
    wanted: &Wanted,
    cancelled: &AtomicBool,
    ways: &Ways<'_>,
) -> Result<Created, String> {
    let listening = (ways.bind)()?;
    let port = listening.port();
    let state = State::random()?;
    let word = State::random()?;
    let action = new_app_page(&wanted.owner, state.expose())?;
    let redirect = format!("http://127.0.0.1:{port}{BACK_PATH}");
    let (page, policy) = post_page(&action, &manifest(wanted, &redirect));
    let start = format!("{START_PATH}{}", word.expose());
    (ways.open)(&format!("http://127.0.0.1:{port}{start}"))?;
    let code = loopback::take_with(listening, ALLOWED, cancelled, &ways.clock, &mut |target| {
        if target == start {
            return Heard::Page {
                policy: policy.clone(),
                page: page.clone(),
            };
        }
        match pkce::read_redirect_at(BACK_PATH, target, &state) {
            Redirected::Code(code) => Heard::Took {
                value: code,
                page: CAME_BACK_PAGE,
            },
            Redirected::Declined(why) => Heard::Stop {
                why: Stopped::Declined(why),
                page: DECLINED_PAGE,
            },
            Redirected::NotOurs => Heard::NotOurs,
        }
    })
    .map_err(stopped_said)?;
    convert(ways.exchange, &code, wanted)
}

/// The reserved key an App's private key is kept under.
#[must_use]
pub fn key_of(app: &str) -> String {
    format!("{}github-{app}", mint::RESERVED_PREFIX)
}

/// The client id in a reserved key's name.
fn app_of(key: &str) -> Option<&str> {
    key.strip_prefix(mint::RESERVED_PREFIX)?
        .strip_prefix("github-")
        .filter(|app| client_id_ok(app))
}

/// **Stores the App's key, marked by its name** ([`mint::RESERVED_PREFIX`]), in [`VAULT`], which
/// is made in the keyring where there is none. A vault of that name in another provider is
/// refused, so the key lands only where it was promised.
pub fn keep_key(ctx: &Ctx, created: &Created) -> Result<(), String> {
    let doc = registry::load_registry(ctx).map_err(|e| e.message.to_string())?;
    if !registry::vaults(&doc).contains_key(VAULT) {
        registry::add_vault(
            ctx,
            VAULT,
            "keyring",
            serde_json::Map::new(),
            None,
            false,
            false,
        )
        .map_err(|e| e.message.to_string())?;
    }
    let vault = cmd::provider(ctx, VAULT).map_err(|e| e.message.to_string())?;
    if vault.provider != "keyring" {
        return Err(format!(
            "The vault {VAULT} isn't in the system keyring, so purlis kept nothing. Remove it or \
             rename it, then start over. The App is on GitHub and can be deleted there."
        ));
    }
    cmd::set_value(ctx, &vault, &key_of(&created.app), &created.pem)
        .map_err(|e| e.message.to_string())
}

/// **The github.com accounts the project's clones are under**, each once: what the person is
/// offered as the App's owner (Q3). Only a suggestion, which they confirm on GitHub's own page.
#[must_use]
pub fn owners_on_github(root: &Path) -> Vec<String> {
    let plane = crate::workspaces::Plane::open(root.to_path_buf());
    let known = super::known(root);
    let forge_of = |url: &str| super::resolve_in(url, &known);
    let mut owners: Vec<String> = plane
        .workspaces()
        .unwrap_or_default()
        .iter()
        .filter_map(|workspace| crate::repos::clones(root, workspace).ok())
        .flat_map(|clones| clones.repos)
        .filter_map(|repo| {
            let run = crate::worktree::git::run(
                &repo.path,
                &[
                    "config",
                    "--local",
                    "--no-includes",
                    "--get-regexp",
                    r"^remote\..*\.url$",
                ],
                crate::worktree::git::READ,
            )
            .ok()?;
            Some(apptoken::remotes_in(&repo.path, &run.out, &forge_of))
        })
        .flatten()
        .filter(|remote| remote.host == HOST && account_ok(&remote.owner))
        .map(|remote| remote.owner)
        .collect();
    owners.sort();
    owners.dedup();
    owners
}

/// **An App a persona's tool line names**, read from the project file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Held {
    pub app: String,
    pub vault: String,
    pub key: String,
    pub personas: Vec<String>,
}

/// **Every App the project's tool lines mint with**, by client id. Nothing where there is no file;
/// a file that doesn't read is refused rather than read as no identity.
pub fn held(root: &Path) -> Result<Vec<Held>, String> {
    let text = match std::fs::read_to_string(crate::names::manifest(root)) {
        Ok(text) => text,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(format!("the project's settings can't be read: {e}")),
    };
    let cfg: toml::Table = text
        .parse()
        .map_err(|_| "the project's settings file doesn't parse".to_owned())?;
    let mut found: Vec<Held> = Vec::new();
    let personas = cfg
        .get("sandbox")
        .and_then(|sandbox| sandbox.get("personas"))
        .and_then(toml::Value::as_table);
    for (persona, grants) in personas.into_iter().flatten() {
        let Some(tool) = grants
            .get("tools")
            .and_then(|tools| tools.get("gh"))
            .and_then(toml::Value::as_table)
            .filter(|tool| tool.contains_key(mint::KEY))
        else {
            continue;
        };
        let text = |key: &str| Some(tool.get(key)?.as_str()?.trim().to_owned());
        let (Some(app), Some(vault), Some(key)) = (text(mint::APP_KEY), text("vault"), text("key"))
        else {
            continue;
        };
        match found.iter_mut().find(|one| one.app == app) {
            Some(one) => one.personas.push(persona.clone()),
            None => found.push(Held {
                app,
                vault,
                key,
                personas: vec![persona.clone()],
            }),
        }
    }
    Ok(found)
}

/// **The client ids of keys kept and named by no line yet**: an App waiting for Finish, also over a
/// restart.
pub fn pending(ctx: &Ctx, root: &Path) -> Result<Vec<String>, String> {
    let doc = registry::load_registry(ctx).map_err(|e| e.message.to_string())?;
    if !registry::vaults(&doc).contains_key(VAULT) {
        return Ok(Vec::new());
    }
    let vault = cmd::provider(ctx, VAULT).map_err(|e| e.message.to_string())?;
    let keys = cmd::keys(ctx, &vault).map_err(|e| e.message.to_string())?;
    let lines = held(root)?;
    Ok(keys
        .iter()
        .filter_map(|key| app_of(key))
        .filter(|app| !lines.iter().any(|one| one.app == *app))
        .map(str::to_owned)
        .collect())
}

/// The value of a tool line for App `app`. The one source of what Finish shows and writes.
fn line_value(app: &str) -> toml_edit::InlineTable {
    let mut line = toml_edit::InlineTable::new();
    line.insert("vault", VAULT.into());
    line.insert(mint::KEY, mint::GITHUB_APP.into());
    line.insert(mint::APP_KEY, app.into());
    line.insert("key", key_of(app).into());
    line
}

/// **The exact lines Finish adds** (Q11), for each persona named.
#[must_use]
pub fn lines_shown(app: &str, personas: &[String]) -> String {
    personas
        .iter()
        .map(|persona| {
            format!(
                "[sandbox.personas.{}.tools]\ngh = {}",
                toml_edit::Key::new(persona.as_str()).display_repr(),
                line_value(app)
            )
        })
        .collect::<Vec<_>>()
        .join("\n\n")
}

/// The table under `key` of `item`, made where there is none, of the same form as `item`.
fn child<'a>(item: &'a mut toml_edit::Item, key: &str) -> Result<&'a mut toml_edit::Item, String> {
    let inline = item.is_inline_table();
    let not_a_table = || format!("{key} in the project's settings isn't a table");
    let table = item.as_table_like_mut().ok_or_else(not_a_table)?;
    if !table.contains_key(key) {
        let made = if inline {
            toml_edit::Item::Value(toml_edit::Value::InlineTable(toml_edit::InlineTable::new()))
        } else {
            let mut made = toml_edit::Table::new();
            made.set_implicit(true);
            toml_edit::Item::Table(made)
        };
        table.insert(key, made);
    }
    table.get_mut(key).ok_or_else(not_a_table)
}

/// **The project file `text` with App `app`'s line added for each of `personas`**, all else as it
/// was. A persona whose `gh` is something else is refused by name: Finish replaces nothing the
/// person didn't see.
pub fn with_lines(text: &str, app: &str, personas: &[String]) -> Result<String, String> {
    let mut doc: toml_edit::DocumentMut = text
        .parse()
        .map_err(|_| "the project's settings file doesn't parse".to_owned())?;
    for persona in personas {
        let sandbox = child(doc.as_item_mut(), "sandbox")?;
        let all = child(sandbox, "personas")?;
        let grants = child(all, persona)?;
        let tools = child(grants, "tools")?;
        let table = tools
            .as_table_like_mut()
            .ok_or_else(|| "tools in the project's settings isn't a table".to_owned())?;
        match table.get("gh") {
            None => {
                table.insert(
                    "gh",
                    toml_edit::Item::Value(toml_edit::Value::InlineTable(line_value(app))),
                );
            }
            Some(there)
                if there
                    .as_table_like()
                    .and_then(|gh| gh.get(mint::APP_KEY))
                    .and_then(toml_edit::Item::as_str)
                    == Some(app) => {}
            Some(_) => {
                return Err(format!(
                    "{} already has a gh tool of its own, so purlis changed nothing. Untick it, \
                     or remove that line first.",
                    crate::shown::short(persona)
                ));
            }
        }
    }
    Ok(doc.to_string())
}

/// **The project file `text` with every `gh` line of App `app` taken out**, all else as it was.
pub fn without_lines(text: &str, app: &str) -> Result<String, String> {
    let mut doc: toml_edit::DocumentMut = text
        .parse()
        .map_err(|_| "the project's settings file doesn't parse".to_owned())?;
    let personas = doc
        .get_mut("sandbox")
        .and_then(|sandbox| sandbox.get_mut("personas"))
        .and_then(toml_edit::Item::as_table_like_mut);
    if let Some(personas) = personas {
        for (_, grants) in personas.iter_mut() {
            let Some(grants) = grants.as_table_like_mut() else {
                continue;
            };
            let Some(tools) = grants
                .get_mut("tools")
                .and_then(toml_edit::Item::as_table_like_mut)
            else {
                continue;
            };
            let ours = tools
                .get("gh")
                .and_then(toml_edit::Item::as_table_like)
                .and_then(|gh| gh.get(mint::APP_KEY))
                .and_then(toml_edit::Item::as_str)
                == Some(app);
            if ours {
                tools.remove("gh");
            }
            if tools.is_empty() {
                grants.remove("tools");
            }
        }
    }
    Ok(doc.to_string())
}

/// `text` put where the project's file is, whole or not at all.
fn write_manifest(root: &Path, text: &str) -> Result<(), String> {
    use std::io::Write as _;
    let path = crate::names::manifest(root);
    let said = |e: std::io::Error| format!("the project's settings couldn't be written: {e}");
    let folder = path.parent().unwrap_or(root);
    let mut file = tempfile::NamedTempFile::new_in(folder).map_err(said)?;
    file.write_all(text.as_bytes()).map_err(said)?;
    file.persist(&path).map_err(|e| said(e.error))?;
    Ok(())
}

/// The project's file as it stands, empty where there is none.
fn read_manifest(root: &Path) -> Result<String, String> {
    match std::fs::read_to_string(crate::names::manifest(root)) {
        Ok(text) => Ok(text),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(String::new()),
        Err(e) => Err(format!("the project's settings can't be read: {e}")),
    }
}

/// **Adds App `app`'s line for `personas`** to the project at `root`: what Finish writes (Q11).
pub fn add_lines(root: &Path, app: &str, personas: &[String]) -> Result<(), String> {
    write_manifest(root, &with_lines(&read_manifest(root)?, app, personas)?)
}

/// **An App as GitHub says it is.**
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Stand {
    pub slug: String,
    pub owner: String,
    pub personal: bool,
    /// The installations of it, which is where its repos are.
    pub installations: Vec<u64>,
}

impl Stand {
    /// What it acts as on GitHub.
    #[must_use]
    pub fn acts_as(&self) -> String {
        self.slug.clone()
    }

    /// The App's page on GitHub, where it can be deleted.
    #[must_use]
    pub fn settings_page(&self) -> String {
        if self.personal {
            format!("https://{HOST}/settings/apps/{}", self.slug)
        } else {
            format!(
                "https://{HOST}/organizations/{}/settings/apps/{}",
                self.owner, self.slug
            )
        }
    }
}

fn github_com() -> Forge {
    Forge {
        kind: Kind::GitHub,
        host: HOST.to_owned(),
    }
}

/// One read of `path`, as JSON, or a sentence naming what was asked.
fn read_json(
    transport: &dyn Transport,
    path: String,
    what: &str,
) -> Result<serde_json::Value, String> {
    let reply: Reply = transport
        .send(&github_com(), &Call::get(path, CALL_WAITS))
        .map_err(|e| format!("GitHub could not be asked for {what} ({})", e.said()))?;
    if reply.code != 0 {
        return Err(format!(
            "GitHub refused to say {what} ({})",
            reply.err.trim()
        ));
    }
    serde_json::from_str(&reply.out).map_err(|_| format!("GitHub's answer on {what} didn't read"))
}

/// **Who the App is and where it is installed**, asked with its JWT. Changes nothing.
pub fn stand_over(jwt: &dyn Transport) -> Result<Stand, String> {
    let app = read_json(jwt, "app".to_owned(), "who the App is")?;
    let slug = app
        .get("slug")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("");
    let owner = app.get("owner");
    let login = owner
        .and_then(|owner| owner.get("login"))
        .and_then(serde_json::Value::as_str)
        .unwrap_or("");
    if !slug_ok(slug) || !account_ok(login) {
        return Err("GitHub's answer on who the App is didn't read".to_owned());
    }
    let personal = owner
        .and_then(|owner| owner.get("type"))
        .and_then(serde_json::Value::as_str)
        == Some("User");
    let listed = read_json(
        jwt,
        "app/installations?per_page=100".to_owned(),
        "where the App is installed",
    )?;
    let installations = listed
        .as_array()
        .map(|all| {
            all.iter()
                .filter_map(|one| one.get("id")?.as_u64())
                .collect()
        })
        .unwrap_or_default();
    Ok(Stand {
        slug: slug.to_owned(),
        owner: login.to_owned(),
        personal,
        installations,
    })
}

/// **What Finish proved** (Q17): a real token was minted and read with.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Proof {
    pub stand: Stand,
    pub repos: u64,
    /// The repo it read, where it has any.
    pub read: Option<String>,
}

/// **Mints a real token from each installation, reads with it and revokes it** (Q17). Each is
/// narrowed to reading metadata, so the proof can change nothing even while it lives.
pub fn prove_over(
    jwt: &dyn Transport,
    with_token: &dyn Fn(&str) -> Result<Box<dyn Transport>, String>,
) -> Result<Proof, String> {
    let stand = stand_over(jwt)?;
    if stand.installations.is_empty() {
        return Err(format!(
            "{} isn't installed on any repos yet, so no token can be made.",
            stand.slug
        ));
    }
    let forge = github_com();
    let mut repos = 0;
    let mut read = None;
    for installation in &stand.installations {
        let call = Call::write(
            Method::Post,
            format!("app/installations/{installation}/access_tokens"),
            vec![Field::Text(
                "permissions[metadata]".to_owned(),
                "read".to_owned(),
            )],
        );
        let reply = jwt
            .send(&forge, &call)
            .map_err(|e| format!("GitHub could not be asked for a token ({})", e.said()))?;
        if reply.code != 0 {
            return Err(format!("GitHub refused a token ({})", reply.err.trim()));
        }
        let answer: serde_json::Value = serde_json::from_str(&reply.out)
            .map_err(|_| "GitHub's answer held no token".to_owned())?;
        let token = Zeroizing::new(
            answer
                .get("token")
                .and_then(serde_json::Value::as_str)
                .ok_or_else(|| "GitHub's answer held no token".to_owned())?
                .to_owned(),
        );
        let held = with_token(&token)?;
        let checked = read_with(held.as_ref());
        if let Err(why) = apptoken::revoke_over(held.as_ref(), &forge) {
            tracing::warn!(
                "purlis: a proof's token was not revoked, and ends within the hour: {why}"
            );
        }
        let (count, first) = checked?;
        repos += count;
        read = read.or(first);
    }
    Ok(Proof { stand, repos, read })
}

/// How many repos a token reaches, and the first one, read through it.
fn read_with(token: &dyn Transport) -> Result<(u64, Option<String>), String> {
    let listed = read_json(
        token,
        "installation/repositories?per_page=1".to_owned(),
        "which repos the App reaches",
    )?;
    let count = listed
        .get("total_count")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(0);
    let first = listed
        .get("repositories")
        .and_then(serde_json::Value::as_array)
        .and_then(|repos| repos.first())
        .and_then(|repo| repo.get("full_name")?.as_str())
        .map(str::to_owned);
    if let Some(name) = &first {
        read_json(token, format!("repos/{name}"), "what a repo holds")?;
    }
    Ok((count, first))
}

/// The JWT of the App `app`, its key read by the one read that may, and dropped.
fn jwt_of(ctx: &Ctx, vault: &str, key: &str, app: &str) -> Result<Zeroizing<String>, String> {
    if crate::fence::FENCED {
        return Err("A test build of purlis doesn't talk to GitHub.".to_owned());
    }
    let vault = cmd::provider(ctx, vault).map_err(|e| e.message.to_string())?;
    let pem =
        Zeroizing::new(cmd::read_to_mint(ctx, &vault, key).map_err(|e| e.message.to_string())?);
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|e| e.to_string())?
        .as_secs();
    apptoken::jwt(&pem, app, now)
}

/// [`stand_over`] for the App `app` on github.com.
pub fn stand(ctx: &Ctx, vault: &str, key: &str, app: &str) -> Result<Stand, String> {
    let jwt = jwt_of(ctx, vault, key, app)?;
    stand_over(&apptoken::jwt_transport::to_github_com(&jwt)?)
}

/// [`prove_over`] for the App `app` on github.com.
pub fn prove(ctx: &Ctx, vault: &str, key: &str, app: &str) -> Result<Proof, String> {
    let jwt = jwt_of(ctx, vault, key, app)?;
    prove_over(&apptoken::jwt_transport::to_github_com(&jwt)?, &|token| {
        Ok(Box::new(apptoken::jwt_transport::to_github_com(token)?) as Box<dyn Transport>)
    })
}

/// **Removes the identity at once** (Q13): the key first, so no line ever outlives it unmarked,
/// then the lines, then every live token. Nothing is taken out if the key can't go. Answers how
/// many live tokens went.
pub fn remove(ctx: &Ctx, root: &Path, vault: &str, key: &str, app: &str) -> Result<usize, String> {
    let held_in = cmd::provider(ctx, vault).map_err(|e| e.message.to_string())?;
    let keys = cmd::keys(ctx, &held_in).map_err(|e| e.message.to_string())?;
    if keys.iter().any(|one| one == key) {
        cmd::delete(ctx, &held_in, key).map_err(|e| e.message.to_string())?;
    }
    write_manifest(root, &without_lines(&read_manifest(root)?, app)?)?;
    Ok(apptoken::app_removed(root, app, Duration::from_secs(5)))
}

#[cfg(test)]
mod tests;
