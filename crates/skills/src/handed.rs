// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A skill's frontmatter, read as the YAML it is, and the `SKILL.md` purlis hands a harness**
//! (SK-8, #2771; decision 58: a skill can only narrow a chat).
//!
//! purlis never splices a skill's own text into what a harness reads. It parses the frontmatter
//! with a YAML parser, and writes a new one from the keys it knows to be safe ([`Field::kept`]),
//! each value written out again by purlis, then the skill's body as it is. So whatever a harness's
//! own reader would make of a trick of YAML (a `---` inside a value, a quoted or indented key, a
//! flow mapping, an explicit `? key`), it is never handed a key purlis did not choose to hand.
//!
//! **It fails closed.** A frontmatter that does not parse, that is not one mapping of plain names,
//! that reuses part of itself by an anchor, an alias or a merge key, that carries a tag, that sets
//! a key twice, that spells a key so it reads as one that widens a chat (`allowed_tools`,
//! `allowedTools`), or that gives a kept key a value it cannot take, is refused: the skill is
//! handed to no chat, and the chat's tab says why ([`Refusal`]).

use yaml_rust2::parser::{Event, Parser};
use yaml_rust2::scanner::TScalarStyle;

use crate::compat::Field;

/// Why purlis hands a skill to no chat. Its [`std::fmt::Display`] is the end of a sentence a
/// person reads, which names no key.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum Refusal {
    /// The text does not start with a frontmatter fenced by `---` lines.
    NoFrontmatter,
    /// The frontmatter is not YAML: what the parser said.
    NotYaml(String),
    /// The frontmatter is not one mapping of names to values.
    NotAMapping,
    /// Part of it is reused by an anchor, an alias or a merge key.
    Reused,
    /// Part of it carries a tag.
    Tagged,
    /// A key is not a plain name.
    OddKey,
    /// A key is set twice.
    Twice(String),
    /// A key spelled so it reads as one that widens the chat, but not as purlis knows it.
    Respelled(String),
    /// A kept key has a value it cannot take.
    BadValue(Field),
    /// Its folder is a link.
    FolderLink,
    /// Its `SKILL.md` is a link, or no plain file.
    FileLink,
    /// Its `SKILL.md` is larger than purlis reads.
    TooLarge,
    /// Its `SKILL.md` cannot be read: what the system said.
    Unreadable(String),
}

impl std::fmt::Display for Refusal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NoFrontmatter => {
                f.write_str("its SKILL.md does not start with a frontmatter between two --- lines")
            }
            Self::NotYaml(why) => write!(f, "its frontmatter cannot be read ({why})"),
            Self::NotAMapping => f.write_str("its frontmatter is not a list of names and values"),
            Self::Reused => {
                f.write_str("its frontmatter reuses part of itself, which purlis does not hand on")
            }
            Self::Tagged => {
                f.write_str("its frontmatter carries a tag, which purlis does not hand on")
            }
            Self::OddKey => f.write_str("its frontmatter has a name that is not plain words"),
            Self::Twice(_) => f.write_str("its frontmatter sets one name twice"),
            Self::Respelled(_) => f.write_str(
                "its frontmatter spells a name like one that would widen the chat, so purlis \
                 cannot tell what a chat would read",
            ),
            Self::BadValue(field) => write!(f, "{} is not something it can be", field.said()),
            Self::FolderLink => f.write_str("its folder is a link, which purlis does not follow"),
            Self::FileLink => f.write_str(
                "its SKILL.md is a link or not a plain file, which purlis does not follow",
            ),
            Self::TooLarge => f.write_str("its SKILL.md is larger than purlis reads"),
            Self::Unreadable(why) => write!(f, "its SKILL.md cannot be read ({why})"),
        }
    }
}

/// A value of the frontmatter, as purlis read it: no anchors, aliases or tags.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Node {
    /// A scalar and whether it was written plain (so `true` is a boolean).
    Scalar {
        text: String,
        plain: bool,
    },
    Seq(Vec<Node>),
    Map(Vec<(Node, Node)>),
}

/// **A frontmatter, read** ([`parse`]): every top-level key in the order written, the field
/// purlis knows it as where it knows it, and where the body starts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Frontmatter {
    entries: Vec<(String, Node)>,
    body: usize,
}

impl Frontmatter {
    /// The fields it sets, in the order written.
    pub fn fields(&self) -> Vec<Field> {
        self.entries
            .iter()
            .filter_map(|(key, _)| Field::of(key))
            .collect()
    }

    /// The text `field` is set to, where it is set to one scalar: a source's skill is named by
    /// it (SK-10, #2773).
    pub fn scalar(&self, field: Field) -> Option<&str> {
        self.entries.iter().find_map(|(key, node)| match node {
            Node::Scalar { text, .. } if Field::of(key) == Some(field) => Some(text.as_str()),
            _ => None,
        })
    }

    /// The keys it sets that purlis knows of no harness to read, as written.
    pub fn unknown(&self) -> Vec<String> {
        self.entries
            .iter()
            .filter(|(key, _)| Field::of(key).is_none())
            .map(|(key, _)| key.clone())
            .collect()
    }

    /// Whether it makes the skill manual-only: `disable-model-invocation` true (`yes`, `on` and
    /// `1` too).
    pub fn manual(&self) -> bool {
        self.entries.iter().any(|(key, node)| {
            Field::of(key) == Some(Field::ManualOnly)
                && matches!(node, Node::Scalar { text, .. } if truthy(text))
        })
    }
}

impl Frontmatter {
    /// The tools its `disallowed-tools` takes away, as written: one comma-separated line, or a
    /// list.
    pub fn tools_taken(&self) -> Vec<String> {
        let mut out = Vec::new();
        for (key, node) in &self.entries {
            if Field::of(key) != Some(Field::DisallowedTools) {
                continue;
            }
            let items: Vec<&str> = match node {
                Node::Scalar { text, .. } => text.split(',').collect(),
                Node::Seq(items) => items
                    .iter()
                    .filter_map(|item| match item {
                        Node::Scalar { text, .. } => Some(text.as_str()),
                        _ => None,
                    })
                    .collect(),
                Node::Map(_) => Vec::new(),
            };
            out.extend(
                items
                    .into_iter()
                    .map(str::trim)
                    .filter(|tool| !tool.is_empty())
                    .map(str::to_owned),
            );
        }
        out
    }

    /// Whether its `context` is `fork`.
    pub fn fork(&self) -> bool {
        self.entries.iter().any(|(key, node)| {
            Field::of(key) == Some(Field::Context)
                && matches!(node, Node::Scalar { text, .. } if text.trim().eq_ignore_ascii_case("fork"))
        })
    }
}

/// **Reads the `SKILL.md` at `path`, following no link**: refused where its folder or the file
/// is a link, the file is no plain file or is larger than [`MOST_BYTES`], or it changed from a
/// plain file into something else while it was opened. The one reader of a skill's own file
/// for what purlis hands, badges and lints (SK-8).
pub fn read_skill_md(path: &std::path::Path) -> Result<String, Refusal> {
    if let Some(folder) = path.parent() {
        let meta = std::fs::symlink_metadata(folder)
            .map_err(|why| Refusal::Unreadable(why.to_string()))?;
        if meta.file_type().is_symlink() || !meta.is_dir() {
            return Err(Refusal::FolderLink);
        }
    }
    read_plain(path, MOST_BYTES).map_err(|why| match why {
        NotRead::Unreadable(why) => Refusal::Unreadable(why),
        NotRead::NotPlain => Refusal::FileLink,
        NotRead::TooLarge => Refusal::TooLarge,
    })
}

/// Why [`read_plain`] read nothing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum NotRead {
    /// It could not be read: what the system said.
    Unreadable(String),
    /// It is a link or no plain file, or it changed into one while it was opened.
    NotPlain,
    /// It is larger than the most asked for.
    TooLarge,
}

/// The end of a sentence about the file: "it cannot be read (…)", "it is not a plain file", "it
/// is too large". The one way a file [`read_plain`] read nothing is said.
impl std::fmt::Display for NotRead {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Unreadable(why) => write!(f, "it cannot be read ({why})"),
            Self::NotPlain => f.write_str("it is not a plain file"),
            Self::TooLarge => f.write_str("it is too large"),
        }
    }
}

/// **The text of the plain file at `path`, following no link**, of at most `most` bytes: refused
/// where it is a link or no plain file, is larger, or changed from a plain file into something
/// else while it was opened. The one reader of a skill's files and the file beside its folder.
pub(crate) fn read_plain(path: &std::path::Path, most: u64) -> Result<String, NotRead> {
    use std::io::Read as _;
    let unreadable = |why: std::io::Error| NotRead::Unreadable(why.to_string());
    let meta = std::fs::symlink_metadata(path).map_err(unreadable)?;
    if !meta.is_file() {
        return Err(NotRead::NotPlain);
    }
    if meta.len() > most {
        return Err(NotRead::TooLarge);
    }
    let file = std::fs::File::open(path).map_err(unreadable)?;
    let opened = file.metadata().map_err(unreadable)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt as _;
        if (opened.dev(), opened.ino()) != (meta.dev(), meta.ino()) {
            return Err(NotRead::NotPlain);
        }
    }
    if !opened.is_file() {
        return Err(NotRead::NotPlain);
    }
    let mut text = String::new();
    file.take(most + 1)
        .read_to_string(&mut text)
        .map_err(unreadable)?;
    if text.len() as u64 > most {
        return Err(NotRead::TooLarge);
    }
    Ok(text)
}

/// Whether a boolean says yes, in any case: `true`, `yes`, `on` or `1`.
fn truthy(text: &str) -> bool {
    ["true", "yes", "on", "1"]
        .iter()
        .any(|yes| text.trim().eq_ignore_ascii_case(yes))
}

/// How deep a frontmatter may nest lists and mappings: far past any skill's, and short of what
/// would overflow the stack of the reader that walks it.
const MOST_NESTED: usize = 64;

/// The most bytes of a `SKILL.md` purlis reads.
pub const MOST_BYTES: u64 = 256 * 1024;

/// Where a frontmatter is in `text`: the YAML between a first line that is `---` and the next
/// line that is `---`, each on its own line, and where the body after it starts. `None` where
/// there is none.
fn fenced(text: &str) -> Option<(&str, usize)> {
    let text_start = text.strip_prefix('\u{feff}').map_or(0, |_| 3);
    let mut lines = text[text_start..].split_inclusive('\n');
    let first = lines.next()?;
    if first.trim_end() != "---" {
        return None;
    }
    let yaml_start = text_start + first.len();
    let mut at = yaml_start;
    for line in lines {
        if line.trim_end() == "---" {
            return Some((&text[yaml_start..at], at + line.len()));
        }
        at += line.len();
    }
    None
}

/// The spelling a key is compared by when purlis looks for one that reads as a widening key:
/// lowercase, with no `-`, `_` or blank.
fn folded(key: &str) -> String {
    key.chars()
        .filter(|c| !matches!(c, '-' | '_') && !c.is_whitespace())
        .flat_map(char::to_lowercase)
        .collect()
}

/// **Reads `text`'s frontmatter as YAML**, failing closed ([`Refusal`]): the one reader of a
/// skill's frontmatter for what purlis hands, badges, lints and installs. The frontmatter is
/// between a first line that is `---` (after a byte-order mark, if any) and the next line that
/// is `---`.
pub fn parse(text: &str) -> Result<Frontmatter, Refusal> {
    let (yaml, body) = fenced(text).ok_or(Refusal::NoFrontmatter)?;
    let mut parser = Parser::new_from_str(yaml);
    let mut next = || -> Result<Event, Refusal> {
        parser
            .next_token()
            .map(|(event, _)| event)
            .map_err(|why| Refusal::NotYaml(why.info().to_owned()))
    };
    if next()? != Event::StreamStart {
        return Err(Refusal::NotAMapping);
    }
    if next()? != Event::DocumentStart {
        return Err(Refusal::NotAMapping);
    }
    let root = node(next()?, &mut next, 0)?;
    match next()? {
        Event::DocumentEnd => {}
        _ => return Err(Refusal::NotAMapping),
    }
    if next()? != Event::StreamEnd {
        return Err(Refusal::NotAMapping);
    }
    let Node::Map(pairs) = root else {
        return Err(Refusal::NotAMapping);
    };
    let mut entries: Vec<(String, Node)> = Vec::new();
    for (key, value) in pairs {
        let Node::Scalar { text: key, .. } = key else {
            return Err(Refusal::OddKey);
        };
        if key == "<<" {
            return Err(Refusal::Reused);
        }
        if key.is_empty() || key.contains(['\n', '\r', ':']) {
            return Err(Refusal::OddKey);
        }
        if entries.iter().any(|(k, _)| folded(k) == folded(&key)) {
            return Err(Refusal::Twice(key));
        }
        let widening = Field::ALL
            .iter()
            .find(|field| field.widens() && folded(field.key()) == folded(&key));
        if widening.is_some_and(|field| field.key() != key) {
            return Err(Refusal::Respelled(key));
        }
        entries.push((key, value));
    }
    Ok(Frontmatter { entries, body })
}

/// The node `event` starts, `depth` lists and mappings in, reading the rest of it with `next`:
/// refused for an alias, an anchor or a tag, and past [`MOST_NESTED`].
fn node(
    event: Event,
    next: &mut dyn FnMut() -> Result<Event, Refusal>,
    depth: usize,
) -> Result<Node, Refusal> {
    if depth > MOST_NESTED {
        return Err(Refusal::NotYaml(format!(
            "it nests more than {MOST_NESTED} lists and mappings deep"
        )));
    }
    match event {
        Event::Alias(_) => Err(Refusal::Reused),
        Event::Scalar(_, _, anchor, _)
        | Event::SequenceStart(anchor, _)
        | Event::MappingStart(anchor, _)
            if anchor != 0 =>
        {
            Err(Refusal::Reused)
        }
        Event::Scalar(_, _, _, Some(_))
        | Event::SequenceStart(_, Some(_))
        | Event::MappingStart(_, Some(_)) => Err(Refusal::Tagged),
        Event::Scalar(text, style, _, None) => Ok(Node::Scalar {
            text,
            plain: style == TScalarStyle::Plain,
        }),
        Event::SequenceStart(..) => {
            let mut items = Vec::new();
            loop {
                match next()? {
                    Event::SequenceEnd => return Ok(Node::Seq(items)),
                    event => items.push(node(event, next, depth + 1)?),
                }
            }
        }
        Event::MappingStart(..) => {
            let mut pairs = Vec::new();
            loop {
                match next()? {
                    Event::MappingEnd => return Ok(Node::Map(pairs)),
                    event => {
                        let key = node(event, next, depth + 1)?;
                        let value = node(next()?, next, depth + 1)?;
                        pairs.push((key, value));
                    }
                }
            }
        }
        _ => Err(Refusal::NotAMapping),
    }
}

/// The shape a kept field's value takes.
enum Shape {
    /// One line of words.
    Text,
    /// A boolean.
    Flag,
    /// One line of words, or a list of them.
    TextOrList,
    /// Names mapped to words.
    Names,
}

impl Field {
    /// **Whether purlis hands a harness this field**, and the shape its value takes: every field
    /// but the ones that widen a chat.
    fn kept(self) -> Option<Shape> {
        Some(match self {
            Self::AllowedTools | Self::Hooks => return None,
            Self::ManualOnly | Self::UserInvocable | Self::Background => Shape::Flag,
            Self::Arguments | Self::DisallowedTools | Self::Paths => Shape::TextOrList,
            Self::Metadata => Shape::Names,
            Self::Name
            | Self::Description
            | Self::WhenToUse
            | Self::ArgumentHint
            | Self::Model
            | Self::Effort
            | Self::Context
            | Self::Agent
            | Self::Shell
            | Self::License
            | Self::Compatibility => Shape::Text,
        })
    }
}

/// `node`, written out again as `shape`: `None` where it is not one.
fn written(node: &Node, shape: &Shape) -> Option<String> {
    let text = |node: &Node, flow: bool| match node {
        Node::Scalar { text, .. } => Some(scalar(text, flow)),
        _ => None,
    };
    match (shape, node) {
        (Shape::Text | Shape::TextOrList, Node::Scalar { .. }) => text(node, false),
        (Shape::Flag, Node::Scalar { text, plain }) => {
            let word = text.trim().to_ascii_lowercase();
            if *plain && matches!(word.as_str(), "true" | "false") {
                Some(word)
            } else if ["true", "yes", "on", "1", "false", "no", "off", "0"].contains(&word.as_str())
            {
                Some(if truthy(&word) { "true" } else { "false" }.to_owned())
            } else {
                None
            }
        }
        (Shape::TextOrList, Node::Seq(items)) => {
            let items: Option<Vec<String>> = items.iter().map(|item| text(item, true)).collect();
            Some(format!("[{}]", items?.join(", ")))
        }
        (Shape::Names, Node::Map(pairs)) => {
            let pairs: Option<Vec<String>> = pairs
                .iter()
                .map(|(key, value)| Some(format!("{}: {}", text(key, true)?, text(value, true)?)))
                .collect();
            Some(format!("{{{}}}", pairs?.join(", ")))
        }
        _ => None,
    }
}

/// `text` as a YAML scalar that reads back as the same words: plain where it is plain words
/// that no reader would take for anything else, double-quoted otherwise ([`quoted`]). Inside a
/// flow list or mapping (`flow`), a comma is no plain word.
fn scalar(text: &str, flow: bool) -> String {
    let plain_words = !text.is_empty()
        && text.chars().next().is_some_and(char::is_alphabetic)
        && text
            .chars()
            .all(|c| c.is_alphanumeric() || " ./()+-_'".contains(c) || (c == ',' && !flow))
        && !text.ends_with(' ')
        && !text.contains("  ")
        && !["true", "false", "yes", "no", "on", "off", "null", "y", "n"]
            .iter()
            .any(|word| text.eq_ignore_ascii_case(word));
    if plain_words {
        text.to_owned()
    } else {
        quoted(text)
    }
}

/// `text` as a YAML double-quoted scalar: every character it can hold, the ones it cannot escaped.
fn quoted(text: &str) -> String {
    let mut out = String::with_capacity(text.len() + 2);
    out.push('"');
    for c in text.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if c.is_control() || matches!(c, '\u{85}' | '\u{2028}' | '\u{2029}' | '\u{feff}') => {
                out.push_str(&format!("\\u{:04x}", u32::from(c)));
            }
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

impl Frontmatter {
    /// Where the skill's body starts in the text it was read from: just after the closing `---`
    /// line.
    pub fn body_at(&self) -> usize {
        self.body
    }

    /// **The frontmatter purlis writes from it**, fences included: every kept field as written,
    /// each value written out again (never a key that widens the chat, nor one purlis knows no
    /// harness to read), and, where `without_description`, a manual-only skill's description
    /// left out. Refused where a kept field has a value it cannot take.
    pub fn emit(&self, honours: &crate::Honours) -> Result<String, Refusal> {
        let without_description = honours.manual == crate::ManualOnly::WithoutDescription;
        let drop_description = without_description && self.manual();
        let keep_agent = self.agent_kept(honours);
        let mut out = String::from("---\n");
        for (key, node) in &self.entries {
            let Some(field) = Field::of(key) else {
                continue;
            };
            let Some(shape) = field.kept() else {
                continue;
            };
            if (drop_description && field == Field::Description)
                || (field == Field::Agent && !keep_agent)
            {
                continue;
            }
            let value = written(node, &shape).ok_or(Refusal::BadValue(field))?;
            out.push_str(&format!("{}: {value}\n", field.key()));
        }
        out.push_str("---\n");
        Ok(out)
    }

    /// **Whether its `agent` is handed on to a harness that reads `honours`**: only for a fork
    /// (`context: fork`) as one of the harness's own agents. Any other agent, a repo's or the
    /// person's, may carry its own permissions and hooks, and would widen the chat.
    pub fn agent_kept(&self, honours: &crate::Honours) -> bool {
        self.fork()
            && self.entries.iter().any(|(key, node)| {
                Field::of(key) == Some(Field::Agent)
                    && matches!(node, Node::Scalar { text, .. } if honours.agents.contains(&text.trim()))
            })
    }
}

/// **The `SKILL.md` purlis hands a harness that reads `honours`, for a skill whose own is
/// `text`**: the frontmatter purlis writes ([`Frontmatter::emit`]), then the skill's body as it
/// is. Refused, handing nothing, where the frontmatter is refused ([`parse`]).
pub(crate) fn handed(text: &str, honours: &crate::Honours) -> Result<String, Refusal> {
    let read = parse(text)?;
    let mut out = read.emit(honours)?;
    out.push_str(&text[read.body..]);
    Ok(out)
}

#[cfg(test)]
mod tests;
