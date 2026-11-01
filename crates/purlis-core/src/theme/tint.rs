// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A workspace's colour, as a hue shift** (charter-app#281): the arithmetic that turns the
//! tokens a theme's [`Tint`] names to a workspace's hue, so [`super::check`] can hold a theme's
//! pairs at every hue a workspace can name, and the recipe itself (#2329): whether a theme is
//! tinted at all, which tokens, and the hue each workspace colour gives.
//!
//! The shift is done in OKLCH, and then the lightness is searched for until the tinted colour
//! has **the relative luminance the original had**, and each channel is rounded to its byte on
//! the side of that luminance away from the middle ([`rounded`]), so every contrast ratio a
//! theme cleared between a dark shade and a light one its tinted shades clear too. A grey is given
//! [`NEUTRAL_CHROMA`]; a colour keeps its own chroma, lowered (never the lightness) until the
//! screen can show it.
//!
//! The window is handed each resolved theme's recipe and draws the same tints live
//! (`app/src/theme/tint.ts` has the same arithmetic), and the two are held to one fixture of
//! inputs and answers (`theme/tint-cases.json`), which both test suites read.
//!
//! # A theme's `tint`
//!
//! ```json
//! "tint": { "on": true, "tokens": ["accent.base", "surface.raised"], "hues": { "red": "#c0392b" } }
//! ```
//!
//! - **`on`**, `true` or `false`: off, a workspace's colour tints nothing in this theme.
//! - **`tokens`**, the tokens a workspace's colour turns, from [`TINTABLE`]. It replaces the
//!   parent's list; a token outside [`TINTABLE`] refuses the theme, and a name purlis does not
//!   have is said and left out. The strip shades ([`TAB_ONLY`]) are tinted on a workspace's own
//!   tab and chat strip; the rest on the whole window too, from the workspace in front.
//! - **`hues`**, a workspace colour's name to a hex colour or a name in the theme's palette,
//!   whose OKLCH hue that colour gives in this theme. A colour it does not name keeps its
//!   parent's hue, and a workspace's own `#rrggbb` is always its own hue.
//!
//! Each key a theme does not give is its parent's, and a theme with no parent starts from
//! Purlis Classic's recipe: on, [`TINTED`], and [`PALETTE`]'s hues.

use std::collections::BTreeMap;

use serde_json::Value;

use super::floors::{ALLOW_APART, DANGER_APART, WAITING_APART};
use super::vocabulary::{ACCENT_WORDS, TAB_ONLY, TINTABLE, TINTED, TOKENS, is_colour};
use super::{Complaint, ResolvedTheme, stringify};

/// The palette a workspace names its colour from, as OKLCH hues in degrees, in
/// [`crate::extension::project::theme::PALETTE`]'s order.
pub const PALETTE: [(&str, f64); 8] = [
    ("red", 25.0),
    ("orange", 55.0),
    ("yellow", 95.0),
    ("green", 145.0),
    ("teal", 190.0),
    ("blue", 255.0),
    ("purple", 305.0),
    ("pink", 350.0),
];

/// How much colour a neutral grey is given: a step a person feels on a strip rather than sees.
const NEUTRAL_CHROMA: f64 = 0.03;

/// Below this chroma a colour counts as grey, and has no hue of its own worth keeping.
const GREY: f64 = 0.02;

type Vec3 = [f64; 3];

/// The hue a workspace's colour tints with: a [`PALETTE`] name's, or the OKLCH hue of a
/// `#rrggbb`. `None` for anything else, and for a grey, which has no hue to give.
pub fn hue_of(colour: &str) -> Option<f64> {
    if let Some((_, hue)) = PALETTE.iter().find(|(name, _)| *name == colour) {
        return Some(*hue);
    }
    let digits = colour.strip_prefix('#')?;
    if digits.len() != 6 || !digits.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    hue_of_any(colour)
}

/// `hex` turned to `hue`, with the relative luminance it had. `hex` is a theme's value, so it is
/// `#rgb`, `#rgba`, `#rrggbb` or `#rrggbbaa`; an alpha it had, it keeps. A value that is not a
/// colour is answered as it is.
pub fn tint_hex(hex: &str, hue: f64) -> String {
    tint_hex_within(hex, hue, f64::INFINITY)
}

/// [`tint_hex`] with the chroma held to at most `most`: how a tint keeps its accent apart from
/// the danger colour ([`apart_chroma`]).
pub fn tint_hex_within(hex: &str, hue: f64, most: f64) -> String {
    let Some((rgb, alpha)) = parse(hex) else {
        return hex.to_owned();
    };
    let target = luminance(rgb);
    let [_, a, b] = oklab(rgb);
    let had = a.hypot(b);
    let chroma = if had < GREY { NEUTRAL_CHROMA } else { had }.min(most);
    let at = |lightness: f64| shown(lightness, chroma, hue);
    // Luminance rises with lightness at a fixed chroma and hue, so the lightness that gives the
    // luminance the colour had is found by halving the interval.
    let (mut low, mut high) = (0.0_f64, 1.0_f64);
    for _ in 0..32 {
        let middle = (low + high) / 2.0;
        if luminance(at(middle)) < target {
            low = middle;
        } else {
            high = middle;
        }
    }
    let [r, g, b] = rounded(at((low + high) / 2.0), target);
    format!("#{r:02x}{g:02x}{b:02x}{alpha}")
}

/// One workspace colour's hue in a theme: a [`PALETTE`] name and its OKLCH hue in degrees.
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize)]
pub struct Hue {
    pub name: &'static str,
    pub hue: f64,
}

/// **A theme's tint recipe** (#2329): whether a workspace's colour tints it, which tokens, and
/// the hue each workspace colour gives. The same in each of a theme's appearances.
///
/// The window is handed it as the tokens it tints on the whole window ([`Self::window`]), the
/// tokens it tints on a workspace's own tab and chat strip ([`Self::tabs`]), and the hues: a
/// theme that is off is handed two empty lists.
#[derive(Debug, Clone, PartialEq)]
pub struct Tint {
    /// Whether a workspace's colour tints this theme at all.
    pub on: bool,
    /// The tokens it turns, in [`TOKENS`]' order, each one of [`TINTABLE`].
    tokens: Vec<&'static str>,
    /// Every [`PALETTE`] name's hue, in its order.
    pub hues: Vec<Hue>,
}

impl Default for Tint {
    /// Purlis Classic's recipe, the one a theme starts from.
    fn default() -> Self {
        Self {
            on: true,
            tokens: TINTED.to_vec(),
            hues: PALETTE
                .iter()
                .map(|(name, hue)| Hue { name, hue: *hue })
                .collect(),
        }
    }
}

impl Tint {
    /// The tokens a workspace's colour turns on its own tab and chat strip: none when off.
    pub fn tabs(&self) -> &[&'static str] {
        if self.on { &self.tokens } else { &[] }
    }

    /// The tokens the colour of the workspace in front turns on the whole window: [`Self::tabs`]
    /// without the strip shades.
    pub fn window(&self) -> Vec<&'static str> {
        self.tabs()
            .iter()
            .copied()
            .filter(|token| !TAB_ONLY.contains(token))
            .collect()
    }

    /// The recipe without `tokens`: a workspace's colour leaves them as the theme draws them. The
    /// accent's words go where the accent goes (#3100), so a chosen item's are never turned away
    /// from the accent beside them.
    pub(super) fn leave(&mut self, tokens: &[&str]) {
        let accent_left = tokens.contains(&"accent.base");
        self.tokens
            .retain(|token| !tokens.contains(token) && !(accent_left && *token == ACCENT_WORDS));
    }

    /// The hue a workspace's colour gives in this theme: a [`PALETTE`] name's hue here, or the
    /// OKLCH hue of a `#rrggbb`; `None` for anything else, and for a grey.
    pub fn hue(&self, colour: &str) -> Option<f64> {
        if let Some(one) = self.hues.iter().find(|one| one.name == colour) {
            return Some(one.hue);
        }
        colour.starts_with('#').then(|| hue_of(colour)).flatten()
    }
}

/// **A rule every tint holds**: `token`, tinted, stays at least `distance` from `from` in OKLab,
/// `from` as the tint draws it (tinted too when the recipe turns it). A rule held `alone` gives
/// less colour only to its own sides (#3130): the accent gives way to the waiting amber, and the
/// accent's kin beside it keep the colour the other rules give them.
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize)]
pub struct Apart {
    pub token: &'static str,
    pub from: &'static str,
    pub distance: f64,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub alone: bool,
}

/// **What every tint holds apart**, at every hue, a workspace's own colour included: the accent
/// from the danger colour (danger is distinguishable from primary), Allow's edge from Keep
/// blocked's (Allow is distinguishable from Keep blocked), the consent floors of ADR 0097 §6, and
/// each accent-hued token drawn beside danger from it as the accent is ([`ACCENT_KIN`]); and the
/// accent from the waiting colour (waiting is distinguishable from the accent, #3130), so a
/// yellow or orange workspace's accent never reads as the waiting signal.
pub const APART: [Apart; 17] = [
    danger_apart("accent.base"),
    // Keep blocked's edge is a plain control's (#3177).
    Apart {
        token: "accent.base",
        from: "border.control",
        distance: ALLOW_APART,
        alone: false,
    },
    danger_apart(ACCENT_KIN[0]),
    danger_apart(ACCENT_KIN[1]),
    danger_apart(ACCENT_KIN[2]),
    danger_apart(ACCENT_KIN[3]),
    danger_apart(ACCENT_KIN[4]),
    danger_apart(ACCENT_KIN[5]),
    danger_apart(ACCENT_KIN[6]),
    danger_apart(ACCENT_KIN[7]),
    danger_apart(ACCENT_KIN[8]),
    danger_apart(ACCENT_KIN[9]),
    // The waiting edge of an ask or a Notice never draws in the accent, nor in the edges drawn
    // in it (#3130): each gives way alone, and the accent's washes keep their colour.
    waiting_apart(WAITING_KIN[0]),
    waiting_apart(WAITING_KIN[1]),
    waiting_apart(WAITING_KIN[2]),
    waiting_apart(WAITING_KIN[3]),
    waiting_apart(WAITING_KIN[4]),
];

/// **The accent and the edges drawn in it** (#3130): the accent, the focus ring, the selected
/// row's edge and the primary's edge, each a line or a word in the accent's colour that the
/// waiting edge must not be taken for, and the accent's words on a chosen item (#3100). The
/// accent's washes (its surface, the primary's fill, the active tab) are not marks, and keep their
/// colour.
pub const WAITING_KIN: [&str; 5] = [
    "accent.base",
    "accent.text",
    "focus.ring",
    "list.selected-edge",
    "accent.fill-edge",
];

/// `token` held [`WAITING_APART`] from `state.waiting`, alone.
const fn waiting_apart(token: &'static str) -> Apart {
    Apart {
        token,
        from: "state.waiting",
        distance: WAITING_APART,
        alone: true,
    }
}

/// The tokens in the accent's hue that the window draws beside the danger colour: the focus
/// ring, the selected row's edge, the accent's surface and the active tab (#2526), and the
/// primary's fill at rest and under the pointer and its edge (#3177), which stand beside a danger
/// answer; and a link and the first chart colour, which a person reads as the accent (#3049), so a
/// red, orange or pink workspace never draws a link that looks like danger; and the accent's words
/// on a chosen item (#3100).
/// Each is kept apart from danger as the accent is, by the tint and by the floors.
pub const ACCENT_KIN: [&str; 10] = [
    "focus.ring",
    "list.selected-edge",
    "accent.surface",
    "tab.active",
    "accent.fill",
    "accent.fill-hover",
    "accent.fill-edge",
    "markdown.link",
    "chart.1",
    "accent.text",
];

/// `token` held [`DANGER_APART`] from `danger.base`.
const fn danger_apart(token: &'static str) -> Apart {
    Apart {
        token,
        from: "danger.base",
        distance: DANGER_APART,
        alone: false,
    }
}

/// What the window is handed: the two lists, the hues, and the rules a tint holds.
#[derive(serde::Serialize)]
struct Handed<'a> {
    window: Vec<&'static str>,
    tabs: &'a [&'static str],
    hues: &'a [Hue],
    apart: &'a [Apart],
}

impl serde::Serialize for Tint {
    fn serialize<S: serde::Serializer>(&self, to: S) -> Result<S::Ok, S::Error> {
        Handed {
            window: self.window(),
            tabs: self.tabs(),
            hues: &self.hues,
            apart: &APART,
        }
        .serialize(to)
    }
}

/// The keys of a theme's `tint`.
const KEYS: &[&str] = &["on", "tokens", "hues"];

/// **Reads a theme's `tint`** over `parent`'s recipe, saying in `complaints` what refuses the
/// theme and in `notes` a token name purlis does not have, which is left out. `palette` is the
/// theme's own, which a hue may name a colour of.
pub(super) fn read(
    raw: &Value,
    parent: &Tint,
    palette: &BTreeMap<String, String>,
    complaints: &mut Vec<Complaint>,
    notes: &mut Vec<Complaint>,
) -> Tint {
    let mut tint = parent.clone();
    let Some(given) = raw.as_object() else {
        complaints.push(Complaint::said("tint is an object of on, tokens and hues"));
        return tint;
    };
    for key in given.keys() {
        if !KEYS.contains(&key.as_str()) {
            complaints.push(Complaint::said(format!("{key} is not a key of tint")));
        }
    }
    match given.get("on") {
        None => {}
        Some(Value::Bool(on)) => tint.on = *on,
        Some(other) => complaints.push(Complaint::said(format!(
            "tint's on is {}, which is not true or false",
            stringify(other)
        ))),
    }
    if let Some(raw) = given.get("tokens")
        && let Some(tokens) = tokens(raw, complaints, notes)
    {
        tint.tokens = tokens;
    }
    if let Some(raw) = given.get("hues") {
        hues(raw, palette, &mut tint.hues, complaints);
    }
    tint
}

/// A `tokens` list, in [`TOKENS`]' order, or `None` (and why) when it is not one.
fn tokens(
    raw: &Value,
    complaints: &mut Vec<Complaint>,
    notes: &mut Vec<Complaint>,
) -> Option<Vec<&'static str>> {
    const NOT_A_LIST: &str = "tint's tokens is a list of token names";
    let Some(given) = raw.as_array() else {
        complaints.push(Complaint::said(NOT_A_LIST));
        return None;
    };
    // Held before anything is read, so a list of any length costs one count.
    if given.len() > TOKENS.len() || !given.iter().all(Value::is_string) {
        complaints.push(Complaint::said(NOT_A_LIST));
        return None;
    }
    let mut named = Vec::new();
    let before = complaints.len();
    for token in given.iter().filter_map(Value::as_str) {
        match TOKENS.iter().find(|it| **it == token) {
            Some(known) if TINTABLE.contains(known) => named.push(*known),
            Some(_) => complaints.push(Complaint::about(
                token,
                format!("tint's tokens name {token}, which a workspace's colour never tints"),
            )),
            None => notes.push(Complaint::about(
                token,
                format!(
                    "tint's tokens name {token}, which is not a purlis token, so it is left out"
                ),
            )),
        }
    }
    if complaints.len() != before {
        return None;
    }
    // A recipe that turns the accent turns its words (#3100), named or not: what a chosen item
    // says was the accent before it was a token of its own.
    if named.contains(&"accent.base") {
        named.push(ACCENT_WORDS);
    }
    Some(
        TOKENS
            .iter()
            .copied()
            .filter(|token| named.contains(token))
            .collect(),
    )
}

/// Lays a `hues` object over `hues`, saying what is wrong with any of it.
fn hues(
    raw: &Value,
    palette: &BTreeMap<String, String>,
    hues: &mut [Hue],
    complaints: &mut Vec<Complaint>,
) {
    let Some(given) = raw.as_object() else {
        complaints.push(Complaint::said(
            "tint's hues is an object of workspace colours to colours",
        ));
        return;
    };
    for (name, value) in given {
        let Some(one) = hues.iter_mut().find(|one| one.name == name) else {
            complaints.push(Complaint::said(format!(
                "tint's hues name {}, which is not a workspace colour",
                as_written(name)
            )));
            continue;
        };
        let hex = value.as_str().and_then(|it| {
            if is_colour(it) {
                Some(it)
            } else {
                palette.get(it).map(String::as_str)
            }
        });
        let Some(hex) = hex else {
            complaints.push(Complaint::said(format!(
                "tint's hue for {name} is {}, which is neither a hex colour nor a name in this \
                 theme's palette",
                stringify(value)
            )));
            continue;
        };
        match hue_of_any(hex) {
            Some(hue) => one.hue = hue,
            None => complaints.push(Complaint::said(format!(
                "tint's hue for {name} is {hex}, a grey, which has no hue to give"
            ))),
        }
    }
}

/// A name a file wrote, as a sentence shows it: bare when it is a plain name, quoted otherwise.
fn as_written(name: &str) -> String {
    if super::format::is_name(name) {
        name.to_owned()
    } else {
        stringify(&Value::String(name.to_owned()))
    }
}

/// The OKLCH hue of any theme colour (`#rgb` to `#rrggbbaa`, its alpha left out); `None` for a
/// grey.
fn hue_of_any(hex: &str) -> Option<f64> {
    let [_, a, b] = oklab(parse(hex)?.0);
    if a.hypot(b) < GREY {
        return None;
    }
    Some(degrees(b.atan2(a)))
}

/// `theme` with a workspace's colour turning the tokens its recipe names to the hue it gives
/// there ([`Tint`]): what the window draws on that workspace's tab and chat strip. The theme as
/// it is for a theme that is not tinted, and for a colour with no hue.
///
/// **Every tint holds [`APART`]**: the accent and the accent-hued tokens beside danger apart
/// from the danger colour, and Allow's edge from Keep blocked's. At a hue that would bring them
/// closer, the tint keeps the hue and gives those tokens less colour ([`apart_chroma`]), so they
/// stay apart from danger; where no amount holds them apart, they are drawn untinted. **Only the
/// tokens a rule of [`APART`] names are held** ([`chroma_for`], #3034): the strip's shades and
/// every other token the recipe turns keep the hue's full colour, so a red workspace keeps a red
/// strip in every theme.
pub fn tinted(theme: &ResolvedTheme, colour: &str) -> ResolvedTheme {
    match theme.tint.hue(colour) {
        Some(hue) => tinted_at(theme, hue),
        None => theme.clone(),
    }
}

/// [`tinted`] by a hue in degrees: what the floors measure at each hue a workspace's own colour
/// can take.
pub(super) fn tinted_at(theme: &ResolvedTheme, hue: f64) -> ResolvedTheme {
    let tokens = theme.tint.tabs();
    if tokens.is_empty() {
        return theme.clone();
    }
    let most = most_at(theme, hue);
    let mut tint = theme.clone();
    for token in tokens {
        let Some(chroma) = chroma_of(token, most) else {
            continue;
        };
        if let Some(value) = tint.values.get_mut(token) {
            *value = tint_hex_within(value, hue, chroma);
        }
    }
    tint
}

/// **Whether `token` is a side of a rule of [`APART`]**: the tokens a tint gives less colour to
/// hold the rules.
pub fn held_apart(token: &str) -> bool {
    APART
        .iter()
        .any(|rule| rule.token == token || rule.from == token)
}

/// **Whether `token` is a side of a rule of [`APART`] held alone** (#3130).
fn held_alone(token: &str) -> bool {
    APART
        .iter()
        .any(|rule| rule.alone && (rule.token == token || rule.from == token))
}

/// **The most chroma a tint at one hue gives**: `shared`, what [`apart_chroma`] answers, for
/// every side of a rule of [`APART`]; and `alone`, what every rule together answers, for the
/// sides of a rule held alone, which give way further on their own (#3130).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Most {
    pub shared: Option<f64>,
    pub alone: Option<f64>,
}

/// [`Most`] for `theme` at `hue`.
pub fn most_at(theme: &ResolvedTheme, hue: f64) -> Most {
    most_of(&held_rules(theme), &shared_rules(theme), hue)
}

/// [`Most`] of every held rule and of the shared ones among them.
pub(super) fn most_of(all: &[Rule<'_>], shared: &[Rule<'_>], hue: f64) -> Most {
    let shared_most = apart_chroma_of(shared, hue);
    let alone = if all.len() == shared.len() {
        shared_most
    } else {
        apart_chroma_of(all, hue)
    };
    Most {
        shared: shared_most,
        alone,
    }
}

/// **The most chroma a tint gives `token`** at a hue whose [`Most`] is `most`: the alone
/// answer for a side of a rule held alone, else as [`chroma_for`] gives it. `tint.ts`'
/// `chromaFor` is the same.
pub fn chroma_of(token: &str, most: Most) -> Option<f64> {
    if held_alone(token) {
        most.alone
    } else {
        chroma_for(token, most.shared)
    }
}

/// **The most chroma a tint gives `token`** when [`apart_chroma`] answers `most` (#3034): `most`
/// for a side of a rule of [`APART`] (`None`, untinted, where no amount holds them), and no limit
/// for every other token, which no rule measures. `tint.ts`' `chromaFor` is the same.
pub fn chroma_for(token: &str, most: Option<f64>) -> Option<f64> {
    if held_apart(token) {
        most
    } else {
        Some(f64::INFINITY)
    }
}

/// `theme` with those of `only` its recipe turns drawn at `hue` and at most `most` chroma, as
/// [`tinted_at`] draws them when [`apart_chroma`] answers `most`; the theme as it is for `None`.
pub(super) fn turned_at(
    theme: &ResolvedTheme,
    hue: f64,
    most: Most,
    only: &[&'static str],
) -> ResolvedTheme {
    let mut tint = theme.clone();
    for token in theme
        .tint
        .tabs()
        .iter()
        .filter(|token| only.contains(token))
    {
        let Some(chroma) = chroma_of(token, most) else {
            continue;
        };
        if let Some(value) = tint.values.get_mut(token) {
            *value = tint_hex_within(value, hue, chroma);
        }
    }
    tint
}

/// A theme value's relative luminance, its alpha left out; `None` for a value that is not a
/// colour.
pub(super) fn luminance_of(hex: &str) -> Option<f64> {
    parse(hex).map(|(rgb, _)| luminance(rgb))
}

/// Whether a colour of luminance `of` is on the light side of [`middle`]: the side a tint rounds
/// it away from the middle on.
pub(super) fn light_side(of: f64) -> bool {
    of >= middle()
}

/// How far a luminance is from [`middle`].
pub(super) fn from_middle(of: f64) -> f64 {
    (of - middle()).abs()
}

/// The step the chroma a tint gives is lowered by to hold its accent apart.
const CHROMA_STEP: f64 = 0.005;

/// One rule of [`APART`] as a theme draws it: the value of each side, whether the recipe turns
/// each, and the distance it holds.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rule<'a> {
    pub token: &'a str,
    pub token_tinted: bool,
    pub from: &'a str,
    pub from_tinted: bool,
    pub distance: f64,
}

impl Rule<'_> {
    /// Each side as a tint at `hue` and `most` chroma draws it: turned when the recipe turns it.
    fn sides(&self, hue: f64, most: f64) -> [String; 2] {
        let side = |value: &str, tinted: bool| {
            if tinted {
                tint_hex_within(value, hue, most)
            } else {
                value.to_owned()
            }
        };
        [
            side(self.token, self.token_tinted),
            side(self.from, self.from_tinted),
        ]
    }
}

/// **The most chroma a tint of `theme` at `hue` gives**: no limit (infinity) when every rule of
/// [`APART`] with a side the recipe tints holds as tinted; else the largest multiple of
/// [`CHROMA_STEP`] under the tinted sides' own chroma that holds them all, every token of the
/// tint drawn at that chroma; `None` when not even a grey does. Every rule holds whichever side
/// a theme tints. The window does the same arithmetic (`tint.ts`' `apartChroma`), held to it by
/// the shared fixture.
pub fn apart_chroma(theme: &ResolvedTheme, hue: f64) -> Option<f64> {
    apart_chroma_of(&shared_rules(theme), hue)
}

/// The rules of [`APART`] as `theme` draws them, each with a side its recipe turns.
pub(super) fn held_rules(theme: &ResolvedTheme) -> Vec<Rule<'_>> {
    rules_where(theme, |_| true)
}

/// [`held_rules`] but those held alone: what every held token gives way to.
pub(super) fn shared_rules(theme: &ResolvedTheme) -> Vec<Rule<'_>> {
    rules_where(theme, |rule| !rule.alone)
}

fn rules_where(theme: &ResolvedTheme, keep: impl Fn(&Apart) -> bool) -> Vec<Rule<'_>> {
    let tokens = theme.tint.tabs();
    APART
        .iter()
        .filter(|rule| keep(rule))
        .map(|rule| Rule {
            token: theme.value(rule.token),
            token_tinted: tokens.contains(&rule.token),
            from: theme.value(rule.from),
            from_tinted: tokens.contains(&rule.from),
            distance: rule.distance,
        })
        .filter(|rule| rule.token_tinted || rule.from_tinted)
        .collect()
}

/// [`apart_chroma`] of the rules as drawn: what the shared fixture holds.
pub fn apart_chroma_of(rules: &[Rule<'_>], hue: f64) -> Option<f64> {
    let far = |most: f64| {
        rules.iter().all(|rule| {
            let [token, from] = rule.sides(hue, most);
            match (oklab_of(&token), oklab_of(&from)) {
                (Some(token), Some(from)) => distance(token, from) >= rule.distance,
                // A value that is not a colour has nothing to be near.
                _ => true,
            }
        })
    };
    if far(f64::INFINITY) {
        return Some(f64::INFINITY);
    }
    // The most colour any tinted side is given: a grey is given [`NEUTRAL_CHROMA`].
    let had = rules
        .iter()
        .flat_map(|rule| {
            [
                (rule.token, rule.token_tinted),
                (rule.from, rule.from_tinted),
            ]
        })
        .filter(|(_, tinted)| *tinted)
        .filter_map(|(value, _)| oklab_of(value))
        .map(|[_, a, b]| {
            let own = a.hypot(b);
            if own < GREY { NEUTRAL_CHROMA } else { own }
        })
        .fold(0.0_f64, f64::max);
    (0..=80)
        .rev()
        .map(|step| f64::from(step) * CHROMA_STEP)
        .filter(|most| *most < had)
        .find(|most| far(*most))
}

/// The OKLab distance between two colours, written out so the window's sum is the same sum:
/// what the tint holds apart and the consent floors measure ([`super::floors`]).
pub(super) fn distance(a: Vec3, b: Vec3) -> f64 {
    let (l, x, y) = (a[0] - b[0], a[1] - b[1], a[2] - b[2]);
    (l * l + x * x + y * y).sqrt()
}

/// A theme value in OKLab, its alpha left out; `None` for a value that is not a colour.
pub(super) fn oklab_of(hex: &str) -> Option<Vec3> {
    parse(hex).map(|(rgb, _)| oklab(rgb))
}

/// **`hex` at the OKLab lightness `lightness`** (#2940): its hue kept, its chroma lowered only as
/// far as the screen needs, its alpha kept, each channel rounded to its nearest byte. `None` for
/// a value that is not a colour. What the tier floors walk a grey along.
pub(super) fn at_lightness(hex: &str, lightness: f64) -> Option<String> {
    let (rgb, alpha) = parse(hex)?;
    let [_, a, b] = oklab(rgb);
    let shade = shown(lightness.clamp(0.0, 1.0), a.hypot(b), degrees(b.atan2(a)));
    let [r, g, b] = shade.map(|v| (gamma(v) * 255.0).round().clamp(0.0, 255.0) as u8);
    Some(format!("#{r:02x}{g:02x}{b:02x}{alpha}"))
}

/// WCAG's relative luminance of a colour in linear sRGB.
fn luminance([r, g, b]: Vec3) -> f64 {
    0.2126 * r + 0.7152 * g + 0.0722 * b
}

/// The colour at this OKLCH, in linear sRGB, with the chroma lowered until the screen can show
/// it when it cannot.
fn shown(lightness: f64, chroma: f64, hue: f64) -> Vec3 {
    let at = |c: f64| linear([lightness, c * cos(hue), c * sin(hue)]);
    let whole = at(chroma);
    if in_gamut(whole) {
        return whole;
    }
    let (mut low, mut high) = (0.0_f64, chroma);
    for _ in 0..24 {
        let middle = (low + high) / 2.0;
        if in_gamut(at(middle)) {
            low = middle;
        } else {
            high = middle;
        }
    }
    at(low).map(|v| v.clamp(0.0, 1.0))
}

fn in_gamut(rgb: Vec3) -> bool {
    rgb.iter().all(|v| *v >= -1e-6 && *v <= 1.0 + 1e-6)
}

fn cos(deg: f64) -> f64 {
    (deg * std::f64::consts::PI / 180.0).cos()
}

fn sin(deg: f64) -> f64 {
    (deg * std::f64::consts::PI / 180.0).sin()
}

fn degrees(rad: f64) -> f64 {
    (rad * 180.0 / std::f64::consts::PI + 360.0) % 360.0
}

/// A theme value as linear sRGB, and its alpha as the two hex digits it was written with.
fn parse(hex: &str) -> Option<(Vec3, String)> {
    let digits = hex.strip_prefix('#')?;
    if !is_colour(hex) {
        return None;
    }
    let long: String = if digits.len() <= 4 {
        digits.chars().flat_map(|d| [d, d]).collect()
    } else {
        digits.to_owned()
    };
    let byte = |at: usize| u8::from_str_radix(&long[at..at + 2], 16).ok();
    let rgb = [
        to_linear(f64::from(byte(0)?) / 255.0),
        to_linear(f64::from(byte(2)?) / 255.0),
        to_linear(f64::from(byte(4)?) / 255.0),
    ];
    Some((rgb, long.get(6..8).unwrap_or_default().to_owned()))
}

/// WCAG's middle luminance: a colour this bright contrasts with black exactly as much as with
/// white. A darker colour is read against lighter ones, and a lighter against darker ones.
fn middle() -> f64 {
    (1.05_f64 * 0.05).sqrt() - 0.05
}

/// **A colour in linear sRGB as three bytes, its luminance kept on its own side** (#2526): each
/// channel rounded down or up, whichever of the eight ways gives the luminance nearest `target`,
/// the luminance the colour had, without passing it toward [`middle`]. A dark shade is never
/// rounded lighter than it was and a light one never darker, so whatever it was read against
/// on the other side of the middle it contrasts with at least as much as before it was tinted.
fn rounded(rgb: Vec3, target: f64) -> [u8; 3] {
    let exact = rgb.map(|v| gamma(v) * 255.0);
    let lighter = light_side(target);
    let mut best = [0_u8; 3];
    let mut best_key = (true, f64::INFINITY);
    for pick in 0..8_u8 {
        let bytes: [u8; 3] = std::array::from_fn(|at| {
            let up = f64::from((pick >> (2 - at)) & 1);
            // A byte, by the clamp.
            (exact[at].floor() + up).clamp(0.0, 255.0) as u8
        });
        let shade = luminance(bytes.map(|byte| to_linear(f64::from(byte) / 255.0)));
        let past = if lighter {
            shade < target
        } else {
            shade > target
        };
        let key = (past, (shade - target).abs());
        // Not past it before past it, then the nearer: the window compares the same way.
        if key < best_key {
            best = bytes;
            best_key = key;
        }
    }
    best
}

/// One linear channel gamma-encoded, 0 to 1.
fn gamma(linear_value: f64) -> f64 {
    let v = linear_value.clamp(0.0, 1.0);
    if v <= 0.003_130_8 {
        12.92 * v
    } else {
        1.055 * v.powf(1.0 / 2.4) - 0.055
    }
}

fn to_linear(v: f64) -> f64 {
    if v <= 0.04045 {
        v / 12.92
    } else {
        ((v + 0.055) / 1.055).powf(2.4)
    }
}

/// Linear sRGB to OKLab (Björn Ottosson's matrices).
fn oklab([r, g, b]: Vec3) -> Vec3 {
    let l = (0.412_221_470_8 * r + 0.536_332_536_3 * g + 0.051_445_992_9 * b).cbrt();
    let m = (0.211_903_498_2 * r + 0.680_699_545_1 * g + 0.107_396_956_6 * b).cbrt();
    let s = (0.088_302_461_9 * r + 0.281_718_837_6 * g + 0.629_978_700_5 * b).cbrt();
    [
        0.210_454_255_3 * l + 0.793_617_785 * m - 0.004_072_046_8 * s,
        1.977_998_495_1 * l - 2.428_592_205 * m + 0.450_593_709_9 * s,
        0.025_904_037_1 * l + 0.782_771_766_2 * m - 0.808_675_766 * s,
    ]
}

/// OKLab to linear sRGB.
fn linear([lightness, a, b]: Vec3) -> Vec3 {
    let l = (lightness + 0.396_337_777_4 * a + 0.215_803_757_3 * b).powf(3.0);
    let m = (lightness - 0.105_561_345_8 * a - 0.063_854_172_8 * b).powf(3.0);
    let s = (lightness - 0.089_484_177_5 * a - 1.291_485_548 * b).powf(3.0);
    [
        4.076_741_662_1 * l - 3.307_711_591_3 * m + 0.230_969_929_2 * s,
        -1.268_438_004_6 * l + 2.609_757_401_1 * m - 0.341_319_396_5 * s,
        -0.004_196_086_3 * l - 0.703_418_614_7 * m + 1.707_614_701 * s,
    ]
}
