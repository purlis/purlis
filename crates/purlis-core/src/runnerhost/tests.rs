// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use std::os::unix::process::ExitStatusExt as _;
use std::path::{Path, PathBuf};
use std::process::{ExitStatus, Output};
use std::sync::{Arc, Mutex};

use super::*;

fn versioned() -> Service {
    Service {
        binary: PathBuf::from("/home/runner/.config/purlis/server/0.9.0-rc.1/purlis"),
        version: Some("0.9.0-rc.1".into()),
    }
}

fn by_hand() -> Service {
    Service {
        binary: PathBuf::from("/opt/pur lis/$purlis%"),
        version: None,
    }
}

/// The commands a test's manager was asked to run, each answering as the test says.
#[derive(Default)]
struct Ran {
    lines: Mutex<Vec<String>>,
    failing: Mutex<Vec<(String, String)>>,
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
            let said = ran
                .failing
                .lock()
                .unwrap()
                .iter()
                .find(|(of, _)| *of == line)
                .map(|(_, said)| said.clone());
            Ok(Output {
                status: ExitStatus::from_raw(if said.is_some() { 1 << 8 } else { 0 }),
                stdout: Vec::new(),
                stderr: said.unwrap_or_default().into_bytes(),
            })
        })
    }

    fn take(&self) -> Vec<String> {
        std::mem::take(&mut *self.lines.lock().unwrap())
    }

    fn fails(&self, line: &str, said: &str) {
        self.failing
            .lock()
            .unwrap()
            .push((line.to_owned(), said.to_owned()));
    }
}

fn here(keeper: Keeper, home: &Path, ran: &Arc<Ran>) -> Here {
    Here {
        keeper,
        user: "runner".into(),
        uid: 501,
        launch_agents: home.join("Library/LaunchAgents"),
        user_units: home.join(".config/systemd/user"),
        linger_dir: home.join("linger"),
        log: home.join("serve.log"),
        runs: ran.runs(),
    }
}

#[test]
fn each_installed_version_has_a_service_of_its_own_named_as_its_socket_is() {
    let key = crate::hostsocket::version_key("0.9.0-rc.1");
    assert_eq!(
        versioned().unit_name(),
        format!("purlis-runner-{key}.service")
    );
    assert_eq!(versioned().label(), format!("dev.purlis.runner.{key}"));
    let other = Service {
        version: Some("0.9.0".into()),
        ..versioned()
    };
    assert_ne!(other.unit_name(), versioned().unit_name());
    assert_ne!(other.label(), versioned().label());
    assert_eq!(by_hand().unit_name(), "purlis-runner.service");
    assert_eq!(by_hand().label(), "dev.purlis.runner");
    // Never the desktop's own login item's names (FD-17).
    assert_ne!(by_hand().unit_name(), loginitem::Names::RELEASE.host_unit);
    assert_ne!(by_hand().label(), loginitem::Names::RELEASE.host_label);
}

#[test]
fn the_unit_starts_with_the_user_manager_whether_or_not_anyone_has_a_desktop() {
    let unit = unit(&versioned());
    assert!(unit.contains("\nWantedBy=default.target\n"), "{unit}");
    assert!(!unit.contains("graphical-session"), "{unit}");
    assert!(unit.contains("\nRestart=on-failure\n"), "{unit}");
    assert!(unit.contains("\nStartLimitBurst=3\n"), "{unit}");
    assert!(unit.contains("\nStartLimitIntervalSec=300\n"), "{unit}");
    assert!(
        unit.contains(
            "\nExecStart=\"/home/runner/.config/purlis/server/0.9.0-rc.1/purlis\" \"serve\" \
             \"--as-a-service\"\n"
        ),
        "{unit}"
    );
    assert!(unit.contains("version 0.9.0-rc.1"), "{unit}");
}

#[test]
fn the_units_command_is_quoted_so_systemd_expands_nothing_in_it() {
    let unit = unit(&by_hand());
    assert!(
        unit.contains("ExecStart=\"/opt/pur lis/$$purlis%%\" \"serve\" \"--as-a-service\""),
        "{unit}"
    );
}

#[test]
fn the_agent_loads_in_a_background_session_too_and_restarts_after_a_failure() {
    let plist = plist(&versioned(), Path::new("/Users/runner/serve.log"));
    let agent = plist::Value::from_reader_xml(plist.as_bytes())
        .unwrap()
        .into_dictionary()
        .unwrap();
    assert_eq!(
        agent["Label"].as_string(),
        Some(versioned().label().as_str())
    );
    let arguments: Vec<&str> = agent["ProgramArguments"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(plist::Value::as_string)
        .collect();
    assert_eq!(
        arguments,
        [
            "/home/runner/.config/purlis/server/0.9.0-rc.1/purlis",
            "serve",
            "--as-a-service"
        ]
    );
    assert_eq!(agent["RunAtLoad"].as_boolean(), Some(true));
    assert_eq!(
        agent["KeepAlive"].as_dictionary().unwrap()["SuccessfulExit"].as_boolean(),
        Some(false)
    );
    let sessions: Vec<&str> = agent["LimitLoadToSessionType"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(plist::Value::as_string)
        .collect();
    assert_eq!(sessions, ["Aqua", "Background"]);
    assert_eq!(
        agent["StandardErrorPath"].as_string(),
        Some("/Users/runner/serve.log")
    );
}

#[test]
fn what_keeps_the_host_running_is_the_service_manager_and_a_container_runs_it_in_front() {
    assert_eq!(keeper(Os::Macos, false, false), Keeper::Launchd);
    assert_eq!(keeper(Os::Linux, true, false), Keeper::SystemdUser);
    // A container that boots systemd has a user manager like any machine.
    assert_eq!(keeper(Os::Linux, true, true), Keeper::SystemdUser);
    assert_eq!(
        keeper(Os::Linux, false, true),
        Keeper::Foreground(Why::Container)
    );
    assert_eq!(
        keeper(Os::Linux, false, false),
        Keeper::Foreground(Why::NoServiceManager)
    );
    assert_eq!(
        keeper(Os::Other, true, true),
        Keeper::Foreground(Why::Unsupported)
    );
}

#[test]
fn a_container_is_known_by_the_marks_its_engine_leaves() {
    let root = tempfile::tempdir().unwrap();
    assert!(!in_a_container(root.path(), None));
    assert!(!in_a_container(root.path(), Some("".as_ref())));
    assert!(in_a_container(root.path(), Some("podman".as_ref())));
    std::fs::write(root.path().join(".dockerenv"), "").unwrap();
    assert!(in_a_container(root.path(), None));
    let podman = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(podman.path().join("run")).unwrap();
    std::fs::write(podman.path().join("run/.containerenv"), "").unwrap();
    assert!(in_a_container(podman.path(), None));
}

#[test]
fn installing_with_systemd_turns_linger_on_writes_the_unit_and_starts_it_now() {
    let home = tempfile::tempdir().unwrap();
    let ran = Arc::new(Ran::default());
    let here = here(Keeper::SystemdUser, home.path(), &ran);
    let installed = install(&here, &versioned()).expect("installed");
    assert_eq!(
        installed,
        Installed {
            keeper: Keeper::SystemdUser,
            linger: Linger::TurnedOn
        }
    );
    let name = versioned().unit_name();
    assert_eq!(
        ran.take(),
        [
            "loginctl enable-linger".to_owned(),
            "systemctl --user daemon-reload".to_owned(),
            format!("systemctl --user enable --now {name}"),
        ]
    );
    let written = std::fs::read_to_string(home.path().join(".config/systemd/user").join(&name))
        .expect("the unit is written");
    assert_eq!(written, unit(&versioned()));
}

#[test]
fn installing_again_leaves_linger_and_an_unchanged_unit_alone_and_its_host_running() {
    let home = tempfile::tempdir().unwrap();
    let ran = Arc::new(Ran::default());
    let here = here(Keeper::SystemdUser, home.path(), &ran);
    install(&here, &versioned()).unwrap();
    std::fs::create_dir_all(&here.linger_dir).unwrap();
    std::fs::write(here.linger_dir.join("runner"), "").unwrap();
    ran.take();
    let again = install(&here, &versioned()).unwrap();
    assert_eq!(again.linger, Linger::On);
    // `enable --now` starts a unit that is not running and leaves one that is.
    assert_eq!(
        ran.take(),
        [format!(
            "systemctl --user enable --now {}",
            versioned().unit_name()
        )]
    );
}

#[test]
fn a_new_versions_service_never_touches_the_old_ones() {
    let home = tempfile::tempdir().unwrap();
    let ran = Arc::new(Ran::default());
    let here = here(Keeper::SystemdUser, home.path(), &ran);
    install(&here, &versioned()).unwrap();
    let old = std::fs::read_to_string(here.unit_path(&versioned())).unwrap();
    ran.take();
    let new = Service {
        binary: PathBuf::from("/home/runner/.config/purlis/server/0.9.0/purlis"),
        version: Some("0.9.0".into()),
    };
    install(&here, &new).unwrap();
    assert_eq!(
        std::fs::read_to_string(here.unit_path(&versioned())).unwrap(),
        old
    );
    for line in ran.take() {
        assert!(!line.contains(&versioned().unit_name()), "{line}");
    }
}

#[test]
fn where_only_an_administrator_may_turn_linger_on_the_host_is_still_installed_and_it_says_so() {
    let home = tempfile::tempdir().unwrap();
    let ran = Arc::new(Ran::default());
    ran.fails(
        "loginctl enable-linger",
        "Could not enable linger: Access denied",
    );
    let here = here(Keeper::SystemdUser, home.path(), &ran);
    let installed = install(&here, &versioned()).expect("installed all the same");
    assert!(
        matches!(&installed.linger, Linger::NeedsAnAdministrator { said } if said.contains("Access denied")),
        "{installed:?}"
    );
    assert!(ran.take().contains(&format!(
        "systemctl --user enable --now {}",
        versioned().unit_name()
    )));
    let said = said(&installed, &versioned(), "runner").join("\n");
    assert!(
        said.contains("Your chats will stop when you sign out of this machine"),
        "{said}"
    );
    assert!(
        said.contains("Ask an administrator to run `sudo loginctl enable-linger runner`."),
        "{said}"
    );
    assert!(said.contains("Access denied"), "{said}");
}

#[test]
fn a_manager_that_refuses_the_unit_is_said_with_what_it_said() {
    let home = tempfile::tempdir().unwrap();
    let ran = Arc::new(Ran::default());
    let name = versioned().unit_name();
    ran.fails(
        &format!("systemctl --user enable --now {name}"),
        "Failed to connect to bus",
    );
    let here = here(Keeper::SystemdUser, home.path(), &ran);
    let refused = install(&here, &versioned()).unwrap_err();
    assert!(
        matches!(&refused, Refused::Manager(why) if why.contains("Failed to connect to bus")),
        "{refused:?}"
    );
}

#[test]
fn in_a_container_nothing_is_installed_and_it_says_to_run_the_host_in_front() {
    let home = tempfile::tempdir().unwrap();
    let ran = Arc::new(Ran::default());
    let here = here(Keeper::Foreground(Why::Container), home.path(), &ran);
    assert_eq!(
        install(&here, &versioned()),
        Err(Refused::Foreground(Why::Container))
    );
    assert!(ran.take().is_empty());
    assert!(!home.path().join(".config").exists());
    let said = refused(Why::Container);
    assert!(said.contains("`purlis serve --foreground`"), "{said}");
    assert!(said.contains("restart policy"), "{said}");
}

#[test]
fn installing_with_launchd_loads_the_agent_in_the_user_domain_when_nobody_is_at_the_screen() {
    let home = tempfile::tempdir().unwrap();
    let ran = Arc::new(Ran::default());
    ran.fails("launchctl print gui/501", "Could not find domain");
    let label = versioned().label();
    ran.fails(
        &format!("launchctl print user/501/{label}"),
        "Could not find service",
    );
    let here = here(Keeper::Launchd, home.path(), &ran);
    let installed = install(&here, &versioned()).unwrap();
    assert_eq!(installed.linger, Linger::NotHere);
    let plist_path = here.plist_path(&versioned());
    assert_eq!(
        ran.take(),
        [
            "launchctl print gui/501".to_owned(),
            format!("launchctl print user/501/{label}"),
            format!("launchctl bootstrap user/501 {}", plist_path.display()),
        ]
    );
    let said = said(&installed, &versioned(), "runner").join("\n");
    assert!(said.contains("when this account signs in"), "{said}");
}

#[test]
fn removing_stops_and_disables_the_service_and_leaves_linger_be() {
    let home = tempfile::tempdir().unwrap();
    let ran = Arc::new(Ran::default());
    let here = here(Keeper::SystemdUser, home.path(), &ran);
    install(&here, &versioned()).unwrap();
    ran.take();
    remove(&here, &versioned()).unwrap();
    assert!(!here.unit_path(&versioned()).exists());
    let name = versioned().unit_name();
    assert_eq!(
        ran.take(),
        [
            format!("systemctl --user disable --now {name}"),
            "systemctl --user daemon-reload".to_owned(),
        ]
    );
    // Nothing installed: nothing asked.
    remove(&here, &versioned()).unwrap();
    assert!(ran.take().is_empty());
}

fn a_look(keeper: Keeper) -> Look {
    Look {
        keeper,
        service: versioned(),
        installed: true,
        enabled: true,
        lingers: matches!(keeper, Keeper::SystemdUser).then_some(true),
        user: "runner".into(),
        socket: PathBuf::from("/run/user/1000/purlis/purlisd-v1a2b3c4d.sock"),
        reachable: true,
    }
}

fn statuses(found: &[Finding]) -> Vec<(&'static str, Status)> {
    found.iter().map(|row| (row.name, row.status)).collect()
}

#[test]
fn doctor_finds_the_unit_installed_linger_on_and_the_host_answering() {
    let found = rows(&a_look(Keeper::SystemdUser));
    assert_eq!(
        statuses(&found),
        [
            (SERVICE_ROW, Status::Ok),
            (LINGER_ROW, Status::Ok),
            (REACHABLE_ROW, Status::Ok)
        ]
    );
    assert!(found[0].detail.contains(&versioned().unit_name()));
    assert!(found[0].detail.contains("after a restart"));
}

#[test]
fn doctor_says_each_fault_with_what_to_do() {
    let missing = rows(&Look {
        installed: false,
        enabled: false,
        lingers: Some(false),
        reachable: false,
        ..a_look(Keeper::SystemdUser)
    });
    assert_eq!(
        statuses(&missing),
        [
            (SERVICE_ROW, Status::Warn),
            (LINGER_ROW, Status::Warn),
            (REACHABLE_ROW, Status::Fail)
        ]
    );
    let bin = "/home/runner/.config/purlis/server/0.9.0-rc.1/purlis";
    assert!(
        missing[0]
            .hint
            .contains(&format!("`{bin} serve --install`")),
        "{missing:?}"
    );
    assert!(missing[1].detail.contains("stop when you sign out"));
    assert!(
        missing[1]
            .hint
            .contains("`sudo loginctl enable-linger runner`")
    );
    let off = rows(&Look {
        enabled: false,
        ..a_look(Keeper::SystemdUser)
    });
    assert_eq!(off[0].status, Status::Warn);
    assert!(off[0].detail.contains("switched off"), "{off:?}");
    let stopped = rows(&Look {
        reachable: false,
        ..a_look(Keeper::SystemdUser)
    });
    assert!(
        stopped[2].hint.contains(&format!(
            "systemctl --user status {}",
            versioned().unit_name()
        )),
        "{stopped:?}"
    );
}

#[test]
fn doctor_on_a_mac_or_in_a_container_has_no_linger_row() {
    assert_eq!(
        statuses(&rows(&a_look(Keeper::Launchd))),
        [(SERVICE_ROW, Status::Ok), (REACHABLE_ROW, Status::Ok)]
    );
    let container = rows(&Look {
        installed: false,
        enabled: false,
        ..a_look(Keeper::Foreground(Why::Container))
    });
    assert_eq!(
        statuses(&container),
        [(SERVICE_ROW, Status::Ok), (REACHABLE_ROW, Status::Ok)]
    );
    assert!(container[0].hint.contains("--foreground"));
}

#[test]
fn a_binary_path_with_a_space_is_quoted_in_what_doctor_says_to_run() {
    let found = rows(&Look {
        service: by_hand(),
        installed: false,
        ..a_look(Keeper::SystemdUser)
    });
    assert!(
        found[0]
            .hint
            .contains("`'/opt/pur lis/$purlis%' serve --install`"),
        "{found:?}"
    );
}

#[test]
fn what_install_says_is_plain() {
    let installed = Installed {
        keeper: Keeper::SystemdUser,
        linger: Linger::TurnedOn,
    };
    let said = said(&installed, &versioned(), "runner");
    assert_eq!(said.len(), 2, "{said:?}");
    assert!(said[0].contains("starts it again if it stops or the machine restarts"));
    assert_eq!(said[1], "✓ Your chats keep running after you sign out.");
}

#[test]
fn a_pruned_versions_service_is_removed_by_its_name_alone_without_its_binary() {
    let home = tempfile::tempdir().unwrap();
    let ran = Arc::new(Ran::default());
    let here = here(Keeper::SystemdUser, home.path(), &ran);
    install(&here, &versioned()).unwrap();
    ran.take();
    remove_version(&here, "0.9.0-rc.1").expect("removed");
    assert!(!here.unit_path(&versioned()).exists());
    let name = versioned().unit_name();
    assert_eq!(
        ran.take(),
        [
            format!("systemctl --user disable --now {name}"),
            "systemctl --user daemon-reload".to_owned(),
        ],
        "systemd is told by the unit's name; the version's own binary is never run"
    );
}

#[test]
fn a_pruned_versions_agent_is_removed_by_its_label_alone() {
    let home = tempfile::tempdir().unwrap();
    let ran = Arc::new(Ran::default());
    let here = here(Keeper::Launchd, home.path(), &ran);
    install(&here, &versioned()).unwrap();
    ran.take();
    remove_version(&here, "0.9.0-rc.1").expect("removed");
    assert!(!here.plist_path(&versioned()).exists());
    let label = versioned().label();
    let lines = ran.take();
    assert!(
        lines.contains(&format!("launchctl bootout gui/501/{label}")),
        "{lines:?}"
    );
    assert!(
        lines.iter().all(|line| line.starts_with("launchctl ")),
        "{lines:?}"
    );
}

#[test]
fn a_service_that_cannot_be_removed_says_so_so_its_version_is_kept() {
    let home = tempfile::tempdir().unwrap();
    let ran = Arc::new(Ran::default());
    let here = here(Keeper::SystemdUser, home.path(), &ran);
    install(&here, &versioned()).unwrap();
    ran.fails("systemctl --user daemon-reload", "Failed to connect to bus");
    assert!(remove_version(&here, "0.9.0-rc.1").is_err());
}
