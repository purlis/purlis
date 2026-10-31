// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use std::path::{Path, PathBuf};

use super::*;

fn machine(restricts: bool, profiles: &[&str], home: &str) -> Machine {
    Machine {
        restricts,
        profiles: profiles.iter().map(|one| (*one).to_owned()).collect(),
        home: Some(PathBuf::from(home)),
    }
}

/// The engines folder of a data home `data`.
fn engines(data: &str) -> PathBuf {
    crate::browsers::engines_in(Path::new(data))
}

#[test]
fn where_user_namespaces_are_not_restricted_the_engine_sandboxes_with_no_profile() {
    let here = machine(false, &[], "/home/ada");
    assert!(sandboxes(&here, &engines("/home/ada/.local/share/purlis")));
    assert!(sandboxes(&here, &engines("/srv/elsewhere")));
}

#[test]
fn where_they_are_restricted_and_no_profile_names_the_engine_it_cannot_sandbox() {
    let here = machine(true, &[], "/home/ada");
    assert!(!sandboxes(&here, &engines("/home/ada/.local/share/purlis")));
}

#[test]
fn the_profile_the_package_installs_names_the_engine_in_the_default_data_folder_alone() {
    let here = machine(true, &[SHIPPED], "/home/ada");
    assert!(sandboxes(&here, &engines("/home/ada/.local/share/purlis")));
    assert!(
        sandboxes(&here, &engines("/home/ada/.local/share/charter")),
        "a data folder rename-local has not moved yet"
    );
    assert!(
        !sandboxes(&here, &engines("/home/ada/elsewhere")),
        "a data folder the person moved"
    );
    assert!(
        !sandboxes(&here, &engines("/home/bob/.local/share/purlis")),
        "another person's home"
    );
}

#[test]
fn the_profile_the_notice_offers_names_this_machines_engine_wherever_its_data_folder_is() {
    let data = engines("/srv/data with space/purlis");
    let profile = profile_for(&data).expect("a profile");
    let here = machine(true, &[&profile], "/home/ada");
    assert!(sandboxes(&here, &data));
    assert!(!sandboxes(&here, &engines("/home/ada/.local/share/purlis")));
    assert!(profile.contains("userns,"), "{profile}");
    assert!(profile.contains("profile purlis-engine-here"), "{profile}");
}

#[test]
fn the_shipped_profile_allows_user_namespaces_and_names_one_program() {
    assert!(SHIPPED.contains("userns,"));
    assert_eq!(
        SHIPPED
            .lines()
            .filter(|line| line.trim_start().starts_with("profile "))
            .count(),
        1,
        "one profile"
    );
    assert_eq!(
        attachment(SHIPPED),
        Some(
            "@{HOME}/.local/share/{purlis,charter}/engines/chrome-for-testing/*/chrome-linux64/chrome"
                .to_owned()
        )
    );
}

#[test]
fn the_command_writes_the_profile_and_loads_it_and_runs_nothing_else() {
    let data = engines("/srv/it's here");
    let command = install_command(&data).expect("a command");
    assert!(command.starts_with("printf '%s\\n' "), "{command}");
    assert!(
        command.ends_with(&format!(
            "| sudo tee {HERE_PROFILE} >/dev/null && sudo apparmor_parser -r {HERE_PROFILE}"
        )),
        "{command}"
    );
    // What the shell gives `tee` is the profile, line by line.
    let lines = shell_words(command.split(" | ").next().unwrap());
    assert_eq!(lines[0], "printf");
    assert_eq!(lines[1..].join("\n").trim_start_matches("%s\\n\n"), {
        let profile = profile_for(&data).expect("a profile");
        profile.trim_end().to_owned()
    });
}

#[test]
fn the_command_for_another_machine_installs_the_shipped_profile_where_the_deb_does() {
    let command = install_shipped_command();
    assert!(
        command.ends_with(&format!(
            "| sudo tee {PROFILE} >/dev/null && sudo apparmor_parser -r {PROFILE}"
        )),
        "{command}"
    );
    let lines = shell_words(command.split(" | ").next().unwrap());
    assert_eq!(
        lines[1..].join("\n").trim_start_matches("%s\\n\n"),
        SHIPPED.trim_end()
    );
}

#[test]
fn what_apparmor_reads_as_a_pattern_in_the_data_folder_is_written_as_itself() {
    let data = engines("/srv/{a,b} [x]?*^@{HOME}/purlis");
    let profile = profile_for(&data).expect("a profile");
    let line = profile
        .lines()
        .find(|line| line.starts_with("profile "))
        .expect("its line");
    assert!(
        line.contains(
            r#""/srv/\{a,b\} \[x\]\?\*\^\@\{HOME\}/purlis/engines/chrome-for-testing/*/"#
        ),
        "{line}"
    );
}

#[test]
fn a_data_folder_with_a_control_character_in_it_is_offered_no_profile_and_no_command() {
    let data = engines("/srv/one\ntwo");
    assert_eq!(profile_for(&data), None);
    assert_eq!(install_command(&data), None);
}

#[test]
fn an_engine_log_that_says_it_had_no_sandbox_is_told_apart() {
    assert!(said_no_usable_sandbox(
        "[1:1:1020/154758.614434:FATAL:content/browser/zygote_host/zygote_host_impl_linux.cc:130] No usable sandbox! If you are running on Ubuntu 23.10+"
    ));
    assert!(!said_no_usable_sandbox("DevTools listening"));
}

/// The words of `line` as a POSIX shell reads them, for the single quotes `install_command`
/// writes.
fn shell_words(line: &str) -> Vec<String> {
    let mut words = Vec::new();
    let mut word = String::new();
    let mut quoted = false;
    let mut escaped = false;
    let mut any = false;
    for c in line.chars() {
        if escaped {
            word.push(c);
            escaped = false;
            continue;
        }
        match c {
            '\'' => {
                quoted = !quoted;
                any = true;
            }
            ' ' if !quoted => {
                if any || !word.is_empty() {
                    words.push(std::mem::take(&mut word));
                }
                any = false;
            }
            '\\' if !quoted => escaped = true,
            c => word.push(c),
        }
    }
    if any || !word.is_empty() {
        words.push(word);
    }
    words
}

/// The restriction is an unconfined process's alone: a runner's host in a container runs under
/// its runtime's profile, which decides for it (#3298). An unread label is taken as unconfined.
#[test]
fn ubuntus_restriction_holds_an_unconfined_process_alone() {
    assert!(restricts(Some("1\n"), Some("unconfined\n")));
    assert!(restricts(Some("1\n"), None));
    assert!(restricts(Some("1"), Some("")));
    assert!(!restricts(Some("1\n"), Some("docker-default (enforce)\n")));
    assert!(!restricts(
        Some("1\n"),
        Some("containers-default-0.57.4 (enforce)\n")
    ));
    assert!(!restricts(Some("0\n"), Some("unconfined\n")));
    assert!(!restricts(None, None));
}
