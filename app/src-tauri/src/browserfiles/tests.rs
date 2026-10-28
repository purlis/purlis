// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use std::path::PathBuf;

use purlis_session_protocol::browsing::{Act, DownloadSaid, DownloadState};

use super::{ChatFolder, Files, Prepared, save_asked_said, save_kept_out_said};

fn at(folder: &str) -> ChatFolder {
    ChatFolder {
        folder: PathBuf::from(folder),
        root: PathBuf::from("/p"),
        kept: vec![PathBuf::from("/p/.purlis/vaults")],
    }
}

fn from_here() -> Result<ChatFolder, String> {
    Ok(at("/p/workspaces/app"))
}

fn upload(files: &[&str]) -> Act {
    Act::Upload {
        target: "e4".into(),
        files: files.iter().map(|f| (*f).to_owned()).collect(),
        from: Some("/elsewhere".into()),
    }
}

fn save(download: &str, to: &str) -> Act {
    Act::SaveDownload {
        download: download.into(),
        to: to.into(),
        into: Some("/elsewhere".into()),
    }
}

fn done(download: &str, origin: Option<&str>) -> DownloadSaid {
    DownloadSaid {
        download: download.into(),
        name: "report.pdf".into(),
        origin: origin.map(str::to_owned),
        bytes: 10,
        total: Some(10),
        state: DownloadState::Done,
        file: "GUID".into(),
    }
}

#[test]
fn an_upload_names_the_chat_s_folder_as_the_window_knows_it_never_the_one_it_was_sent() {
    let files = Files::default();
    assert_eq!(
        files.prepared(1, "B", upload(&["docs/a.pdf"]), from_here, false, 0),
        Prepared::Go(Act::Upload {
            target: "e4".into(),
            files: vec!["docs/a.pdf".into()],
            from: Some("/p/workspaces/app".into()),
        })
    );
}

#[test]
fn an_upload_from_outside_the_chat_s_folder_or_of_what_purlis_keeps_is_refused() {
    let files = Files::default();
    for outside in ["../other/secret", "/etc/hosts", "~/.ssh/id_ed25519"] {
        assert!(
            matches!(
                files.prepared(1, "B", upload(&[outside]), from_here, false, 0),
                Prepared::No(_)
            ),
            "{outside}"
        );
    }
    let kept_inside = || Ok(at("/p"));
    let Prepared::No(why) = files.prepared(
        1,
        "B",
        upload(&[".PURLIS/vaults/forge.json"]),
        kept_inside,
        false,
        0,
    ) else {
        panic!("a kept file is refused, whatever its spelling");
    };
    assert!(why.contains("keeps from every chat"), "{why}");
}

#[test]
fn a_runner_chat_s_browser_takes_no_files() {
    let files = Files::default();
    assert_eq!(
        files.prepared(1, "B", upload(&["a.pdf"]), from_here, true, 0),
        Prepared::No(super::NOT_ON_A_RUNNER.to_owned())
    );
}

#[test]
fn a_folder_that_cannot_be_told_refuses_the_upload_in_its_words() {
    let files = Files::default();
    assert_eq!(
        files.prepared(1, "B", upload(&["a"]), || Err("why".into()), false, 0),
        Prepared::No("why".into())
    );
}

#[test]
fn a_download_is_saved_only_once_the_person_allowed_its_origin_for_that_chat() {
    let files = Files::default();
    files.heard("B", &[done("d1", Some("https://files.example.com"))]);
    // The first save asks, and saves nothing.
    assert_eq!(
        files.prepared(1, "B", save("d1", "downloads/r.pdf"), from_here, false, 7),
        Prepared::Asked(save_asked_said("https://files.example.com"))
    );
    let waits = files.saves_waiting();
    assert_eq!(waits.len(), 1);
    assert_eq!(waits[0].session, 1);
    assert_eq!(waits[0].since, 7);
    assert_eq!(waits[0].origins, ["https://files.example.com"]);
    // Asked once: the next call waits, and asks nothing more.
    assert!(matches!(
        files.prepared(1, "B", save("d1", "downloads/r.pdf"), from_here, false, 8),
        Prepared::No(_)
    ));
    assert_eq!(files.saves_waiting().len(), 1);
    // Another chat is asked on its own.
    files.heard("C", &[done("d1", Some("https://files.example.com"))]);
    assert!(matches!(
        files.prepared(2, "C", save("d1", "r.pdf"), from_here, false, 9),
        Prepared::Asked(_)
    ));
    files
        .saves_answered(1, &["https://files.example.com".to_owned()], true)
        .unwrap();
    assert_eq!(
        files.prepared(1, "B", save("d1", "downloads/r.pdf"), from_here, false, 10),
        Prepared::Go(Act::SaveDownload {
            download: "d1".into(),
            to: "downloads/r.pdf".into(),
            into: Some("/p/workspaces/app".into()),
        })
    );
    // Allowed for chat 1 alone.
    assert!(matches!(
        files.prepared(2, "C", save("d1", "r.pdf"), from_here, false, 11),
        Prepared::No(_)
    ));
}

#[test]
fn an_origin_kept_out_refuses_its_saves_and_an_answer_to_nothing_waiting_is_refused() {
    let files = Files::default();
    files.heard("B", &[done("d1", Some("https://x.test"))]);
    let _ = files.prepared(1, "B", save("d1", "r"), from_here, false, 0);
    assert!(
        files
            .saves_answered(1, &["https://other.test".to_owned()], true)
            .is_err()
    );
    files
        .saves_answered(1, &["https://x.test".to_owned()], false)
        .unwrap();
    assert_eq!(
        files.prepared(1, "B", save("d1", "r"), from_here, false, 0),
        Prepared::No(save_kept_out_said("https://x.test"))
    );
    assert!(files.saves_waiting().is_empty());
}

#[test]
fn a_save_goes_nowhere_a_chat_may_not_write_nor_out_of_its_folder() {
    let files = Files::default();
    files.heard("B", &[done("d1", Some("https://x.test"))]);
    let _ = files.prepared(1, "B", save("d1", "r"), from_here, false, 0);
    files
        .saves_answered(1, &["https://x.test".to_owned()], true)
        .unwrap();
    for refused in [
        ".git/hooks/pre-commit",
        ".claude/settings.json",
        ".envrc",
        "../outside",
        "/etc/x",
        "charter.toml",
    ] {
        assert!(
            matches!(
                files.prepared(1, "B", save("d1", refused), from_here, false, 0),
                Prepared::No(_)
            ),
            "{refused}"
        );
    }
}

#[test]
fn only_a_finished_download_with_an_origin_is_saved() {
    let files = Files::default();
    let going = DownloadSaid {
        state: DownloadState::Going,
        ..done("d1", Some("https://x.test"))
    };
    files.heard("B", &[going, done("d2", None)]);
    for download in ["d1", "d2", "d3"] {
        assert!(
            matches!(
                files.prepared(1, "B", save(download, "r"), from_here, false, 0),
                Prepared::No(_)
            ),
            "{download}"
        );
    }
    assert!(files.saves_waiting().is_empty(), "nothing asked");
}

#[test]
fn what_an_answer_said_of_a_download_is_kept_as_it_is_now_and_goes_with_its_browser() {
    let files = Files::default();
    let going = DownloadSaid {
        state: DownloadState::Going,
        ..done("d1", Some("https://x.test"))
    };
    assert!(files.heard("B", std::slice::from_ref(&going)));
    assert!(
        !files.heard("B", std::slice::from_ref(&going)),
        "nothing new"
    );
    assert!(files.heard("B", &[done("d1", Some("https://x.test"))]));
    assert_eq!(files.of("B"), vec![done("d1", Some("https://x.test"))]);
    files.ended("B");
    assert!(files.of("B").is_empty());
}

#[test]
fn an_act_that_names_no_folder_goes_as_it_is() {
    let files = Files::default();
    let act = Act::Click {
        target: "e1".into(),
    };
    assert_eq!(
        files.prepared(
            1,
            "B",
            act.clone(),
            || panic!("no folder is asked"),
            false,
            0
        ),
        Prepared::Go(act)
    );
}

#[test]
fn a_download_named_as_a_program_is_never_opened() {
    for program in [
        "Invoice.command",
        "setup.PKG",
        "x.app",
        "run.sh",
        "a.webloc",
    ] {
        assert!(super::opens_as_a_program(program), "{program}");
    }
    for document in ["report.pdf", "photo.jpeg", "notes", "data.csv", "sh"] {
        assert!(!super::opens_as_a_program(document), "{document}");
    }
}
