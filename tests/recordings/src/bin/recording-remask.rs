// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A checked-in recording under the mask as it is now** (T1.1-03):
//!
//! ```text
//! recording-remask <recording.json>...
//! ```
//!
//! Each recording is masked again and written back, and the turn script beside it is made from
//! it again. A mask that hides more changes what a recording says only where it now hides it;
//! nothing else in it moves, and no recording is edited by hand
//! (`every_checked_in_recording_is_masked_and_reproduced_by_its_turn_script` holds them to it).

use std::process::ExitCode;

use purlis_recordings::recording::Recording;

fn main() -> ExitCode {
    let paths: Vec<String> = std::env::args().skip(1).collect();
    if paths.is_empty() {
        eprintln!("usage: recording-remask <recording.json>...");
        return ExitCode::from(2);
    }
    for path in paths {
        let path = std::path::PathBuf::from(path);
        let result = std::fs::read_to_string(&path)
            .map_err(|e| e.to_string())
            .and_then(|text| {
                serde_json::from_str::<serde_json::Value>(&text).map_err(|e| e.to_string())
            })
            .and_then(|value| {
                let masked = purlis_recordings::mask::mask(&value);
                let parsed: Recording =
                    serde_json::from_value(masked.clone()).map_err(|e| e.to_string())?;
                let turns = serde_json::to_value(purlis_recordings::turns::of(&parsed))
                    .map_err(|e| e.to_string())?;
                for (file, value) in [
                    (path.clone(), masked),
                    (purlis_recordings::recording::turns_beside(&path), turns),
                ] {
                    let mut text =
                        serde_json::to_string_pretty(&value).map_err(|e| e.to_string())?;
                    text.push('\n');
                    std::fs::write(&file, text).map_err(|e| format!("{}: {e}", file.display()))?;
                }
                Ok(())
            });
        if let Err(e) = result {
            eprintln!("recording-remask: {}: {e}", path.display());
            return ExitCode::from(1);
        }
    }
    ExitCode::SUCCESS
}
