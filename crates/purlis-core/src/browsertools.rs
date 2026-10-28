// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A chat's browser tools** (#2013, spec #2008): the typed tools a chat's agent drives a real
//! browser with, the same on every harness, because they are served by purlis's own MCP server
//! ([`crate::chattools`]) that every harness is handed.
//!
//! A snapshot of a page as a tree of elements with references (`e12`), actions on those
//! references, and a screenshot. No tool runs script: text the agent writes reaches a page only
//! as typed text, or as text to find or wait for.
//!
//! **Nothing is decided here.** A tool's arguments become one [`BrowseAsk`] ([`asked`]), which
//! the server hands to the app that started the chat over the chat's hook socket, on the framed
//! stream; the app decides whether the chat may have a browser (the one Ask that sets its
//! posture), and the session host's browser carries the act out and enforces what it may reach.
//! What comes back is said in lines the model reads ([`said`]), and a screenshot is attached as
//! an image.
//!
//! [`BrowseAsk`]: crate::hookwire::BrowseAsk

pub use purlis_session_protocol::browsing::{
    Act, Browsed, DownloadSaid, DownloadState, DragTo, PageSaid, Typed,
};
use serde_json::{Map, Value, json};

use purlis_session_protocol::browsing::BrowserReach;

use crate::chattools::Tool;
use crate::hookwire::Browsing;
use crate::sandbox::reach::{Reach, layer_of};

/// The prefix every browser tool's name starts with.
pub const PREFIX: &str = "browser_";

/// The longest a `browser_wait_for_text` waits, in seconds, when the call does not say.
const WAIT_SECONDS: u64 = 10;

/// What a tool call asks, before the app hears it: which browser, the call's own id where the
/// agent gave one, the act, and whether its answer is to say what changed on the page (#2137).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Asked {
    pub browser: Option<String>,
    pub call_id: Option<String>,
    pub asked: Browsing,
    /// Whether the act's answer is followed by [`Act::Changes`] of its page: the lines new or
    /// gone since the page was last read.
    pub changes: bool,
}

/// The tools whose answer may say what changed on the page (#2137): every one that acts on a
/// page already open.
const SAY_CHANGES: [&str; 10] = [
    "browser_navigate",
    "browser_click",
    "browser_double_click",
    "browser_hover",
    "browser_type",
    "browser_press",
    "browser_select",
    "browser_drag",
    "browser_scroll",
    "browser_back",
];

/// The call id of the changes that follow the act of `call_id`: its own, so a retry of the act
/// answers its changes again too.
pub fn changes_call_id(call_id: &str) -> String {
    format!("{call_id}:changes")
}

/// A call id of its own for a call the agent gave none, so no two calls share one.
pub fn fresh_call_id() -> String {
    format!("mcp-{}", uuid::Uuid::new_v4())
}

/// **The browser tools a harness runs without its own prompt** (#2245, ruled 2026-10-16,
/// amending ADR 0064): every one of [`TOOLS`], by name. The consent for them is the person's
/// answer to the chat's browser Ask, which the app holds in front of every call, as the
/// dispatch grant is for [`crate::chattools::DISPATCH_TOOLS`]; each harness's adapter turns
/// these names into its own rule where it has one.
pub fn run_on_the_browser_ask() -> impl Iterator<Item = &'static str> {
    TOOLS.iter().map(|tool| tool.name)
}

/// Whether `name` is one of the browser tools.
pub fn is_one(name: &str) -> bool {
    TOOLS.iter().any(|tool| tool.name == name)
}

/// **What a call of tool `name` with `args` asks**, or why it is refused, in words that name
/// what to give.
pub fn asked(name: &str, args: &Map<String, Value>) -> Result<Asked, String> {
    let browser = optional(args, "browser")?;
    let call_id = optional(args, "call_id")?;
    let page = || optional(args, "page");
    let act = match name {
        "browser_open" => Act::Open {
            address: required(args, "address")?,
        },
        "browser_navigate" => Act::Navigate {
            page: page()?,
            address: required(args, "address")?,
        },
        "browser_snapshot" => Act::Snapshot {
            page: page()?,
            within: optional(args, "within")?,
            from: number(args, "from")?,
            lines: number(args, "lines")?,
        },
        "browser_find" => {
            let (text, role) = (optional(args, "text")?, optional(args, "role")?);
            if text.is_none() && role.is_none() {
                return Err("give `text`, `role` or both to find elements by".into());
            }
            Act::Find {
                page: page()?,
                text,
                role,
                within: optional(args, "within")?,
            }
        }
        "browser_click" => Act::Click {
            target: required(args, "target")?,
        },
        "browser_double_click" => Act::DoubleClick {
            target: required(args, "target")?,
        },
        "browser_hover" => Act::Hover {
            target: required(args, "target")?,
        },
        "browser_type" => Act::Type {
            page: page()?,
            fields: fields(args)?,
        },
        "browser_press" => Act::Press {
            page: page()?,
            keys: strings(args, "keys")?,
        },
        "browser_select" => Act::Select {
            target: required(args, "target")?,
            options: strings(args, "options")?,
        },
        "browser_drag" => Act::Drag {
            from: required(args, "from")?,
            to: match (
                optional(args, "to")?,
                offset(args, "x")?,
                offset(args, "y")?,
            ) {
                (Some(to), None, None) => DragTo::Element(to),
                (None, x, y) if x.is_some() || y.is_some() => DragTo::Offset {
                    x: x.unwrap_or(0),
                    y: y.unwrap_or(0),
                },
                _ => {
                    return Err(
                        "say where the drag ends: `to`, an element, or `x` and `y`, a distance \
                         in pixels, not both"
                            .into(),
                    );
                }
            },
        },
        "browser_scroll" => Act::Scroll {
            page: page()?,
            at: optional(args, "at")?,
            x: offset(args, "x")?.unwrap_or(0),
            y: offset(args, "y")?.unwrap_or(0),
        },
        "browser_back" => Act::Back { page: page()? },
        "browser_wait_for_text" => Act::WaitForText {
            page: page()?,
            text: required(args, "text")?,
            gone: flag(args, "gone")?,
            within_ms: u64::from(number(args, "seconds")?.unwrap_or(WAIT_SECONDS as u32))
                .saturating_mul(1000),
        },
        "browser_pages" => Act::Pages,
        "browser_switch_page" => Act::SwitchPage {
            page: required(args, "page")?,
        },
        "browser_screenshot" => Act::Screenshot { page: page()? },
        "browser_downloads" => Act::Downloads,
        "browser_upload" => Act::Upload {
            target: required(args, "target")?,
            files: strings(args, "files")?,
            from: None,
        },
        "browser_save_download" => Act::SaveDownload {
            download: required(args, "download")?,
            to: required(args, "to")?,
            into: None,
        },
        "browser_new" => {
            return Ok(Asked {
                browser: None,
                call_id,
                asked: Browsing::New,
                changes: false,
            });
        }
        "browser_close" => {
            return Ok(Asked {
                browser,
                call_id,
                asked: Browsing::Close,
                changes: false,
            });
        }
        "browser_login" => {
            let login = required(args, "login")?.trim().to_owned();
            if login.is_empty() {
                return Err("give `login` the name of one of your persona's browser logins".into());
            }
            return Ok(Asked {
                browser,
                call_id,
                asked: Browsing::LogIn { login },
                changes: false,
            });
        }
        other => return Err(format!("purlis has no browser tool {other:?}")),
    };
    let changes = flag(args, "changes")?;
    if changes && !SAY_CHANGES.contains(&name) {
        return Err(format!(
            "{name} takes no `changes`: only a tool that acts on a page does"
        ));
    }
    Ok(Asked {
        browser,
        call_id,
        asked: Browsing::Act(act),
        changes,
    })
}

/// **Whether an `ask_operator` call is pointed at one of the chat's browsers** (#2015): it
/// names one in `browser`. Such a call asks the app to hand that browser to the person, never
/// the harness's own prompt ([`needs_you`]).
pub fn points_at_a_browser(args: &Map<String, Value>) -> bool {
    !matches!(args.get("browser"), None | Some(Value::Null))
}

/// **What an `ask_operator` call pointed at a browser asks** (#2015), or why it is refused: the
/// browser, by the name the app gave it, and the question, trimmed. The app answers at once:
/// the person sees it under Needs you, takes the browser over and hands it back with Done.
pub fn needs_you(args: &Map<String, Value>) -> Result<Asked, String> {
    let question = crate::chattools::question(args)?;
    let browser = optional(args, "browser")?
        .map(|name| name.trim().to_owned())
        .filter(|name| !name.is_empty())
        .ok_or_else(|| {
            "give `browser` the name of the browser the person is to take over, as the browser \
             tools name it (b1)"
                .to_owned()
        })?;
    Ok(Asked {
        browser: Some(browser),
        call_id: None,
        asked: Browsing::NeedsYou { question },
        changes: false,
    })
}

/// **What a browser answered, in lines the model reads**: the browser and its page, whether it
/// settled, the pages it opened, any note, then the page's lines.
pub fn said(browser: &str, browsed: &Browsed) -> String {
    let mut lines = Vec::new();
    match &browsed.page {
        Some(page) => lines.push(format!("Browser {browser}, {}", page_line(page))),
        None => lines.push(format!("Browser {browser}, no page open")),
    }
    if !browsed.settled {
        lines.push(if browsed.waiting_for.is_empty() {
            "Not settled in the time allowed.".to_owned()
        } else {
            format!(
                "Not settled: still waiting for {}.",
                browsed.waiting_for.join(", ")
            )
        });
    }
    for opened in &browsed.opened {
        lines.push(format!(
            "Opened {}{}",
            page_line(opened),
            if opened.current {
                " (now the current page)"
            } else {
                ""
            }
        ));
    }
    if !browsed.pages.is_empty() {
        lines.push("Pages:".to_owned());
        for page in &browsed.pages {
            lines.push(format!(
                "- {}{}",
                page_line(page),
                if page.current { " (current)" } else { "" }
            ));
        }
    }
    if let Some(seen) = browsed.seen {
        lines.push(if seen {
            "The text was seen.".to_owned()
        } else {
            "The text was not seen in the time allowed.".to_owned()
        });
    }
    if let Some(note) = &browsed.note {
        lines.push(format!("Note: {note}"));
    }
    for download in &browsed.downloads {
        lines.push(download_line(download));
    }
    if browsed.repeated {
        lines.push("This answers a retry of the same call id: nothing was done again.".to_owned());
    }
    let mut out = lines.join("\n");
    if let Some(text) = &browsed.text {
        out.push_str("\n\n");
        out.push_str(text);
    }
    out
}

/// `reach`'s listing as a browser's start carries it, with the open web or not (#2013): the
/// window's side of `crate::browsers::reach_of`. The floor is never carried: a browser reaches only what is
/// listed (D-2011-10).
pub fn browser_reach(reach: &Reach, open_web: bool) -> BrowserReach {
    let written = reach.listed_only().written();
    BrowserReach {
        listed: written
            .listed
            .into_iter()
            .map(|(host, by)| purlis_session_protocol::session::Listed {
                host,
                layer: layer_of(by),
            })
            .collect(),
        never: written.never,
        withdrawn: written.withdrawn,
        open_web,
    }
}

/// **A download, as the agent reads it** (#2020): its name in the browser, the page's name for
/// it, its size and where it came from. Never where it is held.
pub fn download_line(download: &DownloadSaid) -> String {
    let size = match (download.state, download.total) {
        (DownloadState::Going, Some(total)) => format!(
            "{} of {}",
            crate::browserfiles::bytes_said(download.bytes),
            crate::browserfiles::bytes_said(total)
        ),
        _ => crate::browserfiles::bytes_said(download.bytes),
    };
    let state = match download.state {
        DownloadState::Going => "downloading",
        DownloadState::Done => "done: browser_save_download brings it into your folder",
        DownloadState::Cancelled => "cancelled",
        DownloadState::TooLarge => "cancelled by purlis: larger than a download may be",
        DownloadState::TooMany => {
            "cancelled by purlis: this browser has made as many downloads as it may"
        }
    };
    let from = download
        .origin
        .as_deref()
        .map(|origin| format!(" from {origin}"))
        .unwrap_or_default();
    format!(
        "Download {}: {:?}, {size}{from}, {state}",
        download.download, download.name
    )
}

fn page_line(page: &PageSaid) -> String {
    format!("page {}: {:?} at {}", page.page, page.title, page.address)
}

fn optional(args: &Map<String, Value>, key: &str) -> Result<Option<String>, String> {
    match args.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(text)) => Ok(Some(text.clone())),
        Some(_) => Err(format!("`{key}` is a string")),
    }
}

fn required(args: &Map<String, Value>, key: &str) -> Result<String, String> {
    optional(args, key)?.ok_or_else(|| format!("give `{key}`"))
}

fn number(args: &Map<String, Value>, key: &str) -> Result<Option<u32>, String> {
    match args.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(value) => value
            .as_u64()
            .and_then(|n| u32::try_from(n).ok())
            .map(Some)
            .ok_or_else(|| format!("`{key}` is a whole number of at least 0")),
    }
}

fn offset(args: &Map<String, Value>, key: &str) -> Result<Option<i32>, String> {
    match args.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(value) => value
            .as_f64()
            .filter(|n| n.is_finite() && n.abs() <= 100_000.0)
            .map(|n| Some(n.round() as i32))
            .ok_or_else(|| format!("`{key}` is a number of pixels")),
    }
}

fn flag(args: &Map<String, Value>, key: &str) -> Result<bool, String> {
    match args.get(key) {
        None | Some(Value::Null) => Ok(false),
        Some(Value::Bool(on)) => Ok(*on),
        Some(_) => Err(format!("`{key}` is true or false")),
    }
}

fn strings(args: &Map<String, Value>, key: &str) -> Result<Vec<String>, String> {
    let list = args
        .get(key)
        .and_then(Value::as_array)
        .filter(|list| !list.is_empty())
        .ok_or_else(|| format!("give `{key}`, a list of at least one"))?;
    list.iter()
        .map(|one| {
            one.as_str()
                .map(str::to_owned)
                .ok_or_else(|| format!("each of `{key}` is a string"))
        })
        .collect()
}

fn fields(args: &Map<String, Value>) -> Result<Vec<Typed>, String> {
    let list = args
        .get("fields")
        .and_then(Value::as_array)
        .filter(|list| !list.is_empty())
        .ok_or_else(|| "give `fields`, a list of at least one field to type into".to_owned())?;
    list.iter()
        .map(|field| {
            let field = field
                .as_object()
                .ok_or_else(|| "each field is an object with `text`".to_owned())?;
            Ok(Typed {
                into: optional(field, "into")?,
                text: required(field, "text")?,
                replace: flag(field, "replace")?,
            })
        })
        .collect()
}

// ---- the tools, as the MCP server offers them ----

/// What every tool takes besides its own: which browser, and the call's id.
fn common() -> Map<String, Value> {
    let mut properties = Map::new();
    properties.insert(
        "browser".into(),
        json!({
            "type": "string",
            "description": "Which of this chat's browsers, by the name a tool answered with \
                            (b1, b2). Leave out for the one used last.",
        }),
    );
    properties.insert(
        "call_id".into(),
        json!({
            "type": "string",
            "description": "This call's own id. Give the same id when you retry a call whose \
                            answer was lost: it answers again and does nothing twice.",
        }),
    );
    properties
}

fn schema(own: Value, required: &[&str]) -> Value {
    let mut properties = common();
    if let Value::Object(own) = own {
        properties.extend(own);
    }
    json!({
        "type": "object",
        "properties": properties,
        "required": required,
        "additionalProperties": false,
    })
}

fn page() -> Value {
    json!({
        "type": "string",
        "description": "The page, as browser_pages lists it (p2). Leave out for the current page.",
    })
}

fn element(what: &str) -> Value {
    json!({ "type": "string", "description": what })
}

fn s_open() -> Value {
    schema(
        json!({ "address": { "type": "string", "description": "An http or https address." } }),
        &["address"],
    )
}

fn s_navigate() -> Value {
    schema(
        json!({
            "address": { "type": "string", "description": "An http or https address." },
            "page": page(),
        }),
        &["address"],
    )
}

fn s_snapshot() -> Value {
    schema(
        json!({
            "page": page(),
            "within": element("Only the part of the page inside this element (e12)."),
            "from": { "type": "integer", "minimum": 1, "description": "The line to read from: 1 for the start, or the line a snapshot's last line or a find names." },
            "lines": { "type": "integer", "minimum": 1, "description": "How many lines at most." },
        }),
        &[],
    )
}

fn s_find() -> Value {
    schema(
        json!({
            "page": page(),
            "text": { "type": "string", "description": "Text the element's name or value holds." },
            "role": { "type": "string", "description": "The element's role: button, link, textbox, …" },
            "within": element("Only inside this element."),
        }),
        &[],
    )
}

fn s_target() -> Value {
    schema(
        json!({ "target": element("The element, as a snapshot or a find named it (e12).") }),
        &["target"],
    )
}

fn s_type() -> Value {
    schema(
        json!({
            "page": page(),
            "fields": {
                "type": "array",
                "minItems": 1,
                "description": "Each field to type into, in turn.",
                "items": {
                    "type": "object",
                    "properties": {
                        "into": element("The field (e12). Leave out to type into whatever has the focus."),
                        "text": { "type": "string" },
                        "replace": { "type": "boolean", "description": "Replace what the field holds rather than add to it." },
                    },
                    "required": ["text"],
                    "additionalProperties": false,
                },
            },
        }),
        &["fields"],
    )
}

fn s_press() -> Value {
    schema(
        json!({
            "page": page(),
            "keys": { "type": "array", "minItems": 1, "items": { "type": "string" }, "description": "Keys or chords, pressed in turn: Enter, Tab, Shift+Tab, Control+a." },
        }),
        &["keys"],
    )
}

fn s_select() -> Value {
    schema(
        json!({
            "target": element("The drop-down (e12)."),
            "options": { "type": "array", "minItems": 1, "items": { "type": "string" }, "description": "The options to choose, by value or by label." },
        }),
        &["target", "options"],
    )
}

fn s_drag() -> Value {
    schema(
        json!({
            "from": element("The element to drag (e12)."),
            "to": element("The element to drop it on. Or give x and y instead."),
            "x": { "type": "number", "description": "Pixels to the right of where it starts (negative: left)." },
            "y": { "type": "number", "description": "Pixels down from where it starts (negative: up)." },
        }),
        &["from"],
    )
}

fn s_scroll() -> Value {
    schema(
        json!({
            "page": page(),
            "at": element("Scroll over this element; with no distance, bring it into view."),
            "x": { "type": "number", "description": "Pixels to scroll right." },
            "y": { "type": "number", "description": "Pixels to scroll down." },
        }),
        &[],
    )
}

fn s_page_only() -> Value {
    schema(json!({ "page": page() }), &[])
}

fn s_wait() -> Value {
    schema(
        json!({
            "page": page(),
            "text": { "type": "string", "description": "The text to wait for." },
            "gone": { "type": "boolean", "description": "Wait for the text to go instead." },
            "seconds": { "type": "integer", "minimum": 0, "description": "How long to wait at most (10 when left out; at most 60)." },
        }),
        &["text"],
    )
}

fn s_switch() -> Value {
    schema(
        json!({ "page": { "type": "string", "description": "The page to make current, as browser_pages lists it (p2)." } }),
        &["page"],
    )
}

fn s_upload() -> Value {
    schema(
        json!({
            "target": element("The page's file input (e12)."),
            "files": {
                "type": "array",
                "minItems": 1,
                "items": { "type": "string" },
                "description": "Each file to upload, as a path relative to your working folder                                 (reports/summary.pdf). Only files in it that you may read; no                                 link out of it is followed.",
            },
        }),
        &["target", "files"],
    )
}

fn s_save_download() -> Value {
    schema(
        json!({
            "download": { "type": "string", "description": "The download, as an answer named it (d1)." },
            "to": {
                "type": "string",
                "description": "Where in your working folder to save it, as a path relative to                                 it (downloads/report.pdf). Its folder must be there, and nothing                                 there already.",
            },
        }),
        &["download", "to"],
    )
}

fn s_none() -> Value {
    schema(json!({}), &[])
}

fn s_login() -> Value {
    schema(
        json!({
            "login": {
                "type": "string",
                "description": "The browser login's name, as your persona declares it in the \
                                project's settings (shop-login).",
            }
        }),
        &["login"],
    )
}

/// `schema` with `changes`, for a tool in [`SAY_CHANGES`].
fn saying_changes(mut schema: Value) -> Value {
    schema["properties"]["changes"] = json!({
        "type": "boolean",
        "description": "Also answer what changed on the page: the lines of its snapshot new \
                        or gone since it was last read, new ones with their references. \
                        Saves a whole snapshot after the step.",
    });
    schema
}

fn c_navigate() -> Value {
    saying_changes(s_navigate())
}

fn c_target() -> Value {
    saying_changes(s_target())
}

fn c_type() -> Value {
    saying_changes(s_type())
}

fn c_press() -> Value {
    saying_changes(s_press())
}

fn c_select() -> Value {
    saying_changes(s_select())
}

fn c_drag() -> Value {
    saying_changes(s_drag())
}

fn c_scroll() -> Value {
    saying_changes(s_scroll())
}

fn c_page_only() -> Value {
    saying_changes(s_page_only())
}

/// **The browser tools**, in the order the spec names them.
pub const TOOLS: [Tool; 23] = [
    Tool {
        name: "browser_open",
        description: "Open a new page at an http or https address in this chat's browser; it \
                      becomes the current page. The first browser tool a chat uses asks the \
                      person what its browser may reach.",
        schema: s_open,
        read_only: false,
    },
    Tool {
        name: "browser_navigate",
        description: "Send a page to an http or https address.",
        schema: c_navigate,
        read_only: false,
    },
    Tool {
        name: "browser_snapshot",
        description: "The page as a tree of elements, each with a reference (e12) the other \
                      tools act on. Long pages come a part at a time: the last line says where \
                      the next part starts. An element the page, or a box or frame it is in, \
                      lays out beyond what it shows is marked (out of view): scroll to it, or \
                      scroll over the box it is in.",
        schema: s_snapshot,
        read_only: true,
    },
    Tool {
        name: "browser_find",
        description: "Find elements by text or role: their references, and the snapshot line \
                      to read from.",
        schema: s_find,
        read_only: true,
    },
    Tool {
        name: "browser_click",
        description: "Click an element. Answers once the page has settled, or says what it \
                      still waits for.",
        schema: c_target,
        read_only: false,
    },
    Tool {
        name: "browser_double_click",
        description: "Double-click an element.",
        schema: c_target,
        read_only: false,
    },
    Tool {
        name: "browser_hover",
        description: "Move the pointer over an element.",
        schema: c_target,
        read_only: false,
    },
    Tool {
        name: "browser_type",
        description: "Type text into one or more fields, in turn, or into whatever has the \
                      focus.",
        schema: c_type,
        read_only: false,
    },
    Tool {
        name: "browser_press",
        description: "Press keys or chords, in turn.",
        schema: c_press,
        read_only: false,
    },
    Tool {
        name: "browser_select",
        description: "Choose options in a drop-down.",
        schema: c_select,
        read_only: false,
    },
    Tool {
        name: "browser_drag",
        description: "Drag an element onto another, or by a distance in pixels.",
        schema: c_drag,
        read_only: false,
    },
    Tool {
        name: "browser_scroll",
        description: "Scroll the page, or over an element, by a distance; or bring an element \
                      into view.",
        schema: c_scroll,
        read_only: false,
    },
    Tool {
        name: "browser_back",
        description: "Go back to the page's previous address.",
        schema: c_page_only,
        read_only: false,
    },
    Tool {
        name: "browser_wait_for_text",
        description: "Wait for text to show on the page, or to go.",
        schema: s_wait,
        read_only: true,
    },
    Tool {
        name: "browser_pages",
        description: "The browser's pages, and which is current.",
        schema: s_none,
        read_only: true,
    },
    Tool {
        name: "browser_switch_page",
        description: "Make another page of the browser the current one.",
        schema: s_switch,
        read_only: false,
    },
    Tool {
        name: "browser_screenshot",
        description: "A picture of what the page shows now. Fields the person typed into are \
                      blanked.",
        schema: s_page_only,
        read_only: true,
    },
    Tool {
        name: "browser_new",
        description: "Open another browser for this chat, apart from its others (its own \
                      cookies and storage), and use it from now. Answers its name.",
        schema: s_none,
        read_only: false,
    },
    Tool {
        name: "browser_close",
        description: "End one of this chat's browsers, and everything it held.",
        schema: s_none,
        read_only: false,
    },
    Tool {
        name: "browser_login",
        description: "Log this chat's browser in with one of your persona's browser logins, by \
                      its name. purlis goes to the login's page, fills it from the vault and \
                      submits it: the login never reaches you. Once logged in, the browser \
                      reaches the login's site alone; use browser_new for any other site.",
        schema: s_login,
        read_only: false,
    },
    Tool {
        name: "browser_downloads",
        description: "The browser's downloads: each one's name (d1), the page's name for it,                       its size and where it came from. A download is held where no chat                       reaches it until you save it with browser_save_download.",
        schema: s_none,
        read_only: true,
    },
    Tool {
        name: "browser_save_download",
        description: "Save a finished download into your working folder, as a new file. The                       first from a site asks the person, once; until they allow it, nothing is                       saved.",
        schema: s_save_download,
        read_only: false,
    },
    Tool {
        name: "browser_upload",
        description: "Put files from your working folder into the page's file input, as if                       chosen in its file dialog. Only files in your working folder that you may                       read, at most 20 and 100 MB.",
        schema: s_upload,
        read_only: false,
    },
];

#[cfg(test)]
mod tests;
