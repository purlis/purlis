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

/// A value of a YAML document, as purlis read it: no anchors, aliases or tags.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
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

/// **Reads the `SKILL.md` at `path`, under `root`, following no link**: refused where any
/// folder below `root` on the way to it or the file is a link, the file is no plain file, has a
/// second name or is larger than [`MOST_BYTES`]. Every folder below `root` is opened through the
/// handle of the one above and the file through its folder's ([`crate::SkillFolder`]), so a
/// link put in place after a look is never read through. The one reader of a skill's own file
/// for what purlis hands, badges and lints (SK-8).
pub fn read_skill_md(root: &std::path::Path, path: &std::path::Path) -> Result<String, Refusal> {
    crate::SkillFolder::open(root, path)?.skill_md()
}

/// **[`read_skill_md`] of a skill from `origin`** (SK-44): a harness skill is the person's own,
/// in their folders, where a `SKILL.md` that is a hard link is read (the Nix store, `cp -al` and
/// `rsync --link-dest` leave them). Every other skill's is refused, as [`read_skill_md`] refuses
/// it. Still never through a link, and of at most [`MOST_BYTES`].
pub fn read_skill_md_of(
    root: &std::path::Path,
    path: &std::path::Path,
    origin: crate::Origin,
) -> Result<String, Refusal> {
    crate::SkillFolder::open(root, path)?.skill_md_of(origin)
}

/// The `SKILL.md` named `name` in the skill's folder held at `folder`, as [`read_skill_md`]
/// reads one.
pub(crate) fn read_held(
    folder: &crate::held::Folder,
    name: impl AsRef<std::ffi::OsStr>,
) -> Result<String, Refusal> {
    read_held_as(folder, name, crate::held::HardLinks::Refused)
}

/// [`read_held`], a hard link read or refused as `hard_links` says.
pub(crate) fn read_held_as(
    folder: &crate::held::Folder,
    name: impl AsRef<std::ffi::OsStr>,
    hard_links: crate::held::HardLinks,
) -> Result<String, Refusal> {
    use crate::held::NotRead;
    folder
        .read_as(name, MOST_BYTES, hard_links)
        .map_err(|why| match why {
            NotRead::Unreadable(why) => Refusal::Unreadable(why),
            NotRead::NotPlain => Refusal::FileLink,
            NotRead::TooLarge => Refusal::TooLarge,
        })
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

/// **Where `text`'s frontmatter is, as bytes, and what it says as plain values** (#3866): the one
/// strict reader ([`document`]), for what is taken out of one before it leaves this machine. `None`
/// where it has none. Refused where it is not one document with plain keys, each set once. A
/// mapping is one only if it says so: text between two rules reads as a value that is no object. A
/// plain boolean, null or number is that value where it writes back the same, and anything else
/// is its text.
pub fn frontmatter_value(
    text: &str,
) -> Option<(std::ops::Range<usize>, Result<serde_json::Value, Refusal>)> {
    let (yaml, _) = fenced(text)?;
    let start = yaml.as_ptr() as usize - text.as_ptr() as usize;
    let read = document(yaml).and_then(|root| {
        if !keys_once(&root) {
            return Err(Refusal::Twice(String::new()));
        }
        as_value(&root)
    });
    Some((start..start + yaml.len(), read))
}

fn as_value(node: &Node) -> Result<serde_json::Value, Refusal> {
    use serde_json::Value;
    Ok(match node {
        Node::Scalar { text, plain: true } => match text.as_str() {
            "true" | "True" | "TRUE" => Value::Bool(true),
            "false" | "False" | "FALSE" => Value::Bool(false),
            "" | "~" | "null" => Value::Null,
            other => match serde_json::from_str::<serde_json::Number>(other) {
                Ok(number) if number.to_string() == other => Value::Number(number),
                _ => Value::String(other.to_owned()),
            },
        },
        Node::Scalar { text, .. } => Value::String(text.clone()),
        Node::Seq(items) => Value::Array(items.iter().map(as_value).collect::<Result<_, _>>()?),
        Node::Map(pairs) => {
            let mut map = serde_json::Map::with_capacity(pairs.len());
            for (key, value) in pairs {
                let Node::Scalar { text: key, .. } = key else {
                    return Err(Refusal::OddKey);
                };
                if key == "<<" {
                    return Err(Refusal::Reused);
                }
                map.insert(key.clone(), as_value(value)?);
            }
            Value::Object(map)
        }
    })
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
    let Node::Map(pairs) = document(yaml)? else {
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

/// **The one YAML document `yaml` is, read failing closed**: refused where it does not parse,
/// is not one document, reuses part of itself or carries a tag, or nests past [`MOST_NESTED`].
/// The one reader of untrusted YAML here, for a frontmatter ([`parse`]) and for a file a harness
/// reads beside a `SKILL.md` ([`flag_at`]).
fn document(yaml: &str) -> Result<Node, Refusal> {
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
    Ok(root)
}

/// **The boolean the YAML file `yaml` sets at `keys`, each a mapping's key inside the one
/// before** (SK-39), read by the one strict reader ([`document`]). `None` where it sets none so:
/// the file is refused, any mapping in it sets a key twice, a key on the way is missing, or the
/// last value is anything but a plain `true` or `false`. It fails closed, so a caller that acts
/// on one value acts only where the file says it plainly. It names no file: an adapter says
/// which one its harness reads.
pub fn flag_at(yaml: &str, keys: &[&str]) -> Option<bool> {
    let mut at = document(yaml).ok()?;
    if !keys_once(&at) {
        return None;
    }
    for key in keys {
        let Node::Map(pairs) = at else {
            return None;
        };
        at = pairs
            .into_iter()
            .find(|(k, _)| matches!(k, Node::Scalar { text, .. } if text == key))?
            .1;
    }
    match at {
        Node::Scalar { text, plain: true } if text == "true" => Some(true),
        Node::Scalar { text, plain: true } if text == "false" => Some(false),
        _ => None,
    }
}

/// Whether no mapping in `node` sets one key twice: a reader that refuses such a file, as a
/// harness's may, would read none of it. A scalar key is compared by its text alone, however it
/// is quoted, so `policy` and `"policy"` are one key. Each mapping's keys are checked in one
/// pass.
fn keys_once(node: &Node) -> bool {
    /// What a key is compared by: a scalar's text, or a whole list or mapping.
    #[derive(PartialEq, Eq, Hash)]
    enum Key<'a> {
        Text(&'a str),
        Whole(&'a Node),
    }
    match node {
        Node::Scalar { .. } => true,
        Node::Seq(items) => items.iter().all(keys_once),
        Node::Map(pairs) => {
            let mut seen = std::collections::HashSet::with_capacity(pairs.len());
            pairs.iter().all(|(key, value)| {
                let compared = match key {
                    Node::Scalar { text, .. } => Key::Text(text),
                    whole => Key::Whole(whole),
                };
                seen.insert(compared) && keys_once(key) && keys_once(value)
            })
        }
    }
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
        self.emit_named(honours, None)
    }

    /// [`Self::emit`], its `name` written as `name` where one is given: the name the set holds
    /// the skill by, where a route hands it under one of its own ([`crate::Route::built_in_name`]).
    fn emit_named(&self, honours: &crate::Honours, name: Option<&str>) -> Result<String, Refusal> {
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
            let value = match name {
                Some(name) if field == Field::Name => scalar(name, false),
                _ => written(node, &shape).ok_or(Refusal::BadValue(field))?,
            };
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
    handed_as(text, honours, None)
}

/// **`text`, a `SKILL.md`, with the frontmatter purlis writes for a harness that reads `honours`**
/// (SK-55): for a copy of a skill laid out by hand, as [`handed`] gives one that reaches a chat by a
/// set. Refused, as [`parse`] refuses it.
pub fn rewritten(text: &str, honours: &crate::Honours) -> Result<String, Refusal> {
    handed(text, honours)
}

/// [`handed`], its `name` written as `name` where one is given ([`Frontmatter::emit_named`]).
pub(crate) fn handed_as(
    text: &str,
    honours: &crate::Honours,
    name: Option<&str>,
) -> Result<String, Refusal> {
    let read = parse(text)?;
    let mut out = read.emit_named(honours, name)?;
    out.push_str(&text[read.body..]);
    Ok(out)
}

#[cfg(test)]
mod tests;
