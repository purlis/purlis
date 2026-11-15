// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use std::os::unix::process::ExitStatusExt as _;
use std::path::{Path, PathBuf};
use std::process::{ExitStatus, Output};
use std::sync::{Arc, Mutex};

use super::*;

fn trigger(name: &str) -> Reason {
    Reason::Trigger { name: name.into() }
}

fn device(name: &str) -> Reason {
    Reason::Device { name: name.into() }
}

fn host_job() -> Job {
    Job {
        program: PathBuf::from("/Applications/purl & is <x>.app/Contents/MacOS/purlis"),
        args: vec!["purlis-serve-as-a-login-item".into()],
    }
}

fn app_job() -> Job {
    Job {
        program: PathBuf::from("/opt/pur lis/\"purlis\""),
        args: vec!["--in-the-background".into()],
    }
}

fn dict(plist: &str) -> plist::Dictionary {
    plist::Value::from_reader_xml(plist.as_bytes())
        .expect("the plist reads")
        .into_dictionary()
        .expect("a plist is a dictionary")
}

#[test]
fn nothing_keeps_the_host_running_until_triggers_or_paired_devices_exist() {
    let config = tempfile::tempdir().unwrap();
    assert_eq!(reasons_here(config.path()), Vec::new());
    assert_eq!(kept_for(&[]), None);
}

#[test]
fn why_the_host_keeps_running_names_one_reason_and_counts_more() {
    assert_eq!(
        kept_for(&[trigger("nightly review")]).as_deref(),
        Some("your trigger “nightly review”")
    );
    assert_eq!(
        kept_for(&[device("Ana’s phone")]).as_deref(),
        Some("your paired device “Ana’s phone”")
    );
    assert_eq!(
        kept_for(&[trigger("a"), trigger("b")]).as_deref(),
        Some("2 triggers")
    );
    assert_eq!(
        kept_for(&[trigger("a"), device("phone")]).as_deref(),
        Some("your trigger “a” and your paired device “phone”")
    );
    assert_eq!(
        kept_for(&[trigger("a"), trigger("b"), device("p"), device("q")]).as_deref(),
        Some("2 triggers and 2 paired devices")
    );
}

#[test]
fn the_window_supervises_a_host_nothing_keeps_running_on_every_platform() {
    for os in [Os::Macos, Os::Linux, Os::Other] {
        assert_eq!(supervisor(&[], os, true), Supervisor::Window, "{os:?}");
    }
}

#[test]
fn a_host_kept_running_is_supervised_by_launchd_on_macos_and_systemd_on_linux() {
    let kept = [trigger("nightly")];
    assert_eq!(supervisor(&kept, Os::Macos, false), Supervisor::Launchd);
    assert_eq!(supervisor(&kept, Os::Linux, true), Supervisor::SystemdUser);
}

#[test]
fn where_no_manager_may_be_used_the_window_supervises_a_host_kept_running() {
    let kept = [device("phone")];
    assert_eq!(supervisor(&kept, Os::Linux, false), Supervisor::Window);
    assert_eq!(supervisor(&kept, Os::Other, true), Supervisor::Window);
}

#[test]
fn a_systemd_user_manager_is_found_by_its_private_socket() {
    let runtime = tempfile::tempdir().unwrap();
    assert!(!systemd_user_here(Some(runtime.path())));
    assert!(!systemd_user_here(None));
    std::fs::create_dir_all(runtime.path().join("systemd")).unwrap();
    std::fs::write(runtime.path().join("systemd/private"), "").unwrap();
    assert!(systemd_user_here(Some(runtime.path())));
}

#[test]
fn the_hosts_launchd_agent_restarts_it_only_after_an_exit_that_was_not_a_success() {
    let plist = dict(&host_plist(
        &host_job(),
        Path::new("/Users/me/log/host.log"),
    ));
    assert_eq!(
        plist.get("Label").and_then(plist::Value::as_string),
        Some(HOST_LABEL)
    );
    let args: Vec<&str> = plist
        .get("ProgramArguments")
        .and_then(plist::Value::as_array)
        .expect("its arguments")
        .iter()
        .filter_map(plist::Value::as_string)
        .collect();
    assert_eq!(
        args,
        [
            "/Applications/purl & is <x>.app/Contents/MacOS/purlis",
            "purlis-serve-as-a-login-item"
        ]
    );
    let keep_alive = plist
        .get("KeepAlive")
        .and_then(plist::Value::as_dictionary)
        .expect("KeepAlive is a dictionary of conditions");
    assert_eq!(
        keep_alive
            .get("SuccessfulExit")
            .and_then(plist::Value::as_boolean),
        Some(false)
    );
    assert_eq!(
        plist.get("RunAtLoad").and_then(plist::Value::as_boolean),
        Some(true)
    );
    // In the person's GUI session, where the Keychain answers the app's own binary.
    assert_eq!(
        plist
            .get("LimitLoadToSessionType")
            .and_then(plist::Value::as_string),
        Some("Aqua")
    );
    assert_eq!(
        plist
            .get("StandardErrorPath")
            .and_then(plist::Value::as_string),
        Some("/Users/me/log/host.log")
    );
}

#[test]
fn the_apps_launchd_agent_starts_it_once_at_login_in_the_background() {
    let plist = dict(&app_plist(&app_job()));
    assert_eq!(
        plist.get("Label").and_then(plist::Value::as_string),
        Some(APP_LABEL)
    );
    assert_eq!(
        plist.get("RunAtLoad").and_then(plist::Value::as_boolean),
        Some(true)
    );
    assert!(plist.get("KeepAlive").is_none(), "a quit app stays quit");
    let args: Vec<&str> = plist
        .get("ProgramArguments")
        .and_then(plist::Value::as_array)
        .expect("its arguments")
        .iter()
        .filter_map(plist::Value::as_string)
        .collect();
    assert_eq!(args, ["/opt/pur lis/\"purlis\"", "--in-the-background"]);
}

#[test]
fn the_hosts_systemd_unit_restarts_it_on_failure_with_the_breakers_limits() {
    let unit = host_unit(&host_job());
    let section = |name: &str| -> Vec<String> {
        unit.split("\n[")
            .find(|part| {
                part.trim_start_matches('[')
                    .starts_with(&format!("{name}]"))
            })
            .unwrap_or_else(|| panic!("a [{name}] section in\n{unit}"))
            .lines()
            .skip(1)
            .map(str::to_owned)
            .collect()
    };
    let unit_section = section("Unit");
    assert!(
        unit_section.contains(&"StartLimitBurst=3".to_owned()),
        "{unit}"
    );
    assert!(
        unit_section.contains(&"StartLimitIntervalSec=300".to_owned()),
        "{unit}"
    );
    let service = section("Service");
    assert!(service.contains(&"Restart=on-failure".to_owned()), "{unit}");
    assert!(
        service.contains(
            &"ExecStart=\"/Applications/purl & is <x>.app/Contents/MacOS/purlis\" \
              \"purlis-serve-as-a-login-item\""
                .to_owned()
        ),
        "{unit}"
    );
    assert!(
        section("Install").contains(&"WantedBy=default.target".to_owned()),
        "{unit}"
    );
}

#[test]
fn a_units_command_line_escapes_what_systemd_would_expand() {
    let unit = host_unit(&Job {
        program: PathBuf::from("/home/me/a\"b\\c$d%e/purlis"),
        args: vec![],
    });
    assert!(
        unit.contains("ExecStart=\"/home/me/a\\\"b\\\\c$$d%%e/purlis\"\n"),
        "{unit}"
    );
}

#[test]
fn the_apps_autostart_entry_starts_it_in_the_background_quoted_for_the_desktop() {
    let entry = app_autostart(&app_job());
    let lines: Vec<&str> = entry.lines().collect();
    assert_eq!(lines.first(), Some(&"[Desktop Entry]"));
    assert!(lines.contains(&"Type=Application"), "{entry}");
    assert!(lines.contains(&"Name=purlis"), "{entry}");
    assert!(
        lines.contains(&"Exec=\"/opt/pur lis/\\\\\"purlis\\\\\"\" \"--in-the-background\""),
        "{entry}"
    );
}

#[test]
fn the_login_items_go_where_each_manager_reads_them() {
    let home = Path::new("/home/me");
    let places = places(home, None);
    assert_eq!(
        places.host_plist,
        home.join("Library/LaunchAgents/dev.purlis.host.plist")
    );
    assert_eq!(
        places.app_plist,
        home.join("Library/LaunchAgents/dev.purlis.app.background.plist")
    );
    assert_eq!(
        places.host_unit,
        home.join(".config/systemd/user/purlis-host.service")
    );
    assert_eq!(
        places.app_autostart,
        home.join(".config/autostart/purlis.desktop")
    );
    let elsewhere = super::places(home, Some(Path::new("/xdg")));
    assert_eq!(
        elsewhere.host_unit,
        Path::new("/xdg/systemd/user/purlis-host.service")
    );
    // A relative XDG_CONFIG_HOME is ignored, as the XDG spec says.
    assert_eq!(super::places(home, Some(Path::new("rel"))), places);
}

/// The commands a manager ran, each as one line, and what each answers.
#[derive(Default)]
struct Ran {
    lines: Mutex<Vec<String>>,
    /// Lines that end in failure.
    failing: Mutex<Vec<String>>,
    /// What a line prints.
    prints: Mutex<Vec<(String, String)>>,
}

impl Ran {
    fn runs(self: &Arc<Self>) -> Runs {
        let ran = Arc::clone(self);
        Arc::new(move |program, args| {
            let line = std::iter::once(program.to_owned())
                .chain(args.iter().cloned())
                .collect::<Vec<_>>()
                .join(" ");
            ran.lines.lock().unwrap().push(line.clone());
            let fails = ran.failing.lock().unwrap().contains(&line);
            let stdout = ran
                .prints
                .lock()
                .unwrap()
                .iter()
                .find(|(of, _)| *of == line)
                .map(|(_, out)| out.clone().into_bytes())
                .unwrap_or_default();
            Ok(Output {
                status: ExitStatus::from_raw(if fails { 1 << 8 } else { 0 }),
                stdout,
                stderr: Vec::new(),
            })
        })
    }

    fn take(&self) -> Vec<String> {
        std::mem::take(&mut *self.lines.lock().unwrap())
    }

    fn fails(&self, line: &str) {
        self.failing.lock().unwrap().push(line.to_owned());
    }

    fn prints(&self, line: &str, out: &str) {
        self.prints
            .lock()
            .unwrap()
            .push((line.to_owned(), out.to_owned()));
    }
}

fn launchd(home: &Path, ran: &Arc<Ran>) -> Launchd {
    Launchd {
        places: places(home, None),
        host: host_job(),
        app: app_job(),
        log: home.join("host.log"),
        uid: 501,
        runs: ran.runs(),
    }
}

fn systemd(home: &Path, ran: &Arc<Ran>) -> SystemdUser {
    SystemdUser {
        places: places(home, None),
        host: host_job(),
        app: app_job(),
        runs: ran.runs(),
    }
}

#[test]
fn installing_on_macos_writes_both_agents_and_loads_only_the_hosts() {
    let home = tempfile::tempdir().unwrap();
    let ran = Arc::new(Ran::default());
    // Not loaded yet.
    ran.fails("launchctl print gui/501/dev.purlis.host");
    let manager = launchd(home.path(), &ran);
    manager.install().unwrap();
    let places = &manager.places;
    assert_eq!(
        std::fs::read_to_string(&places.host_plist).unwrap(),
        host_plist(&host_job(), &home.path().join("host.log"))
    );
    assert_eq!(
        std::fs::read_to_string(&places.app_plist).unwrap(),
        app_plist(&app_job())
    );
    assert_eq!(
        ran.take(),
        [
            "launchctl print gui/501/dev.purlis.host".to_owned(),
            format!(
                "launchctl bootstrap gui/501 {}",
                places.host_plist.display()
            ),
        ],
        "the app's agent is loaded at the next login, not now: loading it starts a second app"
    );
}

#[test]
fn installing_again_unchanged_leaves_a_running_host_alone() {
    let home = tempfile::tempdir().unwrap();
    let ran = Arc::new(Ran::default());
    let manager = launchd(home.path(), &ran);
    manager.install().unwrap();
    ran.take();
    manager.install().unwrap();
    assert_eq!(ran.take(), ["launchctl print gui/501/dev.purlis.host"]);
}

#[test]
fn installing_a_changed_agent_loads_it_again() {
    let home = tempfile::tempdir().unwrap();
    let ran = Arc::new(Ran::default());
    let manager = launchd(home.path(), &ran);
    std::fs::create_dir_all(manager.places.host_plist.parent().unwrap()).unwrap();
    std::fs::write(&manager.places.host_plist, "an older build's").unwrap();
    manager.install().unwrap();
    let plist = manager.places.host_plist.display().to_string();
    assert_eq!(
        ran.take(),
        [
            "launchctl print gui/501/dev.purlis.host".to_owned(),
            "launchctl bootout gui/501/dev.purlis.host".to_owned(),
            format!("launchctl bootstrap gui/501 {plist}"),
        ]
    );
}

#[test]
fn launchd_starts_holds_and_tries_the_host_again_by_its_label() {
    let home = tempfile::tempdir().unwrap();
    let ran = Arc::new(Ran::default());
    let manager = launchd(home.path(), &ran);
    manager.start().unwrap();
    assert_eq!(ran.take(), ["launchctl kickstart gui/501/dev.purlis.host"]);
    manager.hold().unwrap();
    assert_eq!(ran.take(), ["launchctl bootout gui/501/dev.purlis.host"]);
    // launchd keeps no limit of its own: the window's breaker is the only one.
    assert!(!manager.tripped());
    ran.fails("launchctl print gui/501/dev.purlis.host");
    manager.try_again().unwrap();
    let lines = ran.take();
    assert_eq!(
        lines.last().map(String::as_str),
        Some("launchctl kickstart gui/501/dev.purlis.host"),
        "{lines:?}"
    );
    assert!(
        lines
            .iter()
            .any(|line| line.starts_with("launchctl bootstrap gui/501 ")),
        "a held agent is loaded again: {lines:?}"
    );
}

#[test]
fn a_launchd_command_that_fails_says_which() {
    let home = tempfile::tempdir().unwrap();
    let ran = Arc::new(Ran::default());
    ran.fails("launchctl kickstart gui/501/dev.purlis.host");
    let why = launchd(home.path(), &ran).start().unwrap_err();
    assert!(why.contains("launchctl kickstart"), "{why}");
}

#[test]
fn removing_on_macos_unloads_the_host_and_deletes_both_agents() {
    let home = tempfile::tempdir().unwrap();
    let ran = Arc::new(Ran::default());
    let manager = launchd(home.path(), &ran);
    manager.install().unwrap();
    ran.take();
    manager.remove().unwrap();
    assert_eq!(ran.take(), ["launchctl bootout gui/501/dev.purlis.host"]);
    assert!(!manager.places.host_plist.exists());
    assert!(!manager.places.app_plist.exists());
    // Removing what is not there is no failure.
    ran.fails("launchctl bootout gui/501/dev.purlis.host");
    manager.remove().unwrap();
}

#[test]
fn installing_on_linux_writes_the_unit_and_the_autostart_entry_and_enables_the_unit() {
    let home = tempfile::tempdir().unwrap();
    let ran = Arc::new(Ran::default());
    let manager = systemd(home.path(), &ran);
    manager.install().unwrap();
    assert_eq!(
        std::fs::read_to_string(&manager.places.host_unit).unwrap(),
        host_unit(&host_job())
    );
    assert_eq!(
        std::fs::read_to_string(&manager.places.app_autostart).unwrap(),
        app_autostart(&app_job())
    );
    assert_eq!(
        ran.take(),
        [
            "systemctl --user daemon-reload",
            "systemctl --user enable purlis-host.service"
        ]
    );
    manager.install().unwrap();
    assert_eq!(
        ran.take(),
        ["systemctl --user enable purlis-host.service"],
        "an unchanged unit is not reloaded"
    );
}

#[test]
fn systemds_own_limit_is_read_from_the_units_result() {
    let home = tempfile::tempdir().unwrap();
    let ran = Arc::new(Ran::default());
    let manager = systemd(home.path(), &ran);
    let asked = "systemctl --user show --property=Result --value purlis-host.service";
    ran.prints(asked, "success\n");
    assert!(!manager.tripped());
    let ran = Arc::new(Ran::default());
    let manager = systemd(home.path(), &ran);
    ran.prints(asked, "start-limit-hit\n");
    assert!(manager.tripped());
    assert_eq!(ran.take(), [asked]);
}

#[test]
fn systemd_starts_holds_and_tries_the_host_again_by_its_unit() {
    let home = tempfile::tempdir().unwrap();
    let ran = Arc::new(Ran::default());
    let manager = systemd(home.path(), &ran);
    manager.start().unwrap();
    assert_eq!(ran.take(), ["systemctl --user start purlis-host.service"]);
    manager.hold().unwrap();
    assert_eq!(ran.take(), ["systemctl --user stop purlis-host.service"]);
    manager.try_again().unwrap();
    assert_eq!(
        ran.take(),
        [
            "systemctl --user reset-failed purlis-host.service",
            "systemctl --user start purlis-host.service"
        ]
    );
}

#[test]
fn removing_on_linux_disables_the_unit_and_deletes_both_files() {
    let home = tempfile::tempdir().unwrap();
    let ran = Arc::new(Ran::default());
    let manager = systemd(home.path(), &ran);
    manager.install().unwrap();
    ran.take();
    manager.remove().unwrap();
    assert_eq!(
        ran.take(),
        [
            "systemctl --user disable --now purlis-host.service",
            "systemctl --user daemon-reload"
        ]
    );
    assert!(!manager.places.host_unit.exists());
    assert!(!manager.places.app_autostart.exists());
}
