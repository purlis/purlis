// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **Names every platform holds** (spec #2313, #2588): a theme made on one platform opens on
//! every other, so a theme folder's name and the name of every file in it is one Windows can
//! hold too, on every platform.
//!
//! Windows keeps the device names `CON`, `PRN`, `AUX`, `NUL`, `CONIN$`, `CONOUT$`,
//! `COM0`–`COM9` and `LPT0`–`LPT9` (and `COM¹`–`COM³`, `LPT¹`–`LPT³`) in any case, alone or
//! before an extension (`nul.json`), drops a dot or a space at the end of a name, and holds no
//! name with `<`, `>`, `:`, `"`, `/`, `\`, `|`, `?`, `*` or a control character in it (#2671).
//! The device table and the characters are the ones a workspace's new name is held to
//! (`crate::contain::mintable`); this is the one place a theme asks them: the theme folder's
//! name grammar ([`super::is_folder_name`]), import, export, the stage and the readers of a
//! theme folder and of the files it names.

/// Why Windows cannot hold a name.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NotHeld {
    /// One of the device names, alone or before an extension.
    Device,
    /// It ends in a dot, which Windows drops.
    Dot,
    /// It ends in a space, which Windows drops.
    Space,
    /// It has a character in it that Windows holds in no name.
    Character(char),
}

impl NotHeld {
    /// What is wrong with the name, as the end of a sentence about it.
    fn says(self) -> String {
        match self {
            Self::Device => "is a name Windows keeps for a device (CON, PRN, AUX, NUL, CONIN$, \
                             CONOUT$, COM0 to COM9, LPT0 to LPT9, with or without an extension)"
                .to_owned(),
            Self::Dot => "ends in a dot that Windows drops".to_owned(),
            Self::Space => "ends in a space that Windows drops".to_owned(),
            Self::Character(c) if c.is_control() => format!(
                "has the control character U+{:04X} in it, which Windows holds in no name",
                u32::from(c)
            ),
            Self::Character(c) => format!("has {c} in it, which Windows holds in no name"),
        }
    }
}

/// **Why Windows cannot hold the one file or folder name `name`**, or `None` when it can.
/// Asked of one name, never of a path.
pub fn not_held(name: &str) -> Option<NotHeld> {
    if let Some(c) = crate::contain::unspellable(name) {
        return Some(NotHeld::Character(c));
    }
    if name.ends_with('.') {
        return Some(NotHeld::Dot);
    }
    if name.ends_with(' ') {
        return Some(NotHeld::Space);
    }
    // Windows reads the device in front of the first dot, without the spaces before it: the
    // device table a workspace's new name is held to.
    crate::contain::dos_device(name).then_some(NotHeld::Device)
}

/// **The sentence that refuses `name`**, one name of what `subject` says (`subject` is the
/// name itself, or the path or the entry that holds it), or `None` when Windows can hold it.
pub fn refusal(subject: &str, name: &str) -> Option<String> {
    let why = not_held(name)?;
    let shown = crate::shown::short(name);
    Some(if subject == name {
        format!("{shown} {}, so no theme holds it", why.says())
    } else {
        format!(
            "{} names {shown}, which {}, so no theme holds it",
            crate::shown::short(subject),
            why.says()
        )
    })
}

/// **The sentence that refuses the path `path`** (names joined by `/`), said of `subject`, for
/// its first name Windows cannot hold; `None` when it can hold every one.
pub fn path_refusal(subject: &str, path: &str) -> Option<String> {
    path.split('/')
        .find_map(|name| not_held(name).is_some().then(|| refusal(subject, name)))
        .flatten()
}
