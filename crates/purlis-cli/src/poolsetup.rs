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
    // A cluster is asked through the person's own kubectl, as the window asks it (#4492).
    let kubectl = poolsetup::cluster::kubectl_here();
    let here = Here {
        config: &config,
        plane: plane.as_deref(),
        docker: docker.as_ref(),
        engine: Docker::engine_on_this_machine(),
        kubectl: kubectl.as_ref().map_err(String::as_str),
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

/// **`purlis pool doctor <name>`** (#6270, ADR 0123 §3): whether pool `name`'s machine and engine
/// are the ones the person trusted, in the window's words, on stdout. Exits 1 where they aren't or
/// the machine can't be reached. It reaches the machine as a run for the pool does, and changes
/// nothing.
pub fn doctor(name: &str) -> Result<u8, String> {
    let config = purlis_core::machine::config_root()
        .ok_or_else(|| "this machine has no home folder to read pools from".to_owned())?;
    let pools = purlis_core::runners::pools(&config).map_err(|why| why.to_string())?;
    let pool = pools
        .iter()
        .find(|pool| pool.name() == name)
        .ok_or_else(|| format!("there is no pool named {name} on this machine"))?;
    let host = match pool.remote_engine() {
        None => {
            println!(
                "Pool {name} runs on this machine's Docker, so it has no other machine to check."
            );
            return Ok(0);
        }
        Some(purlis_core::runners::RemoteEngine::Ssh(remote)) => remote.route.host().to_owned(),
        Some(purlis_core::runners::RemoteEngine::Tls(remote)) => remote.route.host().to_owned(),
    };
    let docker = poolsetup::docker_here()
        .ok_or_else(|| "purlis found no docker command on this machine.".to_owned())?;
    let reached = purlis_core::sshroute::pool::docker_of(&docker, pool);
    let (said, code) =
        poolsetup::pinchange::doctor_said(name, reached.as_ref().map(|_| host.as_str()));
    print!("{said}");
    Ok(code)
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
        // The machine, as Connect shows it: the key to check, or the one trusted.
        if let Some(machine) = &step.machine {
            if let Some(key) = &machine.key {
                let _ = writeln!(
                    out,
                    "             shows {} ({})",
                    key.fingerprint, key.algorithm
                );
            }
            if let Some(key) = &machine.trusted {
                let _ = writeln!(
                    out,
                    "             trusted {} ({})",
                    key.fingerprint, key.algorithm
                );
            }
            if let Some(key) = &machine.now {
                let _ = writeln!(
                    out,
                    "             shows now {} ({})",
                    key.fingerprint, key.algorithm
                );
            }
        }
        for check in &step.checks {
            let word = serde_json::to_value(check.verdict)
                .ok()
                .and_then(|word| word.as_str().map(str::to_owned))
                .unwrap_or_default();
            let _ = writeln!(out, "             {word:<8} {}", check.title);
            // Doctor's next step, beside its line, as the window shows it (#4492).
            if let (purlis_core::poolsetup::CheckOf::Cluster, Some(more)) =
                (check.about, &check.more)
            {
                let _ = writeln!(out, "                      {more}");
            }
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
    let other = setup
        .at
        .and_then(|at| setup.step(at))
        .and_then(|step| step.other.as_ref());
    if let Some(other) = other {
        let _ = writeln!(out, "Or: {}. {}", other.label, other.does);
    }
    // Send to your admin's words (#4496), as the window gives them: held free of secrets.
    if let Some(words) = setup
        .at
        .and_then(|at| setup.step(at))
        .and_then(|step| step.admin.as_ref())
    {
        let _ = writeln!(
            out,
            "For your admin, to send by chat or email. It holds no password, token, key or credential:"
        );
        for line in words.lines() {
            let _ = writeln!(out, "  {line}");
        }
    }
    out
}
