// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use super::{Fitted, to_fit};

/// A `width` by `height` RGB image of noise, which PNG cannot squeeze: its size is about its
/// pixels'.
fn noise(width: u32, height: u32) -> Vec<u8> {
    let mut seed: u32 = 0x2013;
    let data: Vec<u8> = (0..width * height * 3)
        .map(|_| {
            seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            (seed >> 24) as u8
        })
        .collect();
    let mut out = Vec::new();
    {
        let mut encoder = png::Encoder::new(&mut out, width, height);
        encoder.set_color(png::ColorType::Rgb);
        encoder.set_depth(png::BitDepth::Eight);
        encoder
            .write_header()
            .unwrap()
            .write_image_data(&data)
            .unwrap();
    }
    out
}

fn size_of(png_bytes: &[u8]) -> (u32, u32) {
    let reader = png::Decoder::new(std::io::Cursor::new(png_bytes))
        .read_info()
        .unwrap();
    (reader.info().width, reader.info().height)
}

#[test]
fn a_screenshot_within_the_bound_is_passed_on_as_it_is() {
    let small = noise(40, 30);

    let fitted = to_fit(&small, small.len()).unwrap();

    assert_eq!(
        fitted,
        Fitted {
            png: small,
            width: 40,
            height: 30,
            halved: 0,
        }
    );
}

#[test]
fn a_screenshot_past_the_bound_is_halved_until_it_fits() {
    let big = noise(400, 200);
    let bound = big.len() / 3;

    let fitted = to_fit(&big, bound).unwrap();

    assert!(fitted.png.len() <= bound, "{} > {bound}", fitted.png.len());
    assert_eq!(
        fitted.halved, 1,
        "a quarter of the pixels fits a third of the bytes"
    );
    assert_eq!((fitted.width, fitted.height), (200, 100));
    assert_eq!(
        size_of(&fitted.png),
        (200, 100),
        "the image is what it says"
    );
}

#[test]
fn a_screenshot_that_cannot_be_made_to_fit_is_refused() {
    let big = noise(64, 64);

    assert!(to_fit(&big, 10).is_err());
}
