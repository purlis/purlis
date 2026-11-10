// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A link in an artifact's page** (ADR 0122 §5, #5956): it opens the system browser, for
//! `https:` only, after the window has shown its host and its whole URL, unless the person chose
//! to always open that host on this machine.
//!
//! - **The page never navigates to it.** The navigation wall refuses anything off the page's
//!   own origin (`navguard.rs`, [`crate::artifacts::Artifacts::may_navigate`]). purlis's own
//!   script in every page ([`SCRIPT`]) sees a person's click on a link that leaves the page,
//!   stops it, and tells purlis on the page's own origin (`/.purlis/link?href=…`), which an
//!   image may ask under the CSP and nothing else. A click a script made (`isTrusted` false)
//!   is stopped and told nothing.
//! - **Only a press is followed** ([`Links::heard`]): a report carries the one token purlis
//!   handed the page's open load, which lives in purlis's script and nowhere the page can
//!   read, and is spent by the report. A report the page makes itself, an `<img>` naming the
//!   path among them, holds none and follows nothing: no always-open host opens, no dialog
//!   comes up and no `purlis:` link navigates. One page follows at most one link per
//!   half-second.
//! - **Only a link written in the template is followed** ([`classify`]): purlis reads the
//!   template it served for the very `href` the page reported
//!   ([`purlis_core::artifact::links::written`]), so a link the page's script built or rewrote
//!   is refused, even when an approved page forges the report.
//! - **`https:` to the browser.** `file:`, `smb:`, `http:`, `javascript:` and another app's
//!   scheme are refused with a sentence the window shows. So is a link carrying a name or a
//!   password.
//! - **`purlis:` navigates inside purlis, never acts** (#5967): the core reads it into a place
//!   of a closed list ([`purlis_core::artifact::places`]) and refuses anything else, an action
//!   included; [`crate::artifact_places`] checks the place is in the artifact's project, and the
//!   window goes there without asking, as a press on its own row would.
//! - **The person decides** ([`artifact_link_open`]): the window asks, with the host and the
//!   whole URL; Open opens it, and Always open also keeps the host on this machine
//!   ([`purlis_core::artifact::links::always_open`], Machine tier). What is opened is what the
//!   page's webview reported, held here by its label, never a URL the window names.

use std::collections::HashMap;
use std::sync::{Mutex, PoisonError};
use std::time::{Duration, Instant};

use purlis_core::artifact::{links, places};
use serde::Serialize;
use tauri::{Emitter, Manager, Runtime, Url};

use crate::artifact_places::{self, Goes};
use crate::planes::{PlaneId, Planes};

/// purlis's script in every artifact page: a person's click on a link that leaves the page
/// is stopped and told to purlis on the page's own origin, with the one token purlis handed
/// this load ([`Links::heard`]). A click a script made follows nothing, and nor does a report
/// the page makes itself (an `<img>` naming the path), which holds no token.
///
/// The token lives in this script's closure, handed in by purlis through `__purlisLink`, a
/// property the page cannot replace. Everything the report is built with is taken before the
/// page's own script runs, so a page that rewrites `Element.prototype.closest`, `Event`'s
/// `target` or an image's `src` cannot read the token or make a press report another link.
pub(crate) const SCRIPT: &str = r#"(() => {
  const apply = Reflect.apply;
  const targetOf = Object.getOwnPropertyDescriptor(Event.prototype, "target").get;
  const closest = Element.prototype.closest;
  const attribute = Element.prototype.getAttribute;
  const setSrc = Object.getOwnPropertyDescriptor(HTMLImageElement.prototype, "src").set;
  const encode = encodeURIComponent;
  const ImageOf = Image;
  const ElementOf = Element;
  const URLOf = URL;
  const send = (path) => apply(setSrc, new ImageOf(), [path]);
  const load = Array.from(crypto.getRandomValues(new Uint8Array(16)), (b) => b.toString(16).padStart(2, "0")).join("");
  let token = null;
  Object.defineProperty(window, "__purlisLink", { value: (to, next) => { if (to === load && typeof next === "string") token = next; } });
  send("/.purlis/link/open/" + load);
  const own = (to) => to.protocol === location.protocol && to.host === location.host;
  const follow = (e) => {
    if (e.type === "auxclick" && e.button !== 1) return;
    const target = apply(targetOf, e, []);
    const link = target instanceof ElementOf ? apply(closest, target, ["a[href], area[href]"]) : null;
    if (link === null) return;
    const href = apply(attribute, link, ["href"]);
    let to;
    try { to = new URLOf(href, location.href); } catch { to = null; }
    if (to !== null && own(to)) return;
    e.preventDefault();
    e.stopImmediatePropagation();
    if (!e.isTrusted || token === null) return;
    const spent = token;
    token = null;
    send("/.purlis/link?href=" + encode(href) + "&load=" + load + "&token=" + spent);
  };
  addEventListener("click", follow, true);
  addEventListener("auxclick", follow, true);
})();"#;

/// The path under the page's origin [`SCRIPT`] reports a link on.
pub(crate) const PATH: &str = "/.purlis/link";

/// The path under the page's origin [`SCRIPT`] opens its load on, its word after it.
const OPEN_PATH: &str = "/.purlis/link/open/";

/// The function [`SCRIPT`] leaves for purlis to hand a load its token with.
const TOKEN: &str = "__purlisLink";

/// The least time between two links one page follows: a person's presses, never a stream.
const LEAST_GAP: Duration = Duration::from_millis(500);

/// What the window hears when a person follows a link in an artifact's page that purlis asks
/// about or refuses.
pub(crate) const ARTIFACT_LINK: &str = "artifact-link";

/// What [`ARTIFACT_LINK`] carries: the artifact webview's label, and one of the link to ask
/// about (its whole URL and its host), the place in purlis to go to (#5967), or the sentence
/// that refused it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
pub struct ArtifactLink {
    pub label: String,
    pub url: Option<String>,
    pub host: Option<String>,
    pub refused: Option<String>,
    pub goes: Option<Goes>,
}

/// A link the page's person followed, as the artifact's webview reported it.
pub(crate) struct Followed {
    /// The window the page is in.
    pub(crate) window: String,
    /// The project the artifact is of.
    pub(crate) plane: PlaneId,
    /// The page's own origin, as its webview spells it.
    pub(crate) own: String,
    /// The link's `href`, as the page holds it.
    pub(crate) href: String,
    /// Whether the template purlis served holds that `href` as a link.
    pub(crate) written: bool,
}

/// What purlis does with a link that leaves the page.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Link {
    /// An `https:` link to `host`: opened in the browser, asked first.
    Browser { url: String, host: String },
    /// A `purlis:` link to a place in purlis, gone to without asking: it only navigates.
    Place(places::Place),
    /// Not opened, and the sentence that says why.
    Refused(String),
}

/// **What purlis does with `href`, followed in a page served at `own`**: nothing when it
/// stays on the page's own origin (the page goes there itself), otherwise a [`Link`].
pub(crate) fn classify(own: &str, href: &str, written: bool) -> Option<Link> {
    let base = Url::parse(&format!("{own}/")).ok()?;
    let Ok(to) = base.join(href) else {
        return Some(Link::Refused(
            "purlis could not read where that link goes, so it did not open it".into(),
        ));
    };
    let stays = to.scheme() == base.scheme()
        && to.host_str() == base.host_str()
        && to.port() == base.port();
    if stays {
        return None;
    }
    if !written {
        return Some(Link::Refused(
            "purlis opens only a link written in the artifact's page, and its script made this one"
                .into(),
        ));
    }
    match to.scheme() {
        "https" => {}
        places::SCHEME => {
            return Some(match places::place(href) {
                Ok(place) => Link::Place(place),
                Err(why) => Link::Refused(why),
            });
        }
        other => {
            return Some(Link::Refused(format!(
                "purlis opens only https links from an artifact, so it will not open this {other}: link"
            )));
        }
    }
    if !to.username().is_empty() || to.password().is_some() {
        return Some(Link::Refused(
            "purlis will not open a link that carries a name or a password".into(),
        ));
    }
    let host = to.host_str()?;
    let host = match to.port() {
        Some(port) => format!("{host}:{port}"),
        None => host.to_owned(),
    };
    Some(Link::Browser {
        url: to.to_string(),
        host,
    })
}

/// What a request for `uri` asks of the links of its page: a load opened, a link reported
/// with the load and token it names, or neither.
#[derive(Debug, PartialEq, Eq)]
enum Asked {
    Open {
        load: String,
    },
    Report {
        href: String,
        load: String,
        token: String,
    },
}

fn asked(uri: &str) -> Option<Asked> {
    let asked = Url::parse(uri).ok()?;
    if let Some(load) = asked.path().strip_prefix(OPEN_PATH) {
        return is_word(load).then(|| Asked::Open {
            load: load.to_owned(),
        });
    }
    if asked.path() != PATH {
        return None;
    }
    let pair = |key: &str| {
        asked
            .query_pairs()
            .find(|(name, _)| name == key)
            .map(|(_, value)| value.into_owned())
    };
    Some(Asked::Report {
        href: pair("href")?,
        load: pair("load").filter(|w| is_word(w))?,
        token: pair("token").filter(|w| is_word(w))?,
    })
}

/// Whether `word` is a load's word or a token: 32 lowercase hex digits.
fn is_word(word: &str) -> bool {
    word.len() == 32 && word.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
}

/// A fresh token: 16 random bytes, as 32 lowercase hex digits.
fn minted() -> Option<String> {
    let mut bytes = [0u8; 16];
    getrandom::fill(&mut bytes).ok()?;
    Some(bytes.iter().map(|b| format!("{b:02x}")).collect())
}

/// The one token a page's open load holds: unspent until a report carries it.
struct Held {
    load: String,
    token: String,
    /// When this page last followed a link.
    last: Option<Instant>,
}

/// What purlis does with a request its page made of its links ([`Links::heard`]).
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Heard {
    /// Nothing: it was no link's, or it held no token of the open load, or came too soon.
    Nothing,
    /// Hand `token` to load `load`, as [`delivery`] says it.
    Hand { load: String, token: String },
    /// Follow `href`, and hand load `load` its next token.
    Follow {
        href: String,
        load: String,
        token: String,
    },
}

/// The script that hands load `load` its token `token`: data, never markup.
pub(crate) fn delivery(load: &str, token: &str) -> String {
    format!(
        "window.{TOKEN} && window.{TOKEN}({}, {});",
        serde_json::Value::String(load.to_owned()),
        serde_json::Value::String(token.to_owned())
    )
}

/// A link the window was asked about and has not answered.
struct Asking {
    window: String,
    url: String,
    host: String,
}

/// The links waiting on the person's answer, by the label of the artifact webview that
/// reported each: one per page, the last one followed.
#[derive(Default)]
pub(crate) struct Links {
    asking: Mutex<HashMap<String, Asking>>,
    /// Each page's open load and the token it holds, by its webview's label.
    held: Mutex<HashMap<String, Held>>,
    /// What a scenario test's build opened, in place of the browser.
    #[cfg(feature = "e2e")]
    opened: Mutex<Vec<String>>,
}

impl Links {
    fn asking(&self) -> std::sync::MutexGuard<'_, HashMap<String, Asking>> {
        self.asking.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// **What a request for `uri` from artifact webview `label` does to its links**, at `now`.
    ///
    /// - A load opening (`/.purlis/link/open/<word>`) is handed a fresh token, and becomes
    ///   the page's one open load: an older load's token is spent with it.
    /// - A report is followed only when it names the open load and carries its unspent token,
    ///   and at least [`LEAST_GAP`] after the page's last. The token is spent then, and the
    ///   load is handed the next. Anything else, a report an `<img>` made among it, is nothing.
    pub(crate) fn heard(&self, label: &str, uri: &str, now: Instant) -> Heard {
        let mut held = self.held.lock().unwrap_or_else(PoisonError::into_inner);
        match asked(uri) {
            None => Heard::Nothing,
            Some(Asked::Open { load }) => {
                let Some(token) = minted() else {
                    return Heard::Nothing;
                };
                let last = held.get(label).and_then(|one| one.last);
                held.insert(
                    label.to_owned(),
                    Held {
                        load: load.clone(),
                        token: token.clone(),
                        last,
                    },
                );
                Heard::Hand { load, token }
            }
            Some(Asked::Report { href, load, token }) => {
                let Some(one) = held
                    .get_mut(label)
                    .filter(|one| one.load == load && one.token == token)
                else {
                    return Heard::Nothing;
                };
                if one
                    .last
                    .is_some_and(|last| now.saturating_duration_since(last) < LEAST_GAP)
                {
                    return Heard::Nothing;
                }
                let Some(next) = minted() else {
                    return Heard::Nothing;
                };
                one.token.clone_from(&next);
                one.last = Some(now);
                Heard::Follow {
                    href,
                    load,
                    token: next,
                }
            }
        }
    }

    /// Forgets artifact webview `label`'s load and token: it closed.
    pub(crate) fn forget(&self, label: &str) {
        self.held
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .remove(label);
        self.asking().remove(label);
    }

    /// The link artifact webview `label` of window `window` is waiting on, taken.
    fn take(&self, label: &str, window: &str) -> Option<Asking> {
        let mut asking = self.asking();
        if asking.get(label).is_some_and(|one| one.window == window) {
            asking.remove(label)
        } else {
            None
        }
    }
}

/// **A person followed a link in artifact webview `label`'s page**: refused with a sentence,
/// opened (an always-open host), or asked about in its window.
pub(crate) fn follow<R: Runtime>(app: &tauri::AppHandle<R>, label: &str, followed: Followed) {
    let Some(link) = classify(&followed.own, &followed.href, followed.written) else {
        return;
    };
    let refused = |why: String| ArtifactLink {
        label: label.to_owned(),
        url: None,
        host: None,
        refused: Some(why),
        goes: None,
    };
    let said = match link {
        Link::Refused(why) => refused(why),
        Link::Place(place) => {
            // Read off the page's thread: an issue's chat is found by folding the work log.
            let (app, label) = (app.clone(), label.to_owned());
            tauri::async_runtime::spawn_blocking(move || {
                let said = match place_in(&app, &followed.plane, place) {
                    Ok(goes) => ArtifactLink {
                        label,
                        url: None,
                        host: None,
                        refused: None,
                        goes: Some(goes),
                    },
                    Err(why) => ArtifactLink {
                        label,
                        url: None,
                        host: None,
                        refused: Some(why),
                        goes: None,
                    },
                };
                let _ = app.emit_to(
                    tauri::EventTarget::webview(followed.window.as_str()),
                    ARTIFACT_LINK,
                    said,
                );
            });
            return;
        }
        Link::Browser { url, host } => {
            let config = purlis_core::machine::config_root();
            if links::always_opens(config.as_deref(), &host) {
                if let Err(why) = open(app, &url) {
                    tracing::warn!("an artifact's link to {host} did not open: {why}");
                }
                return;
            }
            let Some(state) = app.try_state::<Links>() else {
                return;
            };
            state.asking().insert(
                label.to_owned(),
                Asking {
                    window: followed.window.clone(),
                    url: url.clone(),
                    host: host.clone(),
                },
            );
            ArtifactLink {
                label: label.to_owned(),
                url: Some(url),
                host: Some(host),
                refused: None,
                goes: None,
            }
        }
    };
    let _ = app.emit_to(
        tauri::EventTarget::webview(followed.window.as_str()),
        ARTIFACT_LINK,
        said,
    );
}

/// **Where the window goes for `place`**, checked against project `plane` as it is now.
fn place_in<R: Runtime>(
    app: &tauri::AppHandle<R>,
    plane: &PlaneId,
    place: places::Place,
) -> Result<Goes, String> {
    let planes = app
        .try_state::<Planes>()
        .ok_or_else(|| "purlis has no project open, so it did not follow the link".to_owned())?;
    let held = planes.held(plane)?;
    artifact_places::goes(
        place,
        &artifact_places::Project {
            root: held.root(),
            chats: held.chats(),
        },
    )
}

/// Opens `url` in the system browser; in a scenario test's build, notes it instead.
fn open<R: Runtime>(app: &tauri::AppHandle<R>, url: &str) -> Result<(), String> {
    #[cfg(feature = "e2e")]
    {
        if let Some(links) = app.try_state::<Links>() {
            links
                .opened
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .push(url.to_owned());
        }
        Ok(())
    }
    #[cfg(not(feature = "e2e"))]
    {
        use tauri_plugin_opener::OpenerExt as _;
        app.opener()
            .open_url(url, None::<&str>)
            .map_err(|e| format!("purlis could not open the link in your browser: {e}"))
    }
}

/// **The person's answer to a link an artifact's page followed**, pressed in the window's
/// dialog: opens in the browser the link artifact webview `label` reported, never a URL the
/// window names, and with `always`, keeps its host on this machine first.
#[tauri::command]
#[specta::specta]
pub async fn artifact_link_open(
    app: tauri::AppHandle,
    window: tauri::Window,
    links: tauri::State<'_, Links>,
    label: String,
    always: bool,
) -> Result<(), String> {
    let Some(asking) = links.take(&label, window.label()) else {
        return Err("that link is not waiting to be opened any more".into());
    };
    if always {
        let config = crate::extensions::config_root().map_err(|_| {
            "this machine has no config home, so purlis cannot keep the choice".to_owned()
        })?;
        let host = asking.host.clone();
        tauri::async_runtime::spawn_blocking(move || links::always_open(&config, &host))
            .await
            .map_err(|e| format!("purlis could not keep the choice: {e}"))??;
    }
    open(&app, &asking.url)
}

/// **As if the person followed `href` in artifact webview `label`'s page** (the scenario tests:
/// WebDriver cannot reach into the page's own webview). The `e2e` build only.
#[cfg(feature = "e2e")]
#[tauri::command]
#[specta::specta]
pub fn artifact_link_follow(
    app: tauri::AppHandle,
    artifacts: tauri::State<'_, crate::artifacts::Artifacts>,
    label: String,
    href: String,
) -> Result<(), String> {
    let followed = artifacts
        .followed(&label, &href)
        .ok_or_else(|| format!("no artifact webview is called {label}"))?;
    follow(&app, &label, followed);
    Ok(())
}

/// **What this run opened in place of the browser**, in order. The `e2e` build only.
#[cfg(feature = "e2e")]
#[tauri::command]
#[specta::specta]
pub fn artifact_links_opened(links: tauri::State<'_, Links>) -> Vec<String> {
    links
        .opened
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .clone()
}

#[cfg(test)]
mod tests {
    use super::*;

    const OWN: &str = "artifact://0123abcd";

    fn browser(url: &str, host: &str) -> Option<Link> {
        Some(Link::Browser {
            url: url.to_owned(),
            host: host.to_owned(),
        })
    }

    fn refused(href: &str) -> String {
        match classify(OWN, href, true) {
            Some(Link::Refused(why)) => why,
            other => panic!("{href} was not refused: {other:?}"),
        }
    }

    #[test]
    fn an_https_link_written_in_the_page_opens_in_the_browser_with_its_host() {
        assert_eq!(
            classify(
                OWN,
                "https://github.com/purlis/purlis/issues/1?q=a b#top",
                true
            ),
            browser(
                "https://github.com/purlis/purlis/issues/1?q=a%20b#top",
                "github.com"
            )
        );
        assert_eq!(
            classify(OWN, "HTTPS://Example.COM:8443/x", true),
            browser("https://example.com:8443/x", "example.com:8443")
        );
        assert_eq!(
            classify(OWN, "https://example.com:443/", true),
            browser("https://example.com/", "example.com")
        );
    }

    #[test]
    fn a_link_within_the_page_s_own_origin_is_the_page_s_to_follow() {
        for within in [
            "other.html",
            "/sub/page.html#x",
            "#top",
            "artifact://0123abcd/a",
        ] {
            assert_eq!(classify(OWN, within, false), None, "{within}");
        }
    }

    #[test]
    fn file_smb_and_other_schemes_are_refused_with_a_sentence_naming_the_scheme() {
        for (href, scheme) in [
            ("file:///etc/passwd", "file"),
            ("smb://server/share", "smb"),
            ("http://example.com/", "http"),
            ("vscode://file/x", "vscode"),
            ("slack://open", "slack"),
            ("javascript:alert(1)", "javascript"),
            ("mailto:a@example.com", "mailto"),
            ("artifact://ffff/", "artifact"),
        ] {
            assert_eq!(
                refused(href),
                format!(
                    "purlis opens only https links from an artifact, so it will not open this {scheme}: link"
                ),
                "{href}"
            );
        }
    }

    #[test]
    fn a_purlis_link_written_in_the_page_goes_to_its_place() {
        assert_eq!(
            classify(OWN, "purlis://workspace/alpha", true),
            Some(Link::Place(places::Place::Workspace {
                workspace: "alpha".into()
            }))
        );
        assert_eq!(
            classify(OWN, "purlis://todo/alpha/fix-login", true),
            Some(Link::Place(places::Place::Todo {
                workspace: "alpha".into(),
                todo: "fix-login".into()
            }))
        );
    }

    #[test]
    fn a_purlis_link_naming_an_action_or_made_by_the_page_s_script_is_refused() {
        assert!(refused("purlis://run/regenerate").contains("`run` is none of them"));
        assert!(refused("purlis://todo/alpha/fix-login/done").contains("(`done`)"));
        assert!(matches!(
            classify(OWN, "purlis://workspace/alpha", false),
            Some(Link::Refused(why)) if why.contains("its script made this one")
        ));
    }

    #[test]
    fn a_link_the_page_s_script_made_is_refused_whatever_it_names() {
        for href in ["https://github.com/", "file:///etc/passwd"] {
            assert_eq!(
                classify(OWN, href, false),
                Some(Link::Refused(
                    "purlis opens only a link written in the artifact's page, and its script made this one"
                        .into()
                )),
                "{href}"
            );
        }
    }

    #[test]
    fn a_link_carrying_a_name_or_a_password_is_refused() {
        for href in [
            "https://github.com@evil.example/",
            "https://user:pass@example.com/",
        ] {
            assert!(refused(href).contains("name or a password"), "{href}");
        }
    }

    const LOAD: &str = "0123456789abcdef0123456789abcdef";

    fn report(href: &str, load: &str, token: &str) -> String {
        format!(
            "{OWN}/.purlis/link?href={}&load={load}&token={token}",
            percent_encoding::utf8_percent_encode(href, percent_encoding::NON_ALPHANUMERIC)
        )
    }

    /// Opens load `load` of webview `label`: the token it is handed.
    fn opened(links: &Links, label: &str, load: &str, at: Instant) -> String {
        match links.heard(label, &format!("{OWN}{OPEN_PATH}{load}"), at) {
            Heard::Hand { load: to, token } if to == load => token,
            other => panic!("not handed a token: {other:?}"),
        }
    }

    /// The exploit of the review of #5978: `<img src="/.purlis/link?href=…">` beside a
    /// matching `<a>` reported a follow with nothing pressed, so an always-open host opened,
    /// the dialog came up, or a `purlis:` link navigated.
    #[test]
    fn a_report_an_image_made_with_no_press_follows_nothing() {
        let links = Links::default();
        let now = Instant::now();
        let bare = format!("{OWN}/.purlis/link?href=https%3A%2F%2Fexample.com%2F");
        assert_eq!(links.heard("artifact-1", &bare, now), Heard::Nothing);
        // Its load open, still nothing without the token purlis handed it.
        let _ = opened(&links, "artifact-1", LOAD, now);
        assert_eq!(links.heard("artifact-1", &bare, now), Heard::Nothing);
        let guessed = report("https://example.com/", LOAD, &"0".repeat(32));
        assert_eq!(links.heard("artifact-1", &guessed, now), Heard::Nothing);
    }

    #[test]
    fn a_press_s_token_follows_once_and_hands_the_load_its_next() {
        let links = Links::default();
        let now = Instant::now();
        let token = opened(&links, "artifact-1", LOAD, now);
        let href = "https://example.com/?a=1&b=2";
        let next = match links.heard("artifact-1", &report(href, LOAD, &token), now) {
            Heard::Follow {
                href: followed,
                load,
                token: next,
            } => {
                assert_eq!(followed, href, "decoded once");
                assert_eq!(load, LOAD);
                next
            }
            other => panic!("not followed: {other:?}"),
        };
        assert_ne!(next, token);
        // Spent: the same report again, as a page that read it would replay it, is nothing.
        let later = now + LEAST_GAP;
        assert_eq!(
            links.heard("artifact-1", &report(href, LOAD, &token), later),
            Heard::Nothing
        );
        assert!(matches!(
            links.heard("artifact-1", &report(href, LOAD, &next), later),
            Heard::Follow { .. }
        ));
    }

    #[test]
    fn a_token_is_its_own_webview_s_and_its_open_load_s_only() {
        let links = Links::default();
        let now = Instant::now();
        let token = opened(&links, "artifact-1", LOAD, now);
        let href = "https://example.com/";
        assert_eq!(
            links.heard("artifact-2", &report(href, LOAD, &token), now),
            Heard::Nothing
        );
        let other = "fedcba9876543210fedcba9876543210";
        assert_eq!(
            links.heard("artifact-1", &report(href, other, &token), now),
            Heard::Nothing
        );
        // A newer load (a reload, or a load the page opened itself) spends the older one's.
        let _ = opened(&links, "artifact-1", other, now);
        assert_eq!(
            links.heard("artifact-1", &report(href, LOAD, &token), now),
            Heard::Nothing
        );
        links.forget("artifact-1");
        assert_eq!(
            links.heard("artifact-1", &report(href, LOAD, &token), now),
            Heard::Nothing
        );
    }

    #[test]
    fn a_page_follows_one_link_per_gap_at_most() {
        let links = Links::default();
        let now = Instant::now();
        let token = opened(&links, "artifact-1", LOAD, now);
        let href = "https://example.com/";
        let Heard::Follow { token: next, .. } =
            links.heard("artifact-1", &report(href, LOAD, &token), now)
        else {
            panic!("the first press follows");
        };
        let soon = now + LEAST_GAP / 2;
        assert_eq!(
            links.heard("artifact-1", &report(href, LOAD, &next), soon),
            Heard::Nothing
        );
        // Too soon is not spent: the person's next press, after the gap, follows.
        assert!(matches!(
            links.heard("artifact-1", &report(href, LOAD, &next), now + LEAST_GAP),
            Heard::Follow { .. }
        ));
    }

    #[test]
    fn only_a_load_s_word_opens_and_only_the_link_path_reports() {
        let links = Links::default();
        let now = Instant::now();
        for not in [
            format!("{OWN}{OPEN_PATH}short"),
            format!("{OWN}{OPEN_PATH}{}", LOAD.to_uppercase()),
            format!("{OWN}/.purlis/key/escape?href=x"),
            format!("{OWN}/index.html?href=x&load={LOAD}&token={LOAD}"),
        ] {
            assert_eq!(
                links.heard("artifact-1", &not, now),
                Heard::Nothing,
                "{not}"
            );
        }
    }

    #[test]
    fn a_token_reaches_the_page_as_data_never_as_markup() {
        assert_eq!(
            delivery(LOAD, "abc\"</script>"),
            format!(
                "window.__purlisLink && window.__purlisLink(\"{LOAD}\", \"abc\\\"</script>\");"
            )
        );
    }

    #[test]
    fn a_link_is_opened_only_by_the_window_its_page_is_in_and_only_once() {
        let links = Links::default();
        links.asking().insert(
            "artifact-1".into(),
            Asking {
                window: "main".into(),
                url: "https://example.com/".into(),
                host: "example.com".into(),
            },
        );
        assert!(links.take("artifact-1", "window-2").is_none());
        assert!(links.take("artifact-2", "main").is_none());
        assert_eq!(
            links
                .take("artifact-1", "main")
                .map(|one| one.url)
                .as_deref(),
            Some("https://example.com/")
        );
        assert!(links.take("artifact-1", "main").is_none());
    }
}
