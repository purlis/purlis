// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The folder a chat is in now** (#6367, ADR 0126): where a relative path in its pane starts.
//!
//! A chat starts in a folder, and its program may move: a `cd` in a shell, a harness's tool
//! changing its working folder. Two things say where it is now, and neither is trusted for more
//! than where a relative path starts:
//!
//! - **Its harness's hook** ([`reported`], [`of_its_own_harness`]): the payload's `cwd`, read by
//!   the harness's adapter ([`crate::harness::HarnessAdapter::reported_folder`]), and believed
//!   only from the chat's own harness, as its mode is.
//! - **OSC 7 from its program** ([`said_by_osc7`]): `ESC ] 7 ; file://host/path ST`, which a shell
//!   sends at its prompt. The program's own words, so untrusted input: a folder on another host
//!   is none, and so is anything that is not a plain absolute path.
//!
//! The hook's report is taken first, then OSC 7, then the folder the chat started in
//! ([`now_in`]). **The chat's home stays the start folder's** ([`super::home_of`]), and so does
//! the list of folders a hover may look in although they are guarded: a folder the chat moved to
//! outside its workspace makes a relative path there no more than the same path typed in full,
//! which asks first, and one in a guarded folder is never looked in.

use std::path::{Component, Path, PathBuf};

/// The longest folder read, in bytes: a longer one is none. As long as a marked link's target.
pub const MOST_FOLDER: usize = super::osc8::MOST_TARGET;

/// **`path` as a folder a chat may be in**: absolute, at most [`MOST_FOLDER`] bytes, with no
/// control character and no `.` or `..` part. `None` for anything else.
fn plain_folder(path: &Path) -> Option<PathBuf> {
    let text = path.to_str()?;
    if text.len() > MOST_FOLDER || text.chars().any(char::is_control) || !path.is_absolute() {
        return None;
    }
    // Read by its text: `components` drops a `.` part without a word.
    let plain = text
        .split('/')
        .skip(1)
        .all(|part| part != "." && part != "..");
    (plain
        && path
            .components()
            .all(|part| matches!(part, Component::RootDir | Component::Normal(_))))
    .then(|| path.to_path_buf())
}

/// **The folder a hook's payload names as its `cwd`**, as an adapter that reads it says it: a
/// plain absolute folder ([`plain_folder`]), or `None`.
pub fn reported(cwd: Option<&str>) -> Option<PathBuf> {
    plain_folder(Path::new(cwd?))
}

/// **The folder `report` names as its chat's own harness's**, or `None`: only from no sub-agent,
/// and about `now`, the conversation the board holds for the chat after it took the report, as
/// [`crate::dispatchmode::of_its_own_harness`] reads its mode. A harness nested in the chat names
/// another conversation, and moves no folder of the chat's.
pub fn of_its_own_harness(report: &crate::hookwire::Report, now: Option<&str>) -> Option<PathBuf> {
    let crate::hookwire::Conversation::Named(said) = &report.conversation else {
        return None;
    };
    if report.agent.is_some() || now != Some(said.as_str()) {
        return None;
    }
    report.detail.folder.clone()
}

/// **The folder an OSC 7 sequence's `data` names**, on this machine (`this_host`, as the system
/// names it): `file:///path`, `file://localhost/path` or `file://<this host>/path`, the host
/// compared as [`same_host`] says. `None` for another
/// host, another scheme, a query or a fragment, and anything [`plain_folder`] refuses.
pub fn said_by_osc7(data: &str, this_host: &str) -> Option<PathBuf> {
    if data.len() > MOST_FOLDER || data.chars().any(char::is_control) {
        return None;
    }
    let url = url::Url::parse(data).ok()?;
    if url.scheme() != "file" || url.query().is_some() || url.fragment().is_some() {
        return None;
    }
    if !url.username().is_empty() || url.password().is_some() || url.port().is_some() {
        return None;
    }
    match url.host_str() {
        None | Some("") | Some("localhost") => {}
        Some(host) if same_host(host, this_host) => {}
        Some(_) => return None,
    }
    // `to_file_path` refuses a host it would hand on, and the host is this machine's: the path
    // is read again on its own (a URL whose host was taken off still refuses).
    let local = url::Url::parse(&format!("file://{}", url.path())).ok()?;
    plain_folder(&local.to_file_path().ok()?)
}

/// Whether `said`, a host an OSC 7 sequence names, is `this_host`: the same name without case
/// once a `.local` is taken off either, or a bare name that is this machine's first label (a
/// shell's `$HOST` and the system's name may differ so). `mac.evil.example` is not `mac`.
fn same_host(said: &str, this_host: &str) -> bool {
    let bare = |host: &str| {
        let host = host.to_ascii_lowercase();
        host.strip_suffix(".local")
            .map(str::to_owned)
            .unwrap_or(host)
    };
    let (said, this) = (bare(said), bare(this_host));
    if said.is_empty() || this.is_empty() {
        return false;
    }
    said == this || (!said.contains('.') && this.split('.').next() == Some(said.as_str()))
}

/// **This machine's name**, as an OSC 7 sequence's host names it.
pub fn this_host() -> String {
    #[cfg(unix)]
    {
        rustix::system::uname()
            .nodename()
            .to_string_lossy()
            .into_owned()
    }
    #[cfg(not(unix))]
    {
        String::new()
    }
}

/// **The folder a chat is in now**: what its own harness's hook last reported, else what its
/// program last said by OSC 7, else none (and a relative path starts from where it started).
pub fn now_in(reported: Option<PathBuf>, said: Option<PathBuf>) -> Option<PathBuf> {
    reported.or(said)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn osc7_names_a_folder_on_this_machine() {
        for data in [
            "file:///Users/me/src",
            "file://localhost/Users/me/src",
            "file://mac/Users/me/src",
            "file://MAC.local/Users/me/src",
        ] {
            assert_eq!(
                said_by_osc7(data, "mac.local"),
                Some(PathBuf::from("/Users/me/src")),
                "{data}"
            );
        }
        assert_eq!(
            said_by_osc7("file:///Users/me/my%20src", "mac"),
            Some(PathBuf::from("/Users/me/my src"))
        );
    }

    #[test]
    fn osc7_from_another_host_or_not_a_plain_folder_is_none() {
        for data in [
            "file://build-box/Users/me/src",
            "file://mac.evil.example/x",
            "http://mac/Users/me",
            "/Users/me/src",
            "src",
            "file:///Users/me?x=1",
            "file:///Users/me#top",
            "file://me@mac/Users/me",
            "file://mac:22/Users/me",
            "file:///Users/me/%0Aevil",
            "file:///Users/me/\u{7}",
            "",
        ] {
            assert_eq!(said_by_osc7(data, "mac"), None, "{data:?}");
        }
        let long = format!("file:///{}", "a".repeat(MOST_FOLDER));
        assert_eq!(said_by_osc7(&long, "mac"), None);
        // With no name for this machine, only an address naming no host is read.
        assert_eq!(said_by_osc7("file://mac/x", ""), None);
        assert_eq!(said_by_osc7("file:///x", ""), Some(PathBuf::from("/x")));
    }

    #[test]
    fn a_hook_s_cwd_is_a_folder_only_when_absolute_and_plain() {
        assert_eq!(
            reported(Some("/work/app")),
            Some(PathBuf::from("/work/app"))
        );
        for cwd in [
            None,
            Some(""),
            Some("app"),
            Some("/work/../etc"),
            Some("/a/./b"),
            Some("/a\nb"),
        ] {
            assert_eq!(reported(cwd), None, "{cwd:?}");
        }
    }

    #[test]
    fn the_hook_s_folder_comes_first_then_osc7() {
        let hook = Some(PathBuf::from("/hook"));
        let osc7 = Some(PathBuf::from("/osc7"));
        assert_eq!(now_in(hook.clone(), osc7.clone()), hook);
        assert_eq!(now_in(None, osc7.clone()), osc7);
        assert_eq!(now_in(None, None), None);
    }

    fn report(conversation: Option<&str>, agent: Option<&str>) -> crate::hookwire::Report {
        use crate::hookwire::Conversation;
        crate::hookwire::Report {
            chat: 7,
            event: crate::state::Event::Stop,
            conversation: conversation.map_or(Conversation::Unknown, |id| {
                Conversation::Named(id.to_owned())
            }),
            pid: Some(4007),
            agent: agent.map(str::to_owned),
            detail: crate::state::Detail {
                folder: Some(PathBuf::from("/work/app")),
                ..crate::state::Detail::default()
            },
        }
    }

    #[test]
    fn only_the_chat_s_own_harness_moves_its_folder() {
        let now = Some("11111111-2222-4333-8444-555555555555");
        assert_eq!(
            of_its_own_harness(&report(now, None), now),
            Some(PathBuf::from("/work/app"))
        );
        let nested = report(Some("99999999-2222-4333-8444-555555555555"), None);
        assert_eq!(of_its_own_harness(&nested, now), None);
        assert_eq!(of_its_own_harness(&report(None, None), now), None);
        assert_eq!(of_its_own_harness(&report(now, None), None), None);
        assert_eq!(of_its_own_harness(&report(now, Some("a1")), now), None);
    }
}
