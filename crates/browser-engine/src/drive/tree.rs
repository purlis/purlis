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
//! Blanked boxes next to each other on the page, with nothing but short separators between
//! them (a code typed one character a box), are a group: what they hold, joined in their order
//! on the page, is taken out as one more value (#2144).

use std::collections::{HashMap, HashSet};
use std::fmt::Write as _;

use serde_json::Value;

use super::kept::Kept;

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
/// The most characters shown between two blanked boxes of one group (a dash, a space).
const LONGEST_SEPARATOR: usize = 2;
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

/// One line of a tree as a later read compares it (#2137): its element, and its words without
/// its reference.
pub(crate) type Seen = (Option<NodeKey>, String);

/// A page's tree, built once and read in pages.
#[derive(Debug, Clone)]
pub(crate) struct Snapshot {
    title: String,
    lines: Vec<Line>,
    /// What the blanked elements held when this was built, to keep out of every later answer.
    blanked_text: Kept,
    /// What has the focus: every element the engine says has it, a document only where nothing
    /// in it does, each once.
    focused: Vec<NodeKey>,
    /// Every element whose name or value held a text kept out: where the page shows it.
    echoes: Vec<NodeKey>,
    /// The groups of blanked boxes, each in its order on the page.
    groups: Vec<Group>,
    /// Every element the page lays out wholly outside what it shows (#2137): marked so.
    out_of_view: HashSet<NodeKey>,
}

/// Blanked elements next to each other on the page, with nothing between them but separators:
/// boxes that hold one value between them, such as a code typed one character a box.
#[derive(Debug, Clone)]
struct Group {
    parts: Vec<Part>,
}

#[derive(Debug, Clone)]
enum Part {
    /// A blanked element, and what it holds.
    Box(NodeKey, String),
    /// A separator between two boxes.
    Between(String),
}

impl Group {
    /// What the group holds joined: in its order on the page without its separators and with
    /// them, and in `order` (the order its boxes were typed into) where that differs. A box in
    /// `held` holds what `held` says in place of what it held when read.
    fn joined(&self, held: &HashMap<NodeKey, String>, order: &[NodeKey]) -> Vec<String> {
        let holds = |at: &NodeKey, was: &str| held.get(at).map_or(was, String::as_str).to_owned();
        let mut bare = String::new();
        let mut spaced = String::new();
        let mut boxes = Vec::new();
        for part in &self.parts {
            match part {
                Part::Box(at, was) => {
                    let text = holds(at, was);
                    bare.push_str(&text);
                    spaced.push_str(&text);
                    boxes.push((at, text));
                }
                Part::Between(text) => spaced.push_str(text),
            }
        }
        boxes.sort_by_key(|(at, _)| order.iter().position(|o| o == *at).unwrap_or(usize::MAX));
        let typed: String = boxes.into_iter().map(|(_, text)| text).collect();
        let mut out: Vec<String> = Vec::new();
        for text in [bare, spaced, typed] {
            if !text.is_empty() && !out.contains(&text) {
                out.push(text);
            }
        }
        out
    }

    fn first(&self) -> Option<&NodeKey> {
        self.parts.iter().find_map(|part| match part {
            Part::Box(key, _) => Some(key),
            Part::Between(_) => None,
        })
    }

    fn holds(&self, key: &NodeKey) -> bool {
        self.parts
            .iter()
            .any(|part| matches!(part, Part::Box(at, _) if at == key))
    }

    fn boxes(&self) -> usize {
        self.parts
            .iter()
            .filter(|part| matches!(part, Part::Box(..)))
            .count()
    }
}

/// The groups of blanked boxes among `lines`: blanked elements of one frame next to each other,
/// each holding less than [`SHORTEST_SCRUBBED`] characters (one holding more is kept out on its
/// own, and ends a group), with nothing between two of them but lines that show at most
/// [`LONGEST_SEPARATOR`] characters. A group has two boxes or more.
fn groups_of(lines: &[Line], held: &HashMap<NodeKey, String>) -> Vec<Group> {
    let mut out = Vec::new();
    let mut open: Option<Group> = None;
    let mut between: Vec<String> = Vec::new();
    let mut close = |open: &mut Option<Group>| {
        if let Some(group) = open.take()
            && group.boxes() >= 2
        {
            out.push(group);
        }
    };
    for line in lines {
        match &line.key {
            Some(key) if line.blanked => {
                let text = held.get(key).cloned().unwrap_or_default();
                let joins = open
                    .as_ref()
                    .and_then(Group::first)
                    .is_some_and(|first| first.frame == key.frame);
                if text.chars().count() >= SHORTEST_SCRUBBED {
                    close(&mut open);
                } else if joins && let Some(group) = &mut open {
                    group.parts.extend(between.drain(..).map(Part::Between));
                    group.parts.push(Part::Box(key.clone(), text));
                } else {
                    close(&mut open);
                    open = Some(Group {
                        parts: vec![Part::Box(key.clone(), text)],
                    });
                }
                between.clear();
            }
            _ => {
                let shown = format!("{}{}", line.name, line.value.as_deref().unwrap_or_default());
                let same_frame = line.key.as_ref().is_none_or(|key| {
                    open.as_ref()
                        .and_then(Group::first)
                        .is_some_and(|first| first.frame == key.frame)
                });
                let separates = open.is_some()
                    && line.address.is_none()
                    && same_frame
                    && between.iter().map(|t| t.chars().count()).sum::<usize>()
                        + shown.chars().count()
                        <= LONGEST_SEPARATOR;
                if separates {
                    if !shown.is_empty() {
                        between.push(shown);
                    }
                } else {
                    close(&mut open);
                    between.clear();
                }
            }
        }
    }
    close(&mut open);
    out
}

/// What is taken out of an answer: texts kept out, matched as they are, and the texts groups of
/// boxes hold together, matched also with any spaces or dashes between their characters.
pub(crate) struct Scrub<'k> {
    exact: Vec<&'k str>,
    loose: Vec<&'k str>,
}

impl<'k> Scrub<'k> {
    pub(crate) fn of(kept: &'k Kept) -> Self {
        Self {
            exact: kept.texts(),
            loose: kept.loose(),
        }
    }

    /// `text` with every text kept out taken out.
    pub(crate) fn text(&self, text: &str) -> String {
        loosely(&scrubbed(text, &self.exact), &self.loose)
    }

    /// [`Scrub::text`] for an address, read as its reader would read it ([`scrubbed_address_by`]).
    pub(crate) fn address(&self, address: &str) -> String {
        scrubbed_address_by(address, |text| self.text(text))
    }

    /// Whether `text` holds any text kept out.
    pub(crate) fn holds(&self, text: &str) -> bool {
        self.text(text) != text
    }
}

/// Whether `text` holds any of `loose`, ignoring case and any spaces or dashes between its
/// characters.
pub(crate) fn holds_loosely(text: &str, loose: &[&str]) -> bool {
    loosely(text, loose) != text
}

/// Characters a page may put between the characters of a code: spaces and dashes.
fn separates(c: char) -> bool {
    c.is_whitespace() || matches!(c, '-' | '\u{2010}'..='\u{2015}' | '\u{2212}')
}

/// `text` with every one of `loose` (at least [`SHORTEST_SCRUBBED`] characters once its
/// separators are left out) taken out, ignoring case and any separators between its characters.
fn loosely(text: &str, loose: &[&str]) -> String {
    let mut needles: Vec<Vec<char>> = loose
        .iter()
        .map(|needle| {
            needle
                .chars()
                .filter(|c| !separates(*c))
                .flat_map(char::to_lowercase)
                .collect::<Vec<char>>()
        })
        .filter(|needle| needle.len() >= SHORTEST_SCRUBBED)
        .collect();
    needles.sort_by_key(|needle| std::cmp::Reverse(needle.len()));
    let mut out: Vec<char> = text.chars().collect();
    for needle in needles {
        out = replace_loosely(&out, &needle);
    }
    out.into_iter().collect()
}

fn replace_loosely(text: &[char], needle: &[char]) -> Vec<char> {
    let mut out = Vec::with_capacity(text.len());
    let mut at = 0;
    while at < text.len() {
        let mut look = at;
        let mut matched = 0;
        while look < text.len() && matched < needle.len() {
            let c = text[look];
            if matched > 0 && separates(c) {
                look += 1;
                continue;
            }
            let mut lower = c.to_lowercase();
            if lower.next() == Some(needle[matched]) && lower.next().is_none() {
                matched += 1;
                look += 1;
            } else {
                break;
            }
        }
        if matched == needle.len() {
            out.extend("(blanked)".chars());
            at = look;
        } else {
            out.push(text[at]);
            at += 1;
        }
    }
    out
}

/// Builds the tree of `frames` (the page's own frame first), blanking every element in
/// `excluded`.
#[cfg(test)]
pub(crate) fn build(frames: &[FrameAx], excluded: &HashSet<NodeKey>, scrub: &[&str]) -> Snapshot {
    let mut kept = Kept::default();
    kept.run(scrub);
    build_kept(frames, excluded, &kept)
}

/// Builds the tree of `frames` (the page's own frame first), blanking every element in
/// `excluded`, with every text `scrub` keeps out taken out.
pub(crate) fn build_kept(
    frames: &[FrameAx],
    excluded: &HashSet<NodeKey>,
    scrub: &Kept,
) -> Snapshot {
    let Some(main) = frames.first() else {
        return Snapshot {
            title: String::new(),
            lines: Vec::new(),
            blanked_text: Kept::default(),
            focused: Vec::new(),
            echoes: Vec::new(),
            groups: Vec::new(),
            out_of_view: HashSet::new(),
        };
    };
    let mut builder = Builder {
        frames,
        excluded,
        used: HashSet::from([0]),
        lines: Vec::new(),
        blanked_text: Kept::default(),
        focused: Vec::new(),
        focused_documents: Vec::new(),
        held: HashMap::new(),
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
    let mut blanked_text = builder.blanked_text;
    let groups = groups_of(&lines, &builder.held);
    for group in &groups {
        if let Some(first) = group.first() {
            blanked_text.group_tree(first, &group.joined(&HashMap::new(), &[]));
        }
    }
    let mut all = scrub.clone();
    all.take_in(&blanked_text);
    let all = Scrub::of(&all);
    title = cut(&all.text(&title));
    let mut echoes = Vec::new();
    for line in &mut lines {
        if let Some(key) = &line.key
            && (all.holds(&line.name)
                || line.value.as_deref().is_some_and(|value| all.holds(value)))
        {
            echoes.push(key.clone());
        }
        line.name = cut(&all.text(&line.name));
        if let Some(value) = &mut line.value {
            *value = cut(&all.text(value));
        }
        if let Some(address) = &mut line.address {
            *address = all.address(address);
        }
        if let Some(frame) = &mut line.frame {
            // A frame's label is its name, or its address when it has none.
            *frame = cut(&all.address(frame));
        }
    }
    Snapshot {
        title,
        lines,
        blanked_text,
        // A real engine says a document has the focus while a field in it has it too (#2294):
        // the field is what has it.
        focused: if builder.focused.is_empty() {
            builder.focused_documents
        } else {
            builder.focused
        },
        echoes,
        groups,
        out_of_view: HashSet::new(),
    }
}

/// `address` with every one of `blanked` taken out of it ([`scrubbed_address_by`]).
#[cfg(test)]
pub(crate) fn scrubbed_address(address: &str, blanked: &[&str]) -> String {
    scrubbed_address_by(address, |text| scrubbed(text, blanked))
}

/// `address` with what `scrub` takes out of a text taken out of it as a reader of the address
/// would read it: percent-decoded, and in its query a `+` read as a space. An address that held
/// none is given back as it was written.
fn scrubbed_address_by(address: &str, scrub: impl Fn(&str) -> String) -> String {
    let decoded = decoded_address(address);
    let out = scrub(&decoded);
    if out == decoded { scrub(address) } else { out }
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
pub(crate) fn holds_any(text: &str, blanked: &[&str]) -> bool {
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

    /// What the blanked elements held (each at least [`SHORTEST_SCRUBBED`] characters), each
    /// with its element.
    pub(crate) fn blanked_text(&self) -> &Kept {
        &self.blanked_text
    }

    /// What each group of blanked boxes that holds one of `held`'s boxes holds, joined as
    /// [`Kept::group`] keeps it, each box in `held` holding what `held` says: each group's first
    /// box, and its joined texts. `order` is the order the boxes were typed into.
    pub(crate) fn groups_holding(
        &self,
        held: &HashMap<NodeKey, String>,
        order: &[NodeKey],
    ) -> Vec<(NodeKey, Vec<String>)> {
        self.groups
            .iter()
            .filter(|group| held.keys().any(|key| group.holds(key)))
            .filter_map(|group| Some((group.first()?.clone(), group.joined(held, order))))
            .collect()
    }

    /// Every element whose name or value held a text kept out: where the page shows it.
    pub(crate) fn echoes(&self) -> &[NodeKey] {
        &self.echoes
    }

    /// Every element the engine says has the focus: one, when it is plain which.
    pub(crate) fn focused(&self) -> &[NodeKey] {
        &self.focused
    }

    /// Marks each of `keys` as laid out wholly outside what the page shows (#2137).
    pub(crate) fn mark_out_of_view(&mut self, keys: impl IntoIterator<Item = NodeKey>) {
        self.out_of_view.extend(keys);
    }

    /// The elements of the lines `view` asks for.
    pub(crate) fn keys_in(&self, view: &View) -> Result<Vec<NodeKey>, NotOnPage> {
        let (start, end) = self.scope(view.within.as_ref())?;
        let first = start + view.from.max(1) - 1;
        let last = (first + view.lines.max(1)).min(end);
        Ok(self
            .lines
            .get(first..last)
            .unwrap_or_default()
            .iter()
            .filter_map(|line| line.key.clone())
            .collect())
    }

    /// The elements `query` matches inside `within`, as [`Snapshot::find`] lists them.
    pub(crate) fn keys_found(
        &self,
        query: &Query,
        within: Option<&NodeKey>,
    ) -> Result<Vec<NodeKey>, NotOnPage> {
        Ok(self
            .matches(query, within)?
            .into_iter()
            .take(MOST_FOUND)
            .filter_map(|(_, line)| line.key.clone())
            .collect())
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
            out.push_str(&self.render(line, line.depth - base, refer));
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
        let matches = self.matches(query, within)?;
        let mut out = String::new();
        if matches.is_empty() {
            out.push_str("Nothing on the page matches.\n");
            return Ok(out);
        }
        for (at, line) in matches.iter().take(MOST_FOUND) {
            out.push_str(&self.render(line, 0, refer));
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

    /// Every line as a later read compares it (#2137): its element and its words, without its
    /// reference or its place in view.
    pub(crate) fn seen(&self) -> Vec<Seen> {
        self.lines
            .iter()
            .map(|line| (line.key.clone(), rendered(line, 0, None)))
            .collect()
    }

    /// The lines of this tree that `before` did not have, by their index.
    fn new_since(&self, before: &[Seen]) -> (Vec<usize>, Vec<String>) {
        let mut left: HashMap<&Seen, usize> = HashMap::new();
        for seen in before {
            *left.entry(seen).or_default() += 1;
        }
        let now = self.seen();
        let mut new = Vec::new();
        for (at, seen) in now.iter().enumerate() {
            match left.get_mut(seen) {
                Some(count) if *count > 0 => *count -= 1,
                _ => new.push(at),
            }
        }
        let mut gone = Vec::new();
        for seen in before {
            if let Some(count) = left.get_mut(seen)
                && *count > 0
            {
                *count -= 1;
                gone.push(seen.1.clone());
            }
        }
        (new, gone)
    }

    /// The elements of the lines new since `before`, as [`Snapshot::changes`] lists them.
    pub(crate) fn keys_new_since(&self, before: &[Seen], most: usize) -> Vec<NodeKey> {
        self.new_since(before)
            .0
            .into_iter()
            .take(most)
            .filter_map(|at| self.lines[at].key.clone())
            .collect()
    }

    /// **The lines new and gone since `before`**, an earlier read of the same document (#2137):
    /// each new one with the reference `refer` deals it and the line it is read at, then each
    /// gone one, at most `most` in all; what `scrub` holds is taken out of every gone one,
    /// plainly and as an address on it reads.
    pub(crate) fn changes(
        &self,
        before: &[Seen],
        most: usize,
        scrub: &Kept,
        refer: &mut dyn FnMut(&NodeKey) -> String,
    ) -> String {
        let scrub = Scrub::of(scrub);
        let (new, gone) = self.new_since(before);
        let mut out = String::new();
        if new.is_empty() && gone.is_empty() {
            out.push_str("Nothing changed since the page was last read.\n");
            return out;
        }
        let _ = writeln!(
            out,
            "Changed since the page was last read: {} new, {} gone.",
            new.len(),
            gone.len()
        );
        let mut told = 0;
        for at in &new {
            if told == most {
                break;
            }
            let _ = writeln!(
                out,
                "new: {} (line {})",
                self.render(&self.lines[*at], 0, refer),
                at + 1
            );
            told += 1;
        }
        for words in &gone {
            if told == most {
                break;
            }
            // Taken out as a reader of an address on the line would read it, too.
            let _ = writeln!(out, "gone: {}", cut(&scrub.address(words)));
            told += 1;
        }
        let all = new.len() + gone.len();
        if all > told {
            let _ = writeln!(out, "(and {} more: take a snapshot)", all - told);
        }
        out
    }

    /// Every line `query` matches inside `within`, each with its line number in the scope.
    fn matches(
        &self,
        query: &Query,
        within: Option<&NodeKey>,
    ) -> Result<Vec<(usize, &Line)>, NotOnPage> {
        let (start, end) = self.scope(within)?;
        let text = query.text.as_deref().map(str::to_lowercase);
        let role = query.role.as_deref().map(str::to_lowercase);
        Ok(self.lines[start..end]
            .iter()
            .enumerate()
            .filter(|(_, line)| {
                role.as_ref()
                    .is_none_or(|role| line.role.to_lowercase() == *role)
                    && text.as_ref().is_none_or(|text| shows(line, text))
            })
            .collect())
    }

    /// `line` as an answer shows it, `depth` deep.
    fn render(
        &self,
        line: &Line,
        depth: usize,
        refer: &mut dyn FnMut(&NodeKey) -> String,
    ) -> String {
        let mut out = render(line, depth, refer);
        if line
            .key
            .as_ref()
            .is_some_and(|key| self.out_of_view.contains(key))
        {
            out.push_str(" (out of view)");
        }
        out
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
    rendered(line, depth, Some(refer))
}

/// `line`, `depth` deep, its element's reference dealt by `refer`, or none shown without it.
fn rendered(
    line: &Line,
    depth: usize,
    refer: Option<&mut dyn FnMut(&NodeKey) -> String>,
) -> String {
    let mut out = String::new();
    for _ in 0..depth {
        out.push_str("  ");
    }
    out.push_str("- ");
    out.push_str(&line.role);
    if !line.name.is_empty() {
        let _ = write!(out, " \"{}\"", quoted(&line.name));
    }
    if let (Some(key), Some(refer)) = (&line.key, refer) {
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
    /// Text of blanked elements (each at least [`SHORTEST_SCRUBBED`] characters), to take out
    /// of every other line.
    blanked_text: Kept,
    focused: Vec<NodeKey>,
    /// The documents the engine says have the focus.
    focused_documents: Vec<NodeKey>,
    /// What each blanked element holds as its value, to join a group of boxes by.
    held: HashMap<NodeKey, String>,
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
            let focused = if role == "RootWebArea" {
                &mut self.focused_documents
            } else {
                &mut self.focused
            };
            if !focused.contains(key) {
                focused.push(key.clone());
            }
        }
        // A blanked element is blanked whatever the engine says of it, ignored or a wrapper.
        if let Some(key) = &key
            && self.excluded.contains(key)
        {
            self.held
                .insert(key.clone(), value.clone().unwrap_or_default());
            self.blank(key, tree, node, value);
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
    fn blank(&mut self, key: &NodeKey, tree: &FrameNodes<'_>, node: &Value, value: Option<String>) {
        self.keep_blanked(key, value.as_deref().unwrap_or_default());
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
                self.keep_blanked(key, &text);
            }
            stack.extend(child_ids(inside).filter_map(|c| tree.get(c)));
        }
    }
}

impl Builder<'_> {
    /// Keeps `text`, read from blanked element `key`, if it is long enough to take out.
    fn keep_blanked(&mut self, key: &NodeKey, text: &str) {
        if text.chars().count() >= SHORTEST_SCRUBBED {
            self.blanked_text.tree(key, &[text]);
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
