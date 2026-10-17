// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! `tools/no-orphans.sh`, the CI step that fails a suite which leaves a process behind (#923).
#![cfg(unix)]

use std::path::PathBuf;
use std::process::Command;

fn the_check() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tools/no-orphans.sh")
}

#[test]
fn a_command_that_leaves_a_process_behind_fails_and_names_it() {
    // A sleep with a marker in its argument, so its line in the report can be told apart from
    // anything else this user started meanwhile. Not `--reap`: on a shared machine that would
    // kill those too. The test kills its own sleep, by the pid the report names.
    let said = Command::new(the_check())
        .args(["--grace", "1", "--", "/bin/sh", "-c"])
        .arg("sleep 47.923 </dev/null >/dev/null 2>&1 & exit 0")
        // As on a runner, where the list is an annotation on the run too.
        .env("GITHUB_ACTIONS", "true")
        .output()
        .expect("the check runs");

    let stderr = String::from_utf8_lossy(&said.stderr);
    let orphan = stderr.lines().find_map(|line| {
        let rest = line.trim().strip_prefix("pid ")?;
        let (pid, cmd) = rest.split_once(": ")?;
        cmd.contains("sleep 47.923")
            .then(|| pid.parse::<u32>().ok())?
    });
    if let Some(pid) = orphan {
        drop(stand_in::Ends::group(pid));
    }
    assert_eq!(said.status.code(), Some(1), "{stderr}");
    assert!(orphan.is_some(), "the orphan was not named: {stderr}");
    // A red run is read through its annotations, never its log: the orphan is named there,
    // under the title a failed test has.
    let stdout = String::from_utf8_lossy(&said.stdout);
    assert!(
        stdout.lines().any(|line| line
            .starts_with("::error title=Failed%3A processes left running after the command::")
            && line.contains("sleep 47.923")
            && line.contains(", running for ")),
        "the orphan was not annotated: {stdout}"
    );
}

#[test]
fn off_a_runner_the_orphan_is_named_on_stderr_only() {
    let said = Command::new(the_check())
        .args(["--grace", "1", "--", "/bin/sh", "-c"])
        .arg("sleep 47.375 </dev/null >/dev/null 2>&1 & exit 0")
        .env_remove("GITHUB_ACTIONS")
        .output()
        .expect("the check runs");

    let stderr = String::from_utf8_lossy(&said.stderr);
    let orphan = stderr.lines().find_map(|line| {
        let rest = line.trim().strip_prefix("pid ")?;
        let (pid, cmd) = rest.split_once(": ")?;
        cmd.contains("sleep 47.375")
            .then(|| pid.parse::<u32>().ok())?
    });
    if let Some(pid) = orphan {
        drop(stand_in::Ends::group(pid));
    }
    assert_eq!(said.status.code(), Some(1), "{stderr}");
    let stdout = String::from_utf8_lossy(&said.stdout);
    assert!(!stdout.contains("::error"), "{stdout}");
}

#[test]
fn the_annotation_lists_a_bounded_number_of_processes_and_a_bounded_part_of_each() {
    // A `ps` of the test's own answers for the check, so nothing is started and nothing is
    // left: one process before the command, and twenty-five more after it, this user's, each
    // with a command line three hundred characters past its name.
    let dir = tempfile::tempdir().expect("a directory");
    let long = "x".repeat(300);
    stand_in::program(
        dir.path(),
        "ps",
        &format!(
            "#!/bin/sh\nhere=$(dirname \"$0\")\n\
             case \"$*\" in *etime*) echo ' 00:07'; exit 0 ;; esac\n\
             echo '1 0 0 Wed Oct 14 01:00:00 2026 /sbin/init'\n\
             if [ -e \"$here/asked\" ]; then\n\
             me=$(id -u); i=1\n\
             while [ \"$i\" -le 25 ]; do\n\
             echo \"$((9000000 + i)) 1 $me Wed Oct 14 02:00:00 2026 left-$i-of-25 {long}\"\n\
             i=$((i + 1))\n\
             done\n\
             fi\n\
             : > \"$here/asked\"\n"
        ),
    );
    let path = format!(
        "{}:{}",
        dir.path().display(),
        std::env::var("PATH").unwrap_or_default()
    );

    let said = Command::new(the_check())
        .args(["--grace", "0", "--", "true"])
        .env("PATH", path)
        .env("GITHUB_ACTIONS", "true")
        .output()
        .expect("the check runs");

    let stderr = String::from_utf8_lossy(&said.stderr);
    assert_eq!(said.status.code(), Some(1), "{stderr}");
    // The log still names every one, in full.
    assert_eq!(
        stderr
            .lines()
            .filter(|line| line.trim().starts_with("pid ") && line.ends_with(&long))
            .count(),
        25,
        "{stderr}"
    );
    let stdout = String::from_utf8_lossy(&said.stdout);
    let annotation = stdout
        .lines()
        .find_map(|line| {
            line.strip_prefix("::error title=Failed%3A processes left running after the command::")
        })
        .unwrap_or_else(|| panic!("no annotation: {stdout}"));
    let lines: Vec<&str> = annotation.split("%0A").collect();
    assert_eq!(
        lines.len(),
        21,
        "twenty processes and what is left out: {lines:?}"
    );
    for (at, line) in lines[..20].iter().enumerate() {
        let before = format!("pid {}, running for 00:07: ", 9_000_001 + at);
        let command = line
            .strip_prefix(before.as_str())
            .unwrap_or_else(|| panic!("line {at} is {line}"));
        assert!(
            command.starts_with(&format!("left-{}-of-25 x", at + 1)),
            "{line}"
        );
        // Two hundred characters of the command line, and the mark that it was cut.
        assert_eq!(command.len(), 200 + "...".len(), "{line}");
        assert!(command.ends_with("x..."), "{line}");
    }
    assert_eq!(lines[20], "and 5 more, each named in the job's log");
}

#[test]
fn a_command_that_cleans_up_after_itself_keeps_its_own_status() {
    let said = Command::new(the_check())
        .args([
            "--grace",
            "0",
            "--",
            "/bin/sh",
            "-c",
            "sleep 0.1 & wait; exit 3",
        ])
        .output()
        .expect("the check runs");

    let stderr = String::from_utf8_lossy(&said.stderr);
    // Anything else this user started meanwhile would be counted too, which is why CI runs it
    // on a machine of its own; here, only a report that names this command's own work fails.
    if said.status.code() == Some(1) {
        assert!(
            !stderr
                .lines()
                .any(|line| line.trim().starts_with("pid ") && line.contains("sleep 0.1")),
            "{stderr}"
        );
    } else {
        assert_eq!(said.status.code(), Some(3), "{stderr}");
    }
}

#[test]
fn a_check_that_cannot_list_the_processes_fails_rather_than_passing_blind() {
    // A `ps` that fails, or answers nothing, must not read as "nothing was left behind".
    let dir = tempfile::tempdir().expect("a directory");
    for (name, ps) in [
        ("failing", "#!/bin/sh\nexit 1\n"),
        ("silent", "#!/bin/sh\nexit 0\n"),
    ] {
        let bin = dir.path().join(name);
        std::fs::create_dir(&bin).expect("a bin directory");
        stand_in::program(&bin, "ps", ps);
        let path = format!(
            "{}:{}",
            bin.display(),
            std::env::var("PATH").unwrap_or_default()
        );

        let said = Command::new(the_check())
            .args(["--grace", "0", "--", "true"])
            .env("PATH", path)
            .output()
            .expect("the check runs");

        let stderr = String::from_utf8_lossy(&said.stderr);
        assert_eq!(said.status.code(), Some(3), "a {name} ps: {stderr}");
        assert!(stderr.contains("nothing was checked"), "{stderr}");
    }
}

#[test]
fn on_macos_an_agent_launchd_starts_for_this_user_is_not_the_suites() {
    // #2458: on a macOS runner launchd starts the runner user's own agents while a suite runs
    // (nbagent, mdworker_shared, CoreSimulator's services…), each with launchd as its parent
    // and its program in the system's own folders. They are not the suite's, and counting them
    // failed every nightly part and killed them. A process the suite left behind is reparented
    // to launchd too, so it is the program's folder that tells them apart; and Linux, where
    // the check has never miscounted, keeps counting every one.
    let dir = tempfile::tempdir().expect("a directory");
    let state = dir.path().join("calls");
    let ps = format!(
        "#!/bin/sh\n\
         me=$(id -u)\n\
         n=$(cat '{state}' 2>/dev/null || echo 0)\n\
         echo $((n + 1)) > '{state}'\n\
         echo '    1     0     0 Fri Oct 17 16:00:00 2026 /sbin/launchd'\n\
         if [ \"$n\" -gt 0 ]; then\n\
         echo \"  901     1 $me Fri Oct 17 16:01:00 2026 /System/Library/PrivateFrameworks/Noticeboard.framework/Versions/A/Resources/nbagent.app/Contents/MacOS/nbagent\"\n\
         echo \"  902     1 $me Fri Oct 17 16:01:00 2026 /usr/libexec/proactiveeventtrackerd\"\n\
         echo \"  903     1 $me Fri Oct 17 16:01:00 2026 /Library/Developer/PrivateFrameworks/CoreSimulator.framework/Versions/A/XPCServices/SimLaunchHost.arm64.xpc/Contents/MacOS/SimLaunchHost.arm64\"\n\
         echo \"  904     1 $me Fri Oct 17 16:01:00 2026 /bin/sleep 47.2458\"\n\
         fi\n",
        state = state.display()
    );
    for (os, counted) in [
        ("Darwin", &["/bin/sleep 47.2458"][..]),
        (
            "Linux",
            &[
                "/bin/sleep 47.2458",
                "nbagent",
                "/usr/libexec/proactiveeventtrackerd",
                "SimLaunchHost.arm64",
            ][..],
        ),
    ] {
        let _ = std::fs::remove_file(&state);
        let bin = dir.path().join(os);
        std::fs::create_dir(&bin).expect("a bin directory");
        stand_in::program(&bin, "ps", &ps);
        stand_in::program(&bin, "uname", &format!("#!/bin/sh\necho {os}\n"));
        let path = format!(
            "{}:{}",
            bin.display(),
            std::env::var("PATH").unwrap_or_default()
        );

        let said = Command::new(the_check())
            .args(["--grace", "0", "--", "true"])
            .env("PATH", path)
            .env_remove("GITHUB_ACTIONS")
            .output()
            .expect("the check runs");

        let stderr = String::from_utf8_lossy(&said.stderr);
        assert_eq!(said.status.code(), Some(1), "{os}: {stderr}");
        let named = stderr
            .lines()
            .filter(|line| line.trim().starts_with("pid "))
            .count();
        assert_eq!(named, counted.len(), "{os}: {stderr}");
        for one in counted {
            assert!(stderr.contains(one), "{os}: {one} was not named: {stderr}");
        }
    }
}

#[test]
fn a_grace_with_no_value_is_a_usage_error() {
    let said = Command::new(the_check())
        .arg("--grace")
        .output()
        .expect("the check runs");

    assert_eq!(said.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&said.stderr).contains("usage:"));
}
