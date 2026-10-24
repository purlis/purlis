// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **What a profile is drawn with, as the window is handed it** (#2922): the core's
//! [`purlis_core::harness_icon::HarnessIcon`], in the window's types.

use std::collections::BTreeMap;

use purlis_core::harness_icon::HarnessIcon;
use purlis_core::safesvg::SafeSvg;

/// A profile's icon: the glyph of the theme's icon set it is drawn with, or the shapes of the
/// person's own SVG, and always the monogram of its harness kind, which the window draws where
/// it can draw neither.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
pub struct HarnessIconRow {
    /// `spark`, `braces` or `terminal`.
    pub glyph: Option<String>,
    pub svg: Option<SvgIcon>,
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

impl From<HarnessIcon> for HarnessIconRow {
    fn from(icon: HarnessIcon) -> Self {
        Self {
            glyph: icon.glyph.map(str::to_owned),
            svg: icon.svg.map(SvgIcon::from),
            monogram: icon.monogram,
        }
    }
}

/// What the profile `name` of `kind` is drawn with, `icon` what its definition names. The
/// config home is asked for only where the icon names a file in it.
pub fn of(name: &str, kind: &str, icon: Option<&str>) -> HarnessIconRow {
    let root = matches!(
        icon.map(purlis_core::harness_icon::named),
        Some(purlis_core::harness_icon::Named::File(_))
    )
    .then(purlis_core::machine::config_root_if_there)
    .flatten();
    purlis_core::harness_icon::of(name, kind, icon, root.as_deref()).into()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_built_in_is_handed_its_glyph_and_its_monogram() {
        let row = of("claude", "claude", None);
        assert_eq!(row.glyph.as_deref(), Some("spark"));
        assert_eq!(row.svg, None);
        assert_eq!(row.monogram, "C");
    }

    #[test]
    fn a_profile_naming_a_glyph_reads_nothing() {
        let row = of("work", "codex", Some("braces"));
        assert_eq!(row.glyph.as_deref(), Some("braces"));
        assert_eq!(row.monogram, "CX");
    }
}
