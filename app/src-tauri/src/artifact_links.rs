// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A link in an artifact's page** (ADR 0120 §5, #5956): it opens the system browser, for
//! `https:` only, after the window has shown its host and its whole URL, unless the person chose
//! to always open that host on this machine.
//!
//! - **The page never navigates to it.** The navigation wall refuses anything off the page's
//!   own origin (`navguard.rs`, [`crate::artifacts::Artifacts::may_navigate`]). purlis's own
//!   script in every page ([`SCRIPT`]) sees a person's click on a link that leaves the page,
//!   stops it, and tells purlis on the page's own origin (`/.purlis/link?href=…`), which an
//!   image may ask under the CSP and nothing else. A click a script made (`isTrusted` false)
//!   is stopped and told nothing.
//! - **Only a link written in the template is followed** ([`classify`]): purlis reads the
//!   template it served for the very `href` the page reported
//!   ([`purlis_core::artifact::links::written`]), so a link the page's script built or rewrote
//!   is refused, even when an approved page forges the report.
//! - **`https:` only.** `file:`, `smb:`, `http:`, `javascript:`, another app's scheme, and
//!   `purlis:` (whose links into purlis are a ticket of their own) are refused with a sentence
//!   the window shows. So is a link carrying a name or a password.
//! - **The person decides** ([`artifact_link_open`]): the window asks, with the host and the
//!   whole URL; Open opens it, and Always open also keeps the host on this machine
//!   ([`purlis_core::artifact::links::always_open`], Machine tier). What is opened is what the
//!   page's webview reported, held here by its label, never a URL the window names.

use std::collections::HashMap;
use std::sync::{Mutex, PoisonError};

use purlis_core::artifact::links;
use serde::Serialize;
use tauri::{Emitter, Manager, Runtime, Url};

/// purlis's script in every artifact page: a person's click on a link that leaves the page
/// is stopped and told to purlis on the page's own origin. A click a script made follows
/// nothing.
pub(crate) const SCRIPT: &str = r#"(() => {
  const own = (to) => to.protocol === location.protocol && to.host === location.host;
  const follow = (e) => {
    if (e.type === "auxclick" && e.button !== 1) return;
    const link = e.target instanceof Element ? e.target.closest("a[href], area[href]") : null;
    if (link === null) return;
    const href = link.getAttribute("href");
    let to;
    try { to = new URL(href, location.href); } catch { to = null; }
    if (to !== null && own(to)) return;
    e.preventDefault();
    e.stopImmediatePropagation();
    if (!e.isTrusted) return;
    new Image().src = "/.purlis/link?href=" + encodeURIComponent(href);
  };
  addEventListener("click", follow, true);
  addEventListener("auxclick", follow, true);
})();"#;

/// The path under the page's origin [`SCRIPT`] reports a link on.
pub(crate) const PATH: &str = "/.purlis/link";

/// What the window hears when a person follows a link in an artifact's page that purlis asks
/// about or refuses.
pub(crate) const ARTIFACT_LINK: &str = "artifact-link";

/// What [`ARTIFACT_LINK`] carries: the artifact webview's label, and either the link to ask
/// about (its whole URL and its host) or the sentence that refused it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
pub struct ArtifactLink {
    pub label: String,
    pub url: Option<String>,
    pub host: Option<String>,
    pub refused: Option<String>,
}

/// A link the page's person followed, as the artifact's webview reported it.
pub(crate) struct Followed {
    /// The window the page is in.
    pub(crate) window: String,
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
        "purlis" => {
            return Some(Link::Refused(
                "purlis does not follow its own links from an artifact yet, so it did not open \
                 this one"
                    .into(),
            ));
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

/// The `href` a request for `uri` reports, decoded once, when it asks [`PATH`].
pub(crate) fn href_of(uri: &str) -> Option<String> {
    let asked = Url::parse(uri).ok()?;
    if asked.path() != PATH {
        return None;
    }
    asked
        .query_pairs()
        .find(|(name, _)| name == "href")
        .map(|(_, value)| value.into_owned())
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
    /// What a scenario test's build opened, in place of the browser.
    #[cfg(feature = "e2e")]
    opened: Mutex<Vec<String>>,
}

impl Links {
    fn asking(&self) -> std::sync::MutexGuard<'_, HashMap<String, Asking>> {
        self.asking.lock().unwrap_or_else(PoisonError::into_inner)
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
    let said = match link {
        Link::Refused(why) => ArtifactLink {
            label: label.to_owned(),
            url: None,
            host: None,
            refused: Some(why),
        },
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
            }
        }
    };
    let _ = app.emit_to(
        tauri::EventTarget::webview(followed.window.as_str()),
        ARTIFACT_LINK,
        said,
    );
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
        assert!(refused("purlis://workspace/alpha").contains("does not follow its own links"));
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

    #[test]
    fn the_reported_href_is_decoded_once_from_the_query() {
        assert_eq!(
            href_of(&format!(
                "{OWN}/.purlis/link?href=https%3A%2F%2Fexample.com%2F%3Fa%3D1%26b%3D2"
            ))
            .as_deref(),
            Some("https://example.com/?a=1&b=2")
        );
        assert_eq!(href_of(&format!("{OWN}/.purlis/link?other=x")), None);
        assert_eq!(href_of(&format!("{OWN}/.purlis/key/escape?href=x")), None);
        assert_eq!(href_of(&format!("{OWN}/index.html?href=x")), None);
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
