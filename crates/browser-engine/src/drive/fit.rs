// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A screenshot made to fit what carries it** (#2013): a browser's answer crosses the session
//! protocol's control lane, whose frames are bounded, so a screenshot whose PNG is past the
//! bound is halved (each 2×2 block of pixels averaged into one) until it fits. Painting over
//! blanked fields happens before, so a halved blank is still a blank.

use std::io::Cursor;

/// The most times a screenshot is halved before it is refused: a sixteenth of its width.
const MOST_HALVINGS: u32 = 4;

/// A screenshot as it fits: its PNG, its size in pixels, and how many times it was halved.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Fitted {
    pub png: Vec<u8>,
    pub width: u32,
    pub height: u32,
    pub halved: u32,
}

/// `png` at most `most` bytes long: as it is where it fits, else halved until it does. Refused
/// in words where it cannot be read, or does not fit after [`MOST_HALVINGS`].
pub fn to_fit(png_bytes: &[u8], most: usize) -> Result<Fitted, String> {
    let (mut width, mut height, channels, mut pixels) = decode(png_bytes)?;
    if png_bytes.len() <= most {
        return Ok(Fitted {
            png: png_bytes.to_vec(),
            width,
            height,
            halved: 0,
        });
    }
    for halved in 1..=MOST_HALVINGS {
        (width, height, pixels) = halve(width, height, channels, &pixels);
        let png = encode(width, height, channels, &pixels)?;
        if png.len() <= most {
            return Ok(Fitted {
                png,
                width,
                height,
                halved,
            });
        }
    }
    Err(format!(
        "the screenshot is larger than {most} bytes even at a sixteenth of its size"
    ))
}

/// The image's size, its samples a pixel (1 to 4, eight bits each), and its pixels in rows.
fn decode(png_bytes: &[u8]) -> Result<(u32, u32, usize, Vec<u8>), String> {
    let mut decoder = png::Decoder::new(Cursor::new(png_bytes));
    decoder.set_transformations(png::Transformations::EXPAND | png::Transformations::STRIP_16);
    let mut reader = decoder
        .read_info()
        .map_err(|e| format!("the screenshot could not be read: {e}"))?;
    let size = reader
        .output_buffer_size()
        .ok_or_else(|| "the screenshot is too large to read".to_owned())?;
    let mut data = vec![0; size];
    let info = reader
        .next_frame(&mut data)
        .map_err(|e| format!("the screenshot could not be read: {e}"))?;
    let channels = info.color_type.samples();
    // Rows packed with no gap between them.
    let row = info.width as usize * channels;
    let mut pixels = Vec::with_capacity(row * info.height as usize);
    for y in 0..info.height as usize {
        let at = y * info.line_size;
        pixels.extend_from_slice(&data[at..at + row]);
    }
    Ok((info.width, info.height, channels, pixels))
}

/// Each 2×2 block averaged into one pixel; an odd last row or column is dropped.
fn halve(width: u32, height: u32, channels: usize, pixels: &[u8]) -> (u32, u32, Vec<u8>) {
    let (w, h) = ((width / 2).max(1), (height / 2).max(1));
    let row = width as usize * channels;
    let mut out = Vec::with_capacity(w as usize * h as usize * channels);
    for y in 0..h as usize {
        for x in 0..w as usize {
            for c in 0..channels {
                let mut sum = 0u32;
                for (dy, dx) in [(0, 0), (0, 1), (1, 0), (1, 1)] {
                    let yy = (2 * y + dy).min(height as usize - 1);
                    let xx = (2 * x + dx).min(width as usize - 1);
                    sum += u32::from(pixels[yy * row + xx * channels + c]);
                }
                out.push(u8::try_from(sum / 4).unwrap_or(u8::MAX));
            }
        }
    }
    (w, h, out)
}

fn encode(width: u32, height: u32, channels: usize, pixels: &[u8]) -> Result<Vec<u8>, String> {
    let color = match channels {
        1 => png::ColorType::Grayscale,
        2 => png::ColorType::GrayscaleAlpha,
        3 => png::ColorType::Rgb,
        _ => png::ColorType::Rgba,
    };
    let mut out = Vec::new();
    {
        let mut encoder = png::Encoder::new(&mut out, width, height);
        encoder.set_color(color);
        encoder.set_depth(png::BitDepth::Eight);
        encoder.set_compression(png::Compression::High);
        encoder
            .write_header()
            .and_then(|mut writer| writer.write_image_data(pixels))
            .map_err(|e| format!("the screenshot could not be written again: {e}"))?;
    }
    Ok(out)
}

#[cfg(test)]
mod tests;
