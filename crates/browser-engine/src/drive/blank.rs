// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! Blanking in a screenshot: each blanked element's box painted over in the image the engine
//! took, by purlis, after the engine took it. The page is never touched, so nothing a page's
//! script does can undo the blank, and a screenshot whose image purlis cannot read is refused
//! rather than passed on.

use std::io::Cursor;

/// A box in the image's pixels.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Rect {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

/// The image could not be read or written again.
#[derive(Debug, thiserror::Error)]
#[error("the screenshot could not be read to blank its fields: {0}")]
pub(crate) struct NotAnImage(String);

/// The grey a blanked box is painted.
const GREY: u8 = 128;

/// `png` with every one of `rects` painted grey: each pixel any part of a box covers.
pub(crate) fn paint_over(png_bytes: &[u8], rects: &[Rect]) -> Result<Vec<u8>, NotAnImage> {
    let mut decoder = png::Decoder::new(Cursor::new(png_bytes));
    decoder.set_transformations(png::Transformations::EXPAND | png::Transformations::STRIP_16);
    let mut reader = decoder.read_info().map_err(|e| NotAnImage(e.to_string()))?;
    let size = reader
        .output_buffer_size()
        .ok_or_else(|| NotAnImage("too large".into()))?;
    let mut data = vec![0; size];
    let info = reader
        .next_frame(&mut data)
        .map_err(|e| NotAnImage(e.to_string()))?;
    let (width, height) = (info.width, info.height);
    let channels = info.color_type.samples();
    let grey: &[u8] = match info.color_type {
        png::ColorType::Rgba => &[GREY, GREY, GREY, 255],
        png::ColorType::Rgb => &[GREY, GREY, GREY],
        png::ColorType::GrayscaleAlpha => &[GREY, 255],
        png::ColorType::Grayscale => &[GREY],
        png::ColorType::Indexed => return Err(NotAnImage("an indexed image".into())),
    };
    for rect in rects {
        let left = clamp(rect.x.floor(), width);
        let right = clamp((rect.x + rect.width).ceil(), width);
        let top = clamp(rect.y.floor(), height);
        let bottom = clamp((rect.y + rect.height).ceil(), height);
        for y in top..bottom {
            for x in left..right {
                let at = y * info.line_size + x * channels;
                data[at..at + channels].copy_from_slice(grey);
            }
        }
    }
    let mut out = Vec::new();
    {
        let mut encoder = png::Encoder::new(&mut out, width, height);
        encoder.set_color(info.color_type);
        encoder.set_depth(png::BitDepth::Eight);
        let mut writer = encoder
            .write_header()
            .map_err(|e| NotAnImage(e.to_string()))?;
        writer
            .write_image_data(&data[..info.line_size * height as usize])
            .map_err(|e| NotAnImage(e.to_string()))?;
    }
    Ok(out)
}

/// `edge` as a pixel index from 0 to `size`.
fn clamp(edge: f64, size: u32) -> usize {
    if edge.is_nan() || edge <= 0.0 {
        0
    } else if edge >= f64::from(size) {
        size as usize
    } else {
        // In range and whole: the cast loses nothing.
        edge as usize
    }
}

#[cfg(test)]
mod tests;
