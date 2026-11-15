// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A runner's session host, kept running by the machine itself** (RR-15, #724; ADR 0068 §3 as
//! amended for #724, ADR 0078 §5).
//!
//! On a runner nobody sits at, `purlis serve` must outlive the SSH connection that started it,
//! the sign-out that follows, and a restart of the machine. Nothing purlis could build does that
//! as well as the operating system's own service manager, so `purlis serve --install` hands the
//! host to it, as FD-17 (`crate::loginitem`) hands the desktop's host to it:
//!
//! - **Linux with a systemd user manager**: a user unit, `purlis-runner-v<8 hex>.service` for a
//!   version installed in `<config>/server/<version>/` (RR-14), enabled into `default.target`,
//!   which every user manager reaches, with or without a desktop; restarted on failure within
//!   FD-17's limits. And **linger** (`loginctl enable-linger`), without which the user manager,
//!   and every unit in it, stops at the last sign-out and starts only at the next sign-in. Where
//!   the machine lets only an administrator turn it on, purlis says so plainly and names the one
//!   command to ask for.
//! - **macOS**: a LaunchAgent, `dev.purlis.runner.v<8 hex>`, which launchd keeps running after
//!   the SSH connection goes and starts again after an exit that was not a success. After a
//!   restart launchd starts it once the account signs in again: starting before anyone signs in
//!   would need a LaunchDaemon, which only an administrator may install.
//! - **A container, or Linux with no user manager**: nothing to hand it to. `purlis serve
//!   --foreground` (what `purlis serve` always did) runs as the container's command, and its
//!   restart policy keeps it up.
//!
//! **Each installed version has a service of its own**, named by [`crate::hostsocket::version_key`]
//! as its socket is, so installing a new version beside an old one (RR-14) never stops or
//! rewrites the old one's: its chats drain where they run. A binary that is not installed as a
//! version (put somewhere by hand) has the one unversioned service.
//!
//! **A host the manager starts never exits for want of a client**: `purlis serve` never did
//! (`purlis_host::lifetime`). It is started as `purlis serve --as-a-service`, which differs from
//! `purlis serve` in one thing: finding another host of its version already serving is a
//! success, so the manager does not restart it into the same answer until its limit trips.
//!
//! Nothing here reads what a manager prints but the exit status of its own query commands
//! (`systemctl --user is-enabled`, `launchctl print`), which are their interfaces for scripts.
//! Linger is read from the file `systemd-logind` keeps for it, `/var/lib/systemd/linger/<user>`.

use std::path::{Path, PathBuf};

use crate::loginitem::{self, Os, Runs};

/// The hidden flag a service manager starts the host with: `purlis serve --as-a-service`.
pub const AS_A_SERVICE: &str = "--as-a-service";

/// Where `systemd-logind` notes each user whose manager lingers (`loginctl(1)`, `enable-linger`).
pub const LINGER_DIR: &str = "/var/lib/systemd/linger";

/// **The host of one purlis binary on a runner, as a service**: the binary, and the version it
/// is installed as where it is one (RR-14).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Service {
    pub binary: PathBuf,
    pub version: Option<String>,
}

impl Service {
    /// The systemd user unit's name: `purlis-runner-v<8 hex>.service` for a version, else
    /// `purlis-runner.service`.
    pub fn unit_name(&self) -> String {
        match &self.version {
            Some(version) => format!(
                "purlis-runner-{}.service",
                crate::hostsocket::version_key(version)
            ),
            None => "purlis-runner.service".to_owned(),
        }
    }

    /// The launchd label: `dev.purlis.runner.v<8 hex>` for a version, else `dev.purlis.runner`.
    pub fn label(&self) -> String {
        match &self.version {
            Some(version) => format!(
                "dev.purlis.runner.{}",
                crate::hostsocket::version_key(version)
            ),
            None => "dev.purlis.runner".to_owned(),
        }
    }

    fn job(&self) -> loginitem::Job {
        loginitem::Job {
            program: self.binary.clone(),
            args: vec!["serve".to_owned(), AS_A_SERVICE.to_owned()],
        }
    }

    fn described(&self) -> String {
        match &self.version {
            Some(version) => format!("purlis session host, version {version}, on this runner"),
            None => "purlis session host on this runner".to_owned(),
        }
    }
}

/// **The systemd user unit**: wanted by `default.target`, which a user manager reaches whether
/// or not anyone has a desktop; restarted after a failure, and held stopped after
/// [`loginitem::EXITS`] failures within [`loginitem::WITHIN_SECONDS`]. A stop or the machine's
/// shutdown ends the host and every chat in its control group; the next host of this version
/// settles each as ended by that (`purlis_host::HostRecord`).
pub fn unit(service: &Service) -> String {
    let command: Vec<String> = std::iter::once(service.binary.to_string_lossy().into_owned())
        .chain(service.job().args)
        .map(|word| loginitem::systemd_quoted(&word))
        .collect();
    format!(
        "[Unit]\n\
         Description={}\n\
         StartLimitIntervalSec={}\n\
         StartLimitBurst={}\n\
         \n\
         [Service]\n\
         Type=simple\n\
         ExecStart={}\n\
         Restart=on-failure\n\
         RestartSec=5\n\
         \n\
         [Install]\n\
         WantedBy=default.target\n",
        service.described(),
        loginitem::WITHIN_SECONDS,
        loginitem::EXITS,
        command.join(" ")
    )
}

/// **The LaunchAgent**: loaded now and at each sign-in, in a GUI session or a background one
/// (an SSH sign-in's), and started again after an exit that was not a success. What the host
/// says on standard error goes to `log`.
pub fn plist(service: &Service, log: &Path) -> String {
    let mut keep_alive = plist::Dictionary::new();
    keep_alive.insert("SuccessfulExit".into(), false.into());
    let mut agent = plist::Dictionary::new();
    agent.insert("Label".into(), service.label().into());
    agent.insert(
        "ProgramArguments".into(),
        loginitem::arguments(&service.job()),
    );
    agent.insert("RunAtLoad".into(), true.into());
    agent.insert("KeepAlive".into(), keep_alive.into());
    agent.insert(
        "LimitLoadToSessionType".into(),
        plist::Value::Array(vec!["Aqua".into(), "Background".into()]),
    );
    agent.insert(
        "StandardErrorPath".into(),
        log.to_string_lossy().into_owned().into(),
    );
    loginitem::written(agent)
}

/// **What keeps a runner's host running here.**
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Keeper {
    /// The systemd user manager, with linger.
    SystemdUser,
    /// launchd, in this account's domain.
    Launchd,
    /// Nothing purlis may hand it to: it runs in the foreground, under whatever runs it.
    Foreground(Why),
}

/// Why a host runs in the foreground.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Why {
    /// This machine is a container: its own restart policy keeps its command running.
    Container,
    /// Linux with no systemd user manager for this account.
    NoServiceManager,
    /// Not macOS or Linux: no session host runs here (ADR 0031).
    Unsupported,
}

/// **What keeps the host running here**: launchd on macOS; the systemd user manager on Linux
/// where this account has one, container or not (a container that boots systemd has one);
/// otherwise the foreground, said for a container as a container.
pub fn keeper(os: Os, systemd_user: bool, container: bool) -> Keeper {
    match os {
        Os::Macos => Keeper::Launchd,
        Os::Linux if systemd_user => Keeper::SystemdUser,
        Os::Linux if container => Keeper::Foreground(Why::Container),
        Os::Linux => Keeper::Foreground(Why::NoServiceManager),
        Os::Other => Keeper::Foreground(Why::Unsupported),
    }
}

/// **Whether this machine is a container**, by the marks its engine leaves: `/.dockerenv`
/// (Docker), `/run/.containerenv` (Podman), or the `container` variable an init is given
/// (systemd's Container Interface: nspawn, Podman, LXC).
pub fn in_a_container(root: &Path, container_var: Option<&std::ffi::OsStr>) -> bool {
    container_var.is_some_and(|value| !value.is_empty())
        || root.join(".dockerenv").exists()
        || root.join("run").join(".containerenv").exists()
}

/// Whether this account's user manager lingers: `systemd-logind` keeps a file named for it.
pub fn lingers(linger_dir: &Path, user: &str) -> bool {
    !user.is_empty() && !user.contains('/') && linger_dir.join(user).exists()
}

/// **This machine, as installing a runner's host needs to know it.** A test makes its own.
pub struct Here {
    pub keeper: Keeper,
    /// The login name, as the user database has it.
    pub user: String,
    pub uid: u32,
    /// `~/Library/LaunchAgents`.
    pub launch_agents: PathBuf,
    /// `$XDG_CONFIG_HOME/systemd/user`.
    pub user_units: PathBuf,
    /// [`LINGER_DIR`].
    pub linger_dir: PathBuf,
    /// Where a LaunchAgent's host writes its standard error.
    pub log: PathBuf,
    /// Runs `systemctl`, `loginctl` and `launchctl`.
    pub runs: Runs,
}

impl Here {
    /// This machine, for this account, with `log` where a LaunchAgent's host writes.
    pub fn this(log: PathBuf) -> Option<Here> {
        let home = purlis_same_user::account_home().or_else(dirs::home_dir)?;
        let xdg = crate::envvar::var_os("XDG_CONFIG_HOME").map(PathBuf::from);
        let config = xdg
            .filter(|dir| dir.is_absolute())
            .unwrap_or_else(|| home.join(".config"));
        let runtime = crate::envvar::var_os("XDG_RUNTIME_DIR").map(PathBuf::from);
        let container = in_a_container(
            Path::new("/"),
            crate::envvar::var_os("container").as_deref(),
        );
        Some(Here {
            keeper: keeper(
                Os::this(),
                loginitem::systemd_user_here(runtime.as_deref()),
                container,
            ),
            user: purlis_same_user::account_name().unwrap_or_default(),
            uid: rustix::process::getuid().as_raw(),
            launch_agents: home.join("Library").join("LaunchAgents"),
            user_units: config.join("systemd").join("user"),
            linger_dir: PathBuf::from(LINGER_DIR),
            log,
            runs: loginitem::runs_for_real(),
        })
    }

    fn unit_path(&self, service: &Service) -> PathBuf {
        self.user_units.join(service.unit_name())
    }

    fn plist_path(&self, service: &Service) -> PathBuf {
        self.launch_agents
            .join(format!("{}.plist", service.label()))
    }
}

/// Whether the host keeps running once this account signs out, as installing found it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Linger {
    /// It was on already.
    On,
    /// purlis turned it on.
    TurnedOn,
    /// Only an administrator may turn it on here: what `loginctl` said.
    NeedsAnAdministrator { said: String },
    /// Not a thing on this machine (macOS).
    NotHere,
}

/// What `purlis serve --install` did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Installed {
    pub keeper: Keeper,
    pub linger: Linger,
}

/// Why `purlis serve --install` handed nothing to a manager.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Refused {
    /// Nothing here to hand it to.
    Foreground(Why),
    /// The manager refused what it was asked: what it said.
    Manager(String),
}

/// **Hands `service` to this machine's manager, and starts it**: the unit written and enabled
/// with `--now`, linger turned on where it is off; or the LaunchAgent written and loaded. A host
/// the manager already runs is left running, even where its unit or agent changed: the manager
/// reads the new one at its next start.
pub fn install(here: &Here, service: &Service) -> Result<Installed, Refused> {
    let _one_at_a_time = loginitem::installing();
    match here.keeper {
        Keeper::Foreground(why) => Err(Refused::Foreground(why)),
        Keeper::SystemdUser => {
            let linger = linger_on(here);
            let changed = loginitem::write_if_changed(&here.unit_path(service), &unit(service))
                .map_err(Refused::Manager)?;
            if changed {
                loginitem::ran(&here.runs, "systemctl", &["--user", "daemon-reload"])
                    .map_err(Refused::Manager)?;
            }
            loginitem::ran(
                &here.runs,
                "systemctl",
                &["--user", "enable", "--now", &service.unit_name()],
            )
            .map_err(Refused::Manager)?;
            Ok(Installed {
                keeper: here.keeper,
                linger,
            })
        }
        Keeper::Launchd => {
            loginitem::write_if_changed(&here.plist_path(service), &plist(service, &here.log))
                .map_err(Refused::Manager)?;
            let domain = launchd_domain(here);
            let target = format!("{domain}/{}", service.label());
            if !launchd_has(here, &target) {
                let plist = here.plist_path(service).to_string_lossy().into_owned();
                match loginitem::ran(&here.runs, "launchctl", &["bootstrap", &domain, &plist]) {
                    Ok(_) => {}
                    // Loaded since it was asked, by launchd or another install.
                    Err(_) if launchd_has(here, &target) => {}
                    Err(why) => return Err(Refused::Manager(why)),
                }
            }
            Ok(Installed {
                keeper: here.keeper,
                linger: Linger::NotHere,
            })
        }
    }
}

/// Linger, on: left as it is where it is, turned on by `loginctl enable-linger` where it is not
/// (systemd lets an account turn on its own where its policy allows), or what refused it.
fn linger_on(here: &Here) -> Linger {
    if lingers(&here.linger_dir, &here.user) {
        return Linger::On;
    }
    match loginitem::ran(&here.runs, "loginctl", &["enable-linger"]) {
        Ok(_) => Linger::TurnedOn,
        Err(said) => Linger::NeedsAnAdministrator { said },
    }
}

/// The domain a LaunchAgent goes in: the account's GUI session where it has one (someone is
/// signed in at the screen), its user domain otherwise (signed in over SSH alone).
fn launchd_domain(here: &Here) -> String {
    let gui = format!("gui/{}", here.uid);
    if launchd_has(here, &gui) {
        gui
    } else {
        format!("user/{}", here.uid)
    }
}

fn launchd_has(here: &Here, target: &str) -> bool {
    (here.runs)("launchctl", &["print".to_owned(), target.to_owned()])
        .is_ok_and(|output| output.status.success())
}

/// **Takes `service` back from the manager**: stopped, disabled and its file removed. Its
/// chats end with it. Linger is left as it is: other services of this account may need it.
pub fn remove(here: &Here, service: &Service) -> Result<(), String> {
    match here.keeper {
        Keeper::Foreground(_) => Ok(()),
        Keeper::SystemdUser => {
            let path = here.unit_path(service);
            if !path.exists() {
                return Ok(());
            }
            // Not enabled or not running is what removing wants.
            let _ = loginitem::ran(
                &here.runs,
                "systemctl",
                &["--user", "disable", "--now", &service.unit_name()],
            );
            loginitem::removed(&path)?;
            loginitem::ran(&here.runs, "systemctl", &["--user", "daemon-reload"]).map(drop)
        }
        Keeper::Launchd => {
            let path = here.plist_path(service);
            if !path.exists() {
                return Ok(());
            }
            for domain in [format!("gui/{}", here.uid), format!("user/{}", here.uid)] {
                let target = format!("{domain}/{}", service.label());
                if launchd_has(here, &target) {
                    let _ = loginitem::ran(&here.runs, "launchctl", &["bootout", &target]);
                }
            }
            loginitem::removed(&path)
        }
    }
}

/// **Takes the service of the installed version `version` back from the manager, by its name
/// alone** ([`Service::unit_name`], [`Service::label`]): what pruning a drained version does
/// before it deletes the version's folders (#6688). The version's own binary is never run, since
/// an older one may not know `serve --remove`, and it is about to go. An error means the service
/// may still be there, so the version is to be kept.
pub fn remove_version(here: &Here, version: &str) -> Result<(), String> {
    remove(
        here,
        &Service {
            binary: PathBuf::new(),
            version: Some(version.to_owned()),
        },
    )
}

/// **What doctor finds of a runner's host** (read by [`look`], judged by [`rows`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Look {
    pub keeper: Keeper,
    pub service: Service,
    /// Whether the unit or agent file is there.
    pub installed: bool,
    /// Whether the manager will start it: the unit enabled, the agent loaded.
    pub enabled: bool,
    /// Whether this account lingers, on Linux with a user manager.
    pub lingers: Option<bool>,
    pub user: String,
    /// Where its host listens.
    pub socket: PathBuf,
    /// Whether a host answers there.
    pub reachable: bool,
}

/// **What doctor finds here** of `service`'s host, listening at `socket`.
pub fn look(here: &Here, service: &Service, socket: &Path) -> Look {
    let (installed, enabled, lingers) = match here.keeper {
        Keeper::SystemdUser => {
            let installed = here.unit_path(service).exists();
            let enabled = installed
                && loginitem::ran(
                    &here.runs,
                    "systemctl",
                    &["--user", "is-enabled", "--quiet", &service.unit_name()],
                )
                .is_ok();
            (
                installed,
                enabled,
                Some(lingers(&here.linger_dir, &here.user)),
            )
        }
        Keeper::Launchd => {
            let installed = here.plist_path(service).exists();
            let enabled = installed
                && [format!("gui/{}", here.uid), format!("user/{}", here.uid)]
                    .iter()
                    .any(|domain| launchd_has(here, &format!("{domain}/{}", service.label())));
            (installed, enabled, None)
        }
        Keeper::Foreground(_) => (false, false, None),
    };
    Look {
        keeper: here.keeper,
        service: service.clone(),
        installed,
        enabled,
        lingers,
        user: here.user.clone(),
        socket: socket.to_path_buf(),
        reachable: answers(socket),
    }
}

/// Whether something listens at `socket` (the host, which admits or refuses a client after).
#[cfg(unix)]
fn answers(socket: &Path) -> bool {
    std::os::unix::net::UnixStream::connect(socket).is_ok()
}

#[cfg(not(unix))]
fn answers(_socket: &Path) -> bool {
    false
}

/// A row of doctor's, as plain parts: its name, how it stands, what was found, and what to do.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Finding {
    pub name: &'static str,
    pub status: Status,
    pub detail: String,
    pub hint: String,
}

/// How a finding stands.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    Ok,
    Warn,
    Fail,
}

/// The row that says what keeps the host running.
pub const SERVICE_ROW: &str = "runner host service";
/// The row that says whether it outlives a sign-out.
pub const LINGER_ROW: &str = "runner host after sign-out";
/// The row that says whether it answers.
pub const REACHABLE_ROW: &str = "runner host";

/// **What doctor says of a runner's host**: what keeps it running, whether it outlives this
/// account's sign-out (Linux), and whether it answers now.
pub fn rows(look: &Look) -> Vec<Finding> {
    let install = format!("{} serve --install", shell_word(&look.service.binary));
    let mut found = Vec::new();
    let service = match look.keeper {
        Keeper::Foreground(Why::Container) => Finding {
            name: SERVICE_ROW,
            status: Status::Ok,
            detail: "this machine is a container, so the container keeps the host running"
                .to_owned(),
            hint: "Run `purlis serve --foreground` as the container's command, with a restart \
                   policy such as `--restart unless-stopped`."
                .to_owned(),
        },
        Keeper::Foreground(_) => Finding {
            name: SERVICE_ROW,
            status: Status::Warn,
            detail: "nothing on this machine restarts the host if it stops: there is no \
                     service manager purlis can use for your account"
                .to_owned(),
            hint: "Run `purlis serve --foreground` under whatever keeps programs running here."
                .to_owned(),
        },
        _ if look.installed && look.enabled => Finding {
            name: SERVICE_ROW,
            status: Status::Ok,
            detail: match look.keeper {
                Keeper::SystemdUser => format!(
                    "systemd keeps it running as {}, and starts it again after a restart",
                    look.service.unit_name()
                ),
                _ => format!(
                    "launchd keeps it running as {}, and starts it again when you next sign \
                     in after a restart",
                    look.service.label()
                ),
            },
            hint: String::new(),
        },
        _ if look.installed => Finding {
            name: SERVICE_ROW,
            status: Status::Warn,
            detail: "the host's service is written but switched off, so it will not start \
                     again after a restart"
                .to_owned(),
            hint: format!("Run `{install}` to switch it on."),
        },
        _ => Finding {
            name: SERVICE_ROW,
            status: Status::Warn,
            detail: "the host is not kept running by this machine: it stops when you sign \
                     out or restart the machine"
                .to_owned(),
            hint: format!("Run `{install}`."),
        },
    };
    found.push(service);
    if let Some(lingers) = look.lingers {
        found.push(if lingers {
            Finding {
                name: LINGER_ROW,
                status: Status::Ok,
                detail: "your chats keep running after you sign out".to_owned(),
                hint: String::new(),
            }
        } else {
            Finding {
                name: LINGER_ROW,
                status: Status::Warn,
                detail: "your chats stop when you sign out of this machine".to_owned(),
                hint: linger_hint(&look.user),
            }
        });
    }
    found.push(if look.reachable {
        Finding {
            name: REACHABLE_ROW,
            status: Status::Ok,
            detail: format!("answering at {}", look.socket.display()),
            hint: String::new(),
        }
    } else {
        Finding {
            name: REACHABLE_ROW,
            status: Status::Fail,
            detail: format!("nothing answers at {}", look.socket.display()),
            hint: match look.keeper {
                Keeper::SystemdUser if look.installed => format!(
                    "Run `systemctl --user status {}` to see why it stopped, then `{install}` \
                     to start it.",
                    look.service.unit_name()
                ),
                Keeper::Foreground(_) => "Start it with `purlis serve --foreground`.".to_owned(),
                _ => format!("Run `{install}` to start it."),
            },
        }
    });
    found
}

/// What to do where linger is off: turn it on, or ask an administrator to.
pub fn linger_hint(user: &str) -> String {
    let user = if user.is_empty() { "<you>" } else { user };
    format!(
        "Run `loginctl enable-linger`. If this machine refuses, ask an administrator to run \
         `sudo loginctl enable-linger {user}`."
    )
}

/// **What `purlis serve --install` says it did**, in plain words, one line each, for the
/// account `user`.
pub fn said(installed: &Installed, service: &Service, user: &str) -> Vec<String> {
    let mut lines = vec![match installed.keeper {
        Keeper::SystemdUser => format!(
            "✓ The session host runs as {}, and systemd starts it again if it stops or the \
             machine restarts.",
            service.unit_name()
        ),
        _ => format!(
            "✓ The session host runs as {}, and launchd starts it again if it stops.",
            service.label()
        ),
    }];
    match &installed.linger {
        Linger::On | Linger::TurnedOn => {
            lines.push("✓ Your chats keep running after you sign out.".to_owned());
        }
        Linger::NeedsAnAdministrator { .. } => {
            lines.push(
                "! Your chats will stop when you sign out of this machine, and start again \
                 only when you sign in. This machine lets only an administrator change that."
                    .to_owned(),
            );
        }
        Linger::NotHere => lines.push(
            "! After this Mac restarts, the host starts again when this account signs in. To \
             have it start without anyone signing in, turn on automatic login for this account."
                .to_owned(),
        ),
    }
    if let Linger::NeedsAnAdministrator { said } = &installed.linger {
        let user = if user.is_empty() { "<you>" } else { user };
        lines.push(format!(
            "  Ask an administrator to run `sudo loginctl enable-linger {user}`."
        ));
        if let Some(said) = said.lines().next().filter(|line| !line.is_empty()) {
            lines.push(format!("  ({said})"));
        }
    }
    lines
}

/// **What `purlis serve --install` says where nothing can keep the host running.**
pub fn refused(why: Why) -> String {
    match why {
        Why::Container => "This machine is a container, so purlis has no service manager to \
                           hand the host to. Run `purlis serve --foreground` as the container's \
                           command instead, and give the container a restart policy such as \
                           `--restart unless-stopped`."
            .to_owned(),
        Why::NoServiceManager => "This machine has no service manager purlis can use for your \
                                  account (systemd's user manager is not running for you), so \
                                  nothing would start the host again. Run `purlis serve \
                                  --foreground` under whatever keeps programs running here."
            .to_owned(),
        Why::Unsupported => "The session host runs on macOS and Linux only.".to_owned(),
    }
}

/// `path` as one word a POSIX shell reads back unchanged: as it is where it is plain, else in
/// single quotes.
fn shell_word(path: &Path) -> String {
    let text = path.to_string_lossy();
    if !text.is_empty()
        && text
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || "/._+@-".contains(c))
    {
        text.into_owned()
    } else {
        format!("'{}'", text.replace('\'', r"'\''"))
    }
}

#[cfg(test)]
mod tests;
