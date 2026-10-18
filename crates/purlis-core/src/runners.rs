// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The runners this desktop pinned** (ADR 0078 §3 and §8, #2525): `<config>/runners.json`,
//! ruled 2026-10-18, `{"version": 1, "runners": [{"name", "pool", "container", "image",
//! "user", "home", "device", "key", "paired"}]}`.
//!
//! - **A runner's device id and public link key are kept once** ([`keep`]): a later answer for
//!   a runner already kept, with another device id or key, is refused and what was kept stays
//!   ([`Refused::KeyChanged`]), as is a device id or key another kept runner holds, or the
//!   desktop's own device id.
//! - **Removing a runner removes everything it is** ([`remove`]): the desktop from the
//!   runner's peers while the link is up, its container and home volume for a Docker pool,
//!   whatever state they are in, and its row. Whatever could not be removed is said with how to
//!   remove it, and the row stays until all of it is gone, so the removal can be asked again.
//! - **The file is this user's alone**, `0600` in the config home's own folder, and denied to
//!   chats. A file of another version or shape is refused rather than read or replaced.

use std::path::Path;

use purlis_session_protocol::hostlink::Pinned;

use crate::dockerpool::{ContainerId, Docker, Image, Made, Runner, User};
use crate::machine::{RUNNERS_FILE, RUNNERS_LOCK};

/// Why a runner was not kept, or the runners could not be read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Refused {
    /// A runner already kept answered with another device id or key: it is refused, and what
    /// was kept stays. The window says the runner's key changed and offers to remove it.
    KeyChanged { name: String },
    /// Anything else, in words a person reads.
    Other(String),
}

impl std::fmt::Display for Refused {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Refused::KeyChanged { name } => write!(
                f,
                "runner {name} answered with another key than the one kept for it, so purlis \
                 refused it and keeps the key it pinned; remove the runner if it was made again"
            ),
            Refused::Other(why) => f.write_str(why),
        }
    }
}

impl std::error::Error for Refused {}

/// One runner this desktop pinned: where it is, and who it is on a link.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Kept {
    /// The runner's container as it was made, which every check and link holds it to.
    pub made: Made,
    /// The runner's own device id and public link key, as its pairing answered them.
    pub pinned: Pinned,
    /// When it was paired, in seconds since the epoch.
    pub paired: u64,
}

impl Kept {
    pub fn name(&self) -> &str {
        self.made.runner.name()
    }
}

/// The runners kept under `config_root`, in the order they were kept: none where there is no
/// file; refused where it cannot be read safely or is not as [`keep`] writes it.
pub fn read(config_root: &Path) -> Result<Vec<Kept>, Refused> {
    let path = crate::machine::dir(config_root).join(RUNNERS_FILE);
    let text = match purlis_same_user::read_private_file(&path) {
        Ok(text) => text,
        Err(why) if why.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(why) => {
            return Err(Refused::Other(format!(
                "the runners this machine keeps could not be read safely ({}): {why}",
                path.display()
            )));
        }
    };
    let unreadable = || {
        Refused::Other(format!(
            "{} is not as purlis writes it, so no runner of it is used",
            path.display()
        ))
    };
    let value: serde_json::Value = serde_json::from_str(&text).map_err(|_| unreadable())?;
    if value.get("version").and_then(serde_json::Value::as_u64) != Some(1) {
        return Err(unreadable());
    }
    let rows = value
        .get("runners")
        .and_then(serde_json::Value::as_array)
        .ok_or_else(unreadable)?;
    rows.iter()
        .map(|row| from_row(row).ok_or_else(unreadable))
        .collect()
}

/// The runner kept as `name`, if one is.
pub fn named(config_root: &Path, name: &str) -> Result<Option<Kept>, Refused> {
    Ok(read(config_root)?
        .into_iter()
        .find(|kept| kept.name() == name))
}

/// Keep `answered`, what `made`'s runner's pairing answered, for `made`, on the desktop whose
/// own device id is `ours`: once.
///
/// A runner of this name kept already with the same container, device id and key is kept as
/// it was. One with another device id or key is [`Refused::KeyChanged`], and so is one of
/// another container: what was kept stays. A device id or key another kept runner holds, or
/// the desktop's own device id, is refused.
pub fn keep(
    config_root: &Path,
    made: &Made,
    answered: &Pinned,
    ours: &str,
) -> Result<Kept, Refused> {
    let _held = crate::machine::Lock::named(config_root, RUNNERS_LOCK);
    let mut kept = read(config_root)?;
    let name = made.runner.name();
    if let Some(there) = kept.iter().find(|kept| kept.name() == name) {
        return if &there.made == made && &there.pinned == answered {
            Ok(there.clone())
        } else {
            Err(Refused::KeyChanged {
                name: name.to_owned(),
            })
        };
    }
    if answered.device == ours {
        return Err(Refused::Other(format!(
            "runner {name} answered with this machine's own device id, so purlis did not keep \
             it; remove the runner and make it again"
        )));
    }
    if let Some(other) = kept
        .iter()
        .find(|kept| kept.pinned.device == answered.device || kept.pinned.key == answered.key)
    {
        return Err(Refused::Other(format!(
            "runner {name} answered with the device id or key of runner {}, so purlis did not \
             keep it; remove runner {name} and make it again",
            other.name()
        )));
    }
    let new = Kept {
        made: made.clone(),
        pinned: answered.clone(),
        paired: now(),
    };
    kept.push(new.clone());
    write(config_root, &kept)?;
    Ok(new)
}

/// What [`remove`] did, part by part: each `Err` is what is left, said with how to remove it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Removed {
    /// The desktop taken out of the runner's peers over the link: `None` where no link was up.
    pub peer: Option<Result<(), String>>,
    /// The runner's container.
    pub container: Result<(), String>,
    /// The runner's home volume.
    pub volume: Result<(), String>,
    /// Whether its row is gone: only once its container and volume both are.
    pub forgotten: bool,
}

impl Removed {
    /// What is left of the runner, and how to remove it; `None` when nothing is.
    pub fn left(&self) -> Option<String> {
        let left: Vec<&str> = [&self.container, &self.volume]
            .into_iter()
            .filter_map(|part| part.as_ref().err().map(String::as_str))
            .collect();
        if left.is_empty() {
            return None;
        }
        let mut said = left.join("; ");
        if !self.forgotten {
            said.push_str(
                "; purlis keeps the runner until they are gone, so it can be removed again",
            );
        }
        Some(said)
    }
}

/// Takes this desktop out of a kept runner's peers over its link (`unpair`, session protocol
/// 1.25), for [`remove`].
pub type Unpair<'a> = dyn Fn(&Kept) -> Result<(), String> + 'a;

/// Remove the runner kept as `name`: the desktop from its peers over the link when `unpair`
/// is given (the link is up), then its container and home volume with `docker`, whatever state
/// they are in, and its row once both are gone. A peer that could not be taken out goes with
/// the container: the peers file is in its home.
pub fn remove(
    config_root: &Path,
    docker: &Docker,
    name: &str,
    unpair: Option<&Unpair<'_>>,
) -> Result<Removed, Refused> {
    let kept = named(config_root, name)?
        .ok_or_else(|| Refused::Other(format!("no runner named {name} is kept on this machine")))?;
    let peer = unpair.map(|unpair| unpair(&kept));
    let (container, volume) = docker.remove_each(&kept.made);
    let forgotten = container.is_ok() && volume.is_ok();
    if forgotten {
        forget(config_root, name)?;
    }
    Ok(Removed {
        peer,
        container: container.map_err(|why| why.to_string()),
        volume: volume.map_err(|why| why.to_string()),
        forgotten,
    })
}

/// Forget the runner kept as `name`; nothing where none is.
pub fn forget(config_root: &Path, name: &str) -> Result<(), Refused> {
    let _held = crate::machine::Lock::named(config_root, RUNNERS_LOCK);
    let mut kept = read(config_root)?;
    let before = kept.len();
    kept.retain(|kept| kept.name() != name);
    if kept.len() != before {
        write(config_root, &kept)?;
    }
    Ok(())
}

fn write(config_root: &Path, kept: &[Kept]) -> Result<(), Refused> {
    let rows: Vec<serde_json::Value> = kept.iter().map(to_row).collect();
    let text = serde_json::json!({ "version": 1, "runners": rows });
    purlis_same_user::PrivateDirectory::open(&crate::machine::dir(config_root))
        .and_then(|folder| folder.replace_file(RUNNERS_FILE, format!("{text:#}\n").as_bytes()))
        .map_err(|why| {
            Refused::Other(format!(
                "the runners this machine keeps could not be written: {why}"
            ))
        })
}

fn to_row(kept: &Kept) -> serde_json::Value {
    let runner = &kept.made.runner;
    serde_json::json!({
        "name": runner.name(),
        "pool": "docker",
        "container": kept.made.container.as_str(),
        "image": runner.image().as_str(),
        "user": runner.user().spelled(),
        "home": runner.home(),
        "device": kept.pinned.device,
        "key": crate::runnerlink::hex_of(kept.pinned.key.as_bytes()),
        "paired": kept.paired,
    })
}

fn from_row(row: &serde_json::Value) -> Option<Kept> {
    let fields = row.as_object()?;
    if fields.len() != 9 {
        return None;
    }
    let text = |key: &str| fields.get(key).and_then(serde_json::Value::as_str);
    if text("pool")? != "docker" {
        return None;
    }
    let (uid, gid) = text("user")?.split_once(':')?;
    let user = User::new(uid.parse().ok()?, gid.parse().ok()?).ok()?;
    let runner = Runner::new(
        text("name")?,
        Image::pinned(text("image")?).ok()?,
        user,
        text("home")?,
    )
    .ok()?;
    let pinned = crate::runnerlink::from_line(
        &serde_json::json!({ "device": text("device")?, "key": text("key")? }).to_string(),
    )
    .ok()?;
    Some(Kept {
        made: Made {
            container: ContainerId::new(text("container")?).ok()?,
            runner,
        },
        pinned,
        paired: fields.get("paired")?.as_u64()?,
    })
}

fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|since| since.as_secs())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests;
