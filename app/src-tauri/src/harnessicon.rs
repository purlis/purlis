// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **What a profile is drawn with, as the window is handed it** (#2922): the core's
//! [`purlis_core::harness_icon::Drawn`] and the SVGs of an answer
//! ([`purlis_core::harness_icon::Answer`]), in the window's types.

use std::collections::BTreeMap;

use purlis_core::harness_icon::{Answer, Drawn};
use purlis_core::safesvg::SafeSvg;

/// A profile's icon: the glyph of the theme's icon set it is drawn with, or the key of the
/// person's own SVG among the answer's, and always the monogram of its harness kind, which the
/// window draws where it can draw neither.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
pub struct HarnessIconRow {
    /// `spark`, `braces` or `terminal`.
    pub glyph: Option<String>,
    /// The file name its SVG is under in the answer's own icons; each is sent once, however
    /// many rows name it.
    pub own: Option<String>,
    /// `C`, `OC`, `CX`.
    pub monogram: String,
}

/// A person's SVG as checked shapes (`purlis_core::safesvg`): the window draws each with its
/// own element, and never reads it as markup.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
pub struct SvgIcon {
    /// `min-x min-y width height`.
    pub view_box: String,
    pub shapes: Vec<SvgShape>,
}

/// One shape: `path`, `circle`, `ellipse`, `rect`, `line`, `polyline` or `polygon`, and its
/// checked attributes by their SVG names.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
pub struct SvgShape {
    pub tag: String,
    pub attributes: BTreeMap<String, String>,
}

impl From<SafeSvg> for SvgIcon {
    fn from(svg: SafeSvg) -> Self {
        Self {
            view_box: svg.view_box,
            shapes: svg
                .shapes
                .into_iter()
                .map(|shape| SvgShape {
                    tag: shape.tag.to_owned(),
                    attributes: shape
                        .attributes
                        .into_iter()
                        .map(|(key, value)| (key.to_owned(), value))
                        .collect(),
                })
                .collect(),
        }
    }
}

impl From<Drawn> for HarnessIconRow {
    fn from(drawn: Drawn) -> Self {
        Self {
            glyph: drawn.glyph.map(str::to_owned),
            own: drawn.own,
            monogram: drawn.monogram,
        }
    }
}

/// **The icons of one answer**: every row's, with each person's SVG read once and held to the
/// answer's budget. The config home is asked for only once a row names a file in it.
#[derive(Default)]
pub struct Icons {
    answer: Answer,
    root: Option<Option<std::path::PathBuf>>,
}

impl Icons {
    /// What the profile `name` of `kind` is drawn with, `icon` what its definition names.
    pub fn row(&mut self, name: &str, kind: &str, icon: Option<&str>) -> HarnessIconRow {
        let names_a_file = matches!(
            icon.map(purlis_core::harness_icon::named),
            Some(purlis_core::harness_icon::Named::File(_))
        );
        if names_a_file && self.root.is_none() {
            self.root = Some(purlis_core::machine::config_root_if_there());
        }
        let root = self.root.clone().flatten();
        self.answer.drawn(name, kind, icon, root.as_deref()).into()
    }

    /// The answer's own SVGs, by the file name rows name them by.
    pub fn svgs(self) -> BTreeMap<String, SvgIcon> {
        self.answer
            .svgs
            .into_iter()
            .map(|(file, svg)| (file, svg.into()))
            .collect()
    }
}

/// What a profile that names no icon of its own is drawn with: its harness's glyph, where it
/// stands for the built-in, else its kind's monogram.
pub fn of_kind(name: &str, kind: &str) -> HarnessIconRow {
    Answer::default().drawn(name, kind, None, None).into()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_built_in_is_handed_its_glyph_and_its_monogram() {
        let row = of_kind("claude", "claude");
        assert_eq!(row.glyph.as_deref(), Some("spark"));
        assert_eq!(row.own, None);
        assert_eq!(row.monogram, "C");
    }

    #[test]
    fn a_profile_naming_a_glyph_reads_nothing() {
        let mut icons = Icons::default();
        let row = icons.row("work", "codex", Some("braces"));
        assert_eq!(row.glyph.as_deref(), Some("braces"));
        assert_eq!(row.monogram, "CX");
        assert!(icons.root.is_none(), "the config home is not asked");
        assert!(icons.svgs().is_empty());
    }
}
