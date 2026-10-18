// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! Where a point of an element's box is drawn on its page, however the page turns, scales,
//! skews, zooms or puts the element in perspective (#2557).
//!
//! The engine says where the four corners of the box are drawn and how large the box is laid
//! out. Every transform CSS can draw a flat box with is a projective map, and four corners fix
//! one: the map from the laid-out box to the page is the one that takes its corners to those.

/// How an element's box, laid out at its own size, is drawn on its page.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct Drawing {
    /// From a point of the box, in its own CSS pixels from its top left corner, to the page:
    /// row by row, the last row dividing.
    map: [f64; 9],
}

impl Drawing {
    /// The box laid out `width` by `height` CSS pixels and drawn at `corners` (top left, top
    /// right, bottom right, bottom left, as `DOM.getBoxModel` gives them); `None` when it is not
    /// drawn as an area, so no point of it can be told from another.
    pub(super) fn of(corners: &[f64; 8], width: f64, height: f64) -> Option<Self> {
        if !(width > 0.0 && height > 0.0) {
            return None;
        }
        let [x0, y0, x1, y1, x2, y2, x3, y3] = *corners;
        // The map from the unit square to the corners (Heckbert's square to quadrilateral).
        let sx = x0 - x1 + x2 - x3;
        let sy = y0 - y1 + y2 - y3;
        let (dx1, dx2, dy1, dy2) = (x1 - x2, x3 - x2, y1 - y2, y3 - y2);
        let den = dx1 * dy2 - dx2 * dy1;
        if den.abs() < f64::EPSILON {
            return None;
        }
        let g = (sx * dy2 - dx2 * sy) / den;
        let h = (dx1 * sy - sx * dy1) / den;
        let unit = [
            x1 - x0 + g * x1,
            x3 - x0 + h * x3,
            x0,
            y1 - y0 + g * y1,
            y3 - y0 + h * y3,
            y0,
            g,
            h,
            1.0,
        ];
        // From the box's own pixels to the unit square first.
        let map = [
            unit[0] / width,
            unit[1] / height,
            unit[2],
            unit[3] / width,
            unit[4] / height,
            unit[5],
            unit[6] / width,
            unit[7] / height,
            unit[8],
        ];
        let drawing = Self { map };
        drawing.inverse().map(|_| drawing)
    }

    /// Where `point` of the box is drawn on the page; `None` when it is not drawn in front.
    pub(super) fn on_page(&self, point: (f64, f64)) -> Option<(f64, f64)> {
        apply(&self.map, point)
    }

    /// The point of the box drawn at `point` of the page.
    pub(super) fn in_box(&self, point: (f64, f64)) -> Option<(f64, f64)> {
        apply(&self.inverse()?, point)
    }

    fn inverse(&self) -> Option<[f64; 9]> {
        let [a, b, c, d, e, f, g, h, i] = self.map;
        let det = a * (e * i - f * h) - b * (d * i - f * g) + c * (d * h - e * g);
        if !det.is_finite() || det.abs() < 1e-12 {
            return None;
        }
        Some([
            (e * i - f * h) / det,
            (c * h - b * i) / det,
            (b * f - c * e) / det,
            (f * g - d * i) / det,
            (a * i - c * g) / det,
            (c * d - a * f) / det,
            (d * h - e * g) / det,
            (b * g - a * h) / det,
            (a * e - b * d) / det,
        ])
    }
}

fn apply(map: &[f64; 9], (x, y): (f64, f64)) -> Option<(f64, f64)> {
    let w = map[6] * x + map[7] * y + map[8];
    // At or behind the eye in a perspective, or not a number: drawn nowhere.
    if w.is_nan() || w <= f64::EPSILON {
        return None;
    }
    let out = (
        (map[0] * x + map[1] * y + map[2]) / w,
        (map[3] * x + map[4] * y + map[5]) / w,
    );
    (out.0.is_finite() && out.1.is_finite()).then_some(out)
}

#[cfg(test)]
mod tests {
    use super::Drawing;

    fn near(a: (f64, f64), b: (f64, f64)) -> bool {
        (a.0 - b.0).abs() < 1e-9 && (a.1 - b.1).abs() < 1e-9
    }

    #[test]
    fn an_upright_box_is_drawn_moved_only() {
        let drawing = Drawing::of(
            &[10.0, 20.0, 110.0, 20.0, 110.0, 70.0, 10.0, 70.0],
            100.0,
            50.0,
        )
        .unwrap();
        assert!(near(drawing.on_page((30.0, 5.0)).unwrap(), (40.0, 25.0)));
    }

    #[test]
    fn a_turned_and_scaled_box_is_drawn_turned_and_scaled() {
        // A quarter turn clockwise at half size about (100, 200).
        let drawing = Drawing::of(
            &[100.0, 200.0, 100.0, 310.0, 60.0, 310.0, 60.0, 200.0],
            220.0,
            80.0,
        )
        .unwrap();
        assert!(near(drawing.on_page((30.0, 25.0)).unwrap(), (87.5, 215.0)));
        assert!(near(drawing.in_box((87.5, 215.0)).unwrap(), (30.0, 25.0)));
    }

    #[test]
    fn a_box_in_perspective_takes_its_corners_and_middle_where_they_are_drawn() {
        // A square drawn as a trapezoid narrower at the top.
        let corners = [40.0, 0.0, 60.0, 0.0, 100.0, 100.0, 0.0, 100.0];
        let drawing = Drawing::of(&corners, 10.0, 10.0).unwrap();
        for (i, corner) in [(0.0, 0.0), (10.0, 0.0), (10.0, 10.0), (0.0, 10.0)]
            .into_iter()
            .enumerate()
        {
            let drawn = drawing.on_page(corner).unwrap();
            assert!(
                near(drawn, (corners[2 * i], corners[2 * i + 1])),
                "{corner:?} at {drawn:?}"
            );
        }
        // The middle is where the diagonals cross, nearer the narrow edge than halfway down.
        let middle = drawing.on_page((5.0, 5.0)).unwrap();
        assert!(
            (middle.0 - 50.0).abs() < 1e-9 && middle.1 < 50.0,
            "{middle:?}"
        );
        assert!(near(drawing.in_box(middle).unwrap(), (5.0, 5.0)));
    }

    #[test]
    fn a_box_drawn_flat_or_with_no_size_has_no_drawing() {
        let line = [0.0, 0.0, 10.0, 0.0, 10.0, 0.0, 0.0, 0.0];
        assert_eq!(Drawing::of(&line, 10.0, 10.0), None);
        let point = [5.0; 8];
        assert_eq!(Drawing::of(&point, 10.0, 10.0), None);
        let square = [0.0, 0.0, 10.0, 0.0, 10.0, 10.0, 0.0, 10.0];
        assert_eq!(Drawing::of(&square, 0.0, 10.0), None);
    }
}
