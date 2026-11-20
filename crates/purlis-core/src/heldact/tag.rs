// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A tag's name**, checked as git checks a ref's (`git check-ref-format`), so a name that reaches
//! a held act is one the forge and git read the same way.

/// A tag name was refused as part of a held act. It names the part, never the value.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("this is not a tag name a held act can name")]
pub struct InvalidTag;

/// **A tag's name**, without `refs/tags/`: 1 to 255 bytes, none of the characters git refuses in a
/// ref (**nor a zero-width, direction or other character that draws nothing**, which would make a
/// name read as another), no component starting with `.` or ending in `.lock`, and not a commit's own 40 or 64 digit
/// spelling (a tag named like a commit is read two ways).
#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct TagName(String);

impl TagName {
    /// The name, as checked.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// The ref the name is, as a forge writes it.
    pub fn full_ref(&self) -> String {
        format!("refs/tags/{}", self.0)
    }

    /// The name in `ref_name` where it is a tag's (`refs/tags/v1`), `None` for any other ref.
    pub fn from_ref(ref_name: &str) -> Option<Result<TagName, InvalidTag>> {
        ref_name.strip_prefix("refs/tags/").map(TagName::try_from)
    }
}

impl TryFrom<String> for TagName {
    type Error = InvalidTag;

    fn try_from(value: String) -> Result<Self, InvalidTag> {
        if is_tag(&value) {
            Ok(Self(value))
        } else {
            Err(InvalidTag)
        }
    }
}

impl TryFrom<&str> for TagName {
    type Error = InvalidTag;

    fn try_from(value: &str) -> Result<Self, InvalidTag> {
        Self::try_from(value.to_owned())
    }
}

impl From<TagName> for String {
    fn from(value: TagName) -> String {
        value.0
    }
}

fn is_tag(name: &str) -> bool {
    if name.is_empty()
        || name.len() > 255
        || name.starts_with(['-', '/'])
        || name.ends_with(['/', '.'])
        || name == "@"
        || name.contains("..")
        || name.contains("//")
        || name.contains("@{")
        || name.chars().any(|c| {
            c.is_whitespace()
                || c.is_control()
                || super::prepare::unseen(c)
                || "~^:?*[\\".contains(c)
        })
    {
        return false;
    }
    // A name that is a whole commit id, by either hash.
    if matches!(name.len(), 40 | 64) && name.bytes().all(|b| b.is_ascii_hexdigit()) {
        return false;
    }
    name.split('/')
        .all(|part| !part.starts_with('.') && !part.ends_with(".lock"))
}
