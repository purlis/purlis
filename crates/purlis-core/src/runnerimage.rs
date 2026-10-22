// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The runner image of a release** (#2749, ruled 2026-10-20): what a Docker pool of purlis's
//! own image moves to after the desktop updates.
//!
//! A release builds purlis's runner image with the version it stamps its app with, and the
//! update manifest names it: `"runnerImage": {"image": "<name>@sha256:<digest>", "signature":
//! "<base64 minisign signature>"}`, beside `version` (`crates/release-manifest`). The manifest
//! itself is not signed, so the image's line is: by the updater's own key, the one the app ships
//! and checks every bundle with (ADR 0042), with the release's version in its trusted comment,
//! as a bundle's is. [`of_this_version`] takes the image only where all of that holds, so a
//! manifest served by anyone else names no image a pool would run.

use crate::dockerpool::Image;

/// The manifest's key for it.
pub const KEY: &str = "runnerImage";

/// **The runner image the update manifest `manifest` names for `version`**, checked with the
/// updater's public key `pubkey` (the base64 `tauri.conf.json` carries): `None` where the
/// manifest is of another version or names none, as a release from before #2749 does.
///
/// Refused, in a sentence, where it names one that is not signed by that key over its exact
/// line, is signed for another version, is not named by its digest, or is not purlis's own
/// runner image.
pub fn of_this_version(
    manifest: &serde_json::Value,
    pubkey: &str,
    version: &str,
) -> Result<Option<Image>, String> {
    if manifest.get("version").and_then(serde_json::Value::as_str) != Some(version) {
        return Ok(None);
    }
    let Some(named) = manifest.get(KEY) else {
        return Ok(None);
    };
    let text = |key: &str| {
        named
            .get(key)
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| format!("the update manifest's {KEY} has no {key}"))
    };
    let (line, signature) = (text("image")?, text("signature")?);
    verified(pubkey, signature, line.as_bytes(), version)?;
    let image = Image::pinned(line).map_err(|why| why.0)?;
    if !image.is_purlis_own() {
        return Err(format!(
            "the update manifest names {line} as this version's runner image, which is not \
             purlis's own"
        ));
    }
    Ok(Some(image))
}

/// Whether `signature`, base64 of a minisign signature file, is the updater key `pubkey`'s
/// over `signed`, for `version` by its trusted comment's `version:` field.
fn verified(pubkey: &str, signature: &str, signed: &[u8], version: &str) -> Result<(), String> {
    use base64::Engine as _;
    let text = |b64: &str, what: &str| {
        base64::engine::general_purpose::STANDARD
            .decode(b64.trim())
            .ok()
            .and_then(|bytes| String::from_utf8(bytes).ok())
            .ok_or_else(|| format!("the {what} is not base64 text"))
    };
    let key = minisign_verify::PublicKey::decode(&text(pubkey, "updater's public key")?)
        .map_err(|why| format!("the updater's public key could not be read: {why}"))?;
    let signature =
        minisign_verify::Signature::decode(&text(signature, "runner image's signature")?)
            .map_err(|why| format!("the runner image's signature could not be read: {why}"))?;
    key.verify(signed, &signature, false).map_err(|why| {
        format!("the runner image's signature is not the updater key's over its line: {why}")
    })?;
    let signed_for = signature
        .trusted_comment()
        .split('\t')
        .find_map(|field| field.strip_prefix("version:"));
    if signed_for != Some(version) {
        return Err(format!(
            "the runner image is signed for {}, not for {version}",
            signed_for.unwrap_or("no version")
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests;
