// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **Windows Hello, for real** (M75 slice 43): the WinRT `KeyCredentialManager`, through the
//! `windows` crate's safe wrappers, so no `unsafe` is written here.
//!
//! Hello keeps a non-exportable key under [`KEY_NAME`] (made the first time, Q320a N1), and signs
//! [`super::CHALLENGE`] with it after asking the person. **Not compiled or run on the machine this
//! was written on** (macOS): CI's nightly `windows` job and one manual check on a real device are
//! its evidence.

use windows::Security::Credentials::{
    KeyCredentialCreationOption, KeyCredentialManager, KeyCredentialStatus,
};
use windows::Storage::Streams::{DataReader, DataWriter, IBuffer};
use windows::core::HSTRING;
use zeroize::Zeroizing;

use super::{Answer, Confirm};

/// The name of the key Hello keeps for purlis's seal.
const KEY_NAME: &str = "purlis-seal";

/// Windows Hello, as this machine has it.
pub(super) struct WindowsHello;

impl Confirm for WindowsHello {
    fn confirm(&self, challenge: &[u8]) -> Answer {
        sign(challenge).unwrap_or(Answer::No)
    }
}

fn sign(challenge: &[u8]) -> windows::core::Result<Answer> {
    if !KeyCredentialManager::IsSupportedAsync()?.get()? {
        return Ok(Answer::Unavailable);
    }
    let name = HSTRING::from(KEY_NAME);
    let mut found = KeyCredentialManager::OpenAsync(&name)?.get()?;
    if found.Status()? == KeyCredentialStatus::NotFound {
        found = KeyCredentialManager::RequestCreateAsync(
            &name,
            KeyCredentialCreationOption::FailIfExists,
        )?
        .get()?;
    }
    match found.Status()? {
        KeyCredentialStatus::Success => {}
        KeyCredentialStatus::UserCanceled => return Ok(Answer::Cancelled),
        KeyCredentialStatus::SecurityDeviceLocked | KeyCredentialStatus::UserPrefersPassword => {
            return Ok(Answer::Unavailable);
        }
        _ => return Ok(Answer::No),
    }
    let writer = DataWriter::new()?;
    writer.WriteBytes(challenge)?;
    let signed = found
        .Credential()?
        .RequestSignAsync(&writer.DetachBuffer()?)?
        .get()?;
    match signed.Status()? {
        KeyCredentialStatus::Success => Ok(Answer::Yes(bytes(&signed.Result()?)?)),
        KeyCredentialStatus::UserCanceled => Ok(Answer::Cancelled),
        _ => Ok(Answer::No),
    }
}

fn bytes(buffer: &IBuffer) -> windows::core::Result<Zeroizing<Vec<u8>>> {
    let reader = DataReader::FromBuffer(buffer)?;
    let mut out = Zeroizing::new(vec![0u8; reader.UnconsumedBufferLength()? as usize]);
    reader.ReadBytes(&mut out)?;
    Ok(out)
}
