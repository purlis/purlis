// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A skill's own text, made and changed** (SK-7, #2770): the rules a new skill's name and
//! description keep, the templates a new skill starts from, the live lint the light editor shows,
//! and the text of a rename, in the skill itself and in every file that names it.
//!
//! Nothing here touches a disk: purlis-core moves the folders and commits them, and asks this
//! module what each file says afterwards, so the window and the command line follow one rule.

/// The longest name the open SKILL.md format allows.
pub const MOST_NAME: usize = 64;

/// The longest description the open SKILL.md format allows.
pub const MOST_DESCRIPTION: usize = 1024;

/// What is wrong with `name` as a skill's name, as the end of a sentence (`"it holds a capital
/// letter"`), or `None` where it is one: 1 to 64 lowercase letters, digits and hyphens, with no
/// hyphen first, last or twice in a row, as the open SKILL.md format says.
pub fn name_problem(name: &str) -> Option<String> {
    if name.is_empty() {
        return Some("it is empty".to_owned());
    }
    if name.chars().count() > MOST_NAME {
        return Some(format!("it is longer than {MOST_NAME} characters"));
    }
    if let Some(odd) = name
        .chars()
        .find(|c| !(c.is_ascii_lowercase() || c.is_ascii_digit() || *c == '-'))
    {
        return Some(format!(
            "it holds {odd:?}, and a name is lowercase letters, digits and hyphens"
        ));
    }
    if name.starts_with('-') || name.ends_with('-') {
        return Some("it starts or ends with a hyphen".to_owned());
    }
    if name.contains("--") {
        return Some("it holds two hyphens in a row".to_owned());
    }
    None
}

/// What is wrong with `description` as a skill's description, or `None` where it is one: one
/// line, not empty, at most 1024 characters, and nothing a reader of the frontmatter as YAML
/// would take for something else.
pub fn description_problem(description: &str) -> Option<String> {
    let trimmed = description.trim();
    if trimmed.is_empty() {
        return Some(
            "it is empty, and a description is how a chat knows when to use it".to_owned(),
        );
    }
    if trimmed.contains(['\n', '\r']) {
        return Some("it is more than one line".to_owned());
    }
    if trimmed.chars().count() > MOST_DESCRIPTION {
        return Some(format!("it is longer than {MOST_DESCRIPTION} characters"));
    }
    if trimmed.contains(": ") || trimmed.contains(" #") {
        return Some(
            "it holds \": \" or \" #\", which would be read as the start of something else: \
             put a dash or a comma there"
                .to_owned(),
        );
    }
    if trimmed.starts_with([
        '"', '\'', '[', ']', '{', '}', '>', '|', '*', '&', '!', '%', '@', '`', '#', ',', '?', '-',
    ]) {
        return Some(format!(
            "it starts with {:?}, which would be read as something other than words",
            trimmed.chars().next().unwrap_or_default()
        ));
    }
    None
}

/// **Which template a new skill starts from**: one of purlis's own, or one an extension offers,
/// by the extension's id and the template's.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum TemplateId {
    BuiltIn(Template),
    Extension { extension: String, template: String },
}

/// One template a new skill can start from, as the studio offers it: the template, what a person
/// is shown, and what it gives in one line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Offer {
    pub id: TemplateId,
    pub label: String,
    pub about: String,
}

/// **What a new skill starts from** (the SkillTemplate capability, ADR 0101): the templates it
/// offers, and the text of a new skill made from one of them. purlis's own ([`BuiltInTemplates`])
/// and each extension's that declares `skill-template` answer through this same interface.
pub trait SkillTemplate {
    /// Every template it offers, in the order the studio offers them.
    fn offered(&self) -> Vec<Offer>;
    /// The `SKILL.md` of a new skill named `name`, described by `description`, from the template
    /// `id`, or why it has none.
    fn text(&self, id: &TemplateId, name: &str, description: &str) -> Result<String, String>;
}

/// **purlis's own templates** ([`Template`]), the SkillTemplate capability's first user.
#[derive(Debug, Clone, Copy, Default)]
pub struct BuiltInTemplates;

impl SkillTemplate for BuiltInTemplates {
    fn offered(&self) -> Vec<Offer> {
        Template::ALL
            .iter()
            .map(|template| Offer {
                id: TemplateId::BuiltIn(*template),
                label: template.label().to_owned(),
                about: template.about().to_owned(),
            })
            .collect()
    }

    fn text(&self, id: &TemplateId, name: &str, description: &str) -> Result<String, String> {
        match id {
            TemplateId::BuiltIn(template) => Ok(template.text(name, description)),
            TemplateId::Extension { extension, .. } => Err(format!(
                "purlis has no template of the extension '{extension}' here"
            )),
        }
    }
}

/// What a new skill starts from (decision 13: blank from a template). The built-in templates are
/// the SkillTemplate capability's first user ([`BuiltInTemplates`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Template {
    /// The frontmatter and one heading.
    Blank,
    /// When to use it, its steps, and how to check the work.
    Steps,
}

impl Template {
    /// Every template, in the order the studio offers them.
    pub const ALL: [Template; 2] = [Template::Blank, Template::Steps];

    /// What a person is shown for it.
    pub fn label(self) -> &'static str {
        match self {
            Self::Blank => "Blank",
            Self::Steps => "Steps to follow",
        }
    }

    /// What it gives, in one line.
    pub fn about(self) -> &'static str {
        match self {
            Self::Blank => "The name, the description and an empty page.",
            Self::Steps => "When to use it, the steps a chat follows, and how it checks the work.",
        }
    }

    /// The text of a new skill named `name`, described by `description`, made from it.
    pub fn text(self, name: &str, description: &str) -> String {
        let head = format!(
            "---\nname: {name}\ndescription: {}\n---\n\n# {name}\n",
            description.trim()
        );
        match self {
            Self::Blank => head,
            Self::Steps => format!(
                "{head}\n## When to use it\n\n{}\n\n## Steps\n\n1. \n\n## Check the work\n\n- \n",
                description.trim()
            ),
        }
    }
}

/// One thing the live lint says about a skill's `SKILL.md` as it is typed. Its
/// [`std::fmt::Display`] is the line the light editor shows.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Problem {
    /// The text does not start with a `---` frontmatter block.
    NoFrontmatter,
    /// The frontmatter names no `name`.
    NoName,
    /// The frontmatter names another name than the skill's: a rename is its own change.
    NameChanged { was: String, now: String },
    /// The frontmatter names no `description`.
    NoDescription,
    /// The description breaks a rule, said as [`description_problem`] says it.
    Description(String),
}

impl std::fmt::Display for Problem {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NoFrontmatter => f.write_str(
                "it does not start with the frontmatter: a line of ---, then name: and \
                 description:, then ---",
            ),
            Self::NoName => f.write_str("its frontmatter names no name:"),
            Self::NameChanged { was, now } => write!(
                f,
                "its name: says {now}, and this skill is {was}: rename it with Rename, which \
                 changes every persona and skill that names it"
            ),
            Self::NoDescription => f.write_str(
                "its frontmatter names no description:, so no chat would know when to use it",
            ),
            Self::Description(why) => write!(f, "its description: {why}"),
        }
    }
}

/// **The live lint** of `text`, a `SKILL.md` being edited for the skill named `name`: every
/// [`Problem`] it has, none where it is a whole skill that keeps its name. A save is refused while
/// there is one.
pub fn check(text: &str, name: &str) -> Vec<Problem> {
    if crate::read::frontmatter_block(text).is_none() {
        return vec![Problem::NoFrontmatter];
    }
    let pairs = crate::frontmatter(text);
    let value = |key: &str| {
        pairs
            .iter()
            .find(|(k, _)| k == key)
            .map(|(_, v)| v.as_str())
            .filter(|v| !v.is_empty())
    };
    let mut problems = Vec::new();
    match value("name") {
        None => problems.push(Problem::NoName),
        Some(now) if now != name => problems.push(Problem::NameChanged {
            was: name.to_owned(),
            now: now.to_owned(),
        }),
        Some(_) => {}
    }
    match value("description") {
        None => problems.push(Problem::NoDescription),
        Some(description) => {
            if let Some(why) = description_problem(description) {
                problems.push(Problem::Description(why));
            }
        }
    }
    problems
}

/// `text`, a `SKILL.md`, with its frontmatter's `name:` line saying `new`, every other byte as it
/// was. `None` where its frontmatter has no `name:` line.
pub fn with_name(text: &str, new: &str) -> Option<String> {
    let (start, end) = crate::read::frontmatter_block(text)?;
    let mut out = String::with_capacity(text.len() + new.len());
    out.push_str(&text[..start]);
    let mut found = false;
    for line in text[start..end].split_inclusive('\n') {
        let (body, ending) = split_ending(line);
        match body.split_once(':') {
            Some((key, _)) if !found && key.trim() == "name" => {
                found = true;
                let indent = &body[..body.len() - body.trim_start().len()];
                out.push_str(&format!("{indent}name: {new}{ending}"));
            }
            _ => out.push_str(line),
        }
    }
    out.push_str(&text[end..]);
    found.then_some(out)
}

/// Where each reference to a skill in the namespace `namespace` (`skills:<name>`) is in `text`,
/// as byte offsets of its name: the namespace not preceded by a word character, the name a run of
/// the characters a skill's name may hold. The one rule a reference is read by.
fn every_reference(text: &str, namespace: &str) -> Vec<(usize, usize)> {
    let needle = format!("{namespace}:");
    let word = |c: char| c.is_ascii_alphanumeric() || "_.-".contains(c);
    let named = |c: char| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-';
    text.match_indices(&needle)
        .filter(|(at, _)| !text[..*at].chars().next_back().is_some_and(word))
        .map(|(at, _)| {
            let start = at + needle.len();
            let len = text[start..]
                .find(|c: char| !named(c))
                .unwrap_or(text.len() - start);
            (start, start + len)
        })
        .filter(|(start, end)| start < end)
        .collect()
}

/// Where each reference to the skill `name` in the namespace `namespace` (`skills:name`) is in
/// `text` ([`every_reference`]), so `myskills:name` and `skills:named` are not references.
fn reference_spans(text: &str, namespace: &str, name: &str) -> Vec<(usize, usize)> {
    every_reference(text, namespace)
        .into_iter()
        .filter(|(start, end)| &text[*start..*end] == name)
        .collect()
}

/// **Every skill `text` refers to in `namespace`**, by name, each once, in the order first named.
pub fn referenced(text: &str, namespace: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for (start, end) in every_reference(text, namespace) {
        let name = &text[start..end];
        if !out.iter().any(|one| one == name) {
            out.push(name.to_owned());
        }
    }
    out
}

/// `text` with every reference to the skill `old` in `namespace` (`skills:old`) naming `new`
/// ([`reference_spans`]).
pub fn references_renamed(text: &str, namespace: &str, old: &str, new: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut from = 0;
    for (start, end) in reference_spans(text, namespace, old) {
        out.push_str(&text[from..start]);
        out.push_str(new);
        from = end;
    }
    out.push_str(&text[from..]);
    out
}

/// Whether `text` holds a reference to the skill `name` in `namespace` ([`reference_spans`]).
pub fn references(text: &str, namespace: &str, name: &str) -> bool {
    !reference_spans(text, namespace, name).is_empty()
}

/// Each item of a persona's `skills:` list in `text`, as byte offsets into it: the items of its
/// one line (`skills: a, b`), or of a block list under it (`skills:` then `  - a` lines), within
/// its frontmatter. The one place the list's items are found, for its rename and its readers.
fn list_items(text: &str) -> Vec<(usize, usize)> {
    let Some((start, end)) = crate::read::frontmatter_block(text) else {
        return Vec::new();
    };
    let mut items = Vec::new();
    let mut at = start;
    let mut in_block = false;
    for line in text[start..end].split_inclusive('\n') {
        let (body, _) = split_ending(line);
        let trimmed = body.trim_start();
        if in_block && let Some(item) = trimmed.strip_prefix('-') {
            let lead = body.len() - trimmed.len() + 1;
            items.push(span_of(at + lead, item));
        } else if let Some((key, rest)) = body.split_once(':')
            && key.trim() == "skills"
        {
            let mut offset = at + key.len() + 1;
            for item in rest.split(',') {
                items.push(span_of(offset, item));
                offset += item.len() + 1;
            }
            in_block = rest.trim().is_empty();
        } else if !trimmed.is_empty() {
            in_block = false;
        }
        at += line.len();
    }
    items.retain(|(a, b)| a < b);
    items
}

/// The span of `item`'s words, `item` starting at `at`: what is left once its blanks are taken off.
fn span_of(at: usize, item: &str) -> (usize, usize) {
    let lead = item.len() - item.trim_start().len();
    (at + lead, at + lead + item.trim().len())
}

/// Whether a list item names the skill `name`, bare or as `namespace:name`.
fn item_names(item: &str, namespace: &str, name: &str) -> bool {
    item == name
        || item
            .strip_prefix(namespace)
            .and_then(|rest| rest.strip_prefix(':'))
            == Some(name)
}

/// `text`, a persona's definition, with each item of its frontmatter's `skills:` list that names
/// `old` (bare, or as `namespace:old`), on its one line or in a block list, naming `new` the same
/// way. Every other byte is kept.
pub fn persona_list_renamed(text: &str, namespace: &str, old: &str, new: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut from = 0;
    for (start, end) in list_items(text) {
        let item = &text[start..end];
        if !item_names(item, namespace, old) {
            continue;
        }
        out.push_str(&text[from..start]);
        out.push_str(&if item == old {
            new.to_owned()
        } else {
            format!("{namespace}:{new}")
        });
        from = end;
    }
    out.push_str(&text[from..]);
    out
}

/// Whether `text`, a persona's definition, names the skill `name` in its frontmatter's `skills:`
/// list, bare or as `namespace:name`.
pub fn persona_names(text: &str, namespace: &str, name: &str) -> bool {
    list_items(text)
        .into_iter()
        .any(|(start, end)| item_names(&text[start..end], namespace, name))
}

/// A line and the line break it ends with (`\n`, `\r\n`, or none).
fn split_ending(line: &str) -> (&str, &str) {
    if let Some(body) = line.strip_suffix("\r\n") {
        (body, "\r\n")
    } else if let Some(body) = line.strip_suffix('\n') {
        (body, "\n")
    } else {
        (line, "")
    }
}

#[cfg(test)]
mod tests;
