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
        &Names::RELEASE,
        &host_job(),
        Path::new("/Users/me/log/host.log"),
    ));
    assert_eq!(
        plist.get("Label").and_then(plist::Value::as_string),
        Some("dev.purlis.host")
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
    let plist = dict(&app_plist(&Names::RELEASE, &app_job()));
    assert_eq!(
        plist.get("Label").and_then(plist::Value::as_string),
        Some("dev.purlis.app.background")
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
    // Started with the person's graphical session, and after it, so the host and the browsers
    // it runs have the session's display; stopped with it.
    for line in [
        "After=graphical-session.target",
        "PartOf=graphical-session.target",
    ] {
        assert!(unit_section.contains(&line.to_owned()), "{line} in\n{unit}");
    }
    assert!(
        section("Install").contains(&"WantedBy=graphical-session.target".to_owned()),
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
    let places = places(home, None, Names::RELEASE);
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
    let elsewhere = super::places(home, Some(Path::new("/xdg")), Names::RELEASE);
    assert_eq!(
        elsewhere.host_unit,
        Path::new("/xdg/systemd/user/purlis-host.service")
    );
    // A relative XDG_CONFIG_HOME is ignored, as the XDG spec says.
    assert_eq!(super::places(home, Some(Path::new("rel")), Names::RELEASE), places);
}

/// The commands a manager ran, each as one line, and what each answers.
#[derive(Default)]
struct Ran {
    lines: Mutex<Vec<String>>,
    /// Lines that end in failure.
    failing: Mutex<Vec<String>>,
    /// What a line prints.
    prints: Mutex<Vec<(String, String)>>,
    /// Lines that fail so many more times.
    failing_first: Mutex<Vec<(String, usize)>>,
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
            let fails_now = ran
                .failing_first
                .lock()
                .unwrap()
                .iter_mut()
                .find(|(of, left)| *of == line && *left > 0)
                .map(|(_, left)| *left -= 1)
                .is_some();
            let fails = fails_now || ran.failing.lock().unwrap().contains(&line);
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

    /// `line` fails its first `times` runs, then succeeds.
    fn fails_first(&self, line: &str, times: usize) {
        self.failing_first
            .lock()
            .unwrap()
            .push((line.to_owned(), times));
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
        places: places(home, None, Names::RELEASE),
        host: host_job(),
        app: app_job(),
        log: home.join("host.log"),
        uid: 501,
        runs: ran.runs(),
    }
}

fn systemd(home: &Path, ran: &Arc<Ran>) -> SystemdUser {
    SystemdUser {
        places: places(home, None, Names::RELEASE),
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
        host_plist(&Names::RELEASE, &host_job(), &home.path().join("host.log"))
    );
    assert_eq!(
        std::fs::read_to_string(&places.app_plist).unwrap(),
        app_plist(&Names::RELEASE, &app_job())
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
fn a_changed_agent_never_ends_the_host_launchd_runs_and_is_read_at_its_next_start() {
    let home = tempfile::tempdir().unwrap();
    let ran = Arc::new(Ran::default());
    let manager = launchd(home.path(), &ran);
    std::fs::create_dir_all(manager.places.host_plist.parent().unwrap()).unwrap();
    std::fs::write(&manager.places.host_plist, "an older build's").unwrap();
    manager.install().unwrap();
    assert_eq!(
        ran.take(),
        ["launchctl print gui/501/dev.purlis.host"],
        "a host that runs keeps running, with every chat in it"
    );
    assert_eq!(
        std::fs::read_to_string(&manager.places.host_plist).unwrap(),
        host_plist(&Names::RELEASE, &host_job(), &home.path().join("host.log")),
        "written for launchd to read when it next loads the agent"
    );
}

#[test]
fn a_changed_agent_that_is_not_loaded_is_loaded() {
    let home = tempfile::tempdir().unwrap();
    let ran = Arc::new(Ran::default());
    ran.fails("launchctl print gui/501/dev.purlis.host");
    let manager = launchd(home.path(), &ran);
    std::fs::create_dir_all(manager.places.host_plist.parent().unwrap()).unwrap();
    std::fs::write(&manager.places.host_plist, "an older build's").unwrap();
    manager.install().unwrap();
    let plist = manager.places.host_plist.display().to_string();
    assert_eq!(
        ran.take(),
        [
            "launchctl print gui/501/dev.purlis.host".to_owned(),
            format!("launchctl bootstrap gui/501 {plist}"),
        ]
    );
}

#[test]
fn a_bootstrap_refused_because_the_agent_got_loaded_meanwhile_is_no_failure() {
    let home = tempfile::tempdir().unwrap();
    let ran = Arc::new(Ran::default());
    // Not loaded when asked, then loaded by the time bootstrap runs: launchd's error 5.
    ran.fails_first("launchctl print gui/501/dev.purlis.host", 1);
    let manager = launchd(home.path(), &ran);
    let plist = manager.places.host_plist.display().to_string();
    ran.fails(&format!("launchctl bootstrap gui/501 {plist}"));
    manager.install().unwrap();
    assert_eq!(
        ran.take(),
        [
            "launchctl print gui/501/dev.purlis.host".to_owned(),
            format!("launchctl bootstrap gui/501 {plist}"),
            "launchctl print gui/501/dev.purlis.host".to_owned(),
        ]
    );
}

#[test]
fn a_bootstrap_that_fails_with_the_agent_still_not_loaded_says_so() {
    let home = tempfile::tempdir().unwrap();
    let ran = Arc::new(Ran::default());
    ran.fails("launchctl print gui/501/dev.purlis.host");
    let manager = launchd(home.path(), &ran);
    let plist = manager.places.host_plist.display().to_string();
    ran.fails(&format!("launchctl bootstrap gui/501 {plist}"));
    let why = manager.install().unwrap_err();
    assert!(why.contains("launchctl bootstrap"), "{why}");
}

#[test]
fn two_installs_at_once_run_one_after_the_other() {
    let home = tempfile::tempdir().unwrap();
    let inside = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let most = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let runs: Runs = {
        let (inside, most) = (Arc::clone(&inside), Arc::clone(&most));
        Arc::new(move |_program, args| {
            use std::sync::atomic::Ordering::SeqCst;
            let now = inside.fetch_add(1, SeqCst) + 1;
            most.fetch_max(now, SeqCst);
            std::thread::sleep(std::time::Duration::from_millis(50));
            inside.fetch_sub(1, SeqCst);
            // Not loaded, so each goes on to bootstrap.
            let loaded = args.first().map(String::as_str) != Some("print");
            Ok(Output {
                status: ExitStatus::from_raw(if loaded { 0 } else { 1 << 8 }),
                stdout: Vec::new(),
                stderr: Vec::new(),
            })
        })
    };
    let manager = Arc::new(Launchd {
        runs,
        ..launchd(home.path(), &Arc::new(Ran::default()))
    });
    let both: Vec<_> = (0..2)
        .map(|_| {
            let manager = Arc::clone(&manager);
            std::thread::spawn(move || manager.install())
        })
        .collect();
    for one in both {
        one.join().unwrap().unwrap();
    }
    assert_eq!(most.load(std::sync::atomic::Ordering::SeqCst), 1);
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
    // Removing what is not there is no failure, and asks launchd nothing.
    manager.remove().unwrap();
    assert_eq!(ran.take(), Vec::<String>::new());
}

#[test]
fn removing_login_items_never_installed_asks_no_manager_anything() {
    let home = tempfile::tempdir().unwrap();
    let ran = Arc::new(Ran::default());
    launchd(home.path(), &ran).remove().unwrap();
    systemd(home.path(), &ran).remove().unwrap();
    assert_eq!(
        ran.take(),
        Vec::<String>::new(),
        "a quit with nothing keeping the host changes nothing for anyone"
    );
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

#[test]
fn each_platform_has_the_manager_its_login_items_need_and_none_where_there_is_none() {
    let home = Path::new("/home/me");
    let on = |os, systemd_user| {
        manager_on(
            os,
            systemd_user,
            places(home, None, Names::RELEASE),
            host_job(),
            app_job(),
            home.join("host.log"),
            runs_for_real(),
        )
        .map(|(kind, _)| kind)
    };
    assert_eq!(on(Os::Macos, false), Some(Supervisor::Launchd));
    assert_eq!(on(Os::Linux, true), Some(Supervisor::SystemdUser));
    assert_eq!(on(Os::Linux, false), None);
    assert_eq!(on(Os::Other, true), None);
}

#[test]
fn a_development_build_has_login_items_of_its_own_beside_a_release_builds() {
    let home = Path::new("/home/me");
    let release = places(home, None, Names::RELEASE);
    let development = places(home, None, Names::DEVELOPMENT);
    assert_ne!(release.host_plist, development.host_plist);
    assert_ne!(release.app_plist, development.app_plist);
    assert_ne!(release.host_unit, development.host_unit);
    assert_ne!(release.app_autostart, development.app_autostart);
    assert_eq!(
        development.host_plist,
        home.join("Library/LaunchAgents/dev.purlis.host.development.plist")
    );
    let plist = dict(&host_plist(
        &Names::DEVELOPMENT,
        &host_job(),
        Path::new("/log"),
    ));
    assert_eq!(
        plist.get("Label").and_then(plist::Value::as_string),
        Some("dev.purlis.host.development")
    );
    assert_eq!(
        Names::here(),
        if cfg!(debug_assertions) {
            Names::DEVELOPMENT
        } else {
            Names::RELEASE
        }
    );
    let ran = Arc::new(Ran::default());
    let manager = SystemdUser {
        places: development,
        ..systemd(home, &ran)
    };
    manager.start().unwrap();
    assert_eq!(
        ran.take(),
        ["systemctl --user start purlis-host-development.service"]
    );
}
