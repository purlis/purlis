// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use std::path::Path;

use super::*;

const KEY: &str =
    "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAINiGzfjDDJsgYLYCmdkf8Fel9sE6Ma14Y2+c33Y/qZ2n";

fn key() -> HostKey {
    HostKey::parse(KEY).unwrap()
}

/// The value given to `-o name=…` in `args`, where there is one.
fn option<'a>(args: &'a [String], name: &str) -> Option<&'a str> {
    args.windows(2).find_map(|two| {
        (two[0] == "-o")
            .then(|| two[1].strip_prefix(&format!("{name}=")))
            .flatten()
    })
}

#[test]
fn a_route_is_read_and_spelled_back_as_docker_spells_it() {
    for text in [
        "ssh://build-box",
        "ssh://me@build-box",
        "ssh://me@build-box:2222",
        "ssh://me@build-box:2222/var/run/docker.sock",
        "ssh://me@[fd00::1]:2222/run/user/1000/docker.sock",
        "ssh://box/run/podman/podman.sock",
    ] {
        assert_eq!(Route::parse(text).unwrap().spelled(), text);
    }
    let route = Route::parse("ssh://me@build-box:2222/var/run/docker.sock").unwrap();
    assert_eq!(route.user(), Some("me"));
    assert_eq!(route.host(), "build-box");
    assert_eq!(route.port(), Some(2222));
    assert_eq!(route.socket(), Some("/var/run/docker.sock"));
}

#[test]
fn plain_tcp_is_refused_with_the_reason_and_the_ways_forward() {
    let why = Route::parse("tcp://build-box:2375").unwrap_err();
    assert_eq!(why, PLAIN_TCP);
    assert!(why.contains("anyone who can reach that port"), "{why}");
    assert!(why.contains("ssh://") && why.contains("TLS"), "{why}");
}

#[test]
fn a_host_or_user_ssh_could_take_for_an_option_is_refused() {
    for text in [
        "ssh://-oProxyCommand=evil",
        "ssh://-oProxyCommand=evil@box",
        "ssh://me@-box",
        "ssh://me@box lan",
        "ssh://me@box;rm",
        "ssh://",
        "ssh://me@box:0",
        "ssh://me@box:99999",
        "ssh://me@box/relative sock",
        "ssh://me@box/var/run/a:b.sock",
        "ssh://me@box/var/../etc/shadow",
        "ssh://me@box/var/run/$(id).sock",
    ] {
        assert!(Route::parse(text).is_err(), "{text} was read");
    }
}

#[test]
fn a_socket_found_at_connect_is_written_into_the_route() {
    let route = Route::parse("ssh://me@box").unwrap();
    let found = route.with_socket("/run/user/1000/docker.sock").unwrap();
    assert_eq!(found.spelled(), "ssh://me@box/run/user/1000/docker.sock");
    assert!(
        Route::parse("ssh://me@box")
            .unwrap()
            .with_socket("not/absolute")
            .is_err()
    );
}

#[test]
fn a_host_key_is_read_with_its_fingerprint_as_ssh_keygen_prints_it() {
    let key = HostKey::parse(&format!("{KEY} someone@somewhere")).unwrap();
    assert_eq!(key.spelled(), KEY);
    assert_eq!(key.algorithm(), "ssh-ed25519");
    // `ssh-keygen -lf` of the same key.
    assert_eq!(
        key.fingerprint(),
        "SHA256:dmeCC9oOb+c7VYVBSUAdgQchZHZQyXKWgKnGQxfh2rA"
    );
}

#[test]
fn a_key_whose_blob_names_another_algorithm_is_no_key() {
    let (_, blob) = KEY.split_once(' ').unwrap();
    assert!(HostKey::parse(&format!("ssh-rsa {blob}")).is_err());
    assert!(HostKey::parse("ssh-ed25519 not-base64!").is_err());
    assert!(HostKey::parse("ssh-ed25519").is_err());
}

#[test]
fn the_forward_trusts_the_pinned_key_alone_under_the_pools_alias() {
    let route = Route::parse("ssh://me@box:2222/var/run/docker.sock").unwrap();
    let args = forward_args(
        &route,
        "build",
        Path::new("/data/docker-settings/run-x/known_hosts"),
        Path::new("/data/engine-links/build/docker.sock"),
    )
    .unwrap();
    assert_eq!(args[0], "-N");
    assert_eq!(option(&args, "StrictHostKeyChecking"), Some("yes"));
    assert_eq!(
        option(&args, "UserKnownHostsFile"),
        Some("/data/docker-settings/run-x/known_hosts")
    );
    assert_eq!(option(&args, "GlobalKnownHostsFile"), Some("/dev/null"));
    assert_eq!(option(&args, "KnownHostsCommand"), Some("none"));
    assert_eq!(option(&args, "VerifyHostKeyDNS"), Some("no"));
    assert_eq!(option(&args, "UpdateHostKeys"), Some("no"));
    assert_eq!(option(&args, "HostKeyAlias"), Some("purlis-pool-build"));
    assert_eq!(
        known_hosts("build", &key()),
        format!("purlis-pool-build {KEY}\n")
    );
}

#[test]
fn the_forward_shares_no_connection_asks_nothing_and_hands_nothing_on() {
    let route = Route::parse("ssh://me@box/var/run/docker.sock").unwrap();
    let args = forward_args(&route, "build", Path::new("/k"), Path::new("/s")).unwrap();
    for (name, value) in [
        ("ControlMaster", "no"),
        ("ControlPath", "none"),
        ("BatchMode", "yes"),
        ("PasswordAuthentication", "no"),
        ("KbdInteractiveAuthentication", "no"),
        ("ForwardAgent", "no"),
        ("ForwardX11", "no"),
        ("PermitLocalCommand", "no"),
        ("ExitOnForwardFailure", "yes"),
        ("StreamLocalBindUnlink", "yes"),
        ("StreamLocalBindMask", "0177"),
    ] {
        assert_eq!(option(&args, name), Some(value), "{name}");
    }
}

#[test]
fn a_lost_connection_is_noticed_within_a_minute() {
    let route = Route::parse("ssh://me@box/var/run/docker.sock").unwrap();
    let args = forward_args(&route, "build", Path::new("/k"), Path::new("/s")).unwrap();
    let every: u32 = option(&args, "ServerAliveInterval")
        .unwrap()
        .parse()
        .unwrap();
    let count: u32 = option(&args, "ServerAliveCountMax")
        .unwrap()
        .parse()
        .unwrap();
    assert!(every * count <= 60, "{every} × {count}");
}

#[test]
fn the_forward_ends_with_the_socket_pair_and_the_destination_after_its_options() {
    let route = Route::parse("ssh://me@box:2222/run/user/1000/docker.sock").unwrap();
    let args = forward_args(
        &route,
        "build",
        Path::new("/k"),
        Path::new("/data/engine-links/build/docker.sock"),
    )
    .unwrap();
    let tail = &args[args.len() - 6..];
    assert_eq!(
        tail,
        [
            "-p",
            "2222",
            "-L",
            "/data/engine-links/build/docker.sock:/run/user/1000/docker.sock",
            "--",
            "me@box",
        ]
    );
}

#[test]
fn a_route_with_no_socket_yet_opens_no_forward() {
    let route = Route::parse("ssh://me@box").unwrap();
    assert!(forward_args(&route, "build", Path::new("/k"), Path::new("/s")).is_err());
}

#[test]
fn a_first_contact_learns_the_key_and_signs_in_with_nothing() {
    let route = Route::parse("ssh://me@box").unwrap();
    let args = contact_args(&route, Path::new("/run/known_hosts"));
    assert_eq!(option(&args, "StrictHostKeyChecking"), Some("accept-new"));
    assert_eq!(
        option(&args, "UserKnownHostsFile"),
        Some("/run/known_hosts")
    );
    assert_eq!(option(&args, "PubkeyAuthentication"), Some("no"));
    assert_eq!(option(&args, "PasswordAuthentication"), Some("no"));
    assert_eq!(option(&args, "KbdInteractiveAuthentication"), Some("no"));
    assert_eq!(option(&args, "ForwardAgent"), Some("no"));
    assert_eq!(&args[args.len() - 3..], ["--", "me@box", "true"]);
}

#[test]
fn the_key_a_first_contact_met_is_read_from_what_ssh_wrote() {
    let written = format!("purlis-first-contact {KEY}\n");
    assert_eq!(key_met(&written), Some(key()));
    assert_eq!(key_met(""), None);
    assert_eq!(key_met(&format!("another-host {KEY}\n")), None);
}

#[test]
fn the_probe_runs_pinned_and_checks_only_the_socket_a_route_names() {
    let found = Route::parse("ssh://me@box").unwrap();
    let args = probe_args(&found, "build", Path::new("/k"));
    assert_eq!(option(&args, "StrictHostKeyChecking"), Some("yes"));
    assert_eq!(args.last().unwrap(), PROBE);
    let named = Route::parse("ssh://me@box/run/podman/podman.sock").unwrap();
    let args = probe_args(&named, "build", Path::new("/k"));
    assert!(
        args.last()
            .unwrap()
            .starts_with("s=/run/podman/podman.sock;")
    );
}

#[test]
fn the_probe_looks_where_docker_and_podman_keep_their_sockets_in_order() {
    let at = |needle: &str| PROBE.find(needle).unwrap();
    assert!(at("/var/run/docker.sock") < at("$r/docker.sock"));
    assert!(at("$r/docker.sock") < at("/run/podman/podman.sock"));
    assert!(at("/run/podman/podman.sock") < at("$r/podman/podman.sock"));
}

#[test]
fn a_socket_path_too_long_for_this_system_is_refused_with_a_shorter_name_to_choose() {
    let long = Path::new("/a").join("b".repeat(80));
    let why = socket_of(&long, "a-pool-with-a-rather-long-name").unwrap_err();
    assert!(why.contains("too long"), "{why}");
    assert!(socket_of(Path::new("/data/engine-links"), "build").is_ok());
}

#[test]
fn ssh_settings_are_read_from_what_ssh_g_prints() {
    let printed = "user me\nhostname 10.0.0.5\nport 2222\nproxyjump none\n\
                   userknownhostsfile ~/.ssh/known_hosts ~/.ssh/known_hosts2\n";
    let resolved = connect::resolved_from(printed).unwrap();
    assert_eq!(resolved.hostname, "10.0.0.5");
    assert_eq!(resolved.port, 2222);
    assert_eq!(resolved.user, "me");
    assert!(!resolved.proxied);
    assert_eq!(resolved.known_hosts.len(), 2);
    let jumped = connect::resolved_from("hostname box\nproxyjump bastion\n").unwrap();
    assert!(jumped.proxied);
}
