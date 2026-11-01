// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A Docker pool on this machine** (spec #2041, #2044): each runner is a container of the
//! runner image (`docs/runner-image.md`), whose `purlis serve` is the runner's session host,
//! reached through `docker exec -i` (ADR 0078 §2's container preset) and linked like any runner.
//!
//! **Every runner container is confined the same way, and nothing loosens it** ([`Runner`]):
//!
//! - Docker's default seccomp profile with what purlis needs added ([`SECCOMP_PROFILE`]):
//!   `pidfd_getfd`, which the Linux sandbox's supervisor takes a chat's socket with to make its
//!   `listen` (#3410), and what the browser engine's own sandbox needs to start (#3298); the
//!   runtime's default AppArmor profile; and never `--privileged`;
//! - every capability dropped (`--cap-drop ALL`), none added, and no new privileges
//!   (`no-new-privileges`), so nothing in it gains a power its user does not have;
//! - a user who is not root, by number ([`User`]);
//! - the image by its digest alone ([`Image`]), never by a tag;
//! - its home on a volume of its own, and no folder, device or socket of this machine;
//! - its own network namespace on Docker's default bridge, never the machine's;
//! - a read-only root file system, so its home volume is the only place it can write that may
//!   hold a program it runs: `/tmp` is a `tmpfs` (`nosuid`, `nodev`, `noexec`), and Docker's
//!   own `/dev/shm` is memory-backed and `noexec` too, a quarter of the runner's memory, which
//!   the browser engine shares its pages' memory through (#3298);
//! - the pool's limits ([`Limits`]): CPUs, memory with no swap beyond it, processes, and the
//!   size of `/tmp`.
//!
//! purlis's Linux sandbox needs nothing more (`docs/runner-image.md`, #1040).
//!
//! **A container is checked before it is paired and before every link** ([`check`]): what
//! `docker inspect` says of it must be exactly the confinement above, for the container, the
//! image and the user pinned when it was made. Anything else, a field missing or one that
//! cannot be read included, refuses the link.
//!
//! **What each engine writes is measured** (#2680, `.github/workflows/docker-pool-hosts.yml`):
//! Docker on an AppArmor host, and Podman, rootful and rootless, each write the same
//! confinement in words of their own, recorded in `dockerpool/hosts/`, and the check holds each
//! to its own. An SELinux host is not measured: what it writes (a process label) is refused.
//!
//! **Each runner has its own link key, pinned** ([`pair_args`]): the runner makes its key at
//! its one pairing, inside the container, and the desktop keeps its public half with the
//! runner's device id; a new container is a new runner with a new key.

use std::fmt;

/// The label every runner container of the pool carries: its runner's name.
pub const LABEL: &str = "purlis.runner";

/// **The label naming the desktop that made a runner's container** (#2869): what tells this
/// desktop's leftovers from another's on a Docker daemon two config homes share. Internal: a
/// hash of the config home, never a path.
pub const DESKTOP_LABEL: &str = "purlis.desktop";

/// Every runner container's and volume's name starts with this.
pub const PREFIX: &str = "purlis-runner-";

/// Where `purlis` is in a runner image (`docs/runner-image.md`).
pub const PURLIS: &str = "/usr/local/bin/purlis";

/// **The shell script a drained runner's sign-ins are carried with** (#4011, #4055): `sh -c`
/// hands it the old home, the new home, the folder under the new home they go to where its data
/// home has the purlis spelling ([`crate::profilestore::carried_under_a_runner_home`]), the one
/// where it does not ([`crate::profilestore::carried_under_an_old_runner_home`]), so a running
/// runner's data home never moves under it, and each `<from>=<to>` pair
/// ([`crate::profilestore::carried_in_a_runner_home`]). A `*` in a pair is a profile's name:
/// each one found under the old home goes to the same name. The new runner serves chats as it
/// runs, so nothing is written where they could have put something first: the folder is walked
/// to one folder at a time, never through a link, and held by its real path, and the files go
/// into a batch folder of its own that no one knew the name of before it was made, each written
/// only where nothing is (`set -C`) and put in place whole, by a link. Only a plain file inside
/// the old home is carried, never a link or what a folder linked out of it holds. An image that
/// names its data home or Codex's by a variable carries nothing, since the runner reads it
/// elsewhere. A harness's default folder is carried only from a runner that left
/// [`crate::profilestore::DEFAULTS_HELD`]. What
/// it makes is the runner's user's alone. A file that cannot be carried is said and passed over:
/// the profile is then signed in again, as before any was carried, and the drain goes on. A
/// profile named in the colon-separated list after the two folders is not carried at all (#4145).
pub const COPY_PROFILES: &str = r#"set -u
umask 077
[ -z "${PURLIS_DATA_HOME:-}${XDG_DATA_HOME:-}${CODEX_HOME:-}" ] || exit 1
from=$1 to=$2 stage=$3 old_stage=$4 skip=$5
shift 5
from=$(cd -P "$from" && pwd -P) || exit 1
held=
for mark in "$from"/.local/share/*/profiles/.default-folders-held; do
  [ -f "$mark" ] && [ ! -L "$mark" ] && held=1
done
cd -P "$to" || exit 1
here=$(pwd -P)
data=${stage%/*/*}
if [ -L "$data" ] || { [ -e "$data" ] && [ ! -d "$data" ]; }; then exit 1; fi
[ -d "$data" ] || stage=$old_stage
set -f
IFS=/
for part in $stage; do
  [ -n "$part" ] || continue
  [ ! -L "$part" ] || exit 1
  [ -d "$part" ] || mkdir "$part" || exit 1
  cd -P "$part" || exit 1
  here=$here/$part
  [ "$(pwd -P)" = "$here" ] || exit 1
done
unset IFS
set +f
batch=$(mktemp -d ./XXXXXXXX) || exit 1
batch=${batch#./}
cd -P "$batch" || exit 1
[ "$(pwd -P)" = "$here/$batch" ] || exit 1
set -C
for pair in "$@"; do
  src=${pair%%=*} dst=${pair#*=}
  case $src in *'*'*) ;; *) [ -n "$held" ] || continue ;; esac
  for found in "$from"/$src; do
    rel=${found#"$from/"}
    [ -f "$found" ] && [ ! -L "$found" ] && [ -r "$found" ] || continue
    [ "$(realpath -- "$found")" = "$from/$rel" ] || continue
    out=$dst
    case $src in
      *'*'*)
        name=${rel#"${src%%\**}"}
        name=${name%"${src#*\*}"}
        case $name in ''|*/*|.*) continue ;; esac
        out=${dst%%\**}$name${dst#*\*}
        ;;
    esac
    case ":$skip:" in *":${out%%/*}:"*) continue ;; esac
    { mkdir -p "${out%/*}" && [ ! -e "$out" ] && [ ! -L "$out" ] && cat "$found" > "$out.part" \
        && ln "$out.part" "$out"; } || echo "not carried: $rel" >&2
    rm -f "$out.part"
  done
done
cd .. && rmdir "$batch" 2>/dev/null
exit 0
"#;

/// How long carrying a runner's sign-ins may take before it is ended (#4011): a few small files.
pub const CARRIED_WITHIN: std::time::Duration = std::time::Duration::from_secs(120);

/// The longest runner name: a pool's longest, and the dash and eight hex digits a runner made
/// beside it after an update adds ([`successor_name`], #2749).
const MOST_NAME: usize = 49;

/// **purlis's own runner image** (`docs/runner-image.md`): the one image a pool is moved along
/// with the desktop's version after an update (#2749). A team's own image never is.
pub const PURLIS_RUNNER_IMAGE: &str = "ghcr.io/purlis/purlis-runner";

/// **The name of the runner made beside `pool`'s after an update to `image`** (#2749): the
/// pool's name, a dash and the first eight hex digits of the image's digest.
pub fn successor_name(pool: &str, image: &Image) -> String {
    format!("{pool}-{}", &image.digest()[..8])
}

/// **The name of a runner made again for `pool`** (#4149), after its runner was removed outside
/// purlis: the pool's name, a dash and eight hex digits drawn at random, as long as a name made
/// after an update ([`successor_name`]), so it fits wherever that one does.
pub fn remade_name(pool: &str) -> Result<String, Refused> {
    let mut drawn = [0u8; 4];
    getrandom::fill(&mut drawn)
        .map_err(|why| refused(format!("purlis could not draw a runner's name: {why}")))?;
    let hex: String = drawn.iter().map(|byte| format!("{byte:02x}")).collect();
    Ok(format!("{pool}-{hex}"))
}

/// Why a runner container was not made, checked or linked, in words a person reads.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Refused(pub String);

impl fmt::Display for Refused {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for Refused {}

fn refused(why: impl Into<String>) -> Refused {
    Refused(why.into())
}

/// A runner image named by its digest: `<name>[:<tag>]@sha256:<64 hex>`. A tag can move, so
/// a pool names the image it runs by digest alone.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Image(String);

impl Image {
    /// `reference`, when it names its image by a SHA-256 digest of lower-case hex, and its name
    /// is a registry path of the characters one may hold.
    pub fn pinned(reference: &str) -> Result<Image, Refused> {
        let not_pinned = || {
            refused(format!(
                "the runner image \"{reference}\" is not named by its digest; name it as \
                 <image>@sha256:<digest>"
            ))
        };
        let (name, digest) = reference.rsplit_once('@').ok_or_else(not_pinned)?;
        let hex = digest.strip_prefix("sha256:").ok_or_else(not_pinned)?;
        if !crate::runnerlink::is_lower_hex(hex, 64) {
            return Err(not_pinned());
        }
        let first = name.bytes().next().ok_or_else(not_pinned)?;
        let allowed = |b: u8| b.is_ascii_lowercase() || b.is_ascii_digit() || b"._-/:".contains(&b);
        if !first.is_ascii_alphanumeric() || !name.bytes().all(allowed) {
            return Err(not_pinned());
        }
        Ok(Image(reference.to_owned()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Its name, without its tag or digest: `ghcr.io/purlis/purlis-runner`.
    pub fn name(&self) -> &str {
        let named = self
            .0
            .rsplit_once('@')
            .map_or(self.0.as_str(), |(name, _)| name);
        // A tag follows the last `:` after the last `/`; a registry's port comes before it.
        match named.rsplit_once(':') {
            Some((name, tag)) if !tag.contains('/') => name,
            _ => named,
        }
    }

    /// Its digest's 64 hex digits.
    pub fn digest(&self) -> &str {
        self.0
            .rsplit_once("@sha256:")
            .map_or("", |(_, digest)| digest)
    }

    /// **Whether it is purlis's own runner image** ([`PURLIS_RUNNER_IMAGE`], #2749), by name.
    pub fn is_purlis_own(&self) -> bool {
        self.name() == PURLIS_RUNNER_IMAGE
    }
}

/// **The version `purlis --version` says in `line`** (#2749): `purlis <version> (<profile>
/// profile)`, or nothing where the line is not one.
pub fn version_in(line: &str) -> Option<&str> {
    let rest = line.trim().strip_prefix("purlis ")?;
    let (version, profile) = rest.split_once(' ')?;
    (profile.starts_with('(') && profile.ends_with(" profile)") && !version.is_empty())
        .then_some(version)
}

/// The user a runner container runs as, by number: never root's user or group.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct User {
    uid: u32,
    gid: u32,
}

impl User {
    pub fn new(uid: u32, gid: u32) -> Result<User, Refused> {
        if uid == 0 || gid == 0 {
            return Err(refused(
                "a runner never runs as root: its image's user must be another user and group",
            ));
        }
        Ok(User { uid, gid })
    }

    /// `uid:gid`, as `docker run --user` and `docker inspect` spell it.
    pub fn spelled(&self) -> String {
        format!("{}:{}", self.uid, self.gid)
    }

    pub fn uid(&self) -> u32 {
        self.uid
    }
}

/// What a runner of the pool may use (spec #2041: a pool's limits, configurable per pool): its
/// CPUs, in thousandths; its memory, in MiB, with no swap beyond it; how many processes it may
/// run; and the size of its `/tmp`, in MiB, which counts against its memory.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Limits {
    millicpus: u32,
    memory_mib: u64,
    pids: u32,
    tmp_mib: u64,
}

impl Default for Limits {
    /// Two CPUs, 4 GiB of memory, 1024 processes and 1 GiB of `/tmp`.
    fn default() -> Limits {
        Limits {
            millicpus: 2000,
            memory_mib: 4096,
            pids: 1024,
            tmp_mib: 1024,
        }
    }
}

impl Limits {
    /// The fewest processes a runner may be limited to: its host, a chat, its harness and its
    /// sandbox's supervisor, with room for a build.
    pub const FEWEST_PIDS: u32 = 64;
    /// The least CPU a runner may be limited to, in thousandths: Docker's own least, 0.01.
    pub const FEWEST_MILLICPUS: u32 = 10;
    /// The least memory a runner may be limited to, in MiB.
    pub const LEAST_MEMORY_MIB: u64 = 512;

    /// A pool's limits: each at least what a runner needs to start a chat, and `/tmp` no larger
    /// than the memory it counts against.
    pub fn new(
        millicpus: u32,
        memory_mib: u64,
        pids: u32,
        tmp_mib: u64,
    ) -> Result<Limits, Refused> {
        if !(Limits::FEWEST_MILLICPUS..=1_000_000).contains(&millicpus) {
            return Err(refused(
                "a pool's CPU limit is at least 0.01 CPU and at most 1000 CPUs",
            ));
        }
        if !(Limits::LEAST_MEMORY_MIB..=16 * 1024 * 1024).contains(&memory_mib) {
            return Err(refused(format!(
                "a pool's memory limit is at least {} MiB and at most 16 TiB",
                Limits::LEAST_MEMORY_MIB
            )));
        }
        if pids < Limits::FEWEST_PIDS {
            return Err(refused(format!(
                "a pool's process limit is at least {}",
                Limits::FEWEST_PIDS
            )));
        }
        if tmp_mib == 0 || tmp_mib > memory_mib {
            return Err(refused(
                "a pool's /tmp is more than none and no larger than its memory limit",
            ));
        }
        Ok(Limits {
            millicpus,
            memory_mib,
            pids,
            tmp_mib,
        })
    }

    /// Its CPUs, in thousandths.
    pub fn millicpus(&self) -> u32 {
        self.millicpus
    }

    /// Its memory, in MiB.
    pub fn memory_mib(&self) -> u64 {
        self.memory_mib
    }

    /// How many processes it may run.
    pub fn pids(&self) -> u32 {
        self.pids
    }

    /// The size of its `/tmp`, in MiB.
    pub fn tmp_mib(&self) -> u64 {
        self.tmp_mib
    }

    /// `--cpus`'s value: whole CPUs, with thousandths where there are any.
    fn cpus(&self) -> String {
        let (whole, part) = (self.millicpus / 1000, self.millicpus % 1000);
        if part == 0 {
            whole.to_string()
        } else {
            format!("{whole}.{part:03}")
        }
    }

    fn memory_bytes(&self) -> u64 {
        self.memory_mib * 1024 * 1024
    }

    /// What `--tmpfs` mounts at `/tmp`, as `docker inspect` writes it back.
    fn tmp_options(&self) -> String {
        format!("rw,nosuid,nodev,noexec,size={}m", self.tmp_mib)
    }

    /// **The size of a runner's `/dev/shm`, in MiB** (#3298): a quarter of its memory. The
    /// browser engine shares its pages' memory through it, and Docker's own 64 MiB is too
    /// little for a busy page, which then ends. It is memory-backed and counted against the
    /// runner's memory, so it gives the runner nothing past its limit.
    pub fn shm_mib(&self) -> u64 {
        self.memory_mib / 4
    }
}

/// **How long a pool's runner is left without a link from its desktop before it suspends
/// itself, by default** (#2051): a day, as the host's own `UNLINKED_FOR`. A pool sets its own
/// (`suspendAfter`, #3471) within [`LEAST_SUSPEND_AFTER`] and [`MOST_SUSPEND_AFTER`].
pub const SUSPEND_AFTER: std::time::Duration = std::time::Duration::from_secs(24 * 60 * 60);

/// The shortest a pool's runner may be left without a link before it suspends: a minute, so a
/// link that drops and comes back does not suspend it.
pub const LEAST_SUSPEND_AFTER: std::time::Duration = std::time::Duration::from_secs(60);

/// The longest a pool's runner may be left without a link before it suspends: 30 days.
pub const MOST_SUSPEND_AFTER: std::time::Duration =
    std::time::Duration::from_secs(30 * 24 * 60 * 60);

/// `duration`, where it is a time a pool's runner may be left without a link before it
/// suspends: whole seconds, [`LEAST_SUSPEND_AFTER`] to [`MOST_SUSPEND_AFTER`].
pub fn suspend_after(duration: std::time::Duration) -> Result<std::time::Duration, Refused> {
    if duration.subsec_nanos() == 0
        && (LEAST_SUSPEND_AFTER..=MOST_SUSPEND_AFTER).contains(&duration)
    {
        Ok(duration)
    } else {
        Err(refused(
            "a pool's runner suspends after at least a minute and at most 30 days without a link",
        ))
    }
}

/// One runner of the pool, as it is made: its name, its image, its user, its home, its
/// pool's limits, and how long it is left without a link before it suspends itself.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Runner {
    name: String,
    image: Image,
    user: User,
    home: String,
    limits: Limits,
    suspends_after: std::time::Duration,
}

impl Runner {
    /// A runner called `name` (lower-case letters, digits and dashes, at most 49, starting and
    /// ending with a letter or digit), of `image`, run as `user`, whose home is `home` (the
    /// image's `HOME`: an absolute path of plain names).
    pub fn new(name: &str, image: Image, user: User, home: &str) -> Result<Runner, Refused> {
        let plain = |b: u8| b.is_ascii_lowercase() || b.is_ascii_digit();
        let named = !name.is_empty()
            && name.len() <= MOST_NAME
            && name.bytes().all(|b| plain(b) || b == b'-')
            && name.bytes().next().is_some_and(plain)
            && name.bytes().last().is_some_and(plain);
        if !named {
            return Err(refused(format!(
                "\"{name}\" cannot name a runner: use lower-case letters, digits and dashes, \
                 at most {MOST_NAME}"
            )));
        }
        let segments: Vec<&str> = home.split('/').skip(1).collect();
        let plain_path = home.starts_with('/')
            && !segments.is_empty()
            && segments.iter().all(|segment| {
                !segment.is_empty()
                    && *segment != "."
                    && *segment != ".."
                    && segment
                        .bytes()
                        .all(|b| b.is_ascii_alphanumeric() || b"._-".contains(&b))
            });
        if !plain_path {
            return Err(refused(format!(
                "the runner image's home \"{home}\" is not an absolute path of plain names"
            )));
        }
        Ok(Runner {
            name: name.to_owned(),
            image,
            user,
            home: home.to_owned(),
            limits: Limits::default(),
            suspends_after: SUSPEND_AFTER,
        })
    }

    /// This runner, held to its pool's `limits` in place of the defaults.
    pub fn limited(self, limits: Limits) -> Runner {
        Runner { limits, ..self }
    }

    pub fn limits(&self) -> Limits {
        self.limits
    }

    /// This runner, suspending itself after `after` without a link from its desktop, its
    /// pool's `suspendAfter` (#3471), in place of [`SUSPEND_AFTER`].
    pub fn suspending_after(self, after: std::time::Duration) -> Runner {
        Runner {
            suspends_after: after,
            ..self
        }
    }

    /// How long it is left without a link from its desktop before it suspends itself.
    pub fn suspends_after(&self) -> std::time::Duration {
        self.suspends_after
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn image(&self) -> &Image {
        &self.image
    }

    pub fn user(&self) -> User {
        self.user
    }

    /// The image's `HOME`, where the runner's home volume is mounted.
    pub fn home(&self) -> &str {
        &self.home
    }

    /// The container's name.
    pub fn container(&self) -> String {
        format!("{PREFIX}{}", self.name)
    }

    /// The volume that holds the container's home.
    pub fn volume(&self) -> String {
        format!("{PREFIX}{}-home", self.name)
    }

    /// `docker`'s arguments that make and start the runner's container, confined as the
    /// module says, with [`SECCOMP_PROFILE`] read from `seccomp`. Its id is what `docker`
    /// prints.
    pub fn create_args(&self, seccomp: &std::path::Path) -> Vec<String> {
        self.create_args_for(None, seccomp)
    }

    /// [`Runner::create_args`], the container also labelled as made by `desktop`
    /// ([`DESKTOP_LABEL`], #2869), where one is named.
    pub fn create_args_for(&self, desktop: Option<&str>, seccomp: &std::path::Path) -> Vec<String> {
        let mut args: Vec<String> = Vec::new();
        let mut pair = |flag: &str, value: String| {
            args.push(flag.to_owned());
            args.push(value);
        };
        pair("--name", self.container());
        pair("--label", format!("{LABEL}={}", self.name));
        if let Some(desktop) = desktop {
            pair("--label", format!("{DESKTOP_LABEL}={desktop}"));
        }
        // Said back to the person if a declared tool's program is not in the image (#4354).
        pair(
            "--env",
            format!(
                "{}={}",
                crate::sandbox::tools::POOL_IMAGE_VAR,
                self.image.as_str()
            ),
        );
        pair("--user", self.user.spelled());
        pair("--cap-drop", "ALL".into());
        pair("--security-opt", "no-new-privileges".into());
        pair("--security-opt", format!("seccomp={}", seccomp.display()));
        pair("--network", "bridge".into());
        pair("--ipc", "private".into());
        pair("--shm-size", format!("{}m", self.limits.shm_mib()));
        pair("--cgroupns", "private".into());
        pair(
            "--mount",
            format!("type=volume,source={},target={}", self.volume(), self.home),
        );
        pair("--tmpfs", format!("/tmp:{}", self.limits.tmp_options()));
        pair("--cpus", self.limits.cpus());
        pair("--memory", format!("{}m", self.limits.memory_mib));
        pair("--memory-swap", format!("{}m", self.limits.memory_mib));
        pair("--pids-limit", self.limits.pids.to_string());
        args.push("--read-only".to_owned());
        // The `purlis` the check asks whether it holds its programs from namespaces is the one
        // that serves (D-2050-26, #3702): whatever the image's own entrypoint, with the runtime's
        // init as PID 1 to reap what a chat's programs leave behind, as the image's `tini` did.
        args.push("--init".to_owned());
        args.push("--entrypoint".to_owned());
        args.push(PURLIS.to_owned());
        let mut all = vec!["run".to_owned(), "--detach".to_owned()];
        all.extend(args);
        all.push(self.image.as_str().to_owned());
        // It suspends itself after its pool's time without a link from its desktop, keeping its
        // home (#2051, #3471). The bare flag is the default, a day, so a runner image of a
        // purlis before #3471 starts as it did for a pool that keeps it.
        all.push("serve".to_owned());
        all.push(
            match crate::spelledtime::spelled(self.suspends_after, crate::spelledtime::UP_TO_HOURS)
            {
                Some(after) if self.suspends_after != SUSPEND_AFTER => {
                    format!("--suspends-unlinked={after}")
                }
                _ => "--suspends-unlinked".to_owned(),
            },
        );
        all
    }
}

/// A container's full id, as `docker run` prints it: 64 lower-case hex. A name can be taken
/// by another container once this one is gone; the id cannot.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContainerId(String);

impl ContainerId {
    pub fn new(id: &str) -> Result<ContainerId, Refused> {
        if crate::runnerlink::is_lower_hex(id, 64) {
            Ok(ContainerId(id.to_owned()))
        } else {
            Err(refused(format!("\"{id}\" is not a container's full id")))
        }
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Its first twelve digits, as Docker shows a container's id.
    pub fn short(&self) -> &str {
        &self.0[..12]
    }
}

/// A runner's container as it was made: what every check and every link holds it to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Made {
    pub container: ContainerId,
    pub runner: Runner,
}

/// How a runner's sign-in is run: in a terminal the person drives, or driven by purlis with no
/// terminal (#4042).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Typed {
    InATerminal,
    Driven,
    /// Driven by the person, in a terminal the runner's `purlis` makes of this size (#4043).
    InOwnTerminal(crate::engine::Size),
    /// In a terminal the person drives, by the harness's device-code flow: what is left where
    /// the browser's answer cannot reach the runner (#4044).
    DeviceInATerminal,
    /// [`Typed::InOwnTerminal`], by the harness's device-code flow: a login that waits for the
    /// browser's answer on the runner's loopback, which nothing carries from a terminal (#4044).
    DeviceInOwnTerminal(crate::engine::Size),
}

impl Typed {
    /// The flag `docker exec` and `kubectl exec` run it with: `-it` in a terminal the person
    /// drives, `-i` with pipes purlis drives.
    pub fn exec_flag(self) -> &'static str {
        match self {
            Typed::InATerminal | Typed::DeviceInATerminal => "-it",
            Typed::Driven | Typed::InOwnTerminal(_) | Typed::DeviceInOwnTerminal(_) => "-i",
        }
    }
}

/// **The runner's own words that sign its profile `name` in**, to `provider`'s plan where one is
/// named, by `typed`: what `docker exec` runs in a Docker runner's container and `kubectl exec`
/// in a cluster runner's pod (#3819), so both run the same sign-in, and a login over pipes ends
/// with its input on either (#4043).
pub fn sign_in_words(typed: Typed, name: &str, provider: Option<&str>) -> Vec<String> {
    let mut words: Vec<String> = [PURLIS, "profile", "sign-in", name]
        .iter()
        .map(|word| (*word).to_owned())
        .collect();
    if let Some(provider) = provider {
        words.extend(["--provider".to_owned(), provider.to_owned()]);
    }
    // A plan's login asks from a list in a terminal, which nothing answers when driven: a plan
    // whose login waits for the browser is told so, and only it, as a runner before #4044 knows
    // no `--way` (Copilot's device code is its login's default). Where the browser's answer
    // cannot reach the runner, the device-code flow is named (#4044).
    match typed {
        Typed::Driven
            if provider
                .is_some_and(crate::profilestore::sign_in::driven::plan_waits_for_the_browser) =>
        {
            words.extend(["--way".to_owned(), "browser".to_owned()]);
        }
        Typed::DeviceInATerminal | Typed::DeviceInOwnTerminal(_) => {
            words.extend(["--way".to_owned(), "device".to_owned()]);
        }
        Typed::InATerminal | Typed::Driven | Typed::InOwnTerminal(_) => {}
    }
    // Over pipes the login ends with its input (#4043), on a cluster runner too (#3819).
    match typed {
        Typed::InATerminal | Typed::DeviceInATerminal => {}
        Typed::Driven => words.push("--until-input-ends".to_owned()),
        Typed::InOwnTerminal(size) | Typed::DeviceInOwnTerminal(size) => words.extend([
            "--in-own-terminal".to_owned(),
            format!("{}x{}", size.columns, size.rows),
        ]),
    }
    words
}

impl Made {
    /// `docker`'s arguments for the connector (ADR 0078 §2's container preset): the bridge to
    /// the runner's host, run in the container pinned by id, as the runner's user and group
    /// by number, so no name in the image's own files decides who it runs as.
    pub fn connect_args(&self) -> Vec<String> {
        self.exec(&["bridge"])
    }

    /// `docker`'s arguments that pair the runner: the bridge's pairing, which takes the
    /// desktop's device id and key on its standard input and answers the runner's.
    pub fn pair_args(&self) -> Vec<String> {
        self.exec(&["bridge", "--pair"])
    }

    /// `docker`'s arguments that ask the runner's `purlis` its version (#2749).
    pub fn version_args(&self) -> Vec<String> {
        self.exec(&["--version"])
    }

    /// **The name of the container that carries sign-ins to this runner** (#4011): its own
    /// container's, with `-copy`, so what a carry that stopped short left is found with this
    /// runner's ([`Docker::remove_leftovers_of`], [`Docker::copy_profiles`]).
    pub fn copy_container(&self) -> String {
        format!("{}-copy", self.runner.container())
    }

    /// **`docker`'s arguments that carry `from`'s sign-ins into this runner's home** (#4011,
    /// ruled 2026-10-30): a container of this runner's image, named for it, labelled as made by
    /// `desktop` and removed once it ends, that mounts `from`'s home volume read-only and this
    /// runner's home volume, and runs [`COPY_PROFILES`] as this runner's user. It reaches no
    /// network, holds no capability and writes nothing but the new home, so nothing of the homes
    /// leaves the Docker host.
    ///
    /// The profiles in `skip`, signed out of the pool (#4145), are left where they are.
    pub fn copy_profiles_args(&self, from: &Made, desktop: &str, skip: &[String]) -> Vec<String> {
        let mut args: Vec<String> = [
            "run",
            "--rm",
            "--network",
            "none",
            "--name",
            &self.copy_container(),
            "--label",
            &format!("{LABEL}={}", self.runner.name()),
            "--label",
            &format!("{DESKTOP_LABEL}={desktop}"),
            "--user",
            &self.runner.user.spelled(),
            "--cap-drop",
            "ALL",
            "--security-opt",
            "no-new-privileges",
            "--read-only",
            "--pids-limit",
            "64",
            "--memory",
            "256m",
            "--cpus",
            "1",
            "--mount",
            &format!(
                "type=volume,source={},target=/from,readonly",
                from.runner.volume()
            ),
            "--mount",
            &format!("type=volume,source={},target=/to", self.runner.volume()),
            "--entrypoint",
            "/bin/sh",
            self.runner.image.as_str(),
            "-c",
            COPY_PROFILES,
            "sh",
            "/from",
            "/to",
            &crate::profilestore::carried_under_a_runner_home(),
            &crate::profilestore::carried_under_an_old_runner_home(),
            &skip.join(":"),
        ]
        .iter()
        .map(|word| (*word).to_owned())
        .collect();
        args.extend(crate::profilestore::carried_in_a_runner_home());
        args
    }

    /// `docker`'s arguments that run the runner's own `purlis profile sign-in` of its profile
    /// `name` (#2049), to `provider`'s plan where one is named, with a terminal the person drives:
    /// the harness's own sign-in, run on the runner in the runner's folder for that profile.
    pub fn sign_in_args(&self, name: &str, provider: Option<&str>) -> Vec<String> {
        self.sign_in_args_by(Typed::InATerminal, name, provider)
    }

    /// [`Made::sign_in_args`], by `typed`: in a terminal the person drives (`-it`), or with its
    /// input and output pipes (`-i`, #4042), for purlis to drive or to carry a terminal the
    /// runner's `purlis` makes. Over pipes the login ends with its input (#4043), since ending
    /// `docker exec` ends nothing in the container.
    pub fn sign_in_args_by(&self, typed: Typed, name: &str, provider: Option<&str>) -> Vec<String> {
        let mut args: Vec<String> = [
            "exec",
            typed.exec_flag(),
            "--user",
            &self.runner.user.spelled(),
            self.container.as_str(),
        ]
        .iter()
        .map(|word| (*word).to_owned())
        .collect();
        args.extend(sign_in_words(typed, name, provider));
        args
    }

    /// `docker`'s arguments that run the runner's own sign-out of its profile `name` (#4043).
    pub fn sign_out_args(&self, name: &str) -> Vec<String> {
        self.exec(&["profile", "sign-out", name])
    }

    /// [`Made::sign_out_args`], with a terminal the person answers in (#4124): for a harness whose
    /// sign-out asks which sign-in goes.
    pub fn sign_out_in_a_terminal_args(&self, name: &str) -> Vec<String> {
        let mut args = self.sign_out_args(name);
        args[1] = "-it".to_owned();
        args
    }

    fn exec(&self, words: &[&str]) -> Vec<String> {
        let mut args: Vec<String> = [
            "exec",
            "-i",
            "--user",
            &self.runner.user.spelled(),
            self.container.as_str(),
            PURLIS,
        ]
        .iter()
        .map(|word| (*word).to_owned())
        .collect();
        args.extend(words.iter().map(|word| (*word).to_owned()));
        args
    }
}

/// What the daemon's check says of the engine a container runs on (#2680).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Engine {
    /// A rootless Podman, which loads no AppArmor profile for a container.
    pub rootless: bool,
}

/// **Which seccomp profile a runner's container runs** (#3410), as the check found it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Seccomp {
    /// The runner's own, [`SECCOMP_PROFILE`]: a server a chat starts there listens where it
    /// was bound, and the browser engine makes its sandbox there.
    Runners,
    /// The runner's profile as a purlis made it from #3410 until #3298
    /// ([`EARLIER_SECCOMP_PROFILE`]): a server a chat starts there listens, and the browser
    /// engine cannot make its sandbox, so no browser starts there until the runner is removed
    /// and added again. It is stricter than the runner's own, so the runner is still linked.
    RunnersEarlier,
    /// The runtime's own default, which a runner made before #3410 runs. It is stricter than
    /// the runner's (it refuses `pidfd_getfd`), so the runner is still linked, and no server a
    /// chat starts there listens until the runner is removed and added again.
    RuntimeDefault,
}

/// Whether `inspected`, what `docker inspect <container>` printed, is `made`'s container,
/// running and confined exactly as [`Runner::create_args`] made it, on a rootful engine.
/// Anything else is refused, with what differs: a field missing, or of another shape, included.
pub fn check(inspected: &serde_json::Value, made: &Made) -> Result<(), Refused> {
    check_on(inspected, made, Engine::default()).map(drop)
}

/// [`check`], on the `engine` the daemon's check found ([`check_podman`]): the seccomp profile
/// the container runs, where it passes.
pub fn check_on(
    inspected: &serde_json::Value,
    made: &Made,
    engine: Engine,
) -> Result<Seccomp, Refused> {
    checked_on(inspected, made, engine, true)
}

/// The most kept of what a runner's sign-in check prints, on each of its outputs.
const STANDING_MOST: u64 = 64 * 1024;

/// How long a runner's sign-in check is waited for.
const STANDING_WITHIN: std::time::Duration = std::time::Duration::from_secs(180);

/// **What `purlis profile sign-in <name> --check` said of `name`, in `printed`**: signed in, not
/// signed in, or that its harness cannot tell, each by the words that command writes. `Err`, in
/// the runner's own words cut short, for anything else, such as a profile it does not keep.
pub fn standing_said(
    name: &str,
    printed: &str,
) -> Result<crate::profilestore::sign_in::Standing, String> {
    use crate::profilestore::sign_in::Standing;
    let (signed_in, signed_out) = (
        format!("'{name}' is signed in "),
        format!("'{name}' is not "),
    );
    for line in printed.lines() {
        if line.starts_with(&signed_in) {
            return Ok(Standing::SignedIn);
        }
        if line.starts_with(&signed_out) {
            return Ok(Standing::SignedOut);
        }
        if line.contains(&format!("cannot tell whether '{name}'")) {
            return Ok(Standing::CannotTell);
        }
    }
    let last = printed.lines().rfind(|line| !line.trim().is_empty());
    Err(format!(
        "the runner says: {}",
        crate::shown::short(last.unwrap_or("nothing"))
    ))
}

/// **A runner removed outside purlis** (#4149, [`Docker::gone`]): Docker lists no container
/// of its id. Whether its home volume is still there, whose sign-ins a runner made again takes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Gone {
    pub home_kept: bool,
}

/// **Why Docker lists no container of a runner's id** ([`Docker::missing`], #4166): it was
/// removed, or Docker now reaches another engine than the one that made the runner, whose
/// container may well be there still.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Missing {
    Removed(Gone),
    OnAnotherEngine,
}

/// What [`Docker::resumed`] did: what it read of the container first, and whether it started it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Resumed {
    pub before: Ran,
    pub started: bool,
}

/// Whether `inspected`, what `docker inspect <container>` printed, says the container runs.
pub fn is_running(inspected: &serde_json::Value) -> bool {
    inspected.pointer("/0/State/Running") == Some(&serde_json::Value::Bool(true))
}

/// **What Docker says of a container's latest run** (#3492): whether it runs, when it started,
/// and when its last run ended, each in seconds since the epoch and `None` where Docker gives
/// no time (a container never started says `0001-01-01T00:00:00Z`). The runner hours record
/// is written from these ([`crate::runnerhours::seen`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Ran {
    pub running: bool,
    pub started: Option<u64>,
    pub finished: Option<u64>,
}

/// [`Ran`], from what `docker inspect <container>` printed.
pub fn ran(inspected: &serde_json::Value) -> Ran {
    let at = |key: &str| {
        inspected
            .pointer(&format!("/0/State/{key}"))
            .and_then(serde_json::Value::as_str)
            .and_then(|text| chrono::DateTime::parse_from_rfc3339(text).ok())
            .and_then(|time| u64::try_from(time.timestamp()).ok())
            .filter(|secs| *secs > 0)
    };
    Ran {
        running: is_running(inspected),
        started: at("StartedAt"),
        finished: at("FinishedAt"),
    }
}

/// **The disk each volume holds, by its name** (#3493), from what `docker system df -v` printed
/// (`printed`): its `Local Volumes space usage` table, as Docker and Podman both write it, a
/// volume's name first on its line and its size last. A size is read as both write one (Go's
/// `units.HumanSize`, decimal: `0B`, `512kB`, `1.234GB`), in bytes; a line whose size is not
/// written so is left out, so nothing is guessed. Podman will not print this verbose table in
/// another format, so the table is read rather than `--format`.
pub fn homes_held(printed: &str) -> std::collections::BTreeMap<String, u64> {
    let mut held = std::collections::BTreeMap::new();
    let mut lines = printed
        .lines()
        .skip_while(|line| !line.trim_start().starts_with("Local Volumes space usage"))
        .skip(1)
        .skip_while(|line| line.trim().is_empty());
    // Its header.
    if !lines
        .next()
        .is_some_and(|line| line.trim_start().starts_with("VOLUME NAME"))
    {
        return held;
    }
    for line in lines.take_while(|line| !line.trim().is_empty()) {
        let words: Vec<&str> = line.split_whitespace().collect();
        // Its name, its links, its size.
        let [name, _, .., size] = words.as_slice() else {
            continue;
        };
        if let Some(bytes) = bytes_of(size) {
            held.insert((*name).to_owned(), bytes);
        }
    }
    held
}

/// `size` as Go's `units.HumanSize` writes it (`1.234GB`), in bytes; `None` where it is not.
fn bytes_of(size: &str) -> Option<u64> {
    const UNITS: [&str; 9] = ["B", "kB", "MB", "GB", "TB", "PB", "EB", "ZB", "YB"];
    let at = size.find(|c: char| !(c.is_ascii_digit() || c == '.'))?;
    let (number, unit) = size.split_at(at);
    let power = UNITS
        .iter()
        .position(|one| one.eq_ignore_ascii_case(unit))?;
    let (whole, part) = number.split_once('.').unwrap_or((number, ""));
    if whole.is_empty() || part.len() > 9 || !part.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let digits: u128 = format!("{whole}{part}").parse().ok()?;
    let scale = 1000u128.checked_pow(u32::try_from(power).ok()?)?;
    let bytes = digits.checked_mul(scale)? / 10u128.pow(u32::try_from(part.len()).ok()?);
    Some(u64::try_from(bytes).unwrap_or(u64::MAX))
}

/// **How a runner's container was made to suspend itself** (#3717), which its pool's
/// `suspendAfter` does not say for a runner made before it: a runner made before purlis
/// suspended runners (#2051) never does, and one made before a pool had its own time (#3471)
/// keeps the time it was made with.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MadeSuspending {
    /// It was made with `serve` alone, and never suspends itself.
    Never,
    /// It suspends itself after this long without a link from its desktop.
    After(std::time::Duration),
}

/// [`MadeSuspending`], from the command `docker inspect <container>` says it was made with
/// (`Config.Cmd`, as [`Runner::create_args`] gives it); `None` where that is not a runner's
/// `serve` as purlis makes one, a time outside [`suspend_after`]'s bounds included.
pub fn made_suspending(inspected: &serde_json::Value) -> Option<MadeSuspending> {
    let words: Vec<&str> = inspected
        .pointer("/0/Config/Cmd")?
        .as_array()?
        .iter()
        .map(serde_json::Value::as_str)
        .collect::<Option<_>>()?;
    match words.as_slice() {
        ["serve"] => Some(MadeSuspending::Never),
        ["serve", "--suspends-unlinked"] => Some(MadeSuspending::After(SUSPEND_AFTER)),
        ["serve", flag] => {
            let after = flag.strip_prefix("--suspends-unlinked=")?;
            // Held to the bounds `purlis serve` holds it to: a time it refuses is none it
            // suspends after.
            crate::spelledtime::read(after, crate::spelledtime::UP_TO_HOURS)
                .filter(|after| suspend_after(*after).is_ok())
                .map(MadeSuspending::After)
        }
        _ => None,
    }
}

/// [`check_on`], the container's running held to it only where `running` says so: a suspended
/// runner is checked as it was made before it is started again (#2051).
fn checked_on(
    inspected: &serde_json::Value,
    made: &Made,
    engine: Engine,
    running: bool,
) -> Result<Seccomp, Refused> {
    let not = |what: &str| {
        refused(format!(
            "runner {}'s container {what}, so purlis does not link to it; remove the runner and \
             add it again",
            made.runner.name
        ))
    };
    let unreadable = |field: &str| {
        not(&format!(
            "could not be checked ({field} is not as Docker writes it)"
        ))
    };
    let one = match inspected.as_array().map(Vec::as_slice) {
        Some([one]) => one,
        _ => return Err(not("is not one container")),
    };
    let text = |pointer: &str| -> Result<&str, Refused> {
        one.pointer(pointer)
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| unreadable(pointer))
    };
    let truth = |pointer: &str| -> Result<bool, Refused> {
        one.pointer(pointer)
            .and_then(serde_json::Value::as_bool)
            .ok_or_else(|| unreadable(pointer))
    };
    // A list Docker writes as `null` when it is empty.
    let list = |pointer: &str| -> Result<Vec<serde_json::Value>, Refused> {
        match one.pointer(pointer) {
            Some(serde_json::Value::Null) => Ok(Vec::new()),
            Some(serde_json::Value::Array(items)) => Ok(items.clone()),
            _ => Err(unreadable(pointer)),
        }
    };
    let words = |pointer: &str| -> Result<Vec<String>, Refused> {
        list(pointer)?
            .iter()
            .map(|item| {
                item.as_str()
                    .map(str::to_owned)
                    .ok_or_else(|| unreadable(pointer))
            })
            .collect()
    };

    // Podman writes its own shape of what Docker writes (#2680): told by the field Docker
    // never writes, its OCI runtime's name. Each field below is held to the words of the one
    // that wrote it, as `hosts/` recorded them.
    let podman = one.get("OCIRuntime").is_some();
    if text("/Id")? != made.container.as_str() {
        return Err(not("is not the one pinned when it was made"));
    }
    if one
        .pointer(&format!("/Config/Labels/{}", LABEL.replace('/', "~1")))
        .and_then(serde_json::Value::as_str)
        != Some(made.runner.name.as_str())
    {
        return Err(not("does not carry the runner's label"));
    }
    if text("/Config/Image")? != made.runner.image.as_str() {
        return Err(not("does not run the image pinned when it was made"));
    }
    if text("/Config/User")? != made.runner.user.spelled() {
        return Err(not("does not run as the user it was made with"));
    }
    if truth("/HostConfig/Privileged")? {
        return Err(not("is privileged"));
    }
    let options = words("/HostConfig/SecurityOpt")?;
    if !options.iter().any(|option| is_no_new_privileges(option)) {
        return Err(not("may gain new privileges"));
    }
    // The runner's seccomp profile, or, on a runner made before it, its earlier one or the
    // runtime's own default (no seccomp option at all), each stricter.
    let seccomp = if let Some(seccomp) = options
        .iter()
        .find_map(|option| the_runners_seccomp(option, podman))
    {
        seccomp
    } else if options.iter().any(|option| option.starts_with("seccomp")) {
        return Err(not("does not have the runner's seccomp profile"));
    } else {
        Seccomp::RuntimeDefault
    };
    if let Some(option) = options.iter().find(|option| {
        !is_no_new_privileges(option) && the_runners_seccomp(option, podman).is_none()
    }) {
        let named = option.split(['=', ':']).next().unwrap_or_default();
        return Err(not(&format!(
            "has a security option of its own ({named}), where purlis keeps the runtime's defaults"
        )));
    }
    let apparmor = text("/AppArmorProfile")?;
    let default_apparmor = if podman {
        // `containers-default-<version>`, which rootful Podman loads on an AppArmor host; none
        // only on rootless Podman, which cannot load one, and whose user namespace is the wall
        // there (ADR 0089 §10).
        (engine.rootless && apparmor.is_empty()) || is_containers_default(apparmor)
    } else {
        matches!(apparmor, "" | "docker-default")
    };
    if !default_apparmor {
        return Err(not("does not have the runtime's default AppArmor profile"));
    }
    // No SELinux host has been measured (#2680): a container one labels is refused until one is.
    if !text("/ProcessLabel")?.is_empty() || !text("/MountLabel")?.is_empty() {
        return Err(not(
            "carries an SELinux label, and purlis has not been measured on an SELinux host yet",
        ));
    }
    if !words("/HostConfig/CapAdd")?.is_empty() {
        return Err(not("has a capability added"));
    }
    // Podman writes `--cap-drop ALL` as each capability it would have given, and says what the
    // container holds: none, effective or bounding.
    let none_held = |pointer: &str| -> Result<bool, Refused> { Ok(list(pointer)?.is_empty()) };
    let all_dropped = if podman {
        none_held("/EffectiveCaps")? && none_held("/BoundingCaps")?
    } else {
        words("/HostConfig/CapDrop")?
            .iter()
            .any(|cap| cap.eq_ignore_ascii_case("ALL"))
    };
    if !all_dropped {
        return Err(not("keeps capabilities"));
    }
    if text("/HostConfig/NetworkMode")? != "bridge" {
        return Err(not("does not have a network of its own on Docker's bridge"));
    }
    for (pointer, what) in [
        ("/HostConfig/PidMode", "processes"),
        ("/HostConfig/UTSMode", "host name"),
        ("/HostConfig/UsernsMode", "users"),
    ] {
        let mode = text(pointer)?;
        // Podman writes a namespace of the container's own as `private`.
        let own = mode.is_empty() || (podman && mode == "private" && what != "users");
        if !own {
            return Err(not(&format!(
                "shares its {what} with the machine or another container"
            )));
        }
    }
    if text("/HostConfig/IpcMode")? != "private" {
        return Err(not(
            "may share its memory with other containers (its IPC namespace is not private); \
             purlis makes runners with `--ipc private`, and a daemon that changes that is \
             refused",
        ));
    }
    let cgroupns = if podman {
        text("/HostConfig/CgroupMode")?
    } else {
        text("/HostConfig/CgroupnsMode")?
    };
    if cgroupns != "private" {
        return Err(not(
            "shares the machine's cgroup namespace; purlis needs a Docker on cgroup v2 that \
             gives each container a cgroup namespace of its own (`--cgroupns private`)",
        ));
    }
    // Podman writes no device requests at all.
    let requests = if podman && one.pointer("/HostConfig/DeviceRequests").is_none() {
        Vec::new()
    } else {
        list("/HostConfig/DeviceRequests")?
    };
    if !list("/HostConfig/Devices")?.is_empty() || !requests.is_empty() {
        return Err(not("is given a device of this machine"));
    }
    // Podman writes the home's volume among the binds, with the options it mounts it with.
    let binds = words("/HostConfig/Binds")?;
    let binds_its_home_alone = if podman {
        binds
            == [format!(
                "{}:{}:rw,rprivate,nosuid,nodev,rbind",
                made.runner.volume(),
                made.runner.home
            )]
    } else {
        binds.is_empty()
    };
    if !list("/HostConfig/VolumesFrom")?.is_empty() || !binds_its_home_alone {
        return Err(not(
            "is given another container's or this machine's folders",
        ));
    }
    let ports_given = match one.pointer("/HostConfig/PortBindings") {
        Some(serde_json::Value::Null) => false,
        Some(serde_json::Value::Object(bound)) => !bound.is_empty(),
        _ => return Err(unreadable("/HostConfig/PortBindings")),
    };
    if ports_given || truth("/HostConfig/PublishAllPorts")? {
        return Err(not("publishes a port on this machine"));
    }
    let networks = one
        .pointer("/NetworkSettings/Networks")
        .and_then(serde_json::Value::as_object)
        .ok_or_else(|| unreadable("/NetworkSettings/Networks"))?;
    // Podman's default bridge is the network it names `podman`.
    let bridge = if podman { "podman" } else { "bridge" };
    if networks.len() != 1 || !networks.contains_key(bridge) {
        return Err(not("is on a network other than Docker's default bridge"));
    }
    // Absent, `null` or empty: Docker writes none for a container made without one.
    match one.pointer("/HostConfig/Sysctls") {
        None | Some(serde_json::Value::Null) => {}
        Some(serde_json::Value::Object(set)) if set.is_empty() => {}
        Some(serde_json::Value::Object(_)) => {
            return Err(not("is given a kernel setting of its own"));
        }
        Some(_) => return Err(unreadable("/HostConfig/Sysctls")),
    }
    if !truth("/HostConfig/ReadonlyRootfs")? {
        return Err(not("may write to its root file system"));
    }
    let limits = made.runner.limits;
    // Podman writes the options it mounts it with after those it was given.
    let tmp_options = if podman {
        format!("{},rprivate,tmpcopyup", limits.tmp_options())
    } else {
        limits.tmp_options()
    };
    let tmp_only = match one.pointer("/HostConfig/Tmpfs") {
        Some(serde_json::Value::Object(set)) => {
            set.len() == 1
                && set.get("/tmp").and_then(serde_json::Value::as_str) == Some(tmp_options.as_str())
        }
        None | Some(serde_json::Value::Null) => false,
        Some(_) => return Err(unreadable("/HostConfig/Tmpfs")),
    };
    if !tmp_only {
        return Err(not(
            "does not have its /tmp, and no other memory-backed folder, as it was made",
        ));
    }
    let number = |pointer: &str| -> Result<i64, Refused> {
        one.pointer(pointer)
            .and_then(serde_json::Value::as_i64)
            .ok_or_else(|| unreadable(pointer))
    };
    let held = |pointer: &str, value: u64| -> Result<bool, Refused> {
        Ok(u64::try_from(number(pointer)?).ok() == Some(value))
    };
    if !held(
        "/HostConfig/NanoCpus",
        u64::from(limits.millicpus) * 1_000_000,
    )? {
        return Err(not("is not held to its pool's CPU limit"));
    }
    if !held("/HostConfig/Memory", limits.memory_bytes())?
        || !held("/HostConfig/MemorySwap", limits.memory_bytes())?
    {
        return Err(not(
            "is not held to its pool's memory limit, with no swap beyond it",
        ));
    }
    if !held("/HostConfig/PidsLimit", u64::from(limits.pids))? {
        return Err(not("is not held to its pool's process limit"));
    }
    match one.pointer("/HostConfig/OomKillDisable") {
        // Docker writes it for every container, `null` when it was never asked for.
        Some(serde_json::Value::Null) | Some(serde_json::Value::Bool(false)) => {}
        Some(serde_json::Value::Bool(true)) => {
            return Err(not("is never ended when it runs out of memory"));
        }
        None | Some(_) => return Err(unreadable("/HostConfig/OomKillDisable")),
    }
    // Podman writes `oci` here and its runtime's own name beside it.
    let standard_runtime = if podman {
        text("/HostConfig/Runtime")? == "oci" && matches!(text("/OCIRuntime")?, "runc" | "crun")
    } else {
        matches!(
            text("/HostConfig/Runtime")?,
            "runc" | "crun" | "io.containerd.runc.v2"
        )
    };
    if !standard_runtime {
        return Err(not("does not run on a standard container runtime"));
    }
    let mounts = list("/Mounts")?;
    let only_its_home = match mounts.as_slice() {
        [home] => {
            home.get("Type").and_then(serde_json::Value::as_str) == Some("volume")
                && home.get("Name").and_then(serde_json::Value::as_str)
                    == Some(made.runner.volume().as_str())
                && home.get("Destination").and_then(serde_json::Value::as_str)
                    == Some(made.runner.home.as_str())
                && home.get("Driver").and_then(serde_json::Value::as_str) == Some("local")
        }
        _ => false,
    };
    if !only_its_home {
        return Err(not("holds a folder that is not its own home's volume"));
    }
    if running && !truth("/State/Running")? {
        return Err(not("is not running"));
    }
    Ok(seccomp)
}

/// Whether the Docker daemon, by what `docker info --format '{{json .SecurityOptions}}'`
/// printed (`options`), gives containers its built-in seccomp profile: a daemon-wide profile
/// of another name, or none, applies to every container without a word in what
/// `docker inspect` says of it, so anything but `name=seccomp,profile=builtin` (or
/// `profile=default`, as older daemons write it) is refused, the entry missing included.
pub fn check_daemon(options: &serde_json::Value) -> Result<(), Refused> {
    let options = options
        .as_array()
        .ok_or_else(|| refused("docker info did not say which security options its daemon has"))?;
    let seccomp: Vec<&str> = options
        .iter()
        .filter_map(serde_json::Value::as_str)
        .filter(|option| option.split(',').any(|part| part == "name=seccomp"))
        .collect();
    match seccomp.as_slice() {
        [one]
            if one
                .split(',')
                .any(|part| matches!(part, "profile=builtin" | "profile=default")) =>
        {
            Ok(())
        }
        _ => Err(refused(
            "this Docker does not give containers its built-in seccomp profile (its daemon is \
             set to another profile, or to none), so purlis makes no runner on it; set the \
             daemon's seccomp profile back to its default",
        )),
    }
}

/// Whether a Podman, by what `podman info --format '{{json .Host.Security}}'` printed
/// (`security`), gives containers its default seccomp profile (#2680): seccomp on, and the
/// profile Podman ships. Rootful and rootless alike; rootless is the isolation ADR 0089 §10
/// prefers for a local runner. Anything else, a field missing included, is refused.
pub fn check_podman(security: &serde_json::Value) -> Result<Engine, Refused> {
    let rootless = security
        .get("rootless")
        .and_then(serde_json::Value::as_bool);
    let seccomp = security
        .get("seccompEnabled")
        .and_then(serde_json::Value::as_bool);
    let profile = security
        .get("seccompProfilePath")
        .and_then(serde_json::Value::as_str);
    match (rootless, seccomp, profile) {
        (Some(rootless), Some(true), Some(SECCOMP_DEFAULT_PODMAN)) => Ok(Engine { rootless }),
        _ => Err(refused(
            "this Podman does not give containers its default seccomp profile, so purlis makes \
             no runner on it; set its seccomp profile back to its default",
        )),
    }
}

/// The runtime's own file systems in a container, each where Podman mounts it, as measured.
const RUNTIME_MOUNTS: [(&str, &str); 9] = [
    ("/proc", "proc"),
    ("/sys", "sysfs"),
    ("/sys/fs/cgroup", "cgroup"),
    ("/dev", "tmpfs"),
    ("/dev/pts", "devpts"),
    ("/dev/mqueue", "mqueue"),
    ("/tmp", "tmpfs"),
    ("/run", "tmpfs"),
    ("/var/tmp", "tmpfs"),
];

/// The files Podman binds into a container, each where it binds it and by the name it keeps it
/// under in the container's own folder of its storage, or of its run state.
const CONTAINER_FILES: [(&str, &str); 7] = [
    ("/etc/hostname", "hostname"),
    ("/etc/hosts", "hosts"),
    ("/etc/resolv.conf", "resolv.conf"),
    ("/etc/passwd", "passwd"),
    ("/etc/group", "group"),
    ("/run/.containerenv", ".containerenv"),
    ("/dev/shm", "shm"),
];

/// Where Podman keeps its containers, its run state and its volumes, as `podman info` says
/// (`{{json .Store}}`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PodmanStore {
    pub graph_root: std::path::PathBuf,
    pub run_root: std::path::PathBuf,
    pub volume_path: std::path::PathBuf,
}

/// The [`PodmanStore`] `store`, what `podman info --format '{{json .Store}}'` printed, names:
/// each an absolute path, or refused.
pub fn podman_store(store: &serde_json::Value) -> Result<PodmanStore, Refused> {
    let path = |key: &str| {
        store
            .get(key)
            .and_then(serde_json::Value::as_str)
            .map(std::path::PathBuf::from)
            .filter(|path| path.is_absolute())
            .ok_or_else(|| refused(format!("podman did not say its store's {key}")))
    };
    Ok(PodmanStore {
        graph_root: path("graphRoot")?,
        run_root: path("runRoot")?,
        volume_path: path("volumePath")?,
    })
}

/// **Whether `oci`, what rootless Podman asked its runtime to make `made`'s container with, read
/// from `config`, is the pool's own** (#2895). Rootless Podman writes a device it was given
/// nowhere `inspect` reads, only as a mount here (measured 2026-10-21). So: `config` is that
/// container's own (`<graph root>/overlay-containers/<container>/userdata/config.json`); no
/// device for the runtime; and every mount one of the runtime's own file systems where Podman
/// puts it, one of the container's own files from exactly its own folder of Podman's storage or
/// run state, or the home's volume from exactly Podman's own volumes. Anything else, a field
/// missing included, is refused.
pub fn check_oci(
    oci: &serde_json::Value,
    made: &Made,
    config: &std::path::Path,
    store: &PodmanStore,
    seccomp: Seccomp,
) -> Result<(), Refused> {
    let not = |what: &str| {
        refused(format!(
            "runner {}'s container {what}, so purlis does not link to it; remove the runner and \
             add it again",
            made.runner.name
        ))
    };
    let own_folder = |root: &std::path::Path| {
        root.join("overlay-containers")
            .join(made.container.as_str())
            .join("userdata")
    };
    let userdata = own_folder(&store.graph_root);
    let run_userdata = own_folder(&store.run_root);
    if config != userdata.join("config.json") {
        return Err(not("has a runtime config that is not its own"));
    }
    // The runner's profile, exactly as Podman compiles it; on a runner made before it, a
    // profile that refuses what it does not name, which `podman info`'s check holds to Podman's
    // own default ([`check_podman`]).
    let compiled = oci.pointer("/linux/seccomp");
    let held = match seccomp {
        Seccomp::Runners => compiled.is_some_and(|it| is_compiled_from(SECCOMP_PROFILE, it)),
        Seccomp::RunnersEarlier => {
            compiled.is_some_and(|it| is_compiled_from(EARLIER_SECCOMP_PROFILE, it))
        }
        Seccomp::RuntimeDefault => {
            compiled.is_some_and(|it| it.is_object() && it["defaultAction"] == "SCMP_ACT_ERRNO")
        }
    };
    if !held {
        return Err(not("does not run with the runner's seccomp profile"));
    }
    match oci.pointer("/linux/devices") {
        None | Some(serde_json::Value::Null) => {}
        Some(serde_json::Value::Array(devices)) if devices.is_empty() => {}
        Some(_) => return Err(not("is given a device of this machine")),
    }
    let mounts = oci
        .get("mounts")
        .and_then(serde_json::Value::as_array)
        .ok_or_else(|| {
            not("could not be checked (its runtime's mounts are not as Podman writes them)")
        })?;
    let home_volume = store.volume_path.join(made.runner.volume()).join("_data");
    for mount in mounts {
        let field = |name: &str| {
            mount
                .get(name)
                .and_then(serde_json::Value::as_str)
                .unwrap_or("")
        };
        let (to, kind, from) = (field("destination"), field("type"), field("source"));
        let from_path = std::path::Path::new(from);
        let own_file = CONTAINER_FILES.iter().any(|(at, name)| {
            *at == to && (from_path == userdata.join(name) || from_path == run_userdata.join(name))
        });
        let own = RUNTIME_MOUNTS.contains(&(to, kind)) && from == kind
            || (to == "/dev/shm" && kind == "tmpfs")
            || (kind == "bind" && own_file)
            || (kind == "bind" && to == made.runner.home && from_path == home_volume)
            || is_podmans_init(mount);
        if !own {
            return Err(not(&format!(
                "has {to} mounted from {from}, which is not the pool's own"
            )));
        }
    }
    Ok(())
}

/// **Whether `mount` is Podman's own init, as `--init` binds it** (#3702): its `catatonit`, from
/// where the machine's Podman keeps it (a regular file there), bound read-only at `/run/podman-init`, the runner's PID 1
/// that reaps what a chat's programs leave behind. Measured on rootless Podman 2026-10-29.
fn is_podmans_init(mount: &serde_json::Value) -> bool {
    let field = |name: &str| mount.get(name).and_then(serde_json::Value::as_str);
    let from = std::path::Path::new(field("source").unwrap_or_default());
    field("destination") == Some("/run/podman-init")
        && field("type") == Some("bind")
        && from.is_absolute()
        && from.file_name().is_some_and(|name| name == "catatonit")
        && std::fs::metadata(from).is_ok_and(|found| found.is_file())
        && mount.get("options") == Some(&serde_json::json!(["bind", "ro", "rprivate"]))
}

/// Where Podman keeps its default seccomp profile, as `podman info` names it.
const SECCOMP_DEFAULT_PODMAN: &str = "/usr/share/containers/seccomp.json";

/// Whether `inspected`, what `docker volume inspect <volume>` printed, is `made`'s home
/// volume as Docker made it for the container: the `local` driver with no options, so its
/// data is the volume's own and no folder of this machine.
pub fn check_volume(inspected: &serde_json::Value, made: &Made) -> Result<(), Refused> {
    let not = || {
        refused(format!(
            "runner {}'s home volume is not one of Docker's own, made with no options, so purlis \
             does not link to it; remove the runner and add it again",
            made.runner.name
        ))
    };
    let one = match inspected.as_array().map(Vec::as_slice) {
        Some([one]) => one,
        _ => return Err(not()),
    };
    let named = one.get("Name").and_then(serde_json::Value::as_str) == Some(&made.runner.volume());
    let local = one.get("Driver").and_then(serde_json::Value::as_str) == Some("local");
    let no_options = match one.get("Options") {
        Some(serde_json::Value::Null) => true,
        Some(serde_json::Value::Object(options)) => options.is_empty(),
        _ => false,
    };
    if named && local && no_options {
        Ok(())
    } else {
        Err(not())
    }
}

/// Whether `profile` is the AppArmor profile Podman loads for its containers, as it names it:
/// `containers-default-<version>`, a version of digits and dots, and `-apparmor<digits>` after
/// it where the distribution adds one (`containers-default-0.57.4-apparmor1`).
fn is_containers_default(profile: &str) -> bool {
    let Some(rest) = profile.strip_prefix("containers-default-") else {
        return false;
    };
    let (version, suffix) = match rest.split_once("-apparmor") {
        Some((version, number)) => (version, Some(number)),
        None => (rest, None),
    };
    let a_version = version.starts_with(|c: char| c.is_ascii_digit())
        && version.chars().all(|c| c.is_ascii_digit() || c == '.');
    let a_suffix = suffix
        .is_none_or(|number| !number.is_empty() && number.chars().all(|c| c.is_ascii_digit()));
    a_version && a_suffix
}

/// **The runner container's seccomp profile** (#3410, #3298): Docker's default, the Moby
/// project's `seccomp/v0.2.4` (Apache-2.0), kept beside it as published
/// (`seccomp-moby-v0.2.4.json`), with four rules added after its first. Every capability is
/// dropped from a runner, and Docker's default allows each of these calls only with one:
///
/// - `pidfd_getfd`: the Linux sandbox's supervisor takes a chat's own socket with it to make the
///   chat's `listen` (`purlis_linux_sandbox`), so without it no server a chat starts listens;
/// - `clone` and `unshare` making no namespace but a user, a process and a network one
///   (`CLONE_NEWNS`, `CLONE_NEWCGROUP`, `CLONE_NEWUTS` and `CLONE_NEWIPC` refused, and
///   `CLONE_NEWTIME` to `unshare`, which alone takes it), and `chroot`:
///   the browser engine's own sandbox makes them as it starts, and ends without them (measured
///   2026-10-28; ruled that day, ADR 0089). `chroot` reaches nothing outside a user namespace a
///   process made itself, since the kernel still asks for `CAP_SYS_CHROOT`. Only the browser engine
///   may make namespaces on a runner: its host starts everything else unable to
///   (`purlis_linux_sandbox::NO_NAMESPACES`).
///
/// Nothing else differs from Docker's default.
pub const SECCOMP_PROFILE: &str = include_str!("dockerpool/seccomp.json");

/// **The runner's profile as a purlis made it from #3410 until #3298**: Docker's default and
/// `pidfd_getfd` alone, kept as it was so a runner made then is still known and linked
/// ([`Seccomp::RunnersEarlier`]).
pub const EARLIER_SECCOMP_PROFILE: &str = include_str!("dockerpool/seccomp-runner-2026-10-27.json");

/// The file name [`SECCOMP_PROFILE`] is kept under in a run's folder, for `docker` to read: a
/// name of its own, so Podman, which keeps only the path, says which profile a runner runs.
pub const SECCOMP_FILE: &str = "runner-seccomp-engine.json";

/// The file name [`EARLIER_SECCOMP_PROFILE`] is kept under, as a purlis from #3410 to #3298
/// kept the runner's profile.
pub const EARLIER_SECCOMP_FILE: &str = "runner-seccomp.json";

/// **What a runner image's `purlis` is asked, to tell whether it holds what it starts from
/// namespaces** (#3298, D-2050-26): `_no-namespaces` starting a shell that ends at once. A
/// `purlis` from before it does not know the command, and answers otherwise.
const HOLDS_FROM_NAMESPACES: [&str; 5] = ["_no-namespaces", "--", "/bin/sh", "-c", "exit 0"];

/// A profile kept here, read.
fn profile_of(text: &str) -> serde_json::Value {
    serde_json::from_str(text).unwrap_or(serde_json::Value::Null)
}

/// Which of the runner's seccomp profiles `option`, one of a container's security options as
/// `inspect` writes it, is, if either. Docker's client reads the file and sends its contents,
/// and the engine keeps them as they came: held to the runner's profile, or its earlier one, rule
/// for rule. Podman reads the file as it makes the container and keeps only its path: held to an
/// absolute path naming either profile's file, each a name of its own, and, where its runtime's
/// config can be read (rootless), to what it compiled from it ([`check_oci`]). Rootful Podman's
/// runtime config is root's: there the path is all that is read (ADR 0078, amended 2026-10-27
/// and 2026-10-28).
fn the_runners_seccomp(option: &str, podman: bool) -> Option<Seccomp> {
    let profile = option.strip_prefix("seccomp=")?;
    if podman {
        let path = std::path::Path::new(profile);
        if !path.is_absolute() {
            return None;
        }
        match path.file_name().and_then(std::ffi::OsStr::to_str) {
            Some(SECCOMP_FILE) => Some(Seccomp::Runners),
            Some(EARLIER_SECCOMP_FILE) => Some(Seccomp::RunnersEarlier),
            _ => None,
        }
    } else {
        let given = serde_json::from_str::<serde_json::Value>(profile).ok()?;
        if given == profile_of(SECCOMP_PROFILE) {
            Some(Seccomp::Runners)
        } else if given == profile_of(EARLIER_SECCOMP_PROFILE) {
            Some(Seccomp::RunnersEarlier)
        } else {
            None
        }
    }
}

/// Whether `compiled`, the profile Podman's runtime config holds for a container, is exactly what
/// Podman makes of `profile` for a runner, which holds no capability, on the architecture the
/// profile names first ([`compiled_for`]).
fn is_compiled_from(profile: &str, compiled: &serde_json::Value) -> bool {
    compiled
        .pointer("/architectures/0")
        .and_then(serde_json::Value::as_str)
        .and_then(|native| compiled_for(profile, native))
        .is_some_and(|ours| &ours == compiled)
}

/// **What Podman (`containers/common`'s seccomp) makes of `text`**, a runner's profile, for a container
/// with no capability on the architecture `native` (`SCMP_ARCH_X86_64` or `SCMP_ARCH_AARCH64`),
/// as its runtime config holds it: the default action and errno; `native` and the
/// architectures its map puts beside it; and each rule its conditions keep, by its names,
/// action, errno and arguments (an argument's second value left out where it is zero). A rule
/// is kept unless it is for other architectures, excluded on this one, or for a capability. Each
/// rule's lowest kernel is below the 6.12 the Linux sandbox needs, so none is dropped for it.
/// None for any other architecture, or a condition this does not know.
fn compiled_for(text: &str, native: &str) -> Option<serde_json::Value> {
    use serde_json::{Map, Value, json};
    let arch = match native {
        "SCMP_ARCH_X86_64" => "amd64",
        "SCMP_ARCH_AARCH64" => "arm64",
        _ => return None,
    };
    let profile = profile_of(text);
    let mut architectures = vec![Value::from(native)];
    for map in profile["archMap"].as_array()? {
        if map["architecture"] == native {
            architectures.extend(map["subArchitectures"].as_array()?.iter().cloned());
        }
    }
    let names = |list: &Value| -> Option<Vec<String>> {
        match list {
            Value::Null => Some(Vec::new()),
            Value::Array(items) => items
                .iter()
                .map(|it| it.as_str().map(str::to_owned))
                .collect(),
            _ => None,
        }
    };
    let mut rules = Vec::new();
    for rule in profile["syscalls"].as_array()? {
        let (includes, excludes) = (&rule["includes"], &rule["excludes"]);
        for condition in [includes, excludes] {
            let known = condition.as_object().is_none_or(|it| {
                it.keys()
                    .all(|key| matches!(key.as_str(), "arches" | "caps" | "minKernel"))
            });
            if !known {
                return None;
            }
        }
        let arches = names(&includes["arches"])?;
        if (!arches.is_empty() && !arches.iter().any(|it| it == arch))
            || !names(&includes["caps"])?.is_empty()
            || names(&excludes["arches"])?.iter().any(|it| it == arch)
        {
            continue;
        }
        let mut compiled = Map::new();
        compiled.insert("names".into(), rule["names"].clone());
        compiled.insert("action".into(), rule["action"].clone());
        if !rule["errnoRet"].is_null() {
            compiled.insert("errnoRet".into(), rule["errnoRet"].clone());
        }
        if let Some(args) = rule["args"].as_array().filter(|args| !args.is_empty()) {
            let args = args
                .iter()
                .map(|arg| {
                    let mut arg = arg.as_object()?.clone();
                    if arg.get("valueTwo").is_some_and(|it| it == 0) {
                        arg.remove("valueTwo");
                    }
                    Some(Value::Object(arg))
                })
                .collect::<Option<Vec<_>>>()?;
            compiled.insert("args".into(), args.into());
        }
        rules.push(Value::Object(compiled));
    }
    Some(json!({
        "defaultAction": profile["defaultAction"],
        "defaultErrnoRet": profile["defaultErrnoRet"],
        "architectures": architectures,
        "syscalls": rules,
    }))
}

/// Whether a security option is `no-new-privileges`, as Docker takes and writes it.
fn is_no_new_privileges(option: &str) -> bool {
    matches!(
        option,
        "no-new-privileges" | "no-new-privileges:true" | "no-new-privileges=true"
    )
}

/// The `docker` command this machine runs (or a command that answers as it does, such as
/// Podman's), by its path. purlis runs it directly, never through a shell, and never with the
/// person's own Docker settings: each run is given a checked copy of them and the engine pinned
/// ([`settings`], #2752).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Docker {
    program: std::path::PathBuf,
    /// Where each run's copy of Docker's settings is made: `<data>/docker-settings`, unless
    /// another folder is named (a test's).
    settings_in: Option<std::path::PathBuf>,
    /// How long each run that only reads may take (an inspect, a version, a list): [`READS_WITHIN`]
    /// unless a caller holds it to another time ([`Docker::within`]). A run that changes
    /// something (a pull, a make, a start, a removal) takes as long as it takes.
    reads_within: std::time::Duration,
}

/// **How long each `docker` run that only reads may take** (#3546): an inspect, an `info`, a
/// list, a runner's version. One still running then is ended and refused, saying it did not
/// answer, so a wedged engine holds up no pool surface that asks it.
///
/// The time stops this machine's `docker`, not what it started: a runner's `purlis --version`
/// that hangs inside its container keeps running there until the container stops. It is
/// purlis's own, which prints and ends, and a version once read is not asked again.
pub const READS_WITHIN: std::time::Duration = std::time::Duration::from_secs(20);

pub mod settings;

/// Why a `docker` run gave no answer: it did not answer in its time ([`READS_WITHIN`]), or it
/// was refused otherwise. Only the first says the engine may be stuck, so nothing that would
/// follow a refusal (a pull after an inspect, a Podman's `info` after Docker's) is run then.
enum Failed {
    Late(Refused),
    Refused(Refused),
}

impl From<Failed> for Refused {
    fn from(failed: Failed) -> Refused {
        match failed {
            Failed::Late(why) | Failed::Refused(why) => why,
        }
    }
}

/// Where a `docker` command is looked for first: where Docker's installers put it, Docker
/// Desktop's own, `/usr/local`'s, Homebrew's and the system's.
const DOCKER_INSTALLED: &[&str] = &[
    "/Applications/Docker.app/Contents/Resources/bin/docker",
    "/usr/local/bin/docker",
    "/opt/homebrew/bin/docker",
    "/usr/bin/docker",
];

/// The folders of the home where Docker Desktop, OrbStack and Rancher Desktop link `docker`
/// and its credential helpers: never a chat's to write (#2752, [`crate::sandbox`]).
pub const DOCKER_HOME_FOLDERS: [&str; 3] = [".docker/bin", ".orbstack/bin", ".rd/bin"];

/// **Whether `at`, a path in `home`, is in the installer's own folder** (#2752): the folder that
/// holds it (`.docker`, `.orbstack`, `.rd`) is there under exactly that name, itself, and not
/// a link to somewhere else or another spelling of its name, which a chat could have made or
/// moved into its place before the sandbox held it. Anything else is passed over.
pub fn its_own_in(home: &std::path::Path, at: &str) -> bool {
    let Some(holder) = std::path::Path::new(at).components().next() else {
        return false;
    };
    let (Ok(real_home), Ok(found)) = (home.canonicalize(), home.join(holder).canonicalize()) else {
        return false;
    };
    found.as_os_str() == real_home.join(holder).as_os_str()
}

/// Where Docker Desktop, OrbStack and Rancher Desktop link it in the home folder.
const DOCKER_IN_HOME: &[&str] = &[
    ".docker/bin/docker",
    ".orbstack/bin/docker",
    ".rd/bin/docker",
];

/// **Whether the runner `login` reaches says `name` is signed in** (#4046, #4164): that login's
/// own words, with `--check`. Bounded, in time and in what is kept of it: the runner's answer is
/// its own. A runner that fetches the harness first (#4007) is given the time a fetch takes.
/// `Err` where it says neither, as for a profile it does not keep. A check is no login, so it
/// is not held to its input: the runner refuses the two together.
pub fn standing_by_login(
    login: &crate::profilestore::sign_in::Login,
    name: &str,
) -> Result<crate::profilestore::sign_in::Standing, String> {
    let mut command = std::process::Command::new(&login.program);
    command
        .args(login.args.iter().filter(|arg| *arg != "--until-input-ends"))
        .arg("--check")
        .envs(login.env.iter().map(|(n, v)| (n, v)));
    for name in &login.cleared {
        command.env_remove(name);
    }
    let ran = crate::boundedrun::run_within(
        &mut command,
        None,
        STANDING_MOST,
        STANDING_MOST,
        Some(STANDING_WITHIN),
    )
    .map_err(|why| match why {
        crate::boundedrun::Unanswered::NotStarted(e) => {
            format!("{} could not be run: {e}", login.program)
        }
        crate::boundedrun::Unanswered::PrintedTooMuch => {
            "it printed more than a status check does".to_owned()
        }
        crate::boundedrun::Unanswered::NotFinished(_) => "it did not answer in time".to_owned(),
    })?;
    standing_said(name, &String::from_utf8_lossy(&ran.said))
}

impl Docker {
    /// The `docker` command at `program`.
    pub fn at(program: impl Into<std::path::PathBuf>) -> Docker {
        Docker {
            program: program.into(),
            settings_in: None,
            reads_within: READS_WITHIN,
        }
    }

    /// **This `docker`, each of its runs that only reads held to `within`** rather than
    /// [`READS_WITHIN`] (#2052, #3546): a read still going when it is up is ended and refused.
    /// A pull or a make takes as long as it takes.
    #[must_use]
    pub fn within(mut self, within: std::time::Duration) -> Docker {
        self.reads_within = within;
        self
    }

    /// This `docker`, with each run's copy of Docker's settings made in `folder`.
    #[must_use]
    pub fn with_settings_in(mut self, folder: impl Into<std::path::PathBuf>) -> Docker {
        self.settings_in = Some(folder.into());
        self
    }

    /// The command, by its path.
    pub fn program(&self) -> &std::path::Path {
        &self.program
    }

    /// **What one run of this `docker` is given** ([`settings::prepare`]): a checked copy of
    /// Docker's settings, held until the returned value is dropped, the engine pinned and the
    /// `PATH` its helpers are found on, with [`settings::CLEARED`] not handed on.
    pub fn prepared(&self) -> Result<settings::Prepared, Refused> {
        let settings_in = match &self.settings_in {
            Some(folder) => folder.clone(),
            None => crate::datahome::root()
                .ok_or_else(|| refused("purlis has no data folder to keep docker's settings in"))?
                .join(settings::DIR),
        };
        let home = crate::envvar::var_os("HOME").map(std::path::PathBuf::from);
        // Docker's own folders, where every helper it is handed was found, and the system's.
        let path = std::env::join_paths(settings::run_path()).unwrap_or_default();
        let docker_config = crate::envvar::var_os("DOCKER_CONFIG").map(std::path::PathBuf::from);
        let docker_host = crate::envvar::var("DOCKER_HOST");
        let tmp = crate::envvar::var_os("TMPDIR").map(std::path::PathBuf::from);
        let runtime = crate::envvar::var_os("XDG_RUNTIME_DIR").map(std::path::PathBuf::from);
        settings::prepare(&settings::Source {
            settings_in: &settings_in,
            path,
            home: home.as_deref(),
            docker_config: docker_config.as_deref(),
            docker_host: docker_host.as_deref(),
            tmp: tmp.as_deref(),
            runtime: runtime.as_deref(),
            system: std::path::Path::new(settings::SYSTEM_SOCKET),
        })
    }

    /// **The socket of the Docker engine this machine's pools run on, where it is on this
    /// machine** (#2050, [`settings::engine_on_this_machine`]): read as a run reads it, from
    /// this process's `DOCKER_HOST`, else the system's or an installer's socket.
    pub fn engine_on_this_machine() -> Result<std::path::PathBuf, String> {
        let home = crate::envvar::var_os("HOME").map(std::path::PathBuf::from);
        let docker_host = crate::envvar::var("DOCKER_HOST");
        let tmp = crate::envvar::var_os("TMPDIR").map(std::path::PathBuf::from);
        let runtime = crate::envvar::var_os("XDG_RUNTIME_DIR").map(std::path::PathBuf::from);
        settings::engine_on_this_machine(
            docker_host.as_deref(),
            &settings::Places {
                home: home.as_deref(),
                tmp: tmp.as_deref(),
                runtime: runtime.as_deref(),
                system: std::path::Path::new(settings::SYSTEM_SOCKET),
            },
        )
    }

    /// **The `docker` command on this machine** (#2679): the first of the places Docker's
    /// installers put it (`DOCKER_INSTALLED`, then under `home`), else the first on `path`, of
    /// its absolute folders alone. A window started from the Finder has a `PATH` that names
    /// none. Refused, saying how to get one, where there is none.
    pub fn on_this_machine(
        path: Option<&str>,
        home: Option<&std::path::Path>,
    ) -> Result<Docker, Refused> {
        Docker::found_in(DOCKER_INSTALLED, path, home)
    }

    /// [`Docker::on_this_machine`], with `installed` the places installers put it.
    fn found_in(
        installed: &[&str],
        path: Option<&str>,
        home: Option<&std::path::Path>,
    ) -> Result<Docker, Refused> {
        let installed = installed.iter().map(std::path::PathBuf::from);
        let in_home = home.into_iter().flat_map(|home| {
            DOCKER_IN_HOME
                .iter()
                .filter(move |at| its_own_in(home, at))
                .map(move |at| home.join(at))
        });
        let on_path = path
            .into_iter()
            .flat_map(|path| path.split(':'))
            .filter(|dir| dir.starts_with('/'))
            .map(|dir| std::path::Path::new(dir).join("docker"));
        installed
            .chain(in_home)
            .chain(on_path)
            .find(|candidate| {
                crate::secrets::run::which(&candidate.display().to_string(), None).is_some()
            })
            .map(Docker::at)
            .ok_or_else(|| {
                refused(
                    "purlis found no docker command on this machine; install Docker Desktop, or \
                     another Docker, and try again",
                )
            })
    }

    /// **The folders `docker` looks for a registry's credential helper in** (#2752), in order:
    /// Docker runs one (`docker-credential-desktop`, `-osxkeychain`) by its name on its own
    /// `PATH`, and a window started from the Finder has a `PATH` that names none of the places
    /// Docker's installers put one. So it is looked for where `docker` itself is found
    /// ([`Docker::on_this_machine`]): the folder this `docker` is in and the one its link leads
    /// to, where each installer puts its helpers beside it; then each place Docker's installers
    /// put `docker` (`DOCKER_INSTALLED`, then under `home`); then `path`, of its absolute folders
    /// alone. Each folder once, where it first comes.
    pub fn helpers_searched(
        &self,
        path: Option<&str>,
        home: Option<&std::path::Path>,
    ) -> Vec<std::path::PathBuf> {
        use std::path::{Path, PathBuf};
        // A folder named by a relative path is no folder Docker's installers put it in.
        let folder_of = |file: &Path| {
            file.parent()
                .filter(|folder| folder.is_absolute())
                .map(Path::to_path_buf)
        };
        let own = folder_of(&self.program);
        let linked = self
            .program
            .canonicalize()
            .ok()
            .as_deref()
            .and_then(folder_of);
        let installed = DOCKER_INSTALLED
            .iter()
            .filter_map(|at| folder_of(Path::new(at)));
        let in_home = home.into_iter().flat_map(|home| {
            DOCKER_IN_HOME
                .iter()
                .filter(move |at| its_own_in(home, at))
                .filter_map(move |at| folder_of(&home.join(at)))
        });
        let on_path = path
            .into_iter()
            .flat_map(|path| path.split(':'))
            .filter(|dir| dir.starts_with('/'))
            .map(PathBuf::from);
        let mut searched: Vec<PathBuf> = Vec::new();
        for folder in own
            .into_iter()
            .chain(linked)
            .chain(installed)
            .chain(in_home)
            .chain(on_path)
        {
            if !searched.contains(&folder) {
                searched.push(folder);
            }
        }
        searched
    }

    /// Runs `docker` with `args`, handing it `input`, and answers what it printed, read up to
    /// `most` bytes: one that prints more is ended and refused. What it says on standard error
    /// is kept only to be shown, up to [`MOST_SAID`], and never read to decide anything.
    fn run(&self, args: &[String], input: Option<&str>, most: u64) -> Result<String, Refused> {
        // A checked copy of Docker's settings, the engine pinned, and its helpers found where it
        // is (#2752), whatever the window's environment and Docker's own settings say.
        self.run_prepared(&self.prepared()?, args, input, most, None)
    }

    /// [`Docker::run`], for a run that **only reads**: held to its time ([`READS_WITHIN`]).
    fn read(&self, args: &[String], most: u64) -> Result<String, Refused> {
        self.read_or_late(args, most).map_err(Refused::from)
    }

    /// [`Docker::read`], saying apart a read that did not answer in its time ([`Failed::Late`]).
    fn read_or_late(&self, args: &[String], most: u64) -> Result<String, Failed> {
        self.ran_prepared(
            &self.prepared().map_err(Failed::Refused)?,
            args,
            None,
            most,
            Some(self.reads_within),
        )
    }

    /// [`Docker::run`], given its run's folder already prepared, which is kept until the run
    /// ends.
    fn run_prepared(
        &self,
        prepared: &settings::Prepared,
        args: &[String],
        input: Option<&str>,
        most: u64,
        within: Option<std::time::Duration>,
    ) -> Result<String, Refused> {
        self.ran_prepared(prepared, args, input, most, within)
            .map_err(Refused::from)
    }

    /// [`Docker::run_prepared`], saying apart a run that did not answer in its time.
    fn ran_prepared(
        &self,
        prepared: &settings::Prepared,
        args: &[String],
        input: Option<&str>,
        most: u64,
        within: Option<std::time::Duration>,
    ) -> Result<String, Failed> {
        let mut command = std::process::Command::new(&self.program);
        command.envs(prepared.env.iter().map(|(name, value)| (name, value)));
        for name in settings::CLEARED {
            command.env_remove(name);
        }
        command.args(args);
        let first = || args.first().map(String::as_str).unwrap_or_default();
        let ran = crate::boundedrun::run_within(&mut command, input, most, MOST_SAID, within)
            .map_err(|why| {
            use crate::boundedrun::Unanswered;
            if let Unanswered::NotFinished(why) = &why
                && why.kind() == std::io::ErrorKind::TimedOut
            {
                let secs = within.unwrap_or_default().as_secs().max(1);
                return Failed::Late(refused(format!(
                    "docker {} did not answer within {}, so purlis ended it: the Docker engine \
                     may be stuck",
                    first(),
                    crate::counts::counted(secs, "second", "seconds")
                )));
            }
            Failed::Refused(refused(match why {
                Unanswered::NotStarted(why) => {
                    format!("{} could not be run: {why}", self.program.display())
                }
                Unanswered::PrintedTooMuch => format!(
                    "docker {} printed more than the {most} bytes purlis reads of it, so it was ended",
                    first()
                ),
                Unanswered::NotFinished(why) => {
                    format!("{} did not finish: {why}", self.program.display())
                }
            }))
        })?;
        if !ran.status.success() {
            // Docker's own words say why (ADR 0078 §2: a connector's stderr is shown in full).
            return Err(Failed::Refused(refused(format!(
                "docker {} failed: {}",
                first(),
                String::from_utf8_lossy(&ran.said).trim()
            ))));
        }
        String::from_utf8(ran.printed).map_err(|_| {
            Failed::Refused(refused("docker answered with something that is not text"))
        })
    }

    fn json(&self, args: &[&str], what: &str) -> Result<serde_json::Value, Refused> {
        self.json_or_late(args, what).map_err(Refused::from)
    }

    /// [`Docker::json`], saying apart a read that did not answer in its time ([`Failed::Late`]).
    fn json_or_late(&self, args: &[&str], what: &str) -> Result<serde_json::Value, Failed> {
        let args: Vec<String> = args.iter().map(|arg| (*arg).to_owned()).collect();
        let out = self.read_or_late(&args, MOST_INSPECTED)?;
        serde_json::from_str(&out)
            .map_err(|_| Failed::Refused(refused(format!("{what} did not answer JSON"))))
    }

    /// The user and home `image` declares, for the runner made from it: its `USER` must be a
    /// user and a group by number, neither root, and its `HOME` an absolute plain path. An image
    /// this machine does not hold yet is pulled first, by its digest.
    pub fn declared(&self, image: &Image) -> Result<(User, String), Refused> {
        let inspect = || {
            self.json_or_late(
                &["image", "inspect", image.as_str()],
                "docker image inspect",
            )
        };
        // An image this machine does not hold yet is pulled, by its digest, then read. An engine
        // that did not answer is said to be so, and nothing is pulled from it.
        let inspected = match inspect() {
            Ok(inspected) => inspected,
            Err(Failed::Late(why)) => return Err(why),
            Err(Failed::Refused(_)) => {
                self.run(
                    &["pull".into(), "--quiet".into(), image.as_str().into()],
                    None,
                    MOST_INSPECTED,
                )?;
                inspect().map_err(Refused::from)?
            }
        };
        let config = inspected
            .pointer("/0/Config")
            .ok_or_else(|| refused(format!("the runner image {} has no config", image.as_str())))?;
        let spelled = config
            .get("User")
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default();
        let user = spelled
            .split_once(':')
            .and_then(|(uid, gid)| Some((uid.parse().ok()?, gid.parse().ok()?)))
            .ok_or_else(|| {
                refused(format!(
                    "the runner image's USER is \"{spelled}\"; a runner image names a user and \
                     a group by number, neither root (docs/runner-image.md)"
                ))
            })
            .and_then(|(uid, gid)| User::new(uid, gid))?;
        let home = config
            .get("Env")
            .and_then(serde_json::Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(serde_json::Value::as_str)
            .find_map(|entry| entry.strip_prefix("HOME="))
            .ok_or_else(|| refused("the runner image sets no HOME (docs/runner-image.md)"))?;
        Ok((user, home.to_owned()))
    }

    /// The daemon's security options, held to [`check_daemon`]; or, for a Podman, which has no
    /// such field, its host's security, held to [`check_podman`] (#2680).
    fn check_daemon(&self) -> Result<Engine, Refused> {
        let docker = self.json_or_late(
            &["info", "--format", "{{json .SecurityOptions}}"],
            "docker info",
        );
        match docker {
            Ok(options) => check_daemon(&options).map(|()| Engine::default()),
            // An engine that did not answer is not asked again as a Podman.
            Err(Failed::Late(why)) => Err(why),
            Err(Failed::Refused(why)) => match self.json(
                &["info", "--format", "{{json .Host.Security}}"],
                "podman info",
            ) {
                Ok(security) if security.is_object() => check_podman(&security),
                _ => Err(why),
            },
        }
    }

    /// Make and start `runner`'s container on a home volume made for it, and check it is
    /// confined as it was made. A daemon that does not give containers its built-in seccomp
    /// profile makes none, and neither does a volume of the home's name, or a container of the
    /// runner's, that is already there. A run that fails, a container that fails the check, or
    /// one whose id cannot be read, is removed with its volume, and a removal that fails is said
    /// with the refusal.
    pub fn make(&self, runner: &Runner) -> Result<Made, Refused> {
        self.make_for(runner, None)
    }

    /// [`Docker::make`], the container labelled as made by `desktop` where one is named (#2869).
    pub fn make_for(&self, runner: &Runner, desktop: Option<&str>) -> Result<Made, Refused> {
        self.check_daemon()?;
        let volume = runner.volume();
        let listed = self.read(
            &[
                "volume".into(),
                "ls".into(),
                "--quiet".into(),
                "--filter".into(),
                format!("name={volume}"),
            ],
            MOST_INSPECTED,
        )?;
        if listed.lines().any(|line| line.trim() == volume) {
            return Err(refused(format!(
                "a volume named {volume} is already there, and a runner's home is a volume made \
                 for it alone; remove that volume, or name the runner otherwise"
            )));
        }
        // Nor where a container of its name is: one a run that failed can leave is then
        // this runner's alone, and removed with its volume.
        let container = runner.container();
        let named = self.read(
            &[
                "container".into(),
                "ls".into(),
                "--all".into(),
                "--format".into(),
                "{{.Names}}".into(),
                "--filter".into(),
                format!("name={container}"),
            ],
            MOST_INSPECTED,
        )?;
        if named.lines().any(|line| line.trim() == container) {
            return Err(refused(format!(
                "a container named {container} is already there; remove it, or name the runner \
                 otherwise"
            )));
        }
        // Docker makes the container and its volume before it starts it, so a run that fails
        // can leave both: they are removed with the refusal.
        // The runner's seccomp profile, in the run's own folder, read by `docker` as it runs.
        // The profile that lets the browser engine make its sandbox only for an image whose
        // `purlis` holds everything else it starts from namespaces (D-2050-26); the earlier one,
        // where its browsers are refused, for any other.
        let holds = self.holds_from_namespaces(runner);
        let (file, profile) = if holds {
            (SECCOMP_FILE, SECCOMP_PROFILE)
        } else {
            (EARLIER_SECCOMP_FILE, EARLIER_SECCOMP_PROFILE)
        };
        let prepared = self.prepared()?;
        let seccomp = prepared.keep(file, profile).map_err(|why| {
            refused(format!(
                "purlis could not keep the runner's seccomp profile for docker: {why}"
            ))
        })?;
        let made = self.run_prepared(
            &prepared,
            &runner.create_args_for(desktop, &seccomp),
            None,
            MOST_INSPECTED,
            None,
        );
        drop(prepared);
        let out = match made {
            Ok(out) => out,
            Err(why) => return Err(self.and_removed(why, &container, &volume)),
        };
        let made = match ContainerId::new(out.trim()) {
            Ok(container) => Made {
                container,
                runner: runner.clone(),
            },
            Err(why) => {
                return Err(self.and_removed(why, &runner.container(), &volume));
            }
        };
        // A runner made now runs the profile chosen for it: the runtime's own default is taken
        // only for one made before #3410.
        let wanted = if holds {
            Seccomp::Runners
        } else {
            Seccomp::RunnersEarlier
        };
        match self.checked(&made) {
            Ok(seccomp) if seccomp == wanted => Ok(made),
            Ok(_) => {
                let why = refused(format!(
                    "runner {}'s container was made without the runner's seccomp profile, so \
                     purlis does not link to it",
                    made.runner.name
                ));
                Err(self.and_removed(why, made.container.as_str(), &volume))
            }
            Err(why) => Err(self.and_removed(why, made.container.as_str(), &volume)),
        }
    }

    /// **Whether `runner`'s image's `purlis` holds what it starts from namespaces** (D-2050-26),
    /// asked of a container of the image made for that alone and removed as it ends: no network,
    /// every capability dropped, no new privileges, its root read-only. Anything but a clean
    /// answer within [`PROBE_WITHIN`] is a no, and its container is removed: an image whose
    /// `purlis` does not answer holds up no pool.
    fn holds_from_namespaces(&self, runner: &Runner) -> bool {
        // No name of its own, which a runner's could be: the engine's id for it, written to a
        // file of this run's, is what removes it, and nothing else (#3702).
        let Ok(folder) = tempfile::tempdir() else {
            return false;
        };
        let cidfile = folder.path().join("probe.cid");
        let mut args: Vec<String> = [
            "run",
            "--rm",
            "--cidfile",
            &cidfile.display().to_string(),
            "--user",
            &runner.user.spelled(),
            "--cap-drop",
            "ALL",
            "--security-opt",
            "no-new-privileges",
            "--network",
            "none",
            "--read-only",
            "--entrypoint",
            PURLIS,
            runner.image.as_str(),
        ]
        .iter()
        .map(|word| (*word).to_owned())
        .collect();
        args.extend(HOLDS_FROM_NAMESPACES.iter().map(|word| (*word).to_owned()));
        let held = self
            .clone()
            .within(PROBE_WITHIN)
            .run(&args, None, MOST_INSPECTED)
            .is_ok();
        if !held {
            let id = std::fs::read_to_string(&cidfile).unwrap_or_default();
            if let Ok(id) = ContainerId::new(id.trim()) {
                let rm = [
                    "rm".to_owned(),
                    "--force".to_owned(),
                    id.as_str().to_owned(),
                ];
                let _ = self.run(&rm, None, MOST_SAID);
            }
        }
        held
    }

    /// `why`, after removing the container `container` names and the volume `volume`; a
    /// removal that fails is said with it.
    fn and_removed(&self, why: Refused, container: &str, volume: &str) -> Refused {
        match self.remove_named(container, volume) {
            Ok(()) => why,
            Err(left) => refused(format!(
                "{why}; and what was made could not all be removed ({left}): remove the \
                 container {container} and the volume {volume} with docker"
            )),
        }
    }

    /// **The line a person's shell runs to sign the profile `name` in on `made`'s runner**
    /// (#2049), on the engine whose local socket is `engine`, to `provider`'s plan where one is
    /// named: the container checked as it was made first, as the connector checks it before
    /// every link, then [`Docker::sign_in_login`] with settings of purlis's own
    /// ([`settings::for_a_sign_in`]). Refused, with nothing to type, where either fails.
    pub fn sign_in_line(
        &self,
        made: &Made,
        engine: &std::path::Path,
        name: &str,
        provider: Option<&str>,
    ) -> Result<String, String> {
        self.checked_sign_in(made, engine, name, provider, Typed::InATerminal)?
            .line()
    }

    /// [`Docker::sign_in_line`], by the harness's device-code flow (#4044): what a person is left
    /// with where this machine's port for the browser's answer is taken.
    pub fn device_sign_in_line(
        &self,
        made: &Made,
        engine: &std::path::Path,
        name: &str,
        provider: Option<&str>,
    ) -> Result<String, String> {
        self.checked_sign_in(made, engine, name, provider, Typed::DeviceInATerminal)?
            .line()
    }

    /// **The runner's own sign-in of the profile `name`, for purlis to drive with no terminal**
    /// (#4042): [`Docker::sign_in_line`]'s, checked the same way, its input and output pipes.
    pub fn driven_sign_in(
        &self,
        made: &Made,
        engine: &std::path::Path,
        name: &str,
        provider: Option<&str>,
    ) -> Result<crate::profilestore::sign_in::Login, String> {
        self.checked_sign_in(made, engine, name, provider, Typed::Driven)
    }

    /// **Whether the runner says `name` is signed in**, by its own `purlis profile sign-in
    /// --check` ([`standing_said`]), through the same `docker` as [`Docker::driven_sign_in`]:
    /// for a sign-in with no link to the runner to ask over (#4046). `Err` where it says neither,
    /// as for a profile it does not keep.
    pub fn standing_on_runner(
        &self,
        made: &Made,
        engine: &std::path::Path,
        name: &str,
    ) -> Result<crate::profilestore::sign_in::Standing, String> {
        let login = self.driven_sign_in(made, engine, name, None)?;
        standing_by_login(&login, name)
    }

    /// **The runner's own sign-in of the profile `name`, in a terminal of `size` carried over
    /// pipes** (#4043): [`Docker::driven_sign_in`]'s, for the person to drive in the window. With
    /// `device`, by the harness's device-code flow, for a login that waits for the browser's
    /// answer on the runner's loopback, which nothing carries from a terminal (#4044).
    pub fn terminal_sign_in(
        &self,
        made: &Made,
        engine: &std::path::Path,
        name: &str,
        provider: Option<&str>,
        size: crate::engine::Size,
        device: bool,
    ) -> Result<crate::profilestore::sign_in::Login, String> {
        let typed = if device {
            Typed::DeviceInOwnTerminal(size)
        } else {
            Typed::InOwnTerminal(size)
        };
        self.checked_sign_in(made, engine, name, provider, typed)
    }

    /// **Runs the runner's own sign-out of the profile `name`** (#4043), the container checked
    /// as it was made first. Its harness may be fetched for it, so it has [`SIGN_OUT_WITHIN`].
    pub fn sign_out(&self, made: &Made, name: &str) -> Result<(), String> {
        self.check(made).map_err(|why| why.0)?;
        self.clone()
            .within(SIGN_OUT_WITHIN)
            .run(&made.sign_out_args(name), None, MOST_SAID)
            .map(drop)
            .map_err(|why| why.0)
    }

    /// **The runner's own sign-in of the profile `name`, by `typed`**, the container checked as it
    /// was made first ([`Docker::sign_in_line`], [`Docker::driven_sign_in`]).
    pub fn checked_sign_in(
        &self,
        made: &Made,
        engine: &std::path::Path,
        name: &str,
        provider: Option<&str>,
        typed: Typed,
    ) -> Result<crate::profilestore::sign_in::Login, String> {
        self.check(made).map_err(|why| why.0)?;
        let settings_in = match &self.settings_in {
            Some(folder) => folder.clone(),
            None => crate::datahome::root()
                .ok_or_else(|| "purlis has no data folder to keep docker's settings in".to_owned())?
                .join(settings::DIR),
        };
        let settings = settings::for_a_sign_in(&settings_in).map_err(|why| why.0)?;
        self.sign_in_login_by(typed, made, engine, &settings, name, provider)
    }

    /// **What the line [`Docker::sign_in_line`] types runs**: this `docker`, named by its path,
    /// with Docker's settings the folder `settings` (`--config` and `DOCKER_CONFIG`), its plugin
    /// hooks off (`DOCKER_CLI_HOOKS=false`), no context (`DOCKER_CONTEXT` cleared), the engine
    /// named on the line (`--host unix://<engine>`), and the container by its id: no setting of
    /// the shell's, or of the person's Docker, decides what runs.
    pub fn sign_in_login(
        &self,
        made: &Made,
        engine: &std::path::Path,
        settings: &std::path::Path,
        name: &str,
        provider: Option<&str>,
    ) -> Result<crate::profilestore::sign_in::Login, String> {
        self.sign_in_login_by(Typed::InATerminal, made, engine, settings, name, provider)
    }

    /// [`Docker::sign_in_login`], by `by` ([`Made::sign_in_args_by`]).
    pub fn sign_in_login_by(
        &self,
        by: Typed,
        made: &Made,
        engine: &std::path::Path,
        settings: &std::path::Path,
        name: &str,
        provider: Option<&str>,
    ) -> Result<crate::profilestore::sign_in::Login, String> {
        self.login_running(made.sign_in_args_by(by, name, provider), engine, settings)
    }

    /// **The line a shell runs for the runner's own sign-out of the profile `name`** (#4124): as
    /// [`Docker::sign_in_line`]'s, with a terminal, for a harness whose sign-out asks there. The
    /// container is checked as it was made first.
    pub fn sign_out_line(
        &self,
        made: &Made,
        engine: &std::path::Path,
        name: &str,
    ) -> Result<String, String> {
        self.check(made).map_err(|why| why.0)?;
        let settings_in = match &self.settings_in {
            Some(folder) => folder.clone(),
            None => crate::datahome::root()
                .ok_or_else(|| "purlis has no data folder to keep docker's settings in".to_owned())?
                .join(settings::DIR),
        };
        let settings = settings::for_a_sign_in(&settings_in).map_err(|why| why.0)?;
        self.login_running(made.sign_out_in_a_terminal_args(name), engine, &settings)?
            .line()
    }

    /// What this `docker` runs for `words` on a runner, with Docker's settings the folder
    /// `settings`, on `engine`.
    fn login_running(
        &self,
        words: Vec<String>,
        engine: &std::path::Path,
        settings: &std::path::Path,
    ) -> Result<crate::profilestore::sign_in::Login, String> {
        let typed = |path: &std::path::Path, what: &str| {
            path.to_str()
                .map(str::to_owned)
                .ok_or_else(|| format!("{what} is not at a path purlis can type"))
        };
        let settings = typed(settings, "docker's settings for the sign-in")?;
        let mut args = vec![
            "--config".to_owned(),
            settings.clone(),
            "--host".to_owned(),
            format!("unix://{}", typed(engine, "the Docker engine's socket")?),
        ];
        args.extend(words);
        Ok(crate::profilestore::sign_in::Login {
            program: typed(&self.program, "this machine's docker")?,
            args,
            env: vec![
                ("DOCKER_CONFIG".to_owned(), settings),
                ("DOCKER_CLI_HOOKS".to_owned(), "false".to_owned()),
            ],
            cleared: settings::CLEARED
                .iter()
                .map(|name| (*name).to_owned())
                .collect(),
        })
    }

    /// What Docker says of `made`'s container, its home volume and the daemon, held to
    /// [`check`], [`check_volume`] and [`check_daemon`].
    pub fn check(&self, made: &Made) -> Result<(), Refused> {
        self.checked(made).map(drop)
    }

    /// [`Docker::check`]: the seccomp profile the container runs, where it passes.
    fn checked(&self, made: &Made) -> Result<Seccomp, Refused> {
        let engine = self.check_daemon()?;
        let inspected = self.json(
            &["container", "inspect", made.container.as_str()],
            "docker container inspect",
        )?;
        let seccomp = check_on(&inspected, made, engine)?;
        // Rootless Podman writes some of what it was asked for only in what it asked its
        // runtime for (#2895), which this user can read.
        if engine.rootless {
            let path = inspected
                .pointer("/0/OCIConfigPath")
                .and_then(serde_json::Value::as_str)
                .ok_or_else(|| refused("podman did not say where its runtime's config is"))?;
            let store = self.json(&["info", "--format", "{{json .Store}}"], "podman info")?;
            let config = std::path::Path::new(path);
            let (oci, store) = (read_oci(config)?, podman_store(&store)?);
            check_oci(&oci, made, config, &store, seccomp)?;
        }
        let volume = made.runner.volume();
        let inspected = self.json(&["volume", "inspect", &volume], "docker volume inspect")?;
        check_volume(&inspected, made)?;
        // A runner whose profile lets the browser engine make namespaces runs a `purlis` that
        // holds everything else it starts from them (D-2050-26), or it is not linked.
        if seccomp == Seccomp::Runners
            && self
                .run(&made.exec(&HOLDS_FROM_NAMESPACES), None, MOST_INSPECTED)
                .is_err()
        {
            return Err(refused(format!(
                "runner {}'s container lets its browser engine make namespaces, and its purlis \
                 does not hold its programs from them, so purlis does not link to it; remove \
                 the runner and add it again",
                made.runner.name
            )));
        }
        Ok(seccomp)
    }

    /// **Whether `made`'s container runs** (#2051): one a runner suspended stopped itself.
    pub fn running(&self, made: &Made) -> Result<bool, Refused> {
        self.ran(made).map(|ran| ran.running)
    }

    /// **[`Ran`] of `made`'s container** (#3492): [`Docker::running`], with when it started and
    /// last stopped. Read once, as [`Docker::running`] is.
    pub fn ran(&self, made: &Made) -> Result<Ran, Refused> {
        let inspected = self.json(
            &["container", "inspect", made.container.as_str()],
            "docker container inspect",
        )?;
        Ok(ran(&inspected))
    }

    /// **The disk each volume on this engine holds, by its name** (#3493, [`homes_held`]): read
    /// by `docker system df -v`, which only reads, so no container is started by it, a
    /// suspended runner's included; held to its time as every read is ([`READS_WITHIN`]).
    pub fn homes_held(&self) -> Result<std::collections::BTreeMap<String, u64>, Refused> {
        let printed = self.read(
            &["system".into(), "df".into(), "--verbose".into()],
            MOST_INSPECTED,
        )?;
        Ok(homes_held(&printed))
    }

    /// **Whether `made`'s container was removed outside purlis** (#4149): Docker lists no
    /// container of its id, stopped or running, so no link to it can come back. `None` where
    /// Docker still lists it (a runner only stopped or suspended); refused where Docker did not
    /// answer, which says nothing of the runner. A gone runner says whether its home volume, and
    /// with it the sign-ins kept there, is still there.
    pub fn gone(&self, made: &Made) -> Result<Option<Gone>, Refused> {
        let id = made.container.as_str();
        let listed = self.read(
            &[
                "container".into(),
                "ls".into(),
                "--all".into(),
                "--quiet".into(),
                "--no-trunc".into(),
                "--filter".into(),
                format!("id={id}"),
            ],
            MOST_INSPECTED,
        )?;
        if listed.lines().any(|line| line.trim() == id) {
            return Ok(None);
        }
        let volume = made.runner.volume();
        let volumes = self.read(
            &[
                "volume".into(),
                "ls".into(),
                "--quiet".into(),
                "--filter".into(),
                format!("name={volume}"),
            ],
            MOST_INSPECTED,
        )?;
        Ok(Some(Gone {
            home_kept: volumes.lines().any(|line| line.trim() == volume),
        }))
    }

    /// **[`Docker::gone`], held to the engine that made the runner** (#4166): `made_on` is what
    /// [`Docker::engine_id`] answered as it was made, `None` for a runner made before that was
    /// kept, which is taken at Docker's word as it was. Where another engine answers, the runner
    /// is not said to be removed. An engine that does not say which it is says nothing of the
    /// runner, like a Docker that does not answer.
    pub fn missing(&self, made: &Made, made_on: Option<&str>) -> Result<Option<Missing>, Refused> {
        let Some(gone) = self.gone(made)? else {
            return Ok(None);
        };
        let Some(made_on) = made_on else {
            return Ok(Some(Missing::Removed(gone)));
        };
        match self.engine_id()? {
            Some(now) if now == made_on => Ok(Some(Missing::Removed(gone))),
            Some(_) => Ok(Some(Missing::OnAnotherEngine)),
            None => Err(refused(
                "this Docker does not say which engine it is, so purlis cannot tell whether the \
                 runner was removed",
            )),
        }
    }

    /// **Which engine this `docker` reaches** (#4166), by the id `docker info` prints for it.
    /// `None` where it prints none that reads as an id. A Podman has no such field and is
    /// refused.
    pub fn engine_id(&self) -> Result<Option<String>, Refused> {
        let printed = self.read(
            &["info".into(), "--format".into(), "{{.ID}}".into()],
            MOST_INSPECTED,
        )?;
        Ok(Some(printed.trim().to_owned()).filter(|id| crate::runners::usable_noted(id)))
    }

    /// **`made`'s home volume's `CreatedAt`, exactly as Docker printed it** (#4167): the same
    /// volume prints the same words whatever any clock says. `None` where it prints none.
    pub fn home_created(&self, made: &Made) -> Result<Option<String>, Refused> {
        let inspected = self.json(
            &["volume", "inspect", &made.runner.volume()],
            "docker volume inspect",
        )?;
        Ok(inspected
            .pointer("/0/CreatedAt")
            .and_then(serde_json::Value::as_str)
            .filter(|text| crate::runners::usable_noted(text))
            .map(str::to_owned))
    }

    /// **When `made`'s home volume was made**, in seconds since the epoch, as Docker says it
    /// (#4149): a home made after its runner was paired is not the one purlis made for it, and
    /// nothing of it is carried. `None` where Docker gives no time it reads.
    pub fn home_made_at(&self, made: &Made) -> Result<Option<u64>, Refused> {
        let inspected = self.json(
            &["volume", "inspect", &made.runner.volume()],
            "docker volume inspect",
        )?;
        Ok(inspected
            .pointer("/0/CreatedAt")
            .and_then(serde_json::Value::as_str)
            .and_then(|text| chrono::DateTime::parse_from_rfc3339(text).ok())
            .and_then(|time| u64::try_from(time.timestamp()).ok()))
    }

    /// **[`MadeSuspending`] of `made`'s container** (#3717): `None` where its command is not a
    /// runner's `serve` as purlis makes one.
    pub fn made_suspending(&self, made: &Made) -> Result<Option<MadeSuspending>, Refused> {
        let inspected = self.json(
            &["container", "inspect", made.container.as_str()],
            "docker container inspect",
        )?;
        Ok(made_suspending(&inspected))
    }

    /// **[`Docker::check`], a suspended runner started again first** (#2051): a runner left a
    /// day without a link stops its own container, keeping its home volume; the next link to it
    /// starts that container again, once it is checked as it was made, and checks it whole once
    /// it runs. A container that runs is only checked.
    ///
    /// It answers what it read of the container before anything was started ([`Ran`]), and
    /// whether it started it, so the runner hours record has both (#3492).
    pub fn resumed(&self, made: &Made) -> Result<Resumed, Refused> {
        let engine = self.check_daemon()?;
        let inspected = self.json(
            &["container", "inspect", made.container.as_str()],
            "docker container inspect",
        )?;
        let before = ran(&inspected);
        if before.running {
            return self.check(made).map(|()| Resumed {
                before,
                started: false,
            });
        }
        checked_on(&inspected, made, engine, false)?;
        let id = made.container.as_str().to_owned();
        self.run(&["start".to_owned(), id.clone()], None, MOST_INSPECTED)?;
        // Started here and not as it was made once it runs: it is stopped again, and not linked.
        self.check(made)
            .map(|()| Resumed {
                before,
                started: true,
            })
            .inspect_err(|_| {
                if let Err(why) = self.run(&["stop".to_owned(), id], None, MOST_INSPECTED) {
                    tracing::warn!(
                        "purlis: a runner's container started again and refused its check was not \
                     stopped: {why}"
                    );
                }
            })
    }

    /// Pair `made`'s runner with `desktop`, once its container passes the check: the runner's
    /// device id and public key, for the desktop to keep. What the runner prints is read up to
    /// the length of one such line.
    pub fn pair(
        &self,
        made: &Made,
        desktop: &purlis_session_protocol::hostlink::Pinned,
    ) -> Result<purlis_session_protocol::hostlink::Pinned, Refused> {
        self.check(made)?;
        let line = format!("{}\n", crate::runnerlink::to_line(desktop));
        let out = self.run(&made.pair_args(), Some(&line), crate::runnerlink::MOST_LINE)?;
        crate::runnerlink::from_line(&out).map_err(refused)
    }

    /// **Removes each container of exactly a name in `names` that carries the runner label of
    /// `name` and `desktop`'s label** ([`DESKTOP_LABEL`]), by the id the listing gave: one another
    /// desktop made, or one made before the label, is left as it is. Each removed, named.
    fn remove_labelled(
        &self,
        name: &str,
        desktop: &str,
        names: &[String],
    ) -> Result<Vec<String>, Refused> {
        let words = |words: &[&str]| -> Vec<String> {
            words.iter().map(|word| (*word).to_owned()).collect()
        };
        let container = format!("{PREFIX}{name}");
        let mut removed = Vec::new();
        let listed = self.read(
            &words(&[
                "container",
                "ls",
                "--all",
                // The whole id: Docker cuts it to 12 digits otherwise.
                "--no-trunc",
                "--format",
                &format!("{{{{.ID}}}} {{{{.Names}}}} {{{{.Label \"{DESKTOP_LABEL}\"}}}}"),
                "--filter",
                &format!("name={container}"),
                "--filter",
                &format!("label={LABEL}={name}"),
            ]),
            MOST_INSPECTED,
        )?;
        for line in listed.lines() {
            let mut fields = line.split_whitespace();
            let (Some(id), Some(named), Some(made_by), None) =
                (fields.next(), fields.next(), fields.next(), fields.next())
            else {
                continue;
            };
            if !names.iter().any(|one| one == named) || made_by != desktop {
                continue;
            }
            let id = ContainerId::new(id)?;
            self.run(
                &words(&["container", "rm", "--force", "--volumes", id.as_str()]),
                None,
                MOST_INSPECTED,
            )?;
            removed.push(format!("container {named}"));
        }
        Ok(removed)
    }

    /// **Removes what a make of the runner `name` by `desktop` left behind** (#2869): its
    /// container, only where one of exactly its name carries the runner label of that name and
    /// `desktop`'s label ([`DESKTOP_LABEL`]), removed by the id the listing gave; then its home
    /// volume of exactly its name, only once no container of that name is left, whatever its
    /// labels. One another desktop made, or one made before the label, is left as it is. What it
    /// removed, each named. Only for a name no kept runner holds: the caller says so.
    pub fn remove_leftovers_of(&self, name: &str, desktop: &str) -> Result<Vec<String>, Refused> {
        let words = |words: &[&str]| -> Vec<String> {
            words.iter().map(|word| (*word).to_owned()).collect()
        };
        let container = format!("{PREFIX}{name}");
        let volume = format!("{PREFIX}{name}-home");
        // The container that carried sign-ins to it (#4011) is its leftover too.
        let mut removed = self.remove_labelled(
            name,
            desktop,
            &[container.clone(), format!("{container}-copy")],
        )?;
        // A home volume goes only once no container of its name is left, whoever made it.
        let still = self.read(
            &words(&[
                "container",
                "ls",
                "--all",
                "--format",
                "{{.Names}}",
                "--filter",
                &format!("name={container}"),
            ]),
            MOST_INSPECTED,
        )?;
        if still.lines().any(|line| line.trim() == container) {
            return Ok(removed);
        }
        let volumes = self.read(
            &words(&[
                "volume",
                "ls",
                "--quiet",
                "--filter",
                &format!("name={volume}"),
            ]),
            MOST_INSPECTED,
        )?;
        if volumes.lines().any(|line| line.trim() == volume) {
            self.run(
                &words(&["volume", "rm", "--force", &volume]),
                None,
                MOST_INSPECTED,
            )?;
            removed.push(format!("volume {volume}"));
        }
        Ok(removed)
    }

    /// **The version `made`'s runner runs** (#2749), as its `purlis --version` says it, once
    /// its container passes the check.
    pub fn version_of(&self, made: &Made) -> Result<String, Refused> {
        self.check(made)?;
        let out = self.read(&made.version_args(), crate::runnerlink::MOST_LINE)?;
        version_in(&out).map(str::to_owned).ok_or_else(|| {
            refused(format!(
                "runner {}'s purlis did not say its version as purlis does: {:?}",
                made.runner.name(),
                out.trim()
            ))
        })
    }

    /// **Carries `from`'s sign-ins into `to`'s home** (#4011), on the Docker host
    /// ([`Made::copy_profiles_args`]), for at most [`CARRIED_WITHIN`]: nothing of either home
    /// passes through this machine. What did not answer in its time is removed, and one a
    /// carry before left, of `desktop`'s, goes first so it holds no later carry back (#4055).
    pub fn copy_profiles(
        &self,
        from: &Made,
        to: &Made,
        desktop: &str,
        skip: &[String],
    ) -> Result<(), Refused> {
        let prepared = self.prepared()?;
        let _ = self.remove_labelled(to.runner.name(), desktop, &[to.copy_container()]);
        let ran = self
            .ran_prepared(
                &prepared,
                &to.copy_profiles_args(from, desktop, skip),
                None,
                MOST_INSPECTED,
                Some(CARRIED_WITHIN),
            )
            .map(|_| ());
        if let Err(Failed::Late(_)) = &ran {
            let _ = self.run(
                &[
                    "container".into(),
                    "rm".into(),
                    "--force".into(),
                    to.copy_container(),
                ],
                None,
                MOST_INSPECTED,
            );
        }
        ran.map_err(Refused::from)
    }

    /// Remove `made`'s container and its home's volume, whatever state they are in.
    pub fn remove(&self, made: &Made) -> Result<(), Refused> {
        let (container, volume) = self.remove_each(made);
        container.and(volume)
    }

    /// [`Docker::remove`], part by part: the container, then the home volume, each tried
    /// whatever the other did. One that is already gone is removed; one that is still there
    /// after its removal failed is said with Docker's words and the command that removes it.
    pub fn remove_each(&self, made: &Made) -> (Result<(), Refused>, Result<(), Refused>) {
        let id = made.container.as_str();
        let container = self.removed_or_gone(
            &["container", "rm", "--force", "--volumes", id],
            &[
                "container",
                "ls",
                "--all",
                "--quiet",
                "--no-trunc",
                "--filter",
                &format!("id={id}"),
            ],
            id,
            &format!("runner {}'s container {id}", made.runner.name()),
            &format!("docker container rm --force {id}"),
        );
        let volume = made.runner.volume();
        let volume_gone = self.removed_or_gone(
            &["volume", "rm", "--force", &volume],
            &[
                "volume",
                "ls",
                "--quiet",
                "--filter",
                &format!("name={volume}"),
            ],
            &volume,
            &format!("runner {}'s home volume {volume}", made.runner.name()),
            &format!("docker volume rm --force {volume}"),
        );
        (container, volume_gone)
    }

    /// Runs `removing`; when it fails, asks `listing` whether a line of exactly `listed` is
    /// still there, and only then refuses, saying `what` is left and `how` to remove it.
    fn removed_or_gone(
        &self,
        removing: &[&str],
        listing: &[&str],
        listed: &str,
        what: &str,
        how: &str,
    ) -> Result<(), Refused> {
        let words = |words: &[&str]| {
            words
                .iter()
                .map(|word| (*word).to_owned())
                .collect::<Vec<_>>()
        };
        let Err(why) = self.run(&words(removing), None, MOST_INSPECTED) else {
            return Ok(());
        };
        match self.read(&words(listing), MOST_INSPECTED) {
            Ok(list) if !list.lines().any(|line| line.trim() == listed) => Ok(()),
            _ => Err(refused(format!(
                "{what} could not be removed ({why}); remove it with `{how}`"
            ))),
        }
    }

    fn remove_named(&self, container: &str, volume: &str) -> Result<(), Refused> {
        let container = self.run(
            &[
                "container".into(),
                "rm".into(),
                "--force".into(),
                "--volumes".into(),
                container.into(),
            ],
            None,
            MOST_INSPECTED,
        );
        let volume = self.run(
            &[
                "volume".into(),
                "rm".into(),
                "--force".into(),
                volume.into(),
            ],
            None,
            MOST_INSPECTED,
        );
        container.and(volume).map(drop)
    }
}

/// What rootless Podman asked its runtime for, read up to [`MOST_INSPECTED`].
fn read_oci(path: &std::path::Path) -> Result<serde_json::Value, Refused> {
    use std::io::Read;
    use std::os::unix::fs::OpenOptionsExt;
    // Opened without waiting, and read only once the open file is seen to be a plain file: a
    // FIFO in its place is refused at once rather than waited on.
    let file = std::fs::OpenOptions::new()
        .read(true)
        .custom_flags(rustix::fs::OFlags::NONBLOCK.bits() as i32)
        .open(path)
        .map_err(|why| {
            if why.kind() == std::io::ErrorKind::NotFound {
                refused(format!(
                    "podman's runtime config {} is not on this machine, as a Podman machine \
                     keeps it in its VM, so purlis cannot check the runner's container; run the \
                     pool on a Podman or Docker whose containers run on this machine",
                    path.display()
                ))
            } else {
                refused(format!(
                    "podman's runtime config {} could not be read: {why}",
                    path.display()
                ))
            }
        })?;
    if !file.metadata().is_ok_and(|meta| meta.is_file()) {
        return Err(refused(format!(
            "podman's runtime config {} is not a file, so purlis does not read it",
            path.display()
        )));
    }
    let mut text = String::new();
    file.take(MOST_INSPECTED)
        .read_to_string(&mut text)
        .map_err(|why| {
            refused(format!(
                "podman's runtime config {} could not be read: {why}",
                path.display()
            ))
        })?;
    serde_json::from_str(&text).map_err(|_| {
        refused(format!(
            "podman's runtime config {} is not JSON",
            path.display()
        ))
    })
}

/// The most of what `docker` prints that purlis reads, for an answer that is not a pairing's.
const MOST_INSPECTED: u64 = 4 * 1024 * 1024;

/// How long an image's `purlis` is given to answer whether it holds what it starts from
/// namespaces (D-2050-26): its image is already here, and the answer is a start and an exit.
const PROBE_WITHIN: std::time::Duration = std::time::Duration::from_secs(60);

/// How long a runner's own sign-out has: long enough to fetch the version its profile pins.
pub(crate) const SIGN_OUT_WITHIN: std::time::Duration = std::time::Duration::from_secs(180);

/// The most of what `docker` says on standard error that purlis keeps to show.
const MOST_SAID: u64 = 64 * 1024;

#[cfg(test)]
pub(crate) mod tests;
