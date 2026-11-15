// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! `runner host service`, `runner host after sign-out` and `runner host` (RR-15, #724): on a
//! runner, whether this machine keeps the session host running (a systemd user unit or a
//! LaunchAgent, or a container's own restart policy), whether it outlives this account's
//! sign-out (linger), and whether it answers now. Judged by `crate::runnerhost::rows`.
//!
//! **Only on a runner**: a purlis installed as a version in `<config>/server/<version>/`
//! (RR-14), or one whose host a service manager keeps. A container's purlis is not known for a
//! runner's by being in a container, so a container's doctor has these rows only where its purlis
//! is installed as a version. A desktop's doctor has none of these
//! rows, and the preflight none either: it runs at every chat's start, and asks no manager.

use super::{Doctor, Row, Status};
use crate::runnerhost::{self, Finding};

/// The rows, where this doctor found a runner's host.
pub(super) fn rows(d: &Doctor) -> Vec<Row> {
    d.runner_host
        .as_ref()
        .map(|look| runnerhost::rows(look).into_iter().map(row).collect())
        .unwrap_or_default()
}

fn row(found: Finding) -> Row {
    let status = match found.status {
        runnerhost::Status::Ok => Status::Ok,
        runnerhost::Status::Warn => Status::Warn,
        runnerhost::Status::Fail => Status::Fail,
    };
    Row::new(found.name, status, found.detail, found.hint)
}

/// **What this machine's runner host looks like**, where this purlis is a runner's.
pub(super) fn look_here() -> Option<runnerhost::Look> {
    let config = crate::machine::config_root_unheld()?;
    let binary = std::env::current_exe()
        .and_then(std::fs::canonicalize)
        .ok()?;
    let service = runnerhost::Service {
        version: crate::runnerinstall::version_of_binary(&config, &binary),
        binary,
        config_home: Some(config.clone()),
    };
    let here = runnerhost::Here::this(
        crate::machine::host_folder(&config).join("serve.log"),
        &config,
    )?;
    let socket = crate::hostsocket::of_version_here(
        here.runtime.as_deref(),
        &config,
        service.version.as_deref(),
    );
    let look = runnerhost::look(&here, &service, &socket);
    (service.version.is_some() || look.installed).then_some(look)
}
