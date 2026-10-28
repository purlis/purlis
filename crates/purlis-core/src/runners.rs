// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The pools this desktop added, and the runners it pinned** (ADR 0078 §3 and §8, #2525,
//! #2679, #2749): `<config>/runners.json`, ruled 2026-10-18 and 2026-10-20, `{"version": 1,
//! "pools": [{"name", "kind", "image", "cpus", "memory", "processes", "tmp"}], "runners":
//! [{"name", "pool", "container", "image", "user", "home", "device", "key", "paired", "of"?,
//! "draining"?}]}`.
//!
//! - **A pool is named and defined only here** ([`Pool`]): a Docker pool's name, the runner
//!   image by digest, and the limits the person set, each left out where purlis's default
//!   holds ([`crate::dockerpool::Limits`]). A workspace names a pool (`machine.json`), and a name
//!   that is not kept here, or whose runner is not, starts no chat ([`runner_for`]).
//! - **A runner's device id and public link key are kept once** ([`keep`]): a later answer for
//!   a runner already kept, with another device id or key, is refused and what was kept stays
//!   ([`Refused::KeyChanged`]), as is a device id or key another kept runner holds, or the
//!   desktop's own device id or link key. Device ids are compared without regard to case: a
//!   device id is a ULID, which reads the same in either (#2741).
//! - **Removing a runner removes everything it is** ([`remove`]): the desktop from the
//!   runner's peers while the link is up, its container and home volume for a Docker pool,
//!   whatever state they are in, and its row. Whatever could not be removed is said with how to
//!   remove it, and the row stays until all of it is gone, so the removal can be asked again.
//! - **A pool's runner follows the desktop's version, side by side** (#2749, ruled 2026-10-20):
//!   a runner made beside the pool's after an update is kept `"of": "<pool>"`, the one it
//!   replaces is kept `"draining": true` and starts no new chat, and is removed once its last
//!   chat has ended ([`succeed_locked`], [`draining`]). A runner kept without `of` is the
//!   runner of the pool of its name.
//! - **The file is this user's alone**, `0600` in the config home's own folder, and denied to
//!   chats. A file of another version or shape is refused rather than read or replaced.

use std::path::Path;

use purlis_session_protocol::hostlink::Pinned;

use crate::dockerpool::{ContainerId, Docker, Image, Limits, Made, Runner, User};
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

/// The limits a person set on a pool, each `None` where purlis's default holds
/// ([`Limits::default`]): its CPUs in thousandths, its memory and its `/tmp` in MiB, and its
/// processes.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Set {
    pub millicpus: Option<u32>,
    pub memory_mib: Option<u64>,
    pub processes: Option<u32>,
    pub tmp_mib: Option<u64>,
}

/// One Docker pool on this machine, as the person added it (#2679): its name, which its one
/// runner is called by too, the runner image by digest, and the limits the person set.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pool {
    name: String,
    image: Image,
    set: Set,
    limits: Limits,
    price: Option<Price>,
}

/// **What one hour of a pool's runner costs, as the pool declares it** (#3491, the person's
/// ruling of 2026-10-28): `pricePerHour {amount, currency}`. The amount is kept as the decimal
/// it was written as, never as a binary float, and the currency is its ISO 4217 code; money is
/// shown in it and never converted.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Price {
    amount: String,
    currency: String,
}

impl Price {
    /// A price of `amount` (digits, then at most six after a point, with no leading zero but
    /// for `0` itself) in `currency` (three upper-case letters).
    pub fn new(amount: &str, currency: &str) -> Result<Price, Refused> {
        let digits = |text: &str| !text.is_empty() && text.bytes().all(|b| b.is_ascii_digit());
        let (whole, part) = match amount.split_once('.') {
            Some((whole, part)) => (whole, Some(part)),
            None => (amount, None),
        };
        let whole_ok =
            digits(whole) && whole.len() <= 9 && (whole == "0" || !whole.starts_with('0'));
        let part_ok = part.is_none_or(|part| digits(part) && part.len() <= 6);
        if !whole_ok || !part_ok {
            return Err(Refused::Other(format!(
                "\"{amount}\" is not a price per hour: write it as a decimal in a string, such as \
                 \"0.12\", with at most six digits after the point"
            )));
        }
        if currency.len() != 3 || !currency.bytes().all(|b| b.is_ascii_uppercase()) {
            return Err(Refused::Other(format!(
                "\"{currency}\" is not a currency: write its ISO 4217 code, such as \"EUR\""
            )));
        }
        Ok(Price {
            amount: amount.to_owned(),
            currency: currency.to_owned(),
        })
    }

    /// The amount, exactly as it was declared.
    pub fn amount(&self) -> &str {
        &self.amount
    }

    pub fn currency(&self) -> &str {
        &self.currency
    }
}

impl Pool {
    /// A pool called `name` (a runner's name: lower-case letters, digits and dashes, at most
    /// 40), of `image`, held to `set` over purlis's defaults, which together are limits a
    /// runner can start a chat within.
    pub fn new(name: &str, image: Image, set: Set) -> Result<Pool, Refused> {
        usable_pool_name(name).map_err(Refused::Other)?;
        let default = Limits::default();
        let limits = Limits::new(
            set.millicpus.unwrap_or(default.millicpus()),
            set.memory_mib.unwrap_or(default.memory_mib()),
            set.processes.unwrap_or(default.pids()),
            set.tmp_mib.unwrap_or(default.tmp_mib()),
        )
        .map_err(|why| Refused::Other(why.0))?;
        Ok(Pool {
            name: name.to_owned(),
            image,
            set,
            limits,
            price: None,
        })
    }

    /// This pool, declaring `price` per hour of its runner, or none (#3491).
    pub fn priced(self, price: Option<Price>) -> Pool {
        Pool { price, ..self }
    }

    /// The price per hour it declares, if any (#3491): only then is money shown for it.
    pub fn price(&self) -> Option<&Price> {
        self.price.as_ref()
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn image(&self) -> &Image {
        &self.image
    }

    /// The limits the person set, each `None` where the default holds.
    pub fn set(&self) -> Set {
        self.set
    }

    /// The limits its runner is held to: what the person set, and the defaults for the rest.
    pub fn limits(&self) -> Limits {
        self.limits
    }

    /// **What defines this pool, as a digest** (#2050): SHA-256, in hex, of its row as this file
    /// keeps it (its name, kind, image and the limits the person set), so any change to how it is
    /// defined changes it. Its price is not how it is defined, and is left out (#3491).
    pub fn definition_digest(&self) -> String {
        use sha2::Digest as _;
        let mut row = pool_to_row(self);
        if let Some(fields) = row.as_object_mut() {
            fields.remove("pricePerHour");
        }
        let row = row.to_string();
        crate::runnerlink::hex_of(&sha2::Sha256::digest(row.as_bytes()))
    }

    /// This pool, its runner image `image` from now on (#2749).
    fn moved_to(&self, image: Image) -> Pool {
        Pool {
            image,
            ..self.clone()
        }
    }
}

/// Whether `name` can name a pool: as a runner is named, since a pool's runner takes its name.
pub fn usable_pool_name(name: &str) -> Result<(), String> {
    let plain = |b: u8| b.is_ascii_lowercase() || b.is_ascii_digit();
    let named = !name.is_empty()
        && name.len() <= MOST_NAME
        && name.bytes().all(|b| plain(b) || b == b'-')
        && name.bytes().next().is_some_and(plain)
        && name.bytes().last().is_some_and(plain);
    if named {
        Ok(())
    } else {
        Err(format!(
            "\"{name}\" cannot name a pool: use lower-case letters, digits and dashes, at most \
             {MOST_NAME}, starting and ending with a letter or digit"
        ))
    }
}

/// The longest pool name: a runner's longest.
const MOST_NAME: usize = 40;

/// One runner this desktop pinned: where it is, and who it is on a link.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Kept {
    /// The runner's container as it was made, which every check and link holds it to.
    pub made: Made,
    /// The runner's own device id and public link key, as its pairing answered them.
    pub pinned: Pinned,
    /// When it was paired, in seconds since the epoch.
    pub paired: u64,
    /// The pool it is a runner of, where that is not the pool of its name (#2749): one made
    /// beside the pool's runner after an update.
    pub of: Option<String>,
    /// It was replaced by a runner of the desktop's version (#2749): it starts no new chat,
    /// and is removed once its last chat has ended.
    pub draining: bool,
}

impl Kept {
    pub fn name(&self) -> &str {
        self.made.runner.name()
    }

    /// The pool it is a runner of: the one [`Kept::of`] names, or the pool of its name.
    pub fn pool(&self) -> &str {
        self.of.as_deref().unwrap_or(self.name())
    }
}

/// What the file holds: the pools, then the runners, each in the order they were kept.
#[derive(Debug, Default)]
struct Held {
    pools: Vec<Pool>,
    runners: Vec<Kept>,
}

/// The file under `config_root`, or none where there is no file; refused where it cannot be
/// read safely or is not as this module writes it.
fn load(config_root: &Path) -> Result<Held, Refused> {
    let path = crate::machine::dir(config_root).join(RUNNERS_FILE);
    let text = match purlis_same_user::read_private_file(&path) {
        Ok(text) => text,
        Err(why) if why.kind() == std::io::ErrorKind::NotFound => return Ok(Held::default()),
        Err(why) => {
            return Err(Refused::Other(format!(
                "the runners this machine keeps could not be read safely ({}): {why}",
                path.display()
            )));
        }
    };
    let unreadable = || {
        Refused::Other(format!(
            "{} is not as purlis writes it, so no pool or runner of it is used",
            path.display()
        ))
    };
    let value: serde_json::Value = serde_json::from_str(&text).map_err(|_| unreadable())?;
    let fields = value.as_object().ok_or_else(unreadable)?;
    if fields.get("version").and_then(serde_json::Value::as_u64) != Some(1)
        || fields
            .keys()
            .any(|key| !matches!(key.as_str(), "version" | "pools" | "runners"))
    {
        return Err(unreadable());
    }
    let rows = |key: &str| -> Result<&[serde_json::Value], Refused> {
        match fields.get(key) {
            None if key == "pools" => Ok(&[]),
            found => found
                .and_then(serde_json::Value::as_array)
                .map(Vec::as_slice)
                .ok_or_else(unreadable),
        }
    };
    let runners = rows("runners")?
        .iter()
        .map(|row| from_row(row).ok_or_else(unreadable))
        .collect::<Result<Vec<_>, _>>()?;
    let pools = rows("pools")?
        .iter()
        .map(|row| {
            pool_from_row(row).map_err(|why| match why {
                // Said, so the person who typed it knows which value to put right (#3491).
                Some(why) => Refused::Other(format!(
                    "{} is not as purlis writes it, so no pool or runner of it is used: {why}",
                    path.display()
                )),
                None => unreadable(),
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    let mut names: Vec<&str> = pools.iter().map(Pool::name).collect();
    names.sort_unstable();
    if names.windows(2).any(|two| two[0] == two[1]) {
        return Err(unreadable());
    }
    Ok(Held { pools, runners })
}

/// The runners kept under `config_root`, in the order they were kept: none where there is no
/// file; refused where it cannot be read safely or is not as [`keep`] writes it.
pub fn read(config_root: &Path) -> Result<Vec<Kept>, Refused> {
    Ok(load(config_root)?.runners)
}

/// The pools added under `config_root`, in the order they were added (#2679).
pub fn pools(config_root: &Path) -> Result<Vec<Pool>, Refused> {
    Ok(load(config_root)?.pools)
}

/// The runner kept as `name`, if one is.
pub fn named(config_root: &Path, name: &str) -> Result<Option<Kept>, Refused> {
    Ok(read(config_root)?
        .into_iter()
        .find(|kept| kept.name() == name))
}

/// **The runner a workspace whose chats run on `pool` starts them on** (#2679): the pool's own
/// runner, only where both the pool and a runner of its name are kept here, so no name a
/// workspace holds reaches a runner this machine did not pair and keep.
pub fn runner_for(config_root: &Path, pool: &str) -> Result<Kept, Refused> {
    let held = load(config_root)?;
    if !held.pools.iter().any(|kept| kept.name() == pool) {
        return Err(Refused::Other(format!(
            "this workspace's chats run on pool {pool}, which is not on this machine, so \
             nothing was started"
        )));
    }
    let image = held
        .pools
        .iter()
        .find(|kept| kept.name() == pool)
        .map(|kept| kept.image().clone());
    // A draining runner starts no new chat (#2749).
    held.runners
        .into_iter()
        .find(|kept| {
            kept.pool() == pool
                && !kept.draining
                && Some(kept.made.runner.image()) == image.as_ref()
        })
        .ok_or_else(|| {
            Refused::Other(format!(
                "pool {pool} has no runner of its image, so nothing was started; remove the \
                 pool and add it again"
            ))
        })
}

/// **This desktop's label on the runner containers it makes** (#2869,
/// [`crate::dockerpool::DESKTOP_LABEL`]): the first 16 hex digits of a SHA-256 of the config
/// home's path, so two config homes on one Docker daemon never take each other's runners for
/// their own leftovers.
pub fn desktop_label(config_root: &Path) -> String {
    use sha2::Digest;
    sha2::Sha256::digest(config_root.as_os_str().as_encoded_bytes())
        .iter()
        .take(8)
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

/// **The runner a link that names `name` reaches** (#2865), and whether it is draining: a pool's
/// name reaches its runner as [`runner_for`] says, where new chats start; `<pool>/<runner>`
/// reaches that pool's runner of that name only while it is kept draining, so its chats can be
/// answered, watched and ended until the last one has. Any other name reaches nothing.
pub fn reachable(config_root: &Path, name: &str) -> Result<(Kept, bool), Refused> {
    let Some((pool, runner)) = name.split_once('/') else {
        return runner_for(config_root, name).map(|kept| (kept, false));
    };
    read(config_root)?
        .into_iter()
        .find(|kept| kept.draining && kept.pool() == pool && kept.name() == runner)
        .map(|kept| (kept, true))
        .ok_or_else(|| {
            Refused::Other(format!(
                "pool {pool} has no runner {runner} draining on this machine, so nothing was \
                 reached"
            ))
        })
}

/// The lock every change of the file is made under, held for as long as this is: what
/// [`crate::pools`] holds across adding or removing a pool, so another add or removal of it
/// waits for this one.
pub struct Locked(#[expect(dead_code, reason = "held for its drop")] crate::machine::Lock);

/// Takes [`Locked`].
pub fn lock(config_root: &Path) -> Locked {
    Locked(crate::machine::Lock::named(config_root, RUNNERS_LOCK))
}

/// **Records `pool`, once** (#2679): refused where a pool or a runner of its name is kept
/// already, and what is kept stays.
pub fn record_pool(config_root: &Path, pool: &Pool) -> Result<(), Refused> {
    record_pool_locked(config_root, pool, &lock(config_root))
}

/// [`record_pool`], under `_locked`.
pub fn record_pool_locked(
    config_root: &Path,
    pool: &Pool,
    _locked: &Locked,
) -> Result<(), Refused> {
    let mut held = load(config_root)?;
    let name = pool.name();
    if held.pools.iter().any(|kept| kept.name() == name)
        || held.runners.iter().any(|kept| kept.name() == name)
    {
        return Err(Refused::Other(format!(
            "a pool or a runner named {name} is on this machine already; name the pool \
             otherwise"
        )));
    }
    held.pools.push(pool.clone());
    write(config_root, &held)
}

/// Forget the pool recorded as `name`; nothing where none is. Its runner is [`remove`]d first.
pub fn forget_pool(config_root: &Path, name: &str) -> Result<(), Refused> {
    forget_pool_locked(config_root, name, &lock(config_root))
}

/// [`forget_pool`], under `_locked`.
pub fn forget_pool_locked(config_root: &Path, name: &str, _locked: &Locked) -> Result<(), Refused> {
    let mut held = load(config_root)?;
    let before = held.pools.len();
    held.pools.retain(|kept| kept.name() != name);
    if held.pools.len() != before {
        write(config_root, &held)?;
    }
    Ok(())
}

/// Keep `answered`, what `made`'s runner's pairing answered, for `made`, on the desktop that is
/// `ours` on its links (its device id and the public half of its link key, as the pairing
/// handed them to the runner): once.
///
/// A runner of this name kept already with the same container, device id and key is kept as
/// it was. One with another device id or key is [`Refused::KeyChanged`], and so is one of
/// another container: what was kept stays. A device id or key another kept runner holds, or
/// the desktop's own device id or link key, is refused, and so is a name a pool holds: a pool's
/// runner is kept as the pool is added ([`keep_for_its_pool`]).
pub fn keep(
    config_root: &Path,
    made: &Made,
    answered: &Pinned,
    ours: &Pinned,
) -> Result<Kept, Refused> {
    let _locked = lock(config_root);
    let mut held = load(config_root)?;
    let kept = keep_in(&mut held, made, answered, ours, false, None)?;
    write(config_root, &held)?;
    Ok(kept)
}

/// [`keep`], for the runner of the pool of its name as the pool is added, under `locked`
/// ([`crate::pools::add`]).
pub fn keep_for_its_pool(
    config_root: &Path,
    made: &Made,
    answered: &Pinned,
    ours: &Pinned,
    _locked: &Locked,
) -> Result<Kept, Refused> {
    let mut held = load(config_root)?;
    let kept = keep_in(&mut held, made, answered, ours, true, None)?;
    write(config_root, &held)?;
    Ok(kept)
}

/// **Keeps `made`, the runner made beside pool `pool`'s for its new `image`, as that pool's
/// runner from now on** (#2749), under `_locked`: it is kept `of` the pool once, as [`keep`]
/// keeps a runner; each runner of the pool kept before it is kept draining; and the pool's
/// image is `made`'s. A pool not kept here is refused, and nothing changes.
pub fn succeed_locked(
    config_root: &Path,
    pool: &str,
    made: &Made,
    answered: &Pinned,
    ours: &Pinned,
    _locked: &Locked,
) -> Result<Kept, Refused> {
    let mut held = load(config_root)?;
    let Some(at) = held.pools.iter().position(|kept| kept.name() == pool) else {
        return Err(Refused::Other(format!(
            "no pool named {pool} is on this machine"
        )));
    };
    let new = made.runner.name().to_owned();
    for runner in &mut held.runners {
        if runner.pool() == pool && runner.name() != new {
            runner.draining = true;
        }
    }
    let kept = keep_in(&mut held, made, answered, ours, false, Some(pool))?;
    held.pools[at] = held.pools[at].moved_to(made.runner.image().clone());
    write(config_root, &held)?;
    Ok(kept)
}

/// **The runners kept draining** (#2749): each replaced by a runner of the desktop's version,
/// to be removed once its last chat has ended.
pub fn draining(config_root: &Path) -> Result<Vec<Kept>, Refused> {
    Ok(read(config_root)?
        .into_iter()
        .filter(|kept| kept.draining)
        .collect())
}

/// Keeps `made` in `held`, as [`keep`] says, `of` the pool `of` names where one does.
fn keep_in(
    held: &mut Held,
    made: &Made,
    answered: &Pinned,
    ours: &Pinned,
    for_its_pool: bool,
    of: Option<&str>,
) -> Result<Kept, Refused> {
    let name = made.runner.name();
    if !for_its_pool && held.pools.iter().any(|pool| pool.name() == name) {
        return Err(Refused::Other(format!(
            "runner {name} is named as pool {name} is, so purlis did not keep it; a pool's \
             runner is made as the pool is added"
        )));
    }
    let kept = &mut held.runners;
    if let Some(there) = kept.iter().find(|kept| kept.name() == name) {
        return if &there.made == made && same_pin(&there.pinned, answered) {
            Ok(there.clone())
        } else {
            Err(Refused::KeyChanged {
                name: name.to_owned(),
            })
        };
    }
    if same_device(&answered.device, &ours.device) {
        return Err(Refused::Other(format!(
            "runner {name} answered with this machine's own device id, so purlis did not keep \
             it; remove the runner and make it again"
        )));
    }
    if answered.key == ours.key {
        return Err(Refused::Other(format!(
            "runner {name} answered with this machine's own link key, so purlis did not keep \
             it; remove the runner and make it again"
        )));
    }
    if let Some(other) = kept.iter().find(|kept| {
        same_device(&kept.pinned.device, &answered.device) || kept.pinned.key == answered.key
    }) {
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
        of: of.filter(|pool| *pool != name).map(str::to_owned),
        draining: false,
    };
    kept.push(new.clone());
    Ok(new)
}

/// Whether two device ids are one: a device id is a ULID, which reads the same in either case
/// (#2741).
fn same_device(one: &str, other: &str) -> bool {
    one.eq_ignore_ascii_case(other)
}

/// Whether two pins are one: the same device id, in either case, and the same key.
fn same_pin(one: &Pinned, other: &Pinned) -> bool {
    same_device(&one.device, &other.device) && one.key == other.key
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
    remove_locked(config_root, docker, name, unpair, &lock(config_root))
}

/// [`remove`], under `locked`.
pub fn remove_locked(
    config_root: &Path,
    docker: &Docker,
    name: &str,
    unpair: Option<&Unpair<'_>>,
    locked: &Locked,
) -> Result<Removed, Refused> {
    let kept = named(config_root, name)?
        .ok_or_else(|| Refused::Other(format!("no runner named {name} is kept on this machine")))?;
    let peer = unpair.map(|unpair| unpair(&kept));
    // When it ran, as Docker last says, before its container goes (#3492).
    if let Ok(ran) = docker.ran(&kept.made) {
        record_hours(crate::runnerhours::seen(
            config_root,
            kept.pool(),
            name,
            ran,
            now(),
        ));
    }
    let (container, volume) = docker.remove_each(&kept.made);
    let forgotten = container.is_ok() && volume.is_ok();
    if container.is_ok() {
        record_hours(crate::runnerhours::stopped(
            config_root,
            kept.pool(),
            name,
            now(),
        ));
    }
    if forgotten {
        forget_locked(config_root, name, locked)?;
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
    forget_locked(config_root, name, &lock(config_root))
}

fn forget_locked(config_root: &Path, name: &str, _locked: &Locked) -> Result<(), Refused> {
    let mut held = load(config_root)?;
    let before = held.runners.len();
    held.runners.retain(|kept| kept.name() != name);
    if held.runners.len() != before {
        write(config_root, &held)?;
    }
    Ok(())
}

/// Writes `held` whole: `pools` left out while there is none, so a machine that added no pool
/// keeps the file #2525 wrote.
fn write(config_root: &Path, held: &Held) -> Result<(), Refused> {
    let rows: Vec<serde_json::Value> = held.runners.iter().map(to_row).collect();
    let mut text = serde_json::json!({ "version": 1, "runners": rows });
    if !held.pools.is_empty() {
        text["pools"] = held.pools.iter().map(pool_to_row).collect();
    }
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
    let mut row = serde_json::json!({
        "name": runner.name(),
        "pool": "docker",
        "container": kept.made.container.as_str(),
        "image": runner.image().as_str(),
        "user": runner.user().spelled(),
        "home": runner.home(),
        "device": kept.pinned.device,
        "key": crate::runnerlink::hex_of(kept.pinned.key.as_bytes()),
        "paired": kept.paired,
    });
    // Each written only where it says something (#2749), so a runner kept before is as it was.
    if let Some(of) = &kept.of {
        row["of"] = of.as_str().into();
    }
    if kept.draining {
        row["draining"] = true.into();
    }
    row
}

fn from_row(row: &serde_json::Value) -> Option<Kept> {
    let fields = row.as_object()?;
    let known = [
        "name",
        "pool",
        "container",
        "image",
        "user",
        "home",
        "device",
        "key",
        "paired",
        "of",
        "draining",
    ];
    if fields.keys().any(|key| !known.contains(&key.as_str())) {
        return None;
    }
    let text = |key: &str| fields.get(key).and_then(serde_json::Value::as_str);
    // Left out where it says nothing; `draining` is only ever `true`.
    let of = match fields.get("of") {
        None => None,
        Some(of) => {
            let of = of.as_str()?;
            usable_pool_name(of).ok()?;
            Some(of.to_owned())
        }
    };
    let draining = match fields.get("draining") {
        None => false,
        Some(serde_json::Value::Bool(true)) => true,
        Some(_) => return None,
    };
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
        of,
        draining,
    })
}

fn pool_to_row(pool: &Pool) -> serde_json::Value {
    let mut row = serde_json::json!({
        "name": pool.name(),
        "kind": "docker",
        "image": pool.image().as_str(),
    });
    let set = pool.set();
    if let Some(millicpus) = set.millicpus {
        row["cpus"] = if millicpus % 1000 == 0 {
            serde_json::json!(millicpus / 1000)
        } else {
            serde_json::json!(f64::from(millicpus) / 1000.0)
        };
    }
    if let Some(memory) = set.memory_mib {
        row["memory"] = memory.into();
    }
    if let Some(processes) = set.processes {
        row["processes"] = processes.into();
    }
    if let Some(tmp) = set.tmp_mib {
        row["tmp"] = tmp.into();
    }
    if let Some(price) = pool.price() {
        row["pricePerHour"] = serde_json::json!({
            "amount": price.amount(),
            "currency": price.currency(),
        });
    }
    row
}

/// A pool's row: its name, kind `docker` and image, and each limit the person set, as
/// [`pool_to_row`] writes them: `cpus` a number of CPUs to the thousandth, `memory` and `tmp`
/// in MiB, `processes` a count, and `pricePerHour` where the pool declares one (#3491). Any
/// other key, or a value of another shape, is refused: with the reason for a price, which the
/// person writes by hand (#3491), and with none for anything else.
fn pool_from_row(row: &serde_json::Value) -> Result<Pool, Option<String>> {
    let price = match row.get("pricePerHour") {
        None => None,
        Some(price) => Some(price_from_row(price).map_err(|why| {
            let pool = row
                .get("name")
                .and_then(serde_json::Value::as_str)
                .filter(|name| usable_pool_name(name).is_ok())
                .map_or_else(|| "a pool".to_owned(), |name| format!("pool {name}"));
            Some(format!("the pricePerHour of {pool} is not a price: {why}"))
        })?),
    };
    plain_pool_from_row(row)
        .map(|pool| pool.priced(price))
        .ok_or(None)
}

/// A pool's `pricePerHour`, `{"amount": "<decimal>", "currency": "<ISO 4217>"}`, or why not.
fn price_from_row(price: &serde_json::Value) -> Result<Price, String> {
    let shape = || {
        "write it as {\"amount\": \"0.40\", \"currency\": \"EUR\"}, the amount in a string"
            .to_owned()
    };
    let price = price.as_object().ok_or_else(shape)?;
    if price.len() != 2 {
        return Err(shape());
    }
    let said = |key: &str| price.get(key).and_then(serde_json::Value::as_str);
    let (Some(amount), Some(currency)) = (said("amount"), said("currency")) else {
        return Err(shape());
    };
    Price::new(amount, currency).map_err(|why| why.to_string())
}

/// [`pool_from_row`] but its price.
fn plain_pool_from_row(row: &serde_json::Value) -> Option<Pool> {
    let fields = row.as_object()?;
    let known = [
        "name",
        "kind",
        "image",
        "cpus",
        "memory",
        "processes",
        "tmp",
        "pricePerHour",
    ];
    if fields.keys().any(|key| !known.contains(&key.as_str())) {
        return None;
    }
    let text = |key: &str| fields.get(key).and_then(serde_json::Value::as_str);
    if text("kind")? != "docker" {
        return None;
    }
    // A key that is there must be a number of its kind; one left out is the default.
    fn count<T: TryFrom<u64>>(value: Option<&serde_json::Value>) -> Option<Option<T>> {
        match value {
            None => Some(None),
            Some(value) => Some(Some(T::try_from(value.as_u64()?).ok()?)),
        }
    }
    let millicpus = match fields.get("cpus") {
        None => None,
        Some(cpus) => {
            let thousandths = cpus.as_f64()? * 1000.0;
            let whole = thousandths.round();
            if (thousandths - whole).abs() > 1e-6 || !(0.0..=f64::from(u32::MAX)).contains(&whole) {
                return None;
            }
            #[expect(
                clippy::cast_possible_truncation,
                clippy::cast_sign_loss,
                reason = "a whole number in u32's range, checked above"
            )]
            Some(whole as u32)
        }
    };
    let set = Set {
        millicpus,
        memory_mib: count(fields.get("memory"))?,
        processes: count(fields.get("processes"))?,
        tmp_mib: count(fields.get("tmp"))?,
    };
    Pool::new(text("name")?, Image::pinned(text("image")?).ok()?, set).ok()
}

/// A runner's hours not written are said, and change nothing else (#3492).
fn record_hours(written: Result<(), String>) {
    if let Err(why) = written {
        tracing::warn!("purlis: {why}");
    }
}

fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|since| since.as_secs())
        .unwrap_or_default()
}

#[cfg(test)]
pub(crate) mod tests;
