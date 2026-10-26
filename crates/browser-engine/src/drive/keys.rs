// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! A key or a chord as the agent names it (`Enter`, `Shift+Tab`, `Control+a`), as the engine's
//! key events (`Input.dispatchKeyEvent`).

use serde_json::{Map, Value, json};

/// A key purlis does not know.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error(
    "`{0}` is not a key purlis knows: name one such as Enter, Tab, Escape, ArrowDown, Shift+Tab or Control+a"
)]
pub(crate) struct NotAKey(pub String);

/// A modifier: its name, code, key code and bit in the engine's `modifiers`.
const MODIFIERS: &[(&str, &str, i64, i64)] = &[
    ("Alt", "AltLeft", 18, 1),
    ("Control", "ControlLeft", 17, 2),
    ("Meta", "MetaLeft", 91, 4),
    ("Shift", "ShiftLeft", 16, 8),
];

/// Named keys: name, code, key code, and the text the key types, if any.
const NAMED: &[(&str, &str, i64, Option<&str>)] = &[
    ("Enter", "Enter", 13, Some("\r")),
    ("Tab", "Tab", 9, None),
    ("Escape", "Escape", 27, None),
    ("Backspace", "Backspace", 8, None),
    ("Delete", "Delete", 46, None),
    ("Insert", "Insert", 45, None),
    ("ArrowLeft", "ArrowLeft", 37, None),
    ("ArrowUp", "ArrowUp", 38, None),
    ("ArrowRight", "ArrowRight", 39, None),
    ("ArrowDown", "ArrowDown", 40, None),
    ("Home", "Home", 36, None),
    ("End", "End", 35, None),
    ("PageUp", "PageUp", 33, None),
    ("PageDown", "PageDown", 34, None),
    ("Space", "Space", 32, Some(" ")),
    ("F1", "F1", 112, None),
    ("F2", "F2", 113, None),
    ("F3", "F3", 114, None),
    ("F4", "F4", 115, None),
    ("F5", "F5", 116, None),
    ("F6", "F6", 117, None),
    ("F7", "F7", 118, None),
    ("F8", "F8", 119, None),
    ("F9", "F9", 120, None),
    ("F10", "F10", 121, None),
    ("F11", "F11", 122, None),
    ("F12", "F12", 123, None),
];

/// Editing commands a chord with Control or Meta carries, so they work on every platform.
const COMMANDS: &[(&str, &str)] = &[
    ("a", "selectAll"),
    ("c", "copy"),
    ("x", "cut"),
    ("v", "paste"),
    ("z", "undo"),
];

/// The editing command a chord with Control or Meta on `key` carries (`a`: `selectAll`).
pub(crate) fn command_of(key: &str) -> Option<&'static str> {
    COMMANDS
        .iter()
        .find(|(k, _)| k.eq_ignore_ascii_case(key))
        .map(|(_, command)| *command)
}

/// Whether `key` only modifies another key and moves no focus (Shift, Control, Alt, AltGr,
/// Meta, Caps Lock, Fn): one pressed keeps a read of the page made for a key before it (#3043).
pub(crate) fn is_modifier(key: &str) -> bool {
    [
        "Shift", "Control", "Alt", "AltGraph", "Meta", "CapsLock", "Fn",
    ]
    .contains(&key)
}

struct Key {
    key: String,
    code: String,
    key_code: i64,
    text: Option<String>,
}

fn key(name: &str, shifted: bool) -> Option<Key> {
    if let Some((key, code, key_code, text)) = NAMED.iter().find(|n| n.0 == name) {
        let key = if *key == "Space" { " " } else { key };
        return Some(Key {
            key: key.to_owned(),
            code: (*code).to_owned(),
            key_code: *key_code,
            text: text.map(str::to_owned),
        });
    }
    let mut chars = name.chars();
    let (Some(c), None) = (chars.next(), chars.next()) else {
        return None;
    };
    if c.is_ascii_alphabetic() {
        let upper = c.to_ascii_uppercase();
        let typed = if shifted { upper } else { c };
        return Some(Key {
            key: typed.to_string(),
            code: format!("Key{upper}"),
            key_code: i64::from(u32::from(upper)),
            text: Some(typed.to_string()),
        });
    }
    if c.is_ascii_digit() {
        return Some(Key {
            key: c.to_string(),
            code: format!("Digit{c}"),
            key_code: i64::from(u32::from(c)),
            text: Some(c.to_string()),
        });
    }
    if c.is_control() {
        return None;
    }
    Some(Key {
        key: c.to_string(),
        code: String::new(),
        key_code: 0,
        text: Some(c.to_string()),
    })
}

/// The engine's key events for `chord`: each modifier down, the key down and up, each
/// modifier up.
pub(crate) fn key_events(chord: &str) -> Result<Vec<Value>, NotAKey> {
    let refused = || NotAKey(chord.to_owned());
    let parts: Vec<&str> = chord.split('+').collect();
    // `+` alone, or a chord ending in `++`, is the plus key.
    let (held, last) = match parts.as_slice() {
        [] => return Err(refused()),
        [.., "", ""] => (&parts[..parts.len() - 2], "+"),
        [held @ .., last] => (held, *last),
    };
    if last.is_empty() {
        return Err(refused());
    }
    let mut modifiers = Vec::new();
    for name in held {
        let modifier = MODIFIERS
            .iter()
            .find(|m| m.0 == *name)
            .ok_or_else(refused)?;
        modifiers.push(modifier);
    }
    let bits = |upto: usize| modifiers[..upto].iter().map(|m| m.3).sum::<i64>();
    let all = bits(modifiers.len());
    let shifted = all & 8 != 0;
    let commanded = all & (2 | 4) != 0;
    let key = key(last, shifted).ok_or_else(refused)?;

    let event = |kind: &str, key: &str, code: &str, key_code: i64, modifiers: i64| {
        let mut e = Map::new();
        e.insert("type".into(), kind.into());
        e.insert("key".into(), key.into());
        e.insert("code".into(), code.into());
        e.insert("windowsVirtualKeyCode".into(), key_code.into());
        e.insert("modifiers".into(), modifiers.into());
        e
    };
    let mut out = Vec::new();
    for (at, m) in modifiers.iter().enumerate() {
        out.push(Value::Object(event(
            "rawKeyDown",
            m.0,
            m.1,
            m.2,
            bits(at + 1),
        )));
    }
    let text = key.text.clone().filter(|_| all & (1 | 2 | 4) == 0);
    let mut down = event(
        if text.is_some() {
            "keyDown"
        } else {
            "rawKeyDown"
        },
        &key.key,
        &key.code,
        key.key_code,
        all,
    );
    if let Some(text) = &text {
        down.insert("text".into(), text.clone().into());
        down.insert("unmodifiedText".into(), text.clone().into());
    }
    if commanded
        && let Some((_, command)) = COMMANDS
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case(&key.key))
    {
        down.insert("commands".into(), json!([command]));
    }
    out.push(Value::Object(down));
    out.push(Value::Object(event(
        "keyUp",
        &key.key,
        &key.code,
        key.key_code,
        all,
    )));
    for (at, m) in modifiers.iter().enumerate().rev() {
        out.push(Value::Object(event("keyUp", m.0, m.1, m.2, bits(at))));
    }
    Ok(out)
}

#[cfg(test)]
mod tests;
