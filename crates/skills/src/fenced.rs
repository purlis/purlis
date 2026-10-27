// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A frontmatter's top-level entries, read as the YAML they are, with the line each starts
//! on** (#3244): for a writer that changes one key and keeps every other line as written, byte
//! for byte. The frontmatter is fenced as a skill's is (a first line that is `---`, to the next
//! line that is `---`), so a `---` inside a value never ends it, and it is read by the one strict
//! reader of untrusted YAML here ([`crate::handed`]): it fails closed where it does not parse, is
//! not one block mapping of plain names, reuses part of itself, carries a tag or sets a key
//! twice.

use std::ops::Range;

use yaml_rust2::parser::{Event, Parser};

use crate::Refusal;
use crate::handed::{Node, node};

/// Where a frontmatter is in a text: the YAML between its fences, and where the body after the
/// closing one starts, as byte offsets.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Fence {
    pub yaml: Range<usize>,
    pub body: usize,
}

/// **Where `text`'s frontmatter is**, or `None` where it does not start with one.
pub fn fence(text: &str) -> Option<Fence> {
    crate::handed::fence(text).map(|(yaml, body)| Fence { yaml, body })
}

/// A value of a frontmatter, as read: compared whole, so a writer can tell that every other
/// key reads as it did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Value(Node);

impl Value {
    /// The words it is, where it is one scalar. A plain empty value (`key:`, `~`, `null`) is no
    /// words.
    pub fn text(&self) -> Option<&str> {
        match &self.0 {
            Node::Scalar { text, plain: true } if matches!(text.as_str(), "~" | "null" | "") => {
                Some("")
            }
            Node::Scalar { text, .. } => Some(text),
            _ => None,
        }
    }

    /// The words of each item, where it is a list of scalars.
    pub fn items(&self) -> Option<Vec<&str>> {
        match &self.0 {
            Node::Seq(items) => items
                .iter()
                .map(|item| match item {
                    Node::Scalar { text, .. } => Some(text.as_str()),
                    _ => None,
                })
                .collect(),
            _ => None,
        }
    }
}

/// One top-level entry of a frontmatter: its key, the line of the YAML it starts on (from 0),
/// and its value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    pub key: String,
    pub line: usize,
    pub value: Value,
}

/// **Every top-level entry of the frontmatter YAML `yaml`, in the order written**, failing
/// closed ([`Refusal`]): one block mapping of plain names, each set once, nothing reused and no
/// tag. A YAML of only blanks and comments has none.
pub fn entries(yaml: &str) -> Result<Vec<Entry>, Refusal> {
    let first = yaml
        .lines()
        .map(str::trim_start)
        .find(|line| !line.is_empty() && !line.starts_with('#'));
    match first {
        None => return Ok(Vec::new()),
        // A flow mapping (`{a: b}`) is no block a line can be changed in.
        Some(line) if line.starts_with('{') => return Err(Refusal::NotAMapping),
        Some(_) => {}
    }
    let mut parser = Parser::new_from_str(yaml);
    let mut pull = || {
        parser
            .next_token()
            .map_err(|why| Refusal::NotYaml(why.info().to_owned()))
    };
    if pull()?.0 != Event::StreamStart || pull()?.0 != Event::DocumentStart {
        return Err(Refusal::NotAMapping);
    }
    match pull()?.0 {
        Event::MappingStart(0, None) => {}
        Event::MappingStart(0, Some(_)) => return Err(Refusal::Tagged),
        Event::MappingStart(..) => return Err(Refusal::Reused),
        _ => return Err(Refusal::NotAMapping),
    }
    let mut out: Vec<Entry> = Vec::new();
    loop {
        let (event, at) = pull()?;
        if event == Event::MappingEnd {
            break;
        }
        let mut next = || pull().map(|(event, _)| event);
        let Node::Scalar { text: key, .. } = node(event, &mut next, 1)? else {
            return Err(Refusal::OddKey);
        };
        if key == "<<" {
            return Err(Refusal::Reused);
        }
        if key.is_empty() || key.contains(['\n', '\r', ':']) {
            return Err(Refusal::OddKey);
        }
        if out.iter().any(|had| had.key == key) {
            return Err(Refusal::Twice(key));
        }
        let value = node(next()?, &mut next, 1)?;
        out.push(Entry {
            key,
            line: at.line().saturating_sub(1),
            value: Value(value),
        });
    }
    if pull()?.0 != Event::DocumentEnd || pull()?.0 != Event::StreamEnd {
        return Err(Refusal::NotAMapping);
    }
    Ok(out)
}

#[cfg(test)]
mod tests;
