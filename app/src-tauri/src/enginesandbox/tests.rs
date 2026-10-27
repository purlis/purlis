// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use std::path::Path;

use super::*;

const ENGINES: &str = "/srv/data/engines/chrome-for-testing";

#[test]
fn the_notice_offers_the_profile_command_for_this_machines_engine_and_its_chrome_where_found() {
    let shown = shown(Path::new(ENGINES), Some("/usr/bin/google-chrome".into()));

    assert_eq!(
        shown.command,
        purlis_core::browsers::sandboxing::install_command(Path::new(ENGINES))
    );
    assert_eq!(shown.chrome.as_deref(), Some("/usr/bin/google-chrome"));
    assert!(
        shown.says.contains("never runs it without"),
        "{}",
        shown.says
    );
}

#[test]
fn with_no_chrome_on_this_machine_only_the_profile_is_offered() {
    let shown = shown(Path::new(ENGINES), None);

    assert_eq!(shown.chrome, None);
    assert!(!shown.says.contains("Chrome"), "{}", shown.says);
}

#[test]
fn a_start_the_host_refused_for_the_engines_sandbox_is_told_apart_from_any_other() {
    use purlis_core::browsers::sandboxing::NO_ENGINE_SANDBOX;
    assert!(is_no_sandbox(&format!(
        "the browser did not start: {NO_ENGINE_SANDBOX}"
    )));
    assert!(!is_no_sandbox(
        "the engine started but did not answer over its pipe"
    ));
}

#[test]
fn what_the_chat_is_told_names_no_engine_crash() {
    assert!(NO_BROWSER_HERE.contains("not available on this machine"));
    assert!(!NO_BROWSER_HERE.contains("sandbox!"));
    assert!(!NO_BROWSER_HERE.to_lowercase().contains("zygote"));
}

/// The `.deb` carries the profile the core names, and installs it where the core looks.
#[test]
fn the_deb_installs_the_profile_the_core_reads_where_the_core_reads_it() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let conf: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(manifest.join("tauri.conf.json")).expect("tauri.conf.json"),
    )
    .expect("its JSON");
    let deb = &conf["bundle"]["linux"]["deb"];
    let carried = deb["files"]["/usr/share/purlis/apparmor/purlis-engine"]
        .as_str()
        .expect("the profile in the package");
    assert_eq!(
        std::fs::read_to_string(manifest.join(carried)).expect("the file it carries"),
        purlis_core::browsers::sandboxing::SHIPPED
    );
    let postinst = std::fs::read_to_string(
        manifest.join(deb["postInstallScript"].as_str().expect("its postinst")),
    )
    .expect("the postinst");
    assert!(
        postinst.contains(&format!(
            "PROFILE={}",
            purlis_core::browsers::sandboxing::PROFILE
        )),
        "{postinst}"
    );
    assert!(postinst.contains("SHIPPED=/usr/share/purlis/apparmor/purlis-engine"));
    assert!(
        postinst.contains(purlis_core::browsers::sandboxing::RESTRICTS),
        "only where AppArmor restricts user namespaces"
    );
    let postrm = std::fs::read_to_string(
        manifest.join(deb["postRemoveScript"].as_str().expect("its postrm")),
    )
    .expect("the postrm");
    assert!(postrm.contains("apparmor_parser -R"), "{postrm}");
}

/// The package's postrm, its profiles' folder `apparmor` and `apparmor_parser` a stand-in that
/// writes down what it was asked, run with `action`.
#[cfg(unix)]
fn postrm(action: &str, apparmor: &Path, bin: &Path) -> std::process::ExitStatus {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let script = std::fs::read_to_string(manifest.join("packaging/deb/postrm"))
        .expect("the postrm")
        .replace("/etc/apparmor.d", &apparmor.display().to_string());
    let path = format!(
        "{}:{}",
        bin.display(),
        std::env::var("PATH").unwrap_or_default()
    );
    purlis_core::forklock::status(
        std::process::Command::new("/bin/sh")
            .args(["-c", &script, "postrm", action])
            .env("PATH", path),
    )
    .expect("the postrm runs")
}

/// #2900: removing the package takes the profile it installed; purging it also takes the
/// profile the Notice's command wrote for this machine's engine, which nothing else removes.
/// A file by that name that is not that profile is left alone.
#[cfg(unix)]
#[test]
fn a_remove_takes_the_packages_profile_and_only_a_purge_takes_the_one_the_person_added() {
    use std::os::unix::fs::PermissionsExt as _;
    let dir = tempfile::tempdir().expect("a folder");
    let apparmor = dir.path().join("apparmor.d");
    let bin = dir.path().join("bin");
    std::fs::create_dir_all(&apparmor).expect("its profiles");
    std::fs::create_dir_all(&bin).expect("its programs");
    let asked = dir.path().join("asked");
    let parser = bin.join("apparmor_parser");
    std::fs::write(
        &parser,
        format!("#!/bin/sh\necho \"$@\" >> '{}'\n", asked.display()),
    )
    .expect("the stand-in");
    std::fs::set_permissions(&parser, std::fs::Permissions::from_mode(0o755)).expect("runs");
    let shipped = apparmor.join("purlis-engine");
    let here = apparmor.join("purlis-engine-here");
    let theirs =
        purlis_core::browsers::sandboxing::profile_for(Path::new(ENGINES)).expect("a profile");
    let put_back = || {
        std::fs::write(&shipped, purlis_core::browsers::sandboxing::SHIPPED).expect("shipped");
        std::fs::write(&here, &theirs).expect("theirs");
    };

    put_back();
    assert!(postrm("remove", &apparmor, &bin).success());
    assert!(!shipped.exists(), "the package's own profile goes with it");
    assert!(here.exists(), "a remove keeps what the person added");
    let said = std::fs::read_to_string(&asked).unwrap_or_default();
    assert!(
        !said.contains("purlis-engine-here"),
        "a remove never unloads what the person added: {said}"
    );

    put_back();
    assert!(postrm("purge", &apparmor, &bin).success());
    assert!(!shipped.exists());
    assert!(
        !here.exists(),
        "a purge takes the profile the Notice's command wrote"
    );
    let said = std::fs::read_to_string(&asked).expect("the parser was asked");
    assert!(said.contains(&format!("-R {}", here.display())), "{said}");

    // A file by that name that is not purlis's profile, whole, is not purlis's to remove: one
    // of the person's own, one that starts with purlis's line and allows more, and one naming
    // another program.
    let more = theirs.replace("  userns,\n", "  userns,\n  capability sys_admin,\n");
    let elsewhere = theirs.replace("/chrome-linux64/chrome", "/chrome-linux64/other");
    for other in ["# someone else's\n".to_owned(), more, elsewhere] {
        std::fs::write(&here, &other).expect("another file");
        assert!(postrm("purge", &apparmor, &bin).success());
        assert!(here.exists(), "left as it was: {other}");
    }
}

#[test]
fn a_runners_notice_names_it_and_offers_the_shipped_profiles_command_to_run_there_and_no_chrome() {
    // #2990.
    let shown = shown_on_runner("builds");

    assert_eq!(
        shown.machine,
        EngineMachine::Runner {
            pool: "builds".into()
        }
    );
    // Whether the runner's Linux restricts the sandbox with AppArmor is not known here.
    assert!(
        shown.says.contains("If that runner's Linux"),
        "{}",
        shown.says
    );
    assert!(
        shown.says.contains("the runner of pool builds"),
        "{}",
        shown.says
    );
    assert!(shown.says.contains("not on this machine"), "{}", shown.says);
    assert_eq!(
        shown.command.as_deref(),
        Some(purlis_core::browsers::sandboxing::install_shipped_command().as_str())
    );
    assert_eq!(shown.chrome, None);
}

#[test]
fn this_machines_notice_names_no_runner() {
    assert_eq!(shown(Path::new(ENGINES), None).machine, EngineMachine::Here);
}

#[test]
fn what_a_runner_chat_is_told_names_its_runner_and_no_engine_crash() {
    let told = no_browser_on("builds");
    assert!(
        told.contains("not available on the runner of pool builds"),
        "{told}"
    );
    assert!(!told.contains("this machine"), "{told}");
    assert!(!told.to_lowercase().contains("zygote"));
}
