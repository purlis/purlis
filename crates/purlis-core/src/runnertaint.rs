// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A runner a chat ran on without the sandbox starts no browser and is filled no login** (#3298,
//! D-2050-27, the dispatcher's decision of 2026-10-28).
//!
//! A chat with no sandbox runs as the runner's one user, beside its host and the browser engine,
//! so it can change what that runner later runs for a browser. Once one has run on a runner,
//! this desktop keeps that runner's container in `<config>/runners-unsandboxed.json`, which no
//! runner reaches, and the window starts no browser on it and fills it no login until it is made
//! again: a runner made again is a new container. Kept as `{"version": 1, "containers":
//! ["<64 lower-case hex>", …]}`, `0600`, at most [`MOST`] containers, the oldest let go first,
//! and only those of runners this machine still keeps.
//!
//! **It fails closed**: a file that cannot be read safely, or is not as this module writes it,
//! counts every runner as one a chat ran on without the sandbox.

use std::path::Path;

use crate::dockerpool::ContainerId;

/// The file, in the machine store's folder.
pub const FILE: &str = "runners-unsandboxed.json";

/// The most containers it keeps.
pub const MOST: usize = 256;

/// **What the window says where a runner a chat ran on without the sandbox is asked for a
/// browser or a login**: the pool by its name.
pub fn refused_on(pool: &str) -> String {
    format!(
        "no browser starts and no login is filled on pool {pool}'s runner: a chat ran there \
         without the sandbox, beside what runs a browser, so purlis no longer trusts that runner \
         with one. Remove the pool and add it again to make a new runner."
    )
}

/// Whether a program of `kind` that its host says runs `sandboxed` makes its runner one a chat
/// ran on without the sandbox: any program but a person's shell or a browser, unless it runs in
/// a sandbox that holds its files (`landlock`, `bubblewrap`, `seatbelt`). A host that does not
/// say counts as none (a host before 1.29).
pub fn taints(
    kind: purlis_session_protocol::session::ProgramKind,
    sandboxed: Option<&purlis_session_protocol::session::RunSandbox>,
) -> bool {
    use purlis_session_protocol::session::{ProgramKind, RunSandbox};
    if matches!(kind, ProgramKind::Shell | ProgramKind::Browser) {
        return false;
    }
    !matches!(
        sandboxed,
        Some(RunSandbox::By(backend))
            if matches!(backend.as_str(), "landlock" | "bubblewrap" | "seatbelt")
    )
}

/// The containers kept, or `None` where the file is there and cannot be read as this module
/// writes it.
fn read(config_root: &Path) -> Option<Vec<String>> {
    let path = crate::machine::dir(config_root).join(FILE);
    let text = match purlis_same_user::read_private_file(&path) {
        Ok(text) => text,
        Err(why) if why.kind() == std::io::ErrorKind::NotFound => return Some(Vec::new()),
        Err(_) => return None,
    };
    let value: serde_json::Value = serde_json::from_str(&text).ok()?;
    let fields = value.as_object()?;
    if fields.get("version").and_then(serde_json::Value::as_u64) != Some(1)
        || fields
            .keys()
            .any(|key| !matches!(key.as_str(), "version" | "containers"))
    {
        return None;
    }
    fields
        .get("containers")?
        .as_array()?
        .iter()
        .map(|id| {
            let id = id.as_str()?;
            ContainerId::new(id).ok().map(|_| id.to_owned())
        })
        .collect()
}

/// **Whether a chat ran without the sandbox on the runner whose container is `container`**, as
/// this machine keeps it in `config_root`: yes too where the file cannot be read.
pub fn ran_unsandboxed(config_root: &Path, container: &ContainerId) -> bool {
    read(config_root).is_none_or(|kept| kept.iter().any(|id| id == container.as_str()))
}

/// **Keeps that a chat ran without the sandbox on the runner whose container is `container`**,
/// with those kept before of runners this machine still keeps, at most [`MOST`].
pub fn mark(config_root: &Path, container: &ContainerId) -> Result<(), String> {
    let _locked = crate::runners::lock(config_root);
    let still: Vec<String> = crate::runners::read(config_root)
        .map(|kept| {
            kept.iter()
                .map(|runner| runner.made.container.as_str().to_owned())
                .collect()
        })
        .unwrap_or_default();
    let mut kept: Vec<String> = read(config_root)
        .unwrap_or_default()
        .into_iter()
        .filter(|id| still.contains(id) && id != container.as_str())
        .collect();
    kept.push(container.as_str().to_owned());
    let over = kept.len().saturating_sub(MOST);
    kept.drain(..over);
    let text = serde_json::json!({ "version": 1, "containers": kept });
    purlis_same_user::PrivateDirectory::open(&crate::machine::dir(config_root))
        .and_then(|folder| folder.replace_file(FILE, format!("{text:#}\n").as_bytes()))
        .map_err(|why| format!("purlis could not keep that a chat ran without the sandbox: {why}"))
}

/// **Keeps that a chat ran without the sandbox on pool `pool`'s runner**, as this machine keeps
/// its runners in `config_root`.
pub fn mark_pool(config_root: &Path, pool: &str) -> Result<(), String> {
    let runner = crate::runners::runner_for(config_root, pool).map_err(|why| why.to_string())?;
    mark(config_root, &runner.made.container)
}

/// **Whether a chat ran without the sandbox on pool `pool`'s runner**, as this machine keeps its
/// runners in `config_root`: yes where the record cannot be read. A pool with no runner kept
/// there has none to start a browser on, so it is said no and its start fails as such a start
/// does.
pub fn pool_ran_unsandboxed(config_root: &Path, pool: &str) -> bool {
    crate::runners::runner_for(config_root, pool)
        .is_ok_and(|runner| ran_unsandboxed(config_root, &runner.made.container))
}

#[cfg(test)]
mod tests;
