// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **Signing the runner image's line with the updater's key** (#2749): what `sign-runner-image`
//! does in the release environment, the one place the key is. The `.sig` is the one the bundler
//! writes for a bundle: base64 of a minisign signature file whose trusted comment carries the
//! release's version, so the app checks it as it checks a bundle (ADR 0042).

/// The `.sig` of `line` by the secret key `key` (base64 of a minisign secret key file, as
/// `TAURI_SIGNING_PRIVATE_KEY` holds it), opened with `password`, for `version`, made at
/// `timestamp` seconds since the epoch; or why not.
pub fn signed(
    line: &str,
    key: &str,
    password: Option<&str>,
    version: &str,
    timestamp: u64,
) -> Result<String, String> {
    use base64_text::{decode, encode};
    let text = decode(key.trim()).ok_or("the updater's secret key is not base64 text")?;
    let sealed = || minisign::SecretKeyBox::from_string(&text);
    // Opened with its password, an empty one where none is given, as the bundler opens it; a
    // key made with no password at all is read as it is. Never asked for one on a terminal.
    let secret = sealed()
        .and_then(|sealed| sealed.into_secret_key(Some(password.unwrap_or_default().to_owned())))
        .or_else(|why| {
            sealed()
                .and_then(minisign::SecretKeyBox::into_unencrypted_secret_key)
                .map_err(|_| why)
        })
        .map_err(|why| format!("the updater's secret key could not be opened: {why}"))?;
    let trusted = format!("timestamp:{timestamp}\tfile:runner-image\tversion:{version}");
    let signature = minisign::sign(
        None,
        &secret,
        std::io::Cursor::new(line.as_bytes()),
        Some(&trusted),
        Some("signature from tauri secret key"),
    )
    .map_err(|why| format!("the runner image's line could not be signed: {why}"))?;
    Ok(encode(signature.into_string().as_bytes()))
}

mod base64_text {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

    pub fn encode(bytes: &[u8]) -> String {
        let mut out = String::new();
        for chunk in bytes.chunks(3) {
            let n = chunk.len();
            let b = [
                chunk[0],
                *chunk.get(1).unwrap_or(&0),
                *chunk.get(2).unwrap_or(&0),
            ];
            let v = (u32::from(b[0]) << 16) | (u32::from(b[1]) << 8) | u32::from(b[2]);
            for i in 0..4 {
                if i <= n {
                    out.push(char::from(ALPHABET[((v >> (18 - 6 * i)) & 63) as usize]));
                } else {
                    out.push('=');
                }
            }
        }
        out
    }

    pub fn decode(text: &str) -> Option<String> {
        super::super::decode_base64_text(text)
    }
}

#[cfg(test)]
mod tests;
