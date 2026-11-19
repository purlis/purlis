// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use super::*;

fn project() -> tempfile::TempDir {
    tempfile::tempdir().expect("a project")
}

const CHAT: &str = "01JBROWSERPOSTURE0000000001";

#[test]
fn an_answer_is_found_again_by_the_same_chat_after_a_restart() {
    let dir = project();
    keep(
        dir.path(),
        CHAT,
        Answer::Allowed { open_web: true },
        &Reach::default(),
    )
    .expect("kept");

    assert_eq!(
        answer_of(dir.path(), CHAT),
        Some(Answer::Allowed { open_web: true })
    );
    keep(dir.path(), CHAT, Answer::Off, &Reach::default()).expect("kept again");
    assert_eq!(answer_of(dir.path(), CHAT), Some(Answer::Off));
}

#[test]
fn another_chat_finds_no_answer_so_it_asks() {
    let dir = project();
    keep(
        dir.path(),
        CHAT,
        Answer::Allowed { open_web: false },
        &Reach::default(),
    )
    .expect("kept");

    assert_eq!(kept(dir.path(), "01JBROWSERPOSTURE0000000002"), None);
}

#[test]
fn an_answer_can_be_taken_away_and_the_file_goes_with_the_last() {
    let dir = project();
    keep(dir.path(), CHAT, Answer::Off, &Reach::default()).expect("kept");
    keep(dir.path(), "other-chat", Answer::Off, &Reach::default()).expect("kept");

    remove(dir.path(), CHAT).expect("removed");
    assert_eq!(kept(dir.path(), CHAT), None);
    assert_eq!(answer_of(dir.path(), "other-chat"), Some(Answer::Off));
    remove(dir.path(), "other-chat").expect("removed");
    assert!(!path(dir.path()).exists(), "nothing left to keep");
    remove(dir.path(), CHAT).expect("a second end is no failure");
}

#[test]
fn it_is_kept_in_the_state_folder_never_the_plane_and_privately() {
    let dir = project();
    keep(dir.path(), CHAT, Answer::Off, &Reach::default()).expect("kept");

    let path = path(dir.path());
    assert_eq!(
        path,
        crate::names::state(dir.path()).join("app/browser-postures.json"),
        "the clone's state folder, which is never committed or synced"
    );
    let mut written: Vec<_> = walk(dir.path());
    written.sort();
    let lock = path.with_file_name("browser-postures.json.lock");
    assert_eq!(
        written,
        vec![path.clone(), lock],
        "the file and its lock file, nothing else, the plane's files least"
    );
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(
            std::fs::metadata(&path).unwrap().permissions().mode() & 0o777,
            0o600
        );
    }
    let text = std::fs::read_to_string(&path).unwrap();
    assert!(
        text.contains(CHAT) && !text.contains("http"),
        "an identity and a word, no site: {text}"
    );
}

#[test]
fn a_file_that_does_not_read_or_names_an_unknown_word_gives_no_answer() {
    let dir = project();
    let file = path(dir.path());
    std::fs::create_dir_all(file.parent().unwrap()).unwrap();
    std::fs::write(&file, "not json").unwrap();
    assert_eq!(kept(dir.path(), CHAT), None);

    std::fs::write(
        &file,
        format!("{{\"{CHAT}\": {{\"answer\": \"everything\"}}}}"),
    )
    .unwrap();
    assert_eq!(kept(dir.path(), CHAT), None, "an unknown word asks again");
}

#[cfg(unix)]
#[test]
fn a_link_in_the_files_place_is_never_followed() {
    let dir = project();
    let elsewhere = dir.path().join("elsewhere.json");
    std::fs::write(
        &elsewhere,
        format!("{{\"{CHAT}\": {{\"answer\": \"allowed-open-web\"}}}}"),
    )
    .unwrap();
    let file = path(dir.path());
    std::fs::create_dir_all(file.parent().unwrap()).unwrap();
    std::os::unix::fs::symlink(&elsewhere, &file).unwrap();

    assert_eq!(kept(dir.path(), CHAT), None);
}

#[test]
fn an_identity_that_is_no_identity_is_refused_and_never_found() {
    let dir = project();
    assert!(keep(dir.path(), "../x", Answer::Off, &Reach::default()).is_err());
    assert!(keep(dir.path(), "", Answer::Off, &Reach::default()).is_err());
    assert_eq!(kept(dir.path(), "../x"), None);
}

fn answer_of(root: &Path, chat: &str) -> Option<Answer> {
    kept(root, chat).map(|one| one.answer)
}

fn reach(listed: &[&str], never: &[&str], withdrawn: &[u16]) -> Reach {
    Reach {
        listed: listed.iter().map(|s| (*s).to_owned()).collect(),
        never: never.iter().map(|s| (*s).to_owned()).collect(),
        withdrawn: withdrawn.iter().copied().collect(),
        ..Reach::default()
    }
}

#[test]
fn the_reach_the_chat_had_is_kept_with_its_answer() {
    let dir = project();
    let then = reach(&["a.test:80"], &["b.test"], &[3000]);
    keep(dir.path(), CHAT, Answer::Off, &then).expect("kept");

    assert_eq!(kept(dir.path(), CHAT).map(|one| one.reach), Some(then));
}

#[test]
fn a_reach_is_narrower_only_where_it_lost_a_site_or_gained_a_refusal() {
    let then = reach(&["a.test:80", "b.test:80"], &["x.test"], &[3000]);

    assert!(!then.narrower_than(&then), "the same reach is no narrower");
    assert!(
        !reach(
            &["a.test:80", "b.test:80", "c.test:80"],
            &["x.test"],
            &[3000]
        )
        .narrower_than(&then),
        "a wider reach is no narrower"
    );
    assert!(
        reach(&["a.test:80"], &["x.test"], &[3000]).narrower_than(&then),
        "a site no longer listed"
    );
    assert!(
        reach(&["a.test:80", "b.test:80"], &["x.test", "y.test"], &[3000]).narrower_than(&then),
        "a site now never reached"
    );
    assert!(
        reach(&["a.test:80", "b.test:80"], &["x.test"], &[3000, 4000]).narrower_than(&then),
        "a port now withdrawn"
    );
}

#[test]
fn a_reach_that_lost_the_open_web_or_an_answered_site_is_narrower() {
    let then = Reach {
        open_web: true,
        answered: ["a.test:443".to_owned()].into(),
        ..Reach::default()
    };

    assert!(!then.narrower_than(&then));
    assert!(
        Reach {
            answered: then.answered.clone(),
            ..Reach::default()
        }
        .narrower_than(&then),
        "the open web is no longer reached"
    );
    assert!(
        Reach {
            open_web: true,
            ..Reach::default()
        }
        .narrower_than(&then),
        "a site the person allowed is no longer"
    );
    assert!(
        !then.narrower_than(&Reach::default()),
        "gaining either is no narrowing"
    );
}

#[test]
fn answers_of_chats_that_no_longer_exist_are_pruned() {
    let dir = project();
    for chat in ["alive-1", "gone-1", "gone-2", "alive-2"] {
        keep(dir.path(), chat, Answer::Off, &Reach::default()).expect("kept");
    }

    let pruned = prune(dir.path(), |chat| chat.starts_with("alive")).expect("pruned");

    assert_eq!(pruned, 2);
    assert!(
        answer_of(dir.path(), "alive-1").is_some() && answer_of(dir.path(), "alive-2").is_some()
    );
    assert_eq!(answer_of(dir.path(), "gone-1"), None);
    assert_eq!(answer_of(dir.path(), "gone-2"), None);
    assert_eq!(
        prune(dir.path(), |_| true).expect("pruned"),
        0,
        "nothing more to prune"
    );
}

#[test]
fn pruning_with_nothing_kept_writes_nothing() {
    let dir = project();

    assert_eq!(prune(dir.path(), |_| false).expect("pruned"), 0);
    assert!(walk(dir.path()).is_empty(), "no file, no lock file");
}

#[test]
fn writers_at_once_lose_none_of_each_others_answers() {
    let dir = project();
    let root = dir.path().to_path_buf();
    let chats: Vec<String> = (0..24).map(|n| format!("chat-{n}")).collect();
    std::thread::scope(|scope| {
        for chat in &chats {
            let root = &root;
            scope.spawn(move || {
                keep(
                    root,
                    chat,
                    Answer::Allowed { open_web: false },
                    &Reach::default(),
                )
                .expect("kept");
            });
        }
    });

    for chat in &chats {
        assert!(
            answer_of(&root, chat).is_some(),
            "{chat}'s answer was dropped by another writer"
        );
    }
    let debris: Vec<_> = walk(&root)
        .into_iter()
        .filter(|file| {
            crate::rewrite::temp_of(&file.file_name().unwrap().to_string_lossy()).is_some()
        })
        .collect();
    assert!(debris.is_empty(), "a temp file was left: {debris:?}");
}

#[test]
fn a_writer_waits_for_the_lock_file_and_never_writes_without_it() {
    let dir = project();
    keep(dir.path(), CHAT, Answer::Off, &Reach::default()).expect("kept");
    let before = std::fs::read(path(dir.path())).unwrap();
    let other = crate::filelock::beside(&path(dir.path()), Duration::from_secs(1), "the test")
        .expect("another process holds the lock file");

    let refused = keep_within(
        dir.path(),
        "other-chat",
        Answer::Off,
        &Reach::default(),
        Duration::from_millis(60),
    );

    assert_eq!(refused.unwrap_err().kind(), io::ErrorKind::TimedOut);
    assert_eq!(
        std::fs::read(path(dir.path())).unwrap(),
        before,
        "nothing was written"
    );
    drop(other);
    keep(dir.path(), "other-chat", Answer::Off, &Reach::default()).expect("kept once it is free");
}

fn walk(root: &Path) -> Vec<PathBuf> {
    let mut found = Vec::new();
    let mut todo = vec![root.to_path_buf()];
    while let Some(dir) = todo.pop() {
        for entry in std::fs::read_dir(&dir).unwrap().flatten() {
            if entry.file_type().unwrap().is_dir() {
                todo.push(entry.path());
            } else {
                found.push(entry.path());
            }
        }
    }
    found
}
