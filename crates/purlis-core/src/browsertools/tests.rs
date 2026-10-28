// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use purlis_session_protocol::browsing::{Act, Browsed, DragTo, PageSaid, Typed};
use serde_json::{Map, Value, json};

use super::{Asked, TOOLS, asked, browser_reach, said};
use crate::hookwire::Browsing;

fn args(value: Value) -> Map<String, Value> {
    match value {
        Value::Object(map) => map,
        _ => Map::new(),
    }
}

fn act(name: &str, value: Value) -> Act {
    match asked(name, &args(value)).expect("a call the tools take") {
        Asked {
            asked: Browsing::Act(act),
            ..
        } => act,
        other => panic!("{other:?}"),
    }
}

#[test]
fn every_tool_the_spec_names_is_offered_each_with_a_schema() {
    let names: Vec<&str> = TOOLS.iter().map(|tool| tool.name).collect();
    assert_eq!(
        names,
        [
            "browser_open",
            "browser_navigate",
            "browser_snapshot",
            "browser_find",
            "browser_click",
            "browser_double_click",
            "browser_hover",
            "browser_type",
            "browser_press",
            "browser_select",
            "browser_drag",
            "browser_scroll",
            "browser_back",
            "browser_wait_for_text",
            "browser_pages",
            "browser_switch_page",
            "browser_screenshot",
            "browser_new",
            "browser_close",
            "browser_login",
            "browser_downloads",
            "browser_save_download",
            "browser_upload",
        ]
    );
    for tool in TOOLS {
        let schema = (tool.schema)();
        assert_eq!(schema["type"], "object", "{}", tool.name);
        assert_eq!(schema["additionalProperties"], false, "{}", tool.name);
        assert!(!tool.description.is_empty(), "{}", tool.name);
    }
}

#[test]
fn each_tool_asks_the_act_of_the_same_name() {
    assert_eq!(
        act(
            "browser_open",
            json!({ "address": "https://docs.example.com" })
        ),
        Act::Open {
            address: "https://docs.example.com".into()
        }
    );
    assert_eq!(
        act("browser_snapshot", json!({ "within": "e4", "from": 301 })),
        Act::Snapshot {
            page: None,
            within: Some("e4".into()),
            from: Some(301),
            lines: None,
        }
    );
    assert_eq!(
        act("browser_double_click", json!({ "target": "e9" })),
        Act::DoubleClick {
            target: "e9".into()
        }
    );
    assert_eq!(
        act(
            "browser_type",
            json!({ "fields": [{ "into": "e2", "text": "hi", "replace": true }, { "text": "!" }] })
        ),
        Act::Type {
            page: None,
            fields: vec![
                Typed {
                    into: Some("e2".into()),
                    text: "hi".into(),
                    replace: true,
                },
                Typed {
                    into: None,
                    text: "!".into(),
                    replace: false,
                },
            ],
        }
    );
    assert_eq!(
        act("browser_drag", json!({ "from": "e1", "x": 40, "y": -8 })),
        Act::Drag {
            from: "e1".into(),
            to: DragTo::Offset { x: 40, y: -8 },
        }
    );
    assert_eq!(
        act("browser_drag", json!({ "from": "e1", "to": "e2" })),
        Act::Drag {
            from: "e1".into(),
            to: DragTo::Element("e2".into()),
        }
    );
    assert_eq!(
        act(
            "browser_wait_for_text",
            json!({ "text": "Saved", "seconds": 3 })
        ),
        Act::WaitForText {
            page: None,
            text: "Saved".into(),
            gone: false,
            within_ms: 3000,
        }
    );
}

#[test]
fn a_call_names_its_browser_and_its_id_or_neither() {
    let named = asked(
        "browser_click",
        &args(json!({ "target": "e1", "browser": "b2", "call_id": "retry-1" })),
    )
    .unwrap();
    assert_eq!(named.browser.as_deref(), Some("b2"));
    assert_eq!(named.call_id.as_deref(), Some("retry-1"));

    let plain = asked("browser_new", &args(json!({}))).unwrap();
    assert_eq!(
        (plain.browser, plain.call_id, plain.asked),
        (None, None, Browsing::New)
    );
}

#[test]
fn an_act_on_a_page_may_ask_for_what_changed_and_a_read_may_not() {
    // #2137.
    let click = asked(
        "browser_click",
        &args(json!({ "target": "e1", "changes": true })),
    )
    .unwrap();
    assert!(click.changes);
    assert!(
        !asked("browser_click", &args(json!({ "target": "e1" })))
            .unwrap()
            .changes
    );
    let refused = asked("browser_snapshot", &args(json!({ "changes": true }))).unwrap_err();
    assert!(refused.contains("changes"), "{refused}");
    for tool in TOOLS {
        let offers = (tool.schema)()["properties"].get("changes").is_some();
        let acts_on_a_page = !tool.read_only
            && ![
                "browser_open",
                "browser_switch_page",
                "browser_new",
                "browser_close",
                "browser_login",
            ]
            .contains(&tool.name);
        assert_eq!(offers, acts_on_a_page, "{}", tool.name);
    }
    assert_eq!(super::changes_call_id("c-1"), "c-1:changes");
}

#[test]
fn a_call_missing_what_it_needs_is_refused_in_words_that_say_what_to_give() {
    let refused = asked("browser_click", &args(json!({}))).unwrap_err();
    assert!(refused.contains("target"), "{refused}");

    let refused = asked("browser_drag", &args(json!({ "from": "e1" }))).unwrap_err();
    assert!(
        refused.contains("`to`") && refused.contains("`x`"),
        "{refused}"
    );

    assert!(asked("browser_teleport", &args(json!({}))).is_err());
}

#[test]
fn an_answer_is_said_as_lines_the_model_reads_page_first() {
    let page = PageSaid {
        page: "p1".into(),
        address: "https://docs.example.com/".into(),
        title: "Docs".into(),
        current: true,
    };
    let browsed = Browsed {
        page: Some(page),
        settled: false,
        waiting_for: vec!["a request still open".into()],
        opened: vec![PageSaid {
            page: "p2".into(),
            address: "https://docs.example.com/new".into(),
            title: "New".into(),
            current: true,
        }],
        text: Some("- heading \"Docs\" [e1]".into()),
        note: Some("the page did not load".into()),
        repeated: true,
        ..Browsed::default()
    };

    assert_eq!(
        said("b1", &browsed),
        "Browser b1, page p1: \"Docs\" at https://docs.example.com/\n\
         Not settled: still waiting for a request still open.\n\
         Opened page p2: \"New\" at https://docs.example.com/new (now the current page)\n\
         Note: the page did not load\n\
         This answers a retry of the same call id: nothing was done again.\n\
         \n\
         - heading \"Docs\" [e1]"
    );
}

#[test]
fn a_browser_is_given_what_its_chat_lists_at_each_layer_and_never_its_floor() {
    use crate::sandbox::reach::{By, Reach};
    use purlis_session_protocol::session::{Layer, Listed};

    let chat = Reach::anything().and([
        ("docs.example.com".to_owned(), By::Open),
        ("127.0.0.1:5173".to_owned(), By::You),
    ]);

    let given = browser_reach(&chat, true);

    assert_eq!(
        given.listed,
        [
            Listed {
                host: "docs.example.com".into(),
                layer: Layer::Open,
            },
            Listed {
                host: "127.0.0.1:5173".into(),
                layer: Layer::You,
            },
        ]
    );
    assert!(given.open_web);
    // A browser's reach has no floor to carry: what it is given is what is listed.
}

#[test]
fn a_port_withdrawn_from_the_chat_is_withdrawn_from_the_browser_it_is_given() {
    use crate::sandbox::reach::{By, Reach};

    let chat = Reach::anything().and([("127.0.0.1:5173".to_owned(), By::You)]);
    chat.withdraw(5173);

    let given = browser_reach(&chat, false);

    assert_eq!(given.withdrawn, [5173]);
}

#[test]
fn ask_operator_pointed_at_a_browser_asks_the_app_to_hand_that_browser_to_the_person() {
    // #2015: the existing ask tool, pointed at one of the chat's browsers by its name.
    let asked = super::needs_you(&args(json!({
        "question": "  The page wants a code sent to your phone  ",
        "browser": "b1",
    })))
    .expect("a question about a browser");
    assert_eq!(
        asked,
        Asked {
            browser: Some("b1".into()),
            call_id: None,
            asked: Browsing::NeedsYou {
                question: "The page wants a code sent to your phone".into(),
            },
            changes: false,
        }
    );
    assert!(super::points_at_a_browser(&args(
        json!({ "question": "q", "browser": "b1" })
    )));
    assert!(!super::points_at_a_browser(&args(
        json!({ "question": "q" })
    )));
    assert!(!super::points_at_a_browser(&args(
        json!({ "question": "q", "browser": null })
    )));
}

#[test]
fn a_browser_question_needs_words_and_a_browser_by_name() {
    let no_words = super::needs_you(&args(json!({ "question": " ", "browser": "b1" })));
    assert!(no_words.unwrap_err().contains("words"));
    let no_name = super::needs_you(&args(json!({ "question": "Solve it", "browser": " " })));
    assert!(no_name.unwrap_err().contains("b1"));
    let not_a_name = super::needs_you(&args(json!({ "question": "Solve it", "browser": 1 })));
    assert!(not_a_name.is_err());
}

#[test]
fn an_upload_names_files_of_the_chat_s_folder_and_never_the_folder_itself() {
    assert_eq!(
        act(
            "browser_upload",
            json!({"target":"e4","files":["docs/a.pdf","b.txt"],"from":"/elsewhere"})
        ),
        Act::Upload {
            target: "e4".into(),
            files: vec!["docs/a.pdf".into(), "b.txt".into()],
            from: None,
        },
        "the folder is the window's to name, never the chat's"
    );
    assert!(asked("browser_upload", &args(json!({"target":"e4","files":[]}))).is_err());
}

#[test]
fn saving_a_download_names_it_and_a_path_in_the_chat_s_folder() {
    assert_eq!(
        act(
            "browser_save_download",
            json!({"download":"d2","to":"downloads/r.pdf","into":"/elsewhere"})
        ),
        Act::SaveDownload {
            download: "d2".into(),
            to: "downloads/r.pdf".into(),
            into: None,
        }
    );
    assert_eq!(act("browser_downloads", json!({})), Act::Downloads);
}

#[test]
fn a_download_is_said_by_its_name_size_and_origin_and_never_where_it_is_held() {
    use purlis_session_protocol::browsing::{DownloadSaid, DownloadState};
    let browsed = Browsed {
        settled: true,
        downloads: vec![
            DownloadSaid {
                download: "d1".into(),
                name: "report.pdf".into(),
                origin: Some("https://files.example.com".into()),
                bytes: 2048,
                total: Some(2048),
                state: DownloadState::Done,
                file: "GUID-1".into(),
            },
            DownloadSaid {
                download: "d2".into(),
                name: "big.iso".into(),
                origin: None,
                bytes: 1024,
                total: Some(4096),
                state: DownloadState::Going,
                file: "GUID-2".into(),
            },
        ],
        ..Browsed::default()
    };
    let text = said("b1", &browsed);
    assert!(
        text.contains(
            "Download d1: \"report.pdf\", 2.0 KB from https://files.example.com, done: \
             browser_save_download brings it into your folder"
        ),
        "{text}"
    );
    assert!(
        text.contains("Download d2: \"big.iso\", 1.0 KB of 4.0 KB, downloading"),
        "{text}"
    );
    assert!(!text.contains("GUID"), "{text}");
}
