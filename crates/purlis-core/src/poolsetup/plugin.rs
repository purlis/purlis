// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A kubeconfig's credential plugin** (#7192, #7191): the command a context signs in with
//! (`users[].user.exec.command`, as `gke-gcloud-auth-plugin` is), where it is found, and, where
//! it isn't, a folder the person names.
//!
//! The plugin's command is read from the chosen context's `exec`, through kubectl, which leaves
//! every credential out. It is looked for in the known install folders ([`KNOWN`]), then in the
//! folder the person named. **Only the folder it is found in is added to a run's `PATH`**, never
//! the person's whole shell `PATH`, and the `exec`'s own `env` is left for kubectl to pass on, as
//! it does: nothing here reads or drops it.

use std::path::{Path, PathBuf};

use serde_json::Value;

/// The folders credential plugins are installed in, under the home (`~/`) or absolute: Homebrew,
/// `/usr/local/bin`, the gcloud SDK's, krew's.
pub const KNOWN: &[&str] = &[
    "/opt/homebrew/bin",
    "/usr/local/bin",
    "/usr/bin",
    "/opt/homebrew/share/google-cloud-sdk/bin",
    "/usr/local/Caskroom/google-cloud-sdk/latest/google-cloud-sdk/bin",
    "/usr/lib/google-cloud-sdk/bin",
    "~/google-cloud-sdk/bin",
    "~/.krew/bin",
];

/// What a context's `exec` asks for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Wanted {
    /// The plugin's command, as the kubeconfig names it: a bare name, or a path.
    pub command: String,
    /// The names of the variables its `exec` hands it. Never their values.
    pub env: Vec<String>,
}

/// Where the plugin is.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Where {
    /// In this folder, which a run's `PATH` then holds.
    In(PathBuf),
    /// In none of the known folders, nor the one named.
    Missing,
}

/// **The plugin `kubectl config view --minify --output json` shows for a context**, or none where
/// the context signs in with something else.
#[must_use]
pub fn wanted(view: &Value) -> Option<Wanted> {
    let exec = view.pointer("/users/0/user/exec")?;
    let command = exec.get("command")?.as_str()?.trim();
    if command.is_empty() || command.contains('\n') {
        return None;
    }
    let env = exec
        .get("env")
        .and_then(Value::as_array)
        .map(|vars| {
            vars.iter()
                .filter_map(|one| one.get("name").and_then(Value::as_str))
                .map(str::to_owned)
                .collect()
        })
        .unwrap_or_default();
    Some(Wanted {
        command: command.to_owned(),
        env,
    })
}

/// **Where `command` is**: an absolute command where it is, else the folder named, then the known
/// folders ([`KNOWN`], `~/` under `home`), then the absolute folders of this process's `PATH`.
#[must_use]
pub fn find(command: &str, named: Option<&Path>, home: Option<&Path>) -> Where {
    let as_path = Path::new(command);
    if as_path.is_absolute() {
        return match as_path.parent() {
            Some(folder) if crate::programs::runnable(as_path) => Where::In(folder.to_path_buf()),
            _ => Where::Missing,
        };
    }
    if command.contains('/') {
        return Where::Missing;
    }
    let known = KNOWN
        .iter()
        .filter_map(|folder| match folder.strip_prefix("~/") {
            Some(under) => home.map(|home| home.join(under)),
            None => Some(PathBuf::from(folder)),
        });
    // The folders of the person's own `PATH` come last (asdf, nix, mise, a custom SDK): a plugin
    // an existing setup already runs is found without naming its folder.
    let on_path = crate::envvar::var("PATH")
        .into_iter()
        .flat_map(|path| std::env::split_paths(&path).collect::<Vec<_>>());
    named
        .map(Path::to_path_buf)
        .into_iter()
        .chain(known)
        .chain(on_path)
        .find(|folder| folder.is_absolute() && crate::programs::runnable(&folder.join(command)))
        .map_or(Where::Missing, Where::In)
}

/// **The words for a plugin that is missing**, as Check the cluster says them.
#[must_use]
pub fn needs(command: &str) -> String {
    format!(
        "Needs {}, not found. Your kubeconfig signs in with it, and purlis looks in the usual \
         install folders only. Name the folder it is in.",
        crate::shown::short(command)
    )
}

#[cfg(test)]
mod tests;
