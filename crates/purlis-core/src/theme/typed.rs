// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The tokens that are not colours** (#2327): a flag (`true` or `false`), a choice from a closed
//! list, or a number within its bound. A theme gives them under an appearance's `tokens` as it
//! gives a colour, and each is held to its own type as a colour is held to hex: a value outside
//! its type or bound refuses the theme and names the token.
//!
//! - **Flags** ([`FLAGS`]): whether each syntax role is italic and bold, whether the terminal's
//!   cursor blinks, and whether the terminal draws bold text in the bright ANSI colours.
//! - **The terminal cursor's shape** ([`CURSOR_STYLE`]), one of [`CURSOR_STYLES`].
//! - **The terminal's line height** ([`LINE_HEIGHT`]), a multiple of its text size within
//!   [`LINE_HEIGHTS`].
//!
//! What reaches the window is written from the parsed value, never from a file's text: a flag is
//! a boolean, a choice is the engine's own string, and a number is a number.

use std::collections::BTreeMap;

use serde_json::{Map, Value};

use super::stringify;
use super::vocabulary::{CURSOR_STYLE, CURSOR_STYLES, FLAGS, LINE_HEIGHT, LINE_HEIGHTS};

/// Every typed token's value, complete in a resolved theme.
#[derive(Debug, Clone, Default, PartialEq, serde::Serialize)]
pub struct Typed {
    /// Each of [`FLAGS`].
    pub flags: BTreeMap<&'static str, bool>,
    /// [`CURSOR_STYLE`], as one of [`CURSOR_STYLES`].
    pub choices: BTreeMap<&'static str, &'static str>,
    /// [`LINE_HEIGHT`].
    pub numbers: BTreeMap<&'static str, f64>,
}

impl Typed {
    /// The flag `token`, `false` when it is not one.
    pub fn flag(&self, token: &str) -> bool {
        self.flags.get(token).copied().unwrap_or_default()
    }

    /// The choice `token`, empty when it is not one.
    pub fn choice(&self, token: &str) -> &'static str {
        self.choices.get(token).copied().unwrap_or_default()
    }

    /// These values over `under`: each one given replaces `under`'s.
    pub fn onto(&self, under: &mut Typed) {
        under
            .flags
            .extend(self.flags.iter().map(|(token, on)| (*token, *on)));
        under
            .choices
            .extend(self.choices.iter().map(|(token, chosen)| (*token, *chosen)));
        under
            .numbers
            .extend(self.numbers.iter().map(|(token, number)| (*token, *number)));
    }

    /// The number `token`, `0` when it is not one.
    pub fn number(&self, token: &str) -> f64 {
        self.numbers.get(token).copied().unwrap_or_default()
    }
}

/// Every typed token, in the order a theme file is easiest to read in.
pub fn all() -> impl Iterator<Item = &'static str> {
    FLAGS.iter().copied().chain([CURSOR_STYLE, LINE_HEIGHT])
}

/// Whether `token` is a typed token.
pub fn is_typed(token: &str) -> bool {
    all().any(|it| it == token)
}

/// **Reads `value` as the typed token `token`** into `into`. `None` when `token` is not typed;
/// otherwise whether the value is one of its type, and the sentence that says why not, `at` saying
/// where the theme wrote it (`in appearance dark`).
pub(super) fn read(
    token: &str,
    value: &Value,
    at: &str,
    into: &mut Typed,
) -> Option<Result<(), String>> {
    if let Some(flag) = FLAGS.iter().copied().find(|it| *it == token) {
        return Some(match value {
            Value::Bool(on) => {
                into.flags.insert(flag, *on);
                Ok(())
            }
            other => Err(format!(
                "{token} {at} is {}, which is not true or false",
                stringify(other)
            )),
        });
    }
    if token == CURSOR_STYLE {
        let chosen = value
            .as_str()
            .and_then(|given| CURSOR_STYLES.iter().copied().find(|it| *it == given));
        return Some(match chosen {
            Some(style) => {
                into.choices.insert(CURSOR_STYLE, style);
                Ok(())
            }
            None => Err(format!(
                "{token} {at} is {}, which is not one of \"{}\"",
                stringify(value),
                CURSOR_STYLES.join("\", \"")
            )),
        });
    }
    if token == LINE_HEIGHT {
        let (least, most) = LINE_HEIGHTS;
        return Some(match value.as_f64() {
            Some(number) if (least..=most).contains(&number) => {
                into.numbers.insert(LINE_HEIGHT, number);
                Ok(())
            }
            _ => Err(format!(
                "{token} {at} is {}, which is not a number from {least} to {most}",
                stringify(value)
            )),
        });
    }
    None
}

/// A built-in's typed values, every one present and of its type, or the first it lacks.
pub(super) fn complete(given: Option<&Map<String, Value>>) -> Result<Typed, &'static str> {
    let mut typed = Typed::default();
    for token in all() {
        let given = given.and_then(|it| it.get(token)).ok_or(token)?;
        match read(token, given, "", &mut typed) {
            Some(Ok(())) => {}
            _ => return Err(token),
        }
    }
    Ok(typed)
}
