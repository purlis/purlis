// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use super::*;
use std::time::Duration;

/// How long `purlis serve <args>` is left without a link before it suspends, as parsed.
fn asked(args: &[&str]) -> Result<Option<Duration>, clap::Error> {
    let words = ["purlis", "serve"].iter().chain(args).copied();
    match Cli::try_parse_from(words)?.command {
        Command::Serve {
            suspends_unlinked, ..
        } => Ok(suspends_after(suspends_unlinked)),
        _ => unreachable!("serve parses as serve"),
    }
}

#[test]
fn serve_suspends_only_when_asked_a_day_bare_and_a_pools_own_time_given() {
    assert_eq!(asked(&[]).unwrap(), None);
    assert_eq!(
        asked(&["--suspends-unlinked"]).unwrap(),
        Some(Duration::from_secs(24 * 60 * 60))
    );
    assert_eq!(
        asked(&["--suspends-unlinked=12h"]).unwrap(),
        Some(Duration::from_secs(12 * 60 * 60))
    );
    assert_eq!(
        asked(&["--suspends-unlinked=90m"]).unwrap(),
        Some(Duration::from_secs(90 * 60))
    );
}

#[test]
fn serve_refuses_a_time_a_pool_could_not_keep() {
    for refused in [
        "--suspends-unlinked=30s",
        "--suspends-unlinked=721h",
        "--suspends-unlinked=1d",
        "--suspends-unlinked=",
    ] {
        assert!(asked(&[refused]).is_err(), "{refused} was taken");
    }
}

/// What `purlis serve <args>` was asked to do with its host: install it, remove it, or serve it
/// as a service manager's.
fn kept(args: &[&str]) -> Result<(bool, bool, bool), clap::Error> {
    let words = ["purlis", "serve"].iter().chain(args).copied();
    match Cli::try_parse_from(words)?.command {
        Command::Serve {
            install,
            remove,
            as_a_service,
            ..
        } => Ok((install, remove, as_a_service)),
        _ => unreachable!("serve parses as serve"),
    }
}

#[test]
fn serve_installs_or_removes_its_host_and_runs_in_front_by_default() {
    assert_eq!(kept(&[]).unwrap(), (false, false, false));
    assert_eq!(kept(&["--foreground"]).unwrap(), (false, false, false));
    assert_eq!(kept(&["--install"]).unwrap(), (true, false, false));
    assert_eq!(kept(&["--remove"]).unwrap(), (false, true, false));
    assert_eq!(kept(&["--as-a-service"]).unwrap(), (false, false, true));
}

#[test]
fn serve_refuses_to_install_and_run_in_front_or_remove_at_once() {
    for refused in [
        &["--install", "--remove"][..],
        &["--install", "--foreground"],
        &["--remove", "--foreground"],
        &["--install", "--suspends-unlinked"],
        &["--install", "--as-a-service"],
    ] {
        assert!(kept(refused).is_err(), "{refused:?} was taken");
    }
}
