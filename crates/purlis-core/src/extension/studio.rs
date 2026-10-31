// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The six studio capabilities, and the confinement of an extension that declares one**
//! (ADR 0101; #2777).
//!
//! A studio capability is one of six the Skill studio is built from: `skill-source`, `grader`,
//! `skill-lint`, `skill-template`, `studio-panel` and `skill-router`. Each declares its shape in
//! `contributes` under its own word, read here ([`Declared`]). Five are asked now, in protocol 3,
//! by [`crate::executor::Executor::studio`], each with a question of its own ([`Ask`]) whose
//! answer is read here against a closed shape, failing closed with a sentence: the SkillRouter's
//! ([`Ask::Route`], SK-15b) within its own deadline, [`ROUTER_DEADLINE`]. The Grader's is asked
//! in protocol 4 (#3236), one question of many lines: a grade's four messages (grade request,
//! run-chat request, run-chat result, grade report, ADR 0105 §1), read by [`grading`].
//!
//! **An extension that declares one is confined** (the operator's ruling of 2026-10-20, "confine
//! first"), for every question its program is asked, not only its studio ones:
//!
//! - **It reads only its own folder and what purlis hands it.** The questions carry a skill's
//!   files in the request itself, so nothing of the project is a path it is handed; besides its
//!   folder it reads the system's programs and libraries ([`crate::sandbox::confined`]).
//! - **It writes only its state folder and a temporary folder of its own**, made for the question
//!   and removed after it. So a manifest that also declares paths it writes in the project, or a
//!   folder in each workspace, is refused.
//! - **Its network goes only to the hosts it declares** (top-level `hosts`), through a proxy of
//!   its own made for the question ([`crate::sandbox::egress::Serving::confined`]); none where it
//!   declares none. **A host that serves a model is never one of them** ([`model_host`]): the
//!   manifest is refused, and the proxy leaves such a host out whatever the manifest says.
//! - **A program purlis cannot confine does not run** (ADR 0031): on a system with no sandbox
//!   purlis can apply, the whole extension is refused at its read ([`refused_here`]), and where
//!   the sandbox is missing at a question, nothing starts ([`Confined::start`]).
//!
//! The words are known only where the `skill-studio` flag is on, and are experimental there
//! ([`super::capability::Capability::known`]).

pub mod grading;

use std::ffi::OsString;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use super::capability::Capability;
use crate::sandbox::Os;
use crate::sandbox::hosts::Host;

/// The most hosts one manifest declares. Each is named in the approval prompt, one by one.
pub const MOST_HOSTS: usize = 16;

/// The most templates one extension offers.
pub const MOST_TEMPLATES: usize = 16;

/// The most panels one extension adds to a skill's tab.
pub const MOST_PANELS: usize = 8;

/// The top-level manifest key a confined extension names the hosts it reaches under.
pub const HOSTS: &str = "hosts";

/// What a studio capability's shape declares, read from `contributes.<word>`, and the hosts.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Declared {
    /// `skill-source`: its name, as the studio lists the source.
    pub source: Option<String>,
    /// `grader`: its name, as the studio lists the grading framework.
    pub grader: Option<String>,
    /// `skill-lint`: its name, as the lint says whose finding a line is.
    pub lint: Option<String>,
    /// `skill-template`: the templates it offers.
    pub templates: Vec<Template>,
    /// `studio-panel`: the panels it adds to a skill's tab.
    pub panels: Vec<Panel>,
    /// `skill-router`: its name, and whether it reads prompts (SK-15b).
    pub router: Option<DeclaredRouter>,
    /// The hosts its program reaches, through its own proxy, and no others: each one name.
    pub hosts: Vec<Host>,
}

/// **What `contributes.skill-router` declares** (SK-15b, ADR 0102 §5): `{"title": "…",
/// "reads-prompts": true}`, `reads-prompts` false where absent. A router that reads prompts is
/// asked on each prompt too (SK-15c), and its approval prompt says so.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeclaredRouter {
    pub title: String,
    pub reads_prompts: bool,
}

/// One template an extension offers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Template {
    pub id: String,
    pub title: String,
    /// What it gives, in one line.
    pub about: String,
}

/// One panel an extension adds to a skill's tab.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Panel {
    pub id: String,
    pub title: String,
}

/// Why an extension that declares a studio capability is not read on `os`, or `None` where purlis
/// can confine its program: Seatbelt on macOS, purlis's own wrap on Linux (ADR 0101 §4).
pub fn refused_here(os: Os) -> Option<String> {
    match os {
        Os::MacOs | Os::Linux => None,
        Os::Windows | Os::Other => Some(
            "declares a studio capability, and purlis runs such an extension only confined, \
             which it cannot do on this system yet, so it loads none of this extension"
                .to_owned(),
        ),
    }
}

/// **The studio capabilities' shapes in `contributes`, and the hosts in `doc`**, for a manifest
/// that asks for `capabilities` and declares `program`, read on `os` — or why purlis will not
/// read them: on a system purlis cannot confine a program on, none of the extension is read.
/// `None` for a manifest that asks for no studio capability, which may then declare no hosts.
pub(super) fn declared_of(
    doc: &serde_json::Map<String, serde_json::Value>,
    contributes: &serde_json::Map<String, serde_json::Value>,
    capabilities: &[Capability],
    program: Option<&str>,
    os: Os,
) -> Result<Option<Declared>, String> {
    if !capabilities.iter().any(|it| it.is_studio()) {
        if doc.contains_key(HOSTS) {
            return Err(
                "declares 'hosts' and no studio capability: an extension is held to the hosts it \
                 names only where it runs confined, so the list would be a claim nothing checks"
                    .into(),
            );
        }
        return Ok(None);
    }
    if let Some(why) = refused_here(os) {
        return Err(why);
    }
    if program.is_none() {
        return Err(
            "declares a studio capability and no program ('runs') to ask, so purlis would offer \
             what nothing answers"
                .into(),
        );
    }
    let shape = |capability: Capability| contributes.get(capability.as_str());
    let named = |capability: Capability| -> Result<Option<String>, String> {
        shape(capability)
            .map(|value| named_of(value, capability.as_str()))
            .transpose()
    };
    Ok(Some(Declared {
        source: named(Capability::SkillSource)?,
        grader: named(Capability::Grader)?,
        lint: named(Capability::SkillLint)?,
        templates: match shape(Capability::SkillTemplate) {
            None => Vec::new(),
            Some(value) => templates_of(value)?,
        },
        panels: match shape(Capability::StudioPanel) {
            None => Vec::new(),
            Some(value) => panels_of(value)?,
        },
        router: shape(Capability::SkillRouter).map(router_of).transpose()?,
        hosts: match doc.get(HOSTS) {
            None => Vec::new(),
            Some(value) => hosts_of(value)?,
        },
    }))
}

/// A shape that is only a title: `{"title": "…"}`.
fn named_of(value: &serde_json::Value, word: &str) -> Result<String, String> {
    let object = value
        .as_object()
        .ok_or_else(|| format!("has a 'contributes.{word}' that is not an object"))?;
    only_keys(object, &["title"], &format!("'contributes.{word}'"))?;
    super::title_of(object.get("title"), "the studio capability", word)
}

/// `contributes.skill-router`: `{title, reads-prompts}`.
fn router_of(value: &serde_json::Value) -> Result<DeclaredRouter, String> {
    let word = Capability::SkillRouter.as_str();
    let object = value
        .as_object()
        .ok_or_else(|| format!("has a 'contributes.{word}' that is not an object"))?;
    only_keys(
        object,
        &["title", READS_PROMPTS],
        &format!("'contributes.{word}'"),
    )?;
    let reads_prompts = match object.get(READS_PROMPTS) {
        None => false,
        Some(serde_json::Value::Bool(reads)) => *reads,
        Some(_) => {
            return Err(format!(
                "has a 'contributes.{word}.{READS_PROMPTS}' that is not true or false"
            ));
        }
    };
    Ok(DeclaredRouter {
        title: super::title_of(object.get("title"), "the studio capability", word)?,
        reads_prompts,
    })
}

/// The key of `contributes.skill-router` that says the router reads prompts.
pub const READS_PROMPTS: &str = "reads-prompts";

/// `contributes.skill-template`: a list of `{id, title, about}`.
fn templates_of(value: &serde_json::Value) -> Result<Vec<Template>, String> {
    let list = value
        .as_array()
        .ok_or("has a 'contributes.skill-template' that is not a list")?;
    if list.is_empty() {
        return Err("declares 'contributes.skill-template' with no template in it".into());
    }
    if list.len() > MOST_TEMPLATES {
        return Err(format!(
            "declares {} templates, and purlis offers at most {MOST_TEMPLATES} from one extension",
            list.len()
        ));
    }
    let mut out: Vec<Template> = Vec::with_capacity(list.len());
    for (at, raw) in list.iter().enumerate() {
        let object = raw
            .as_object()
            .ok_or_else(|| format!("declares a template at {at} that is not an object"))?;
        only_keys(object, &["id", "title", "about"], "a template")?;
        let id = part_id(object.get("id"), "template", at)?;
        if out.iter().any(|it| it.id == id) {
            return Err(format!("declares two templates called {id:?}"));
        }
        let title = super::title_of(object.get("title"), "the template", &id)?;
        let about = super::title_of(object.get("about"), "the template's line", &id)?;
        out.push(Template { id, title, about });
    }
    Ok(out)
}

/// `contributes.studio-panel`: a list of `{id, title}`.
fn panels_of(value: &serde_json::Value) -> Result<Vec<Panel>, String> {
    let list = value
        .as_array()
        .ok_or("has a 'contributes.studio-panel' that is not a list")?;
    if list.is_empty() {
        return Err("declares 'contributes.studio-panel' with no panel in it".into());
    }
    if list.len() > MOST_PANELS {
        return Err(format!(
            "declares {} panels for a skill's tab, and purlis draws at most {MOST_PANELS} from \
             one extension",
            list.len()
        ));
    }
    let mut out: Vec<Panel> = Vec::with_capacity(list.len());
    for (at, raw) in list.iter().enumerate() {
        let object = raw
            .as_object()
            .ok_or_else(|| format!("declares a panel at {at} that is not an object"))?;
        only_keys(object, &["id", "title"], "a skill tab's panel")?;
        let id = part_id(object.get("id"), "panel", at)?;
        if out.iter().any(|it| it.id == id) {
            return Err(format!(
                "declares two panels called {id:?} for a skill's tab"
            ));
        }
        let title = super::title_of(object.get("title"), "the panel", &id)?;
        out.push(Panel { id, title });
    }
    Ok(out)
}

/// An id of a part, at `at` in its list, or why not.
fn part_id(value: Option<&serde_json::Value>, what: &str, at: usize) -> Result<String, String> {
    let id = value
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| format!("declares a {what} at {at} with no id"))?;
    if !super::ok_in_a_part_id(id) {
        return Err(format!(
            "declares the {what} id {:?}, and a {what}'s id is letters, digits, '-' and '_', \
             starting with a letter or a digit",
            crate::shown::readable(id, 64)
        ));
    }
    Ok(id.to_owned())
}

/// Refuses a key of `object` that is not one of `keys`, naming `what` it is.
fn only_keys(
    object: &serde_json::Map<String, serde_json::Value>,
    keys: &[&str],
    what: &str,
) -> Result<(), String> {
    match object.keys().find(|key| !keys.contains(&key.as_str())) {
        Some(key) => Err(format!(
            "declares {what} carrying {:?}, which is not part of what it may say — it is {}",
            crate::shown::readable(key, 64),
            keys.join(", ")
        )),
        None => Ok(()),
    }
}

/// **The hosts a manifest declares**, or why not: each a name, never an address, a wildcard or a
/// port, never a host that serves a model ([`model_host`]), and each once.
fn hosts_of(value: &serde_json::Value) -> Result<Vec<Host>, String> {
    let list = value
        .as_array()
        .ok_or("has a 'hosts' that is not a list of hosts")?;
    if list.len() > MOST_HOSTS {
        return Err(format!(
            "declares {} hosts, and purlis names at most {MOST_HOSTS} in the approval",
            list.len()
        ));
    }
    let mut out: Vec<Host> = Vec::with_capacity(list.len());
    for raw in list {
        let typed = raw
            .as_str()
            .ok_or("has a 'hosts' entry that is not a host")?;
        let shown = crate::shown::readable(typed, 64);
        let host = Host::parse(typed)
            .map_err(|why| format!("declares the host {shown:?}, which is not one: {why}"))?;
        let name = host.to_string();
        if host.address().is_some() || host.port().is_some() || name.starts_with("*.") {
            return Err(format!(
                "declares the host {shown:?}: a host it reaches is one name, with no wildcard, \
                 address or port, reached on the ports a web address uses"
            ));
        }
        if model_host(&host) {
            return Err(format!(
                "declares the host {name}, which serves a model: a studio capability reaches a \
                 model only through a chat purlis runs for it, so purlis loads none of this \
                 extension"
            ));
        }
        if out.contains(&host) {
            return Err(format!("declares the host {name} twice"));
        }
        out.push(host);
    }
    Ok(out)
}

/// **The domains whose hosts serve a model** (ADR 0101 §3): every host a shipped harness or its
/// model sources reach ([`crate::sandbox::Preset::ModelProviders`], whose hosts are folded in by
/// [`model_host`]), and the model providers those reach in turn. A host is one of them where it is
/// the domain or a name under it. The list grows with every harness and model source purlis
/// learns, and each host added closes it to an extension already approved: its proxy leaves the
/// host out at the next question ([`reachable`]).
pub const MODEL_DOMAINS: [&str; 41] = [
    "anthropic.com",
    "claude.ai",
    "claude.com",
    "openai.com",
    "chatgpt.com",
    "oaiusercontent.com",
    "opencode.ai",
    "models.dev",
    "openrouter.ai",
    "mistral.ai",
    "cohere.com",
    "cohere.ai",
    "groq.com",
    "together.xyz",
    "together.ai",
    "deepseek.com",
    "x.ai",
    "perplexity.ai",
    "fireworks.ai",
    "generativelanguage.googleapis.com",
    "aiplatform.googleapis.com",
    "ai.google.dev",
    "openai.azure.com",
    "cognitiveservices.azure.com",
    "services.ai.azure.com",
    "githubcopilot.com",
    "copilot-proxy.githubusercontent.com",
    "models.github.ai",
    "models.inference.ai.azure.com",
    "ollama.com",
    "huggingface.co",
    "hf.co",
    "replicate.com",
    "deepinfra.com",
    "cerebras.ai",
    "ai21.com",
    "voyageai.com",
    "moonshot.ai",
    "z.ai",
    "dashscope.aliyuncs.com",
    "minimax.io",
];

/// **Whether `host` serves a model** (ADR 0101 §3): one of [`MODEL_DOMAINS`] or a name under one,
/// a host the model-providers preset lists or a name under it, or Amazon Bedrock's runtime in any
/// region.
pub fn model_host(host: &Host) -> bool {
    let host = host.to_string();
    let under = |domain: &str| {
        let domain = domain.trim_start_matches("*.");
        host == domain
            || host
                .strip_suffix(domain)
                .is_some_and(|head| head.ends_with('.'))
    };
    MODEL_DOMAINS.iter().any(|domain| under(domain))
        || crate::sandbox::Preset::ModelProviders
            .own_hosts()
            .iter()
            .any(|domain| under(domain))
        || (under("amazonaws.com") && host.split('.').any(|label| label.starts_with("bedrock")))
}

/// **The hosts a confined program's proxy carries**: `declared`, less every host that serves a
/// model, whatever the manifest said ([`model_host`]). The manifest's own read refuses such a
/// host, so this leaves one out only where [`MODEL_DOMAINS`] grew after the extension was approved.
pub fn reachable(declared: &[Host]) -> Vec<Host> {
    declared
        .iter()
        .filter(|host| !model_host(host))
        .cloned()
        .collect()
}

/// **What the approval prompt says of a confined extension**, in place of
/// [`super::RUNS_AS_YOU`], which would be false of it (ADR 0101 §4): from the core, so the window
/// cannot say something kinder or harsher than is true.
pub(super) fn confined_said(hosts: &[Host]) -> String {
    let names: Vec<String> = hosts.iter().map(ToString::to_string).collect();
    let reach = match names.as_slice() {
        [] => "It reaches no host on the network".to_owned(),
        [one] => format!("It reaches only {one} on the network"),
        [rest @ .., last] => format!(
            "It reaches only {} and {last} on the network",
            rest.join(", ")
        ),
    };
    format!(
        "purlis runs this extension in the sandbox a chat runs in, because it declares a studio \
         capability. It reads only its own folder, the system's programs and what purlis hands \
         it with each question, never your projects or your home folder. It writes only its own \
         state folder. {reach}, through a proxy that carries a connection only when its \
         encryption names that host, and never to a host that serves a model or to an address \
         of a private network. A host it names can still pass on what it is sent. The list above \
         is what it can do, not only what it says it does."
    )
}

/// **What the prompt lists of a studio extension**: each template it offers, each panel it adds,
/// and the hosts it reaches.
pub(super) fn declares(declared: &Declared) -> Vec<String> {
    let mut out = Vec::new();
    for template in &declared.templates {
        out.push(format!(
            "a template for new skills, “{}” — {}",
            template.title, template.about
        ));
    }
    for panel in &declared.panels {
        out.push(format!(
            "a panel on each skill's tab, “{}”, which purlis draws from what its program answers",
            panel.title
        ));
    }
    if let Some(router) = &declared.router {
        out.push(if router.reads_prompts {
            format!(
                "a skill router, “{}” — decides which skills each chat that chooses it lists, \
                 asked at the chat's start within {} ms, and it reads every prompt of every such \
                 chat",
                router.title,
                ROUTER_DEADLINE.as_millis()
            )
        } else {
            format!(
                "a skill router, “{}” — decides which skills each chat that chooses it lists, \
                 asked at the chat's start within {} ms, from the skills' names and \
                 descriptions; it never reads a prompt",
                router.title,
                ROUTER_DEADLINE.as_millis()
            )
        });
    }
    let hosts: Vec<String> = declared.hosts.iter().map(ToString::to_string).collect();
    out.push(if hosts.is_empty() {
        "hosts it reaches: none".to_owned()
    } else {
        format!("hosts it reaches: {}", hosts.join(", "))
    });
    out
}

/// Why a confined extension may not also declare what writes the project, or `None`.
pub(super) fn writes_nothing_of_the_project(
    writes: &[String],
    workspace_folder: Option<&str>,
) -> Option<String> {
    if !writes.is_empty() {
        return Some(
            "declares paths it writes in the project and a studio capability: an extension that \
             declares a studio capability runs confined and writes nothing of the project"
                .to_owned(),
        );
    }
    workspace_folder.map(|folder| {
        format!(
            "keeps a folder in each workspace ({folder}) and declares a studio capability: an \
             extension that declares a studio capability runs confined and writes nothing of \
             the project"
        )
    })
}

// ---------------------------------------------------------------------------------------
// Confinement at a question
// ---------------------------------------------------------------------------------------

/// **A confined program's start**: the line that starts it inside the sandbox, the environment,
/// and what has to live as long as it does: its own temporary folder and its proxy.
pub(crate) struct Confined {
    pub(crate) program: OsString,
    pub(crate) args: Vec<OsString>,
    pub(crate) env: Vec<(OsString, OsString)>,
    /// Its own temporary folder, removed when the question is over.
    _tmp: tempfile::TempDir,
    /// Its proxy, the extension's one for the whole run ([`proxy_for`]), open while this question
    /// is being answered; none where it declares no host.
    _proxy: Option<Asking>,
}

/// **One question being answered through a confined extension's proxy**: the proxy carries
/// connections only while one is ([`crate::sandbox::egress::Serving::asking`]), so a program its
/// extension left running between questions reaches no host.
struct Asking {
    proxy: std::sync::Arc<crate::sandbox::egress::Proxy>,
}

impl Asking {
    fn of(proxy: std::sync::Arc<crate::sandbox::egress::Proxy>) -> Self {
        if let Some(asking) = proxy.asking() {
            asking.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        }
        Self { proxy }
    }

    fn proxy(&self) -> &crate::sandbox::egress::Proxy {
        &self.proxy
    }
}

impl Drop for Asking {
    fn drop(&mut self) {
        // The last question over: what the proxy is still carrying ends with it.
        // Never below none: a proxy closed for good was set to none while this was asked.
        if let Some(asking) = self.proxy.asking()
            && asking
                .try_update(
                    std::sync::atomic::Ordering::SeqCst,
                    std::sync::atomic::Ordering::SeqCst,
                    |now| Some(now.saturating_sub(1)),
                )
                .is_ok_and(|was| was <= 1)
        {
            self.proxy.cut();
        }
    }
}

/// **Closes the extension `id`'s proxy for good**: it was turned off on this machine or removed.
/// Its ports stay held for the rest of the run, so no other listener is given them
/// ([`crate::sandbox::egress::Serving::confined`]); a later question opens a proxy of its own.
pub(crate) fn close_proxy(id: &str) {
    let closed = PROXIES
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .remove(id);
    // Whatever it was carrying ends now, whoever else still holds the proxy.
    if let Some((_, proxy)) = closed {
        if let Some(asking) = proxy.asking() {
            asking.store(0, std::sync::atomic::Ordering::SeqCst);
        }
        proxy.cut();
    }
}

/// A confined extension's proxy, with the hosts it carries.
type Held = (Vec<String>, std::sync::Arc<crate::sandbox::egress::Proxy>);

/// **Each confined extension's one proxy for the run**, by its id, with the hosts it carries.
/// Every question to an extension is asked through the same pair of ports, so a confined program
/// is only ever given one pair per extension, and those never go to another listener while purlis
/// runs. A proxy whose hosts changed (a manifest the person approved again) is replaced, and the
/// one it replaces keeps its ports bound ([`crate::sandbox::egress::Serving::confined`]).
static PROXIES: std::sync::Mutex<std::collections::BTreeMap<String, Held>> =
    std::sync::Mutex::new(std::collections::BTreeMap::new());

/// The extension `id`'s proxy carrying `hosts`: the one it already has where it carries the same
/// hosts, or a new one, kept for the rest of the run.
fn proxy_for(
    id: &str,
    hosts: Vec<String>,
) -> std::io::Result<std::sync::Arc<crate::sandbox::egress::Proxy>> {
    let mut held = PROXIES
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    if let Some((carried, proxy)) = held.get(id)
        && *carried == hosts
    {
        return Ok(std::sync::Arc::clone(proxy));
    }
    let proxy = std::sync::Arc::new(crate::sandbox::egress::Proxy::serving(
        crate::sandbox::egress::Serving::confined(hosts.clone()),
    )?);
    held.insert(id.to_owned(), (hosts, std::sync::Arc::clone(&proxy)));
    Ok(proxy)
}

/// How many confined extensions hold a proxy now.
#[cfg(test)]
fn proxies_held() -> usize {
    PROXIES
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .len()
}

impl Confined {
    /// **Starts nothing, and builds the line `program` of `found` starts confined with**, its
    /// environment `env` beside what the sandbox adds, or says why it will not run: no sandbox on
    /// this machine, a profile that could not be written, a proxy that could not start.
    pub(crate) fn start(
        found: &super::Extension,
        declared: &Declared,
        program: &Path,
        env: Vec<(String, String)>,
    ) -> Result<Self, String> {
        let id = found.id();
        let os = Os::this();
        if let Some(why) = refused_here(os) {
            return Err(format!("'{id}' {why}"));
        }
        if let Some(missing) = sandbox_missing(os) {
            return Err(format!(
                "purlis runs '{id}' only confined, and {missing}, so it did not start it."
            ));
        }
        let could_not = |why: &dyn std::fmt::Display| {
            format!("purlis could not confine '{id}''s program ({why}), so it did not start it.")
        };
        let tmp = tempfile::Builder::new()
            .prefix("purlis-extension-")
            .tempdir()
            .map_err(|why| could_not(&why))?;
        let tmp_path = crate::sandbox::real(tmp.path());
        let hosts: Vec<String> = reachable(&declared.hosts)
            .iter()
            .map(ToString::to_string)
            .collect();
        let proxy = if hosts.is_empty() {
            None
        } else {
            Some(Asking::of(
                proxy_for(id, hosts).map_err(|why| could_not(&why))?,
            ))
        };
        let mut writes = vec![tmp_path.clone()];
        if let Some(state) = &found.manifest.state {
            writes.push(crate::sandbox::real(&found.path.join(state)));
        }
        // The proxy's own two listening ports on the loopback, and no other port of this machine.
        let ports: Vec<u16> = proxy
            .as_ref()
            .map(|asking| vec![asking.proxy().port(), asking.proxy().socks_port()])
            .unwrap_or_default();
        let profile = crate::sandbox::confined::profile(
            os,
            &[crate::sandbox::real(&found.path)],
            &writes,
            &ports,
        )
        .map_err(|why| could_not(&why))?;

        let mut env: Vec<(String, String)> = env
            .into_iter()
            .filter(|(name, _)| name != "TMPDIR")
            .collect();
        match &proxy {
            Some(asking) => env.extend(crate::sandbox::seatbelt::env(
                &asking.proxy().url(),
                &tmp_path,
            )),
            None => env.push(("TMPDIR".to_owned(), tmp_path.display().to_string())),
        }
        let wrap = match os {
            Os::Linux => Some(own_program().map_err(|why| could_not(&why))?),
            _ => None,
        };
        // By the name the kernel resolves it to: a path through a link (`/var` is `/private/var`
        // on macOS) would be looked up through a folder the profile lets it resolve, never list.
        let (start, args) = crate::sandbox::wrapper::line(
            os,
            wrap.as_deref(),
            profile,
            crate::sandbox::real(program).display().to_string(),
            Vec::new(),
        )
        .map_err(|why| could_not(&why))?;
        let env: Vec<(OsString, OsString)> = match os {
            // The wrap is purlis's own program, started from an environment purlis builds, and
            // carries the program's own past itself (#2460).
            Os::Linux => purlis_linux_sandbox::carried::carried(env),
            _ => env
                .into_iter()
                .map(|(name, value)| (name.into(), value.into()))
                .collect(),
        };
        Ok(Self {
            program: start.into(),
            args: args.into_iter().map(OsString::from).collect(),
            env,
            _tmp: tmp,
            _proxy: proxy,
        })
    }
}

/// What this machine lacks to confine a program on `os`, as a clause, or `None`.
fn sandbox_missing(os: Os) -> Option<String> {
    match os {
        Os::MacOs => (!crate::sandbox::backend::installed("sandbox-exec"))
            .then(|| "this Mac has no sandbox-exec".to_owned()),
        Os::Linux => (!crate::sandbox::backend::installed(crate::sandbox::backend::LANDLOCK))
            .then(|| purlis_linux_sandbox::NEEDS_LANDLOCK.to_owned()),
        Os::Windows | Os::Other => Some("this system has no sandbox purlis can apply".to_owned()),
    }
}

/// purlis's own running program, which applies the sandbox on Linux as `purlis _sandbox`, by its
/// real path; never one replaced while it ran.
fn own_program() -> Result<PathBuf, String> {
    let this = std::env::current_exe().map_err(|why| why.to_string())?;
    if this.to_string_lossy().ends_with(" (deleted)") {
        return Err("purlis's own program was replaced while it ran".to_owned());
    }
    this.canonicalize().map_err(|why| why.to_string())
}

// ---------------------------------------------------------------------------------------
// The questions, and their answers (protocol 3)
// ---------------------------------------------------------------------------------------

/// The most files of a skill purlis hands a question, and the most bytes of them in all.
pub const MOST_HANDED_FILES: usize = 64;
pub const MOST_HANDED_BYTES: usize = 256 << 10;

/// The most skills a source answers in one listing.
pub const MOST_OFFERED: usize = purlis_skills::tree::MOST_SKILLS;

/// The most findings one lint answers, and the most bytes of one.
pub const MOST_FINDINGS: usize = 50;
pub const MOST_FINDING_BYTES: usize = 400;

/// The most bytes of a template's text.
pub const MOST_TEMPLATE_BYTES: usize = 64 << 10;

/// The most a revision may be: an opaque name of what the source holds, never a path.
pub const MOST_REVISION_BYTES: usize = 128;

/// One file of a skill, as a question hands it and a source answers it: its path inside the
/// skill's folder, `/` between the parts, and its text.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct HandedFile {
    pub path: String,
    pub text: String,
    pub executable: bool,
}

/// One file of a skill as purlis read it, to hand a question ([`handed`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SkillFile {
    /// Its path inside the skill's folder, `/` between the parts.
    pub path: String,
    pub bytes: Vec<u8>,
    pub executable: bool,
}

/// **A studio question**, as its request line carries it under `ask`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "ask", rename_all = "kebab-case")]
pub enum Ask<'a> {
    /// A source's listing: its revision, and the skills it offers at it.
    Skills,
    /// One skill's files at `revision`.
    Files { skill: &'a str, revision: &'a str },
    /// A lint's findings on `skill`, whose files are `files`.
    Findings {
        skill: &'a str,
        files: &'a [HandedFile],
    },
    /// The text of a new skill `name`, described by `description`, from `template`.
    Template {
        template: &'a str,
        name: &'a str,
        description: &'a str,
    },
    /// A panel on `skill`'s tab, whose files are `files`.
    Panel {
        panel: &'a str,
        skill: &'a str,
        files: &'a [HandedFile],
    },
    /// **Which of a chat's `skills` its listing hides** (SK-15b, ADR 0102 §1), asked at its
    /// start: each skill the router may hide, by name and description and whether the chat pins
    /// it (never hidden), what the rest of the listing spends, and the budget. Never a prompt,
    /// the chat's id or anything else of the chat.
    Route {
        skills: &'a [RouteSkill],
        budget: RouteBudget,
        besides: RouteBudget,
    },
    /// **A grade of a skill** (protocol 4, #3236): the grade request, whose answer is run-chat
    /// requests and then the grade report, asked only by
    /// [`crate::executor::Executor::grade`] ([`grading`]).
    Grade {
        request: &'a purlis_skills::grade::GradeRequest,
    },
}

/// One skill a router is asked about ([`Ask::Route`]).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RouteSkill {
    pub name: String,
    pub description: String,
    pub pinned: bool,
}

/// Skills and characters of their descriptions: a chat's budget, or what part of its listing
/// spends.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct RouteBudget {
    pub skills: usize,
    pub chars: usize,
}

/// **How long a router has** to answer one question (ADR 0102 §6), from before its confined
/// program starts to its answer's last byte.
pub const ROUTER_DEADLINE: std::time::Duration = std::time::Duration::from_millis(100);

impl Ask<'_> {
    /// The capability it asks.
    pub fn capability(&self) -> Capability {
        match self {
            Self::Skills | Self::Files { .. } => Capability::SkillSource,
            Self::Findings { .. } => Capability::SkillLint,
            Self::Template { .. } => Capability::SkillTemplate,
            Self::Panel { .. } => Capability::StudioPanel,
            Self::Route { .. } => Capability::SkillRouter,
            Self::Grade { .. } => Capability::Grader,
        }
    }

    /// Why `declared` does not offer what this asks for, or `None`.
    pub(crate) fn unoffered(&self, extension: &str, declared: &Declared) -> Option<String> {
        match self {
            Self::Template { template, .. }
                if !declared.templates.iter().any(|it| it.id == *template) =>
            {
                Some(format!("'{extension}' offers no template {template:?}"))
            }
            Self::Panel { panel, .. } if !declared.panels.iter().any(|it| it.id == *panel) => Some(
                format!("'{extension}' adds no panel {panel:?} to a skill's tab"),
            ),
            _ => None,
        }
    }

    /// Its `ask` in the request line.
    pub(crate) fn line(&self) -> serde_json::Value {
        serde_json::to_value(self).unwrap_or(serde_json::Value::Null)
    }
}

/// **A skill's files to hand a question**, of `files`: the text files, in path order, until
/// [`MOST_HANDED_FILES`] or [`MOST_HANDED_BYTES`], so a question is bounded however large the
/// skill.
pub fn handed(files: &[SkillFile]) -> Vec<HandedFile> {
    let mut sorted: Vec<&SkillFile> = files.iter().collect();
    sorted.sort_by(|a, b| a.path.cmp(&b.path));
    let mut out = Vec::new();
    let mut bytes = 0usize;
    for file in sorted {
        let Ok(text) = std::str::from_utf8(&file.bytes) else {
            continue;
        };
        if out.len() == MOST_HANDED_FILES || bytes + text.len() > MOST_HANDED_BYTES {
            break;
        }
        bytes += text.len();
        out.push(HandedFile {
            path: file.path.clone(),
            text: text.to_owned(),
            executable: file.executable,
        });
    }
    out
}

/// `doc`, an answer read as far as every answer is, as `T`: every key `T` knows and none it does
/// not, failing closed with a sentence naming the extension.
fn answer_of<T: for<'de> Deserialize<'de>>(
    extension: &str,
    doc: &serde_json::Map<String, serde_json::Value>,
    what: &str,
) -> Result<T, String> {
    let mut doc = doc.clone();
    doc.remove("charter");
    serde_json::from_value(serde_json::Value::Object(doc)).map_err(|why| {
        format!(
            "'{extension}' answered {what} purlis cannot read: {}",
            crate::shown::readable(&why.to_string(), 300)
        )
    })
}

/// One skill a source offers, as its listing answers it.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OfferedSkill {
    pub name: String,
    #[serde(default)]
    pub description: String,
}

/// What a source offers at one revision.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Offered {
    pub revision: String,
    pub skills: Vec<OfferedSkill>,
}

/// **A source's listing**, read from its answer `doc`, or why not.
pub fn offered_of(
    extension: &str,
    doc: &serde_json::Map<String, serde_json::Value>,
) -> Result<Offered, String> {
    if doc
        .get("skills")
        .and_then(serde_json::Value::as_array)
        .is_some_and(|list| list.len() > MOST_OFFERED)
    {
        return Err(format!(
            "'{extension}' offers more than {MOST_OFFERED} skills, the most purlis reads of one \
             skill source"
        ));
    }
    let offered: Offered = answer_of(extension, doc, "a listing")?;
    revision_ok(extension, &offered.revision)?;
    let mut seen: Vec<&str> = Vec::with_capacity(offered.skills.len());
    for skill in &offered.skills {
        let name = skill.name.as_str();
        if let Some(why) = purlis_skills::change::name_problem(name) {
            return Err(format!(
                "'{extension}' offers {:?}, which cannot be a skill's name: {why}",
                crate::shown::readable(name, 64)
            ));
        }
        if skill.description.contains(crate::panel::undrawable) || skill.description.len() > 1024 {
            return Err(format!(
                "'{extension}' offers {name} with a description purlis will not draw: it is \
                 longer than 1024 bytes or holds a control or invisible formatting character"
            ));
        }
        if seen.contains(&name) {
            return Err(format!("'{extension}' offers {name} twice"));
        }
        seen.push(name);
    }
    Ok(offered)
}

/// One file of a skill, as a source answers it.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
struct AnsweredFile {
    path: String,
    text: String,
    #[serde(default)]
    executable: bool,
}

/// A source's answer for one skill's files.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
struct Files {
    revision: String,
    files: Vec<AnsweredFile>,
}

/// **One skill's files**, read from a source's answer `doc` to a question at `revision`: each a
/// plain path inside the folder, once, with its text, the whole within one skill's limits; the
/// revision answered must be the one asked.
pub fn files_of(
    extension: &str,
    revision: &str,
    doc: &serde_json::Map<String, serde_json::Value>,
) -> Result<Vec<HandedFile>, String> {
    if doc
        .get("files")
        .and_then(serde_json::Value::as_array)
        .is_some_and(|list| list.len() > purlis_skills::tree::MOST_FILES)
    {
        return Err(format!(
            "'{extension}' answered more than {} files, the most a skill purlis installs may hold",
            purlis_skills::tree::MOST_FILES
        ));
    }
    let answered: Files = answer_of(extension, doc, "a skill's files")?;
    revision_ok(extension, &answered.revision)?;
    if answered.revision != revision {
        return Err(format!(
            "'{extension}' answered the files of another revision than the one purlis asked for, \
             so purlis reads none of them"
        ));
    }
    let mut out: Vec<HandedFile> = Vec::with_capacity(answered.files.len());
    for file in answered.files {
        let path = file.path.as_str();
        let plain = !path.starts_with('/')
            && !path.ends_with('/')
            && !path.contains("//")
            && !path.contains('\\');
        let parts = purlis_skills::tree::folder_parts(path).filter(|parts| {
            plain
                && !parts.is_empty()
                && parts.len() <= purlis_skills::tree::MOST_SKILL_DEPTH + 1
                && path.len() <= purlis_skills::tree::MOST_PATH_BYTES
        });
        if parts.is_none() {
            return Err(format!(
                "'{extension}' answered the file {:?}, which is no plain path inside a skill's \
                 folder",
                crate::shown::readable(path, 64)
            ));
        }
        if out.iter().any(|it| it.path == path) {
            return Err(format!("'{extension}' answered {path} twice"));
        }
        out.push(HandedFile {
            path: file.path,
            text: file.text,
            executable: file.executable,
        });
    }
    if !out.iter().any(|it| it.path == purlis_skills::FILE) {
        return Err(format!(
            "'{extension}' answered a skill with no {}",
            purlis_skills::FILE
        ));
    }
    Ok(out)
}

/// A revision, as an answer names it: present, bounded, and letters, digits and `.-_:+`.
fn revision_ok(extension: &str, revision: &str) -> Result<(), String> {
    if revision.is_empty()
        || revision.len() > MOST_REVISION_BYTES
        || !revision
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_' | ':' | '+'))
    {
        return Err(format!(
            "'{extension}' answered the revision {:?}: a revision is at most {MOST_REVISION_BYTES} \
             letters, digits and . - _ : +",
            crate::shown::readable(revision, 64)
        ));
    }
    Ok(())
}

/// One finding, as a lint answers it.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
struct AnsweredFinding {
    says: String,
}

/// A lint's answer.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
struct Findings {
    findings: Vec<AnsweredFinding>,
}

/// **A lint's findings**, read from its answer `doc`: each one line of words, bounded, that
/// purlis shows as a warning beside its own. A third party's finding never stops a save.
pub fn findings_of(
    extension: &str,
    doc: &serde_json::Map<String, serde_json::Value>,
) -> Result<Vec<String>, String> {
    if doc
        .get("findings")
        .and_then(serde_json::Value::as_array)
        .is_some_and(|list| list.len() > MOST_FINDINGS)
    {
        return Err(format!(
            "'{extension}' answered more than {MOST_FINDINGS} findings, the most purlis shows \
             from one lint"
        ));
    }
    let answered: Findings = answer_of(extension, doc, "findings")?;
    answered
        .findings
        .into_iter()
        .map(|finding| {
            let says = finding.says.trim();
            if says.is_empty() {
                return Err(format!(
                    "'{extension}' answered a finding that says nothing"
                ));
            }
            if says.len() > MOST_FINDING_BYTES || says.contains(crate::panel::undrawable) {
                return Err(format!(
                    "'{extension}' answered a finding purlis will not draw: it is longer than \
                     {MOST_FINDING_BYTES} bytes or holds a control or invisible formatting \
                     character"
                ));
            }
            Ok(says.to_owned())
        })
        .collect()
}

/// A template's answer.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
struct TemplateText {
    text: String,
}

/// **A template's text** for the new skill `name`, read from its answer `doc`: bounded, and a
/// whole skill that keeps its name, as any new skill's text is held to.
pub fn template_of(
    extension: &str,
    name: &str,
    doc: &serde_json::Map<String, serde_json::Value>,
) -> Result<String, String> {
    let answered: TemplateText = answer_of(extension, doc, "a template")?;
    let text = answered.text;
    if text.len() > MOST_TEMPLATE_BYTES {
        return Err(format!(
            "'{extension}' answered a template longer than {} KiB",
            MOST_TEMPLATE_BYTES >> 10
        ));
    }
    if let Some(problem) = purlis_skills::change::check(&text, name).first() {
        return Err(format!(
            "'{extension}' answered a template that is not a whole skill: {problem}"
        ));
    }
    Ok(text)
}

/// A panel's answer.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
struct Blocks {
    blocks: serde_json::Value,
}

/// **A panel's blocks**, read from its answer `doc` against the panel vocabulary.
pub fn blocks_of(
    extension: &str,
    doc: &serde_json::Map<String, serde_json::Value>,
) -> Result<Vec<crate::panel::Block>, String> {
    let answered: Blocks = answer_of(extension, doc, "a panel")?;
    crate::panel::answered(&answered.blocks).map_err(|why| format!("'{extension}' {why}"))
}

/// One skill a router hid, as it answers it.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
struct AnsweredHidden {
    skill: String,
    why: String,
}

/// A router's answer.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
struct Routed {
    hidden: Vec<AnsweredHidden>,
}

/// **A router's answer** (SK-15b), read from its answer `doc`: each skill it hides, by a skill's
/// name, with its own one line ([`purlis_skills::Line`]), at most as many as it was asked about.
/// Whether each is a skill it may hide is the set's to say ([`purlis_skills::SkillSet::routed_by`]).
pub fn routed_of(
    extension: &str,
    asked: usize,
    doc: &serde_json::Map<String, serde_json::Value>,
) -> Result<Vec<(String, purlis_skills::Line)>, String> {
    if doc
        .get("hidden")
        .and_then(serde_json::Value::as_array)
        .is_some_and(|list| list.len() > asked)
    {
        return Err(format!(
            "'{extension}' hid more skills than it was asked about"
        ));
    }
    let answered: Routed = answer_of(extension, doc, "a routing")?;
    answered
        .hidden
        .into_iter()
        .map(|one| {
            if let Some(why) = purlis_skills::change::name_problem(&one.skill) {
                return Err(format!(
                    "'{extension}' hid {:?}, which cannot be a skill's name: {why}",
                    crate::shown::readable(&one.skill, 64)
                ));
            }
            let line = purlis_skills::Line::new(&one.why)
                .map_err(|why| format!("'{extension}' hid {} with {why}", one.skill))?;
            Ok((one.skill, line))
        })
        .collect()
}

#[cfg(test)]
mod tests;
