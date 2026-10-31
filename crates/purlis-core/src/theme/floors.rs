// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The consent floors** (ADR 0097 §6, ADR 0041 as amended 2026-10-17): what holds for the
//! asks, the danger colours and the state marks whatever a theme says, every time a theme is
//! drawn.
//!
//! A theme may set every token, the consent surfaces' and the state marks' included. These floors
//! hold for them:
//!
//! - **an ask contrasts with its surface** ([`Floor::Ask`]): an ask's words on the dialog it is
//!   asked in, selected or not, its answers' words on their buttons, what it says on the danger
//!   wash, selected or not, and the needs-you count's number on its chip;
//! - **Allow is distinguishable from Keep blocked** ([`Floor::AllowApart`]): the two carry equal
//!   weight on the control's own ground, and Allow is edged in `accent.base` where Keep blocked
//!   is edged in `border.control`, a plain control's edge (none in Purlis, #3177). The two edges are at least [`ALLOW_APART`] apart as OKLab draws
//!   colour, as the theme writes them and under each of its own tints, and the Allow's edge
//!   contrasts with its ground, at rest and under the pointer;
//! - **the danger colours contrast with what they are drawn on** ([`Floor::Danger`]): the
//!   trouble Notice's words on the danger surface, and the danger colour on each ground the
//!   window draws it on, a theme's selection included, and selected text a theme colours on
//!   its selection over the danger surface;
//! - **danger is distinguishable from primary** ([`Floor::DangerApart`]): the danger colour is
//!   at least [`DANGER_APART`] apart as OKLab draws colour from the accent and from each
//!   accent-hued token drawn beside it ([`tint::ACCENT_KIN`]), as the theme writes them and under
//!   each of its own tints;
//! - **the state marks meet the mark floor** ([`Floor::StateMarks`]): every state's colour and
//!   the needs-you mark on each ground they are drawn on;
//! - **Allow is distinguishable from the primary** ([`Floor::AllowNotPrimary`], #2948): a
//!   theme's tokens alone draw an answer that grants at least [`ALLOW_FROM_PRIMARY`] from the suggested
//!   answer, at rest and in each state, measured as the component parts check measures two
//!   looks ([`super::parts`]): the fills as each shows on the grounds the family is drawn on,
//!   the nearest ground counting, and the edges where they stand out. It holds as the theme
//!   writes them, under each of its own tints, and at every whole degree a workspace's own
//!   colour can take. Like every floor here it is measured on the token set [`super::draw`] is
//!   handed, so a layer laid over a theme after it is read (the person's own colours) passes
//!   through it only when it is laid before the theme is drawn, never in the window after.
//!   Where it fails, the primary's fill is drawn as purlis's own first, and
//!   Allow's only when that is not enough.
//! - **waiting is distinguishable from the accent** ([`Floor::WaitingApart`], #3130): the
//!   waiting colour is at least [`WAITING_APART`] apart as OKLab draws colour from the accent
//!   and from each edge drawn in it ([`tint::WAITING_KIN`]), as the theme writes them and under
//!   each of its own tints, so the waiting edge of an ask or
//!   a Notice never reads as a chosen or primary mark. The tint holds it as it holds danger
//!   apart: at a hue that would bring the accent near the waiting amber (a yellow or an orange
//!   workspace), the accent is given less colour.
//! - **the hot count is distinguishable from the plain count** ([`Floor::HotCountApart`], #3098):
//!   `needs-you.count` is at least [`HOT_COUNT_APART`] apart as OKLab draws colour from
//!   `surface.hover`, the plain count's pill, each laid on each ground a bar stands on as the
//!   screen draws them, in the theme as written, under each of its own tints and at every whole
//!   degree a workspace's own colour can take. A part rule that brings the plain count's fill near
//!   the hot one draws its family default ([`super::parts`]).
//!
//! The contrast floors are pairs of the check's own [`PAIRS`], held to the floor the check holds
//! each to, so a pair is measured in one place. Each holds as the theme is and tinted by every
//! colour a workspace can name, by the theme's own tint recipe, as [`super::check`] measures.
//!
//! **A theme that breaks one is drawn with purlis's own values for those tokens**: Purlis
//! Classic's of the same appearance, for the selection's tokens of a pair drawn on a selection
//! (the platform's, so the pair is no longer drawn), else for the consent and state tokens of
//! each pair that fails.
//! When Classic's colour still fails on the ground the theme draws it on, the ground is drawn as
//! Classic's too, and a token whose workspace tint still breaks the floor is drawn untinted
//! ([`super::Tint`]). A hot count that follows its needs-you mark falls with it, either way round
//! ([`super::vocabulary::HOT_COUNT_KIN`]), so the two never disagree; a theme whose count is its own colour is held on
//! its own. The rest of the theme draws as written, and [`Held`] says which floors and which
//! tokens, for one Notice per theme.

use std::collections::BTreeSet;

use super::check::{self, PAIRS, Pair, ratio};
use super::vocabulary::hot_count_following;
use super::{Appearance, ResolvedTheme, built_in_for, tint};

#[cfg(test)]
#[path = "allow_primary_tests.rs"]
mod allow_primary_tests;

/// One consent floor.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, serde::Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Floor {
    /// An ask contrasts with its surface.
    Ask,
    /// Allow is distinguishable from Keep blocked.
    AllowApart,
    /// The danger colours contrast with what they are drawn on.
    Danger,
    /// Danger is distinguishable from primary.
    DangerApart,
    /// The state marks meet the mark floor.
    StateMarks,
    /// Allow is distinguishable from the primary (#2948).
    AllowNotPrimary,
    /// Waiting is distinguishable from the accent (#3130).
    WaitingApart,
    /// The hot count is distinguishable from the plain count (#3098).
    HotCountApart,
}

impl Floor {
    /// The floor in the words a Notice names it by.
    pub fn says(self) -> &'static str {
        match self {
            Self::Ask => "an ask contrasts with its surface",
            Self::AllowApart => "Allow is distinguishable from Keep blocked",
            Self::Danger => "the danger colours contrast with what they are drawn on",
            Self::DangerApart => "danger is distinguishable from primary",
            Self::StateMarks => "the state marks meet the mark floor",
            Self::AllowNotPrimary => "Allow is distinguishable from the primary",
            Self::WaitingApart => "waiting is distinguishable from the accent",
            Self::HotCountApart => "the hot count is distinguishable from the plain count",
        }
    }
}

/// **How far apart danger and primary are, at least**: the distance between `danger.base` and
/// `accent.base` in OKLab (Björn Ottosson's), where 1 is the distance from black to white and a
/// difference of about 0.02 is the least a person can see. 0.1 is five times that: two colours
/// told apart at a glance, not on a second look.
pub const DANGER_APART: f64 = 0.1;

/// **How far apart Allow's edge and Keep blocked's are, at least**: the distance between
/// `accent.base` and `border.control` in OKLab, held as [`DANGER_APART`] is, for the same reason:
/// two answers told apart at a glance, not on a second look.
pub const ALLOW_APART: f64 = 0.1;

/// **How far apart the waiting colour and the accent are, at least**: the distance between
/// `state.waiting` and `accent.base` in OKLab, held as [`DANGER_APART`] is (#3130): the waiting
/// signal told from a chosen or primary mark at a glance.
pub const WAITING_APART: f64 = 0.1;

/// **How far apart the hot count's fill and the plain count's are, at least**: the distance
/// between `needs-you.count` and `surface.hover` in OKLab, held as [`DANGER_APART`] is (#3098):
/// what waits on the person told from any other count at a glance.
pub const HOT_COUNT_APART: f64 = 0.1;

/// The hot count's fill, the role a theme restyles it through (#3098).
pub(super) const HOT_COUNT: &str = "needs-you.count";

/// The plain count's pill by default (`activity.css`, [`super::parts`]'s activity family).
const PLAIN_COUNT: &str = "surface.hover";

/// The grounds a bar stands on, each of which both counts are laid on.
const BAR_GROUNDS: [&str; 3] = ["surface.deep", "surface.base", "surface.raised"];

/// **How far apart Allow's look and the primary's are, at least**, as a theme's tokens alone
/// draw them ([`Floor::AllowNotPrimary`]): measured as the component parts check measures two
/// looks, on its own scale.
pub const ALLOW_FROM_PRIMARY: f64 = super::parts::SEEN;

/// The ask's pairs, by `front` on `back`: its words on the dialog it is asked in, its answers'
/// words on their buttons (Allow's and Keep blocked's alike) at rest and under the pointer, what
/// it says on the danger wash, and the needs-you count on its chip, the hot count's on its own.
const ASK: [(&str, &str); 6] = [
    ("text.primary", "surface.overlay"),
    ("text.primary", "control.base"),
    ("text.primary", "control.hover"),
    ("text.primary", "danger.wash"),
    ("needs-you.text", "needs-you.base"),
    ("needs-you.count-text", "needs-you.count"),
];

/// The pairs that keep Allow apart from Keep blocked, by `front` on `back`: the Allow's edge on
/// its own ground, at rest and under the pointer, so the edge that tells it apart is seen.
const ALLOW: [(&str, &str); 2] = [
    ("accent.base", "control.base"),
    ("accent.base", "control.hover"),
];

/// Selected text a theme colours, on its selection over a consent surface, by the ground and the
/// floor it is held to (#2706): every selected word is drawn in `selection.text`, so a trouble
/// Notice's danger words are on `danger.surface`, a dialog's ask is on `surface.overlay`, and the
/// command a profile will run as you is on the danger wash over the dialog (#2724).
const SELECTED: [(&str, Floor); 3] = [
    ("danger.surface", Floor::Danger),
    ("surface.overlay", Floor::Ask),
    ("danger.wash", Floor::Ask),
];

/// Whether `token` is a consent surface's or a state mark's: what purlis draws its own value of
/// first when a floor fails.
fn is_consent(token: &str) -> bool {
    ["danger.", "needs-you.", "state.", "attention."]
        .iter()
        .any(|prefix| token.starts_with(prefix))
}

/// The floor a pair of [`PAIRS`] is held to as a consent floor, if it is one.
fn floor_of(pair: &Pair) -> Option<Floor> {
    if (pair.front, pair.back) == ("selection.text", "selection.background") {
        SELECTED
            .iter()
            .find(|(ground, _)| pair.under == Some(*ground))
            .map(|(_, floor)| *floor)
    } else if ASK.contains(&(pair.front, pair.back)) {
        Some(Floor::Ask)
    } else if ALLOW.contains(&(pair.front, pair.back)) {
        Some(Floor::AllowApart)
    } else if pair.front.starts_with("danger.") {
        Some(Floor::Danger)
    } else if pair.front.starts_with("state.") || matches!(pair.front, "needs-you.base" | HOT_COUNT)
    {
        Some(Floor::StateMarks)
    } else {
        None
    }
}

/// Every pair of [`PAIRS`] that is a consent floor, with its floor.
pub fn pairs() -> impl Iterator<Item = (Floor, &'static Pair)> {
    PAIRS
        .iter()
        .filter_map(|pair| floor_of(pair).map(|floor| (floor, pair)))
}

/// What holding the floors put right: which floors the theme broke, and the tokens drawn as
/// purlis's own in its place.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct Held {
    /// The theme's name, as the Notice names it.
    pub theme: String,
    /// The floors it broke, each once.
    pub floors: Vec<Floor>,
    /// The tokens drawn as Purlis Classic's of the same appearance.
    pub tokens: Vec<&'static str>,
    /// The tokens drawn untinted in a tinted workspace, because a workspace's tint broke a floor
    /// that purlis's own values could not hold ([`super::Tint`]).
    pub untinted: Vec<&'static str>,
}

impl Held {
    /// The Notice's sentence: the theme, the floors it breaks, and what purlis draws instead:
    /// its own values of [`Self::tokens`], and [`Self::untinted`] untinted in a tinted
    /// workspace. A clause with no tokens is left out.
    pub fn said(&self) -> String {
        let floors = self
            .floors
            .iter()
            .map(|floor| floor.says())
            .collect::<Vec<_>>()
            .join("; ");
        let (theme, ours, untinted) = (
            &self.theme,
            self.tokens.join(", "),
            self.untinted.join(", "),
        );
        const REST: &str = "the rest of the theme draws as written";
        match (ours.is_empty(), untinted.is_empty()) {
            (false, true) => format!(
                "{theme} breaks the consent floors ({floors}), so purlis draws its own {ours}; {REST}"
            ),
            (true, false) => format!(
                "In a tinted workspace, {theme} would fall under the consent floors ({floors}), \
                 so purlis draws {untinted} untinted there to keep its words readable; {REST}"
            ),
            (false, false) => format!(
                "{theme} breaks the consent floors ({floors}), so purlis draws its own {ours}, \
                 and {untinted} untinted in tinted workspaces; {REST}"
            ),
            (true, true) => format!("{theme} breaks the consent floors ({floors})"),
        }
    }
}

/// **Two holds' answers as one** (#2925): what holding the floors put right over a theme, and
/// then over the person's layer drawn on it, each floor and token named once.
pub(super) fn joined(first: Option<Held>, then: Option<Held>) -> Option<Held> {
    match (first, then) {
        (None, then) => then,
        (first, None) => first,
        (Some(mut first), Some(then)) => {
            for floor in then.floors {
                if !first.floors.contains(&floor) {
                    first.floors.push(floor);
                }
            }
            for token in then.tokens {
                if !first.tokens.contains(&token) {
                    first.tokens.push(token);
                }
            }
            for token in then.untinted {
                if !first.untinted.contains(&token) {
                    first.untinted.push(token);
                }
            }
            first.floors.sort();
            Some(first)
        }
    }
}

/// One failure: the floor, whether only a workspace's tint broke it, and the tokens it is
/// between, a wash's ground included.
struct Failure {
    floor: Floor,
    tinted: bool,
    tokens: Vec<&'static str>,
}

/// Every consent floor `theme` breaks, as it is and tinted by every workspace colour of its own
/// recipe: the contrast floors, Allow apart from Keep blocked, and danger apart from the accent
/// and from each accent-hued token drawn beside it ([`tint::APART`]).
/// The chroma each whole degree's tint gives ([`tint::apart_chroma`]), kept for the rules it was
/// found under: a round of [`hold`] that changes neither side of any rule asks again for nothing.
#[derive(Default)]
struct Chromas {
    rules: Vec<(String, bool, String, bool)>,
    at: Vec<tint::Most>,
}

impl Chromas {
    /// Each whole degree's chroma for `theme`, found again only when its rules are not the last.
    fn of(&mut self, theme: &ResolvedTheme) -> &[tint::Most] {
        let held = tint::held_rules(theme);
        let shared = tint::shared_rules(theme);
        let rules: Vec<(String, bool, String, bool)> = held
            .iter()
            .map(|rule| {
                (
                    rule.token.to_owned(),
                    rule.token_tinted,
                    rule.from.to_owned(),
                    rule.from_tinted,
                )
            })
            .collect();
        if self.at.is_empty() || rules != self.rules {
            self.at = KNOWN.with(|known| {
                let mut known = known.borrow_mut();
                if let Some((_, at)) = known.iter().find(|(them, _)| *them == rules) {
                    return at.clone();
                }
                let at: Vec<tint::Most> = (0..360)
                    .map(|degree| tint::most_of(&held, &shared, f64::from(degree)))
                    .collect();
                if known.len() == KNOWN_MOST {
                    known.remove(0);
                }
                known.push((rules.clone(), at.clone()));
                at
            });
            self.rules = rules;
        }
        &self.at
    }
}

/// How many recipes' chromas a thread keeps between draws: a window draws the same few themes
/// again and again, and finding each degree's chroma is most of what holding the floors costs.
const KNOWN_MOST: usize = 8;

/// The recipes' chromas this thread found last, by their rules.
type Known = Vec<(Vec<(String, bool, String, bool)>, Vec<tint::Most>)>;

thread_local! {
    static KNOWN: std::cell::RefCell<Known> = const { std::cell::RefCell::new(Vec::new()) };
}

fn failures(theme: &ResolvedTheme, chromas: &mut Chromas) -> Vec<Failure> {
    let mut found = Vec::new();
    // With more contrast asked for, every consent pair is held at its raised floor (#2925), in
    // a tint as in the theme as written.
    let failing = |drawn: &ResolvedTheme, tinted: bool, (floor, pair): (Floor, &Pair)| {
        ratio(drawn, pair)
            .is_some_and(|ratio| ratio < drawn.least.floor(pair.floor))
            .then(|| Failure {
                floor,
                tinted,
                tokens: pair.tokens().collect(),
            })
    };
    let tints = theme
        .tint
        .hues
        .iter()
        .map(|one| tint::tinted(theme, one.name));
    for (at, drawn) in std::iter::once(theme.clone()).chain(tints).enumerate() {
        let tinted = at != 0;
        found.extend(pairs().filter_map(|one| failing(&drawn, tinted, one)));
        // Every rule the tint holds apart, as written and under each of the theme's own tints:
        // the tint holds them itself (`tint::apart_chroma`), and the floors measure them too.
        for rule in &tint::APART {
            if apart_of(drawn.value(rule.token), drawn.value(rule.from)) < rule.distance {
                found.push(Failure {
                    floor: apart_floor(rule),
                    tinted,
                    tokens: vec![rule.from, rule.token],
                });
            }
        }
        found.extend(allow_near_primary(&drawn, tinted));
        if BAR_GROUNDS.iter().any(|ground| {
            counts_near(
                drawn.value(HOT_COUNT),
                drawn.value(PLAIN_COUNT),
                drawn.value(ground),
            )
        }) {
            found.push(hot_count_near(tinted));
        }
    }
    // The plain count's pill at every whole degree a workspace's own colour can take: its own and
    // its grounds' tokens turned, as `tint::turned_at` draws them, and the hot count, which a tint
    // never turns. A ground only matters under a see-through colour.
    let tabs = theme.tint.tabs();
    let see_through =
        !check::is_opaque(theme.value(PLAIN_COUNT)) || !check::is_opaque(theme.value(HOT_COUNT));
    if tabs.contains(&PLAIN_COUNT)
        || (see_through && BAR_GROUNDS.iter().any(|ground| tabs.contains(ground)))
    {
        let near = chromas.of(theme).iter().zip(0_u32..).any(|(most, degree)| {
            let turned = |token: &'static str| {
                tint::chroma_of(token, *most)
                    .filter(|_| tabs.contains(&token))
                    .map_or_else(
                        || theme.value(token).to_owned(),
                        |chroma| {
                            tint::tint_hex_within(theme.value(token), f64::from(degree), chroma)
                        },
                    )
            };
            let plain = turned(PLAIN_COUNT);
            BAR_GROUNDS
                .iter()
                .any(|ground| counts_near(theme.value(HOT_COUNT), &plain, &turned(ground)))
        });
        if near {
            found.push(hot_count_near(true));
        }
    }
    // Allow against the primary at every whole degree a workspace's own colour can take, as the
    // contrast floors below are: only the fills and the grounds they are measured on are turned.
    if !theme.tint.tabs().is_empty() {
        let only: Vec<&'static str> = super::parts::allow_from_primary(theme)
            .iter()
            .flat_map(|look| look.fills)
            .chain(super::parts::allow_grounds())
            .collect();
        let turned: Vec<&'static str> = theme
            .tint
            .tabs()
            .iter()
            .copied()
            .filter(|token| only.contains(token))
            .collect();
        if !turned.is_empty() {
            // One copy, its few turned tokens rewritten at each degree, as `tint::turned_at`
            // draws them; a degree with no chroma that holds the tint apart is drawn untinted.
            let mut drawn = theme.clone();
            for (degree, most) in (0_u32..).zip(chromas.of(theme).to_vec()) {
                for token in &turned {
                    let chroma = tint::chroma_of(token, most);
                    if let Some(value) = drawn.values.get_mut(token) {
                        *value = chroma.map_or_else(
                            || theme.value(token).to_owned(),
                            |chroma| {
                                tint::tint_hex_within(theme.value(token), f64::from(degree), chroma)
                            },
                        );
                    }
                }
                found.extend(allow_near_primary(&drawn, true));
            }
        }
    }
    // A workspace's own colour can take any hue. A tint keeps each shade's luminance on its own
    // side of the middle, so a pair of a dark shade and a light one holds at every hue as it does
    // at the palette's; the pairs that can move between hues are measured at every whole degree.
    let moving: Vec<(Floor, &Pair)> = pairs()
        .filter(|(_, pair)| moves_with_hue(theme, pair))
        .collect();
    if !moving.is_empty() && !theme.tint.tabs().is_empty() {
        // Only the tokens of those pairs are turned: each is drawn as the whole tint draws it.
        let only: Vec<&'static str> = moving.iter().flat_map(|(_, pair)| pair.tokens()).collect();
        for (degree, most) in (0_u32..).zip(chromas.of(theme).to_vec()) {
            let drawn = tint::turned_at(theme, f64::from(degree), most, &only);
            found.extend(moving.iter().filter_map(|one| failing(&drawn, true, *one)));
        }
    }
    found
}

/// Whether the two counts, laid on `ground` as the screen draws them, are nearer than the floor.
fn counts_near(hot: &str, plain: &str, ground: &str) -> bool {
    apart_of(&check::onto(hot, ground), &check::onto(plain, ground)) < HOT_COUNT_APART
}

/// The hot count drawn too near the plain count, the hot count's fill named first: purlis's own
/// is drawn before the ground it is measured against is.
fn hot_count_near(tinted: bool) -> Failure {
    Failure {
        floor: Floor::HotCountApart,
        tinted,
        tokens: vec![HOT_COUNT, PLAIN_COUNT],
    }
}

/// Each state in which `drawn`'s tokens bring Allow nearer the primary than [`ALLOW_FROM_PRIMARY`]: the
/// primary's fill, then Allow's, the order they are drawn as purlis's own in.
fn allow_near_primary(drawn: &ResolvedTheme, tinted: bool) -> Vec<Failure> {
    let mut found: Vec<Failure> = Vec::new();
    for look in super::parts::allow_from_primary(drawn) {
        let tokens = look.fills.to_vec();
        if look.apart < ALLOW_FROM_PRIMARY && !found.iter().any(|one| one.tokens == tokens) {
            found.push(Failure {
                floor: Floor::AllowNotPrimary,
                tinted,
                tokens,
            });
        }
    }
    found
}

/// Whether a tint can move `pair`'s contrast from one hue to another: the recipe turns one of its
/// tokens, and the pair's colours are mixed as they are drawn (words on a wash, faded words, or a
/// colour written see-through), or a turned shade is read against a colour on its own side of the
/// middle and no farther from it, so rounding the shade away from the middle brings it nearer.
pub(super) fn moves_with_hue(theme: &ResolvedTheme, pair: &Pair) -> bool {
    let turned = |token: &str| theme.tint.tabs().contains(&token);
    if !pair.tokens().any(turned) {
        return false;
    }
    let see_through = |token: &str| matches!(theme.value(token).len(), 5 | 9);
    if pair.under.is_some() || pair.fade.is_some() || pair.tokens().any(see_through) {
        return true;
    }
    // The two as the screen draws them, any alpha laid over what it is drawn on.
    let Some((front, back)) = check::drawn(theme, pair) else {
        return false;
    };
    let (Some(front), Some(back)) = (tint::luminance_of(&front), tint::luminance_of(&back)) else {
        return false;
    };
    let nearer = |one: f64, other: f64| {
        tint::light_side(one) == tint::light_side(other)
            && tint::from_middle(one) <= tint::from_middle(other)
    };
    (turned(pair.front) && nearer(front, back)) || (turned(pair.back) && nearer(back, front))
}

/// The floor a rule of [`tint::APART`] keeps: danger apart from primary for a rule from the
/// danger colour, waiting apart from the accent for one from the waiting colour, and Allow
/// apart from Keep blocked for the other.
fn apart_floor(rule: &tint::Apart) -> Floor {
    match rule.from {
        "danger.base" => Floor::DangerApart,
        "state.waiting" => Floor::WaitingApart,
        _ => Floor::AllowApart,
    }
}

/// How far two colours are in OKLab; none at all for a value that is not a colour.
fn apart_of(one: &str, other: &str) -> f64 {
    let (Some(one), Some(other)) = (tint::oklab_of(one), tint::oklab_of(other)) else {
        return 0.0;
    };
    tint::distance(one, other)
}

/// How far `danger.base` is from `accent.base` in OKLab, as the theme writes them.
#[cfg(test)]
pub(super) fn apart(theme: &ResolvedTheme) -> f64 {
    apart_of(theme.value("danger.base"), theme.value("accent.base"))
}

/// How far Allow's edge, `accent.base`, is from Keep blocked's, `border.control`, in OKLab, as
/// the theme writes them.
#[cfg(test)]
pub(super) fn allow_apart(theme: &ResolvedTheme) -> f64 {
    apart_of(theme.value("accent.base"), theme.value("border.control"))
}

/// **`theme` with the consent floors held**: as it is when it clears them; otherwise with
/// Purlis Classic's values of its appearance for the tokens of the pairs that fail, and what was
/// put right.
pub(super) fn hold(theme: &ResolvedTheme) -> (ResolvedTheme, Option<Held>) {
    // With more contrast asked for, purlis's own values are its own raised (#2925), so a token
    // put back is put back at the raised floors.
    let raised;
    let ours = if theme.least == super::Least::More {
        raised = super::least::raise(built_in_for(theme.appearance));
        &raised
    } else {
        built_in_for(theme.appearance)
    };
    let mut drawn = theme.clone();
    let mut floors = BTreeSet::new();
    let mut taken: BTreeSet<&'static str> = BTreeSet::new();
    let mut untinted: BTreeSet<&'static str> = BTreeSet::new();
    let mut chromas = Chromas::default();
    // A count the theme draws as its mark falls with it, whichever of the two a floor names.
    let following = hot_count_following(&theme.values);
    loop {
        let failing = failures(&drawn, &mut chromas);
        if failing.is_empty() {
            break;
        }
        // A theme that clears every floor as written, and falls under one only in a tinted
        // workspace, keeps its own values: the tint leaves the tokens it would turn under it.
        if failing.iter().all(|failure| failure.tinted) {
            let turned: Vec<&'static str> = failing
                .iter()
                .flat_map(|failure| failure.tokens.iter().copied())
                .filter(|token| drawn.tint.tabs().contains(token))
                .collect();
            if !turned.is_empty() {
                floors.extend(failing.iter().map(|failure| failure.floor));
                drawn.tint.leave(&turned);
                untinted.extend(turned);
                continue;
            }
        }
        let failing: Vec<&Failure> = if failing.iter().any(|failure| !failure.tinted) {
            failing.iter().filter(|failure| !failure.tinted).collect()
        } else {
            failing.iter().collect()
        };
        // Each round decides from what was purlis's before it, so a pair failing in several
        // tints is one failure and its ground waits for the round after its consent colour.
        let mut take: BTreeSet<&'static str> = BTreeSet::new();
        for failure in &failing {
            floors.insert(failure.floor);
            let open: Vec<&'static str> = failure
                .tokens
                .iter()
                .copied()
                .filter(|token| !taken.contains(token))
                .collect();
            let consent: Vec<&'static str> = open
                .iter()
                .copied()
                .filter(|token| is_consent(token))
                .collect();
            // A selection that hides what it is laid on is put right first: purlis's own
            // selection is the platform's, so the pair is no longer drawn and its consent colour
            // and its ground stay as the theme wrote them. Otherwise purlis's own consent colour
            // first, and the ground too only once that is not enough.
            let selection: Vec<&'static str> = open
                .iter()
                .copied()
                .filter(|token| token.starts_with("selection."))
                .collect();
            take.extend(if !selection.is_empty() {
                selection
            } else if failure.floor == Floor::AllowNotPrimary {
                // The primary's fill first, so Allow stays as plain as Keep blocked; Allow's own
                // fill only once the primary's is purlis's and that is not enough.
                open.into_iter().take(1).collect()
            } else if consent.is_empty() {
                open
            } else {
                consent
            });
        }
        // Every token a failing floor names is purlis's already, and a workspace's tint still
        // breaks it: the tint leaves those tokens as they are drawn.
        if take.is_empty() {
            let tinted: Vec<&'static str> = failing
                .iter()
                .flat_map(|failure| failure.tokens.iter().copied())
                .filter(|token| drawn.tint.tabs().contains(token))
                .collect();
            if tinted.is_empty() {
                break;
            }
            drawn.tint.leave(&tinted);
            untinted.extend(tinted);
            continue;
        }
        for (count, mark) in &following {
            if take.contains(count) || take.contains(mark) {
                take.extend([*count, *mark]);
            }
        }
        for token in take {
            drawn.values.insert(token, ours.value(token).to_owned());
            taken.insert(token);
        }
    }
    if floors.is_empty() {
        return (drawn, None);
    }
    // A token the theme already gave purlis's value is not one it is drawn without.
    let tokens = taken
        .into_iter()
        .filter(|token| theme.value(token) != drawn.value(token))
        .collect();
    let held = Held {
        theme: theme_name(theme),
        floors: floors.into_iter().collect(),
        tokens,
        untinted: untinted.into_iter().collect(),
    };
    (drawn, Some(held))
}

/// What a Notice calls a theme: its name, and its appearance's id when it has several.
fn theme_name(theme: &ResolvedTheme) -> String {
    let tag = match theme.appearance {
        Appearance::Dark => "dark",
        Appearance::Light => "light",
    };
    if theme.id == tag {
        theme.name.clone()
    } else {
        format!("{} ({})", theme.name, theme.id)
    }
}
