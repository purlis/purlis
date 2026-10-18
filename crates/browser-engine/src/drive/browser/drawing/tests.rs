// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

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

#[test]
fn a_box_whose_corners_cross_has_no_drawing() {
    // A bow tie: the top edge drawn from (0, 0) to (100, 100) and the bottom from (100, 0) to
    // (0, 100), the box folded through itself.
    let crossed = [0.0, 0.0, 100.0, 100.0, 100.0, 0.0, 0.0, 100.0];
    assert_eq!(Drawing::of(&crossed, 100.0, 100.0), None);
}

#[test]
fn a_box_drawn_across_the_horizon_has_no_drawing() {
    // A perspective whose horizon passes through the box: a far corner is drawn behind the eye.
    let corners = [0.0, 0.0, 100.0, 0.0, -400.0, -500.0, 500.0, -500.0];
    assert_eq!(Drawing::of(&corners, 100.0, 100.0), None);
}
