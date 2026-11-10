// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **An extension's artifact templates** (ADR 0122 §9, ADR 0053 as amended 2026-11-09, #5973):
//! the `artifacts` capability. An extension ships page templates in its own folder, and a
//! person may pin one to a workspace like any other page.
//!
//! # Declared
//!
//! `contributes.artifacts` lists each template with its `name` (one id segment; the template's
//! id is `<extension-id>.<name>`), a `title` purlis draws, the `folder` inside the extension
//! that holds its `index.html`, and an optional `icon` from purlis's icon set. The folder is
//! inside the extension's directory, so the extension's fingerprint covers every byte of the
//! template, and the approval prompt names each template ([`declares`]).
//!
//! # Pinned
//!
//! [`pin`] **copies the template into the workspace's `artifacts/<extension-id>.<name>/` and
//! pins that folder through [`crate::artifact::pin`]**, so an extension's page meets the same
//! refusals as any pin: its folder is read within the size cap, scanned for anything shaped like
//! a secret before a byte is copied, and never written over a folder already there. From then
//! on it is the workspace's page: its template travels with the workspace, its script is
//! approved by each person on their own machine over the copy's fingerprint (ADR 0122 §4), and
//! a teammate without the extension sees it with `needs-extension` where its queries' data would
//! be. A template's data files are never copied: data is each person's own.
//!
//! Only an extension installed, approved, unchanged since its approval and on in the project
//! and workspace offers a template ([`offered`]). **On a platform where purlis starts no
//! extension's program** (Windows, ADR 0031) none does, and a pin from one is refused.

use std::path::{Path, PathBuf};

use serde_json::Value;

use super::facts;
use super::project::Choices;
use super::{BuiltIn, Capability, Extension, Manifest, Standing};
use crate::artifact::{self, Pinned, Request};

/// The most templates one extension may declare: the bar draws eight artifact icons.
const MOST_TEMPLATES: usize = 8;

/// The keys a declared template may carry.
const KEYS: [&str; 4] = ["name", "title", "folder", "icon"];

/// One template an extension declares.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Declared {
    /// The id after `<extension-id>.`.
    pub name: String,
    /// What the pin is called unless the person names it.
    pub title: String,
    /// The folder holding its `index.html`, relative to the extension's directory.
    pub folder: String,
    /// One of purlis's icons; none is the pin's default.
    pub icon: Option<String>,
}

/// The templates `contributes.artifacts` declares, or why purlis will not read them.
pub(super) fn declared_of(value: &Value) -> Result<Vec<Declared>, String> {
    let list = facts::entries(value, "artifacts", MOST_TEMPLATES)?;
    let mut out: Vec<Declared> = Vec::with_capacity(list.len());
    for (at, object) in list.iter().enumerate() {
        facts::only(object, &KEYS, "an artifact template", at)?;
        let name = object
            .get("name")
            .and_then(Value::as_str)
            .ok_or_else(|| format!("declares an artifact template at {at} with no name"))?;
        if !facts::id_ok(name) {
            return Err(format!(
                "declares the artifact template {}, and a template's name is letters, digits, \
                 '-' and '_', starting with a letter or a digit",
                crate::shown::readable(name, 64)
            ));
        }
        let name = name.to_owned();
        if out.iter().any(|it| it.name == name) {
            return Err(format!("declares two artifact templates called {name:?}"));
        }
        let title = object
            .get("title")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|title| !title.is_empty())
            .ok_or_else(|| format!("declares the artifact template {name:?} with no title"))?;
        if !facts::drawable_label(title) {
            return Err(format!(
                "declares the artifact template {name:?} with a title purlis will not draw: it \
                 is longer than {} bytes or holds a control or invisible formatting character",
                facts::MOST_LABEL_BYTES
            ));
        }
        let folder = object.get("folder").and_then(Value::as_str).unwrap_or("");
        super::declarable(folder).map_err(|why| {
            format!(
                "declares the artifact template {name:?} in the folder {}, which {why}",
                crate::shown::readable(folder, 64)
            )
        })?;
        let icon = match object.get("icon") {
            None => None,
            Some(icon) => Some(
                icon.as_str()
                    .filter(|icon| crate::personamark::ICONS.contains(icon))
                    .ok_or_else(|| {
                        format!(
                            "declares the artifact template {name:?} with an icon that is not \
                             one of purlis's icons; it is one of: {}",
                            crate::personamark::ICONS.join(", ")
                        )
                    })?
                    .to_owned(),
            ),
        };
        out.push(Declared {
            name,
            title: title.to_owned(),
            folder: folder.to_owned(),
            icon,
        });
    }
    Ok(out)
}

/// The approval prompt's line for each template, in purlis's words.
pub(super) fn declares(manifest: &Manifest) -> Vec<String> {
    manifest
        .artifacts
        .iter()
        .map(|template| {
            format!(
                "an artifact template, “{}.{}” — “{}”, from its folder {}",
                manifest.id, template.name, template.title, template.folder
            )
        })
        .collect()
}

/// `<extension-id>.<name>`, split: the name holds no dot, the extension's id may.
fn split(id: &str) -> Option<(&str, &str)> {
    let (extension, name) = id.rsplit_once('.')?;
    (extension != crate::catalogue::SOURCE && super::an_id(extension) && facts::id_ok(name))
        .then_some((extension, name))
}

/// One template this machine's extensions offer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Offered {
    /// `<extension-id>.<name>`: what `purlis artifact pin --template` takes, and the folder it
    /// is copied to.
    pub id: String,
    /// The extension's name, as the Extensions list draws it.
    pub extension: String,
    pub title: String,
    pub icon: String,
}

/// **The templates offered here**: every one declared by an extension installed, approved,
/// unchanged and on in `choices`' project and workspace. None where purlis starts no
/// extension's program.
pub fn offered(config_root: &Path, built_in: &BuiltIn, choices: &Choices) -> Vec<Offered> {
    if !crate::executor::RUNS_PROGRAMS {
        return Vec::new();
    }
    let loaded = super::read(config_root, built_in);
    let mut out = Vec::new();
    for (id, entry) in &loaded.registry.entries {
        if !entry.in_force() {
            continue;
        }
        let Ok(found) = usable(&loaded, id, &entry.path, choices) else {
            continue;
        };
        for template in &found.manifest.artifacts {
            out.push(Offered {
                id: format!("{id}.{}", template.name),
                extension: found.manifest.name.clone(),
                title: template.title.clone(),
                icon: template
                    .icon
                    .clone()
                    .unwrap_or_else(|| artifact::DEFAULT_ICON.to_owned()),
            });
        }
    }
    out
}

/// The extension at `path`, under `id`, if it may offer templates here: declaring the
/// capability, on here, approved and unchanged since.
fn usable(
    loaded: &super::Loaded,
    id: &str,
    path: &Path,
    choices: &Choices,
) -> Result<Extension, String> {
    let found = super::read_at(path).map_err(|why| format!("'{id}' cannot be read: it {why}"))?;
    if found.id() != id || loaded.standing(&found) != Standing::Approved {
        return Err(format!(
            "'{id}' is not approved, or changed since you approved it: approve it in the \
             Extensions list first"
        ));
    }
    if !found.manifest.capabilities.contains(&Capability::Artifacts) {
        return Err(format!("'{id}' offers no artifact template"));
    }
    if !facts::on_here(id, &found.manifest, choices) {
        return Err(format!(
            "'{id}' is turned off in this project or workspace, so it offers no template here"
        ));
    }
    Ok(found)
}

/// What a person pins: a template's id, and the name and icon they chose, if any.
pub struct Wanted<'a> {
    pub id: &'a str,
    pub name: Option<String>,
    pub icon: Option<String>,
}

/// What [`pin`] did: the folder the template was copied to, and the pin.
#[derive(Debug)]
pub struct FromExtension {
    pub folder: String,
    pub pinned: Pinned,
}

/// **Pin extension template `wanted.id` to the workspace at `workspace`** in the plane at
/// `root`. See the module docstring. A refusal leaves nothing behind.
pub fn pin(
    config_root: &Path,
    built_in: &BuiltIn,
    choices: &Choices,
    root: &Path,
    workspace: &Path,
    wanted: &Wanted<'_>,
) -> Result<FromExtension, String> {
    pin_where(
        crate::executor::RUNS_PROGRAMS,
        config_root,
        built_in,
        choices,
        root,
        workspace,
        wanted,
    )
}

/// [`pin`], where `runs` says whether purlis starts extensions' programs on this platform.
fn pin_where(
    runs: bool,
    config_root: &Path,
    built_in: &BuiltIn,
    choices: &Choices,
    root: &Path,
    workspace: &Path,
    wanted: &Wanted<'_>,
) -> Result<FromExtension, String> {
    if !runs {
        return Err(
            "an extension's artifact templates are not on this platform: no extension is \
             started here, so none of their pages is pinned (ADR 0031)"
                .into(),
        );
    }
    let shown = crate::personas::one_line(wanted.id);
    let (extension, name) = split(wanted.id).ok_or_else(|| {
        format!(
            "'{shown}' is not an extension's template: name it <extension-id>.<name> \
             (`purlis artifact templates` lists them)"
        )
    })?;
    let loaded = super::read(config_root, built_in);
    let entry = loaded
        .registry
        .entries
        .get(extension)
        .filter(|entry| entry.on)
        .ok_or_else(|| format!("no extension '{extension}' is installed and on here"))?;
    let found = usable(&loaded, extension, &entry.path, choices)?;
    let template = found
        .manifest
        .artifacts
        .iter()
        .find(|it| it.name == name)
        .ok_or_else(|| format!("'{extension}' offers no artifact template '{name}'"))?;
    let source = template_folder(&found.path, &template.folder, &shown)?;
    let snapshot = artifact::serve::Snapshot::read(&source, wanted.id)?;
    artifact::scanned(&source, wanted.id)?;
    copy_in(root, workspace, wanted.id, &snapshot)?;
    let request = Request {
        folder: wanted.id.to_owned(),
        name: Some(
            wanted
                .name
                .clone()
                .unwrap_or_else(|| template.title.clone()),
        ),
        icon: wanted.icon.clone().or_else(|| template.icon.clone()),
        command: None,
        cwd: None,
    };
    match artifact::pin(root, workspace, &request) {
        Ok(pinned) => Ok(FromExtension {
            folder: wanted.id.to_owned(),
            pinned,
        }),
        Err(why) => {
            let _ = std::fs::remove_dir_all(workspace.join(artifact::DIR).join(wanted.id));
            Err(why)
        }
    }
}

/// Where the template's folder is, every link followed, or why it is not one inside the
/// extension.
fn template_folder(extension: &Path, folder: &str, shown: &str) -> Result<PathBuf, String> {
    let home = extension
        .canonicalize()
        .map_err(|_| format!("the extension of '{shown}' cannot be read"))?;
    let real = extension
        .join(folder)
        .canonicalize()
        .map_err(|_| format!("the extension has no folder {folder} for '{shown}'"))?;
    if !real.starts_with(&home) || real == home || !real.is_dir() {
        return Err(format!(
            "the folder of '{shown}' is not a folder inside its extension, so it is not pinned"
        ));
    }
    Ok(real)
}

/// Write the template's files, not its data, into a new `artifacts/<folder>/` of the workspace.
/// Never over a folder that is there; a failure part way removes what it wrote.
fn copy_in(
    root: &Path,
    workspace: &Path,
    folder: &str,
    snapshot: &artifact::serve::Snapshot,
) -> Result<(), String> {
    let dir = workspace.join(artifact::DIR);
    std::fs::create_dir_all(&dir).map_err(|e| {
        format!(
            "purlis could not make {}/ in the workspace ({})",
            artifact::DIR,
            crate::rewrite::os_words(&e)
        )
    })?;
    let at = dir.join(folder);
    match std::fs::create_dir(&at) {
        Ok(()) => {}
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
            return Err(format!(
                "there is already a folder '{folder}' in {}/, so the template is not copied \
                 over it. Unpin it and remove the folder to take the extension's template again",
                artifact::DIR
            ));
        }
        Err(e) => {
            return Err(format!(
                "purlis could not make '{folder}' in {}/ ({})",
                artifact::DIR,
                crate::rewrite::os_words(&e)
            ));
        }
    }
    let written = (|| {
        for path in snapshot
            .paths()
            .filter(|path| !artifact::serve::is_data(path))
        {
            let target = at.join(path);
            if let Some(parent) = target.parent() {
                std::fs::create_dir_all(parent)?;
            }
            crate::rewrite::replace(
                root,
                &target,
                snapshot.get(path).unwrap_or_default(),
                crate::rewrite::Mode::Kept,
            )?;
        }
        Ok::<(), std::io::Error>(())
    })();
    written.map_err(|e| {
        let _ = std::fs::remove_dir_all(&at);
        format!(
            "purlis could not copy the template into '{folder}' ({})",
            crate::rewrite::os_words(&e)
        )
    })
}

#[cfg(test)]
mod tests;
