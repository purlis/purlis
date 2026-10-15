// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use super::{Rect, paint_over};

/// A `width` by `height` image of one colour, as the engine's PNG.
fn image(width: u32, height: u32, color: png::ColorType, pixel: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    {
        let mut encoder = png::Encoder::new(&mut out, width, height);
        encoder.set_color(color);
        encoder.set_depth(png::BitDepth::Eight);
        let mut writer = encoder.write_header().unwrap();
        let data: Vec<u8> = (0..width * height).flat_map(|_| pixel.to_vec()).collect();
        writer.write_image_data(&data).unwrap();
    }
    out
}

fn pixels(png_bytes: &[u8]) -> (u32, Vec<u8>, usize) {
    let mut reader = png::Decoder::new(std::io::Cursor::new(png_bytes))
        .read_info()
        .unwrap();
    let mut buf = vec![0; reader.output_buffer_size().unwrap()];
    let info = reader.next_frame(&mut buf).unwrap();
    let channels = info.line_size / info.width as usize;
    (info.width, buf, channels)
}

fn at(image: &(u32, Vec<u8>, usize), x: u32, y: u32) -> &[u8] {
    let (width, data, channels) = image;
    let start = (y * width + x) as usize * channels;
    &data[start..start + channels]
}

#[test]
fn a_blanked_rectangle_is_painted_over_and_nothing_else_changes() {
    let white = image(10, 10, png::ColorType::Rgba, &[255, 255, 255, 255]);
    let painted = paint_over(
        &white,
        &[Rect {
            x: 2.0,
            y: 3.0,
            width: 4.0,
            height: 2.0,
        }],
    )
    .unwrap();
    let image = pixels(&painted);
    assert_eq!(at(&image, 2, 3), [128, 128, 128, 255]);
    assert_eq!(at(&image, 5, 4), [128, 128, 128, 255]);
    assert_eq!(at(&image, 6, 4), [255, 255, 255, 255]);
    assert_eq!(at(&image, 2, 5), [255, 255, 255, 255]);
    assert_eq!(at(&image, 1, 3), [255, 255, 255, 255]);
}

#[test]
fn a_rectangle_partly_off_the_image_is_painted_where_it_is_on_it() {
    let white = image(4, 4, png::ColorType::Rgb, &[255, 255, 255]);
    let painted = paint_over(
        &white,
        &[
            Rect {
                x: -3.0,
                y: 2.5,
                width: 5.0,
                height: 100.0,
            },
            Rect {
                x: 50.0,
                y: 50.0,
                width: 5.0,
                height: 5.0,
            },
        ],
    )
    .unwrap();
    let image = pixels(&painted);
    assert_eq!(at(&image, 0, 3), [128, 128, 128]);
    assert_eq!(at(&image, 1, 2), [128, 128, 128]);
    assert_eq!(at(&image, 2, 3), [255, 255, 255]);
    assert_eq!(at(&image, 0, 1), [255, 255, 255]);
}

#[test]
fn what_is_not_an_image_is_refused_rather_than_passed_on() {
    assert!(paint_over(b"not a picture", &[]).is_err());
}
