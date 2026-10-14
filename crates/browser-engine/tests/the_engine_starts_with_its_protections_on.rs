// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! What the engine is started with: DevTools over the pipe and never a port, its own sandbox
//! on, the reach proxy as its only route with no loopback bypass, no direct UDP and no lookup
//! of its own, password saving, autofill and crash reports off, and none of the host's
//! environment but what it needs.

use std::ffi::{OsStr, OsString};
use std::net::SocketAddr;
use std::path::Path;

use purlis_browser_engine::launch::{Launch, ReachProxy};
use serde_json::Value;

fn launch(data: &Path) -> Launch {
    Launch {
        program: "/opt/engine/chrome".into(),
        data_dir: data.to_owned(),
        reach_proxy: ReachProxy("127.0.0.1:40123".parse::<SocketAddr>().unwrap()),
    }
}

fn arguments(data: &Path) -> Vec<String> {
    launch(data)
        .arguments()
        .into_iter()
        .map(|a| a.into_string().unwrap())
        .collect()
}

#[test]
fn devtools_is_the_pipe_and_never_a_port() {
    let data = tempfile::tempdir().unwrap();
    let args = arguments(data.path());
    assert!(
        args.iter().any(|a| a == "--remote-debugging-pipe"),
        "{args:?}"
    );
    assert!(
        !args.iter().any(|a| a.starts_with("--remote-debugging-port")
            || a.starts_with("--remote-debugging-address")),
        "{args:?}"
    );
}

#[test]
fn the_engines_own_sandbox_stays_on() {
    let data = tempfile::tempdir().unwrap();
    let args = arguments(data.path());
    for off in [
        "--no-sandbox",
        "--disable-setuid-sandbox",
        "--no-zygote",
        "--single-process",
    ] {
        assert!(!args.iter().any(|a| a == off), "{off} in {args:?}");
    }
}

#[test]
fn the_reach_proxy_is_the_only_route_and_loopback_is_not_bypassed() {
    let data = tempfile::tempdir().unwrap();
    let args = arguments(data.path());
    assert!(
        args.iter()
            .any(|a| a == "--proxy-server=http://127.0.0.1:40123"),
        "{args:?}"
    );
    assert!(
        args.iter().any(|a| a == "--proxy-bypass-list=<-loopback>"),
        "{args:?}"
    );
    assert!(
        !args
            .iter()
            .any(|a| a == "--no-proxy-server" || a.starts_with("--proxy-pac-url")),
        "{args:?}"
    );
    // No lookup of the engine's own: every name resolves through the proxy, never beside it.
    assert!(
        args.iter()
            .any(|a| a == "--host-resolver-rules=MAP * ~NOTFOUND , EXCLUDE 127.0.0.1"),
        "{args:?}"
    );
}

#[test]
fn an_ipv6_reach_proxy_is_written_in_brackets() {
    let data = tempfile::tempdir().unwrap();
    let mut launch = launch(data.path());
    launch.reach_proxy = ReachProxy("[::1]:40123".parse().unwrap());
    let args: Vec<String> = launch
        .arguments()
        .into_iter()
        .map(|a| a.into_string().unwrap())
        .collect();
    assert!(
        args.iter()
            .any(|a| a == "--proxy-server=http://[::1]:40123"),
        "{args:?}"
    );
    assert!(
        args.iter()
            .any(|a| a == "--host-resolver-rules=MAP * ~NOTFOUND , EXCLUDE [::1]"),
        "{args:?}"
    );
}

#[test]
fn no_direct_udp_leaves_the_engine() {
    let data = tempfile::tempdir().unwrap();
    let args = arguments(data.path());
    assert!(args.iter().any(|a| a == "--disable-quic"), "{args:?}");
    assert!(
        args.iter()
            .any(|a| a == "--force-webrtc-ip-handling-policy=disable_non_proxied_udp"),
        "{args:?}"
    );
}

#[test]
fn crash_reports_are_off_and_the_engine_has_no_window_and_its_own_data_folder() {
    let data = tempfile::tempdir().unwrap();
    let args = arguments(data.path());
    for on in [
        "--disable-breakpad",
        "--disable-crash-reporter",
        "--headless",
        "--use-mock-keychain",
    ] {
        assert!(args.iter().any(|a| a == on), "{on} missing from {args:?}");
    }
    let folder = format!("--user-data-dir={}", data.path().display());
    assert!(args.contains(&folder), "{args:?}");
    assert_eq!(args.last().map(String::as_str), Some("about:blank"));
}

#[test]
fn password_saving_and_autofill_are_off_in_the_data_folder() {
    let data = tempfile::tempdir().unwrap();
    launch(data.path()).prepare_data_dir().unwrap();
    let prefs: Value =
        serde_json::from_slice(&std::fs::read(data.path().join("Default/Preferences")).unwrap())
            .unwrap();
    assert_eq!(prefs["credentials_enable_service"], false);
    assert_eq!(prefs["profile"]["password_manager_enabled"], false);
    assert_eq!(prefs["autofill"]["profile_enabled"], false);
    assert_eq!(prefs["autofill"]["credit_card_enabled"], false);
}

#[test]
fn a_kept_data_folder_keeps_its_own_preferences_and_still_gets_the_protections() {
    let data = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(data.path().join("Default")).unwrap();
    std::fs::write(
        data.path().join("Default/Preferences"),
        r#"{"homepage":"https://example.com","profile":{"password_manager_enabled":true,"name":"qa"}}"#,
    )
    .unwrap();
    launch(data.path()).prepare_data_dir().unwrap();
    let prefs: Value =
        serde_json::from_slice(&std::fs::read(data.path().join("Default/Preferences")).unwrap())
            .unwrap();
    assert_eq!(prefs["homepage"], "https://example.com");
    assert_eq!(prefs["profile"]["name"], "qa");
    assert_eq!(prefs["profile"]["password_manager_enabled"], false);
    assert_eq!(prefs["credentials_enable_service"], false);
}

#[test]
fn the_engine_gets_only_the_environment_it_needs() {
    let data = tempfile::tempdir().unwrap();
    let host: Vec<(OsString, OsString)> = [
        ("HOME", "/Users/someone"),
        ("PATH", "/usr/bin:/bin"),
        ("LANG", "en_GB.UTF-8"),
        ("TMPDIR", "/tmp/x"),
        ("GITHUB_TOKEN", "not-for-the-engine"),
        ("PURLIS_ROOT", "/plane"),
        ("SSH_AUTH_SOCK", "/tmp/agent.sock"),
        ("http_proxy", "http://elsewhere:3128"),
        ("HTTPS_PROXY", "http://elsewhere:3128"),
    ]
    .into_iter()
    .map(|(k, v)| (k.into(), v.into()))
    .collect();
    let env = launch(data.path()).environment(host);
    let names: Vec<&OsStr> = env.iter().map(|(k, _)| k.as_os_str()).collect();
    for kept in ["HOME", "PATH", "LANG", "TMPDIR"] {
        assert!(
            names.contains(&OsStr::new(kept)),
            "{kept} missing from {names:?}"
        );
    }
    for dropped in [
        "GITHUB_TOKEN",
        "PURLIS_ROOT",
        "SSH_AUTH_SOCK",
        "http_proxy",
        "HTTPS_PROXY",
    ] {
        assert!(
            !names.contains(&OsStr::new(dropped)),
            "{dropped} passed in {names:?}"
        );
    }
}
