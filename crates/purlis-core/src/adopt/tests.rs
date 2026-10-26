// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use super::*;

#[test]
fn this_charters_version_is_the_workspaces_one_as_numbers() {
    // The one number `charter version` prints and a pin is compared with: the app's, which
    // every crate in the workspace shares — never a placeholder, and always dotted numbers.
    assert_eq!(app_version(), env!("CARGO_PKG_VERSION"));
    let parts: Vec<&str> = app_version().split('.').collect();
    assert_eq!(parts.len(), 3, "{}", app_version());
    assert!(
        parts
            .iter()
            .all(|p| !p.is_empty() && p.chars().all(|c| c.is_ascii_digit())),
        "{}",
        app_version()
    );
}

fn said(report: &Report) -> String {
    report
        .said
        .iter()
        .map(|line| match line {
            Say::Info(t) | Say::Ok(t) | Say::Warn(t) | Say::Err(t) => t.to_string(),
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn it_says_which_half_it_does_and_where_to_read_what_a_version_brought() {
    let report = update_report(&UpdateArgs::default());
    assert!(matches!(report.said.first(), Some(Say::Warn(_))));
    assert!(said(&report).contains("does not install anything here"));
    assert!(said(&report).contains("purlis news"), "{}", said(&report));
    assert!(report.out.is_empty(), "{}", report.out);
    assert_eq!(report.code, 0);
}

#[test]
fn a_python_update_baseline_is_not_read_and_not_mentioned() {
    // #352: the range view is retired, so an update baseline the Python charter left on a
    // plane is history like its corpus, and no sentence here names one.
    let report = update_report(&UpdateArgs::default());
    assert!(!said(&report).contains("baseline"), "{}", said(&report));
    assert!(!said(&report).contains("--pending"), "{}", said(&report));
}

#[test]
fn the_two_flags_that_install_a_charter_are_refused_by_name() {
    let args = UpdateArgs {
        to: "0.63.0".to_owned(),
        bump: true,
    };
    let text = said(&update_report(&args));
    assert!(text.contains("--to names a published version"));
    assert!(text.contains("--bump moves this plane's `[charter] version` pin"));
}

// `charter version` (M2.12, ADR 0030 as amended by ADR 0045)

/// A plane whose manifest is `manifest`.
fn pinned(manifest: &str) -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join(crate::plane::MANIFEST), manifest).unwrap();
    dir
}

#[test]
fn the_rows_name_the_apps_version_and_the_pin_and_nothing_from_the_news_corpus() {
    // ADR 0045: one number for "which charter is this", and it is the app's. The frozen
    // corpus reaches 0.62.1, and printing that here is the answer this replaced.
    let dir = pinned("");
    let report = version_report(Some(dir.path()));
    assert_eq!(
        report.out,
        format!("  purlis    {}\n  pinned     {NO_PIN}\n\n", app_version())
    );
    assert!(
        !report.out.contains(PYTHON_LINE_LAST),
        "the corpus's version leaked into the rows: {}",
        report.out
    );
    assert_eq!(report.code, 0, "pinning nothing is not drift");
}

#[test]
fn the_python_lines_last_release_is_not_the_apps_version() {
    assert!(
        crate::version::version_key(app_version()) != crate::version::version_key(PYTHON_LINE_LAST),
        "the app's version and the Python line's last are the same number; the verdict \
         for that pin would be `Met` and the tests below would say less than they claim"
    );
}

#[test]
fn the_apps_own_version_is_met_and_says_so_without_claiming_a_python_parity() {
    let met = pinned(&format!("[charter]\nversion = \"{}\"\n", app_version()));
    let report = version_report(Some(met.path()));
    assert_eq!(report.code, 0, "{}", said(&report));
    assert!(
        said(&report).contains(&format!("this purlis is {}, which is what", app_version())),
        "{}",
        said(&report)
    );
    assert!(!said(&report).contains("in sync with the lock"));
}

#[test]
fn a_pin_on_the_python_line_is_named_as_that_and_is_not_drift() {
    // The plane every Python charter left behind: pinned to a charter-cp release. Opening
    // it with the app must not turn it red, and must not tell the operator to run Python.
    for pin in [PYTHON_LINE_LAST, "0.44.0", "0.2.0"] {
        if pin == app_version() {
            continue;
        }
        let dir = pinned(&format!("[charter]\nversion = \"{pin}\"\n"));
        let report = version_report(Some(dir.path()));
        let text = said(&report);
        assert_eq!(report.code, 0, "{pin}: {text}");
        assert!(
            text.contains(&format!(
                "this control plane pins {pin}, a release of the Python charter"
            )),
            "{text}"
        );
        assert!(text.contains("not drift"), "{text}");
        assert!(!text.contains("drift:"), "{text}");
        assert!(
            text.contains(&format!("[charter] version = \"{}\"", app_version())),
            "the way to hold the plane to this app is named: {text}"
        );
        assert!(
            !text.contains("uv tool") && !text.contains("run the charter-cp"),
            "{text}"
        );
        assert!(report.out.contains(&format!("  pinned     {pin}\n")));
    }
}

#[test]
fn a_pin_past_the_python_line_that_this_app_is_not_is_drift_and_exits_one() {
    for pin in ["0.62.2", "0.63.0", "1.0.0", "not-a-version"] {
        let dir = pinned(&format!("[charter]\nversion = \"{pin}\"\n"));
        let report = version_report(Some(dir.path()));
        let text = said(&report);
        assert_eq!(report.code, 1, "{pin}: drift is exit 1");
        assert!(
            text.contains(&format!(
                "drift: this control plane pins {pin}, and this purlis is {}.",
                app_version()
            )),
            "{text}"
        );
        assert!(!text.contains("charter-cp"), "{text}");
    }
}

#[test]
fn every_surface_reads_one_verdict() {
    assert_eq!(pin_verdict(None), PinVerdict::Unpinned);
    assert_eq!(
        pin_verdict(Some(app_version())),
        PinVerdict::Met(app_version().to_owned())
    );
    assert_eq!(
        pin_verdict(Some("0.50.0")),
        PinVerdict::PythonLine("0.50.0".to_owned())
    );
    assert_eq!(
        pin_verdict(Some("9.0.0")),
        PinVerdict::Drift("9.0.0".to_owned())
    );
    assert_eq!(PinVerdict::Unpinned.code(), 0);
    assert_eq!(PinVerdict::PythonLine("0.50.0".to_owned()).code(), 0);
    assert_eq!(PinVerdict::Drift("9.0.0".to_owned()).code(), 1);
}

#[test]
fn a_pin_is_reported_as_written_and_a_manifest_that_pins_nothing_reads_as_no_pin() {
    // `instance.locked_version`: as written, malformed included, because folding a bad
    // pin into "no pin" makes a plane that opted into conformance behave like one that
    // never did.
    let junk = pinned("[charter]\nversion = \"  not-a-version  \"\n");
    assert_eq!(
        locked_version(junk.path()).as_deref(),
        Some("not-a-version"),
        "the value is stripped, not judged"
    );
    for manifest in [
        "",
        "schema = 1\n",
        "[charter]\n",
        "[charter]\nversion = \"\"\n",
        "[charter]\nversion = \"   \"\n",
        // `isinstance(v, str)` in Python: a number is not a pin.
        "[charter]\nversion = 62\n",
        // `charter` is not a table: Python raises here, and this answers no-pin rather
        // than reproducing a traceback.
        "charter = \"nope\"\n",
        "this is not toml at all [[[\n",
    ] {
        let dir = pinned(manifest);
        assert_eq!(
            locked_version(dir.path()),
            None,
            "{manifest:?} read as a pin"
        );
    }
    assert_eq!(locked_version(Path::new("/nonexistent/plane")), None);
}

#[test]
fn outside_a_plane_there_is_no_pin_to_report_and_nothing_is_read() {
    let report = version_report(None);
    assert!(report.out.contains(NO_PIN), "{}", report.out);
    assert_eq!(report.code, 0);
}

#[test]
fn moving_a_published_release_is_refused_by_name_and_exits_one() {
    for verb in ["sync", "bump"] {
        let report = version_move_refusal(verb);
        assert_eq!(report.code, 1, "a refusal is not a success");
        assert!(report.out.is_empty(), "nothing goes to stdout");
        let text = said(&report);
        assert!(
            text.contains(&format!("purlis version {verb} moves a pin")),
            "{text}"
        );
        assert!(text.contains(THE_APP_MOVES_IT), "{text}");
        assert!(!text.contains("charter-cp"), "{text}");
    }
}

#[test]
fn one_sentence_says_what_moves_this_charter_and_both_commands_use_it() {
    // The constant exists so `update` and `version` cannot drift into two accounts of one
    // artifact. This is what holds that: both outputs carry the same bytes.
    let dir = tempfile::tempdir().unwrap();
    let update = said(&update_report(&UpdateArgs::default()));
    let version = said(&version_report(Some(dir.path())));
    assert!(update.contains(THE_APP_MOVES_IT), "{update}");
    assert!(version.contains(THE_APP_MOVES_IT), "{version}");
}

// `charter update --channel` (ADR 0042)

#[test]
fn update_says_which_channel_the_app_takes_charter_from_right_under_the_refusal() {
    let machine = tempfile::tempdir().unwrap();
    let report = update_report_with_channel(Some(machine.path()), &UpdateArgs::default());
    let Some(Say::Info(line)) = report.said.get(1) else {
        panic!("{:?}", report.said);
    };
    assert!(line.contains("from the stable channel"), "{line}");
    assert!(line.contains("purlis update --channel dev"), "{line}");
}

#[test]
fn a_channel_set_from_the_terminal_is_the_one_the_app_reads() {
    let machine = tempfile::tempdir().unwrap();
    let report = set_channel_report(Some(machine.path()), "dev");
    assert_eq!(report.code, 0, "{}", said(&report));
    assert_eq!(
        crate::machine::read(machine.path()).store.channel,
        crate::updates::Channel::Dev
    );
    let back = update_report_with_channel(Some(machine.path()), &UpdateArgs::default());
    assert!(
        said(&back).contains("from the dev channel"),
        "{}",
        said(&back)
    );
}

#[test]
fn a_word_that_is_not_a_channel_exits_one_and_moves_nothing() {
    let machine = tempfile::tempdir().unwrap();
    set_channel_report(Some(machine.path()), "dev");
    for word in ["Dev", "STABLE", "nightly", "", "dev "] {
        let report = set_channel_report(Some(machine.path()), word);
        assert_eq!(report.code, 1, "{word:?} was accepted");
        assert_eq!(
            crate::machine::read(machine.path()).store.channel,
            crate::updates::Channel::Dev,
            "{word:?} moved the channel"
        );
    }
}

#[test]
fn the_version_a_build_says_is_its_releases_stamp_of_the_workspaces_one() {
    // Unstamped, the crate's own; stamped by a release, that version or a prerelease of it.
    let own = env!("CARGO_PKG_VERSION");
    assert!(
        release_version() == own || release_version().starts_with(&format!("{own}-")),
        "{}",
        release_version()
    );
    assert!(VERSION_LINE.starts_with(&format!("{} (", release_version())));
}
