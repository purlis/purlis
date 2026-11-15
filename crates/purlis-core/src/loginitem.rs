// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The session host as a login item** (FD-17, #655; ADR 0068 §2 and §3, X24).
//!
//! X24's ruling: the session host is a login item only when the person has a **trigger** or a
//! **paired remote device**, shows in the tray with a one-click stop, and otherwise ends with
//! the app. This module holds the parts of that which are plain Rust:
//!
//! - **Whether anything keeps the host running** ([`reasons_here`]), the one place that answers
//!   "a trigger or a device". Neither exists yet: local triggers are AC-9 (M38, #560) and a
//!   paired device is the relay's phone or PWA (ADR 0078 as it amends ADR 0068 §2). A runner
//!   is not one: a runner is a device this host connects *to*. Until they land the answer is
//!   none, and the host ends with the app, as before.
//! - **Who supervises the host** ([`supervisor`]): the operating system's service manager where
//!   the host is a login item and there is one (a launchd agent on macOS, a systemd user unit on
//!   Linux with a user manager), and the window everywhere else (ADR 0068 §3, D-2007-1).
//! - **What each manager is given** ([`host_plist`], [`host_unit`]): the host restarted after
//!   an unexpected exit, with the limits the window's own breaker keeps (three exits within five
//!   minutes); and the app started at login in the background, so the tray is there after a
//!   restart of the machine ([`app_plist`], [`app_autostart`]).
//! - **Installing and removing them** ([`Manager`], [`Launchd`], [`SystemdUser`]): the files are
//!   written whole, and the manager is told by its own command (`launchctl`, `systemctl --user`).
//!   Nothing reads what those commands print but `systemctl show`'s one property, which is its
//!   interface for scripts.
//!
//! Windows has no session host (ADR 0031), so nothing here runs there.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::Arc;

/// Something that keeps the session host running after the app quits (X24).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Reason {
    /// A trigger that starts chats on its own (AC-9), by the name the person gave it.
    Trigger { name: String },
    /// A device paired to watch and answer this machine's chats (the relay's phone or PWA), by
    /// its name.
    Device { name: String },
}

/// **What keeps the host running on this machine now** (X24): its triggers and its paired
/// devices. None of either exists yet (AC-9; the relay's pairing), so this is always empty, and
/// the host ends with the app as it always has. Each lands here as it is built.
pub fn reasons_here(_config: &Path) -> Vec<Reason> {
    Vec::new()
}

/// **Why the host keeps running, in the window's words**, for the tray: `None` when nothing
/// keeps it. Names one reason by name; more are counted.
pub fn kept_for(reasons: &[Reason]) -> Option<String> {
    let triggers: Vec<&str> = reasons
        .iter()
        .filter_map(|reason| match reason {
            Reason::Trigger { name } => Some(name.as_str()),
            Reason::Device { .. } => None,
        })
        .collect();
    let devices: Vec<&str> = reasons
        .iter()
        .filter_map(|reason| match reason {
            Reason::Device { name } => Some(name.as_str()),
            Reason::Trigger { .. } => None,
        })
        .collect();
    let said = |names: &[&str], one: &str, many: &str| match names {
        [] => None,
        [name] => Some(format!("your {one} “{name}”")),
        more => Some(format!("{} {many}", more.len())),
    };
    let parts: Vec<String> = [
        said(&triggers, "trigger", "triggers"),
        said(&devices, "paired device", "paired devices"),
    ]
    .into_iter()
    .flatten()
    .collect();
    (!parts.is_empty()).then(|| parts.join(" and "))
}

/// The operating system purlis runs on, as this module needs to know it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Os {
    Macos,
    Linux,
    /// Windows, or anything else: no session host here.
    Other,
}

impl Os {
    pub fn this() -> Os {
        if cfg!(target_os = "macos") {
            Os::Macos
        } else if cfg!(target_os = "linux") {
            Os::Linux
        } else {
            Os::Other
        }
    }
}

/// Who supervises the session host (ADR 0068 §3).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Supervisor {
    /// The window starts it, starts it again, and holds its breaker; it ends with the app.
    Window,
    /// A launchd agent in the person's GUI session, which starts it at login.
    Launchd,
    /// A systemd user unit, enabled, which starts it at login.
    SystemdUser,
}

/// **Who supervises the host here**: the service manager when something keeps the host running
/// and there is one purlis may use; the window otherwise. `systemd_user` is whether this Linux
/// session has a systemd user manager ([`systemd_user_here`]).
pub fn supervisor(reasons: &[Reason], os: Os, systemd_user: bool) -> Supervisor {
    if reasons.is_empty() {
        return Supervisor::Window;
    }
    match os {
        Os::Macos => Supervisor::Launchd,
        Os::Linux if systemd_user => Supervisor::SystemdUser,
        Os::Linux | Os::Other => Supervisor::Window,
    }
}

/// Whether this session has a systemd user manager: its private socket is in the runtime
/// folder, as `sd_booted` reads `/run/systemd/system` for the system's.
pub fn systemd_user_here(runtime_dir: Option<&Path>) -> bool {
    runtime_dir.is_some_and(|dir| dir.join("systemd").join("private").exists())
}

/// **What this build's login items are called**: a development build's are its own, so one
/// never loads, rewrites or removes a release build's on the same machine.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Names {
    /// The launchd label of the host's agent, and its plist's name.
    pub host_label: &'static str,
    /// The launchd label of the agent that starts the app in the background at login.
    pub app_label: &'static str,
    /// The systemd user unit that runs the host.
    pub host_unit: &'static str,
    /// The XDG autostart entry's file name.
    pub app_autostart: &'static str,
}

impl Names {
    /// A release build's.
    pub const RELEASE: Names = Names {
        host_label: "dev.purlis.host",
        app_label: "dev.purlis.app.background",
        host_unit: "purlis-host.service",
        app_autostart: "purlis.desktop",
    };

    /// A development build's.
    pub const DEVELOPMENT: Names = Names {
        host_label: "dev.purlis.host.development",
        app_label: "dev.purlis.app.background.development",
        host_unit: "purlis-host-development.service",
        app_autostart: "purlis-development.desktop",
    };

    /// This build's.
    pub fn here() -> Names {
        if cfg!(debug_assertions) {
            Names::DEVELOPMENT
        } else {
            Names::RELEASE
        }
    }
}

/// How many unexpected exits within [`WITHIN_SECONDS`] stop a manager restarting the host: the
/// window's own breaker's limits (D-2007-2).
pub const EXITS: u32 = 3;

/// How far back an unexpected exit counts, in seconds.
pub const WITHIN_SECONDS: u32 = 300;

/// A program a manager starts: its path and its arguments.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Job {
    pub program: PathBuf,
    pub args: Vec<String>,
}

/// **The host's launchd agent**: started at login in the person's GUI session (where the
/// Keychain answers without a prompt), and again after an exit that was not a success, which
/// launchd throttles on its own. A host that ends because it was told to quit exits 0 and is
/// left stopped. `log` is where what it says on standard error goes.
pub fn host_plist(names: &Names, job: &Job, log: &Path) -> String {
    let mut keep_alive = plist::Dictionary::new();
    keep_alive.insert("SuccessfulExit".into(), false.into());
    let mut agent = plist::Dictionary::new();
    agent.insert("Label".into(), names.host_label.into());
    agent.insert("ProgramArguments".into(), arguments(job));
    agent.insert("RunAtLoad".into(), true.into());
    agent.insert("KeepAlive".into(), keep_alive.into());
    agent.insert("LimitLoadToSessionType".into(), "Aqua".into());
    agent.insert(
        "StandardErrorPath".into(),
        log.to_string_lossy().into_owned().into(),
    );
    written(agent)
}

/// A job's program and arguments, as `ProgramArguments`.
fn arguments(job: &Job) -> plist::Value {
    plist::Value::Array(
        std::iter::once(job.program.to_string_lossy().into_owned())
            .chain(job.args.iter().cloned())
            .map(plist::Value::String)
            .collect(),
    )
}

/// A property list written as XML, the form launchd reads and a person can.
fn written(dictionary: plist::Dictionary) -> String {
    let mut xml = Vec::new();
    // Into memory, which cannot fail.
    let _ = plist::Value::Dictionary(dictionary).to_writer_xml(&mut xml);
    xml.push(b'\n');
    String::from_utf8_lossy(&xml).into_owned()
}

/// **The agent that starts the app in the background at login**: once, at load, with no
/// restart, so the tray is there to say why the host runs and to stop it.
pub fn app_plist(names: &Names, job: &Job) -> String {
    let mut agent = plist::Dictionary::new();
    agent.insert("Label".into(), names.app_label.into());
    agent.insert("ProgramArguments".into(), arguments(job));
    agent.insert("RunAtLoad".into(), true.into());
    agent.insert("LimitLoadToSessionType".into(), "Aqua".into());
    written(agent)
}

/// **The host's systemd user unit**: enabled into `graphical-session.target`, so it starts with
/// the person's desktop, after it (so the host, and the browsers it runs, have the session's
/// `DISPLAY` or `WAYLAND_DISPLAY`) and stops with it; restarted on failure, and held stopped after [`EXITS`] failures within [`WITHIN_SECONDS`],
/// systemd's own breaker.
pub fn host_unit(job: &Job) -> String {
    let command: Vec<String> = std::iter::once(job.program.to_string_lossy().into_owned())
        .chain(job.args.iter().cloned())
        .map(|word| systemd_quoted(&word))
        .collect();
    format!(
        "[Unit]\n\
         Description=purlis session host\n\
         PartOf=graphical-session.target\n\
         After=graphical-session.target\n\
         StartLimitIntervalSec={WITHIN_SECONDS}\n\
         StartLimitBurst={EXITS}\n\
         \n\
         [Service]\n\
         Type=simple\n\
         ExecStart={}\n\
         Restart=on-failure\n\
         \n\
         [Install]\n\
         WantedBy=graphical-session.target\n",
        command.join(" ")
    )
}

/// One word of a unit's command line, double-quoted, with what systemd would expand escaped:
/// a backslash and a quote by a backslash, `$` and `%` by doubling (`systemd.service(5)`,
/// "Command lines"; `systemd.unit(5)`, "Specifiers").
fn systemd_quoted(word: &str) -> String {
    let mut quoted = String::from("\"");
    for c in word.chars() {
        match c {
            '\\' => quoted.push_str("\\\\"),
            '"' => quoted.push_str("\\\""),
            '$' => quoted.push_str("$$"),
            '%' => quoted.push_str("%%"),
            c => quoted.push(c),
        }
    }
    quoted.push('"');
    quoted
}

/// **The XDG autostart entry that starts the app in the background at login**, the desktop's
/// own way to start a program with the session.
pub fn app_autostart(job: &Job) -> String {
    let command: Vec<String> = std::iter::once(job.program.to_string_lossy().into_owned())
        .chain(job.args.iter().cloned())
        .map(|word| desktop_quoted(&word))
        .collect();
    format!(
        "[Desktop Entry]\n\
         Type=Application\n\
         Name=purlis\n\
         Comment=Starts purlis in the background, for what keeps it running\n\
         Exec={}\n\
         NoDisplay=true\n\
         X-GNOME-Autostart-enabled=true\n",
        command.join(" ")
    )
}

/// One argument of an entry's `Exec`, as the Desktop Entry Specification quotes it: inside
/// double quotes, a quote, backquote, dollar sign or backslash after a backslash, and `%`
/// doubled; then, since the value is a string, every backslash written twice.
fn desktop_quoted(word: &str) -> String {
    let mut quoted = String::from("\"");
    for c in word.chars() {
        match c {
            '"' | '`' | '$' | '\\' => {
                quoted.push('\\');
                quoted.push(c);
            }
            '%' => quoted.push_str("%%"),
            c => quoted.push(c),
        }
    }
    quoted.push('"');
    quoted.replace('\\', "\\\\")
}

/// Where each manager's files go, and what this build calls them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Places {
    /// `~/Library/LaunchAgents/dev.purlis.host.plist`
    pub host_plist: PathBuf,
    /// `~/Library/LaunchAgents/dev.purlis.app.background.plist`
    pub app_plist: PathBuf,
    /// `$XDG_CONFIG_HOME/systemd/user/purlis-host.service`
    pub host_unit: PathBuf,
    /// `$XDG_CONFIG_HOME/autostart/purlis.desktop`
    pub app_autostart: PathBuf,
    /// The labels and the unit's name the files are written with.
    pub names: Names,
}

/// The files' places under `home`, with `xdg_config` where `XDG_CONFIG_HOME` names one, for a
/// build that calls them `names`.
pub fn places(home: &Path, xdg_config: Option<&Path>, names: Names) -> Places {
    let agents = home.join("Library").join("LaunchAgents");
    let config = xdg_config
        .filter(|dir| dir.is_absolute())
        .map_or_else(|| home.join(".config"), Path::to_path_buf);
    Places {
        host_plist: agents.join(format!("{}.plist", names.host_label)),
        app_plist: agents.join(format!("{}.plist", names.app_label)),
        host_unit: config.join("systemd").join("user").join(names.host_unit),
        app_autostart: config.join("autostart").join(names.app_autostart),
        names,
    }
}

/// This build's files' places for this user: under the home folder, with `XDG_CONFIG_HOME`
/// where it names one.
pub fn places_here() -> Option<Places> {
    let home = dirs::home_dir()?;
    let xdg = crate::envvar::var_os("XDG_CONFIG_HOME").map(PathBuf::from);
    Some(places(&home, xdg.as_deref(), Names::here()))
}

/// Runs a manager's command (`launchctl`, `systemctl`) and gives what it printed and how it
/// ended. A test records the commands instead.
pub type Runs = Arc<dyn Fn(&str, &[String]) -> std::io::Result<Output> + Send + Sync>;

/// The commands run for real, through the fork lock.
pub fn runs_for_real() -> Runs {
    Arc::new(|program, args| crate::forklock::output(Command::new(program).args(args)))
}

/// **A service manager that supervises the host as a login item** (ADR 0068 §3).
pub trait Manager: Send + Sync {
    /// Writes the host's and the app's login items and loads the host's, so it starts at login
    /// and is restarted on failure. Leaves a host the manager already runs running.
    fn install(&self) -> Result<(), String>;
    /// Starts the host now, if the manager is not running it.
    fn start(&self) -> Result<(), String>;
    /// Stops the manager restarting the host: the window's breaker tripped.
    fn hold(&self) -> Result<(), String>;
    /// Whether the manager's own restart limit stopped it restarting the host.
    fn tripped(&self) -> bool;
    /// The person pressed Try again: whatever held the host is let go of, and it is started.
    fn try_again(&self) -> Result<(), String>;
    /// Removes both login items: nothing keeps the host running any more. Called once the host
    /// has quit.
    fn remove(&self) -> Result<(), String>;
}

/// **launchd, in the person's GUI domain** (`gui/<uid>`).
pub struct Launchd {
    pub places: Places,
    pub host: Job,
    pub app: Job,
    /// Where the host's standard error goes.
    pub log: PathBuf,
    pub uid: u32,
    pub runs: Runs,
}

/// **The systemd user manager** (`systemctl --user`).
pub struct SystemdUser {
    pub places: Places,
    pub host: Job,
    pub app: Job,
    pub runs: Runs,
}

/// **This platform's service manager, whether or not anything keeps the host running now**: so
/// a login item an earlier launch installed is removed once nothing does. launchd on macOS, the
/// systemd user manager on Linux where there is one, and none elsewhere. Answers which it is.
pub fn manager_on(
    os: Os,
    systemd_user: bool,
    places: Places,
    host: Job,
    app: Job,
    log: PathBuf,
    runs: Runs,
) -> Option<(Supervisor, Arc<dyn Manager>)> {
    match os {
        Os::Macos => Some((
            Supervisor::Launchd,
            Arc::new(Launchd {
                places,
                host,
                app,
                log,
                uid: rustix::process::getuid().as_raw(),
                runs,
            }),
        )),
        Os::Linux if systemd_user => Some((
            Supervisor::SystemdUser,
            Arc::new(SystemdUser {
                places,
                host,
                app,
                runs,
            }),
        )),
        Os::Linux | Os::Other => None,
    }
}

/// Runs a manager's command, failing with which command and what it said.
fn ran(runs: &Runs, program: &str, args: &[&str]) -> Result<Output, String> {
    let args: Vec<String> = args.iter().map(|arg| (*arg).to_owned()).collect();
    let line = || format!("{program} {}", args.join(" "));
    let output = runs(program, &args).map_err(|why| format!("`{}` did not run: {why}", line()))?;
    if output.status.success() {
        Ok(output)
    } else {
        Err(format!(
            "`{}` failed: {}",
            line(),
            String::from_utf8_lossy(&output.stderr).trim()
        ))
    }
}

/// Writes `contents` at `path`, whole, `0600`, unless it already holds them: whether it wrote.
fn write_if_changed(path: &Path, contents: &str) -> Result<bool, String> {
    if std::fs::read(path).is_ok_and(|held| held == contents.as_bytes()) {
        return Ok(false);
    }
    let dir = path
        .parent()
        .ok_or_else(|| format!("{} has no folder", path.display()))?;
    crate::rewrite::create_dir_all(dir)
        .and_then(|()| {
            crate::rewrite::replace(
                dir,
                path,
                contents.as_bytes(),
                crate::rewrite::Mode::Private,
            )
        })
        .map_err(|why| format!("{} could not be written: {why}", path.display()))?;
    Ok(true)
}

/// One install at a time in this process: two at once (the window's supervisor and a quit to
/// the background) would each find the agent not loaded and each load it.
fn installing() -> std::sync::MutexGuard<'static, ()> {
    static INSTALLING: std::sync::Mutex<()> = std::sync::Mutex::new(());
    INSTALLING
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

/// Removes `path`, which may not be there.
fn removed(path: &Path) -> Result<(), String> {
    match std::fs::remove_file(path) {
        Err(why) if why.kind() != std::io::ErrorKind::NotFound => {
            Err(format!("{} could not be removed: {why}", path.display()))
        }
        _ => Ok(()),
    }
}

impl Launchd {
    fn domain(&self) -> String {
        format!("gui/{}", self.uid)
    }

    fn target(&self) -> String {
        format!("gui/{}/{}", self.uid, self.places.names.host_label)
    }

    /// Whether launchd has the host's agent loaded.
    fn loaded(&self) -> bool {
        (self.runs)("launchctl", &["print".to_owned(), self.target()])
            .is_ok_and(|output| output.status.success())
    }

    /// Whether either agent is written, so there is something to remove.
    fn installed(&self) -> bool {
        self.places.host_plist.exists() || self.places.app_plist.exists()
    }
}

impl Manager for Launchd {
    fn install(&self) -> Result<(), String> {
        let _one_at_a_time = installing();
        write_if_changed(
            &self.places.host_plist,
            &host_plist(&self.places.names, &self.host, &self.log),
        )?;
        // Loaded at the next login, never now: loading it would start a second app.
        write_if_changed(
            &self.places.app_plist,
            &app_plist(&self.places.names, &self.app),
        )?;
        // A host launchd runs is never booted out for a changed agent: that would end every
        // chat in it. launchd reads the agent written here the next time it loads it, at the
        // next login or after a hold.
        if self.loaded() {
            return Ok(());
        }
        let plist = self.places.host_plist.to_string_lossy();
        match ran(
            &self.runs,
            "launchctl",
            &["bootstrap", &self.domain(), &plist],
        ) {
            Ok(_) => Ok(()),
            // launchd refuses to load an agent it already has (its error 5): another install,
            // or launchd itself, loaded it since it was asked. Loaded is what was wanted.
            Err(_) if self.loaded() => Ok(()),
            Err(why) => Err(why),
        }
    }

    fn start(&self) -> Result<(), String> {
        ran(&self.runs, "launchctl", &["kickstart", &self.target()]).map(drop)
    }

    fn hold(&self) -> Result<(), String> {
        ran(&self.runs, "launchctl", &["bootout", &self.target()]).map(drop)
    }

    /// launchd throttles a job's restarts and never gives up on it: the window's breaker is the
    /// only limit, and [`Manager::hold`] what it does when it trips.
    fn tripped(&self) -> bool {
        false
    }

    fn try_again(&self) -> Result<(), String> {
        self.install()?;
        self.start()
    }

    fn remove(&self) -> Result<(), String> {
        // Never installed, nothing to ask launchd: a quit changes nothing for anyone who never
        // had something keep the host running.
        if !self.installed() {
            return Ok(());
        }
        // Not loaded is not a failure: it is what removing wants.
        let _ = ran(&self.runs, "launchctl", &["bootout", &self.target()]);
        removed(&self.places.host_plist)?;
        removed(&self.places.app_plist)
    }
}

impl SystemdUser {
    fn unit(&self) -> &'static str {
        self.places.names.host_unit
    }
}

impl Manager for SystemdUser {
    fn install(&self) -> Result<(), String> {
        let _one_at_a_time = installing();
        let changed = write_if_changed(&self.places.host_unit, &host_unit(&self.host))?;
        write_if_changed(&self.places.app_autostart, &app_autostart(&self.app))?;
        if changed {
            ran(&self.runs, "systemctl", &["--user", "daemon-reload"])?;
        }
        ran(&self.runs, "systemctl", &["--user", "enable", self.unit()]).map(drop)
    }

    fn start(&self) -> Result<(), String> {
        ran(&self.runs, "systemctl", &["--user", "start", self.unit()]).map(drop)
    }

    fn hold(&self) -> Result<(), String> {
        ran(&self.runs, "systemctl", &["--user", "stop", self.unit()]).map(drop)
    }

    /// The unit's `Result` property, which systemd sets to `start-limit-hit` once
    /// `StartLimitBurst` failures came within `StartLimitIntervalSec`.
    fn tripped(&self) -> bool {
        ran(
            &self.runs,
            "systemctl",
            &[
                "--user",
                "show",
                "--property=Result",
                "--value",
                self.unit(),
            ],
        )
        .is_ok_and(|output| String::from_utf8_lossy(&output.stdout).trim() == "start-limit-hit")
    }

    fn try_again(&self) -> Result<(), String> {
        // A unit that is not failed has nothing to reset.
        let _ = ran(
            &self.runs,
            "systemctl",
            &["--user", "reset-failed", self.unit()],
        );
        self.start()
    }

    fn remove(&self) -> Result<(), String> {
        // Never installed, nothing to ask systemd.
        if !self.places.host_unit.exists() && !self.places.app_autostart.exists() {
            return Ok(());
        }
        let _ = ran(
            &self.runs,
            "systemctl",
            &["--user", "disable", "--now", self.unit()],
        );
        removed(&self.places.host_unit)?;
        removed(&self.places.app_autostart)?;
        let _ = ran(&self.runs, "systemctl", &["--user", "daemon-reload"]);
        Ok(())
    }
}

#[cfg(test)]
mod tests;
