// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The pools this desktop added, and the runners it pinned** (ADR 0078 §3 and §8, #2525,
//! #2679, #2749): `<config>/runners.json`, ruled 2026-10-18 and 2026-10-20, `{"version": 1,
//! "pools": [{"name", "kind", "image", "cpus", "memory", "processes", "tmp", "suspendAfter",
//! "frames", "width", "profiles"}], "runners":
//! [{"name", "pool", "container", "image", "user", "home", "device", "key", "paired", "of"?,
//! "draining"?, "engine"?, "home_created"?}]}`.
//!
//! - **A pool is named and defined only here** ([`Pool`]): a Docker pool's name, the runner
//!   image by digest, and the limits the person set, each left out where purlis's default
//!   holds ([`crate::dockerpool::Limits`]), and how long its runner is left without a link
//!   before it suspends itself (`suspendAfter`, #3471, ruled 2026-10-28), left out at a day. Its browsers' frame limits over the link
//!   are among them (#3297). So are the profiles its runner keeps (`profiles`, #4047, ruled
//!   2026-10-30): the names chosen for it, or, left out, the profile a new chat in the open
//!   project gets ([`Pool::profiles_kept`]). A workspace names a pool (`machine.json`), and a name
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
//! - **A cluster pool is a Pool of a team's cluster, reached through `kubectl`** (#3292): its
//!   row names the kubeconfig and context, the cluster's Pool, the namespace its claims are made
//!   in, and the cluster operator key pinned as it was added ([`ClusterPool`]); its one runner's
//!   row names its claim, its pod and what its pairing answered ([`ClusterKept`]).
//! - **The file is this user's alone**, `0600` in the config home's own folder, and denied to
//!   chats. A file of another version or shape is refused rather than read or replaced.

use std::path::Path;

use purlis_session_protocol::browsing::LinkFrames;
use purlis_session_protocol::hostlink::Pinned;

use crate::clusterpool::{self, Claimed, Kubectl, OperatorKey, PoolPin};
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

/// What a person set on a pool, each `None` where purlis's default holds: its limits
/// ([`Limits::default`]), its CPUs in thousandths, its memory and its `/tmp` in MiB, and its
/// processes; and how long its runner is left without a link from this desktop before it
/// suspends itself (`suspendAfter`, #3471, [`crate::dockerpool::SUSPEND_AFTER`]).
/// And how its runner's browsers are watched over the link (#3297): the most frames a
/// second, and the widest frame in pixels ([`LinkFrames::DEFAULT`]).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Set {
    pub millicpus: Option<u32>,
    pub memory_mib: Option<u64>,
    pub processes: Option<u32>,
    pub tmp_mib: Option<u64>,
    pub suspend_after: Option<std::time::Duration>,
    pub frames: Option<u32>,
    pub width: Option<u16>,
    /// A pool's limits on what purlis sends and waits for ([`crate::poollimits`], spec #4275).
    pub chosen: crate::poollimits::Chosen,
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
    /// The profiles chosen for it (`profiles`, #4047), in order; `None` where none were.
    profiles: Option<Vec<String>>,
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
    pub fn new(name: &str, image: Image, mut set: Set) -> Result<Pool, Refused> {
        usable_pool_name(name).map_err(Refused::Other)?;
        let default = Limits::default();
        let limits = Limits::new(
            set.millicpus.unwrap_or(default.millicpus()),
            set.memory_mib.unwrap_or(default.memory_mib()),
            set.processes.unwrap_or(default.pids()),
            set.tmp_mib.unwrap_or(default.tmp_mib()),
        )
        .map_err(|why| Refused::Other(why.0))?;
        LinkFrames::new(
            set.frames.unwrap_or(LinkFrames::DEFAULT.frames),
            set.width.unwrap_or(LinkFrames::DEFAULT.width),
        )
        .map_err(Refused::Other)?;
        if let Some(after) = set.suspend_after {
            crate::dockerpool::suspend_after(after).map_err(|why| Refused::Other(why.0))?;
        }
        // A day is the default, so it is left out of the file as an unset one is.
        set.suspend_after = set
            .suspend_after
            .filter(|after| *after != crate::dockerpool::SUSPEND_AFTER);
        Ok(Pool {
            name: name.to_owned(),
            image,
            set,
            limits,
            price: None,
            profiles: None,
        })
    }

    /// **How its runner's browsers are watched over the link** (#3297): what the person set,
    /// and spec #2041's defaults for the rest. Given with each browser's start on its runner.
    pub fn link_frames(&self) -> LinkFrames {
        LinkFrames {
            frames: self.set.frames.unwrap_or(LinkFrames::DEFAULT.frames),
            width: self.set.width.unwrap_or(LinkFrames::DEFAULT.width),
        }
    }

    /// This pool, declaring `price` per hour of its runner, or none (#3491).
    pub fn priced(self, price: Option<Price>) -> Pool {
        Pool { price, ..self }
    }

    /// The price per hour it declares, if any (#3491): only then is money shown for it.
    pub fn price(&self) -> Option<&Price> {
        self.price.as_ref()
    }

    /// This pool, keeping the profiles `profiles` on its runner, or, `None`, the default
    /// (#4047).
    pub fn keeping_profiles(self, profiles: Option<Vec<String>>) -> Pool {
        Pool { profiles, ..self }
    }

    /// The profiles chosen for it (#4047), if any were: `None` keeps the default.
    pub fn profiles(&self) -> Option<&[String]> {
        self.profiles.as_deref()
    }

    /// **The profiles its runner keeps** (#4047): the ones chosen for it, or, where none were,
    /// `default`, the profile a new chat in the open project gets, if there is one.
    pub fn profiles_kept(&self, default: Option<&str>) -> Vec<String> {
        kept_of(self.profiles.as_deref(), default)
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

    /// How long its runner is left without a link from this desktop before it suspends
    /// itself: what the person set, or a day.
    pub fn suspend_after(&self) -> std::time::Duration {
        self.set
            .suspend_after
            .unwrap_or(crate::dockerpool::SUSPEND_AFTER)
    }

    /// **What defines this pool, as a digest** (#2050): SHA-256, in hex, of its row as this file
    /// keeps it (its name, kind, image, the limits the person set and its `suspendAfter`), so any
    /// change to how it is defined changes it. Its price is not how it is defined, and is left
    /// out (#3491), and so are the profiles it keeps (#4047).
    pub fn definition_digest(&self) -> String {
        use sha2::Digest as _;
        let mut row = pool_to_row(self);
        if let Some(fields) = row.as_object_mut() {
            fields.remove("pricePerHour");
            fields.remove("profiles");
            // These apply from the next attempt, so a change makes nothing again (Q13).
            for limit in crate::poollimits::all() {
                if limit.takes_effect == crate::poollimits::TakesEffect::NextAttempt {
                    fields.remove(limit.key);
                }
            }
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
    /// Its sign-ins were carried to its pool's runner (#4146): it is not carried from again.
    pub carried: bool,
    /// Docker failed its carry (#4298): the next look tries once more, then it is `carried`.
    pub carry_failed: bool,
    /// It answered that this desktop is out of its peers (#4146): it cannot be linked to, so it
    /// is not asked how many chats it has, nor unpaired again.
    pub unpaired: bool,
    /// The profiles signed out of its pool while it drains (#4145): their sign-ins here are
    /// never carried, even where the sign-out could not reach it.
    pub signed_out: Vec<String>,
    /// The engine that made it, as `docker info` prints its id (#4166): only that engine's word
    /// says it was removed. `None` for a runner kept before this, or an engine that prints none.
    pub engine: Option<String>,
    /// Its home volume's `CreatedAt`, as Docker printed it when it was made (#4167): only a home
    /// that still prints this is its own, and gives sign-ins to a runner made again.
    pub home_created: Option<String>,
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
    /// The cluster pools, after the Docker pools in the file (#3292).
    clusters: Vec<ClusterPool>,
    /// The cluster pools' runners, after the Docker runners in the file.
    cluster_runners: Vec<ClusterKept>,
}

impl Held {
    /// Whether a pool or runner of either kind is kept as `name`.
    fn names(&self, name: &str) -> bool {
        self.pools.iter().any(|kept| kept.name() == name)
            || self.runners.iter().any(|kept| kept.name() == name)
            || self.clusters.iter().any(|kept| kept.name() == name)
            || self.cluster_runners.iter().any(|kept| kept.name == name)
    }

    /// Every runner's pin, of either kind.
    fn pins(&self) -> impl Iterator<Item = (&str, &Pinned)> {
        self.runners
            .iter()
            .map(|kept| (kept.name(), &kept.pinned))
            .chain(
                self.cluster_runners
                    .iter()
                    .map(|kept| (kept.name.as_str(), &kept.pinned)),
            )
    }
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
    let kind = |row: &serde_json::Value, key: &str| {
        row.get(key).and_then(serde_json::Value::as_str) == Some(CLUSTER)
    };
    let (cluster_runners, runners): (Vec<&serde_json::Value>, Vec<&serde_json::Value>) =
        rows("runners")?.iter().partition(|row| kind(row, "pool"));
    let (clusters, pools): (Vec<&serde_json::Value>, Vec<&serde_json::Value>) =
        rows("pools")?.iter().partition(|row| kind(row, "kind"));
    let runners = runners
        .into_iter()
        .map(|row| from_row(row).ok_or_else(unreadable))
        .collect::<Result<Vec<_>, _>>()?;
    let cluster_runners = cluster_runners
        .into_iter()
        .map(|row| cluster_runner_from_row(row).ok_or_else(unreadable))
        .collect::<Result<Vec<_>, _>>()?;
    let clusters = clusters
        .into_iter()
        .map(|row| cluster_pool_from_row(row).ok_or_else(unreadable))
        .collect::<Result<Vec<_>, _>>()?;
    let pools = pools
        .into_iter()
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
    let mut names: Vec<&str> = pools
        .iter()
        .map(Pool::name)
        .chain(clusters.iter().map(ClusterPool::name))
        .collect();
    names.sort_unstable();
    if names.windows(2).any(|two| two[0] == two[1]) {
        return Err(unreadable());
    }
    // A cluster runner is kept only as the runner of its pool, under the pool's name.
    if cluster_runners
        .iter()
        .any(|kept| !clusters.iter().any(|pool| pool.name() == kept.name))
    {
        return Err(unreadable());
    }
    Ok(Held {
        pools,
        runners,
        clusters,
        cluster_runners,
    })
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
    if held.names(name) {
        return Err(Refused::Other(format!(
            "a pool or a runner named {name} is on this machine already; name the pool \
             otherwise"
        )));
    }
    held.pools.push(pool.clone());
    write(config_root, &held)
}

/// **Has the pool `name` keep the profiles `profiles` on its runner** (#4047), a Docker pool or
/// a cluster pool (#3819), or, `None`, the default: refused where no such pool is kept, and what
/// is kept stays.
pub fn set_pool_profiles(
    config_root: &Path,
    name: &str,
    profiles: Option<Vec<String>>,
) -> Result<(), Refused> {
    change_pool_profiles(config_root, name, |_| profiles)
}

/// **Has the Docker pool `name`, where it names no profiles, name `profiles` from now on**
/// (#4060): read and written under the file's lock, so a pool that names its own meanwhile
/// keeps them as they are. Refused where no such pool is kept.
pub fn name_pool_profiles_where_none(
    config_root: &Path,
    name: &str,
    profiles: Vec<String>,
) -> Result<(), Refused> {
    change_pool_profiles(config_root, name, |chosen| {
        Some(chosen.map_or(profiles, <[String]>::to_vec))
    })
}

/// **Adds the profile `profile` to the pool `name`** (#4047, of either kind since #3819): a pool
/// that named none keeps `default`, the profile a new chat in the open project gets, beside it.
/// Kept once.
pub fn add_pool_profile(
    config_root: &Path,
    name: &str,
    profile: &str,
    default: Option<&str>,
) -> Result<(), Refused> {
    if !crate::profiles::name_ok(profile) {
        return Err(Refused::Other(format!(
            "'{}' is not a profile's name, so pool {name} keeps it not",
            crate::shown::short(profile)
        )));
    }
    change_pool_profiles(config_root, name, |chosen| {
        let mut kept = kept_of(chosen, default);
        if !kept.iter().any(|one| one == profile) {
            kept.push(profile.to_owned());
        }
        Some(kept)
    })
}

/// **Takes the profile `profile` off the pool `name`** (#4047, of either kind since #3819): a
/// pool that named none starts from `default`, so the last one taken off leaves it keeping none.
pub fn remove_pool_profile(
    config_root: &Path,
    name: &str,
    profile: &str,
    default: Option<&str>,
) -> Result<(), Refused> {
    change_pool_profiles(config_root, name, |chosen| {
        let mut kept = kept_of(chosen, default);
        kept.retain(|one| one != profile);
        Some(kept)
    })
}

/// The profiles `chosen` for a pool, or, where none were, `default` (#4047).
fn kept_of(chosen: Option<&[String]>, default: Option<&str>) -> Vec<String> {
    match chosen {
        Some(chosen) => chosen.to_vec(),
        None => default.map(str::to_owned).into_iter().collect(),
    }
}

/// **The profiles chosen for pool `name`**, a Docker pool or a cluster pool (#4047, #3819), if
/// any were: `None` keeps the default. Refused where no pool is called `name`.
pub fn profiles_of(config_root: &Path, name: &str) -> Result<Option<Vec<String>>, Refused> {
    let held = load(config_root)?;
    held.pools
        .iter()
        .find(|pool| pool.name() == name)
        .map(|pool| pool.profiles().map(<[String]>::to_vec))
        .or_else(|| {
            held.clusters
                .iter()
                .find(|pool| pool.name() == name)
                .map(|pool| pool.profiles().map(<[String]>::to_vec))
        })
        .ok_or_else(|| no_pool_keeps_profiles(name))
}

/// **The profiles pool `name`'s runner keeps**, of either kind (#4047, #3819): the ones chosen
/// for it, or, where none were, `default`. Refused where no pool is called `name`.
pub fn profiles_kept_of(
    config_root: &Path,
    name: &str,
    default: Option<&str>,
) -> Result<Vec<String>, Refused> {
    Ok(kept_of(profiles_of(config_root, name)?.as_deref(), default))
}

fn no_pool_keeps_profiles(name: &str) -> Refused {
    Refused::Other(format!(
        "there is no pool named {name} on this machine that keeps profiles"
    ))
}

/// Changes the profiles chosen for pool `name`, of either kind, to what `change` makes of the
/// ones chosen for it now, `None` where none were.
fn change_pool_profiles(
    config_root: &Path,
    name: &str,
    change: impl FnOnce(Option<&[String]>) -> Option<Vec<String>>,
) -> Result<(), Refused> {
    let _locked = lock(config_root);
    let mut held = load(config_root)?;
    if let Some(at) = held.pools.iter().position(|pool| pool.name() == name) {
        let profiles = change(held.pools[at].profiles());
        held.pools[at] = held.pools[at].clone().keeping_profiles(profiles);
    } else if let Some(at) = held.clusters.iter().position(|pool| pool.name() == name) {
        let profiles = change(held.clusters[at].profiles());
        held.clusters[at].profiles = profiles;
    } else {
        return Err(no_pool_keeps_profiles(name));
    }
    write(config_root, &held)
}

/// **The values chosen for pool `name`'s limits** (spec #4275), of either kind; none set where no
/// such pool is kept, so purlis's defaults hold.
pub fn chosen_limits_of(
    config_root: &Path,
    name: &str,
) -> Result<crate::poollimits::Chosen, Refused> {
    let held = load(config_root)?;
    let docker = held.pools.iter().find(|pool| pool.name() == name);
    let cluster = held.clusters.iter().find(|pool| pool.name() == name);
    Ok(docker
        .map(|pool| pool.set.chosen)
        .or_else(|| cluster.map(|pool| *pool.chosen))
        .unwrap_or_default())
}

/// **How long a sign-in on pool `name` may wait**: its limit as it stands (spec #4275), or
/// purlis's default. Only the wait: every check around a sign-in stands.
#[must_use]
pub fn longest_login_of(config_root: &Path, name: &str) -> std::time::Duration {
    chosen_limits_of(config_root, name)
        .unwrap_or_default()
        .wait(crate::poollimits::LimitId::SignInWait)
        .unwrap_or(crate::profilestore::sign_in::driven::LONGEST)
}

/// **Sets pool `name`'s limit `id` to `value`**, or, `None`, back to purlis's default (spec
/// #4275), of either kind, under the file's lock. It applies as the limit says
/// ([`crate::poollimits::TakesEffect`]). A value [`crate::poollimits::Limit::check`] refuses, or no such pool, is
/// refused, and what is kept stays.
pub fn set_pool_limit(
    config_root: &Path,
    name: &str,
    id: crate::poollimits::LimitId,
    value: Option<u64>,
) -> Result<(), Refused> {
    if let Some(value) = value {
        id.limit().check(value, None).map_err(Refused::Other)?;
    }
    let _locked = lock(config_root);
    let mut held = load(config_root)?;
    if let Some(pool) = held.pools.iter_mut().find(|pool| pool.name == name) {
        if id.limit().clusters_only {
            return Err(Refused::Other(format!(
                "{} is a cluster pool's, and {name} is a Docker pool",
                id.limit().plain
            )));
        }
        pool.set.chosen = pool.set.chosen.with(id, value);
    } else if let Some(pool) = held.clusters.iter_mut().find(|pool| pool.name == name) {
        *pool.chosen = pool.chosen.with(id, value);
    } else {
        return Err(Refused::Other(format!(
            "there is no pool named {name} on this machine"
        )));
    }
    write(config_root, &held)
}

/// Forget the pool recorded as `name`; nothing where none is. Its runner is [`remove`]d first.
pub fn forget_pool(config_root: &Path, name: &str) -> Result<(), Refused> {
    forget_pool_locked(config_root, name, &lock(config_root))
}

/// [`forget_pool`], under `_locked`.
pub fn forget_pool_locked(config_root: &Path, name: &str, _locked: &Locked) -> Result<(), Refused> {
    let mut held = load(config_root)?;
    let before = held.pools.len() + held.clusters.len();
    held.pools.retain(|kept| kept.name() != name);
    held.clusters.retain(|kept| kept.name() != name);
    if held.pools.len() + held.clusters.len() != before {
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
    if held.clusters.iter().any(|pool| pool.name() == name)
        || held.cluster_runners.iter().any(|kept| kept.name == name)
    {
        return Err(Refused::Other(format!(
            "a cluster pool named {name} is on this machine already; name the pool otherwise"
        )));
    }
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
    let other = kept
        .iter()
        .map(|kept| (kept.name(), &kept.pinned))
        .chain(
            held.cluster_runners
                .iter()
                .map(|kept| (kept.name.as_str(), &kept.pinned)),
        )
        .find(|(_, pinned)| {
            same_device(&pinned.device, &answered.device) || pinned.key == answered.key
        });
    if let Some((other, _)) = other {
        return Err(Refused::Other(format!(
            "runner {name} answered with the device id or key of runner {other}, so purlis did \
             not keep it; remove runner {name} and make it again"
        )));
    }
    let new = Kept {
        made: made.clone(),
        pinned: answered.clone(),
        paired: now(),
        of: of.filter(|pool| *pool != name).map(str::to_owned),
        draining: false,
        carried: false,
        carry_failed: false,
        unpaired: false,
        signed_out: Vec::new(),
        engine: None,
        home_created: None,
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
    /// Whether its row is gone: once its container and volume both are, or are on an engine this
    /// `docker` no longer reaches (#4325).
    pub forgotten: bool,
    /// What an earlier runner of the same pool left, as [`Removed::left`] said it.
    pub earlier: Option<String>,
}

impl Removed {
    /// What is left of the runner, and how to remove it; `None` when nothing is.
    pub fn left(&self) -> Option<String> {
        let left: Vec<&str> = [&self.container, &self.volume]
            .into_iter()
            .filter_map(|part| part.as_ref().err().map(String::as_str))
            .collect();
        if left.is_empty() {
            return self.earlier.clone();
        }
        let mut said = left.join("; ");
        if !self.forgotten {
            said.push_str(
                "; purlis keeps the runner until they are gone, so it can be removed again",
            );
        }
        Some(match &self.earlier {
            Some(earlier) => format!("{earlier}; {said}"),
            None => said,
        })
    }
}

/// **Whether a removal may act on `kept` through `docker`** (#4325): `Ok` where the engine that
/// made it answers, or none was noted of it, whose removal goes by name and id as it always did.
/// Otherwise what stands in the way, with nothing touched: a same-named volume on another engine
/// is not this runner's. `forgotten` says whether its row goes: on another engine it does, so a
/// pool whose engine is gone for good can still be removed; where nothing says which engine
/// answers, it stays.
fn engine_stands(kept: &Kept, docker: &Docker) -> Result<(), Removed> {
    let Some(made_on) = kept.engine.as_deref() else {
        return Ok(());
    };
    let (id, volume) = (kept.made.container.as_str(), kept.made.runner.volume());
    let name = kept.name();
    match docker.engine_id() {
        Ok(Some(now)) if now == made_on => Ok(()),
        Ok(Some(_)) => Err(Removed {
            peer: None,
            container: Err(format!(
                "runner {name}'s container {id} stays on Docker engine {made_on}, which made it \
                 and which Docker does not reach now, and any chats running in it may go on \
                 there. Once Docker points back at that engine: docker container rm --force {id}"
            )),
            volume: Err(format!(
                "its home volume {volume} stays there too, with the sign-ins in it: docker volume \
                 rm --force {volume}"
            )),
            forgotten: true,
            earlier: None,
        }),
        unsure => {
            let why = unsure.err().map_or_else(
                || "this Docker does not say which engine it is".to_owned(),
                |why| why.to_string(),
            );
            Err(Removed {
                peer: None,
                container: Err(format!(
                    "runner {name}'s container {id} was not removed: {why}, so purlis cannot tell \
                     whether the engine that made it answers"
                )),
                volume: Err(format!("its home volume {volume} was not removed either")),
                forgotten: false,
                earlier: None,
            })
        }
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
    if let Err(stands) = engine_stands(&kept, docker) {
        if stands.forgotten {
            forget_locked(config_root, name, locked)?;
        }
        return Ok(stands);
    }
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
        earlier: None,
    })
}

/// **Notes `note` on the runner kept as `name`** (#4146), under `locked`, so the next look knows
/// how far its drain got.
pub fn note_locked(
    config_root: &Path,
    name: &str,
    note: impl FnOnce(&mut Kept),
    _locked: &Locked,
) -> Result<(), Refused> {
    let mut held = load(config_root)?;
    if let Some(kept) = held.runners.iter_mut().find(|kept| kept.name() == name) {
        note(kept);
        write(config_root, &held)?;
    }
    Ok(())
}

/// **Notes on every runner of `pool` that `profile` is being signed out of the pool**, so no drain
/// carries its sign-in while the person's own sign-out waits in a terminal: taken back by
/// [`carries_again`] when it is signed in there again.
pub fn hold_every_runner_from_carrying(
    config_root: &Path,
    pool: &str,
    profile: &str,
) -> Result<(), Refused> {
    let _locked = lock(config_root);
    let mut held = load(config_root)?;
    let mut changed = false;
    for kept in held.runners.iter_mut().filter(|kept| kept.pool() == pool) {
        if !kept.signed_out.iter().any(|name| name == profile) {
            kept.signed_out.push(profile.to_owned());
            changed = true;
        }
    }
    if changed {
        write(config_root, &held)?;
    }
    Ok(())
}

/// **`profile` was signed in on pool `pool`'s runner**: a drain of that runner carries it again.
/// A runner already draining keeps its note.
pub fn carries_again(config_root: &Path, pool: &str, profile: &str) -> Result<(), Refused> {
    let _locked = lock(config_root);
    let mut held = load(config_root)?;
    let mut changed = false;
    for kept in held
        .runners
        .iter_mut()
        .filter(|kept| !kept.draining && kept.pool() == pool)
    {
        let before = kept.signed_out.len();
        kept.signed_out.retain(|name| name != profile);
        changed |= kept.signed_out.len() != before;
    }
    if changed {
        write(config_root, &held)?;
    }
    Ok(())
}

/// **Notes on each draining runner of `pool` that `profile` was signed out of the pool** (#4145),
/// so no drain carries its sign-in: taken under the runners' lock, so it waits for a carry
/// under way and holds for every carry after it. The draining runners, as they are now.
pub fn hold_from_carrying(
    config_root: &Path,
    pool: &str,
    profile: &str,
) -> Result<Vec<Kept>, Refused> {
    if !crate::profiles::name_ok(profile) {
        return Err(Refused::Other(format!(
            "'{}' is not a profile's name",
            crate::shown::short(profile)
        )));
    }
    let _locked = lock(config_root);
    let mut held = load(config_root)?;
    let mut changed = false;
    let mut draining = Vec::new();
    for kept in held
        .runners
        .iter_mut()
        .filter(|kept| kept.draining && kept.pool() == pool)
    {
        if !kept.signed_out.iter().any(|name| name == profile) {
            kept.signed_out.push(profile.to_owned());
            changed = true;
        }
        draining.push(kept.clone());
    }
    if changed {
        write(config_root, &held)?;
    }
    Ok(draining)
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
    let rows: Vec<serde_json::Value> = held
        .runners
        .iter()
        .map(to_row)
        .chain(held.cluster_runners.iter().map(cluster_runner_to_row))
        .collect();
    let mut text = serde_json::json!({ "version": 1, "runners": rows });
    if !held.pools.is_empty() || !held.clusters.is_empty() {
        text["pools"] = held
            .pools
            .iter()
            .map(pool_to_row)
            .chain(held.clusters.iter().map(cluster_pool_to_row))
            .collect();
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
    if kept.carried {
        row["carried"] = true.into();
    }
    if kept.carry_failed {
        row["carry_failed"] = true.into();
    }
    if kept.unpaired {
        row["unpaired"] = true.into();
    }
    if !kept.signed_out.is_empty() {
        row["signed_out"] = kept.signed_out.clone().into();
    }
    if let Some(engine) = &kept.engine {
        row["engine"] = engine.as_str().into();
    }
    if let Some(created) = &kept.home_created {
        row["home_created"] = created.as_str().into();
    }
    row
}

/// Whether `text`, an engine's id or a volume's `CreatedAt` as Docker printed it, can be kept
/// (#4166, #4167): one short line of printable words. It is only ever compared.
pub(crate) fn usable_noted(text: &str) -> bool {
    !text.is_empty()
        && text.len() <= 128
        && text.chars().all(|one| one.is_ascii_graphic() || one == ' ')
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
        "carried",
        "carry_failed",
        "unpaired",
        "signed_out",
        "engine",
        "home_created",
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
    let carried = match fields.get("carried") {
        None => false,
        Some(serde_json::Value::Bool(true)) => true,
        Some(_) => return None,
    };
    let carry_failed = match fields.get("carry_failed") {
        None => false,
        Some(serde_json::Value::Bool(true)) => true,
        Some(_) => return None,
    };
    let unpaired = match fields.get("unpaired") {
        None => false,
        Some(serde_json::Value::Bool(true)) => true,
        Some(_) => return None,
    };
    // Each a profile's name, which goes to the carry's shell as a word of its own.
    let signed_out = match fields.get("signed_out") {
        None => Vec::new(),
        Some(names) => names
            .as_array()?
            .iter()
            .map(|name| {
                name.as_str()
                    .filter(|name| crate::profiles::name_ok(name))
                    .map(str::to_owned)
            })
            .collect::<Option<Vec<_>>>()?,
    };
    // Left out where nothing was noted; anything else that is not usable is a file of another shape.
    let noted = |key: &str| match fields.get(key) {
        None => Some(None),
        Some(value) => value
            .as_str()
            .filter(|text| usable_noted(text))
            .map(|text| Some(text.to_owned())),
    };
    let engine = noted("engine")?;
    let home_created = noted("home_created")?;
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
        carried,
        carry_failed,
        unpaired,
        signed_out,
        engine,
        home_created,
    })
}

/// **How pool `name`'s runner's browsers are watched over the link** (#3297), as this machine
/// keeps the pool: the defaults where it is not kept, or the file cannot be read. A fenced
/// (test) build reads no store outside its fence: there, the defaults.
pub fn link_frames_of(name: &str) -> LinkFrames {
    let Some(config) = crate::machine::config_root_unheld() else {
        return LinkFrames::DEFAULT;
    };
    #[cfg(feature = "fenced")]
    if !crate::fence::inside(&config, &crate::fence::fence()) {
        return LinkFrames::DEFAULT;
    }
    pools(&config)
        .ok()
        .and_then(|pools| pools.into_iter().find(|pool| pool.name() == name))
        .map(|pool| pool.link_frames())
        .unwrap_or_default()
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
    if let Some(after) = set
        .suspend_after
        .and_then(|after| crate::spelledtime::spelled(after, crate::spelledtime::UP_TO_HOURS))
    {
        row["suspendAfter"] = after.into();
    }
    if let Some(price) = pool.price() {
        row["pricePerHour"] = serde_json::json!({
            "amount": price.amount(),
            "currency": price.currency(),
        });
    }
    if let Some(frames) = set.frames {
        row["frames"] = frames.into();
    }
    if let Some(width) = set.width {
        row["width"] = width.into();
    }
    if let Some(profiles) = pool.profiles() {
        row["profiles"] = profiles.into();
    }
    set.chosen.to_row(&mut row);
    row
}

/// A pool's row: its name, kind `docker` and image, and each limit the person set, as
/// [`pool_to_row`] writes them: `cpus` a number of CPUs to the thousandth, `memory` and `tmp`
/// in MiB, `processes` a count, `suspendAfter` a duration in seconds, minutes or hours (`"24h"`,
/// [`crate::spelledtime`]), `frames` a count a second and `width` in pixels (#3297), and
/// `pricePerHour` where the pool declares one (#3491), and `profiles`, the names of the profiles
/// chosen for it, each once (#4047). Any other
/// key, or a value of another shape, is refused: with the reason for a price, which the person
/// writes by hand (#3491), and with none for anything else.
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
    let profiles = match row.get("profiles") {
        None => None,
        Some(profiles) => Some(profiles_from_row(profiles).ok_or(None)?),
    };
    plain_pool_from_row(row)
        .map(|pool| pool.priced(price).keeping_profiles(profiles))
        .ok_or(None)
}

/// A pool's `profiles`: a list of profile names, each once, or `None`.
fn profiles_from_row(profiles: &serde_json::Value) -> Option<Vec<String>> {
    let mut named: Vec<String> = Vec::new();
    for profile in profiles.as_array()? {
        let profile = profile.as_str()?;
        if !crate::profiles::name_ok(profile) || named.iter().any(|kept| kept == profile) {
            return None;
        }
        named.push(profile.to_owned());
    }
    Some(named)
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
        "suspendAfter",
        "pricePerHour",
        "frames",
        "width",
        "profiles",
    ];
    if fields
        .keys()
        .any(|key| !known.contains(&key.as_str()) && crate::poollimits::of_key(key).is_none())
    {
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
        suspend_after: match fields.get("suspendAfter") {
            None => None,
            Some(after) => Some(crate::spelledtime::read(
                after.as_str()?,
                crate::spelledtime::UP_TO_HOURS,
            )?),
        },
        frames: count(fields.get("frames"))?,
        width: count(fields.get("width"))?,
        // The same checks as the command's: a limit only a cluster pool has is no Docker pool's.
        chosen: crate::poollimits::Chosen::from_row(fields)
            .ok()
            .filter(|chosen| {
                !crate::poollimits::all()
                    .iter()
                    .any(|limit| limit.clusters_only && chosen.get(limit.id).is_some())
            })?,
    };
    Pool::new(text("name")?, Image::pinned(text("image")?).ok()?, set).ok()
}

/// The kind a cluster pool's row, and its runner's, are of (#3292).
const CLUSTER: &str = "cluster";

/// **A cluster pool on this machine, as the person added it** (#3292): its name, which its one
/// runner is called by too; the kubeconfig and context its `kubectl` is pointed at, each named
/// on every run; the cluster's Pool its runner is claimed from; the namespace its claim is made
/// in; and its pin ([`PoolPin`], #3305): the cluster operator's key and the pool's cluster, as
/// the pool was added and as the key has rolled since, which every statement about its runner
/// must be signed with and made in.
///
/// **A pin with no cluster is no pin** (#3305): a row holding the key alone is read, so the file
/// stays readable, but its pool reaches no runner and says to add it again ([`Self::pin`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClusterPool {
    name: String,
    kubeconfig: std::path::PathBuf,
    context: String,
    pool: String,
    namespace: String,
    operator: OperatorKey,
    cluster: Option<String>,
    /// The programs its kubectl runs, each as named, every link on the way and its real file, as
    /// they were found when it was added:
    /// kubectl itself and each credential plugin its context names (#3292). A chat writes none of
    /// them ([`cluster_holds`]).
    holds: Vec<std::path::PathBuf>,
    /// **The person's mark that this pool is their own** (#4071): set only by their press in
    /// their window ([`set_cluster_owned`]), never as it is added, and read from nothing a chat,
    /// the Pool or the operator says.
    owned: bool,
    /// **Whether the cluster's Pool opts it in to minting there** (`mintInCluster`, #4204), as
    /// the Pool's spec said when it was added: the admin's word, never the person's or a chat's.
    /// The operator holds the Pool itself and refuses by it, so this says only how to ask.
    mint_in_cluster: bool,
    /// The profiles chosen for it (`profiles`, #3819), in order, as a Docker pool's are;
    /// `None` where none were.
    profiles: Option<Vec<String>>,
    /// The values this person chose for its limits ([`crate::poollimits`], spec #4275).
    /// Held apart, as the list of limits grows and [`Reached`] carries a whole pool.
    chosen: Box<crate::poollimits::Chosen>,
}

impl ClusterPool {
    /// How long this pool waits for its operator to vouch for a runner: what the person raised it
    /// to, or purlis's default (spec #4275).
    #[must_use]
    pub fn vouch_wait(&self) -> std::time::Duration {
        self.chosen
            .wait(crate::poollimits::LimitId::VouchWait)
            .unwrap_or(crate::pools::VOUCHED_WITHIN)
    }

    /// This pool with its limit `id` set to `value`: what its add was retried with (spec #4275).
    #[must_use]
    pub fn with_limit(mut self, id: crate::poollimits::LimitId, value: u64) -> ClusterPool {
        *self.chosen = self.chosen.with(id, Some(value));
        self
    }

    /// A cluster pool called `name` (as a Docker pool is named), its kubeconfig an absolute
    /// path, its context a name kubectl knows, its cluster `pool` and `namespace` named as
    /// Kubernetes names them, pinning `pin`.
    pub fn new(
        name: &str,
        kubeconfig: &Path,
        context: &str,
        pool: &str,
        namespace: &str,
        pin: PoolPin,
    ) -> Result<ClusterPool, Refused> {
        ClusterPool::held(
            name,
            kubeconfig,
            context,
            pool,
            namespace,
            pin.key,
            Some(pin.cluster),
        )
    }

    /// [`ClusterPool::new`], its pin's cluster `cluster`, where there is one.
    fn held(
        name: &str,
        kubeconfig: &Path,
        context: &str,
        pool: &str,
        namespace: &str,
        operator: OperatorKey,
        cluster: Option<String>,
    ) -> Result<ClusterPool, Refused> {
        if cluster
            .as_deref()
            .is_some_and(|cluster| !clusterpool::is_uid(cluster))
        {
            return Err(Refused::Other(
                "a cluster pool's cluster is the uid of its kube-system namespace".into(),
            ));
        }
        usable_pool_name(name).map_err(Refused::Other)?;
        if !kubeconfig.is_absolute() || kubeconfig.to_str().is_none_or(|text| !plain_text(text)) {
            return Err(Refused::Other(format!(
                "\"{}\" is not a kubeconfig purlis can name: give its absolute path",
                kubeconfig.display()
            )));
        }
        if !clusterpool::is_context(context) {
            return Err(Refused::Other(format!(
                "\"{context}\" is not a kubeconfig context purlis can name"
            )));
        }
        if !clusterpool::is_subdomain(pool) {
            return Err(Refused::Other(format!(
                "\"{pool}\" is not a cluster pool's name: lower-case letters, digits, dashes and \
                 dots, as Kubernetes names it"
            )));
        }
        if !clusterpool::is_label(namespace) {
            return Err(Refused::Other(format!(
                "\"{namespace}\" is not a namespace's name"
            )));
        }
        Ok(ClusterPool {
            name: name.to_owned(),
            kubeconfig: kubeconfig.to_path_buf(),
            context: context.to_owned(),
            pool: pool.to_owned(),
            namespace: namespace.to_owned(),
            operator,
            cluster,
            holds: Vec::new(),
            owned: false,
            mint_in_cluster: false,
            profiles: None,
            chosen: Box::default(),
        })
    }

    /// This pool, as the cluster's Pool `opted_in` to minting there as it was added (#4204).
    pub fn minting_in_cluster(self, opted_in: bool) -> ClusterPool {
        ClusterPool {
            mint_in_cluster: opted_in,
            ..self
        }
    }

    /// Whether the cluster's Pool opted it in to minting there as it was added (#4204).
    pub fn mints_in_cluster(&self) -> bool {
        self.mint_in_cluster
    }

    /// This pool, holding `holds`, the programs its kubectl runs as named, every link on the way
    /// and their real files, for no chat to write (#3292). Each is absolute.
    pub fn holding(self, holds: Vec<std::path::PathBuf>) -> Result<ClusterPool, Refused> {
        if let Some(relative) = holds.iter().find(|held| !held.is_absolute()) {
            return Err(Refused::Other(format!(
                "{} is not a program's path purlis can hold",
                relative.display()
            )));
        }
        Ok(ClusterPool { holds, ..self })
    }

    /// The real paths of the programs its kubectl runs ([`ClusterPool::holding`]).
    pub fn holds(&self) -> &[std::path::PathBuf] {
        &self.holds
    }

    /// Whether the person marked this pool as their own (#4071). Never enough alone: see
    /// `poollogins::Bound::of_a_cluster_pool`.
    pub fn owned(&self) -> bool {
        self.owned
    }

    /// The profiles chosen for it (#3819), if any were: `None` keeps the default, as a Docker
    /// pool's ([`Pool::profiles`]).
    pub fn profiles(&self) -> Option<&[String]> {
        self.profiles.as_deref()
    }

    /// **The profiles its runner keeps** (#3819), as [`Pool::profiles_kept`] says of a Docker
    /// pool's.
    pub fn profiles_kept(&self, default: Option<&str>) -> Vec<String> {
        kept_of(self.profiles.as_deref(), default)
    }

    /// **What a cluster runner's values are bound to, as a digest** (#4071, the person's ruling
    /// of 2026-10-30): SHA-256, in hex, of this pool's context, cluster and operator key and
    /// `kept`'s claim uid and pod, so any change to one asks again. Its device and link key are
    /// bound apart, as a Docker runner's are; no image digest is.
    pub fn bound_digest(&self, kept: &ClusterKept) -> String {
        use sha2::Digest as _;
        let bound = serde_json::json!([
            self.context,
            self.cluster,
            self.operator.spelled(),
            kept.claimed.uid,
            kept.pod,
        ])
        .to_string();
        crate::runnerlink::hex_of(&sha2::Sha256::digest(bound.as_bytes()))
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn kubeconfig(&self) -> &Path {
        &self.kubeconfig
    }

    pub fn context(&self) -> &str {
        &self.context
    }

    /// The cluster's Pool its runner is claimed from.
    pub fn pool(&self) -> &str {
        &self.pool
    }

    /// The namespace its claim is made in.
    pub fn namespace(&self) -> &str {
        &self.namespace
    }

    /// The cluster operator's key it pins.
    pub fn operator(&self) -> &OperatorKey {
        &self.operator
    }

    /// **Its pin**: the operator's key and its cluster. A pool kept with the key alone has none,
    /// and is refused, saying to add it again: its statements could be made in any cluster.
    pub fn pin(&self) -> Result<PoolPin, Refused> {
        let cluster = self.cluster.clone().ok_or_else(|| {
            Refused::Other(format!(
                "cluster pool {} was kept with its cluster operator's key alone, not the cluster \
                 it is in, so purlis does not link to its runner; remove the pool and add it again",
                self.name
            ))
        })?;
        Ok(PoolPin {
            key: self.operator.clone(),
            cluster,
        })
    }

    /// This pool, its kubectl pointed at the kubeconfig `kubeconfig` from now on: its own copy
    /// of the context, kept as it is added ([`keep_kubeconfig`]).
    pub fn kept_at(&self, kubeconfig: &Path) -> ClusterPool {
        ClusterPool {
            kubeconfig: kubeconfig.to_path_buf(),
            ..self.clone()
        }
    }

    /// This pool, pinning `pin` from now on: its key moved by a roll (#3305).
    fn repinned(&self, pin: &PoolPin) -> ClusterPool {
        ClusterPool {
            operator: pin.key.clone(),
            cluster: Some(pin.cluster.clone()),
            ..self.clone()
        }
    }

    /// `kubectl`, pointed at this pool's kubeconfig and context.
    pub fn kubectl(&self, kubectl: &Kubectl) -> Kubectl {
        kubectl.clone().pointed_at(&self.kubeconfig, &self.context)
    }
}

/// Whether `text` can be a kubeconfig's path or context in the file: not empty, at most 4096
/// bytes, and nothing a control character could hide behind.
fn plain_text(text: &str) -> bool {
    !text.is_empty() && text.len() <= 4096 && !text.chars().any(char::is_control)
}

/// **A cluster pool's runner this desktop paired** (#3292): the claim it was made by, the pod
/// the operator vouched for at its pairing, and the device id and key its pairing answered, kept
/// once.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClusterKept {
    /// Its pool's name, which it is called by.
    pub name: String,
    pub claimed: Claimed,
    pub pod: String,
    pub pinned: Pinned,
    /// When it was paired, in seconds since the epoch.
    pub paired: u64,
}

impl ClusterKept {
    /// **The name a link to this runner reports** (`relay_to`'s answer, #4071): `<pool>/<runner>`
    /// with the link key it was pinned with, so a link made to a runner since forgotten never
    /// reads as the one kept under its name now.
    pub fn reached(&self) -> String {
        format!(
            "{name}/{name}@{key}",
            name = self.name,
            key = crate::runnerlink::hex_of(self.pinned.key.as_bytes())
        )
    }
}

/// **What a pool's name reaches** (#3292): a Docker pool's runner, or a cluster pool with its
/// runner.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Reached {
    Docker(Kept),
    Cluster(ClusterPool, ClusterKept),
}

/// The cluster pools added under `config_root`, in the order they were added (#3292).
pub fn cluster_pools(config_root: &Path) -> Result<Vec<ClusterPool>, Refused> {
    Ok(load(config_root)?.clusters)
}

/// The cluster pools' runners kept under `config_root` (#3292).
pub fn cluster_runners(config_root: &Path) -> Result<Vec<ClusterKept>, Refused> {
    Ok(load(config_root)?.cluster_runners)
}

/// The cluster pool `name` and its runner, where both are kept here; `None` where no cluster
/// pool is called `name`, and refused where one is but its runner is not.
pub fn cluster_runner_for(
    config_root: &Path,
    name: &str,
) -> Result<Option<(ClusterPool, ClusterKept)>, Refused> {
    let held = load(config_root)?;
    let Some(pool) = held.clusters.into_iter().find(|pool| pool.name() == name) else {
        return Ok(None);
    };
    // A pool kept without its cluster reaches nothing (#3305).
    pool.pin()?;
    let kept = held
        .cluster_runners
        .into_iter()
        .find(|kept| kept.name == name)
        .ok_or_else(|| {
            Refused::Other(format!(
                "cluster pool {name} has no runner kept on this machine, so nothing was started; \
                 remove the pool and add it again"
            ))
        })?;
    Ok(Some((pool, kept)))
}

/// **The runner a workspace whose chats run on `pool` starts them on**, of either kind
/// (#3292): a cluster pool's, as [`cluster_runner_for`] says, else a Docker pool's, as
/// [`runner_for`] says.
pub fn runner_of(config_root: &Path, pool: &str) -> Result<Reached, Refused> {
    if let Some((cluster, kept)) = cluster_runner_for(config_root, pool)? {
        return Ok(Reached::Cluster(cluster, kept));
    }
    runner_for(config_root, pool).map(Reached::Docker)
}

/// **Every pool's name on this machine**, of either kind, Docker pools first, each in the order
/// added: what a workspace's *Runs on* setting offers.
pub fn pool_names(config_root: &Path) -> Result<Vec<String>, Refused> {
    let held = load(config_root)?;
    Ok(held
        .pools
        .iter()
        .map(|pool| pool.name().to_owned())
        .chain(held.clusters.iter().map(|pool| pool.name().to_owned()))
        .collect())
}

/// **The person marks the cluster pool `name` as their own, or takes the mark back** (#4071):
/// called only from their press in their window (`ui::WINDOW_ONLY`). Refused where no cluster
/// pool is called `name`. Written only where it changes anything.
pub fn set_cluster_owned(config_root: &Path, name: &str, owned: bool) -> Result<(), Refused> {
    let _locked = lock(config_root);
    let mut held = load(config_root)?;
    let Some(at) = held.clusters.iter().position(|pool| pool.name() == name) else {
        return Err(Refused::Other(format!(
            "no cluster pool named {name} is on this machine"
        )));
    };
    if held.clusters[at].owned == owned {
        return Ok(());
    }
    held.clusters[at].owned = owned;
    write(config_root, &held)
}

/// **Records the cluster pool `pool`, once** (#3292), under `_locked`: refused where a pool or
/// runner of its name is kept already.
pub fn record_cluster_pool_locked(
    config_root: &Path,
    pool: &ClusterPool,
    _locked: &Locked,
) -> Result<(), Refused> {
    let mut held = load(config_root)?;
    let name = pool.name();
    if held.names(name) {
        return Err(Refused::Other(format!(
            "a pool or a runner named {name} is on this machine already; name the pool \
             otherwise"
        )));
    }
    held.clusters.push(pool.clone());
    write(config_root, &held)
}

/// **Keeps `kept`, the runner of the cluster pool of its name, once** (#3292), under `_locked`,
/// for the desktop that is `ours`: refused where its pool is not kept, where a runner of its
/// name is kept already ([`Refused::KeyChanged`] where it was another), and where its device id
/// or key is the desktop's own or another kept runner's.
pub fn keep_cluster_runner_locked(
    config_root: &Path,
    kept: &ClusterKept,
    ours: &Pinned,
    _locked: &Locked,
) -> Result<(), Refused> {
    let mut held = load(config_root)?;
    let name = kept.name.as_str();
    if !held.clusters.iter().any(|pool| pool.name() == name) {
        return Err(Refused::Other(format!(
            "no cluster pool named {name} is on this machine"
        )));
    }
    if let Some(there) = held.cluster_runners.iter().find(|one| one.name == name) {
        return if there.claimed == kept.claimed
            && there.pod == kept.pod
            && same_pin(&there.pinned, &kept.pinned)
        {
            Ok(())
        } else {
            Err(Refused::KeyChanged {
                name: name.to_owned(),
            })
        };
    }
    let answered = &kept.pinned;
    if same_device(&answered.device, &ours.device) || answered.key == ours.key {
        return Err(Refused::Other(format!(
            "runner {name} answered with this machine's own device id or link key, so purlis \
             did not keep it; remove the pool and add it again"
        )));
    }
    if let Some((other, _)) = held.pins().find(|(_, pinned)| {
        same_device(&pinned.device, &answered.device) || pinned.key == answered.key
    }) {
        return Err(Refused::Other(format!(
            "runner {name} answered with the device id or key of runner {other}, so purlis did \
             not keep it; remove the pool and add it again"
        )));
    }
    held.cluster_runners.push(kept.clone());
    write(config_root, &held)
}

/// Forgets the cluster runner kept as `name`, under `_locked`; nothing where none is.
pub fn forget_cluster_runner_locked(
    config_root: &Path,
    name: &str,
    _locked: &Locked,
) -> Result<(), Refused> {
    let mut held = load(config_root)?;
    let before = held.cluster_runners.len();
    held.cluster_runners.retain(|kept| kept.name != name);
    if held.cluster_runners.len() != before {
        write(config_root, &held)?;
    }
    Ok(())
}

/// **The programs every cluster pool's kubectl runs** (#3292), each as named and by its real
/// path, once: what a chat's sandbox holds whole, with the package each belongs to, for no chat
/// to write. None where the file cannot be read.
pub fn cluster_holds(config_root: &Path) -> Vec<std::path::PathBuf> {
    let mut held: Vec<std::path::PathBuf> = Vec::new();
    for pool in cluster_pools(config_root).unwrap_or_default() {
        for program in pool.holds() {
            if !held.contains(program) {
                held.push(program.clone());
            }
        }
    }
    held
}

/// **Where every cluster pool's kubectl keeps its caches** (`--cache-dir`, #3292): a folder of
/// this user's alone beside the pools' own copies of their contexts, made where it is not.
pub fn kubectl_cache(config_root: &Path) -> Result<std::path::PathBuf, Refused> {
    let at = crate::machine::dir(config_root)
        .join(crate::machine::CLUSTER_KUBECONFIGS_DIR)
        .join(KUBECTL_CACHE);
    purlis_same_user::PrivateDirectory::open(&at).map_err(|why| {
        Refused::Other(format!("kubectl's cache folder could not be made: {why}"))
    })?;
    Ok(at)
}

/// The cache folder's name in [`crate::machine::CLUSTER_KUBECONFIGS_DIR`]: no pool's copy, which
/// ends in `.yaml`, is called so.
const KUBECTL_CACHE: &str = "cache";

/// **Where the cluster pool `name` keeps its own copy of its kubeconfig's context** (#3292):
/// `<pool>.yaml` in [`crate::machine::CLUSTER_KUBECONFIGS_DIR`].
pub fn kubeconfig_kept_at(config_root: &Path, name: &str) -> std::path::PathBuf {
    crate::machine::dir(config_root)
        .join(crate::machine::CLUSTER_KUBECONFIGS_DIR)
        .join(format!("{name}.yaml"))
}

/// **Keeps `context`, a kubeconfig of one context, as the cluster pool `name`'s own copy**
/// (#3292), `0600` in a folder of this user's alone, replacing whole any it had: every run of
/// its kubectl is pointed at it from then on, whatever the kubeconfig it was taken from says
/// later. Where it is.
pub fn keep_kubeconfig(
    config_root: &Path,
    name: &str,
    context: &str,
) -> Result<std::path::PathBuf, Refused> {
    usable_pool_name(name).map_err(Refused::Other)?;
    let at = kubeconfig_kept_at(config_root, name);
    let folder = at.parent().map(Path::to_path_buf).unwrap_or_default();
    purlis_same_user::PrivateDirectory::open(&folder)
        .and_then(|folder| folder.replace_file(&format!("{name}.yaml"), context.as_bytes()))
        .map_err(|why| {
            Refused::Other(format!(
                "pool {name}'s copy of its kubeconfig could not be kept: {why}"
            ))
        })?;
    Ok(at)
}

/// **Removes the cluster pool `name`'s own copy of its kubeconfig** (#3292); nothing where
/// there is none.
pub fn forget_kubeconfig(config_root: &Path, name: &str) -> Result<(), Refused> {
    match std::fs::remove_file(kubeconfig_kept_at(config_root, name)) {
        Ok(()) => Ok(()),
        Err(why) if why.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(why) => Err(Refused::Other(format!(
            "pool {name}'s copy of its kubeconfig could not be removed: {why}"
        ))),
    }
}

/// **Keeps the cluster pool `name`'s pin as `pin`** (#3305), as [`PoolPin::follow`] moved it
/// before a link: the pool's runner is linked with it from now on. Nothing where the pool is not
/// kept, or pins it already.
pub fn keep_cluster_pin(config_root: &Path, name: &str, pin: &PoolPin) -> Result<(), Refused> {
    let _locked = lock(config_root);
    let mut held = load(config_root)?;
    let Some(at) = held.clusters.iter().position(|pool| pool.name() == name) else {
        return Ok(());
    };
    if held.clusters[at].pin().as_ref() == Ok(pin) {
        return Ok(());
    }
    held.clusters[at] = held.clusters[at].repinned(pin);
    write(config_root, &held)
}

/// **Keeps whether the cluster pool `name`'s Pool opts in to minting now** (#4330): a Pool may opt in
/// after it was added. Read only to say whether a chat's repos are sent to the Claim, so it decides
/// nothing the cluster's own policy and operator do not.
pub fn keep_cluster_minting(config_root: &Path, name: &str, opted_in: bool) -> Result<(), Refused> {
    let _locked = lock(config_root);
    let mut held = load(config_root)?;
    let Some(at) = held.clusters.iter().position(|pool| pool.name() == name) else {
        return Ok(());
    };
    if held.clusters[at].mints_in_cluster() == opted_in {
        return Ok(());
    }
    held.clusters[at] = held.clusters[at].clone().minting_in_cluster(opted_in);
    write(config_root, &held)
}

/// A cluster pool's row (#3292): `{"name", "kind": "cluster", "kubeconfig", "context", "pool",
/// "namespace", "operatorKey", "cluster", "holds", "owned", "mintInCluster", "profiles"}`,
/// `cluster` left out only of a pool kept before its cluster was pinned (#3305), `owned` and
/// `mintInCluster` `true` alone, left out otherwise (#4071, #4204), and `profiles` as a Docker
/// pool's, left out where none were chosen (#3819).
fn cluster_pool_to_row(pool: &ClusterPool) -> serde_json::Value {
    let mut row = serde_json::json!({
        "name": pool.name(),
        "kind": CLUSTER,
        "kubeconfig": pool.kubeconfig().display().to_string(),
        "context": pool.context(),
        "pool": pool.pool(),
        "namespace": pool.namespace(),
        "operatorKey": pool.operator().spelled(),
        "holds": pool
            .holds()
            .iter()
            .map(|held| held.display().to_string())
            .collect::<Vec<_>>(),
    });
    if let Some(cluster) = &pool.cluster {
        row["cluster"] = cluster.as_str().into();
    }
    if pool.owned {
        row["owned"] = true.into();
    }
    if pool.mint_in_cluster {
        row["mintInCluster"] = true.into();
    }
    if let Some(profiles) = pool.profiles() {
        row["profiles"] = profiles.into();
    }
    pool.chosen.to_row(&mut row);
    row
}

/// [`cluster_pool_to_row`]'s row read, exactly those keys, each as it writes them.
fn cluster_pool_from_row(row: &serde_json::Value) -> Option<ClusterPool> {
    let fields = row.as_object()?;
    let known = [
        "name",
        "kind",
        "kubeconfig",
        "context",
        "pool",
        "namespace",
        "operatorKey",
        "cluster",
        "holds",
        "owned",
        "mintInCluster",
        "profiles",
    ];
    if fields
        .keys()
        .any(|key| !known.contains(&key.as_str()) && crate::poollimits::of_key(key).is_none())
    {
        return None;
    }
    let chosen = crate::poollimits::Chosen::from_row(fields).ok()?;
    let profiles = match fields.get("profiles") {
        None => None,
        Some(profiles) => Some(profiles_from_row(profiles)?),
    };
    // `true` alone: a mark spelled any other way is no mark, and the row is refused.
    let owned = match fields.get("owned") {
        None => false,
        Some(serde_json::Value::Bool(true)) => true,
        Some(_) => return None,
    };
    let mint_in_cluster = match fields.get("mintInCluster") {
        None => false,
        Some(serde_json::Value::Bool(true)) => true,
        Some(_) => return None,
    };
    let holds = fields
        .get("holds")?
        .as_array()?
        .iter()
        .map(|held| held.as_str().map(std::path::PathBuf::from))
        .collect::<Option<Vec<_>>>()?;
    let text = |key: &str| fields.get(key).and_then(serde_json::Value::as_str);
    // Every key but `cluster` and `owned` is there.
    if known[..7].iter().any(|key| text(key).is_none()) {
        return None;
    }
    let cluster = match fields.get("cluster") {
        None => None,
        Some(cluster) => Some(cluster.as_str()?.to_owned()),
    };
    ClusterPool::held(
        text("name")?,
        Path::new(text("kubeconfig")?),
        text("context")?,
        text("pool")?,
        text("namespace")?,
        OperatorKey::from_spelled(text("operatorKey")?).ok()?,
        cluster,
    )
    .and_then(|pool| pool.holding(holds))
    .map(|pool| ClusterPool {
        owned,
        mint_in_cluster,
        profiles,
        chosen: Box::new(chosen),
        ..pool
    })
    .ok()
}

/// A cluster runner's row (#3292): `{"name", "pool": "cluster", "namespace", "claim",
/// "claimUid", "requester", "pod", "device", "key", "paired"}`.
fn cluster_runner_to_row(kept: &ClusterKept) -> serde_json::Value {
    serde_json::json!({
        "name": kept.name,
        "pool": CLUSTER,
        "namespace": kept.claimed.namespace,
        "claim": kept.claimed.name,
        "claimUid": kept.claimed.uid,
        "requester": kept.claimed.requester,
        "pod": kept.pod,
        "device": kept.pinned.device,
        "key": crate::runnerlink::hex_of(kept.pinned.key.as_bytes()),
        "paired": kept.paired,
    })
}

/// [`cluster_runner_to_row`]'s row read, exactly those keys, each as it writes them.
fn cluster_runner_from_row(row: &serde_json::Value) -> Option<ClusterKept> {
    let fields = row.as_object()?;
    let known = [
        "name",
        "pool",
        "namespace",
        "claim",
        "claimUid",
        "requester",
        "pod",
        "device",
        "key",
        "paired",
    ];
    if fields.len() != known.len() || fields.keys().any(|key| !known.contains(&key.as_str())) {
        return None;
    }
    let text = |key: &str| fields.get(key).and_then(serde_json::Value::as_str);
    let name = text("name")?;
    usable_pool_name(name).ok()?;
    let (namespace, claim, uid, pod) = (
        text("namespace")?,
        text("claim")?,
        text("claimUid")?,
        text("pod")?,
    );
    let requester = text("requester")?;
    let named = clusterpool::is_label(namespace)
        && clusterpool::is_subdomain(claim)
        && clusterpool::is_uid(uid)
        && clusterpool::is_subdomain(pod)
        && plain_text(requester)
        && requester.len() <= 512;
    if !named {
        return None;
    }
    let pinned = crate::runnerlink::from_line(
        &serde_json::json!({ "device": text("device")?, "key": text("key")? }).to_string(),
    )
    .ok()?;
    Some(ClusterKept {
        name: name.to_owned(),
        claimed: Claimed {
            namespace: namespace.to_owned(),
            name: claim.to_owned(),
            uid: uid.to_owned(),
            requester: requester.to_owned(),
        },
        pod: pod.to_owned(),
        pinned,
        paired: fields.get("paired")?.as_u64()?,
    })
}

/// A runner's hours not written are said, and change nothing else (#3492).
fn record_hours(written: Result<(), String>) {
    if let Err(why) = written {
        tracing::warn!("purlis: {why}");
    }
}

pub(crate) fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|since| since.as_secs())
        .unwrap_or_default()
}

#[cfg(test)]
pub(crate) mod tests;
