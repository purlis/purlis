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
