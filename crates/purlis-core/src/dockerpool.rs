// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A Docker pool on this machine** (spec #2041, #2044): each runner is a container of the
//! runner image (`docs/runner-image.md`), whose `purlis serve` is the runner's session host,
//! reached through `docker exec -i` (ADR 0078 §2's container preset) and linked like any runner.
//!
//! **Every runner container is confined the same way, and nothing loosens it** ([`Runner`]):
//!
//! - Docker's default seccomp profile, and the runtime's default AppArmor profile: no
//!   `--security-opt` that names either, and never `--privileged`;
//! - every capability dropped (`--cap-drop ALL`), none added, and no new privileges
//!   (`no-new-privileges`), so nothing in it gains a power its user does not have;
//! - a user who is not root, by number ([`User`]);
//! - the image by its digest alone ([`Image`]), never by a tag;
//! - its home on a volume of its own, and no folder, device or socket of this machine;
//! - its own network namespace on Docker's default bridge, never the machine's;
//! - a read-only root file system, so its home volume is the only place it can write that may
//!   hold a program it runs: `/tmp` is a `tmpfs` (`nosuid`, `nodev`, `noexec`), and Docker's
//!   own `/dev/shm` is memory-backed and `noexec` too;
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
//! **Each runner has its own link key, pinned** ([`pair_args`]): the runner makes its key at
//! its one pairing, inside the container, and the desktop keeps its public half with the
//! runner's device id; a new container is a new runner with a new key.

use std::fmt;

/// The label every runner container of the pool carries: its runner's name.
pub const LABEL: &str = "purlis.runner";

/// Every runner container's and volume's name starts with this.
pub const PREFIX: &str = "purlis-runner-";

/// Where `purlis` is in a runner image (`docs/runner-image.md`).
pub const PURLIS: &str = "/usr/local/bin/purlis";

/// The longest runner name.
const MOST_NAME: usize = 40;

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
}

/// One runner of the pool, as it is made: its name, its image, its user, its home and its
/// pool's limits.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Runner {
    name: String,
    image: Image,
    user: User,
    home: String,
    limits: Limits,
}

impl Runner {
    /// A runner called `name` (lower-case letters, digits and dashes, at most 40, starting and
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
        })
    }

    /// This runner, held to its pool's `limits` in place of the defaults.
    pub fn limited(self, limits: Limits) -> Runner {
        Runner { limits, ..self }
    }

    pub fn limits(&self) -> Limits {
        self.limits
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
    /// module says. Its id is what `docker` prints.
    pub fn create_args(&self) -> Vec<String> {
        let mut args: Vec<String> = Vec::new();
        let mut pair = |flag: &str, value: String| {
            args.push(flag.to_owned());
            args.push(value);
        };
        pair("--name", self.container());
        pair("--label", format!("{LABEL}={}", self.name));
        pair("--user", self.user.spelled());
        pair("--cap-drop", "ALL".into());
        pair("--security-opt", "no-new-privileges".into());
        pair("--network", "bridge".into());
        pair("--ipc", "private".into());
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
        let mut all = vec!["run".to_owned(), "--detach".to_owned()];
        all.extend(args);
        all.push(self.image.as_str().to_owned());
        all.push("serve".to_owned());
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
}

/// A runner's container as it was made: what every check and every link holds it to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Made {
    pub container: ContainerId,
    pub runner: Runner,
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

/// Whether `inspected`, what `docker inspect <container>` printed, is `made`'s container,
/// running and confined exactly as [`Runner::create_args`] made it. Anything else is refused,
/// with what differs: a field missing, or of another shape, included.
pub fn check(inspected: &serde_json::Value, made: &Made) -> Result<(), Refused> {
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
    if let Some(option) = options.iter().find(|option| !is_no_new_privileges(option)) {
        let named = option.split(['=', ':']).next().unwrap_or_default();
        return Err(not(&format!(
            "has a security option of its own ({named}), where purlis keeps the runtime's defaults"
        )));
    }
    if !matches!(text("/AppArmorProfile")?, "" | "docker-default") {
        return Err(not("does not have the runtime's default AppArmor profile"));
    }
    if !words("/HostConfig/CapAdd")?.is_empty() {
        return Err(not("has a capability added"));
    }
    if !words("/HostConfig/CapDrop")?
        .iter()
        .any(|cap| cap.eq_ignore_ascii_case("ALL"))
    {
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
        if !text(pointer)?.is_empty() {
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
    if text("/HostConfig/CgroupnsMode")? != "private" {
        return Err(not(
            "shares the machine's cgroup namespace; purlis needs a Docker on cgroup v2 that \
             gives each container a cgroup namespace of its own (`--cgroupns private`)",
        ));
    }
    if !list("/HostConfig/Devices")?.is_empty() || !list("/HostConfig/DeviceRequests")?.is_empty() {
        return Err(not("is given a device of this machine"));
    }
    if !list("/HostConfig/VolumesFrom")?.is_empty() || !list("/HostConfig/Binds")?.is_empty() {
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
    if networks.len() != 1 || !networks.contains_key("bridge") {
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
    let tmp_only = match one.pointer("/HostConfig/Tmpfs") {
        Some(serde_json::Value::Object(set)) => {
            set.len() == 1
                && set.get("/tmp").and_then(serde_json::Value::as_str)
                    == Some(limits.tmp_options().as_str())
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
    if !matches!(
        text("/HostConfig/Runtime")?,
        "runc" | "crun" | "io.containerd.runc.v2"
    ) {
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
    if !truth("/State/Running")? {
        return Err(not("is not running"));
    }
    Ok(())
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

/// Whether a security option is `no-new-privileges`, as Docker takes and writes it.
fn is_no_new_privileges(option: &str) -> bool {
    matches!(
        option,
        "no-new-privileges" | "no-new-privileges:true" | "no-new-privileges=true"
    )
}

/// The `docker` command this machine runs (or a command that answers as it does, such as
/// Podman's), by its path. purlis runs it directly, never through a shell; `DOCKER_HOST` and
/// Docker's own configuration are the command's to read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Docker(pub std::path::PathBuf);

impl Docker {
    /// Runs `docker` with `args`, handing it `input`, and answers what it printed, read up to
    /// `most` bytes: one that prints more is ended and refused. What it says on standard error
    /// is kept only to be shown, up to [`MOST_SAID`], and never read to decide anything.
    fn run(&self, args: &[String], input: Option<&str>, most: u64) -> Result<String, Refused> {
        use std::io::{Read, Write};
        let mut command = std::process::Command::new(&self.0);
        command
            .args(args)
            .stdin(if input.is_some() {
                std::process::Stdio::piped()
            } else {
                std::process::Stdio::null()
            })
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped());
        let mut child = crate::forklock::spawn(&mut command)
            .map_err(|why| refused(format!("{} could not be run: {why}", self.0.display())))?;
        if let (Some(input), Some(mut stdin)) = (input, child.stdin.take()) {
            let _ = stdin.write_all(input.as_bytes());
        }
        let said = child.stderr.take().map(|stderr| {
            std::thread::spawn(move || {
                let mut kept = Vec::new();
                let mut stderr = stderr;
                let _ = (&mut stderr).take(MOST_SAID).read_to_end(&mut kept);
                // What is past the limit is read and let go of, so the command never waits.
                let _ = std::io::copy(&mut stderr, &mut std::io::sink());
                kept
            })
        });
        let mut out = Vec::new();
        if let Some(stdout) = child.stdout.take() {
            let _ = stdout.take(most + 1).read_to_end(&mut out);
        }
        if out.len() as u64 > most {
            let _ = child.kill();
            let _ = child.wait();
            return Err(refused(format!(
                "docker {} printed more than the {most} bytes purlis reads of it, so it was ended",
                args.first().map(String::as_str).unwrap_or_default()
            )));
        }
        let status = child
            .wait()
            .map_err(|why| refused(format!("{} did not finish: {why}", self.0.display())))?;
        let said = said.and_then(|said| said.join().ok()).unwrap_or_default();
        if !status.success() {
            // Docker's own words say why (ADR 0078 §2: a connector's stderr is shown in full).
            return Err(refused(format!(
                "docker {} failed: {}",
                args.first().map(String::as_str).unwrap_or_default(),
                String::from_utf8_lossy(&said).trim()
            )));
        }
        String::from_utf8(out)
            .map_err(|_| refused("docker answered with something that is not text"))
    }

    fn json(&self, args: &[&str], what: &str) -> Result<serde_json::Value, Refused> {
        let args: Vec<String> = args.iter().map(|arg| (*arg).to_owned()).collect();
        let out = self.run(&args, None, MOST_INSPECTED)?;
        serde_json::from_str(&out).map_err(|_| refused(format!("{what} did not answer JSON")))
    }

    /// The user and home `image` declares, for the runner made from it: its `USER` must be a
    /// user and a group by number, neither root, and its `HOME` an absolute plain path.
    pub fn declared(&self, image: &Image) -> Result<(User, String), Refused> {
        let inspected = self.json(
            &["image", "inspect", image.as_str()],
            "docker image inspect",
        )?;
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

    /// The daemon's security options, held to [`check_daemon`].
    fn check_daemon(&self) -> Result<(), Refused> {
        let options = self.json(
            &["info", "--format", "{{json .SecurityOptions}}"],
            "docker info",
        )?;
        check_daemon(&options)
    }

    /// Make and start `runner`'s container on a home volume made for it, and check it is
    /// confined as it was made. A daemon that does not give containers its built-in seccomp
    /// profile makes none, and neither does a volume of the home's name, or a container of the
    /// runner's, that is already there. A run that fails, a container that fails the check, or
    /// one whose id cannot be read, is removed with its volume, and a removal that fails is said
    /// with the refusal.
    pub fn make(&self, runner: &Runner) -> Result<Made, Refused> {
        self.check_daemon()?;
        let volume = runner.volume();
        let listed = self.run(
            &[
                "volume".into(),
                "ls".into(),
                "--quiet".into(),
                "--filter".into(),
                format!("name={volume}"),
            ],
            None,
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
        let named = self.run(
            &[
                "container".into(),
                "ls".into(),
                "--all".into(),
                "--format".into(),
                "{{.Names}}".into(),
                "--filter".into(),
                format!("name={container}"),
            ],
            None,
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
        let out = match self.run(&runner.create_args(), None, MOST_INSPECTED) {
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
        if let Err(why) = self.check(&made) {
            return Err(self.and_removed(why, made.container.as_str(), &volume));
        }
        Ok(made)
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

    /// What Docker says of `made`'s container, its home volume and the daemon, held to
    /// [`check`], [`check_volume`] and [`check_daemon`].
    pub fn check(&self, made: &Made) -> Result<(), Refused> {
        self.check_daemon()?;
        let inspected = self.json(
            &["container", "inspect", made.container.as_str()],
            "docker container inspect",
        )?;
        check(&inspected, made)?;
        let volume = made.runner.volume();
        let inspected = self.json(&["volume", "inspect", &volume], "docker volume inspect")?;
        check_volume(&inspected, made)
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
        match self.run(&words(listing), None, MOST_INSPECTED) {
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

/// The most of what `docker` prints that purlis reads, for an answer that is not a pairing's.
const MOST_INSPECTED: u64 = 4 * 1024 * 1024;

/// The most of what `docker` says on standard error that purlis keeps to show.
const MOST_SAID: u64 = 64 * 1024;

#[cfg(test)]
pub(crate) mod tests;
