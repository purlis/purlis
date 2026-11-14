// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use std::cell::RefCell;

use super::*;

fn url(text: &str) -> Candidate {
    Candidate {
        target: None,
        kind: Kind::Url,
        text: text.to_owned(),
    }
}

/// Opens `text` as the command does, with a browser that records what it was handed.
fn opened(text: &str) -> (LinkOpened, Vec<String>) {
    let handed = RefCell::new(Vec::new());
    let answered = answer(
        &Resolver::default(),
        &Pane::default(),
        &Asks::default(),
        &url(text),
        None,
        |url| {
            handed.borrow_mut().push(url.to_owned());
            Ok(())
        },
    );
    (answered, handed.into_inner())
}

#[test]
fn an_https_address_opens_in_the_browser() {
    let (answered, handed) = opened("https://github.com/purlis/purlis/pull/6148");

    assert_eq!(
        answered,
        LinkOpened::Opened {
            kind: KindHint::Url,
            target: "https://github.com/purlis/purlis/pull/6148".into()
        }
    );
    assert_eq!(handed, ["https://github.com/purlis/purlis/pull/6148"]);
}

#[test]
fn another_scheme_is_refused_and_the_browser_is_handed_nothing() {
    for text in [
        "ftp://mirror.example.org/pub",
        "file:///etc/hosts",
        "javascript:alert(1)",
    ] {
        let (answered, handed) = opened(text);
        assert!(
            matches!(&answered, LinkOpened::Refused { reason } if reason.contains("only http and https")),
            "{text}: {answered:?}"
        );
        assert!(handed.is_empty(), "{text} reached the browser");
    }
}

#[test]
fn what_opens_is_the_core_s_reading_of_the_text_never_a_target_the_pane_named() {
    // The command takes no target: only the text, which the core reads again. A pane that
    // sent words with a hidden byte, or an address the browser would read another way, opens
    // only what the core makes of them.
    let (answered, handed) = opened("https://purlis.dev\u{0}.evil.example/");
    assert!(
        matches!(answered, LinkOpened::Refused { .. }),
        "{answered:?}"
    );
    assert!(handed.is_empty());

    let (_, handed) = opened("HTTPS://PURLIS.dev");
    assert_eq!(handed, ["https://purlis.dev/"]);
}

#[test]
fn a_browser_that_will_not_open_it_is_a_refusal_in_purlis_s_words() {
    let answered = answer(
        &Resolver::default(),
        &Pane::default(),
        &Asks::default(),
        &url("https://purlis.dev/"),
        None,
        |_| Err("purlis could not open the address in your browser".into()),
    );

    assert_eq!(
        answered,
        LinkOpened::Refused {
            reason: "purlis could not open the address in your browser".into()
        }
    );
}

#[test]
fn a_hover_says_the_target_and_that_it_opens() {
    let seen = seen(
        &Resolver::default(),
        &Pane::default(),
        &url("https://purlis.dev"),
    );

    assert_eq!(
        seen,
        Some(LinkSeen {
            its_text: None,
            kind: KindHint::Url,
            target: "https://purlis.dev/".into(),
            shown: "https://purlis.dev/".into(),
            does: LinkDoes::Opens {
                into: OpensIn::Browser
            },
            chat: None,
        })
    );
}

#[test]
fn a_hover_over_another_scheme_says_why_it_won_t_open_and_so_does_one_carrying_a_password() {
    assert_eq!(
        seen(
            &Resolver::default(),
            &Pane::default(),
            &url("ftp://x.example/")
        ),
        Some(LinkSeen {
            its_text: None,
            kind: KindHint::Url,
            target: "ftp://x.example/".into(),
            shown: "ftp://x.example/".into(),
            does: LinkDoes::Refused {
                reason: "purlis opens only http and https addresses, so it will not open \
                         this ftp: one"
                    .into()
            },
            chat: None,
        })
    );
    assert!(matches!(
        seen(
            &Resolver::default(),
            &Pane::default(),
            // A made-up sign-in, spelled in parts so the commit scan does not take it for one.
            &url(&["https://", "me:", "made-up", "@purlis.dev/"].concat())
        ),
        Some(LinkSeen {
            does: LinkDoes::Refused { .. },
            ..
        })
    ));
}

#[test]
fn the_window_s_words_for_the_answers_are_these() {
    // The shape the window reads (`paneLinks.ts`).
    let json = |answer: &LinkOpened| serde_json::to_value(answer).expect("serialises");
    assert_eq!(
        json(&LinkOpened::Opened {
            kind: KindHint::Url,
            target: "https://a.example/".into()
        }),
        serde_json::json!({"answer": "opened", "kind": "url", "target": "https://a.example/"})
    );
    assert_eq!(
        json(&LinkOpened::Refused {
            reason: "no".into()
        }),
        serde_json::json!({"answer": "refused", "reason": "no"})
    );
    assert_eq!(
        serde_json::to_value(LinkDoes::Asks { reason: "r".into() }).expect("serialises"),
        serde_json::json!({"does": "asks", "reason": "r"})
    );
    assert_eq!(
        serde_json::to_value(LinkDoes::Opens {
            into: OpensIn::EditorTab
        })
        .expect("serialises"),
        serde_json::json!({"does": "opens", "into": "editor-tab"})
    );
}

/// A project holding workspace `smart-ide`, a chat working in it, and a file outside both.
struct Here {
    _held: tempfile::TempDir,
    pane: Pane,
    notes: PathBuf,
    hosts: PathBuf,
}

fn here() -> Here {
    let held = tempfile::tempdir().expect("a temporary folder");
    let top = std::fs::canonicalize(held.path()).expect("resolved");
    let ws = top.join("project/workspaces/smart-ide");
    std::fs::create_dir_all(&ws).expect("a workspace");
    let notes = ws.join("notes.md");
    std::fs::write(&notes, "one\ntwo\n").expect("written");
    let hosts = top.join("etc/hosts");
    std::fs::create_dir_all(hosts.parent().expect("a parent")).expect("folders");
    std::fs::write(&hosts, "127.0.0.1 localhost\n").expect("written");
    Here {
        pane: Pane {
            project: Some(top.join("project")),
            session: Some(3),
            folder: Some(ws),
            runs: Runs::Here,
        },
        _held: held,
        notes,
        hosts,
    }
}

fn file(text: &str) -> Candidate {
    Candidate {
        target: None,
        kind: Kind::File,
        text: text.to_owned(),
    }
}

/// Opens `text` as a file link in `pane`, with a browser that must never be handed anything.
fn opened_file(pane: &Pane, asks: &Asks, text: &str, token: Option<&str>) -> LinkOpened {
    answer(
        &Resolver::default(),
        pane,
        asks,
        &file(text),
        token,
        |handed| panic!("a file reached the system's opener: {handed}"),
    )
}

#[test]
fn a_file_in_the_workspace_is_answered_for_the_editor_tab_at_its_line_and_never_handed_on() {
    let here = here();

    assert_eq!(
        opened_file(&here.pane, &Asks::default(), "notes.md:2:1", None),
        LinkOpened::File {
            target: format!("{}:2:1", here.notes.display()),
            at: LinkFileAt::Workspace {
                workspace: "smart-ide".into(),
                path: "notes.md".into()
            },
            line: Some(2),
            column: Some(1),
        }
    );
}

#[test]
fn a_file_outside_the_workspace_asks_with_a_token_and_its_answer_opens_it_once() {
    let here = here();
    let asks = Asks::default();
    let text = format!("{}:1", here.hosts.display());

    let LinkOpened::Ask {
        reason,
        shown,
        target,
        token: _,
        about,
    } = opened_file(&here.pane, &asks, &text, None)
    else {
        panic!("expected an ask");
    };
    assert_eq!(reason, "outside smart-ide");
    assert_eq!(
        (shown.as_str(), target.as_str()),
        (text.as_str(), text.as_str())
    );
    let folder = here.pane.folder.clone().expect("a folder");
    assert_eq!(
        about,
        LinkAsk::FileOutside {
            home: Some(LinkHome::Workspace {
                name: "smart-ide".into(),
                folder: folder.display().to_string(),
            }),
        }
    );
    // The target the ask showed is no answer: only its token is.
    assert!(matches!(
        opened_file(&here.pane, &asks, &text, Some(&target)),
        LinkOpened::Ask { .. }
    ));
    assert_eq!(asks.opened(&here.pane, &here.hosts), None);

    let LinkOpened::Ask { token, .. } = opened_file(&here.pane, &asks, &text, None) else {
        panic!("expected an ask");
    };
    assert_eq!(
        opened_file(&here.pane, &asks, &text, Some(&token)),
        LinkOpened::File {
            target: text.clone(),
            at: LinkFileAt::Outside {
                path: here.hosts.display().to_string()
            },
            line: Some(1),
            column: None,
        }
    );
    assert!(asks.opened(&here.pane, &here.hosts).is_some());
    assert!(matches!(
        opened_file(&here.pane, &asks, &text, Some(&token)),
        LinkOpened::Ask { .. }
    ));
}

#[test]
fn a_hover_over_a_path_to_no_file_says_nothing() {
    let here = here();

    assert_eq!(
        seen(&Resolver::default(), &here.pane, &file("src/gone.rs:10")),
        None
    );
    assert_eq!(
        seen(&Resolver::default(), &here.pane, &file("notes.md")),
        Some(LinkSeen {
            its_text: None,
            kind: KindHint::File,
            target: here.notes.display().to_string(),
            // In the chat's workspace: said from the workspace (D2).
            shown: "notes.md".into(),
            does: LinkDoes::Opens {
                into: OpensIn::EditorTab
            },
            chat: None,
        })
    );
}

#[test]
fn a_file_outside_the_project_reads_only_in_the_pane_its_ask_was_answered_in() {
    let here = here();
    let asks = Asks::default();
    let text = here.hosts.display().to_string();
    let LinkOpened::Ask { token, .. } = opened_file(&here.pane, &asks, &text, None) else {
        panic!("expected an ask");
    };
    opened_file(&here.pane, &asks, &text, Some(&token));

    let identity = asks.opened(&here.pane, &here.hosts).expect("opened");
    assert_eq!(
        purlis_core::files::folder::read_absolute(&here.hosts, identity).map(PieceFile::from),
        Ok(PieceFile::Text {
            text: "127.0.0.1 localhost\n".into()
        })
    );
    let another_chat = Pane {
        session: Some(4),
        ..here.pane.clone()
    };
    assert_eq!(asks.opened(&another_chat, &here.hosts), None);
}

#[test]
fn a_look_that_does_not_end_in_time_answers_none_and_so_does_one_past_the_most() {
    assert_eq!(looked(Duration::from_secs(5), || 7), Some(7));
    assert_eq!(
        looked(Duration::from_millis(20), || {
            std::thread::sleep(Duration::from_millis(400));
            7
        }),
        None
    );
}

#[test]
fn the_window_s_words_for_a_file_and_an_ask_are_these() {
    let json = |answer: &LinkOpened| serde_json::to_value(answer).expect("serialises");
    assert_eq!(
        json(&LinkOpened::File {
            target: "/p/a.rs:3".into(),
            at: LinkFileAt::Branch {
                workspace: "w".into(),
                repo: "r".into(),
                piece: None,
                path: "a.rs".into()
            },
            line: Some(3),
            column: None,
        }),
        serde_json::json!({
            "answer": "file", "target": "/p/a.rs:3", "line": 3, "column": null,
            "at": {"in": "branch", "workspace": "w", "repo": "r", "piece": null, "path": "a.rs"}
        })
    );
    assert_eq!(
        json(&LinkOpened::Ask {
            reason: "outside w".into(),
            shown: "/etc/hosts".into(),
            target: "/etc/hosts".into(),
            token: "t".into(),
            about: LinkAsk::FileOutside {
                home: Some(LinkHome::Workspace {
                    name: "w".into(),
                    folder: "/p/workspaces/w".into()
                }),
            },
        }),
        serde_json::json!({
            "answer": "ask", "reason": "outside w", "shown": "/etc/hosts",
            "target": "/etc/hosts", "token": "t",
            "about": {"about": "file-outside",
                      "home": {"is": "workspace", "name": "w", "folder": "/p/workspaces/w"}}
        })
    );
}

// ---- Chat links (#6217): the window's half, over the core's chat kind. ----

use purlis_core::chatlookup::OpenChat;
use purlis_core::panelinks::ChatNumber;

fn chat(text: &str) -> Candidate {
    Candidate {
        target: None,
        kind: Kind::Chat,
        text: text.to_owned(),
    }
}

/// Chat 652's pane in a project with no records: only open chats link.
fn pane_of_652(root: &std::path::Path) -> Pane {
    Pane {
        project: Some(root.to_owned()),
        session: Some(652),
        ..Pane::default()
    }
}

fn named(session: u32, name: &str, workspace: &str) -> OpenNamed {
    OpenNamed {
        session,
        name: name.to_owned(),
        workspace: Some(workspace.to_owned()),
    }
}

/// The window's chats: 652 (this pane's, in smart-ide), 660 (in docs) and 661 (in smart-ide).
fn chats_here() -> (Resolver, ChatsHere<'static>) {
    let open = [(652, "A"), (660, "B"), (661, "C")]
        .map(|(number, id)| OpenChat {
            number,
            id: Some(format!("01K6{id}")),
        })
        .to_vec();
    (
        Resolver::default().with_kind(Box::new(ChatNumber::with_open(open))),
        ChatsHere {
            open: vec![
                named(652, "Grill #5820: chat links", "smart-ide"),
                named(660, "Docs site: the setup page", "docs"),
                named(661, "Signed commits #5607", "smart-ide"),
            ],
            card: Box::new(|_, _| None),
        },
    )
}

#[test]
fn a_chat_open_in_another_workspace_is_seen_by_number_and_title_with_that_workspace() {
    let dir = tempfile::tempdir().expect("a project");
    let (links, chats) = chats_here();

    assert_eq!(
        seen_in(&links, &pane_of_652(dir.path()), &chat("chat 660"), &chats),
        Some(LinkSeen {
            its_text: None,
            kind: KindHint::Chat,
            target: "chat 660".into(),
            shown: "#660 · Docs site: the setup page".into(),
            does: LinkDoes::Opens {
                into: OpensIn::Window
            },
            chat: Some(ChatSeen::Open {
                number: 660,
                workspace: Some("docs".into())
            }),
        })
    );
    // In the pane's own workspace, it names none.
    assert!(matches!(
        seen_in(&links, &pane_of_652(dir.path()), &chat("chat 661"), &chats),
        Some(LinkSeen {
            chat: Some(ChatSeen::Open {
                number: 661,
                workspace: None
            }),
            ..
        })
    ));
}

#[test]
fn opening_an_open_chat_shows_it_and_reaches_no_browser() {
    let dir = tempfile::tempdir().expect("a project");
    let (links, chats) = chats_here();
    let answered = answer_in(
        &links,
        &pane_of_652(dir.path()),
        &Asks::default(),
        &chat("chat 660"),
        None,
        |url| panic!("{url} reached the browser"),
        &chats,
    );

    assert_eq!(
        answered,
        LinkOpened::Chat {
            go: ChatGo::Chat { session: 660 }
        }
    );
}

#[test]
fn its_own_number_is_this_chat_and_opening_it_shows_nothing_more() {
    let dir = tempfile::tempdir().expect("a project");
    let (links, chats) = chats_here();

    assert!(matches!(
        seen_in(&links, &pane_of_652(dir.path()), &chat("chat 652"), &chats),
        Some(LinkSeen {
            chat: Some(ChatSeen::ThisChat { number: 652 }),
            ref shown,
            ..
        }) if shown == "#652 · Grill #5820: chat links"
    ));
    assert_eq!(
        answer_in(
            &links,
            &pane_of_652(dir.path()),
            &Asks::default(),
            &chat("chat 652"),
            None,
            |url| panic!("{url} reached the browser"),
            &chats,
        ),
        LinkOpened::Chat {
            go: ChatGo::ThisChat
        }
    );
}

#[test]
fn an_unknown_number_says_nothing_on_a_hover_and_opening_it_is_refused() {
    let dir = tempfile::tempdir().expect("a project");
    let (links, chats) = chats_here();

    assert_eq!(
        seen_in(&links, &pane_of_652(dir.path()), &chat("chat 9999"), &chats),
        None
    );
    assert!(matches!(
        answer_in(
            &links,
            &pane_of_652(dir.path()),
            &Asks::default(),
            &chat("chat 9999"),
            None,
            |url| panic!("{url} reached the browser"),
            &chats,
        ),
        LinkOpened::Refused { .. }
    ));
}

#[test]
fn a_pane_in_no_project_links_no_chat_and_says_nothing_on_its_hover() {
    let (links, chats) = for_project(None);
    assert_eq!(
        seen_in(&links, &Pane::default(), &chat("chat 652"), &chats),
        None
    );
}

#[test]
fn the_window_s_words_for_a_chat_link_are_these() {
    let json = |answer: &LinkOpened| serde_json::to_value(answer).expect("serialises");
    assert_eq!(
        json(&LinkOpened::Chat {
            go: ChatGo::Chat { session: 660 }
        }),
        serde_json::json!({"answer": "chat", "go": {"to": "chat", "session": 660}})
    );
    assert_eq!(
        json(&LinkOpened::Chat {
            go: ChatGo::ThisChat
        }),
        serde_json::json!({"answer": "chat", "go": {"to": "this_chat"}})
    );
    assert_eq!(
        serde_json::to_value(ChatSeen::Open {
            number: 660,
            workspace: None
        })
        .expect("serialises"),
        serde_json::json!({"is": "open", "number": 660, "workspace": null})
    );
}

/// #6357: **the link and the light editor agree on a file git ignores in a branch.** A
/// clone's ignored file is no link, its hover says why, and ⌘-click is refused with the same
/// sentence; the editor's own open refuses it too. A file the clone tracks links and opens.
/// Asked of the bounded reader, as the window's resolver asks it.
#[cfg(unix)]
#[test]
fn a_file_git_ignores_in_a_clone_or_a_piece_is_no_link_as_the_editor_refuses_it() {
    let here = here();
    let project = here.pane.project.clone().expect("a project");
    let clone = project.join("workspaces/smart-ide/repos/app");
    std::fs::create_dir_all(clone.join("src")).expect("folders");
    std::fs::write(clone.join(".gitignore"), "*.log\n").expect("written");
    std::fs::write(clone.join("src/main.rs"), "fn main() {}\n").expect("written");
    std::fs::write(clone.join("debug.log"), "secret-ish\n").expect("written");
    let git = |args: &[&str]| {
        let ran = purlis_core::forklock::output(
            std::process::Command::new("git")
                .arg("-C")
                .arg(&clone)
                .args(["-c", "user.email=t@e.invalid", "-c", "user.name=t"])
                .args(["-c", "commit.gpgsign=false"])
                .args(args),
        )
        .expect("git ran");
        assert!(ran.status.success(), "git {args:?}: {ran:?}");
    };
    git(&["init", "-q", "-b", "main"]);
    git(&["add", ".gitignore", "src/main.rs"]);
    git(&["commit", "-q", "-m", "one"]);
    let pane = Pane {
        folder: Some(clone.clone()),
        ..here.pane.clone()
    };

    assert_eq!(
        seen(&resolver(), &pane, &file("debug.log")),
        Some(LinkSeen {
            its_text: None,
            kind: KindHint::File,
            target: "debug.log".into(),
            shown: "debug.log".into(),
            does: LinkDoes::Refused {
                reason: purlis_core::panelinks::NOT_OFFERED.into()
            },
            chat: None,
        })
    );
    assert_eq!(
        answer(
            &resolver(),
            &pane,
            &Asks::default(),
            &file("debug.log"),
            None,
            |handed| panic!("a file reached the system's opener: {handed}"),
        ),
        LinkOpened::Refused {
            reason: purlis_core::panelinks::NOT_OFFERED.into()
        }
    );
    // The editor refuses it as well: the two agree.
    let branch = purlis_core::files::Branch::repo("smart-ide", "app");
    assert!(matches!(
        purlis_core::files::open(&project, branch, "debug.log"),
        Err(purlis_core::files::Refused::NotOffered(_))
    ));

    let tracked = seen(&resolver(), &pane, &file("src/main.rs:1")).expect("a link");
    assert_eq!(
        tracked.does,
        LinkDoes::Opens {
            into: OpensIn::EditorTab
        }
    );
    assert!(purlis_core::files::open(&project, branch, "src/main.rs").is_ok());

    // A piece of the clone, its own branch folder, is read the same way.
    let piece = project.join("workspaces/smart-ide/.worktrees/app/feature");
    let at = piece.to_str().expect("a path");
    git(&["worktree", "add", "-q", "-b", "feature", at]);
    std::fs::write(piece.join("debug.log"), "secret-ish\n").expect("written");
    let in_piece = Pane {
        folder: Some(piece.clone()),
        ..here.pane.clone()
    };
    assert_eq!(
        seen(&resolver(), &in_piece, &file("debug.log")).map(|seen| seen.does),
        Some(LinkDoes::Refused {
            reason: purlis_core::panelinks::NOT_OFFERED.into()
        })
    );
    let branch = purlis_core::files::Branch::piece("smart-ide", "app", "feature");
    assert!(matches!(
        purlis_core::files::open(&project, branch, "debug.log"),
        Err(purlis_core::files::Refused::NotOffered(_))
    ));
    assert_eq!(
        seen(&resolver(), &in_piece, &file("src/main.rs:1")).map(|seen| seen.does),
        Some(LinkDoes::Opens {
            into: OpensIn::EditorTab
        })
    );
    assert!(purlis_core::files::open(&project, branch, "src/main.rs").is_ok());
}

/// A link a program marked (OSC 8): its words, and the target it named.
fn marked(text: &str, target: &str) -> Candidate {
    Candidate {
        kind: Kind::Osc8,
        text: text.to_owned(),
        target: Some(target.to_owned()),
    }
}

/// Opens `candidate` as the command does, with a browser that records what it was handed.
fn opened_marked(
    candidate: &Candidate,
    asks: &Asks,
    token: Option<&str>,
) -> (LinkOpened, Vec<String>) {
    let handed = RefCell::new(Vec::new());
    let answered = answer(
        &Resolver::default(),
        &Pane::default(),
        asks,
        candidate,
        token,
        |url| {
            handed.borrow_mut().push(url.to_owned());
            Ok(())
        },
    );
    (answered, handed.into_inner())
}

#[test]
fn a_marked_link_whose_words_say_nothing_else_opens_its_target_in_the_browser() {
    let link = marked("Click here", "https://purlis.dev/docs");
    let hovered = seen(&Resolver::default(), &Pane::default(), &link).expect("a link");
    assert_eq!(
        hovered.its_text, None,
        "words that say nothing else are not said"
    );

    let (answered, handed) = opened_marked(&link, &Asks::default(), None);
    assert_eq!(
        answered,
        LinkOpened::Opened {
            kind: KindHint::Url,
            target: "https://purlis.dev/docs".into()
        }
    );
    assert_eq!(handed, ["https://purlis.dev/docs"]);
    assert_eq!(link.read_by(), Some(Kind::Url));
}

#[test]
fn a_marked_link_naming_another_host_asks_with_both_and_its_token_opens_it_once() {
    let asks = Asks::default();
    let link = marked(
        "docs.purlis.dev/setup",
        "https://purlis-setup.example.net/s?id=7",
    );

    let (answered, handed) = opened_marked(&link, &asks, None);
    let LinkOpened::Ask {
        reason,
        shown,
        target,
        token,
        about,
    } = answered
    else {
        panic!("expected an ask, got {answered:?}");
    };
    assert!(handed.is_empty(), "an ask reached the browser");
    assert_eq!(reason, "its text says docs.purlis.dev");
    assert_eq!(shown, "docs.purlis.dev/setup");
    assert_eq!(target, "https://purlis-setup.example.net/s?id=7");
    assert_eq!(
        about,
        LinkAsk::Misleading {
            kind: KindHint::Url,
            host: Some("purlis-setup.example.net".into())
        }
    );

    let (answered, handed) = opened_marked(&link, &asks, Some(&token));
    assert!(
        matches!(answered, LinkOpened::Opened { .. }),
        "{answered:?}"
    );
    assert_eq!(handed, ["https://purlis-setup.example.net/s?id=7"]);
    let (again, handed) = opened_marked(&link, &asks, Some(&token));
    assert!(matches!(again, LinkOpened::Ask { .. }), "{again:?}");
    assert!(handed.is_empty());
}

#[test]
fn a_marked_link_of_another_scheme_is_refused_and_its_hover_shows_the_target() {
    let link = marked("Click here", "javascript:alert(1)");

    let (answered, handed) = opened_marked(&link, &Asks::default(), None);
    assert!(
        matches!(&answered, LinkOpened::Refused { reason } if reason.contains("javascript:")),
        "{answered:?}"
    );
    assert!(handed.is_empty());
    assert_eq!(link.read_by(), None);
    let seen = seen(&Resolver::default(), &Pane::default(), &link).expect("it says why");
    assert_eq!(seen.kind, KindHint::Osc8);
    assert_eq!(seen.target, "javascript:alert(1)");
    assert!(matches!(seen.does, LinkDoes::Refused { .. }));
}

#[test]
fn a_marked_link_s_hover_says_it_asks_and_why() {
    let seen = seen(
        &Resolver::default(),
        &Pane::default(),
        &marked("github.com/login", "https://evil.example/login"),
    )
    .expect("a link");

    assert_eq!(seen.kind, KindHint::Url);
    assert_eq!(seen.target, "https://evil.example/login");
    assert_eq!(seen.its_text.as_deref(), Some("github.com/login"));
    assert_eq!(
        seen.does,
        LinkDoes::Asks {
            reason: "its text says github.com".into()
        }
    );
}

#[test]
fn the_window_s_words_for_a_marked_link_are_these() {
    let candidate: LinkCandidate = serde_json::from_value(serde_json::json!({
        "kind": "osc8", "text": "Click here", "target": "https://purlis.dev/"
    }))
    .expect("reads");
    assert_eq!(
        Candidate::from(candidate),
        marked("Click here", "https://purlis.dev/")
    );
    // A found link sends no target.
    let found: LinkCandidate =
        serde_json::from_value(serde_json::json!({"kind": "url", "text": "https://a.example/"}))
            .expect("reads");
    assert_eq!(found.target, None);
    assert_eq!(
        serde_json::to_value(LinkAsk::Misleading {
            kind: KindHint::Url,
            host: Some("a.example".into())
        })
        .expect("serialises"),
        serde_json::json!({"about": "misleading", "kind": "url", "host": "a.example"})
    );
}
