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

/// What `sh -c command` prints with `env` alone, `PATH` at `path` and `HOME` at `home`: a
/// POSIX login shell handed `command` as ssh hands it one.
fn sh_on_path(
    command: &str,
    home: &Path,
    path: &str,
    env: &[(&str, &str)],
    input: Option<&[u8]>,
) -> (i32, String) {
    let mut run = std::process::Command::new("/bin/sh");
    run.args(["-c", command])
        .env_clear()
        .env("PATH", path)
        .env("HOME", home);
    for (name, value) in env {
        run.env(name, value);
    }
    let ran = crate::boundedrun::run_within_giving(
        &mut run,
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

const PATH: &str = "/usr/bin:/bin:/usr/sbin:/sbin";

/// [`sh_on_path`] with the system's `PATH`.
fn sh(command: &str, home: &Path, env: &[(&str, &str)], input: Option<&[u8]>) -> (i32, String) {
    sh_on_path(command, home, PATH, env, input)
}

/// The hash tool this machine has, as the probe picks it.
fn hash_tool_here() -> HashTool {
    let on_path = |name: &str| {
        PATH.split(':')
            .any(|folder| Path::new(folder).join(name).exists())
    };
    if on_path("sha256sum") {
        HashTool::Sha256sum
    } else {
        HashTool::Shasum
    }
}

fn mode_of(path: &str) -> u32 {
    std::os::unix::fs::PermissionsExt::mode(&std::fs::metadata(path).unwrap().permissions()) & 0o777
}

/// The temporary files in `config`'s server folder.
fn incoming_left(config: &str) -> Vec<String> {
    std::fs::read_dir(server_dir(config))
        .map(|entries| {
            entries
                .filter_map(|entry| entry.ok()?.file_name().into_string().ok())
                .filter(|name| name.starts_with(INCOMING))
                .collect()
        })
        .unwrap_or_default()
}

#[test]
fn every_command_is_handed_to_posix_sh_whatever_the_login_shell_is() {
    let config = "/home/runner/.config/purlis";
    let incoming = "/home/runner/.config/purlis/server/.incoming-AbCdE12345";
    let digest = "a".repeat(64);
    let url = asset_url("1.4.0", Target::LinuxX86_64);
    for command in [
        probe_command(),
        incoming_command(config),
        receive_command(incoming),
        fetch_command(incoming, &url).unwrap(),
        settle_command(HashTool::Sha256sum, config, "1.4.0", incoming, &digest),
        settle_command(HashTool::Shasum, config, "1.4.0", incoming, &digest),
        installed_command(HashTool::Sha256sum, &binary_at(config, "1.4.0"), &digest),
        discard_command(incoming),
    ] {
        let script = command
            .strip_prefix("exec sh -c '")
            .and_then(|rest| rest.strip_suffix('\''))
            .unwrap_or_else(|| panic!("not handed to sh: {command}"));
        // fish reads `\` inside single quotes, and csh reads `!`.
        assert!(!script.contains(['\'', '\\', '!']), "{script}");
    }
}

#[test]
#[should_panic(expected = "quote")]
fn a_script_that_would_end_its_quoting_is_never_sent() {
    let _ = for_login_shell("echo 'x'");
}

#[test]
fn the_probe_names_the_platform_the_store_the_tools_and_each_version_installed() {
    let home = tempfile::tempdir().unwrap();
    let home_path = std::fs::canonicalize(home.path()).unwrap();
    let (code, printed) = sh(&probe_command(), &home_path, &[], None);
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
    let (_, printed) = sh(&probe_command(), &home_path, &[], None);
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
        let (_, printed) = sh(&probe_command(), &home_path, env, None);
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
fn the_probe_deletes_what_a_delivery_cut_off_a_day_ago_left_and_keeps_the_rest() {
    let home = tempfile::tempdir().unwrap();
    let home_path = std::fs::canonicalize(home.path()).unwrap();
    let server = crate::machine::dir(&home_path.join(".config")).join(SERVER_DIR);
    std::fs::create_dir_all(server.join("1.4.0")).unwrap();
    let stale = server.join(".incoming-Stale12345");
    let fresh = server.join(".incoming-Fresh12345");
    let binary = server.join("1.4.0").join(BINARY);
    let two_days_ago = std::time::SystemTime::now() - Duration::from_secs(2 * 24 * 3600);
    for path in [&stale, &fresh, &binary] {
        std::fs::write(path, b"x").unwrap();
    }
    for path in [&stale, &binary] {
        std::fs::File::options()
            .write(true)
            .open(path)
            .unwrap()
            .set_modified(two_days_ago)
            .unwrap();
    }
    let (code, printed) = sh(&probe_command(), &home_path, &[], None);
    assert_eq!(code, 0, "{printed}");
    assert!(!stale.exists(), "a day-old temporary file is deleted");
    assert!(
        fresh.exists(),
        "a delivery that may still be going on is left"
    );
    assert!(binary.exists(), "an installed version is never swept");
}

#[test]
fn the_probe_says_when_wget_is_the_only_way_to_download_and_install_refuses_it() {
    let home = tempfile::tempdir().unwrap();
    let home_path = std::fs::canonicalize(home.path()).unwrap();
    // A PATH with what the probe needs, and wget alone of the two.
    let bin = home_path.join("bin");
    std::fs::create_dir_all(&bin).unwrap();
    let found = |name: &str| {
        PATH.split(':')
            .map(|folder| Path::new(folder).join(name))
            .find(|path| path.exists())
            .unwrap_or_else(|| panic!("no {name} here"))
    };
    for tool in ["sh", "uname", "find"] {
        std::os::unix::fs::symlink(found(tool), bin.join(tool)).unwrap();
    }
    let wget = bin.join("wget");
    std::fs::write(&wget, "#!/bin/sh\nexit 0\n").unwrap();
    std::fs::set_permissions(&wget, std::os::unix::fs::PermissionsExt::from_mode(0o755)).unwrap();
    let (code, printed) = sh_on_path(
        &probe_command(),
        &home_path,
        bin.to_str().unwrap(),
        &[],
        None,
    );
    assert_eq!(code, 0, "{printed}");
    let probed = probed_from(&printed).unwrap();
    assert_eq!(probed.fetch, Fetch::OnlyWget);

    let reach = stub_reach(&home_path, "/bin/sh", PATH, "");
    let key = Key::new();
    let target = probed.target;
    let name = asset_name("1.4.0", target);
    let verified = verify(
        &key.public(),
        BYTES,
        &key.sign(BYTES, "1.4.0", &name),
        "1.4.0",
        target,
    )
    .unwrap();
    let probed = Probed {
        hash: Some(hash_tool_here()),
        ..probed
    };
    let refused = install(&reach.reach, &probed, &verified, Delivery::Download).unwrap_err();
    assert!(matches!(refused, Refused::OnlyWget { .. }), "{refused:?}");
    let said = refused.to_string();
    assert!(said.contains("redirect away from https"), "{said}");
    assert!(said.contains("Install curl"), "{said}");
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
fn an_upload_lands_private_and_not_executable_then_one_command_checks_and_places_it() {
    let home = tempfile::tempdir().unwrap();
    let root = std::fs::canonicalize(home.path()).unwrap();
    let config = root.join("store").display().to_string();
    // An older version, running, which nothing here may touch.
    let older = Path::new(&binary_at(&config, "1.3.0")).to_path_buf();
    std::fs::create_dir_all(older.parent().unwrap()).unwrap();
    std::fs::write(&older, b"older").unwrap();

    let (code, printed) = sh(&incoming_command(&config), &root, &[], None);
    assert_eq!(code, 0);
    let incoming = incoming_from(&config, &printed).expect("its temporary name is said");
    assert_eq!(
        mode_of(&incoming),
        0o600,
        "private, and executable by no one"
    );
    let (code, _) = sh(&receive_command(&incoming), &root, &[], Some(BYTES));
    assert_eq!(code, 0);
    assert_eq!(mode_of(&incoming), 0o600, "still private once filled");
    assert_eq!(std::fs::read(&incoming).unwrap(), BYTES);

    let (code, printed) = sh(
        &settle_command(
            hash_tool_here(),
            &config,
            "1.4.0",
            &incoming,
            &digest_of(BYTES),
        ),
        &root,
        &[],
        None,
    );
    assert_eq!(code, 0, "{printed}");
    let placed = binary_at(&config, "1.4.0");
    assert_eq!(mode_of(&placed), 0o700);
    assert_eq!(std::fs::read(&placed).unwrap(), BYTES);
    assert!(!Path::new(&incoming).exists());
    assert_eq!(
        std::fs::read(&older).unwrap(),
        b"older",
        "the old version is left as it is"
    );
    // The version there, hashed again, is the one verified, and another digest is not.
    let (code, _) = sh(
        &installed_command(hash_tool_here(), &placed, &digest_of(BYTES)),
        &root,
        &[],
        None,
    );
    assert_eq!(code, 0);
    let (code, _) = sh(
        &installed_command(hash_tool_here(), &placed, &"0".repeat(64)),
        &root,
        &[],
        None,
    );
    assert_eq!(code, NOT_THE_SAME);
}

#[test]
fn a_copy_that_is_not_the_one_verified_is_deleted_by_the_command_that_checks_it() {
    let home = tempfile::tempdir().unwrap();
    let root = std::fs::canonicalize(home.path()).unwrap();
    let config = root.join("store").display().to_string();
    let (_, printed) = sh(&incoming_command(&config), &root, &[], None);
    let incoming = incoming_from(&config, &printed).unwrap();
    sh(
        &receive_command(&incoming),
        &root,
        &[],
        Some(b"changed on the way"),
    );
    let (code, printed) = sh(
        &settle_command(
            hash_tool_here(),
            &config,
            "1.4.0",
            &incoming,
            &digest_of(BYTES),
        ),
        &root,
        &[],
        None,
    );
    assert_eq!(code, NOT_THE_SAME);
    assert_eq!(found_in(&printed), digest_of(b"changed on the way"));
    assert!(!Path::new(&incoming).exists(), "deleted");
    assert!(
        !Path::new(&binary_at(&config, "1.4.0")).exists(),
        "nothing placed"
    );
}

#[test]
fn a_discarded_upload_is_gone() {
    let home = tempfile::tempdir().unwrap();
    let root = std::fs::canonicalize(home.path()).unwrap();
    let config = root.join("store").display().to_string();
    let (_, printed) = sh(&incoming_command(&config), &root, &[], None);
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
fn a_download_is_of_the_release_by_curl_over_https_alone_redirects_included() {
    let url = asset_url("1.4.0", Target::LinuxX86_64);
    let incoming = "/s/server/.incoming-AbCdE12345";
    let curl = fetch_command(incoming, &url).unwrap();
    // `-q` is read only as curl's first argument: no `.curlrc` of the person's turns any of it
    // off.
    assert!(curl.contains("curl -q "), "{curl}");
    assert!(curl.contains("--proto =https "), "{curl}");
    assert!(curl.contains("--proto-redir =https "), "{curl}");
    assert!(curl.contains(&url));
    assert!(!curl.contains("wget"));
    for bad in [
        "http://github.com/purlis/purlis/releases/download/v1/x",
        "https://example.com/purlis",
        "https://github.com/purlis/purlis/releases/download/v1/x;rm -rf ~",
        "https://github.com/purlis/purlis/releases/download/../x",
    ] {
        assert!(fetch_command(incoming, bad).is_err(), "{bad}");
    }
}

#[test]
fn what_the_check_found_is_read_only_as_a_digest() {
    let hex = "a".repeat(64);
    assert_eq!(found_in(&format!("found={hex}\n")), hex);
    assert_eq!(found_in(&format!("found={}", hex.to_uppercase())), hex);
    assert_eq!(found_in("found="), "");
    assert_eq!(found_in("found=$(rm)"), "");
    assert_eq!(found_in(""), "");
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

// ---- install(), end to end, through a stub ssh --------------------------------------------

const HOST_KEY: &str =
    "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAINiGzfjDDJsgYLYCmdkf8Fel9sE6Ma14Y2+c33Y/qZ2n";

/// A login shell that is not bash, as a person's may be: fish where it is installed, else zsh,
/// else the system's `sh`.
fn not_bash() -> &'static str {
    [
        "/opt/homebrew/bin/fish",
        "/usr/local/bin/fish",
        "/usr/bin/fish",
        "/bin/zsh",
        "/usr/bin/zsh",
    ]
    .into_iter()
    .find(|shell| Path::new(shell).exists())
    .unwrap_or("/bin/sh")
}

struct StubReach {
    reach: Reach,
    _folder: tempfile::TempDir,
}

/// A [`Reach`] whose `ssh` is a script that runs the command it is handed, as sshd does, with
/// `shell -c <command>`, in an environment of `HOME` at `home` and `PATH` at `path` alone.
/// `before` runs first in the script, with the command in `$last`.
fn stub_reach(home: &Path, shell: &str, path: &str, before: &str) -> StubReach {
    let folder = tempfile::tempdir().unwrap();
    let ssh = folder.path().join("ssh");
    std::fs::write(
        &ssh,
        format!(
            "#!/bin/sh\nfor last; do :; done\n{before}\nexec /usr/bin/env -i HOME={} PATH={path} \
             {shell} -c \"$last\"\n",
            home.display()
        ),
    )
    .unwrap();
    std::fs::set_permissions(&ssh, std::os::unix::fs::PermissionsExt::from_mode(0o755)).unwrap();
    let settings = folder.path().join("settings");
    StubReach {
        reach: Reach {
            ssh,
            route: sshroute::Route::parse("ssh://runner@127.0.0.1:2224").unwrap(),
            pool: "rr14".to_owned(),
            key: sshroute::HostKey::parse(HOST_KEY).unwrap(),
            settings_in: settings,
        },
        _folder: folder,
    }
}

fn verified_here(version: &str) -> Verified {
    let key = Key::new();
    let target = Target::here().expect("a platform purlis runs on as a runner");
    verify(
        &key.public(),
        BYTES,
        &key.sign(BYTES, version, &asset_name(version, target)),
        version,
        target,
    )
    .unwrap()
}

#[test]
fn an_install_through_a_login_shell_that_is_not_bash_places_checks_again_and_replaces_a_changed_copy()
 {
    let home = tempfile::tempdir().unwrap();
    let home = std::fs::canonicalize(home.path()).unwrap();
    let stub = stub_reach(&home, not_bash(), PATH, "");
    let verified = verified_here("1.4.0");

    let probed = probe(&stub.reach).expect("probed");
    assert!(probed.versions.is_empty());
    let installed = install(&stub.reach, &probed, &verified, Delivery::Upload(BYTES)).unwrap();
    let binary = binary_at(&probed.config, "1.4.0");
    assert_eq!(
        installed,
        Installed::Placed {
            binary: binary.clone(),
            // What is placed here is no purlis that could prune: a prune that fails leaves
            // the old versions, and never fails the install.
            pruned: Vec::new(),
        }
    );
    assert_eq!(std::fs::read(&binary).unwrap(), BYTES);
    assert_eq!(mode_of(&binary), 0o700);
    assert!(incoming_left(&probed.config).is_empty());

    // There already, and the file verified: nothing is sent.
    let probed = probe(&stub.reach).unwrap();
    assert_eq!(probed.versions, ["1.4.0"]);
    assert_eq!(
        install(&stub.reach, &probed, &verified, Delivery::Upload(BYTES)).unwrap(),
        Installed::AlreadyThere {
            binary: binary.clone(),
            pruned: Vec::new(),
        }
    );

    // There, but changed since: hashed again, not trusted, and placed again.
    std::fs::write(&binary, b"changed since").unwrap();
    assert_eq!(
        install(&stub.reach, &probed, &verified, Delivery::Upload(BYTES)).unwrap(),
        Installed::Placed {
            binary: binary.clone(),
            pruned: Vec::new(),
        }
    );
    assert_eq!(std::fs::read(&binary).unwrap(), BYTES);
}

#[test]
fn an_install_has_the_installed_version_prune_the_runner_and_says_what_it_removed() {
    let home = tempfile::tempdir().unwrap();
    let home = std::fs::canonicalize(home.path()).unwrap();
    let stub = stub_reach(&home, not_bash(), PATH, "");
    // A "purlis" that answers `bridge --prune` as the runner's does, and says it was run so.
    let script: &[u8] = b"#!/bin/sh\n[ \"$*\" = \"bridge --prune\" ] || exit 9\n\
        echo removed=1.2.0; echo \"kept=1.3.0 serving\"; echo removed=nonsense/\n";
    let key = Key::new();
    let target = Target::here().unwrap();
    let verified = verify(
        &key.public(),
        script,
        &key.sign(script, "1.5.0", &asset_name("1.5.0", target)),
        "1.5.0",
        target,
    )
    .unwrap();
    let probed = probe(&stub.reach).unwrap();
    let installed = install(&stub.reach, &probed, &verified, Delivery::Upload(script)).unwrap();
    assert!(matches!(installed, Installed::Placed { .. }));
    assert_eq!(installed.pruned(), ["1.2.0"]);
    // Checked again on the next install, it prunes again.
    let probed = probe(&stub.reach).unwrap();
    let installed = install(&stub.reach, &probed, &verified, Delivery::Upload(script)).unwrap();
    assert!(matches!(installed, Installed::AlreadyThere { .. }));
    assert_eq!(installed.pruned(), ["1.2.0"]);
    assert_eq!(
        prune(&stub.reach, &probed, "1.5.0; rm -rf /").unwrap_err(),
        Refused::Unusable {
            host: "127.0.0.1".to_owned(),
            why: "1.5.0; rm -rf / isn't a version purlis installs".to_owned(),
        }
    );
    // A `Probed` made by hand with a folder that is no plain path is refused, never run, and
    // never panics on the login shell's check.
    let mut unplain = probed.clone();
    unplain.config = "/home/r/it's".to_owned();
    assert!(matches!(
        prune(&stub.reach, &unplain, "1.5.0").unwrap_err(),
        Refused::Unusable { why, .. } if why.contains("can't prune")
    ));
}

#[test]
fn a_copy_changed_on_the_way_is_refused_and_nothing_is_left() {
    let home = tempfile::tempdir().unwrap();
    let home = std::fs::canonicalize(home.path()).unwrap();
    // Both tools, first on the PATH, report another digest.
    let liar = home.join("liar");
    std::fs::create_dir_all(&liar).unwrap();
    for tool in ["sha256sum", "shasum"] {
        let path = liar.join(tool);
        std::fs::write(
            &path,
            format!("#!/bin/sh\necho \"{}  x\"\n", "0".repeat(64)),
        )
        .unwrap();
        std::fs::set_permissions(&path, std::os::unix::fs::PermissionsExt::from_mode(0o755))
            .unwrap();
    }
    let path = format!("{}:{PATH}", liar.display());
    let stub = stub_reach(&home, not_bash(), &path, "");
    let verified = verified_here("1.4.0");
    let probed = probe(&stub.reach).unwrap();
    let refused = install(&stub.reach, &probed, &verified, Delivery::Upload(BYTES)).unwrap_err();
    let Refused::NotTheSame { found, .. } = &refused else {
        panic!("{refused:?}");
    };
    assert_eq!(found, &"0".repeat(64));
    assert!(
        incoming_left(&probed.config).is_empty(),
        "nothing left behind"
    );
    assert!(!Path::new(&binary_at(&probed.config, "1.4.0")).exists());
}

#[test]
fn a_delivery_cut_off_leaves_nothing_behind() {
    let home = tempfile::tempdir().unwrap();
    let home = std::fs::canonicalize(home.path()).unwrap();
    // The connection drops partway through the upload, as ssh says with 255.
    let stub = stub_reach(
        &home,
        not_bash(),
        PATH,
        "case \"$last\" in *\"cat > \"*) head -c 3 >/dev/null; exit 255;; esac",
    );
    let verified = verified_here("1.4.0");
    let probed = probe(&stub.reach).unwrap();
    let refused = install(&stub.reach, &probed, &verified, Delivery::Upload(BYTES)).unwrap_err();
    assert!(matches!(refused, Refused::Unreached { .. }), "{refused:?}");
    assert!(
        incoming_left(&probed.config).is_empty(),
        "the temporary file is deleted: {:?}",
        incoming_left(&probed.config)
    );
    assert!(!Path::new(&binary_at(&probed.config, "1.4.0")).exists());
}
