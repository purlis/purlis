// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! What a page draws that may show a text kept out (#2303), read from the engine's own layout
//! of the page (`DOMSnapshot.captureSnapshot`), never from a script in the page.
//!
//! Once a page keeps anything out, a screenshot paints over:
//!
//! - every text the page lays out that holds a text kept out, whatever its tree says of it
//!   (hidden from the tree, drawn by the page's style, deeper than a snapshot reads, split over
//!   elements), and every field, button or drop-down whose value, options or labels hold one;
//! - every frame whose document this read does not hold, unless it is read in a session of its
//!   own;
//! - every SVG `use` a document lays out once any text of it holds a text kept out, laid out or
//!   not: what a `use` draws is laid out nowhere the read reaches.
//!
//! On a page whose site a text was kept out on ([`Drawn::pictured`]), it also paints over every canvas,
//! image, video and embedded object, and every element drawn with an image or a paint script:
//! what they show cannot be read, so they are painted whatever they show. Each is widened by the
//! `drop-shadow(…)` filters it is drawn through, and on such a page one drawn through a filter
//! whose reach cannot be read ([`Drawn::pictured_unread`]) withholds the screenshot.
//!
//! A text's shadow (`text-shadow`) or a `drop-shadow(…)` filter, on what is painted or on an
//! element around it, widens its paint by as far as the shadow reaches ([`Drawn::reach`]),
//! where its offsets and blur are read in pixels.
//!
//! An answer purlis cannot read, a page that draws a reflection, one that draws a frame through
//! a filter that may draw it elsewhere, one that draws a blanked element or a painted text
//! through a filter of its own (`url(…)`) or a shadow whose reach cannot be read, or one that
//! draws a backdrop through a filter of its own (`backdrop-filter: url(…)`), withholds the
//! screenshot.

use std::collections::{HashMap, HashSet};

use serde_json::Value;

use super::blank::Rect;
use super::scrub::{holds_any, holds_loosely};

/// The styles read for each element laid out: an image in any of the first six is painted over,
/// a filter, a backdrop filter or a text's shadow (the three after them) may widen what is
/// painted or withhold the screenshot, and a reflection (the last) withholds it.
pub(crate) const STYLES: [&str; 10] = [
    "background-image",
    "border-image-source",
    "mask-image",
    "-webkit-mask-box-image-source",
    "list-style-image",
    "content",
    "filter",
    "backdrop-filter",
    "text-shadow",
    "-webkit-box-reflect",
];

/// Filters that may draw an element away from its box: a frame drawn through one, itself or by
/// an element it is in, withholds the screenshot.
const FILTERS_THAT_MOVE: [&str; 2] = ["url(", "drop-shadow"];

/// Elements that show what no read of the page can tell: painted whatever they show.
const OPAQUE: [&str; 7] = [
    "CANVAS", "IMG", "VIDEO", "EMBED", "OBJECT", "IMAGE", "FEIMAGE",
];

/// The engine's number for an element node.
const ELEMENT: i64 = 1;

/// Elements that hold another document.
const FRAMES: [&str; 3] = ["IFRAME", "FRAME", "FENCEDFRAME"];

/// What one session's documents draw that is to be painted over.
#[derive(Debug, Default, PartialEq)]
pub(crate) struct Drawn {
    /// Each element to paint over, then the elements it is in, innermost first, by the engine's
    /// ids: the first of them the engine can place is painted.
    pub paint: Vec<Vec<i64>>,
    /// What cannot be read (a canvas, an image…), as [`Drawn::paint`]: painted on a page whose
    /// site a text was kept out on.
    pub pictured: Vec<Vec<i64>>,
    /// Whether an item of [`Drawn::pictured`] is drawn through a filter, its own or one of an
    /// element around it, whose reach cannot be read (a filter of the page's own, `url(…)`, or a
    /// shadow in other lengths): a screenshot that paints the pictured items is withheld. The
    /// reach of each that can be read is in [`Drawn::reach`].
    pub pictured_unread: bool,
    /// The address of the first document read: what the page shows, whatever it was told.
    pub address: Option<String>,
    /// Each frame element whose document this read does not hold, as [`Drawn::paint`]: painted
    /// unless that document is read in a session of its own.
    pub frames: Vec<Vec<i64>>,
    /// How far past its box each item painted (the first of a [`Drawn::paint`] or
    /// [`Drawn::pictured`] chain) or blanked element draws a shadow, by the engine's id: its paint
    /// is widened by as much. An item with no shadow is not listed.
    pub reach: HashMap<i64, Reach>,
}

/// How far past each side of a box something draws, in CSS pixels.
#[derive(Debug, Default, Clone, Copy, PartialEq)]
pub(crate) struct Reach {
    pub left: f64,
    pub top: f64,
    pub right: f64,
    pub bottom: f64,
}

impl Reach {
    /// What draws as far as either does.
    fn or(self, other: Self) -> Self {
        Self {
            left: self.left.max(other.left),
            top: self.top.max(other.top),
            right: self.right.max(other.right),
            bottom: self.bottom.max(other.bottom),
        }
    }

    /// `rect` with as much more on each side.
    pub(crate) fn around(&self, rect: &Rect) -> Rect {
        Rect {
            x: rect.x - self.left,
            y: rect.y - self.top,
            width: rect.width + self.left + self.right,
            height: rect.height + self.top + self.bottom,
        }
    }

    /// What draws as far as both, one drawn through the other.
    fn and(self, other: Self) -> Self {
        Self {
            left: self.left + other.left,
            top: self.top + other.top,
            right: self.right + other.right,
            bottom: self.bottom + other.bottom,
        }
    }
}

/// The engine's answer could not be read, or shows what a screenshot cannot paint over.
#[derive(Debug, thiserror::Error)]
#[error("what the page draws could not be read: {0}")]
pub(crate) struct Unread(&'static str);

/// What `answer` (of `DOMSnapshot.captureSnapshot`, with [`STYLES`]) draws that is to be painted
/// over on a page that keeps `kept` out, and `loose` out also with any separators between
/// its characters (what a group of boxes holds together); `blanked` the engine's ids of its
/// session's blanked elements.
pub(crate) fn drawn(
    answer: &Value,
    kept: &[&str],
    loose: &[&str],
    blanked: &HashSet<i64>,
) -> Result<Drawn, Unread> {
    let strings: Vec<&str> = answer
        .get("strings")
        .and_then(Value::as_array)
        .ok_or(Unread("it names no strings"))?
        .iter()
        .map(|s| s.as_str().ok_or(Unread("a string is not text")))
        .collect::<Result<_, _>>()?;
    let documents = answer
        .get("documents")
        .and_then(Value::as_array)
        .ok_or(Unread("it lists no documents"))?;
    let kept: Vec<String> = kept
        .iter()
        .map(|k| collapsed(k).trim().to_owned())
        .collect();
    let kept: Vec<&str> = kept.iter().map(String::as_str).collect();
    let mut out = Drawn::default();
    if let Some(first) = documents.first() {
        let at = first
            .get("documentURL")
            .and_then(Value::as_i64)
            .and_then(|i| usize::try_from(i).ok())
            .and_then(|i| strings.get(i));
        out.address = at.map(|a| (*a).to_owned());
    }
    for document in documents {
        read_document(document, &strings, (&kept, loose, blanked), &mut out)?;
    }
    Ok(out)
}

/// `text` with every run of white space (a line break and a no-break space among them) one
/// space, as the page draws it.
fn collapsed(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut space = false;
    for c in text.chars() {
        if c.is_whitespace() {
            space = true;
        } else {
            if space && !out.is_empty() {
                out.push(' ');
            }
            space = false;
            out.push(c);
        }
    }
    if space {
        out.push(' ');
    }
    out
}

fn read_document(
    document: &Value,
    strings: &[&str],
    (kept, loose, blanked): (&[&str], &[&str], &HashSet<i64>),
    out: &mut Drawn,
) -> Result<(), Unread> {
    let nodes = document
        .get("nodes")
        .ok_or(Unread("a document has no nodes"))?;
    let layout = document
        .get("layout")
        .ok_or(Unread("a document has no layout"))?;
    let string = |index: i64| -> Result<&str, Unread> {
        if index < 0 {
            return Ok("");
        }
        usize::try_from(index)
            .ok()
            .and_then(|at| strings.get(at).copied())
            .ok_or(Unread("a string is not listed"))
    };
    let parents = numbers(nodes, "parentIndex")?;
    let names = numbers(nodes, "nodeName")?;
    let kinds = numbers(nodes, "nodeType")?;
    let values = numbers(nodes, "nodeValue")?;
    let backends = numbers(nodes, "backendNodeId")?;
    let attributes = nodes
        .get("attributes")
        .and_then(Value::as_array)
        .ok_or(Unread("its nodes have no attributes"))?;
    let count = parents.len();
    if [
        names.len(),
        kinds.len(),
        values.len(),
        backends.len(),
        attributes.len(),
    ]
    .iter()
    .any(|n| *n != count)
    {
        return Err(Unread("its nodes do not line up"));
    }
    let laid = numbers(layout, "nodeIndex")?;
    let texts = numbers(layout, "text")?;
    let styles = layout
        .get("styles")
        .and_then(Value::as_array)
        .ok_or(Unread("its layout has no styles"))?;
    if texts.len() != laid.len() || styles.len() != laid.len() {
        return Err(Unread("its layout does not line up"));
    }
    let node_at = |n: i64| {
        usize::try_from(n)
            .ok()
            .filter(|at| *at < count)
            .ok_or(Unread("its layout names a node it does not have"))
    };
    let name = |at: usize| string(names[at]).map(str::to_ascii_uppercase);
    let parent = |at: usize| usize::try_from(parents[at]).ok().filter(|p| *p < count);
    let chain = |at: usize| -> Vec<i64> {
        let mut out = Vec::new();
        let mut next = Some(at);
        while let Some(at) = next {
            if out.len() > count {
                break;
            }
            out.push(backends[at]);
            next = parent(at);
        }
        out
    };
    let holds = |text: &str| {
        !text.is_empty() && (holds_any(&collapsed(text), kept) || holds_loosely(text, loose))
    };
    // Whether `text` holds any of the texts kept out numbered `which`: those of `kept` first,
    // then those of `loose`.
    let holds_one_of = |text: &str, which: &[usize]| {
        let strict: Vec<&str> = which
            .iter()
            .filter_map(|one| kept.get(*one).copied())
            .collect();
        let spaced: Vec<&str> = which
            .iter()
            .filter_map(|one| {
                one.checked_sub(kept.len())
                    .and_then(|at| loose.get(at))
                    .copied()
            })
            .collect();
        (!strict.is_empty() && holds_any(&collapsed(text), &strict))
            || (!spaced.is_empty() && holds_loosely(text, &spaced))
    };
    let mut painted: HashSet<usize> = HashSet::new();
    let mut paint = |at: usize, out: &mut Drawn| {
        if painted.insert(at) {
            out.paint.push(chain(at));
        }
    };

    let contents = rare(nodes, "contentDocumentIndex")?;
    let mut shown: HashSet<usize> = HashSet::new();
    // The text each node lays out itself.
    let mut own: Vec<String> = vec![String::new(); count];
    // Whether each element draws through a filter that may draw it away from its box, and how
    // far its filter's shadows reach (`None` where that cannot be told).
    let mut filtered = vec![false; count];
    let mut filter_reach: Vec<Option<Reach>> = vec![Some(Reach::default()); count];
    // How far past its box each element draws its text's shadow (`None` where that cannot be
    // read).
    let mut shadowed: Vec<Option<Reach>> = vec![Some(Reach::default()); count];
    // What cannot be read (a canvas, an image…), as listed in `out.pictured`.
    let mut pictures: Vec<usize> = Vec::new();
    for (entry, node) in laid.iter().enumerate() {
        let at = node_at(*node)?;
        shown.insert(at);
        let name = name(at)?;
        let style: Vec<String> = styles[entry]
            .as_array()
            .ok_or(Unread("a style list is not a list"))?
            .iter()
            .map(|v| {
                v.as_i64()
                    .ok_or(Unread("a style is not a string"))
                    .and_then(string)
                    .map(str::to_ascii_lowercase)
            })
            .collect::<Result<_, _>>()?;
        // The engine gives no styles for what is not an element (a text, the document): it
        // draws them in the styles of the element they are in.
        let style = match style.as_slice() {
            [] if kinds[at] != ELEMENT => vec![String::new(); STYLES.len()],
            _ => style,
        };
        let [images @ .., filter, backdrop, shadow, reflected] = style.as_slice() else {
            return Err(Unread("an element has no styles"));
        };
        if images.len() + 4 != STYLES.len() {
            return Err(Unread("an element has styles missing"));
        }
        filtered[at] = FILTERS_THAT_MOVE.iter().any(|moves| filter.contains(moves));
        filter_reach[at] = filters_reach(filter);
        shadowed[at] = shadows_reach(shadow);
        if !reflected.is_empty() && reflected != "none" {
            return Err(Unread("the page draws a reflection"));
        }
        // A backdrop filter redraws what lies under its element, which may be a blanked field,
        // and one drawn by a page's own filter may draw it anywhere.
        if backdrop.contains("url(") {
            return Err(Unread("the page draws a backdrop through a filter"));
        }
        let image = images
            .iter()
            .any(|style| style.contains("url(") || style.contains("paint("));
        own[at].push_str(string(texts[entry])?);
        let pictured = image
            || OPAQUE.contains(&name.as_str())
            || (name == "INPUT"
                && attribute(&attributes[at], &string, "type")?.eq_ignore_ascii_case("image"));
        if pictured {
            out.pictured.push(chain(at));
            pictures.push(at);
        }
        if FRAMES.contains(&name.as_str()) && !contents.iter().any(|(i, _)| *i == at) {
            out.frames.push(chain(at));
        }
    }
    // Text laid out that holds a text kept out, whole or split over elements: the innermost
    // node whose text, read in order, holds it. Each node comes after the one it is in and
    // passes its text on to it, with the texts kept out it holds whole: a node is painted for
    // a text it holds that no node in it holds whole, so two texts side by side, one inside an
    // element and one across its edge, are each painted, and what is in it never paints more.
    let drew: Vec<bool> = own.iter().map(|t| !t.is_empty()).collect();
    let mut whole: Vec<HashSet<usize>> = vec![HashSet::new(); count];
    let mut holders: HashSet<usize> = HashSet::new();
    for at in (0..count).rev() {
        let text = std::mem::take(&mut own[at]);
        let mut held = std::mem::take(&mut whole[at]);
        let fresh: Vec<usize> = (0..kept.len() + loose.len())
            .filter(|one| !held.contains(one))
            .collect();
        let found: Vec<usize> = if text.is_empty() || !holds_one_of(&text, &fresh) {
            Vec::new()
        } else {
            fresh
                .into_iter()
                .filter(|one| holds_one_of(&text, &[*one]))
                .collect()
        };
        if !found.is_empty() {
            paint(at, out);
            holders.insert(at);
        }
        held.extend(found);
        if let Some(up) = parent(at) {
            if up >= at {
                return Err(Unread("a node comes before the one it is in"));
            }
            own[up].insert_str(0, &text);
            whole[up].extend(held);
        }
    }
    // A text split over elements is painted at each text it is drawn in as well, wherever
    // each is drawn.
    let mut under = vec![false; count];
    for at in 0..count {
        under[at] = parent(at).is_some_and(|up| holders.contains(&up) || under[up]);
        if under[at] && drew[at] {
            paint(at, out);
        }
    }
    // An SVG `use` draws a copy of what it points at, which may be laid out nowhere else (a text
    // inside `defs`): once any text of the document holds a text kept out, laid out or not,
    // every `use` it lays out is painted.
    let mut uses = Vec::new();
    for at in 0..count {
        if shown.contains(&at) && name(at)? == "USE" {
            uses.push(at);
        }
    }
    if !uses.is_empty() {
        let mut any = !holders.is_empty();
        for value in &values {
            if any {
                break;
            }
            any = holds(string(*value)?);
        }
        if any {
            for at in &uses {
                paint(*at, out);
            }
        }
    }
    // What a field, a button or a drop-down shows is its value or its options, not text laid
    // out where the read reaches it.
    for field in ["inputValue", "textValue"] {
        for (at, value) in rare(nodes, field)? {
            if shown.contains(&at) && holds(string(value)?) {
                paint(at, out);
            }
        }
    }
    for (at, pairs) in attributes.iter().enumerate() {
        if !shown.contains(&at) {
            continue;
        }
        for shows in ["value", "placeholder"] {
            if holds(attribute(pairs, &string, shows)?) {
                paint(at, out);
            }
        }
    }
    // A drop-down shows its options' text and labels, which are laid out nowhere the read
    // reaches: the drop-down they are in is painted.
    let drop_down = |at: usize| -> Result<Option<usize>, Unread> {
        let mut next = parent(at);
        let mut steps = 0;
        while let Some(up) = next {
            if steps > count {
                break;
            }
            if shown.contains(&up) && name(up)? == "SELECT" {
                return Ok(Some(up));
            }
            next = parent(up);
            steps += 1;
        }
        Ok(None)
    };
    for at in 0..count {
        let labelled = matches!(name(at)?.as_str(), "OPTION" | "OPTGROUP")
            && holds(attribute(&attributes[at], &string, "label")?);
        if (labelled || holds(string(values[at])?))
            && let Some(list) = drop_down(at)?
        {
            paint(list, out);
        }
    }
    // A frame is drawn through no filter that may draw it elsewhere, by itself or by an element
    // it is in.
    for at in 0..count {
        if !FRAMES.contains(&name(at)?.as_str()) {
            continue;
        }
        let mut next = Some(at);
        let mut steps = 0;
        while let Some(up) = next {
            if filtered[up] {
                return Err(Unread("the page draws through a filter"));
            }
            steps += 1;
            next = parent(up).filter(|_| steps <= count);
        }
    }
    // A blanked element, and what is painted for what it shows, is widened by the shadows drawn
    // of it: the text shadows of every element in it and of the element it is drawn in (a text's
    // shadow is drawn in that element's style), and the shadows of the filters of it and of every
    // element around it, one drawn through the next. One whose reach cannot be read withholds the
    // screenshot, as does a filter of the page's own (`url(…)`), but for a `use`: its paint is
    // widened by what can be read.
    let mut items: HashMap<usize, bool> = painted.iter().map(|at| (*at, false)).collect();
    for at in uses.iter().filter(|at| painted.contains(at)) {
        items.insert(*at, true);
    }
    for (at, backend) in backends.iter().enumerate() {
        if blanked.contains(backend) {
            items.insert(at, false);
        }
    }
    let mut reach: HashMap<usize, Option<Reach>> = HashMap::new();
    for (one, own) in shadowed.iter().enumerate() {
        let Some(own) = *own else {
            // Its shadow cannot be read: what it is in and what is painted for it withhold.
            let mut next = Some(one);
            let mut steps = 0;
            while let Some(up) = next {
                if items.contains_key(&up) {
                    reach.insert(up, None);
                }
                steps += 1;
                next = parent(up).filter(|_| steps <= count);
            }
            continue;
        };
        if own == Reach::default() {
            continue;
        }
        let mut next = Some(one);
        let mut steps = 0;
        while let Some(up) = next {
            if items.contains_key(&up) {
                let was = reach.entry(up).or_insert(Some(Reach::default()));
                *was = was.map(|was| was.or(own));
            }
            steps += 1;
            next = parent(up).filter(|_| steps <= count);
        }
    }
    for (at, is_use) in items {
        let mut widened = reach.get(&at).copied().unwrap_or(Some(Reach::default()));
        // A text's shadow is drawn in the style of the element the text is in.
        let mut styled = Some(at);
        let mut steps = 0;
        while let Some(one) = styled.filter(|one| kinds[*one] != ELEMENT && steps <= count) {
            styled = parent(one);
            steps += 1;
        }
        if let Some(one) = styled.filter(|one| *one != at) {
            widened = widened.zip(shadowed[one]).map(|(a, b)| a.or(b));
        }
        let mut widened = match widened {
            Some(widened) => widened,
            None if is_use => Reach::default(),
            None => return Err(Unread("the page draws a shadow whose reach cannot be read")),
        };
        let mut next = Some(at);
        let mut steps = 0;
        while let Some(up) = next {
            match filter_reach[up] {
                Some(more) => widened = widened.and(more),
                None if is_use => {}
                None => return Err(Unread("the page draws through a filter")),
            }
            steps += 1;
            next = parent(up).filter(|_| steps <= count);
        }
        if widened != Reach::default() {
            let was = out.reach.entry(backends[at]).or_default();
            *was = was.or(widened);
        }
    }
    // What cannot be read is widened by the shadows of the filters of it and of every element
    // around it, one drawn through the next; one whose reach cannot be read is flagged, as it
    // withholds only a screenshot that paints what cannot be read. A text's shadow draws its
    // text, not what an image or a canvas shows.
    for at in pictures {
        let mut widened = Some(Reach::default());
        let mut next = Some(at);
        let mut steps = 0;
        while let Some(up) = next {
            widened = widened.zip(filter_reach[up]).map(|(a, b)| a.and(b));
            steps += 1;
            next = parent(up).filter(|_| steps <= count);
        }
        match widened {
            None => out.pictured_unread = true,
            Some(widened) if widened != Reach::default() => {
                let was = out.reach.entry(backends[at]).or_default();
                *was = was.or(widened);
            }
            Some(_) => {}
        }
    }
    Ok(())
}

/// How far past its box `shadow`, a computed `text-shadow`, draws its text: as far as the
/// furthest of its shadows on each side. `None` when it cannot be read.
fn shadows_reach(shadow: &str) -> Option<Reach> {
    if shadow.is_empty() || shadow == "none" {
        return Some(Reach::default());
    }
    let mut furthest = Reach::default();
    for one in outside_parentheses(shadow, ',') {
        furthest = furthest.or(one_shadows_reach(&one)?);
    }
    Some(furthest)
}

/// How far past its box `filter`, a computed `filter`, draws its element: as far as its
/// `drop-shadow(…)` shadows reach, each drawn through the ones before it. `None` when that
/// cannot be told: a filter of the page's own (`url(…)`), or a shadow that cannot be read.
fn filters_reach(filter: &str) -> Option<Reach> {
    if filter.contains("url(") {
        return None;
    }
    let mut reach = Reach::default();
    let mut rest = filter;
    while let Some(at) = rest.find("drop-shadow(") {
        let inside = &rest[at + "drop-shadow(".len()..];
        let mut depth = 1_usize;
        let end = inside.char_indices().find_map(|(i, c)| {
            match c {
                '(' => depth += 1,
                ')' => depth -= 1,
                _ => {}
            }
            (depth == 0).then_some(i)
        })?;
        reach = reach.and(one_shadows_reach(
            &outside_parentheses(&inside[..end], ',').concat(),
        )?);
        rest = &inside[end + 1..];
    }
    Some(reach)
}

/// The parts of `text` between each `split` outside parentheses, with what is inside any
/// parentheses (a colour's numbers) left out.
fn outside_parentheses(text: &str, split: char) -> Vec<String> {
    let mut parts = vec![String::new()];
    let mut depth = 0_usize;
    for c in text.chars() {
        match c {
            '(' => depth += 1,
            ')' => depth = depth.saturating_sub(1),
            _ if c == split && depth == 0 => parts.push(String::new()),
            _ if depth == 0 => {
                if let Some(part) = parts.last_mut() {
                    part.push(c);
                }
            }
            _ => {}
        }
    }
    parts
}

/// How far past its box one shadow draws, from its words outside parentheses: its offsets
/// across and down and its blur, in pixels, the blur reaching three times its length and a
/// pixel more (as far as a blur of that radius or deviation is drawn). `None` when a length is
/// not in pixels or the words are not a shadow's.
fn one_shadows_reach(words: &str) -> Option<Reach> {
    let mut lengths = Vec::new();
    for word in words.split_whitespace() {
        if word.starts_with(|c: char| c.is_ascii_alphabetic()) {
            continue;
        }
        let length = word
            .strip_suffix("px")
            .unwrap_or(word)
            .parse::<f64>()
            .ok()?;
        if !(length.is_finite() && (word.ends_with("px") || length == 0.0)) {
            return None;
        }
        lengths.push(length);
    }
    let [across, down, blur @ ..] = lengths.as_slice() else {
        return None;
    };
    let blur = match blur {
        [] => 0.0,
        [blur] if *blur >= 0.0 => *blur,
        _ => return None,
    };
    let spread = 3.0 * blur + 1.0;
    Some(Reach {
        left: (spread - across).max(0.0),
        top: (spread - down).max(0.0),
        right: (spread + across).max(0.0),
        bottom: (spread + down).max(0.0),
    })
}

/// A list of numbers of `of`, by `field`.
fn numbers(of: &Value, field: &str) -> Result<Vec<i64>, Unread> {
    of.get(field)
        .and_then(Value::as_array)
        .ok_or(Unread("a list is missing"))?
        .iter()
        .map(|n| {
            n.as_i64()
                .ok_or(Unread("a list holds what is not a number"))
        })
        .collect()
}

/// The pairs of a field given for some nodes only (`{index, value}`); none when it is absent.
fn rare(of: &Value, field: &str) -> Result<Vec<(usize, i64)>, Unread> {
    let Some(data) = of.get(field) else {
        return Ok(Vec::new());
    };
    let index = numbers(data, "index")?;
    let value = numbers(data, "value")?;
    if index.len() != value.len() {
        return Err(Unread("a field for some nodes does not line up"));
    }
    index
        .into_iter()
        .zip(value)
        .map(|(i, v)| {
            Ok((
                usize::try_from(i).map_err(|_| Unread("a field names no node"))?,
                v,
            ))
        })
        .collect()
}

/// The value of the attribute `wanted` among `pairs` (name, value, name, value…), empty when
/// it has none.
fn attribute<'a>(
    pairs: &Value,
    string: &impl Fn(i64) -> Result<&'a str, Unread>,
    wanted: &str,
) -> Result<&'a str, Unread> {
    let pairs = pairs
        .as_array()
        .ok_or(Unread("an attribute list is not a list"))?;
    for pair in pairs.chunks(2) {
        let [name, value] = pair else {
            return Err(Unread("an attribute has no value"));
        };
        let name = string(name.as_i64().ok_or(Unread("an attribute is not named"))?)?;
        if name.eq_ignore_ascii_case(wanted) {
            return string(
                value
                    .as_i64()
                    .ok_or(Unread("an attribute's value is not text"))?,
            );
        }
    }
    Ok("")
}
