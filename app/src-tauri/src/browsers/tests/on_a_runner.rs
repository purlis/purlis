// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A chat's browser on a Docker pool's runner, on a real engine** (#3298, #2050): the window
//! starts it on the pool's runner through its own host, as it starts a runner chat's browser,
//! and the runner's host downloads the pinned engine and starts it there with its own sandbox
//! on. purlis never starts the engine without its sandbox, so a browser that starts is one whose
//! sandbox made its namespaces; the test also reads that from the runner's processes.
//!
//! It runs in the parity job against a Docker pool (`.github/workflows/ci.yml`), which builds
//! this commit's runner image: its name ends in `in_a_docker_pool`, as the behaviour suite's
//! Docker tests do. Elsewhere it runs nothing.

use std::sync::Arc;

use purlis_session_protocol::browsing::{Act, BrowserReach};

use crate::browsers::WindowBrowsers;
use crate::chatbrowsers::Driver as _;

/// The processes of the engine in the runner, each with the process ids its `status` says it
/// has, one for each process namespace it is in: more than one where the engine's sandbox put
/// it in a namespace of its own.
const ENGINE_NAMESPACES: &str = r#"for status in /proc/[0-9]*/status; do
  name=$(sed -n 's/^Name:[[:space:]]*//p' "$status" 2>/dev/null)
  case "$name" in chrome*) sed -n 's/^NSpid:[[:space:]]*/pids /p' "$status" 2>/dev/null ;; esac
done"#;

mod a_browser_starts_on_a_docker_pools_runner_with_its_engines_sandbox_on {
    use super::*;

    /// Run by the parity job, which names a runner image; elsewhere it runs nothing.
    #[test]
    fn in_a_docker_pool() {
        let Some(target) = crate::behaviour::in_a_docker_pool() else {
            return;
        };
        let runner = target.runner.as_ref().expect("a Docker pool's runner");
        let browsers = WindowBrowsers::new(Arc::clone(
            target.host.as_ref().expect("the window's link to its host"),
        ));

        let program = browsers
            .start(
                &target.root,
                Some(&runner.pool),
                BrowserReach::default(),
                None,
            )
            .unwrap_or_else(|why| panic!("the browser starts on the runner: {why:?}"));
        let read = browsers
            .act(&program, "c-1", Act::Pages, BrowserReach::default())
            .unwrap_or_else(|why| panic!("its pages are read on the runner: {why:?}"));
        // Its first page, blank: a fresh page may still be settling, which says nothing here.
        assert_eq!(read.pages.len(), 1, "{read:?}");

        // The engine's processes in the runner: at least one in a process namespace of its own.
        let said = runner.prints(ENGINE_NAMESPACES);
        assert!(
            said.lines().any(|line| line.split_whitespace().count() > 2),
            "no engine process in a namespace of its own: {said}"
        );
        browsers.close(&program).expect("the browser ends");
    }
}

/// **The browser suite on a Docker pool's runner** (#2050's close condition, with #2045's
/// target): a chat's browser on the runner opens a public page and reads it, through every hop
/// as the window drives it, and the person takes it over and hands it back. The engine on the
/// runner reaches the web from the runner's own network, so the page is a public one, which the
/// browser may reach as it may reach the open web.
mod a_runners_browser_opens_and_reads_a_page_and_is_taken_over_and_handed_back {
    use super::*;

    /// Run by the parity job, which names a runner image; elsewhere it runs nothing.
    #[test]
    fn in_a_docker_pool() {
        let Some(target) = crate::behaviour::in_a_docker_pool() else {
            return;
        };
        let runner = target.runner.as_ref().expect("a Docker pool's runner");
        let browsers = WindowBrowsers::new(Arc::clone(
            target.host.as_ref().expect("the window's link to its host"),
        ));
        let web = BrowserReach {
            open_web: true,
            ..BrowserReach::default()
        };
        let program = browsers
            .start(&target.root, Some(&runner.pool), web.clone(), None)
            .unwrap_or_else(|why| panic!("the browser starts on the runner: {why:?}"));

        let opened = browsers
            .act(
                &program,
                "c-1",
                Act::Open {
                    address: "https://example.com/".into(),
                },
                web.clone(),
            )
            .unwrap_or_else(|why| panic!("the page opens on the runner: {why:?}"));
        let page = opened.page.expect("the page it opened");
        assert!(page.address.starts_with("https://example.com"), "{page:?}");
        let read = browsers
            .act(
                &program,
                "c-2",
                Act::Snapshot {
                    page: None,
                    within: None,
                    from: None,
                    lines: None,
                },
                web.clone(),
            )
            .unwrap_or_else(|why| panic!("its page is read on the runner: {why:?}"));
        let text = read.text.unwrap_or_default();
        assert!(text.contains("Example Domain"), "{text}");

        browsers
            .take_over(&program)
            .expect("the person takes it over");
        let refused = browsers.act(&program, "c-3", Act::Pages, web.clone());
        assert!(refused.is_err(), "the chat acts on it no more: {refused:?}");
        browsers
            .hand_back(&program)
            .expect("the person hands it back");
        browsers
            .act(&program, "c-4", Act::Pages, web)
            .unwrap_or_else(|why| panic!("the chat acts on it again: {why:?}"));
        browsers.close(&program).expect("the browser ends");
    }
}
