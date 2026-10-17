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
//! - its own network namespace on Docker's default bridge, never the machine's.
//!
//! purlis's Linux sandbox needs nothing more (`docs/runner-image.md`, #1040).
//!
//! **A container is checked before it is paired and before every link** ([`check`]): what
//! `docker inspect` says of it must be exactly the confinement above, for the container, the
//! image and the user pinned when it was made. Anything else, a field missing or one that
//! cannot be read included, refuses the link.
//!
//! **Each runner has its own link key, pinned** ([`pair_args`]): the runner's host makes its
//! key on its first pairing, inside the container, and the desktop pins it with the
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
        if hex.len() != 64 || !hex.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f')) {
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

/// One runner of the pool, as it is made: its name, its image, its user and its home.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Runner {
    name: String,
    image: Image,
    user: User,
    home: String,
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
        })
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
        pair(
            "--mount",
            format!("type=volume,source={},target={}", self.volume(), self.home),
        );
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
        if id.len() == 64 && id.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f')) {
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
    for (pointer, what) in [
        ("/HostConfig/IpcMode", "shared memory"),
        ("/HostConfig/CgroupnsMode", "cgroups"),
    ] {
        if !matches!(text(pointer)?, "private" | "") {
            return Err(not(&format!(
                "shares its {what} with the machine or another container"
            )));
        }
    }
    if !list("/HostConfig/Devices")?.is_empty() || !list("/HostConfig/DeviceRequests")?.is_empty() {
        return Err(not("is given a device of this machine"));
    }
    if !list("/HostConfig/VolumesFrom")?.is_empty() || !list("/HostConfig/Binds")?.is_empty() {
        return Err(not(
            "is given another container's or this machine's folders",
        ));
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
    fn run(&self, args: &[String], input: Option<&str>) -> Result<String, Refused> {
        use std::io::Write;
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
        let done = child
            .wait_with_output()
            .map_err(|why| refused(format!("{} did not finish: {why}", self.0.display())))?;
        if !done.status.success() {
            // Docker's own words say why (ADR 0078 §2: a connector's stderr is shown in full).
            return Err(refused(format!(
                "docker {} failed: {}",
                args.first().map(String::as_str).unwrap_or_default(),
                String::from_utf8_lossy(&done.stderr).trim()
            )));
        }
        String::from_utf8(done.stdout)
            .map_err(|_| refused("docker answered with something that is not text"))
    }

    /// The user and home `image` declares, for the runner made from it: its `USER` must be a
    /// user and a group by number, neither root, and its `HOME` an absolute plain path.
    pub fn declared(&self, image: &Image) -> Result<(User, String), Refused> {
        let out = self.run(
            &["image".into(), "inspect".into(), image.as_str().into()],
            None,
        )?;
        let inspected: serde_json::Value = serde_json::from_str(&out)
            .map_err(|_| refused("docker image inspect did not answer JSON"))?;
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

    /// Make and start `runner`'s container, and check it is confined as it was made. One that
    /// fails the check is removed, with its volume.
    pub fn make(&self, runner: &Runner) -> Result<Made, Refused> {
        let out = self.run(&runner.create_args(), None)?;
        let made = Made {
            container: ContainerId::new(out.trim())?,
            runner: runner.clone(),
        };
        if let Err(why) = self.check(&made) {
            let _ = self.remove(&made);
            return Err(why);
        }
        Ok(made)
    }

    /// What Docker says of `made`'s container, held to [`check`].
    pub fn check(&self, made: &Made) -> Result<(), Refused> {
        let out = self.run(
            &[
                "container".into(),
                "inspect".into(),
                made.container.as_str().into(),
            ],
            None,
        )?;
        let inspected: serde_json::Value = serde_json::from_str(&out)
            .map_err(|_| refused("docker container inspect did not answer JSON"))?;
        check(&inspected, made)
    }

    /// Pair `made`'s runner with `desktop`, once its container passes the check: the runner's
    /// own pin, for the desktop to keep.
    pub fn pair(
        &self,
        made: &Made,
        desktop: &purlis_session_protocol::hostlink::Pinned,
    ) -> Result<purlis_session_protocol::hostlink::Pinned, Refused> {
        self.check(made)?;
        let line = format!("{}\n", crate::runnerlink::to_line(desktop));
        let out = self.run(&made.pair_args(), Some(&line))?;
        crate::runnerlink::from_line(&out).map_err(refused)
    }

    /// Remove `made`'s container and its home's volume, whatever state they are in.
    pub fn remove(&self, made: &Made) -> Result<(), Refused> {
        let container = self.run(
            &[
                "container".into(),
                "rm".into(),
                "--force".into(),
                "--volumes".into(),
                made.container.as_str().into(),
            ],
            None,
        );
        let volume = self.run(
            &[
                "volume".into(),
                "rm".into(),
                "--force".into(),
                made.runner.volume(),
            ],
            None,
        );
        container.and(volume).map(drop)
    }
}

#[cfg(test)]
mod tests;
