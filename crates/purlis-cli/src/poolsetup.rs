// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! `purlis pool setup`: Pool setup on the command line (spec #4469, Q44). It shows what the window
//! shows, from the same module ([`purlis_core::poolsetup`]), and changes nothing.

use purlis_core::dockerpool::Docker;
use purlis_core::poolsetup::{self, Here, Setup};

/// Prints pool `name`'s setup, or a new pool's, as JSON on stdout or as lines on stderr, where
/// purlis prints its reports.
pub fn answered(name: Option<&str>, json: bool) -> Result<(), String> {
    let config = purlis_core::machine::config_root()
        .ok_or_else(|| "this machine has no home folder to read pools from".to_owned())?;
    let plane = std::env::current_dir()
        .ok()
        .and_then(|cwd| purlis_core::plane::find_root(&cwd).ok());
    let docker = poolsetup::docker_here();
    let here = Here {
        config: &config,
        plane: plane.as_deref(),
        docker: docker.as_ref(),
        engine: Docker::engine_on_this_machine(),
    };
    let setup = poolsetup::read(&here, name).map_err(|why| why.to_string())?;
    if json {
        let text = serde_json::to_string_pretty(&setup).map_err(|why| why.to_string())?;
        println!("{text}");
        return Ok(());
    }
    eprint!("{}", said(&setup));
    if name.is_none() {
        let pools = purlis_core::runners::pools(&config).map_err(|why| why.to_string())?;
        if let Some(first) = pools.first() {
            let names: Vec<&str> = pools.iter().map(|pool| pool.name()).collect();
            eprintln!(
                "Pools on this machine: {}. `purlis pool setup {}` shows one.",
                names.join(", "),
                first.name()
            );
        }
    }
    Ok(())
}

/// The lines of a setup.
fn said(setup: &Setup) -> String {
    use std::fmt::Write as _;
    let mut out = match &setup.pool {
        Some(pool) => format!("Pool {pool}\n"),
        None => "A new pool\n".to_owned(),
    };
    for step in &setup.steps {
        let _ = write!(out, "  {:<10} {}", step.state.word(), step.name);
        if let Some(line) = &step.line {
            let _ = write!(out, ": {line}");
        }
        out.push('\n');
        for finding in &step.findings {
            let _ = writeln!(out, "             {finding}");
        }
    }
    let next = setup
        .at
        .and_then(|at| setup.step(at))
        .and_then(|step| step.next.as_ref());
    if let Some(next) = next {
        let _ = writeln!(out, "Next: {}. {}", next.label, next.does);
        if let Some(command) = &next.command {
            let _ = writeln!(out, "  {command}");
        }
    }
    out
}
