// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use std::path::PathBuf;

use purlis_session_protocol::browsing::{Act, DownloadSaid, DownloadState};

use super::{ChatFolder, Files, NOT_ON_A_RUNNER, Prepared, save_not_allowed_said};

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

fn refused(prepared: Prepared) -> String {
    match prepared {
        Prepared::No(why) => why,
        other => panic!("refused, not {other:?}"),
    }
}

#[test]
fn an_upload_names_the_chat_s_folder_as_the_window_knows_it_never_the_one_it_was_sent() {
    let files = Files::default();
    assert_eq!(
        files.prepared(1, "B", upload(&["docs/a.pdf"]), from_here, false),
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
        refused(files.prepared(1, "B", upload(&[outside]), from_here, false));
    }
    let why = refused(files.prepared(
        1,
        "B",
        upload(&[".PURLIS/vaults/forge.json"]),
        || Ok(at("/p")),
        false,
    ));
    assert!(why.contains("keeps from every chat"), "{why}");
}

#[test]
fn a_runner_chat_s_browser_takes_no_files_and_a_folder_not_told_refuses_in_its_words() {
    let files = Files::default();
    assert_eq!(
        refused(files.prepared(1, "B", upload(&["a.pdf"]), from_here, true)),
        NOT_ON_A_RUNNER
    );
    assert_eq!(
        refused(files.prepared(1, "B", upload(&["a"]), || Err("why".into()), false)),
        "why"
    );
}

#[test]
fn a_download_is_never_saved_into_a_chat_s_folder_without_the_person_s_yes_for_its_origin() {
    let files = Files::default();
    files.heard("B", &[done("d1", Some("https://files.example.com"))]);
    assert_eq!(
        refused(files.prepared(1, "B", save("d1", "downloads/r.pdf"), from_here, false)),
        save_not_allowed_said("https://files.example.com")
    );
    files.allow_saves(1, "https://files.example.com");
    assert_eq!(
        files.prepared(1, "B", save("d1", "downloads/r.pdf"), from_here, false),
        Prepared::Go(Act::SaveDownload {
            download: "d1".into(),
            to: "downloads/r.pdf".into(),
            into: Some("/p/workspaces/app".into()),
        })
    );
    // A yes is for that chat alone, and goes with it.
    files.heard("C", &[done("d1", Some("https://files.example.com"))]);
    refused(files.prepared(2, "C", save("d1", "r.pdf"), from_here, false));
    files.chat_ended(1);
    refused(files.prepared(1, "B", save("d1", "downloads/r.pdf"), from_here, false));
}

#[test]
fn a_save_goes_nowhere_a_chat_may_not_write_nor_out_of_its_folder() {
    let files = Files::default();
    files.heard("B", &[done("d1", Some("https://x.test"))]);
    files.allow_saves(1, "https://x.test");
    for to in [
        ".git/hooks/pre-commit",
        ".claude/settings.json",
        ".envrc",
        "../outside",
        "/etc/x",
        "charter.toml",
        ".purlis/vaults/x.json",
    ] {
        refused(files.prepared(1, "B", save("d1", to), || Ok(at("/p")), false));
    }
}

#[test]
fn only_a_finished_download_with_an_origin_is_saved_and_one_gone_with_its_browser_is_not() {
    let files = Files::default();
    let going = DownloadSaid {
        state: DownloadState::Going,
        ..done("d1", Some("https://x.test"))
    };
    files.heard("B", &[going, done("d2", None)]);
    files.allow_saves(1, "https://x.test");
    for download in ["d1", "d2", "d3"] {
        refused(files.prepared(1, "B", save(download, "r"), from_here, false));
    }
    // The last word of a download is the one kept.
    files.heard("B", &[done("d1", Some("https://x.test"))]);
    assert!(matches!(
        files.prepared(1, "B", save("d1", "r"), from_here, false),
        Prepared::Go(_)
    ));
    files.ended("B");
    refused(files.prepared(1, "B", save("d1", "r"), from_here, false));
}

#[test]
fn an_act_that_names_no_folder_goes_as_it_is() {
    let files = Files::default();
    let act = Act::Click {
        target: "e1".into(),
    };
    assert_eq!(
        files.prepared(1, "B", act.clone(), || panic!("no folder is asked"), false),
        Prepared::Go(act)
    );
}

#[test]
fn a_yes_for_one_origin_covers_no_download_from_another() {
    let files = Files::default();
    files.heard(
        "B",
        &[
            done("d1", Some("https://files.example.com")),
            done("d2", Some("https://evil.example.net")),
            done("d3", Some("https://files.example.com:8443")),
        ],
    );
    files.allow_saves(1, "https://files.example.com");
    assert!(matches!(
        files.prepared(1, "B", save("d1", "a"), from_here, false),
        Prepared::Go(_)
    ));
    assert_eq!(
        refused(files.prepared(1, "B", save("d2", "b"), from_here, false)),
        save_not_allowed_said("https://evil.example.net")
    );
    // Another port is another origin.
    refused(files.prepared(1, "B", save("d3", "c"), from_here, false));
}

#[test]
fn a_save_or_an_upload_spelled_as_a_folding_disk_reads_a_kept_or_planted_name_is_refused() {
    let files = Files::default();
    files.heard("B", &[done("d1", Some("https://x.test"))]);
    files.allow_saves(1, "https://x.test");
    for to in [".claude/\u{17f}ettings.json", "purli\u{17f}.toml", ".ENVRC"] {
        refused(files.prepared(1, "B", save("d1", to), || Ok(at("/p")), false));
    }
    refused(files.prepared(
        1,
        "B",
        upload(&[".purlis/vault\u{17f}/forge.json"]),
        || Ok(at("/p")),
        false,
    ));
}

#[test]
fn a_chat_the_app_has_no_folder_for_gets_no_files_never_the_projects_root() {
    // P2 of the review: no record of where the chat runs is a refusal, not the whole project.
    let project = tempfile::tempdir().expect("a directory");
    assert_eq!(
        super::folder_of(None, project.path()).unwrap_err(),
        super::NO_FOLDER
    );
    let files = Files::default();
    assert_eq!(
        refused(files.prepared(
            1,
            "B",
            upload(&["a.txt"]),
            || super::folder_of(None, project.path()),
            false
        )),
        super::NO_FOLDER
    );
    // A folder outside the project is refused too.
    let elsewhere = tempfile::tempdir().expect("a directory");
    assert!(super::folder_of(Some(elsewhere.path().to_path_buf()), project.path()).is_err());
}
