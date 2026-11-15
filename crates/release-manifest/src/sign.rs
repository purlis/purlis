// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **Signing the runner image's line with the updater's key** (#2749): what `sign-runner-image`
//! does in the release environment, the one place the key is. The `.sig` is the one the bundler
//! writes for a bundle: base64 of a minisign signature file whose trusted comment carries the
//! release's version, so the app checks it as it checks a bundle (ADR 0042).
//!
//! `sign-runner-binary` signs each runner platform's `purlis` the same way ([`signed_runner`],
//! RR-14, #723), with the asset's own name in the trusted comment beside the version.

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
    signed_as(
        line.as_bytes(),
        "runner-image",
        key,
        password,
        version,
        timestamp,
    )
    .map_err(|why| why.replace("{what}", "the runner image's line"))
}

/// **The `.sig` of a runner binary** (RR-14, #723; ADR 0042 as amended by ADR 0078): `bytes`,
/// the `purlis` built for `target`, signed by the updater's key with `version:<version>` and
/// `file:purlis-<version>-<target>` in the trusted comment, which is what
/// `purlis_core::runnerinstall::verify` checks before the desktop trusts its digest. The
/// release publishes it beside the asset, as `<asset>.sig`.
pub fn signed_runner(
    bytes: &[u8],
    target: purlis_core::runnerinstall::Target,
    key: &str,
    password: Option<&str>,
    version: &str,
    timestamp: u64,
) -> Result<String, String> {
    if !purlis_core::runnerinstall::a_version(version) {
        return Err(format!(
            "{version} isn't a version a runner binary is published under"
        ));
    }
    let file = purlis_core::runnerinstall::asset_name(version, target);
    signed_as(bytes, &file, key, password, version, timestamp)
        .map_err(|why| why.replace("{what}", &format!("the runner binary {file}")))
}

/// `bytes` signed as `file` for `version`; a failure's sentence names what was signed as
/// `{what}`, for the caller to fill in.
fn signed_as(
    bytes: &[u8],
    file: &str,
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
    let trusted = format!("timestamp:{timestamp}\tfile:{file}\tversion:{version}");
    let signature = minisign::sign(
        None,
        &secret,
        std::io::Cursor::new(bytes),
        Some(&trusted),
        Some("signature from tauri secret key"),
    )
    .map_err(|why| format!("{{what}} could not be signed: {why}"))?;
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
