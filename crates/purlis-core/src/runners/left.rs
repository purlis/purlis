// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The runners left behind on another Docker engine** (#2587): `<config>/runners-left.json`.
//!
//! A runner whose removal finds Docker answering as another engine than the one that made it
//! is forgotten in `runners.json`, so a pool whose engine is gone for good can still be removed
//! ([`super::remove`], #4325), and its container and home volume stay on the engine that made
//! them. This keeps each such runner, so `purlis pool left-behind` can list it and remove it once
//! Docker answers as that engine again ([`remove`]): `{"version": 1, "runners": [{"name",
//! "pool", "container", "image", "user", "home", "engine", "since"}]}`, each key as
//! `runners.json` spells it, `engine` as `docker info` printed it, `since` in seconds since the
//! epoch. No device id or link key: a runner left behind is never linked to again.
//!
//! - **Written under `runners.json.lock`**, before the runner's row goes, so a runner is in one
//!   file or the other: where this cannot be written, the runner stays kept and the removal can
//!   be asked again.
//! - **A record that cannot be read is never written over**: it is refused, and a removal that
//!   would add to it keeps the runner instead.
//! - `0600` in the config home's own folder, as `runners.json` is.

use std::path::Path;

use crate::dockerpool::{ContainerId, Docker, Image, Made, Runner, User};

use super::{Kept, Locked, Refused, lock, now, usable_noted, usable_pool_name};

/// The file, in the machine store's folder.
pub const FILE: &str = "runners-left.json";

/// **A runner left behind on another engine**: what it was made as, its pool, the engine that
/// made it and holds it, and since when.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Left {
    /// Its container and its runner, whose home volume is [`Runner::volume`].
    pub made: Made,
    /// The pool it was a runner of.
    pub pool: String,
    /// The engine that made it, as `docker info` printed its id.
    pub engine: String,
    /// When it was left behind, in seconds since the epoch.
    pub since: u64,
}

impl Left {
    /// The runner's name.
    pub fn name(&self) -> &str {
        self.made.runner.name()
    }

    /// Its home volume's name.
    pub fn volume(&self) -> String {
        self.made.runner.volume()
    }

    /// The commands that remove it where Docker answers as its engine.
    pub fn how(&self) -> String {
        format!(
            "docker container rm --force {}; docker volume rm --force {}",
            self.made.container.as_str(),
            self.volume()
        )
    }
}

/// **The runners left behind on another engine**, kept in `config_root`, oldest first: none
/// where there is no file; refused where it cannot be read safely or is not as this module
/// writes it.
pub fn read(config_root: &Path) -> Result<Vec<Left>, Refused> {
    let path = crate::machine::dir(config_root).join(FILE);
    let text = match purlis_same_user::read_private_file(&path) {
        Ok(text) => text,
        Err(why) if why.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(why) => {
            return Err(Refused::Other(format!(
                "the runners left on another Docker engine could not be read safely ({}): {why}",
                path.display()
            )));
        }
    };
    let unreadable = || {
        Refused::Other(format!(
            "{} is not as purlis writes it, so purlis leaves it as it is and keeps no more \
             runners in it",
            path.display()
        ))
    };
    let value: serde_json::Value = serde_json::from_str(&text).map_err(|_| unreadable())?;
    let fields = value.as_object().ok_or_else(unreadable)?;
    if fields.get("version").and_then(serde_json::Value::as_u64) != Some(1)
        || fields
            .keys()
            .any(|key| !matches!(key.as_str(), "version" | "runners"))
    {
        return Err(unreadable());
    }
    fields
        .get("runners")
        .and_then(serde_json::Value::as_array)
        .ok_or_else(unreadable)?
        .iter()
        .map(|row| from_row(row).ok_or_else(unreadable))
        .collect()
}

/// **Keeps `kept` as left behind on the engine that made it**, under `_locked`, before its row
/// in `runners.json` goes: refused, with nothing written, where no engine was noted of it or
/// the record cannot be read.
pub(super) fn keep_locked(
    config_root: &Path,
    kept: &Kept,
    _locked: &Locked,
) -> Result<(), Refused> {
    let Some(engine) = kept.engine.clone() else {
        return Err(Refused::Other(format!(
            "runner {} names no engine that made it, so purlis cannot say where it was left",
            kept.name()
        )));
    };
    let mut left = read(config_root)?;
    left.retain(|one| one.made.container != kept.made.container);
    left.push(Left {
        made: kept.made.clone(),
        pool: kept.pool().to_owned(),
        engine,
        since: now(),
    });
    write(config_root, &left)
}

/// What [`remove`] did with one runner left behind: `Ok` where its container and home volume
/// are gone and it is forgotten, otherwise what is left, in words.
pub type Outcome = (Left, Result<(), String>);

/// **Removes each runner left behind whose engine `docker` answers as now**, its container and
/// home volume, and forgets it once both are gone and Docker still answers as that engine. One
/// on an engine Docker does not answer as is kept, and said; nothing is removed by name on
/// another engine.
pub fn remove(config_root: &Path, docker: &Docker) -> Result<Vec<Outcome>, Refused> {
    let _locked = lock(config_root);
    let left = read(config_root)?;
    if left.is_empty() {
        return Ok(Vec::new());
    }
    let mut outcomes = Vec::with_capacity(left.len());
    let mut kept = Vec::new();
    for one in left {
        let done = removed(&one, docker);
        if done.is_err() {
            kept.push(one.clone());
        }
        outcomes.push((one, done));
    }
    if kept.len() != outcomes.len() {
        write(config_root, &kept)?;
    }
    Ok(outcomes)
}

/// Removes `one` through `docker` where Docker answers as its engine, reading which engine it
/// is again before it acts and before it is forgotten.
fn removed(one: &Left, docker: &Docker) -> Result<(), String> {
    answers_as(one, docker)?;
    let (container, volume) = docker.remove_each(&one.made);
    let said: Vec<String> = [container, volume]
        .into_iter()
        .filter_map(|part| part.err().map(|why| why.to_string()))
        .collect();
    if !said.is_empty() {
        return Err(said.join("; "));
    }
    answers_as(one, docker).map_err(|why| {
        format!(
            "{why}, so purlis keeps it in case its container and home volume are still there: \
             {}",
            one.how()
        )
    })
}

/// `Ok` where `docker` answers as the engine that made `one`; otherwise why not, in words.
fn answers_as(one: &Left, docker: &Docker) -> Result<(), String> {
    let (name, engine) = (one.name(), one.engine.as_str());
    match docker.engine_id() {
        Ok(Some(now)) if now == engine => Ok(()),
        Ok(Some(now)) => Err(format!(
            "runner {name} is on Docker engine {engine}, and Docker answers as engine {now}. \
             Point Docker at engine {engine} and run this again, or remove it there yourself: {}",
            one.how()
        )),
        Ok(None) => Err(format!(
            "runner {name} is on Docker engine {engine}, and this Docker does not say which \
             engine it is"
        )),
        Err(why) => Err(format!(
            "runner {name} is on Docker engine {engine}, and purlis could not read which engine \
             Docker is ({why})"
        )),
    }
}

/// Writes `left` whole.
fn write(config_root: &Path, left: &[Left]) -> Result<(), Refused> {
    let rows: Vec<serde_json::Value> = left.iter().map(to_row).collect();
    let text = serde_json::json!({ "version": 1, "runners": rows });
    purlis_same_user::PrivateDirectory::open(&crate::machine::dir(config_root))
        .and_then(|folder| folder.replace_file(FILE, format!("{text:#}\n").as_bytes()))
        .map_err(|why| {
            Refused::Other(format!(
                "the runners left on another Docker engine could not be written: {why}"
            ))
        })
}

fn to_row(left: &Left) -> serde_json::Value {
    let runner = &left.made.runner;
    serde_json::json!({
        "name": runner.name(),
        "pool": left.pool,
        "container": left.made.container.as_str(),
        "image": runner.image().as_str(),
        "user": runner.user().spelled(),
        "home": runner.home(),
        "engine": left.engine,
        "since": left.since,
    })
}

fn from_row(row: &serde_json::Value) -> Option<Left> {
    let fields = row.as_object()?;
    let known = [
        "name",
        "pool",
        "container",
        "image",
        "user",
        "home",
        "engine",
        "since",
    ];
    if fields.len() != known.len() || fields.keys().any(|key| !known.contains(&key.as_str())) {
        return None;
    }
    let text = |key: &str| fields.get(key).and_then(serde_json::Value::as_str);
    let pool = text("pool")?;
    usable_pool_name(pool).ok()?;
    let engine = text("engine").filter(|engine| usable_noted(engine))?;
    let (uid, gid) = text("user")?.split_once(':')?;
    let user = User::new(uid.parse().ok()?, gid.parse().ok()?).ok()?;
    let runner = Runner::new(
        text("name")?,
        Image::pinned(text("image")?).ok()?,
        user,
        text("home")?,
    )
    .ok()?;
    Some(Left {
        made: Made {
            container: ContainerId::new(text("container")?).ok()?,
            runner,
        },
        pool: pool.to_owned(),
        engine: engine.to_owned(),
        since: fields.get("since")?.as_u64()?,
    })
}
