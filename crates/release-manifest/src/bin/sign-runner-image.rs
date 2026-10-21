// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! `sign-runner-image <file> <version>`: signs the runner image's line in `<file>` with the
//! updater's key, `TAURI_SIGNING_PRIVATE_KEY` (and `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`), and
//! writes `<file>.sig` beside it (#2749). Run by `release.yml` in the release environment only.

fn main() -> std::process::ExitCode {
    match run() {
        Ok(path) => {
            println!("{}", path.display());
            std::process::ExitCode::SUCCESS
        }
        Err(why) => {
            eprintln!("sign-runner-image: {why}");
            std::process::ExitCode::FAILURE
        }
    }
}

fn run() -> Result<std::path::PathBuf, String> {
    let mut args = std::env::args().skip(1);
    let (Some(file), Some(version), None) = (args.next(), args.next(), args.next()) else {
        return Err("usage: sign-runner-image <file> <version>".into());
    };
    let line = std::fs::read_to_string(&file).map_err(|why| format!("{file}: {why}"))?;
    let key = std::env::var("TAURI_SIGNING_PRIVATE_KEY")
        .map_err(|_| "TAURI_SIGNING_PRIVATE_KEY is not set, so nothing is signed".to_owned())?;
    let password = std::env::var("TAURI_SIGNING_PRIVATE_KEY_PASSWORD").ok();
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |since| since.as_secs());
    let signature =
        release_manifest::sign::signed(line.trim(), &key, password.as_deref(), &version, now)?;
    let path = std::path::PathBuf::from(format!("{file}.sig"));
    std::fs::write(&path, signature).map_err(|why| format!("{}: {why}", path.display()))?;
    Ok(path)
}
