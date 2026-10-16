// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! A page as the agent reads it: the engine's accessibility tree of every frame, made into one
//! tree of lines, each element with its reference.
//!
//! What the engine answers (`Accessibility.getFullAXTree`, one frame at a time) is reduced to
//! what a reader needs: nodes the engine itself ignores and wrappers with no name or role of
//! their own are passed through to their children; text that repeats its element's name is
//! dropped; an element whose only content is one text takes that text as its name. Each frame's
//! tree is put under the element that holds it, named by the frame's name or else its title.
//! Shadow content is in the engine's tree like any other, so it is here too.
//!
//! The tree is read in pages, from a line, and scoped to one element; a find by text or role
//! answers references with the line to read from, so nothing past a page's cut is out of reach.
//!
//! **Blanked elements.** An element in the excluded set (the field purlis filled, and every
//! field the person typed into) shows its role and reference and nothing else: no name, no
//! value, nothing inside it. Its value is also taken out of every other line, so a page that
//! echoes a field elsewhere does not show it there, and neither a find nor a wait sees it.

use std::collections::{HashMap, HashSet};
use std::fmt::Write as _;

use serde_json::Value;

/// One element of a page: its frame, and the engine's id for its node in that frame's document.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) struct NodeKey {
    pub frame: String,
    pub backend: i64,
}

/// One frame's accessibility tree, and where the frame sits.
#[derive(Debug, Clone)]
pub(crate) struct FrameAx {
    pub id: String,
    /// The frame's own name (its element's `name`), empty when it has none.
    pub name: String,
    pub url: String,
    pub parent: Option<String>,
    /// The element in the parent frame that holds this frame.
    pub owner: Option<i64>,
    /// The `nodes` of `Accessibility.getFullAXTree`.
    pub nodes: Value,
}

/// Which part of a snapshot to read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct View {
    /// Only this element and what is inside it.
    pub within: Option<NodeKey>,
    /// The first line to read, from 1.
    pub from: usize,
    /// How many lines at most.
    pub lines: usize,
}

/// What a find looks for: text in an element's name or value, a role, or both.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub(crate) struct Query {
    pub text: Option<String>,
    pub role: Option<String>,
}

/// An element that is not on the page now.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct NotOnPage;

/// The longest name or value a line shows, in characters.
const LONGEST_TEXT: usize = 200;
/// The shortest value of a blanked element that is also taken out of other lines: shorter ones
/// would take ordinary words out of the page.
const SHORTEST_SCRUBBED: usize = 3;
/// How deep a tree is followed; a page nested deeper shows its content no deeper than this.
const DEEPEST: usize = 256;
/// How many matches a find lists.
const MOST_FOUND: usize = 50;

/// Roles that carry nothing for a reader when they have no name: their children stand in their
/// place.
const PASS_THROUGH: &[&str] = &[
    "generic",
    "none",
    "presentation",
    "GenericContainer",
    "LabelText",
    "Section",
];
/// Roles that are never shown, nor anything inside them.
const NEVER_SHOWN: &[&str] = &["InlineTextBox", "LineBreak", "ListMarker"];

#[derive(Debug, Clone)]
struct Line {
    depth: usize,
    key: Option<NodeKey>,
    role: String,
    name: String,
    value: Option<String>,
    /// The state words after the value, such as `level=1` or `focused`.
    states: Vec<String>,
    /// A frame's address.
    address: Option<String>,
    blanked: bool,
    /// The name of the frame the element is in, when it is not the page's own.
    frame: Option<String>,
    /// The index just past this line's last descendant.
    end: usize,
}

/// A page's tree, built once and read in pages.
#[derive(Debug, Clone)]
pub(crate) struct Snapshot {
    title: String,
    lines: Vec<Line>,
    /// What the blanked elements held when this was built, to keep out of every later answer.
    blanked_text: Vec<String>,
    /// Every element the engine says has the focus.
    focused: Vec<NodeKey>,
    /// Every element whose name or value held a text kept out: where the page shows it.
    echoes: Vec<NodeKey>,
}

/// Builds the tree of `frames` (the page's own frame first), blanking every element in
/// `excluded`.
pub(crate) fn build(frames: &[FrameAx], excluded: &HashSet<NodeKey>, scrub: &[String]) -> Snapshot {
    let Some(main) = frames.first() else {
        return Snapshot {
            title: String::new(),
            lines: Vec::new(),
            blanked_text: Vec::new(),
            focused: Vec::new(),
            echoes: Vec::new(),
        };
    };
    let mut builder = Builder {
        frames,
        excluded,
        used: HashSet::from([0]),
        lines: Vec::new(),
        blanked_text: Vec::new(),
        focused: Vec::new(),
    };
    let tree = FrameNodes::new(&main.nodes);
    let mut title = tree
        .root()
        .map(|root| text_of(root.get("name")))
        .unwrap_or_default();
    builder.frame(0, &tree, 0, None);
    // A frame whose holder is not in its parent's tree is listed at the end, so none is lost.
    for at in 0..frames.len() {
        if !builder.used.contains(&at) {
            builder.used.insert(at);
            builder.frame_line(at, None, String::new(), 0, None);
        }
    }
    let mut lines = builder.lines;
    let blanked_text: Vec<String> = builder
        .blanked_text
        .into_iter()
        .filter(|text| text.chars().count() >= SHORTEST_SCRUBBED)
        .collect();
    let all: Vec<&str> = blanked_text
        .iter()
        .map(String::as_str)
        .chain(scrub.iter().map(String::as_str))
        .collect();
    title = cut(&scrubbed(&title, &all));
    let mut echoes = Vec::new();
    for line in &mut lines {
        if let Some(key) = &line.key
            && (holds_any(&line.name, &all)
                || line
                    .value
                    .as_deref()
                    .is_some_and(|value| holds_any(value, &all)))
        {
            echoes.push(key.clone());
        }
        line.name = cut(&scrubbed(&line.name, &all));
        if let Some(value) = &mut line.value {
            *value = cut(&scrubbed(value, &all));
        }
        if let Some(address) = &mut line.address {
            *address = scrubbed_address(address, &all);
        }
        if let Some(frame) = &mut line.frame {
            // A frame's label is its name, or its address when it has none.
            *frame = cut(&scrubbed_address(frame, &all));
        }
    }
    Snapshot {
        title,
        lines,
        blanked_text,
        focused: builder.focused,
        echoes,
    }
}

/// `address` with every one of `blanked` taken out of it as a reader of the address would read
/// it: percent-decoded, and in its query a `+` read as a space. An address that held none is
/// given back as it was written.
pub(crate) fn scrubbed_address(address: &str, blanked: &[&str]) -> String {
    let decoded = decoded_address(address);
    let out = scrubbed(&decoded, blanked);
    if out == decoded {
        scrubbed(address, blanked)
    } else {
        out
    }
}

/// `address` percent-decoded (as UTF-8, any byte that is not read as `?`), with `+` read as a
/// space after its `?`.
fn decoded_address(address: &str) -> String {
    let query_at = address.find('?').unwrap_or(address.len());
    let bytes = address.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut at = 0;
    while at < bytes.len() {
        let byte = bytes[at];
        let hex = |b: u8| char::from(b).to_digit(16);
        match byte {
            b'%' if at + 2 < bytes.len() => match (hex(bytes[at + 1]), hex(bytes[at + 2])) {
                (Some(high), Some(low)) => {
                    out.push(u8::try_from(high * 16 + low).unwrap_or(b'?'));
                    at += 3;
                    continue;
                }
                _ => out.push(byte),
            },
            b'+' if at > query_at => out.push(b' '),
            _ => out.push(byte),
        }
        at += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// `text` with every one of `blanked` (at least [`SHORTEST_SCRUBBED`] characters) taken out,
/// ignoring case: the longest first, so a text kept out is taken out whole, never in part by a
/// shorter one it begins or ends with.
pub(crate) fn scrubbed(text: &str, blanked: &[&str]) -> String {
    let mut needles: Vec<&str> = blanked
        .iter()
        .copied()
        .filter(|needle| needle.chars().count() >= SHORTEST_SCRUBBED)
        .collect();
    needles.sort_by_key(|needle| std::cmp::Reverse(needle.chars().count()));
    let mut out = text.to_owned();
    for needle in needles {
        out = replace_ignoring_case(&out, needle, "(blanked)");
    }
    out
}

/// Whether `text` holds any of `blanked` (at least [`SHORTEST_SCRUBBED`] characters), ignoring
/// case.
fn holds_any(text: &str, blanked: &[&str]) -> bool {
    scrubbed(text, blanked) != text
}

fn replace_ignoring_case(text: &str, needle: &str, with: &str) -> String {
    let wanted: Vec<char> = needle.chars().flat_map(char::to_lowercase).collect();
    let mut out = String::new();
    let mut rest = text;
    'outer: while !rest.is_empty() {
        let mut matched = Vec::new();
        for (at, c) in rest.char_indices() {
            matched.extend(c.to_lowercase());
            if matched.len() >= wanted.len() {
                if matched == wanted {
                    out.push_str(with);
                    rest = &rest[at + c.len_utf8()..];
                    continue 'outer;
                }
                break;
            }
            if !wanted.starts_with(&matched) {
                break;
            }
        }
        let first = rest.chars().next().expect("not empty");
        out.push(first);
        rest = &rest[first.len_utf8()..];
    }
    out
}

/// `text` cut at [`LONGEST_TEXT`] characters.
fn cut(text: &str) -> String {
    if text.chars().count() > LONGEST_TEXT {
        let mut cut: String = text.chars().take(LONGEST_TEXT).collect();
        cut.push('…');
        cut
    } else {
        text.to_owned()
    }
}

impl Snapshot {
    /// The page's title, as its accessibility tree names it.
    pub(crate) fn title(&self) -> &str {
        &self.title
    }

    /// What the blanked elements held (each at least [`SHORTEST_SCRUBBED`] characters).
    pub(crate) fn blanked_text(&self) -> &[String] {
        &self.blanked_text
    }

    /// Every element whose name or value held a text kept out: where the page shows it.
    pub(crate) fn echoes(&self) -> &[NodeKey] {
        &self.echoes
    }

    /// Every element the engine says has the focus: one, when it is plain which.
    pub(crate) fn focused(&self) -> &[NodeKey] {
        &self.focused
    }

    /// The lines `view` asks for, each element with the reference `refer` deals it, and a last
    /// line that says where the next page starts when more is left.
    pub(crate) fn page(
        &self,
        view: &View,
        refer: &mut dyn FnMut(&NodeKey) -> String,
    ) -> Result<String, NotOnPage> {
        let (start, end) = self.scope(view.within.as_ref())?;
        let base = self.lines.get(start).map_or(0, |l| l.depth);
        let total = end - start;
        let from = view.from.max(1);
        let mut out = String::new();
        if from > total {
            let _ = writeln!(out, "(line {from} is past the end: this has {total} lines)");
            return Ok(out);
        }
        let first = start + from - 1;
        let last = (first + view.lines.max(1)).min(end);
        for line in &self.lines[first..last] {
            out.push_str(&render(line, line.depth - base, refer));
            out.push('\n');
        }
        let shown_to = last - start;
        if shown_to < total {
            let _ = writeln!(
                out,
                "(lines {from} to {shown_to} of {total}; the next page starts at line {})",
                shown_to + 1
            );
        } else if from > 1 {
            let _ = writeln!(
                out,
                "(lines {from} to {shown_to} of {total}; the end of the page)"
            );
        }
        Ok(out)
    }

    /// Every element `query` matches inside `within` (the whole page when `None`), one a line,
    /// each with the line to read from and the frame it is in.
    pub(crate) fn find(
        &self,
        query: &Query,
        within: Option<&NodeKey>,
        refer: &mut dyn FnMut(&NodeKey) -> String,
    ) -> Result<String, NotOnPage> {
        let (start, end) = self.scope(within)?;
        let text = query.text.as_deref().map(str::to_lowercase);
        let role = query.role.as_deref().map(str::to_lowercase);
        let matches: Vec<(usize, &Line)> = self.lines[start..end]
            .iter()
            .enumerate()
            .filter(|(_, line)| {
                role.as_ref()
                    .is_none_or(|role| line.role.to_lowercase() == *role)
                    && text.as_ref().is_none_or(|text| shows(line, text))
            })
            .collect();
        let mut out = String::new();
        if matches.is_empty() {
            out.push_str("Nothing on the page matches.\n");
            return Ok(out);
        }
        for (at, line) in matches.iter().take(MOST_FOUND) {
            out.push_str(&render(line, 0, refer));
            let _ = write!(out, " (line {}", at + 1);
            if let Some(frame) = &line.frame {
                let _ = write!(out, ", in frame \"{}\"", quoted(frame));
            }
            out.push_str(")\n");
        }
        if matches.len() > MOST_FOUND {
            let _ = writeln!(
                out,
                "(and {} more: narrow the find, or scope it to an element)",
                matches.len() - MOST_FOUND
            );
        }
        Ok(out)
    }

    /// Whether any element's name or value shows `text`, ignoring case. A blanked element's
    /// value is in none.
    pub(crate) fn shows_text(&self, text: &str) -> bool {
        let text = text.to_lowercase();
        self.lines.iter().any(|line| shows(line, &text))
    }

    fn scope(&self, within: Option<&NodeKey>) -> Result<(usize, usize), NotOnPage> {
        match within {
            None => Ok((0, self.lines.len())),
            Some(key) => self
                .lines
                .iter()
                .position(|line| line.key.as_ref() == Some(key))
                .map(|at| (at, self.lines[at].end))
                .ok_or(NotOnPage),
        }
    }
}

fn shows(line: &Line, lowered: &str) -> bool {
    line.name.to_lowercase().contains(lowered)
        || line
            .value
            .as_ref()
            .is_some_and(|v| v.to_lowercase().contains(lowered))
}

fn render(line: &Line, depth: usize, refer: &mut dyn FnMut(&NodeKey) -> String) -> String {
    let mut out = String::new();
    for _ in 0..depth {
        out.push_str("  ");
    }
    out.push_str("- ");
    out.push_str(&line.role);
    if !line.name.is_empty() {
        let _ = write!(out, " \"{}\"", quoted(&line.name));
    }
    if let Some(key) = &line.key {
        let _ = write!(out, " [{}]", refer(key));
    }
    if let Some(value) = &line.value {
        let _ = write!(out, " value=\"{}\"", quoted(value));
    }
    for state in &line.states {
        out.push(' ');
        out.push_str(state);
    }
    if line.blanked {
        out.push_str(" (blanked)");
    }
    if let Some(address) = &line.address {
        out.push(' ');
        out.push_str(address);
    }
    out
}

fn quoted(text: &str) -> String {
    text.replace('\\', "\\\\").replace('"', "\\\"")
}

/// One frame's nodes, by id.
struct FrameNodes<'a> {
    by_id: HashMap<&'a str, &'a Value>,
    root: Option<&'a Value>,
}

impl<'a> FrameNodes<'a> {
    fn new(nodes: &'a Value) -> Self {
        let list: &[Value] = nodes.as_array().map_or(&[], Vec::as_slice);
        let mut by_id = HashMap::new();
        let mut children = HashSet::new();
        for node in list {
            if let Some(id) = node.get("nodeId").and_then(Value::as_str) {
                by_id.insert(id, node);
            }
            for child in child_ids(node) {
                children.insert(child);
            }
        }
        let roots: Vec<&Value> = list
            .iter()
            .filter(|n| {
                n.get("nodeId")
                    .and_then(Value::as_str)
                    .is_some_and(|id| !children.contains(id))
            })
            .collect();
        let root = roots
            .iter()
            .copied()
            .find(|n| role_of(n) == "RootWebArea")
            .or_else(|| roots.first().copied());
        Self { by_id, root }
    }

    fn root(&self) -> Option<&'a Value> {
        self.root
    }

    fn get(&self, id: &str) -> Option<&'a Value> {
        self.by_id.get(id).copied()
    }
}

fn child_ids(node: &Value) -> impl Iterator<Item = &str> {
    node.get("childIds")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
}

fn role_of(node: &Value) -> &str {
    node.get("role")
        .and_then(|r| r.get("value"))
        .and_then(Value::as_str)
        .unwrap_or_default()
}

/// A name or value as one line: whitespace collapsed.
fn text_of(field: Option<&Value>) -> String {
    match field.and_then(|f| f.get("value")) {
        Some(Value::String(s)) => one_line(s),
        Some(Value::Number(n)) => n.to_string(),
        Some(Value::Bool(b)) => b.to_string(),
        _ => String::new(),
    }
}

/// `raw` with its whitespace collapsed (cut only once blanked text is out of it).
fn one_line(raw: &str) -> String {
    raw.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn property<'a>(node: &'a Value, name: &str) -> Option<&'a Value> {
    node.get("properties")
        .and_then(Value::as_array)?
        .iter()
        .find(|p| p.get("name").and_then(Value::as_str) == Some(name))
        .and_then(|p| p.get("value"))
        .and_then(|v| v.get("value"))
}

fn is_true(value: Option<&Value>) -> bool {
    matches!(value, Some(Value::Bool(true))) || value.and_then(Value::as_str) == Some("true")
}

/// The state words of an element, in a fixed order.
fn states(node: &Value) -> Vec<String> {
    let mut out = Vec::new();
    for (name, label) in [("valuemin", "min"), ("valuemax", "max")] {
        if let Some(Value::Number(n)) = property(node, name) {
            out.push(format!("{label}={n}"));
        }
    }
    if let Some(Value::String(url)) = property(node, "url")
        && role_of(node) != "RootWebArea"
    {
        out.push(format!("url={url}"));
    }
    if let Some(Value::Number(level)) = property(node, "level") {
        out.push(format!("level={level}"));
    }
    match property(node, "checked").and_then(Value::as_str) {
        Some("true") => out.push("checked".into()),
        Some("false") => out.push("unchecked".into()),
        Some("mixed") => out.push("mixed".into()),
        _ => {}
    }
    match property(node, "pressed").and_then(Value::as_str) {
        Some("true") => out.push("pressed".into()),
        Some("mixed") => out.push("mixed".into()),
        _ => {}
    }
    match property(node, "expanded") {
        Some(Value::Bool(true)) => out.push("expanded".into()),
        Some(Value::Bool(false)) => out.push("collapsed".into()),
        _ => {}
    }
    for (name, word) in [
        ("selected", "selected"),
        ("disabled", "disabled"),
        ("focused", "focused"),
        ("required", "required"),
        ("readonly", "readonly"),
    ] {
        if is_true(property(node, name)) {
            out.push(word.into());
        }
    }
    out
}

struct Builder<'f> {
    frames: &'f [FrameAx],
    excluded: &'f HashSet<NodeKey>,
    /// The frames already placed.
    used: HashSet<usize>,
    lines: Vec<Line>,
    /// Text of blanked elements, to take out of every other line.
    blanked_text: Vec<String>,
    focused: Vec<NodeKey>,
}

impl Builder<'_> {
    /// Places frame `at`'s tree from its root, at `depth`.
    fn frame(&mut self, at: usize, tree: &FrameNodes<'_>, depth: usize, label: Option<&str>) {
        if let Some(root) = tree.root() {
            let mut seen = HashSet::new();
            let label = label.map(str::to_owned);
            self.node(
                at,
                tree,
                root,
                depth,
                ("", None),
                label.as_deref(),
                &mut seen,
            );
        }
    }

    /// A frame's own line, then its tree inside it.
    fn frame_line(
        &mut self,
        at: usize,
        key: Option<NodeKey>,
        title: String,
        depth: usize,
        outer: Option<&str>,
    ) {
        let frames = self.frames;
        let frame = &frames[at];
        let name = if frame.name.is_empty() {
            title
        } else {
            one_line(&frame.name)
        };
        let index = self.lines.len();
        self.lines.push(Line {
            depth,
            key,
            role: "frame".into(),
            name: name.clone(),
            value: None,
            states: Vec::new(),
            address: Some(frame.url.clone()),
            blanked: false,
            frame: outer.map(str::to_owned),
            end: index + 1,
        });
        let label = if name.is_empty() {
            frame.url.clone()
        } else {
            name
        };
        let tree = FrameNodes::new(&frame.nodes);
        self.frame(at, &tree, depth + 1, Some(&label));
        self.lines[index].end = self.lines.len();
    }

    /// Places `node` and what is inside it. `parent` is the name and value of the nearest
    /// shown element, so text repeating it is dropped.
    #[allow(clippy::too_many_arguments)]
    fn node(
        &mut self,
        at: usize,
        tree: &FrameNodes<'_>,
        node: &Value,
        depth: usize,
        parent: (&str, Option<&str>),
        label: Option<&str>,
        seen: &mut HashSet<String>,
    ) {
        let Some(id) = node.get("nodeId").and_then(Value::as_str) else {
            return;
        };
        if !seen.insert(id.to_owned()) || depth > DEEPEST {
            return;
        }
        let role = role_of(node);
        if NEVER_SHOWN.contains(&role) {
            return;
        }
        let children: Vec<&Value> = child_ids(node).filter_map(|c| tree.get(c)).collect();
        let ignored = node.get("ignored").and_then(Value::as_bool) == Some(true);
        let frames = self.frames;
        let frame_id = &frames[at].id;
        let key = node
            .get("backendDOMNodeId")
            .and_then(Value::as_i64)
            .map(|backend| NodeKey {
                frame: frame_id.clone(),
                backend,
            });
        let name = text_of(node.get("name"));
        let value = Some(text_of(node.get("value"))).filter(|v| !v.is_empty());
        if let Some(key) = &key
            && is_true(property(node, "focused"))
        {
            self.focused.push(key.clone());
        }
        // A blanked element is blanked whatever the engine says of it, ignored or a wrapper.
        if let Some(key) = &key
            && self.excluded.contains(key)
        {
            self.blank(tree, node, value);
            let index = self.lines.len();
            self.lines.push(Line {
                depth,
                key: Some(key.clone()),
                role: shown_role(role).to_owned(),
                name: String::new(),
                value: None,
                states: Vec::new(),
                address: None,
                blanked: true,
                frame: label.map(str::to_owned),
                end: index + 1,
            });
            return;
        }
        if ignored || role == "RootWebArea" || (name.is_empty() && PASS_THROUGH.contains(&role)) {
            for child in children {
                self.node(at, tree, child, depth, parent, label, seen);
            }
            return;
        }
        if role == "StaticText" {
            if name.is_empty() || name == parent.0 || Some(name.as_str()) == parent.1 {
                return;
            }
            let index = self.lines.len();
            self.lines.push(Line {
                depth,
                key,
                role: "text".into(),
                name,
                value: None,
                states: Vec::new(),
                address: None,
                blanked: false,
                frame: label.map(str::to_owned),
                end: index + 1,
            });
            return;
        }
        if role == "Iframe" {
            let owner = key.as_ref().map(|k| k.backend);
            let held = (0..frames.len()).find(|&f| {
                !self.used.contains(&f)
                    && frames[f].parent.as_deref() == Some(frame_id.as_str())
                    && frames[f].owner.is_some()
                    && frames[f].owner == owner
            });
            if let Some(held) = held {
                self.used.insert(held);
                self.frame_line(held, key, name, depth, label);
                return;
            }
        }
        let index = self.lines.len();
        self.lines.push(Line {
            depth,
            key,
            role: shown_role(role).to_owned(),
            name: name.clone(),
            value: value.clone(),
            states: states(node),
            address: None,
            blanked: false,
            frame: label.map(str::to_owned),
            end: index + 1,
        });
        for child in children {
            self.node(
                at,
                tree,
                child,
                depth + 1,
                (&name, value.as_deref()),
                label,
                seen,
            );
        }
        let end = self.lines.len();
        // An element with no name whose only content is one text takes it as its name, and
        // its reference is the text's: acting on it acts where its content is shown.
        if name.is_empty() && end == index + 2 && self.lines[index + 1].role == "text" {
            let text = self.lines.remove(index + 1);
            self.lines[index].name = text.name;
            if text.key.is_some() {
                self.lines[index].key = text.key;
            }
        }
        self.lines[index].end = self.lines.len();
    }

    /// Keeps the text a blanked element holds, to take it out of every other line.
    fn blank(&mut self, tree: &FrameNodes<'_>, node: &Value, value: Option<String>) {
        self.blanked_text.extend(value);
        let mut stack: Vec<&Value> = child_ids(node).filter_map(|c| tree.get(c)).collect();
        let mut seen = HashSet::new();
        while let Some(inside) = stack.pop() {
            let Some(id) = inside.get("nodeId").and_then(Value::as_str) else {
                continue;
            };
            if !seen.insert(id.to_owned()) {
                continue;
            }
            for field in ["name", "value"] {
                let text = text_of(inside.get(field));
                if !text.is_empty() {
                    self.blanked_text.push(text);
                }
            }
            stack.extend(child_ids(inside).filter_map(|c| tree.get(c)));
        }
    }
}

/// The role a reader is shown.
fn shown_role(role: &str) -> &str {
    match role {
        "StaticText" => "text",
        "Iframe" => "frame",
        "" => "element",
        other => other,
    }
}

#[cfg(test)]
mod tests;
