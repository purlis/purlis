// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use base64::Engine as _;

use super::*;

fn b64(bytes: &[u8]) -> String {
    base64::engine::general_purpose::STANDARD.encode(bytes)
}

struct Key(minisign::KeyPair);

impl Key {
    fn new() -> Key {
        Key(minisign::KeyPair::generate_unencrypted_keypair().unwrap())
    }

    fn public(&self) -> String {
        b64(self.0.pk.to_box().unwrap().into_string().as_bytes())
    }

    /// A signature of `bytes` as the release makes one for a runner asset: the updater's key,
    /// the version and the asset's name in its trusted comment.
    fn sign(&self, bytes: &[u8], version: &str, file: &str) -> String {
        let trusted = format!("timestamp:1790077121\tfile:{file}\tversion:{version}");
        let signature = minisign::sign(
            None,
            &self.0.sk,
            std::io::Cursor::new(bytes),
            Some(&trusted),
            Some("signature from tauri secret key"),
        )
        .unwrap();
        b64(signature.into_string().as_bytes())
    }
}

const BYTES: &[u8] = b"\x7fELF a runner's purlis";

#[test]
fn an_asset_signed_for_its_version_and_platform_is_verified_with_its_digest() {
    let key = Key::new();
    let signature = key.sign(BYTES, "1.4.0", "purlis-1.4.0-linux-x86_64");
    let verified = verify(
        &key.public(),
        BYTES,
        &signature,
        "1.4.0",
        Target::LinuxX86_64,
    )
    .unwrap();
    assert_eq!(verified.version(), "1.4.0");
    assert_eq!(verified.target(), Target::LinuxX86_64);
    assert_eq!(verified.digest(), digest_of(BYTES));
    assert_eq!(verified.digest().len(), 64);
}

#[test]
fn an_asset_of_another_key_version_platform_or_bytes_is_refused() {
    let key = Key::new();
    let other = Key::new();
    let name = "purlis-1.4.0-linux-x86_64";
    // Another key.
    let by_other = other.sign(BYTES, "1.4.0", name);
    assert!(
        verify(
            &key.public(),
            BYTES,
            &by_other,
            "1.4.0",
            Target::LinuxX86_64
        )
        .is_err()
    );
    // Other bytes than the ones signed.
    let signed = key.sign(BYTES, "1.4.0", name);
    assert!(
        verify(
            &key.public(),
            b"else",
            &signed,
            "1.4.0",
            Target::LinuxX86_64
        )
        .is_err()
    );
    // Signed for another version.
    let older = key.sign(BYTES, "1.3.9", name);
    let refused = verify(&key.public(), BYTES, &older, "1.4.0", Target::LinuxX86_64).unwrap_err();
    assert!(refused.contains("1.3.9"), "{refused}");
    // Signed as another platform's asset: the same version's arm build is not this one.
    let arm = key.sign(BYTES, "1.4.0", "purlis-1.4.0-linux-aarch64");
    assert!(verify(&key.public(), BYTES, &arm, "1.4.0", Target::LinuxX86_64).is_err());
    // A version that could be read as a path is never one.
    assert!(
        verify(
            &key.public(),
            BYTES,
            &signed,
            "../1.4.0",
            Target::LinuxX86_64
        )
        .is_err()
    );
}

#[test]
fn each_platform_has_an_asset_named_and_published_under_its_version() {
    assert_eq!(
        asset_name("1.4.0", Target::MacosArm64),
        "purlis-1.4.0-macos-arm64"
    );
    assert_eq!(
        asset_url("1.4.0", Target::LinuxAarch64),
        "https://github.com/purlis/purlis/releases/download/v1.4.0/purlis-1.4.0-linux-aarch64"
    );
    // A dev build's assets are on the one `dev` release, whose tag moves with every build.
    assert_eq!(
        asset_url("1.4.0-dev.812", Target::LinuxX86_64),
        "https://github.com/purlis/purlis/releases/download/dev/purlis-1.4.0-dev.812-linux-x86_64"
    );
    let names: Vec<_> = Target::ALL.iter().map(|t| t.name()).collect();
    assert_eq!(names, ["linux-x86_64", "linux-aarch64", "macos-arm64"]);
}

#[test]
fn uname_names_the_three_platforms_and_refuses_the_rest_in_words() {
    assert_eq!(Target::of_uname("Linux", "x86_64"), Ok(Target::LinuxX86_64));
    assert_eq!(
        Target::of_uname("Linux", "aarch64"),
        Ok(Target::LinuxAarch64)
    );
    assert_eq!(Target::of_uname("Darwin", "arm64"), Ok(Target::MacosArm64));
    let refused = Target::of_uname("Darwin", "x86_64").unwrap_err();
    assert!(refused.contains("Apple silicon"), "{refused}");
    assert!(Target::of_uname("FreeBSD", "amd64").is_err());
}

#[test]
fn a_version_is_one_a_folder_and_a_command_hold_as_they_are() {
    for good in ["1.4.0", "0.9.1-dev.17", "1.4.0+abc"] {
        assert!(a_version(good), "{good}");
    }
    for bad in ["", "v1.4.0", "1.4.0/x", "1..4", "1.4 0", "1.4.0;rm", "$(x)"] {
        assert!(!a_version(bad), "{bad}");
    }
}

/// What `sh -c script` prints with `env` alone and `HOME` at `home`.
fn sh(script: &str, home: &Path, env: &[(&str, &str)], input: Option<&[u8]>) -> (i32, String) {
    let mut command = std::process::Command::new("/bin/sh");
    command
        .args(["-c", script])
        .env_clear()
        .env("PATH", "/usr/bin:/bin:/usr/sbin:/sbin")
        .env("HOME", home);
    for (name, value) in env {
        command.env(name, value);
    }
    let ran = crate::boundedrun::run_within_giving(
        &mut command,
        input,
        64 * 1024,
        64 * 1024,
        Some(Duration::from_secs(30)),
    )
    .unwrap();
    (
        ran.status.code().unwrap_or(-1),
        String::from_utf8_lossy(&ran.printed).into_owned(),
    )
}

#[test]
fn the_probe_names_the_platform_the_store_the_tools_and_each_version_installed() {
    let home = tempfile::tempdir().unwrap();
    let home_path = std::fs::canonicalize(home.path()).unwrap();
    let (code, printed) = sh(&probe_script(), &home_path, &[], None);
    assert_eq!(code, 0, "{printed}");
    let probed = probed_from(&printed).expect("the probe is read");
    // No folder of purlis's is there yet: the one it would make.
    let config = crate::machine::dir(&home_path.join(".config"));
    assert_eq!(probed.config, config.display().to_string());
    assert_eq!(Some(probed.target), Target::here());
    assert!(probed.hash.is_some(), "{printed}");
    assert!(probed.versions.is_empty());

    // Two versions installed, a folder with no binary, and a binary that is not executable.
    let server = config.join(SERVER_DIR);
    for (version, mode) in [("1.3.0", 0o700), ("1.4.0", 0o700), ("1.5.0", 0o600)] {
        std::fs::create_dir_all(server.join(version)).unwrap();
        let binary = server.join(version).join(BINARY);
        std::fs::write(&binary, b"x").unwrap();
        std::fs::set_permissions(&binary, std::os::unix::fs::PermissionsExt::from_mode(mode))
            .unwrap();
    }
    std::fs::create_dir_all(server.join("1.6.0")).unwrap();
    let (_, printed) = sh(&probe_script(), &home_path, &[], None);
    let probed = probed_from(&printed).unwrap();
    assert_eq!(probed.versions, ["1.3.0", "1.4.0"]);
}

#[test]
fn the_probe_resolves_the_store_as_purlis_does_from_its_variables() {
    let home = tempfile::tempdir().unwrap();
    let home_path = std::fs::canonicalize(home.path()).unwrap();
    let elsewhere = home_path.join("elsewhere");
    let xdg = home_path.join("xdg");
    let case = |env: &[(&str, &str)]| {
        let (_, printed) = sh(&probe_script(), &home_path, env, None);
        probed_from(&printed).unwrap().config
    };
    let named = |root: &Path| crate::machine::dir(root).display().to_string();
    let home_var = crate::machine::HOME_VAR;
    assert_eq!(
        case(&[(home_var, elsewhere.to_str().unwrap())]),
        named(&elsewhere)
    );
    // A blank variable is unset, so the next one is read.
    assert_eq!(
        case(&[(home_var, ""), ("XDG_CONFIG_HOME", xdg.to_str().unwrap())]),
        named(&xdg)
    );
    assert_eq!(
        case(&[("XDG_CONFIG_HOME", xdg.to_str().unwrap())]),
        named(&xdg)
    );
    // The purlis folder, once it is there, wins as it does here.
    std::fs::create_dir_all(xdg.join(crate::names::CONFIG_HOME.write)).unwrap();
    assert_eq!(
        case(&[("XDG_CONFIG_HOME", xdg.to_str().unwrap())]),
        xdg.join(crate::names::CONFIG_HOME.write)
            .display()
            .to_string()
    );
}

#[test]
fn a_store_whose_path_a_shell_would_read_as_its_own_is_refused() {
    let printed = "system=Linux\nmachine=x86_64\nconfig=/home/a b/.config/purlis\nhash=sha256sum\nfetch=none\n";
    let refused = probed_from(printed).unwrap_err();
    assert!(refused.contains("full path"), "{refused}");
    assert!(probed_from("system=Linux\nmachine=x86_64\nconfig=relative\n").is_err());
    assert!(probed_from("nothing").is_err());
}

#[test]
fn an_upload_lands_private_and_not_executable_is_checked_then_placed_beside_the_others() {
    let home = tempfile::tempdir().unwrap();
    let root = std::fs::canonicalize(home.path()).unwrap();
    let config = root.join("store").display().to_string();
    // An older version, running, which nothing here may touch.
    let older = Path::new(&binary_at(&config, "1.3.0")).to_path_buf();
    std::fs::create_dir_all(older.parent().unwrap()).unwrap();
    std::fs::write(&older, b"older").unwrap();

    let (code, printed) = sh(&receive_command(&config), &root, &[], Some(BYTES));
    assert_eq!(code, 0);
    let incoming = incoming_from(&config, &printed).expect("its temporary name is said");
    let mode = std::os::unix::fs::PermissionsExt::mode(
        &std::fs::metadata(&incoming).unwrap().permissions(),
    );
    assert_eq!(mode & 0o777, 0o600, "private, and executable by no one");
    assert_eq!(std::fs::read(&incoming).unwrap(), BYTES);

    let tool = if Path::new("/usr/bin/sha256sum").exists() || Path::new("/bin/sha256sum").exists() {
        HashTool::Sha256sum
    } else {
        HashTool::Shasum
    };
    let (code, printed) = sh(&digest_command(tool, &incoming), &root, &[], None);
    assert_eq!(code, 0);
    assert_eq!(
        digest_from(&printed).as_deref(),
        Some(digest_of(BYTES).as_str())
    );

    let (code, _) = sh(
        &place_command(&config, "1.4.0", &incoming),
        &root,
        &[],
        None,
    );
    assert_eq!(code, 0);
    let placed = binary_at(&config, "1.4.0");
    let mode =
        std::os::unix::fs::PermissionsExt::mode(&std::fs::metadata(&placed).unwrap().permissions());
    assert_eq!(mode & 0o777, 0o700);
    assert_eq!(std::fs::read(&placed).unwrap(), BYTES);
    assert!(!Path::new(&incoming).exists());
    assert_eq!(
        std::fs::read(&older).unwrap(),
        b"older",
        "the old version is left as it is"
    );
}

#[test]
fn a_discarded_upload_is_gone() {
    let home = tempfile::tempdir().unwrap();
    let root = std::fs::canonicalize(home.path()).unwrap();
    let config = root.join("store").display().to_string();
    let (_, printed) = sh(
        &receive_command(&config),
        &root,
        &[],
        Some(b"changed on the way"),
    );
    let incoming = incoming_from(&config, &printed).unwrap();
    let (code, _) = sh(&discard_command(&incoming), &root, &[], None);
    assert_eq!(code, 0);
    assert!(!Path::new(&incoming).exists());
}

#[test]
fn only_a_temporary_name_of_the_store_is_taken_from_what_the_runner_printed() {
    let config = "/home/runner/.config/purlis";
    assert_eq!(
        incoming_from(
            config,
            "/home/runner/.config/purlis/server/.incoming-AbCdE12345\n"
        )
        .as_deref(),
        Some("/home/runner/.config/purlis/server/.incoming-AbCdE12345")
    );
    for bad in [
        "/etc/passwd",
        "/home/runner/.config/purlis/server/.incoming-short",
        "/home/runner/.config/purlis/server/.incoming-AbCdE1234;",
        "/home/runner/.config/purlis/server/1.4.0/purlis",
    ] {
        assert_eq!(incoming_from(config, bad), None, "{bad}");
    }
}

#[test]
fn a_download_is_of_the_release_over_https_alone() {
    let url = asset_url("1.4.0", Target::LinuxX86_64);
    let curl = fetch_command("/s", &url, Fetcher::Curl).unwrap();
    assert!(curl.contains("--proto =https"), "{curl}");
    assert!(curl.contains(&url));
    let wget = fetch_command("/s", &url, Fetcher::Wget).unwrap();
    assert!(wget.contains("--https-only"), "{wget}");
    for bad in [
        "http://github.com/purlis/purlis/releases/download/v1/x",
        "https://example.com/purlis",
        "https://github.com/purlis/purlis/releases/download/v1/x;rm -rf ~",
        "https://github.com/purlis/purlis/releases/download/../x",
    ] {
        assert!(fetch_command("/s", bad, Fetcher::Curl).is_err(), "{bad}");
    }
}

#[test]
fn a_digest_is_read_from_either_tool_and_nothing_else_is_one() {
    let hex = "a".repeat(64);
    assert_eq!(digest_from(&format!("{hex}  /x\n")), Some(hex.clone()));
    assert_eq!(
        digest_from(&format!("{}  /x", hex.to_uppercase())),
        Some(hex)
    );
    assert_eq!(digest_from("sha256sum: /x: No such file"), None);
    assert_eq!(digest_from(""), None);
}

#[test]
fn a_mismatch_is_said_in_words_with_what_to_do() {
    let said = Refused::NotTheSame {
        host: "build-box".into(),
        expected: "a".into(),
        found: "b".into(),
    }
    .to_string();
    assert!(said.contains("deleted"), "{said}");
    assert!(said.contains("Try again"), "{said}");
}

#[test]
fn a_binary_knows_its_version_from_its_folder_in_the_store_and_only_there() {
    let home = tempfile::tempdir().unwrap();
    let root = std::fs::canonicalize(home.path()).unwrap();
    let server = crate::machine::dir(&root).join(SERVER_DIR);
    let installed = server.join("1.4.0").join(BINARY);
    std::fs::create_dir_all(installed.parent().unwrap()).unwrap();
    std::fs::write(&installed, b"x").unwrap();
    assert_eq!(
        version_of_binary(&root, &installed).as_deref(),
        Some("1.4.0")
    );
    // Through a link to it, the same.
    let link = root.join("purlis-link");
    std::os::unix::fs::symlink(&installed, &link).unwrap();
    assert_eq!(version_of_binary(&root, &link).as_deref(), Some("1.4.0"));
    // Anywhere else, or named otherwise, none.
    let elsewhere = root.join("bin").join(BINARY);
    std::fs::create_dir_all(elsewhere.parent().unwrap()).unwrap();
    std::fs::write(&elsewhere, b"x").unwrap();
    assert_eq!(version_of_binary(&root, &elsewhere), None);
    let renamed = server.join("1.4.0").join("other");
    std::fs::write(&renamed, b"x").unwrap();
    assert_eq!(version_of_binary(&root, &renamed), None);
    let unversioned = server.join("latest").join(BINARY);
    std::fs::create_dir_all(unversioned.parent().unwrap()).unwrap();
    std::fs::write(&unversioned, b"x").unwrap();
    assert_eq!(version_of_binary(&root, &unversioned), None);
}
