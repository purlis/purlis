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
//! ["<64 lower-case hex>", …]}`, `0600`, only those of runners this machine still keeps: a mark
//! is let go of only once its runner is (#3702). A cluster runner has no container: it is kept
//! as the SHA-256 of its claim's uid, in the same form ([`cluster_runner_id`], #4071), so a claim
//! made again is a new runner.
//!
//! **It fails closed**: a file that cannot be read safely, or is not as this module writes it,
//! counts every runner as one a chat ran on without the sandbox.

use std::path::Path;

use crate::dockerpool::ContainerId;

/// The file, in the machine store's folder.
pub const FILE: &str = "runners-unsandboxed.json";

/// **What the window says where a runner a chat ran on without the sandbox is asked for a
/// browser or a login**: the pool by its name.
pub fn refused_on(pool: &str) -> String {
    format!(
        "no browser starts and no login is filled on pool {pool}'s runner: a chat ran there \
         without the sandbox, beside what runs a browser, or may have (its start was not \
         answered, or the window could not say how a chat it took back there runs), so purlis \
         no longer trusts that runner with one. Remove the pool and add it again to make a new \
         runner."
    )
}

/// **What a chat on a marked pool's runner is told once a chat ran there without the sandbox**
/// (#4071): the mark stands, and such a runner is sent no value or token either.
pub fn sent_nothing_on(pool: &str) -> String {
    format!(
        "purlis sends pool {pool}'s runner no vault value and no token: a chat ran there without \
         the sandbox, or may have. Remove the pool and add it again to make a new runner."
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
/// with those kept before of runners this machine still keeps.
///
/// Refused, and nothing written, where either record cannot be read: an unreadable record of
/// runners says nothing of which to let go of, and one of these is never written over, so it
/// stays one that counts every runner (#3702).
pub fn mark(config_root: &Path, container: &ContainerId) -> Result<(), String> {
    let _locked = crate::runners::lock(config_root);
    let still = kept_runners(config_root)?;
    keep(config_root, &still, &[container.as_str().to_owned()])
}

/// **Keeps that a chat ran without the sandbox on pool `pool`'s runner**, as this machine keeps
/// its runners in `config_root`: every runner kept for the pool, the ones still draining after
/// an update too, since a link to the pool reaches whichever of them holds its chat and the
/// window is not told which (#3702). Refused where no runner of the pool is kept, or where
/// either record cannot be read.
pub fn mark_pool(config_root: &Path, pool: &str) -> Result<(), String> {
    let _locked = crate::runners::lock(config_root);
    let still = kept_runners(config_root)?;
    let of_pool: Vec<String> = still
        .iter()
        .filter(|(of, _)| of == pool)
        .map(|(_, id)| id.clone())
        .collect();
    if of_pool.is_empty() {
        return Err(format!(
            "purlis could not keep that a chat ran without the sandbox: pool {pool} has no \
             runner kept on this machine"
        ));
    }
    keep(config_root, &still, &of_pool)
}

/// What a cluster runner is kept as here: the SHA-256 of its claim's uid, in hex (#4071).
fn cluster_runner_id(claim_uid: &str) -> String {
    use sha2::Digest as _;
    crate::runnerlink::hex_of(&sha2::Sha256::digest(claim_uid.as_bytes()))
}

/// The pool and id of every runner kept in `config_root`, of either kind, or why they cannot be
/// read.
fn kept_runners(config_root: &Path) -> Result<Vec<(String, String)>, String> {
    let said = |why: crate::runners::Refused| {
        format!("purlis could not keep that a chat ran without the sandbox: {why}")
    };
    let docker = crate::runners::read(config_root).map_err(said)?;
    let cluster = crate::runners::cluster_runners(config_root).map_err(said)?;
    Ok(docker
        .iter()
        .map(|runner| {
            (
                runner.pool().to_owned(),
                runner.made.container.as_str().to_owned(),
            )
        })
        .chain(
            cluster
                .iter()
                .map(|kept| (kept.name.clone(), cluster_runner_id(&kept.claimed.uid))),
        )
        .collect())
}

/// Writes `marked` beside those kept before of the runners in `still`, under the runners' lock.
fn keep(config_root: &Path, still: &[(String, String)], marked: &[String]) -> Result<(), String> {
    let Some(before) = read(config_root) else {
        return Err(format!(
            "purlis could not keep that a chat ran without the sandbox: {} is not as purlis \
             writes it, so it is left as it is and counts every runner as one",
            crate::machine::dir(config_root).join(FILE).display()
        ));
    };
    let mut kept: Vec<String> = before
        .into_iter()
        .filter(|id| still.iter().any(|(_, kept)| kept == id) && !marked.contains(id))
        .collect();
    kept.extend(marked.iter().cloned());
    let text = serde_json::json!({ "version": 1, "containers": kept });
    purlis_same_user::PrivateDirectory::open(&crate::machine::dir(config_root))
        .and_then(|folder| folder.replace_file(FILE, format!("{text:#}\n").as_bytes()))
        .map_err(|why| format!("purlis could not keep that a chat ran without the sandbox: {why}"))
}

/// **Whether a chat ran without the sandbox on any runner pool `pool` keeps**, the ones still
/// draining after an update too, as this machine keeps its runners in `config_root`: yes where
/// either record cannot be read. A pool with no runner kept there has none to start a browser
/// on, so it is said no and its start fails as such a start does.
pub fn pool_ran_unsandboxed(config_root: &Path, pool: &str) -> bool {
    let Ok(kept) = kept_runners(config_root) else {
        return true;
    };
    let Some(marked) = read(config_root) else {
        return true;
    };
    kept.iter()
        .filter(|(of, _)| of == pool)
        .any(|(_, runner)| marked.contains(runner))
}

#[cfg(test)]
mod tests;
