// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use std::path::PathBuf;

use super::*;
use crate::runnerinstall::BINARY;

fn versions(names: &[&str]) -> Vec<String> {
    names.iter().map(|name| (*name).to_owned()).collect()
}

#[test]
fn only_versions_older_than_the_one_before_the_running_one_may_go() {
    assert_eq!(
        may_go("3.0.0", &versions(&["1.0.0", "2.0.0", "3.0.0"])),
        ["1.0.0"]
    );
    // Ordered as versions, not as text: 1.10.0 is after 1.9.0.
    assert_eq!(
        may_go("1.10.0", &versions(&["1.8.0", "1.9.0", "1.10.0", "1.2.0"])),
        ["1.2.0", "1.8.0"]
    );
    // A dev build is before its release.
    assert_eq!(
        may_go(
            "1.4.0",
            &versions(&["1.4.0-dev.7", "1.4.0-dev.8", "1.4.0", "1.3.0"])
        ),
        ["1.3.0", "1.4.0-dev.7"]
    );
}

#[test]
fn the_running_version_the_one_before_it_a_newer_one_and_one_not_read_as_a_version_stay() {
    // Nothing before the running one but the one before it.
    assert!(may_go("2.0.0", &versions(&["1.0.0", "2.0.0"])).is_empty());
    // A version newer than the running one is not this desktop's to remove.
    assert!(may_go("1.0.0", &versions(&["1.0.0", "2.0.0", "3.0.0"])).is_empty());
    // A folder not read as a version is never chosen, and does not count as the one before.
    assert_eq!(
        may_go("3.0.0", &versions(&["1.0.0", "2.0", "2.0.0", "3.0.0"])),
        ["1.0.0"]
    );
    // A running version not read as one removes nothing.
    assert!(may_go("3.0", &versions(&["1.0.0", "2.0.0"])).is_empty());
}

/// A runner's config home with `installed` versions, each with its binary and its host's folder.
struct Runner {
    _home: tempfile::TempDir,
    config_root: PathBuf,
    socket: PathBuf,
}

impl Runner {
    fn with(installed: &[&str]) -> Runner {
        // Short: a socket's path must fit in 104 bytes on macOS.
        let home = tempfile::Builder::new().prefix("pr").tempdir().unwrap();
        let root = std::fs::canonicalize(home.path()).unwrap();
        let config_root = root.join("c");
        let run = root.join("r");
        std::fs::create_dir_all(&run).unwrap();
        let runner = Runner {
            _home: home,
            config_root,
            socket: run.join("purlisd.sock"),
        };
        for version in installed {
            std::fs::create_dir_all(runner.binary(version).parent().unwrap()).unwrap();
            std::fs::write(runner.binary(version), b"x").unwrap();
            std::fs::create_dir_all(runner.host_folder(version)).unwrap();
            std::fs::write(runner.host_folder(version).join("credentials"), b"x").unwrap();
        }
        runner
    }

    fn binary(&self, version: &str) -> PathBuf {
        crate::machine::dir(&self.config_root)
            .join(SERVER_DIR)
            .join(version)
            .join(BINARY)
    }

    fn host_folder(&self, version: &str) -> PathBuf {
        host_folder_of(&crate::machine::charterd(&self.config_root), version)
    }

    fn socket_of(&self, version: &str) -> PathBuf {
        crate::hostsocket::of_version(&self.socket, version)
    }

    fn prune(&self, running: &str) -> Pruned {
        prune_in(
            &self.config_root,
            running,
            std::slice::from_ref(&self.socket),
        )
    }
}

fn gone(runner: &Runner, version: &str) -> bool {
    !runner.binary(version).parent().unwrap().exists() && !runner.host_folder(version).exists()
}

#[test]
fn a_drained_versions_folders_are_removed_and_the_running_one_and_the_one_before_stay() {
    let runner = Runner::with(&["1.0.0", "2.0.0", "3.0.0"]);
    let pruned = runner.prune("3.0.0");
    assert_eq!(pruned.removed, ["1.0.0"]);
    assert!(pruned.kept.is_empty(), "{:?}", pruned.kept);
    assert!(gone(&runner, "1.0.0"));
    for version in ["2.0.0", "3.0.0"] {
        assert!(runner.binary(version).exists(), "{version}'s binary stays");
        assert!(
            runner.host_folder(version).exists(),
            "{version}'s host stays"
        );
    }
    // Pruned again, nothing more goes.
    assert!(runner.prune("3.0.0").removed.is_empty());
}

#[test]
fn a_version_whose_host_holds_its_lock_is_never_removed() {
    let runner = Runner::with(&["1.0.0", "2.0.0", "3.0.0"]);
    let lock = std::fs::File::create(runner.socket_of("1.0.0").with_extension("lock")).unwrap();
    lock.try_lock().unwrap();
    let pruned = runner.prune("3.0.0");
    assert!(pruned.removed.is_empty());
    assert_eq!(pruned.kept, [("1.0.0".to_owned(), Live::Serving)]);
    assert!(runner.binary("1.0.0").exists() && runner.host_folder("1.0.0").exists());
    drop(lock);
    assert_eq!(runner.prune("3.0.0").removed, ["1.0.0"]);
}

#[test]
fn a_version_whose_host_answers_on_its_socket_is_never_removed() {
    let runner = Runner::with(&["1.0.0", "2.0.0", "3.0.0"]);
    // A host whose lock file the system cleaned away still answers.
    let listening = std::os::unix::net::UnixListener::bind(runner.socket_of("1.0.0")).unwrap();
    let pruned = runner.prune("3.0.0");
    assert!(pruned.removed.is_empty());
    assert_eq!(pruned.kept, [("1.0.0".to_owned(), Live::Serving)]);
    assert!(runner.binary("1.0.0").exists() && runner.host_folder("1.0.0").exists());
    drop(listening);
    // A socket file a host that ended left behind answers nothing, and goes with the version.
    assert_eq!(runner.prune("3.0.0").removed, ["1.0.0"]);
    assert!(!runner.socket_of("1.0.0").exists());
}

#[test]
fn a_version_whose_host_left_chats_in_its_record_is_never_removed() {
    let runner = Runner::with(&["1.0.0", "2.0.0", "3.0.0"]);
    let record = runner.host_folder("1.0.0").join(HOST_RECORD);
    std::fs::write(&record, started("p1")).unwrap();
    let pruned = runner.prune("3.0.0");
    assert!(pruned.removed.is_empty());
    assert_eq!(pruned.kept, [("1.0.0".to_owned(), Live::Chats)]);
    assert!(runner.binary("1.0.0").exists() && record.exists());
    // A host that ended as it should empties its record.
    std::fs::write(&record, b"").unwrap();
    assert_eq!(runner.prune("3.0.0").removed, ["1.0.0"]);
}

/// A `started` line as `purlis_host`'s host record writes it, of the program `program` whose
/// process is `process`, noted as started at `process_started` in the boot `boot`.
fn started_as(
    program: &str,
    process: u32,
    process_started: Option<&str>,
    boot: Option<&str>,
) -> String {
    let mut line = serde_json::json!({"started": {
        "program": program,
        "project": "01J9PRJCT00000000000000000",
        "project_path": "/w/p",
        "number": 1,
        "process": process,
    }});
    if let Some(process_started) = process_started {
        line["started"]["process_started"] = process_started.into();
    }
    if let Some(boot) = boot {
        line["started"]["boot"] = boot.into();
    }
    format!("{line}\n")
}

/// When this test's own process started, as the kernel says it.
fn this_process_started() -> String {
    purlis_same_user::Parents::started(std::process::id())
        .expect("the kernel says when this test's own process started")
}

/// A `started` line of a program that still runs: this test's own process, as noted in this
/// boot.
fn started(program: &str) -> String {
    started_as(
        program,
        std::process::id(),
        Some(&this_process_started()),
        purlis_host::this_boot().as_deref(),
    )
}

fn ended(program: &str) -> String {
    format!("{{\"ended\":{{\"program\":\"{program}\"}}}}\n")
}

#[test]
fn a_host_stopped_by_a_signal_whose_chats_had_all_ended_has_drained() {
    let runner = Runner::with(&["1.0.0", "2.0.0", "3.0.0"]);
    let record = runner.host_folder("1.0.0").join(HOST_RECORD);
    // Killed outright, a host empties nothing: its record still names each chat it ended.
    std::fs::write(
        &record,
        format!(
            "{}{}{}{}",
            started("p1"),
            started("p2"),
            ended("p2"),
            ended("p1")
        ),
    )
    .unwrap();
    assert_eq!(runner.prune("3.0.0").removed, ["1.0.0"]);
}

#[test]
fn a_chat_started_and_never_ended_keeps_its_version_and_a_torn_line_is_passed_over() {
    let runner = Runner::with(&["1.0.0", "2.0.0", "3.0.0"]);
    let record = runner.host_folder("1.0.0").join(HOST_RECORD);
    std::fs::write(
        &record,
        format!(
            "{}{}{}{{\"ended\":{{\"prog",
            started("p1"),
            started("p2"),
            ended("p1")
        ),
    )
    .unwrap();
    assert_eq!(
        runner.prune("3.0.0").kept,
        [("1.0.0".to_owned(), Live::Chats)]
    );
    assert!(runner.binary("1.0.0").exists());
    // A record larger than any host writes is not read, and keeps its version.
    let runner = Runner::with(&["1.0.0", "2.0.0", "3.0.0"]);
    let record = runner.host_folder("1.0.0").join(HOST_RECORD);
    std::fs::write(&record, vec![b'\n'; (READ_AT_MOST + 1) as usize]).unwrap();
    assert_eq!(
        runner.prune("3.0.0").kept,
        [("1.0.0".to_owned(), Live::Chats)]
    );
}

#[test]
fn a_chat_noted_in_another_boot_keeps_nothing_after_the_reboot() {
    let runner = Runner::with(&["1.0.0", "2.0.0", "3.0.0"]);
    let record = runner.host_folder("1.0.0").join(HOST_RECORD);
    // Started and never ended: its host went down with the machine. Its process number and
    // start are this process's own, as a later boot may well give them again.
    let other_boot = format!("{}-before", purlis_host::this_boot().unwrap_or_default());
    std::fs::write(
        &record,
        started_as(
            "p1",
            std::process::id(),
            Some(&this_process_started()),
            Some(&other_boot),
        ),
    )
    .unwrap();
    assert_eq!(runner.prune("3.0.0").removed, ["1.0.0"]);
}

#[test]
fn a_chat_whose_process_number_went_to_another_process_keeps_nothing() {
    let runner = Runner::with(&["1.0.0", "2.0.0", "3.0.0"]);
    let record = runner.host_folder("1.0.0").join(HOST_RECORD);
    // Its host was killed outright (SIGKILL, #6700), and its chat ended with it: the number it
    // ran as is now this process's, which started at another time.
    std::fs::write(
        &record,
        started_as(
            "p1",
            std::process::id(),
            Some("not when this process started"),
            purlis_host::this_boot().as_deref(),
        ),
    )
    .unwrap();
    assert_eq!(runner.prune("3.0.0").removed, ["1.0.0"]);
}

#[test]
fn a_chat_noted_with_nothing_to_tell_its_process_by_keeps_nothing() {
    let runner = Runner::with(&["1.0.0", "2.0.0", "3.0.0"]);
    let record = runner.host_folder("1.0.0").join(HOST_RECORD);
    std::fs::write(
        &record,
        format!(
            "{}{}",
            started_as("p1", 4242, None, purlis_host::this_boot().as_deref()),
            // A process the kernel says nothing of: no number left that is this user's.
            started_as(
                "p2",
                u32::try_from(i32::MAX).unwrap(),
                Some(&this_process_started()),
                purlis_host::this_boot().as_deref()
            ),
        ),
    )
    .unwrap();
    assert_eq!(runner.prune("3.0.0").removed, ["1.0.0"]);
}

#[test]
fn the_versions_lock_is_held_while_each_of_its_folders_and_files_is_removed() {
    let runner = Runner::with(&["1.0.0", "2.0.0", "3.0.0"]);
    let lock = runner.socket_of("1.0.0").with_extension("lock");
    std::fs::File::create(&lock).unwrap();
    // A second name for the lock file: `flock` holds the file, not its name, so this still
    // tells whether the lock is held once the prune has unlinked the first.
    let peek = runner.socket_of("1.0.0").with_extension("peek");
    std::fs::hard_link(&lock, &peek).unwrap();
    let mut held = Vec::new();
    let pruned = prune_with(
        &runner.config_root,
        "3.0.0",
        std::slice::from_ref(&runner.socket),
        &mut |_| Ok(()),
        &mut |path| {
            let looked = std::fs::File::open(&peek).unwrap();
            let free = looked.try_lock_shared().is_ok();
            held.push((path.to_owned(), !free));
            drop(looked);
            remove(path)
        },
    );
    assert_eq!(pruned.removed, ["1.0.0"]);
    assert_eq!(
        held.len(),
        5,
        "binary, link socket, socket, lock, host: {held:?}"
    );
    for (path, held) in &held {
        assert!(held, "the lock was free as {} was removed", path.display());
    }
    assert!(gone(&runner, "1.0.0") && !lock.exists());
}

#[test]
fn a_removal_cut_short_at_any_step_is_finished_by_the_next_prune() {
    for failing in 0..5 {
        let runner = Runner::with(&["1.0.0", "2.0.0", "3.0.0"]);
        let mut step = 0;
        let pruned = prune_with(
            &runner.config_root,
            "3.0.0",
            std::slice::from_ref(&runner.socket),
            &mut |_| Ok(()),
            &mut |path| {
                step += 1;
                if step - 1 == failing {
                    return Err(std::io::Error::other("the disk said no"));
                }
                remove(path)
            },
        );
        assert!(pruned.removed.is_empty(), "step {failing}: {pruned:?}");
        assert_eq!(
            pruned.failed,
            [("1.0.0".to_owned(), "the disk said no".to_owned())],
            "step {failing}"
        );
        assert!(
            runner.host_folder("1.0.0").exists(),
            "step {failing}: the host's folder, its credentials in it, is left for the next"
        );
        assert!(pruned.said().contains("1.0.0 could not be removed"));
        let again = runner.prune("3.0.0");
        assert_eq!(again.removed, ["1.0.0"], "step {failing}: {again:?}");
        assert!(gone(&runner, "1.0.0"), "step {failing}");
        for left in ["lock", "link.sock", "sock"] {
            assert!(
                !runner.socket_of("1.0.0").with_extension(left).exists(),
                "step {failing}: {left}"
            );
        }
        assert!(runner.binary("2.0.0").exists() && runner.host_folder("2.0.0").exists());
    }
}

#[test]
fn a_host_folder_left_with_no_binary_is_removed_but_a_newer_one_stays() {
    let runner = Runner::with(&["1.0.0", "2.0.0", "3.0.0", "4.0.0"]);
    std::fs::remove_dir_all(runner.binary("1.0.0").parent().unwrap()).unwrap();
    std::fs::remove_dir_all(runner.binary("4.0.0").parent().unwrap()).unwrap();
    assert_eq!(runner.prune("3.0.0").removed, ["1.0.0"]);
    assert!(
        runner.host_folder("4.0.0").exists(),
        "not this desktop's to remove"
    );
    assert!(runner.host_folder("2.0.0").exists());
}

#[test]
fn a_lock_that_can_not_be_opened_keeps_its_version() {
    let runner = Runner::with(&["1.0.0", "2.0.0", "3.0.0"]);
    let elsewhere = runner.config_root.join("elsewhere");
    std::fs::write(&elsewhere, b"x").unwrap();
    // A lock that is a link is never opened through it.
    std::os::unix::fs::symlink(&elsewhere, runner.socket_of("1.0.0").with_extension("lock"))
        .unwrap();
    let pruned = runner.prune("3.0.0");
    assert!(pruned.removed.is_empty());
    assert_eq!(pruned.kept, [("1.0.0".to_owned(), Live::Serving)]);
    assert!(runner.binary("1.0.0").exists() && runner.host_folder("1.0.0").exists());
    assert_eq!(std::fs::read(&elsewhere).unwrap(), b"x");
}

#[test]
fn a_host_record_that_is_not_a_regular_file_keeps_its_version() {
    let runner = Runner::with(&["1.0.0", "2.0.0", "3.0.0"]);
    let record = runner.host_folder("1.0.0").join(HOST_RECORD);
    std::fs::create_dir(&record).unwrap();
    assert_eq!(
        runner.prune("3.0.0").kept,
        [("1.0.0".to_owned(), Live::Chats)]
    );
    // Nor is one that is a link, whatever it names.
    std::fs::remove_dir(&record).unwrap();
    let empty = runner.config_root.join("empty");
    std::fs::write(&empty, b"").unwrap();
    std::os::unix::fs::symlink(&empty, &record).unwrap();
    assert_eq!(
        runner.prune("3.0.0").kept,
        [("1.0.0".to_owned(), Live::Chats)]
    );
    assert!(runner.binary("1.0.0").exists());
}

#[test]
fn the_host_record_named_is_the_one_the_host_keeps() {
    assert_eq!(HOST_RECORD, purlis_host::HOST_RECORD);
    assert_eq!(READ_AT_MOST, purlis_host::READ_AT_MOST);
}

#[test]
fn a_version_folder_that_is_a_link_is_removed_as_a_link_and_what_it_names_stays() {
    let runner = Runner::with(&["2.0.0", "3.0.0"]);
    let elsewhere = runner.config_root.join("elsewhere");
    std::fs::create_dir_all(&elsewhere).unwrap();
    std::fs::write(elsewhere.join(BINARY), b"x").unwrap();
    let server = runner
        .binary("2.0.0")
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .to_owned();
    std::os::unix::fs::symlink(&elsewhere, server.join("1.0.0")).unwrap();
    assert_eq!(runner.prune("3.0.0").removed, ["1.0.0"]);
    assert!(std::fs::symlink_metadata(server.join("1.0.0")).is_err());
    assert!(elsewhere.join(BINARY).exists(), "what the link named stays");
}

#[test]
fn what_the_runner_printed_names_the_versions_it_removed() {
    let printed = "removed=1.0.0\nkept=1.1.0 serving\nremoved=; rm -rf /\nremoved=1.2.0\n";
    assert_eq!(removed_from(printed), ["1.0.0", "1.2.0"]);
    assert_eq!(
        Pruned {
            removed: versions(&["1.0.0"]),
            kept: vec![("1.1.0".to_owned(), Live::Chats)],
            failed: Vec::new(),
        }
        .printed(),
        "removed=1.0.0\nkept=1.1.0 chats\n"
    );
}

#[test]
fn the_prune_command_runs_the_running_versions_binary_for_sh() {
    assert_eq!(
        prune_command("/home/r/.config/purlis", "3.0.0"),
        "exec sh -c '/home/r/.config/purlis/server/3.0.0/purlis bridge --prune'"
    );
}

#[test]
fn a_drained_versions_service_is_taken_back_first_while_its_lock_is_held() {
    let runner = Runner::with(&["1.0.0", "2.0.0", "3.0.0"]);
    let lock = runner.socket_of("1.0.0").with_extension("lock");
    std::fs::File::create(&lock).unwrap();
    let peek = runner.socket_of("1.0.0").with_extension("peek");
    std::fs::hard_link(&lock, &peek).unwrap();
    let steps = std::cell::RefCell::new(Vec::new());
    let pruned = prune_with(
        &runner.config_root,
        "3.0.0",
        std::slice::from_ref(&runner.socket),
        &mut |version| {
            let looked = std::fs::File::open(&peek).unwrap();
            let held = looked.try_lock_shared().is_err();
            steps
                .borrow_mut()
                .push(format!("service of {version}, lock held: {held}"));
            Ok(())
        },
        &mut |path| {
            steps
                .borrow_mut()
                .push(format!("removed {}", path.display()));
            remove(path)
        },
    );
    assert_eq!(pruned.removed, ["1.0.0"]);
    let steps = steps.into_inner();
    assert_eq!(
        steps.first().map(String::as_str),
        Some("service of 1.0.0, lock held: true"),
        "the service goes first, under the lock: {steps:?}"
    );
    assert_eq!(steps.len(), 6, "{steps:?}");
}

#[test]
fn a_version_whose_service_could_not_be_taken_back_is_kept_whole() {
    let runner = Runner::with(&["1.0.0", "2.0.0", "3.0.0"]);
    let mut removed = Vec::new();
    let pruned = prune_with(
        &runner.config_root,
        "3.0.0",
        std::slice::from_ref(&runner.socket),
        &mut |_| {
            Err("`systemctl --user daemon-reload` failed: Failed to connect to bus".to_owned())
        },
        &mut |path| {
            removed.push(path.to_owned());
            remove(path)
        },
    );
    assert!(pruned.removed.is_empty(), "{pruned:?}");
    assert!(removed.is_empty(), "nothing of it was removed: {removed:?}");
    assert_eq!(pruned.failed.len(), 1, "{pruned:?}");
    assert_eq!(pruned.failed[0].0, "1.0.0");
    assert!(
        pruned.failed[0]
            .1
            .contains("its service could not be removed"),
        "{pruned:?}"
    );
    assert!(runner.binary("1.0.0").exists() && runner.host_folder("1.0.0").exists());
}

#[test]
fn a_version_whose_host_serves_keeps_its_service() {
    let runner = Runner::with(&["1.0.0", "2.0.0", "3.0.0"]);
    let lock = std::fs::File::create(runner.socket_of("1.0.0").with_extension("lock")).unwrap();
    lock.lock().unwrap();
    let mut asked = Vec::new();
    let pruned = prune_with(
        &runner.config_root,
        "3.0.0",
        std::slice::from_ref(&runner.socket),
        &mut |version| {
            asked.push(version.to_owned());
            Ok(())
        },
        &mut remove,
    );
    assert_eq!(pruned.kept.len(), 1, "{pruned:?}");
    assert!(
        asked.is_empty(),
        "a serving version's service is left be: {asked:?}"
    );
}

#[test]
fn without_a_runtime_folder_named_prune_looks_where_the_user_manager_keeps_its_hosts() {
    let run_user = tempfile::tempdir().unwrap();
    let named = std::ffi::OsString::from("/run/user/1000");
    assert_eq!(
        runtime_dir(Some(&named), run_user.path()),
        Some(PathBuf::from("/run/user/1000")),
        "a runtime folder the environment names is the one"
    );
    assert_eq!(
        runtime_dir(None, run_user.path()),
        Some(run_user.path().to_path_buf()),
        "run from cron or `sudo -u`, it looks where a supervised host listens"
    );
    assert_eq!(runtime_dir(None, &run_user.path().join("absent")), None);
}

#[test]
fn a_version_serving_in_the_user_managers_runtime_folder_is_kept_though_the_one_named_is_empty() {
    // Prune run with another runtime folder named (a `su` shell, a job's own) still finds a host
    // systemd keeps, which listens in the user manager's (#6704).
    let runner = Runner::with(&["1.0.0", "2.0.0", "3.0.0"]);
    let managers = runner.socket.parent().unwrap().parent().unwrap().join("m");
    std::fs::create_dir_all(&managers).unwrap();
    let elsewhere = managers.join("purlisd.sock");
    let lock = std::fs::File::create(
        crate::hostsocket::of_version(&elsewhere, "1.0.0").with_extension("lock"),
    )
    .unwrap();
    lock.try_lock().unwrap();
    let sockets = [runner.socket.clone(), elsewhere.clone()];
    let pruned = prune_in(&runner.config_root, "3.0.0", &sockets);
    assert!(pruned.removed.is_empty(), "{pruned:?}");
    assert_eq!(pruned.kept, [("1.0.0".to_owned(), Live::Serving)]);
    assert!(runner.binary("1.0.0").exists());
    drop(lock);
    let pruned = prune_in(&runner.config_root, "3.0.0", &sockets);
    assert_eq!(pruned.removed, ["1.0.0"]);
    for left in ["lock", "sock", "link.sock"] {
        assert!(
            !crate::hostsocket::of_version(&elsewhere, "1.0.0")
                .with_extension(left)
                .exists(),
            "its {left} in the user manager's folder went too"
        );
    }
}

#[test]
fn prune_looks_in_the_runtime_folder_named_and_in_the_user_managers() {
    let run_user = tempfile::tempdir().unwrap();
    let ours = run_user.path().to_path_buf();
    let other = std::ffi::OsString::from("/run/user/0");
    assert_eq!(
        runtimes_to_look_in(Some(&other), &ours),
        [Some(PathBuf::from("/run/user/0")), Some(ours.clone())],
        "another folder named: both"
    );
    assert_eq!(
        runtimes_to_look_in(Some(ours.as_os_str()), &ours),
        [Some(ours.clone())],
        "the user manager's named: once"
    );
    assert_eq!(
        runtimes_to_look_in(None, &ours),
        [Some(ours.clone())],
        "none named: the user manager's"
    );
    let absent = ours.join("absent");
    assert_eq!(
        runtimes_to_look_in(None, &absent),
        [None],
        "no user manager: the folder a host takes with none named"
    );
}

#[test]
fn the_user_managers_runtime_folder_named_through_a_link_is_looked_in_once() {
    // `XDG_RUNTIME_DIR=/var/run/user/<uid>`, where `/var/run` links to `/run`: one folder.
    let root = tempfile::tempdir().unwrap();
    let ours = root.path().join("run").join("user").join("1000");
    std::fs::create_dir_all(&ours).unwrap();
    let var = root.path().join("var");
    std::fs::create_dir_all(&var).unwrap();
    std::os::unix::fs::symlink(root.path().join("run"), var.join("run")).unwrap();
    let named = var.join("run").join("user").join("1000");
    assert_eq!(
        runtimes_to_look_in(Some(named.as_os_str()), &ours),
        [Some(named)],
        "the folder named, and not the user manager's again"
    );
}

#[test]
fn a_drained_version_goes_though_its_runtime_folder_is_named_through_a_link() {
    // Looked in twice, one lock file would be opened twice, and the second `flock` would
    // conflict with the first, so the version would be kept as serving for ever.
    let runner = Runner::with(&["1.0.0", "2.0.0", "3.0.0"]);
    let run = runner.socket.parent().unwrap();
    let linked = run.parent().unwrap().join("l");
    std::os::unix::fs::symlink(run, &linked).unwrap();
    std::fs::File::create(runner.socket_of("1.0.0").with_extension("lock")).unwrap();
    let sockets: Vec<PathBuf> = runtimes_to_look_in(Some(linked.as_os_str()), run)
        .into_iter()
        .flatten()
        .map(|runtime| runtime.join("purlisd.sock"))
        .collect();
    let pruned = prune_in(&runner.config_root, "3.0.0", &sockets);
    assert_eq!(pruned.removed, ["1.0.0"], "{pruned:?}");
    assert!(pruned.kept.is_empty(), "{pruned:?}");
    assert_eq!(sockets.len(), 1, "{sockets:?}");
}

#[test]
fn a_version_is_old_once_a_later_one_is_installed_beside_it_with_its_binary() {
    let all = |_: &str| true;
    let runner = Runner::with(&["1.9.0", "1.10.0"]);
    assert!(
        newer_beside(&runner.config_root, "1.9.0", &all),
        "1.10.0 is later"
    );
    assert!(
        !newer_beside(&runner.config_root, "1.10.0", &all),
        "the newest"
    );
    // A dev build is before its release, and its release is later.
    let runner = Runner::with(&["2.0.0-dev.3", "2.0.0"]);
    assert!(newer_beside(&runner.config_root, "2.0.0-dev.3", &all));
    // A folder not read as a version, or a later one with no binary yet (an install cut short),
    // makes nothing old.
    let runner = Runner::with(&["1.0.0"]);
    let server = crate::machine::dir(&runner.config_root).join(SERVER_DIR);
    std::fs::create_dir_all(server.join("9.0")).unwrap();
    std::fs::write(server.join("9.0").join(BINARY), b"x").unwrap();
    std::fs::create_dir_all(server.join("2.0.0")).unwrap();
    assert!(!newer_beside(&runner.config_root, "1.0.0", &all));
    std::fs::write(server.join("2.0.0").join(BINARY), b"x").unwrap();
    assert!(newer_beside(&runner.config_root, "1.0.0", &all));
    // A version not read as one is never old.
    assert!(!newer_beside(&runner.config_root, "1.0", &all));
}

#[test]
fn a_later_version_whose_service_does_not_start_at_boot_takes_over_from_nothing() {
    // A person going back to 1.0.0 beside 2.0.0 disables or takes back 2.0.0's service: 1.0.0's
    // host serves on, and does not stop itself for a version nothing starts (#6704).
    let runner = Runner::with(&["1.0.0", "2.0.0", "3.0.0"]);
    assert!(newer_beside(
        &runner.config_root,
        "1.0.0",
        &|version| version == "2.0.0"
    ));
    assert!(newer_beside(
        &runner.config_root,
        "1.0.0",
        &|version| version == "3.0.0"
    ));
    assert!(!newer_beside(&runner.config_root, "1.0.0", &|_| false));
    assert!(
        !newer_beside(&runner.config_root, "1.0.0", &|version| version == "1.0.0"),
        "its own service is not a newer one's"
    );
    assert!(
        !newer_beside(&runner.config_root, "2.0.0", &|version| version == "1.0.0"),
        "an older version's service takes over from nothing"
    );
}
