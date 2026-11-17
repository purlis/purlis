// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! `sign-runner-binary <file> <version> <target>`: signs a runner platform's `purlis` binary in
//! `<file>` with the updater's key, `TAURI_SIGNING_PRIVATE_KEY` (and
//! `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`), as `purlis-<version>-<target>`, and writes `<file>.sig`
//! beside it (RR-14, #723; ADR 0042 as amended by ADR 0078). `<target>` is `linux-x86_64`,
//! `linux-aarch64` or `macos-arm64`. Run by `release.yml` in the release environment only.

fn main() -> std::process::ExitCode {
    match run() {
        Ok(path) => {
            println!("{}", path.display());
            std::process::ExitCode::SUCCESS
        }
        Err(why) => {
            eprintln!("sign-runner-binary: {why}");
            std::process::ExitCode::FAILURE
        }
    }
}

fn run() -> Result<std::path::PathBuf, String> {
    use purlis_core::runnerinstall::Target;
    let mut args = std::env::args().skip(1);
    let (Some(file), Some(version), Some(target), None) =
        (args.next(), args.next(), args.next(), args.next())
    else {
        return Err("usage: sign-runner-binary <file> <version> <target>".into());
    };
    let target = Target::ALL
        .into_iter()
        .find(|it| it.name() == target)
        .ok_or_else(|| {
            let names: Vec<_> = Target::ALL.iter().map(|it| it.name()).collect();
            format!("{target} is none of {}", names.join(", "))
        })?;
    let bytes = std::fs::read(&file).map_err(|why| format!("{file}: {why}"))?;
    let key = std::env::var("TAURI_SIGNING_PRIVATE_KEY")
        .map_err(|_| "TAURI_SIGNING_PRIVATE_KEY is not set, so nothing is signed".to_owned())?;
    let password = std::env::var("TAURI_SIGNING_PRIVATE_KEY_PASSWORD").ok();
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |since| since.as_secs());
    let signature = release_manifest::sign::signed_runner(
        &bytes,
        target,
        &key,
        password.as_deref(),
        &version,
        now,
    )?;
    let path = std::path::PathBuf::from(format!("{file}.sig"));
    std::fs::write(&path, signature).map_err(|why| format!("{}: {why}", path.display()))?;
    Ok(path)
}
