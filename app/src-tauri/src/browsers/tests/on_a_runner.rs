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
        assert!(read.settled, "{read:?}");

        // The engine's processes in the runner: at least one in a process namespace of its own.
        let said = runner.said(ENGINE_NAMESPACES);
        assert!(
            said.lines().any(|line| line.split_whitespace().count() > 2),
            "no engine process in a namespace of its own: {said}"
        );
        browsers.close(&program).expect("the browser ends");
    }
}
